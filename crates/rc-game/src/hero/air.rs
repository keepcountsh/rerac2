//! The fall 6 (group 2): SetState entry (L00 `0x2c4560` case 6 / 0x74), physics (`0x2ba8e0` case 6 / 0x2c, the air
//! control `0x2b6cd0`, gravity `0x18a8a0` with the fall speed capped at 50 / 60) and transitions (`0x2cb650` case 6
//! / 0x2c): the high-fall sequence 0xb after 18 ticks or above 1.75, and the landing — idle, stop or walk by the
//! stick and the speed, with the landing lag `0x18a000` after a high fall.
//!
//! The tumble 0x2c (group 2, camera 7: knocked through the air) shares it: the body's pitch springs to 0.44 above a
//! height of 4 and the roll to 0 (0.02, 0.3, at most 0.0437), and it lands with a longer lag (10).
//!
//! In the air: ✕ held with Clank on the back (more than 0.6 up) opens the glide 8; a ledge in reach (0x17).
//!
//! Not ported: the stuck-in-air jump (`0x18a022`), the landing effects (`0x2a6188`) and the
//! camera jolt (`0x2d35e8`).

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::hero::anim::AnimCtl;

impl Hero {
    pub(super) fn fall_entry(&mut self, _anim: &mut dyn AnimCtl, _env: &Env, id: i32, _play: bool) -> Entry {
        if id == 0x2c {
            self.group = 2;
            self.cam_mode = 7;
            self.rot_vel = [0.0; 2];
            self.vel = self.eff;
            self.substate = 0;
            return Entry::Anim(15.0, 0xb, 0);
        }
        self.group = 2;
        self.fall_g = 0.006_111_111_5;
        self.fall_dec = 0.001_944_444_5;
        self.substate = 0;
        self.vel = self.eff;
        if 1.75 < self.height {
            self.substate = 1;
            return Entry::Anim(12.0, 0xb, 0);
        }
        Entry::Anim(10.0, 10, 0)
    }

    pub(super) fn phys_fall(&mut self, env: &Env, _anim: &mut dyn AnimCtl) {
        if self.state == 0x2c {
            let t = if 4.0 < self.height { 0.436_332_3 } else { 0.0 };
            let (mut a, mut v) = (self.rot[1], self.rot_vel[1]);
            turn_spring(t, 0.02, 0.3, 0.043_633_23, &mut a, &mut v, 0);
            (self.rot[1], self.rot_vel[1]) = (a, v);
            let (mut a, mut v) = (self.rot[0], self.rot_vel[0]);
            turn_spring(0.0, 0.02, 0.3, 0.043_633_23, &mut a, &mut v, 0);
            (self.rot[0], self.rot_vel[0]) = (a, v);
            if 1.0 < self.height { self.substate = 1; }
        }
        self.run_target(env);
        // `0x18a000`: the air control waits out the landing lag.
        if self.land_lag == 0 { self.air_control(env); }
        self.lean();
        self.drag(0.7, 0.0);
        self.vel[2] -= self.fall_g;
        if self.vel[2] < -50.0 * DT { self.vel[2] = -50.0 * DT; }
    }

    pub(super) fn trans_fall(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        if self.substate == 0 && (0x11 < self.timer || 1.75 < self.height) {
            self.set_anim(anim, 16.0, 0xb, 0);
            self.substate = 1;
            self.cam_mode = if self.state == 0x2c { 7 } else { 0 };
        }
        if self.air_ticks != 0 {
            if self.hp < 1 { return None; }
            // In the air: ✕ held with Clank on the back → the glide; a ledge in reach is grabbed (`0x18a398`), a rail
            // (0x27) or a pole (0x71).
            if env.pad.held & crate::pad::button::CROSS != 0 && env.clank && env.pack != 4 && self.land_lag == 0 && 0.6 < self.height { return Some(8); }
            if self.ledge.found { return Some(0x17); }
            if self.grind.found { return Some(0x27); }
            if self.grind.pole.is_some() { return Some(0x71); }
            return None;
        }
        if self.state == 0x2c {
            self.rot[0] = 0.0;
            self.rot[1] = 0.0;
        }
        self.disp[2] = self.disp[2].max(-0.15);
        self.vel[2] = self.vel[2].max(-0.15);
        if self.hp <= 0 { return Some(0x39); }
        if self.timer < 0x5a {
            if self.substate == 1 {
                if self.set_state(anim, env, 0, false) { self.set_anim(anim, -1.0, 0xc, 9); }
                self.momentum[2] = 0.0;
                if 0.116_666_67 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.116_666_67); }
                self.land_lag = if self.prev_state == 0x2c { 10 } else { 7 };
                if !self.on_moby && 0x32 < self.timer { self.land_lag = 0x12; }
            } else if self.stick_mag <= 0.5 || self.land_lag != 0 {
                if 0.05 < self.eff_len_xy {
                    if self.set_state(anim, env, 3, false) {
                        self.set_anim(anim, 12.0, 6, 0);
                        if 0.128_333 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.128_333); }
                    }
                    return None;
                }
                self.set_state(anim, env, 0, true);
            } else {
                self.set_state(anim, env, 2, true);
            }
        } else {
            if self.set_state(anim, env, 0, false) { self.set_anim(anim, 6.0, 0xc, 4); }
            if !self.on_moby { self.land_lag = 0x16; }
        }
        None
    }

    /// `0x2b6cd0`'s default path (the jumps 7 / 9 and the fall): turn toward the stick (0.04, 0.2, `0x18a2b4`) —
    /// strafing (`0x18b274`), toward the camera's yaw and forward row instead, the stick's yaw and direction then put
    /// back and kept as the strafe's (`0x18b278` / `0x18b260`), so he moves along the stick facing ahead — the drag
    /// (0.7, 0) carried as momentum, then the air acceleration `0x2b62a0` — 1/180 when the stick asks for more than a
    /// fifth of the run speed, else (fall group 2) the horizontal speed falls toward 0 by `0x18a8a4`, or 1/450 (1/1800
    /// when blocked ahead; 11/3600 on a moby).
    pub(super) fn air_control(&mut self, env: &Env) {
        let (yaw, dir) = (self.target_yaw, self.stick_dir);
        if self.strafe != 0 {
            self.target_yaw = env.cam_yaw;
            self.stick_dir = env.cam_rows[0];
        }
        self.turn_to(0.04, 0.2, self.jump.air_turn, 0);
        if self.strafe != 0 {
            (self.target_yaw, self.stick_dir) = (yaw, dir);
            (self.strafe_yaw, self.strafe_dir) = (yaw, dir);
        }
        self.drag(0.7, 0.0);
        self.momentum = self.drag;
        let mut a = 0.005_555_556;
        if self.target_speed < RUN_SPEED * 0.003_333_333_6 {
            if self.group == 2 {
                let mut s = len2(self.vel);
                approach(0.0, self.fall_dec, &mut s);
                self.vel = set_len2(self.vel, s);
                return;
            }
            a = if self.jump.blocked != 0 { 0.000_555_555_6 } else { 0.002_222_222_3 };
        }
        self.air_accel(a);
    }

    /// `0x2b62a0(a)` in gravity mode 0: the velocity's xy moves toward the stick's (target yaw, target speed; 1/12 when
    /// blocked against a wall in a jump) by at most `a·k`, k = 1.5·(1 − cos(angle between them)) in 1..3; then the
    /// speed = forward speed − the drag along the facing.
    pub(super) fn air_accel(&mut self, a: f32) {
        let want = if self.group == 4 && self.jump.blocked != 0 && self.cap_hit != 0 { 1.0 / 12.0 } else { self.target_speed };
        let (s, c) = self.target_yaw.sin_cos();
        let d = [c * want, s * want, self.vel[2]];
        let (ld, lv) = (len2(d), len2(self.vel));
        let f = if ld == 0.0 || lv == 0.0 { 0.0 } else { (-(self.vel[0] * d[0] + self.vel[1] * d[1]) / ld) / lv };
        let k = ((f + 1.0) * 1.5).clamp(1.0, 3.0);
        let mut diff = sub(d, self.vel);
        diff[2] = 0.0;
        if a * k < len2(diff) { diff = set_len2(diff, a * k); }
        self.vel = add(self.vel, diff);
        self.eff_h = self.vel;
        let (s, c) = self.rot[2].sin_cos();
        self.speed = self.fwd_speed - (self.drag[0] * c + self.drag[1] * s);
    }
}
