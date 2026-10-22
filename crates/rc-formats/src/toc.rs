//! RAC1 table of contents at sector 1500. Spec: docs/formats/disc_layout.md section 2.
//! The game itself reads 6 sectors at LBA 1500 and copies 0x2960 bytes to EE 0x137b80.

use bytemuck::{Pod, Zeroable};

pub(crate) const LEVEL_HEADER_SIZE: usize = 0x2434;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct SectorRange { pub(crate) offset: i32, pub(crate) size: i32 }          // both in sectors
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct SectorByteRange { pub(crate) offset: i32, pub(crate) size: i32 }      // offset in sectors, size in bytes

/// Chunk-sector slots per region (the last used one is a 1-sector sentinel).
pub(crate) const SCENE_CHUNK_SLOTS: usize = 71;
/// Scene records in the level header (`0x13a664 + id·0x250`; docs/plan/cutscenes_transitions.md §1).
pub(crate) const SCENE_RECORDS: usize = 15;

/// One in-engine scene (cutscene) record, 0x250 bytes: speech VAG sector per language, then the sectors of
/// the NTSC and the PAL chunk WADs (`fun_00215970` / `FUN_002591d0`). A chunk's size is the sector
/// difference to the next entry; the last entry is a 1-sector sentinel. (Wrench reads this block as
/// 30 × 0x128 `{sounds[6], wads[68]}`, which splits every scene into an NTSC and a PAL half.)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct SceneRecord { pub(crate) speech: [i32; 6], pub(crate) ntsc: [i32; SCENE_CHUNK_SLOTS], pub(crate) pal: [i32; SCENE_CHUNK_SLOTS] }

/// Amalgamated level header, stored at the front of each level's sector run.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct LevelHeader {
    pub(crate) id: i32,
    pub(crate) header_size: i32,
    pub(crate) data: SectorRange,
    pub(crate) gameplay_ntsc: SectorRange,
    pub(crate) gameplay_pal: SectorRange,
    pub(crate) occlusion: SectorRange,
    pub(crate) bindata: [SectorByteRange; 36],
    pub(crate) music: [i32; 15],
    pub(crate) scenes: [SceneRecord; SCENE_RECORDS],
}
const _: () = assert!(std::mem::size_of::<LevelHeader>() == LEVEL_HEADER_SIZE);
const _: () = assert!(std::mem::size_of::<SceneRecord>() == 0x250);

