//! The water: swimming on the surface (group 0x12: the stroke 0x35, treading 0x36), under it (group 0x11: the swim
//! 0x32, the drift 0x33, the Clank-driven fast swim 0x34) and the hurts there (0x72 on the surface, 0x73 under) —
//! SetState (L00 `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`), with the prologue's water checks
//! (`0x2c9098` in, `0x2c8f38` out and the dive) and the surface bob `0x2c94b0`.
//!
//! **In and out.** Falling into water deeper than 0.8 with the feet within the vertical speed (+0.07, at least 0.2)
//! of 0.45 below its surface, or sinking 0.8 below it, gives 0x36 (with no health 0x67). Every 16th frame a line
//! from 4 (16 every 63rd) above down to 1.3 above the feet finds a water surface with nothing under it: 0x33. Under
//! water, 0.4 above the surface again: the fall 6. On the surface, ■ within 7 ticks (R1 / R2 with Clank) dives.
//!
//! **Under water.** ✕ pitches up, ■ down (to 78°: the button's pressure, or a ramp of 1/40 a tick from half), the
//! pitch springs there (0.015, 0.2); the yaw follows the stick (0.007, 0.08, 0.087); the speed eases toward 0.05
//! (the dive 0.1, the fast swim 0.047 or 0.117 with R1 / R2) by 1/514 / 1/900 (fast: 1/277 / 1/450); the stroke's
//! speed follows the sequence (`0x25dc90`, × 5); the body rolls into the turn.
//!
//! **On the surface.** The stroke moves at the stick's 0.05 (the stroke curve × 5.5), turning as under water; the
//! height bobs on a spring to the surface (0.027, 0.3) less 0.12, the bob's own offset settling (0.005, 0.045).
//!
//! Not ported: the bubbles and splashes (`0x3378b0`, `0x2aac58`, `0x37d5a0`), the sounds, the swim stats counters,
//! the pressure buttons (the ramps are used: no pressure-mode pad `0x13835c` = 0x79), the vehicle's water.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::hero::anim::AnimCtl;
use crate::pad::button;

/// `0x25dc90`: the stroke's speed by the frame of its sequence (30 frames).
const STROKE: [f32; 30] = [
    0.4, 0.4, 0.45, 0.52, 0.6, 0.68, 0.78, 0.89, 1.0, 1.0, 1.0, 1.0, 0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5,
    0.45, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4,
];
/// R2 | R1 (`0x138320 & 10`): the fast swim's buttons.
const FAST: u32 = button::R1 | button::R2;

/// The water block (`0x18a840..0x18a874`, `0x18a5b0..0x18a5c4`).
#[derive(Clone, Copy, Debug, Default)]
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
        // Under a water surface with nothing below it (every 16th frame).
        if !matches!(self.group, 0x11 | 0x12 | 7) && self.state != 0x12 && self.frame & 0xf == 0 {
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
        if self.group == 0x11 && wz + 0.4 < self.pos[2] { return self.set_state(anim, env, 6, true); }
        if self.group != 0x12 { return false; }
        let a = [self.pos[0], self.pos[1], wz + 0.01];
        let b = [self.pos[0], self.pos[1], wz + 0.31];
        if env.line(a, b, 2).is_none() { return false; }
        let id = if env.clank && env.pad.held & FAST != 0 { 0x34 } else { 0x32 };
        self.set_state(anim, env, id, true)
    }

    /// `0x2c94b0`: the surface bob (module docs).
    fn surface_bob(&mut self) {
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

    pub(super) fn water_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        let from = self.prev_group;
        match id {
            0x32 | 0x34 => {
                self.group = 0x11;
                self.cam_mode = 0xb;
                self.damage.fall_v = 0.0;
                if from == 0x12 { self.substate = 1; }
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
                self.momentum_decay(0.001_166_666_7);
                self.wall_check(env, false);
                self.surface_bob();
            }
            0x72 => {
                self.vel[2] = 0.0;
                let l = len2(self.vel);
                self.vel = set_len2(self.vel, l * 0.92);
                self.wall_check(env, false);
                self.surface_bob();
            }
            0x73 => {
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
        if self.substate == 1 {
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
