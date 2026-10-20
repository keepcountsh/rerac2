//! The Swingshot (group 0xd): the aim 0x23, the pull 0x24 and the swing 0x25 — SetState (L00 `0x2c4560`), physics
//! (`0x2ba8e0`) and transitions (`0x2cb650`), with the node search `0x2c4098` (class 0x2f6, the item trigger
//! `0x2c96d0`'s case 0xd).
//!
//! * The search: over the nodes not hidden, the one with the lowest `0x2ae400` score within its range (`+0x68`[8],
//!   30 if 0) and the cones (0.524 in yaw; in the look stance 0.087 both), at least 4 away across, drawn ones
//!   preferred (+10 for a hidden one), with a clear line from half a metre up.
//! * 0x23: standing, facing the node (0.02, 0.15, 0.1048), while the Swingshot is in hand (its hook flies; the
//!   item's moby sets the next state).
//! * 0x24: pulled to the node: the speed rises to 0.45 by 1/45 (substate 0) until the parabola `−0.00583·t² + v·t +
//!   1.9 − d` has a root ahead (substate 1: it falls by 0.0117), the body pitches toward the line until 20 ticks out;
//!   the velocity aims the left hand (0.57 aside) at the node, sinking `(30 − eta)·0.00806` in the last 30 ticks.
//!   Letting go (or slowing under 1/15 in substate 1): the fall, or idle on the ground.
//! * 0x25: swinging round the node: the radius eases to the node's (`+0x68`[4]) by 1/6, the velocity is the
//!   displacement × 0.9 with 1/240 of gravity, kept on the circle. Letting go: the fall above 0.4, else idle.
//!
//! Not ported: the hook moby and its line (the item's class), the sounds, the stats counters.

use super::physics::*;
use super::states::Entry;
use super::Hero;
use rc_game::hero::anim::AnimCtl;

/// A Swingshot node (class 0x2f6) as the search sees it.
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub id: usize,
    pub pos: V3,
    /// `+0x68`[0] ≠ 0: in use; [4]: the swing radius; [8]: its kind; [0x20]: the range (0: 30).
    pub busy: bool,
    pub radius: f32,
    pub kind: f32,
    pub range: f32,
    pub drawn: bool,
    pub hidden: bool,
}

/// The Swingshot block (`0x18a8c0..0x18a8ec`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SwingBlock {
    /// `0x18a8c4` the node, `0x18a8c8` it is in sight; its position, `0x18a8e0` its radius, `0x18a8d8` its kind,
    /// `0x18a8e4` its score.
    pub node: Option<usize>,
    pub found: bool,
    pub node_pos: V3,
    pub node_radius: f32,
    pub node_kind: f32,
    pub score: f32,
    /// `0x18a8c0` the pull's speed, `0x18a8cc` the ticks to the node, `0x18a8d0` the distance, `0x18a8d4` close,
    /// `0x18a8dc` the swing's radius, `0x18a8e8` let go early.
    pub speed: f32,
    pub eta: f32,
    pub dist: f32,
    pub close: bool,
    pub radius: f32,
    pub early: bool,
}

impl Hero {
    /// `0x2c4098`: the node search (module docs). Returns whether a node is in sight.
    pub fn swing_search(&mut self, env: &Env, nodes: &[Node]) -> bool {
        if self.group == 0xd { return self.swing.found; }
        self.swing.found = false;
        let look = self.state == 1 || self.state == 0x1d;
        let (yc, pc) = if look { (0.087_266_47, 0.087_266_47) } else { (0.523_598_8, -1.0) };
        let mut best = 1e8;
        let mut pick = None;
        for n in nodes.iter().filter(|n| !n.hidden && !n.busy) {
            let range = if n.range == 0.0 { 30.0 } else { n.range };
            if n.range != 0.0 && n.range < dist(self.pos, n.pos) { continue; }
            let Some(score) = self.cone_score(env, self.rot[2], range, yc, pc, n.pos) else { continue };
            if dist2(self.pos, n.pos) < 4.0 { continue; }
            let score = if n.drawn { score } else { score + 10.0 };
            if score < best {
                best = score;
                pick = Some(*n);
            }
        }
        if let Some(n) = pick {
            let s = &mut self.swing;
            (s.node, s.node_pos, s.node_radius, s.node_kind, s.score) = (Some(n.id), n.pos, n.radius, n.kind, best);
            let a = add(self.pos, [0.0, 0.0, 0.5]);
            if env.line(a, n.pos, 2).is_none() { self.swing.found = true; }
        }
        self.swing.found
    }

    /// `0x2ae400(yaw, range, yaw cone, pitch cone, moby)`: the target score `d + d·(yaw off + pitch off)`, None out of
    /// range or outside a cone (in the look stance measured from the camera).
    pub fn cone_score(&self, env: &Env, yaw: f32, range: f32, yaw_cone: f32, pitch_cone: f32, at: V3) -> Option<f32> {
        let eye = if self.state == 0x14 { [self.pos[0], self.pos[1], self.ground_z] } else { self.pos };
        let d = dist(eye, at);
        let mut out = range < d;
        let mut a_yaw = rot_dist(atan2(at[0] - eye[0], at[1] - eye[1]), yaw);
        let mut a_pitch = rot_dist(atan2(dist2(eye, at), at[2] - eye[2]), 0.0);
        let mut off = a_yaw;
        if self.state == 1 || self.state == 0x1d {
            let c = env.cam_pos;
            a_yaw = rot_dist(atan2(at[0] - c[0], at[1] - c[1]), env.cam_yaw);
            a_pitch = rot_dist(atan2(dist2(c, at), at[2] - c[2]), 0.0);
            off = a_yaw + a_pitch;
            a_yaw = a_yaw.max(a_pitch);
        }
        if 0.0 < yaw_cone && yaw_cone < a_yaw { out = true; }
        if 0.0 < pitch_cone && pitch_cone < a_pitch { out = true; }
        (!out).then_some(d + off * d)
    }

    /// `0x2c96d0`'s case 0xd (the Swingshot in hand): the search, then the trigger pressed within 5 ticks (8 in the
    /// air, 11 from the rail jump 0x28) or held once the item has been out 16 ticks → the aim 0x23 at a node in sight.
    /// Returns whether a state was set.
    pub fn swingshot_trigger(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        let it = self.items;
        if it.no_use != 0 || self.mode != 0 || it.hand.is_none() || it.item != 0xd { return false; }
        self.swing_search(env, env.nodes);
        let n = if self.state == 0x28 { 0xb } else if self.air_ticks != 0 { 8 } else { 5 };
        let ok_group = self.group <= 1 || matches!(self.group, 0xc | 4 | 2 | 0x1e | 5) || (self.state == 0x28 && 6 <= self.timer) || self.state == 0x1d;
        if !ok_group { return false; }
        if env.pad.pressed_within(it.trigger, n).is_none() {
            if env.pad.held & it.trigger == 0 || it.item_ticks < 0x10 { return false; }
        }
        if self.swing.found { return self.set_state(anim, env, 0x23, true); }
        false
    }

    pub(super) fn swing_entry(&mut self, anim: &mut dyn AnimCtl, _env: &Env, id: i32, play: bool) -> Entry {
        self.group = 0xd;
        self.cam_mode = 7;
        match id {
            0x23 => {
                let n = self.swing.node_pos;
                self.target_yaw = atan2(n[0] - self.pos[0], n[1] - self.pos[1]);
                if self.prev_state == 0x28 { self.momentum = self.vel; }
                if play {
                    self.set_anim(anim, 5.0, 0x2f, 2);
                    anim.set_loop(10, 0xe);
                }
                Entry::Done
            }
            _ => {
                self.swing.speed = 0.0;
                self.swing.close = false;
                if id == 0x25 { self.swing.radius = dist(self.pos, self.swing.node_pos); }
                if play { self.set_anim(anim, if id == 0x24 { 10.0 } else { 5.0 }, 0x30, 1); }
                Entry::Done
            }
        }
    }

    pub(super) fn phys_swing(&mut self, env: &Env, _anim: &mut dyn AnimCtl) {
        let node = self.swing.node_pos;
        match self.state {
            0x23 => {
                self.edge_brake(env, 3.7, 0.0);
                self.target_speed = 0.0;
                self.speed_step(0.001_232_998, 0.001_232_998);
                self.vel = [0.0; 3];
                self.momentum_decay(0.001_232_998);
                if self.air_ticks == 0 {
                    self.vel[2] -= 0.015;
                } else {
                    self.gravity_from(self.eff[2], 0.006_111_111);
                    self.edge_slide();
                }
                if self.swing.node.is_some() {
                    self.target_yaw = atan2(node[0] - self.pos[0], node[1] - self.pos[1]);
                    self.turn_to(0.02, 0.15, 0.104_719_76, 0);
                }
            }
            0x24 => {
                let s = &mut self.swing;
                s.dist = dist(self.pos, node);
                s.eta = if 0.0 < s.speed && 0.0 < self.eff_len { s.dist / self.eff_len } else { 60.0 };
                if self.substate == 0 {
                    approach(0.45, 0.022_222_223, &mut self.swing.speed);
                    self.target_yaw = atan2(node[0] - self.pos[0], node[1] - self.pos[1]);
                    self.turn_to(0.03, 0.15, 0.122_173_05, 0);
                } else {
                    approach(0.0, 0.011_666_667, &mut self.swing.speed);
                }
                let s = self.swing;
                let (a, b, c) = (-0.005_833_333_4, s.speed, 1.9 - s.dist);
                let disc = b * b - 4.0 * a * c;
                if 0.0 < disc {
                    let r = disc.sqrt();
                    let t = ((-b + r) / (a + a)).max((-b - r) / (a + a));
                    if 0.0 < t {
                        self.substate = 1;
                        self.swing.eta = t;
                    }
                }
                if self.swing.eta < 20.0 { self.swing.close = true; }
                let (target, step) = if self.swing.close {
                    (0.0, 0.064_028_04)
                } else {
                    let p = atan2(dist2(self.pos, node), node[2] - self.pos[2]);
                    (add_rot(-p, PI / 2.0), 0.107_626_7)
                };
                let d = angle_diff(target, self.rot[1], 0).clamp(-step, step);
                self.rot[1] = add_rot(self.rot[1], d);
                let side = add_rot(self.rot[2], -PI / 2.0);
                let (sn, cs) = side.sin_cos();
                let hand = add(self.pos, [cs * -0.57, sn * -0.57, 0.0]);
                self.vel = set_len(sub(node, hand), self.swing.speed);
                if self.swing.eta < 30.0 { self.vel[2] -= (30.0 - self.swing.eta) * 0.008_055_556; }
            }
            _ => {
                approach(self.swing.node_radius, 1.0 / 6.0, &mut self.swing.radius);
                self.vel = scale(self.disp, 0.9);
                self.vel[2] -= 0.004_166_667;
                self.swing.dist = dist(self.pos, node);
                let d = set_len(sub(add(self.pos, self.vel), node), self.swing.radius);
                self.vel = sub(add(node, d), self.pos);
            }
        }
    }

    pub(super) fn trans_swing(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> Option<i32> {
        let held = env.pad.held & self.items.trigger != 0;
        match self.state {
            0x23 => (self.items.item != 0xd).then_some(0),
            0x24 => {
                let v = anim.view();
                self.anim_speed_for_ticks(&v, 10.0, self.swing.eta, 0.5, -1.0);
                if self.swing.eta < 22.0 && v.seq_b != 10 { self.set_anim(anim, 16.0, 10, 0); }
                let mut go = false;
                if !held {
                    if 25.0 < self.swing.eta { self.swing.early = true; }
                    go = true;
                }
                if self.substate == 1 && self.swing.speed < 1.0 / 15.0 { go = true; }
                if !go { return None; }
                self.speed = self.swing.speed.min(0.116_666_675);
                if self.height < 0.3 {
                    self.set_state(anim, env, 0, false);
                    let seq = self.idle_seq();
                    self.set_anim(anim, 30.0, seq, 0);
                    return None;
                }
                self.set_state(anim, env, 6, false);
                if anim.view().seq_b != 10 { self.set_anim(anim, 17.0, 10, 0); }
                self.fall_dec = 0.011_666_667;
                self.fall_g = 0.008_055_556;
                self.cam_mode = 0;
                None
            }
            _ => {
                if held { return None; }
                self.speed = self.swing.speed.min(0.116_666_675);
                if 0.4 <= self.height {
                    self.set_state(anim, env, 6, true);
                    self.cam_mode = 0;
                    return None;
                }
                Some(0)
            }
        }
    }
}
