//! Grinding (group 0xf, the Grind Boots `0x1a7b14`): the rail probe `0x2adbb8`, the grind 0x27, the jump off a
//! rail 0x28 and the hop between rails 0x29 (their entry is the jump group's), the wrench on a rail 0x2a and the
//! knock off it 0x3e — SetState (L00 `0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`); and the zip
//! line 0x71 (group 0x1a, the pole probe `0x2adda8`).
//!
//! **The probe.** Each tick (after the ledge probe): standing, walking, falling, swinging, in the look stance,
//! descending in a jump, or in a rail jump that left its rail: a path (`crate::paths::find_rail`) within 0.9 across
//! (0.3 standing or walking; +0.5 and 10 up when `0x18a016`), its point no more than 0.5 below and 0.57 above the
//! feet → the rail `0x18a3f0..`, found `0x18a45c`. The jumps, falls and swings then land on it (0x27).
//!
//! **The grind.** Along the rail at the speed `0x18a464`: the base `0x18a4a0` (from the speed along the rail on
//! landing) eases to 0.2 by 1/240, plus the slope's term `0x18a494` (uphill −0.08·slope, downhill −0.38·slope,
//! easing by 1/1800 / 1/7200, −1/24..0.2); the feet pulled to the rail point (a spring 0.03 / 0.3, at most 0.117 a
//! tick); the facing along the rail ± 90° (the side Ratchet stands on, `0x18a478`). The rail's end: 6 ticks off it,
//! then the fall. ✕: the jump 0x28 (a stick sideways in its first 10 ticks hops to a parallel rail 2..4.5 away: 0x29,
//! the move blended over 48 ticks); ■: the wrench 0x2a; hitting something: 0x3e (on levels 0xd and 0x12 a hurt
//! thrown off the rail).
//!
//! **The zip line 0x71.** Along a pole path (`0x18a500..`) from its end, the speed easing to 0.233 by 1/400; the
//! hands pulled to the line (a spring 0.05 / 0.3, at most 1/6); 6 ticks past its end: the fall.
//!
//! Not ported: the sparks and sounds, the lean (`0x25bdf0..`), the rail-jump targets (`0x18a4ac` / `0x18a4b0` /
//! `0x18a440`, set by objects: the jump keeps its height), the weapon stance's lean (`0x18c0da`), the zip line's
//! spark (`0x336148`).

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::paths::{advance, closest, find_rail, next_index};
use crate::hero::anim::AnimCtl;
use crate::pad::button;

/// The grind block (`0x18a3f0..0x18a4e8`) and the pole's (`0x18a4f0..0x18a528`).
#[derive(Clone, Copy, Debug, Default)]
pub struct GrindBlock {
    /// `0x18a3f0` the rail point, `0x18a450` the path, `0x18a454` / `0x18a458` the segment and the distance along it,
    /// `0x18a474` it loops, `0x18a45c` found by the probe.
    pub point: V3,
    pub path: usize,
    pub seg: usize,
    pub t: f32,
    pub looped: bool,
    pub found: bool,
    /// `0x18a460`: the ticks past the rail's end (0 on it); `0x18a4e8`: the ticks high off it.
    pub off: i32,
    pub high: i32,
    /// `0x18a464` the speed, `0x18a468` the direction (±1), `0x18a46c` / `0x18a470` the rail's yaw and pitch,
    /// `0x18a478` the stance's yaw offset (∓90°) and `0x18a47c` its side.
    pub speed: f32,
    pub dir: i32,
    pub yaw: f32,
    pub pitch: f32,
    pub stance: f32,
    pub side: i32,
    /// The hop (`0x18a480`): the rail left (`0x18a484..`, its point `0x18a430`) and the blend's ticks `0x18a498`.
    pub hopping: bool,
    pub old: (usize, usize, f32, bool),
    pub old_point: V3,
    pub hop_ticks: i32,
    /// `0x18a494` the slope's speed, `0x18a49c` the lean (−1 none, 0 up, 1 down), `0x18a4a0` the base speed,
    /// `0x18a4bc` the knock's speed, `0x18a4c0` / `0x18a4c4` the weapon lean, `0x18a4cc` the pull spring.
    pub slope_speed: f32,
    pub lean: i32,
    pub base: f32,
    pub knock_speed: f32,
    pub lean_a: f32,
    pub lean_v: f32,
    pub pull_v: f32,
    /// `0x18a4a8` / `0x18a4b0` / `0x18a4b4`: the rail jump's target flags (objects'); `0x18a4ae`: a hit to react to.
    pub jump_ok: i32,
    pub jump_req: i32,
    pub to_target: bool,
    pub hit: bool,
    /// `0x18a014`: the probe's rest after leaving a rail; `0x18a016`: the wide probe.
    pub rest: i16,
    pub wide: i16,
    /// The pole: `0x18a500` path, `0x18a504` / `0x18a508` position, `0x18a51c` loops, `0x18a50c` found, `0x18a4f0`
    /// its point; `0x18a510` the ticks past its end, `0x18a514` the speed, `0x18a520` / `0x18a524` its yaw / pitch,
    /// `0x18a528` the pull spring; `0x18a024` the pole probe's rest.
    pub pole: Option<(usize, usize, f32, bool, V3)>,
    pub pole_off: i32,
    pub pole_speed: f32,
    pub pole_yaw: f32,
    pub pole_v: f32,
    pub pole_rest: i16,
}

impl Hero {
    /// `0x2adbb8`: the rail probe (module docs).
    pub fn rail_probe(&mut self, env: &Env) {
        self.grind.found = false;
        if !env.grind_boots || self.grind.rest != 0 || env.rails.is_empty() { return; }
        let g = self.grind;
        let on = matches!(self.group, 0 | 1 | 2 | 6)
            || self.state == 1
            || (matches!(self.state, 0x28 | 0x29) && (g.off != 0 || g.to_target))
            || (self.group == 4 && (self.jump.descending != 0 || g.wide != 0));
        if !on { return; }
        let (mut reach, mut height) = (if self.group < 2 { 0.3 } else { 0.9 }, 1.5);
        if g.wide != 0 {
            reach += 0.5;
            height = 10.0;
        }
        let floor = |a: V3, b: V3| env.line(a, b, 2).is_some();
        let Some(h) = find_rail(env.rails, self.pos, None, reach, height, &floor) else { return };
        if self.state == 0x28 && g.to_target && h.path == g.path { return; }
        let dz = self.pos[2] - h.point[2];
        if -0.5 < dz && (dz < 0.57 || g.wide != 0) {
            let gb = &mut self.grind;
            (gb.point, gb.path, gb.seg, gb.t, gb.looped, gb.found) = (h.point, h.path, h.seg, h.t, h.looped, true);
        }
    }

    /// `0x2adda8`: the pole probe — in a jump going up, falling or grinding: a path within 0.9 of the hands, the
    /// hands 0.7 (□ just pressed: 1.2) below to 0.4 above it.
    pub fn pole_probe(&mut self, env: &Env) {
        self.grind.pole = None;
        if self.grind.pole_rest != 0 || env.rails.is_empty() { return; }
        if !((self.group == 4 && self.jump.descending == 0) || self.group == 2 || self.group == 0xf) { return; }
        // `0x2add70`: the hands, 0.3 ahead and 1.34 up.
        let head = self.local_point(0.3, 0.0, 1.34);
        let floor = |_: V3, _: V3| true;
        let Some(h) = find_rail(env.rails, head, None, 0.9, 1.5, &floor) else { return };
        let lo = if env.pad.pressed & button::SQUARE != 0 { -1.2 } else { -0.7 };
        let dz = head[2] - h.point[2];
        if lo < dz && dz < 0.4 { self.grind.pole = Some((h.path, h.seg, h.t, h.looped, h.point)); }
    }

    pub(super) fn grind_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        match id {
            0x28 | 0x29 => self.jump_entry(anim, env, id, play),
            0x27 => {
                self.group = 0xf;
                self.cam_mode = 3;
                let g = &mut self.grind;
                (g.dir, g.stance, g.off, g.high, g.side) = (1, -PI / 2.0, 0, 0, 0);
                if self.prev_group != 0xf {
                    (g.slope_speed, g.lean, g.base) = (0.0, -1, 0.0);
                }
                (g.pull_v, g.jump_req, g.jump_ok) = (0.0, 0, 0);
                let path = env.rails.get(g.path).map(|r| r.points.as_slice()).unwrap_or(&[]);
                let (mut seg, mut t) = (g.seg, g.t);
                let (p, _) = advance(path, 1.0, &mut seg, &mut t, g.looped);
                let d = sub(p, self.pos);
                if 0.001 < len(d) && self.prev_group != 0xf { self.grind.base = dot(self.disp, unit(d)); }
                if play { self.set_anim(anim, 6.0, 0x31 + self.grind.side as u8, 0); }
                Entry::Done
            }
            0x2a => {
                self.group = 0xf;
                self.cam_mode = 3;
                // The two rail swings (rows 0 / 1) alternate.
                self.melee.row = if self.prev_state == 0x2a { (self.melee.row == 0) as i16 } else { 0 };
                if play { self.set_anim(anim, 4.0, 0x4b + self.melee.row as u8, 0); }
                Entry::Done
            }
            0x3e => {
                self.group = 0xf;
                self.cam_mode = 0;
                if self.grind.off == 0 {
                    let dmg = self.pending_damage.take().unwrap_or(1);
                    self.take_damage(dmg);
                }
                if self.hp < 1 {
                    // No health left: thrown off the rail (back and 30..35° aside), hurt.
                    let r = self.rng.randf(0.523_598_8, 0.610_865_3);
                    let away = add_rot(add_rot(atan2(self.vel[0], self.vel[1]), PI), r);
                    let (s, c) = away.sin_cos();
                    self.eff = [c * self.grind.speed * 0.7, s * self.grind.speed * 0.7, 0.075];
                    if 0xf < self.grind.off && self.hp < 2 { self.hp = 2; }
                    self.damage.took = true;
                    self.set_state(anim, env, 0x16, true);
                    self.ledge.cooldown = 0x46;
                    self.grind.rest = 0x96;
                    return Entry::Return(false);
                }
                self.damage.invincible = 0x4d;
                self.grind.knock_speed = 1.0 / 24.0;
                self.grind.base = self.grind.base.min(1.0 / 24.0);
                self.grind.slope_speed = 0.0;
                self.vel = [0.0; 3];
                self.disp = [0.0; 3];
                self.eff[2] = 0.183_333_34;
                self.vel[2] = 0.183_333_34;
                if play { self.set_anim(anim, -3.0, 0x10, 2); }
                Entry::Done
            }
            _ => {
                // 0x71: the zip line.
                self.group = 0x1a;
                self.cam_mode = 3;
                self.grind.pole_off = 0;
                self.grind.pole_v = 0.0;
                if let Some((path, seg, t, looped, _)) = self.grind.pole {
                    let pts = env.rails.get(path).map(|r| r.points.as_slice()).unwrap_or(&[]);
                    let (mut s, mut tt) = (seg, t);
                    let (p, _) = advance(pts, 1.0, &mut s, &mut tt, looped);
                    let d = sub(p, self.pos);
                    if 0.001 < len(d) { self.grind.pole_speed = dot(self.disp, unit(d)); }
                }
                self.grind.pole_speed = (self.grind.pole_speed * 0.5).max(0.0);
                Entry::Anim(8.0, 0x6c, 0)
            }
        }
    }

    pub(super) fn phys_grind(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        if self.state == 0x71 { return self.phys_pole(env); }
        let Some(rail) = env.rails.get(self.grind.path) else { return };
        let pts = rail.points.as_slice();
        let looped = self.grind.looped;
        if let Some((q, seg, t)) = closest(pts, 1000.0, 2.0, 2.5, self.pos, looped) { (self.grind.point, self.grind.seg, self.grind.t) = (q, seg, t); }
        if self.grind.hopping {
            let (op, _, _, ol) = self.grind.old;
            if let Some(r) = env.rails.get(op) {
                if let Some((q, seg, t)) = closest(&r.points, 1000.0, 2.0, 2.5, self.pos, ol) {
                    self.grind.old_point = q;
                    (self.grind.old.1, self.grind.old.2) = (seg, t);
                }
            }
        }
        let high = 1.07 < self.height && self.state != 0x28 && self.state != 0x3e && !self.grind.hopping && 0x1e < self.group_timer;
        self.grind.high = if high { self.grind.high + 1 } else { 0 };
        if self.grind.off != 0 { self.grind.off += 1; }
        if self.grind.off == 0 && !high {
            let spd = if self.state == 0x3e { self.grind.knock_speed } else { self.grind.speed };
            let (mut seg, mut t) = (self.grind.seg, self.grind.t);
            let (p, _) = advance(pts, spd * self.grind.dir as f32, &mut seg, &mut t, looped);
            (self.grind.seg, self.grind.t) = (seg, t);
            let mut p_old = p;
            if self.grind.hopping {
                let (op, os, ot, ol) = self.grind.old;
                if let Some(r) = env.rails.get(op) {
                    let (mut s2, mut t2) = (os, ot);
                    p_old = advance(&r.points, spd * self.grind.dir as f32, &mut s2, &mut t2, ol).0;
                    self.grind.old = (op, s2, t2, ol);
                }
            }
            let nxt = next_index(pts.len(), self.grind.seg, self.grind.dir, looped);
            let d = sub([pts[nxt][0], pts[nxt][1], pts[nxt][2]], p);
            if len(d) < 0.001 {
                if self.grind.seg == nxt {
                    self.grind.off = 1;
                    return;
                }
            } else {
                self.grind.yaw = atan2(d[0], d[1]);
                self.grind.pitch = atan2(len2(d), d[2]);
            }
            self.target_yaw = add_rot(self.grind.yaw, self.grind.stance);
            self.turn_to(0.015, 0.15, 0.104_719_76, 0);
            if (matches!(self.state, 0x28 | 0x29) && self.jump.landed == 0) || self.state == 0x3e {
                self.rail_air(env, p, p_old);
            } else {
                self.rail_ride(env, pts, p);
            }
            if self.state == 0x2a && self.timer < 0x14 {
                let at = add(self.pos, [0.0, 0.0, 0.6]);
                let (s, c) = self.grind.yaw.sin_cos();
                self.hits_out.push(super::HeroHit { centre: at, r: 1.0, damage: 1.0, flags: 0x10000, dir: [c, s, 0.0], joints: None });
            }
            let _ = anim;
            return;
        }
        // Off the rail (past its end or high above it): free flight.
        self.vel = self.eff;
        if matches!(self.state, 0x28 | 0x29) {
            self.jump_vertical(env);
            return;
        }
        if len2(self.vel) < 1.0 / 24.0 {
            approach(0.066_666_67, 0.002_777_777_8, &mut self.speed);
            let (s, c) = self.grind.yaw.sin_cos();
            self.vel[0] = c * self.speed;
            self.vel[1] = s * self.speed;
        }
        self.vel[2] -= 0.006_111_111_5;
    }

    /// The rail jumps' and the knock's flight along the rail (`0x2ba8e0`'s group 0xf in the air).
    fn rail_air(&mut self, env: &Env, p: V3, p_old: V3) {
        if self.grind.to_target {
            let y = atan2(self.jump.wall_dir[0] - self.pos[0], self.jump.wall_dir[1] - self.pos[1]);
            if rot_dist(self.rot[2], y) < PI / 2.0 {
                self.target_yaw = y;
                self.turn_to(0.04, 0.2, 0.104_719_76, 0);
            }
        }
        if !self.grind.hopping || 0x30 < self.grind.hop_ticks {
            let d = sub(p, self.grind.point);
            self.vel[0] = d[0];
            self.vel[1] = d[1];
            self.pos[2] += d[2];
            let mut c = sub(self.grind.point, self.pos);
            c[2] = 0.0;
            if 1.0 / 60.0 < len(c) { c = set_len(c, 1.0 / 60.0); }
            self.pos[0] += c[0];
            self.pos[1] += c[1];
        } else {
            self.grind.hop_ticks += 1;
            let k = self.grind.hop_ticks as f32 / 48.0;
            let k0 = (self.grind.hop_ticks - 1) as f32 / 48.0;
            let d1 = scale(sub(p, p_old), k);
            let d0 = scale(sub(p, p_old), k0);
            self.pos[2] += d1[2] - d0[2];
            let target = add(p_old, d1);
            self.vel[0] = target[0] - self.pos[0];
            self.vel[1] = target[1] - self.pos[1];
            let d = sub(p, self.grind.point);
            self.pos[2] += d[2];
            if self.pos[2] < self.grind.point[2] && self.grind.hop_ticks < 0x31 { self.pos[2] = self.grind.point[2]; }
        }
        if self.state == 0x3e { self.vel[2] -= 0.006_388_889 } else { self.jump_vertical(env); }
    }

    /// Riding the rail (`0x2ba8e0`'s group 0xf on it).
    fn rail_ride(&mut self, env: &Env, pts: &[[f32; 4]], p: V3) {
        self.grind.hopping = false;
        let (mut a, mut v) = (self.grind.lean_a, self.grind.lean_v);
        turn_spring(0.0, 0.015, 0.3, 0.0, &mut a, &mut v, 0);
        (self.grind.lean_a, self.grind.lean_v) = (a, v);
        approach(0.2, 0.004_166_667, &mut self.grind.base);
        let (mut seg, mut t) = (self.grind.seg, self.grind.t);
        let (ahead, _) = advance(pts, 0.3, &mut seg, &mut t, self.grind.looped);
        let slope = atan2(dist2(self.grind.point, ahead), ahead[2] - self.grind.point[2]);
        let k;
        if 0.0 < slope {
            k = slope * -0.079_577_46;
            if 0.261_799_42 < slope { self.grind.lean = 0 } else if slope < 0.139_626_35 && self.grind.lean == 0 { self.grind.lean = -1 }
        } else {
            k = -slope * 0.381_971_84;
            if slope < -0.191_986_23 { self.grind.lean = 1 } else if -0.122_173_056 < slope && self.grind.lean == 1 { self.grind.lean = -1 }
        }
        let step = if self.grind.slope_speed < k { 0.000_555_555_6 } else { 0.000_138_888_9 };
        approach(k, step, &mut self.grind.slope_speed);
        self.grind.slope_speed = self.grind.slope_speed.clamp(-1.0 / 24.0, 0.2);
        self.grind.speed = (self.grind.base + self.grind.slope_speed).max(0.0);
        self.vel = sub(p, self.grind.point);
        let mut c = sub(self.grind.point, self.pos);
        let mut cl = -len(c);
        let mut pv = self.grind.pull_v;
        spring(0.0, 0.03, 0.3, 0.116_666_67, &mut cl, &mut pv);
        self.grind.pull_v = pv;
        let l = len(c);
        if l < self.grind.pull_v { self.grind.pull_v = l; }
        if self.grind.pull_v < l { c = set_len(c, self.grind.pull_v); }
        self.pos = add(self.pos, c);
        if self.pos[2] < self.grind.point[2] && self.height < 1.0 { self.pos[2] = self.grind.point[2]; }
        let _ = env;
    }

    /// `0x2ba8e0` case 0x71: along the zip line.
    fn phys_pole(&mut self, env: &Env) {
        let Some((path, _, _, looped, _)) = self.grind.pole else { return };
        let Some(r) = env.rails.get(path) else { return };
        let pts = r.points.as_slice();
        let head = self.local_point(0.3, 0.0, 1.34);
        let Some((q, s, tt)) = closest(pts, 1000.0, 8.0, 2.5, head, looped) else { return };
        let mut point = q;
        let (mut seg, mut t) = (s, tt);
        if self.grind.pole_off == 0 {
            let (p, _) = advance(pts, self.grind.pole_speed, &mut seg, &mut t, looped);
            let nxt = next_index(pts.len(), seg, 1, looped);
            let d = sub([pts[nxt][0], pts[nxt][1], pts[nxt][2]], p);
            if len(d) < 0.001 {
                if seg == nxt {
                    self.grind.pole_off = 1;
                    self.grind.pole = Some((path, seg, t, looped, point));
                    return;
                }
            } else {
                self.grind.pole_yaw = atan2(d[0], d[1]);
            }
            self.target_yaw = self.grind.pole_yaw;
            self.turn_to(0.018, 0.2, 0.107_626_7, 0);
            approach(0.233_333_35, 0.002_5, &mut self.grind.pole_speed);
            self.vel = sub(p, point);
            let mut c = sub(point, head);
            let mut cl = -len(c);
            let mut pv = self.grind.pole_v;
            spring(0.0, 0.05, 0.3, 1.0 / 6.0, &mut cl, &mut pv);
            self.grind.pole_v = pv;
            let l = len(c);
            if l < self.grind.pole_v { self.grind.pole_v = l; }
            if self.grind.pole_v < l { c = set_len(c, self.grind.pole_v); }
            self.pos = add(self.pos, c);
            point = p;
            self.grind.pole = Some((path, seg, t, looped, point));
            return;
        }
        self.vel = [self.eff[0], self.eff[1], self.eff[2] - 0.006_111_111_5];
        self.grind.pole_off += 1;
    }

    pub(super) fn trans_grind(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        if self.state == 0x71 {
            let v = anim.view();
            // The grab's key 11 passed: the sound 10 (the line's sparks `0x38cc88` are not ported).
            if v.seq_b == 0x6c && 11.0 < v.frame && v.frame - 11.0 <= v.frame_step { self.sounds.push((10, 0)); }
            if anim.view().seq_b == 0x6c && anim.view().flags & 2 != 0 { self.set_anim(anim, 30.0, 0x5f, 0); }
            if self.grind.pole_off < 6 { return None; }
            self.set_state(anim, env, 6, true);
            self.grind.pole_rest = 0x1e;
            return None;
        }
        if self.state == 0x3e { self.grind.hit = false; }
        if self.state != 0x3e {
            let mut hit = false;
            if self.cap_hit != 0 {
                let up = if matches!(self.state, 0x28 | 0x29) { 0.45 } else if env.level == 0x14 { 0.3 } else { 0.02 };
                let (s, c) = self.grind.yaw.sin_cos();
                let at = add(self.pos, [c * 0.25, s * 0.25, up + 0.5]);
                hit = env.sphere(at, 0.5, 2).is_some();
            }
            if hit || self.grind.hit {
                let mut clear = true;
                if self.cap_hit != 0 {
                    let up = if env.level == 0x12 { 1.8 } else { 1.5 };
                    let a = [self.grind.point[0], self.grind.point[1], self.grind.point[2] + up];
                    let (s, c) = self.grind.yaw.sin_cos();
                    if env.line(a, add(a, [c * 5.0, s * 5.0, 0.0]), 2).is_some() { clear = false; }
                }
                self.grind.hit = false;
                if (env.level == 0xd || env.level == 0x12) && !clear {
                    self.eff = scale(self.rows[1], 0.216_666_68);
                    self.eff[2] = 1.0 / 6.0;
                    self.set_state(anim, env, 0x16, true);
                    self.ledge.cooldown = 0x46;
                    self.grind.rest = 0x46;
                    return None;
                }
                return Some(0x3e);
            }
        }
        let g = self.grind;
        match self.state {
            0x27 => {
                if Self::cross_within(env, 7) {
                    if g.jump_ok == 0 { return Some(0x28); }
                    self.grind.jump_req = 1;
                    return None;
                }
                if g.off < 6 && g.high < 6 {
                    if g.off == 0 && g.high == 0 && env.pad.pressed_within(button::SQUARE, 10).is_some() { return Some(0x2a); }
                    let seq = if g.lean == -1 { 0x31 + g.side as u8 } else { (g.lean + 0x47) as u8 };
                    if anim.view().seq_b != seq { self.set_anim(anim, 12.0, seq, 0); }
                    self.grind.jump_ok = 0;
                    return None;
                }
                self.rail_fall(env, anim);
                None
            }
            0x2a => {
                let sw = super::melee::SWINGS[self.melee.row.clamp(0, 8) as usize];
                let f = anim.view().frame;
                if Self::cross_within(env, 5) && sw[5] as f32 <= f { return Some(0x28); }
                let n = if self.melee.row == 0 { 0x17 } else { 0xe };
                if g.off == 0 && g.high == 0 && env.pad.pressed_within(button::SQUARE, 10).is_some() {
                    return ((n as f32) < f).then_some(0x2a);
                }
                if (sw[6] as f32) < f { return Some(0x27); }
                if g.off < 6 && g.high < 6 { return None; }
                self.rail_fall(env, anim);
                None
            }
            0x3e => {
                if 0.0 < self.vel[2] {
                    let eta = self.land_eta(0.006_388_889);
                    let v = anim.view();
                    self.anim_speed_for_ticks(&v, 9.0, eta, 0.5, -1.0);
                }
                if g.off == 0 || self.grounded_ticks == 0 {
                    if anim.view().flags & 2 != 0 { return Some(0x27); }
                    if 0x10 <= self.timer && self.vel[2] < 0.0 && (self.pos[2] - g.point[2]).abs() < 0.3 { return Some(0x27); }
                    return None;
                }
                if self.set_state(anim, env, 0, false) { self.set_anim(anim, 6.0, 0xc, 4); }
                None
            }
            _ => self.trans_rail_jump(env, anim),
        }
    }

    /// The rail's end, or too high off it: the fall (on level 0xd from more than 4: the death fall), the probes rest.
    fn rail_fall(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        if 8.0 < self.height { self.land_lag = 0x46; }
        let id = if env.level == 0xd && 4.0 < self.height { 0x74 } else { 6 };
        self.set_state(anim, env, id, true);
        self.grind.rest = 8;
        self.ledge.cooldown = 0x46;
    }

    /// `0x2cb650` case 0x28 / 0x29.
    fn trans_rail_jump(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let g = self.grind;
        if !((g.off < 6 && !g.to_target) || !g.found || 0.35 <= self.pos[2] - g.point[2]) { return Some(0x27); }
        if 6 <= g.off && env.level == 0x14 { return Some(6); }
        // A hop to a rail beside this one (the stick sideways in the first 10 ticks).
        if self.state == 0x28 && self.timer < 10 && self.jump.descending == 0 && 0.5 < self.stick_mag && !g.hopping {
            self.stick_target(env, 1.0, 0);
            let off = rot_dist(rot_dist(self.target_yaw, g.yaw), 0.0);
            let _ = off;
            if rot_dist(rot_dist(self.target_yaw, g.yaw), PI / 2.0) < 1.221_730_6 {
                let mut side = add_rot(g.yaw, PI / 2.0);
                if PI / 2.0 < rot_dist(self.target_yaw, side) { side = add_rot(g.yaw, -PI / 2.0); }
                let pts = env.rails.get(g.path).map(|r| r.points.as_slice()).unwrap_or(&[]);
                let mut d = 2.0;
                while d <= 4.5 {
                    let (mut s, mut t) = (g.seg, g.t);
                    let (mut p, end) = advance(pts, 0.0, &mut s, &mut t, g.looped);
                    if end { p = add(self.pos, scale(self.disp, 48.0)); }
                    let (sn, cs) = side.sin_cos();
                    p = add(p, [cs * d, sn * d, 0.0]);
                    let floor = |a: V3, b: V3| env.line(a, b, 2).is_some();
                    if let Some(h) = find_rail(env.rails, p, Some(g.path), 0.9, 1.5, &floor) {
                        let gb = &mut self.grind;
                        gb.old = (gb.path, gb.seg, gb.t, gb.looped);
                        gb.old_point = gb.point;
                        (gb.path, gb.looped) = (h.path, h.looped);
                        if let Some((q, seg, t)) = env.rails.get(h.path).and_then(|r| closest(&r.points, 1000.0, 2.0, 0.0, self.pos, h.looped)) {
                            (gb.point, gb.seg, gb.t) = (q, seg, t);
                        }
                        gb.hopping = true;
                        gb.hop_ticks = 0;
                        self.set_state(anim, env, 0x29, true);
                        return None;
                    }
                    d += 0.75;
                }
            }
        }
        let j = self.jump;
        if j.takeoff < self.timer && self.vel[2] < 0.001 { self.jump.descending = 1; }
        let v = anim.view();
        if self.timer < j.takeoff + 1 || j.takeoff + 9 <= self.timer || self.jump.descending != 0 {
            if self.jump.descending != 0 && self.jump.landed == 0 {
                let eta = if g.off == 0 { self.land_eta(j.g).min(60.0) } else { 30.0 };
                self.jump.land_eta = eta as i16;
                if eta != 0.0 { self.anim_speed_for_ticks(&v, j.f_apex, eta, 0.5, 0.25); }
                if g.hopping { self.anim_speed = self.anim_speed.clamp(0.5, 2.0); }
                if g.off == 0 && g.high == 0 && self.height < 0.5 && self.pos[2] + self.vel[2] < g.point[2] {
                    self.vel[2] = g.point[2] - self.pos[2];
                    self.jump.landed = 1;
                    self.anim_speed = 1.0;
                }
                if self.pos[2] - g.point[2] < -2.5 {
                    self.set_state(anim, env, 6, true);
                    self.ledge.cooldown = 0x46;
                    self.grind.rest = 8;
                    self.land_lag = 0x46;
                    return None;
                }
            }
        } else if !v.blending() {
            let eta = self.land_eta(j.g);
            self.anim_speed_for_ticks(&v, j.f_hold, eta, 0.5, 0.25);
            if g.hopping { self.anim_speed = self.anim_speed.clamp(0.5, 2.0); }
        }
        if g.pole.is_some() { return Some(0x71); }
        if self.jump.landed != 0 {
            self.jump.landed = self.jump.landed.saturating_add(1);
            if 2 < self.jump.landed {
                // Back on the rail: the grind without SetState (the original writes the state).
                self.state = 0x27;
                self.set_anim(anim, 7.0, 0x31 + self.grind.side as u8, 0);
                return None;
            }
        }
        if g.off == 0 || self.air_ticks != 0 { return None; }
        if self.set_state(anim, env, 0, false) { self.set_anim(anim, 6.0, 0xc, 4); }
        None
    }
}
