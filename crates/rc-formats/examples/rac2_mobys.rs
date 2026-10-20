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
    for c in [0, 13, 14, 71, 500, 501, 511] {
        let Some(b) = core.block(&data, &format!("moby_class/{c:04}")) else { continue };
        if let Ok(k) = moby::parse_moby_class_for(Game::Rac2, b) {
            println!("class {c}: mode {:#x} ty {:#x} mode2 {:#x} scale {} coll {:#x} seqs {} joints {}", k.header.mode_bits, k.header.ty, k.header.mode_bits2, k.header.scale, k.header.collision, k.header.sequence_count, k.header.joint_count);
        }
    }
    {
        use rac2::level::Lump;
        let hh = w.lump(Lump::Lump3)?.unwrap_or(&[]);
        let dec = |l: Lump| -> Vec<u8> { w.lump(l).ok().flatten().map(|b| if wad::is_wad(b) { wad::decompress(b).unwrap_or_default() } else { b.to_vec() }).unwrap_or_default() };
        let banks: Vec<Vec<u8>> = vec![dec(Lump::HudHeader), dec(Lump::HudBank0), dec(Lump::HudBank1), dec(Lump::HudBank2), dec(Lump::HudBank3)];
        println!("hud header {} bytes, banks {:?}", hh.len(), banks.iter().map(|b| b.len()).collect::<Vec<_>>());
        match hud::parse_hud(hh, [&banks[0], &banks[1], &banks[2], &banks[3], &banks[4]]) {
            Ok(h) => {
                println!("hud: {} icons, {} frames, {} textures", h.icons.len(), h.frames.len(), h.textures.len());
                for id in [0x7567u16, 0xeaa2, 0xeaa8, 0x301e] { let i = h.icon_index(id); println!("  icon {id:#x}: index {i} frame {:?}", h.frame_size(h.icon_frame(id, 0))); }
            }
            Err(e) => println!("hud parse: {e}"),
        }
    }
    let mut by: BTreeMap<i32, Vec<&rac2::gameplay::Rac2MobyInstance>> = BTreeMap::new();
    for m in &mobys { by.entry(m.base.o_class).or_default().push(m); }
    println!("{} instances, {} classes", mobys.len(), by.len());
    for (c, v) in &by {
        let m = v[0].base;
        let pv = v.iter().filter(|m| m.base.pvar_index >= 0).count();
        println!("class {c:#06x} ({c:4}): {:3} × first at ({:.1}, {:.1}, {:.1}) pvars {pv} group {} upd {} flags {:#x}/{:#x} extra {:x?}",
            v.len(), m.position[0], m.position[1], m.position[2], m.group, m.update_distance, m.spawn_flags, m.mode_bits, v[0].extra);
    }
    let pv = rac2::gameplay::pvar_blocks(&g)?;
    if let Ok(k) = std::env::var("PVAR_CLASS") {
        let k: i32 = k.parse()?;
        for m in mobys.iter().filter(|m| m.base.o_class == k).take(3) {
            let Some(b) = usize::try_from(m.base.pvar_index).ok().and_then(|i| pv.get(i)) else { continue };
            println!("class {k} pvar {} ({:#x} bytes):", m.base.pvar_index, b.len());
            for (r, c) in b.chunks(16).enumerate() {
                let w: Vec<String> = c.chunks(4).map(|x| format!("{:08x}", u32::from_le_bytes(x.try_into().unwrap_or([0; 4])))).collect();
                println!("  +{:03x}: {}", r * 16, w.join(" "));
            }
        }
    }
    for name in ["moby_groups", "shared_data", "pvar_moby_links", "pvar_table", "pvar_data", "pvar_pointer_fixups"] {
        let s = rac2::gameplay::section(&g, name)?;
        println!("{name}: {:?} bytes, first words {:x?}", s.map(<[u8]>::len), s.map(|s| s.chunks(4).take(12).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>()));
    }
    Ok(())
}
