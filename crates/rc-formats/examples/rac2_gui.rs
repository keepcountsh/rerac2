//! Development probe: Going Commando GUI sprites (G/HUD.WAD's GUI block) by id — vertices, prims, the textures
//! they sample, bones, frames. `cargo run -p rc-formats --example rac2_gui -- <disc folder> [ids…]`.
use rc_formats::rac2::{gui_sprite, toc::Toc};

fn main() -> anyhow::Result<()> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("disc folder"));
    let hdr = std::fs::read(root.join("RC2.HDR"))?;
    let t = Toc::parse(&hdr)?;
    let hud = std::fs::read(root.join("G/HUD.WAD"))?;
    let block = t.gui_block()?;
    let sprites = gui_sprite::parse(&hud[block.bytes()])?;
    let ids: Vec<u32> = std::env::args().skip(2).filter_map(|a| u32::from_str_radix(a.trim_start_matches("0x"), 16).ok()).collect();
    for id in ids {
        let Some(s) = sprites.get(&id) else { println!("{id:#x}: none"); continue };
        let tex: Vec<u8> = s.prims.iter().filter_map(|p| if let gui_sprite::Prim::Texture(t) = p { Some(*t) } else { None }).collect();
        println!("{id:#x}: {} vertices, {} prims, textures {tex:?}, {} textures, {} frames, scale {}", s.vertices.len(), s.prims.len(), s.textures, s.frames.len(), s.scale);
    }
    Ok(())
}
