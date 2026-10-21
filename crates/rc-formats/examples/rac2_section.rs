//! Development probe: a Going Commando gameplay section's bytes as words. `cargo run -p rc-formats --example
//! rac2_section -- <LEVELn.WAD> <section> [bytes]`.
use rc_formats::rac2::level::{LevelWad, Part};
use rc_formats::*;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("LEVELn.WAD");
    let name = args.next().expect("section");
    let max: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(0x400);
    let file = std::fs::read(&path)?;
    let w = LevelWad::parse(&file)?;
    let g = w.part_bytes(Part::Gameplay)?.expect("gameplay part");
    let g = if wad::is_wad(g) { wad::decompress(g)? } else { g.to_vec() };
    if let Some(list) = name.strip_prefix("pvars:") {
        let blocks = rac2::gameplay::pvar_blocks(&g)?;
        for i in list.split(',').filter_map(|v| v.parse::<usize>().ok()) {
            let Some(b) = blocks.get(i) else { continue };
            println!("pvar {i} ({:#x} bytes):", b.len());
            for (r, c) in b.chunks(16).enumerate() {
                let w: Vec<String> = c.chunks(4).map(|x| format!("{:08x}", u32::from_le_bytes(x.try_into().unwrap_or([0; 4])))).collect();
                println!("  +{:03x}: {}", r * 16, w.join(" "));
            }
        }
        return Ok(());
    }
    let Some(s) = rac2::gameplay::section(&g, &name)? else { println!("no section {name}"); return Ok(()) };
    println!("{name}: {:#x} bytes", s.len());
    for (r, c) in s[..s.len().min(max)].chunks(16).enumerate() {
        let w: Vec<String> = c.chunks(4).map(|x| format!("{:08x}", u32::from_le_bytes(x.try_into().unwrap_or([0; 4])))).collect();
        println!("  +{:04x}: {}", r * 16, w.join(" "));
    }
    Ok(())
}
