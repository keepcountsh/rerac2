//! In-engine scenes (cutscenes): the level header's scene table and the WAD-compressed scene chunks.
//! Spec: docs/plan/cutscenes_transitions.md §1 (table), §2 (chunk format); played by
//! `rc_game::scene_player` (§3).
//!
//! **Table** (level header +0x184, [`crate::toc::SceneRecord`]): 15 records of 0x250 bytes, each
//! `{speech VAG sector per language[6], NTSC chunk sectors[71], PAL chunk sectors[71]}` (`fun_00215970`,
//! `FUN_002591d0`). A chunk's size is the sector difference to the next entry; the last used entry is a
//! 1-sector sentinel (zeros). The chunks of a region are contiguous, so the extractor writes the region as
//! one file, `levels/NN/scene/KK_ntsc.bin` / `_pal.bin` (chunks + sentinel), and the speech as
//! `levels/NN/speech/KK_<lang>.bin`.
//!
//! **Chunk** (decompressed into 0x16cd38 and parsed by `FUN_00259288`): 96 ticks on NTSC (80 PAL).
//! Header `{s16 end tick, s16 0, s32 subtitle table offset (< 0x400 = none), s16 audio start tick,
//! s16 −1, u16 actor count, u16 0, s32 camera table offset, s32 actor offsets[count]}`. The camera table
//! has one 0x20-byte [`CamRecord`] per tick plus one (the last equals the next chunk's first). An actor
//! record is `{s32 class, s32 ?, s32 0, s32 position track offset}` followed by a standard moby sequence
//! (header at +0x10, frame offsets relative to +0x10) whose frames run at 30 Hz; the position track is
//! one vec4 per frame. Subtitles are 16-byte `{s16 start, s16 end, s16 text[5] (En Fr De Es It, offsets
//! from the table start), s16 0}` ended by start = −1, then the Latin-1 strings.

use crate::buf::{invalid, Buf, Result};
use crate::moby_anim::{parse_sequence, MobySequence};
use crate::toc::SCENE_RECORDS;
use bytemuck::{Pod, Zeroable};

/// Ticks per chunk: NTSC 96 (0x60), PAL 80 (0x50) (`CutsceneModeUpdate` 0x2aca80).
pub(crate) const NTSC_TICKS_PER_CHUNK: u32 = 96;
pub(crate) const PAL_TICKS_PER_CHUNK: u32 = 80;
/// Subtitle table offsets below this mean "no table" (`FUN_00259288`).
pub(crate) const SUBTITLE_MIN_OFFSET: i32 = 0x400;
/// Subtitle languages in table order: En, Fr, De, Es, It.
pub(crate) const SUBTITLE_LANGUAGES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Region {
    Ntsc,
    Pal,
}

impl Region {
    pub(crate) fn ticks_per_chunk(self) -> u32 { if self == Region::Pal { PAL_TICKS_PER_CHUNK } else { NTSC_TICKS_PER_CHUNK } }
}

const _: () = assert!(SCENE_RECORDS == 15);

// ---------------------------------------------------------------------------------------------------
// Chunks

/// The 0x14-byte chunk header before the actor offsets.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct ChunkHeader {
    /// +0x00: scene end tick (the same in every chunk) → 0x16cd20.
    pub(crate) end_tick: i16,
    /// +0x02: 0 on the disc.
    pub(crate) unknown_02: i16,
    /// +0x04: subtitle table offset; < 0x400 = none → 0x16cd2c.
    pub(crate) subtitle_offset: i32,
    /// +0x08: audio start tick → 0x16cd28 (−6 on Novalis; the start overwrites it with −3).
    pub(crate) audio_start: i16,
    /// +0x0a: −1 on the disc.
    pub(crate) unknown_0a: i16,
    /// +0x0c: actor count → 0x16cd24.
    pub(crate) actor_count: u16,
    /// +0x0e: 0 on the disc.
    pub(crate) unknown_0e: u16,
    /// +0x10: camera table offset → 0x16cd34.
    pub(crate) camera_offset: i32,
}
const _: () = assert!(std::mem::size_of::<ChunkHeader>() == 0x14);

/// One camera record (one per 60 Hz tick), `FUN_002ac8d8`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CamRecord {
    /// +0x00: eye position → 0x167240.
    pub eye: [f32; 3],
    /// +0x0c: the byte is the cut flag (returned); the whole word is copied to 0x16724c.
    pub(crate) cut: u32,
    /// +0x10 / +0x14 / +0x18: angles (rad) for `sceVu0RotMatrixX` / `Y` / `Z` (rc_game::scene_player).
    pub angles: [f32; 3],
    /// +0x1c: tan(hfov / 2) → 0x16cf70.
    pub tan_half_fov: f32,
}
const _: () = assert!(std::mem::size_of::<CamRecord>() == 0x20);

impl CamRecord {
    /// The cut flag (byte +0xc): snap actors across the cut on odd ticks.
    pub fn is_cut(&self) -> bool { self.cut & 0xff != 0 }
}

/// One actor of a chunk (persisting across chunks by index).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneActor {
    /// Record offset in the chunk.
    pub(crate) offset: u32,
    /// +0x00: moby class (`CreateMoby`).
    pub class: i32,
    /// +0x04: one value per scene, its chunk count as the authoring tool counted it (the NTSC count, also in most PAL
    /// copies; e.g. 0x10 for Novalis scene 5); `FUN_00259288` never reads it (checked on every chunk on the disc).
    pub(crate) unknown_04: i32,
    /// +0x08: this chunk's index in the scene (0, 1, …); not read by the game either.
    pub(crate) unknown_08: i32,
    /// +0x0c: position track offset (chunk-relative).
    pub(crate) track_offset: i32,
    /// +0x2c: frame offsets as stored (relative to +0x10).
    pub(crate) frame_offsets: Vec<u32>,
    /// +0x10: the streamed sequence (frame offsets relative to +0x10), a new class sequence slot.
    pub sequence: MobySequence,
    /// One position per animation frame (30 Hz), w = 0.
    pub positions: Vec<[f32; 4]>,
}

/// One subtitle line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subtitle {
    /// Scene ticks, inclusive.
    pub(crate) start: i16,
    pub(crate) end: i16,
    /// Offsets from the table start (En, Fr, De, Es, It) and the trailing 0.
    pub(crate) text_offsets: [i16; SUBTITLE_LANGUAGES],
    pub(crate) pad: i16,
    /// The NUL-terminated Latin-1 strings (without the NUL).
    pub(crate) text: [Vec<u8>; SUBTITLE_LANGUAGES],
}

impl Subtitle {
    /// Whether the line shows at scene tick `tick` (`fun_001f4be0`: start ≤ tick ≤ end).
    pub(crate) fn covers(&self, tick: i32) -> bool { self.start as i32 <= tick && tick <= self.end as i32 }
}

/// One decompressed and parsed chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneChunk {
    pub(crate) header: ChunkHeader,
    pub camera: Vec<CamRecord>,
    pub actors: Vec<SceneActor>,
    pub(crate) subtitles: Vec<Subtitle>,
}

/// The chunks of a global scene lump (L00 `0x305690`: a table of `{s32 offset, s32 size}` pairs ended by a size of 0,
/// at most 0x46 entries, the chunk data at `0x800 + offset`; each chunk a WAD in the chunk format above, decompressed
/// into `0x1bcaac` by `0x305768` / `0x30f798`). The compressed slices, in order.
pub fn lump_chunks(lump: &[u8]) -> Result<Vec<&[u8]>> {
    let b = Buf(lump);
    let mut out = Vec::new();
    for i in 0..0x46 {
        let (off, size) = (b.i32(8 * i)?, b.i32(8 * i + 4)?);
        if size == 0 { break; }
        if off < 0 || size < 0 { return invalid(format!("scene lump entry {i}: offset {off} size {size}")); }
        out.push(b.sub(0x800 + off as usize, size as usize, "scene lump chunk")?.bytes());
    }
    if out.is_empty() { return invalid("scene lump: no chunks".to_string()); }
    Ok(out)
}

/// Camera records of chunk `index` of a scene ending at `end_tick`: the chunk's ticks plus one.
pub(crate) fn camera_records(end_tick: i32, index: usize, region: Region) -> Result<usize> {
    let tpc = region.ticks_per_chunk() as i32;
    let left = end_tick - index as i32 * tpc;
    if left <= 0 { return invalid(format!("scene chunk {index} starts at or after the end tick {end_tick}")); }
    Ok(left.min(tpc) as usize + 1)
}

/// `FUN_00259288` on a decompressed chunk: chunk `index` of its scene.
pub(crate) fn parse_scene_chunk(dec: &[u8], index: usize, region: Region) -> Result<SceneChunk> {
    let b = Buf(dec);
    let header: ChunkHeader = b.pod(0, "scene chunk header")?;
    let n_cam = camera_records(header.end_tick as i32, index, region)?;
    if header.camera_offset < 0 { return invalid("scene chunk: negative camera offset"); }
    let camera: Vec<CamRecord> = b.pod_slice(header.camera_offset as usize, n_cam, "scene camera table")?;
    let offsets: Vec<u32> = b.pod_slice(0x14, header.actor_count as usize, "scene actor offsets")?;
    let mut actors = Vec::with_capacity(offsets.len());
    for &at in &offsets {
        let a = at as usize;
        let seq_base = b.tail(a + 0x10, "scene actor sequence")?;
        // Scene sequences store 0xff in the trigger-count byte (+0x12) and no trigger words follow the
        // frame offsets: parse as 0 triggers, keep the stored byte in the header.
        let sequence = if seq_base.u8(0x12)? == 0xff {
            let mut patched = seq_base.bytes().to_vec();
            patched[0x12] = 0;
            let mut s = parse_sequence(&patched, 0)?;
            s.header.trigger_count = 0xff;
            s
        } else {
            parse_sequence(seq_base.bytes(), 0)?
        };
        let fc = sequence.header.frame_count as usize;
        let track_offset = b.i32(a + 0xc)?;
        if track_offset < 0 { return invalid("scene actor: negative position track offset"); }
        actors.push(SceneActor {
            offset: at,
            class: b.i32(a)?,
            unknown_04: b.i32(a + 4)?,
            unknown_08: b.i32(a + 8)?,
            track_offset,
            frame_offsets: b.pod_slice(a + 0x2c, fc, "scene actor frame offsets")?,
            positions: b.pod_slice(track_offset as usize, fc, "scene actor position track")?,
            sequence,
        });
    }
    let mut subtitles = Vec::new();
    if header.subtitle_offset >= SUBTITLE_MIN_OFFSET {
        let base = header.subtitle_offset as usize;
        let mut at = base;
        loop {
            let start = b.i16(at)?;
            if start == -1 { break; }
            let text_offsets: [i16; SUBTITLE_LANGUAGES] = std::array::from_fn(|k| b.i16(at + 4 + 2 * k).unwrap_or(-1));
            b.check(at, 16, "scene subtitle entry")?;
            let mut text: [Vec<u8>; SUBTITLE_LANGUAGES] = Default::default();
            for (t, &o) in text.iter_mut().zip(&text_offsets) {
                if o < 0 { continue; }
                let s = b.tail(base + o as usize, "scene subtitle text")?.bytes();
                let Some(nul) = s.iter().position(|&c| c == 0) else { return invalid("scene subtitle text without a NUL") };
                *t = s[..nul].to_vec();
            }
            subtitles.push(Subtitle { start, end: b.i16(at + 2)?, text_offsets, pad: b.i16(at + 14)?, text });
            at += 16;
        }
    }
    Ok(SceneChunk { header, camera, actors, subtitles })
}

// ---------------------------------------------------------------------------------------------------
// A whole scene

/// Every chunk of one scene region, decompressed and parsed.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Record index in the level's scene table (the id `DialogStreamStart` takes).
    pub(crate) index: usize,
    pub(crate) region: Region,
    pub chunks: Vec<SceneChunk>,
}

impl Scene {

    /// A scene from already parsed chunks (checks that they agree on the end tick and cover it).
    pub(crate) fn from_chunks(index: usize, region: Region, chunks: Vec<SceneChunk>) -> Result<Scene> {
        let Some(first) = chunks.first() else { return invalid(format!("scene {index}: no chunks")) };
        let end = first.header.end_tick as i32;
        let tpc = region.ticks_per_chunk() as i32;
        if chunks.iter().any(|c| c.header.end_tick as i32 != end) { return invalid(format!("scene {index}: chunks disagree on the end tick")); }
        if (end + tpc - 1) / tpc != chunks.len() as i32 { return invalid(format!("scene {index}: {} chunks for end tick {end}", chunks.len())); }
        Ok(Scene { index, region, chunks })
    }

    /// A global scene lump ([`lump_chunks`]: the ship's take-off and landing scenes of `SPACE.WAD`, L00 `0x305690` /
    /// `0x305768`), every chunk decompressed (`0x30f798`) and parsed as a level scene's (`0x305428` reads the same
    /// chunk layout); `index` is the lump's row.
    pub(crate) fn from_lump(lump: &[u8], index: usize, region: Region) -> Result<Scene> {
        let chunks = lump_chunks(lump)?
            .into_iter()
            .enumerate()
            .map(|(i, c)| parse_scene_chunk(&crate::wad::decompress(c)?, i, region))
            .collect::<Result<Vec<_>>>()?;
        Scene::from_chunks(index, region, chunks)
    }

    pub fn end_tick(&self) -> i32 { self.chunks[0].header.end_tick as i32 }
    pub fn ticks_per_chunk(&self) -> u32 { self.region.ticks_per_chunk() }
    /// Actor classes (chunk 0 order = the slot order).
    pub fn actor_classes(&self) -> Vec<i32> { self.chunks[0].actors.iter().map(|a| a.class).collect() }

    /// `(chunk, chunk tick)` the player is on at scene tick `tick` ≥ 1: the chunk rolls over when the
    /// chunk tick reaches ticks-per-chunk (record 0 of the next chunk = the last record of this one).
    pub(crate) fn chunk_at(&self, tick: i32) -> (usize, u32) {
        let tpc = self.ticks_per_chunk() as i32;
        let t = tick.max(0);
        ((t / tpc) as usize, (t % tpc) as u32)
    }

    /// The subtitle drawn at `tick`: the first entry of the current chunk's table covering it.
    pub fn subtitle_at(&self, tick: i32) -> Option<&Subtitle> {
        let (c, _) = self.chunk_at(tick);
        self.chunks.get(c)?.subtitles.iter().find(|s| s.covers(tick))
    }
}

