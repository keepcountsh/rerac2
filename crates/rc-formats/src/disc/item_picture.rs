//! A pause page's item picture (`HUD.WAD`, [`super::toc::Toc::item_picture`]): a WAD stream (the Controls pictures:
//! stored as they are) of an 8-bit indexed image (`PIF2`) as `0x306378` uploads it: the width at `+0x08`, the height at `+0x0c`, the 256-entry RGBA32
//! CLUT (CSM1 order) at `+0x20`, the indices at `+0x420`.

use super::Picture;
use crate::buf::{invalid, Buf, Result};
use crate::texture::clut_index;

const CLUT: usize = 0x20;
const PIXELS: usize = 0x420;

/// From the picture's WAD stream or its bytes.
pub fn parse(stream: &[u8]) -> Result<Picture> {
    let d = if crate::wad::is_wad(stream) { crate::wad::decompress(stream)? } else { stream.to_vec() };
    let b = Buf(&d);
    let (w, h) = (b.u32(8)?, b.u32(0xc)?);
    let n = w as usize * h as usize;
    if !(1..=1024).contains(&w) || !(1..=1024).contains(&h) || d.len() < PIXELS + n { return invalid(format!("item picture: {w}×{h} in {:#x} bytes", d.len())); }
    let clut = &d[CLUT..CLUT + 1024];
    let rgba = d[PIXELS..PIXELS + n].iter().flat_map(|&i| { let o = clut_index(i as u32) as usize * 4; [clut[o], clut[o + 1], clut[o + 2], clut[o + 3]] }).collect();
    Ok(Picture { width: w, height: h, rgba })
}
