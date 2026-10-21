//! Going Commando's mobys: the record (L00, 0x100 bytes per moby), `InitMobyInstance` `0x311418`, `CreateMoby`
//! `0x311320`, the level loader's static spawn (in `0x3012a0`), the active-list builder `0x313718`, the per-frame loop
//! `0x32e398` and the matrix rebuild `0x313dc0`. Native `f32` (docs/plan/decisions.md "native-first"); the record
//! fields keep their L00 offsets in the docs. R&C2's moby system is R&C1's (rc_game::moby_runtime,
//! rc_game::moby_update::scheduler) with the record reordered and 240 class slots (R&C1 224); each step here is read
//! from R&C2's code (docs/plan/rac2_bringup.md §6.6).
//!
//! **Record** (the fields the port keeps):
//!
//! | Off | Field | Off | Field |
//! | --- | --- | --- | --- |
//! | 0x00 | bounding sphere, ×1024 world (the rebuild) | 0x10 | position |
//! | 0x20 | state (≥ 0x80 deleted, 0xfe free, 0xff end) | 0x21 | group (−1 none) |
//! | 0x22 | class slot | 0x23 | alpha (0x80) |
//! | 0x24 | class header | 0x28 | next in the run list |
//! | 0x2c | scale (class 0x24 × instance) | 0x30 / 0x31 | update distance / drawn last frame |
//! | 0x32 | draw distance | 0x34 | mode bits ([`mode`]) |
//! | 0x64 | update function | 0x68 | pvars (0x80 bytes per created moby) |
//! | 0x98 | collision blob (class 0x10) | 0x9c | collision disable word |
//! | 0xa4..0xa7 | grid rectangle `{x0, y0, x1, y1}` (`0x80807f7f` none) | 0xaa | o_class |
//! | 0xac | `index << 16 | rebuild count` (the pose-cache key) | 0xb0 / 0xb1 / 0xb2 | mission, sub-index, uid |
//! | 0xc0..0xe0 | rotation rows | 0xf0 | Euler angles |
//!
//! **Class slots.** `InitMobyInstance` reads the slot of an o_class from `0x1da7e0[o_class]` and, when
//! `0x1da600[slot]` names that class, the update function `0x1da240[slot]` and the class header `0x1d9e80[slot]`.
//! The loader's slot assignment is not mapped yet; the port gives the level's classes slots in core-index order
//! ([`MobyWorld::new`]). The slot only orders the run list.
//!
//! **Not ported yet:** the spawn conditions of the static spawn (missions, the save's bits `0x19b2e8` / `0x19b4a8`,
//! the bolt / XP tallies `0x1c5400` / `0x1c54c0`), the pvar pointer fixups (kept relative), the ground snap of
//! rooted instances, the lights and colours (`0x3132e0`), the
//! animation advance `0x313420` (the engine animates every placed moby itself), the deletion and the created mobys'
//! pvar pool.

pub mod classes;
pub mod ai;
pub mod attach;
pub mod bolts;
pub mod hits;
pub mod shards;

use crate::hero::physics::{add, scale, V3};
use crate::hero::Hero;
use rc_formats::collision::Collision;
use rc_formats::moby_anim::AnimState;
use rc_game::collision_query::mobys::{CollMoby, MobyGrid, MobyScene, GRID_NONE};
use rc_formats::rac2::gameplay::Rac2MobyInstance;

/// Mode bits (`+0x34`).
pub mod mode {
    /// No update function (the moby is left out of the run list).
    pub const NO_UPDATE: u16 = 0x2;
    /// No matrix rebuild after the update.
    pub const NO_MATRIX: u16 = 0x4;
    /// Keep the rows (not rebuilt from the Euler angles).
    pub const KEEP_ROWS: u16 = 0x100;
    /// No animation advance before the update.
    pub const NO_ADVANCE: u16 = 0x40;
    /// In the target list (`0x1fac60`).
    pub const TARGET: u16 = 0x1000;
    /// Tested by line queries with flag 0x1 / 0x8.
    pub const LINE: u16 = 0x4000;
    /// Mirror the second row.
    pub const MIRROR: u16 = 0x8000;
}

/// Class slots of the run-list builder (`0x70000000..0x70000780`, 8 bytes each).
pub const CLASS_SLOTS: usize = 0xf0;
/// Moby slots (the level's instances plus the slots the loader leaves for created ones).
pub const MAX_MOBYS: usize = 1024;
/// Bytes of a created moby's pvars (`CreateMoby`: `0x1b1e2c + index·0x80`).
pub const PVAR_BYTES: usize = 0x80;

/// A class update the port has (`+0x64`).
pub type UpdateFn = fn(&mut MobyWorld, usize, &mut Ctx);

/// What a class update reads and writes outside the moby table: the hero block (the updates read Ratchet's state
/// and write his fields), the game state, the level collision and the moby snapshot the queries test, the engine.
pub struct Ctx<'a> {
    pub host: &'a mut dyn MobyHost,
    pub hero: &'a mut Hero,
    pub game: &'a mut crate::game::GameState,
    pub coll: Option<&'a Collision>,
    pub scene: Option<&'a MobyScene<'a>>,
    /// Ratchet's moby (`0x18c0b0`).
    pub hero_moby: Option<usize>,
    /// Ratchet's animation (`0x18aa40..0x18aa60`, his moby's `+0x40..+0x43`) as this tick left it.
    pub hero_view: rc_game::hero::anim::AnimView,
    /// The level's volumes and paths.
    pub level: &'a crate::level::LevelData,
}

/// What the level's class table gives a moby at init.
#[derive(Clone, Debug, Default)]
pub struct ClassInfo {
    pub o_class: i16,
    pub slot: u8,
    /// The port of the class's update (`0x1da240[slot]`); `None`: no port (mode 2, as for a class without one).
    pub update: Option<UpdateFn>,
    /// Class header 0x24: the scale.
    pub scale: f32,
    /// Class header 0x10 ≠ 0: a collision blob.
    pub has_collision: bool,
    /// Class header 0x44: mode bits every instance gets.
    pub mode: u16,
    /// Sequence 0 has 2 frames or more (`0x3116e0`'s test): the moby animates, so it stays in the run list.
    pub animated: bool,
    /// Class header +0x46: the class's type (0x16: a target the aims prefer, 0x14 / 0x12: the wrench's exceptions).
    pub kind: i16,
    /// Per sequence, its reaction record (sequence `+0x14`, [`ai::Reaction`]).
    pub reactions: Vec<Option<ai::Reaction>>,
    /// The break pieces' centre and points (1/1024 of the moby's scale; `rc_formats::moby::MobyPiece`), by piece.
    pub pieces: Vec<Option<(V3, Vec<V3>)>>,
}

/// One moby.
#[derive(Clone, Debug)]
pub struct Moby {
    pub index: usize,
    pub bsphere: [f32; 4],
    pub pos: V3,
    pub state: u8,
    pub group: i8,
    pub slot: u8,
    pub alpha: u8,
    pub scale: f32,
    pub update_dist: u8,
    pub drawn: u8,
    pub draw_dist: i16,
    pub mode: u16,
    pub update: Option<UpdateFn>,
    pub pvars: Vec<u8>,
    pub has_collision: bool,
    pub coll_disable: u32,
    pub grid_rect: u32,
    pub o_class: i16,
    pub rebuilds: u16,
    pub mission: u8,
    pub sub_index: u8,
    pub uid: i16,
    pub rows: [V3; 3],
    pub rot: V3,
    /// The placed instance this moby was spawned from (the engine's instance index), if any.
    pub instance: Option<usize>,
    /// `+0xa8`: its hit record ([`hits::HitLog`]), −1 none.
    pub hit_slot: i8,
    /// `+0xb8`: the moby that owns it (the hand items: Ratchet).
    pub parent: Option<usize>,
    /// The instance record's pvar index (−1: none or a created moby).
    pub pvar_index: i32,
    /// `+0xb4` / `+0xb6`: the bolts and the XP the moby gives (the record's words 5 / 7).
    pub bolts: i16,
    pub xp: i16,
    /// `+0xbc`: the class's timer byte; `+0x7f` the drop count.
    pub timer: u8,
    pub drops: u8,
    /// `+0x94` the state before, `+0x95` the sub-state, `+0x96` the ticks in the state (the classes count them).
    pub prev_state: u8,
    pub sub_state: u8,
    pub state_ticks: i16,
    /// `+0xbe`: bit 0 / 1 the once-flags (`0x32a7c8` / `0x32a7f0`, cleared by a state change), bit 2 hooked
    /// (`0x32a818`).
    pub once: u8,
    /// The port's: another moby moves or hides this one (a carrier, a group shown or hidden), so the engine keeps
    /// its draw following it even without an update of its own.
    pub drive: bool,
    /// `+0x38`: the light word (byte 0 the directional light set, byte 1 the second, byte 2 their cross-fade) and
    /// `+0x3c` the ambient (r, g, b; 0x80 = 1), as MobyProc lights the moby.
    pub light: u32,
    pub ambient: [u8; 3],
    /// `+0x7c` as a piece mask (0x8000 | 1 << k): drawn as break piece k only (crate::moby::shards).
    pub piece: Option<u8>,
}

/// The spawn rules of the static spawn (`0x3012a0`), over the save's uid bits and the missions.
pub struct SpawnRules<'a> {
    pub game: &'a mut crate::game::GameState,
    /// The level's first load (`0x3012a0`'s argument 0); a respawn clears the count of flag-8 records instead.
    pub fresh: bool,
    /// The ground height below a point (`0x31ce20`), for the rooted records.
    pub ground: &'a dyn Fn(V3) -> Option<f32>,
}

impl SpawnRules<'_> {
    /// Whether the record stays out, and whether it takes the alternative bolts / XP (words 6 / 8). The rank test
    /// (word 9 against the mission's rank `0x2e4450`) leaves the challenge mobys out, banking their rewards for the
    /// mission (`0x1c5400` / `0x1c54c0`); the flags (word 3): 0 always; dead since the checkpoint (`0x21e1c4`):
    /// never; bits 0 / 1 by the mission's table `0x1b1c60` (bit 1: the uid's bit 3 set, its count cleared, the
    /// alternative rewards); bit 2 (with 3 clear) once — not while the uid's count is set; bit 3 the count cleared on
    /// a respawn, not spawned on a first load with a count.
    pub fn test(&mut self, r: &Rac2MobyInstance) -> (bool, bool) {
        let b = &r.base;
        let g = &mut *self.game;
        let (mission, sub, flags, uid, rank) = (b.unknown_4, b.spawn_flags, b.spawn_id, r.extra[0] as i16, b.unknown_14);
        if 0 <= mission && 0 < rank {
            let mut have = g.mission(mission as u8, sub as u8) as i32;
            if have == 0 { have = 1; }
            if b.o_class == 0x1f5 || b.o_class == 0x200 {
                if rank < have { return (true, false); }
            } else if have < rank {
                let k = (mission as usize & 0xf) * 3 + (sub as usize & 3);
                if let Some(x) = g.mission_xp.get_mut(k) { *x += r.extra[3]; }
                if let Some(x) = g.mission_bolts.get_mut(k) { *x += r.extra[1]; }
                return (true, false);
            }
        }
        if flags == 0 { return (false, false); }
        if g.uid_dead.get(uid.max(0) as usize).copied().unwrap_or(0) != 0 { return (true, false); }
        if flags & 3 == 0 {
            if flags & 8 == 0 {
                return (flags & 0xc == 4 && g.uid_count(uid) != 0, false);
            }
            if self.fresh { return (g.uid_count(uid) != 0, false); }
            g.clear_uid(uid);
            return (false, false);
        }
        let active = g.mission_table.get(mission.max(0) as usize).copied().unwrap_or(0) & 0x80 != 0;
        if !active { return (flags & 1 == 0, false); }
        if flags & 2 == 0 { return (true, false); }
        if !g.uid_flag(uid) { g.set_uid_flag(uid); }
        (false, true)
    }
}

/// `0x322668(block, pos, old_pos, old_angles)`: a mover's step (the block `+0x20` of its pvars, read for whoever
/// stands on it): Ratchet grounded on moby `id` is carried by its move from `old` (and turned by its yaw's change
/// about its new place).
pub fn carry_hero(ctx: &mut Ctx, id: usize, old: V3, old_yaw: f32, new: V3, yaw: f32) {
    let h = &mut *ctx.hero;
    if h.ground_moby != Some(id) || h.air_ticks != 0 { return; }
    let dy = crate::hero::physics::sub_rot(yaw, old_yaw);
    let (s, c) = dy.sin_cos();
    let r = [h.pos[0] - old[0], h.pos[1] - old[1]];
    h.pos = [new[0] + r[0] * c - r[1] * s, new[1] + r[0] * s + r[1] * c, h.pos[2] + new[2] - old[2]];
    h.rot[2] = crate::hero::physics::add_rot(h.rot[2], dy);
}

/// `0x2eeab0` / `0x2eeae0`: a countdown — 1 down (not below 0); true when it was 0 already or just reached it.
pub fn countdown(x: &mut i32) -> bool {
    if *x == 0 { return true; }
    *x = (*x).max(1) - 1;
    *x < 1
}

impl Moby {
    /// `0x31b8d0(moby, state, sub)`: a new state (the last kept, its ticks from 0; `sub` ≥ 0 sets the sub-state).
    pub fn set_state(&mut self, state: u8, sub: i32) {
        self.prev_state = self.state;
        self.state = state;
        self.state_ticks = 0;
        self.once &= !1;
        if 0 <= sub {
            self.sub_state = sub as u8;
            self.once &= !2;
        }
    }

    /// `0x32a7c8`: true the first time since the last state change.
    pub fn first(&mut self) -> bool {
        if self.once & 1 != 0 { return false; }
        self.once |= 1;
        true
    }


    /// `InitMobyInstance(moby, o_class)` `0x311418`: the record cleared, alpha 0x80, group −1, the grid rectangle
    /// none, the key `index << 16`; with the class: its update (none: mode 2), scale, mode bits and collision blob;
    /// an animated class stays updatable. A class the level does not have gets mode 5 (7 with no update).
    pub fn init(index: usize, o_class: i16, class: Option<&ClassInfo>) -> Moby {
        let mut m = Moby {
            index, bsphere: [0.0; 4], pos: [0.0; 3], state: 0, group: -1, slot: 0, alpha: 0x80, scale: 0.0, update_dist: 0,
            drawn: 0, draw_dist: 0, mode: 0, update: None, pvars: Vec::new(), has_collision: false, coll_disable: 0,
            grid_rect: GRID_NONE, o_class, rebuilds: 0, mission: 0, sub_index: 0, uid: 0, rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            rot: [0.0; 3], instance: None, hit_slot: -1, parent: None, pvar_index: -1, bolts: 0, xp: 0, timer: 0, drops: 0, prev_state: 0, sub_state: 0, state_ticks: 0, once: 0, drive: false,
            // `0x311418`: `+0x38` = 0x40404000000000.
            light: 0, ambient: [0x40; 3], piece: None,
        };
        match class {
            Some(c) => {
                m.slot = c.slot;
                m.update = c.update;
                if m.update.is_none() { m.mode |= mode::NO_UPDATE; }
                m.mode |= c.mode;
                m.has_collision = c.has_collision;
                m.scale = c.scale;
                if c.animated { m.mode &= !mode::NO_UPDATE; }
            }
            None => m.mode |= mode::NO_UPDATE | 5,
        }
        m
    }

    /// The pvar word at `off` (zero past the block).
    pub fn pv_i(&self, off: usize) -> i32 { self.pvars.get(off..off + 4).map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])) }
    pub fn pv_f(&self, off: usize) -> f32 { f32::from_bits(self.pv_i(off) as u32) }
    pub fn pv_s(&self, off: usize) -> i16 { self.pvars.get(off..off + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]])) }
    pub fn pv_b(&self, off: usize) -> u8 { self.pvars.get(off).copied().unwrap_or(0) }
    pub fn pv_v(&self, off: usize) -> V3 { [self.pv_f(off), self.pv_f(off + 4), self.pv_f(off + 8)] }
    fn grow(&mut self, n: usize) { if self.pvars.len() < n { self.pvars.resize(n, 0); } }
    pub fn set_pv_i(&mut self, off: usize, v: i32) {
        self.grow(off + 4);
        self.pvars[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn set_pv_f(&mut self, off: usize, v: f32) { self.set_pv_i(off, v.to_bits() as i32); }
    pub fn set_pv_s(&mut self, off: usize, v: i16) {
        self.grow(off + 2);
        self.pvars[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn set_pv_b(&mut self, off: usize, v: u8) {
        self.grow(off + 1);
        self.pvars[off] = v;
    }
    pub fn set_pv_v(&mut self, off: usize, v: V3) { for (k, x) in v.iter().enumerate() { self.set_pv_f(off + 4 * k, *x); } }
    /// A moby link in the pvars (a runtime moby index; 0 / negative: none, as the game's null pointer).
    pub fn pv_link(&self, off: usize) -> Option<usize> { usize::try_from(self.pv_i(off)).ok().filter(|&i| i != 0) }

    /// `+0xac`: `index << 16 | rebuild count`.
    pub fn key(&self) -> u32 { (self.index as u32) << 16 | self.rebuilds as u32 }

    /// What the collision kernels read of this moby (`rc_game::collision_query::mobys`, whose moby pass is R&C2's:
    /// `0x2de878`'s reads the same blob, scale, rows and position × 1024).
    pub fn coll(&self) -> CollMoby {
        let b = |v: V3| v.map(f32::to_bits);
        CollMoby {
            bsphere: self.bsphere.map(f32::to_bits),
            position: b(self.pos),
            rows: self.rows.map(b),
            scale: self.scale.to_bits(),
            mode: self.mode,
            disable: self.coll_disable,
            key: self.key(),
            o_class: self.o_class,
            collision: self.has_collision,
        }
    }
}

/// What the moby loop needs from outside (the engine): the animation, and the sequence bounding sphere the matrix
/// rebuild places.
pub trait MobyHost {
    /// `0x313420`: advance moby `m`'s animation one tick (the engine's animation player).
    fn advance(&mut self, _m: &Moby) {}
    /// Moby `m`'s animation state (`+0x40..+0x4c`: frames, sequences, blend, speed); `None`: not animated here.
    fn anim(&self, _m: &Moby) -> Option<AnimState> { None }
    /// `0x31b5f0(moby, seq, frame, blend)`: start sequence `seq` at `frame`, blended over `blend` ticks.
    fn set_anim(&mut self, _m: &Moby, _seq: u8, _frame: u8, _blend: u8) {}
    /// The camera's position (`0x1b8d40`).
    fn camera_pos(&self) -> V3 { [0.0; 3] }
    /// `+0x48`: the playback speed.
    fn set_anim_speed(&mut self, _m: &Moby, _speed: f32) {}
    /// `0x311880`: the moby's place in its sequence in the frames' time units (`+0x44` the blend between two keys of one
    /// sequence; else key A's time plus `t`).
    fn frame_time(&self, _m: &Moby) -> f32 { 0.0 }
    /// `0x312728(m, j)`: joint `j`'s frame in the world (rows, translation).
    fn joint_matrix(&self, _m: &Moby, _j: usize) -> Option<([V3; 3], V3)> { None }
    /// The position of joint `j` of moby `m` in the world (`0x3127c8`: the posed joint's translation · scale / 1024,
    /// turned by the rows, plus the position).
    fn joint(&self, _m: &Moby, _j: usize) -> Option<V3> { None }
    /// Ratchet's attachment matrix `k` (`0x18aa70 + 0x40·k`, `0x2aa1f0`: the attach points 0..6, 29, 30 of his class
    /// as `0x3175f8` evaluates them, in the world): rows and position.
    fn hero_attach(&self, _k: usize) -> Option<([V3; 3], V3)> { None }
    /// The current sequence's bounding sphere of moby `m` (class 0x48's sequence header 0x00, blended between the
    /// two sequences of a blend: the rebuild's `+0x42` / `+0x43` / `+0x44`), packed model units (×1024); `None`: the
    /// class has none (the sphere stays at the position).
    fn sequence_sphere(&self, _m: &Moby) -> Option<[f32; 4]> { None }
    /// `0x3605f0(index, mode, moby)`: play the moby's class sound `index` (mode 0 once, 4 looped); returns its
    /// handle (−1: none).
    fn play_sound(&mut self, _m: &Moby, _index: i32, _mode: i32) -> i32 { -1 }
    /// `0x3602f0(handle)`: stop a sound; `0x3602a8(moby, handle)`: it still plays.
    fn stop_sound(&mut self, _handle: i32) {}
    fn sound_playing(&self, _handle: i32) -> bool { false }
}

/// The level's mobys (`0x1b1e1c`, 0x100 bytes each) with the class table and the collision grid.
#[derive(Clone, Default)]
pub struct MobyWorld {
    pub mobys: Vec<Moby>,
    pub classes: Vec<ClassInfo>,
    pub grid: MobyGrid,
    /// A rebuild moved a moby's sphere since the flag was last cleared (the engine's collision snapshot and poses
    /// follow then).
    pub changed: bool,
    /// The hit log (`0x1c3400`).
    pub hits: hits::HitLog,
    /// The moby groups (`0x1fa8a0`): runtime moby indices per group.
    pub groups: Vec<Vec<usize>>,
    /// The ties' and shrubs' world matrices (`0x1b249c` / `0x1b2380`, 0x40 each: rows, position) and their groups
    /// (`0x1a9014` / `0x1a901c`), for the carriers ([`attach`]).
    pub ties: Vec<[[f32; 4]; 4]>,
    pub shrubs: Vec<[[f32; 4]; 4]>,
    pub tie_groups: Vec<Vec<u16>>,
    pub shrub_groups: Vec<Vec<u16>>,
    /// The ties' o_class (the runtime tie's class byte `+0x1d` through `0x226880`).
    pub tie_classes: Vec<i32>,
    /// The carriers' members by carrier ([`attach`]), and the ties / shrubs a carrier moved since the engine last
    /// took them.
    pub attached: std::collections::HashMap<usize, attach::Carrier>,
    pub moved_ties: Vec<usize>,
    pub moved_shrubs: Vec<usize>,
    /// The ties hidden by the level's objects (the runtime tie's `+0x1e` bit 2), for the engine.
    pub hidden_ties: std::collections::HashSet<usize>,
}

/// The instance data the static spawn reads besides the records (gameplay sections 20, 22..25).
#[derive(Clone, Debug, Default)]
pub struct SpawnData {
    pub pvars: Vec<Vec<u8>>,
    /// `(pvar index, offset)`: a moby record index there becomes the runtime moby's (−1: did not spawn).
    pub links: Vec<(usize, usize)>,
    /// `(pvar index, offset)`: a pointer relative to its own block (kept relative here: the classes read offsets).
    pub fixups: Vec<(usize, usize)>,
    pub groups: Vec<Vec<u16>>,
    /// The ties' and shrubs' matrices and groups ([`MobyWorld::ties`]).
    pub ties: Vec<[[f32; 4]; 4]>,
    pub shrubs: Vec<[[f32; 4]; 4]>,
    pub tie_groups: Vec<Vec<u16>>,
    pub shrub_groups: Vec<Vec<u16>>,
    pub tie_classes: Vec<i32>,
}

impl MobyWorld {
    /// The class table from the level's classes in core-index order (slot = that order).
    pub fn new(mut classes: Vec<ClassInfo>) -> MobyWorld {
        for (i, c) in classes.iter_mut().enumerate() { c.slot = i.min(CLASS_SLOTS - 1) as u8; }
        MobyWorld { mobys: Vec::new(), classes, grid: MobyGrid::new(), changed: true, hits: Default::default(), groups: Vec::new(), ..Default::default() }
    }

    pub fn class(&self, o_class: i16) -> Option<&ClassInfo> { self.classes.iter().find(|c| c.o_class == o_class) }

    /// The loader's static spawn (`0x3012a0`, the moby instance loop): per record, `InitMobyInstance` with its
    /// class, then uid `+0xb2` (word 4), mission `+0xb0` (word 1), sub-index `+0xb1` (word 2's low nibble, | 0x80
    /// without spawn flags), the scale (class × instance), draw and update distances, position, Euler angles, group,
    /// the mode bits of the record (also OR'd into the class's), and the matrix rebuild when the class is loaded.
    /// `instance(i)` is the engine's placed-instance index of record `i`.
    pub fn spawn_static(&mut self, records: &[Rac2MobyInstance], data: &SpawnData, host: &dyn MobyHost, rules: &mut SpawnRules) {
        // Which records spawn (the rules of `0x3012a0`), and the record index → runtime moby (`0x1fb5e0`).
        let mut map = vec![-1i32; records.len()];
        let mut alt = vec![false; records.len()];
        let mut n = self.mobys.len() as i32;
        for (i, r) in records.iter().enumerate() {
            let (skip, use_alt) = rules.test(r);
            alt[i] = use_alt;
            if skip { continue; }
            map[i] = n;
            n += 1;
        }
        for (i, r) in records.iter().enumerate() {
            if map[i] < 0 { continue; }
            let b = &r.base;
            let class = self.class(b.o_class as i16).cloned();
            let mut m = Moby::init(self.mobys.len(), b.o_class as i16, class.as_ref());
            m.uid = r.extra[0] as i16;
            (m.bolts, m.xp) = if alt[i] { (r.extra[2] as i16, b.unknown_10 as i16) } else { (r.extra[1] as i16, r.extra[3] as i16) };
            m.mission = b.unknown_4 as u8;
            m.sub_index = (b.spawn_flags & 0xf) as u8 | if b.spawn_id == 0 { 0x80 } else { 0 };
            if let Some(c) = &class { m.scale = c.scale * b.scale; }
            m.draw_dist = b.draw_distance as i16;
            m.update_dist = b.update_distance as u8;
            m.pos = b.position;
            // Rooted (word 0x17): on the ground below (`0x31ce20(0.5, pos)`) + the word 0x18.
            if b.is_rooted != 0 {
                if let Some(z) = (rules.ground)(m.pos) { if 0.0 < z { m.pos[2] = z + b.rooted_distance; } }
            }
            m.rot = b.rotation;
            m.group = b.group as i8;
            m.mode |= b.mode_bits as u16;
            (m.light, m.ambient) = (b.light_word(), b.ambient_rgb());
            m.instance = Some(i);
            // `+0x68`: the record's pvar block (word 0x1a), its moby links made runtime indices.
            if b.o_class != 0 {
                if let Some(block) = usize::try_from(b.pvar_index).ok().and_then(|k| data.pvars.get(k)) {
                    let mut pv = block.clone();
                    for &(k, off) in &data.links {
                        if k as i32 != b.pvar_index || off + 4 > pv.len() { continue; }
                        let rec = i32::from_le_bytes([pv[off], pv[off + 1], pv[off + 2], pv[off + 3]]);
                        if rec >= 0 {
                            let to = map.get(rec as usize).copied().unwrap_or(-1);
                            pv[off..off + 4].copy_from_slice(&to.to_le_bytes());
                        }
                    }
                    m.pvars = pv;
                    m.pvar_index = b.pvar_index;
                }
            }
            self.mobys.push(m);
            let id = self.mobys.len() - 1;
            if class.is_some() { self.rebuild(id, host); }
        }
        self.groups = data.groups.iter().map(|g| g.iter().filter_map(|&r| map.get(r as usize).and_then(|&x| usize::try_from(x).ok())).collect()).collect();
        self.ties = data.ties.clone();
        self.shrubs = data.shrubs.clone();
        self.tie_groups = data.tie_groups.clone();
        self.shrub_groups = data.shrub_groups.clone();
        self.tie_classes = data.tie_classes.clone();
    }

    /// `CreateMoby(o_class)` `0x311320`: the first free slot (state ≥ 0xfe), initialised, with zeroed pvars.
    pub fn create(&mut self, o_class: i16) -> usize {
        let class = self.class(o_class).cloned();
        let id = self.mobys.iter().position(|m| m.state >= 0xfe).unwrap_or(self.mobys.len());
        let mut m = Moby::init(id, o_class, class.as_ref());
        m.pvars = vec![0; PVAR_BYTES];
        if id == self.mobys.len() { self.mobys.push(m) } else { self.mobys[id] = m }
        id
    }

    /// The target list (`0x1fac60`): the run list's mobys with mode 0x1000, in run order.
    pub fn targets(&self, cam: V3) -> Vec<crate::hero::melee::Target> {
        self.run_list(cam)
            .into_iter()
            .filter(|&i| self.mobys[i].mode & mode::TARGET != 0)
            .map(|i| {
                let m = &self.mobys[i];
                let kind = self.class(m.o_class).map_or(0, |c| c.kind);
                let look = (m.mode & ai::AI != 0).then(|| { let h = ai::health(m); [m.pv_b(h + 0x28), m.pv_b(h + 0x29), m.pv_b(h + 0x2a)] });
                crate::hero::melee::Target { id: i, pos: m.pos, up: m.rows[2], crate_like: (500..=540).contains(&m.o_class), kind, look }
            })
            .collect()
    }

    /// `0x313718`: the run list for the camera at `cam`. Walk the mobys to the end; skip deleted ones and mode 2. A
    /// moby is active when it was drawn last frame, its update distance is 0xff, or the camera is within it
    /// (`d² − dx² − dy² − dz² ≥ 0`). An active moby of a group flags the group instead (every member is added after
    /// the walk; the port has no groups yet). The additions go to per-slot lists, chained in slot order.
    pub fn run_list(&self, cam: V3) -> Vec<usize> {
        let mut slots: Vec<Vec<usize>> = vec![Vec::new(); CLASS_SLOTS];
        let mut flagged = vec![false; self.groups.len()];
        for m in &self.mobys {
            if m.state == 0xff { break; }
            if m.state >= 0x80 || m.mode & mode::NO_UPDATE != 0 { continue; }
            let d = m.update_dist as f32;
            let [dx, dy, dz] = [m.pos[0] - cam[0], m.pos[1] - cam[1], m.pos[2] - cam[2]];
            let near = ((d * d - dx * dx) - dy * dy) - dz * dz >= 0.0;
            if !(m.drawn != 0 || m.update_dist == 0xff || near) { continue; }
            match usize::try_from(m.group) {
                Ok(g) => if let Some(f) = flagged.get_mut(g) { *f = true },
                Err(_) => slots[m.slot as usize].push(m.index),
            }
        }
        // The flagged groups' members (state < 0x80, not mode 2), in group order.
        for (g, members) in self.groups.iter().enumerate() {
            if !flagged[g] { continue; }
            for &i in members {
                let m = &self.mobys[i];
                if m.state < 0x80 && m.mode & mode::NO_UPDATE == 0 { slots[m.slot as usize].push(i); }
            }
        }
        slots.concat()
    }

    /// `0x32e398`: one tick of the moby loop over the run list for the camera at `cam`. Each moby still alive when
    /// reached: the animation advance (unless mode 0x40), its update (if it has one), the matrix rebuild (unless
    /// mode 4). Mobys created during the loop wait a tick.
    pub fn tick(&mut self, cam: V3, ctx: &mut Ctx) {
        for id in self.run_list(cam) {
            if self.mobys[id].state >= 0x80 { continue; }
            if self.mobys[id].mode & mode::NO_ADVANCE == 0 { ctx.host.advance(&self.mobys[id]); }
            if let Some(f) = self.mobys[id].update { f(self, id, ctx); }
            if self.mobys[id].mode & mode::NO_MATRIX == 0 { self.rebuild(id, &*ctx.host); }
        }
    }

    /// `0x1b1d40`: the free moby slots.
    pub fn free_slots(&self) -> usize { MAX_MOBYS.saturating_sub(self.mobys.iter().filter(|m| m.state < 0xfe).count()) }

    /// Adds a class the level's table does not have (the gadget classes `0x306018` loads), with the next slot.
    pub fn add_class(&mut self, mut c: ClassInfo) {
        if self.class(c.o_class).is_some() { return; }
        c.slot = self.classes.len().min(CLASS_SLOTS - 1) as u8;
        self.classes.push(c);
    }

    /// `DeleteMoby`: state 0xfe (free), out of the grid.
    pub fn delete(&mut self, id: usize) {
        let Some(m) = self.mobys.get_mut(id) else { return };
        m.state = 0xfe;
        let old = m.grid_rect;
        m.grid_rect = GRID_NONE;
        self.grid.update(id as u16, old, GRID_NONE);
        self.changed = true;
    }

    /// `0x313dc0`: rows from the Euler angles (unless mode 0x100; mode 0x8000 mirrors the second row), the bounding
    /// sphere = position · 1024 + rows · (the sequence's sphere · scale), radius · scale; the rebuild count; and,
    /// with a collision blob, the grid rectangle from the sphere (`0x313ae0` when it changed and is on the grid).
    pub fn rebuild(&mut self, id: usize, host: &dyn MobyHost) {
        let sphere = host.sequence_sphere(&self.mobys[id]);
        let m = &mut self.mobys[id];
        if m.state >= 0x80 { return; }
        if m.mode & mode::KEEP_ROWS == 0 {
            let r = rc_formats::moby_light::rotation_rows(m.rot).map(|row| [f32::from_bits(row[0]), f32::from_bits(row[1]), f32::from_bits(row[2])]);
            m.rows = r;
            if m.mode & mode::MIRROR != 0 { m.rows[1] = scale(m.rows[1], -1.0); }
        }
        let s = sphere.unwrap_or([0.0; 4]);
        let local = [s[0] * m.scale, s[1] * m.scale, s[2] * m.scale];
        let c = add(add(add(scale(m.rows[0], local[0]), scale(m.rows[1], local[1])), scale(m.rows[2], local[2])), scale(m.pos, 1024.0));
        let b = [c[0], c[1], c[2], s[3] * m.scale];
        let moved = b != m.bsphere;
        m.bsphere = b;
        m.rebuilds = m.rebuilds.wrapping_add(1);
        if m.has_collision {
            let new = MobyGrid::rect_for(m.bsphere.map(f32::to_bits));
            if new != m.grid_rect && new & 0xc0c0 == 0 {
                self.grid.update(m.index as u16, m.grid_rect, new);
                m.grid_rect = new;
            }
        }
        self.changed |= moved;
    }
}
