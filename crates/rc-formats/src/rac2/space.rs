//! `G/SPACE.WAD`: what the level entry (`0x2a3b88`) reads of it for Ratchet's ship.
//!
//! * **Paint** (`0x2ff030`, when `0x1a7af8 >> 16 & 0x1f` changed): row `paint` of the `{sector, sectors}` table at
//!   `SPACE` header + 0xad0 (`0x14fe30`; sectors from the WAD's start) is one PIF (`crate::pif`), uploaded with its
//!   own TEX0 (`0x152c38`). The level load (`0x304660`) gives that TEX0 to every moby class whose core entry has
//!   `+8 ≠ 0` (the ship 0x0d54 and its parts; their texture slots are all 0xff).
//! * **Hull** (`0x2fef68`, `0x1a7af8 & 3`): row `hull` at header + 0xab8 is a mesh for a further class 0x0d54 the
//!   level load appends; not read here.

use super::toc::{Global, Part, Toc};
use crate::buf::{Buf, Result};
use crate::texture::Texture;

/// The paint table's offset in the `SPACE` header.
const PAINT_ROWS: usize = 0xad0;

/// Paint `paint`'s part of `SPACE.WAD`.
pub fn paint_part(toc: &Toc, paint: usize) -> Result<Part> {
    let h = toc.header(Global::Space)?;
    let o = PAINT_ROWS + 8 * paint;
    Ok(Part { sector: h.u32(o)?, sectors: h.u32(o + 4)? })
}

/// Paint `paint`'s texture, read from `SPACE.WAD`.
pub fn paint(toc: &Toc, wad: &[u8], paint: usize) -> Result<Texture> {
    let r = paint_part(toc, paint)?.bytes();
    let b = Buf(wad).sub(r.start, r.len().min(wad.len().saturating_sub(r.start)), "ship paint")?;
    crate::pif::Pif::parse_at(b.bytes(), 0)?.decode()
}
