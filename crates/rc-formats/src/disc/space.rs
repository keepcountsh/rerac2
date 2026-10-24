//! `G/SPACE.WAD`: what the level entry (`0x2a3b88`) reads of it for Ratchet's ship.
//!
//! * **Paint** (`0x2ff030`, when `0x1a7af8 >> 16 & 0x1f` changed): row `paint` of the `{sector, sectors}` table at
//!   `SPACE` header + 0xad0 (`0x14fe30`; sectors from the WAD's start) is one PIF (`crate::pif`), uploaded with its
//!   own TEX0 (`0x152c38`). The level load (`0x304660`) gives that TEX0 to every moby class whose core entry has
//!   `+8 ≠ 0` (the ship 0x0d54 and its parts; their texture slots are all 0xff).
//! * **Hull** (`0x2fef68`, `0x1a7af8 & 3`): row `hull` of the same `{sector, sectors}` table at header + 0xab8 is a
//!   whole moby class blob, read into `0x1bf130` and relocated as any class (`0x2ffe70`). The level load (`0x304660`
//!   0x304c14) appends it as class 0x0d54, so the class-id table points the ship's mobys at it and not at the core's
//!   0x0d54; its metal packets get the chrome TEX0 (`0x1b1c70`). Its shine is the ship's (moby +0x63 is only set for a
//!   class with metal packets, `0x3115e0`).

use super::toc::{Global, Part, Toc};
use crate::buf::{Buf, Result};
use crate::texture::Texture;

/// The paint table's offset in the `SPACE` header.
const PAINT_ROWS: usize = 0xad0;
/// The hull table's offset in the `SPACE` header (three rows, up to the paint table).
const HULL_ROWS: usize = 0xab8;
/// The class id the level load gives the hull (`0x304c14`).
pub const HULL_CLASS: i32 = 0x0d54;

/// Paint `paint`'s part of `SPACE.WAD`.
pub(crate) fn paint_part(toc: &Toc, paint: usize) -> Result<Part> {
    let h = toc.header(Global::Space)?;
    let o = PAINT_ROWS + 8 * paint;
    Ok(Part { sector: h.u32(o)?, sectors: h.u32(o + 4)? })
}

/// Hull `hull`'s part of `SPACE.WAD`.
pub(crate) fn hull_part(toc: &Toc, hull: usize) -> Result<Part> {
    if hull >= (PAINT_ROWS - HULL_ROWS) / 8 { return crate::buf::invalid(format!("ship hull {hull}")); }
    let h = toc.header(Global::Space)?;
    let o = HULL_ROWS + 8 * hull;
    Ok(Part { sector: h.u32(o)?, sectors: h.u32(o + 4)? })
}

/// Hull `hull`'s class blob (class [`HULL_CLASS`]), read from `SPACE.WAD`.
pub fn hull(toc: &Toc, wad: &[u8], hull: usize) -> Result<Vec<u8>> {
    let r = hull_part(toc, hull)?.bytes();
    Ok(Buf(wad).sub(r.start, r.len().min(wad.len().saturating_sub(r.start)), "ship hull")?.bytes().to_vec())
}

/// Paint `paint`'s texture, read from `SPACE.WAD`.
pub fn paint(toc: &Toc, wad: &[u8], paint: usize) -> Result<Texture> {
    let r = paint_part(toc, paint)?.bytes();
    let b = Buf(wad).sub(r.start, r.len().min(wad.len().saturating_sub(r.start)), "ship paint")?;
    crate::pif::Pif::parse_at(b.bytes(), 0)?.decode()
}

