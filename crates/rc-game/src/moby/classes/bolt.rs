//! The bolts, classes 13..19 (L00 `0x381e60`, crate::moby::bolts for their making and look):
//!
//! * **State 0** (a placed bolt, once): the bolt look ([`dress`]), update and draw distance 0x20, its ground point on
//!   a line from 0.2 above to 3 below along its up row (flags 0x22), or where it is; a random spin way and angles;
//!   then state 3.
//! * **State 1** (thrown): gravity 0.003 a tick, the tumble (`+0x30`, a turn a tick, crate::quat) on its rows, a 0.25
//!   sphere against the world and the mobys (flags 0x22). Falling 2 below both where it started and Ratchet's feet,
//!   it flies to him with a lean of ±15°; a hit bounces it (`0x2ef2a8`) and sends it to him at 0.133 when Ratchet is
//!   alive and it is within the pickup radii (or has been) or the surface has an id (`0x2e23e0` ≠ −1); a floor within
//!   45° (the contact's way and the face) lands it: state 2 for 30..40 ticks, its tumble now its rows' quaternion.
//! * **State 2** (landing): each tick the resting pose ([`rest_pose`]), its rows eased from the tumble to the rest by
//!   1/n of the way (`0x2efad8`) and its place from 0.25 above the ground point to the rest place by n/of
//!   (`0x2eeeb8`), with a hop `(0.25 − f²)·0.025·of` (f = n/of − 0.5); then state 3 with a bob of 10..20 ticks.
//! * **State 3** (resting): within the pickup radii (`0x18c2c4` 2.125 across, `0x18c2c8` 1.25 up) it flies to him; a
//!   made bolt more than 32 across from him that is not drawn goes to the bank; drawn, the resting pose and the bob
//!   (`(0.25 − f²)·0.0125·of`).
//! * **State 4** (to Ratchet, `0x3831a8` [`attract`]): below his ground plane its velocity is pushed toward his up
//!   at 0.133 (at most 0.0139 a tick); else it brakes 0.0067 a tick and the speed eases to 0.533. His move past 0.0667
//!   a tick carries it; it tumbles and moves by its velocity, then by the speed toward his centre (`0x189f00`), the
//!   way leaned in his frame by its two angles (`+0x4c` about his up, `+0x48` sideways, mirrored when it is ahead of
//!   him) — the curve the bolts fly in on. Within the speed: collected (`0x382c18`: its value to the bolt count
//!   `0x1a7a00`, the counter shown 180 ticks), the pickup sound, gone.
//!
//! Drawn, every state but 0 adds its glitter ([`glitter`], `0x383758`).
//!
//! Not ported: the platforms a bolt rests on (`+0x5c`, `0x322570` / `0x3223e8`), the per-level tally (`0x18d208`),
//! targets other than Ratchet (`+0x60`: every caller passes him).

use crate::hero::physics::{add, add_rot, approach, atan2, dist2, len, scale, set_len, slope_of, sub, V3};
use crate::moby::attach::rows_of;
use crate::moby::bolts::{bolt_value, dress};
use crate::moby::{Ctx, Moby, MobyWorld};
use crate::quat;

/// `0x18c2c4` / `0x18c2c8` (Ratchet's pickup radii without the Bolt Grabber).
const PICK_XY: f32 = 2.125;
const PICK_Z: f32 = 1.25;

/// The pvars: the ground point, the ground's angles, the velocity, the tumble, the near flag, the angles of the way
/// in, the spin way, the landing ticks (of, left), the speed in, the starting height.
const GROUND: usize = 0;
const GROUND_ROT: usize = 0x10;
const VEL: usize = 0x20;
const TUMBLE: usize = 0x30;
const ROT: usize = 0x41;
const NEAR: usize = 0x42;
const SIDE: usize = 0x48;
const LEAN: usize = 0x4c;
const SPIN_WAY: usize = 0x55;
const OF: usize = 0x56;
const LEFT: usize = 0x57;
const SPEED: usize = 0x64;
const START_Z: usize = 0x68;

const DEG: f32 = 0.017_453_294;
/// 8 units a second.
const HOP: f32 = 0.133_333_34;

fn near(pos: V3, hero: V3) -> bool { dist2(pos, hero) < PICK_XY && (pos[2] - hero[2]).abs() < PICK_Z }

/// Ratchet's centre (`0x189f00`): a unit above his feet.
fn centre(ctx: &Ctx) -> V3 { add(ctx.hero.pos, [0.0, 0.0, 1.0]) }

/// `0x381e60`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let hero = ctx.hero.pos;
    let state = w.mobys[id].state;
    match state {
        0 => {
            let class_scale = w.class(w.mobys[id].o_class).map_or(0.0, |c| c.scale);
            let m = &mut w.mobys[id];
            m.state = 3;
            m.update_dist = 0x20;
            m.draw_dist = 0x20;
            if ctx.game.rng.randi(2) == 0 { m.set_pv_b(SPIN_WAY, m.pv_b(SPIN_WAY) | 1); }
            let (pos, up) = (m.pos, m.rows[2]);
            let (a, b) = (add(pos, set_len(up, 0.2)), add(pos, set_len(up, -3.0)));
            let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(0x22), Some(id)));
            let m = &mut w.mobys[id];
            match hit {
                Some(o) if m.pv_b(0x54) == 0 => set_ground(m, o.point, o.normal),
                _ => {
                    m.set_pv_v(GROUND, pos);
                    m.set_pv_v(GROUND_ROT, [0.0; 3]);
                }
            }
            let (s, t) = (ctx.game.rng.rand_angle(), ctx.game.rng.rand_angle());
            m.set_pv_f(0x50, s);
            m.set_pv_f(0x58, t);
            m.set_pv_f(START_Z, m.pos[2]);
            dress(m, class_scale);
        }
        1 => {
            mark_near(&mut w.mobys[id], hero);
            let m = &mut w.mobys[id];
            let mut vel = m.pv_v(VEL);
            vel[2] -= 0.003;
            m.set_pv_v(VEL, vel);
            m.pos = add(m.pos, vel);
            m.rows = quat::turn_rows(&m.rows, m.pv_q(TUMBLE));
            if !(m.pv_f(START_Z) - 2.0 <= m.pos[2] || hero[2] - 2.0 <= m.pos[2]) {
                // Fell past where it started and below Ratchet: off to him.
                let lean = ctx.game.rng.randf(-15.0, 15.0) * DEG;
                attract(w, id, lean, 0.0, ctx);
                glitter(w, id, false, ctx);
                return;
            }
            let pos = m.pos;
            if let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_sphere_m(c, ctx.scene, pos, 0.25, crate::collision::QueryFlags(0x22), Some(id))) {
                let m = &mut w.mobys[id];
                set_ground(m, o.point, o.normal);
                if let Some(pc) = o.pushed_centre { m.pos = pc; }
                // Bounce: the velocity reflected off the contact (`0x2ef2a8`).
                let n = sub(m.pos, o.point);
                let l = len(n);
                if 0.0 < l {
                    let u = scale(n, 1.0 / l);
                    let d = vel[0] * u[0] + vel[1] * u[1] + vel[2] * u[2];
                    if d < 0.0 { vel = sub(vel, scale(u, 2.0 * d)); }
                }
                m.set_pv_v(VEL, vel);
                if ctx.hero.hp != 0 && (m.pv_b(NEAR) != 0 || near(m.pos, hero) || o.surface_id() != -1) {
                    m.set_pv_v(VEL, set_len(vel, HOP));
                    attract(w, id, 0.0, 0.0, ctx);
                } else if slope_of(n) < std::f32::consts::FRAC_PI_4 && slope_of(o.normal) < std::f32::consts::FRAC_PI_4 {
                    // A floor: it lands.
                    let t = ctx.game.rng.randf(30.0, 40.0) as u8;
                    let m = &mut w.mobys[id];
                    m.state = 2;
                    m.set_pv_b(OF, t);
                    m.set_pv_b(LEFT, t);
                    let q = quat::from_rows(&m.rows);
                    m.set_pv_q(TUMBLE, q);
                }
            }
            glitter(w, id, false, ctx);
        }
        2 => {
            mark_near(&mut w.mobys[id], hero);
            let m = &mut w.mobys[id];
            let ground = rest_pose(m);
            let target = add(ground, [0.0, 0.0, 0.25]);
            let rest = quat::from_rows(&m.rows);
            let (n, of) = (m.pv_b(LEFT) as f32, m.pv_b(OF) as f32);
            let q = quat::nlerp(m.pv_q(TUMBLE), rest, 1.0 / n);
            m.set_pv_q(TUMBLE, q);
            let t = n / of;
            m.pos = add(m.pos, scale(sub(target, m.pos), t));
            let f = t - 0.5;
            m.pos[2] += (0.25 - f * f) * 0.025 * of;
            m.rows = quat::rows(q);
            let left = m.pv_b(LEFT).wrapping_sub(1);
            m.set_pv_b(LEFT, left);
            if left == 0 {
                let t = ctx.game.rng.randf(10.0, 20.0) as u8;
                let m = &mut w.mobys[id];
                m.state = 3;
                m.set_pv_b(OF, t);
                m.set_pv_b(LEFT, t);
            }
            glitter(w, id, false, ctx);
        }
        3 => {
            let m = &w.mobys[id];
            let across = dist2(m.pos, hero);
            if m.pv_b(NEAR) != 0 || near(m.pos, hero) {
                attract(w, id, 0.0, 0.0, ctx);
                glitter(w, id, false, ctx);
                return;
            }
            if m.instance.is_none() && 32.0 < across && m.drawn == 0 {
                ctx.game.bolt_bank += bolt_value(m.o_class);
                w.delete(id);
                return;
            }
            let m = &mut w.mobys[id];
            if m.drawn != 0 {
                rest_pose(m);
                let (n, of) = (m.pv_b(LEFT), m.pv_b(OF));
                if n != 0 {
                    let f = n as f32 / of as f32 - 0.5;
                    m.pos[2] += (0.25 - f * f) * 0.0125 * of as f32;
                }
            }
            let n = m.pv_b(LEFT);
            if n != 0 { m.set_pv_b(LEFT, n - 1); }
            glitter(w, id, false, ctx);
        }
        4 => fly_in(w, id, ctx),
        _ => {}
    }
}

/// Within the pickup radii once: it flies to him when it can.
fn mark_near(m: &mut Moby, hero: V3) {
    if m.pv_b(NEAR) == 0 && near(m.pos, hero) { m.set_pv_b(NEAR, 1); }
}

/// State 4 (module docs).
fn fly_in(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let h = &*ctx.hero;
    let (rows, ground, carry) = (h.rows, h.ground_point, h.carry);
    let to = centre(ctx);
    let m = &mut w.mobys[id];
    let mut vel = m.pv_v(VEL);
    // `(pos − 0x18a0f0)` in his frame (`0x189e60`): below his ground plane.
    let d = sub(m.pos, ground);
    if d[0] * rows[2][0] + d[1] * rows[2][1] + d[2] * rows[2][2] < 0.0 {
        let mut push = sub(set_len(rows[2], HOP), vel);
        if 0.013_888_889 < len(push) { push = set_len(push, 0.013_888_889); }
        vel = add(vel, push);
    } else {
        if 0.006_666_667 < len(vel) { vel = sub(vel, set_len(vel, 0.006_666_667)); }
        let mut s = m.pv_f(SPEED);
        approach(0.533_333_3, 0.006_666_667, &mut s);
        m.set_pv_f(SPEED, s);
    }
    m.set_pv_v(VEL, vel);
    let c = len(carry);
    if 0.066_666_67 < c { m.pos = add(m.pos, set_len(carry, c - 0.066_666_67)); }
    m.rows = quat::turn_rows(&m.rows, m.pv_q(TUMBLE));
    m.pos = add(m.pos, vel);
    let mut d = sub(to, m.pos);
    let speed = m.pv_f(SPEED);
    if len(d) <= speed {
        // Its pickup sound (class sound 0, flags 0x20), from where it was.
        ctx.host.play_sound(&w.mobys[id], 0, 0x20);
        collect(w, id, ctx);
        return;
    }
    let (side, lean) = (m.pv_f(SIDE), m.pv_f(LEAN));
    if lean != 0.0 || side != 0.0 {
        // The way into his frame (`0x189e60`), leaned (`0x2ef6b8(0, ±side, lean)`), back out (`0x189e20`).
        let l = rows.map(|r| d[0] * r[0] + d[1] * r[1] + d[2] * r[2]);
        let yaw = atan2(l[0], l[1]);
        let side = if -std::f32::consts::FRAC_PI_2 < yaw && yaw < std::f32::consts::FRAC_PI_2 { -side } else { side };
        let turn = rows_of([0.0, side, lean]);
        let l2: V3 = std::array::from_fn(|k| l[0] * turn[0][k] + l[1] * turn[1][k] + l[2] * turn[2][k]);
        d = std::array::from_fn(|k| l2[0] * rows[0][k] + l2[1] * rows[1][k] + l2[2] * rows[2][k]);
    }
    m.pos = add(m.pos, set_len(d, speed));
    glitter(w, id, true, ctx);
}

/// The ground point (pvars 0..0xc) and the ground's frame angles (0x10..0x1c: 0, slope, yaw of the normal).
fn set_ground(m: &mut Moby, point: V3, normal: V3) {
    m.set_pv_v(GROUND, point);
    m.set_pv_v(GROUND_ROT, [0.0, slope_of(normal), atan2(normal[0], normal[1])]);
}

/// `0x382e20(moby, &ground, 0)` without a platform: the spin (`+0x50` by the class's step, the way `+0x55` bit 0
/// picks, `+0x58` by 0.0125) at the class's tilt, turned into the ground's frame for the rows (mode 0x100 keeps them);
/// the position the ground point + the class's lift along the ground's up + its offset along the first row. Returns
/// the ground point.
fn rest_pose(m: &mut Moby) -> V3 {
    let (step, tilt, lift, out) = match m.o_class {
        0xd => (0.04, -0.25, 0.048_75, 0.325),
        0xe => (0.03, -0.5, 0.0245, 0.35),
        0xf => (0.025, -0.4, 0.056_25, 0.375),
        _ => (0.0175, -0.35, 0.135, 0.3),
    };
    let k = if m.pv_b(SPIN_WAY) & 1 == 0 { -1.0 } else { 1.0 };
    let (a, b) = (add_rot(m.pv_f(0x50), k * step), add_rot(m.pv_f(0x58), k * -0.0125));
    m.set_pv_f(0x50, a);
    m.set_pv_f(0x58, b);
    let ground = rows_of(m.pv_v(GROUND_ROT));
    let spin = rows_of([a, tilt, b]);
    m.rows = spin.map(|r| std::array::from_fn(|j| r[0] * ground[0][j] + r[1] * ground[1][j] + r[2] * ground[2][j]));
    let g = m.pv_v(GROUND);
    m.pos = add(add(scale(ground[2], lift), scale(m.rows[0], out)), g);
    g
}

/// `0x3831a8(lean, side, bolt, Ratchet)`: off to him (state 4) unless it already is; no lean: ±30°, no side: 0..30°.
/// From rest it hops off its ground at 0.133 a tick; its speed in starts at 0; it tumbles 12° a tick about
/// `(him − it) × velocity`.
pub fn attract(w: &mut MobyWorld, id: usize, lean: f32, side: f32, ctx: &mut Ctx) {
    if w.mobys[id].state == 4 { return; }
    let rng = &mut ctx.game.rng;
    let lean = if lean == 0.0 { rng.randf(-30.0, 30.0) * DEG } else { lean };
    let side = if side == 0.0 { rng.randf(0.0, 30.0) * DEG } else { side };
    let to = ctx.hero.pos;
    let m = &mut w.mobys[id];
    if m.state == 3 { m.set_pv_v(VEL, set_len(rows_of(m.pv_v(GROUND_ROT))[2], HOP)); }
    m.set_pv_f(SPEED, 0.0);
    m.set_pv_f(LEAN, lean);
    m.set_pv_f(SIDE, side);
    let axis = set_len(crate::hero::physics::cross(sub(to, m.pos), m.pv_v(VEL)), 1.0);
    m.set_pv_q(TUMBLE, quat::axis_angle(0.209_439_51, axis));
    m.state = 4;
}

/// `0x383758(bolt, resting)` when drawn: one tick in 150 a twinkle (2.0 swelling from 0.5, 30 ticks, turning ±4); a
/// sparkle on it every tick (size 1.1, two ticks, frame `*def[0x3e]`); flying in, on odd frames a glint 0.05 around it
/// sinking up to 0.8 a second, 40..90 ticks, its colour between `0x60004040` and `0x60202040`.
fn glitter(w: &mut MobyWorld, id: usize, flying: bool, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if m.drawn == 0 { return; }
    let (pos, rot) = (m.pos, m.pv_b(ROT));
    let hero = ctx.hero.pos;
    let Some(parts) = ctx.parts.as_deref_mut() else { return };
    let rng = &mut ctx.game.rng;
    if rng.randi(0x96) == 0 {
        let spin = if rng.randi(1) != 0 { -4 } else { 4 };
        crate::particles::twinkle(parts, rng, 2.0, 0.5, 0.0, pos, 0x1e, 0x6000_4040, 2, spin, [0.0; 3], 0);
    }
    if let Some(i) = crate::particles::sparkle(parts, 1.1, pos, 0x6010_2830, rot, 0, false, 2, false, hero) {
        parts.pool.recs[i][2] = parts.def_first(0x3e);
    }
    if flying && w.frame & 1 != 0 {
        let p = add(pos, rng.rand_vec(0.0, 0.05));
        let sink = -(rng.randf(0.0, 0.8) * 0.016_666_668);
        let life = rng.rand_range(0x28, 0x5a) as u16 & 0xff;
        let t = rng.randf(0.0, 1.0);
        let rgba = crate::particles::tween_color(t.to_bits(), 0x6000_4040, 0x6020_2040);
        let rot = rng.randi(0xff) as u8;
        crate::particles::glint(parts, 0.125, p, [0.0, 0.0, sink], rgba, life, rot, false, hero);
    }
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
