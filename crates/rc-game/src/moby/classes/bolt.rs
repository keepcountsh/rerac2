//! The bolts, classes 13..19 (L00 `0x381e60`): a bolt flies (state 1: gravity 0.003 a tick, a 0.25 sphere against
//! the world and the mobys, flags 0x22), comes to rest (states 2 / 3), and once Ratchet is within the pickup radii
//! (`0x18c2c4` 2.125 across, `0x18c2c8` 1.25 up) flies to him (state 4, `0x3831a8`: speed toward 0.533 by 0.0067 a
//! tick) and is collected (`0x382c18`: its value to the bolt count `0x1a7a00`, the counter shown 180 ticks). A resting
//! bolt more than 32 from Ratchet that is not drawn goes to the bank.
//!
//! A resting bolt that is drawn sits on its ground point (pvars 0..0xc: a placed bolt's line down at state 0, a
//! thrown one's contact) in the ground's frame (angles 0x10..0x1c), spinning (`0x382e20`, [`rest_pose`]): that is
//! what puts the placed bolts on the floor, 0.1..0.2 above it, instead of at their placed heights.
//!
//! Not ported: the settle arc of state 2 (a landed bolt goes straight to 3 and bobs there), the platforms a bolt
//! rests on (`+0x5c`), the sparkle (`0x33a6a0`), the per-level tally (`0x18d208`).

use crate::hero::physics::{add, add_rot, atan2, dist, dist2, len, scale, set_len, slope_of, sub, V3};
use crate::moby::attach::rows_of;
use crate::moby::bolts::bolt_value;
use crate::moby::{mode, Ctx, Moby, MobyWorld};

/// `0x18c2c4` / `0x18c2c8` (Ratchet's pickup radii without the Bolt Grabber).
const PICK_XY: f32 = 2.125;
const PICK_Z: f32 = 1.25;

fn near(pos: V3, hero: V3) -> bool { dist2(pos, hero) < PICK_XY && (pos[2] - hero[2]).abs() < PICK_Z }

/// `0x381e60`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let hero = ctx.hero.pos;
    let state = w.mobys[id].state;
    match state {
        0 => {
            // A placed bolt: its ground point on a line from 0.2 above to 3 below along its up row (flags 0x22), or
            // where it is; a random spin direction and angles.
            let m = &mut w.mobys[id];
            m.state = 3;
            m.mode |= mode::KEEP_ROWS;
            m.update_dist = 0x20;
            m.draw_dist = 0x20;
            if ctx.game.rng.randi(2) == 0 { m.set_pv_b(0x55, m.pv_b(0x55) | 1); }
            let (pos, up) = (m.pos, m.rows[2]);
            let (a, b) = (add(pos, set_len(up, 0.2)), add(pos, set_len(up, -3.0)));
            let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(0x22), Some(id)));
            let m = &mut w.mobys[id];
            match hit {
                Some(o) if m.pv_b(0x54) == 0 => set_ground(m, o.point, o.normal),
                _ => {
                    m.set_pv_v(0, pos);
                    m.set_pv_v(0x10, [0.0; 3]);
                }
            }
            let (s, t) = (ctx.game.rng.rand_angle(), ctx.game.rng.rand_angle());
            m.set_pv_f(0x50, s);
            m.set_pv_f(0x58, t);
            m.set_pv_f(0x68, m.pos[2]);
        }
        1 => {
            let m = &mut w.mobys[id];
            if m.pv_b(0x42) == 0 && near(m.pos, hero) { m.set_pv_b(0x42, 1); }
            let mut vel = m.pv_v(0x20);
            vel[2] -= 0.003;
            m.pos = add(m.pos, vel);
            m.set_pv_v(0x20, vel);
            let low = m.pv_f(0x68) - 2.0 <= m.pos[2] || hero[2] - 2.0 <= m.pos[2];
            if !low {
                // Fell past where it started and below Ratchet: off to him.
                attract(w, id, ctx);
                return;
            }
            let pos = m.pos;
            let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_sphere_m(c, ctx.scene, pos, 0.25, crate::collision::QueryFlags(0x22), Some(id))) else { return };
            let m = &mut w.mobys[id];
            set_ground(m, o.point, o.normal);
            if let Some(pc) = o.pushed_centre { m.pos = pc; }
            // Bounce: the velocity reflected off the contact (`0x2ef2a8`).
            let n = sub(m.pos, o.point);
            let l = len(n);
            if 0.0 < l {
                let n = scale(n, 1.0 / l);
                let d = vel[0] * n[0] + vel[1] * n[1] + vel[2] * n[2];
                vel = sub(vel, scale(n, 2.0 * d));
            }
            m.set_pv_v(0x20, vel);
            if ctx.hero.hp != 0 && (m.pv_b(0x42) != 0 || near(m.pos, hero)) {
                m.set_pv_v(0x20, scale(vel, 0.133_333_34));
                attract(w, id, ctx);
                return;
            }
            // A floor within 45°: it rests (the settle's 30..40 ticks are not ported).
            if 0.0 < l && (n[2] / l) > 0.707_106_77 {
                let t = ctx.game.rng.randf(30.0, 40.0) as u8;
                let m = &mut w.mobys[id];
                m.state = 3;
                m.set_pv_b(0x56, t);
                m.set_pv_b(0x57, t);
                m.set_pv_v(0x20, [0.0; 3]);
            }
        }
        2 | 3 => {
            let m = &w.mobys[id];
            if m.pv_b(0x42) != 0 || near(m.pos, hero) {
                attract(w, id, ctx);
                return;
            }
            if 32.0 < dist(m.pos, hero) && m.drawn == 0 && m.instance.is_none() {
                ctx.game.bolt_bank += bolt_value(m.o_class);
                w.delete(id);
                return;
            }
            // Drawn: on its ground point; the bob after landing (`+0x57` ticks of `+0x56`).
            let m = &mut w.mobys[id];
            if m.drawn != 0 { rest_pose(m); }
            let (n, of) = (m.pv_b(0x57), m.pv_b(0x56));
            if n == 0 { return; }
            if m.drawn != 0 && of != 0 {
                let f = n as f32 / of as f32 - 0.5;
                m.pos[2] += (0.25 - f * f) * 0.0125 * of as f32;
            }
            m.set_pv_b(0x57, n - 1);
        }
        4 => {
            // Toward Ratchet's head (`0x189f00`): the speed eases to 0.533.
            let target = add(hero, [0.0, 0.0, 1.0]);
            let m = &mut w.mobys[id];
            let mut s = m.pv_f(0x64);
            crate::hero::physics::approach(0.533_333_3, 0.006_666_667, &mut s);
            m.set_pv_f(0x64, s);
            let d = sub(target, m.pos);
            let l = len(d);
            if l <= s.max(0.0) {
                // Its pickup sound (class sound 0, flags 0x20), from where it was.
                ctx.host.play_sound(&w.mobys[id], 0, 0x20);
                collect(w, id, ctx);
                return;
            }
            if 0.0 < l { m.pos = add(m.pos, scale(d, s / l)); }
            // Ratchet's own move carries it.
            let c = ctx.hero.carry;
            m.pos = add(m.pos, c);
        }
        _ => {}
    }
}

/// The ground point (pvars 0..0xc) and the ground's frame angles (0x10..0x1c: 0, slope, yaw of the normal).
fn set_ground(m: &mut Moby, point: V3, normal: V3) {
    m.set_pv_v(0, point);
    m.set_pv_v(0x10, [0.0, slope_of(normal), atan2(normal[0], normal[1])]);
}

/// `0x382e20` without a platform: the spin (`+0x50` by the class's step, the way `+0x55` bit 0 picks, `+0x58` by
/// 0.0125) at the class's tilt, turned into the ground's frame for the rows (mode 0x100 keeps them); the position
/// the ground point + the class's lift along the ground's up + its offset along the first row.
fn rest_pose(m: &mut Moby) {
    let (step, tilt, lift, out) = match m.o_class {
        0xd => (0.04, -0.25, 0.048_75, 0.325),
        0xe => (0.03, -0.5, 0.0245, 0.35),
        0xf => (0.025, -0.4, 0.056_25, 0.375),
        _ => (0.0175, -0.35, 0.135, 0.3),
    };
    let k = if m.pv_b(0x55) & 1 == 0 { -1.0 } else { 1.0 };
    let (a, b) = (add_rot(m.pv_f(0x50), k * step), add_rot(m.pv_f(0x58), k * -0.0125));
    m.set_pv_f(0x50, a);
    m.set_pv_f(0x58, b);
    let ground = rows_of(m.pv_v(0x10));
    let spin = rows_of([a, tilt, b]);
    m.rows = spin.map(|r| std::array::from_fn(|j| r[0] * ground[0][j] + r[1] * ground[1][j] + r[2] * ground[2][j]));
    m.pos = add(add(scale(ground[2], lift), scale(m.rows[0], out)), m.pv_v(0));
}

/// `0x3831a8`: state 4, its speed from 0.
fn attract(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 4 { return; }
    m.set_pv_f(0x64, 0.0);
    m.state = 4;
}

/// `0x382c18`: the value to the bolt count, the counter shown, the bolt gone.
fn collect(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let v = bolt_value(w.mobys[id].o_class);
    // A placed bolt stays gone (`0x21d554` / `0x21e1c4`).
    if w.mobys[id].instance.is_some() {
        let (uid, mission) = (w.mobys[id].uid, w.mobys[id].mission);
        ctx.game.uid_gone(uid, mission);
    }
    ctx.game.bolts = ctx.game.bolts.saturating_add(v as u32);
    ctx.game.show_bolts = Some(0xb4);
    w.delete(id);
}
