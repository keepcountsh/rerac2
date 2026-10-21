//! Walk / run 2 (group 1) and its floor variants, the ice slide 0x2e and the wading walk 0x70: the SetState entry
//! (L00 `0x2c4560` case 2 / 0x2e / 0x70), physics (`0x2ba8e0` case 2 / 0x70, case 0x2e) and transitions (`0x2cb650`
//! case 2 / 0x2e / 0x70), with the walk ↔ run animation selector `0x2ca358`.
//!
//! The animation: substate 0 walks (sequence 3, playback speed · 157 in 0.6..4), substate 1 runs (sequence 4,
//! speed · 14 in 0.6..2.2). The substate moves up above the edge `0x1a7dac + 16·s` (2.35 / 10 u/s) and down below
//! `$gp − 0x7248 + 16·s` (0 / 1.9 u/s, `$gp` = 0x1aeff0); the new sequence starts at the old one's phase plus
//! `$gp − 0x7228[new + 2·old]` (1 walk → run, 5 run → walk).
//!
//! **Strafing** (L2 / R2, `0x18b274`): the facing springs to the camera's yaw and the walk moves along the stick
//! (`0x2b36a8`); 0x2b3370 keeps the walk going while the facing is off the camera's.
//!
//! **Floors.** Mud (`0x18b2ba`) turns the walk into 0x76; ice (`0x18b2b2`) into the slide 0x2e (sequence 0x37: the
//! velocity eases toward the stick by 1/1470 + the opposition, the slope pulls 0.17·slope, `0x2b96a0`); wading
//! (`0x18c0d9`, water 0.25..0.85 deep) into 0x70 (sequence 0x59, 0.8 of the run speed, at least 2.5).
//!
//! Not ported: the held item's run speeds and the item 10's kept direction, the water effects, the body lean
//! (`0x2b7c88`, `0x2b8dc0`, the slide's lean angles), the sliding floors' drag (`0x18a890`), the weapon stances on
//! stopping (`0x2cb1c8`), the stick-gravity frames (`0x18c0e3`).

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

/// Walk → run / run → walk speed edges (u/s): `$gp − 0x7248 + 16·s` (low), `0x1a7dac + 16·s` (high).
const WALK_LOW: [f32; 2] = [0.0, 1.9];
const WALK_HIGH: [f32; 2] = [2.35, 10.0];
/// The phase offset when the substate changes, `[new + 2·old]` (`$gp − 0x7228`).
const PHASE: [i32; 4] = [0, 1, 5, 0];
/// The walk entry's start frame per substate (`$gp − 0x7218`).
const ENTRY_FRAME: [i32; 2] = [8, 0x10];

impl Hero {
    pub(super) fn walk_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        self.group = 1;
        if self.prev_group != 4 && self.prev2_group != 4 { self.speed = len2(self.eff); }
        self.run_dir = self.rows[0];
        self.run_keep = 0;
        self.run_yaw = self.rot[2];
        self.speed = self.speed.min(7.0 * DT);
        self.quick_turn = 0;
        self.stick_target(env, 1.0, 0);
        if PI / 2.0 < rot_dist(self.rot[2], self.target_yaw) && (self.prev_group == 4 || self.prev_state == 3 || self.prev2_state == 3) {
            self.speed = 0.0;
        }
        if self.surf.mud && (self.f16c == 0 || self.height < 0.3) { return Entry::Return(self.set_state(anim, env, 0x76, true)); }
        if self.surf.ice || id == 0x2e {
            self.speed = self.speed.min(0.128_333_33);
            self.vel[2] = 0.0;
            self.state = 0x2e;
            self.ice_slope = 0.0;
            return Entry::Anim(8.0, 0x37, 0);
        }
        if self.surf.wading || id == 0x70 {
            self.state = 0x70;
            return Entry::Anim(8.0, 0x59, 0);
        }
        self.substate = 0;
        self.run_hold = 0;
        self.walk_gate = 0;
        if !play { return Entry::Done; }
        let mut blend = if self.hp == 1 { 0xc } else { 8 };
        if self.prev2_state == 0x22 || self.prev_state == 0x22 || self.prev2_state == 0x83 || self.prev_state == 0x83 { blend = 0xf; }
        if self.prev_state == 0x13 { blend = 0xd; }
        if self.prev2_state == 6 || self.prev2_state == 0x2c { blend = 0x12; }
        if self.prev2_group == 4 && 5 < self.jump.landed { blend = 0xd; }
        Entry::Anim(blend as f32, (self.substate + 3) as u8, ENTRY_FRAME[self.substate as usize & 1])
    }

    /// `0x2ba8e0` case 2 / 0x70 (and 0x2e).
    pub(super) fn phys_walk(&mut self, env: &Env, _anim: &mut dyn AnimCtl) {
        if self.state == 0x2e {
            self.ice_move(env);
            if self.air_ticks != 0 {
                self.gravity_from(self.disp[2], 0.006_944_444_5);
            } else {
                self.vel[2] = 0.0;
                self.vel[2] -= 1.0 / 24.0;
            }
            return;
        }
        if self.strafe == 0 {
            if self.eff_len_xy < 0.028_333_336 {
                self.run_hold = 1;
                if 2.094_395_4 < self.yaw_residual.abs() && self.stick_mag == 1.0 { self.quick_turn = 1; }
            }
            // A stick swung past 110° within 7 ticks at full tilt: the quick turn.
            let (a1, a7) = (stick_angle_ago(env.pad, 1), stick_angle_ago(env.pad, 7));
            if 1.919_862_4 < rot_dist(a1, a7) && 1.221_730_6 < self.yaw_residual.abs() && self.stick_mag == 1.0 {
                self.run_hold = 1;
                self.quick_turn = 1;
            }
            if self.yaw_residual.abs() < 0.087_266_47 { self.run_hold = 0; }
        }
        if self.quick_turn != 0 && self.yaw_residual.abs() < 0.069_813_18 { self.quick_turn = 0; }
        if self.stick_mag < 0.2 {
            if self.run_keep != 0 { self.quick_turn = 1; }
            self.target_speed = 0.0;
            self.stick_dir = self.run_dir;
            self.target_yaw = self.run_yaw;
        } else {
            self.run_target(env);
            self.run_dir = self.rows[0];
            self.run_yaw = self.rot[2];
        }
        let mut acc_k = 1.0;
        if self.strafe == 0 {
            self.strafe_yaw = self.rot[2];
            self.strafe_dir = self.rows[0];
            if self.substate == 1 && self.quick_turn == 0 {
                self.turn_to(0.02, 0.17, 0.165_806_28, 0);
            } else {
                if self.stick_mag < 0.9 && PI / 4.0 < self.yaw_residual.abs() { acc_k = 0.0; }
                self.run_turn();
            }
        } else {
            self.strafe_turn(env);
        }
        self.lean();
        if self.quick_turn != 0 {
            let v = (0.099_166_67 - self.yaw_residual.abs() * 0.029_178_405).max(0.0);
            self.target_speed = self.target_speed.min(v);
        }
        let acc = acc_k * 0.005;
        let mut dec = 0.005_555_556;
        if self.quick_turn != 0 {
            let f = (2.617_994 - self.yaw_residual).clamp(0.2, 1.0);
            dec = 0.004_722_222;
            self.target_speed *= f;
        }
        let r = self.yaw_residual.abs();
        if self.stick_mag < 0.75 && 0.610_865_3 < r { self.target_speed *= (1.0 - r * 0.4).max(0.0); }
        self.speed_step(acc, dec);
        if self.run_hold == 0 && self.strafe == 0 {
            self.set_planar_vel(f32::MAX)
        } else if self.strafe == 0 {
            self.set_planar_vel(self.target_yaw)
        } else {
            self.set_planar_vel(self.strafe_yaw)
        }
        self.wall_check(env, true);
        if self.air_ticks != 0 {
            self.gravity_from(self.eff_v[2], 0.006_944_44);
            self.edge_slide();
        } else {
            self.gravity(0.015);
        }
    }

    /// `0x2b96a0`: the ice slide (module docs).
    pub fn ice_move(&mut self, env: &Env) {
        self.stick_target(env, 1.0, 0);
        self.turn_to(0.02, 0.2, 0.061_086_5, 0);
        match self.state {
            0x13 => {
                if self.stick_mag < 0.2 { self.target_yaw = self.rot[2]; }
                self.target_speed = 0.4;
            }
            0x15 => self.target_speed = 0.0,
            _ => {}
        }
        let (s, c) = self.target_yaw.sin_cos();
        let k = self.target_speed * 0.098_333_34;
        let want = [c * k, s * k, 0.0];
        let mut f = -(self.vel[0] * want[0] + self.vel[1] * want[1]);
        if 0.0 < f {
            let (lw, lv) = (len2(want), len2(self.vel));
            f = if 0.0 < lw && 0.0 < lv { f / lw / lv } else { 0.0 };
        }
        let f = f.clamp(0.0, 1.0);
        let mut a = self.target_speed * 0.000_680_555_6 + f * 0.000_555_555_6;
        if env.pad.no_direction { a = 0.000_305_555_56; }
        if self.state == 0x13 && self.timer < 0xf && self.eff_len < 0.05 { a = 0.003_055_555_7; }
        if 0.098_333_34 < self.eff_len { a = 0.003_611_111; }
        let mut d = sub(want, self.eff_h);
        d[2] = 0.0;
        if a < len(d) { d = set_len(d, a); }
        self.vel = add(self.eff_h, d);
        if 0.087_266_47 < self.slope {
            let yaw = atan2(self.ground_normal[0], self.ground_normal[1]);
            let (s, c) = yaw.sin_cos();
            let pull = [c * self.slope * 0.169_765_26, s * self.slope * 0.169_765_26, 0.0];
            let mut d = sub(pull, self.vel);
            d[2] = 0.0;
            let m = self.slope * 0.003_006_259_7;
            if m < len(d) { d = set_len(d, m); }
            self.vel = add(self.vel, d);
        }
        let lw = len(want);
        self.ice_along = (self.vel[0] * want[0] + self.vel[1] * want[1]) / if 0.0 < lw { lw * 0.098_333_34 } else { 1.0 };
        self.wall_check(env, false);
        self.ice_slope = self.slope;
        if (self.pos[2] - self.ground_z).abs() < 0.15 { self.pos[2] = self.ground_z; }
    }

    /// `0x2cb650` case 2 / 0x2e / 0x70.
    pub(super) fn trans_walk(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        if self.walk_gate != 0 {
            let v = anim.view();
            if 0.9 < v.t || v.seq_a != 0xff { self.walk_gate = 0; }
        }
        if self.surf.mud && self.f16c == 0 { return Some(0x76); }
        let wading_in = self.prev_group == 0x12 && self.surf.wading;
        if !(self.air_ticks < 5 || wading_in || self.height <= 1.35 || self.step_down(env)) { return Some(6); }
        let n = if self.glide_lag != 0 { 1 } else { 9 };
        if Self::cross_within(env, n) && !(4 < self.air_ticks) {
            self.stick_target(env, 1.0, 0);
            if self.try_jump(anim, env, n, true) { return None; }
            return Some(if 0.85 < self.stick_mag && self.strafe == 0 { 9 } else { 7 });
        }
        if self.air_ticks != 0 { return None; }
        let slow_ice = !self.surf.ice || (self.eff_len_xy < 0.033_333_335 && self.slope < 0.087_266_47);
        if slow_ice && env.pad.held & button::L1 != 0 { return Some(1); }
        if env.pad.held & button::R1 != 0 { return Some(4); }
        // The floor's variant.
        match self.state {
            2 if self.surf.ice => { self.set_state(anim, env, 0x2e, true); }
            0x2e if !self.surf.ice => { self.set_state(anim, env, 2, true); }
            _ => {}
        }
        match self.state {
            2 if self.surf.wading => { self.set_state(anim, env, 0x70, true); }
            0x70 if !self.surf.wading => { self.set_state(anim, env, 2, true); }
            _ => {}
        }
        if self.grind.found { return Some(0x27); }
        let turned = self.strafe_turned(env, 0.034_906_585);
        match self.state {
            2 => {
                let keep = (0.17 <= self.stick_mag && self.land_lag == 0) || turned || (self.run_keep != 0 && 0.017_453_294 <= self.yaw_residual.abs());
                if keep {
                    self.walk_anim(anim);
                    return None;
                }
                if self.set_state(anim, env, 0, false) {
                    if 0.1 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.1); }
                    self.speed = self.speed.min(0.025);
                    let seq = self.idle_seq();
                    self.set_anim(anim, if seq == 0x51 { 15.0 } else { -1.0 }, seq, 0);
                }
                None
            }
            0x2e => {
                let v = anim.view();
                if env.pad.no_direction && len(self.vel) < 0.000_833_333_4 && self.slope < 0.087_266_47 && self.set_state(anim, env, 0, false) {
                    let seq = self.idle_seq();
                    self.set_anim(anim, 17.0, seq, 0);
                    return None;
                }
                if v.seq_b == 0x37 {
                    self.anim_speed = ((1.0 - self.ice_along) * 2.0).min(1.7).max(0.75);
                } else if v.seq_b == 0x66 {
                    self.anim_speed = (self.eff_len * 10.0).min(1.1).max(0.3);
                }
                None
            }
            0x70 => {
                if env.pad.no_direction && len(self.vel) < 0.053_333_34 && self.set_state(anim, env, 0, false) {
                    let seq = self.idle_seq();
                    self.set_anim(anim, 17.0, seq, 0);
                    return None;
                }
                self.anim_speed = (self.eff_len * 35.0).max(0.6).min(1.05);
                None
            }
            _ => None,
        }
    }

    /// `0x2cb038`: a step down ahead (a line 0.75·r ahead from 0.47 up to 1 down hits less than 0.37 above and more
    /// than 1 below the feet): the walk does not fall.
    fn step_down(&self, env: &Env) -> bool {
        if self.eff_len_xy < self.cap_radius * 0.05 { return false; }
        if 0.523_598_85 < rot_dist(atan2(self.disp[0], self.disp[1]), self.rot[2]) { return false; }
        let (s, c) = self.rot[2].sin_cos();
        let p = [self.pos[0] + c * self.cap_radius * 0.75, self.pos[1] + s * self.cap_radius * 0.75, self.pos[2]];
        let a = [p[0], p[1], p[2] + 0.47];
        let b = [p[0], p[1], p[2] - 1.0];
        match env.line(a, b, 2) {
            Some(o) => {
                let d = o.point[2] - self.pos[2];
                d < 0.37 && -1.0 < d
            }
            None => false,
        }
    }

    /// `0x2ca358`: the walk / run sequence and its playback speed (Clank's body; the back-pedal while strafing
    /// backwards; else the walk 3 / run 4 by the speed, swapped in phase).
    pub(super) fn walk_anim(&mut self, anim: &mut dyn AnimCtl) {
        let v = anim.view();
        if self.mode == 1 {
            // Clank's body: its own walk, at |eff| × 90.
            self.anim_speed = (self.eff_len * 90.0).max(0.5);
            return;
        }
        if self.strafe != 0 && self.strafe_back {
            // Strafing backwards: the back-pedal 0x14 at the speed × 25 (0.7..2.2).
            if v.seq_b != 0x14 { self.set_anim(anim, 11.0, 0x14, 0); }
            self.anim_speed = (self.speed * 25.0).clamp(0.7, 2.2);
            return;
        }
        if v.seq_b == 0x14 {
            self.set_anim(anim, 7.0, (self.substate + 3) as u8, 0);
        } else if 4 < self.substate_timer && self.walk_gate == 0 {
            let old = self.substate.clamp(0, 1);
            let mut s = old;
            let sp = self.speed / DT;
            if sp < WALK_LOW[s as usize] || WALK_HIGH[s as usize] < sp {
                while 0 < s && sp < WALK_LOW[s as usize] { s -= 1; }
                while s < 1 && WALK_HIGH[s as usize] < sp { s += 1; }
                if s != old {
                    let (n_old, n_new) = (anim.frame_count(old as u8 + 3) as i32, anim.frame_count(s as u8 + 3) as i32);
                    let frame = if n_old == 0 || n_new == 0 { 0 } else { (v.frame_b as i32 * n_new / n_old + PHASE[(s + 2 * old) as usize]) % n_new };
                    self.substate = s;
                    self.set_anim(anim, 8.0, (s + 3) as u8, frame);
                }
            }
        }
        self.anim_speed = if self.substate == 0 { (self.speed * 157.0).clamp(0.6, 4.0) } else { (self.speed * 14.0).clamp(0.6, 2.2) };
    }
}
