//! Development probe: Going Commando level moby instances per class (count, first position, pvar index, group,
//! update distance). `cargo run -p rc-formats --example rac2_mobys -- <LEVELn.WAD>`.
use rc_formats::rac2::level::{LevelWad, Part};
use rc_formats::*;
use std::collections::BTreeMap;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("LEVELn.WAD");
    let file = std::fs::read(&path)?;
    let w = LevelWad::parse(&file)?;
    let g = w.part_bytes(Part::Gameplay)?.expect("gameplay part");
    let g = if wad::is_wad(g) { wad::decompress(g)? } else { g.to_vec() };
    let mobys = rac2::gameplay::moby_instances(&g)?;
    let index = w.lump(rac2::level::Lump::CoreIndex)?.expect("core index");
    let data = wad::decompress(w.lump(rac2::level::Lump::CoreData)?.expect("core data"))?;
    let core = level::parse_level_core_for(Game::Rac2, index, data.len())?;
    let ids: Vec<i32> = core.moby_classes.iter().map(|e| e.o_class).collect();
    println!("core moby classes ({}): {ids:?}", ids.len());
    println!("gadget words: {:x?}", core.gadgets.iter().take(16).collect::<Vec<_>>());
    let mut by: BTreeMap<i32, Vec<&rac2::gameplay::Rac2MobyInstance>> = BTreeMap::new();
    for m in &mobys { by.entry(m.base.o_class).or_default().push(m); }
    println!("{} instances, {} classes", mobys.len(), by.len());
    for (c, v) in &by {
        let m = v[0].base;
        let pv = v.iter().filter(|m| m.base.pvar_index >= 0).count();
        println!("class {c:#06x} ({c:4}): {:3} × first at ({:.1}, {:.1}, {:.1}) pvars {pv} group {} upd {} flags {:#x}/{:#x} extra {:x?}",
            v.len(), m.position[0], m.position[1], m.position[2], m.group, m.update_distance, m.spawn_flags, m.mode_bits, v[0].extra);
    }
    for name in ["moby_groups", "shared_data", "pvar_moby_links", "pvar_table", "pvar_data", "pvar_pointer_fixups"] {
        let s = rac2::gameplay::section(&g, name)?;
        println!("{name}: {:?} bytes, first words {:x?}", s.map(<[u8]>::len), s.map(|s| s.chunks(4).take(12).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>()));
    }
    Ok(())
}
