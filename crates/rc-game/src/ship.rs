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
//! lasers and the boost (the missiles ○ `0x3caae8`, the weapons 2 / 3 / 4), the
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
//!
//! **The hits** (`0x375e78`): each takes 1 from the shield (34), then from the hull (100); the hull at 0 is the
//! death (`0x434ad0`, mode 6: the ship gone, the camera drifting back 0.133 a tick for 60 ticks, then `0x2c63f0`,
//! here the level again). Not ported: the shield's upgrades, the half damage of `0x4380b8`, the burst.
//!
//! **The boost** (`0x436440`, □; the weapon slot 7 of `0x29c300`, 0x28 each: `+0` had, `+0x10` the most fuel 600,
//! `+0x14` the fuel): held with fuel, the ship's `+0x34` counts up to 8 and the fuel down by 1 a tick; else `+0x34`
//! counts down, and at 0 with □ up the fuel comes back by 1 every third frame. With `+0x34` the move (`0x435a18`)
//! springs the speed to 50 a second (`$gp+0x25d4`; 30 with `0x375168`) at 0.005 (`+0x2700`) instead of the cruise.
//! Not ported: its sound (`0x36cc38(0x11, …)`).

use crate::hero::physics::{add, cross, dot, scale, set_len, sub, unit, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{countdown, mode, Ctx, MobyHost, MobyWorld};
use crate::pad::{button, PadState};

/// The camera class.
pub const CLASS: i16 = 0x19;
/// The ship's moby class.
pub const SHIP_CLASS: i16 = 0x0fbf;

const DEG: f32 = 0.017_453_294;
/// The view's half-angle tangent (`0x1bd2f0` in play) for the screen tests.
const TAN: f32 = 0.62;

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
    /// The missiles: `+10` their wait, the weapon slot 1's ammo, the lock's candidate (`+0x14`), its ticks (`+0x1c`)
    /// and the lock (`+0x18`).
    missile_wait: i16,
    pub missiles: i32,
    candidate: Option<usize>,
    lock_ticks: i32,
    locked: Option<usize>,
    aim: V3,
    frame: u32,
    /// The ship's `+0x34` (the boost's ramp, 0..8) and the boost's fuel (slot 7's `+0x14`).
    boost: i32,
    fuel: i32,
    /// `0x221828` the hull (100), the shield (the weapon slot 6's `+0x14`, 34); `$gp+0x2634` the death's ticks; the
    /// camera's place at the death (`0x1b3100`).
    pub hull: i32,
    pub shield: i32,
    dying: i32,
    /// The death is over: the fight starts again (`0x2c63f0`, the engine's).
    pub restart: bool,
    /// The ram's wait (the camera record's `+0x58`, 15 ticks).
    ram_wait: i32,
    /// The straightening (D `+0x6e` its state, `+0x6c` its ticks, `+0x70` 1 / them; running).
    level_state: u8,
    level_ticks: i16,
    level_inv: f32,
    levelling: bool,
    /// The sounds' handles: the engine's hum (the ship's `+0xc`), the boost's loop, the lock's loop (`+0x270`), the
    /// lock heard (`+0x26c`).
    hum: i32,
    boost_sound: i32,
    lock_sound: i32,
    lock_heard: bool,
    /// The lock marker's turn (`+0x27c`).
    lock_spin: f32,
    hits_heard: u32,
    hit_sound: bool,
    /// The shield bubble's texels' turns, its hit flash's ticks (a hit this tick); the dust's specks and their runs.
    shell_spin: [f32; 3],
    hit_flash: i32,
    hit_flash_new: bool,
    dust: [V3; 16],
    dust_left: [f32; 16],
    /// The camera's shake: its size, ticks left and length.
    shake: f32,
    shake_ticks: i32,
    shake_len: i32,
    /// The opening help shown; the closing one asked.
    pub opening_shown: bool,
    closing_asked: bool,
    /// The help texts the mission asks for (`0x307d80(text, slot)`, the engine's [`crate::game::GameState::help`]).
    pub help: Option<(i32, i32)>,
    /// The fight's sphere (its centre and radius), for the camera's clamp.
    pub arena: Option<(V3, f32)>,
    /// The mission (0x0eee, [`Flight::mission`]): its state (1 fighting, 3 the cut), the quiet start's ticks, the
    /// wait's, the waves done, the cut's ticks, the place kept over the cut, the screen's fade, the level won.
    mission_state: i32,
    quiet: i32,
    wave_wait: i32,
    pub waves: i32,
    cut_ticks: i32,
    saved: (V3, [V3; 3]),
    pub fade: f32,
    pub done: bool,
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
    /// `+0x25d0`: the cruise speed a second; `+0x2704` / 0.2: its spring; `+0x25d4` / `+0x2700` the boost's.
    pub const CRUISE: f32 = 8.0;
    pub const SPEED_K: f32 = 0.02;
    pub const BOOST: f32 = 50.0;
    pub const BOOST_K: f32 = 0.005;
    /// The boost's fuel (slot 7's `+0x10`).
    pub const FUEL: i32 = 600;
    /// The shield (slot 6's `+0x10`); `+0x25ec` the death's ticks; `0x434670`'s 0.133 back a tick.
    pub const SHIELD: i32 = 34;
    pub const DEATH_TICKS: i32 = 60;
    pub const DEATH_BACK: f32 = 0.133_333_34;
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
            missile_wait: 0,
            missiles: missile::AMMO,
            candidate: None,
            lock_ticks: 0,
            locked: None,
            aim: pos,
            frame: 0,
            boost: 0,
            fuel: k::FUEL,
            hull: 100,
            shield: k::SHIELD,
            dying: 0,
            restart: false,
            ram_wait: 0,
            level_state: 0,
            level_ticks: 0,
            level_inv: 0.0,
            levelling: false,
            hum: -1,
            boost_sound: -1,
            lock_sound: -1,
            lock_heard: false,
            lock_spin: 0.0,
            hits_heard: 0,
            hit_sound: false,
            shell_spin: [0.0; 3],
            hit_flash: 0,
            hit_flash_new: false,
            dust: [[0.0; 3]; 16],
            dust_left: [0.0; 16],
            shake: 0.0,
            shake_ticks: 0,
            shake_len: 0,
            opening_shown: false,
            closing_asked: false,
            help: None,
            arena: None,
            mission_state: 0,
            quiet: 0,
            wave_wait: 0,
            waves: 0,
            cut_ticks: 0,
            saved: (pos, [[0.0; 3]; 3]),
            fade: 0.0,
            done: false,
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
    pub fn tick(&mut self, pad: &PadState, w: &mut MobyWorld, host: &mut dyn MobyHost, coll: Option<&rc_formats::collision::Collision>, scene: Option<&crate::collision::mobys::MobyScene>) {
        if self.ship.is_some() && self.parts.is_empty() { self.make_parts(w, &*host); }
        self.engine_hum(w, host);
        self.take_hits(w);
        if let (true, Some(s)) = (self.hit_sound, self.ship) { host.play_sound_of(&w.mobys[s], SHIP_CLASS, 0xe, 0); }
        self.shake_tick(w);
        if self.mode == 6 { return self.dying(w); }
        self.mission(w);
        if !self.cut() { return; }
        self.frame = self.frame.wrapping_add(1);
        let (pitch, yaw) = self.steer(pad);
        self.turn(pitch, yaw);
        self.advance(w);
        let before = self.ship.map(|s| w.mobys[s].pos);
        self.place_ship(w);
        if let Some(from) = before { self.collide(w, coll, scene, from); }
        if self.mode == 6 { return; }
        self.follow(w, &*host);
        self.fire(pad, w, host);
        self.lock(w, TAN);
        self.lock_sounds(w, host);
        self.fire_missile(pad, w, host);
        // The boost's ramp in the ship's `+0x34` (its flame's, [`draw_flame`]).
        if let Some(ship) = self.ship { w.mobys[ship].set_pv_i(0x34, self.boost); }
        self.cut_after();
    }

    /// `0x375e78(1)` for each shot that hit: the shield first, then the hull; at 0 the death (`0x434ad0`: mode 6,
    /// the ship gone, 60 ticks).
    fn take_hits(&mut self, w: &mut MobyWorld) {
        self.hit_sound = false;
        self.hit_flash_new = false;
        let Some(ship) = self.ship else { return };
        let add = w.mobys[ship].pv_i(SHIELD_ADD);
        if add != 0 {
            self.shield = (self.shield + add).min(k::SHIELD);
            w.mobys[ship].set_pv_i(SHIELD_ADD, 0);
        }
        w.mobys[ship].set_pv_i(SHIELD_NOW, self.shield);
        let n = w.mobys[ship].pv_i(HITS);
        if n == 0 { return; }
        w.mobys[ship].set_pv_i(HITS, 0);
        self.hit_flash_new = true;
        // `0x375410`: one hit in six is heard (`0x36cc38(0xe, 0, ship, 0xfbf)`).
        self.hits_heard = self.hits_heard.wrapping_add(1);
        if self.hits_heard % 6 == 1 { self.hit_sound = true; }
        if self.mode == 6 { return; }
        self.damage(w, n);
    }

    /// `0x375e78(n)`: the shield takes it first, then the hull; at 0 the death.
    fn damage(&mut self, w: &mut MobyWorld, n: i32) {
        if self.mode == 6 { return; }
        let from_shield = n.min(self.shield);
        self.shield -= from_shield;
        self.hull -= n - from_shield;
        if self.hull < 1 { self.die(w); }
    }

    /// `0x434ad0`: mode 6, the ship and its parts gone, 60 ticks.
    fn die(&mut self, w: &mut MobyWorld) {
        if self.mode == 6 { return; }
        if std::env::var_os("RC_SHIP_TRACE").is_some() { println!("ship: death at {:?} mode {} hull {}", self.pos, self.mode, self.hull); }
        self.hull = self.hull.max(0);
        self.mode = 6;
        self.fade = 0.0;
        self.dying = k::DEATH_TICKS;
        // `0x3cbfe8(ship, 0, 1)`: the blast, by the mission object ([`update_mission`]).
        if let (Some(s), Some(mid)) = (self.ship, w.mobys.iter().position(|m| m.o_class == MISSION_CLASS && m.state < 0x80)) {
            let at = w.mobys[s].pos;
            w.mobys[mid].set_pv_v(BOOM, at);
            w.mobys[mid].set_pv_i(BOOM + 0xc, 1);
            w.mobys[mid].mode &= !mode::NO_UPDATE;
            w.mobys[mid].update_dist = 0xff;
        }
        for id in self.ship.into_iter().chain(self.parts.iter().map(|p| p.0)) {
            let m = &mut w.mobys[id];
            m.draw_dist = 0;
            m.drawn = 0;
        }
        w.changed = true;
    }

    /// `0x434670` mode 6: the camera drifts back (`0x434bf0`) until the death's ticks run out.
    fn dying(&mut self, _w: &mut MobyWorld) {
        self.pos = sub(self.pos, scale(self.rows[0], k::DEATH_BACK));
        self.dying -= 1;
        if self.dying <= 0 { self.restart = true; }
    }

    /// `0x436440`'s lasers and boost.
    fn fire(&mut self, pad: &PadState, w: &mut MobyWorld, host: &mut dyn MobyHost) {
        if self.roll_dir != 0 && self.frame & 1 != 0 { return; }
        let square = pad.held & button::SQUARE != 0;
        if self.fuel == 0 || !square {
            countdown(&mut self.boost);
            if self.boost == 0 && self.frame % 3 == 0 && !square { self.fuel = (self.fuel + 1).min(k::FUEL); }
            // `0x36c890`: the boost's loop stops.
            if 0 <= self.boost_sound {
                host.stop_sound(self.boost_sound);
                self.boost_sound = -1;
            }
        } else {
            self.boost = (self.boost + 1).min(8);
            countdown(&mut self.fuel);
            // `0x36cc38(0x11, 4, ship, 0xfbf)`: its loop while it burns.
            if let Some(s) = self.ship {
                if self.boost_sound < 0 || !host.sound_playing(self.boost_sound) { self.boost_sound = host.play_sound_of(&w.mobys[s], SHIP_CLASS, 0x11, 4); }
            }
        }
        let Some(ship) = self.ship else { return };
        if dec_timer(&mut self.gun_wait) == 0 || pad.held & button::CROSS == 0 { return; }
        self.gun_wait = laser::WAIT;
        let target = add(self.pos, set_len(sub(self.aim, self.pos), laser::AIM));
        let (pos, rows) = (w.mobys[ship].pos, w.mobys[ship].rows);
        // `0x36cb90(3, 0, ship)`: the volley's sound.
        host.play_sound(&w.mobys[ship], 3, 0);
        for gun in laser::GUNS {
            let at = add(pos, through(&rows, gun));
            spawn_laser(w, &*host, at, sub(target, at), rows, ship);
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
            // `+0x98` = 0: no collision.
            m.has_collision = false;
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
        // `0x435190`'s straighten state (D `+0x6e`): 1 once steered, 2 asked by L2 / R2 with the nose within 0.7 of
        // level; `0x4356c8` levels the ship.
        if self.level_state == 0 && (y != 0.0 || p != 0.0) { self.level_state = 1; }
        if self.level_state < 2 && pad.pressed & (button::L2 | button::R2) != 0 && dot([0.0, 0.0, 1.0], self.rows[0]).abs() < 0.7 {
            self.level_state = 2;
        }
        self.straighten();
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

    /// `0x4356c8`: the straightening (D `+0x6c` its ticks, `+0x70` 1 / them): asked (state 2) and idle, 120 ticks
    /// (`$gp+0x26d4` / `+0x26d8`); each tick the rows go from the level ones (the facing kept, the left across the
    /// world's up) toward the current by the cosine ease of the ticks left (`0x32e480`, `0x3025d0`'s slerp; here a
    /// normalised blend), while the nose stays within 0.75 of level; at the end (or too steep) state 0.
    fn straighten(&mut self) {
        if self.level_state == 2 && self.level_ticks == 0 && !self.levelling {
            self.level_ticks = 120;
            self.level_inv = 1.0 / 120.0;
            self.levelling = true;
        }
        if !self.levelling { return; }
        if dec_timer(&mut self.level_ticks) != 0 {
            self.level_state = 0;
            self.levelling = false;
            return;
        }
        let f = self.rows[0];
        if 0.75 <= dot([0.0, 0.0, 1.0], f).abs() {
            self.level_state = 0;
            self.levelling = false;
            self.level_ticks = 0;
            return;
        }
        let l = unit(cross([0.0, 0.0, 1.0], f));
        let target = [f, l, cross(f, l)];
        let t = self.level_ticks as f32 * self.level_inv;
        let e = (1.0 - (t * std::f32::consts::PI).cos()) * 0.5;
        let left = unit(add(scale(target[1], 1.0 - e), scale(self.rows[1], e)));
        let u = unit(cross(f, left));
        self.rows = [f, unit(cross(u, f)), u];
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
        let (top, rate) = if self.boost == 0 { (k::CRUISE, k::SPEED_K) } else { (k::BOOST, k::BOOST_K) };
        self.speed = spring(self.speed, top / 60.0, rate, k::SPEED_D, 0.0, &mut self.speed_v);
        let fwd = self.ship.and_then(|i| w.mobys.get(i)).map_or(self.rows[0], |m| m.rows[0]);
        self.pos = add(self.pos, scale(fwd, self.speed));
        if self.roll_dir != 0 {
            let t = self.roll_t as f32 * self.roll_inv * std::f32::consts::PI;
            let step = k::ROLL_STEP * t.sin() * if self.roll_dir == 1 { -1.0 } else { 1.0 };
            self.pos = add(self.pos, scale(self.rows[1], step));
        }
        for c in &mut self.pos { *c = c.clamp(5.0, 1000.0); }
        // Inside the fight's sphere (the mission object's `+0x68`[0]) less the ship's lead (`$gp+0x258c`).
        if let Some((c, r)) = self.arena {
            let d = sub(self.pos, c);
            let room = r - k::AHEAD;
            if room < crate::hero::physics::len(d) { self.pos = add(c, set_len(d, room)); }
        }
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

/// `0x412e38` (and the fighters' shots, `+0x44`).
pub fn update_laser(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pv_i(0x44) != 0 { return update_enemy_shot(w, id, ctx); }
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
    if let Some(h) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, from, to, crate::collision::QueryFlags(0), ctx.hero_moby)) {
        // `0x36cc38(7, 0, laser, 0xfbf)`; its end's burst.
        ctx.host.play_sound_of(&w.mobys[id], SHIP_CLASS, 7, 0);
        laser_burst(ctx, h.point, w.mobys[id].pv_i(0x20) as u32);
        return w.delete(id);
    }
    let owner = w.mobys[id].pv_i(4) - 1;
    let seg = sub(to, from);
    let seg_len2 = dot(seg, seg);
    let hit = (0..w.mobys.len()).find(|&i| {
        let m = &w.mobys[i];
        let rock = crate::moby::classes::space::ASTEROIDS.contains(&m.o_class);
        if i == id || i as i32 == owner || m.state >= 0x80 || (m.mode & mode::TARGET == 0 && !rock) || m.draw_dist == 0 { return false; }
        // The rocks by their size (the lasers' moby search `0x2f45d0` takes their spheres).
        let r = if rock { r + m.scale * 2.0 } else { r };
        let t = if 0.0 < seg_len2 { (dot(sub(m.pos, from), seg) / seg_len2).clamp(0.0, 1.0) } else { 0.0 };
        crate::hero::physics::dist(add(from, scale(seg, t)), m.pos) < r
    });
    if let Some(t) = hit {
        let d = unit(seg);
        let tmpl = HitTemplate::new(1.0, Some(id), 0x0011_0000, [d[0], d[1], d[2], 0.0]);
        let p = w.mobys[t].pos;
        w.deliver_hit(t, &tmpl, [p[0], p[1], p[2], 1.0]);
        ctx.host.play_sound_of(&w.mobys[id], SHIP_CLASS, 7, 0);
        laser_burst(ctx, add(from, scale(seg, 0.5)), w.mobys[id].pv_i(0x20) as u32);
        return w.delete(id);
    }
    w.register_draw(id);
}

/// The laser's streak (standing in for `0x4137d0`'s meshes): FX 0xe, additive, over its last step and a half, a thin
/// bright core and a wide faint glow in its colour.
pub fn draw_laser(w: &MobyWorld, id: usize, cam: V3, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    let st = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    // A fighter's shot (`0x378960` twice: 0.05 white, 0.1 in its colour 0x60008000, from its tail to its head).
    let (a, b, layers) = if m.pv_i(0x44) != 0 {
        (m.pv_v(0x50), m.pos, [(0.05, 0x807f_7f7fu32), (0.1, m.pv_i(0x68) as u32)])
    } else {
        let step = m.pv_v(0x10);
        let rgb = m.pv_i(0x20) as u32 & 0xff_ffff;
        (sub(m.pos, scale(step, 1.5)), m.pos, [(0.03, 0x7fa0_a0ff), (0.12, 0x6000_0000 | rgb)])
    };
    for (half, c) in layers {
        let side = set_len(cross(sub(b, a), sub(cam, a)), half);
        out.push(crate::fx::FxQuad { fx: 0xe, additive: true, corners: [add(a, side), sub(a, side), add(b, side), sub(b, side)], st, rgba: [c; 4] });
    }
}

/// The ship moby's words: the enemy shots add their hits to `HITS`, the shield's pickups their shield to
/// `SHIELD_ADD` (both read and cleared by [`Flight::tick`]); `SHIELD_NOW` the shield as the flight left it.
pub const HITS: usize = 0x300;
pub const SHIELD_ADD: usize = 0x304;
pub const SHIELD_NOW: usize = 0x308;
/// The shield's most (the weapon slot 6's `+0x10`).
pub const SHIELD_MAX: i32 = k::SHIELD;

/// `0x375198`'s shot from `from` toward `at` (a fighter's: the pool `0x2218c0`, 0x80 each; here a 0xffa moby with
/// `+0x44` set): a streak 2 long (`+0x10` its step's direction, the tail `+0x50`), `+0x48` the distance flown,
/// `+0x4c` the ship it may hit (index + 1).
/// `shot`: its hit radius (`+0x68`), speed (`+0x70`) and colour (`+0x5c`), kept here in `+0x60` / `+0x64` / `+0x68`.
pub fn spawn_enemy_shot(w: &mut MobyWorld, host: &dyn MobyHost, from: V3, at: V3, ship: usize, shot: (f32, f32, u32)) {
    if w.class(LASER_CLASS).is_none() { return; }
    let dir = set_len(sub(at, from), 2.0);
    let id = w.create(LASER_CLASS);
    let m = &mut w.mobys[id];
    m.state = 0;
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.drawn = 1;
    m.mode |= mode::KEEP_ROWS;
    m.pos = add(from, dir);
    m.set_pv_i(0x44, 1);
    m.set_pv_v(0x10, dir);
    m.set_pv_v(0x50, from);
    m.set_pv_f(0x48, 0.0);
    m.set_pv_i(0x4c, ship as i32 + 1);
    m.set_pv_f(0x60, shot.0);
    m.set_pv_f(0x64, shot.1);
    m.set_pv_i(0x68, shot.2 as i32);
    w.rebuild(id, host);
}

/// `0x375410`'s shot: a step of 0.6..1.1 along its direction; gone after 350; within 1 of the ship a hit (1, through
/// `0x375e78`).
fn update_enemy_shot(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    // The quiet (`0x22182c`) takes the shots away.
    if w.mobys.iter().any(|m| m.o_class == MISSION_CLASS && m.state < 0x80 && m.pv_i(crate::moby::classes::space::PASSIVE) != 0) { return w.delete(id); }
    let r = ctx.game.rng.randf(1.1, 0.6) * w.mobys[id].pv_f(0x64);
    let m = &mut w.mobys[id];
    let step = set_len(m.pv_v(0x10), r);
    m.pos = add(m.pos, step);
    let tail = add(m.pv_v(0x50), step);
    m.set_pv_v(0x50, tail);
    let flown = m.pv_f(0x48) + r;
    m.set_pv_f(0x48, flown);
    if 350.0 <= flown { return w.delete(id); }
    let head = m.pos;
    if let Some(s) = usize::try_from(m.pv_i(0x4c) - 1).ok().filter(|&s| w.mobys.get(s).is_some_and(|x| x.state < 0x80 && x.draw_dist != 0)) {
        if crate::hero::physics::dist(head, w.mobys[s].pos) < w.mobys[id].pv_f(0x60) {
            let n = w.mobys[s].pv_i(HITS) + 1;
            w.mobys[s].set_pv_i(HITS, n);
            // `0x375410`: the shake (0.05, 30 ticks).
            shake(w, s, 0.05, 30);
            return w.delete(id);
        }
    }
    w.register_draw(id);
}

/// The flame's layers (`$gp+0x1d88..+0x1dec`): (colour, first size, last size, jitter, step back, sprites), cruising and
/// boosting.
const FLAME: [[(u32, f32, f32, f32, f32, i32); 2]; 2] = [
    [(0x4020_4080, 0.075, 0.02, 0.015, 0.0075, 25), (0xffff_ffff, 0.035, 0.005, 0.015, 0.005, 15)],
    [(0x4020_5080, 0.15, 0.04, 0.03, 0.015, 30), (0xffff_ffff, 0.04, 0.01, 0.015, 0.01, 20)],
];

/// `0x302af8(t, a, b)`: the colours' bytes from `a` to `b` by `t`.
fn lerp_rgba(t: f32, a: u32, b: u32) -> u32 {
    (0..4).fold(0, |acc, k| {
        let (x, y) = ((a >> (8 * k)) & 0xff, (b >> (8 * k)) & 0xff);
        acc | (((x as f32 + (y as f32 - x as f32) * t) as u32 & 0xff) << (8 * k))
    })
}

/// `0x411c78` (the ship's draw callback, registered by its update `0x411a80`; drawn here after the flight's tick so
/// it sits on the ship as this tick placed it, the boost moving it 0.8 a tick): its exhaust from (−0.135, 0, 0.07) in its frame, two layers of sprites
/// (`0x412308` → `0x300aa0`: FX 0x2e, additive, a random roll each, jittered) stepping back along its forward and
/// shrinking, their sizes, steps and colours from the cruise's to the boost's by `+0x34` / 8. Not ported: the hull
/// 0x10d9's three nozzles, the sparks `0x412910`, the boost's trail `0x412430`.
pub fn draw_flame(w: &MobyWorld, id: usize, cam: V3, rng: &mut crate::rng::Rng, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    if m.draw_dist == 0 { return; }
    let t = m.pv_i(0x34) as f32 / 8.0;
    let r = m.rows;
    let nozzle = add(m.pos, through(&r, [-0.135, 0.0, 0.07]));
    let st = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    for layer in 0..2 {
        let (c0, a0, b0, j0, s0, n0) = FLAME[0][layer];
        let (c1, a1, b1, j1, s1, n1) = FLAME[1][layer];
        let mix = |x: f32, y: f32| x + (y - x) * t;
        let (first, last, jitter, step) = (mix(a0, a1), mix(b0, b1), mix(j0, j1), mix(s0, s1));
        let n = (n0 as f32 + (n1 - n0) as f32 * t) as i32;
        let rgba = lerp_rgba(t, c0, c1);
        let back = scale(r[0], -step);
        let mut p = nozzle;
        for i in 0..n.max(1) {
            let size = first + (last - first) * (i as f32 / n.max(1) as f32);
            let roll = rng.rand_angle();
            let q = [p[0] + rng.randf(-jitter, jitter), p[1] + rng.randf(-jitter, jitter), p[2] + rng.randf(-jitter, jitter)];
            let f = unit(sub(cam, q));
            let up = if f[2].abs() < 0.99 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
            let a = unit(cross(up, f));
            let b = cross(f, a);
            let (s, c) = roll.sin_cos();
            let (a, b) = (add(scale(a, c), scale(b, s)), sub(scale(b, c), scale(a, s)));
            // `0x300aa0`'s size as the sprite's half width (matched to the PS2's boost flame).
            let h = size;
            let corner = |x: f32, y: f32| add(q, add(scale(a, x * h), scale(b, y * h)));
            out.push(crate::fx::FxQuad { fx: 0x2e, additive: true, corners: [corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0)], st, rgba: [rgba; 4] });
            p = add(p, back);
        }
    }
}

/// The radar (`0x3757f0`, `$gp−0x2a90..`): its centre (440, 340), the dot's size 8, its reach 48 pixels for 128
/// units, the FX texture 0x3c; the crosshair's (`0x4383d8`) FX texture 0x3f, 0.2 across, green (`$gp+0x27a4`).
mod hud {
    pub const RADAR: [f32; 2] = [440.0, 340.0];
    pub const DOT: f32 = 8.0;
    pub const REACH: f32 = 48.0;
    pub const RANGE: f32 = 128.0;
    pub const DOT_FX: u16 = 0x3c;
    /// The dots' colours (the ship's 0x80808080, a fighter's `+0x150` 0x80) on FX 0x3c, whose texels the HUD atlas
    /// holds at half the GS's (its colours up to 2.0): doubled here.
    pub const DOT_SELF: u32 = 0x80ff_ffff;
    pub const DOT_ENEMY: u32 = 0x0000_00ff;
    pub const CROSS_FX: u16 = 0x3f;
    pub const CROSS: f32 = 0.2;
    pub const CROSS_RGBA: u32 = 0x8020_8020;
    /// The PS2's crosshair against the template's square (its 3D draw's scale on screen).
    pub const CROSS_FIT: [f32; 2] = [1.11, 1.41];
    /// `0x2b32e0`: the corners (across, up); `0x2b3370`: the texels; `$gp+0x2750`: each strip's (corner, texel).
    pub const CROSS_CORNERS: [[f32; 2]; 9] = [[-0.5, 0.5], [-0.5, 0.0], [0.0, 0.0], [0.0, 0.5], [-0.5, -0.5], [0.0, -0.5], [0.5, 0.0], [0.5, 0.5], [0.5, -0.5]];
    pub const CROSS_TEXELS: [[f32; 2]; 16] = [
        [0.1, 0.1], [0.99, 0.1], [0.99, 0.99], [0.1, 0.99], [0.1, 0.99], [0.1, 0.1], [0.99, 0.1], [0.99, 0.99],
        [0.99, 0.1], [0.99, 0.99], [0.1, 0.99], [0.1, 0.1], [0.99, 0.99], [0.1, 0.99], [0.1, 0.1], [0.99, 0.1],
    ];
    pub const CROSS_STRIPS: [[(usize, usize); 4]; 4] = [
        [(0, 0), (1, 1), (3, 3), (2, 2)],
        [(1, 4), (4, 5), (2, 7), (5, 6)],
        [(3, 8), (2, 9), (7, 11), (6, 10)],
        [(2, 12), (5, 13), (6, 15), (8, 14)],
    ];
}

/// The FX textures the flight's HUD draws with.
pub const HUD_FX: [u16; 2] = [hud::DOT_FX, hud::CROSS_FX];

impl Flight {
    /// The flight's HUD: the crosshair (`0x4383d8`: at the aim point, facing the camera; drawn here as one sprite) and
    /// the radar (`0x3757f0`: the ship's dot in the middle; each active fighter within 128 a dot across (left, right)
    /// and ahead (up) of the middle by its distance · 48 / 128, its size by its height (4 + up / 8, 0..16), fading
    /// out with the distance (√(1 − d / 128)), in its colour `+0x150` (0x80)). `tan`: the view's half-angle tangent.
    pub fn hud_quads(&self, w: &MobyWorld, tan: f32, tex: &dyn Fn(u16) -> Option<(crate::gui::Tex, [f32; 2])>) -> Vec<crate::gui::Quad> {
        use crate::gui::{Quad, SCREEN_H, SCREEN_W};
        let mut out = Vec::new();
        // Not over the death or the cut's view.
        if matches!(self.mode, 3..=6) { return out; }
        let r = self.rows;
        let (cx, cy) = (SCREEN_W * 0.5, SCREEN_H * 0.5);
        // The crosshair at the aim point: four strips (`$gp+0x2750`: the corner and texel pairs into `0x2b32e0` /
        // `0x2b3370`) of the quarter reticle, 0.2 across.
        let d = sub(self.aim, self.pos);
        let z = dot(r[0], d);
        if let (true, Some((t, [tw, th]))) = (0.0 < z, tex(hud::CROSS_FX)) {
            let (sx, sy) = (cx / tan / z, cy / (tan * 0.775) / z);
            let x = cx - dot(r[1], d) * sx;
            let y = cy - dot(r[2], d) * sy;
            // Sized as the PS2's 3D draw shows it (measured against a capture: ×1.11 across, ×1.41 up).
            let (sx, sy) = (sx * hud::CROSS_FIT[0], sy * hud::CROSS_FIT[1]);
            for strip in hud::CROSS_STRIPS {
                let pos = strip.map(|(c, _)| { let [u, a] = hud::CROSS_CORNERS[c]; [x - a * hud::CROSS * sx, y - u * hud::CROSS * sy] });
                let uv = strip.map(|(_, k)| { let [u, v] = hud::CROSS_TEXELS[k]; [u * tw, v * th] });
                out.push(Quad { pos, uv, tex: t, rgba: hud::CROSS_RGBA.to_le_bytes() });
            }
        }
        // The lock (`0x437258` → `0x437530`): on the candidate, four quarters of FX 0x3f (`0x437708`: each a square of
        // `$gp+0x262c` (20) pixels × its scale, its texel (63, 63) at the middle, turned a quarter more each). Until the
        // lock, green (0x60) and still; locked, red, turning 1° a tick (`+0x27c`) and growing by 5 × its ticks past
        // the lock / 75, fading past half.
        if let Some(t) = self.locked.or(self.candidate).filter(|&t| w.mobys.get(t).is_some()) {
            let d = sub(w.mobys[t].pos, self.pos);
            let z = dot(r[0], d);
            if let (true, Some((tx, [tw, th]))) = (0.0 < z, tex(hud::CROSS_FX)) {
                let x = cx - dot(r[1], d) / z * (cx / tan);
                let y = cy - dot(r[2], d) / z * (cy / (tan * 0.775));
                let (scale, spin, rgba) = if self.locked.is_some() {
                    let share = (self.lock_ticks - missile::LOCK_AT) as f32 / missile::LOCK_TICKS as f32;
                    let a = (2.0 * (1.0 - share)).min(1.0) * 96.0;
                    (1.0 + share * 5.0, self.lock_spin, [0xff, 0, 0, a as u8])
                } else {
                    (1.0, 0.0, [0, 0xff, 0, 0x60])
                };
                let h = missile::LOCK_PIXELS * scale;
                let (u0, u1) = (tw / 64.0, tw * 63.0 / 64.0);
                let (v0, v1) = (th / 64.0, th * 63.0 / 64.0);
                for k in 0..4 {
                    let (s, c) = (spin + k as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
                    let (a, b) = ([h * s, h * c], [h * c, -h * s]);
                    let pos = [[x + a[0] + b[0], y + a[1] + b[1]], [x + a[0], y + a[1]], [x + b[0], y + b[1]], [x, y]];
                    out.push(Quad { pos, uv: [[u0, v0], [u1, 0.0], [0.0, v1], [u1, v1]], tex: tx, rgba });
                }
            }
        }
        out
    }

    /// The radar's dots (`0x3757f0`, the last of the HUD: over its frame) — see [`Flight::hud_quads`]'s doc.
    pub fn radar_quads(&self, w: &MobyWorld, tex: &dyn Fn(u16) -> Option<(crate::gui::Tex, [f32; 2])>) -> Vec<crate::gui::Quad> {
        use crate::gui::Quad;
        let mut out = Vec::new();
        if matches!(self.mode, 3..=6) { return out; }
        let sprite = |out: &mut Vec<Quad>, fx: u16, x: f32, y: f32, h: f32, rgba: u32| {
            let Some((t, [tw, th])) = tex(fx) else { return };
            out.push(Quad { pos: [[x - h, y + h], [x + h, y + h], [x - h, y - h], [x + h, y - h]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], tex: t, rgba: rgba.to_le_bytes() });
        };
        let r = self.rows;
        let [rx, ry] = hud::RADAR;
        sprite(&mut out, hud::DOT_FX, rx, ry, hud::DOT * 0.5, hud::DOT_SELF);
        let ai = |c: i16| c == crate::moby::classes::space::FIGHTER || c == crate::moby::classes::space::GUNSHIP;
        for m in w.mobys.iter().filter(|m| ai(m.o_class) && m.state < 0x80 && m.draw_dist != 0) {
            let rel = sub(m.pos, self.pos);
            let dist = crate::hero::physics::len(rel);
            if hud::RANGE < dist || dist == 0.0 { continue; }
            let v = scale(rel, hud::REACH / hud::RANGE);
            let (ahead, left, up) = (dot(r[0], v), dot(r[1], v), dot(r[2], v));
            let size = (up * 0.125 + hud::DOT * 0.5).clamp(0.0, hud::DOT * 2.0);
            let alpha = ((1.0 - dist / hud::RANGE).max(0.0).sqrt() * 128.0) as u32;
            sprite(&mut out, hud::DOT_FX, rx - left, ry - ahead, size * 0.5, hud::DOT_ENEMY | alpha << 24);
        }
        out
    }
}

/// The mission object's class (Wupash's waves, `0x40e340`).
pub const MISSION_CLASS: i16 = 0x0eee;

/// The mission's tuning (its pvars from the disc: `+0x34` the quiet start 720 ticks, `+0x2c` the wait before the
/// second wave 150, then the wait before the end 540; `$gp+0x1c70` / `+0x1c80` the cut's angles and place, 240 ticks;
/// the second wave's gathering point and goals (`0x378250` / `0x378878`, 10 round them), the ship's new place and
/// the point it faces (`0x4381d0`)).
mod mission {
    pub const QUIET: i32 = 0x2d0;
    pub const WAVE_WAIT: i32 = 0x96;
    pub const END_WAIT: i32 = 0x21c;
    pub const CUT_POS: [f32; 3] = [390.0, 430.0, 524.36];
    pub const CUT_ROT: [f32; 3] = [0.0, -0.01, 2.0];
    pub const CUT_TICKS: i32 = 0xf0;
    pub const GATHER: [f32; 3] = [359.0, 440.0, 524.0];
    pub const GOAL: [f32; 3] = [403.0, 456.0, 524.0];
    pub const SHIP_AT: [f32; 3] = [420.93, 413.79, 534.54];
    pub const SHIP_FACES: [f32; 3] = [417.18, 454.03, 532.6];
    pub const WAVES: i32 = 1;
}

impl Flight {
    /// The mission object 0x0eee (`0x40e340`, run here with the flight it drives): the quiet start (`0x22182c`: the
    /// fighters hold their fire and their shots go, [`crate::moby::classes::space`]); when no fighter is active, the
    /// next wave after 150 ticks (the cut `0x434f60`: the camera fades to the cut's view for 240 ticks and back; the
    /// fighters come back (`0x376548`) gathered at a point and going for another, the ship moved to the wave's start),
    /// or after the last one, 540 ticks and the level's end (`0x3763b8`, `0x307bf0(2, 1)`: Maktar Nebula, saved).
    fn mission(&mut self, w: &mut MobyWorld) {
        let Some(mid) = w.mobys.iter().position(|m| m.o_class == MISSION_CLASS && m.state < 0x80) else { return };
        if self.mission_state == 0 {
            self.mission_state = 1;
            self.quiet = mission::QUIET;
            self.wave_wait = mission::WAVE_WAIT;
        }
        countdown(&mut self.quiet);
        // `0x307d80(0xa16, 0x6d)`: the fight's opening help (asked until shown).
        if !self.opening_shown { self.help = Some((0xa16, 0x6d)); }
        let mut passive = self.quiet != 0;
        let active = crate::moby::classes::space::active_count(w);
        if self.mission_state == 3 {
            passive = true;
            if self.cut_ticks == 0 && self.mode == 0 {
                self.waves += 1;
                self.mission_state = 1;
                passive = self.quiet != 0;
            }
        } else if active == 0 && 5 < self.frame {
            passive = true;
            if self.waves < mission::WAVES {
                if countdown(&mut self.wave_wait) {
                    self.wave_wait = mission::END_WAIT;
                    // `0x434f60`: the cut.
                    self.mode = 1;
                    self.cut_ticks = mission::CUT_TICKS;
                    self.mission_state = 3;
                    crate::moby::classes::space::next_wave(w, mission::GATHER, mission::GOAL);
                    // `0x4381d0`: the ship at the wave's start.
                    self.pos = mission::SHIP_AT;
                    let f = unit(sub(mission::SHIP_FACES, self.pos));
                    let l = unit(cross(self.rows[2], f));
                    self.rows = [f, l, unit(cross(f, l))];
                }
            } else if countdown(&mut self.wave_wait) {
                self.done = true;
            } else if !self.closing_asked {
                // `0x307d80(0xa36, 0x8d)`: the win's.
                self.help = Some((0xa36, 0x8d));
                self.closing_asked = true;
            }
        }
        w.mobys[mid].set_pv_i(crate::moby::classes::space::PASSIVE, passive as i32);
        w.mobys[mid].set_pv_i(crate::moby::classes::space::ACTIVE_COUNT, active as i32);
        let lead = crate::moby::classes::space::leader(w, self.frame);
        w.mobys[mid].set_pv_i(crate::moby::classes::space::LEADER, lead);
    }

    /// `0x434670`'s modes 1..5: the fades of the cut (`$gp+0x2a70`, 0.05 a tick). 1 flying while the screen goes
    /// black, then the cut's view (4: back in; 5: its ticks; 3: black again), then the flight where it was (2: back
    /// in while flying). Returns whether the flight flies this tick.
    fn cut(&mut self) -> bool {
        match self.mode {
            // Black: the cut's view after this tick's flight ([`Flight::cut_after`]).
            1 => {
                self.fade = (self.fade + 0.05).min(1.0);
                true
            }
            4 => {
                self.fade -= 0.05;
                if self.fade <= 0.0 {
                    self.fade = 0.0;
                    self.mode = 5;
                }
                false
            }
            5 => {
                if countdown(&mut self.cut_ticks) { self.mode = 3; }
                false
            }
            3 => {
                self.fade = (self.fade + 0.05).min(1.0);
                if 1.0 <= self.fade {
                    self.mode = 2;
                    (self.pos, self.rows) = self.saved;
                }
                false
            }
            2 => {
                self.fade -= 0.05;
                if self.fade <= 0.0 {
                    self.fade = 0.0;
                    self.mode = 0;
                }
                true
            }
            _ => true,
        }
    }

    /// The end of mode 1's tick (`0x434670` flies first): black, the flight kept and the cut's view.
    fn cut_after(&mut self) {
        if self.mode == 1 && 1.0 <= self.fade {
            self.mode = 4;
            self.saved = (self.pos, self.rows);
            self.pos = mission::CUT_POS;
            self.rows = crate::moby::attach::rows_of(mission::CUT_ROT);
        }
    }

    /// The HUD's counts (`0x40e7a8`: `"%s: %d"` with the strings 0x3067 and 0x3068 at (24, 357) and (24, 384)): the
    /// fighters active and the wave.
    pub fn counts(&self, w: &MobyWorld) -> [(u32, i32, [f32; 2]); 2] {
        [(0x3067, crate::moby::classes::space::active_count(w) as i32, [24.0, 357.0]), (0x3068, self.waves + 1, [24.0, 384.0])]
    }
}

/// The missile's class (`0x3caae8`: 0x974; 0x1161 with the upgrade, not kept).
pub const MISSILE_CLASS: i16 = 0x0974;

/// The missiles' tuning (`$gp+0xbe4` 90 a second, `+0xbec` 500 far, `+0xbf4` its scale 0.3, `+0xc04` the damage 5,
/// `+0x25b4` 90 ticks between, the weapon slot 1's 10; the lock past `+0x2610` 60 ticks, held to `+0x2618` 75,
/// `+0x2624` 25°, `+0x2628` 60 far, `+0x262c` 20 pixels round the crosshair plus the class's own from `0x2b3280`).
mod missile {
    pub const SPEED: f32 = 90.0;
    pub const RANGE: f32 = 500.0;
    pub const SCALE: f32 = 0.3;
    pub const DAMAGE: f32 = 5.0;
    pub const WAIT: i16 = 90;
    pub const AMMO: i32 = 10;
    pub const LOCK_AT: i32 = 60;
    pub const LOCK_TICKS: i32 = 75;
    pub const LOCK_CONE: f32 = 25.0;
    pub const LOCK_PIXELS: f32 = 20.0;
    /// `0x2b3280`: the classes locked on, their pixels added to `+0x262c`'s and their range (others: 40, 60).
    pub const LOCK_CLASSES: [(i16, f32, f32); 2] = [(0x0957, 40.0, 60.0), (0x097c, 60.0, 60.0)];
}

impl Flight {
    /// `0x436d20`: the missiles' lock (the ship's `+0x14` the candidate, `+0x1c` its ticks, `+0x18` the lock). With
    /// missiles, the candidate (`0x436f78`: a target-listed fighter on screen within 40 pixels of the crosshair,
    /// within 60, the line clear) held for 75 ticks becomes the lock; the lock holds while it lives within 25° of the
    /// ship's facing (the line clear, tested every fifth tick).
    fn lock(&mut self, w: &MobyWorld, tan: f32) {
        if self.missiles < 1 {
            (self.candidate, self.lock_ticks, self.locked) = (None, 0, None);
            return;
        }
        let Some(ship) = self.ship else { return };
        let alive = |t: usize| w.mobys.get(t).is_some_and(|m| m.state < 0x80 && m.draw_dist != 0 && m.mode & 1 == 0);
        if let Some(t) = self.locked {
            let d = sub(w.mobys[t].pos, w.mobys[ship].pos);
            let off = crate::hero::physics::len(d);
            let angle = if off == 0.0 { std::f32::consts::PI } else { (dot(w.mobys[ship].rows[0], d) / off).clamp(-1.0, 1.0).acos() };
            if !alive(t) || missile::LOCK_CONE * DEG < angle {
                (self.locked, self.candidate, self.lock_ticks) = (None, None, 0);
            } else {
                self.lock_ticks = (self.lock_ticks + 1).min(missile::LOCK_TICKS);
            }
            return;
        }
        let r = self.rows;
        let (cx, cy) = (crate::gui::SCREEN_W * 0.5, crate::gui::SCREEN_H * 0.5);
        let screen = |p: V3| {
            let d = sub(p, self.pos);
            let z = dot(r[0], d);
            (0.0 < z).then(|| [cx - dot(r[1], d) / z * (cx / tan), cy - dot(r[2], d) / z * (cy / (tan * 0.775))])
        };
        let Some(aim) = screen(self.aim) else { return };
        let found = (0..w.mobys.len()).find(|&i| {
            let m = &w.mobys[i];
            let Some(&(_, px, range)) = missile::LOCK_CLASSES.iter().find(|c| c.0 == m.o_class) else { return false };
            if !alive(i) || m.mode & mode::TARGET == 0 { return false; }
            if range < crate::hero::physics::dist(self.pos, m.pos) { return false; }
            let win = missile::LOCK_PIXELS + px;
            screen(m.pos).is_some_and(|p| (p[0] - aim[0]).abs() < win && (p[1] - aim[1]).abs() < win)
        });
        if found == self.candidate && found.is_some() {
            self.lock_ticks = (self.lock_ticks + 1).min(missile::LOCK_TICKS);
            // `0x437258`: past 60 ticks the candidate becomes the lock.
            if missile::LOCK_AT < self.lock_ticks { self.locked = found; }
        } else {
            self.candidate = found;
            self.lock_ticks = 0;
        }
    }

    /// `0x436440`'s missiles (○, slot 1): with ammo every 90 ticks one from just under the ship (0, 0, −0.1 in its
    /// frame) along its facing, at the lock (`0x3caae8`).
    fn fire_missile(&mut self, pad: &PadState, w: &mut MobyWorld, host: &mut dyn MobyHost) {
        let fired = dec_timer(&mut self.missile_wait) != 0 && pad.held & button::CIRCLE != 0 && 0 < self.missiles;
        if !fired { return; }
        let Some(ship) = self.ship else { return };
        if w.class(MISSILE_CLASS).is_none() { return; }
        self.missiles -= 1;
        self.missile_wait = missile::WAIT;
        // `0x36cb90(2, 0, ship)`.
        host.play_sound(&w.mobys[ship], 2, 0);
        let (pos, rows) = (w.mobys[ship].pos, w.mobys[ship].rows);
        let at = add(pos, through(&rows, [0.0, 0.0, -0.1]));
        let speed = missile::SPEED / 60.0;
        let ticks = (missile::RANGE / speed) as i32;
        let base = w.class(MISSILE_CLASS).map_or(1.0, |c| c.scale);
        let id = w.create(MISSILE_CLASS);
        let m = &mut w.mobys[id];
        m.state = 1;
        m.update_dist = 0xff;
        m.draw_dist = 0x1ff;
        m.drawn = 1;
        m.mode |= mode::KEEP_ROWS;
        m.rows = rows;
        m.pos = at;
        m.scale = base * missile::SCALE;
        m.set_pv_v(0, set_len(rows[0], speed));
        m.set_pv_i(0x10, self.locked.map_or(0, |t| t as i32 + 1));
        m.set_pv_i(0x18, ticks);
        m.set_pv_f(0x1c, 1.0 / ticks as f32);
        m.set_pv_i(0x20, ship as i32 + 1);
        m.set_pv_f(0x30, base);
        w.rebuild(id, &*host);
    }

    /// `0x4112e8`'s tick: the engine's hum (`0x36cb90(0, 4, ship)`, kept going: the ship's `+0xc`).
    fn engine_hum(&mut self, w: &MobyWorld, host: &mut dyn MobyHost) {
        let Some(s) = self.ship else { return };
        if self.mode == 6 {
            if 0 <= self.hum { host.stop_sound(self.hum); self.hum = -1; }
            return;
        }
        if self.hum < 0 || !host.sound_playing(self.hum) { self.hum = host.play_sound(&w.mobys[s], 0, 4); }
    }

    /// `0x437258`'s sounds: while a lock builds its loop (`0x36cc38(9, 4, ship, 0xfbf)`, `+0x270`); once locked the
    /// lock's (8, once, `+0x26c`).
    fn lock_sounds(&mut self, w: &MobyWorld, host: &mut dyn MobyHost) {
        let Some(s) = self.ship else { return };
        // `0x437258`: the candidate's beep (8) once; locked, the lock's loop (9) kept playing and its marker turning;
        // none, both reset.
        if self.locked.is_some() {
            self.lock_spin += DEG;
            if self.lock_sound < 0 || !host.sound_playing(self.lock_sound) { self.lock_sound = host.play_sound_of(&w.mobys[s], SHIP_CLASS, 9, 4); }
        } else if self.candidate.is_some() {
            if !self.lock_heard { host.play_sound_of(&w.mobys[s], SHIP_CLASS, 8, 0); }
            self.lock_heard = true;
        } else {
            if 0 <= self.lock_sound { host.stop_sound(self.lock_sound); }
            self.lock_sound = -1;
            self.lock_heard = false;
        }
    }
}

/// `0x3caca8`: the missile. While its target lives, after its first 4 ticks it flies at it (before, it turns to it,
/// `0x3cb1e0`: here at once too); it grows to its scale (0.05 a tick); its time, the 2..1021 box, the world's line
/// (`0x2f1998`) and the mobys' (`0x2f29f8`, 0.2) end it — a moby hit takes 5 (flags 0x810000, `0x3cb4f0`), and it
/// bursts (`0x3cbfe8`, here the blast at 0.3). Not ported: its trail (`0x3420e8`), the 0x956's sparks.
pub fn update_missile(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let from = w.mobys[id].pos;
    let mut v = w.mobys[id].pv_v(0);
    let speed = crate::hero::physics::len(v);
    if let Some(t) = usize::try_from(w.mobys[id].pv_i(0x10) - 1).ok().filter(|&t| w.mobys.get(t).is_some_and(|m| m.state < 0x80 && m.draw_dist != 0)) {
        v = set_len(sub(w.mobys[t].pos, from), speed);
        w.mobys[id].set_pv_v(0, v);
        let f = unit(v);
        let l = unit(cross([0.0, 0.0, 1.0], f));
        w.mobys[id].rows = [f, l, cross(f, l)];
    }
    let to = add(from, v);
    let base = w.mobys[id].pv_f(0x30);
    {
        let m = &mut w.mobys[id];
        m.pos = to;
        m.scale = (m.scale + 0.05).min(base * missile::SCALE.max(1.0));
    }
    let mut left = w.mobys[id].pv_i(0x18);
    let over = countdown(&mut left);
    w.mobys[id].set_pv_i(0x18, left);
    if over || to.iter().any(|&c| !(2.0..=1021.0).contains(&c)) { return w.delete(id); }
    // Its trail (`0x3420e8` four times along the step back: yellow 30000 and 15000 puffs drifting 0.0125, 10 ticks).
    if let Some(sys) = ctx.parts.as_deref_mut() {
        let back = scale(v, -0.333);
        let mut at = to;
        for _ in 0..4 {
            at = add(at, back);
            let g = &mut ctx.game.rng;
            let drift = crate::moby::classes::shot::polar(0.0125, g.rand_angle(), g.rand_angle());
            for (size, rgba) in [(30_000.0, 0x4f00_ffff_u32), (15_000.0, 0x4f20_ffff)] {
                if let Some(i) = crate::particles::puff(sys, g, 0.0, 1.0, 1.0, size, at, 0, drift, rgba, 0) { (sys.pool.recs[i][10], sys.pool.recs[i][11]) = (10, 0); }
            }
        }
    }
    let owner = w.mobys[id].pv_i(0x20) - 1;
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, from, to, crate::collision::QueryFlags(0), usize::try_from(owner).ok()));
    if let Some(h) = hit {
        if let Some(t) = h.moby.filter(|&t| t != id) {
            let d = unit(v);
            let tmpl = HitTemplate::new(missile::DAMAGE, Some(id), 0x0081_0000, [d[0], d[1], d[2], 0.0]);
            let p = w.mobys[t].pos;
            w.deliver_hit_any(t, &tmpl, [p[0], p[1], p[2], 1.0]);
        }
        crate::moby::blast::burst(ctx, h.point, &crate::moby::blast::Blast { size: 0.3, bursts: [0x14, 0x0a, 0x0a], puffs: 0x0a, streaks: 0x10 });
        ctx.host.play_sound_of(&w.mobys[id], SHIP_CLASS, 7, 0);
        return w.delete(id);
    }
    w.rebuild(id, &*ctx.host);
}

impl Flight {
    /// `0x437a70`: the ship's step swept in 0.25 spheres (0.075 up in its frame; at most 30 long) against the world
    /// and the mobys. The world met nose-on (|n · forward| > 0.75) is the death (`0x434ad0`); else the camera and the
    /// ship are pushed out and the hull takes 1 every fourth frame. A moby met head-on (its facing against the ship's)
    /// takes 1 (flags 0x10000) and the hull 30, once in 15 ticks (the record's `+0x58`). Not ported: the Hrugis
    /// special case, the sparks (`0x4382b0`), the camera shake.
    fn collide(&mut self, w: &mut MobyWorld, coll: Option<&rc_formats::collision::Collision>, scene: Option<&crate::collision::mobys::MobyScene>, from: V3) {
        countdown(&mut self.ram_wait);
        let (Some(ship), Some(c)) = (self.ship, coll) else { return };
        let to = w.mobys[ship].pos;
        let up = scale(w.mobys[ship].rows[2], 0.075);
        let d = sub(to, from);
        let l = crate::hero::physics::len(d);
        if 30.0 < l { return; }
        let n = if l <= 0.25 { 2 } else { (l / 0.25) as i32 + 2 };
        for i in 1..=n {
            let p = add(add(from, scale(d, i as f32 / n as f32)), up);
            let Some(h) = crate::collision::coll_sphere_m(c, scene, p, 0.25, crate::collision::QueryFlags(0), Some(ship)) else { continue };
            match h.moby {
                Some(m) if w.mobys.get(m).is_some_and(|x| x.o_class != SHIP_CLASS) => {
                    let facing = dot(w.mobys[m].rows[0], w.mobys[ship].rows[0]);
                    if facing <= 0.0 && self.ram_wait == 0 {
                        let dir = unit(sub(w.mobys[m].pos, to));
                        let tmpl = HitTemplate::new(1.0, Some(ship), 0x0001_0000, [dir[0], dir[1], dir[2], 0.0]);
                        let q = w.mobys[m].pos;
                        w.deliver_hit_any(m, &tmpl, [q[0], q[1], q[2], 1.0]);
                        self.damage(w, 30);
                        self.ram_wait = 15;
                    }
                }
                _ => {
                    let nrm = unit(h.normal);
                    if 0.75 < dot(nrm, w.mobys[ship].rows[0]).abs() {
                        self.die(w);
                        return;
                    }
                    if let Some(c) = h.pushed_centre {
                        let out = sub(c, p);
                        self.pos = add(self.pos, out);
                        w.mobys[ship].pos = add(w.mobys[ship].pos, out);
                    }
                    if self.frame % 4 == 0 { self.damage(w, 1); }
                }
            }
            return;
        }
    }
}

/// The ship HUD's icon set (`0x30d5a8(0xea97, frame)`): frames 0 / 1 the backings, the weapon slots' `+0x1c` their
/// icons (the missiles 7, the boost 15).
pub const HUD_ICONS: u16 = 0xea97;

impl Flight {
}

/// The ship moby's words the camera's shake is asked through (`0x1b91a0` its size, `0x1b91a8` its ticks).
pub const SHAKE: usize = 0x30c;
pub const SHAKE_TICKS: usize = 0x310;

/// `0x1b91a0 / 0x1b91a8 = size, ticks`: a shake of the camera asked (the bigger one kept).
pub fn shake(w: &mut MobyWorld, ship: usize, size: f32, ticks: i32) {
    let m = &mut w.mobys[ship];
    if m.pv_f(SHAKE) <= size {
        m.set_pv_f(SHAKE, size);
        m.set_pv_i(SHAKE_TICKS, ticks);
    }
}

impl Flight {
    /// The view: the camera shaken (the camera's shake `0x2d9528`: its size eased out over its ticks, here a
    /// wobble of the place along the rows).
    pub fn view_pos(&self) -> V3 {
        if self.shake_ticks <= 0 || self.shake_len <= 0 { return self.pos; }
        let t = self.shake_ticks as f32 / self.shake_len as f32;
        let a = self.shake * t;
        let f = self.frame as f32;
        add(self.pos, add(scale(self.rows[1], (f * 2.3).sin() * a), scale(self.rows[2], (f * 3.1).cos() * a)))
    }

    /// The shakes asked this tick (the ship's words), counted down.
    fn shake_tick(&mut self, w: &mut MobyWorld) {
        if 0 < self.shake_ticks { self.shake_ticks -= 1; }
        let Some(s) = self.ship else { return };
        let size = w.mobys[s].pv_f(SHAKE);
        if 0.0 < size {
            let ticks = w.mobys[s].pv_i(SHAKE_TICKS);
            if self.shake_ticks <= 0 || self.shake <= size {
                (self.shake, self.shake_ticks, self.shake_len) = (size, ticks, ticks);
            }
            w.mobys[s].set_pv_f(SHAKE, 0.0);
        }
    }
}

/// The mission object's word the flight asks the ship's blast through (`0x434ad0` → `0x3cbfe8`, the place).
pub const BOOM: usize = 0x110;

/// The mission object 0x0eee's update here: what [`Flight::mission`] cannot do without the particles — the ship's
/// blast at its death (`0x3cbfe8(ship, 0, 1)`: here the ground's blast, crate::moby::blast, at size 1).
pub fn update_mission(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pv_i(BOOM + 0xc) == 0 { return; }
    let at = w.mobys[id].pv_v(BOOM);
    w.mobys[id].set_pv_i(BOOM + 0xc, 0);
    crate::moby::blast::burst(ctx, at, &crate::moby::blast::Blast { size: 1.0, bursts: [0x32, 0x1e, 0x1e], puffs: 0x1e, streaks: 0x28 });
}

/// `0x4134b0(at, 20, colour, laser)`: the laser's end — 20 sparks (crate::particles::spark standing in for `0x346ee8`)
/// thrown toward the camera's side, 0.05..3 a second across and 1..5 up, a white and a coloured one each.
fn laser_burst(ctx: &mut Ctx, at: V3, rgb: u32) {
    let cam = ctx.host.camera_pos();
    let Some(sys) = ctx.parts.as_deref_mut() else { return };
    let toward = set_len(sub(cam, at), 0.5);
    for _ in 0..20 {
        let g = &mut ctx.game.rng;
        let size = g.randf(0.25, 1.0);
        let across = crate::moby::classes::shot::polar(g.randf(0.05, 3.0) / 60.0, g.rand_angle(), g.rand_angle());
        let v = add(add(across, [0.0, 0.0, g.randf(1.0, 5.0) / 60.0]), scale(toward, 0.01));
        let p = [at[0] + g.randf(-0.15, 0.15), at[1] + g.randf(-0.15, 0.15), at[2] + g.randf(-0.15, 0.15)];
        crate::particles::spark(sys, g, size * 0.1 * 210_000.0, 0.0, 0.002, 0.0, 1.0, p, v, 60, 0x7f, rgb & 0xff_ffff, 0);
        crate::particles::spark(sys, g, size * 0.07 * 210_000.0, 0.0, 0.002, 0.0, 1.0, p, v, 60, 0x7f, 0x7f_7f7f, 0);
    }
}

/// A HUD piece in the order `0x40e7a8` draws it: a GUI draw or a plain quad (the bars).
pub enum HudItem {
    Draw(crate::hud::Draw),
    Quad(crate::gui::Quad),
}

/// `0x2fc7f8(x0, y0, x1, y1, 1, colours)`: a bar from `left`'s colour to `right`'s (here in 32 slices).
fn bar(out: &mut Vec<HudItem>, top: i32, bottom: i32, x0: f32, x1: f32, left: u32, right: u32) {
    const N: usize = 32;
    for k in 0..N {
        let (a, b) = (k as f32 / N as f32, (k + 1) as f32 / N as f32);
        let c = crate::hud::lerp_rgba((a + b) * 0.5, left, right);
        let (l, r) = (x0 + (x1 - x0) * a, x0 + (x1 - x0) * b);
        let (l, r) = if l <= r { (l, r) } else { (r, l) };
        out.push(HudItem::Quad(crate::gui::Quad { pos: [[l, top as f32], [r, top as f32], [l, bottom as f32], [r, bottom as f32]], uv: [[0.0; 2]; 4], tex: crate::gui::Tex::None, rgba: c.to_le_bytes() }));
    }
}

impl Flight {
    /// `0x40e7a8` (with `0x40f3e0(0, 1, 1, 1)` first) in its order (the last the frame's highlight 0x82):
    /// * the panels: the bolts' (sprites 0xbf / 0xc0 at (84, 0x1b0cbc), growing in and out over its 300 ticks after a
    ///   bolt is taken, its icon 0xeaa2 frame 3 and its count) and the counts' (0xc1 / 0xc2, ×1.1 across, at (84, 365)
    ///   and (84, 392));
    /// * the counts `"%s: %d"` (the strings 0x3067 / 0x3068 at (24, 357) / (24, 384));
    /// * the frame (sprites 0x80 / 0x81) and the shield's gauge (0x83, red to green by the shield left);
    /// * the hull's bar (156 → 156 + 200 · hull, 14..27, from red to its colour, blinking every 5 ticks under 30%) and
    ///   the boost's (450 → 386 by the fuel, 8..19);
    /// * the weapon's icon (0xea97: frames 1 and 0 behind it, 7 the missiles, at (21, 7) 30 square) and the boost's
    ///   (frame 15 at (462, 9)), the ammo `"%d/%d"` at (67, 5);
    /// * the radar's frame (0xba..0xbc at (444, 344), ×0.8).
    /// Not ported: the upgraded shields' rings 0x84 / 0x85, the 16:9 shift, the weapon switch (one weapon here).
    pub fn hud_items(&self, w: &MobyWorld, bolts: (i32, i32), label: &dyn Fn(u32) -> String) -> Vec<HudItem> {
        use crate::hud::{Align, Draw};
        let mut out = Vec::new();
        if matches!(self.mode, 3..=6) { return out; }
        let d = |x: Draw| HudItem::Draw(x);
        let lerp = |t: f32| crate::hud::lerp_rgba(t.clamp(0.0, 1.0), 0x8000_00ff, 0x8000_ff00);
        // The bolts' panel (`0x40f3e0`'s second part): `bolts` = (the count, the ticks of its showing left).
        let (count, left) = bolts;
        if 0 < left {
            let s = if left < 0x123 && 9 < left { 1.0 } else if 0x123 <= left { ((300 - left) as f32 * 0.1).clamp(0.0, 1.0) } else { (left as f32 * 0.1).clamp(0.0, 1.0) };
            let at = [84.0, 0x16d as f32 - 54.0];
            out.push(d(Draw::Sprite { id: 0xbf, pos: at, scale: [s, s], frame: 0.0, colour: 0x6044_2d00 }));
            out.push(d(Draw::Sprite { id: 0xc0, pos: at, scale: [s, s], frame: 0.0, colour: 0x55f0_c070 }));
            if (10..0x123).contains(&left) {
                out.push(d(Draw::Icon { id: 0xeaa2, frame: 3, pos: [84.0 - 51.0, at[1] - 13.0], size: [28.0, 28.0], colour: 0x8080_8080 }));
                out.push(d(Draw::Text { text: format!("{count}"), pos: [89.0, at[1] + 2.0], scale: 1.0, colour: 0x8080_8080, align: Align::Left }));
            }
        }
        // The counts' panels and the counts.
        for y in [365.0, 392.0] {
            out.push(d(Draw::Sprite { id: 0xc1, pos: [84.0, y], scale: [1.1, 1.0], frame: 0.0, colour: 0x6044_2d00 }));
            out.push(d(Draw::Sprite { id: 0xc2, pos: [84.0, y], scale: [1.1, 1.0], frame: 0.0, colour: 0x55f0_c070 }));
        }
        for (id, n, pos) in self.counts(w) {
            out.push(d(Draw::Text { text: format!("{}: {n}", label(id)), pos, scale: 1.0, colour: 0x8080_8080, align: Align::Left }));
        }
        // The frame and the shield's gauge.
        let mid = [256.0, 200.0];
        out.push(d(Draw::Sprite { id: 0x80, pos: mid, scale: [1.0, 1.0], frame: 0.0, colour: 0x6044_2d00 }));
        out.push(d(Draw::Sprite { id: 0x81, pos: mid, scale: [1.0, 1.0], frame: 0.0, colour: 0x55f0_c070 }));
        let shield = self.shield as f32 / k::SHIELD as f32;
        if 0.0 < shield { out.push(d(Draw::Sprite { id: 0x83, pos: mid, scale: [1.0, 1.0], frame: 0.0, colour: lerp(shield) })); }
        // The hull's and the boost's bars.
        let hull = self.hull as f32 * 0.01;
        let shown = if hull <= 0.3 { (self.frame / 5) % 2 == 0 } else { true };
        if shown && 0.0 < hull { bar(&mut out, 14, 27, 156.0, hull * 200.0 + 156.0, 0x8000_00ff, lerp(hull)); }
        let fuel = self.fuel as f32 / k::FUEL as f32;
        if 0.0 < fuel { bar(&mut out, 8, 19, 450.0, 450.0 + (386.0 - 450.0) * fuel, 0x8000_00ff, lerp(fuel)); }
        // The icons and the ammo.
        for (frame, pos, colour) in [(1, [21.0, 7.0], 0x6044_2d00), (0, [21.0, 7.0], 0x55f0_c070), (7, [21.0, 7.0], 0x8080_8080), (1, [462.0, 9.0], 0x6044_2d00), (15, [462.0, 9.0], 0x8080_8080)] {
            out.push(d(Draw::Icon { id: HUD_ICONS, frame, pos, size: [30.0, 30.0], colour }));
        }
        out.push(d(Draw::Text { text: format!("{}/{}", self.missiles, missile::AMMO), pos: [67.0, 5.0], scale: 1.0, colour: 0x8080_8080, align: Align::Left }));
        // The frame's highlight over the bars (sprite 0x82, 0x80f0f0f0).
        out.push(d(Draw::Sprite { id: 0x82, pos: mid, scale: [1.0, 1.0], frame: 0.0, colour: 0x80f0_f0f0 }));
        // The radar's frame.
        for (id, colour) in [(0xba, 0x6044_2d00), (0xbb, 0x6024_1700), (0xbc, 0x55f0_c070)] {
            out.push(d(Draw::Sprite { id, pos: [444.0, 344.0], scale: [0.8, 0.8], frame: 0.0, colour }));
        }
        out
    }

    /// `0x412430` (from the flame's callback while the shield holds, `0x375df8`): the bubble — three quads facing the
    /// camera (`0x412628`: ±0.27, ±0.27, ±0.22, 0.35 toward it from just above the ship), their texels turning (0.01,
    /// −0.007, 0.007 a tick), FX 0x46 (the ship's `+0x58`) in 0x0fc85a5a twice and FX 0x36 in 0x28ff3232; hit this
    /// tick (`0x221840`), 7 ticks of a brighter flash.
    pub fn draw_shield(&mut self, w: &MobyWorld, cam: V3, out: &mut Vec<crate::fx::FxQuad>) {
        let Some(s) = self.ship else { return };
        if self.shield <= 0 || self.mode == 6 { return; }
        let m = &w.mobys[s];
        self.shell_spin = [crate::hero::physics::add_rot(self.shell_spin[0], 0.01), crate::hero::physics::add_rot(self.shell_spin[1], -0.007), crate::hero::physics::add_rot(self.shell_spin[2], 0.007)];
        if self.hit_flash_new { self.hit_flash = 7; }
        let (c0, c1) = if 0 < self.hit_flash {
            self.hit_flash -= 1;
            let t = self.hit_flash as u32 * 5;
            let low = (0x6e - t) | 0x1e00_0000;
            ((0x6e + t) << 16 | low, (0x96 + t) << 16 | low)
        } else {
            (0x0fc8_5a5a, 0x28ff_3232)
        };
        let centre = add(m.pos, scale(m.rows[2], 0.05));
        let f = unit(sub(cam, centre));
        let up0 = if f[2].abs() < 0.99 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let r = unit(cross(f, up0));
        let u = cross(r, f);
        let at = add(centre, scale(f, 0.35));
        for (k, (size, fx, rgba)) in [(0.27, 0x46, c0), (0.27, 0x46, c0), (0.22, 0x36, c1)].into_iter().enumerate() {
            // The texels' turn as the quad's own (the FX draw repeats its textures; turned texels would show its tiles).
            let (sn, cs) = (-self.shell_spin[k]).sin_cos();
            let (rr, uu) = (add(scale(r, cs), scale(u, sn)), sub(scale(u, cs), scale(r, sn)));
            let st = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
            let corner = |x: f32, y: f32| add(at, add(scale(rr, x * size), scale(uu, y * size)));
            out.push(crate::fx::FxQuad { fx, additive: true, corners: [corner(-1.0, 1.0), corner(1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0)], st, rgba: [rgba; 4] });
        }
    }

    /// `0x412910` on Wupash: 16 specks of dust streaming past — each set 24..30 ahead (its facing ±0.2) with 11..14 to
    /// run, stepping back 0.15..0.25 a tick (the ship's facing), drawn (`0x300aa0`) 0.07..0.12 across, FX 0x47,
    /// 0x7f7f6464.
    pub fn draw_dust(&mut self, w: &MobyWorld, cam: V3, rng: &mut crate::rng::Rng, out: &mut Vec<crate::fx::FxQuad>) {
        let Some(s) = self.ship else { return };
        if self.mode != 0 && self.mode != 1 && self.mode != 2 { return; }
        let (pos, fwd) = (w.mobys[s].pos, w.mobys[s].rows[0]);
        let st = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        for k in 0..16 {
            if self.dust_left[k] <= 0.0 {
                let j = [fwd[0] + rng.randf(-0.2, 0.2), fwd[1] + rng.randf(-0.2, 0.2), fwd[2] + rng.randf(-0.2, 0.2)];
                self.dust[k] = add(pos, set_len(j, rng.randf(24.0, 30.0)));
                self.dust_left[k] = rng.randf(11.0, 14.0);
            }
            let step = rng.randf(0.15, 0.25);
            self.dust[k] = add(self.dust[k], scale(fwd, -step));
            self.dust_left[k] -= step;
            let size = rng.randf(0.07, 0.12) * 0.5;
            let roll = rng.rand_angle();
            let p = self.dust[k];
            let f = unit(sub(cam, p));
            let up0 = if f[2].abs() < 0.99 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
            let (a0, b0) = (unit(cross(up0, f)), cross(f, unit(cross(up0, f))));
            let (sn, cs) = roll.sin_cos();
            let (a, b) = (add(scale(a0, cs), scale(b0, sn)), sub(scale(b0, cs), scale(a0, sn)));
            let corner = |x: f32, y: f32| add(p, add(scale(a, x * size), scale(b, y * size)));
            out.push(crate::fx::FxQuad { fx: 0x47, additive: true, corners: [corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0)], st, rgba: [0x7f7f_6464; 4] });
        }
    }
}
