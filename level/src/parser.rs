use crate::level::*;
use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use glam::DVec2;
use std::io::{Cursor, Read};
use wad::Archive;

fn parse_texture_name(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_uppercase()
}

pub fn load_level(archive: &Archive, map_name: &str) -> Result<Level> {
    let map_idx = archive
        .find_lump_index(map_name)
        .ok_or_else(|| anyhow!("Map {} not found", map_name))?;

    let things = load_things(&archive.lumps[map_idx + 1].data)?;
    let vertices = load_vertices(&archive.lumps[map_idx + 4].data)?;
    let sectors = load_sectors(&archive.lumps[map_idx + 8].data)?;
    let sidedefs = load_sidedefs(&archive.lumps[map_idx + 3].data)?;
    let linedefs = load_linedefs(&archive.lumps[map_idx + 2].data, &sidedefs)?;
    let segs = load_segs(&archive.lumps[map_idx + 5].data)?;
    let subsectors = load_subsectors(
        &archive.lumps[map_idx + 6].data,
        &segs,
        &linedefs,
        &sidedefs,
    )?;
    let nodes = load_nodes(&archive.lumps[map_idx + 7].data)?;

    Ok(Level {
        vertices,
        sectors,
        sidedefs,
        linedefs,
        segs,
        subsectors,
        nodes,
        things,
        active_doors: Vec::new(),
        active_floors: Vec::new(),
    })
}

fn load_things(data: &[u8]) -> Result<Vec<Thing>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 10;
    let mut things = Vec::with_capacity(count);
    for _ in 0..count {
        let x = cursor.read_i16::<LittleEndian>()?;
        let y = cursor.read_i16::<LittleEndian>()?;
        let angle = cursor.read_i16::<LittleEndian>()?;
        let type_id = cursor.read_i16::<LittleEndian>()?;
        let flags = cursor.read_i16::<LittleEndian>()?;
        things.push(Thing {
            x,
            y,
            angle,
            type_id,
            flags,
        });
    }
    Ok(things)
}

fn load_vertices(data: &[u8]) -> Result<Vec<Vertex>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 4;
    let mut vertices = Vec::with_capacity(count);
    for _ in 0..count {
        let x = cursor.read_i16::<LittleEndian>()? as f64;
        let y = cursor.read_i16::<LittleEndian>()? as f64;
        vertices.push(Vertex {
            p: DVec2::new(x, y),
        });
    }
    Ok(vertices)
}

fn load_sectors(data: &[u8]) -> Result<Vec<Sector>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 26;
    let mut sectors = Vec::with_capacity(count);
    for _ in 0..count {
        let floor_height = cursor.read_i16::<LittleEndian>()? as f64;
        let ceiling_height = cursor.read_i16::<LittleEndian>()? as f64;
        let mut floor_tex = [0u8; 8];
        cursor.read_exact(&mut floor_tex)?;
        let floor_texture = parse_texture_name(&floor_tex);
        let mut ceiling_tex = [0u8; 8];
        cursor.read_exact(&mut ceiling_tex)?;
        let ceiling_texture = parse_texture_name(&ceiling_tex);
        let light_level = cursor.read_i16::<LittleEndian>()?;
        let special = cursor.read_i16::<LittleEndian>()?;
        let tag = cursor.read_i16::<LittleEndian>()?;
        sectors.push(Sector {
            floor_height,
            ceiling_height,
            floor_texture,
            ceiling_texture,
            light_level,
            special,
            tag,
        });
    }
    Ok(sectors)
}

fn load_sidedefs(data: &[u8]) -> Result<Vec<SideDef>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 30;
    let mut sidedefs = Vec::with_capacity(count);
    for _ in 0..count {
        let texture_offset = cursor.read_i16::<LittleEndian>()? as f64;
        let row_offset = cursor.read_i16::<LittleEndian>()? as f64;
        let mut top_tex = [0u8; 8];
        cursor.read_exact(&mut top_tex)?;
        let top_texture = parse_texture_name(&top_tex);
        let mut bottom_tex = [0u8; 8];
        cursor.read_exact(&mut bottom_tex)?;
        let bottom_texture = parse_texture_name(&bottom_tex);
        let mut mid_tex = [0u8; 8];
        cursor.read_exact(&mut mid_tex)?;
        let mid_texture = parse_texture_name(&mid_tex);
        let sector = cursor.read_i16::<LittleEndian>()? as usize;
        sidedefs.push(SideDef {
            texture_offset,
            row_offset,
            top_texture,
            bottom_texture,
            mid_texture,
            sector,
        });
    }
    Ok(sidedefs)
}

fn load_linedefs(data: &[u8], sidedefs: &[SideDef]) -> Result<Vec<LineDef>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 14;
    let mut linedefs = Vec::with_capacity(count);
    for _ in 0..count {
        let v1 = cursor.read_i16::<LittleEndian>()? as usize;
        let v2 = cursor.read_i16::<LittleEndian>()? as usize;
        let flags = cursor.read_u16::<LittleEndian>()?;
        let special = cursor.read_u16::<LittleEndian>()?;
        let tag = cursor.read_i16::<LittleEndian>()?;
        let right_side_idx = cursor.read_i16::<LittleEndian>()?;
        let left_side_idx = cursor.read_i16::<LittleEndian>()?;

        let right_side = if right_side_idx == -1 {
            None
        } else {
            Some(right_side_idx as usize)
        };
        let left_side = if left_side_idx == -1 {
            None
        } else {
            Some(left_side_idx as usize)
        };

        let front_sector = right_side
            .and_then(|idx| sidedefs.get(idx))
            .map(|s| s.sector);
        let back_sector = left_side
            .and_then(|idx| sidedefs.get(idx))
            .map(|s| s.sector);

        linedefs.push(LineDef {
            v1,
            v2,
            flags,
            special,
            tag,
            sidedef: [right_side, left_side],
            sectors: [front_sector, back_sector],
        });
    }
    Ok(linedefs)
}

fn load_segs(data: &[u8]) -> Result<Vec<Seg>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 12;
    let mut segs = Vec::with_capacity(count);
    for _ in 0..count {
        let v1 = cursor.read_u16::<LittleEndian>()? as usize;
        let v2 = cursor.read_u16::<LittleEndian>()? as usize;
        let angle = cursor.read_u16::<LittleEndian>()?;
        let linedef_idx = cursor.read_u16::<LittleEndian>()? as usize;
        let side = cursor.read_u16::<LittleEndian>()? as u8;
        let offset = cursor.read_u16::<LittleEndian>()?;

        segs.push(Seg {
            v1,
            v2,
            angle,
            linedef: Some(linedef_idx),
            side,
            offset,
        });
    }
    Ok(segs)
}

fn load_subsectors(
    data: &[u8],
    segs: &[Seg],
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> Result<Vec<SubSector>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 4;
    let mut subsectors = Vec::with_capacity(count);
    for _ in 0..count {
        let num_segs = cursor.read_u16::<LittleEndian>()?;
        let first_seg = cursor.read_u16::<LittleEndian>()? as usize;

        // Find a valid sector for this subsector
        let mut sector = 0;
        if let Some(first_seg_data) = segs.get(first_seg) {
            if let Some(ld_idx) = first_seg_data.linedef {
                if let Some(ld) = linedefs.get(ld_idx) {
                    let sd_idx = ld.sidedef[first_seg_data.side as usize];
                    if let Some(sd_idx) = sd_idx {
                        if let Some(sd) = sidedefs.get(sd_idx) {
                            sector = sd.sector;
                        }
                    }
                }
            }
        }

        subsectors.push(SubSector {
            num_segs,
            first_seg,
            sector,
        });
    }
    Ok(subsectors)
}

fn load_nodes(data: &[u8]) -> Result<Vec<Node>> {
    let mut cursor = Cursor::new(data);
    let count = data.len() / 28;
    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let x = cursor.read_i16::<LittleEndian>()?;
        let y = cursor.read_i16::<LittleEndian>()?;
        let dx = cursor.read_i16::<LittleEndian>()?;
        let dy = cursor.read_i16::<LittleEndian>()?;

        let mut bbox = [[0i16; 4]; 2];
        for i in 0..4 {
            bbox[0][i] = cursor.read_i16::<LittleEndian>()?;
        }
        for i in 0..4 {
            bbox[1][i] = cursor.read_i16::<LittleEndian>()?;
        }

        let right_child = cursor.read_u16::<LittleEndian>()?;
        let left_child = cursor.read_u16::<LittleEndian>()?;

        nodes.push(Node {
            x,
            y,
            dx,
            dy,
            bbox,
            children: [right_child, left_child],
        });
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le_u16(value: u16, out: &mut Vec<u8>) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn loads_subsector_unsigned_seg_range() -> Result<()> {
        let mut data = Vec::new();
        le_u16(4, &mut data);
        le_u16(0x8001, &mut data);

        let subsectors = load_subsectors(&data, &[], &[], &[])?;

        assert_eq!(subsectors.len(), 1);
        assert_eq!(subsectors[0].num_segs, 4);
        assert_eq!(subsectors[0].first_seg, 0x8001);
        Ok(())
    }

    #[test]
    fn loads_seg_unsigned_indices() -> Result<()> {
        let mut data = Vec::new();
        le_u16(0x8000, &mut data);
        le_u16(0x8001, &mut data);
        le_u16(0x4000, &mut data);
        le_u16(0x8002, &mut data);
        le_u16(1, &mut data);
        le_u16(0x8003, &mut data);

        let segs = load_segs(&data)?;

        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].v1, 0x8000);
        assert_eq!(segs[0].v2, 0x8001);
        assert_eq!(segs[0].linedef, Some(0x8002));
        assert_eq!(segs[0].side, 1);
        assert_eq!(segs[0].offset, 0x8003);
        Ok(())
    }
}
