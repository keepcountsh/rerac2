//! `G/GADGET.WAD`: the hand gadgets and weapons (the Lancer, class 0x5d2, …), streamed one at a time by L00 `0x306018`
//! into one of three 0xc800-byte slots. `RC2.HDR`'s gadget header: `s32 size`, a word, then 0x30 entries of `{o_class,
//! part A sector, part A sectors, part B sector, part B sectors}` (sectors from the file's start; `0x150080`).
//!
//! * **Part A** (`0x305d50`): `{class offset, class size, texture offset, texture size}` (byte offsets in the file),
//!   each a WAD stream: the moby class blob (Going Commando's format) and its texture list (as the armours',
//!   [`crate::rac2::armor::read_textures`]; size 0: none).
//! * **Part B** (`0x305810`): the gadget's sound bank (an `SBlk` after a small header).

use super::toc::{Global, Toc};
use crate::buf::{invalid, Buf, Result};
use crate::texture::Texture;

/// One entry of the gadget header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GadgetEntry {
    pub o_class: i32,
    pub a: (usize, usize),
    pub b: (usize, usize),
}

/// The gadget header's entries (class 0: unused).
pub fn gadgets(toc: &Toc) -> Result<Vec<GadgetEntry>> {
    let h = toc.header(Global::Gadget)?;
    let n = (h.len().saturating_sub(8)) / 0x14;
    (0..n)
        .map(|i| {
            let o = 8 + 0x14 * i;
            let w = |k: usize| -> Result<usize> { Ok(h.u32(o + 4 * k)? as usize) };
            Ok(GadgetEntry { o_class: h.i32(o)?, a: (w(1)?, w(2)?), b: (w(3)?, w(4)?) })
        })
        .collect()
}

/// A gadget's class blob and textures (part A).
pub struct Gadget {
    pub class: Vec<u8>,
    pub textures: Vec<Texture>,
}

/// Part A of `e` from `GADGET.WAD`'s bytes.
pub fn read(wad: &[u8], e: &GadgetEntry) -> Result<Gadget> {
    let f = Buf(wad);
    let at = e.a.0 * 2048;
    let w = |k: usize| -> Result<usize> { Ok(f.u32(at + 4 * k)? as usize) };
    let (co, cs, to, ts) = (w(0)?, w(1)?, w(2)?, w(3)?);
    if cs == 0 { return invalid(format!("gadget {:#x}: no class", e.o_class)); }
    let class = crate::wad::decompress(f.sub(co, cs, "gadget class")?.bytes())?;
    let textures = if ts == 0 { Vec::new() } else { super::armor::read_textures(&crate::wad::decompress(f.sub(to, ts, "gadget textures")?.bytes())?)? };
    Ok(Gadget { class, textures })
}
