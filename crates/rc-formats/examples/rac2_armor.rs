//! Going Commando's armour rows: `cargo run -p rc-formats --example rac2_armor -- <extracted-rac2>` prints each row's
//! mesh counts (high, low, metal, metal begin) and textures (size); `ARMOR_DUMP=<dir>` writes the textures (PAM).
fn main() -> anyhow::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("the extracted folder"));
    let hdr = std::fs::read(dir.join("RC2.HDR"))?;
    let toc = rc_formats::rac2::toc::Toc::parse(&hdr)?;
    let wad = std::fs::read(dir.join("G/ARMOR.WAD"))?;
    for (i, p) in rc_formats::rac2::armor::armors(&toc)?.into_iter().enumerate() {
        match rc_formats::rac2::armor::read(&wad, p) {
            Ok(a) => {
                if let Ok(out) = std::env::var("ARMOR_DUMP") {
                    for (k, t) in a.textures.iter().enumerate() {
                        let mut f = format!("P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n", t.width, t.height).into_bytes();
                        f.extend_from_slice(&t.rgba);
                        std::fs::write(format!("{out}/armor{i}_{k}.pam"), f)?;
                    }
                }
                println!("row {i}: mesh {:?} bytes {} counts {:?}, textures {:?}", p.mesh, a.mesh.len(), a.mesh.get(..4), a.textures.iter().map(|t| (t.width, t.height)).collect::<Vec<_>>());
            }
            Err(e) => println!("row {i}: {p:?} {e}"),
        }
    }
    if let Ok(h) = toc.header(rc_formats::rac2::toc::Global::Gadget) {
        println!("gadget header: {} bytes", h.len());
        for (i, r) in h.bytes().chunks(0x14).enumerate().take(0x40) {
            let w: Vec<String> = r.chunks(4).map(|c| format!("{:#x}", u32::from_le_bytes(c.try_into().unwrap_or([0; 4])))).collect();
            println!("gadget {i:2}: {}", w.join(" "));
        }
    }
    Ok(())
}
