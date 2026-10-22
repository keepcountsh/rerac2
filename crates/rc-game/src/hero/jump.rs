//! The jump group (group 4): SetState's shared jump entry (L00 `0x2c4560`, the case of 7, 9, 0xa..0x12, 0x1b, 0x38,
//! 0x45, 0x48, 0x5b, 0x66, 0x6e, 0x81, 0x89, 0x92), the physics (`0x2ba8e0` case 7…, the vertical `0x2b6668`, the air
//! control `0x2b6cd0`) and the transitions (`0x2cb650` case 7…, the landing `0x2cab40`).
//!
//! **The jumps.** 7 the jump, 9 the run jump, 0xb the flip, 0xe the double jump, 0x1b the climb out of a hang,
//! 0x11 the wall jump, 0x38 the lava hop (1 damage; the third in a row, or with no health, kills), 0x81 the bounce
//! pad (classes 0x9b9 / 0xf05 under the feet), 0x6e the jump on a magnetic walkway, 0x66 out of quicksand (group
//! 0x19), 0x12 out of the water, 0x45 / 0x48 the swing and climb jumps, 0x89 the scripted leap; the packs' 10 (the
//! Heli-Pack's long jump), 0xf (its high jump: the rotors lift 49/3600 a tick from tick 35 for 36 ticks), 0x10 (the
//! Thruster-Pack's boost: its target speed 0.19, 0.095 above 1.4) and 0xd (its high jump, 4 high). 0xc, 0x5b and 0x92
//! take only the shared defaults.
//!
//! **Height.** The entry sets a minimum and maximum height, the gravity 0.00825 (descending: 0.009652 for 7), the
//! takeoff tick and the ramp. While ✕ is held (and for the first tick), up to the ramp's end, the height aimed at
//! grows by `(max − min) / ramp` per tick and the missing vertical speed `sqrt(2·h·g) − applied` is added from the
//! takeoff on: the longer ✕ is held, the higher the jump. The double jump instead raises the vertical speed toward
//! 7/60 for 8 ticks, less the higher above the first jump's takeoff (with the Thruster-Pack and no ceiling within
//! 2.7: other limits, ×1.25). A scripted curve adds its table's push in its tick window.
//!
//! **Animation.** One tick after the takeoff the playback is timed so the hold frame comes at the landing ETA (the
//! curve's jumps: its frame at its start); descending, the apex frame is timed to the landing; at touchdown the
//! sequence jumps to the landing frame + 2.
//!
//! **Out of a jump.** R1 within the first 7 ticks: the pack's long jump (10 / 0x10); ✕ held after the apex: the
//! glide 8; ✕ against an armed wall (`wall.rs`): the wall jump 0x11; a ledge: 0x17; a rail (0x27) or a pole (0x71);
//! hitting a wall in a pack jump: 0x77.
//!
//! Not ported: the ice's air control (`0x2b96a0`), the splash and sounds, the landing
//! effects and camera jolt, the body lean `0x2b7c88`, the vehicle (`0x18c0d4` = 1) and the scripted timers
//! `0x18a012` (→ 0x82) / `0x18a032` (the state refused).

use super::physics::*;
use super::states::{Entry, FLIP_DIRS};
use super::Hero;
use crate::hero::anim::AnimCtl;
use crate::pad::button;

/// `$gp − 0x7210`: the run jump's sequence by the run's frame (the leading foot).
const RUN_JUMP_SEQ: [u8; 24] = [9, 9, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9];
/// `0x1a7db4` (0.82): the target speed above which a re-jump can become the run jump. The target is the one the flip
/// test `0x2c3cc0` left this tick (`0x2b3af8(1, 0)`: the stick's magnitude), so a re-jump at nearly full stick toward
/// the facing is the run jump.
pub(super) const RUN_JUMP_SPEED: f32 = 0.82;
/// `0x25dc30`: the water jump's curve.
const WATER_CURVE: [(f32, f32, i32); 8] = [(-27.0, 0.0, 9), (40.0, 0.0, 1), (55.0, 0.0, 1), (75.0, 0.0, 1), (90.5, 0.0, 9), (75.0, 0.0, 4), (40.0, 0.0, 3), (0.0, 0.0, 100)];
/// `$gp − 0x71f0`: the Heli-Pack high jump's curve.
const HELI_CURVE: [(f32, f32, i32); 1] = [(49.0, 0.0, 1000)];
/// The pad classes (`0x2c4560`: a jump from their top is the bounce 0x81).
const BOUNCE_PADS: [i16; 2] = [0x9b9, 0xf05];

impl Hero {
    pub(super) fn jump_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        // From a bounce pad's top every jump is the bounce; on a magnetic walkway the magnetic jump.
        if id != 0x81 && BOUNCE_PADS.contains(&self.ground_class) { return Entry::Return(self.set_state(anim, env, 0x81, true)); }
        if id != 0x6e && self.surf.boots == 1 && id != 0xe && id != 0x38 { return Entry::Return(self.set_state(anim, env, 0x6e, true)); }
        if self.prev_state == 0x11 && !self.wall.high { self.wall.count = self.wall.count.saturating_add(1) } else { self.wall.count = 0 }
        // An updraft under him: the float.
        if self.gear.updraft != 0 { return Entry::Return(self.set_state(anim, env, 0x82, true)); }
        self.group = 4;
        self.cam_mode = if matches!(id, 0x28 | 0x29) { 3 } else { 0x50 };
        self.vel = self.eff;
        let height = self.height;
        let wall_lock = self.wall.lock;
        self.wall.lock = 0;
        let j = &mut self.jump;
        // The block's fields the entry does not write keep their values (the takeoff position: the double jump
        // measures from the first jump's).
        *j = super::JumpBlock {
            g: 0.008_25,
            acc: 0.005_555_556,
            dec: 0.003_055_555_7,
            keep_speed: (self.surf.ice) as u8,
            max_air: 0x50,
            f34a: 0x46,
            air_turn: 0.250_011_4,
            terminal: 0.833_333_3,
            ref_z: self.pos[2],
            fallover_h: 2.5,
            f334: 0.5,
            air_speed: 0.099_166_67,
            curve_row: -1,
            takeoff_pos: j.takeoff_pos,
            land_eta: j.land_eta,
            climb_push: j.climb_push,
            wall_dir: j.wall_dir,
            wall_yaw: j.wall_yaw,
            ..Default::default()
        };
        // Most jumps: takeoff 5, the capsule's bottom 0.6, the hold frame 18 (`0x18a2cc`, `0x18a2d8`, `0x18a2e8`).
        let std = |j: &mut super::JumpBlock| (j.takeoff, j.bottom, j.f_hold) = (5, 0.6, 18.0);
        match id {
            7 | 9 => {
                if id == 7 {
                    j.g_down = 0.009_652_62;
                    (j.f_apex, j.f_land, j.hmax, j.hmin) = (30.0, 12.0, 2.3, 1.28);
                } else {
                    (j.f_apex, j.f_land, j.hmax, j.hmin) = (29.0, 11.0, 2.7, 1.0);
                }
                (j.can_double, j.ramp, j.f34c, j.f34e) = (1, 0xf, 1, 1);
                std(j);
            }
            0x81 => {
                let [h, g] = self.bounce;
                std(j);
                (j.hmin, j.hmax, j.g, j.f_apex, j.f_land, j.ramp) = (h, h + 0.5, g / 3600.0, 30.0, 12.0, 0xf);
                (j.max_air, j.fallover_h, j.f34a, j.air_speed, j.f346) = (400, 300.0, 0x4b0, 0.116_666_7, 0x4b0);
            }
            0x12 => {
                let shallow = self.surf.depth < 1.25;
                (j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land) = (2, 0.6, 18.0, 28.0, 14.0);
                (j.hmax, j.hmin, j.ramp) = (0.2, 0.1, 10);
                if shallow {
                    (j.hmin, j.hmax, j.takeoff) = (1.95, 1.98, 0xf);
                } else {
                    self.vel[2] = 0.0;
                    let j = &mut self.jump;
                    (j.curve, j.curve_start, j.curve_end, j.curve_frame, j.curve_last, j.curve_hold) = (&WATER_CURVE, 2, 0x25, 0.0, 0x28, 18.0);
                }
                let j = &mut self.jump;
                j.curve_on = !shallow;
                j.no_double = 1;
            }
            0x38 => {
                std(j);
                (j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp, j.max_air, j.no_double) = (26.0, 13.0, 5.55, 5.5, 2, 0x78, 1);
                self.take_damage(1);
                if self.prev_state == 0x38 || self.prev2_state == 0x38 || self.hp == 0 {
                    self.hot_hops += 1;
                    if 2 < self.hot_hops || self.hp == 0 {
                        // The third hop in a row, or no health left: the death (0x79 for a body that can burn; the
                        // electric 0x7d on level 2 is not ported).
                        return Entry::Return(self.set_state(anim, env, 0x39, true));
                    }
                } else {
                    self.hot_hops = 0;
                }
                let j = &mut self.jump;
                (j.f34a, j.f33e, j.g, j.f33c) = (0x96, 4, 0.004_166_667, 5);
            }
            0x66 => {
                self.group = 0x19;
                let up = (self.surf.sand_z - self.pos[2]).max(0.0);
                let j = &mut self.jump;
                std(j);
                (j.hmin, j.hmax, j.no_double, j.f_apex, j.f_land, j.ramp) = (up + 1.7, up + 1.75, 1, 30.0, 12.0, 0x13);
            }
            0x28 | 0x29 => {
                // The grind's jumps (group 0xf): 0x28 off the rail (higher with the rail's boost `0x18b2b8`), 0x29 the
                // hop between rails.
                if id == 0x28 {
                    (j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp, j.g) = (5, 0.8, 18.0, 28.0, 14.0, 2.45, 2.4, 0x11, 0.007_5);
                } else {
                    (j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp, j.g) = (2, 0.8, 18.0, 31.0, 14.0, 2.55, 2.5, 10, 0.006_666_667);
                }
                self.group = 0xf;
            }
            0x6e => {
                std(j);
                (j.air_speed, j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp, j.no_double, j.f34c) = (0.116_024_04, 30.0, 12.0, 2.75, 2.7, 5, 1, 1);
            }
            0x45 => {
                (j.takeoff, j.air_speed, j.bottom, j.f_hold, j.f_apex, j.f_land) = (3, 0.033_333_335, 0.6, 15.0, 24.0, 12.0);
                (j.hmax, j.hmin, j.ramp, j.g, j.dec) = (1.35, 1.3, 0xe, 0.006_944_444_5, 0.013_888_889);
            }
            10 => {
                std(j);
                (j.f346, j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp, j.g) = (0x46, 29.0, 10.0, 1.95, 1.9, 0xe, 0.003_055_555_7);
                (j.f34e, j.max_air, j.f34a) = (1, 0x78, 0x5a);
            }
            0x10 => {
                (j.f_hold, j.f_apex, j.f_land, j.takeoff, j.bottom, j.hmin, j.hmax, j.ramp) = (18.0, 30.0, 11.0, 5, 0.8, 0.4, 0.9, 1);
                (j.f34e, j.g, j.dec, j.acc) = (1, 0.002_361_111, 0.013_888_889, 0.012_222_223);
                j.thrust = if 1.4 < height { 0.095 } else { 0.191_666_67 };
            }
            0x1b => {
                // The climb out of a hang: no gravity for 9 ticks, then up to 2 high, pushed forward by `0x18a2dc`.
                (j.f34e, j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land) = (1, 10, 0.85, 20.0, 29.0, 19.0);
                (j.hmax, j.hmin, j.f334, j.g, j.ramp, j.no_double) = (2.05, 2.0, 1.0, 0.006_944_444_5, 1, 1);
                j.climb_push = 0.0;
                self.items.no_use = 1;
            }
            0x48 => {
                (j.takeoff, j.g, j.bottom, j.f_hold, j.f_land, j.hmax, j.hmin, j.ramp, j.f_apex) = (0xd, 0.007_777_778, 0.85, 8.0, 14.0, 1.59, 1.55, 1, 14.0);
                j.climb_push = 0.0;
            }
            0x11 => {
                (j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land, j.ramp) = (9, 0.6, 12.0, 20.0, 10.0, 0xc);
                if self.wall.high { (j.hmax, j.hmin, j.wall_speed) = (3.35, 3.3, 0.081_666_67) } else { (j.hmax, j.hmin, j.wall_speed) = (1.85, 1.8, 0.028_333_334) }
                (j.no_double, j.f34a, j.g, j.f34c) = (1, 0x50, 0.008_055_556, 1);
                // Off the wall along its normal.
                (j.wall_dir, j.wall_yaw) = (self.wall.normal, self.wall.yaw);
                self.wall.lock = 1;
            }
            0xd => {
                (j.f_hold, j.f_apex, j.f_land, j.hmax, j.takeoff, j.bottom, j.hmin, j.ramp) = (26.0, 38.0, 18.0, 4.07, 9, 0.6, 4.0, 0xe);
                (j.fallover_h, j.max_air, j.air_speed, j.f346, j.f34e) = (4.5, 0x78, 0.031_666_67, 0x25, 1);
            }
            0xf => {
                (j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land, j.hmax, j.hmin, j.ramp) = (9, 0.6, 50.0, 64.0, 28.0, 1.7, 1.65, 5);
                (j.fallover_h, j.max_air, j.air_speed, j.f346, j.curve_on) = (4.5, 0x82, 0.055, 0x55, true);
                self.ledge.cooldown = 0x46;
                let j = &mut self.jump;
                // `0x1a7b98` (the Heli-Pack's upgrade) starts the lift a tick earlier: not ported.
                (j.curve, j.curve_start) = (&HELI_CURVE, 0x23);
                (j.curve_end, j.f34e, j.curve_frame, j.curve_hold, j.curve_last) = (0x23 + 0x24, 1, 29.0, 50.0, 0x56);
            }
            0xe => {
                // Lower the higher he already is: 0.7, plus half of what is missing to 2.5 below a height of 2.
                let h = if height < 2.0 { (2.5 - height) * 0.5 + 0.7 } else { 0.7 };
                (j.hmin, j.hmax, j.f346, j.takeoff, j.bottom) = (h, h + 0.05, 0x1e, -1, 0.6);
                (j.f_hold, j.f_apex, j.f_land, j.ramp, j.f334, j.air_speed) = (32.0, 43.0, 19.0, 0xe, 1.0, 0.079_166_67);
            }
            0xb => {
                (j.f34a, j.takeoff, j.bottom, j.f_hold, j.f_apex, j.f_land) = (0x32, 5, 0.6, 18.0, 31.0, 24.0);
                (j.hmax, j.hmin, j.ramp, j.fallover_h, j.f334, j.no_double) = (3.25, 3.2, 0xe, 4.5, 1.0, 1);
            }
            _ => {}
        }
        let _ = wall_lock;
        self.jump.h = self.jump.hmin;
        if id == 0xb {
            self.jump.flip_yaw = self.yaw_ago(10);
            self.jump.flip_fwd = self.rows[0];
            self.jump.flip_dir = self.flip_direction(env);
            let j = &mut self.jump;
            (j.flip_target_yaw, j.flip_stick_dir) = (self.target_yaw, self.stick_dir);
            j.flip_speed = if 0.025 < self.eff_len_xy { self.eff_len_xy } else { 0.0 };
            (j.flip_push, j.f34c, j.f34e, j.flip_strafe) = (0.0, 1, 1, false);
            if self.strafe != 0 && 0.3 < self.stick_mag {
                (j.flip_target_yaw, j.flip_stick_dir, j.flip_strafe, j.flip_push) = (self.strafe_yaw, self.strafe_dir, true, 1.0 / 12.0);
            }
        }
        let j = &mut self.jump;
        if j.fallover_h == 0.0 { j.fallover_h = j.h * 1.5; }
        if !play { return Entry::Done; }
        match id {
            7 | 0x6e | 0x81 | 0x66 => Entry::Anim(5.0, 7, 0),
            0x12 => {
                self.set_anim(anim, -1.0, 0x6b, 1);
                Entry::Done
            }
            0x38 => Entry::Anim(4.0, 0x43, 0),
            0x28 => {
                self.set_anim(anim, 5.0, 0x4d, 4);
                Entry::Done
            }
            0x29 => {
                // The hop's side by the rail ahead (`0x18a3f0` − `0x18a430`): not ported, the left one.
                Entry::Anim(5.0, 0x1e, 0)
            }
            9 => {
                let v = anim.view();
                Entry::Anim(5.0, if v.seq_a == 4 { RUN_JUMP_SEQ.get(v.frame as usize).copied().unwrap_or(8) } else { 8 }, 0)
            }
            0x45 => {
                self.set_anim(anim, 5.0, 7, 5);
                Entry::Done
            }
            10 => Entry::Anim(5.0, 0x12, 0),
            0x10 => {
                if self.prev_group == 4 || (self.prev_state == 4 && self.prev_timer < 7) {
                    self.set_anim(anim, 8.0, 0x26, 0);
                    self.jump.takeoff += 3;
                    Entry::Done
                } else {
                    Entry::Anim(5.0, 0x26, 0)
                }
            }
            0x1b => Entry::Anim(4.0, 0x22, 0),
            0x89 => {
                self.set_anim(anim, 9.0, 0x7d, 0xb);
                Entry::Done
            }
            0x48 => Entry::Anim(4.0, 9, 0),
            0x11 => {
                self.set_anim(anim, 7.0, 0x23, 2);
                Entry::Done
            }
            0xd => Entry::Anim(5.0, 0x11, 0),
            0xe => {
                self.set_anim(anim, 7.0, 0x16, 3);
                self.anim_speed = 0.8;
                Entry::Done
            }
            0xf => Entry::Anim(5.0, 0x15, 0),
            0xb => {
                // The flip's sequence by its direction (0x1c left, 0x1d right, 0x1e ahead, 0x1f back; the option
                // `0x1a7a4d` swaps left and right: not ported); chained from a jump it starts at frame 1, takeoff 1.
                let seq = 0x1c + self.jump.flip_dir as u8;
                if self.prev_group == 4 {
                    self.set_anim(anim, 4.0, seq, 1);
                    self.jump.takeoff = 1;
                    Entry::Done
                } else {
                    Entry::Anim(5.0, seq, 0)
                }
            }
            _ => Entry::Done,
        }
    }

    pub(super) fn phys_jump(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        if self.jump.keep_speed == 0 || !self.surf.ice {
            if self.jump.landed != 0 { self.stick_target(env, 0.099_166_67, 0); }
            self.stick_target(env, self.jump.air_speed, 0);
        } else {
            self.speed = self.target_speed;
        }
        if self.timer == self.jump.takeoff { self.jump.takeoff_pos = self.pos; }
        let early = matches!(self.state, 0xb | 0x10 | 0xc | 0x11 | 0x1b | 0x89 | 0x48);
        if self.jump.landed == 0 && (self.jump.takeoff <= self.timer || early) {
            self.jump_air(env, anim);
            if self.state == 0x12 && 0.061_666_67 < len(self.vel) { self.vel = set_len(self.vel, 0.061_666_67); }
        } else if self.jump.keep_speed == 0 {
            self.turn_to(0.015, 0.2, 0.250_011_4, 0);
            let s = self.speed;
            if 1 < self.jump.landed { self.speed *= 0.4; }
            self.set_planar_vel(f32::MAX);
            self.speed = s;
            if self.jump.landed != 1 { self.speed_step(self.jump.acc, self.jump.dec); }
            if self.jump.landed != 0 { self.momentum_decay(self.jump.dec); }
            self.drag(0.5, 1.0 / 30.0);
            // `0x2d3b38(k, v, v)`: v + the facing · k — the run jump's and the Heli-Pack jump's windups push forward.
            if self.state == 9 && self.timer < self.jump.takeoff + 4 { self.vel = self.along_facing(self.vel, self.timer as f32 * 0.004_444_444_6); }
            if self.state == 10 && self.timer < 0x22 { self.vel = self.along_facing(self.vel, self.timer as f32 * 0.014_722_222); }
            // Blocked ahead in the first 10 ticks: a line (flags 4) from 0.5 up to 0.95 ahead.
            if self.timer < 10 && 0.2 < self.stick_mag {
                let a = add(self.pos, [0.0, 0.0, 0.5]);
                if env.line(a, self.along_facing(a, 0.95), 4).is_some() { self.jump.blocked = 1; }
            }
        } else {
            // On ice (`0x2b96a0`): the slide keeps the speed; the Heli-Pack windup's push.
            if self.state == 10 && self.timer < 10 && self.fwd_speed < 0.116_666_675 {
                let (s, c) = self.rot[2].sin_cos();
                self.vel[0] += c * 0.011_666_667;
                self.vel[1] += s * 0.011_666_667;
                self.eff_h = self.vel;
            }
        }
        if self.jump.ref_z < self.ground_z {
            self.jump.ref_z = (self.jump.ref_z + 0.1).min(self.ground_z);
        }
        if self.pos[2] < self.jump.ref_z { self.jump.ref_z = self.pos[2]; }
        let j = self.jump;
        if j.takeoff < self.timer && self.vel[2] < 0.001 && (!j.curve_on || j.curve_end < self.timer) && (self.state != 0xe || 0x11 < self.timer) {
            self.jump.descending = 1;
            if self.jump.g_down != 0.0 { self.jump.g = self.jump.g_down; }
        }
        // The Thruster-Pack's boost rams what is ahead (8 ticks in, before frame 28): a 0.9 sphere 0.8 ahead, 0.3 up
        // (`0x2e14b0`), 1 damage with flags 0x10000 along the displacement × 7 (`0x31d4c8`).
        if self.state == 0x10 && 8 < self.timer && self.anim_frame < 28.0 {
            let c = self.local_point(0.8, 0.0, 0.3);
            self.hits_out.push(super::HeroHit { centre: c, r: 0.9, damage: 1.0, flags: 0x10000, dir: scale(self.disp, 7.0), joints: None });
        }
        self.jump_vertical(env);
        self.wall_probe(env);
    }

    /// `0x2b6cd0`: the jumps' air control by state.
    fn jump_air(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            10 => {
                self.drag(0.7, 0.0);
                self.momentum = self.drag;
                self.speed = self.eff_len_xy;
            }
            0x10 => {
                // The boost: after 54 ticks the gravity eases to 1/144 and the target to 0.0583.
                let mut t = self.jump.thrust;
                if 0x36 < self.timer {
                    approach(0.006_944_444_5, 0.001_944_444_5, &mut self.jump.g);
                    t = 0.058_333_337;
                }
                self.target_speed = t;
                self.speed_step(0.012_222_223, 0.012_5);
                self.vel[0] = 0.0;
                self.vel[1] = 0.0;
                self.vel = self.along_facing(self.vel, self.speed);
            }
            0x1b => self.climb_air(anim),
            0x48 => {
                let f = anim.view().frame;
                if (5.0..=18.0).contains(&f) {
                    let t = if f <= 15.0 { 0.020_833_334 } else { 0.0 };
                    approach(t, 0.002_222_222_3, &mut self.jump.climb_push);
                }
                let up = self.vel[2];
                let p = self.jump.climb_push;
                self.vel = [self.rows[0][0] * p, self.rows[0][1] * p, up];
            }
            0xb => self.flip_air(),
            0x11 => {
                // Off the wall: the facing turns to the wall's normal after 9 ticks; from 9 ticks on the velocity is
                // 0.0817 along it (on a magnetic walkway along the normal itself).
                self.target_yaw = self.jump.wall_yaw;
                self.stick_dir = self.jump.wall_dir;
                if 9 < self.timer { self.turn_to(0.042, 0.2, 0.197_809_66, -1); }
                if 8 < self.timer {
                    if self.surf.boots == 0 {
                        let (s, c) = self.target_yaw.sin_cos();
                        self.vel[0] = c * 0.081_666_67;
                        self.vel[1] = s * 0.081_666_67;
                    } else {
                        self.vel = add([0.0, 0.0, self.vel[2]], scale(self.stick_dir, 0.081_666_67));
                    }
                }
                self.speed = self.eff_len_xy;
            }
            0x81 => {
                self.turn_to(0.04, 0.2, self.jump.air_turn, 0);
                self.speed = 0.0;
            }
            _ => self.air_control(env),
        }
    }

    /// `0x2b6668`: before the takeoff a small push down (1/75); landed, 0.015 down; in the air, gravity with the fall
    /// speed capped at `0x18a2c0` (the climb's first 9 ticks, the leap 0x89's 11 and the climb jump 0x48's 12 keep
    /// the vertical speed). Then the height ramp (module docs) and the scripted curve.
    pub(super) fn jump_vertical(&mut self, env: &Env) {
        let j = self.jump;
        let keep = (self.state == 0x1b && self.timer <= 9) || (self.state == 0x89 && self.timer <= 0xb) || (self.state == 0x48 && self.timer <= 0xc);
        if !keep {
            if self.timer < j.takeoff {
                self.vel[2] = -0.013_333_334;
            } else if j.landed != 0 {
                self.vel[2] = -0.015;
            } else {
                self.vel[2] = (self.vel[2] - j.g).max(-j.terminal);
            }
        }
        if self.state == 0xe {
            self.double_jump_rise(env);
            return;
        }
        let first = j.pending == 0.0 && j.applied == 0.0;
        if (env.pad.held & button::CROSS != 0 || first || self.state == 0x12) && j.h < j.hmax && self.timer <= j.ramp {
            let h = (j.h + (j.hmax - j.hmin) / j.ramp as f32).min(j.hmax);
            self.jump.h = h;
            let v = ((h + h) * j.g).sqrt();
            self.jump.pending = v - self.jump.applied;
        }
        if j.takeoff <= self.timer && 0.0 < self.jump.pending {
            self.vel[2] += self.jump.pending;
            self.jump.applied += self.jump.pending;
            self.jump.pending = 0.0;
        }
        if !j.curve_on || self.timer < j.curve_start || j.curve_end <= self.timer || j.curve.is_empty() { return; }
        // The curve: the next row when this one's ticks are done (a base not −999999 restarts the push), then the
        // row's step.
        let jb = &mut self.jump;
        jb.curve_tick += 1;
        let rows = jb.curve;
        let row_done = |jb: &super::JumpBlock| jb.curve_row < 0 || rows.get(jb.curve_row as usize).map_or(true, |r| r.2 <= jb.curve_tick);
        if row_done(jb) {
            loop {
                jb.curve_row += 1;
                jb.curve_tick = 0;
                let Some(&(base, _, n)) = rows.get(jb.curve_row as usize) else { jb.curve_row = rows.len() as i32 - 1; break };
                if base != -999_999.0 { jb.curve_v = base / 3600.0; }
                if 1 <= n { break; }
            }
        }
        if 0 < jb.curve_tick {
            if let Some(r) = rows.get(jb.curve_row as usize) { jb.curve_v += r.1 / 3600.0; }
        }
        self.vel[2] += self.jump.curve_v;
    }

    /// `0x2b6668`'s double jump: for the first 8 ticks the vertical speed rises toward 7/60 by 1/24 a tick, ×0.7 more
    /// than 1.7 above the first jump's takeoff and ×0.45 more than 2.1 above it. With the Thruster-Pack and no ceiling
    /// between 1 and 2.7 above the feet: ×0.7 above 2.2, ×0.47 above 2.8, and ×1.25.
    fn double_jump_rise(&mut self, env: &Env) {
        let mut k = 0.116_666_675;
        let thruster = env.clank && env.pack == 3;
        let mut clear = true;
        if thruster {
            let a = add(self.pos, [0.0, 0.0, 1.0]);
            let b = add(self.pos, [0.0, 0.0, 2.7]);
            clear = env.line(a, b, 2).is_none();
        }
        let d = self.pos[2] - self.jump.takeoff_pos[2];
        let (lo, hi, khi) = if thruster && clear { (2.2, 2.8, 0.47) } else { (1.7, 2.1, 0.45) };
        if hi < d {
            k *= khi;
        } else if lo < d {
            k *= 0.7;
        }
        if thruster && clear { k *= 1.25; }
        if self.timer < 8 {
            let mut v = self.vel[2];
            if v < k { approach(k, 0.041_666_668, &mut v); }
            self.vel[2] = v;
        }
    }

    /// `0x2b6cd0`'s climb 0x1b: after 0x28 ticks the stick steers (0.04, 0.2, 0.25 and 1/180); the forward push
    /// `0x18a2dc` grows to 1/30 by 1/450 from frame 13 to 27 and falls back to 0 until 30; the velocity is the facing
    /// · push with the vertical speed kept.
    fn climb_air(&mut self, anim: &mut dyn AnimCtl) {
        if 0x28 < self.timer {
            self.turn_to(0.04, 0.2, 0.250_011_4, 0);
            self.air_accel(0.005_555_556);
        }
        let f = anim.view().frame;
        if (13.0..=30.0).contains(&f) {
            let t = if f <= 27.0 { 0.033_333_335 } else { 0.0 };
            approach(t, 0.002_222_222_3, &mut self.jump.climb_push);
        }
        let up = self.vel[2];
        let p = self.jump.climb_push;
        self.vel = [self.rows[0][0] * p, self.rows[0][1] * p, up];
    }

    /// `0x2b6cd0`'s flip: the facing turns toward the yaw of 10 ticks before (0.04, 0.2, at most 0.1164 a tick); the
    /// push along the stick's yaw grows toward 0.07 by 1/450 (toward 0 once descending within 11 ticks of the
    /// ground); the ground speed carried goes along the old facing, scaled by the sine of the angle between it and the
    /// stick (the back flip carries none); then the drag (0.7) as momentum. (Strafing and the camera-relative mode
    /// `0x18c0e3`: not ported.)
    fn flip_air(&mut self) {
        let j = self.jump;
        self.target_yaw = j.flip_yaw;
        self.stick_dir = j.flip_fwd;
        self.turn_to(0.04, 0.2, 0.116_355_3, 0);
        self.speed = if j.flip_dir == 3 { 0.0 } else { j.flip_speed };
        if j.flip_strafe {
            approach((self.stick_mag / 12.0).max(1.0 / 30.0), 0.010_277_778, &mut self.jump.flip_push);
            self.speed = self.eff_len_xy;
        } else {
            let t = if j.descending == 0 || 0xb < j.land_eta { 0.07 } else { 0.0 };
            approach(t, 0.002_222_222_3, &mut self.jump.flip_push);
        }
        let (s, c) = j.flip_target_yaw.sin_cos();
        self.vel[0] = c * self.jump.flip_push;
        self.vel[1] = s * self.jump.flip_push;
        if !j.flip_strafe {
            let carry = j.flip_speed * rot_dist(j.flip_yaw, j.flip_target_yaw).sin();
            let (s, c) = j.flip_yaw.sin_cos();
            self.vel[0] += c * carry;
            self.vel[1] += s * carry;
        }
        self.drag(0.7, 0.0);
        self.momentum = self.drag;
    }

    /// `0x2cb650` case 7…: the flip chained on landing, the pack jumps from a jump, the rails, the fall-over when
    /// airborne too long, the wall bonk, the animation timing, the touchdown, the re-jump and the landing, the double
    /// jump, the glide, the wall jump, the ledge.
    pub(super) fn trans_jump(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        // The flip chained from a jump: the stick not ahead, in the first 12 ticks or once landed, on the ground; once
        // landed ✕ within 7 (the flip itself only once landed); a direction press within 5 with R1 held.
        if self.jump.f34e != 0 {
            let gap = self.flip_gap(env);
            let j = self.jump;
            if 0 < gap && (self.timer < gap || j.landed != 0) && self.grounded_ticks != 0 && (self.state != 0xb || Self::cross_within(env, 7)) {
                let mut ok = true;
                if self.state == 0x1b || j.landed != 0 { ok = Self::cross_within(env, 7); }
                if self.state == 0xb && j.landed == 0 { ok = false; }
                if ok && env.pad.pressed_within(FLIP_DIRS, 5).is_some() && env.pad.held & button::R1 != 0 { return Some(0xb); }
            }
        }
        if self.gear.updraft != 0 && matches!(self.state, 7 | 9) && self.timer <= 0x13 { return Some(0x82); }
        // The pack's long jump: R1 within 2 ticks in the first 7.
        if self.timer < 7 && env.pad.pressed_within(button::R1, 2).is_some() && !matches!(self.state, 0x10 | 10 | 0x12 | 0x38 | 0x11 | 0xe | 0x66) && env.clank {
            if env.pack == 2 && 0.015 < self.fwd_speed {
                self.set_state(anim, env, 10, true);
            } else if env.pack == 3 && 0.015 < self.fwd_speed {
                self.set_state(anim, env, 0x10, true);
            }
        }
        // A rail (`0x18a45c`, the grind 0x27) or a pole (`0x18a50c`, 0x71) in reach.
        if self.grind.found { return Some(0x27); }
        if self.grind.pole.is_some() { return Some(0x71); }
        let j = self.jump;
        if j.max_air < self.timer {
            if j.landed == 0 { return Some(6); }
        } else if j.landed == 0 && matches!(self.state, 10 | 0x10) && self.ahead.dist < 0.9 && self.cap_hit != 0 && 0.116_666_675 < len(self.vel) {
            // A pack jump into a wall: the bonk 0x77.
            if len(self.disp) < len(self.vel) * 0.5 && !self.ledge.found && self.height < 2.0 { return Some(0x77); }
        }
        // The double jump's first 35 frames cannot touch down.
        let early_double = self.state == 0xe && anim.view().frame < 35.0;
        if self.timer <= j.takeoff { return None; }
        if self.timer == j.takeoff + 1 {
            let v = anim.view();
            if j.curve_on {
                let ticks = (j.curve_start - self.timer) as f32;
                if 0.0 < ticks { self.anim_speed_for_ticks(&v, j.curve_frame, ticks, j.f334, -1.0); }
            } else if self.state != 0xe {
                let eta = self.land_eta(j.g);
                self.anim_speed_for_ticks(&v, j.f_hold, eta, j.f334, -1.0);
                self.anim_speed = self.anim_speed.min(1.5);
            }
            return None;
        }
        if j.curve_on && j.curve_start <= self.timer && self.timer < j.curve_end {
            let v = anim.view();
            let ticks = (j.curve_last - j.curve_start) as f32;
            if 0.0 < ticks { self.anim_speed_for_ticks(&v, j.f_hold, ticks, j.f334, -1.0); }
            return None;
        }
        // Into water this tick (`0x18a158` = 1): the splash's sound 12 (its splash `0x2aac58` is not ported).
        if self.water.under == 1 { self.sounds.push((12, 0)); }
        let v = anim.view();
        if j.landed == 0 && self.air_ticks == 0 && j.descending != 0 && !early_double {
            self.jump.landed = 1;
            self.wall.count = 0;
            if self.stick_mag < 0.2 && 0.041_666_668 < self.speed { self.speed = 0.041_666_668; }
            if v.frame < j.f_apex - 1.5 { self.set_anim(anim, 7.0, v.seq_b, j.f_land as i32 + 2); }
        }
        if self.jump.landed == 0 {
            if j.descending != 0 {
                if (v.frame_b as f32) < j.f_land {
                    if self.state != 0xe || 37.0 <= v.frame {
                        let eta = self.land_eta(j.g).min(60.0);
                        self.jump.land_eta = eta as i16;
                        self.anim_speed_for_ticks(&v, j.f_apex, eta, j.f334, 0.2);
                    } else {
                        approach(1.0, 0.15, &mut self.anim_speed);
                    }
                    if !early_double && 1.8 < self.anim_speed { self.set_anim(anim, self.jump.land_eta as f32 + 3.0, v.seq_b, j.f_land as i32 + 2); }
                    self.anim_speed = self.anim_speed.clamp(0.2, 2.7);
                }
                // A slow run jump turns into the jump 7's landing.
                if self.state == 9 && 6.0 < j.f_apex - v.frame && matches!(v.seq_a, 8 | 9) && v.seq_a == v.seq_b
                    && self.target_speed < RUN_SPEED * 0.001_666_666_8 && self.eff_len_xy < 0.041_666_668
                {
                    let blend = (9.0 / self.anim_speed.max(1e-6)).min(10.0);
                    self.set_anim(anim, blend, 7, (v.frame.round() as i32 + 4) / 2);
                    self.state = 7;
                }
                if v.frame.round() == j.f_apex.round() { self.anim_speed = 0.0; }
            }
        } else {
            self.anim_speed = 1.0;
        }
        if self.jump.landed != 0 {
            if self.state == 10 {
                if self.set_state(anim, env, 3, false) {
                    self.set_anim(anim, -2.0, 5, 0);
                    self.momentum = scale(self.momentum, 0.8);
                }
                return None;
            }
            if !self.surf.mud && self.land_lag == 0 && Self::cross_within(env, 8) && self.air_ticks < 4 {
                let mut n = 7;
                if RUN_JUMP_SPEED < self.target_speed
                    && rot_dist(self.target_yaw, self.rot[2]) < std::f32::consts::FRAC_PI_3
                    && RUN_SPEED * 0.014_166_668 < self.eff_len_xy
                {
                    n = 9;
                }
                return Some(n);
            }
        }
        if self.jump.landed < 2 || self.state == 10 {
            if j.descending != 0 && j.f34a <= self.timer && j.fallover_h < self.height { self.set_state(anim, env, 6, true); }
        } else {
            self.land(env, anim);
        }
        if self.group != 4 && self.group != 0xf && self.group != 0x19 { return None; }
        if self.state == 0x10 && 0x3c < self.timer && 1.5 < self.height { return Some(6); }
        // The wall jump: armed (`wall.rs`), in the air, from 7 / 9 / 0x11 (0x11 after 26 ticks), ✕ pressed since the
        // jump began, within 7.
        let from = matches!(self.state, 7 | 9 | 0x11);
        if self.wall.timer != 0 && self.jump.landed == 0 && from && !(self.state == 0x11 && self.timer < 0x1a) {
            if let Some(ago) = env.pad.pressed_within(button::CROSS, 7) {
                if ago < self.timer { return Some(0x11); }
            }
        }
        if matches!(self.state, 10 | 0x10) && 0x19 < self.timer && self.on_moby { self.jump.f346 = 0x14; }
        let mut glide_after = if self.state == 0x10 { 8 } else { 0x14 };
        if self.state == 0xe && 4.0 < self.height {
            if self.jump.descending == 0 && self.anim_speed < 1.3 { approach(1.3, 0.1, &mut self.anim_speed); }
            glide_after = 0x11;
            self.jump.f346 = 0x11;
        }
        let window = (self.timer - 4).clamp(2, 0x1e);
        let j = self.jump;
        if j.no_double == 0 && j.landed == 0 {
            let pressed = Self::cross_within(env, window);
            if pressed || matches!(self.state, 0xf | 0xd) {
                // The double jump: ✕ pressed since the jump began (4 ticks in, 2..30), after 14 ticks, from a jump that
                // allows it (not in mud below 0.4): on the way up, or near the top (more than 0.7 up, the vertical speed
                // under 0.0967).
                let mud_ok = !self.surf.mud || 0.4 <= self.height;
                if pressed && self.state != 0xe && mud_ok && j.can_double != 0 && 0xe < self.timer {
                    if j.descending == 0 || (0.7 < self.height && self.vel[2].abs() < 0.096_666_67) { return Some(0xe); }
                }
            }
            // The glide: Clank on the back (any pack), ✕ pressed again (from the double jump: still held) and held, high
            // enough (0.6; 1 from 7 / 9; 1.5 from the double jump).
            if (pressed || matches!(self.state, 0xe | 0xf | 0xd)) && env.clank && env.pack != 4 && env.pad.held & button::CROSS != 0 && glide_after < self.timer && j.f346 < self.timer {
                let min_h = match self.state { 7 | 9 => 1.0, 0xe => 1.5, _ => 0.6 };
                if min_h < self.height { return Some(8); }
            }
        }
        // A ledge in reach (`0x18a398`) is grabbed, except from the flip.
        if self.ledge.found && self.state != 0xb { return Some(0x17); }
        None
    }

    /// `0x2cab40`: the landing — crouch 4 with R1, the stop 3 with speed (after 3 landed ticks) or idle 0 when the
    /// landing sequence ends (13 ticks or the wrap), else the walk 2 with the landing sequence 4.
    fn land(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        self.stick_target(env, 1.0, 0);
        if env.pad.held & button::R1 != 0 {
            self.set_state(anim, env, 4, false);
            self.set_anim(anim, -2.0, 0xd, 2);
            anim.set_loop(3, 0x12);
            return;
        }
        if self.stick_mag <= 0.3 || self.land_lag != 0 {
            if 0.05 < self.fwd_speed && 3 < self.jump.landed {
                if self.set_state(anim, env, 3, false) {
                    self.set_anim(anim, -1.0, 5, 0);
                    if 1.0 / 12.0 < len(self.momentum) { self.momentum = set_len(self.momentum, 1.0 / 12.0); }
                }
                return;
            }
            let wrapped = anim.view().flags & 2 != 0;
            if self.jump.landed < 0xd && !wrapped { return; }
            if !self.set_state(anim, env, 0, false) { return; }
            let seq = self.idle_seq();
            if seq == 0x51 { self.set_anim(anim, 15.0, seq, 0) } else if wrapped { self.set_anim(anim, -2.0, seq, 0) }
            return;
        }
        if !self.set_state(anim, env, 2, false) { return; }
        let blend = if 5 < self.jump.landed { 0xe } else { 8 };
        self.substate = 1;
        self.set_anim(anim, blend as f32, 4, 0x10);
        self.walk_gate = 1;
    }

    /// The ticks until the feet reach the ground at gravity `g` (`0x2d4138`, here from the current height).
    pub fn land_eta(&self, g: f32) -> f32 {
        let (vz, h) = (self.vel[2], (self.pos[2] - self.ground_z).max(0.0));
        if g <= 0.0 { return 1.0; }
        ((vz + (vz * vz + 2.0 * g * h).sqrt()) / g).max(1.0)
    }
}
