//! The standing ground states: idle 0 (group 0), stop / skid 3 (group 1) and crouch 4 (group 0xc) — their SetState
//! entries (L00 `0x2c4560`), physics (`0x2ba8e0`, the case shared by 0, 1, 3, 4, 0x1c, 0x1d, 0x1e, 0x39, 0x6d, 0x6f,
//! 0x7d) and transitions (`0x2cb650`).
//!
//! Also the look stance 1 (L1 held: the camera's mode 4; it gives way after 31 ticks) and its scripted twin 0x1d
//! (camera mode 0x51), and the ice floor's variants (the drag 0.7, the crouch's slide 1/2118) and the magnetic
//! walkway with the Gravity Boots (held to the floor).
//!
//! Not ported (the branches are noted in place): the weapon stances (`0x2cb1c8`, `0x18c0da`), the idle fidgets
//! (`0x2ca788` → 0x3c) and the back items' poses, the water effects, the cutscene lock `0x18c0d5` (the look stance
//! faces the camera).

use super::physics::*;
use super::states::{Entry, FLIP_DIRS};
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

impl Hero {
    pub(super) fn ground_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, _play: bool) -> Entry {
        match id {
            0 => {
                self.group = 0;
                self.momentum = self.eff;
                let seq = self.idle_seq();
                Entry::Anim(if seq == 0x51 { 18.0 } else { -1.0 }, seq, 0)
            }
            3 => {
                self.group = 1;
                self.momentum = self.eff;
                // `0x2cb1c8`: a weapon in use that keeps the stance stands him in its sequence instead.
                if self.gun_stance(anim, _env) { return Entry::Return(false); }
                // The stop's sequence 6, or 5 right after the landing sequence 4 (frames 3..16).
                let v = anim.view();
                let seq = if v.seq_a == 4 && 3.0 < v.frame && v.frame < 16.0 { 5 } else { 6 };
                Entry::Anim(9.0, seq, 4)
            }
            1 | 0x1d => {
                self.group = 0;
                self.cam_mode = if id == 1 { 4 } else { 0x51 };
                let seq = self.idle_seq();
                // `0x2c4560`: a use that keeps the stance keeps its sequence.
                if self.items.keep || anim.view().seq_b == seq { return Entry::Done; }
                Entry::Anim(if id == 1 { 18.0 } else { 5.0 }, seq, 0)
            }
            4 => {
                self.group = 0xc;
                self.speed = len2(self.eff).min(7.0 * DT);
                self.momentum = self.eff;
                if 0.116_666_67 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.116_666_67); }
                let v = anim.view();
                let blend = if v.seq_a == self.idle_seq() { 9.0 } else { 14.0 };
                self.set_anim(anim, blend, 0xd, 2);
                anim.set_loop(3, 0x12);
                Entry::Done
            }
            _ => Entry::Done,
        }
    }

    /// `0x2cb1c8` (R&C1's `0x242858`): with the held item in use and keeping the stance (`0x18c0d8` / `0x18c0da`,
    /// the item still the one used), he stands (SetState 0 without its sequence), his momentum cut to 0.025 and his
    /// speed to at most 0.025, in the item's standing sequence (its row's `+0x18`) blended over 11 ticks. Another item
    /// ends the use. True when he stood.
    pub(super) fn gun_stance(&mut self, anim: &mut dyn AnimCtl, env: &Env) -> bool {
        let it = self.items;
        if !(it.using && it.keep) { return false; }
        if it.ready != 2 || it.item != it.use_item {
            let idle = self.idle_seq();
            self.items.use_end(self.state, anim.view().seq_b, idle);
            return false;
        }
        let Some(r) = super::items::row(it.item) else { return false };
        if !self.set_state(anim, env, 0, false) { return false; }
        if 0.025 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.025); }
        self.speed = self.speed.min(0.025);
        self.set_anim(anim, 11.0, r.use_seq as u8, 0);
        self.items.use_seq = r.use_seq;
        true
    }

    /// The ground case of `0x2ba8e0`.
    pub(super) fn phys_ground(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        if self.surf.ice { self.drag(0.7, 0.0) } else { self.drag(0.5, 1.0 / 30.0) }
        self.edge_brake(env, 3.7, 0.0);
        self.stick_target(env, 1.0, 0);
        // `0x2ba8e0`: the stop (3) with a weapon in use that keeps the stance becomes the stance (`0x2cb1c8`).
        if self.state == 3 && self.gun_stance(anim, env) { return; }
        if self.state == 4 {
            let seq = anim.view().seq_b;
            if 0.2 < self.stick_mag || seq == 0xe || seq == 0xf {
                self.turn_to(0.002, 0.07, 0.116_355_3, 0);
                if 0.005_817_765 < self.yaw_vel.abs() {
                    let s = 0xe + (self.yaw_vel < 0.0) as u8;
                    if anim.view().seq_b != s { self.set_anim(anim, 5.0, s, 0); }
                    self.anim_speed = (self.yaw_vel.abs() * 18.093_4).clamp(0.55, 2.5);
                } else if self.yaw_vel == 0.0 && anim.view().seq_b != 0xd {
                    self.set_anim(anim, 7.0, 0xd, 0);
                }
            }
        }
        self.target_speed = 0.0;
        let dec = match self.state {
            3 => {
                let dec = (self.slope_ratio + 1.0).min(1.0) * 0.0075;
                self.stop_decel = dec;
                let v = anim.view();
                if v.seq_a == v.seq_b {
                    let r = self.eff_len_xy / dec;
                    if 5.0 < r && (v.frame_b as f32) < 5.5 {
                        self.anim_speed_for_ticks(&v, 13.0, r, 0.5, -1.0);
                        self.anim_speed = self.anim_speed.min(1.1);
                    } else if 6.5 < v.frame_b as f32 {
                        self.anim_speed = 1.0;
                    }
                }
                dec
            }
            4 => if self.surf.ice { 0.000_472_222_25 } else { 0.0075 },
            _ => 0.004_916_667,
        };
        self.speed_step(0.0, dec);
        self.vel = [0.0; 3];
        self.momentum_decay(dec);
        // The magnetic walkway with the Gravity Boots: held to the floor (on a moby, on its surface).
        if self.surf.magnet && env.boots {
            if 0.2 < self.height {
                self.gravity_from(self.eff[2], 0.006_111_111);
                return;
            }
            self.vel[2] -= 0.015;
            if self.ground_moby.is_some() {
                self.pos = self.ground_point;
                self.air_ticks = 0;
                self.height = 0.0;
            }
            return;
        }
        self.wall_check(env, false);
        if self.air_ticks != 0 {
            self.gravity_from(self.eff_v[2], 0.006_944_44);
        } else {
            self.gravity(0.015);
        }
    }

    /// The ground cases 0, 3, 4 of `0x2cb650`. Returns the state the dispatcher sets with `SetState(n, 1)`.
    pub(super) fn trans_ground(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let held = env.pad.held;
        match self.state {
            0 => {
                // L1: the look stance (`0x4287b0`, a level's veto, is not ported).
                if held & button::L1 != 0 { return Some(1); }
                if self.gear.floating { return Some(0x82); }
                self.idle_fidget(anim, env);
                {
                    let v = anim.view();
                    let idle = self.idle_seq();
                    self.items.use_stance(self.state, v.seq_b, v.blending(), idle);
                    if let Some((b, s)) = self.items.hero_anim.take() { self.set_anim(anim, b, s, 0); }
                }
                self.stick_target(env, 1.0, 0);
                if self.surf.mud && self.f16c == 0 { return Some(0x76); }
                if 5 <= self.air_ticks && 0.4 < self.height {
                    if self.prev_state == 8 && self.timer < 0xf { return None; }
                    return Some(6);
                }
                if self.land_lag != 0 { return None; }
                let from_look = (self.prev_state == 1 || self.prev_state == 0x1d) && self.timer < 5;
                if self.surf.boots == 1 {
                    if Self::cross_within(env, 7) { return Some(0x6e); }
                } else if !from_look {
                    let n = if self.glide_lag != 0 { 1 } else { 7 };
                    if self.try_jump(anim, env, n, false) { return None; }
                }
                if held & button::R1 != 0 && self.grounded_ticks != 0 && !(anim.view().seq_b == 0x1a && self.anim_frame < 108.0) { return Some(4); }
                // A rail in reach (`0x18a45c` → 0x27); a sloped ice floor: the slide 0x2e.
                if self.grind.found { return Some(0x27); }
                if self.surf.ice && 0.087_266_47 < self.slope { return Some(0x2e); }
                if (self.prev_state == 1 || self.prev_state == 0x1d) && self.timer < 0x18 && self.surf.boots != 1 {
                    self.stick_target(env, 1.0, 0);
                    if 1.221_730_6 < rot_dist(self.target_yaw, env.cam_yaw) && dist(self.pos, env.cam_pos) < 2.5 { return None; }
                }
                // `0x2b3370(0.1745)`: strafing and turned 10° from the camera; or a magnitude above 0.22.
                if self.strafe_turned(env, 0.174_532_9) || 0.22 < self.stick_mag { return Some(2); }
                None
            }
            1 | 0x1d => {
                if self.mode == 0 && !self.items.keep {
                    let seq = if held & button::R1 != 0 { 0xd } else { self.idle_seq() };
                    if anim.view().seq_b != seq { self.set_anim(anim, 8.0, seq, 0); }
                }
                if self.state == 0x1d {
                    if held & button::L1 != 0 { return None; }
                } else {
                    // The wrench busy (thrown or swinging) keeps the stance too.
                    let busy = self.items.thrown != 0;
                    if (held & button::L1 != 0 || busy) && self.timer < 0x1f { return None; }
                    if held & button::R1 != 0 { return Some(4); }
                }
                if self.set_state(anim, env, 0, false) {
                    let seq = self.idle_seq();
                    if anim.view().seq_b != seq { self.set_anim(anim, 8.0, seq, 0); }
                }
                None
            }
            3 => {
                if held & button::L1 != 0 { return Some(1); }
                if self.land_lag == 0 && env.pad.pressed & button::CROSS != 0 { return Some(7); }
                if self.land_lag == 0 && held & button::R1 != 0 { return Some(4); }
                if 9 <= self.air_ticks && 0.3 < self.height { return Some(6); }
                if anim.view().flags & 2 != 0 { return Some(0); }
                if self.grind.found { return Some(0x27); }
                if self.stick_mag <= 0.2 || self.land_lag != 0 { return None; }
                if self.idle_seq() == 0x51 {
                    if 10.0 < self.anim_frame && self.set_state(anim, env, 0, false) {
                        let seq = self.idle_seq();
                        self.set_anim(anim, 17.0, seq, 0);
                    }
                } else if self.speed == 0.0 {
                    self.set_state(anim, env, 0, false);
                    return None;
                }
                (0.5 < self.stick_mag).then_some(2)
            }
            4 => {
                if 0xb < self.timer && held & button::R1 == 0 {
                    self.set_state(anim, env, 0, false);
                    let seq = self.idle_seq();
                    self.set_anim(anim, -2.0, seq, 0);
                    return None;
                }
                if self.crouch_jump(anim, env) { return None; }
                if 9 <= self.air_ticks {
                    if self.prev_state == 8 && self.timer < 0xf { return None; }
                    return Some(6);
                }
                None
            }
            _ => None,
        }
    }

    /// `0x2cadb0`: the crouch's jumps. R1 held, the stick not ahead, ✕ and a direction press within 12 ticks of each
    /// other (both within 17) → the flip 0xb; after 7 crouched ticks ✕ within 9 → with Clank's packs the long jumps
    /// (moving: stick over 0.7, forward speed over 0.015) — the Heli-Pack's 10 (unless a wall within 2.5 slopes under
    /// 45°), the Thruster-Pack's 0x10 (no wall within 2.5, or a moby there) — or standing the high jumps (Thruster-Pack
    /// 0xd, Heli-Pack 0xf); without a pack the jump 7.
    fn crouch_jump(&mut self, anim: &mut dyn AnimCtl, env: &Env) -> bool {
        if self.state == 0xe { return false; }
        let gap = self.flip_gap(env);
        if env.pad.held & button::R1 != 0 && 0 < gap && env.pad.combo(button::CROSS, FLIP_DIRS, gap + 5, gap) {
            self.set_state(anim, env, 0xb, true);
            return true;
        }
        if self.timer < 7 || !Self::cross_within(env, 9) { return false; }
        let moving = 0.7 < self.stick_mag && 0.015 < self.fwd_speed;
        let a = self.ahead;
        let id = if env.clank && env.pack == 2 && moving && !(a.dist <= 2.5 && std::f32::consts::FRAC_PI_4 <= a.slope) {
            10
        } else if env.clank && env.pack == 3 && moving && (2.5 < a.dist || a.solid) {
            0x10
        } else if env.clank && env.pack == 3 {
            0xd
        } else if env.clank && env.pack == 2 {
            0xf
        } else {
            7
        };
        self.set_state(anim, env, id, true);
        true
    }

    /// `0x2a9ea8(target, ticks, k, step)`: the playback speed that reaches frame `target` in `ticks` ticks;
    /// `step` < 0 sets it, otherwise the speed approaches it by `step`.
    pub fn anim_speed_for_ticks(&mut self, v: &rc_game::hero::anim::AnimView, target: f32, ticks: f32, k: f32, step: f32) {
        let cur = v.frame;
        let dist = if cur < target { target - cur } else { target + (v.frame_count_b as f32 - cur) };
        let rate = if v.rate != 0.0 { v.rate } else { 1.0 };
        let want = (k * dist) / (ticks * rate).max(1e-6);
        if step < 0.0 { self.anim_speed = want } else { approach(want, step, &mut self.anim_speed); }
    }
}
