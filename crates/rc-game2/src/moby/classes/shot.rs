//! The Lancer's shot, class 0xbfe (spawned by `0x3e3600`, update L00 `0x3e3990`): a small spinning blob from the
//! muzzle at the row's speed (60 / s), toward the lock's aim point or along Ratchet's facing (`0x18a580`), for range /
//! speed ticks. Each tick in flight (state 0) it turns to the lock while the lock lives, steps, spins (its rows from
//! random angles, each scaled by 0.35 + 0.15·sin of its own phase, `0x3e4d10`) and tests the step's line (`0x2de878`,
//! Ratchet ignored) with the row's damage: a hit — a moby takes the hit — plays its class sound 0; a hit or the end of
//! its time hides it (state 1) and 6 ticks later it is deleted. Out of the world's 2..1021 box it is deleted at once.
//!
//! **Effects** (crate::particles): each tick in flight two orange glows on it (420000, 2 ticks, `def[0x35]`) and five
//! shrinking white sprites along the step; every tenth frame a slow orange halo (`def[0x2d]`). A wall or floor hit
//! ([`impact`] `0x3e46d8`): twenty grey smoke puffs rising and twenty white sparks thrown toward the camera; a moby hit
//! adds two big orange glows. The end of its time ([`fizzle`] `0x3e4ae8`): twenty small sparks. Its **trail** (the draw
//! callback `0x3e4050`, [`trail`]): the last 40 positions (newest first), drawn as two additive ribbons by `0x3e4300`
//! — FX 0x17 white, 0.05 wide, and FX 0xe orange (`0x204080` at alpha 0x40), four times as wide — fading along their
//! length, over the first 20 points and again over all of them; hidden (state 1) it fades out over its last 6 ticks.
//!
//! **pvars**: +0x04..+0x0c the scale phases, +0x10 the trail's points, +0x1c the lock, +0x20 the velocity (per tick),
//! +0x2c the speed, +0x30 the range, +0x38 the ticks left, +0x3c 1 / the ticks, +0x40 1.0, +0x44..+0x4c the spin (per
//! tick); from [`TRAIL`] the trail's points (the class's slot of the gadget trail buffer `0x32b3b0`, 40 of 0x10 bytes).
//!
//! Not ported: a hit moby's reaction to the shot (`0x4126b8`) and its `+0x98` test, the gravity boots' bend
//! (`0x37f538` / `0x37f618`), the first-person aim (`0x380ef8`).

use crate::hero::items::{row, LANCER};
use crate::hero::physics::{add, len, scale, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{aim, countdown, mode, Ctx, MobyWorld};

/// The shot's class.
pub const SHOT_CLASS: i16 = 0xbfe;
/// Where the trail's points sit in the pvars (after the record's 0x80 bytes) and how many it keeps.
pub const TRAIL: usize = 0x80;
pub const TRAIL_POINTS: usize = 40;
/// `0x1fe8c0[0x35]` / `[0x2d]`: the glows' frames.
const GLOW_DEF: u8 = 0x35;
const HALO_DEF: u8 = 0x2d;

/// `0x325100(len, yaw, pitch)`.
pub fn polar(len: f32, yaw: f32, pitch: f32) -> V3 { [yaw.cos() * len * pitch.cos(), yaw.sin() * pitch.cos() * len, pitch.sin() * len] }

/// The jitter `0x31ba98(0, j)` on each axis.
fn jitter(g: &mut crate::rng::Rng, p: V3, j: f32) -> V3 { std::array::from_fn(|k| p[k] + g.randf_signed(0.0, j)) }

fn normalize(v: V3) -> V3 {
    let l = len(v);
    if 0.0 < l { scale(v, 1.0 / l) } else { v }
}

/// `0x3e3600(range, at, owner, target)`: a shot from `at`.
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, range: f32, at: V3, owner: usize, target: Option<usize>) {
    if w.class(SHOT_CLASS).is_none() { return; }
    let Some(r) = row(LANCER) else { return };
    let speed = r.speed / 60.0;
    let vel = match target {
        Some(t) if w.mobys.get(t).is_some() => scale(normalize(sub(aim::aim_point(w, t), at)), speed),
        _ => scale(normalize(ctx.hero.rows[0]), speed),
    };
    let id = w.create(SHOT_CLASS);
    let class_scale = w.class(SHOT_CLASS).map_or(1.0, |c| c.scale);
    let ticks = (range / speed).round() as i32;
    {
        let m = &mut w.mobys[id];
        m.update_dist = 0xff;
        m.mode = mode::KEEP_ROWS;
        m.drawn = 1;
        m.draw_dist = 0xff;
        m.parent = Some(owner);
        m.state = 0;
        m.set_pv_f(0x30, range);
        m.set_pv_v(0x20, vel);
        m.pos = at;
        m.set_pv_f(0x2c, speed);
    }
    // The rows first (from no angles), then the random spin, angles and phases.
    spin(w, id);
    let g = &mut ctx.game.rng;
    let s = [g.randf(0.0, 0.069_813_17), g.randf(0.0, 0.139_626_34), g.randf(0.0, 0.279_252_68)];
    let rot = [g.rand_angle(), g.rand_angle(), g.rand_angle()];
    let phases = [g.rand_angle(), g.rand_angle(), g.rand_angle()];
    let m = &mut w.mobys[id];
    m.set_pv_i(0x38, ticks);
    m.set_pv_f(0x3c, 1.0 / ticks.max(1) as f32);
    m.scale = class_scale * 6.0;
    m.set_pv_i(0x1c, target.map_or(0, |t| t as i32));
    m.set_pv_f(0x40, 1.0);
    m.set_pv_v(0x44, s);
    m.rot = rot;
    m.set_pv_v(4, phases);
    w.rebuild(id, &*ctx.host);
}

/// `0x3e4d10`: the rows from the Euler angles, each scaled by 0.35 + 0.15·sin of its phase; the phases step 0.5.
fn spin(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    let mut rows = crate::hero::physics::euler_rows(m.rot);
    let ph = m.pv_v(4);
    for k in 0..3 { rows[k] = scale(rows[k], ph[k].sin() * 0.15 + 0.35); }
    m.rows = rows;
    m.set_pv_v(4, ph.map(|p| crate::hero::physics::add_rot(p, 0.5)));
}

/// `0x3e3990`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let from = w.mobys[id].pos;
    // Homing on the lock while it lives.
    if let Some(t) = w.mobys[id].pv_link(0x1c) {
        let gone = w.mobys.get(t).is_none_or(|x| x.state >= 0xfd) || aim::health(w, t).is_some_and(|(hp, _)| hp <= 0.0 && !aim::is_crate(w, t));
        if gone {
            w.mobys[id].set_pv_i(0x1c, 0);
        } else {
            let d = sub(aim::aim_point(w, t), from);
            let l = len(d);
            if l == 0.0 {
                w.mobys[id].set_pv_i(0x1c, 0);
            } else {
                let s = w.mobys[id].pv_f(0x2c);
                w.mobys[id].set_pv_v(0x20, scale(d, s / l));
            }
        }
    }
    match w.mobys[id].state {
        0 => {
            let vel = w.mobys[id].pv_v(0x20);
            let to = add(from, vel);
            {
                let m = &mut w.mobys[id];
                m.pos = to;
                let s = m.pv_v(0x44);
                m.rot = [crate::hero::physics::add_rot(m.rot[0], s[0]), crate::hero::physics::add_rot(m.rot[1], s[1]), crate::hero::physics::add_rot(m.rot[2], s[2])];
            }
            spin(w, id);
            let damage = row(LANCER).map_or(0.0, |r| r.damage);
            let d = normalize(vel);
            let mut t = HitTemplate::new(damage, Some(id), 0x10000, [d[0], d[1], d[2], crate::moby::classes::lancer::NO_KNOCK_LIMIT]);
            t.kind = (w.mobys[id].o_class as u16 as u32) << 16 | 0x101;
            let hit = ctx.coll.and_then(|c| rc_game::collision_query::coll_line_m(c, ctx.scene, from, to, rc_game::collision_query::QueryFlags(0), ctx.hero_moby));
            let cam = ctx.host.camera_pos();
            match hit {
                Some(o) => {
                    if let Some(m) = o.moby { w.deliver_hit_any(m, &t, [o.point[0], o.point[1], o.point[2], 1.0]); }
                    if let Some(sys) = ctx.parts.as_deref_mut() { impact(sys, &mut ctx.game.rng, o.point, cam, o.moby.is_some()); }
                    ctx.host.play_sound(&w.mobys[id], 0, 0);
                    hide(w, id);
                }
                None => {
                    let mut n = w.mobys[id].pv_i(0x38);
                    let done = countdown(&mut n);
                    w.mobys[id].set_pv_i(0x38, n);
                    if done {
                        if let Some(sys) = ctx.parts.as_deref_mut() { fizzle(sys, &mut ctx.game.rng, to, cam); }
                        hide(w, id);
                    }
                }
            }
            if let Some(sys) = ctx.parts.as_deref_mut() { flight_fx(sys, &mut ctx.game.rng, from, to, ctx.game.frame); }
            trail_step(w, id);
            w.rebuild(id, &*ctx.host);
        }
        1 => {
            let mut n = w.mobys[id].pv_i(0x38);
            let done = countdown(&mut n);
            w.mobys[id].set_pv_i(0x38, n);
            if done {
                w.delete(id);
                return;
            }
        }
        _ => {}
    }
    let p = w.mobys[id].pos;
    if p.iter().any(|&c| !(2.0..=1021.0).contains(&c)) { w.delete(id); }
}

/// State 1: hidden, the lock dropped, 6 ticks left.
fn hide(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    m.state = 1;
    m.set_pv_i(0x1c, 0);
    m.mode |= 0x41;
    m.set_pv_i(0x38, 6);
}

/// The sprites of a tick in flight from `from` to `to`: two orange glows on the shot (the second turning the other
/// half), five shrinking white sprites jittered along the step, every tenth frame a slow halo.
fn flight_fx(sys: &mut rc_game::particles::Particles, g: &mut crate::rng::Rng, from: V3, to: V3, frame: i32) {
    use crate::particles::sprite;
    let spin = g.randi(0xff);
    let glow = sys.def_first(GLOW_DEF);
    if let Some(i) = sprite(sys, g, 420_000.0, 420_000.0, 0.0, to, [0.0; 3], 2, 0x20_2080, spin as u8, 0x40, 0x10, 1) { sys.pool.recs[i][2] = glow; }
    if let Some(i) = sprite(sys, g, 420_000.0, 420_000.0, 0.0, to, [0.0; 3], 2, 0x20_2080, ((spin + 0x80) % 0xff) as u8, 0x40, 0x10, 1) { sys.pool.recs[i][2] = glow; }
    let step = scale(sub(to, from), 0.2);
    let mut q = from;
    for _ in 0..5 {
        let spin = g.randi(0xff);
        let f = g.randf(0.1, 0.3);
        let p = jitter(g, q, 0.1);
        sprite(sys, g, f * 210_000.0, f * 42_000.0, 0.0, p, [0.0; 3], 0xf, 0x80_8080, spin as u8, 0x20, 0, 1);
        q = add(q, step);
    }
    if frame % 10 == 0 {
        let halo = sys.def_first(HALO_DEF);
        if let Some(i) = sprite(sys, g, 2100.0, 420_000.0, 0.0, to, [0.0; 3], 0x1e, 0x20_2080, 0, 0x20, 0, 1) { sys.pool.recs[i][2] = halo; }
    }
}

/// `0x3e46d8(at, _, big)`: twenty times a grey smoke puff (the size 0.3..0.75 of 210000, jittered 0.2, rising
/// 0.2..3 a second, fading in over 5 ticks to 0x30 and lasting 90, normal blend) and a white spark (0.1..0.5 of
/// 210000, 1..2 a second toward the camera within 80° of yaw and up to 80° up, 30 ticks, growing 500 a tick); `big`
/// (a moby hit): two big orange glows (525000, 5 ticks, `def[0x35]`).
pub fn impact(sys: &mut rc_game::particles::Particles, g: &mut crate::rng::Rng, at: V3, cam: V3, big: bool) {
    use crate::particles::{puff, spark, GRAVITY};
    use rc_game::particles::rec;
    let to_cam = crate::hero::physics::atan2(cam[0] - at[0], cam[1] - at[1]);
    for _ in 0..20 {
        let rise = g.randf(0.2, 3.0);
        let vel = scale(GRAVITY, -(rise * (1.0 / 60.0)));
        let spin = g.randf_signed(1.0, 2.0) as i32;
        let size = g.randf(0.3, 0.75) * 210_000.0;
        let p = jitter(g, at, 0.2);
        if let Some(i) = puff(sys, g, 0.0, 1.0, 1.01, size, p, spin, vel, 0x7f20_2020, 1) {
            let r = &mut sys.pool.recs[i];
            rec::set_i16(r, 0xa, 0x5a);
            r[3] = 0x44;
            r[9] = 0x40 + 4;
            rec::set_ff(r, 0x3c, 5.0);
            rec::set_u32(r, 0x24, 3);
            r[0x2a] = 0x30;
            r[0x2b] = r[0xa];
        }
        let speed = g.randf(1.0, 2.0);
        let pitch = g.randf(0.0, 1.396_263_4);
        let yaw = crate::hero::physics::add_rot(to_cam, g.randf_signed(0.0, 1.396_263_4));
        let vel = polar(speed * (1.0 / 60.0), yaw, pitch);
        let p = jitter(g, at, 0.1);
        let size = g.randf(0.1, 0.5) * 210_000.0;
        spark(sys, g, size, 500.0, 1.0, 0.0, 1.0, p, vel, 0x1e, 0x7f, 0x7f_7f7f, spin);
    }
    if big {
        let glow = sys.def_first(GLOW_DEF);
        let spin = g.randi(0xff);
        if let Some(i) = spark(sys, g, 525_000.0, 0.0, 1.0, 0.0, 1.0, at, [0.0; 3], 5, 0x7f, 0x20_7080, spin) { sys.pool.recs[i][2] = glow; }
        if let Some(i) = spark(sys, g, 525_000.0, 0.0, 1.0, 0.0, 1.0, at, [0.0; 3], 5, 0x7f, 0x20_7080, (spin + 0x80) & 0xff) { sys.pool.recs[i][2] = glow; }
    }
}

/// `0x3e4ae8(at)`: twenty small white sparks (0.05..0.2 of 210000, jittered 0.075, 0.25..0.5 a second toward the
/// camera within 80° of yaw and up to 80° up, 15 ticks).
pub fn fizzle(sys: &mut rc_game::particles::Particles, g: &mut crate::rng::Rng, at: V3, cam: V3) {
    use crate::particles::spark;
    let to_cam = crate::hero::physics::atan2(cam[0] - at[0], cam[1] - at[1]);
    for _ in 0..20 {
        let speed = g.randf(0.25, 0.5);
        let spin = g.randf_signed(1.0, 2.0) as i32;
        // A draw the game makes and drops.
        let _ = g.randf(0.05, 0.2);
        let pitch = g.randf(0.0, 1.396_263_4);
        let yaw = crate::hero::physics::add_rot(to_cam, g.randf_signed(0.0, 1.396_263_4));
        let vel = polar(speed * (1.0 / 60.0), yaw, pitch);
        let p = jitter(g, at, 0.075);
        let size = g.randf(0.05, 0.2) * 210_000.0;
        spark(sys, g, size, 500.0, 1.0, 0.0, 1.0, p, vel, 0xf, 0x7f, 0x7f_7f7f, spin);
    }
}

/// `0x3e4050`'s step (in flight): the trail keeps one point more (at most 40), the older ones shift back, the
/// newest is the shot (width 0.05).
fn trail_step(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    let n = (m.pv_i(0x10) + 1).min(TRAIL_POINTS as i32);
    m.set_pv_i(0x10, n);
    for k in (1..n as usize).rev() {
        let o = TRAIL + 0x10 * k;
        let (v, wd) = (m.pv_v(o - 0x10), m.pv_f(o - 4));
        m.set_pv_v(o, v);
        m.set_pv_f(o + 0xc, wd);
    }
    let p = m.pos;
    m.set_pv_v(TRAIL, p);
    m.set_pv_f(TRAIL + 0xc, 0.05);
}

/// A shot's trail to draw (`0x3e4050` → `0x3e4300`): its points (x, y, z, width), newest first, and the two ribbons'
/// colours (FX 0x17 inner: white at alpha 0xff; FX 0xe outer: `0x204080` at 0x40; hidden, both × the ticks left / 6).
pub fn trail(m: &crate::moby::Moby) -> Option<(Vec<[f32; 4]>, u32, u32)> {
    if m.o_class != SHOT_CLASS || m.state >= 0xfe { return None; }
    let n = m.pv_i(0x10).clamp(0, TRAIL_POINTS as i32) as usize;
    if n < 2 { return None; }
    let pts = (0..n).map(|k| { let o = TRAIL + 0x10 * k; let v = m.pv_v(o); [v[0], v[1], v[2], m.pv_f(o + 0xc)] }).collect();
    let (mut a1, mut a2) = (0xffu32, 0x40u32);
    if m.state == 1 {
        let t = m.pv_i(0x38) as f32 / 6.0;
        a1 = (255.0 * t) as i32 as u32;
        a2 = (64.0 * t) as i32 as u32;
    }
    Some((pts, a1 << 24 | 0xff_ffff, a2 << 24 | 0x20_4080))
}

/// `0x3e4300(_, 4, _, points, 0x17, 0xe, n, inner, outer)`: the ribbons' quads, segment by segment: at each point the
/// corners ± its width along unit(cross(point − previous, point − camera)) (the outer four times as far), the alpha
/// × (1 − k / n) at point k.
pub fn ribbon_quads(pts: &[[f32; 4]], n: usize, inner: u32, outer: u32, cam: V3) -> [Vec<([V3; 4], [u32; 4])>; 2] {
    let n = n.min(pts.len());
    let mut out = [Vec::new(), Vec::new()];
    if n < 2 { return out; }
    let p3 = |k: usize| [pts[k][0], pts[k][1], pts[k][2]];
    let side = |k: usize| {
        let (p, q) = if k == 0 { (p3(0), p3(1)) } else { (p3(k), p3(k - 1)) };
        let along = if k == 0 { sub(q, p) } else { sub(p, q) };
        let c = crate::hero::physics::cross(along, sub(p, cam));
        let l = len(c);
        if 0.0 < l { scale(c, pts[k][3] / l) } else { [0.0; 3] }
    };
    let fade = |c: u32, k: usize| (c & 0xff_ffff) | ((((c >> 24) as f32) * (1.0 - k as f32 / n as f32)) as i32 as u32) << 24;
    let mut prev: Option<(V3, V3, usize)> = None;
    for k in 0..n {
        let (p, d) = (p3(k), side(k));
        if let Some((pp, pd, pk)) = prev {
            out[0].push(([add(pp, pd), sub(pp, pd), add(p, d), sub(p, d)], [fade(inner, pk), fade(inner, pk), fade(inner, k), fade(inner, k)]));
            let (pd4, d4) = (scale(pd, 4.0), scale(d, 4.0));
            out[1].push(([add(pp, pd4), sub(pp, pd4), add(p, d4), sub(p, d4)], [fade(outer, pk), fade(outer, pk), fade(outer, k), fade(outer, k)]));
        }
        prev = Some((p, d, k));
    }
    out
}
