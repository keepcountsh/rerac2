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
//! the bolt / XP tallies `0x1c5400` / `0x1c54c0`), the instance pvars (sections 23 / 24), the moby groups (section 20:
//! no groups, so no group-added mobys), the ground snap of rooted instances, the lights and colours (`0x3132e0`), the
//! animation advance `0x313420` (the engine animates every placed moby itself), the deletion and the created mobys'
//! pvar pool.

pub mod classes;
pub mod hits;

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
}

impl Moby {
    /// `InitMobyInstance(moby, o_class)` `0x311418`: the record cleared, alpha 0x80, group −1, the grid rectangle
    /// none, the key `index << 16`; with the class: its update (none: mode 2), scale, mode bits and collision blob;
    /// an animated class stays updatable. A class the level does not have gets mode 5 (7 with no update).
    pub fn init(index: usize, o_class: i16, class: Option<&ClassInfo>) -> Moby {
        let mut m = Moby {
            index, bsphere: [0.0; 4], pos: [0.0; 3], state: 0, group: -1, slot: 0, alpha: 0x80, scale: 0.0, update_dist: 0,
            drawn: 0, draw_dist: 0, mode: 0, update: None, pvars: Vec::new(), has_collision: false, coll_disable: 0,
            grid_rect: GRID_NONE, o_class, rebuilds: 0, mission: 0, sub_index: 0, uid: 0, rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            rot: [0.0; 3], instance: None, hit_slot: -1, parent: None,
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
    /// `+0x48`: the playback speed.
    fn set_anim_speed(&mut self, _m: &Moby, _speed: f32) {}
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
}

impl MobyWorld {
    /// The class table from the level's classes in core-index order (slot = that order).
    pub fn new(mut classes: Vec<ClassInfo>) -> MobyWorld {
        for (i, c) in classes.iter_mut().enumerate() { c.slot = i.min(CLASS_SLOTS - 1) as u8; }
        MobyWorld { mobys: Vec::new(), classes, grid: MobyGrid::new(), changed: true, hits: Default::default() }
    }

    pub fn class(&self, o_class: i16) -> Option<&ClassInfo> { self.classes.iter().find(|c| c.o_class == o_class) }

    /// The loader's static spawn (`0x3012a0`, the moby instance loop): per record, `InitMobyInstance` with its
    /// class, then uid `+0xb2` (word 4), mission `+0xb0` (word 1), sub-index `+0xb1` (word 2's low nibble, | 0x80
    /// without spawn flags), the scale (class × instance), draw and update distances, position, Euler angles, group,
    /// the mode bits of the record (also OR'd into the class's), and the matrix rebuild when the class is loaded.
    /// `instance(i)` is the engine's placed-instance index of record `i`.
    pub fn spawn_static(&mut self, records: &[Rac2MobyInstance], host: &dyn MobyHost) {
        for (i, r) in records.iter().enumerate() {
            let b = &r.base;
            let class = self.class(b.o_class as i16).cloned();
            let mut m = Moby::init(self.mobys.len(), b.o_class as i16, class.as_ref());
            m.uid = r.extra[0] as i16;
            m.mission = b.unknown_4 as u8;
            m.sub_index = (b.spawn_flags & 0xf) as u8 | if b.spawn_id == 0 { 0x80 } else { 0 };
            if let Some(c) = &class { m.scale = c.scale * b.scale; }
            m.draw_dist = b.draw_distance as i16;
            m.update_dist = b.update_distance as u8;
            m.pos = b.position;
            m.rot = b.rotation;
            m.group = b.group as i8;
            m.mode |= b.mode_bits as u16;
            m.instance = Some(i);
            self.mobys.push(m);
            let id = self.mobys.len() - 1;
            if class.is_some() { self.rebuild(id, host); }
        }
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
                crate::hero::melee::Target { id: i, pos: m.pos, up: m.rows[2], crate_like: (500..=540).contains(&m.o_class), kind }
            })
            .collect()
    }

    /// `0x313718`: the run list for the camera at `cam`. Walk the mobys to the end; skip deleted ones and mode 2. A
    /// moby is active when it was drawn last frame, its update distance is 0xff, or the camera is within it
    /// (`d² − dx² − dy² − dz² ≥ 0`). An active moby of a group flags the group instead (every member is added after
    /// the walk; the port has no groups yet). The additions go to per-slot lists, chained in slot order.
    pub fn run_list(&self, cam: V3) -> Vec<usize> {
        let mut slots: Vec<Vec<usize>> = vec![Vec::new(); CLASS_SLOTS];
        for m in &self.mobys {
            if m.state == 0xff { break; }
            if m.state >= 0x80 || m.mode & mode::NO_UPDATE != 0 { continue; }
            let d = m.update_dist as f32;
            let [dx, dy, dz] = [m.pos[0] - cam[0], m.pos[1] - cam[1], m.pos[2] - cam[2]];
            let near = ((d * d - dx * dx) - dy * dy) - dz * dz >= 0.0;
            if !(m.drawn != 0 || m.update_dist == 0xff || near) { continue; }
            if m.group < 0 { slots[m.slot as usize].push(m.index); }
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
