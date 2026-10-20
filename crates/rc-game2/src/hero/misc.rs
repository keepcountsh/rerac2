//! The floor and knock states: the stagger 0x26 / 0x2f (group 8), the chute slide 0x30 (group 0x10), the mud 0x76
//! (group 2) and the wall bonk 0x77 (group 10) — their SetState entries (L00 `0x2c4560`), physics (`0x2ba8e0`) and
//! transitions (`0x2cb650`).
//!
//! * 0x2f: knocked back on the spot: the momentum is the displacement, the speed falls by 1/811 a tick; after 7
//!   ticks (and the item button let go) back to idle once the stagger sequence 0x36 is over. 0x26 is its scripted
//!   twin (no physics, the script ends it).
//! * 0x30: the chute slide (surface 4, or a chute moby `0x39f588` holding Ratchet for `0x18a020` ticks): the facing
//!   turns down the chute (0.033, 0.3, 0.0697), the momentum carries, the fall at least 1/90; it ends a metre off the
//!   chute or once it lets go (idle, the carry as the momentum).
//! * 0x76: mud: sliding down the ground's slope (0.0917, the air acceleration 1/144); out of the mud: idle (in the air
//!   the fall); 5 seconds in it: the death's bookkeeping.
//! * 0x77: a pack jump into a wall: thrown back off it at 0.15, easing to 0 by 1/300, until the sequence 0x29 ends.
//!
//! Not ported: the chute's sparks and sounds (`0x33a058`, `0x2b9cb8`), the bank's lean of the body.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;

impl Hero {
    pub(super) fn misc_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        match id {
            0x26 => {
                self.group = 8;
                Entry::Anim(10.0, self.idle_seq(), 0)
            }
            0x2f => {
                self.group = 8;
                self.momentum = self.disp;
                let seq = self.idle_seq();
                self.set_anim(anim, 5.0, seq, 0);
                self.stagger = 7;
                Entry::Done
            }
            0x30 => {
                self.group = 0x10;
                self.cam_mode = 3;
                (self.slide_yaw, self.slide_bank, self.slide_vel, self.slide_speed) = (0.0, 0.0, [0.0; 2], 0.0);
                self.momentum = self.disp;
                Entry::Anim(15.0, 0x5d, 0)
            }
            0x76 => {
                self.group = 2;
                self.vel = self.eff;
                Entry::Anim(12.0, 0xb, 0)
            }
            _ => {
                // 0x77.
                self.group = 10;
                self.bonk_yaw = add_rot(self.rot[2], PI);
                self.speed = 0.15;
                if play {
                    self.set_anim(anim, 8.0, 0x29, 5);
                }
                Entry::Done
            }
        }
    }

    pub(super) fn phys_misc(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            0x2f => {
                if self.surf.ice { self.drag(0.7, 0.0) } else { self.drag(0.5, 1.0 / 30.0) }
                self.edge_brake(env, 3.7, 0.0);
                self.stick_target(env, 1.0, 0);
                self.speed_step(0.001_232_998, 0.001_232_998);
                self.vel = [0.0; 3];
                self.momentum_decay(0.001_232_998);
                if self.air_ticks == 0 {
                    self.vel[2] -= 0.015;
                } else {
                    self.gravity_from(self.disp[2], 0.006_944_444_5);
                    self.edge_slide();
                }
                let v = anim.view();
                if v.flags & 2 != 0 && v.seq_a == 0x36 {
                    let seq = self.idle_seq();
                    self.set_anim(anim, 5.0, seq, 0);
                }
            }
            0x30 => {
                let old = self.rot[2];
                self.target_yaw = self.slide_yaw;
                self.turn_to(0.033_000_003, 0.3, 0.069_813_18, 0);
                let d = rot_dist(self.rot[2], old).min(0.023_271_061) * 9.0;
                self.vel = [0.0; 3];
                self.momentum_decay(0.001_944_444_5);
                self.vel[2] = (self.disp[2] - 0.013_888_889).min(-0.011_111_111);
                // The body banks into the turn and with the chute.
                let (mut a, mut v) = (self.rot[0], self.slide_vel[0]);
                turn_spring(d * 0.5, 0.007_000_000_6, 0.17, 0.020_362_3, &mut a, &mut v, 0);
                (self.rot[0], self.slide_vel[0]) = (a, v);
                let (mut a, mut v) = (self.rot[1], self.slide_vel[1]);
                turn_spring(-self.slide_bank, 0.035, 0.3, 0.032_000_002, &mut a, &mut v, 0);
                (self.rot[1], self.slide_vel[1]) = (a, v);
            }
            0x76 => {
                self.target_yaw = self.ground_yaw;
                self.target_speed = 0.091_666_67;
                self.air_accel(0.006_944_444_5);
                self.gravity_from(self.eff_v[2], 0.005);
            }
            _ => {
                // 0x77: off the wall.
                approach(0.0, 0.003_333_333_5, &mut self.speed);
                let (s, c) = self.bonk_yaw.sin_cos();
                self.vel[0] = c * self.speed;
                self.vel[1] = s * self.speed;
                self.vel[2] = if 0.0 < self.height { self.eff[2] - 0.006_944_444_5 } else { 0.0 };
                self.target_yaw = add_rot(self.bonk_yaw, PI);
                self.turn_to(0.05, 0.2, 0.282_162_3, 0);
            }
        }
    }

    pub(super) fn trans_misc(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        match self.state {
            0x2f => {
                if env.pad.held & self.items.trigger != 0 || self.stagger != 0 { return None; }
                if anim.view().seq_b != 0x36 { return Some(0); }
                self.set_state(anim, env, 0, false);
                None
            }
            0x30 => {
                if (!self.surf.slide && self.slide_hold == 0) || 1.0 < self.height {
                    if self.set_state(anim, env, 0, false) {
                        let seq = self.idle_seq();
                        self.set_anim(anim, 17.0, seq, 0);
                        self.momentum = self.carry;
                    }
                }
                None
            }
            0x76 => {
                if 300 < self.timer { self.die(); }
                if self.surf.mud { return None; }
                Some(if self.air_ticks == 0 { 0 } else { 6 })
            }
            _ => (anim.view().flags & 2 != 0).then_some(0),
        }
    }
}
