//! Oozla's swamp flyer, **0x0a49** (L01 `0x3bfe10`; set-up `0x3beab8`, checks `0x3bfa30`, tail `0x3bf8d8`, the
//! steering `0x3bf460`), on the framework ([`crate::moby::ai`]) without a movement block: health `+0x20`, target
//! `+0x100` (its moby `+0x110`, found `+0x114`), reaction `+0x50` (a hurt goes to state 9).
//!
//! **Its pvars:** `+0x1d0` home, `+0x1e0` the goal, `+0x1f0` the velocity, `+0x200` / `+0x201` its update distance
//! and the wander radius, `+0x208` how it starts (0 under the water, 1 hovering, 2 launched), `+0x210` / `+0x214` its
//! paths (the polygon it keeps to, the second while patrolling), `+0x218` / `+0x21c` a cuboid / sphere it may also
//! be in, `+0x224` the floor under it (`+0x27c` the lowest, `+0x288` the top of the floor test), `+0x228` its hover
//! height, `+0x22c` / `+0x29c` the target range and area (`+0x20c` / `+0x294` / `+0x298`), `+0x230` its timer,
//! `+0x238` alarmed, `+0x23c` its wait after a bite, `+0x240` its speed, `+0x24c` the hover offset, `+0x250` the
//! group's attackers (a shared word, at most 3), `+0x254` it is one.
//!
//! **States** (`+0x20`): 0 the set-up. 1 under the water until Ratchet is in its area (or it is poked), then 3 a
//! wait and 2 rising to its hover height (sub 0, then 5) or sinking back (sub 1, then 1). 4 launched upward until
//! it stalls (then 5). 5 hovering on three random sine wobbles for its timer, then 6 patrolling: new goals about
//! home (sub 0 / 1), home and down when nothing is about (sub 2). Seeing him or alarmed, 1 in 9 a tick it hunts (7):
//! goals near him; within 12 and inside its volumes, with fewer than 3 of its group at him and its wait over, it dives
//! (8): to him, the bite (sequence 4: frames 19.5..29, a sphere 0.4 at joint 1, its damage `+0x34`), a 120-tick
//! hover and again, while he is still about; else away. 9 the knock (the framework's flight; 360 ticks at most), then
//! hunting. 0xb dying: its groups alarmed, its bolts, deleted.
//!
//! **The steering** (`0x3bf460(turn accel, brake, most, accel, slow, top, height, stop)`): its speed eases toward the
//! goal across (braking within ten ticks of it, at most `top · +0x28c`/60), the facing springs to it, the height to
//! the floor ahead (a line down) plus the hover height, pitched; the move is checked against collision (L00
//! `0x31c618`, not ported: stood in for by a line to the next place, the move stopping at what it meets).
//!
//! Not ported: its glow (a point light, `0x2f3538`, and its colour), its trail particles (`0x335288`), the splashes
//! and ripples (`0x3be580`, `0x3be818`), the death's fade (`0x3eb1d0`), the screen test of the glow (`0x2f0080`), the
//! game-mode word `$gp−0x60b0` (= 2 stops the dives) and Ratchet's word `0x18c0bc` (0x11 / 0x12 stop them).

use crate::hero::physics::*;
use crate::moby::{ai, countdown, Ctx, MobyWorld};
use super::leaper::{play, seq_a, seq_b, tick_down};
use super::props::wrapped;

/// The class.
const CLASS: i16 = 0x0a49;
const HOME: usize = 0x1d0;
const GOAL: usize = 0x1e0;
const VEL: usize = 0x1f0;
const T_POINT: usize = 0x100;
const T_MOBY: usize = 0x110;
const T_FOUND: usize = 0x114;
/// Ratchet's states that stop the dives: 0x31 and 0x6f (hidden).
const HERO_AWAY: [u16; 2] = [0x31, 0x6f];

/// The steering's arguments.
#[derive(Clone, Copy)]
struct Steer {
    turn_a: f32,
    turn_b: f32,
    turn_most: f32,
    accel: f32,
    slow: f32,
    top: f32,
    height: f32,
    stop: f32,
}

/// `0x317808(p, path, cuboid, sphere, cylinder)`: which of its volumes holds `p` (1 the path's polygon, 2 the cuboid,
/// 3 the sphere, 4 the cylinder; 0 none).
fn in_volume(ctx: &Ctx, p: V3, path: i32, cuboid: i32, sphere: i32) -> i32 {
    if 0 <= path && ctx.level.in_polygon(path, p) { return 1; }
    if ctx.level.in_cuboid(cuboid, p) { return 2; }
    if ctx.level.in_sphere(sphere, p) { return 3; }
    0
}

/// Whether it is inside its volumes (`+0x210`, `+0x218`, `+0x21c`).
fn inside(w: &MobyWorld, id: usize, ctx: &Ctx, p: V3) -> bool {
    let m = &w.mobys[id];
    in_volume(ctx, p, m.pv_i(0x210), m.pv_i(0x218), m.pv_i(0x21c)) != 0
}

/// `0x338d40(&out, p, path)`: `p` inside the path's polygon, else the nearest point of its edges.
fn into_polygon(ctx: &Ctx, path: i32, p: V3) -> V3 {
    let Some(pts) = usize::try_from(path).ok().and_then(|i| ctx.level.paths.get(i)) else { return p };
    if pts.is_empty() || ctx.level.in_polygon(path, p) { return p; }
    let mut best = (f32::MAX, p);
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let (a, b) = ([a[0], a[1], a[2]], [b[0], b[1], b[2]]);
        let ab = sub(b, a);
        let l = dot(ab, ab);
        let t = if l <= 0.0 { 0.0 } else { (dot(sub(p, a), ab) / l).clamp(0.0, 1.0) };
        let q = add(a, scale(ab, t));
        let d = len(sub(p, q));
        if d < best.0 { best = (d, q); }
    }
    best.1
}

/// `0x3be968(top, bottom, lowest, cap, p)`: the floor under `p` (a line from `top`, at most `cap`, down to
/// `bottom`), at least `lowest`.
fn floor_at(ctx: &Ctx, top: f32, bottom: f32, lowest: f32, cap: f32, p: V3) -> f32 {
    let top = if cap != 0.0 { top.min(cap) } else { top };
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, [p[0], p[1], top], [p[0], p[1], bottom], crate::collision::QueryFlags(2), None));
    hit.map_or(lowest, |o| lowest.max(o.point[2]))
}

/// `0x3be9f0`: the floor under it (`+0x224`) and its hover height (`+0x228`, at least 0.1 over Ratchet).
fn floor(w: &mut MobyWorld, id: usize, ctx: &Ctx) {
    let m = &w.mobys[id];
    let (z, up) = if m.state == 0 { (m.pv_f(HOME + 8), 20.0) } else { (m.pos[2], 2.0) };
    let f = floor_at(ctx, z + up, z - 10.0, m.pv_f(0x27c), m.pv_f(0x288), m.pos).max(m.pv_f(0x27c));
    let m = &mut w.mobys[id];
    m.set_pv_f(0x224, f);
    m.set_pv_f(0x228, (f + 1.7).max(ctx.hero.pos[2] + 0.1));
}

/// `0x311f48`: the facing springs toward `yaw` (velocity `+0x244`).
fn spring_yaw(w: &mut MobyWorld, id: usize, yaw: f32, a: f32, b: f32, most: f32) {
    let m = &mut w.mobys[id];
    let (a2, v) = spring_angle(m.rot[2], yaw, (a, b, most), m.pv_f(0x244));
    m.rot[2] = a2;
    m.set_pv_f(0x244, v);
}

/// `0x311de0(a, t, k, d, most, &v)`: `a` sprung toward `t` (`(k, d, most)`, velocity `v`); the new angle and velocity.
fn spring_angle(a: f32, t: f32, (k, d, most): (f32, f32, f32), v: f32) -> (f32, f32) {
    let e = sub_rot(t, a);
    let mut v = v + (k * (e * 6.366_197).clamp(-1.0, 1.0) - d * v);
    if most != 0.0 { v = v.clamp(-most, most); }
    v = v.clamp(-e.abs(), e.abs());
    (add_rot(a, v), v)
}

/// The move along its velocity, stopped at what the line to the next place meets (stands in for L00 `0x31c618`);
/// flags 1 when it met something.
fn step(w: &mut MobyWorld, id: usize, ctx: &Ctx) -> u32 {
    let m = &w.mobys[id];
    let to = add(m.pos, m.pv_v(VEL));
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, m.pos, to, crate::collision::QueryFlags(0), Some(id)));
    let m = &mut w.mobys[id];
    match hit {
        Some(o) => {
            m.pos = add(o.point, set_len(sub(m.pos, o.point), 0.3));
            1
        }
        None => {
            m.pos = to;
            0
        }
    }
}

/// `0x3bf460`: the steering (module docs). The flags: 8 at the goal, 1 met something, 7 outside its volumes.
fn steer(w: &mut MobyWorld, id: usize, ctx: &Ctx, s: Steer) -> u32 {
    let m = &w.mobys[id];
    let goal = m.pv_v(GOAL);
    let yaw_to = atan2(goal[0] - m.pos[0], goal[1] - m.pos[1]);
    let mut flags = 0;
    let d = (len2(sub(goal, m.pos)) - s.stop).max(0.0);
    let mut v = m.pv_f(0x240) + (s.accel * d / 3600.0).min(s.accel / 3600.0);
    if d < v * 10.0 { v -= s.slow * v; }
    if d < s.stop { flags = 8; }
    let most = s.top * m.pv_f(0x28c) / 60.0;
    v = v.clamp(-most, most);
    w.mobys[id].set_pv_f(0x240, v);
    spring_yaw(w, id, yaw_to, s.turn_a * 4.848_137_5e-6, s.turn_b * 4.848_137_5e-6, s.turn_most * 0.000_290_888_25);
    let m = &w.mobys[id];
    let (sn, cs) = yaw_to.sin_cos();
    let ahead = [m.pos[0] + cs * v * 10.0, m.pos[1] + sn * v * 10.0, m.pos[2]];
    let mut low = m.pv_f(0x27c);
    if m.state == 8 { low = low.max(ctx.hero.pos[2]); }
    let home_z = m.pv_f(HOME + 8);
    let mut z = floor_at(ctx, home_z + 2.0, home_z - 3.0, low, m.pv_f(0x288), ahead);
    if dist2(m.pos, ctx.hero.pos) < 5.0 && ctx.hero.pos[2] < z { z = ctx.hero.pos[2]; }
    z += s.height;
    let pitch = atan2(dist2(m.pos, ahead), z - m.pos[2]);
    let yaw = m.rot[2];
    let mut vel = [yaw.cos() * v * pitch.cos(), yaw.sin() * v * pitch.cos(), pitch.sin() * v];
    if m.state == 8 {
        if matches!(m.sub_state, 1 | 2) {
            vel[0] = 0.0;
            vel[1] = 0.0;
        }
    } else {
        vel[2] = m.pv_f(VEL + 8) * 0.4;
    }
    w.mobys[id].set_pv_v(VEL, vel);
    flags |= step(w, id, ctx);
    let m = &mut w.mobys[id];
    let (a, v) = spring_angle(m.rot[1], -pitch, (0.001_745_329_3, 0.000_872_664_6, 0.000_872_664_6), m.pv_f(0x248));
    m.rot[1] = a;
    m.set_pv_f(0x248, v);
    if 0 < w.mobys[id].pv_i(0x238) && !inside(w, id, ctx, w.mobys[id].pos) { flags |= 7; }
    if flags & 7 != 0 { spring_yaw(w, id, yaw_to, s.turn_a * 1.454_441_2e-5, s.turn_b * 4.848_137_5e-6, s.turn_most * 0.000_872_664_74); }
    flags
}

/// `0x3beff0`: the goal kept to its volumes: outside, a place 1.2 about Ratchet (when hunting) or home 1.7 up,
/// into the path's polygon, jittered 3..5 when it is on top of him or of itself; pulled 2 back from a wall between.
fn pick_goal(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let path = if 6 < m.state || m.pv_i(0x214) == -1 { m.pv_i(0x210) } else { m.pv_i(0x214) };
    w.mobys[id].set_pv_i(0xcc, path);
    let m = &w.mobys[id];
    let mut goal = m.pv_v(GOAL);
    if in_volume(ctx, goal, path, m.pv_i(0x218), m.pv_i(0x21c)) == 0 {
        let hunt = ctx.game.rng.rand() & 1 == 0 && m.pv_i(T_FOUND) == 1 && m.pv_i(0x23c) == 0;
        let g = &mut ctx.game.rng;
        if hunt {
            let (a, b) = (g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI));
            goal = add(ctx.hero.pos, [a.cos() * 1.2, b.sin() * 1.2, 1.7]);
        } else {
            g.randi(1);
            goal = add(m.pv_v(HOME), [0.0, 0.0, 1.7]);
        }
        goal = into_polygon(ctx, path, goal);
        let m = &w.mobys[id];
        if (m.pv_i(0x23c) != 0 && dist2(goal, ctx.hero.pos) < 3.0) || dist2(goal, m.pos) < 0.5 {
            for c in goal.iter_mut().take(2) {
                let d = ctx.game.rng.randf(3.0, 5.0);
                *c += if ctx.game.rng.rand() & 1 == 0 { d } else { -d };
            }
        }
    }
    let at = w.mobys[id].pos;
    if let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, at, goal, crate::collision::QueryFlags(2), Some(id))) {
        goal = add(o.point, set_len(sub(o.point, goal), 2.0));
    }
    w.mobys[id].set_pv_v(GOAL, goal);
}

/// `0x3bf330`: a new goal about home (the wander radius `+0x201`), 8 farther from Ratchet after a bite.
fn new_goal(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (a, b) = (ctx.game.rng.randf(-std::f32::consts::PI, std::f32::consts::PI), ctx.game.rng.randf(-std::f32::consts::PI, std::f32::consts::PI));
    let m = &mut w.mobys[id];
    let r = m.pv_b(0x201) as f32;
    let mut goal = add(m.pv_v(HOME), [a.cos() * r, b.sin() * r, 1.7]);
    if m.pv_i(0x23c) != 0 {
        let away = sub(goal, ctx.hero.pos);
        goal = add(goal, set_len([away[0], away[1], 0.0], 8.0));
    }
    m.set_pv_v(GOAL, goal);
    pick_goal(w, id, ctx);
}

/// `0x3befb8`: it leaves the group's attackers.
fn release(w: &mut MobyWorld, id: usize) {
    if w.mobys[id].pv_i(0x254) == 0 { return; }
    let p = w.mobys[id].pv_i(0x250);
    let mut n = w.shared_i(p);
    countdown(&mut n);
    w.set_shared_i(p, n);
    w.mobys[id].set_pv_i(0x254, 0);
}

/// `0x3bef30`: its groups (its own, `+0x280`; `+0x284` reads `+0x280` again in the game) poked (`0x31c970`: the
/// members' `+0xbc`), itself too.
fn alarm(w: &mut MobyWorld, id: usize) {
    let m = &w.mobys[id];
    let mut groups = vec![m.group as i32];
    if m.pv_i(0x280) != -1 { groups.push(m.pv_i(0x280)); }
    if m.pv_i(0x284) != -1 { groups.push(m.pv_i(0x280)); }
    for g in groups {
        for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() {
            if w.mobys[i].state < 0x80 { w.mobys[i].timer = 1; }
        }
    }
    w.mobys[id].timer = 1;
}

/// Ratchet is about for a dive (`0x18c0b4` not 0x31 / 0x6f).
fn hero_about(ctx: &Ctx) -> bool { !HERO_AWAY.contains(&(ctx.hero.state as u16)) }

/// `0x3beab8`: the set-up (module docs).
fn setup(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if ctx.game.frame == 0 { let p = w.mobys[id].pv_i(0x250); w.set_shared_i(p, 0); }
    let m = &mut w.mobys[id];
    let pos = m.pos;
    m.set_pv_v(HOME, pos);
    floor(w, id, ctx);
    let g = &mut ctx.game.rng;
    let r = [g.randf(0.0, 1.0), g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI)];
    let rates = [g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI)];
    let flip = (g.rand() & 0x8000) as u16;
    let m = &mut w.mobys[id];
    m.set_pv_b(0x48, 10);
    m.set_pv_b(0x4a, 4);
    let ud = m.update_dist;
    m.set_pv_b(0x200, ud);
    m.set_pv_b(0x201, ud);
    // The reaction block (`+0x50`): a hurt goes to state 9.
    m.set_pv_i(0x10, 0x50);
    m.set_pv_b(0x60, 9);
    m.set_pv_f(0x80, 0.3);
    m.set_pv_f(0x8c, 0.4);
    m.set_pv_b(0x69, 1);
    m.set_pv_b(0xc6, 1);
    m.set_pv_s(0x88, 0);
    m.set_pv_b(0x6a, 0);
    m.set_pv_i(0x98, 0);
    m.set_pv_f(0x84, 0.0);
    m.set_pv_i(0x220, 0);
    m.set_pv_b(0x28, 0);
    m.set_pv_f(0x30, 0.0);
    m.set_pv_v(GOAL, pos);
    m.set_pv_f(0x240, 0.0);
    m.set_pv_f(0x24c, r[0]);
    m.set_pv_f(0x26c, r[1]);
    m.set_pv_f(0x270, r[2]);
    m.set_pv_f(0x274, r[3]);
    m.set_pv_f(0x260, rates[0] * 0.000_290_888_25);
    m.set_pv_f(0x264, rates[1] * 0.000_290_888_25);
    m.set_pv_f(0x268, rates[2] * 0.000_290_888_25);
    m.set_pv_s(0x202, -1);
    m.set_pv_f(0x204, 0.0);
    m.set_pv_i(0x290, 0);
    m.set_pv_f(0x22c, 0.0);
    m.mode |= flip;
    m.set_pv_i(0xcc, m.pv_i(0x214));
    m.set_pv_i(0x29c, m.pv_i(0x294));
    m.set_pv_f(0x28c, 1.0);
    match m.pv_i(0x208) {
        1 => {
            m.pos[2] = m.pv_f(0x224) + m.pv_f(0x24c) + 1.7;
            let p = m.pos;
            m.set_pv_v(HOME, p);
            m.set_pv_v(VEL, [0.0; 3]);
            m.mode |= 0x1000;
            play(w, id, ctx, 1, 0, 3);
            w.mobys[id].set_state(6, 0);
        }
        0 => {
            m.pos[2] = m.pv_f(0x224) - 1.7;
            let p = m.pos;
            m.set_pv_v(HOME, p);
            m.set_pv_v(VEL, [0.0; 3]);
            m.mode &= !0x1000;
            play(w, id, ctx, 1, 0, 3);
            w.mobys[id].set_state(1, -1);
        }
        2 => {
            let v = m.pv_v(0xd0);
            m.set_pv_v(VEL, v);
            m.pos = add(m.pos, v);
            let p = m.pos;
            m.set_pv_v(HOME, p);
            play(w, id, ctx, 5, 0, 3);
            w.mobys[id].mode |= 0x1000;
            w.mobys[id].set_state(4, -1);
        }
        _ => {}
    }
}

/// `0x3bfa30`: the target, the floor, the damage, the hover's moods.
fn checks(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (range, area) = (w.mobys[id].pv_f(0x22c), w.mobys[id].pv_i(0x29c));
    ai::find_target(w, id, ctx, range, 0.0, area, None, false);
    floor(w, id, ctx);
    w.mobys[id].scale = w.class(CLASS).map_or(1.0, |c| c.scale) * 1.25;
    if ai::take_damage(w, id, ctx, false) != 0 {
        if w.mobys[id].pv_f(0x20) <= 0.0 { w.mobys[id].set_pv_i(0xcc, -1); }
        release(w, id);
        alarm(w, id);
    }
    let state = w.mobys[id].state;
    if 6 <= state && tick_down(w, id, 600) {
        let hi = 0.5 < w.mobys[id].pv_f(0x24c);
        let off = if hi { ctx.game.rng.randf(0.0, 0.4) } else { ctx.game.rng.randf(0.6, 1.0) };
        let t = ctx.game.rng.rand_range(0x2d, 0x96);
        let m = &mut w.mobys[id];
        m.set_pv_f(0x24c, off);
        m.set_pv_i(600, t);
    }
    if state == 8 {
        w.mobys[id].set_pv_f(0x28c, 1.0);
    } else if 5 < state && tick_down(w, id, 0x25c) {
        let k = if -1.75 < w.mobys[id].pv_f(0x28c) { ctx.game.rng.randf(0.5, 1.1) } else { ctx.game.rng.randf(1.65, 5.0) };
        let t = ctx.game.rng.rand_range(0x6e, 0x3c);
        let m = &mut w.mobys[id];
        m.set_pv_f(0x28c, k);
        m.set_pv_i(0x25c, t);
    }
}

/// `0x3bf8d8`: the timers at the end of the tick.
fn tail(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].timer != 0 {
        w.mobys[id].timer = 0;
        if w.mobys[id].pv_i(0x238) < 200 {
            let t = ctx.game.rng.rand_range(300, 360);
            w.mobys[id].set_pv_i(0x238, t);
        }
    }
    if w.mobys[id].pv_i(T_FOUND) != 1 { tick_down(w, id, 0x238); }
    let m = &mut w.mobys[id];
    if (m.state_ticks as u16) < 0xfffe { m.state_ticks = m.state_ticks.wrapping_add(1); }
    tick_down(w, id, 0x230);
    tick_down(w, id, 0x23c);
}

/// A state with the range and area its hunt or rest uses.
fn go(w: &mut MobyWorld, id: usize, state: u8, sub: i32, hunting: bool) {
    let m = &mut w.mobys[id];
    m.set_pv_i(0x22c, if hunting { m.pv_i(0x20c) } else { 0 });
    m.set_pv_i(0x29c, m.pv_i(0x298));
    m.set_state(state, sub);
}

/// 0x0a49 (module docs).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if ctx.hero.state == 0x6f {
        let m = &mut w.mobys[id];
        m.drawn = 0;
        m.mode |= 1;
        return;
    }
    if w.mobys[id].mode & 1 != 0 && w.mobys[id].pv_s(0x202) == -1 {
        let m = &mut w.mobys[id];
        m.mode &= !1;
        m.drawn = 1;
    }
    if w.mobys[id].state != 0 { checks(w, id, ctx); }
    match w.mobys[id].state {
        0 => setup(w, id, ctx),
        1 => {
            let m = &mut w.mobys[id];
            m.set_pv_f(0x22c, 0.0);
            m.set_pv_i(0x29c, m.pv_i(0x294));
            if m.timer != 0 || m.pv_i(T_FOUND) == 1 {
                m.update_dist = m.pv_b(0x200);
                m.set_pv_v(VEL, [0.0; 3]);
                let t = ctx.game.rng.rand_range(0x3c, 0x78);
                w.mobys[id].set_pv_i(0x230, t);
                w.mobys[id].set_state(3, -1);
            }
        }
        2 => rise(w, id, ctx),
        3 => {
            if tick_down(w, id, 0x230) {
                if seq_b(w, id, ctx) != 0 { play(w, id, ctx, 0, 0, 10); }
                w.mobys[id].set_state(2, 0);
            }
        }
        4 => launched(w, id, ctx),
        5 => hover(w, id, ctx),
        6 => patrol(w, id, ctx),
        7 => hunt(w, id, ctx),
        8 => dive(w, id, ctx),
        9 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x220, m.pv_i(0x220) + 1);
            let res = ai::flight(w, id, ctx);
            if res & 1 != 0 {
                w.mobys[id].set_state(0xb, -1);
            } else if res & 2 != 0 || 0x169 <= w.mobys[id].pv_i(0x220) {
                w.mobys[id].set_pv_b(0x6a, 2);
                play(w, id, ctx, 1, 0, 3);
                let m = &mut w.mobys[id];
                m.set_pv_i(0x220, 0);
                m.set_pv_v(VEL, [0.0; 3]);
                go(w, id, 7, 0, true);
            }
        }
        0xb => {
            alarm(w, id);
            release(w, id);
            crate::moby::bolts::drop(w, id, 0, 3, ctx);
            w.delete(id);
            return;
        }
        _ => {}
    }
    if w.mobys[id].state >= 0x80 { return; }
    tail(w, id, ctx);
    let p = w.mobys[id].pos;
    if p.iter().any(|&c| !(30.0..=993.0).contains(&c)) { w.delete(id); }
}

/// State 2: rising to its hover height (sub 0) or sinking under the water (sub 1).
fn rise(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    let t = match m.sub_state {
        0 => m.pv_f(0x228),
        1 => m.pv_f(0x224) - 1.7,
        _ => m.pos[2],
    };
    let (mut z, mut v) = (m.pos[2], m.pv_f(VEL + 8));
    ease(t, 0.006_944_444_6, 0.004_166_667, 0.5, &mut z, &mut v);
    m.pos[2] = z;
    m.set_pv_f(VEL + 8, v);
    if 0.2 <= (m.pos[2] - t).abs() { return; }
    match m.sub_state {
        1 => {
            m.mode &= !0x1000;
            if seq_b(w, id, ctx) != 1 { play(w, id, ctx, 1, 0, 3); }
            let m = &mut w.mobys[id];
            m.set_pv_i(0x230, 0);
            m.set_pv_f(0x22c, 0.0);
            m.set_pv_i(0x29c, m.pv_i(0x294));
            m.set_state(1, -1);
        }
        0 => {
            m.mode |= 0x1000;
            if seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
            let t = ctx.game.rng.rand_range(0xb4, 0x168);
            w.mobys[id].set_pv_i(0x230, t);
            w.mobys[id].set_state(5, -1);
        }
        _ => {}
    }
}

/// State 4: launched upward until it stalls, meets something, or 240 ticks.
fn launched(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    m.pos = add(m.pos, m.pv_v(VEL));
    m.set_pv_f(VEL + 8, m.pv_f(VEL + 8) - 0.002_722_222_4);
    m.pos[2] += 0.3;
    let at = m.pos;
    let hit = ctx.coll.and_then(|c| crate::collision::coll_sphere_m(c, ctx.scene, at, 0.3, crate::collision::QueryFlags(0), Some(id)));
    let to_hover = |w: &mut MobyWorld, ctx: &mut Ctx| {
        if seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
        let t = ctx.game.rng.rand_range(0xb4, 0x168);
        w.mobys[id].set_pv_i(0x230, t);
        w.mobys[id].set_state(5, -1);
    };
    if let Some(o) = hit {
        if o.moby.is_none_or(|i| w.mobys[i].o_class != 0x945) { to_hover(w, ctx); }
    }
    if w.mobys[id].pv_f(VEL + 8) < 0.0 || 0xf0 < w.mobys[id].pv_i(0x234) { to_hover(w, ctx); }
    let m = &mut w.mobys[id];
    m.pos[2] -= 0.3;
    m.set_pv_f(VEL + 8, m.pv_f(VEL + 8) - 0.002_722_222_4);
    if m.pos[2] < m.pv_f(0x224) { m.pos[2] = m.pv_f(0x224); }
}

/// State 5: hovering on its three wobbles.
fn hover(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    for (timer, rate, lo, hi, tlo, thi) in [(600, 0x260, 10.0, 180.0, 0x1e, 0xb4), (0x25c, 0x264, 60.0, 170.0, 0x3c, 200), (0x278, 0x268, 30.0, 160.0, 0x2d, 0x5a)] {
        if tick_down(w, id, timer) {
            let r = ctx.game.rng.randf(lo, hi) * 0.000_290_888_25;
            let t = ctx.game.rng.rand_range(tlo, thi);
            let m = &mut w.mobys[id];
            m.set_pv_f(rate, r);
            m.set_pv_i(timer, t);
        }
    }
    let m = &mut w.mobys[id];
    let a = add_rot(m.pv_f(0x26c), m.pv_f(0x260));
    m.set_pv_f(0x26c, a);
    let b = add_rot(m.pv_f(0x270), m.pv_f(0x264) * a.sin());
    m.set_pv_f(0x270, b);
    let c = add_rot(m.pv_f(0x274), m.pv_f(0x268) * add_rot(a.cos(), b.sin()));
    m.set_pv_f(0x274, c);
    let dir = add_rot(m.rot[2], b.sin() * 0.162_897_42);
    let goal = add(m.pos, [dir.cos(), dir.sin(), 0.0]);
    m.set_pv_v(GOAL, goal);
    pick_goal(w, id, ctx);
    let h = w.mobys[id].pv_f(0x24c) + 2.0 * w.mobys[id].pv_f(0x274).sin();
    steer(w, id, ctx, Steer { turn_a: 360.0, turn_b: 180.0, turn_most: 560.0, accel: 20.0, slow: 0.0, top: 16.0, height: h, stop: -2.0 });
    if w.mobys[id].pv_i(0x230) != 0 { return; }
    if seq_b(w, id, ctx) != 1 { play(w, id, ctx, 1, 0, 3); }
    w.mobys[id].set_pv_f(0x22c, 0.0);
    w.mobys[id].set_state(6, 0);
}

/// The patrol's steering.
fn patrol_steer(w: &mut MobyWorld, id: usize, ctx: &Ctx) -> u32 {
    let h = w.mobys[id].pv_f(0x24c) + 1.7;
    steer(w, id, ctx, Steer { turn_a: 120.0, turn_b: 360.0, turn_most: 120.0, accel: 30.0, slow: 0.0, top: 8.0, height: h, stop: 1.0 })
}

/// State 6: patrolling about home.
fn patrol(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].sub_state {
        1 => {
            patrol_steer(w, id, ctx);
            if w.mobys[id].pv_i(0x230) == 0 {
                new_goal(w, id, ctx);
                let t = ctx.game.rng.rand_range(0, 0);
                w.mobys[id].set_pv_i(0x230, t);
                if 6 < t && seq_b(w, id, ctx) != 1 { play(w, id, ctx, 1, 0, 3); }
                w.mobys[id].set_pv_f(0x22c, 0.0);
                w.mobys[id].set_state(6, 0);
            }
        }
        0 => {
            if w.mobys[id].pv_i(0x230) == 0 {
                let t = ctx.game.rng.rand_range(0x1e, 0x5a);
                w.mobys[id].set_pv_i(0x230, t);
                if 6 < t && seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
                w.mobys[id].set_pv_f(0x22c, 0.0);
                w.mobys[id].set_state(6, 1);
            }
        }
        2 => {
            let home = w.mobys[id].pv_v(HOME);
            w.mobys[id].set_pv_v(GOAL, home);
            if patrol_steer(w, id, ctx) & 8 != 0 {
                if seq_b(w, id, ctx) != 2 { play(w, id, ctx, 2, 0, 10); }
                w.mobys[id].set_state(2, 1);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.set_pv_f(0x22c, 0.0);
    if m.pv_i(T_FOUND) != 1 && m.pv_i(0x238) == 0 {
        if m.state == 6 && m.pv_i(0x208) == 0 { m.set_state(6, 2); }
        return;
    }
    if ctx.game.rng.randi(9) != 0 { return; }
    let t = ctx.game.rng.rand_range(300, 0x168);
    w.mobys[id].set_pv_i(0x238, t);
    go(w, id, 7, 0, true);
}

/// State 7: hunting near Ratchet.
fn hunt(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].sub_state {
        0 => {
            if w.mobys[id].pv_i(0x230) == 0 {
                let t = ctx.game.rng.rand_range(0x1e, 0x5a);
                w.mobys[id].set_pv_i(0x230, t);
                if 6 < t && seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
                go(w, id, 7, 1, true);
            }
        }
        1 => {
            let h = w.mobys[id].pv_f(0x24c) + 1.7;
            let f = steer(w, id, ctx, Steer { turn_a: 250.0, turn_b: 180.0, turn_most: 250.0, accel: 32.0, slow: 0.1, top: 13.0, height: h, stop: 1.0 });
            if f & 8 != 0 { w.mobys[id].set_pv_i(0x230, 0); }
            if w.mobys[id].pv_i(0x230) == 0 {
                new_goal(w, id, ctx);
                let t = ctx.game.rng.rand_range(0, 0);
                w.mobys[id].set_pv_i(0x230, t);
                if 6 < t && seq_b(w, id, ctx) != 1 { play(w, id, ctx, 1, 0, 3); }
                go(w, id, 7, 0, true);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.set_pv_i(0x22c, m.pv_i(0x20c));
    m.set_pv_i(0x29c, m.pv_i(0x298));
    if m.pv_i(T_FOUND) != 0 && hero_about(ctx) {
        let slot = m.pv_i(0x250);
        let attackers = w.shared_i(slot);
        let m = &w.mobys[id];
        if attackers < 3 && m.pv_i(0x23c) == 0 && dist2(m.pos, m.pv_v(T_POINT)) < 12.0 && inside(w, id, ctx, m.pos) {
            let p = m.pv_i(0x250);
            w.set_shared_i(p, attackers + 1);
            let m = &mut w.mobys[id];
            m.set_pv_i(0x254, 1);
            let t = m.pv_v(T_POINT);
            m.set_pv_v(GOAL, t);
            go(w, id, 8, 0, false);
        }
        return;
    }
    w.mobys[id].set_pv_i(T_FOUND, 0);
    if w.mobys[id].pv_i(0x238) != 0 { return; }
    w.mobys[id].set_pv_f(0x22c, 0.0);
    w.mobys[id].set_state(6, 0);
}

/// State 8: the dive and the bite.
fn dive(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let target = w.mobys[id].pv_v(T_POINT);
    w.mobys[id].set_pv_v(GOAL, target);
    let s = Steer { turn_a: 260.0, turn_b: 180.0, turn_most: 270.0, accel: 32.0, slow: 0.2, top: 3.5, height: 0.4, stop: 0.5 };
    let t_z = ai::link(&w.mobys[id], T_MOBY).and_then(|t| w.mobys.get(t)).map_or(target[2], |t| t.pos[2]);
    let near = |w: &MobyWorld| dist2(w.mobys[id].pos, target) < 1.0 && (w.mobys[id].pos[2] - t_z).abs() < 1.3;
    let phase = w.mobys[id].sub_state;
    match phase {
        1 => {
            steer(w, id, ctx, s);
            go(w, id, 8, 2, false);
        }
        0 => {
            steer(w, id, ctx, s);
            if near(w) {
                if seq_b(w, id, ctx) != 4 { play(w, id, ctx, 4, 0, 10); }
                go(w, id, 8, 1, false);
            }
        }
        2 => {
            steer(w, id, ctx, s);
            if seq_a(w, id, ctx) == 4 {
                let t = ctx.host.frame_time(&w.mobys[id]);
                if 19.5 < t && t < 29.0 {
                    if let Some(j) = ctx.host.joint(&w.mobys[id], 1) {
                        let damage = w.mobys[id].pv_b(0x34) as f32;
                        ai::attack_sphere(w, id, ctx, 0.4, damage, 1.0, j, 0x400_0001, 0, 1, 0);
                    }
                }
                if wrapped(w, id, ctx) {
                    let wait = ctx.game.rng.rand_range(0xb4, 0x104);
                    let m = &mut w.mobys[id];
                    m.set_pv_i(0x230, 0x78);
                    m.set_pv_i(0x23c, wait);
                    if seq_b(w, id, ctx) != 1 { play(w, id, ctx, 1, 0, 3); }
                    go(w, id, 8, 3, false);
                }
            }
        }
        3 if tick_down(w, id, 0x230) => {
            if near(w) {
                if seq_b(w, id, ctx) != 4 { play(w, id, ctx, 4, 0, 3); }
                go(w, id, 8, 1, false);
            } else {
                w.mobys[id].set_pv_v(GOAL, target);
                go(w, id, 8, 0, false);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.set_pv_i(0x22c, 0);
    m.set_pv_i(0x29c, m.pv_i(0x298));
    let inside_now = inside(w, id, ctx, w.mobys[id].pos);
    if w.mobys[id].pv_i(T_FOUND) != 0 && hero_about(ctx) && inside_now { return; }
    release(w, id);
    if w.mobys[id].pv_i(0x238) != 0 {
        w.mobys[id].set_pv_i(0x230, 0);
        if seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
        w.mobys[id].set_pv_f(0x22c, 0.0);
        w.mobys[id].set_state(6, 1);
        return;
    }
    // Away: the game's goal is `set_len(2·pos − goal, 5)`, which the volumes then pull back in.
    let m = &mut w.mobys[id];
    let g = set_len(sub(scale(m.pos, 2.0), m.pv_v(GOAL)), 5.0);
    m.set_pv_v(GOAL, g);
    pick_goal(w, id, ctx);
    let m = &mut w.mobys[id];
    m.set_pv_f(0x240, m.pv_f(0x240) * 3.0);
    let (turn, most) = if inside_now { (260.0, 270.0) } else { (780.0, 810.0) };
    steer(w, id, ctx, Steer { turn_a: turn, turn_b: 180.0, turn_most: most, accel: 32.0, slow: 0.2, top: 3.5, height: 0.3, stop: 0.5 });
    w.mobys[id].set_pv_i(0x230, 0);
    if seq_b(w, id, ctx) != 3 { play(w, id, ctx, 3, 0, 3); }
    let wait = ctx.game.rng.rand_range(0xb4, 0x104);
    w.mobys[id].set_pv_i(0x23c, wait);
    go(w, id, 7, 1, true);
}
