//! The level's camera records (gameplay section 0x08) and their pvar blocks: the data of the level camera system
//! (docs/plan/player_controller.md §15, "The level camera system").
//!
//! **File layout** (docs/formats/wad_layouts_rac1.md §3.13): `s32 count, pad[3]`, then `count` × 0x20-byte records
//! `{i32 class, f32 pos[3], f32 rot[3], i32 pvar}`. **Loader** (`InitLevelRenderGlobals` 0x255958, level01): the table
//! `0x15ef50` (count `0x15ef54`) gets the records **reordered** to `{pos[3], class @+0x0c, rot[3], pvar @+0x1c}`, and
//! after the moby pvars each record's `+0x1c` becomes the address of its pvar block's copy (0 for index −1; the
//! pointer fixups apply to it as to a moby's). Every camera's update and activation reads its record through
//! `0x15ef50 + slot·0x20` (UpdateCam +0x84 = the record index).
//!
//! **The pvar header** (every class, [`CameraHeader`]): the region test's shapes (+0x08 sphere, +0x0c cuboid, +0x10
//! cylinder, +0x14 path), +0x04 = the slot's UpdateCam (written by the slot init 0x20ef58), and the four bytes the slot
//! init and the switch read: priority +0x1c (UpdateCam +0x7c), blend kind +0x1d (`FUN_0020d110`), +0x1e (not read by
//! the system [L]) and the activation kind +0x1f (UpdateCam +0x74, `Camera_ActivationCheckPriority` 0x20d410).
//! **Class 17** (the follow-camera tweak regions, 58 records on 14 levels) has a 0x60-byte block, [`RegionTweak`].
//!
//! **The classes a level has** are its overlay's `lvl.camvtbl` (`crate::level_overlay::LevelOverlay::camvtbl`):
//! `{class, activate, init, update, pre}` per class, `-1` ending it.

/// One camera record, as the file has it (the loader's reordering is not kept: the fields are named).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRecord {
    /// File +0x00 (runtime +0x0c, read as s16): the camera class, UpdateCam +0x86.
    pub class: i32,
    /// File +0x04 (runtime +0x00).
    pub pos: [f32; 3],
    /// File +0x10 (runtime +0x10); class 17 reads `rot.z` as the region's facing (`0x318de0`).
    pub(crate) rot: [f32; 3],
    /// File +0x1c: the pvar index (−1: none).
    pub pvar_index: i32,
}
