//! The level camera system (L00): the level's camera records (the gameplay's cameras section, `0x1b17c4`, with their
//! pvar blocks) as the camera slots (`0x1b9150`, 0xa0 each, active flags `0x1bb8b0`; set up by `0x2da248`), the
//! per-tick choice of the current camera (`0x2d8618` with the check `0x2d8408`), the switch (`0x2d8168`), and the
//! classes Aranos needs: 0 the follow camera ([`crate::camera::FollowCamera`], kept by the engine) and 0x13 the
//! fly-by. R&C2's system is R&C1's evolved (`crate::camera::level`, `flyby`): the class table `0x2a3a00`
//! (`{class, activate, init, update, pre}` rows: 0 follow, 4 first person, 5 script, 6, 7 the Swingshot's, 0x13
//! the fly-by, 0x17 the placed view, 0x1b).
//!
//! **Slots** (`0x2da248`): one per record in record order: the record's class, its pvar header's priority (+0x1c),
//! blend kind (+0x1d) and activation kind (+0x1f); the follow camera current.
//!
//! **The choice** (`0x2d8618`): for every slot but the current one, in order, `0x2d8408(slot, best)`: priority 0
//! never; the class's hook (−1 no, 1 yes, 0 go on); then by the kind: 0 always, 1 / 2 once entered (+0x7d), 4
//! Ratchet in the cuboid (pvar +0x0c), 7 Ratchet's camera mode equal to the class; and a candidate beats the best only
//! with a higher priority unless the best is releasing (+0x7e ≠ 0). A new best is switched in: a release of kind 4
//! (the fly-by's end) is a cut, as is every switch between Aranos's cameras (their blend kinds are 0); the new
//! camera's init runs.
//!
//! **The fly-by** (class 0x13; activate `0x42b0d8`, init `0x42a660`, update `0x42aa28`): a scripted camera an object
//! arms (`0x42a4e0`: the record's +0x38 = 1, +0x39 = 0; the gate 0x0ac7, the arrival 0x1330). Its hook: done → no;
//! armed and the best not a fly-by → yes; else not above the best (not releasing) → go on; its region (the header's
//! cuboid, cylinder, sphere or path holding Ratchet; none: armed and not done, `0x42a580`) with Ratchet's state
//! (+0x44) and group (+0x48) when set → yes. Init: armed; the FOV keys (the camera path's points with w > 0, at most 8;
//! above 180° the current FOV) and the FOV found; the paths' chords and lengths; with ticks (+0x3a) the speed
//! (+0x3c) = the length over them; Ratchet held (state 0x6f) unless +0x4c. Update: phase 0 the fade up by 0.05 to
//! black, then the letterbox, the camera at the path's start facing the look path's; phase 1 the fade down, along the path at the speed, the timer, the end (the path's, the timer's,
//! done; with +0x40 under 20 ticks left) → phase 2; the FOV between its keys; the look along the look path by the
//! share flown; rows toward it about up. Phase 2 the fade up to black → phase 3: the FOV back, done, the fade back in
//! over 15 ticks (`0x2d9d68`), the release (4), the letterbox off, Ratchet idle.
//!
//! **The placed view** (class 0x17; hook `0x42b950`, worker `0x42b428`, the still test `0x42b1a8`): never current
//! (its activation kind 3), it works from its hook each tick ([`Cameras::placed_views`]): from the level's 4th frame,
//! with Ratchet still (no stick turn when `+0x34`, at most 1/60 a tick unless in the scripted walk 0x62 / 0x63) in its
//! region and the follow camera current and not held by a higher priority (`0x425e88`), L1 not held, it drives the
//! follow camera: its first two ticks snap the camera to the record's place (`FollowCamera::placed_snap`: the
//! distance across `+0x38`, height `+0x3c`, look height `+0x40` = height − distance·tan(pitch `+0x2c`), the yaw
//! offset `+0x44` = yaw `+0x30` − the yaw to Ratchet when they differ by over 2°), then every tick it holds them
//! (distance at 0.003 with the toward-lock, height at 0.003, look height at 0.005, the pitch `+0x28`° when set, the
//! leash off unless `+0x24`, the yaw target at 60″ a tick). Moving or leaving turns it off (`+0x26`) until its
//! reset (`0x42b9f0`, the arrival 0x1330's after the fly-by).
//!
//! **The script camera** (class 5) is `crate::script_camera`: switched in by its callers, not the choice; its
//! release hands back to the follow camera with a cut or with its pose carried over ([`Out::follow_pose`]).
//!
//! Not ported: the classes 4 (first person: Ratchet's camera mode 4 / 0x51), 6, 7 (no Swingshot on Aranos), the
//! placed view's record angles (`+0x14` / `+0x18`, used when its pvars' are both 0: not on Aranos), 0x1b (its hook
//! says no); the blends (`0x2d8ab0`,
//! `0x2d9488`: none between Aranos's cameras); the fly-by's look at a moby (+0x28 with the offset +0x2c: none on
//! Aranos); the pre hooks; the camera-moby list `0x2d80c0`.

use crate::hero::physics::*;
use crate::camera::FollowCamera;
use crate::hero::Hero;
use crate::level::LevelData;
use crate::script_camera::{ScriptCamera, CLASS_SCRIPT};
use crate::spline::{advance, Cursor};

pub const CLASS_FOLLOW: i16 = 0;
pub const CLASS_FLYBY: i16 = 0x13;
pub const CLASS_PLACED: i16 = 0x17;

/// The pvar header every class has.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Header {
    /// +0x08 / +0x0c / +0x10 / +0x14: the region's sphere, cuboid, cylinder, path (−1 none).
    pub sphere: i32,
    pub cuboid: i32,
    pub cylinder: i32,
    pub path: i32,
    /// +0x1c / +0x1d / +0x1f.
    pub priority: u8,
    pub blend: u8,
    pub kind: u8,
}

/// One slot (`0x1b9150 + 0xa0·i`).
#[derive(Clone, Debug)]
pub struct Slot {
    pub class: i16,
    pub pos: V3,
    pub header: Header,
    /// The record's pvar block (the fly-by keeps its state words in it).
    pub rec: Vec<u8>,
    /// +0x7d entered, +0x7e releasing.
    pub entered: u8,
    pub release: i16,
}

impl Slot {
    fn i(&self, o: usize) -> i32 { self.rec.get(o..o + 4).map_or(-1, |b| i32::from_le_bytes(b.try_into().unwrap())) }
    fn f(&self, o: usize) -> f32 { self.rec.get(o..o + 4).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap())) }
    fn b(&self, o: usize) -> u8 { self.rec.get(o).copied().unwrap_or(0) }
    fn s(&self, o: usize) -> i16 { self.rec.get(o..o + 2).map_or(0, |b| i16::from_le_bytes(b.try_into().unwrap())) }
    fn set_b(&mut self, o: usize, v: u8) { if let Some(b) = self.rec.get_mut(o) { *b = v; } }
    fn set_f(&mut self, o: usize, v: f32) { if let Some(b) = self.rec.get_mut(o..o + 4) { b.copy_from_slice(&v.to_le_bytes()); } }
    fn set_i(&mut self, o: usize, v: i32) { if let Some(b) = self.rec.get_mut(o..o + 4) { b.copy_from_slice(&v.to_le_bytes()); } }
    fn set_s(&mut self, o: usize, v: i16) { if let Some(b) = self.rec.get_mut(o..o + 2) { b.copy_from_slice(&v.to_le_bytes()); } }
}

/// The fly-by's D block.
#[derive(Clone, Debug, Default)]
pub struct Flyby {
    pub cursor: Cursor,
    pub cam_path: Vec<[f32; 4]>,
    pub look_path: Vec<[f32; 4]>,
    pub cam_len: f32,
    pub look_len: f32,
    pub flown: f32,
    pub phase: i16,
    pub timer: i16,
    pub keys: [i32; 8],
    pub degrees: [f32; 8],
    pub base_fov: f32,
}

/// What the camera system leaves for the screen and Ratchet this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Out {
    /// The fly-by's camera (else the follow camera's).
    pub view: Option<(V3, [V3; 3])>,
    /// `0x1bd2f0`: the view's tangent when it changed.
    pub tan: Option<f32>,
    /// The follow camera is current again after a cut: it starts over behind Ratchet.
    pub follow_reset: bool,
    /// The follow camera is current again with the old camera's pose carried over (`0x2d8168` with the release flag
    /// 3 or 5: the slot entered 2, [`FollowCamera::from_pose`]).
    pub follow_pose: Option<(V3, [V3; 3])>,
    /// Ratchet's `SetState(state, 1)`.
    pub hero_state: Option<i32>,
    /// `0x1b1868`: the letterbox up or down.
    pub letterbox: Option<bool>,
}

/// The system.
#[derive(Clone, Debug, Default)]
pub struct Cameras {
    pub slots: Vec<Slot>,
    pub current: Option<usize>,
    pub flyby: Flyby,
    pub pos: V3,
    pub rows: [V3; 3],
    /// `0x1bd2f0`: the view's tangent (0.62 in play).
    pub tan: f32,
    /// The script camera (class 5, `crate::script_camera`).
    pub script: ScriptCamera,
    /// `0x1b8d40` / `0x1b8f70`: the view's position and rows as the last camera update left them (the follow
    /// camera's when it is current).
    pub last_view: (V3, [V3; 3]),
}

/// The level path `i` with each point's w its chord to the next (the last's to the first), and the length of the
/// first n − 1.
fn chords(level: &LevelData, i: i32) -> (Vec<[f32; 4]>, f32) {
    let Some(src) = usize::try_from(i).ok().and_then(|i| level.paths.get(i)) else { return (Vec::new(), 0.0) };
    let n = src.len();
    let mut out = src.clone();
    let mut total = 0.0;
    for k in 0..n {
        let (a, b) = (src[k], src[(k + 1) % n]);
        let d = dist([a[0], a[1], a[2]], [b[0], b[1], b[2]]);
        out[k][3] = d;
        if k + 1 < n { total += d; }
    }
    (out, total)
}

/// Rows looking along `fwd` with `up` up (`0x2eef88` pairs: left = up × forward, up = forward × left).
fn rows_about(fwd: V3, up: V3) -> [V3; 3] {
    let f = set_len(fwd, 1.0);
    let cross = |a: V3, b: V3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let l = set_len(cross(up, f), 1.0);
    let u = cross(f, l);
    [f, l, u]
}

impl Cameras {
    /// `0x2da248`: the slots from the records (`(class, position, pvar block)`), the follow camera current.
    pub fn load(records: Vec<(i16, V3, Vec<u8>)>) -> Cameras {
        let slots: Vec<Slot> = records
            .into_iter()
            .map(|(class, pos, rec)| {
                let w = |o: usize| rec.get(o..o + 4).map_or(-1, |b| i32::from_le_bytes(b.try_into().unwrap()));
                let b = |o: usize| rec.get(o).copied().unwrap_or(0);
                let header = Header { sphere: w(8), cuboid: w(0xc), cylinder: w(0x10), path: w(0x14), priority: b(0x1c), blend: b(0x1d), kind: b(0x1f) };
                Slot { class, pos, header, rec, entered: 0, release: 0 }
            })
            .collect();
        let current = slots.iter().position(|s| s.class == CLASS_FOLLOW);
        Cameras { slots, current, tan: 0.62, ..Default::default() }
    }

    /// The current camera's class (the follow camera without records).
    pub fn current_class(&self) -> i16 { self.current.and_then(|i| self.slots.get(i)).map_or(CLASS_FOLLOW, |s| s.class) }

    /// `0x42a4e0`: record `i`'s fly-by armed (+0x38 = 1, +0x39 = 0).
    pub fn arm(&mut self, i: i32) {
        let Some(s) = usize::try_from(i).ok().and_then(|i| self.slots.get_mut(i)) else { return };
        s.set_b(0x38, 1);
        s.set_b(0x39, 0);
    }

    /// `0x42b9f0`: record `i`'s placed view on again (its tick count and off flag cleared).
    pub fn reset_placed(&mut self, i: i32) {
        let Some(s) = usize::try_from(i).ok().and_then(|i| self.slots.get_mut(i)) else { return };
        s.set_i(0x20, 0);
        s.set_s(0x26, 0);
    }

    /// The placed views' hooks (`0x42b950`, run by the choice `0x2d8618` before the follow camera's update): see the
    /// module docs. `frame` is the level's (`0x1b1948`), `mode` the game's (`0x1a8f00`), `l1` L1 held.
    pub fn placed_views(&mut self, cam: &mut FollowCamera, hero: &Hero, level: &LevelData, frame: i32, mode: i32, l1: bool) {
        for i in 0..self.slots.len() {
            let s = &self.slots[i];
            if s.class != CLASS_PLACED || s.header.priority == 0 || Some(i) == self.current || s.s(0x26) != 0 { continue; }
            if mode != crate::scene::PLAY && self.placed_still(i, cam, hero, level) {
                self.slots[i].set_i(0x20, 0);
                self.slots[i].set_s(0x26, 0);
            } else {
                self.placed_view(i, cam, hero, level, frame, l1);
            }
        }
    }

    /// `0x42b1a8`: the placed view may hold — the follow camera current, Ratchet's body as its `+0x48` / `+0x4a`
    /// allow, no stick turn (with `+0x34`), still (unless walked by a script), in its region (the scripted walk:
    /// within 3 of its centre too); else it is turned off (`+0x26`).
    fn placed_still(&mut self, i: usize, cam: &FollowCamera, hero: &Hero, level: &LevelData) -> bool {
        if self.current_class() != CLASS_FOLLOW { return false; }
        let s = &self.slots[i];
        let clank = matches!(hero.mode, 1 | 2);
        if (s.s(0x4a) != 0 && !clank) || (s.s(0x48) != 0 && clank) { return false; }
        if s.i(0x34) != 0 && !cam.stick_still() { return false; }
        let walked = matches!(hero.state, 0x62 | 0x63);
        if !walked && 0.016_666_668 < hero.eff_len_xy { return false; }
        let h = s.header;
        let near = |l: &[crate::level::Volume], i: i32| {
            hero.state == 0x62 && usize::try_from(i).ok().and_then(|i| l.get(i)).is_some_and(|v| dist(hero.pos, [v.matrix[3][0], v.matrix[3][1], v.matrix[3][2]]) < 3.0)
        };
        if 0 <= h.cuboid && (near(&level.cuboids, h.cuboid) || level.in_cuboid(h.cuboid, hero.pos)) { return true; }
        if 0 <= h.cylinder && (near(&level.cylinders, h.cylinder) || level.in_cylinder(h.cylinder, hero.pos)) { return true; }
        if 0 <= h.sphere && (near(&level.spheres, h.sphere) || level.in_sphere(h.sphere, hero.pos)) { return true; }
        self.slots[i].set_s(0x26, 1);
        false
    }

    /// `0x42b428`.
    fn placed_view(&mut self, i: usize, cam: &mut FollowCamera, hero: &Hero, level: &LevelData, frame: i32, l1: bool) {
        let off = |s: &mut Slot| {
            if s.i(0x20) != 0 { s.set_s(0x26, 1); }
            s.set_i(0x20, 0);
        };
        if 3 < frame && self.placed_still(i, cam, hero, level) {
            let prio: Vec<u8> = self.slots.iter().map(|s| s.header.priority).collect();
            if !cam.take(i, |k| prio.get(k).copied().unwrap_or(0)) { return off(&mut self.slots[i]); }
            if !l1 || hero.mode == 2 {
                let s = &mut self.slots[i];
                if s.i(0x20) < 2 {
                    let p = s.pos;
                    let to_hero = (hero.pos[1] - p[1]).atan2(hero.pos[0] - p[0]);
                    let yaw = s.f(0x30);
                    let gap = { let d = (yaw - to_hero).abs(); if std::f32::consts::PI <= d { std::f32::consts::TAU - d } else { d } };
                    if 0.034_906_59 < gap { s.set_f(0x44, sub_rot(yaw, to_hero)); }
                    let a = sub(p, hero.pos);
                    let height = a[2];
                    let dist = len([a[0], a[1], 0.0]);
                    let pitch = s.f(0x2c);
                    let look_h = if pitch.abs() == 1.570_796_5 { 0.0 } else { height - dist * pitch.tan() };
                    (s.set_f(0x38, dist), s.set_f(0x3c, height), s.set_f(0x40, look_h));
                    cam.placed_snap(p, hero.pos, dist, height, s.f(0x44), look_h);
                }
                s.set_i(0x20, (s.i(0x20) + 1).min(200));
                let (dist, height, look_h, pitch, leash, yaw) = (s.f(0x38), s.f(0x3c), s.f(0x40), s.f(0x28), s.s(0x24), s.f(0x44));
                if dist != 0.0 {
                    cam.set_dist(dist, 0.003, false);
                    cam.lock_toward();
                }
                if height != 0.0 { cam.set_pivot_h(height, 0.003); }
                if look_h != 0.0 { cam.set_look_h(look_h, 0.005, false); }
                if pitch != 0.0 { cam.set_script_pitch(pitch * 0.017_453_294); }
                if leash == 0 { cam.set_leash(false); }
                if yaw != 0.0 { cam.set_yaw_target(yaw, 60.0, 60.0); }
                return;
            }
        }
        off(&mut self.slots[i]);
        cam.release(i);
    }

    /// `0x42a508`: the fly-by is over (no fly-by current, or record `i`'s in its phase 3; phase 1 still black counts).
    pub fn flyby_over(&self, fade: f32) -> bool {
        if self.current_class() != CLASS_FLYBY { return true; }
        match self.flyby.phase {
            1 => fade != 0.0,
            p => p == 3,
        }
    }

    /// `0x42a580`: the region holds Ratchet (none: armed and not done).
    fn region(&self, i: usize, hero: &Hero, level: &LevelData) -> bool {
        let s = &self.slots[i];
        let h = s.header;
        let p = hero.pos;
        if 0 <= h.cuboid { return level.in_cuboid(h.cuboid, p); }
        if 0 <= h.cylinder { return level.in_cylinder(h.cylinder, p); }
        if 0 <= h.sphere { return level.in_sphere(h.sphere, p); }
        if 0 <= h.path { return level.in_polygon(h.path, p); }
        s.b(0x38) != 0 && s.b(0x39) == 0
    }

    /// The class's activation hook: −1 no, 1 yes, 0 go on.
    fn hook(&self, i: usize, best: Option<usize>, hero: &Hero, level: &LevelData) -> i32 {
        let s = &self.slots[i];
        match s.class {
            CLASS_FLYBY => {
                // `0x42b0d8`.
                if s.b(0x39) != 0 { return -1; }
                let b = best.map(|b| &self.slots[b]);
                if s.b(0x38) != 0 && b.is_none_or(|b| b.class != CLASS_FLYBY) { return 1; }
                if let Some(b) = b { if b.release == 0 && s.header.priority <= b.header.priority { return 0; } }
                let group = s.i(0x48);
                let state = s.i(0x44);
                if self.region(i, hero, level) && (group < 0 || hero.group == group) && (state < 0 || hero.state == state) { 1 } else { 0 }
            }
            // `0x42c030`: never.
            0x1b => -1,
            _ => 0,
        }
    }

    /// `0x2d8408(slot, best)`.
    fn check(&self, i: usize, best: Option<usize>, hero: &Hero, level: &LevelData) -> bool {
        let s = &self.slots[i];
        if s.header.priority == 0 { return false; }
        match self.hook(i, best, hero, level) {
            -1 => return false,
            1 => return true,
            _ => {}
        }
        let beats = |b: Option<usize>| b.map(|b| &self.slots[b]).is_none_or(|b| b.release != 0 || b.header.priority < s.header.priority);
        match s.header.kind {
            0 => beats(best),
            1 | 2 => s.entered != 0 && beats(best),
            4 => beats(best) && level.in_cuboid(s.header.cuboid, hero.pos),
            // 7: Ratchet's camera mode (not kept by the port: none of these classes is ported).
            _ => false,
        }
    }

    /// `0x2d8618`: the choice, the switch, the current camera's update. `fade` is `0x1b1860` and its fade-in
    /// (`fade_in`: the rate per tick, 0 none).
    pub fn tick(&mut self, hero: &Hero, level: &LevelData, follow: (V3, [V3; 3]), fade: &mut f32, fade_in: &mut f32) -> Out {
        let mut out = Out::default();
        let mut best = self.current;
        let mut switched = false;
        for i in 0..self.slots.len() {
            if Some(i) == best { continue; }
            if self.check(i, best, hero, level) {
                best = Some(i);
                switched = true;
            }
        }
        if switched {
            let prev = self.current;
            // `0x2d8168`: the old camera's release flag 3 / 5 carries its pose into the new slot (entered 2); its
            // flags cleared; the new one's init.
            let keep_pose = prev.is_some_and(|p| matches!(self.slots[p].release, 3 | 5));
            if let Some(p) = prev.and_then(|p| self.slots.get_mut(p)) {
                p.release = 0;
                p.entered = 0;
            }
            self.current = best;
            let prev_view = match prev.map(|p| self.slots[p].class) {
                Some(CLASS_FLYBY) => (self.pos, self.rows),
                Some(CLASS_SCRIPT) => (self.script.pos, self.script.rows),
                _ => follow,
            };
            match self.current_class() {
                CLASS_FLYBY => self.flyby_init(level, prev_view, &mut out),
                CLASS_SCRIPT => self.script_init(),
                _ if keep_pose => out.follow_pose = Some(prev_view),
                _ => out.follow_reset = true,
            }
        }
        match self.current_class() {
            CLASS_FLYBY => {
                self.flyby_update(level, fade, fade_in, &mut out);
                out.view = Some((self.pos, self.rows));
            }
            CLASS_SCRIPT => self.script_update(hero, &mut out),
            _ => {}
        }
        self.last_view = out.view.unwrap_or(follow);
        out
    }

    /// `0x42a660`.
    fn flyby_init(&mut self, level: &LevelData, prev: (V3, [V3; 3]), out: &mut Out) {
        let Some(i) = self.current else { return };
        let s = &mut self.slots[i];
        s.set_b(0x38, 1);
        let (cam_src, look_src) = (s.i(0x20), s.i(0x24));
        let (cam_path, cam_len) = chords(level, cam_src);
        let (look_path, look_len) = chords(level, look_src);
        let base_fov = self.tan.atan() * 2.0;
        let ticks = s.s(0x3a);
        let mut f = Flyby { cam_len, look_len, base_fov, keys: [-1; 8], timer: ticks, ..Flyby::default() };
        let src = usize::try_from(cam_src).ok().and_then(|p| level.paths.get(p)).cloned().unwrap_or_default();
        let mut k = 0;
        for (j, q) in src.iter().enumerate() {
            if 0.0 < q[3] && k < 8 {
                f.degrees[k] = if 180.0 < q[3] { base_fov * 57.295_776 } else { q[3] };
                f.keys[k] = j as i32;
                k += 1;
            }
        }
        if 0 < ticks { s.set_f(0x3c, cam_len / ticks as f32 * 60.0); }
        let hold = s.i(0x4c) == 0;
        f.cam_path = cam_path;
        f.look_path = look_path;
        self.flyby = f;
        (self.pos, self.rows) = prev;
        if hold { out.hero_state = Some(0x6f); }
    }

    /// The look point: the moby (not ported: the path) or `path_pt`.
    fn flyby_face(&mut self, look: V3) { self.rows = rows_about(sub(look, self.pos), [0.0, 0.0, 1.0]); }

    /// `0x42aa28`.
    fn flyby_update(&mut self, _level: &LevelData, fade: &mut f32, fade_in: &mut f32, out: &mut Out) {
        let Some(i) = self.current else { return };
        let (speed, keep, no_hold) = (self.slots[i].f(0x3c), self.slots[i].i(0x40) != 0, self.slots[i].i(0x4c) != 0);
        match self.flyby.phase {
            0 => {
                *fade += 0.05;
                if *fade < 1.0 { return; }
                *fade = 1.0;
                out.letterbox = Some(true);
                self.flyby.phase = 1;
                let start = self.flyby.cam_path.first().map_or(self.pos, |q| [q[0], q[1], q[2]]);
                let look = self.flyby.look_path.first().map_or(start, |q| [q[0], q[1], q[2]]);
                self.pos = start;
                self.flyby_face(look);
                return;
            }
            2 => {
                *fade += 0.05;
                if 1.0 <= *fade {
                    self.flyby.phase = 3;
                    *fade = 1.0;
                    let half = self.flyby.base_fov * 0.5;
                    self.tan = half.sin() / half.cos();
                    out.tan = Some(self.tan);
                    self.slots[i].set_b(0x39, 1);
                    *fade_in = 1.0 / 15.0;
                    self.slots[i].release = 4;
                    out.letterbox = Some(false);
                    if !no_hold { out.hero_state = Some(0); }
                }
                if !keep { return; }
            }
            3 => return,
            _ => *fade = (*fade - 0.05).max(0.0),
        }
        let old = self.pos;
        let f = &mut self.flyby;
        let (p, mut ended) = advance(&f.cam_path, false, speed * 0.016_666_668, &mut f.cursor);
        self.pos = p;
        if f.timer != 0 {
            let mut t = f.timer as i32;
            let out_of_time = crate::moby::countdown(&mut t);
            f.timer = t as i16;
            if out_of_time || (keep && (f.timer as i32) < 20) { ended = true; }
        }
        if self.slots[i].b(0x39) != 0 { ended = true; }
        if ended {
            self.flyby.phase = 2;
            if !keep { return; }
        }
        // The FOV between its keys.
        let f = &self.flyby;
        let seg = f.cursor.seg;
        for k in 0..7 {
            let (a, b) = (f.keys[k], f.keys[k + 1]);
            if a < 0 || b < 0 { break; }
            if !(a <= seg && seg < b) { continue; }
            let chord = |j: i32| f.cam_path.get(j as usize).map_or(0.0, |q| q[3]);
            let before: f32 = (a..seg).map(chord).sum();
            let span: f32 = before + (seg..b).map(chord).sum::<f32>();
            let at = before + f.cursor.t;
            let deg = f.degrees[k] + (f.degrees[k + 1] - f.degrees[k]) * (at / span);
            let half = deg * 0.008_726_647;
            self.tan = half.sin() / half.cos();
            out.tan = Some(self.tan);
        }
        // The look along the look path by the share flown.
        let f = &mut self.flyby;
        f.flown += dist(old, p);
        let share = if f.cam_len == 0.0 { 0.0 } else { f.flown / f.cam_len * f.look_len };
        let mut c = Cursor::default();
        let look = advance(&f.look_path, false, share, &mut c).0;
        self.flyby_face(look);
    }
}
