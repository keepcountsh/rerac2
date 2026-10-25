//! The script camera, class 5 (L00; the class table `0x2a3a00`'s row: hook `0x428fe8` = 0, init `0x428b38`, update
//! `0x428ff0` → `0x428bf8`, pre `0x429048` = nothing) and the calls the classes make: the switch
//! `0x428880(pos, euler, mode, ticks, collide)`, the targets `0x4287b8` (position) / `0x428810` (Euler) and the
//! release `0x4289f0(kind)`. It is the camera of the in-level cutaways: on Aranos the big lift 0x0ac1's ride up
//! (`crate::moby::classes::props::update_0ac1`); elsewhere the vendor (`0x374038` / `0x374628` / `0x375f40`), the
//! ship's take-off and landing (`0x364260`) and `0x3b6cf8`. Only `0x428880` brings it up: its hook says "go on" and
//! the choice (`crate::cameras`) then needs its record entered (+0x7d, set by the switch and cleared by the release).
//!
//! **The switch** (`0x428880`): the class-5 slot (`0x2d7e50(5)`) is switched in by `0x2d8168` — the current
//! camera's release kind decides the hand-over (0 or 1: a cut, +0x8e = 1) — and its init runs (`0x428b38`: the
//! position spring D+0x10 (k 0.3, damping 0.3, at most 1 a tick, on), the Euler spring D+0x50 (0.05, 0.3, 1, on),
//! the velocities D+0x00 / D+0x30 zero, the rows from the slot's Euler, the release flag 0). Then the slot is
//! entered (+0x7d = 1), the camera and its targets (D+0x80 / D+0x90) are `(pos, euler)`, the global view `0x1b8d40`
//! is `pos`, D+0x120 = `collide`, +0x88 = `mode`. Mode 3 starts from the view switched away from instead: D+0x124 /
//! D+0x128 = 1 / 0 (the distance curve's slopes), the camera at the old view's position and Euler, the anchor D+0x110
//! = Ratchet, D+0xfc = his (yaw, elevation, distance) to the old position (`0x2da530`). Modes 2 and 3: the timer
//! D+0xe0 = D+0xe4 = `ticks`.
//!
//! **The update** (`0x428bf8`, by +0x88): 1 snaps to the targets; 2 counts the timer down (`0x2eeab0`), `t = (total −
//! left) / total`, the position eased to the target (`0x31bed8`: the cosine `(1 − cos πt) / 2` applied to the
//! current pose each tick, t = 0 keeps it, 1 lands) and the Euler likewise (`0x31bfb8`, every tick); 3 (the timer
//! out: as 1) swings about Ratchet: the target's (yaw, elevation, distance) to him, the yaw eased the long way round
//! (`0x31bdc0` from 0 over the raw difference, flipped against the view's turn) when the yaw to go and the view's yaw
//! to go disagree by over 20° in opposite senses, else the short way (`0x31be58`); the distance by the Hermite curve
//! `0x31bc68(D+0x124, 0, 1, D+0x128, t)`; the elevation eased; the anchor eased to Ratchet; the position at the
//! (yaw, elevation, distance) about the anchor (`0x2da480`); the Euler eased; else (0) the springs (`0x2d7e98` /
//! `0x2d7f80`) toward the targets, each when its D+0x1c / D+0x5c is 1. Then the rows from the Euler (`0x2ef6d8`).
//!
//! **The release** (`0x4289f0(kind)`): the current camera's release flag 1 (a cut) and entered flag 0 (`0x2d80a0`);
//! kind 0 first puts it 1 behind and 1.2 above Ratchet along his Euler's rows (+0x8a = 1: the push skips a tick);
//! kind 2 → the flag 2 (a blend; the rates `0x1b8ea8` / `0x1b8eb4` 0.009, the kind `0x1b8e93` 0); kind 1 → 3 (the
//! pose carried over); kind 4 → 5 (the pose carried over, the rates set). The next choice switches the follow camera
//! in (its record always wants in and a releasing camera gives way): with the flag 3 or 5 `0x2d8168` copies the
//! script camera's rows and position into its slot and marks it entered 2, so its init `0x424908` keeps that pose
//! (`0x4247a0`: the targets at Ratchet, the offset from the kept position, the row blend of 90 ticks
//! [`crate::camera::FollowCamera::from_pose`]); with 1 it snaps behind Ratchet ([`crate::camera::FollowCamera::new`]).
//!
//! Not ported: the collision push `0x2da5a8` (D+0x120: every caller on the port passes 0); the blend of the release
//! kind 2 (`0x2d8ab0` kind 0, `0x2d9488`: the vendor's close, no consumer yet — it is a cut here); the rates of kind 4
//! (nothing reads them without a blend); the shared D block (the targets written through `0x4287b8` / `0x428810`
//! land in the script camera's own state, never in the current camera's); `0x4289f0`'s placement of a current camera
//! of another class (it is released, not moved).

use crate::cameras::{Cameras, Out};
use crate::hero::physics::*;
use crate::hero::Hero;
use crate::moby::attach::angles_of;
use std::f32::consts::PI;

pub const CLASS_SCRIPT: i16 = 5;

/// +0x88: the mode.
pub const MODE_SPRING: u8 = 0;
pub const MODE_SNAP: u8 = 1;
pub const MODE_MOVE: u8 = 2;
pub const MODE_SWING: u8 = 3;

/// The release kinds (`0x4289f0`'s argument).
pub const RELEASE_CUT_BEHIND: u8 = 0;
pub const RELEASE_KEEP: u8 = 1;
pub const RELEASE_BLEND: u8 = 2;
pub const RELEASE_KEEP_RATES: u8 = 4;

/// A spring's D+0x10.. / D+0x50..: k, damping, the step at most, on (1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub k: f32,
    pub d: f32,
    pub max: f32,
    pub on: i16,
}

/// The script camera's slot state (+0x30 / +0x40 / +0x88 / +0x8a) and its D block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptCamera {
    /// +0x30 the position, +0x40 the Euler (x, y, z), +0x00.. the rows (forward, left, up).
    pub pos: V3,
    pub euler: V3,
    pub rows: [V3; 3],
    /// +0x88.
    pub mode: u8,
    /// +0x8a: placed by the release; the push would skip a tick.
    pub placed: bool,
    /// D+0x80 / D+0x90: the targets.
    pub target_pos: V3,
    pub target_euler: V3,
    /// D+0x00 / D+0x30: the springs' velocities; D+0x10 / D+0x50 the springs.
    pub vel: V3,
    pub avel: V3,
    pub spring_pos: Spring,
    pub spring_rot: Spring,
    /// D+0xe0 left, D+0xe4 total.
    pub timer: i32,
    pub total: i32,
    /// D+0x120.
    pub collide: bool,
    /// Mode 3: D+0xe8 the target's (yaw, elevation, distance) about Ratchet, D+0xfc the camera's, D+0x110 the anchor,
    /// D+0x124 / D+0x128 the distance curve's end slopes.
    pub swing_tgt: V3,
    pub swing: V3,
    pub anchor: V3,
    pub curve: [f32; 2],
    /// `0x1b9004`: the updates since the switch.
    pub ticks: u32,
}

/// `0x31bdc0(a, b, t)`: `a` at 0, `b` at 1, else the cosine blend.
fn cos_interp(a: f32, b: f32, t: f32) -> f32 {
    if t == 0.0 { return a; }
    if t == 1.0 { return b; }
    a + (b - a) * (1.0 - (t * PI).cos()) * 0.5
}

/// `0x31bed8(t, cur, tgt)`: the cosine blend per component (0 keeps, 1 lands).
fn cos_interp3(cur: V3, tgt: V3, t: f32) -> V3 {
    if t == 0.0 { return cur; }
    if t == 1.0 { return tgt; }
    let s = (1.0 - (t * PI).cos()) * 0.5;
    add(cur, scale(sub(tgt, cur), s))
}

/// `0x31be58(a, b, t)`: `a` turned toward `b` by the cosine share of their wrapped difference.
fn rot_ease(a: f32, b: f32, t: f32) -> f32 { add_rot(a, sub_rot(b, a) * 0.5 * (1.0 - (t * PI).cos())) }

/// `0x31bfb8(t, cur, tgt)`: [`rot_ease`] per angle.
fn rot_ease3(cur: V3, tgt: V3, t: f32) -> V3 {
    let s = (1.0 - (t * PI).cos()) * 0.5;
    std::array::from_fn(|k| add_rot(cur[k], sub_rot(tgt[k], cur[k]) * s))
}

/// `0x31bc68(p1, p2, p3, p4, t)`.
fn hermite(p1: f32, p2: f32, p3: f32, p4: f32, t: f32) -> f32 {
    let f = (p4 - p3) - (p1 - p2);
    f * t * t * t + ((p1 - p2) - f) * t * t + (p3 - p1) * t + p2
}

/// `0x2d7e98(cur, tgt, k, d, max, &v)`.
fn spring(cur: f32, tgt: f32, s: Spring, v: &mut f32) -> f32 {
    let e = tgt - cur;
    *v += s.k * e - s.d * *v;
    if s.max != 0.0 { *v = v.clamp(-s.max, s.max); }
    *v = v.clamp(-e.abs(), e.abs());
    cur + *v
}

/// `0x2d7f80`: [`spring`] on wrapped angles.
fn ang_spring(cur: f32, tgt: f32, s: Spring, v: &mut f32) -> f32 {
    let e = sub_rot(tgt, cur);
    *v += s.k * e - s.d * *v;
    if s.max != 0.0 { *v = v.clamp(-s.max, s.max); }
    *v = v.clamp(-e.abs(), e.abs());
    add_rot(cur, *v)
}

/// `0x2da530(out, p, c)`: (yaw, elevation, distance) of `p − c`.
fn spherical(p: V3, c: V3) -> V3 {
    let d = sub(p, c);
    [atan2(d[0], d[1]), atan2(len2(d), d[2]), len(d)]
}

/// `0x2da480(out, s, c)`: the point at (yaw, elevation, distance) `s` about `c`.
fn sph_point(s: V3, c: V3) -> V3 {
    let [yaw, el, d] = s;
    add([d * el.cos() * yaw.cos(), d * el.cos() * yaw.sin(), d * el.sin()], c)
}

/// 20° (0x3eb2b8c2).
const SWING_LIMIT: f32 = 0.349_065_9;

/// `RC_PLAY_TRACE=1`: the switches and releases on stdout (a dev trace).
fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("RC_PLAY_TRACE").is_ok_and(|v| v.trim() == "1"))
}

impl ScriptCamera {
    /// The init `0x428b38`.
    fn init(&mut self) {
        self.spring_rot = Spring { k: 0.05, d: 0.3, max: 1.0, on: 1 };
        self.avel = [0.0; 3];
        self.spring_pos = Spring { k: 0.3, d: 0.3, max: 1.0, on: 1 };
        self.vel = [0.0; 3];
        self.rows = euler_rows(self.euler);
        self.ticks = 0;
    }

    /// The update `0x428bf8` with Ratchet's position (`0x189ea0`) and the view's yaw (`0x1b8d58`) as the last camera
    /// update left it.
    fn update(&mut self, hero: V3, view_yaw: f32) {
        self.ticks += 1;
        match self.mode {
            MODE_SNAP => self.snap(),
            MODE_MOVE => {
                crate::moby::countdown(&mut self.timer);
                let t = (self.total - self.timer) as f32 / self.total as f32;
                self.pos = cos_interp3(self.pos, self.target_pos, t);
                self.euler = rot_ease3(self.euler, self.target_euler, t);
            }
            MODE_SWING => {
                if crate::moby::countdown(&mut self.timer) { return self.snap_rows(); }
                let t = (self.total - self.timer) as f32 / self.total as f32;
                self.swing_tgt = spherical(self.target_pos, hero);
                let dyaw = sub_rot(self.swing_tgt[0], self.swing[0]);
                let dview = sub_rot(self.target_euler[2], view_yaw);
                let long_way = (SWING_LIMIT < dyaw && dview < -SWING_LIMIT) || (dyaw < -SWING_LIMIT && SWING_LIMIT < dview);
                if long_way {
                    let mut d = self.swing_tgt[0] - self.swing[0];
                    if (d < 0.0 && 0.0 < dview) || (0.0 < d && dview < 0.0) { d = -d; }
                    self.swing[0] += cos_interp(0.0, d, t);
                } else {
                    self.swing[0] = rot_ease(self.swing[0], self.swing_tgt[0], t);
                }
                let k = hermite(self.curve[0], 0.0, 1.0, self.curve[1], t);
                self.swing[2] += (self.swing_tgt[2] - self.swing[2]) * k;
                self.swing[1] = rot_ease(self.swing[1], self.swing_tgt[1], t);
                self.anchor = cos_interp3(self.anchor, hero, t);
                self.pos = sph_point(self.swing, self.anchor);
                self.euler = rot_ease3(self.euler, self.target_euler, t);
            }
            _ => {
                if self.spring_pos.on == 1 {
                    for k in 0..3 { self.pos[k] = spring(self.pos[k], self.target_pos[k], self.spring_pos, &mut self.vel[k]); }
                }
                if self.spring_rot.on == 1 {
                    for k in 0..3 { self.euler[k] = ang_spring(self.euler[k], self.target_euler[k], self.spring_rot, &mut self.avel[k]); }
                }
            }
        }
        self.rows = euler_rows(self.euler);
    }

    fn snap(&mut self) {
        self.pos = self.target_pos;
        self.euler = self.target_euler;
    }

    fn snap_rows(&mut self) {
        self.snap();
        self.rows = euler_rows(self.euler);
    }
}

impl Cameras {
    /// `0x2d7e50(5)`: the level's class-5 slot.
    pub fn script_slot(&self) -> Option<usize> { self.slots.iter().position(|s| s.class == CLASS_SCRIPT) }

    /// The script camera is the current camera.
    pub fn script_current(&self) -> bool { self.current_class() == CLASS_SCRIPT }

    /// The view as the last camera update left it (`0x1b8d40` and its Euler `0x1b8d50`): what the callers pass to
    /// start the script camera where the view is.
    pub fn view_euler(&self) -> (V3, V3) { (self.last_view.0, angles_of(&self.last_view.1)) }

    /// `0x428880(pos, euler, mode, ticks, collide)`: the script camera switched in and placed (module docs). Without
    /// a class-5 record nothing happens (the original would fault).
    pub fn camera_script(&mut self, pos: V3, euler: V3, mode: u8, ticks: i32, collide: bool, hero: &Hero) {
        let Some(slot) = self.script_slot() else { return };
        let old = self.last_view;
        // `0x2d8168`: the old camera's flags cleared, the new one current, its init.
        if let Some(p) = self.current.and_then(|p| self.slots.get_mut(p)) {
            p.release = 0;
            p.entered = 0;
        }
        self.current = Some(slot);
        let s = &mut self.script;
        s.init();
        self.slots[slot].entered = 1;
        (s.pos, s.euler, s.target_pos, s.target_euler) = (pos, euler, pos, euler);
        s.collide = collide;
        s.mode = mode;
        if mode == MODE_SWING {
            s.curve = [1.0, 0.0];
            s.pos = old.0;
            s.euler = angles_of(&old.1);
            s.anchor = hero.pos;
            s.swing = spherical(old.0, hero.pos);
        }
        if mode == MODE_MOVE || mode == MODE_SWING {
            s.timer = ticks;
            s.total = ticks;
        }
        s.rows = euler_rows(s.euler);
        self.last_view = (s.pos, s.rows);
        if trace() {
            let h = self.slots[slot].header;
            println!("script camera: in (slot {slot}, priority {}, kind {}, blend {}) mode {mode} ticks {ticks} at ({:.2}, {:.2}, {:.2}) euler ({:.2}, {:.2}, {:.2})", h.priority, h.kind, h.blend, s.pos[0], s.pos[1], s.pos[2], s.euler[0], s.euler[1], s.euler[2]);
        }
    }

    /// `0x4287b8` / `0x428810`: the script camera's targets (it need not be current).
    pub fn camera_script_targets(&mut self, pos: Option<V3>, euler: Option<V3>) {
        if let Some(p) = pos { self.script.target_pos = p; }
        if let Some(e) = euler { self.script.target_euler = e; }
    }

    /// `0x4289f0(kind)`: the current camera released (module docs); the next [`Cameras::tick`] hands over.
    pub fn camera_script2(&mut self, kind: u8, hero: &Hero) {
        let Some(cur) = self.current else { return };
        let script = self.slots[cur].class == CLASS_SCRIPT;
        if trace() {
            let s = &self.script;
            println!("script camera: release {kind} (class {} current, {} updates) at ({:.2}, {:.2}, {:.2})", self.slots[cur].class, s.ticks, s.pos[0], s.pos[1], s.pos[2]);
        }
        // `0x2d80a0`.
        self.slots[cur].release = 1;
        self.slots[cur].entered = 0;
        match kind {
            RELEASE_CUT_BEHIND => {
                if script {
                    let r = euler_rows(hero.rot);
                    let s = &mut self.script;
                    s.rows = r;
                    s.pos = add(add(hero.pos, set_len(r[2], 1.2)), set_len(r[0], -1.0));
                    s.placed = true;
                }
            }
            RELEASE_BLEND => self.slots[cur].release = 2,
            RELEASE_KEEP => self.slots[cur].release = 3,
            RELEASE_KEEP_RATES => self.slots[cur].release = 5,
            _ => {}
        }
    }

    /// The switch into the script camera by the choice itself (never on the disc's data: its hook says "go on" and
    /// its record is entered only by `0x428880`): the init alone.
    pub(crate) fn script_init(&mut self) { self.script.init(); }

    /// The update with the script camera current (`0x428ff0`): the view this tick.
    pub(crate) fn script_update(&mut self, hero: &Hero, out: &mut Out) {
        let yaw = angles_of(&self.last_view.1)[2];
        self.script.update(hero.pos, yaw);
        self.script.placed = false;
        out.view = Some((self.script.pos, self.script.rows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spherical_and_back() {
        let c = [10.0, -3.0, 2.0];
        let p = [14.0, 1.0, 5.0];
        let s = spherical(p, c);
        let q = sph_point(s, c);
        for k in 0..3 { assert!((q[k] - p[k]).abs() < 1e-4, "{q:?}"); }
    }

    #[test]
    fn move_mode_lands_on_its_targets_when_the_timer_runs_out() {
        let mut s = ScriptCamera { mode: MODE_MOVE, timer: 30, total: 30, ..Default::default() };
        s.target_pos = [3.0, -6.0, 9.0];
        s.target_euler = [0.0, 0.5, -2.0];
        let mut last = 0.0;
        for _ in 0..30 {
            s.update([0.0; 3], 0.0);
            assert!(last <= s.pos[0] && s.pos[0] <= 3.0);
            last = s.pos[0];
        }
        assert_eq!(s.pos, [3.0, -6.0, 9.0]);
        assert!((s.euler[2] + 2.0).abs() < 1e-5);
        // The rows look along the yaw: −2 rad is forward (cos −2, sin −2) turned up by the pitch.
        assert!((s.rows[0][0] - 0.5f32.cos() * (-2.0f32).cos()).abs() < 1e-5);
    }

    #[test]
    fn swing_mode_keeps_its_distance_about_the_hero() {
        let hero = [10.0, 0.0, 0.0];
        let mut s = ScriptCamera { mode: MODE_SWING, timer: 60, total: 60, curve: [1.0, 0.0], anchor: hero, ..Default::default() };
        s.pos = [5.0, 0.0, 0.0];
        s.swing = spherical(s.pos, hero);
        s.target_pos = [10.0, 5.0, 0.0];
        s.target_euler = [0.0, 0.0, -PI / 2.0];
        let mut dmin = f32::MAX;
        for _ in 0..59 {
            s.update(hero, 0.0);
            dmin = dmin.min(dist2(s.pos, hero));
        }
        assert!((dmin - 5.0).abs() < 1e-3, "{dmin}");
        s.update(hero, 0.0);
        assert_eq!(s.pos, s.target_pos);
        assert_eq!(s.euler, s.target_euler);
        assert!((hermite(1.0, 0.0, 1.0, 0.0, 0.5) - 0.5).abs() < 1e-6 && hermite(1.0, 0.0, 1.0, 0.0, 1.0) == 1.0);
    }

    #[test]
    fn spring_mode_never_overshoots() {
        let mut s = ScriptCamera { mode: MODE_SPRING, ..Default::default() };
        s.init();
        s.target_pos = [10.0, 0.0, 0.0];
        s.target_euler = [0.0, 0.0, 3.0];
        s.euler = [0.0, 0.0, -3.0];
        let mut last = 0.0;
        for _ in 0..100 {
            s.update([0.0; 3], 0.0);
            assert!(last <= s.pos[0] && s.pos[0] <= 10.0);
            last = s.pos[0];
        }
        // The angle spring took the short way across ±π.
        assert!((sub_rot(s.euler[2], 3.0)).abs() < 1e-2, "{}", s.euler[2]);
    }
}
