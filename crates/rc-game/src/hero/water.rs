//! The water: swimming on the surface (group 0x12: the stroke 0x35, treading 0x36), under it (group 0x11: the swim
//! 0x32, the drift 0x33, the Clank-driven fast swim 0x34) and the hurts there (0x72 on the surface, 0x73 under) —
//! SetState (L00 `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`), with the prologue's water checks
//! (`0x2c9098` in, `0x2c8f38` out and the dive), the surface bob `0x2c94b0` and the swim's effects.
//!
//! **Where the water is.** The ground probe (`0x2b4a08`, crate::hero::physics): a surface-0 face under the feet sets
//! the water level `0x18a150` to the face's height through `0x31d7a0` (the ripple module's patch height `0x37dc10`
//! when the level has one — G-REN-009, not ported: the face's own height is used — else a flat plane `0x1b25bc`
//! nothing writes on L00, else the face's height) and the line goes on through it; a deep liquid (0xd) sets it too, the
//! quicksand (3) and a pit (0xb) set `0x18a154`. The surface step (`0x2aca80`, crate::hero::surface) turns the surface
//! id into this tick's flags, the depth `0x18c2e4` and the wading `0x18c0d9`.
//!
//! **In and out.** Falling into water deeper than 0.8 with the feet within the vertical speed (+0.07, at least 0.2)
//! of 0.45 below its surface, or sinking 0.8 below it, gives 0x36 (with no health 0x67). Every 16th frame (not on
//! level 0xc) a line from 4 (16 every 63rd) above down to 1.3 above the feet finds a water surface with nothing
//! under it: 0x33. Under water, 0.4 above the surface again: the fall 6. On the surface, ■ within 7 ticks (R1 / R2
//! with Clank) dives.
//!
//! **Under water.** ✕ pitches up, ■ down (to 78°: the button's pressure, or a ramp of 1/40 a tick from half), the
//! pitch springs there (0.015, 0.2); the yaw follows the stick (0.007, 0.08, 0.087); the speed eases toward 0.05
//! (the dive 0.1, the fast swim 0.047 or 0.117 with R1 / R2) by 1/514 / 1/900 (fast: 1/277 / 1/450); the stroke's
//! speed follows the sequence (`0x25dc90`, × 5); the body rolls into the turn.
//!
//! **On the surface.** The stroke moves at the stick's 0.05 (the stroke curve × 5.5), turning as under water; the
//! height bobs on a spring to the surface (0.027, 0.3) less 0.12, the bob's own offset settling (0.005, 0.045).
//!
//! **No air.** Going Commando has no drowning: the air `0x18c2e0` is set to 10000 out of the water (`0x2c8f38`), zeroed
//! by the level load, and never counted down (its one reader, the physics' "under 1500: full pitch up", never fires);
//! there is no air meter on the HUD.
//!
//! **Effects** (the game's spawners, through [`WaterFx`] and [`fx_pass`], which the engine runs after the hero with the
//! particle pool, crate::particles::water): the splash `0x2aac58(rings, drops, big)` ([`Hero::splash`]: the plunge into
//! water from 0x36's entry — rings 3, drops up to 40 by the fall speed — the surfacing's 3 / 10, the dive's and the
//! jump out's 3 / 16), the treading wake `0x2aa4f0` (rings every 15+ ticks, reset to `randi(15)`... [`Hero::wake`]),
//! the bow rings `0x2aa7c8` (drops ahead when moving, [`Hero::bow_rings`]), the spray `0x2aa5e8` (two spray sprites at
//! ±45° of the facing in the stroke, [`Hero::spray`]), the bubbles `0x2aa9b0(n, mode, 0)` ([`Hero::bubbles`]: at the
//! feet, or at his joints 0 / 0xe), the plunge's trailing bubbles (`0x18a86a`: 75 after a plunge, 70 after a dive,
//! fewer each tick), the stroke's four bubbles from the hands early in the stroke sequence, the hurt 0x73's bubbles and
//! the breath (`0x18a850`: a 25200 bubble from the mouth joint 4 every `rand_range(4, 11)` or `rand_range(40, 90)`
//! ticks, [`Hero::breath_bubble`]; in the water death 0x67 every `rand_range(2, 5) + timer / 5` then `rand_range(10,
//! 20)`, [`Hero::drown_breath`]).
//!
//! Not ported: the ripple disturbances (`0x37d5a0`, with the ripple module: G-REN-009), the fast swim's jets
//! (`0x18a858`: four bubbles a time from Clank's joints 0 / 1 while the back item is class 0x261 — the back slot's moby
//! is not ported), the splash moby 0x307 (in no level's class table: the game spawns none either), the dive's gate on
//! Ratchet's pvar `+0x134` and the scripted no-jump / no-dive timers `0x18a018` / `0x18a01e` (always allowed here),
//! the swim's voice `0x2d6cc8(0x16)`, the swim stats counters (G-HERO-005), the help flag `0x18cf10`, the body lean's
//! records (`0x25bea0..`, `0x2ab138`: G-HERO-002), the pressure buttons (the ramps are used: no pressure-mode pad
//! `0x13835c` = 0x79), the vehicle's water.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::hero::anim::AnimCtl;
use crate::moby::{Ctx, MobyWorld};
use crate::pad::button;

/// `0x25dc90`: the stroke's speed by the frame of its sequence (30 frames).
const STROKE: [f32; 30] = [
    0.4, 0.4, 0.45, 0.52, 0.6, 0.68, 0.78, 0.89, 1.0, 1.0, 1.0, 1.0, 0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5,
    0.45, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4,
];
/// R2 | R1 (`0x138320 & 10`): the fast swim's buttons.
const FAST: u32 = button::R1 | button::R2;

/// Where a bubble starts: a point, or one of Ratchet's joint lists (`0x3127c8`) plus an offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BubbleAt {
    World(V3),
    Joint(usize, V3),
}

/// One effect the hero's code asked for this tick, spawned by [`fx_pass`] in the game's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaterFx {
    /// `0x339b60(size, 5250, pos, &water_z, −1)`: a ring on the water level.
    Ring { size: f32, pos: V3 },
    /// `0x337cc0(pos, vel, kind, life)`: a drop.
    Drop { pos: V3, vel: V3, kind: i32, life: i32 },
    /// `0x339e10(size, spin, pos, vel, &water_z)`: the bow spray.
    Spray { size: f32, spin: f32, pos: V3, vel: V3 },
    /// `0x3378b0(size, top, pos, vel)`: a bubble rising to `top`.
    Bubble { size: f32, top: f32, at: BubbleAt, vel: V3 },
}

/// The water block (`0x18a840..0x18a874`, `0x18a5b0..0x18a5c4`).
#[derive(Clone, Debug, Default)]
pub struct WaterBlock {
    /// `0x18a840`: the dive's ticks; `0x18a844` / `0x18a848`: the roll's and pitch's spring velocities; `0x18a84c`:
    /// the yaw velocity of the turn.
    pub dive_ticks: i32,
    pub roll_v: f32,
    pub pitch_v: f32,
    pub yaw_v: f32,
    /// `0x18a860` / `0x18a862`: the ticks ✕ / ■ have been held.
    pub up_held: i16,
    pub down_held: i16,
    /// `0x18a5b0` the bob's offset and `0x18a5b4` its speed, `0x18a5bc` the surface the bob follows and `0x18a5c0`
    /// its spring's velocity, `0x18a5b8` the stroke's hold.
    pub bob: f32,
    pub bob_v: f32,
    pub level: f32,
    pub level_v: f32,
    pub stroke_hold: i16,
    /// `0x18a158`: ticks since he went below the surface (0: above it).
    pub under: i32,
    /// `0x18a850`: ticks to the next breath bubble; `0x18a854`: a hold the swim entries set (45; the fast swim's 60),
    /// counted down under water and read by nothing.
    pub breath: i32,
    pub hold: i32,
    /// `0x18a864` / `0x18a866` / `0x18a868`: the wake's, the bow rings' and the spray's tick counters.
    pub wake: i16,
    pub bow: i16,
    pub spray: i16,
    /// `0x18a86a`: the plunge's trailing bubbles left.
    pub plunge: i16,
    /// The effects asked for this tick, in the game's order ([`fx_pass`] spawns them).
    pub fx: Vec<WaterFx>,
}

/// `0x2eeab0(&x)`: x down one (never below 0); 1 when it was already 0, 2 when it reached 0 now, else 0.
fn tick_down(x: &mut i32) -> i32 {
    if *x == 0 { return 1; }
    *x = (*x).max(1) - 1;
    if *x < 1 { 2 } else { 0 }
}

impl Hero {
    /// `0x2a9fa8(table, n)`: the table's value at Ratchet's frame, blended with the one before.
    fn stroke(&self, anim: &dyn AnimCtl) -> f32 {
        let v = anim.view();
        let i = (v.frame.max(0.0) as usize).min(STROKE.len() - 1);
        let j = if v.flags & 2 != 0 { STROKE.len() - 1 } else { (i + STROKE.len() - 1) % STROKE.len() };
        let f = v.frame.fract();
        f * STROKE[i] + (1.0 - f) * STROKE[j]
    }

    /// The prologue's water checks (`0x2c9098`, `0x2c8f38`). Returns whether a state was set.
    pub fn water_checks(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        if self.state == 0x74 || self.state == 0x67 { return false; }
        let wz = self.surf.water_z;
        let mut into = false;
        if self.surf.water {
            if self.group == 0x11 {
                if self.substate != 1 && 0.0 < self.disp[2] && wz - 0.4 < self.pos[2] { into = true; }
            } else if self.group != 0x12 && self.group != 3 {
                let reach = (self.vel[2].abs() + 0.07).max(0.2);
                let near = (wz - (self.pos[2] + 0.45)).abs() < reach;
                if near && (self.group != 4 || self.jump.descending != 0) && 0.8 < self.surf.depth && self.disp[2] < 0.0 { into = true; }
                if self.pos[2] < wz - 0.8 && (self.group != 4 || self.jump.descending != 0) { into = true; }
            }
        }
        if self.state == 0x12 && self.jump.descending != 0 && self.pos[2] < wz - 0.7 { into = true; }
        // Under a water surface with nothing below it (every 16th frame; not on level 0xc).
        if env.level != 0xc && !matches!(self.group, 0x11 | 0x12 | 7) && self.state != 0x12 && self.frame & 0xf == 0 {
            let up = if self.frame % 0x3f == 0 { 16.0 } else { 4.0 };
            let a = add(self.pos, [0.0, 0.0, up]);
            let b = add(self.pos, [0.0, 0.0, 1.3]);
            if let Some(o) = env.line(a, b, 2) {
                if o.surface_id() == 0 && env.line([a[0], a[1], o.point[2] - 0.01], b, 2).is_none() {
                    let id = if self.group != 0x14 && self.hp != 0 { 0x33 } else { 0x67 };
                    return self.set_state(anim, env, id, true);
                }
            }
        }
        if into {
            // Into the water: its sound 3 (`0x2b9c90(3, 0)`) after the state.
            self.sounds.push((3, 0));
            let id = if self.group == 0x14 || self.hp == 0 { 0x67 } else { 0x36 };
            if self.group == 0x11 {
                // Up through the surface from below (`0x2c9098` returns 1; the treading 0x36).
                return self.set_state(anim, env, 0x36, true);
            }
            return self.set_state(anim, env, id, true);
        }
        // `0x2c8f38`: out of the water from below; the dive from the surface.
        if self.group == 0x11 && wz + 0.4 < self.pos[2] {
            self.underwater_write = Some(false);
            return self.set_state(anim, env, 6, true);
        }
        if self.group != 0x12 { return false; }
        let a = [self.pos[0], self.pos[1], wz + 0.01];
        let b = [self.pos[0], self.pos[1], wz + 0.31];
        if env.line(a, b, 2).is_none() { return false; }
        let id = if env.clank && env.pad.held & FAST != 0 { 0x34 } else { 0x32 };
        self.set_state(anim, env, id, true)
    }

    /// `0x2c94b0`: the surface bob (module docs). The drowning 0x7c (crate::hero::damage) bobs too.
    pub(super) fn surface_bob(&mut self) {
        let w = &mut self.water;
        if 10 < self.timer || w.bob < 0.0 || self.prev_group == 0x11 || self.prev2_group == 0x11 {
            if self.state == 0x35 {
                let (mut b, mut v) = (w.bob, self.damage.fall_v);
                spring(0.0, 0.03, 0.3, 0.025, &mut b, &mut v);
                (w.bob, self.damage.fall_v) = (b, v);
                w.bob_v = 0.0;
            } else {
                w.bob_v -= w.bob * 0.005 + w.bob_v * 0.045;
                if w.bob.abs() < 0.001 && w.bob_v < 0.0001 { w.bob_v = 0.0; }
            }
        }
        w.bob += w.bob_v;
        if self.state == 0x35 && 0.0 < w.bob {
            if 0.0 < w.bob_v { w.bob_v = 0.0; }
            approach(0.0, 0.011_666_667, &mut w.bob);
        }
        if self.state != 0x7c {
            let (mut l, mut v) = (w.level, w.level_v);
            spring(self.surf.water_z, 0.027, 0.3, 0.0, &mut l, &mut v);
            (w.level, w.level_v) = (l, v);
        }
        self.pos[2] = w.level - 0.12 + w.bob;
    }

    // ---------------------------------------------------------------------------------------------
    // The effects (the spawners' random draws made here, in the game's order; the spawns in `fx_pass`).

    /// A bubble's top (`0x3378b0`'s −1): the water level, or 0.4 above him in the slide 0x30.
    fn bubble_top(&self) -> f32 { if self.state == 0x30 { self.pos[2] + 0.4 } else { self.surf.water_z } }

    /// `0x2aac58(rings, drops, big)`: `rings` rings within 0.3 of him on the water level (sizes 0.3..0.6) and `drops`
    /// drops (a random direction at up to 0.05 a tick, 0.05..0.133 up, from up to 0.2 under the level, kinds 0 / 1,
    /// 90..120 ticks). The game moves a drop's start by 8 × its x velocity on both axes; kept. `big`: the splash moby
    /// 0x307 (`0x39f938(2.25, (x, y, level))`) — the class is in no level's table, so the game spawns none either.
    pub(super) fn splash(&mut self, rings: i32, drops: i32, _big: bool) {
        let (p, wz) = (self.pos, self.surf.water_z);
        for _ in 0..rings.max(0) {
            let x = p[0] + self.rng.randf(-0.3, 0.3);
            let y = p[1] + self.rng.randf(-0.3, 0.3);
            let size = self.rng.randf(0.3, 0.6);
            self.water.fx.push(WaterFx::Ring { size, pos: [x, y, wz] });
        }
        for _ in 0..drops.max(0) {
            let a = self.rng.rand_angle();
            let s = self.rng.randf(0.0, 0.050_000_004);
            let (vx, vy) = (a.cos() * s, a.sin() * s);
            let vz = self.rng.randf(0.050_000_004, 0.133_333_34);
            let (x, y) = (p[0] + vx * 8.0, p[1] + vx * 8.0);
            let z = wz - self.rng.randf(0.0, 0.2);
            let kind = self.rng.randi(2);
            let life = self.rng.rand_range(0x5a, 0x78);
            self.water.fx.push(WaterFx::Drop { pos: [x, y, z], vel: [vx, vy, vz], kind, life });
        }
    }

    /// `0x2aa4f0(max, after)`: the counter `0x18a864` up; past `after` one ring (size 0.4..0.6) within 0.05 of him on the
    /// water level, and the counter restarts at `randi(max)`.
    pub(super) fn wake(&mut self, max: i32, after: i16) {
        self.water.wake = self.water.wake.wrapping_add(1);
        if self.water.wake <= after { return; }
        let x = self.pos[0] + self.rng.randf(-0.05, 0.05);
        let y = self.pos[1] + self.rng.randf(-0.05, 0.05);
        let size = self.rng.randf(0.4, 0.6);
        self.water.fx.push(WaterFx::Ring { size, pos: [x, y, self.surf.water_z] });
        self.water.wake = self.rng.randi(max) as i16;
    }

    /// `0x2aa7c8(max, after)`: the counter `0x18a866` up; past `after` one drop 8 ticks of his displacement ahead
    /// (within 0.15), 0.1 above the water level, moving 0.75 of his displacement plus up to 0.05 in a random direction
    /// and 0.05..0.108 up, for 90..120 ticks, kind 3 in water less than 0.25 deep (it rings his water level) else 1
    /// (it rings the water face it meets); the counter restarts at `randi(max)`.
    pub(super) fn bow_rings(&mut self, max: i32, after: i16) {
        self.water.bow = self.water.bow.wrapping_add(1);
        if self.water.bow <= after { return; }
        let d = self.disp;
        let x = self.pos[0] + self.rng.randf(-0.15, 0.15) + d[0] * 8.0;
        let dy = self.rng.randf(-0.15, 0.15);
        let z = self.surf.water_z + 0.1;
        let y = self.pos[1] + dy + d[1] * 8.0;
        let a = self.rng.rand_angle();
        let k = self.rng.randf(0.0, 0.050_000_004);
        let vx = a.cos() * k + d[0] * 0.75;
        let vy = a.sin() * k + d[1] * 0.75;
        let vz = self.rng.randf(0.050_000_004, 0.108_333_34);
        let life = self.rng.rand_range(0x5a, 0x78);
        let kind = if self.surf.depth < 0.25 { 3 } else { 1 };
        self.water.fx.push(WaterFx::Drop { pos: [x, y, z], vel: [vx, vy, vz], kind, life });
        self.water.bow = self.rng.randi(max) as i16;
    }

    /// `0x2aa5e8(max, after)`: the counter `0x18a868` up; past `after` two sprays (sizes 0.4..0.5) on the water level,
    /// 18 ticks of 0.01 along the facing ± 45° ahead, moving that less 0.025 along the facing, spinning ±2; the counter
    /// restarts at `randi(max)` (0 with `max` 0) — once per spray, as the game does.
    pub(super) fn spray(&mut self, max: i32, after: i16) {
        self.water.spray = self.water.spray.wrapping_add(1);
        if self.water.spray <= after { return; }
        let yaw = self.rot[2];
        for i in 0..2 {
            let a = if i == 0 { add_rot(yaw, 0.785_398_1) } else { sub_rot(yaw, 0.785_398_1) };
            let (mut vx, mut vy) = (a.cos() * 0.010_000_001, a.sin() * 0.010_000_001);
            let pos = [self.pos[0] + vx * 18.0, self.pos[1] + vy * 18.0, self.surf.water_z];
            vx -= yaw.cos() * 0.025_000_002;
            vy -= yaw.sin() * 0.025_000_002;
            let size = self.rng.randf(0.4, 0.5);
            let spin = if i == 0 { 2.0 } else { -2.0 };
            self.water.fx.push(WaterFx::Spray { size, spin, pos, vel: [vx, vy, 0.0] });
            self.water.spray = if max == 0 { 0 } else { self.rng.randi(max) as i16 };
        }
    }

    /// `0x2aa9b0(n, mode, 0)`: `n` bubbles (sizes 4200..7350) at him (mode 0: within 0.1 across, 0.5 under to 0.1 above
    /// the feet) or at his joint lists 0 / 0xe (mode 1) or 0x17 / 0x16 (mode 2) by turns, moving his displacement
    /// × 0.7 (× 1.15 in the drift 0x33) plus a little sideways and down.
    pub(super) fn bubbles(&mut self, n: i32, mode: i32) {
        let top = self.bubble_top();
        for i in 0..n.max(0) {
            let at = match mode {
                0 => {
                    let x = self.pos[0] + self.rng.randf(-0.1, 0.1);
                    let y = self.pos[1] + self.rng.randf(-0.1, 0.1);
                    let z = self.pos[2] + self.rng.randf(-0.5, 0.1);
                    BubbleAt::World([x, y, z])
                }
                1 => BubbleAt::Joint(if i & 1 == 0 { 0 } else { 0xe }, [0.0; 3]),
                _ => BubbleAt::Joint(if i & 1 == 0 { 0x17 } else { 0x16 }, [0.0; 3]),
            };
            let mut v = scale(self.disp, if self.state == 0x33 { 1.15 } else { 0.7 });
            v[0] += self.rng.randf(-0.006_666_667, 0.006_666_667);
            v[1] += self.rng.randf(-0.006_666_667, 0.006_666_667);
            v[2] += self.rng.randf(-0.016_666_668, 0.0);
            let size = self.rng.randf(4200.0, 7350.0);
            self.water.fx.push(WaterFx::Bubble { size, top, at, vel: v });
        }
    }

    /// The breath (`0x2ba8e0`, cases 0x32..0x34 and 0x67): one 25200 bubble from the mouth (joint list 4, 0.4 along his
    /// x row), moving his displacement plus 0.025 along that row (`0x2ef138`: the offset at length 0.025) and 1/30 up.
    fn breath_bubble(&mut self) {
        let top = self.bubble_top();
        let mut v = add(self.disp, set_len(scale(self.rows[0], 0.4), 0.025_000_002));
        v[2] += 0.033_333_335;
        self.water.fx.push(WaterFx::Bubble { size: 25_200.0, top, at: BubbleAt::Joint(4, scale(self.rows[0], 0.4)), vel: v });
    }

    /// The swim's breath: when `0x18a850` is out, a bubble, then 4..11 ticks (two in five) or 40..90 to the next.
    fn swim_breath(&mut self) {
        if tick_down(&mut self.water.breath) == 0 { return; }
        self.breath_bubble();
        self.water.breath = if self.rng.randi(100) < 40 { self.rng.rand_range(4, 0xb) } else { self.rng.rand_range(0x28, 0x5a) };
    }

    /// The water death's breath (`0x2ba8e0` case 0x67 / 0x7f): when `0x18a850` runs out this tick, a bubble; before
    /// 40 ticks the next in `rand_range(2, 5) + timer / 5`, after 70 in `rand_range(10, 20)` (between: at once).
    pub(super) fn drown_breath(&mut self) {
        if tick_down(&mut self.water.breath) != 2 { return; }
        self.breath_bubble();
        if self.timer < 0x28 {
            self.water.breath = self.rng.rand_range(2, 5) + self.timer / 5;
        } else if 0x46 < self.timer {
            self.water.breath = self.rng.rand_range(10, 0x14);
        }
    }

    /// The jump group's entry (`0x2c4560`): leaving water (not the water jump 0x12 itself) within 0.5 of its level
    /// splashes 3 rings and 16 drops.
    pub(super) fn jump_splash(&mut self, id: i32) {
        if id != 0x12 && self.surf.water && self.pos[2] < self.surf.water_z + 0.5 { self.splash(3, 0x10, false); }
    }

    // ---------------------------------------------------------------------------------------------

    pub(super) fn water_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        let from = self.prev_group;
        match id {
            0x32 | 0x34 => {
                self.group = 0x11;
                self.cam_mode = 0xb;
                if id == 0x32 { self.damage.fall_v = 0.0; }
                if from == 0x12 { self.substate = 1; } else if id == 0x32 { self.water.hold = 0x2d; }
                if id == 0x34 { self.water.hold = 0x3c; }
                self.water.dive_ticks = 0;
                if id == 0x32 { return Entry::Anim(12.0, 0x3b, 0); }
                if play { self.set_anim(anim, if self.prev_state == 0x32 { 15.0 } else { 11.0 }, 0x3d, 0); }
                Entry::Done
            }
            0x33 => {
                self.group = 0x11;
                self.cam_mode = 0xb;
                self.momentum = self.eff;
                Entry::Anim(if self.prev_state == 0x34 { -2.0 } else { 35.0 }, 0x3c, 0)
            }
            0x35 => {
                self.group = 0x12;
                self.water.stroke_hold = 0x2d;
                self.damage.fall_v = 0.0;
                Entry::Anim(17.0, 0x5e, 0)
            }
            0x36 => {
                self.group = 0x12;
                self.water.plunge = 0;
                if from == 0x11 {
                    // Up through the surface: 3 rings and 10 drops (and a ripple: not ported).
                    self.splash(3, 10, false);
                } else if self.disp[2] < -0.008_333_334 {
                    // The plunge: 3 rings, up to 40 drops by the fall speed, the big splash; 75 ticks of bubbles.
                    let n = ((self.disp[2].abs() * 300.0) as i32).min(0x28);
                    self.splash(3, n, true);
                    self.water.plunge = 0x4b;
                }
                let w = &mut self.water;
                w.bob_v = (self.disp[2] * 0.37).max(-0.116_666_675);
                w.stroke_hold = 0;
                w.level = self.surf.water_z;
                w.level_v = 0.0;
                w.bob = self.pos[2] - self.surf.water_z + 0.12;
                self.momentum = [self.eff[0], self.eff[1], 0.0];
                if 0.091_666_67 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.091_666_67); }
                Entry::Anim(22.0, 0x3a, 0)
            }
            _ => {
                // 0x72 / 0x73: hurt in the water.
                self.group = 7;
                self.scale_motion(0.5);
                self.vel = self.disp;
                let dmg = self.pending_damage.take().unwrap_or(1);
                self.take_damage(dmg);
                self.damage.invincible = 0x4d;
                if play {
                    self.set_anim(anim, -3.0, if id == 0x72 { 0x69 } else { 0x68 }, 3);
                    self.damage.hurts += 1;
                }
                Entry::Done
            }
        }
    }

    pub(super) fn phys_water(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            0x35 | 0x36 => {
                if self.state == 0x36 { self.wake(0xf, 0x1e); }
                if 0.025_000_002 < self.eff_len_xy { self.bow_rings(4, 0xc); }
                if self.state == 0x35 && 0.016_666_668 < self.eff_len_xy { self.spray(0, 1); }
                if self.water.plunge != 0 {
                    self.water.plunge -= 1;
                    let n = (self.water.plunge as i32 >> 3).min(6);
                    self.bubbles(n, 0);
                }
                self.anim_speed = 0.6;
                self.stick_target(env, 0.05, 0);
                if !env.pad.no_direction && self.target_speed < 0.025 { self.target_speed = 0.025; }
                self.turn_to(0.007, 0.08, 0.087_266_47, 0);
                if 0 < self.water.stroke_hold { self.water.stroke_hold -= 1; }
                if self.state == 0x35 {
                    if env.pad.no_direction && self.water.stroke_hold != 0 { self.target_speed = 0.05; }
                    if PI / 4.0 < self.yaw_residual.abs() { self.target_speed = 0.0; }
                    self.speed_step(0.001_111_111_2, 0.001_388_888_9);
                    let k = self.speed * self.stroke(anim) * 5.5;
                    let (s, c) = self.rot[2].sin_cos();
                    self.vel = [c * k, s * k, 0.0];
                } else {
                    self.speed = 0.0;
                    self.vel = [0.0; 3];
                }
                // The stroke's hands early in its sequence: four bubbles from them.
                let f = anim.view().frame;
                if self.state == 0x35 && !anim.view().blending() && 0.0 < f && f < 10.0 { self.bubbles(4, 1); }
                self.momentum_decay(0.001_166_666_7);
                self.wall_check(env, false);
                self.surface_bob();
            }
            0x72 => {
                self.wake(0xf, 0x1e);
                self.vel[2] = 0.0;
                let l = len2(self.vel);
                self.vel = set_len2(self.vel, l * 0.92);
                self.wall_check(env, false);
                self.surface_bob();
            }
            0x73 => {
                let n = (10 - self.timer / 3).min(8);
                self.bubbles(n, 0);
                self.vel[2] = 0.0;
                let l = len2(self.vel);
                self.vel = set_len2(self.vel, l * 0.92);
                self.wall_check(env, false);
            }
            _ => self.phys_underwater(env, anim),
        }
    }

    /// `0x2ba8e0` case 0x32 / 0x33 / 0x34.
    fn phys_underwater(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        let held = env.pad.held;
        // The plunge's trailing bubbles, fewer each tick.
        if self.water.plunge != 0 {
            let mut p = self.water.plunge as i32;
            tick_down(&mut p);
            self.water.plunge = p as i16;
            self.bubbles((p >> 2).min(8), 0);
        }
        let f = self.anim_frame;
        if self.state == 0x32 && !anim.view().blending() && 0.0 < f && f < 10.0 { self.bubbles(4, 1); }
        if self.state == 0x33 && self.prev_state == 0x34 && self.timer < 0xf { self.bubbles((0x14 - self.timer) / 3 + 1, 1); }
        if self.substate == 1 {
            if self.timer == 1 {
                // The dive: 3 rings and 16 drops (and a ripple: not ported), 70 ticks of bubbles.
                self.splash(3, 0x10, false);
                self.water.plunge = 0x46;
            }
            self.water.dive_ticks += 1;
            if 0x28 < self.water.dive_ticks { self.substate = 0; }
        }
        let pitch0 = self.rot[1];
        let mut t = 0.0;
        let w = &mut self.water;
        if held & (button::CROSS | button::SQUARE) == button::SQUARE || self.substate == 1 {
            w.down_held += 1;
            w.up_held = 0;
            t = (w.down_held as f32 * 0.025 + 0.5).min(1.0);
            if self.substate == 1 { t = t.max(0.35); }
            t *= 1.361_357;
        } else if held & button::CROSS == 0 {
            w.up_held = 0;
            w.down_held = 0;
        } else {
            w.up_held += 1;
            w.down_held = 0;
            t = -(w.up_held as f32 * 0.025 + 0.5).min(1.0) * 1.361_357;
        }
        if self.state == 0x33 { t = 0.0; }
        let (mut a, mut v) = (self.rot[1], self.water.pitch_v);
        turn_spring(t, 0.015, 0.2, 0.011_635_531 + 0.008_726_647, &mut a, &mut v, 0);
        (self.rot[1], self.water.pitch_v) = (a, v);
        let _ = sub_rot(self.rot[1], pitch0);
        self.stick_target(env, 1.0, 0);
        self.turn_to(0.007, 0.08, 0.087_266_47, 0);
        self.water.yaw_v = self.yaw_vel;
        tick_down(&mut self.water.hold);
        self.target_speed = if self.substate == 1 {
            0.1
        } else if self.state == 0x34 {
            if held & FAST == 0 { 0.046_666_67 } else { 0.116_666_675 }
        } else {
            0.05
        };
        let mut m = self.stick_mag;
        if (held & (button::CROSS | button::SQUARE) != 0 || self.substate == 1) && m < 1.0 { m = 1.0; }
        if 0.15 < self.stick_mag { m = m.max(if self.state == 0x34 { 0.4 } else { 0.35 }); }
        if self.state != 0x34 || held & FAST == 0 { self.target_speed *= m; }
        self.swim_breath();
        if self.state == 0x34 { self.speed_step(0.003_611_111, 0.002_222_222_3) } else { self.speed_step(0.001_944_444_5, 0.001_111_111_2) }
        let p = -self.rot[1];
        let dir = |speed: f32, yaw: f32| {
            let (sy, cy) = yaw.sin_cos();
            let (sp, cp) = p.sin_cos();
            [cy * speed * cp, sy * speed * cp, sp * speed]
        };
        match self.state {
            0x34 => self.vel = dir(self.speed, self.rot[2]),
            0x32 => {
                let mut k = self.speed * self.stroke(anim) * 5.0;
                if self.prev_state == 0x34 && self.timer < 0xf { k = k.max(0.075); }
                self.vel = dir(k, self.rot[2]);
            }
            _ => {
                let step = if self.prev_state == 0x34 { 0.002_222_222_3 } else { 0.001_111_111_2 };
                let mut l = len(self.momentum);
                approach(0.0, step, &mut l);
                self.momentum = set_len(self.momentum, l);
                self.vel = self.momentum;
            }
        }
        self.wall_check(env, false);
        let roll = self.water.yaw_v * p.cos() * -7.0;
        let (mut a, mut v) = (self.rot[0], self.water.roll_v);
        turn_spring(roll, 0.007, 0.17, 0.020_362_3, &mut a, &mut v, 0);
        (self.rot[0], self.water.roll_v) = (a, v);
    }

    pub(super) fn trans_water(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let held = env.pad.held;
        let fast = env.clank && held & FAST != 0;
        let fast_or = |s: &Self| if env.clank && env.pad.held & FAST != 0 { 0x34 } else { let _ = s; 0x32 };
        match self.state {
            0x32 | 0x34 => {
                if self.state == 0x32 {
                    self.anim_speed = (self.speed * 15.0).clamp(0.25, 0.75);
                    let f = anim.view().frame;
                    if held & (button::CROSS | button::SQUARE) == 0 && 9.0 < f && f < 19.0 && self.substate != 1 && self.stick_mag < 0.1 && self.speed < 1.0 / 30.0 {
                        return Some(0x33);
                    }
                }
                if fast {
                    if self.state == 0x32 && 0x14 < self.timer { return Some(0x34); }
                } else if self.state != 0x34 {
                    return None;
                } else if 0.5 < self.stick_mag {
                    return Some(0x32);
                }
                if self.state == 0x34 && held & (button::CROSS | button::SQUARE) == 0 && self.substate != 1 && self.stick_mag < 0.7 && held & FAST == 0 {
                    return Some(0x33);
                }
                None
            }
            0x33 => {
                let v = anim.view();
                if v.flags & 2 != 0 && v.seq_b != 0x3c { self.set_anim(anim, 11.0, 0x3c, 0); }
                if self.stick_mag <= 0.2 && held & (button::CROSS | button::SQUARE) == 0 && !fast { return None; }
                Some(fast_or(self))
            }
            0x35 => {
                if Self::cross_within(env, 8) { return Some(0x12); }
                if env.pad.pressed_within(button::SQUARE, 7).is_some() { return Some(fast_or(self)); }
                if self.surf.wading && self.surf.depth < 0.7 && self.set_state(anim, env, 0x70, false) {
                    self.set_anim(anim, 17.0, 0x59, 0);
                    self.disp[2] = 0.0;
                    self.vel[2] = 0.0;
                    return None;
                }
                if 0.1 <= self.stick_mag || self.speed != 0.0 || self.water.stroke_hold != 0 { return None; }
                Some(0x36)
            }
            0x36 => {
                if Self::cross_within(env, 8) && 10 < self.timer { return Some(0x12); }
                // Treading keeps the △ menu away (`0x18c0d7` = 1 every tick).
                self.no_menu = 1;
                let dive = env.pad.pressed_within(button::SQUARE, 7).is_some() || (env.clank && env.pad.pressed_within(FAST, 7).is_some());
                if dive { return Some(fast_or(self)); }
                if self.surf.wading && self.surf.depth < 0.4 && self.set_state(anim, env, 0, false) {
                    let seq = self.idle_seq();
                    self.set_anim(anim, 18.0, seq, 0);
                    return None;
                }
                if self.stick_mag <= 0.2 || self.timer < 0xb { return None; }
                Some(0x35)
            }
            0x72 | 0x73 => {
                if self.hp < 1 {
                    self.set_state(anim, env, 0x67, true);
                    self.set_anim(anim, 18.0, 0x58, if self.state == 0x72 { 0x25 } else { 0x28 });
                    return None;
                }
                if self.state == 0x73 && anim.view().flags & 2 != 0 { return Some(0x33); }
                if self.timer < 0x33 { return None; }
                // `SetState(n, 0)`: the hurt's sequence carries on.
                let n = if self.state == 0x72 { 0x36 } else { 0x33 };
                self.set_state(anim, env, n, false);
                None
            }
            _ => None,
        }
    }
}

/// The effects pass (after the hero's update, with the item pass): every [`WaterFx`] the hero asked for this tick,
/// spawned into the particle pool in order, the bubbles at his joints through the host (`0x3127c8`). Without a pool
/// the requests are dropped.
pub fn fx_pass(world: &MobyWorld, ctx: &mut Ctx) {
    let fx = std::mem::take(&mut ctx.hero.water.fx);
    if fx.is_empty() { return; }
    let Some(sys) = ctx.parts.as_deref_mut() else { return };
    let rng = &mut ctx.game.rng;
    let hero = ctx.hero_moby.and_then(|i| world.mobys.get(i));
    for f in fx {
        match f {
            WaterFx::Ring { size, pos } => { crate::particles::water::ring(sys, rng, size, 5250.0, pos, true, None); }
            WaterFx::Drop { pos, vel, kind, life } => { crate::particles::water::drop(sys, rng, pos, vel, kind, life); }
            WaterFx::Spray { size, spin, pos, vel } => { crate::particles::water::spray(sys, rng, size, spin, pos, vel, true); }
            WaterFx::Bubble { size, top, at, vel } => {
                let pos = match at {
                    BubbleAt::World(p) => p,
                    BubbleAt::Joint(j, off) => {
                        let Some(p) = hero.and_then(|m| ctx.host.joint(m, j)) else { continue };
                        add(p, off)
                    }
                };
                crate::particles::water::bubble(sys, rng, size, top, pos, vel);
            }
        }
    }
}
