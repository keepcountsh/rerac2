//! Clank's packs: the glide 8 (group 5; SetState L00 `0x2c4560` case 8, physics `0x2ba8e0` case 8, transitions
//! `0x2cb650` case 8). Holding ✕ in a jump with Clank on the back (`trans_jump`) opens the Heli-Pack's rotors (or the
//! Thruster-Pack's jets): the speed eases toward 0.05 (Thruster-Pack 1/12) along the stick (turn 0.025 / 0.3 /
//! 0.2093, speed 1/240 up, 1/514 down), and the fall springs toward 0.72 of that speed (0.025, 0.3, at most 0.1417).
//! Letting go after 31 ticks drops him into the fall 6; a wall in reach within 8 ticks from a jump gives the wall
//! jump 0x11; on landing he crouches (R1), stands or walks on as after a jump.
//!
//! The pack jumps themselves (the Heli-Pack's long jump 10 and high jump 0xf, the Thruster-Pack's 0x10 and 0xd)
//! are in the jump group (`jump.rs`).
//!
//! Not ported: the rotor and jet sounds (`0x2b9cb8`, `0x3602f0`), Clank's own animation (`0x323d10`, `0x323e58`), the
//! updraft boost `0x18c30f`.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::hero::anim::AnimCtl;
use crate::pad::button;

impl Hero {
    /// `0x2b6220(k)`: the motion (displacement, effective velocity, velocity, momentum, speed) scaled by `k`.
    pub fn scale_motion(&mut self, k: f32) {
        self.disp = scale(self.disp, k);
        self.eff = scale(self.eff, k);
        self.vel = scale(self.vel, k);
        self.momentum = scale(self.momentum, k);
        self.speed *= k;
    }

    pub(super) fn pack_entry(&mut self, _anim: &mut dyn AnimCtl, _env: &Env, _id: i32, _play: bool) -> Entry {
        self.group = 5;
        self.cam_mode = 0;
        if 0.075 < self.eff_len { self.scale_motion(0.075 / self.eff_len); }
        self.glide_sink = 0.0;
        self.vel = self.disp;
        Entry::Anim(-2.0, 0x13, 0)
    }

    pub(super) fn phys_pack(&mut self, env: &Env, _anim: &mut dyn AnimCtl) {
        let speed = if env.pack == 3 { 1.0 / 12.0 } else { 0.05 };
        let sink = if self.glide_sink != 0.0 { self.glide_sink } else { 0.72 };
        let mut vz = self.vel[2];
        self.stick_target(env, speed, 0);
        self.turn_to(0.025, 0.3, 0.209_348_8, 0);
        self.speed_step(0.004_166_667, 0.001_944_444_5);
        self.set_planar_vel(f32::MAX);
        self.vel[2] = 0.0;
        let mut v = self.glide_vz;
        spring(-speed * sink, 0.025, 0.3, 0.141_666_67, &mut vz, &mut v);
        self.glide_vz = v;
        self.vel[2] = vz;
        if self.pos[2] < self.jump.ref_z { approach(self.pos[2], 1.0 / 15.0, &mut self.jump.ref_z); }
        self.wall_probe(env);
    }

    pub(super) fn trans_pack(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        if self.wall.timer != 0 && self.timer <= 8 {
            return matches!(self.prev_state, 7 | 9 | 0xe | 0x11).then_some(0x11);
        }
        let held = env.pad.held & button::CROSS != 0;
        if 0x1f <= self.timer && !held { return Some(6); }
        if self.ledge.found { return Some(0x17); }
        if self.air_ticks != 0 && 0.4 <= self.height { return None; }
        if self.timer < 10 { self.glide_lag = 0xc - self.timer as i16; }
        self.stick_target(env, 1.0, 0);
        if self.prev_state == 0x10 && self.timer < 4 {
            if self.set_state(anim, env, 3, false) {
                self.set_anim(anim, -1.0, 5, 0);
                self.land_lag = 0x22;
            }
            return None;
        }
        if env.pad.held & button::R1 != 0 {
            self.set_state(anim, env, 4, true);
            return None;
        }
        const RUN_JUMP_SPEED: f32 = 0.82;
        let ahead = rot_dist(self.target_yaw, self.rot[2]) < std::f32::consts::FRAC_PI_2;
        if self.target_speed <= RUN_JUMP_SPEED {
            if self.air_ticks == 0 {
                if self.set_state(anim, env, 0, false) {
                    let seq = self.idle_seq();
                    self.set_anim(anim, -2.0, seq, 0);
                }
            } else if self.set_state(anim, env, 0, false) {
                self.set_anim(anim, 9.0, 7, 0x17);
            }
            return None;
        }
        if self.surf.ice || self.surf.wading { return Some(2); }
        self.set_state(anim, env, 2, false);
        if self.state == 2 {
            self.substate = 1;
            if ahead {
                self.set_anim(anim, 8.0, 4, 2);
                self.walk_gate = 1;
            } else {
                // `0x1a7ddc`: the walk's landing frame.
                self.set_anim(anim, 12.0, 4, 0x10);
            }
        }
        None
    }
}
