//! Dev probe: SCENEn.WAD's records and its chunks read with R&C1's chunk parser.
use rc_formats::scene::{parse_scene_chunk, Region};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("SCENEn.WAD");
    let d = std::fs::read(&path)?;
    let w = |o: usize| i32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let size = w(0) as usize;
    let n = (size - 8) / 0x14c;
    for r in 0..n {
        let base = 8 + r * 0x14c;
        let speech: Vec<i32> = (0..12).map(|i| w(base + i * 4)).collect();
        let chunks: Vec<i32> = (0..71).map(|i| w(base + 0x30 + i * 4)).take_while(|&s| s != 0).collect();
        if chunks.is_empty() { continue; }
        println!("record {r}: speech {:?} chunks {:?}", &speech[..speech.iter().rposition(|&s| s != 0).map_or(0, |i| i + 1)], chunks);
        for (i, win) in chunks.windows(2).enumerate().take(std::env::var("CHUNKS").ok().and_then(|s| s.parse().ok()).unwrap_or(1)) {
            let bytes = &d[win[0] as usize * 2048..win[1] as usize * 2048];
            let dec = rc_formats::wad::decompress(bytes)?;
            let h: Vec<String> = dec[..0x20].chunks(4).map(|c| format!("{:08x}", u32::from_le_bytes(c.try_into().unwrap()))).collect();
            println!("  chunk {i}: {} bytes, head {}", dec.len(), h.join(" "));
            match parse_scene_chunk(&dec, i, Region::Ntsc) {
                Ok(c) => {
                    println!("    parsed: end {} actors {:?} cams {} subs {}", c.header.end_tick, c.actors.iter().map(|a| a.class).collect::<Vec<_>>(), c.camera.len(), c.subtitles.len());
                    for a in &c.actors { println!("    actor {:#x}: pos0 {:?} posN {:?} n {}", a.class, a.positions.first(), a.positions.last(), a.positions.len()); }
                    if let Some(k) = c.camera.first() { println!("    cam0 {:?}", k); }
                    for sub in &c.subtitles { println!("    subtitle {}..{}: {:?}", sub.start, sub.end, String::from_utf8_lossy(&sub.text[0])); }
                }
                Err(e) => println!("    R&C1 parser: {e}"),
            }
        }
    }
    Ok(())
}
