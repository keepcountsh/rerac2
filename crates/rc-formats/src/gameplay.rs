//! Gameplay (instances) file: moby instances. Spec: docs/formats/wad_layouts_rac1.md §3.5 and
//! docs/formats/moby_rac1.md §7.3; field use from the level loader that turns each record into a
//! runtime moby, `FUN_00255958` in the level01 overlay (the loop after `piVar27[0x11]`, pointer 0x44),
//! with `InitMobyInstance` (boot 0x20c5f0), `pack_render_command_fields` (boot 0x20d4f0) and the
//! matrix builder `fun_0020def8` (boot 0x20def8). Notes: docs/plan/moby_render_notes.md.
//!
//! The file is the decompressed `gameplay_ntsc` lump. Other sections (lights: `tfrag_light`,
//! level settings: the engine's `game_camera`) are read elsewhere.

use bytemuck::{Pod, Zeroable};

/// Record size; the loader advances by each record's own `size` field, which is always this.
pub(crate) const MOBY_INSTANCE_SIZE: usize = 0x78;

/// One static moby instance as stored in the gameplay file (0x78 bytes). "moby+X" names the field of
/// the 0x100-byte runtime moby the loader copies it to.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct MobyInstance {
    /// 0x00: record size, 0x78; the loader steps by it.
    pub(crate) size: i32,
    /// 0x04: → moby+0xb0 (byte) and the index into a table at 0x15fc88 used by the spawn test. -1 on most.
    pub unknown_4: i32,
    /// 0x08: spawn condition flags (0 = always). Bits 0..3 select save/game-state bit tests on
    /// `spawn_id`; 0x10 picks moby+0xb1 from `FUN_0029ab50`. See [`MobyInstance::spawn_flags`] notes.
    pub spawn_flags: i32,
    /// 0x0c: → moby+0xb2 (s16); bit index for the spawn tests (unique per instance, -2 on some).
    pub spawn_id: i32,
    /// 0x10: → moby+0xb4 (s16) (halved by some spawn tests).
    pub unknown_10: i32,
    /// 0x14: alternative for `unknown_10` in one spawn-test branch.
    pub unknown_14: i32,
    /// 0x18: moby class number (`MobyClassEntry::o_class`); passed to `InitMobyInstance`.
    pub o_class: i32,
    /// 0x1c: instance scale; moby+0x2c = class scale (class header 0x24) × this (EE FPU `mul.s`).
    pub scale: f32,
    /// 0x20: → moby+0x32 (s16). Wrench calls it an f32 `draw_distance`; the loader reads it as an
    /// integer (64 on Novalis), and MobyProc culls/fades against it (rc-engine `moby_lod.rs`).
    pub draw_distance: i32,
    /// 0x24: → moby+0x30 (byte), update distance.
    pub update_distance: i32,
    /// 0x28: 32 on the disc, unused by the loader.
    pub(crate) unused_28: i32,
    /// 0x2c: 64 on the disc, unused by the loader.
    pub(crate) unused_2c: i32,
    /// 0x30: world position, game units (→ moby+0x10).
    pub position: [f32; 3],
    /// 0x3c: Euler angles in radians (→ moby+0x40). `fun_0020def8` turns them into rotation rows with
    /// VU0 program 28259 at 0xd18: R = Rz(z)·Ry(y)·Rx(x) (x applied first), see
    /// [`crate::moby_light::rotation_rows`].
    pub rotation: [f32; 3],
    /// 0x48: moby group, or -1 (→ moby+0x21, byte).
    pub group: i32,
    /// 0x4c: non-zero: z = ground height below the position (`FUN_0026e618`) + `rooted_distance` (the loader's snap:
    /// `rc_game::moby_update::scheduler::root_instances`).
    pub is_rooted: i32,
    /// 0x50: see `is_rooted`.
    pub rooted_distance: f32,
    /// 0x54: unused by the loader.
    pub(crate) unknown_54: i32,
    /// 0x58: pvar index or -1 (→ moby+0x78, later relocated to a pointer).
    pub pvar_index: i32,
    /// 0x5c: 0 clears moby+0x36 (occlusion bits), else moby+0x36 = 0x7f80.
    pub(crate) occlusion: i32,
    /// 0x60: OR'd into moby+0x34 (u16 mode bits) and into the class header's 0x44. 0x8000 mirrors the
    /// second rotation row (`fun_0020def8`); 0x100 keeps the existing rows (not rebuilt from `rotation`).
    pub mode_bits: i32,
    /// 0x64: ambient colour r, g, b (0..255): `pack_render_command_fields(moby, b<<16 | g<<8 | r, ..)`
    /// puts it at moby+0x3c..0x3e, the per-moby ambient of the lighting (128 = 1.0).
    pub(crate) color: [i32; 3],
    /// 0x70: written as an s32 to moby+0x38: byte 0 = directional light set, byte 1 = second set,
    /// byte 2 = cross-fade weight (0 = none) of the MobyProc light selection.
    pub(crate) light: i32,
    /// 0x74: if not -1 the loader calls `FUN_0025e7b0(moby)` (`MobyUnknown74Hook`: the NPC's talk slot, rc-game
    /// `moby_update::interact`).
    pub(crate) unknown_74: i32,
}
const _: () = assert!(std::mem::size_of::<MobyInstance>() == MOBY_INSTANCE_SIZE);

impl MobyInstance {
    /// moby+0x38..0x3b as the loader leaves it: the `light` word (little-endian bytes: set 0, set 1,
    /// cross-fade, unused).
    pub fn light_word(&self) -> u32 { self.light as u32 }

    /// moby+0x3c..0x3e: the low three bytes of `b * 0x10000 + g * 0x100 + r` (s32 arithmetic, so
    /// channels above 255 would carry; none do on the disc).
    pub fn ambient_rgb(&self) -> [u8; 3] {
        let [r, g, b] = self.color;
        let w = b.wrapping_mul(0x10000).wrapping_add(g.wrapping_mul(0x100)).wrapping_add(r) as u32;
        [w as u8, (w >> 8) as u8, (w >> 16) as u8]
    }

    /// moby+0x34 bits contributed by the instance.
    pub fn mode(&self) -> u16 { self.mode_bits as u16 }
}

// ---------------------------------------------------------------------------------------------------
// Pvars (per-instance class variables). Loader: `FUN_00255958` right after the instance loop
// (docs/plan/moby_update_catalogue.md §2). Header pointers are indices into the 37-word header.

// ---------------------------------------------------------------------------------------------------
// Paths (splines) and the level settings' ship placement

// ---------------------------------------------------------------------------------------------------
// Environment (fog) transition zones: section 0x80. Loader `FUN_00255958` (level01; after the lights):
// `count = *s; memcpy(0x17f140, s + 0x10, count·16); memcpy(0x17f340, s + 0x10 + count·16, count·0x80)`,
// then every record's hero light −1 becomes 0xb. Readers: lookup 0x26bbc0, fog `fun_001ee4b0` (level
// 0x2102b8), hero lighting 0x26be04. Spec: docs/plan/world_animation.md §5.

/// Record size (0x17f340 + i·0x80).
pub(crate) const FOG_ZONE_SIZE: usize = 0x80;

/// One end of a fog transition: the level fog at t = 0 (side 1) or t = 1 (side 2), in game units and
/// densities (`F = 255 − 255·d`, so 0 = no fog).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct FogZoneSide {
    pub(crate) near_dist: f32,
    pub(crate) near_density: f32,
    pub(crate) far_dist: f32,
    pub(crate) far_density: f32,
}

/// One transition record as the loader copies it to 0x17f340 + i·0x80.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct FogZone {
    /// 0x00: world → cuboid-local matrix, VU row-vector form: `l = x·row0 + y·row1 + z·row2 + row3`.
    /// The cuboid is |l.xyz| ≤ 1; the blend runs along local x (t = (l.x + 1)/2).
    pub(crate) inverse: [[f32; 4]; 4],
    /// 0x40 / 0x44: hero colour at t = 0 / t = 1 (bytes r, g, b, a; hero moby +0x80, flags & 1).
    pub(crate) hero_color: [u32; 2],
    /// 0x48 / 0x4c: hero directional light bank at t = 0 / t = 1 (hero moby +0x38 bytes 0 / 1). The file
    /// stores −1 for "none"; [`parse_fog_zones`] applies the loader's −1 → 0xb.
    pub(crate) hero_light: [i32; 2],
    /// 0x50: bit 0 = cross-fade the hero lighting (0x26be04), bit 1 = lerp the level fog (`fun_001ee4b0`).
    pub(crate) flags: u32,
    /// 0x54 / 0x58: fog colour at t = 0 / t = 1 (bytes r, g, b; the fourth byte is unused).
    pub(crate) fog_color: [u32; 2],
    /// 0x5c..0x6b (t = 0) and 0x6c..0x7b (t = 1).
    pub(crate) side: [FogZoneSide; 2],
    /// 0x7c: not read by any of the three readers (0 on Novalis).
    pub(crate) unused_7c: u32,
}
const _: () = assert!(std::mem::size_of::<FogZone>() == FOG_ZONE_SIZE);

