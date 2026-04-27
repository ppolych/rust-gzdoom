use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

pub mod parser;

pub use parser::{Archive, Lump};

#[derive(Debug, Clone)]
pub struct Patch {
    pub width: u16,
    pub height: u16,
    pub left_offset: i16,
    pub top_offset: i16,
    pub columns: Vec<Vec<Post>>,
}

#[derive(Debug, Clone)]
pub struct Post {
    pub top_delta: u8,
    pub length: u8,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub patches: Vec<TexturePatch>,
}

#[derive(Debug, Clone)]
pub struct TexturePatch {
    pub origin_x: i16,
    pub origin_y: i16,
    pub patch_idx: usize,
}

pub struct Palette {
    pub colors: [[u8; 3]; 256],
}

impl Palette {
    pub fn from_lump(data: &[u8]) -> Self {
        let mut colors = [[0u8; 3]; 256];
        for i in 0..256 {
            let base = i * 3;
            if base + 2 < data.len() {
                colors[i][0] = data[base];
                colors[i][1] = data[base + 1];
                colors[i][2] = data[base + 2];
            }
        }
        Self { colors }
    }
}

impl Archive {
    pub fn find_lump_index(&self, name: &str) -> Option<usize> {
        self.lumps.iter().position(|l| l.name == name)
    }

    pub fn find_lumps_in_range(&self, start_marker: &str, end_marker: &str) -> Vec<usize> {
        let start = self.find_lump_index(start_marker);
        let end = self.find_lump_index(end_marker);
        if let (Some(s), Some(e)) = (start, end) {
            if s < e {
                return (s + 1..e).collect();
            }
        }
        Vec::new()
    }

    pub fn get_lump_data(&self, name: &str) -> Result<&[u8]> {
        for lump in &self.lumps {
            if lump.name == name {
                return Ok(&lump.data);
            }
        }
        Err(anyhow!("Lump {} not found", name))
    }

    pub fn load_patch(&self, name: &str) -> Result<Patch> {
        let data = self.get_lump_data(name)?;
        let mut cursor = Cursor::new(data);

        let width = cursor.read_u16::<LittleEndian>()?;
        let height = cursor.read_u16::<LittleEndian>()?;
        let left_offset = cursor.read_i16::<LittleEndian>()?;
        let top_offset = cursor.read_i16::<LittleEndian>()?;

        let mut column_offsets = Vec::with_capacity(width as usize);
        for _ in 0..width {
            column_offsets.push(cursor.read_u32::<LittleEndian>()?);
        }

        let mut columns = Vec::with_capacity(width as usize);
        for offset in column_offsets {
            cursor.set_position(offset as u64);
            let mut posts = Vec::new();
            loop {
                let top_delta = cursor.read_u8()?;
                if top_delta == 0xFF {
                    break;
                }
                let length = cursor.read_u8()?;
                cursor.read_u8()?; // padding byte
                let mut pixels = vec![0u8; length as usize];
                cursor.read_exact(&mut pixels)?;
                cursor.read_u8()?; // padding byte
                posts.push(Post {
                    top_delta,
                    length,
                    pixels,
                });
            }
            columns.push(posts);
        }

        Ok(Patch {
            width,
            height,
            left_offset,
            top_offset,
            columns,
        })
    }

    pub fn load_textures(&self) -> Result<(Vec<Texture>, Vec<String>)> {
        let pnames_data = self.get_lump_data("PNAMES")?;
        let mut pnames_cursor = Cursor::new(pnames_data);
        let num_pnames = pnames_cursor.read_u32::<LittleEndian>()?;
        let mut pnames = Vec::with_capacity(num_pnames as usize);
        for _ in 0..num_pnames {
            let mut name_bytes = [0u8; 8];
            pnames_cursor.read_exact(&mut name_bytes)?;
            let name = String::from_utf8_lossy(&name_bytes)
                .trim_end_matches('\0')
                .to_uppercase();
            pnames.push(name);
        }

        let mut textures = Vec::new();
        for lump_name in &["TEXTURE1", "TEXTURE2"] {
            if let Ok(data) = self.get_lump_data(lump_name) {
                let mut cursor = Cursor::new(data);
                let num_textures = cursor.read_u32::<LittleEndian>()?;
                let mut offsets = Vec::with_capacity(num_textures as usize);
                for _ in 0..num_textures {
                    offsets.push(cursor.read_u32::<LittleEndian>()?);
                }

                for offset in offsets {
                    cursor.set_position(offset as u64);
                    let mut name_bytes = [0u8; 8];
                    cursor.read_exact(&mut name_bytes)?;
                    let name = String::from_utf8_lossy(&name_bytes)
                        .trim_end_matches('\0')
                        .to_uppercase();

                    cursor.read_u32::<LittleEndian>()?; // masked/unknown
                    let width = cursor.read_u16::<LittleEndian>()?;
                    let height = cursor.read_u16::<LittleEndian>()?;
                    cursor.read_u32::<LittleEndian>()?; // column_directory/unknown

                    let num_patches = cursor.read_u16::<LittleEndian>()?;
                    let mut patches = Vec::with_capacity(num_patches as usize);
                    for _ in 0..num_patches {
                        let origin_x = cursor.read_i16::<LittleEndian>()?;
                        let origin_y = cursor.read_i16::<LittleEndian>()?;
                        let patch_idx = cursor.read_u16::<LittleEndian>()? as usize;
                        cursor.read_u16::<LittleEndian>()?; // stepdir
                        cursor.read_u16::<LittleEndian>()?; // colormap
                        patches.push(TexturePatch {
                            origin_x,
                            origin_y,
                            patch_idx,
                        });
                    }
                    textures.push(Texture {
                        name,
                        width,
                        height,
                        patches,
                    });
                }
            }
        }

        Ok((textures, pnames))
    }

    pub fn load_flats(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let mut flat_lumps = self.find_lumps_in_range("F_START", "F_END");
        flat_lumps.extend(self.find_lumps_in_range("FF_START", "FF_END"));

        let mut flats = Vec::new();
        for &idx in &flat_lumps {
            let lump = &self.lumps[idx];
            if lump.data.len() == 64 * 64 {
                flats.push((lump.name.clone(), lump.data.clone()));
            }
        }
        Ok(flats)
    }
}
