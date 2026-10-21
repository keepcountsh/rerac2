//! Going Commando's gadgets: `cargo run -p rc-formats --example rac2_gadget -- <extracted-rac2> <o_class hex>` reads the
//! gadget's entry of `RC2.HDR`'s gadget table (o_class, part A sector / sectors, part B sector / sectors), decompresses
//! part A's class and texture (part B: its sounds) and parses the class.
use rc_formats::rac2::toc::{Global, Toc};
use rc_formats::*;

fn main() -> anyhow::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("extracted folder"));
    let want = i32::from_str_radix(&std::env::args().nth(2).expect("o_class (hex)"), 16)?;
    let hdr = std::fs::read(dir.join("RC2.HDR"))?;
    let toc = Toc::parse(&hdr)?;
    let g = toc.header(Global::Gadget)?;
    let wad_file = std::fs::read(dir.join("G/GADGET.WAD"))?;
    for i in 0..0x30 {
        let o = 8 + 0x14 * i;
        let c = g.i32(o)?;
        if c != want { continue; }
        let w = |k: usize| g.u32(o + 4 * k).map(|v| v as usize);
        let (a, an, bs, bn) = (w(1)?, w(2)?, w(3)?, w(4)?);
        println!("gadget {i}: class {c:#x}, part A sector {a:#x} x{an:#x}, part B sector {bs:#x} x{bn:#x}");
        let b = &wad_file[a * 2048..(a + an) * 2048];
        let u = |k: usize| u32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap()) as usize;
        let (co, cs, to, ts) = (u(0), u(1), u(2), u(3));
        println!("  A header: class at {co:#x} ({cs:#x}), texture at {to:#x} ({ts:#x})");
        let class = wad::decompress(&wad_file[co..co + cs])?;
        println!("  class {} bytes, header {:02x?}", class.len(), &class[..0x40]);
        match moby::parse_moby_class_for(Game::Rac2, &class) {
            Ok(m) => println!("  glow rgba {:#x} first glow high {} low {}, mode {:#x}\n  parsed: high {} low {} metal {} joints {} seqs {} pieces {}", m.header.glow_rgba, m.header.rac1_byte_a, m.header.rac1_byte_b, m.header.mode_bits, m.high_lod.len(), m.low_lod.len(), m.metal.len(), m.header.joint_count, m.sequence_pointers.len(), m.pieces.len()),
            Err(e) => println!("  parse: {e}"),
        }
        if std::env::var("PACKETS").is_ok() {
            let pt = u32::from_le_bytes(class[0..4].try_into().unwrap()) as usize;
            let n = class[4] as usize + class[5] as usize + class[6] as usize;
            for k in 0..n {
                let e = &class[pt + 16 * k..pt + 16 * k + 16];
                println!("  packet {k:2}: {:02x?}", e);
            }
            if let Ok(m) = moby::parse_moby_class_for(Game::Rac2, &class) {
                for (k, sm) in m.high_lod.iter().enumerate() {
                    let (mut lo, mut hi) = ([i16::MAX; 3], [i16::MIN; 3]);
                    for v in &sm.vertices { for (a, x) in [v.x, v.y, v.z].into_iter().enumerate() { lo[a] = lo[a].min(x); hi[a] = hi[a].max(x); } }
                    println!("  high {k:2}: {} verts tex {:?} hdr {:?} range {:?}..{:?}", sm.vertices.len(), sm.texture_indices, sm.vertex_table, lo, hi);
                }
            }
        }
        if ts != 0 {
            let t = wad::decompress(&wad_file[to..to + ts])?;
            let v = |k: usize| u32::from_le_bytes(t[4 * k..4 * k + 4].try_into().unwrap());
            println!("  texture {} bytes, words {:x?}", t.len(), (0..12).map(v).collect::<Vec<_>>());
        }
        let b_part = &wad_file[bs * 2048..(bs + bn) * 2048];
        println!("  B header {:x?}", (0..8).map(|k| u32::from_le_bytes(b_part[4 * k..4 * k + 4].try_into().unwrap())).collect::<Vec<_>>());
        // The class's sound defs (header +0x0d count, +0x28 pointer) and part B as a bank.
        let n = class[0x0d] as usize;
        let ptr = u32::from_le_bytes(class[0x28..0x2c].try_into().unwrap()) as usize;
        for j in 0..n {
            let d: sound_bank::SoundDef = bytemuck::pod_read_unaligned(&class[ptr + 0x20 * j..ptr + 0x20 * (j + 1)]);
            println!("  def {j}: {d:?}");
        }
        if let Some(i) = b_part.windows(4).position(|x| x == b"SBlk") {
            println!("  SBlk at {i:#x}");
            for at in [i.saturating_sub(4), i, i.saturating_sub(8), 0] {
                match sound_bank::parse_bank(&b_part[at..]) {
                    Ok(bank) => { println!("  bank at {at:#x}: {} sounds, {} grains, {} vags", bank.sounds.len(), bank.grains.len(), bank.vags.len()); break; }
                    Err(e) => println!("  bank at {at:#x}: {e}"),
                }
            }
        }
    }
    Ok(())
}
