//! A level's in-engine scenes, `G/SCENEn.WAD`: the table at its start and the WAD-compressed chunks after it, the
//! chunks in R&C1's format ([`crate::scene`]: 96 ticks each, the camera records, the actors' streamed sequences and
//! position tracks, the subtitles). The scene start `0x370e08` and the chunk loader `0x3053a0` read the table (the
//! level's copy at `0x151850`, its sector `0x151854`).
//!
//! **Table**: `{s32 size (0x137c), s32 0}`, then 15 records of 0x14c bytes, `{s32 speech sector per language[12],
//! s32 chunk sectors[71]}` (sectors from the WAD's start): a chunk runs to the next entry; the last non-zero entry is
//! the 1-sector sentinel. The speech is a `VAGp` stream.
//!
//! **Ids** (`0x369fe8`): the game numbers its scenes and movies together. 0x20..=0xb0 are in-engine scenes, numbered
//! from each level's base (`$gp − 0x1c00`, [`SCENE_BASES`]: Aranos's 0x21..0x25 are its records 0..4); below 0x20
//! and above 0xb0 movies (a movie row: the id itself, or the id less 0x91).

use crate::buf::{invalid, Result};
use crate::scene::{parse_scene_chunk, Region, Scene};

pub const RECORDS: usize = 15;
pub const RECORD_SIZE: usize = 0x14c;
pub const LANGUAGES: usize = 12;
pub const CHUNK_SLOTS: usize = 71;

/// `$gp − 0x1c00` (`0x1ad3f0`): the first scene id of each level, then the last's end.
pub const SCENE_BASES: [u8; 28] = [
    0x21, 0x26, 0x2b, 0x31, 0x37, 0x44, 0x49, 0x4f, 0x53, 0x5f, 0x68, 0x6b, 0x72, 0x77, 0x7a, 0x81, 0x84, 0x89, 0x8f, 0x96,
    0x9b, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa9,
];

/// Whether `id` is an in-engine scene (`(id − 0x20) < 0x91`), else a movie.
pub fn is_scene(id: i32) -> bool { ((id - 0x20) as u32) < 0x91 }

/// `0x369fe8`: a scene's record in its level's table (the id less the largest base not above it), or a movie's row.
pub fn record_of(id: i32) -> i32 {
    if is_scene(id) {
        let last = SCENE_BASES[27] as i32;
        if last <= id { return id - last; }
        return match SCENE_BASES[..27].iter().rev().find(|&&b| b as i32 <= id) {
            Some(&b) => id - b as i32,
            None => id,
        };
    }
    if 0xb0 < id { id - 0x91 } else { id }
}

/// One record: its speech sectors and its chunk sectors (the sentinel last; empty when unused).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub speech: [u32; LANGUAGES],
    pub chunks: Vec<u32>,
}

impl Record {
    /// The byte range of chunk `i` in the WAD.
    pub fn chunk_bytes(&self, i: usize) -> Option<std::ops::Range<usize>> {
        let (a, b) = (*self.chunks.get(i)?, *self.chunks.get(i + 1)?);
        Some(a as usize * 2048..b as usize * 2048)
    }

    /// Chunks (the sentinel not counted).
    pub fn chunk_count(&self) -> usize { self.chunks.len().saturating_sub(1) }
}

/// The table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneWad {
    pub records: Vec<Record>,
}

impl SceneWad {
    pub fn parse(wad: &[u8]) -> Result<SceneWad> {
        let w = |o: usize| -> Result<u32> {
            match wad.get(o..o + 4) {
                Some(b) => Ok(u32::from_le_bytes(b.try_into().unwrap())),
                None => invalid(format!("SCENE.WAD table: {o:#x} past the end")),
            }
        };
        let size = w(0)? as usize;
        if size < 8 + RECORDS * RECORD_SIZE { return invalid(format!("SCENE.WAD table: size {size:#x}")); }
        let mut records = Vec::with_capacity(RECORDS);
        for r in 0..RECORDS {
            let base = 8 + r * RECORD_SIZE;
            let speech: [u32; LANGUAGES] = std::array::from_fn(|i| w(base + 4 * i).unwrap_or(0));
            let mut chunks = Vec::new();
            for i in 0..CHUNK_SLOTS {
                let s = w(base + 0x30 + 4 * i)?;
                if s == 0 { break; }
                chunks.push(s);
            }
            if chunks.windows(2).any(|p| p[1] <= p[0]) { return invalid(format!("SCENE.WAD record {r}: chunk sectors not increasing")); }
            records.push(Record { speech, chunks });
        }
        Ok(SceneWad { records })
    }

    /// Record `k`'s scene: its chunks decompressed and parsed (NTSC); `None` for an unused record.
    pub fn scene(&self, wad: &[u8], k: usize) -> Result<Option<Scene>> {
        let Some(r) = self.records.get(k) else { return Ok(None) };
        if r.chunk_count() == 0 { return Ok(None); }
        let mut chunks = Vec::with_capacity(r.chunk_count());
        for i in 0..r.chunk_count() {
            let range = r.chunk_bytes(i).expect("chunk");
            let Some(bytes) = wad.get(range.clone()) else { return invalid(format!("SCENE.WAD record {k} chunk {i}: {range:?} past the end")) };
            let dec = crate::wad::decompress(bytes)?;
            chunks.push(parse_scene_chunk(&dec, i, Region::Ntsc)?);
        }
        Ok(Some(Scene::from_chunks(k, Region::Ntsc, chunks)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(record_of(0x21), 0);
        assert_eq!(record_of(0x23), 2);
        assert_eq!(record_of(0x26), 0);
        assert_eq!(record_of(0x16), 0x16);
        assert_eq!(record_of(0xbf), 0x2e);
        assert!(is_scene(0x21) && !is_scene(0x16) && !is_scene(0xb1));
    }
}
