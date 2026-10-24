//! The space fights' flight (L25 Wupash Nebula; the same code flies Feltzin 5, Hrugis 10, Gorn 15 and the orbits):
//! the level camera class 0x19 (the class table `0x2b6c40` row `{0x19, activate 0x438608, init 0x434478, update
//! 0x434670, pre 0x4386a8}`). The camera is the ship's frame: its rows (forward, left, up) are the ship's heading,
//! the stick turns them, and the ship moby (the record's `+0x20`, class 0x0fbf, a tenth of its size) is placed just
//! in front of the camera, banked into the turn. Ratchet is hidden meanwhile (state 0x31, the activation).
//!
//! The camera record (`0x1b19c4[+0x84]`, 0x20 each): the position, the angles `+0x10`, the pvars `+0x1c` (`+0x20` the
//! ship's moby, `+0x24..+0x50` 12 spheres it must not leave, `+0x58` a hit timer, `+0x5c` a save byte). The camera's
//! D block (`+0x70`) and the ship's pvars hold the state, kept here in [`Flight`].
//!
//! The update `0x434670` (mode `$gp+0x2630`: 0 flying; 1 / 2 / 3 / 4 / 5 the fades of the intro and the respawn; 6 the
//! death; 7 / 8 the U-turn) in mode 0: the steering `0x435190`, the turn `0x4358d8`, the move `0x435a18`, the ship
//! `0x435e78`.
//!
//! The ship's body is its parts (0x0fbf's start, `0x4112e8`: `0x415ca8(0.05, ship, side, class)` each, the canopy
//! `0x4247c8`), chosen by the ship's upgrades (`0x1a7af8`: the class tables `0x1a8f70..`): with none the hull 0x10d1,
//! the wings 0x10d4 (both sides), 0x10d7, the canopy 0x10e3 (alpha 0x60), 0x10da, 0x10de and 0x10e5 (both sides).
//! Each tick (`0x411aa8` → `0x415d88`) they take the ship's place and rows, the second side's left row turned over.
//!
//! Not ported: the other modes (the fades, the death `0x434ad0`, the U-turn `0x434c98`), the auto level `0x4356c8`,
//! the boost (`0x22183c`, `0x375168`), Gorn's and Hrugis's special cases (levels 0xf, 0x14, 5 / 10), the spheres the
//! ship must stay in (the mission object 0x0eee's `+0x68`[0]), the collision `0x437a70`, the guns but the
//! lasers (the missiles ○ `0x3caae8`, the weapons 2 / 3 / 4, the boost □ and its fuel `0x29c300` slot 7), the
//! upgrades' parts, the ship's light and draws (`0x436d20`, `0x437258`, `0x4383d8`).
//!
//! **The lasers** (`0x436440`, ✕): every 10 ticks (`$gp+0x25b0`; during a barrel roll only on even frames) a shot
//! from each gun (`$gp+0x2640` / `+0x2650`: 1.23 ahead, ±0.045 across, 0.055 up in the ship's frame) toward the point
//! 50 along the aim (`+0x25ac`). The shot (`0x413268`, class 0xffa) flies at 125 a second for 256 (`$gp+0x1df0`,
//! `+0x1df8`), its hit radius growing from 0.6 to 8 (`+0x1e14`, `+0x1e18`), red (0x2020ff; an enemy's blue). Its
//! update (`0x412e38`): the step; gone at the end of its time or out of the 2..1021 box; the step's line against the
//! world (`0x2f1998`, not the material 6) ends it; a moby (`0x2f45d0`) whose centre comes within the radius of the
//! step takes a hit (1, flags 0x110000; `0x32f910`) and ends it. Its draw (`0x4137d0`, meshes of FX 0xb and 8 with a
//! spinning star) is drawn here as an additive streak. Not ported: the shots' sounds (`0x36cb90`), the burst of the
//! end (`0x4134b0`), an enemy's shots hitting the ship.

use crate::hero::physics::{add, cross, dot, scale, set_len, sub, unit, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{countdown, mode, Ctx, MobyHost, MobyWorld};
use crate::pad::{button, PadState};

/// The camera class.
pub const CLASS: i16 = 0x19;
/// The ship's moby class.
pub const SHIP_CLASS: i16 = 0x0fbf;

const DEG: f32 = 0.017_453_294;

/// `0x2eafb8(x, t, k, d, max, &v)`: `v += k·(t − x) − d·v`, clamped to ±max (max ≠ 0) and to ±|t − x|; x + v.
fn spring(x: f32, t: f32, k: f32, d: f32, max: f32, v: &mut f32) -> f32 {
    let e = t - x;
    *v += k * e - d * *v;
    if max != 0.0 { *v = v.clamp(-max, max); }
    *v = v.clamp(-e.abs(), e.abs());
    x + *v
}

/// `0x331ed0` / `0x333850`: `v` turned by `a` about the unit `axis` (the quaternion's rotation).
fn turn(v: V3, axis: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    let kxv = cross(axis, v);
    add(add(scale(v, c), scale(kxv, s)), scale(axis, dot(axis, v) * (1.0 - c)))
}

/// `0x2ef318`: `v` through the rows (the rows as columns).
fn through(r: &[V3; 3], v: V3) -> V3 { add(add(scale(r[0], v[0]), scale(r[1], v[1])), scale(r[2], v[2])) }

/// `0x301608` (s16): 1 already 0, 2 just ran out, else 0.
fn dec_timer(t: &mut i16) -> i32 {
    if *t == 0 { return 1; }
    *t = (*t).max(1) - 1;
    if *t > 0 { 0 } else { 2 }
}

/// The flight's state: the camera (its rows and position), its D block and the ship's pvars.
#[derive(Clone, Debug)]
pub struct Flight {
    pub pos: V3,
    pub rows: [V3; 3],
    /// The ship's moby.
    pub ship: Option<usize>,
    /// Its parts (the moby, the second side).
    parts: Vec<(usize, bool)>,
    /// The ship's `+8`: the lasers' wait; the aim point (`+0x20`); the frames flown (`0x1b1b48`'s parity).
    gun_wait: i16,
    aim: V3,
    frame: u32,
    /// `$gp+0x2630`.
    pub mode: i32,
    /// `$gp+0x2684` / `+0x2688`: the d-pad's yaw and pitch ramps (0.1 a tick to 1).
    ramp_yaw: f32,
    ramp_pitch: f32,
    /// D `+0xb4` / `+0xb8` the pitch and yaw this tick (radians), `+0xa4` / `+0xa8` their springs' velocities.
    pitch: f32,
    yaw: f32,
    pitch_v: f32,
    yaw_v: f32,
    /// D `+0x40` / `+0x44`: the smoothed pitch input (Gorn's climb).
    pitch_in: f32,
    pitch_in_v: f32,
    /// D `+0xd8` the barrel roll's side (±1, 0 none), `+0xda` its ticks left, `+0xdc` 1 / its length, `+0xe0` the
    /// wait before the next.
    roll_dir: i16,
    roll_t: i16,
    roll_inv: f32,
    roll_wait: i16,
    /// The ship's `+0x38` / `+0x3c`: the speed a tick and its spring's velocity; `+0xc` / `+0x10` the dip and its
    /// velocity.
    speed: f32,
    speed_v: f32,
    dip: f32,
    dip_v: f32,
}

/// `$gp+0x2550..`: the tuning (read from the disc's data at `$gp` = 0x1aeff0).
mod k {
    /// `+0x2578`: the stick's dead zone for pitch.
    pub const DEAD: f32 = 0.3;
    /// `+0x268c`: the d-pad ramp a tick.
    pub const RAMP: f32 = 0.1;
    /// `+0x2550..+0x2558`: the pitch input's spring.
    pub const PITCH_IN: (f32, f32, f32) = (0.0075, 0.4, 0.0);
    /// `+0x2568`, `+0x256c`, `+0x2574`, `+0x2580` (D `+0x5c`, `+0x60`, `+0x64`, `+0xc4`): the pitch's spring and its
    /// most (degrees a tick); `+0x255c`, `+0x2560`, `+0x2564`, `+0x257c` (D `+0x50`, `+0x54`, `+0x58`, `+0xc8`): the
    /// yaw's.
    pub const PITCH: (f32, f32, f32, f32) = (0.005, 0.3, 0.0, 2.5);
    pub const YAW: (f32, f32, f32, f32) = (0.0125, 0.3, 0.0, 1.5);
    /// `+0x25d0`: the cruise speed a second; `+0x2704` / 0.2: its spring.
    pub const CRUISE: f32 = 8.0;
    pub const SPEED_K: f32 = 0.02;
    pub const SPEED_D: f32 = 0.2;
    /// `+0x25f8` the barrel roll's ticks, `+0x25fc` the wait after it.
    pub const ROLL_TICKS: i16 = 60;
    pub const ROLL_WAIT: i16 = 2;
    /// `+0x26f0`: the roll's sideways step.
    pub const ROLL_STEP: f32 = 0.25;
    /// `+0x258c` / `+0x2590`: the ship ahead of the camera and below it; `+0x2594` its scale.
    pub const AHEAD: f32 = 1.15;
    pub const BELOW: f32 = -0.162_55;
    pub const SHIP_SCALE: f32 = 0.1;
    /// The ship's `+0x240..+0x25c` (`0x434fb8` from `+0x25c4..+0x25cc`, `+0x2598..+0x25a8`): the aim point ahead,
    /// across and up; the dip's share down, the sideways and the dip's offsets, the bank (degrees), the aim's
    /// distance.
    pub const AIM: (f32, f32, f32) = (2.0, 1.0, 0.6);
    pub const DIP_DOWN: f32 = 0.6;
    pub const SIDE: f32 = 0.4;
    pub const DIP: f32 = 0.5;
    pub const BANK: f32 = 45.0;
    pub const AIM_FAR: f32 = 10.0;
    /// `+0x2708` / `+0x270c`: the dip's spring.
    pub const DIP_K: (f32, f32) = (0.006, 0.05);
    /// `+0x2710`: the ship's offset in its frame (× 0.05).
    pub const NUDGE: [f32; 3] = [0.0, 0.0, -1.355];
}

impl Flight {
    /// `0x434478` (the init, with `0x4345d8`): the camera at the record's place, its rows from the record's angles;
    /// the springs and the roll cleared, the speed the cruise.
    pub fn new(pos: V3, rot: V3, ship: Option<usize>) -> Flight {
        Flight {
            pos,
            rows: crate::moby::attach::rows_of(rot),
            ship,
            parts: Vec::new(),
            gun_wait: 0,
            aim: pos,
            frame: 0,
            mode: 0,
            ramp_yaw: 0.0,
            ramp_pitch: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            pitch_v: 0.0,
            yaw_v: 0.0,
            pitch_in: 0.0,
            pitch_in_v: 0.0,
            roll_dir: 0,
            roll_t: 0,
            roll_inv: 0.0,
            roll_wait: 0,
            speed: k::CRUISE / 60.0,
            speed_v: 0.0,
            dip: 0.0,
            dip_v: 0.0,
        }
    }

    /// `0x434670`, mode 0; the ship's parts made on the first tick (0x0fbf's start) and moved with it.
    pub fn tick(&mut self, pad: &PadState, w: &mut MobyWorld, host: &dyn MobyHost) {
        if self.ship.is_some() && self.parts.is_empty() { self.make_parts(w, host); }
        if self.mode != 0 { return; }
        self.frame = self.frame.wrapping_add(1);
        let (pitch, yaw) = self.steer(pad);
        self.turn(pitch, yaw);
        self.advance(w);
        self.place_ship(w);
        self.follow(w, host);
        self.fire(pad, w, host);
    }

    /// `0x436440`'s lasers.
    fn fire(&mut self, pad: &PadState, w: &mut MobyWorld, host: &dyn MobyHost) {
        if self.roll_dir != 0 && self.frame & 1 != 0 { return; }
        let Some(ship) = self.ship else { return };
        if dec_timer(&mut self.gun_wait) == 0 || pad.held & button::CROSS == 0 { return; }
        self.gun_wait = laser::WAIT;
        let target = add(self.pos, set_len(sub(self.aim, self.pos), laser::AIM));
        let (pos, rows) = (w.mobys[ship].pos, w.mobys[ship].rows);
        for gun in laser::GUNS {
            let at = add(pos, through(&rows, gun));
            spawn_laser(w, host, at, sub(target, at), rows, ship);
        }
    }

    /// `0x415ca8` / `0x4247c8` for the parts of a ship without upgrades.
    fn make_parts(&mut self, w: &mut MobyWorld, host: &dyn MobyHost) {
        const PARTS: [(i16, bool); 11] = [
            (0x10d1, false), (0x10d4, false), (0x10d4, true), (0x10d7, false), (0x10e3, false),
            (0x10da, false), (0x10da, true), (0x10de, false), (0x10de, true), (0x10e5, false), (0x10e5, true),
        ];
        let Some(ship) = self.ship else { return };
        for (class, side) in PARTS {
            if w.class(class).is_none() { continue; }
            let id = w.create(class);
            let base = w.class(class).map_or(1.0, |c| c.scale);
            let (pos, rows) = (w.mobys[ship].pos, w.mobys[ship].rows);
            let m = &mut w.mobys[id];
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
            m.drawn = 1;
            m.mode |= mode::KEEP_ROWS;
            if class == 0x10e3 {
                m.alpha = 0x60;
                m.mode |= 0x8;
            }
            m.state = 0;
            m.pos = pos;
            m.rows = rows;
            m.scale = base * 0.05;
            w.rebuild(id, host);
            self.parts.push((id, side));
        }
        w.changed = true;
    }

    /// `0x411aa8` (`0x415d88` / `0x4248a0`): the parts in the ship's place with its rows (the second side's left row
    /// turned over).
    fn follow(&mut self, w: &mut MobyWorld, host: &dyn MobyHost) {
        let Some(ship) = self.ship else { return };
        let (pos, rows) = (w.mobys[ship].pos, w.mobys[ship].rows);
        for &(id, side) in &self.parts {
            let Some(m) = w.mobys.get_mut(id) else { continue };
            m.pos = pos;
            m.rows = rows;
            if side { m.rows[1] = scale(rows[1], -1.0); }
            w.rebuild(id, host);
        }
    }

    /// `0x435190`: the stick (or the d-pad's ramps) to the pitch and yaw targets (squared), their springs; L1 / R1
    /// start a barrel roll.
    fn steer(&mut self, pad: &PadState) -> (f32, f32) {
        let held = pad.held;
        let ly = pad.ly.to_f32();
        let nx = -pad.lx.to_f32();
        let mut p = self.ramp_pitch;
        if ly.abs() < k::DEAD {
            if ly == 0.0 {
                if held & button::UP != 0 {
                    p = -self.ramp_pitch;
                    self.ramp_pitch = (self.ramp_pitch + k::RAMP).min(1.0);
                } else if held & button::DOWN != 0 {
                    self.ramp_pitch = (self.ramp_pitch + k::RAMP).min(1.0);
                } else {
                    self.ramp_pitch = 0.0;
                    p = 0.0;
                }
            } else {
                self.ramp_pitch = 0.0;
                p = 0.0;
            }
        } else {
            self.ramp_pitch = 0.0;
            p = (if 0.0 < ly { ly - k::DEAD } else { ly + k::DEAD }) / (1.0 - k::DEAD);
        }
        let mut y = self.ramp_yaw;
        let mut ramped = false;
        if nx == 0.0 {
            if held & button::LEFT != 0 {
                self.ramp_yaw = (self.ramp_yaw + k::RAMP).min(1.0);
                ramped = true;
            } else if held & button::RIGHT != 0 {
                y = -self.ramp_yaw;
                self.ramp_yaw = (self.ramp_yaw + k::RAMP).min(1.0);
                ramped = true;
            }
        }
        if !ramped {
            self.ramp_yaw = 0.0;
            y = nx;
        }
        let sq = |v: f32| if v < 0.0 { -(v * v) } else { v * v };
        let (y, p) = (sq(y), sq(p));
        let pitch_t = -p * DEG * k::PITCH.3;
        let yaw_t = y * DEG * k::YAW.3;
        self.pitch = spring(self.pitch, pitch_t, k::PITCH.0, k::PITCH.1, k::PITCH.2, &mut self.pitch_v);
        self.yaw = spring(self.yaw, yaw_t, k::YAW.0, k::YAW.1, k::YAW.2, &mut self.yaw_v);
        self.pitch_in = spring(self.pitch_in, p, k::PITCH_IN.0, k::PITCH_IN.1, k::PITCH_IN.2, &mut self.pitch_in_v);
        // The barrel roll.
        if self.roll_t != 0 && dec_timer(&mut self.roll_t) != 0 { self.roll_dir = 0; }
        if self.roll_wait == 0 {
            if held & (button::L1 | button::R1) != 0 {
                self.roll_t = k::ROLL_TICKS;
                self.roll_wait = k::ROLL_TICKS + k::ROLL_WAIT;
                self.roll_inv = 1.0 / k::ROLL_TICKS as f32;
                self.roll_dir = if held & button::L1 == 0 { 1 } else { -1 };
            }
        } else {
            dec_timer(&mut self.roll_wait);
        }
        (self.pitch, self.yaw)
    }

    /// `0x4358d8`: the yaw about the up row (turning forward and left), the pitch about the left row (forward and up).
    fn turn(&mut self, pitch: f32, yaw: f32) {
        let r = &mut self.rows;
        if 1e-5 < yaw.abs() {
            let a = r[2];
            r[0] = turn(r[0], a, yaw);
            r[1] = turn(r[1], a, yaw);
        }
        if 1e-5 < pitch.abs() {
            let a = r[1];
            r[0] = turn(r[0], a, pitch);
            r[2] = turn(r[2], a, pitch);
        }
        // Kept orthonormal (the quaternion turns keep them so; this only holds off the float drift).
        r[0] = unit(r[0]);
        r[1] = unit(cross(r[2], r[0]));
        r[2] = cross(r[0], r[1]);
    }

    /// `0x435a18`: the speed springs to the cruise; the camera moves along the ship's forward; the barrel roll's side
    /// step; each axis kept within 5..1000.
    fn advance(&mut self, w: &MobyWorld) {
        self.speed = spring(self.speed, k::CRUISE / 60.0, k::SPEED_K, k::SPEED_D, 0.0, &mut self.speed_v);
        let fwd = self.ship.and_then(|i| w.mobys.get(i)).map_or(self.rows[0], |m| m.rows[0]);
        self.pos = add(self.pos, scale(fwd, self.speed));
        if self.roll_dir != 0 {
            let t = self.roll_t as f32 * self.roll_inv * std::f32::consts::PI;
            let step = k::ROLL_STEP * t.sin() * if self.roll_dir == 1 { -1.0 } else { 1.0 };
            self.pos = add(self.pos, scale(self.rows[1], step));
        }
        for c in &mut self.pos { *c = c.clamp(5.0, 1000.0); }
    }

    /// `0x435e78`: the ship a tenth of its size just ahead of and below the camera, shifted across by the yaw and down
    /// by the dip, facing the aim point (`0x436350`) far ahead, banked by the yaw (45° at the most) and spun by the
    /// barrel roll.
    fn place_ship(&mut self, w: &mut MobyWorld) {
        let Some(id) = self.ship else { return };
        let class_scale = w.class(SHIP_CLASS).map_or(1.0, |c| c.scale);
        let r = self.rows;
        let yaw_share = -self.yaw / (k::YAW.3 * DEG);
        let mut p = add(add(self.pos, scale(r[0], k::AHEAD)), scale(r[2], k::BELOW));
        p = add(p, scale(r[1], k::SIDE * yaw_share.clamp(-1.0, 1.0)));
        let mut d = self.pitch / (k::PITCH.3 * DEG);
        d = if d < 0.0 { (d * k::DIP_DOWN).max(-1.0) } else { d.min(1.0) };
        self.dip = spring(self.dip, d, k::DIP_K.0, k::DIP_K.1, 0.0, &mut self.dip_v);
        p = add(p, scale(r[2], k::DIP * self.dip));
        // `0x436350`: the aim point.
        let across = self.yaw / (k::YAW.3 * DEG) * k::AIM.1;
        let up = -self.pitch / (k::PITCH.3 * DEG) * k::AIM.2;
        let aim = add(add(add(self.pos, scale(r[0], k::AIM.0)), scale(r[1], across)), scale(r[2], up));
        self.aim = aim;
        let far = add(self.pos, set_len(sub(aim, self.pos), k::AIM_FAR));
        let f = unit(sub(far, p));
        let l = unit(cross(r[2], f));
        let u = cross(f, l);
        let mut roll = yaw_share.clamp(-1.0, 1.0) * DEG * k::BANK;
        if self.roll_dir != 0 {
            let t = 1.0 - self.roll_t as f32 * self.roll_inv;
            let eased = (1.0 - (t * std::f32::consts::PI).cos()) * 0.5;
            let spin = crate::hero::physics::add_rot(0.0, eased * 4.0 * std::f32::consts::PI);
            roll = crate::hero::physics::add_rot(roll, if self.roll_dir == -1 { -spin } else { spin });
        }
        let (l, u) = if 1e-5 < roll.abs() { (turn(l, f, roll), turn(u, f, roll)) } else { (l, u) };
        let rows = [f, l, u];
        p = add(p, through(&rows, scale(k::NUDGE, 0.05)));
        let m = &mut w.mobys[id];
        m.scale = class_scale * k::SHIP_SCALE;
        m.mode |= mode::KEEP_ROWS;
        m.rows = rows;
        m.pos = p;
        w.changed = true;
    }
}

/// The laser shot's tuning (`$gp+0x1df0..`, `+0x2640..`, `+0x25ac`, `+0x25b0`).
mod laser {
    pub const SPEED: f32 = 125.0;
    pub const RANGE: f32 = 256.0;
    pub const RADIUS: (f32, f32) = (0.6, 8.0);
    pub const WAIT: i16 = 10;
    pub const AIM: f32 = 50.0;
    pub const GUNS: [[f32; 3]; 2] = [[1.23, 0.045, 0.055], [1.23, -0.045, 0.055]];
    /// The ship's shot's colour (`+0x20`).
    pub const RGB: u32 = 0x2020ff;
}

/// The laser's class.
pub const LASER_CLASS: i16 = 0x0ffa;

/// `0x413268(side, owner, at, dir, …)` for the ship's lasers. pvars: `+0` the ticks left, `+4` the owner (index + 1),
/// `+0x10` the step, `+0x20` the colour, `+0x3c` the radius, `+0x40` its growth a tick.
fn spawn_laser(w: &mut MobyWorld, host: &dyn MobyHost, at: V3, dir: V3, rows: [V3; 3], owner: usize) {
    if w.class(LASER_CLASS).is_none() { return; }
    let speed = laser::SPEED / 60.0;
    let ticks = (laser::RANGE / speed) as i32;
    let id = w.create(LASER_CLASS);
    let m = &mut w.mobys[id];
    m.state = 0;
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.drawn = 1;
    m.mode |= mode::KEEP_ROWS;
    m.rows = rows;
    m.pos = at;
    m.set_pv_i(0, ticks);
    m.set_pv_i(4, owner as i32 + 1);
    m.set_pv_v(0x10, set_len(dir, speed));
    m.set_pv_i(0x20, laser::RGB as i32);
    m.set_pv_f(0x3c, laser::RADIUS.0);
    m.set_pv_f(0x40, (laser::RADIUS.1 - laser::RADIUS.0) / ticks.max(1) as f32);
    w.rebuild(id, host);
}

/// `0x412e38`.
pub fn update_laser(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let from = w.mobys[id].pos;
    let step = w.mobys[id].pv_v(0x10);
    let to = add(from, step);
    w.mobys[id].pos = to;
    let mut left = w.mobys[id].pv_i(0);
    let over = countdown(&mut left);
    w.mobys[id].set_pv_i(0, left);
    if over || to.iter().any(|&c| !(2.0..=1021.0).contains(&c)) { return w.delete(id); }
    let r = w.mobys[id].pv_f(0x3c) + w.mobys[id].pv_f(0x40);
    w.mobys[id].set_pv_f(0x3c, r);
    if ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, from, to, crate::collision::QueryFlags(0), ctx.hero_moby)).is_some() {
        return w.delete(id);
    }
    let owner = w.mobys[id].pv_i(4) - 1;
    let seg = sub(to, from);
    let seg_len2 = dot(seg, seg);
    let hit = (0..w.mobys.len()).find(|&i| {
        let m = &w.mobys[i];
        if i == id || i as i32 == owner || m.state >= 0x80 || m.mode & mode::TARGET == 0 { return false; }
        let t = if 0.0 < seg_len2 { (dot(sub(m.pos, from), seg) / seg_len2).clamp(0.0, 1.0) } else { 0.0 };
        crate::hero::physics::dist(add(from, scale(seg, t)), m.pos) < r
    });
    if let Some(t) = hit {
        let d = unit(seg);
        let tmpl = HitTemplate::new(1.0, Some(id), 0x0011_0000, [d[0], d[1], d[2], 0.0]);
        let p = w.mobys[t].pos;
        w.deliver_hit(t, &tmpl, [p[0], p[1], p[2], 1.0]);
        return w.delete(id);
    }
    w.register_draw(id);
}

/// The laser's streak (standing in for `0x4137d0`'s meshes): FX 0xe, additive, over its last step and a half, a thin
/// bright core and a wide faint glow in its colour.
pub fn draw_laser(w: &MobyWorld, id: usize, cam: V3, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    let step = m.pv_v(0x10);
    let (a, b) = (sub(m.pos, scale(step, 1.5)), m.pos);
    let rgb = m.pv_i(0x20) as u32 & 0xff_ffff;
    let st = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    for (half, c) in [(0.03, 0x7fa0_a0ffu32), (0.12, 0x6000_0000 | rgb)] {
        let side = set_len(cross(sub(b, a), sub(cam, a)), half);
        out.push(crate::fx::FxQuad { fx: 0xe, additive: true, corners: [add(a, side), sub(a, side), add(b, side), sub(b, side)], st, rgba: [c; 4] });
    }
}
