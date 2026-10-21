//! Dev probe: a Going Commando level's core sound bank part read with R&C1's bank reader.
//! `cargo run -p rc-formats --example rac2_sound -- <LEVELn.WAD>`
use rc_formats::rac2::level::{LevelWad, Part};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("LEVELn.WAD");
    let file = std::fs::read(&path)?;
    let w = LevelWad::parse(&file)?;
    let b = w.part_bytes(Part::CoreBank)?.expect("core bank part");
    let h: Vec<String> = b[..0x40].chunks(4).map(|c| format!("{:08x}", u32::from_le_bytes(c.try_into().unwrap()))).collect();
    println!("core bank part: {} bytes, head {}", b.len(), h.join(" "));
    let b = if rc_formats::wad::is_wad(b) { rc_formats::wad::decompress(b)? } else { b.to_vec() };
    match rc_formats::sound_bank::parse_bank(&b) {
        Ok(bank) => println!("bank: {} sounds, {} grains, {} vags, header {:?}", bank.sounds.len(), bank.grains.len(), bank.vags.len(), bank.header),
        Err(e) => {
            println!("R&C1 reader: {e}");
            if let Some(i) = b.windows(4).position(|x| x == b"SBlk") { println!("SBlk at {i:#x}: {:x?}", &b[i..i + 0x40]); }
        }
    }
    let index = w.lump(rc_formats::rac2::level::Lump::CoreIndex)?.expect("core index");
    let data = rc_formats::wad::decompress(w.lump(rc_formats::rac2::level::Lump::CoreData)?.expect("core data"))?;
    let core = rc_formats::level::parse_level_core_for(rc_formats::Game::Rac2, index, data.len())?;
    println!("sound_remap_offset {:#x} moby_sound_remap_offset {:#x}", core.header.sound_remap_offset, core.header.moby_sound_remap_offset);
    match rc_formats::sound_bank::parse_level_sounds(index, &core, &data) {
        Ok(s) => {
            println!("level defs {} map {} classes {}", s.level_defs.len(), s.map.len(), s.classes.len());
            for c in s.classes.iter().filter(|c| !c.defs.is_empty() || !c.bank_ids.is_empty()).take(12) {
                println!("  class {:#06x}: ids {:?} defs {:?}", c.o_class, c.bank_ids, c.defs.iter().map(|d| (d.index, d.near, d.far, d.looped)).collect::<Vec<_>>());
            }
        }
        Err(e) => println!("level sounds: {e}"),
    }
    Ok(())
}
