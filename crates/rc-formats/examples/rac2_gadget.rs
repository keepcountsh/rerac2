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
            Ok(m) => println!("  parsed: high {} low {} metal {} joints {} seqs {} pieces {}", m.high_lod.len(), m.low_lod.len(), m.metal.len(), m.header.joint_count, m.sequence_pointers.len(), m.pieces.len()),
            Err(e) => println!("  parse: {e}"),
        }
        if ts != 0 {
            let t = wad::decompress(&wad_file[to..to + ts])?;
            let v = |k: usize| u32::from_le_bytes(t[4 * k..4 * k + 4].try_into().unwrap());
            println!("  texture {} bytes, words {:x?}", t.len(), (0..12).map(v).collect::<Vec<_>>());
        }
        let b_part = &wad_file[bs * 2048..(bs + bn) * 2048];
        println!("  B header {:x?}", (0..8).map(|k| u32::from_le_bytes(b_part[4 * k..4 * k + 4].try_into().unwrap())).collect::<Vec<_>>());
    }
    Ok(())
}
