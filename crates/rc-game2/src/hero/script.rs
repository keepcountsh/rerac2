//! The states the level's objects and scenes put Ratchet in (they leave them by setting another state): the held
//! states of group 9 (0x1c and 0x1e standing, 0x6f standing with the item away, 0x75 frozen in sequence 0xb, 0x31
//! hidden with the updates off, 0x37 turned by the scene), group 0x18 (0x60 / 0x61, still), the scripted walk 0x62
//! to a point (`0x2d4440(yaw, pos, flag)`) with its stop 0x63 and turn 0x64, and the fidgets 0x3c / 0x3d —
//! SetState (L00 `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`). The objects ask through
//! [`Hero::request_state`] and [`Hero::script_walk_to`], applied at the start of Ratchet's update (the original sets
//! them from the moby loop, which runs just before it).
//!
//! **The idle fidgets** (`0x2ca788`, from the idle 0): four entries (`0x25bb80`, 0x70 apart: the sequence `+0x48`,
//! the item it needs `+0x50`, the mean wait `+0x58` s, the rest after it `+0x5c` s, its counter `+0x60`): while the
//! idle sequence plays and the last fidget's 2 s are over, each tick a fidget whose rest is over is picked with the
//! chance 1/(wait·60) each (the fourth only just after walking with 1 health left; with 1 health left the fourth
//! alone, a third of the times the idle loops). The entries at boot: sequences 1 and 2 (10 s, rest 5.5), 0x50 (8 s,
//! 5), 0x57 (4 s, 2.5), all played on the idle (`+0x44` = 0; ≠ 0 would be the fidget state 0x3c).
//!
//! Not ported: the scene's turn of 0x37 (`0x31b5f0` on the held item), the stats counters, the 0x91 / 0x92 fidget
//! variants of the item `0x1a7398` (never 0x1e's table flag here).

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

/// `0x25bb80`: the fidgets (sequence, wait s, rest s).
const FIDGETS: [(u8, f32, f32); 4] = [(1, 10.0, 5.5), (2, 10.0, 5.5), (0x50, 8.0, 5.0), (0x57, 4.0, 2.5)];

/// A state an object asked for.
#[derive(Clone, Copy, Debug)]
pub struct StateReq {
    pub id: i32,
    pub play: bool,
}

/// The scripted walk's target (`0x18b6c0`, its yaw `0x18b6cc`, the flag `0x18b6d0`: stop on arrival).
#[derive(Clone, Copy, Debug, Default)]
pub struct WalkTo {
    pub pos: V3,
    pub yaw: f32,
    pub stop: bool,
}

impl Hero {
    /// An object's `0x2c4560(id, play)`.
    pub fn request_state(&mut self, id: i32, play: bool) { self.state_req = Some(StateReq { id, play }); }

    /// `0x2d4440(yaw, pos, flag)`: walk to `pos` and face `yaw` (0x62, unless already walking there).
    pub fn script_walk_to(&mut self, yaw: f32, pos: V3, stop: bool) {
        self.walk_to = WalkTo { pos, yaw, stop };
        if !(0x62..=0x64).contains(&self.state) { self.request_state(0x62, true); }
    }

    /// `0x2ca788`: a fidget to play now (module docs).
    pub fn fidget_pick(&mut self, env: &Env) -> Option<usize> {
        if self.fidget_gap != 0 { return None; }
        if self.hp == 1 {
            return None;
        }
        // The fidget 2 (sequence 0x50, which only those levels hold) only on levels 7 and 0x13, with nothing within 8
        // above him (the line from 8 up to him finds nothing; its `0x18c0db` ≠ 2 test is not ported).
        let pos = self.pos;
        let fidget2 = (env.level == 7 || env.level == 0x13) && env.line([pos[0], pos[1], pos[2] + 8.0], pos, 2).is_none();
        let ok: Vec<usize> = (0..4)
            .filter(|&i| self.fidget_rest[i] == 0 && FIDGETS[i].1 != 0.0)
            .filter(|&i| i != 2 || fidget2)
            .filter(|&i| i != 3 || (self.prev_group == 1 && self.timer <= 0x32 && self.hp <= 1))
            .collect();
        if ok.is_empty() { return None; }
        let mut sum: f32 = ok.iter().map(|&i| 1.0 / ((FIDGETS[i].1 * 60.0) as i32) as f32).sum();
        let r = self.rng.randf(0.0, 1.0);
        if sum <= r { return None; }
        let mut pick = ok[0];
        for &i in &ok {
            pick = i;
            sum -= 1.0 / ((FIDGETS[i].1 * 60.0) as i32) as f32;
            if sum < r { break; }
        }
        Some(pick)
    }

    /// The idle's fidget (`0x2cb650` case 0): picked while the idle sequence plays (not on ice), played on the idle.
    pub(super) fn idle_fidget(&mut self, anim: &mut dyn AnimCtl, env: &Env) {
        let v = anim.view();
        if v.seq_b == self.idle_seq() && !self.surf.ice {
            if self.hp == 1 {
                if self.fidget_rest[3] == 0 && v.flags & 2 != 0 && self.rng.randf(0.0, 1.0) < 0.33 { self.play_fidget(anim, 3); }
            } else if let Some(i) = self.fidget_pick(env) {
                self.play_fidget(anim, i);
            }
        } else if v.flags & 2 != 0 {
            let seq = self.idle_seq();
            self.set_anim(anim, if seq == 0x51 { 18.0 } else { -2.0 }, seq, 0);
        }
    }

    fn play_fidget(&mut self, anim: &mut dyn AnimCtl, i: usize) {
        self.fidget_gap = 0x78;
        self.fidget_rest[i] = (FIDGETS[i].2 * 60.0) as i32;
        self.set_anim(anim, 12.0, FIDGETS[i].0, 0);
    }

    pub(super) fn script_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        let idle = self.idle_seq();
        match id {
            0x1c | 0x1e | 0x6f => {
                self.group = 9;
                Entry::Anim(10.0, idle, 0)
            }
            0x75 => {
                self.group = 9;
                self.vel = [0.0; 3];
                Entry::Anim(-1.0, 0xb, 0)
            }
            0x31 => {
                self.group = 9;
                self.no_update = true;
                Entry::Anim(-1.0, idle, 0)
            }
            0x37 => {
                self.group = 9;
                self.set_anim(anim, 8.0, 0x44, 2);
                Entry::Done
            }
            0x60 | 0x61 => {
                self.group = 0x18;
                Entry::Anim(-1.0, idle, 0)
            }
            0x62 => {
                self.group = 1;
                self.speed = len2(self.eff);
                if !play { return Entry::Done; }
                Entry::Anim(8.0, (self.substate + 3) as u8, [8, 0x10][self.substate as usize & 1])
            }
            0x63 => {
                self.group = 1;
                self.speed = len2(self.eff);
                self.momentum = self.eff;
                let v = anim.view();
                let seq = if v.seq_a == 4 && 3.0 < self.anim_frame && self.anim_frame < 16.0 { 5 } else { 6 };
                if play { self.set_anim(anim, 9.0, seq, 4); }
                Entry::Done
            }
            0x64 => {
                self.group = 0;
                self.momentum = [0.0; 3];
                Entry::Anim(if idle == 0x51 { 18.0 } else { -1.0 }, idle, 0)
            }
            0x3d => {
                self.set_anim(anim, 9.0, idle, 0);
                let _ = env;
                Entry::Done
            }
            _ => Entry::Done,
        }
    }

    pub(super) fn phys_script(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            0x1c | 0x1e | 0x6f => self.phys_ground(env, anim),
            0x37 => {
                if anim.view().seq_b == 0x44 { self.turn_to(0.137, 0.25, 0.407_243, 0) } else { self.turn_to(1.0, 0.1, 0.407_243, 0) }
            }
            0x62 => {
                let d = sub(self.walk_to.pos, self.pos);
                self.stick_mag = 1.0;
                if 0.2 < len2(d) {
                    self.target_yaw = atan2(d[0], d[1]);
                    self.target_speed = if RUN_JUMP_SPEED <= len2(d) { RUN_SPEED } else { 0.9 } * DT;
                    if self.mode == 1 { self.target_speed = 0.015; }
                } else {
                    self.target_yaw = self.walk_to.yaw;
                    self.target_speed = 0.0;
                }
                let max = if self.mode == 3 { 0.093_084_25 } else { 0.130_899_7 };
                self.turn_to(0.03, 0.3, max, 0);
                self.speed_step(0.005, 0.005_555_556);
                self.set_planar_vel(f32::MAX);
                if self.mode == 0 { self.wall_check(env, true); }
                if self.air_ticks == 0 {
                    self.vel[2] -= 0.015;
                } else {
                    self.gravity_from(self.disp[2], 0.006_944_444_5);
                    self.edge_slide();
                }
                // Walking into the class 0xcb: a 100 hit (flags 0x10000) from a sphere at the head, the radius + 0.2
                // (the classes 0x1366 / 0x1367's own reactions `0x38a988` / `0x38c5e8` are theirs).
                let head = add(self.pos, [0.0, 0.0, 1.0]);
                self.hits_out.push(super::HeroHit { centre: head, r: self.cap_radius + 0.2, damage: 100.0, flags: 0x10000, dir: [0.0; 3], joints: None });
            }
            0x63 => {
                let d = sub(self.walk_to.pos, self.pos);
                if dot(d, self.momentum) < 0.0 { self.momentum = [0.0; 3]; }
                self.vel = [0.0; 3];
                self.momentum_decay(0.007_5);
                self.wall_check(env, false);
                if self.air_ticks == 0 { self.vel[2] -= 0.015 } else { self.gravity_from(self.disp[2], 0.006_944_444_5) }
            }
            0x3c | 0x64 | 0x60 | 0x61 => self.vel = [0.0; 3],
            _ => {}
        }
    }

    pub(super) fn trans_script(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let v = anim.view();
        match self.state {
            0x62 => {
                if 0.2 <= dist2(self.pos, self.walk_to.pos) {
                    self.walk_anim(anim);
                    return None;
                }
                if self.eff_len < 0.045 {
                    if rot_dist(self.rot[2], self.walk_to.yaw) < 0.034_906_585 { return Some(0x64); }
                }
                if self.mode == 0 || self.eff_len < 0.045 { return None; }
                Some(0x63)
            }
            0x63 => (v.flags & 2 != 0).then_some(0x62),
            0x64 => {
                if v.flags & 2 != 0 || self.walk_to.stop { self.set_state(anim, env, 0, true); }
                if dist2(self.pos, self.walk_to.pos) <= 0.2 { return None; }
                Some(0x62)
            }
            0x3c => {
                if env.pad.held & button::L1 != 0 { return Some(1); }
                if self.air_ticks == 0 && env.pad.held & (button::CROSS | button::R1) == 0 {
                    self.run_target(env);
                    if self.target_speed <= 0.0 {
                        if v.flags & 2 == 0 { return None; }
                        return Some(0);
                    }
                }
                Some(0x3d)
            }
            0x3d => {
                if self.timer < 9 { return None; }
                if self.air_ticks != 0 { return Some(6); }
                if self.try_jump(anim, env, 7, false) { return None; }
                if env.pad.held & button::R1 != 0 { return Some(4); }
                self.run_target(env);
                Some(if 0.0 < self.target_speed { 2 } else { 0 })
            }
            _ => None,
        }
    }
}

use super::jump::RUN_JUMP_SPEED;
