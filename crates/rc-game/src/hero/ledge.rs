//! The ledges (group 3): the probe `0x2ad000` (run every tick after the physics into the ledge block `0x18a370`, and
//! by the shimmy for its next hang point), the grab 0x17, the hang 0x18 and the shimmies 0x19 (left) / 0x1a (right):
//! their SetState entries (`0x2c4560`), physics (`0x2ba8e0`) and transitions (`0x2cb650`). The climb up is the jump
//! 0x1b (crate::hero::jump). The jump and fall transitions take the grab when the probe found a ledge (`0x18a398`).
//!
//! **The probe** (Ratchet; Clank has other sizes): only falling (group 2), descending in a jump (or below 0.0583 up),
//! hanging or shimmying, and not in the double jump's first 37 frames. From the capsule's front (`pos + fwd·r`), 4
//! vertical lines (1.65 above to 1.15 below, flags 4) stepping 0.075 forward look for a floor flatter than 20°
//! (surface not 0, 9 or 0xc; on a moby only when its record or class says grabbable). That edge must be at most
//! 1.5 above the feet, at least 0.5 above them and 1.8 above the ground below; nothing between Ratchet and the edge
//! (a line 1 back along the facing, flags 2); then the wall below the edge (5 lines from the feet's height up to
//! 0.7 below the edge, 0.14 apart, flags 2) faced within 65°, and the corner clear (lines 0.03 out, ±0.05 up, flags
//! 4, climbing the wall by 0.03 up to 2.4). Found: the edge, the hang point (edge + wall normal·0.5 + 1.43 down),
//! the wall's yaw, the moby.
//!
//! Clank's body (`0x18c0d4` = 1) probes with its own sizes (0.725 up, 0.7 down, 0.71 below the edge, 0.32 out); its
//! ledge states 0x46 / 0x47 / 0x49 / 0x4a are in `clank.rs`.
//!
//! Not ported: the ledge's moby carrying Ratchet (`+0x38`), the head check flag (`+0x34`, a 0.45 sphere above the
//! edge), the gravity frames.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use crate::hero::anim::AnimCtl;
use crate::pad::button;

/// `0x1acfa8`: the shimmy's speed by frame (×0.5).
const SHIMMY: [f32; 16] = [-0.0, -0.0, 0.025, 0.05, 0.075, 0.162, 0.162, 0.107, 0.037, 0.037, 0.037, -0.017, -0.017, -0.035, -0.035, 0.0];

/// The ledge block (`0x18a370..0x18a3b0`).
#[derive(Clone, Copy, Debug, Default)]
pub struct LedgeBlock {
    /// `+0x00` the hang point (the feet while hanging), `+0x10` the edge, `+0x24` the wall's yaw, `+0x28` found,
    /// `+0x38` the moby the ledge is on.
    pub hang: V3,
    pub edge: V3,
    pub yaw: f32,
    pub found: bool,
    pub moby: Option<usize>,
    /// `0x18a39c` / `0x18a3a0`: the grab's gravity and the eased height.
    pub g: f32,
    pub z: f32,
    /// `0x189ff0` (s16): the ticks before a ledge can be grabbed again.
    pub cooldown: i16,
}

/// A probe's result.
#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub hang: V3,
    pub edge: V3,
    pub yaw: f32,
    pub moby: Option<usize>,
}

const UP: V3 = [0.0, 0.0, 1.0];

impl Hero {
    /// `0x2ad000(pos, rot, out)`; `own` = the hero's block is the output (the per-tick probe, gated by the state).
    pub fn ledge_probe(&self, env: &Env, pos: V3, own: bool) -> Option<Ledge> {
        if self.ledge.cooldown != 0 { return None; }
        let clank = self.mode == 1;
        let mut on = if clank {
            (self.group == 4 && self.jump.descending != 0) || self.group == 2 || matches!(self.state, 0x4b | 0x49 | 0x4a)
        } else {
            let mut on = (self.group == 4 && (self.jump.descending != 0 || self.vel[2] < 0.058_333_337))
                || self.group == 2
                || matches!(self.state, 8 | 0x19 | 0x1a);
            if self.state == 0xe && self.anim_frame < 37.0 { on = false; }
            on
        };
        if !own { on = true; }
        if !on { return None; }
        // Clank's body (`0x18c0d4` = 1) is smaller.
        let (up_h, down_h, max_up, hang_down, min_above, step, wall_down, out) =
            if clank { (0.725, 0.7, 0.725, 0.71, 0.7, 0.25 * 0.25, 0.4, 0.32) } else { (1.5, 1.15, 1.5, 1.43, 1.8, 0.3 * 0.25, 0.7, 0.5) };
        let fwd = self.rows[0];
        let p = add(pos, scale(fwd, self.cap_radius));
        let mut top = add(p, scale(UP, up_h + 0.15));
        let mut bot = add(p, scale(UP, -down_h));
        let mut hit = None;
        for _ in 0..4 {
            top = add(top, scale(fwd, step));
            bot = add(bot, scale(fwd, step));
            if let Some(o) = env.line(top, bot, 4) {
                let s = o.surface_id();
                if s != 9 && s != 0xc && s != 0 && slope_of(o.normal) < 0.349_065_9 {
                    hit = Some(o);
                    break;
                }
            }
        }
        let o = hit?;
        if o.moby.is_some() { return None; }
        let edge = o.point;
        let dz = edge[2] - pos[2];
        if max_up < dz { return None; }
        let near = add(edge, scale(UP, 0.005));
        if env.line(sub(near, fwd), near, 2).is_some() { return None; }
        if dz + self.height < min_above || dz < 0.5 { return None; }
        // The wall below the edge.
        let mut q = add(add(edge, fwd), scale(UP, -wall_down));
        let mut a = [pos[0], pos[1], q[2]];
        let st = scale(UP, wall_down * 0.2);
        let mut wall = None;
        for _ in 0..5 {
            if let Some(w) = env.line(a, q, 2) {
                let n = [w.normal[0], w.normal[1], 0.0];
                let l = len(n);
                if 0.0 < l { wall = Some(scale(n, 1.0 / l)); }
                break;
            }
            a = add(a, st);
            q = add(q, st);
        }
        let n = wall?;
        if dot(fwd, n) >= 1.134_464_f32.cos() { return None; }
        let e0 = scale(n, 0.03);
        let (e1, e2) = (add(e0, scale(UP, 0.05)), sub(e0, scale(UP, 0.05)));
        let mut c = edge;
        let mut t = 0.0;
        while t < 2.4 {
            match env.line(add(c, e1), add(c, e2), 4) {
                None => {
                    let hang = add(add(c, scale(n, out)), scale(UP, -hang_down));
                    return Some(Ledge { hang, edge: c, yaw: atan2(n[0], n[1]), moby: None });
                }
                Some(o) => c = o.point,
            }
            t += 0.03;
            c = add(c, scale(UP, 0.03));
        }
        None
    }

    /// The per-tick probe into the block (`0x2a6dd0`: `0x2ad000(pos, rot, 0x18a370)`).
    pub fn ledge_tick(&mut self, env: &Env) {
        match self.ledge_probe(env, self.pos, true) {
            Some(l) => {
                (self.ledge.hang, self.ledge.edge, self.ledge.yaw, self.ledge.moby) = (l.hang, l.edge, l.yaw, l.moby);
                self.ledge.found = true;
            }
            None => self.ledge.found = false,
        }
    }

    /// `0x2c4560` group 3.
    pub(super) fn ledge_entry(&mut self, _anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.group = 3;
        self.cam_mode = 0xd;
        self.items.no_use = 1;
        match id {
            0x17 => {
                self.ledge.g = 0.006_111_111_5;
                self.ledge.z = self.pos[2];
                if self.prev_group == 4 { (self.ledge.g, self.ledge.z) = (self.jump.g, self.jump.ref_z); }
                if !play { return Entry::Done; }
                if self.eff_len_xy <= 0.058_333_337 { Entry::Anim(11.0, 0x20, 1) } else { Entry::Anim(9.0, 0x20, 0) }
            }
            0x18 => if play { Entry::Anim(6.0, 0x21, 0) } else { Entry::Done },
            0x19 => if play { Entry::Anim(6.0, 0x24, 0) } else { Entry::Done },
            _ => if play { Entry::Anim(6.0, 0x25, 0) } else { Entry::Done },
        }
    }

    /// `0x2ba8e0` group 3.
    pub(super) fn phys_ledge(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        let w = sub(self.ledge.edge, self.ledge.hang);
        self.stick_dir = w;
        self.target_yaw = atan2(w[0], w[1]);
        match self.state {
            0x17 | 0x18 => {
                self.turn_to(0.04, 0.2, 0.104_719_76, 0);
                // `0x31f030(0, 0.0785, &pitch)`: the pitch back to level.
                approach(0.0, 0.078_539_82, &mut self.rot[1]);
                let v = sub(self.ledge.hang, self.pos);
                self.vel = if 0.066_666_67 < len(v) { set_len(v, 0.066_666_67) } else { v };
            }
            _ => {
                self.turn_to(0.02, 0.2, 0.078_539_82, 0);
                self.vel = [0.0; 3];
                // Along the edge: the wall direction × down (0x19), down × the wall direction (0x1a), 0.2 long.
                let down = [0.0, 0.0, -1.0];
                let side = if self.state == 0x19 { cross(w, down) } else { cross(down, w) };
                let p = add(self.pos, scale(side, 0.2));
                if self.ledge_probe(env, self.pos, false).is_some() {
                    if let Some(l) = self.ledge_probe(env, p, false) {
                        let d = sub(l.hang, self.pos);
                        let f = (anim.view().frame as usize).min(SHIMMY.len() - 1);
                        self.vel = scale(d, SHIMMY[f] * 0.5);
                    }
                }
            }
        }
        approach(self.pos[2], 0.05, &mut self.ledge.z);
    }

    /// `0x2cb650` group 3.
    pub(super) fn trans_ledge(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let v = anim.view();
        match self.state {
            0x17 => {
                if 13.5 < v.frame { self.set_state(anim, env, 0x18, false); }
                None
            }
            0x18 => {
                if v.flags & 2 != 0 && v.seq_b == 0x20 { self.set_anim(anim, 6.0, 0x21, 0); }
                self.stick_target(env, 1.0, 0);
                if self.ledge_probe(env, self.pos, false).is_none() {
                    self.set_state(anim, env, 6, true);
                    self.ledge.cooldown = 10;
                    return None;
                }
                let back = 0.5 < self.stick_mag && Self::cross_within(env, 9) && 2.356_194_7 < rot_dist(self.target_yaw, self.rot[2]);
                if back || env.pad.pressed & button::R1 != 0 {
                    self.set_state(anim, env, 6, true);
                    self.ledge.cooldown = 10;
                    return None;
                }
                if Self::cross_within(env, 0x11) { return Some(0x1b); }
                if self.stick_mag <= 0.5 { return None; }
                for (side, id) in [(FRAC_PI_2, 0x19), (-FRAC_PI_2, 0x1a)] {
                    let y = add_rot(self.rot[2], side);
                    if rot_dist(self.target_yaw, y) < FRAC_PI_4 {
                        let p = [self.pos[0] + y.cos() * 0.3, self.pos[1] + y.sin() * 0.3, self.pos[2]];
                        if self.ledge_probe(env, p, false).is_some() { return Some(id); }
                        return None;
                    }
                }
                None
            }
            _ => {
                let side = if self.state == 0x19 { FRAC_PI_2 } else { -FRAC_PI_2 };
                let y = add_rot(self.rot[2], side);
                let p = [self.pos[0] + y.cos() * 0.3, self.pos[1] + y.sin() * 0.3, self.pos[2]];
                self.stick_target(env, 1.0, 0);
                let keep = 0.5 <= self.stick_mag && rot_dist(self.target_yaw, y) <= FRAC_PI_4 && self.ledge_probe(env, p, false).is_some();
                if !keep && v.flags & 2 != 0 { return Some(0x18); }
                let back = 0.5 < self.stick_mag && Self::cross_within(env, 9) && 2.356_194_7 < rot_dist(self.target_yaw, self.rot[2]);
                if back {
                    self.set_state(anim, env, 6, true);
                    self.ledge.cooldown = 0x28;
                    return None;
                }
                if Self::cross_within(env, 9) { return Some(0x1b); }
                None
            }
        }
    }
}

const FRAC_PI_2: f32 = std::f32::consts::FRAC_PI_2;
const FRAC_PI_4: f32 = std::f32::consts::FRAC_PI_4;

/// `0x2eef88(out, a, b)` = b × a.
pub(super) fn cross(a: V3, b: V3) -> V3 { [b[1] * a[2] - a[1] * b[2], b[2] * a[0] - a[2] * b[0], b[0] * a[1] - a[0] * b[1]] }
