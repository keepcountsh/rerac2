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
//! * **The ship's scenes** (the boot's `0x294848(lump, fade)` = L00 `0x305690`): row `lump` of the `{sector offset,
//!   sectors}` table at header + 8 (`0x14f368`; the offsets from the WAD's start `0x14f364`) is a scene lump
//!   ([`crate::scene::lump_chunks`]): the take-off (lump 1, 3 with Clank on Ratchet's back) and the landing (0, 2)
//!   the ship mode plays ship-locally (`0x361d08`, `rc_game::travel`).

use super::toc::{Global, Part, Toc};
use crate::buf::{Buf, Result};
use crate::texture::Texture;

/// The paint table's offset in the `SPACE` header.
const PAINT_ROWS: usize = 0xad0;
/// The hull table's offset in the `SPACE` header (three rows, up to the paint table).
const HULL_ROWS: usize = 0xab8;
/// The class id the level load gives the hull (`0x304c14`).
pub const HULL_CLASS: i32 = 0x0d54;
/// The scene lump table's offset in the `SPACE` header (`0x14f368`).
const SCENE_ROWS: usize = 8;
/// The scene lumps the ship mode reads (`0x361d08`): the landing without and with Clank, the take-off without and with.
pub const SCENE_LUMPS: usize = 4;

/// Scene lump `lump`'s part of `SPACE.WAD`.
pub(crate) fn scene_part(toc: &Toc, lump: usize) -> Result<Part> {
    if lump >= SCENE_LUMPS { return crate::buf::invalid(format!("ship scene lump {lump}")); }
    let h = toc.header(Global::Space)?;
    let o = SCENE_ROWS + 8 * lump;
    Ok(Part { sector: h.u32(o)?, sectors: h.u32(o + 4)? })
}

/// Scene lump `lump` of `SPACE.WAD`, its chunks decompressed and parsed (NTSC).
/// The story scenes between the levels (ids 0xa9..=0xb0, `crate::disc::scene::is_shared`): 8 records of the level
/// scene tables' format (`{speech sectors[12], chunk sectors[71]}`) at `SPACE` header + 0x58 (`0x14f3b8`; the chunks'
/// at + 0x88, `0x366150`'s `0x14f3e8`), their sectors from `SPACE.WAD`'s start.
pub fn story_scenes(toc: &Toc) -> Result<crate::disc::scene::SceneWad> {
    let h = toc.header(Global::Space)?;
    crate::disc::scene::SceneWad::from_table(h.bytes(), STORY_ROWS, STORY_SCENES)
}

/// The story scenes' records in the `SPACE` header, and how many (they end where the hull rows start, + 0xab8).
const STORY_ROWS: usize = 0x58;
pub const STORY_SCENES: usize = 8;

pub fn scene(toc: &Toc, wad: &[u8], lump: usize) -> Result<crate::scene::Scene> {
    let r = scene_part(toc, lump)?.bytes();
    if r.is_empty() { return crate::buf::invalid(format!("ship scene lump {lump}: no sectors")); }
    let b = Buf(wad).sub(r.start, r.len().min(wad.len().saturating_sub(r.start)), "ship scene lump")?;
    crate::scene::Scene::from_lump(b.bytes(), lump, crate::scene::Region::Ntsc)
}

/// The flight lumps' table in the `SPACE` header (`0x365b10`: row `kind` of `{sector offset, sectors}` at
/// `0x14f398` = header + 0x38, read to `0x1bf0d8 + 0x100000` and WAD-decompressed to `0x1bf0d8`).
const TRANSITION_ROWS: usize = 0x38;
/// The kinds (`0x367400`'s `0x152c2e`): 0 the usual flight, 1 to or from the orbit levels 0x16 / 0x17, 2 the
/// Jamming Array 0x1a, 3 Todano (0xe) before its flag `0x1395e3`.
pub const TRANSITION_KINDS: usize = 4;
/// The flight's variants (`0x363360`: 0..3 at random, 4 the arrival).
pub const FLIGHT_VARIANTS: usize = 5;

/// A flight lump (the "transition" of the loading flight between planets, `0x365b10` / `0x365960` / `0x363360`),
/// decompressed. Its header is a table of offsets: [1] the base the tables below add; [0..3] and [6..7] its GS
/// textures and texture table, [4..5] its moby classes (the ship actors, relocated by `0x300208`); [8..9] and
/// [0xd..0xf] the GS pointer tables of `0x2fe578`, [0xa..0xb] the font textures (`0x2fe6b0`); [0x12] + [1] its sky
/// block (`0x2fe7d0`, the same layout as a level's); [0x14 + v] the chunk table of variant v (pairs of `{offset,
/// size}`, the chunk data at `base + [0x14 + v] + offset + 0x800`, the level scene chunk format); [0x19..0x1d] the
/// variants' sounds.
pub struct Transition {
    pub kind: usize,
    pub raw: Vec<u8>,
}

/// The flight lump of `kind`, read and decompressed.
pub fn transition(toc: &Toc, wad: &[u8], kind: usize) -> Result<Transition> {
    if kind >= TRANSITION_KINDS { return crate::buf::invalid(format!("flight lump kind {kind}")); }
    let h = toc.header(Global::Space)?;
    let o = TRANSITION_ROWS + 8 * kind;
    let r = Part { sector: h.u32(o)?, sectors: h.u32(o + 4)? }.bytes();
    if r.is_empty() { return crate::buf::invalid(format!("flight lump {kind}: no sectors")); }
    let b = Buf(wad).sub(r.start, r.len().min(wad.len().saturating_sub(r.start)), "flight lump")?;
    let raw = crate::wad::decompress(b.bytes())?;
    if raw.len() < 0x80 { return crate::buf::invalid(format!("flight lump {kind}: {} bytes", raw.len())); }
    Ok(Transition { kind, raw })
}

impl Transition {
    fn word(&self, k: usize) -> Result<usize> {
        let v = Buf(&self.raw).i32(4 * k)?;
        if v < 0 { return crate::buf::invalid(format!("flight lump: header word {k} is {v}")); }
        Ok(v as usize)
    }

    /// The sky block (`0x365b10`: `0x2fe7d0(base + [0x12] + [1])`, the header `0x1b23a0` as a level's sky block,
    /// `crate::sky`): its shells and textures.
    pub fn sky(&self) -> Result<(crate::sky::Sky, Vec<crate::sky::SkyTexture>)> {
        let at = self.word(1)? + self.word(0x12)?;
        let block = Buf(&self.raw).sub(at, self.raw.len() - at.min(self.raw.len()), "flight sky")?.bytes();
        let sky = crate::sky::parse_sky_block(block)?;
        let textures = crate::sky::parse_sky_textures(block, &sky)?;
        Ok((sky, textures))
    }

    /// Variant `v`'s scene (`0x365960`: its chunk table at `base + [0x14 + v]`).
    pub fn variant(&self, v: usize) -> Result<crate::scene::Scene> {
        if v >= FLIGHT_VARIANTS { return crate::buf::invalid(format!("flight variant {v}")); }
        let at = self.word(1)? + self.word(0x14 + v)?;
        let table = Buf(&self.raw).sub(at, self.raw.len() - at.min(self.raw.len()), "flight variant")?.bytes();
        crate::scene::Scene::from_lump(table, v, crate::scene::Region::Ntsc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extracted disc (the repo's `extracted/`), when it is there.
    fn disc() -> Option<(Vec<u8>, Vec<u8>)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted");
        Some((std::fs::read(root.join("RC2.HDR")).ok()?, std::fs::read(root.join("G/SPACE.WAD")).ok()?))
    }

    #[test]
    fn ship_scene_lumps_parse() {
        let Some((hdr, wad)) = disc() else { return };
        let toc = Toc::parse(&hdr).unwrap();
        for lump in 0..SCENE_LUMPS {
            let s = scene(&toc, &wad, lump).unwrap();
            assert!(0 < s.end_tick(), "lump {lump}");
            assert!(s.chunks[0].actors.iter().any(|a| matches!(a.class, 0x215 | 0x10d1 | 0xd54)), "lump {lump}: no ship actor");
        }
    }

    #[test]
    fn flight_lumps_parse() {
        let Some((hdr, wad)) = disc() else { return };
        let toc = Toc::parse(&hdr).unwrap();
        let t = transition(&toc, &wad, 0).unwrap();
        let words: Vec<String> = (0..0x20).map(|k| format!("{:#x}", t.word(k).unwrap_or(usize::MAX))).collect();
        println!("flight lump 0: {} bytes, header {}", t.raw.len(), words.join(" "));
        let at = t.word(1).unwrap() + t.word(0x12).unwrap();
        println!("sky block at {at:#x}: {:02x?}", &t.raw[at..at + 0x40]);
        let (sky, textures) = t.sky().unwrap();
        println!("flight sky: {} shells, {} textures, {} texture defs", sky.shells.len(), textures.len(), sky.texture_defs.len());
        assert!(!sky.shells.is_empty() && !textures.is_empty());
        for v in 0..FLIGHT_VARIANTS {
            let s = t.variant(v).unwrap();
            assert!(0 < s.end_tick(), "variant {v}");
            println!("flight kind 0 variant {v}: {} ticks, {} chunks, actors {:?}", s.end_tick(), s.chunks.len(), s.actor_classes());
        }
        println!("flight sky: {} shells, {} textures", sky.shells.len(), textures.len());
    }
}

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

