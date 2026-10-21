//! The gear states: the Charge Boots' dash 0x7b (group 0x1c), the updraft's float 0x82 (group 0x1e) and the rides
//! 0x85 / 0x86 (group 0x20) — SetState (L00 `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`), with the
//! Charge Boots' trigger (the end of `0x2c96d0`).
//!
//! * 0x7b: R1 tapped twice within 30 ticks with the boots on (`0x1a7b36`, the item `0x1a739c` = 0x36), standing,
//!   crouching or just landed: the speed eases to 0.192 (1/36 up, 1/240 down) along a facing the stick turns (by
//!   its smoothed sideways part `0x2a9db0`, 0.0204 a tick at full tilt; a tenth in the first second), the feet
//!   springing 0.17 above the ground; a 0.57 sphere 3 ticks ahead rams (1, flags 0x10000). It ends when R1 is let go
//!   after a second (the stop 3, a 30-tick lag), on a magnetic walkway, or against a wall it cannot slide along
//!   (the bonk 0x77).
//! * 0x82: an updraft (`0x18a012` set by its object, or the float flag `0x18c30f` from idle) lifts with 50 (or the
//!   object's `0x18a3d8`) of energy, spent by 0.02 a tick (✕ held: 0.1 and 0.05 up a tick; a ✕ press 0.5); the fall
//!   1/133, the vertical speed −0.2..0.1, cushioned near the ground; the stick drifts it (1/1800, at most 1/720).
//!   Spent: the fall (on level 0xd below 69: the death fall 0x74).
//! * 0x85: seated on an object (`0x18b650`): its position and angles; △ after 4 ticks leaves it. 0x86: flown by an
//!   object at 0.217 along the facing, the stick rolling it.
//!
//! Not ported: the boots' and the updraft's sounds and screen effects (`0x2fc520`), the dash's lean, 0x86's probe
//! `0x2b9008`.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

/// The gear block.
#[derive(Clone, Copy, Debug, Default)]
pub struct GearBlock {
    /// `$gp − 0x732c`: the Charge Boots' double-tap window; `0x18a810..`: the smoothed stick; `0x18a820`: the hover
    /// spring's velocity.
    pub tap: i32,
    pub stick: [f32; 2],
    pub hover_v: f32,
    /// `0x18a012`: an updraft's ticks (its object sets them); `0x18c30f`: floating; `0x18a3d0` / `0x18a3d8`: the
    /// lift's energy and the object's; `0x18a3de` / `0x18a3d4`: the cushion's ticks and spring velocity.
    pub updraft: i16,
    pub floating: bool,
    pub lift: f32,
    pub lift_store: f32,
    pub cushion: i16,
    pub cushion_v: f32,
    /// `0x18b650`: the object ridden (its position and angles, kept by the engine); `0x18a5e0` / `0x18a5e4`: the
    /// flight's roll and its velocity.
    pub ride: Option<(V3, V3)>,
    pub roll: f32,
    pub roll_v: f32,
}

impl Hero {
    /// The end of `0x2c96d0`: the Charge Boots' double tap.
    pub fn charge_trigger(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        if !env.charge_boots || self.state == 0x1d || self.state == 1 { return false; }
        if 0 < self.gear.tap { self.gear.tap -= 1; }
        let ok_group = self.group < 2 || self.group == 0xc || ((self.group == 2 || self.group == 4) && self.grounded_ticks != 0);
        let tap = env.pad.pressed & button::R1 != 0 && env.pad.held & button::R1 != 0;
        let set = self.gear.tap != 0 && ok_group && tap && self.set_state(anim, env, 0x7b, true);
        if env.pad.pressed & button::R1 != 0 { self.gear.tap = 0x1e; }
        set
    }

    pub(super) fn gear_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.cam_mode = 0;
        match id {
            0x7b => {
                self.group = 0x1c;
                self.vel = self.disp;
                Entry::Anim(-2.0, 0x8b, 0)
            }
            0x82 => {
                self.group = 0x1e;
                if self.prev_state != 0x16 {
                    self.gear.lift = 50.0;
                    if self.gear.lift_store != 0.0 {
                        self.gear.lift = self.gear.lift_store;
                        self.gear.lift_store = 0.0;
                    }
                }
                self.gear.floating = true;
                self.gear.cushion = 0;
                self.vel = self.disp;
                Entry::Anim(-2.0, 0x13, 0)
            }
            0x85 => {
                self.group = 0x20;
                self.cam_mode = 8;
                self.no_menu = 1;
                if play { self.set_anim(anim, 5.0, 0x4f, 0); }
                if let Some((p, r)) = self.gear.ride {
                    self.pos = p;
                    self.rot[2] = r[2];
                }
                self.vel = [0.0; 3];
                Entry::Done
            }
            _ => {
                // 0x86.
                self.group = 0x20;
                self.cam_mode = 8;
                self.no_menu = 1;
                Entry::Anim(5.0, 0x4f, 0)
            }
        }
    }

    pub(super) fn phys_gear(&mut self, env: &Env, _anim: &mut dyn AnimCtl) {
        match self.state {
            0x7b => {
                self.stick_target(env, 1.0, 0);
                self.target_speed = 0.191_666_68;
                // `0x2a9db0(0.17, &s, 0x18a810)`: the stick eased by 0.17 a tick.
                approach(-self.stick[0], 0.17, &mut self.gear.stick[0]);
                approach(self.stick[1], 0.17, &mut self.gear.stick[1]);
                let mut turn = self.stick_mag * self.gear.stick[0].clamp(-1.0, 1.0) * 0.020_362_178;
                if self.timer < 0x3c {
                    turn *= 0.1;
                    self.target_speed = 0.5;
                }
                if 0.2 < self.stick_mag { self.rot[2] = add_rot(self.rot[2], turn); }
                self.speed_step(0.027_777_778, 0.004_166_667);
                self.set_planar_vel(f32::MAX);
                let (mut z, mut v) = (self.pos[2], self.gear.hover_v);
                spring(self.ground_z + 0.17, 0.04, 0.3, 0.125, &mut z, &mut v);
                (self.pos[2], self.gear.hover_v) = (z, v);
                let at = add(add(self.pos, [0.0, 0.0, 1.0]), scale(self.disp, 3.0));
                self.hits_out.push(super::HeroHit { centre: at, r: 0.57, damage: 1.0, flags: 0x10000, dir: scale(self.disp, 7.0), joints: None });
            }
            0x82 => {
                let g = &mut self.gear;
                if env.pad.held & button::CROSS != 0 && g.lift != 0.0 {
                    approach(0.0, 0.1, &mut g.lift);
                    self.vel[2] += 0.05;
                } else {
                    approach(0.0, 0.02, &mut g.lift);
                }
                if env.pad.pressed & button::CROSS != 0 { approach(0.0, 0.5, &mut self.gear.lift); }
                self.vel[2] = (self.vel[2] - 0.007_5).clamp(-0.2, 0.1);
                let vz = self.vel[2];
                if 25.0 < self.gear.lift && self.gear.cushion <= 0x59 && vz < 0.0 && self.height < 1.2 {
                    let k = ((self.height - 0.25) * 1.052_631_5).max(0.0);
                    self.gear.cushion += 1;
                    self.vel[2] = vz * k;
                    if self.height < 0.25 {
                        let (mut z, mut v) = (self.pos[2], self.gear.cushion_v);
                        spring(self.ground_z + 0.25, 0.027, 0.3, 1.0 / 12.0, &mut z, &mut v);
                        (self.pos[2], self.gear.cushion_v) = (z, v);
                    }
                } else if 0.0 < vz {
                    self.gear.cushion = 0;
                }
                self.stick_target(env, self.stick_mag * 0.2, 0);
                if self.stick_mag < 0.2 { self.target_yaw = self.rot[2]; }
                self.turn_to(0.027, 0.3, self.stick_mag * 0.087_266_47, 0);
                self.air_accel(if self.stick_mag < 0.2 { 0.000_555_555_6 } else { 0.001_388_889 });
            }
            0x85 => {
                if let Some((p, r)) = self.gear.ride {
                    self.pos = p;
                    self.rot = r;
                }
            }
            _ => {
                // 0x86: along the facing at 0.217, the roll by the stick.
                let t = if 0.2 < self.stick_mag { -self.stick[1] * 0.75 } else { 0.0 };
                let (mut r, mut v) = (self.gear.roll, self.gear.roll_v);
                spring(t, 0.007, 0.3, 0.03, &mut r, &mut v);
                (self.gear.roll, self.gear.roll_v) = (r, v);
                self.speed = 0.216_666_68;
                self.vel = [self.rows[0][0] * self.speed, self.rows[0][1] * self.speed, 0.0];
                let ty = self.rot[2] + self.gear.roll;
                self.target_yaw = ty;
                self.turn_to(0.02, 0.3, 0.034_906_585, 0);
            }
        }
    }

    pub(super) fn trans_gear(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        match self.state {
            0x7b => {
                let stop = self.surf.boots != 0 || (env.pad.held & button::R1 == 0 && 0x3c < self.timer);
                if stop {
                    self.sounds.push((9, 0));
                    self.land_lag = 0x1e;
                    self.set_state(anim, env, 3, false);
                    self.set_anim(anim, 11.0, 6, 4);
                    return None;
                }
                if self.cap_hit == 0 { return None; }
                if len(self.vel) * 0.75 <= len(self.disp) && self.ahead_line(env, 1.0, 1.25).is_none() { return None; }
                Some(0x77)
            }
            0x82 => {
                if self.vel[2] < 0.0 && self.grounded_ticks != 0 && (self.ground_moby.is_none() || self.ground_class == 0x8e2) && self.set_state(anim, env, 0, false) {
                    self.gear.floating = false;
                    self.set_anim(anim, 9.0, 7, 0x17);
                    return None;
                }
                if self.gear.lift != 0.0 { return None; }
                Some(if env.level == 0xd && self.ground_z < 69.0 { 0x74 } else { 6 })
            }
            0x85 => {
                if 4 <= self.timer && env.pad.pressed & button::TRIANGLE != 0 {
                    self.set_state(anim, env, if self.mode == 1 { 0x3f } else { 0 }, true);
                }
                None
            }
            _ => None,
        }
    }
}
