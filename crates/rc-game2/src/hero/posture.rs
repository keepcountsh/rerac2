//! Ratchet's posture: the joint records (L00 `0x25bd90`, 40 of 0xb0 bytes; the 18 on Ratchet's own joints are
//! ported) that turn single joints of his model toward per-tick targets through springs, and the routines that set
//! those targets. The first 0x40 bytes of a record are a joint-modifier node the record links into his moby's list
//! (`+0x54`, `0x312480`) while it has anything to do: the engine's evaluator composes it onto the joint the record's
//! joint list names (`rc_formats::moby_anim::list_target`). R&C1 has the same block (`rc_game::hero::idle::JointRec`).
//!
//! **Each tick**, after the transitions (`0x2a6dd0`): the feet and legs tilt to the ground under each foot (`0x2ac218`,
//! from the ground probe's foot lines `0x2b4a08`), the head looks around while idle and the head's scale holds at
//! 0.875 (`0x2ab498`), the ears and tail twitch while idle (`0x2ab9c0`), the neck follows the head and the eyes blink
//! (`0x2a5518` / `0x2a5480`), then every record springs (`0x2a5368` → `0x2a5048`). The walk and the fall lean in their
//! physics (`0x2b7c88`): strafing, the hips turn toward the move and the chest back (so the run plays sideways); the
//! run leans into its turns, the walk sways, the fall twists with the yaw.
//!
//! The head looks at the enemies (`0x2abc18`): every seventh frame the best of the targets in reach (their look data),
//! held while nothing is in the way of the eyes. A weapon's aim (`0x18aa18`, set by its update each tick it holds a
//! lock) turns the spine and chest onto the target instead.
//!
//! Not ported: the arms at a wall (`0x2ab250`: only for a held item whose hold kind, `0x26250c` of its row, is not 0;
//! the wrench's is 0), the teeter at an edge (the ahead probe's edge test), the lean on gravity frames and magnetic walkways
//! (`0x2b7820`), the dust at the feet, the records of Clank on the back and of Clank's body (kinds 1..5).

use super::physics::*;
use super::Hero;
use rc_formats::moby_anim::JointModifier;
use rc_game::hero::anim::AnimCtl;

/// One record: `+0xa0` its joint list, `+0x01` linked, `+0x40` / `+0x50` / `+0x60` the Euler angles, their spring
/// velocities and this tick's targets, `+0x70` / `+0x80` / `+0x90` the same for the translation, `+0xa4` / `+0xa8` the
/// spring's stiffness and damping, `+0xac` the scale (back to 1 after each spring); the node's scale is the last
/// active tick's.
#[derive(Clone, Copy, Debug)]
pub struct PoseRec {
    pub list: u8,
    pub linked: bool,
    pub cur: V3,
    pub vel: V3,
    pub target: V3,
    pub trans: V3,
    pub trans_vel: V3,
    pub trans_target: V3,
    pub k: f32,
    pub d: f32,
    pub scale: f32,
    pub node_scale: f32,
}

impl PoseRec {
    const fn new(list: u8, k: f32, d: f32) -> PoseRec {
        PoseRec { list, linked: false, cur: [0.0; 3], vel: [0.0; 3], target: [0.0; 3], trans: [0.0; 3], trans_vel: [0.0; 3], trans_target: [0.0; 3], k, d, scale: 1.0, node_scale: 1.0 }
    }

    /// `0x2a5048`: with a target, a translation, a scale ≠ 1 or an angle still out (≥ 0.005; a translation ≥ 0.003),
    /// the angle springs `0x31f0c8(target, k, d, 0, …, 2)` and the translation's `0x31ebd8(target, k, d, 0, …)`, and
    /// the node is linked with the new pose; else it is unlinked. Then the targets clear and the scale is 1 again.
    fn update(&mut self) {
        let active = self.target.iter().any(|&t| t != 0.0)
            || self.trans_target.iter().any(|&t| t != 0.0)
            || self.trans.iter().any(|&x| 0.003 <= x.abs())
            || self.scale != 1.0
            || self.cur.iter().any(|&c| 0.005 <= c.abs());
        if active {
            for i in 0..3 {
                turn_spring(self.target[i], self.k, self.d, 0.0, &mut self.cur[i], &mut self.vel[i], 2);
            }
            for i in 0..3 {
                spring(self.trans_target[i], self.k, self.d, 0.0, &mut self.trans[i], &mut self.trans_vel[i]);
            }
            self.linked = true;
            self.node_scale = self.scale;
        } else {
            self.linked = false;
        }
        self.target = [0.0; 3];
        self.trans_target = [0.0; 3];
        self.scale = 1.0;
    }

    fn set_spring(&mut self, k: f32, d: f32) {
        self.k = k;
        self.d = d;
    }
}

/// The records on Ratchet's joints, `{list, k, d}` as the level's data has them (0..3 get theirs from the routines).
const RECS: [(u8, f32, f32); 18] = [
    (9, 0.0, 0.0),
    (10, 0.0, 0.0),
    (11, 0.0, 0.0),
    (4, 0.0, 0.0),
    (22, 0.1, 0.3),
    (23, 0.1, 0.3),
    (7, 0.1, 0.3),
    (8, 0.1, 0.3),
    (13, 0.0, 0.0),
    (12, 0.0, 0.0),
    (6, 0.0, 0.0),
    (0, 0.0, 0.0),
    (21, 0.12, 0.3),
    (25, 0.015, 0.3),
    (26, 0.015, 0.3),
    (27, 0.013, 0.25),
    (28, 0.017, 0.21),
    (24, 0.03, 0.2),
];

/// Record indices.
pub mod rec {
    /// List 9: the hips (the strafe's turn, the leans' x).
    pub const HIPS: usize = 0;
    /// List 10: the chest (turned back while strafing; follows the head).
    pub const CHEST: usize = 1;
    /// List 11.
    pub const SPINE: usize = 2;
    /// List 4: the head.
    pub const HEAD: usize = 3;
    /// Lists 22 / 23: the feet; 7 / 8: the legs.
    pub const FOOT_L: usize = 4;
    pub const FOOT_R: usize = 5;
    pub const LEG_L: usize = 6;
    pub const LEG_R: usize = 7;
    /// List 21: the neck (follows the head).
    pub const NECK: usize = 12;
    /// Lists 25..28: the ears and the tail.
    pub const SECONDARY: usize = 13;
    /// List 24: scaled 0.875 (1.57 with the big-head cheat `0x1a7a49`).
    pub const SCALED: usize = 17;
}

/// The blink (`0x18ace0`, seven nodes of mode 1 on the lists 15..21, `0x1a7e40`): their weight over the blink's 11
/// ticks (`0x1a7e10`), scales (`0x25dd80`) and translations (`0x25ddf0`); the rotations are the identity.
const BLINK_LISTS: [u8; 7] = [15, 16, 17, 18, 19, 20, 21];
const BLINK_WEIGHTS: [f32; 12] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0];
const BLINK_SCALE: [V3; 7] = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.91], [1.0, 1.0, 0.91], [1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [0.98, 1.0, 1.0]];
const BLINK_TRANS: [V3; 7] = [
    [210.881_8, 85.626_5, 652.459_9],
    [210.881_8, -85.626_5, 652.459_9],
    [782.668_9, 518.005_1, -520.835_7],
    [782.668_9, -513.759_1, -520.835_7],
    [985.058_8, 564.710_4, 78.549_9],
    [985.058_8, -560.464_5, 78.549_9],
    [312.784_5, 2.123, 486.160_5],
];

/// The states whose SetState arms the blink (`0x18afa8` = 0x68; every other state clears it).
pub const BLINK_STATES: [i32; 18] = [0, 0x70, 4, 0x74, 8, 0x18, 0x27, 0x32, 0x33, 0x34, 0x35, 0x36, 0x64, 0x65, 0x71, 0x78, 0x7b, 0x82];

/// The ground under each foot (`0x18a120..0x18a148`, the end of `0x2b4a08`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Feet {
    /// The foot's ground above the ground under Ratchet (`0x18a120`), its pitch (`0x18a128`) and roll (`0x18a130`),
    /// left then right; the ground's own (`0x18a144` / `0x18a148`).
    pub height: [f32; 2],
    pub pitch: [f32; 2],
    pub roll: [f32; 2],
    pub ground_pitch: f32,
    pub ground_roll: f32,
}

/// The posture block.
#[derive(Clone, Debug)]
pub struct Posture {
    pub recs: [PoseRec; 18],
    pub feet: Feet,
    /// `0x18a008`: ticks since the last strafing tick.
    pub not_strafing: i32,
    /// `$gp − 0x72fc`: the scaled joint's scale, easing to 0.875.
    pub head_scale: f32,
    /// The idle look: `0x18afc0` its timer, `0x18afb4` / `0x18afb8` the head's y / z, `0x18afc4` / `0x18afc8` its
    /// spring.
    pub look_timer: i32,
    pub look: [f32; 2],
    pub look_spring: [f32; 2],
    /// The twitches: `0x18b010..` their timers, `0x18afd4 + 0x10·i` their y / z.
    pub twitch_timer: [i32; 4],
    pub twitch: [[f32; 2]; 4],
    /// The blink: `0x18afa0` its tick (0 none), `0x18afa4` the next one's frame, `0x18afa8` the interval (0 off);
    /// the nodes linked (`0x18ace1`).
    pub blink: i32,
    pub blink_next: i32,
    pub blink_every: i32,
    pub blink_linked: bool,
    /// `0x18c2b0`: the moby the head looks at.
    pub look_at: Option<usize>,
}

impl Default for Posture {
    fn default() -> Self {
        Posture {
            recs: RECS.map(|(l, k, d)| PoseRec::new(l, k, d)),
            feet: Feet::default(),
            not_strafing: 0,
            head_scale: 0.875,
            look_timer: 0,
            look: [0.0; 2],
            look_spring: [0.007, 0.3],
            twitch_timer: [0; 4],
            twitch: [[0.0; 2]; 4],
            blink: 0,
            blink_next: 0,
            blink_every: 0,
            blink_linked: false,
            look_at: None,
        }
    }
}

/// `0x31e8d8(a, b, t)`: `a + (b − a)·t`.
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

impl Posture {
    /// The modifier list for the evaluator, the blink's nodes first (the last linked are at the list's head):
    /// `targets[list]` = the joint the class's joint list `list` acts on.
    pub fn modifiers(&self, targets: &[u8]) -> Vec<JointModifier> {
        let mut out = Vec::new();
        let target = |l: u8| targets.get(l as usize).copied().filter(|&t| t != 0xff);
        if self.blink_linked {
            let w = BLINK_WEIGHTS[self.blink.clamp(0, 11) as usize];
            for (i, &l) in BLINK_LISTS.iter().enumerate() {
                let Some(t) = target(l) else { continue };
                out.push(JointModifier { joint: t, mode: 1, weight: w, quat: [0.0, 0.0, 0.0, 1.0], scale: BLINK_SCALE[i], trans: BLINK_TRANS[i] });
            }
        }
        for r in self.recs.iter().filter(|r| r.linked) {
            let Some(t) = target(r.list) else { continue };
            let s = r.node_scale;
            out.push(JointModifier { quat: rc_game::hero::idle::euler_quat(r.cur), scale: [s, s, s], trans: r.trans, ..JointModifier::compose(t) });
        }
        out
    }
}

impl Hero {
    /// The local frame's view of a world vector (`0x2b8f50`): `[roll, pitch]` of a normal — its tilt toward the left
    /// (`atan2(y, √(x² + z²))`) and toward the front (`−atan2(x, z)`).
    fn local_tilt(&self, n: V3) -> [f32; 2] {
        let l = [dot(n, self.rows[0]), dot(n, self.rows[1]), dot(n, self.rows[2])];
        [l[1].atan2((l[0] * l[0] + l[2] * l[2]).sqrt()), -l[0].atan2(l[2])]
    }

    /// The end of `0x2b4a08`: within 0.25 of the ground in the ground, walk, jump-landing and script groups, the
    /// ground's tilt, then a line at each foot (0.2 to the side, 0.45 up to 0.45 down, flags 0x22): its height above
    /// the ground under Ratchet (none past 0.4) and its tilt within 45° (none: the ground's).
    pub(super) fn foot_probe(&mut self, env: &Env) {
        let f = &mut self.posture.feet;
        *f = Feet::default();
        let landing = self.group == 4 && self.jump.landed != 0;
        if 0.25 <= self.height || !(matches!(self.group, 0 | 0xc | 9 | 7 | 1) || landing) { return; }
        let q = std::f32::consts::FRAC_PI_4;
        let [r, p] = self.local_tilt(self.ground_normal);
        if -q < p && p < q { self.posture.feet.ground_pitch = p; }
        if -q < r && r < q { self.posture.feet.ground_roll = r; }
        for i in 0..2 {
            let side = if i == 0 { 0.2 } else { -0.2 };
            let (a, b) = (self.local_point(0.0, side, 0.45), self.local_point(0.0, side, -0.45));
            let hit = env.line(a, b, 0x22);
            let (gp, gr, gz) = (self.posture.feet.ground_pitch, self.posture.feet.ground_roll, self.ground_z);
            match hit {
                None => {
                    let f = &mut self.posture.feet;
                    (f.height[i], f.pitch[i], f.roll[i]) = (0.0, gp, gr);
                }
                Some(o) => {
                    let mut h = o.point[2] - gz;
                    if 0.4 < h.abs() { h = 0.0; }
                    let [r, p] = self.local_tilt(unit(o.normal));
                    let f = &mut self.posture.feet;
                    f.height[i] = h;
                    if -q < p && p < q { f.pitch[i] = p; }
                    if -q < r && r < q { f.roll[i] = r; }
                }
            }
        }
    }

    /// `0x2adff8`: the states that keep the feet as they are (the groups 0x11 / 0x12, 0x67, 0x7f, 0x72, 0x73).
    fn feet_kept(&self) -> bool { matches!(self.group, 0x11 | 0x12) || matches!(self.state, 0x67 | 0x7f | 0x72 | 0x73) }

    /// `0x2ac218`: the feet pitch and roll with the ground under them and the legs reach for it: each leg turns
    /// `atan2(height, 0.2)·1.2` (the left −10°..45°, the right −45°..10°), the foot its roll less that, when a foot
    /// is more than 0.015 off or tilted more than 1° (not on a magnetic walkway).
    fn feet_tilt(&mut self) {
        if self.feet_kept() { return; }
        let f = self.posture.feet;
        let deg = 0.017_453_294;
        let off = f.height.iter().any(|h| 0.015 < h.abs()) || f.pitch.iter().chain(f.roll.iter()).any(|a| deg < a.abs());
        if !off || self.surf.boots == 1 { return; }
        let r = &mut self.posture.recs;
        r[rec::FOOT_L].target[1] = -f.pitch[0];
        r[rec::FOOT_R].target[1] = -f.pitch[1];
        let left = (f.height[0].atan2(0.2) * 1.2).min(0.785_398_24).max(-0.174_532_95);
        let right = (-(f.height[1].atan2(0.2)) * 1.2).min(0.174_532_95).max(-0.785_398_24);
        r[rec::LEG_L].target[0] = left;
        r[rec::LEG_R].target[0] = right;
        r[rec::FOOT_L].target[0] = -sub_rot(f.roll[0], -left);
        r[rec::FOOT_R].target[0] = -sub_rot(f.roll[1], -right);
    }

    /// `0x2b7c88`, from the walk's and the fall's physics: the leans (module docs).
    pub(super) fn lean(&mut self) {
        let res = self.yaw_residual;
        let p = &mut self.posture;
        let r = &mut p.recs;
        if self.strafe != 0 && 0.15 < self.stick_mag && self.state != 0xb && !self.strafe_back {
            r[rec::HIPS].set_spring(0.037, 0.27);
            r[rec::CHEST].set_spring(0.025, 0.27);
            let z = sub_rot(self.strafe_yaw, self.rot[2]);
            r[rec::HIPS].target[2] = z;
            r[rec::CHEST].target[2] = -z;
            return;
        }
        let legs = |r: &mut [PoseRec; 18], z: f32| {
            r[13].target[2] = z * 0.35;
            r[14].target[2] = z * 0.3;
            r[15].target[2] = r[14].target[2];
            r[16].target[2] = r[14].target[2];
        };
        if self.state != 2 {
            if self.group != 2 && self.group != 4 { return; }
            for i in 0..3 { r[i].set_spring(0.04, 0.2); }
            r[rec::HEAD].set_spring(0.02, 0.2);
            let f = res.clamp(-1.4, 1.4);
            r[rec::CHEST].target[2] = f * 0.29;
            r[rec::SPINE].target[0] = f * -0.27;
            r[rec::HEAD].target[2] = f * 0.87;
            r[13].target[2] = -f * 0.35;
            r[14].target[2] = -f * 0.3;
            r[15].target[2] = r[14].target[2];
            r[16].target[2] = r[14].target[2];
            return;
        }
        for i in 0..3 { r[i].set_spring(0.04, 0.2); }
        r[rec::HEAD].set_spring(0.02, 0.2);
        if self.substate != 1 {
            // The walk sways with the turn left to do (eased out over the first 40 ticks under 0.8 of the stick).
            let mut a = res;
            if self.stick_mag < 0.8 && self.timer < 0x28 { approach(0.0, (0x28 - self.timer) as f32 * 0.087_266_47, &mut a); }
            let f = (a * 0.5).clamp(-0.8, 0.8);
            let g = f * f.abs() * 1.25;
            r[rec::CHEST].target[2] = g;
            r[rec::HEAD].target[2] = g;
            r[rec::HEAD].target[1] = g.abs() * 0.25;
            r[13].target[2] = -g * 0.35;
            r[14].target[2] = -g * 0.3;
            r[15].target[2] = r[14].target[2];
            r[16].target[2] = r[14].target[2];
            return;
        }
        // The run leans into the turn (its yaw speed ×17, at most 1.4 and three times the turn left; a quick turn
        // 1.1 and the turn left, with stiffer springs).
        let mut f = (self.yaw_vel * 17.0).clamp(-1.4, 1.4);
        let lim = res.abs() * 3.0;
        f = f.clamp(-lim, lim);
        if self.quick_turn != 0 {
            f = f.clamp(-1.1, 1.1);
            r[rec::CHEST].set_spring(0.055, 0.3);
            r[rec::HEAD].set_spring(0.07, 0.3);
            f = f.clamp(-res.abs(), res.abs());
        }
        let g = -f;
        let mut hips = g * 0.11;
        if self.quick_turn != 0 { hips *= 0.25; }
        hips *= (self.eff_len_xy * 10.526_316).clamp(0.0, 1.0);
        r[rec::HIPS].target[0] = hips;
        r[rec::CHEST].target[2] = if self.quick_turn != 0 { 0.0 } else { f * 0.31 };
        r[rec::SPINE].target[0] = f * -0.15;
        r[rec::HEAD].target[2] = f * 0.7;
        r[rec::HEAD].target[1] = f.abs() * -0.15;
        r[rec::HEAD].target[0] = f.abs() * 0.16;
        legs(r, g);
        r[13].target[1] = f.abs() * 0.35;
        r[14].target[1] = f.abs() * 0.2;
    }

    /// `0x2ab498`: the scaled joint; and, 20 ticks after the last strafe, idle in the sequences 0..2 or 0x3a, the head
    /// looks somewhere new every 90..280 ticks (−25°..9° down, 20°..57° or 50°..70° near a wall to a random side; a
    /// fidget's look within 20° and 15°) with a random spring, the chest following at half.
    fn head_idle(&mut self, anim: &mut dyn AnimCtl) {
        let p = &mut self.posture;
        p.recs[rec::SCALED].scale = p.head_scale;
        approach(0.875, 0.05, &mut p.head_scale);
        if p.not_strafing <= 0x13 { return; }
        // A weapon in use or aiming (`0x18c0d8`, `0x18aa18`): the look and the twitches cleared (`0x2ab458`), the
        // head and the chest on soft springs, the next look and fidget 90..120 ticks off.
        if self.items.using || self.items.aim_target.is_some() {
            p.look = [0.0; 2];
            p.twitch = [[0.0; 2]; 4];
            p.recs[rec::HEAD].set_spring(0.03, 0.2);
            p.recs[rec::CHEST].set_spring(0.03, 0.2);
            p.look_timer = self.rng.rand_range(0x5a, 0x78);
            self.fidget_gap = self.rng.rand_range(0x5a, 0x78) as i16;
            return;
        }
        let seq = anim.view().seq_b;
        if self.state != 0 || !(seq < 3 || seq == 0x3a) { return; }
        if seq == 1 || seq == 2 { p.look_timer = self.rng.rand_range(0x28, 0x46); }
        if crate::moby::countdown(&mut p.look_timer) {
            p.look[0] = -self.rng.randf(-0.157_079_63, 0.436_332_3);
            p.look[1] = if self.ahead.dist < 1.5 { self.rng.randf_signed(0.872_664_6, 1.221_730_5) } else { self.rng.randf_signed(0.349_065_9, 0.994_837_7) };
            let t = self.rng.randf(0.0, 1.0);
            p.look_spring = [lerp(0.015, 0.007, t), lerp(0.29, 0.3, t)];
            p.look_timer = self.rng.rand_range(0x5a, 200) + (t * 80.0) as i32;
        }
        if seq == 1 || seq == 2 {
            p.look[1] = p.look[1].clamp(-0.349_065_9, 0.349_065_9);
            p.look[0] = p.look[0].clamp(0.0, 0.261_799_42);
        }
        let [k, d] = p.look_spring;
        let r = &mut p.recs;
        r[rec::HEAD].set_spring(k, d);
        r[rec::HEAD].target[1] = p.look[0];
        r[rec::HEAD].target[2] = p.look[1];
        r[rec::CHEST].set_spring(0.002, 0.3);
        r[rec::CHEST].target[2] = r[rec::HEAD].target[2] * 0.52;
        r[rec::CHEST].target[1] = r[rec::HEAD].target[1] * 0.55;
    }

    /// `0x2abc18`: in the ground, walk, jump, fall and swim groups, every seventh frame the head picks the target to
    /// look at: an enemy within its look range, within 110° of the facing and 60° of the level, by the distance
    /// weighted by the angle (the current one 1.5 ahead), a higher priority first; dropped when a line from the eyes
    /// (1.2 up) to it (0.5 up) meets something. The head turns to its look height (yaw within 59°, pitch −25°..35°;
    /// while invincible past 27 ticks 17° and level) from 0.3 ahead and 0.85 up, the spine with it.
    fn head_look(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        // `0x18aa18`: a weapon's aim of last tick, read once (cleared at the end).
        let aim = self.items.aim_target.take();
        if !matches!(self.group, 0 | 1 | 2 | 4 | 7) {
            self.posture.look_at = None;
            return;
        }
        // Aiming: the spine turns to the target's aim point (yaw within 59°) and the chest to its pitch (−25°..35°)
        // from 0.5 ahead and 0.55 up; while invincible past 27 ticks 17° and level.
        if let Some((id, up)) = aim {
            if let Some(t) = env.targets.iter().find(|t| t.id == id) {
                let p = [t.pos[0], t.pos[1], t.pos[2] + up];
                let from = self.local_point(0.5, 0.0, 0.55);
                let mut y = sub_rot(atan2(p[0] - from[0], p[1] - from[1]), self.rot[2]).clamp(-1.029_744_4, 1.029_744_4);
                let mut pitch = atan2(dist2(p, from), from[2] - p[2]).min(0.610_865_3).max(-0.436_332_35);
                if 0x1b < self.damage.invincible {
                    y = y.clamp(-0.296_706, 0.296_706);
                    pitch = 0.0;
                }
                let r = &mut self.posture.recs;
                r[rec::SPINE].set_spring(0.037, 0.3);
                r[rec::SPINE].target[2] = y;
                r[rec::CHEST].set_spring(0.037, 0.3);
                r[rec::CHEST].target[1] = pitch;
                return;
            }
        }
        if anim.view().seq_b == 0x51 { return; }
        let pos = self.pos;
        let yaw = self.rot[2];
        if self.frame % 7 == 0 {
            let (mut best, mut best_score, mut best_prio) = (None, 1e8f32, 0.0f32);
            for t in env.targets {
                let Some(look) = t.look else { continue };
                let d = dist2(pos, t.pos);
                if (look[0] as f32) < d { continue; }
                let ang = rot_dist(atan2(t.pos[0] - pos[0], t.pos[1] - pos[1]), yaw);
                if 1.919_862_4 < ang { continue; }
                if 1.047_197_7 < atan2(dist2(t.pos, pos), pos[2] - t.pos[2]).abs() { continue; }
                let mut score = ang * d + d;
                if Some(t.id) == self.posture.look_at { score = (score - 1.5).max(0.0); }
                let prio = look[1] as f32;
                if best_prio < prio || score < best_score {
                    best_score = score;
                    best = Some(t.id);
                    if best_prio < prio { best_prio = prio; }
                }
            }
            self.posture.look_at = best;
        }
        let Some(id) = self.posture.look_at else { return };
        let Some(t) = env.targets.iter().find(|t| t.id == id) else {
            self.posture.look_at = None;
            return;
        };
        let eye = self.local_point(0.0, 0.0, 1.2);
        if env.line(eye, [t.pos[0], t.pos[1], t.pos[2] + 0.5], 2).is_some() {
            self.posture.look_at = None;
            return;
        }
        let mut p = t.pos;
        if let Some(look) = t.look { p[2] += look[2] as f32 * 0.125; }
        let from = self.local_point(0.3, 0.0, 0.85);
        let mut y = sub_rot(atan2(p[0] - from[0], p[1] - from[1]), yaw).clamp(-1.029_744_4, 1.029_744_4);
        let mut pitch = atan2(dist2(p, from), from[2] - p[2]).min(0.610_865_3).max(-0.436_332_35);
        if 0x1b < self.damage.invincible {
            y = y.clamp(-0.296_706, 0.296_706);
            pitch = 0.0;
        }
        let r = &mut self.posture.recs;
        r[rec::HEAD].set_spring(0.021, 0.27);
        r[rec::HEAD].target[2] = y * 0.7;
        r[rec::HEAD].target[1] = pitch;
        r[rec::SPINE].set_spring(0.014, 0.3);
        r[rec::SPINE].target[2] = r[rec::HEAD].target[2] * 0.55;
    }

    /// `0x2ab9c0`: idle in sequence 0, the ears and the tail (records 13..16) twitch to a new pose each time their
    /// timer runs out.
    fn twitch(&mut self, anim: &mut dyn AnimCtl) {
        if self.state != 0 || anim.view().seq_b != 0 { return; }
        for i in 0..4 {
            let p = &mut self.posture;
            if crate::moby::countdown(&mut p.twitch_timer[i]) {
                let rng = &mut self.rng;
                match i {
                    0 => {
                        p.twitch[0][1] = rng.randf_signed(0.174_532_93, 0.523_598_8);
                        p.twitch_timer[0] = rng.rand_range(0x46, 0x96);
                    }
                    1 => {
                        p.twitch[1][1] = rng.randf_signed(0.174_532_93, 0.523_598_8);
                        p.twitch_timer[1] = rng.rand_range(0x28, 0x5a);
                    }
                    2 => {
                        p.twitch[2][1] = rng.randf_signed(0.261_799_4, 0.872_664_6);
                        p.twitch_timer[2] = rng.rand_range(0x28, 0x5a);
                    }
                    _ => {
                        p.twitch[3][0] = rng.randf(-0.087_266_47, 0.523_598_8);
                        let old = p.twitch[3][1];
                        let mut z = rng.randf_signed(0.349_065_9, 0.959_931_1);
                        // The tail swings to the other side when it was to one side and stays (both the same sign).
                        if (0.0 < old || 0.0 < z) && (0.0 <= old && 0.0 <= z) { z = -z; } else if !(0.0 < old || 0.0 < z) { z = -z; }
                        p.twitch[3][1] = z;
                        p.twitch_timer[3] = rng.rand_range(0x23, 0x46);
                    }
                }
            }
            let p = &mut self.posture;
            p.recs[rec::SECONDARY + i].target[1] = p.twitch[i][0];
            p.recs[rec::SECONDARY + i].target[2] = p.twitch[i][1];
        }
    }

    /// `0x2a5518`: the neck takes a quarter of the head's turn (at most 8.5°) and a third of its nod (−10°..17°; in a
    /// fidget 2° and 7°); then the blink (`0x2a5480`): with an interval, the next one comes 20 + two random parts of it
    /// after the last, its nodes linked over 11 ticks.
    fn neck_and_blink(&mut self, anim: &mut dyn AnimCtl) {
        let fidget = matches!(anim.view().seq_b, 1 | 2);
        let frame = self.frame as i32;
        let p = &mut self.posture;
        let (hy, hz) = (p.recs[rec::HEAD].target[1], p.recs[rec::HEAD].target[2]);
        let mut z = (hz * 0.25).clamp(-0.148_353, 0.148_353);
        let mut y = (hy * 0.357_142_87).min(0.296_706).max(-0.174_532_95);
        if fidget {
            z = z.clamp(-0.034_906_59, 0.034_906_59);
            y = y.clamp(-0.122_173_056, 0.122_173_056);
        }
        p.recs[rec::NECK].target[2] = z;
        p.recs[rec::NECK].target[1] = y;
        if p.blink == 0 {
            if p.blink_every == 0 {
                p.blink_next = frame + 0x1e;
            } else if p.blink_next < frame {
                p.blink = 1;
                let a = self.rng.randi(p.blink_every);
                let b = self.rng.randi(p.blink_every);
                p.blink_next = frame + a + 0x14 + b;
            }
        }
        let p = &mut self.posture;
        if p.blink != 0 {
            p.blink += 1;
            if 0xb < p.blink { p.blink = 0; }
            p.blink_linked = p.blink != 0;
        }
    }

    /// The posture's tick (module docs), after the transitions.
    pub(super) fn posture_tick(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        if self.mode != 0 { return; }
        self.feet_tilt();
        self.head_idle(anim);
        self.head_look(env, anim);
        self.twitch(anim);
        self.neck_and_blink(anim);
        for r in self.posture.recs.iter_mut() { r.update(); }
    }
}
