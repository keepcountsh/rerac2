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
    if std::env::var("STASH").is_ok() {
        let h = &core.header;
        println!("stash list at {:#x}, moby textures {:?}, gs_ram {:?}", h.moby_gs_stash_list, h.moby_textures, h.gs_ram);
        let o = h.moby_gs_stash_list as usize;
        if o > 0 { println!("stash: {:?}", index[o..(o + 64).min(index.len())].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()); }
        for e in core.moby_classes.iter().filter(|e| e.textures[0] == 0xff && (e.unknown_8 != 0 || e.o_class == 0xd54 || e.o_class >= 0x10d0 && e.o_class < 0x10f0)) { println!("no-tex class {:#x} off {:#x} u8 {:#x} uc {:#x}", e.o_class, e.offset_in_asset_wad, e.unknown_8, e.unknown_c); }
    }
    println!("gadget words: {:x?}", core.gadgets.iter().take(16).collect::<Vec<_>>());
    for c in std::env::var("CLASSES").ok().map(|v| v.split(',').filter_map(|x| x.parse::<i32>().ok()).collect::<Vec<_>>()).unwrap_or(vec![0, 13, 14, 71, 500, 501, 511]) {
        let Some(b) = core.block(&data, &format!("moby_class/{c:04}")) else { continue };
        if let Ok(k) = moby::parse_moby_class_for(Game::Rac2, b) {
            println!("class {c}: mode {:#x} ty {:#x} mode2 {:#x} scale {} coll {:#x} seqs {} joints {} glow_a {} glow_b {}", k.header.mode_bits, k.header.ty, k.header.mode_bits2, k.header.scale, k.header.collision, k.header.sequence_count, k.header.joint_count, k.header.rac1_byte_a, k.header.rac1_byte_b);
            if std::env::var("CLASSES").is_ok() {
                for (i, sm) in k.high_lod.iter().enumerate().take(30) {
                    let (mut lo, mut hi) = ([i16::MAX; 3], [i16::MIN; 3]);
                    for v in &sm.vertices { for (a, x) in [v.x, v.y, v.z].into_iter().enumerate() { lo[a] = lo[a].min(x); hi[a] = hi[a].max(x); } }
                    let joints: std::collections::BTreeSet<u8> = sm.vertices.iter().flat_map(|v| v.skin.joints[..v.skin.count.max(1) as usize].to_vec()).collect();
                    println!("  part {i}: {} verts, tex {:?}, range {:?}..{:?}, joints {:?}", sm.vertices.len(), sm.texture_indices, lo, hi, joints);
                }
                println!("  bsphere {:?} lod_trans {}", k.header.bsphere, k.header.lod_trans);
                println!("  joints at {:#x}, retarget map {:?}", k.header.joints, moby_anim::retarget_map(b, &k.header));
                if let Some(e) = core.moby_classes.iter().find(|e| e.o_class == c) { println!("  class textures {:?}", e.textures); }
                let tris: std::collections::BTreeMap<i32, usize> = k.high_lod.iter().flat_map(|s| s.triangles.iter().map(|t| t.texture)).fold(Default::default(), |mut m, t| { *m.entry(t).or_default() += 1; m });
                println!("  triangle textures {tris:?}");
            }
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
                for id in [0x7567u16, 0xeaa2, 0xeaa8, 0x301e, 0xea64] { let i = h.icon_index(id); println!("  icon {id:#x}: index {i} frame {:?}", h.frame_size(h.icon_frame(id, 0))); }
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
    if let Ok(list) = std::env::var("CUBOIDS") {
        let c = rac2::gameplay::volumes(&g, "cuboids")?;
        for i in list.split(',').filter_map(|x| usize::from_str_radix(x.trim_start_matches("0x"), 16).ok()) {
            if let Some(v) = c.get(i) { println!("cuboid {i:#x}: at {:?} extra {:?} rows {:?}", &v.matrix[3][..3], v.extra, &v.matrix[..3]); }
        }
    }
    if std::env::var("SETTINGS").is_ok() {
        if let Some(s) = rac2::gameplay::level_settings(&g)? {
            for (i, c) in s.chunks(4).take(0x20).enumerate() { let b: [u8; 4] = c.try_into().unwrap(); println!("settings[{i:#x}] {:#010x} {}", u32::from_le_bytes(b), f32::from_le_bytes(b)); }
        }
    }
    let pv = rac2::gameplay::pvar_blocks(&g)?;
    if let Ok(k) = std::env::var("PVAR_CLASS") {
        let k: i32 = k.parse()?;
        for m in mobys.iter().filter(|m| m.base.o_class == k).take(std::env::var("PVAR_TAKE").ok().and_then(|s| s.parse().ok()).unwrap_or(3)) {
            let Some(b) = usize::try_from(m.base.pvar_index).ok().and_then(|i| pv.get(i)) else { continue };
            println!("class {k} pvar {} ({:#x} bytes) at {:?}:", m.base.pvar_index, b.len(), m.base.position);
            for (r, c) in b.chunks(16).enumerate() {
                let w: Vec<String> = c.chunks(4).map(|x| format!("{:08x}", u32::from_le_bytes(x.try_into().unwrap_or([0; 4])))).collect();
                println!("  +{:03x}: {}", r * 16, w.join(" "));
            }
        }
    }
    match rac2::gameplay::grind_paths(&g) {
        Ok(p) => println!("grind paths: {} ({:?})", p.len(), p.iter().map(|g| (g.points.len(), g.looped)).collect::<Vec<_>>()),
        Err(e) => println!("grind paths: {e}"),
    }
    for name in ["moby_groups", "shared_data", "pvar_moby_links", "pvar_table", "pvar_data", "pvar_pointer_fixups"] {
        let s = rac2::gameplay::section(&g, name)?;
        println!("{name}: {:?} bytes, first words {:x?}", s.map(<[u8]>::len), s.map(|s| s.chunks(4).take(12).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>()));
    }
    Ok(())
}
