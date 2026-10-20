//! Clank's body (`0x18c0d4` = 1: the levels where Clank walks on his own, with his own class and sequences) in
//! Ratchet's machine: idle 0x3f, walk / run 0x40, fall 0x41, hurt 0x42, death 0x43, the hop 0x44, the ledge grab
//! 0x46 and hang 0x47, the shimmies 0x49 / 0x4a, the Heli glide 0x4b, the punch 0x4d, the mud 0x4e, and the
//! transitions of his jump 0x45 and climb 0x48 (their entries and physics are the jump group's) — SetState (L00
//! `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`).
//!
//! Sequences: 0 idle, 1 its fidget, 2 walk, 3 run, 4 fall, 5 death, 6 hurt, 7 jump, 8 / 10 grab / hang, 0xb / 0xc
//! shimmy, 0xe glide, 0xf / 0x10 the punches (swing rows 7 / 8), 0x12 the idle's look-around.
//!
//! Not ported: the idle's look-around timer `0x18c340` (0: never), the punch's sounds, the lean.

use super::ledge::cross;
use super::melee::SWINGS;
use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

/// `0x28d630`: Clank's shimmy speed by frame (×0.5).
const SHIMMY: [f32; 20] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.02, 0.02, 0.042, 0.042, 0.042, 0.042, 0.033, 0.033, 0.013, 0.013, 0.013, 0.013];

impl Hero {
    pub(super) fn clank_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        let mud = self.surf.mud && self.f16c == 0;
        match id {
            0x3f => {
                self.group = 0;
                if mud { return Entry::Return(self.set_state(anim, env, 0x4e, true)); }
                Entry::Anim(9.0, 0, 0)
            }
            0x40 => {
                if mud { return Entry::Return(self.set_state(anim, env, 0x4e, true)); }
                self.group = 1;
                self.speed = len2(self.eff);
                if self.prev_state == 3 { self.speed = 0.0; }
                if self.prev_group == 0 { self.speed = len2(self.momentum); }
                self.quick_turn = 0;
                self.run_hold = 0;
                Entry::Anim(8.0, 2, 0)
            }
            0x41 => {
                self.group = 2;
                self.fall_dec = 0.001_944_444_5;
                self.vel = self.eff;
                Entry::Anim(8.0, 4, 0)
            }
            0x42 => {
                self.group = 7;
                self.scale_motion(0.5);
                self.vel = self.eff;
                if 1.0 / 12.0 < len(self.vel) { self.vel = set_len(self.vel, 1.0 / 12.0); }
                self.take_damage(1);
                self.damage.invincible = 0x32;
                if play {
                    self.set_anim(anim, -3.0, 6, 3);
                    self.damage.hurts += 1;
                }
                Entry::Done
            }
            0x43 => {
                self.group = 0x14;
                self.hp = 0;
                self.momentum = self.eff;
                Entry::Anim(12.0, 5, 0)
            }
            0x44 => {
                self.group = 2;
                Entry::Anim(8.0, 4, 0)
            }
            0x46 => {
                self.group = 3;
                self.cam_mode = 0xd;
                (self.ledge.g, self.ledge.z) = (0.003_333_333_4, self.pos[2]);
                if self.prev_group == 4 { (self.ledge.g, self.ledge.z) = (self.jump.g, self.jump.ref_z); }
                if play { self.set_anim(anim, 7.0, 8, 0xe); }
                Entry::Done
            }
            0x47 | 0x49 | 0x4a => {
                self.group = 3;
                self.cam_mode = 0xd;
                Entry::Anim(6.0, match id { 0x47 => 10, 0x49 => 0xb, _ => 0xc }, 0)
            }
            0x4b => {
                self.group = 5;
                if 0.075 < self.eff_len { self.scale_motion(0.075 / self.eff_len); }
                self.vel = self.disp;
                Entry::Anim(12.0, 0xe, 0)
            }
            0x4d => {
                self.group = 6;
                // The punches alternate (rows 7 / 8) when chained from one before its chain frame.
                let sw = SWINGS[self.melee.row.clamp(0, 8) as usize];
                let mut k = 0;
                if (self.prev_group == 6 || (self.prev2_group == 6 && matches!(anim.view().seq_b, 0xf | 0x10))) && sw[0] == 0 && self.anim_frame < sw[2] as f32 {
                    k = (sw[1] + 1) % 2;
                }
                self.melee.row = 7 + k as i16;
                self.melee.target = None;
                self.melee.aim = 0;
                self.aim_search(env, 5.0, 0.785_398_2, 0.785_398_2);
                if play { self.set_anim(anim, 5.0, 0xf + k as u8, 1); }
                Entry::Done
            }
            _ => {
                // 0x4e: mud.
                self.group = 2;
                self.vel = self.eff;
                Entry::Anim(12.0, 4, 0)
            }
        }
    }

    /// The air or ground end of Clank's ground physics: in the air the effective vertical speed less 1/144 and the
    /// edge slide, on the ground 0.015 down.
    fn clank_gravity(&mut self) {
        if self.air_ticks != 0 {
            self.gravity_from(self.eff_v[2], 0.006_944_444_5);
            self.edge_slide();
        } else {
            self.vel[2] -= 0.015;
        }
    }

    pub(super) fn phys_clank(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            0x3f | 0x43 => {
                self.edge_brake(env, 3.7, 0.0);
                self.stick_target(env, 1.0, 0);
                self.target_speed = 0.0;
                self.speed_step(0.0, 0.003_499_999_9);
                self.vel = [0.0; 3];
                self.wall_check(env, false);
                self.clank_gravity();
            }
            0x40 => {
                self.stick_target(env, 0.05, 0);
                let (k, max) = if self.quick_turn != 0 { (0.037, 0.195_078_1) } else { (0.033, 0.165_806_28) };
                self.turn_to(k, 0.15, max, 0);
                self.speed_step(0.005, 0.005_555_556);
                if self.eff_len_xy < 0.011_666_667 {
                    self.run_hold = 1;
                    if 1.745_329_4 < self.yaw_residual.abs() && 0.8 < self.stick_mag { self.quick_turn = 1; }
                }
                let (a0, a2) = (stick_angle_ago(env.pad, 0), stick_angle_ago(env.pad, 2));
                if 2.617_994 < rot_dist(a0, a2) && 1.221_730_6 < self.yaw_residual.abs() && 0.8 < self.stick_mag {
                    self.run_hold = 1;
                    self.quick_turn = 1;
                }
                if self.yaw_residual.abs() < 0.087_266_47 { self.run_hold = 0; }
                if self.quick_turn != 0 {
                    if self.yaw_residual.abs() < 0.069_813_18 {
                        self.quick_turn = 0;
                    } else {
                        let v = (0.05 - self.yaw_residual.abs() * 0.021_220_66).max(0.0);
                        self.target_speed = self.target_speed.min(v);
                    }
                }
                if self.run_hold == 0 { self.set_planar_vel(f32::MAX) } else { self.set_planar_vel(self.target_yaw) }
                self.wall_check(env, true);
                self.clank_gravity();
            }
            0x41 => {
                self.stick_target(env, 0.05, 0);
                if self.land_lag == 0 { self.air_control(); }
                self.vel[2] = (self.vel[2] - 0.003_333_333_4).max(-1.0 / 6.0);
            }
            0x42 => {
                let k = if self.air_ticks != 0 { 0.99 } else { 0.92 };
                let l = len2(self.vel);
                self.vel = set_len2(self.vel, l * k);
                self.vel[2] = 0.0;
                if self.air_ticks != 0 { self.gravity_from(self.eff[2], 0.006_111_111) } else { self.vel[2] -= 0.004; }
            }
            0x44 => {
                self.vel[2] -= 0.005_555_556;
            }
            0x46 | 0x47 => {
                let w = sub(self.ledge.edge, self.ledge.hang);
                self.stick_dir = w;
                self.target_yaw = atan2(w[0], w[1]);
                self.turn_to(0.02, 0.2, 0.078_539_82, 0);
                let v = sub(self.ledge.hang, self.pos);
                self.vel = if 0.028_333_334 < len(v) { set_len(v, 0.028_333_334) } else { v };
                approach(self.pos[2], 0.05, &mut self.ledge.z);
            }
            0x49 | 0x4a => {
                self.anim_speed = 1.3;
                let w = sub(self.ledge.edge, self.ledge.hang);
                self.stick_dir = w;
                self.target_yaw = atan2(w[0], w[1]);
                self.turn_to(0.02, 0.2, 0.078_539_82, 0);
                self.vel = [0.0; 3];
                let down = [0.0, 0.0, -1.0];
                let side = if self.state == 0x49 { cross(down, w) } else { cross(w, down) };
                let p = add(self.pos, set_len(side, 0.2));
                if self.ledge_probe(env, self.pos, false).is_some() {
                    if let Some(l) = self.ledge_probe(env, p, false) {
                        let d = sub(l.hang, self.pos);
                        let f = (anim.view().frame as usize).min(SHIMMY.len() - 1);
                        self.vel = set_len(d, SHIMMY[f] * 0.5);
                    }
                }
            }
            0x4b => {
                self.stick_target(env, 1.0 / 30.0, 0);
                self.turn_to(0.04, 0.2, 0.145_444_1, 0);
                self.speed_step(0.004_166_667, 0.001_944_444_5);
                self.set_planar_vel(f32::MAX);
                self.vel[2] = -0.024;
            }
            0x4d => self.phys_clank_punch(env, anim),
            _ => {
                // 0x4e: mud.
                self.target_yaw = self.ground_yaw;
                self.target_speed = 0.091_666_67;
                self.air_accel(0.006_944_444_5);
                self.gravity_from(self.eff_v[2], 0.005);
            }
        }
    }

    /// `0x2ba8e0` case 0x4d: the punch: a step forward in its first frames, a 0.25 sphere between the punching hand's
    /// two joints (2 / 3, the other hand 0 / 1) in the hit window dealing 1 (flags 0x10000), the aim.
    fn phys_clank_punch(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        let sw = SWINGS[self.melee.row.clamp(0, 8) as usize];
        self.target_speed = 0.0;
        let f = anim.view().frame;
        if (sw[1] == 0 || sw[1] == 1) && f < 5.5 { self.target_speed = 0.058_333_337; }
        if !anim.view().blending() && sw[7] as f32 <= f && f <= sw[8] as f32 {
            let j = if sw[1] == 1 { 0 } else { 2 };
            self.hits_out.push(super::HeroHit { centre: self.pos, r: 0.25, damage: 1.0, flags: 0x10000, dir: [0.0; 3], joints: Some([j, j | 1]) });
        }
        approach(1.0, 0.2, &mut self.anim_speed);
        if self.melee.aim == 0 && self.melee.target.is_none() {
            self.aim_search(env, 5.0, 0.785_398_2, 0.785_398_2);
        } else {
            self.target_yaw = self.melee.aim_yaw;
            self.turn_to(0.05, 0.2, 0.283_121_5, 0);
        }
        self.speed_step(0.010_277_778, 0.007_777_778);
        if self.melee.aim == 0 { self.set_planar_vel(f32::MAX) } else { self.set_planar_vel(self.melee.aim_yaw) }
        if self.air_ticks == 0 {
            self.vel[2] -= 0.015;
        } else {
            self.gravity_from(self.eff_v[2], 0.006_944_444_5);
            self.edge_slide();
        }
        self.edge_brake(env, 0.7, 0.0);
    }

    pub(super) fn trans_clank(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let mud = self.surf.mud;
        let v = anim.view();
        match self.state {
            0x3f => {
                if v.flags & 2 != 0 {
                    if v.seq_b == 0 {
                        if self.rng.randf(0.0, 1.0) < 0.5 { self.set_anim(anim, 8.0, 1, 0); }
                    } else {
                        self.set_anim(anim, 12.0, 0, 0);
                    }
                }
                if mud { return Some(0x4e); }
                if env.pad.held & button::L1 != 0 { return Some(1); }
                if self.air_ticks != 0 { return Some(0x41); }
                if Self::cross_within(env, 8) { return Some(0x45); }
                if env.pad.pressed_within(button::SQUARE, 9).is_some() { return Some(0x4d); }
                self.stick_target(env, 1.0, 0);
                if 0.2 < self.stick_mag && 0.0 < self.target_speed { return Some(0x40); }
                if v.seq_b == 0x12 { self.set_anim(anim, 12.0, 0, 0); }
                None
            }
            0x40 => {
                if mud { return Some(0x4e); }
                if !v.blending() {
                    let (n2, n3) = (anim.frame_count(2) as u32, anim.frame_count(3) as u32);
                    if self.eff_len < 0.025 && v.seq_b != 2 && n2 != 0 && n3 != 0 {
                        self.set_anim(anim, 8.0, 2, ((v.frame_b as u32 * n2 / n3 + 1) % n2) as i32);
                    } else if 0.03 < self.eff_len && v.seq_b != 3 && n2 != 0 && n3 != 0 {
                        self.set_anim(anim, 8.0, 3, ((v.frame_b as u32 * n3 / n2 + 7) % n3) as i32);
                    }
                }
                let sp = if self.quick_turn != 0 { 0.05 } else { self.eff_len };
                let v = anim.view();
                if v.seq_b == 2 { self.anim_speed = sp * 90.0 } else if v.seq_b == 3 { self.anim_speed = sp * 20.0 }
                self.anim_speed = self.anim_speed.max(0.57);
                if Self::cross_within(env, 8) { return Some(0x45); }
                if env.pad.pressed_within(button::SQUARE, 9).is_some() { return Some(0x4d); }
                if 5 <= self.air_ticks && (0.8 < self.height || SLOPE_MAX < self.slope) { return Some(0x41); }
                if self.air_ticks == 0 && self.stick_mag < 0.17 { return Some(0x3f); }
                None
            }
            0x41 | 0x44 => {
                if self.state == 0x44 {
                    return (self.vel[2] < 0.0 && self.grounded_ticks != 0).then_some(0x3f);
                }
                if self.air_ticks == 0 {
                    self.disp[2] = self.disp[2].max(-0.15);
                    self.vel[2] = self.vel[2].max(-0.15);
                    if self.hp < 1 { return Some(0x43); }
                    return Some(if 0.5 < self.stick_mag && self.land_lag == 0 { 0x40 } else { 0x3f });
                }
                if self.hp < 1 { return None; }
                if self.ledge.found { return Some(0x46); }
                if Self::cross_within(env, 5) && 0.6 < self.height { return Some(0x4b); }
                None
            }
            0x42 => {
                if 0x20 <= self.timer && self.air_ticks != 0 && 1.0 < self.height { return Some(0x41); }
                if self.timer < 0x24 { return None; }
                if 0 < self.hp {
                    self.set_state(anim, env, 0x3f, false);
                    return None;
                }
                Some(0x43)
            }
            0x43 => {
                if v.flags & 2 != 0 { self.die(); }
                None
            }
            0x46 => (v.flags & 2 != 0).then_some(0x47),
            0x47 => {
                self.stick_target(env, 1.0, 0);
                if self.ledge_probe(env, self.pos, false).is_none() { return Some(0x41); }
                let back = 2.356_194_7 < rot_dist(self.target_yaw, self.rot[2]);
                if (0.5 < self.stick_mag && Self::cross_within(env, 9) && back) || env.pad.held & button::R1 != 0 {
                    self.set_state(anim, env, 0x41, true);
                    self.ledge.cooldown = 10;
                    return None;
                }
                if Self::cross_within(env, 9) { return Some(0x48); }
                if self.stick_mag <= 0.5 { return None; }
                for (dir, id) in [(PI / 2.0, 0x49), (-PI / 2.0, 0x4a)] {
                    let side = add_rot(self.rot[2], dir);
                    if rot_dist(self.target_yaw, side) < PI / 4.0 {
                        let (s, c) = side.sin_cos();
                        let p = add(self.pos, [c * 0.3, s * 0.3, 0.0]);
                        return self.ledge_probe(env, p, false).map(|_| id);
                    }
                }
                None
            }
            0x49 | 0x4a => {
                let dir = add_rot(self.rot[2], if self.state == 0x4a { -PI / 2.0 } else { PI / 2.0 });
                let (s, c) = dir.sin_cos();
                let p = add(self.pos, [c * 0.3, s * 0.3, 0.0]);
                self.stick_target(env, 1.0, 0);
                let keep = 0.5 <= self.stick_mag && rot_dist(self.target_yaw, dir) <= PI / 4.0 && self.ledge_probe(env, p, false).is_some();
                if !keep && v.flags & 2 != 0 { return Some(0x47); }
                if 0.5 < self.stick_mag && Self::cross_within(env, 9) && 2.356_194_7 < rot_dist(self.target_yaw, self.rot[2]) {
                    self.set_state(anim, env, 0x41, true);
                    self.ledge.cooldown = 0x28;
                    return None;
                }
                Self::cross_within(env, 9).then_some(0x48)
            }
            0x4b => {
                if 0x1f <= self.timer && env.pad.held & button::CROSS == 0 { return Some(0x41); }
                if self.ledge.found { return Some(0x46); }
                if self.air_ticks != 0 && 0.0 <= self.pos[2] - self.ground_z { return None; }
                if self.timer < 10 { self.glide_lag = 0xc - self.timer as i16; }
                if 0.3 < self.stick_mag { return Some(0x40); }
                if self.air_ticks != 0 { return None; }
                Some(0x3f)
            }
            0x4d => {
                if mud { return Some(0x4e); }
                let sw = SWINGS[self.melee.row.clamp(0, 8) as usize];
                if !v.blending() {
                    if (sw[6] as f32) < v.frame { self.set_state(anim, env, 0x3f, false); }
                    if sw[4] as f32 <= v.frame {
                        let n = (v.frame as i32 - sw[3]).clamp(1, 0xf);
                        if env.pad.pressed_within(button::SQUARE, n).is_some() { return Some(0x4d); }
                    }
                }
                let v = anim.view();
                if v.frame <= sw[5] as f32 || v.blending() { return None; }
                let n = (v.frame as i32 - sw[3]) * 2;
                Self::cross_within(env, n).then_some(0x45)
            }
            0x45 | 0x48 => self.trans_clank_jump(env, anim),
            _ => {
                // 0x4e.
                if 300 < self.timer { self.die(); }
                if mud { return None; }
                Some(if self.air_ticks == 0 { 0x3f } else { 0x41 })
            }
        }
    }

    /// `0x2cb650` case 0x45 / 0x48: Clank's jump and climb. The animation timing as the jump group's; the touchdown;
    /// a re-jump (✕ within 4 after landing); landed: the walk at speed toward the stick, else idle once 12 ticks
    /// down; high up and descending after 40 ticks: the fall; just above the ground descending: ■ → the punch; ✕
    /// within 5 more than 0.55 up: the glide; a ledge: the grab.
    fn trans_clank_jump(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let j = self.jump;
        if self.timer <= j.takeoff { return None; }
        let v = anim.view();
        if self.timer == j.takeoff + 1 {
            let eta = self.land_eta(j.g);
            self.anim_speed_for_ticks(&v, j.f_hold, eta, j.f334, -1.0);
            return None;
        }
        if j.landed == 0 && self.air_ticks == 0 && j.descending != 0 {
            self.jump.landed = 1;
            if v.frame < j.f_apex { self.set_anim(anim, 6.0, v.seq_b, j.f_land as i32 + 3); }
        }
        if self.jump.landed == 0 {
            if j.descending != 0 && (v.frame_b as f32) < j.f_land {
                let eta = (self.land_eta(j.g)).min(60.0);
                self.jump.land_eta = eta as i16;
                self.anim_speed_for_ticks(&v, j.f_apex, eta, j.f334, -1.0);
                if 1.8 < self.anim_speed { self.set_anim(anim, eta + 2.0, v.seq_b, j.f_land as i32 + 1); }
                self.anim_speed = self.anim_speed.clamp(0.2, 2.7);
                if v.frame.round() == j.f_apex.round() { self.anim_speed = 0.0; }
            }
        } else {
            self.anim_speed = 1.0;
        }
        let landed = self.jump.landed;
        if landed != 0 && self.land_lag == 0 && !self.surf.mud && Self::cross_within(env, 4) && self.air_ticks < 4 {
            self.stick_target(env, 1.0, 0);
            return Some(0x45);
        }
        if 1 < landed {
            self.cam_mode = 0;
            self.stick_target(env, 1.0, 0);
            let run = RUN_JUMP_SPEED < self.target_speed && 0.5 < self.stick_mag && !self.surf.mud && self.land_lag == 0 && rot_dist(self.target_yaw, self.rot[2]) < PI / 2.0;
            if run { self.set_state(anim, env, 0x40, true); } else if 0xc < landed { self.set_state(anim, env, 0x3f, true); }
        }
        if 2.0 < self.height {
            if j.descending != 0 && 0x28 < self.timer { return Some(0x41); }
        } else if j.descending != 0 && self.height < 0.2 && env.pad.pressed_within(button::SQUARE, 5).is_some() {
            return Some(0x4d);
        }
        if self.ledge.found { return Some(0x46); }
        if self.state != 0x48 && Self::cross_within(env, 5) && 0.55 < self.height { return Some(0x4b); }
        None
    }
}

/// `0x1a7db4`: the target speed above which a landing runs on.
const RUN_JUMP_SPEED: f32 = super::jump::RUN_JUMP_SPEED;
