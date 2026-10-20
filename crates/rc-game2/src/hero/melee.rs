//! The wrench (group 6): the □ trigger of the hand item (L00 `0x2c96d0`, item 10), the ground combo 0x13 (three
//! swings), the jump attack 0x14, the throw 0x15 / 0x83 (the comet strike: the wrench flies, `0x2ba610`) and the
//! gadget swing 0x22, their SetState entries (`0x2c4560` group 6), physics (`0x2ba8e0`) and transitions (`0x2cb650`),
//! the aim `0x2b76a8` with the target search `0x2ae900` / `0x2ae400`. The wrench moby's own update (the hit test,
//! the flight) is `crate::moby::classes::wrench` (`0x38d318`). R&C1's melee (rc_game::hero::melee) is the map.
//!
//! **The swing table** (`0x25d910`, 0x2c a row, row `0x18aa1c`): `[kind (0 ground, 1 jump, 2 comet), step,
//! chain-before, input-ref, chain-from, jump-after, idle-after, hit-from, hit-to, +0x24, +0x28]`, frames of Ratchet's
//! sequence. Rows 0..2 the combo's swings (sequences 0x17..0x19), 3 the throw (0x1a), 4 the jump attack (0x2b), 5 / 6
//! R&C1's Magneboots rows, 7 / 8 the underwater swings 0x4d (sequences 0xf / 0x10).
//!
//! Not ported: the stats counters the entries bump (`0x18c3d8`, `0x18c750`, `0x18c75c`, `0x18c600`), the sounds
//! (`0x2b9c90`, `0x2d6cc8`), the strafe and the gravity frames (`0x18c0e3` ≠ 0), the sliding floors (`0x18b2b2`:
//! `0x2b96a0`), the jump attack's ground sparks and wall hit (`0x2e14b0` with flag 0x10), the comet aim `0x41fe58`'s
//! own search (`0x3257a0`: the plain aim is used), 0x1f (the glove swing, needs a glove) and 0x4d (the swim family).

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;
use rc_game::pad::button;

/// `0x25d910`: the swing table (11 words a row).
pub const SWINGS: [[i32; 11]; 9] = [
    [0, 0, 33, 19, 25, 24, 26, 17, 23, 17, 24],
    [0, 1, 25, 7, 12, 13, 15, 8, 13, 6, 12],
    [0, 2, 0, 16, 23, 19, 21, 12, 16, 9, 23],
    [2, 0, 99, 83, 88, 35, 91, 99, 99, 99, 99],
    [1, 0, 99, 32, 28, 28, 30, 1, 28, 22, 25],
    [0, 0, 33, 19, 26, 24, 31, 18, 22, 17, 24],
    [0, 1, 25, 7, 17, 18, 23, 8, 13, 6, 12],
    [0, 0, 16, 3, 12, 8, 10, 3, 6, 99, 99],
    [0, 1, 16, 3, 12, 8, 10, 3, 6, 99, 99],
];
const KIND: usize = 1; // +0x04: the swing's step (0 / 1 / 2: which of the combo's swings)
const CHAIN_BEFORE: usize = 2; // +0x08
const REF: usize = 3; // +0x0c
const CHAIN_FROM: usize = 4; // +0x10
const JUMP_AFTER: usize = 5; // +0x14
const IDLE_AFTER: usize = 6; // +0x18
pub const HIT_FROM: usize = 7; // +0x1c
pub const HIT_TO: usize = 8; // +0x20

/// The current-yaw sentinel of `0x2b4310` (99999.0).
const YAW_KEEP: f32 = f32::MAX;

/// A moby the aim can pick (the target list `0x1fac60`: the run list's mobys with mode 0x1000).
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: usize,
    pub pos: V3,
    /// `+0xe0`: the third row (the line test aims at `pos + row`).
    pub up: V3,
    /// `0x3204e8`: a crate (o_class 500..540); with class type 0x16 the range bonus applies too.
    pub crate_like: bool,
    /// Class header +0x46.
    pub kind: i16,
}

/// The melee block (`0x18a9e0..0x18aa30`).
#[derive(Clone, Copy, Debug, Default)]
pub struct MeleeBlock {
    /// `0x18aa1c` (s16): the swing table row.
    pub row: i16,
    /// `0x18aa04`: the target moby and its position; `0x18aa08`: aiming (at a target or along the stick).
    pub target: Option<usize>,
    pub target_pos: V3,
    pub aim: u8,
    /// `0x18aa0c` / `0x18a9e0`: the aim's yaw and direction.
    pub aim_yaw: f32,
    pub aim_dir: V3,
    /// `0x18aa14`: the lunge factor of the combo.
    pub lunge: f32,
    /// `0x18aa20`: the jump attack came down.
    pub fell: f32,
    /// `0x18aa2c`: alternates on each throw.
    pub throw_toggle: u32,
    /// `0x18aa24` / `0x18aa28`: the throw's yaw and pitch (the look stance's camera).
    pub throw_yaw: f32,
    pub throw_pitch: f32,
    /// `0x18aa1e` (s16): the wrench hit a wall this swing.
    pub wall_hit: i16,
    /// `0x18aa64`: the throw's catch timer.
    pub catch_t: i32,
    /// `0x18a980` / `0x18a990`: the wrench's sweep segment last tick (Ratchet's joint 0, the head).
    pub sweep_base: V3,
    pub sweep_head: V3,
}

impl Hero {
    fn swing(&self) -> &'static [i32; 11] { &SWINGS[(self.melee.row.clamp(0, 8)) as usize] }

    /// `0x2c4560` group 6 (0x13, 0x14, 0x15, 0x22, 0x83): the shared part (the throw while a jump is held is the
    /// throw itself), then per state.
    pub(super) fn melee_entry(&mut self, anim: &mut dyn AnimCtl, env: &Env, id: i32, play: bool) -> Entry {
        self.group = 6;
        if id != 0x83 { self.cam_mode = 0; }
        self.melee.lunge = 1.0;
        self.melee.aim = 0;
        self.melee.target = None;
        self.melee.wall_hit = 0;
        match id {
            0x13 => {
                // The next swing of the combo when the last ended late enough (`+0x24` before the frame), or the one
                // before last within 0x1b ticks.
                let row = self.melee.row.clamp(0, 8) as usize;
                let step = SWINGS[row][KIND];
                let frame = anim.view().frame;
                let mut i = 0;
                if self.prev_state == 0x13 && frame < SWINGS[row][CHAIN_BEFORE] as f32 {
                    i = (step + 1) % 3;
                } else if self.prev2_state == 0x13 && self.timer + self.prev_timer < 0x1b {
                    i = (step + 1) % 3;
                }
                self.melee.row = i as i16;
                self.aim_search(env, 5.0, 0.785_398_2, 0.785_398_2);
                if play {
                    let (mut blend, frame0) = match i { 0 => (5, 0), 1 => (7, 2), _ => (7, 4) };
                    if self.melee.aim != 0 && i < 2 {
                        let a = rot_dist(self.melee.aim_yaw, self.rot[2]);
                        blend += (a * 1.5) as i32;
                    }
                    self.set_anim(anim, blend as f32, (i + 0x17) as u8, frame0 / 2);
                    self.items.hand_anim = Some((i as u8 + 3, (frame0 / 2) as u8, blend as u8));
                }
            }
            0x14 => {
                self.cam_mode = 0x50;
                self.melee.row = 4;
                self.melee.fell = 0.0;
                self.aim_search(env, 8.0, 1.047_197_6, -1.0);
                if play {
                    let blend = if self.prev_state == 0xb && self.prev_timer < 0x2a { 0x11 } else { 0xb };
                    self.set_anim(anim, blend as f32, 0x2b, 4);
                    self.items.hand_anim = Some((10, 4, blend + 2));
                }
            }
            0x15 | 0x83 => {
                self.melee.row = 3;
                if id == 0x15 {
                    self.aim_search(env, 14.0, 0.523_598_8, -1.0);
                    self.melee.throw_toggle = (self.melee.throw_toggle == 0) as u32;
                }
                if play {
                    let mut blend = 10;
                    if anim.view().seq_b == 0x1a { blend = 3; }
                    if self.prev_state == 1 { blend = 0x11; }
                    self.set_anim(anim, blend as f32, 0x1a, 0);
                    if id == 0x15 { anim.set_loop(6, 0x15); }
                }
            }
            0x22 => {
                if play { self.set_anim(anim, 9.0, 0x2c, 7); }
            }
            0x1f => {
                // The glove's punch (the item 0x35 in hand): the aim (11, 0.873), sequence 0x60.
                if self.items.hand.is_some() && play { self.set_anim(anim, -1.0, 0x60, 4); }
                self.aim_search(env, 11.0, 0.872_664_6, -1.0);
            }
            0x6d => {
                // The swings on a magnetic walkway (rows 5 / 6, alternating when chained).
                let row = if self.prev_state == 0x6d && SWINGS[self.melee.row.clamp(0, 8) as usize][KIND] == 0 { 6 } else { 5 };
                self.melee.row = row;
                let k = SWINGS[row as usize][KIND];
                if play { self.set_anim(anim, 5.0, (k + 0x17) as u8, (k == 0) as i32); }
            }
            _ => {}
        }
        Entry::Done
    }

    /// `0x2ba8e0` case 0x1f: the glove's punch — aimed in its first 4 ticks, a 0.3 push from tick 10 to 18 (speed
    /// 0.0528 up, 0.025 down), three 0.8 spheres (ahead and ±50°, 0.8 out, 0.55 up) dealing 3 (flags 0x30000, thrown
    /// 2 ahead and 1.3 up) from tick 10 to 22.
    fn phys_glove(&mut self, env: &Env) {
        if self.melee.aim == 0 && self.melee.target.is_none() {
            if self.timer < 4 { self.aim_search(env, 11.0, 0.872_664_6, -1.0); }
        } else {
            self.target_yaw = self.melee.aim_yaw;
            self.turn_to(0.05, 0.2, 0.283_121_5, 0);
        }
        self.target_speed = if 10 < self.timer && self.timer < 0x12 { 0.3 } else { 0.0 };
        if 10 < self.timer { self.speed_step(0.052_777_78, 0.025); }
        if self.melee.aim == 0 { self.set_planar_vel(f32::MAX) } else { self.set_planar_vel(self.melee.aim_yaw) }
        if 10 < self.timer && self.timer < 0x16 && self.items.hand.is_some() {
            let (s, c) = self.rot[2].sin_cos();
            let dir = [c * 2.0, s * 2.0, 1.3];
            for d in [0.0, -0.872_664_6, 0.872_664_6] {
                let (s, c) = add_rot(self.rot[2], d).sin_cos();
                let centre = add(self.pos, [c * 0.8, s * 0.8, 0.55]);
                self.hits_out.push(super::HeroHit { centre, r: 0.8, damage: 3.0, flags: 0x30000, dir, joints: None });
            }
        }
        if 10 < self.timer { self.edge_brake(env, 3.7, 0.0); }
        self.wall_check(env, true);
        if self.air_ticks != 0 { self.gravity_from(self.eff[2], 0.006_111_111) } else { self.vel[2] -= 0.015 }
    }

    /// `0x2b76a8(range, yaw cone, pitch cone)`: the aim. Nothing with a gadget item 0x20 or while aiming; with the
    /// stick pushed past 0.5 the aim follows the stick (aiming, no target); then the target search — a target found
    /// aims at it, else the aim keeps the facing (or the stick's) yaw and direction.
    pub fn aim_search(&mut self, env: &Env, range: f32, yaw_cone: f32, pitch_cone: f32) {
        if self.items.item == 0x20 || self.melee.aim != 0 { return; }
        let (mut dir, mut yaw) = (self.rows[0], self.rot[2]);
        if 0.5 < self.stick_mag {
            self.stick_target(env, 1.0, 0);
            dir = self.stick_dir;
            yaw = self.target_yaw;
            self.melee.target = None;
            self.melee.aim = 1;
        }
        let found = self.target_search(env, range, yaw, yaw_cone, pitch_cone, false);
        match found {
            Some(t) if self.melee.target.is_none() || self.melee.aim != 0 => {
                self.melee.aim_yaw = atan2(t.pos[0] - self.pos[0], t.pos[1] - self.pos[1]);
                self.melee.aim_dir = sub(t.pos, self.pos);
                self.melee.target = Some(t.id);
                self.melee.target_pos = t.pos;
            }
            _ => {
                self.melee.aim_dir = dir;
                self.melee.aim_yaw = yaw;
            }
        }
        self.melee.aim_dir[2] = 0.0;
    }

    /// `0x2ae900(range, yaw, yaw cone, pitch cone, dir, class type −1, line test)` with `0x2ae400`'s score: over the
    /// target list, the moby with the lowest `d + d·(yaw off + pitch off)` (crates and type 0x16 classes: + range ·
    /// 0.8), out of range or outside a cone (cones ≤ 0: none) left out; with `line`, one hidden behind the world (a
    /// line from the hero's head `0x189f00` to `pos + up`) is left out too.
    pub fn target_search(&self, env: &Env, range: f32, yaw: f32, yaw_cone: f32, pitch_cone: f32, line: bool) -> Option<Target> {
        let mut best = 1e8;
        let mut out = None;
        let eye = if self.state == 0x14 { [self.pos[0], self.pos[1], self.ground_z] } else { self.pos };
        for t in env.targets {
            let d = dist(eye, t.pos);
            let mut skip = range < d;
            let a_yaw = rot_dist(atan2(t.pos[0] - eye[0], t.pos[1] - eye[1]), yaw);
            let a_pitch = rot_dist(atan2(dist2(eye, t.pos), t.pos[2] - eye[2]), 0.0);
            if 0.0 < yaw_cone && yaw_cone < a_yaw { skip = true; }
            if 0.0 < pitch_cone && pitch_cone < a_pitch { skip = true; }
            let mut score = d + (a_yaw + a_pitch) * d;
            if t.crate_like || t.kind == 0x16 { score += range * 0.8; }
            if skip || best <= score { continue; }
            if line {
                let head = add(self.pos, [0.0, 0.0, 1.0]);
                if env.line(head, add(t.pos, t.up), 2).is_some() { continue; }
            }
            best = score;
            out = Some(*t);
        }
        out
    }

    /// The aim's turn (the states' "aiming" branch): the target yaw and direction are the aim's; the yaw springs.
    fn aim_turn(&mut self, k: f32, max: f32) {
        self.target_yaw = self.melee.aim_yaw;
        self.stick_dir = self.melee.aim_dir;
        self.turn_to(k, 0.2, max, 0);
    }

    /// `0x2ba8e0` group 6.
    pub(super) fn phys_melee(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        let v = anim.view();
        let sw = *self.swing();
        let frame = v.frame;
        match self.state {
            0x1f => return self.phys_glove(env),
            0x6d => return self.phys_ground(env, anim),
            0x93 => return,
            _ => {}
        }
        match self.state {
            0x13 => {
                if v.blending() || frame < sw[HIT_TO] as f32 { self.items.use_state = 2; }
                self.target_speed = 0.0;
                let mut lunge = self.melee.lunge;
                if let (Some(_), true) = (self.melee.target, self.timer <= 3) {
                    // Closer than the swing's reach: the lunge shortens (to 1.1·the gap, at most 4 / 1.9).
                    let d = (dist2(self.melee.target_pos, self.pos) - 0.5).max(0.0);
                    let (cap, far, d) = match sw[KIND] { 0 => (0.8, 4.0, d - 0.75), 1 => (1.7, 4.0, d - 0.6), _ => (1.7, 1.9, d - 1.3) };
                    if d.abs() < cap + 3.0 {
                        let l = (d * 1.111_111_2).min(far);
                        lunge = if l <= 1.0 { self.melee.lunge } else { l };
                    }
                }
                self.melee.lunge = lunge;
                match sw[KIND] {
                    1 => {
                        if v.blending() || frame < 9.0 {
                            let s = if 8.0 < frame { 0.050_000_004 } else if 7.0 < frame { 0.075 } else { 0.095 };
                            self.target_speed = self.melee.lunge * s;
                        }
                    }
                    0 => if frame < 18.0 { self.target_speed = self.melee.lunge * 0.073_333_34 },
                    2 => if v.blending() || frame < 12.0 { self.target_speed = self.melee.lunge * 0.061_666_67 },
                    _ => {}
                }
                approach(1.0, 0.2, &mut self.anim_speed);
                if self.melee.aim == 0 && self.melee.target.is_none() {
                    if !(sw[KIND] == 1 && !v.blending() && 7.0 < frame) { self.aim_search(env, 5.0, 0.785_398_2, 0.785_398_2); }
                } else {
                    let a = sub_rot(self.melee.aim_yaw, self.rot[2]);
                    if sw[KIND] == 1 && 0.0 < a { self.anim_speed = 1.0 / (a.abs() + 1.0); }
                    self.aim_turn(0.05, 0.282_160_5);
                }
                self.speed_step(0.010_277_778, 0.007_777_778);
                if self.melee.aim == 0 { self.set_planar_vel(YAW_KEEP) } else { self.set_planar_vel(self.melee.aim_yaw) }
                if self.air_ticks == 0 { self.gravity(0.015) } else { self.gravity_from(self.eff_v[2], 0.006_944_444) }
                self.edge_brake(env, 3.7, 0.0);
            }
            0x14 => {
                if v.blending() || frame < sw[IDLE_AFTER] as f32 { self.items.use_state = 2; }
                self.target_speed = 0.0;
                let up_close = 4.0 < self.height;
                match self.melee.target {
                    None => {
                        if up_close && self.timer < 0x46 { self.target_speed = 0.075; }
                        self.speed_step(0.004_166_667, 0.004_166_667);
                        self.vel = set_len2(self.vel, self.speed);
                    }
                    Some(_) => {
                        let d = len2(sub(self.melee.target_pos, self.pos));
                        if (2.0 < d && d < 5.0) || up_close { self.target_speed = 0.075; }
                        self.speed_step(0.006_944_444, 0.006_944_444);
                        let f = scale(self.rows[0], self.speed);
                        (self.vel[0], self.vel[1]) = (f[0], f[1]);
                    }
                }
                if self.melee.aim == 0 && self.melee.target.is_none() {
                    if frame < 25.0 { self.aim_search(env, 8.0, 1.047_197_6, 1.047_197_6); }
                } else {
                    self.aim_turn(0.05, 0.282_160_5);
                }
                let rise = if 2.25 < self.height { 0.064_166_67 } else if 1.8 < self.height { 0.089_833_33 } else { 0.128_333_34 };
                if 8 < self.timer {
                    if 0xe < self.timer {
                        self.gravity(0.036_111_11);
                        if self.vel[2] < 0.0 { self.melee.fell = 1.0; }
                        if self.vel[2] < -0.616_666_7 { self.vel[2] = -0.616_666_7; }
                    }
                } else {
                    let mut z = self.vel[2];
                    approach(rise, 0.041_666_668, &mut z);
                    self.vel[2] = z;
                }
            }
            0x15 | 0x83 => {
                self.target_speed = 0.0;
                if v.blending() || frame < sw[IDLE_AFTER] as f32 { self.items.use_state = 2; }
                let from_look = self.prev_state == 1 && 0x15 <= self.prev_timer;
                if !from_look {
                    if self.melee.aim == 0 && self.melee.target.is_none() {
                        let early = !(self.state == 0x83 && 0xb < self.timer) && (v.blending() || frame <= 6.0);
                        if early && self.state == 0x15 { self.aim_search(env, 14.0, 0.523_598_8, 0.785_398_2); }
                    } else {
                        let a = sub_rot(self.melee.aim_yaw, self.rot[2]);
                        if 0.0 < a { self.anim_speed = 1.0 / (rot_dist(self.melee.aim_yaw, self.rot[2]) + 1.0); }
                        self.aim_turn(0.057, 0.282_160_5);
                    }
                }
                // The throw at frame 33 of the throw's sequence (`0x2b3aa0(33)`).
                if !v.blending() && v.seq_b == 0x1a && passed(&v, 33.0) && self.state == 0x15 { self.throw_wrench(); }
                self.speed_step(0.010_277_778, 0.007_777_778);
                self.set_planar_vel(YAW_KEEP);
                if self.air_ticks != 0 { self.gravity_from(self.eff_v[2], 0.006_944_444) } else { self.gravity(0.015) }
            }
            0x22 => {
                self.target_speed = 0.0;
                if self.melee.aim == 0 {
                    self.aim_search(env, 5.5, 0.872_664_6, -1.0);
                } else {
                    self.aim_turn(0.04, 0.282_160_5);
                }
                self.speed_step(0.008_333_334, 0.009_722_223);
                self.set_planar_vel(YAW_KEEP);
                if self.air_ticks != 0 { self.gravity_from(self.disp[2], 0.015) } else { self.gravity(0.015) }
            }
            _ => {}
        }
    }

    /// `0x2cb650` group 6.
    pub(super) fn trans_melee(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let v = anim.view();
        let sw = *self.swing();
        let frame = v.frame;
        let held_r1 = env.pad.held & button::R1 != 0;
        match self.state {
            0x1f => {
                if self.items.hand.is_some() && !v.blending() && 20.0 < frame { self.set_state(anim, env, 0, false); }
                return None;
            }
            0x6d => return ((sw[IDLE_AFTER] as f32) < frame).then_some(0),
            0x93 => return None,
            _ => {}
        }
        match self.state {
            0x13 | 0x15 => {
                let mut window = (frame as i32 - sw[REF]) * 2;
                if self.state == 0x15 { window = 8; }
                let mut late = true;
                if sw[JUMP_AFTER] as f32 >= frame {
                    late = false;
                } else if !v.blending() {
                    if !(self.state == 0x15 && self.f16c == 1) && Self::cross_within(env, window) {
                        if !held_r1 { return Some(7); }
                        if self.crouch_jump_from(anim, env, 7) { return None; }
                    }
                    late = false;
                }
                if !late && !v.blending() {
                    if 34.0 < frame && 0.9 < self.stick_mag {
                        // The run, at the walk entry's frame for substate 0 (`$gp − 0x7218`).
                        self.set_state(anim, env, 2, false);
                        self.set_anim(anim, 13.0, 3, 8);
                        return None;
                    }
                    if (sw[IDLE_AFTER] as f32) < frame {
                        if 0.8 < self.stick_mag { return Some(2); }
                        if frame <= (sw[IDLE_AFTER] + 5) as f32 && v.flags & 2 == 0 { return None; }
                        self.set_state(anim, env, 0, false);
                        return None;
                    }
                }
                if self.state != 0x15 { return None; }
                if self.items.item == 10 && 0x64 < self.timer && self.melee.catch_t < 0x1a {
                    // `0x2d2870(2)`: out of the throw's hold loop.
                    anim.exit_loop(2);
                    self.melee.catch_t = 0x1a;
                }
                None
            }
            0x83 => {
                if 37.0 < frame && !v.blending() {
                    if Self::cross_within(env, 7) {
                        if !held_r1 { return Some(7); }
                        if self.crouch_jump_from(anim, env, 7) { return None; }
                    }
                    self.set_state(anim, env, 0, false);
                    let seq = self.idle_seq();
                    self.set_anim(anim, 15.0, seq, 0);
                }
                None
            }
            0x14 => {
                let eta = self.land_eta(0.036_111_11).min(60.0) as i32;
                if self.melee.fell == 0.0 {
                    self.anim_speed = 0.33;
                } else if self.air_ticks == 0 || eta == 0 || 26.0 - frame <= 1.0 {
                    self.anim_speed = if self.air_ticks == 0 && frame < 26.0 { 2.0 } else { 1.0 };
                } else {
                    self.anim_speed_for_ticks(&v, 26.0, eta as f32, 0.5, -1.0);
                    if self.height < 0.7 && self.anim_speed < 0.75 { self.anim_speed = 0.75; }
                }
                if (sw[JUMP_AFTER] as f32) < frame {
                    if v.blending() { return None; }
                    if Self::cross_within(env, (frame as i32 - sw[REF]) * 2) {
                        if !held_r1 { return Some(7); }
                        if self.crouch_jump_from(anim, env, 7) { return None; }
                    }
                }
                if v.blending() { return None; }
                if (sw[IDLE_AFTER] as f32) < frame { self.set_state(anim, env, 0, false); }
                None
            }
            0x22 => {
                if 19.0 < frame && Self::cross_within(env, 0xb) { return Some(7); }
                if 21.0 <= frame { self.set_state(anim, env, 0, false); }
                None
            }
            _ => None,
        }
    }

    /// `0x2cadb0(7)` called from a swing's chain: the crouch's jumps (the flip with the stick off the facing, else 7).
    fn crouch_jump_from(&mut self, anim: &mut dyn AnimCtl, env: &Env, _n: i32) -> bool {
        let gap = self.flip_gap(env);
        if env.pad.held & button::R1 != 0 && 0 < gap && env.pad.combo(button::CROSS, super::states::FLIP_DIRS, gap + 5, gap) {
            self.set_state(anim, env, 0xb, true);
            return true;
        }
        if self.timer < 7 || !Self::cross_within(env, 9) { return false; }
        self.set_state(anim, env, 7, true);
        true
    }

    /// `0x2ba610`: the throw — once per throw (`0x18b05a`), the wrench moby leaves the hand (state 10) along the
    /// facing (or the look stance's camera from 0x1 / 0x1d after 0x15 ticks), with the throw's push (0.383) and its
    /// sequence 6; the flight is the wrench's update.
    pub fn throw_wrench(&mut self) {
        if self.items.thrown != 0 { return; }
        let Some(_) = self.items.hand.filter(|_| self.items.hand_class == 0x47) else { return };
        self.items.thrown = 1;
        self.items.throw = Some(super::items::Throw { dir: self.rows[0], push: 0.383_333_33 });
    }

    /// `0x2c96d0` for the wrench (item 10): □ within 10 ticks, on the ground (groups 0 / 1, or a landed jump), the
    /// hand item ready: R1 held, or the look stance after 0x14 ticks → the throw 0x15; else the combo 0x13. In the air
    /// (a jump past 10 ticks, the fall, the double jump past frame 32): □ within 0xe (0x12 after a flip / double jump)
    /// → the swing 0x13 below 0.1 (0.57 descending) above the ground, else the jump attack 0x14. During the combo: □
    /// within the chain window (`+0x10` frame on, `(frame − ref)·2`, 1..0xf) → the next swing (0x15 with R1).
    /// Returns whether a state was set.
    pub fn melee_trigger(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        let it = &self.items;
        // `0x18c0dc` (states that keep the item away; the wrench still with `0x18a168` = 1, not tracked here).
        if it.no_use != 0 { return false; }
        if it.item != 10 || it.hand.is_none() || it.thrown != 0 || it.ready != 2 { return false; }
        let mask = it.trigger;
        let pressed = |n: i32| env.pad.pressed_within(mask, n).is_some();
        let held_r1 = env.pad.held & button::R1 != 0;
        let grounded_ok = self.land_lag == 0 && (self.group < 2 || (self.group == 4 && self.jump.landed != 0));
        if grounded_ok {
            if !pressed(10) { return false; }
            let mut id = 0x13;
            if held_r1 || ((self.state == 1 || self.state == 0x1d) && 0x14 < self.timer) {
                if (self.state == 1 || self.state == 0x1d) && 0x14 < self.timer {
                    self.throw_wrench();
                    return false;
                }
                id = 0x15;
            }
            return self.set_state(anim, env, id, true);
        }
        let air = matches!(self.state, 7 | 9 | 0x89 | 8 | 6) || (self.state == 0xe && 32.0 < anim.view().frame);
        let flip_late = self.state == 0xb && (if self.jump.flip_dir != 3 { 18.0 } else { 20.0 }) < anim.view().frame;
        if air || flip_late || self.state == 0x2c || self.state == 0x11 {
            if self.timer < 0xb { return false; }
            let n = if self.state == 0xb || self.state == 0xe { 0x12 } else { 0xe };
            if !pressed(n) { return false; }
            let low = if self.jump.descending == 0 { 0.1 } else { 0.57 };
            let id = if self.height <= low && !(self.state == 8 && 0.57 < self.height) { 0x13 } else { 0x14 };
            return self.set_state(anim, env, id, true);
        }
        if self.group == 6 && !anim.view().blending() {
            let sw = *self.swing();
            let frame = anim.view().frame;
            if self.state == 0x15 && 0x1e < self.timer && sw[CHAIN_FROM] as f32 <= frame && held_r1 && pressed(0xf) {
                return self.set_state(anim, env, 0x15, true);
            }
            if sw[CHAIN_FROM] as f32 <= frame && self.items.thrown == 0 {
                let w = ((frame as i32 - sw[REF]) * 2).clamp(1, 0xf);
                if pressed(w) { return self.set_state(anim, env, if held_r1 { 0x15 } else { 0x13 }, true); }
            }
        }
        false
    }
}

/// `0x2b3aa0(f)`: the frame passed `f` this tick.
pub fn passed(v: &rc_game::hero::anim::AnimView, f: f32) -> bool { f < v.frame && v.frame - f <= v.frame_step }
