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
//! Not ported: the classes 4 (first person: Ratchet's camera mode 4 / 0x51), 5, 6, 7 (no Swingshot on Aranos), 0x17
//! (the placed view `0x42b950` / `0x42b428`: in its cuboid with Ratchet still it frames him from the record's place
//! through the follow camera's setters `0x425e50..0x426110`), 0x1b (its hook says no); the blends (`0x2d8ab0`,
//! `0x2d9488`: none between Aranos's cameras); the fly-by's look at a moby (+0x28 with the offset +0x2c: none on
//! Aranos); the pre hooks; the camera-moby list `0x2d80c0`.

use crate::hero::physics::*;
use crate::hero::Hero;
use crate::level::LevelData;
use crate::spline::{advance, Cursor};

pub const CLASS_FOLLOW: i16 = 0;
pub const CLASS_FLYBY: i16 = 0x13;

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
            // `0x2d8168`: the old camera's flags cleared; the new one's init.
            if let Some(p) = prev.and_then(|p| self.slots.get_mut(p)) {
                p.release = 0;
                p.entered = 0;
            }
            self.current = best;
            let prev_view = if prev.is_some_and(|p| self.slots[p].class == CLASS_FLYBY) { (self.pos, self.rows) } else { follow };
            match self.current_class() {
                CLASS_FLYBY => self.flyby_init(level, prev_view, &mut out),
                _ => out.follow_reset = true,
            }
        }
        if self.current_class() == CLASS_FLYBY {
            self.flyby_update(level, fade, fade_in, &mut out);
            out.view = Some((self.pos, self.rows));
        }
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
