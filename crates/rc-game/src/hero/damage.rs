//! Damage and death: the hit intake (L00 `0x2b2dd0`, the transitions' prologue: Ratchet's hit record with flag 1 or
//! 8, the damage `0x2a4ef8` scaled by the armour, the knock-back `0x2b2d60`, the invincibility `0x189ffc`), the hurt
//! 0x16 and the heavy knock-back 0x8c (group 7), the deaths 0x39 (on the ground), 0x74 (falling out of the level) and
//! 0x79, and the death's bookkeeping `0x2b31f0` (the respawn request; the game's respawn is the engine's).
//!
//! The other deaths and sinks: the quicksand 0x65 (group 0x19: sinking 1/240 a tick while less than half a metre per
//! earlier sink is above him, else 1/60; ✕ after 10 + 7·n ticks jumps out with 0x66 for the first three; under 1.2
//! below the sand or after 171 ticks: dead) and its death 0x78 (151 ticks), the lava death 0x7a (the vehicle's), the
//! drowning 0x7c (in a deep liquid: the speed ×0.92, dead after 221 ticks), the electric death 0x7d, the burns 0x67 /
//! 0x7f (the momentum eases out by 1/900, the fall springs to −0.025).
//!
//! Not ported: the water hurts 0x72 / 0x73 (with the water states), Clank's (0x42, 0x43), the per-level death
//! counters, the hurt sounds and voice (`0x2d6cc8(0x12)`, `0x2b9c90(0x14)`), the drowning's bubbles moby (class
//! 0x52a), the burns' smoke (`0x3378b0`), the gravity frames.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::moby::hits::HitRecord;
use crate::hero::anim::AnimCtl;

/// `0x1a7f30`: the damage factor by the armour (`0x1a7a20`, 0..4).
const ARMOUR: [f32; 5] = [1.0, 0.67, 0.44, 0.3, 0.1];
/// `0x25daa0`: the hurt sequences' playback rates, cycled per hit (`0x18aa54`).
const HURT_RATES: [f32; 8] = [0.15, 0.29, 0.42, 0.53, 0.63, 0.72, 0.8, 0.87];

/// What the intake reads of the hit (Ratchet's record and its source).
#[derive(Clone, Copy, Debug)]
pub struct HitIn {
    pub rec: HitRecord,
    /// The source moby's position and o_class.
    pub source_pos: Option<V3>,
    pub source_class: i16,
}

/// The damage block.
#[derive(Clone, Copy, Debug, Default)]
pub struct DamageBlock {
    /// `0x189ffc` (s16): the ticks Ratchet takes no hit.
    pub invincible: i16,
    /// `0x1a7a20`: the armour.
    pub armour: i16,
    /// `0x18aa54`: the hurt counter (the playback rate's index).
    pub hurts: i32,
    /// `0x18c0e1`: died (the respawn request, `0x2e7c90(0x10)`).
    pub dead: bool,
    /// The hit record was read this tick: the moby world clears it.
    pub took: bool,
    /// `0x18a154`: the height the death 0x79 started at.
    pub death_z: f32,
    /// `0x18a85c`: the burns' fall speed.
    pub fall_v: f32,
    /// `0x18c2bc`: the moby whose hit he took this tick (the enemies read it).
    pub hit_by: Option<usize>,
}

impl Hero {
    /// `0x2a4ef8(n)`: n health off (Ratchet: × the armour's factor, rounded, at least 1); the health widget shows
    /// (`0x30d9f8(0x168)`, `show_health`).
    pub fn take_damage(&mut self, n: i32) {
        if n == 0 { return; }
        let mut n = n;
        if self.mode == 0 && 0 < n {
            let k = ARMOUR[self.damage.armour.clamp(0, 4) as usize];
            n = ((n as f32 * k + 0.5) as i32).max(1);
        }
        self.hp = (self.hp - n).max(0);
        self.show_health = Some(0x168);
    }

    /// `0x2b2dd0(1)`: a hit on Ratchet's moby (flag 1 or 8) outside the deaths and hurts, while not invincible:
    /// its damage, the hurt state (0x8c with flag 8; in the look stance only the damage) and the knock-back away
    /// from the source (0.055 out, 0.028 up). Returns whether a state was set.
    pub fn hit_intake(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        if matches!(self.group, 0x14 | 7) || self.state == 0x31 || self.damage.invincible != 0 { return false; }
        self.damage.hit_by = None;
        let Some(h) = env.hit else { return false };
        if h.rec.tmpl.flags & 9 == 0 { return false; }
        self.damage.took = true;
        self.damage.hit_by = h.rec.tmpl.source;
        let rec = h.rec;
        // The knock-back direction: the template's (flag 1 of its count word) or away from the source.
        let mut dir = match (rec.tmpl.count & 1 != 0, h.source_pos) {
            (true, _) => [rec.tmpl.dir[0], rec.tmpl.dir[1], rec.tmpl.dir[2]],
            (false, Some(p)) => sub(self.pos, p),
            (false, None) => [-1.0, 0.0, 0.0],
        };
        let along = rec.tmpl.count & 1 != 0 && (rec.tmpl.dir[3] - 5627.925).abs() < 1e-3;
        if h.source_class == 0x4eb || h.source_class == 0x558 {
            self.set_state(anim, env, 0x7d, true);
            return true;
        }
        self.pending_damage = Some(rec.tmpl.damage as i32);
        if self.state == 1 || self.state == 0x1d {
            if rec.tmpl.flags & 8 == 0 {
                self.pending_damage = None;
                self.take_damage(rec.tmpl.damage as i32);
                if self.hp == 0 { self.set_state(anim, env, 0x16, true); }
                self.damage.invincible = 0x4d;
                return false;
            }
        }
        let id = if self.group == 0x12 { 0x72 } else if self.group == 0x11 { 0x73 } else if rec.tmpl.flags & 8 != 0 { 0x8c } else { 0x16 };
        self.set_state(anim, env, id, true);
        if self.state != 0x8c {
            // `0x2b2d60(0.055, 0.0283, dir, along)`: the direction's horizontal part set to 0.055 and 0.0283 up (along
            // the template: its full length to 0.055, its up part to 0.0283).
            let up = 0.028_333_334;
            if along {
                dir = set_len(dir, 0.055);
                dir[2] = up;
            } else {
                dir[2] = 0.0;
                dir = set_len2(dir, 0.055);
                dir[2] = up;
            }
            self.vel = add(self.vel, dir);
            self.disp = add(self.disp, dir);
        }
        true
    }

    /// `0x2c4560` for the hurts and deaths.
    pub(super) fn damage_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        match id {
            0x16 | 0x8c => {
                self.group = 7;
                self.cam_mode = 0;
                // `0x2b6220(0.5)`: the motion halved.
                self.disp = scale(self.disp, 0.5);
                self.eff = scale(self.eff, 0.5);
                self.vel = scale(self.vel, 0.5);
                self.momentum = scale(self.momentum, 0.5);
                self.speed *= 0.5;
                if id == 0x8c { self.items.no_use = 1; }
                if id == 0x16 {
                    self.vel = self.eff;
                    if 0.116_666_7 < len(self.vel) { self.vel = set_len(self.vel, 0.116_666_7); }
                }
                let dmg = self.pending_damage.take().unwrap_or(1);
                self.take_damage(dmg);
                self.damage.invincible = 0x4d;
                if play {
                    let (seq, frame) = if id == 0x16 { (0x10, 3) } else { (0x96, 1) };
                    self.set_anim(anim, -3.0, seq, frame);
                    // The moby's blend fraction `+0x44` starts at the table's value for this hit (not settable here).
                    let _ = HURT_RATES[self.damage.hurts.rem_euclid(HURT_RATES.len() as i32) as usize];
                    self.damage.hurts += 1;
                }
                Entry::Done
            }
            0x39 => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.momentum = self.eff;
                if play { Entry::Anim(12.0, 0x45, 0) } else { Entry::Done }
            }
            0x74 => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.fall_g = 0.006_111_111_5;
                self.fall_dec = 0.001_944_444_5;
                self.vel = self.eff;
                if play {
                    if 1.75 < self.height {
                        self.set_anim(anim, 12.0, 0xb, 0);
                        self.substate = 1;
                        return Entry::Done;
                    }
                    return Entry::Anim(10.0, 10, 0);
                }
                Entry::Done
            }
            0x65 | 0x78 => {
                self.group = 0x19;
                self.cam_mode = 0;
                if id == 0x65 {
                    self.hot_hops += 1;
                    self.target_speed = 0.0;
                    self.speed = 0.0;
                }
                if play { Entry::Anim(8.0, 0x6a, 0) } else { Entry::Done }
            }
            0x67 | 0x7f => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.momentum = self.eff;
                self.damage.fall_v = 0.0;
                if play { Entry::Anim(10.0, if id == 0x67 { 0x58 } else { 0x79 }, 0) } else { Entry::Done }
            }
            0x7a => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.surf.sand_z = self.ground_z;
                self.hp = 0;
                if play { Entry::Anim(8.0, 0x11, 0) } else { Entry::Done }
            }
            0x7c => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.hp = 0;
                if play { Entry::Anim(11.0, 0x6d, 0) } else { Entry::Done }
            }
            0x7d => {
                self.group = 0x14;
                self.cam_mode = 0;
                self.hp = 0;
                self.momentum = self.eff;
                if 0.058_333_337 < len(self.momentum) { self.momentum = set_len(self.momentum, 0.058_333_337); }
                if play { Entry::Anim(12.0, 0x75, 0) } else { Entry::Done }
            }
            _ => {
                // 0x79.
                self.group = 0x14;
                self.cam_mode = 0;
                self.damage.death_z = self.ground_z;
                self.hp = 0;
                if play { Entry::Anim(8.0, 0x6a, 0) } else { Entry::Done }
            }
        }
    }

    /// `0x2ba8e0` for the hurts and deaths.
    pub(super) fn phys_damage(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        match self.state {
            0x16 => {
                // The horizontal speed falls by 1/900 a tick (1/360 on the ground after 10 ticks).
                let mut s = len2(self.vel);
                s -= if self.air_ticks == 0 && 0xb <= self.timer { 0.002_777_777_8 } else { 0.001_111_111_1 };
                self.vel = set_len2(self.vel, s.max(0.0));
                self.gravity(0.004);
            }
            0x8c => {
                self.vel = [0.0, 0.0, -0.383_333_36];
            }
            0x39 | 0x7d => self.phys_ground(env, anim),
            0x65 => {
                self.vel = [0.0; 3];
                self.vel[2] = if self.hot_hops as f32 * 0.5 < self.surf.sand_z - self.pos[2] { -0.004_166_667 } else { -0.016_666_668 };
            }
            0x78 => self.vel = [0.0, 0.0, -0.016_666_668],
            0x7a => self.vel = [0.0, 0.0, -0.005_833_333_4],
            0x7c => {
                self.vel[2] = 0.0;
                let l = len2(self.vel);
                self.vel = set_len2(self.vel, l * 0.92);
            }
            0x67 | 0x7f => {
                let mut l = len(self.momentum);
                approach(0.0, 0.001_111_111_2, &mut l);
                self.momentum = set_len(self.momentum, l);
                self.vel = self.momentum;
                if 0.0 < self.damage.fall_v { self.damage.fall_v = 0.0; }
                approach(-0.025, 0.000_416_666_67, &mut self.damage.fall_v);
                self.vel[2] = self.damage.fall_v;
            }
            0x74 => {
                self.vel = scale(self.vel, 0.9);
                self.vel[2] = (self.vel[2] - self.fall_g).max(-0.833_333_4);
            }
            _ => {
                self.vel = [0.0, 0.0, -0.016_666_668];
            }
        }
    }

    /// `0x2cb650` for the hurts and deaths.
    pub(super) fn trans_damage(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let v = anim.view();
        match self.state {
            0x16 => {
                if 12.5 < v.frame && self.air_ticks != 0 && 1.0 < self.height { return Some(6); }
                if self.timer < 0x1c { return None; }
                if self.hp < 1 { return Some(0x39); }
                self.set_state(anim, env, 0, false);
                None
            }
            0x8c => {
                if v.flags & 2 != 0 { self.set_anim(anim, 2.0, 0x97, 0); }
                if self.timer < 0x33 || self.land_lag != 0 { return None; }
                if 0 < self.hp {
                    if self.set_state(anim, env, 0, false) {
                        let seq = self.idle_seq();
                        self.set_anim(anim, 13.0, seq, 0);
                    }
                    return None;
                }
                Some(0x39)
            }
            0x39 | 0x7d | 0x67 | 0x7f => {
                if v.flags & 2 != 0 { self.die(); }
                None
            }
            0x65 => {
                if self.hot_hops < 3 && self.hot_hops * 7 + 10 < self.timer && Self::cross_within(env, 7) { self.set_state(anim, env, 0x66, true); }
                if self.pos[2] < self.surf.sand_z - 1.2 || 0xab <= self.timer { self.die(); }
                None
            }
            0x78 => {
                if self.pos[2] < self.surf.sand_z - 1.2 || 0x97 <= self.timer { self.die(); }
                None
            }
            0x7a => {
                if self.pos[2] < self.surf.sand_z - 0.8 || 0x97 <= self.timer { self.die(); }
                None
            }
            0x7c => {
                if 0xdd <= self.timer { self.die(); }
                None
            }
            0x74 => {
                if 0x79 <= self.timer { self.die(); }
                None
            }
            _ => {
                if self.pos[2] < self.damage.death_z - 1.2 || 0x65 <= self.timer { self.die(); }
                None
            }
        }
    }

    /// `0x2b31f0`: dead — the respawn request (`0x2e7c90(0x10)`, the engine's), once.
    pub fn die(&mut self) {
        self.damage.dead = true;
    }

    /// The transitions' prologue (`0x2cb650`), its first part: the hit intake, the death on no health from idle or
    /// walk.
    pub fn transitions_prologue(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        if self.hit_intake(env, anim) { return true; }
        if self.hp < 1 && (self.state == 0 || self.state == 2) {
            self.set_state(anim, env, 0x39, true);
            return true;
        }
        false
    }

    /// The prologue's last check: below the level's death height (`0x1b1a68`) with more than 2 above the ground → the
    /// fall out of the level 0x74.
    pub fn fall_out_check(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        if let Some(z) = env.death_z {
            if self.pos[2] < z && 2.0 < self.height && self.mode != 2 && self.state != 0x74 {
                self.set_state(anim, env, 0x74, true);
                return true;
            }
        }
        false
    }
}
