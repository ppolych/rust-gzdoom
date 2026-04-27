use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub struct Lump {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Archive {
    pub lumps: Vec<Lump>,
}

impl Archive {
    pub fn load_wad(path: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut header = [0u8; 12];
        file.read_exact(&mut header)?;
        let magic = &header[0..4];
        if magic != b"IWAD" && magic != b"PWAD" {
            return Err(anyhow!("Invalid WAD magic"));
        }
        let num_lumps = (&header[4..8]).read_u32::<LittleEndian>()?;
        let info_table_ofs = (&header[8..12]).read_u32::<LittleEndian>()?;

        file.seek(SeekFrom::Start(info_table_ofs as u64))?;
        let mut lumps = Vec::with_capacity(num_lumps as usize);
        for _ in 0..num_lumps {
            let mut entry = [0u8; 16];
            file.read_exact(&mut entry)?;
            let filepos = (&entry[0..4]).read_u32::<LittleEndian>()?;
            let size = (&entry[4..8]).read_u32::<LittleEndian>()?;
            let name_bytes = &entry[8..16];
            let name = name_bytes
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect::<String>();

            let file_pos_after_entry = file.stream_position()?;

            let mut data = vec![0u8; size as usize];
            file.seek(SeekFrom::Start(filepos as u64))?;
            file.read_exact(&mut data)?;

            lumps.push(Lump { name, data });
            file.seek(SeekFrom::Start(file_pos_after_entry))?;
        }
        Ok(Archive { lumps })
    }
}
