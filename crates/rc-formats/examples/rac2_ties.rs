//! Going Commando's tie colour tables: `cargo run -p rc-formats --example rac2_ties -- <LEVELn.WAD> [classes]` prints,
//! per tie class, the header's colour fields (0x2a remap offsets, 0x30, 0x34 / 0x38 normals, 0x3a size), the vertex
//! counts per LOD and the first bytes at each remap offset.
use rc_formats::rac2::level::{LevelWad, Lump};
use rc_formats::*;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("LEVELn.WAD");
    let take: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let file = std::fs::read(&path)?;
    let w = LevelWad::parse(&file)?;
    let index = w.lump(Lump::CoreIndex)?.unwrap();
    let data = wad::decompress(w.lump(Lump::CoreData)?.unwrap())?;
    let core = level::parse_level_core_for(Game::Rac2, index, data.len())?;
    for e in core.tie_classes.iter().take(take) {
        let Some(b) = core.block(&data, &format!("tie_class/{:04}", e.o_class)) else { continue };
        let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let remap = [u16_at(0x2a), u16_at(0x2c), u16_at(0x2e)];
        println!("class {:#x}: blob {:#x}; 0x24 {:x?} remap {:x?} 0x30 {:#x} normals {:#x} n {} size(0x3a) {} 0x3c {:#x} 0x3e {}",
            e.o_class, b.len(), [u16_at(0x24), u16_at(0x26), u16_at(0x28)], remap, u32_at(0x30), u32_at(0x34), u16_at(0x38), u16_at(0x3a), u16_at(0x3c), u16_at(0x3e));
        match tie::parse_tie_class_for(Game::Rac2, b) {
            Ok(c) => for (l, lod) in c.lods.iter().enumerate() {
                let verts: Vec<usize> = lod.iter().map(|p| p.vertices.len()).collect();
                let df: Vec<(usize, usize)> = lod.iter().map(|p| (p.dinky.len(), p.fat.len())).collect();
                println!("  lod {l}: packets {} verts {:?} (dinky, fat) {:?}", lod.len(), verts, df);
                for p in lod.iter().take(2) { println!("    header {:?}", p.header); }
                if l == 0 { if let Some(p) = lod.first() {
                    let table = c.header.packets[0] as usize;
                    let base = table + p.header.data as usize;
                    let (a, z) = (base + 16 * p.header.color_ofs as usize, base + 16 * p.header.slot_table_ofs as usize);
                    for row in b[a..z].chunks(16) { println!("    colour region {:02x?}", row); }
                } }
            },
            Err(e) => println!("  parse: {e}"),
        }
        for (l, &o) in remap.iter().enumerate() {
            let o = o as usize;
            if o == 0 || o + 48 > b.len() { continue; }
            println!("  remap {l} @ {o:#x}: {:02x?}", &b[o..o + 48]);
        }
        let a = u32_at(0x30) as usize;
        if a != 0 && a + 32 <= b.len() { println!("  0x30 @ {a:#x}: {:02x?}", &b[a..a + 32]); }
    }
    Ok(())
}
