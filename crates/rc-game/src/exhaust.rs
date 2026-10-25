//! Ratchet's ship's exhaust (L00 `0x362640`, one call per nozzle): the flight's draw (`0x363ba0` → `0x362ac8`) and the
//! fly-away's callback (`0x362ac8`) on the ship, Aranos's arrival ship's callback (`0x362b40`).
//!
//! **A nozzle** (`$gp−0x2ba0`, [`NOZZLES`]: the middle one, and the two side ones with the ship's upgrade bit 4,
//! `0x1a7af8 >> 4 & 1`): its frame the ship's root joint (the two keys' rotations blended by `+0x44`) through the
//! ship's rows, its point the nozzle's offset through that frame; its strength `f` = the ship's `+0x95` · the nozzle's
//! weight / 128. Two rows of sprites ([`flare`], `0x3624f0`) run back along the frame's forward row from it: the
//! outer `f·5 + 25` of them (from `f·0.375 + 0.375` across to `f·0.1 + 0.1`, jittered `f·0.075 + 0.075`, stepping
//! `f·0.06 + 0.06`, blue `0x40204080` → `0x40205080` by `f`) and on the middle nozzle the white core, `f·5 + 15` of them
//! (`f·0.025 + 0.175` → `f·0.025 + 0.025`, jitter 0.075, step `f·0.04 + 0.04`); FX 0x2e, 5 in the flight (`0x152c20` =
//! 2), additive. Then the ship's smoke steps ([`Exhaust::step`], `0x368360`) and one puff starts ([`Exhaust::puff`],
//! `0x3682b0`).
//!
//! **The smoke** (`gp−0x2d24`, 0x200 records of 0x50 bytes; `0x368258` empties it, its fade `gp−0x2d20` 1 and its
//! drift `0x1b2430` nothing): a puff starts at the nozzle ±0.025 with the drift as its velocity, a random roll, a spin
//! of 90..180 · 0.00029 a tick, 1..2 across, its colour `0x2f7f` → `0x7f7f` at random, its alpha 0x1f..0x3f (the
//! fly-away's 0x18..0x28), 70..90 ticks, FX 5, and (the flight's) its blend every other frame; each step its alpha
//! falls with its life left `t`, its colour goes toward grey `0x3f3f3f` by `t³`, it turns by its spin, grows 0.001..0.003,
//! moves by its velocity, which keeps 0.99 of itself. Drawn ([`Exhaust::draw`], `0x3684b0`) 0.001 toward the camera at
//! its alpha times the fade.

use crate::fx::{sprite, FxQuad};
use crate::hero::physics::{add, scale, V3};
use crate::hud::lerp_rgba;
use crate::rng::Rng;

/// `$gp−0x2ba0`: the nozzles' offsets in the ship's frame and their weights.
pub const NOZZLES: [[f32; 4]; 3] = [[-2.75, 0.0, 1.4, 1.0], [-2.75, 0.9, 2.05, 0.4], [-2.75, -0.9, 2.05, 0.4]];
const POOL: usize = 0x200;

#[derive(Clone, Copy, Debug)]
struct Puff {
    pos: V3,
    vel: V3,
    spin: f32,
    roll: f32,
    size: f32,
    base: u32,
    rgb: u32,
    alpha_max: i32,
    alpha: i32,
    flags: u32,
    life: i32,
    total: i32,
    owner: usize,
    fx: usize,
}

/// The smoke's pool, fade and drift.
#[derive(Clone, Debug)]
pub struct Exhaust {
    smoke: Vec<Puff>,
    /// `gp−0x2d20`: the puffs' alpha factor (the flight's arrival fades it in its last ticks).
    pub fade: f32,
    /// `0x1b2430`: the velocity a puff starts with.
    pub drift: V3,
}

impl Default for Exhaust {
    fn default() -> Self { Exhaust { smoke: Vec::new(), fade: 1.0, drift: [0.0; 3] } }
}

/// What the nozzles draw from.
pub struct Ship {
    /// The moby (the puffs' owner).
    pub id: usize,
    pub pos: V3,
    /// The root joint's frame through the ship's rows.
    pub rows: [V3; 3],
    /// `+0x95`.
    pub strength: u8,
    /// The upgrade's two side nozzles.
    pub three: bool,
}

/// `0x3624f0(first, last, jitter, from, step, n, rgba)`: `n` sprites from `from` by `step`, each a random roll and a
/// jitter (`0x3217d8`), across from `first` to `last`.
#[allow(clippy::too_many_arguments)]
fn flare(out: &mut Vec<FxQuad>, rng: &mut Rng, first: f32, last: f32, jitter: f32, from: V3, step: V3, n: i32, rgba: u32, fx: usize, cam: V3) {
    let mut p = from;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let roll = rng.rand_angle();
        let q: V3 = std::array::from_fn(|k| p[k] + rng.randf(-jitter, jitter));
        sprite(out, first + (last - first) * t, 0.0, roll, q, cam, fx, rgba, true);
        p = add(p, step);
    }
}

impl Exhaust {
    /// `0x368258`: the pool emptied, the fade 1, no drift.
    pub fn reset(&mut self) { *self = Exhaust::default(); }

    /// `0x362ac8` / `0x362b40`: each nozzle's flares, the ship's smoke stepped and a puff; `flight`: the flight's
    /// textures (`0x152c20` = 2); `fly_away`: the third argument (the fly-away's smaller alphas, one blend); `frame`:
    /// `0x1bca74` (the scene's tick).
    pub fn nozzles(&mut self, s: &Ship, flight: bool, fly_away: bool, frame: i32, rng: &mut Rng, cam: V3, out: &mut Vec<FxQuad>) {
        let fx = if flight { 5 } else { 0x2e };
        for (k, n) in NOZZLES.iter().enumerate().take(if s.three { 3 } else { 1 }) {
            let r = &s.rows;
            let at = add(s.pos, add(add(scale(r[0], n[0]), scale(r[1], n[1])), scale(r[2], n[2])));
            let f = s.strength as f32 * n[3] * 0.0078125;
            let outer = lerp_rgba(f, 0x4020_4080, 0x4020_5080);
            flare(out, rng, f * 0.375 + 0.375, f * 0.1 + 0.1, f * 0.075 + 0.075, at, scale(r[0], -(f * 0.06 + 0.06)), (f * 5.0 + 25.0) as i32, outer, fx, cam);
            if k == 0 {
                flare(out, rng, f * 0.025 + 0.175, f * 0.025 + 0.025, 0.075, at, scale(r[0], -(f * 0.04 + 0.04)), (f * 5.0 + 15.0) as i32, 0xffff_ffff, fx, cam);
            }
            self.step(s.id, rng);
            self.puff(s.id, at, fly_away, frame, rng);
        }
    }

    /// `0x368360(owner)`: the owner's puffs a tick on.
    fn step(&mut self, owner: usize, rng: &mut Rng) {
        for p in self.smoke.iter_mut().filter(|p| p.owner == owner) {
            let t = p.life as f32 / p.total as f32;
            p.alpha = (p.alpha_max as f32 * t) as i32;
            p.rgb = lerp_rgba(t * t * t, 0x3f_3f3f, p.base);
            p.roll = crate::hero::physics::add_rot(p.roll, p.spin);
            if p.flags != 0 { p.size += rng.randf(0.001, 0.003); }
            p.pos = add(p.pos, p.vel);
            p.vel = scale(p.vel, 0.99);
            p.life -= 1;
        }
        self.smoke.retain(|p| p.life > 0);
    }

    /// `0x362640`'s puff into `0x3682b0` (the first free record of 0x200).
    fn puff(&mut self, owner: usize, at: V3, fly_away: bool, frame: i32, rng: &mut Rng) {
        let base = lerp_rgba(rng.randf(0.0, 1.0), 0x2f7f, 0x7f7f);
        let alpha_max = if fly_away { rng.rand_range(0x18, 0x28) } else { rng.rand_range(0x1f, 0x3f) };
        let size = rng.randf(1.0, 2.0);
        let pos: V3 = std::array::from_fn(|k| at[k] + rng.randf(-0.025, 0.025));
        let roll = rng.rand_angle();
        let spin = rng.randf(90.0, 180.0) * 0.000_290_888_25;
        let life = rng.rand_range(0x46, 0x5a);
        let flags = if fly_away { 1 } else { ((frame % 2 + 1) * 2 | 1) as u32 };
        if self.smoke.len() < POOL {
            self.smoke.push(Puff { pos, vel: self.drift, spin, roll, size, base, rgb: base, alpha_max, alpha: 0, flags, life, total: life, owner, fx: 5 });
        }
    }

    /// `0x3684b0`: every puff, 0.001 toward the camera; blended normally when its flags' upper bits are 1.
    pub fn draw(&self, cam: V3, out: &mut Vec<FxQuad>) {
        for p in &self.smoke {
            let rgba = (p.rgb & 0xff_ffff) | ((p.alpha as f32 * self.fade) as i32 as u32) << 24;
            sprite(out, p.size, 0.001, p.roll, p.pos, cam, p.fx, rgba, p.flags >> 1 != 1);
        }
    }
}
