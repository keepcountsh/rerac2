//! Oozla's spitting swamp creature, **0x0b2b** (L01 `0x3dc300`; set-up `0x3de3e8`, checks `0x3de848`, tail
//! `0x3deb50`, the spit `0x3dd520`), on the framework ([`crate::moby::ai`]), of the leaper's family
//! ([`super::leaper`]): health block `+0x20`, target `+0x2e0` (its moby `+0x2f0`, found `+0x2f4`), reaction `+0x110`
//! (a hurt goes to sub-state 5), movement `+0x1c0`, the wander block `+0x2b0`.
//!
//! It finds Ratchet only by the trigger area `+0x3f8` (`+0x414` / `+0x418` / `+0x41c` by its flags, `+0x41c` once
//! it is out), or takes the moby `+0x408` (a 0x0b1a) as its target.
//!
//! **Sub-states** (`+0x95`): 0 the set-up: `+0x3d0` set it waits 2 under the water, hidden (1), else on the bank
//! (3; `+0x3d1` / `+0x3d2` pick the area and whether it waits at state 0 of sub 1). 1 waiting: with Ratchet in its
//! area and `+0x3d4` run out, a hidden one leaps out to the farthest of its cuboids `+0x3c4..+0x3cc` (sequence 0xc,
//! 51 frames over the flight; 2), a shown one walks out (3, state 3). 2 the leap's flight (as the leaper's, gravity
//! 0.005), down at the landing it stands until its sequence ends, then wanders (3). 3 on the bank: 1 idle, then
//! wandering (2) about home for `+0x3d8` ticks; seeing him (or its target moby), the one its group picked
//! (`+0x400`, below) with no wait `+0x404` turns to him (5, sequence 2/3) and closes in (3, sequence 7); within 3 (1
//! of a 0x0b1a) it spits (sub 4; now and then, 1 in 39 · 1 in 3, unprovoked); 4 backing off after a spit until 7 from
//! him or its wait is over. 4 the spit (sequence 8): frames 18..23 bite at the mouth (radius 0.2, its damage `+0x34`)
//! and spray the stream, which hits along its length and at its tip while it lives; after it, 4 of sub 3. 5 the
//! knock (the framework's flight): dead → 6; over → idle (sub 3), out again. 6 dying: its bolts, deleted.
//!
//! **The group's shared words** (gameplay section 21): `+0x3fc` the frame of the pick, `+0x400` the uid of its
//! creature nearest Ratchet on the bank (sub 3, not backing off), every 10 frames.
//!
//! **The stream.** The game's spray is particles (type 2, `0x328640` = L00 `0x3327e8`: not ported, G-PRT-001); the
//! stream's hits follow its first particle (`+0x3f0`): its reach `+0x3f4` grows with the particle's distance from
//! the mouth while it lives, the hits stop when it is gone. Here the particle is stood in for by its flight: 0.15 a
//! tick from the spit's first spray frame for its life of 30..36 ticks (the spawn's arguments).
//!
//! Not ported: the shadow, the tail's aim (`0x31cfa0`), the splashes and ripples (`0x3dd160`, `0x3dd3e0`,
//! `0x3dd030`), the spray's particles, the death's fade (`0x3eb1d0`).

use crate::hero::physics::*;
use crate::moby::{ai, countdown, Ctx, MobyWorld};
use super::leaper::{cuboid_at, play, seq_a, seq_b, tick_down, tilt_to_floor, wander};
use super::props::wrapped;

/// The class.
const CLASS: i16 = 0x0b2b;
/// The movement, wander and target blocks.
const MV: usize = 0x1c0;
const WANDER: usize = 0x2b0;
const T_MOBY: usize = 0x2f0;
const T_FOUND: usize = 0x2f4;

/// The target moby and its place.
fn target(w: &MobyWorld, id: usize) -> Option<(usize, V3)> {
    ai::link(&w.mobys[id], T_MOBY).and_then(|t| w.mobys.get(t).map(|m| (t, m.pos)))
}

/// A 16-bit countdown on a pvar word (`0x2e3e18`).
fn tick_down16(w: &mut MobyWorld, id: usize, off: usize) -> bool {
    let m = &mut w.mobys[id];
    let mut t = m.pv_s(off) as i32;
    let r = countdown(&mut t);
    m.set_pv_s(off, t as i16);
    r
}

/// `0x3ddce0`: its group's spitters are alarmed (`+0x3da`).
fn alarm_group(w: &mut MobyWorld, id: usize) {
    let g = w.mobys[id].group;
    let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() else { return };
    for i in members {
        let m = &mut w.mobys[i];
        if m.state < 0x80 && m.o_class == CLASS { m.set_pv_s(0x3da, 1); }
    }
}

/// `0x3dde88(m, before)`: the hold of a 0x0a56 / 0x0a53 it stands on (`+0x410`), else kept inside its fence path's
/// polygon (back to `before` when it left it); a floor moby of those classes is taken hold of.
fn cling(w: &mut MobyWorld, id: usize, ctx: &Ctx, before: V3) {
    let held = w.mobys[id].pv_i(0x410);
    if let Some(h) = usize::try_from(held).ok().and_then(|h| w.mobys.get(h)).cloned() {
        let pos = w.mobys[id].pos;
        let goal = match h.o_class {
            0x0a56 => {
                let mut g = if 1.0 < dist2(pos, h.pos) { add(h.pos, set_len(sub(pos, h.pos), 1.0)) } else { pos };
                if g[2] < h.pos[2] + 0.01 { g[2] = h.pos[2] + 0.011; }
                g
            }
            0x0a53 => {
                let rows = h.rows;
                let d = sub(pos, h.pos);
                let l = [dot(d, rows[0]).clamp(-2.0, 2.0), dot(d, rows[1]).clamp(-5.0, 5.0), dot(d, rows[2]).max(0.01)];
                add(h.pos, std::array::from_fn(|k| l[0] * rows[0][k] + l[1] * rows[1][k] + l[2] * rows[2][k]))
            }
            _ => pos,
        };
        let mut step = sub(goal, pos);
        if 0.1 < len(step) { step = set_len(step, 0.1); }
        let m = &mut w.mobys[id];
        m.pos = add(pos, step);
        if m.pos[2] < goal[2] { m.pos[2] = goal[2] + 0.001; }
        return;
    }
    let fence = w.mobys[id].pv_i(MV + 0x3c);
    if fence != -1 && !ctx.level.in_polygon(fence, w.mobys[id].pos) { w.mobys[id].pos = before; }
    if let Some(f) = ai::link(&w.mobys[id], MV + 0x88).filter(|&f| matches!(w.mobys.get(f).map(|x| x.o_class), Some(0x0a56 | 0x0a53))) {
        w.mobys[id].set_pv_i(0x410, f as i32);
    }
}

/// `0x3ddd60` / `0x3dde50`: the 0x0a26 mobys within 3 made passable and stirred for the move (`+0x3e4` set).
fn with_reeds(w: &mut MobyWorld, id: usize, f: impl FnOnce(&mut MobyWorld) -> i32) -> i32 {
    let near: Vec<usize> = if w.mobys[id].pv_b(0x3e4) != 0 {
        let at = w.mobys[id].pos;
        (0..w.mobys.len()).filter(|&i| i != id && w.mobys[i].state < 0x80 && w.mobys[i].o_class == 0x0a26 && dist(w.mobys[i].pos, at) < 3.0).take(16).collect()
    } else {
        Vec::new()
    };
    for &i in &near {
        w.mobys[i].coll_disable = 1;
        w.mobys[i].timer = 1;
    }
    let r = f(w);
    for i in near { w.mobys[i].coll_disable = 0; }
    r
}

/// `0x3de1e8`: the wander's frame.
fn roam(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let before = w.mobys[id].pos;
    with_reeds(w, id, |w| wander(w, id, ctx, WANDER));
    cling(w, id, ctx, before);
}

/// `0x3de280(m, goal)`: the move toward `goal` (`0x322450`).
fn go_to(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, goal: V3) -> i32 {
    let before = w.mobys[id].pos;
    let r = with_reeds(w, id, |w| {
        let p = w.mobys[id].pos;
        ai::move_step(w, id, ctx, atan2(goal[0] - p[0], goal[1] - p[1]), goal)
    });
    cling(w, id, ctx, before);
    r
}

/// The area it watches: the first of `a`, `b`, `c` that is set.
fn area_of(m: &crate::moby::Moby, a: usize, b: usize, c: usize) -> i32 {
    [m.pv_i(a), m.pv_i(b)].into_iter().find(|&x| x != -1).unwrap_or(m.pv_i(c))
}

/// Shown or hidden 2 under the water (no collision, not drawn, mode 1).
fn hide(w: &mut MobyWorld, id: usize, hidden: bool) {
    let solid = w.class(CLASS).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    if hidden {
        m.has_collision = false;
        m.drawn = 0;
        m.mode |= 1;
    } else {
        m.has_collision = solid;
        m.drawn = 1;
        m.mode &= !1;
    }
}

/// `0x3de3e8`: the set-up (module docs).
fn setup(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let turn = ctx.game.rng.randf_signed(25.0, 35.0) * 0.017_453_294;
    let m = &mut w.mobys[id];
    m.set_pv_i(0x18, MV as i32);
    // The reaction block (`+0x110`): a hurt goes to sub-state 5.
    m.set_pv_b(0x48, 10);
    m.set_pv_b(0x4a, 9);
    m.set_pv_i(0x158, 5);
    m.set_pv_b(0x127, 0xb);
    m.set_pv_f(0x12c, 0.2);
    m.set_pv_f(0x130, 0.1);
    m.set_pv_f(0x134, 0.006_11);
    m.set_pv_f(0x138, 0.010_278);
    m.set_pv_f(0x13c, 0.011_111);
    m.set_pv_f(0x14c, 0.4);
    m.set_pv_b(0x182, 6);
    m.set_pv_b(0x183, 10);
    m.set_pv_b(0x29, 1);
    m.set_pv_f(0x30, 0.5);
    for (o, v) in [(0x120, 0u8), (0x121, 10), (0x122, 10), (0x126, 10), (0x17c, 6), (0x17d, 10), (0x17e, 6), (0x17f, 10), (0x28, 0)] { m.set_pv_b(o, v); }
    m.set_pv_f(0x140, 0.5);
    m.set_pv_f(0x144, 0.5);
    m.set_pv_s(0x148, 0);
    m.set_pv_s(0x3d8, 0);
    let pos = m.pos;
    m.set_pv_v(0x3a0, pos);
    m.set_pv_v(WANDER, pos);
    m.set_pv_f(WANDER + 0x20, 5.0);
    m.set_pv_f(WANDER + 0x10, 0.3);
    m.set_pv_f(WANDER + 0x1c, 5.0);
    // The movement block.
    m.set_pv_f(MV, 0.3);
    m.set_pv_f(MV + 4, 0.1);
    m.set_pv_f(MV + 8, 0.2);
    m.set_pv_f(MV + 0xc, 0.2);
    m.set_pv_f(MV + 0x10, 0.872_664_6);
    m.set_pv_f(MV + 0x30, std::f32::consts::FRAC_PI_6);
    m.set_pv_f(MV + 0x2c, 0.005_555_6);
    m.set_pv_i(MV + 0x3c, m.pv_i(0x3bc));
    m.set_pv_f(0x29c, turn);
    m.set_pv_s(0x2a2, 0x78);
    m.set_pv_s(0x2a4, 0xb4);
    // `0x3235c8`: the walk and turn limits.
    m.set_pv_f(MV + 0x1c, 0.192_234_44);
    m.set_pv_f(MV + 0x24, 0.002_222_222_4);
    m.set_pv_f(MV + 0x28, 0.066_666_67);
    m.set_pv_f(MV + 0x18, 0.002_131_074);
    m.set_pv_f(MV + 0x20, 0.002_222_222_4);
    m.set_pv_f(MV + 0x14, 0.002_131_074);
    m.rot[0] = 0.0;
    m.rot[1] = 0.0;
    m.set_pv_v(0x210, [0.0, 0.0, 1.0]);
    if m.group != -1 {
        let (a, b) = (m.pv_i(0x3fc), m.pv_i(0x400));
        w.set_shared_i(a, 0);
        w.set_shared_i(b, -1);
    }
    let m = &w.mobys[id];
    let (under, alt, waits) = (m.pv_b(0x3d0) != 0, m.pv_b(0x3d1) != 0, m.pv_b(0x3d2) != 0);
    if under {
        let a = area_of(m, 0x414, 0x418, 0x41c);
        w.mobys[id].set_pv_i(0x3f8, a);
        hide(w, id, true);
        let m = &mut w.mobys[id];
        m.pos[2] -= 2.0;
        m.set_state(0, 1);
        if seq_b(w, id, ctx) != 0 {
            let b = ctx.game.rng.rand_range(2, 10);
            play(w, id, ctx, 0, 0, b);
        }
        return;
    }
    let a = if alt { area_of(m, 0x414, 0x418, 0x41c) } else { area_of(m, 0x418, 0x414, 0x41c) };
    let m = &mut w.mobys[id];
    m.set_pv_i(0x3f8, a);
    if alt && waits { m.set_state(0, 1) } else { m.set_state(1, 3) }
    let (s, now) = if m.pv_i(0x408) == -1 { (0, seq_b(w, id, ctx)) } else { (7, seq_b(w, id, ctx)) };
    if now != s {
        let b = ctx.game.rng.rand_range(2, 10);
        play(w, id, ctx, s, 0, b);
    }
}

/// `0x3de848`: the damage, the target, the group's pick.
fn checks(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let scale = w.class(CLASS).map_or(1.0, |c| c.scale) * 1.5;
    w.mobys[id].scale = scale;
    if ai::take_damage(w, id, ctx, false) != 0 {
        let m = &mut w.mobys[id];
        let h = ai::health(m);
        let fence = if 0.0 < m.pv_f(h) { m.pv_i(MV + 0x3c) } else { -1 };
        m.set_pv_i(0x18c, fence);
        alarm_group(w, id);
    }
    let area = w.mobys[id].pv_i(0x3f8);
    ai::find_target(w, id, ctx, 0.0, 0.0, area, None, false);
    if let Some(o) = usize::try_from(w.mobys[id].pv_i(0x408)).ok().filter(|&o| w.mobys.get(o).is_some_and(|x| x.state != 0xfe && x.state != 0xfd)) {
        ai::set_link(&mut w.mobys[id], T_MOBY, Some(o));
    }
    let m = &w.mobys[id];
    if 3 <= m.sub_state && (m.pv_i(T_FOUND) == 1 || m.pv_i(0x408) != -1) && m.pv_i(MV + 0x3c) != -1 {
        let tp = target(w, id).map(|t| t.1);
        let m = &mut w.mobys[id];
        if m.pv_i(0x41c) != -1 { m.set_pv_i(0x3f8, m.pv_i(0x41c)); }
        if m.pv_i(0x3c0) != -1 { m.set_pv_i(MV + 0x3c, m.pv_i(0x3c0)); }
        if let Some(p) = tp { m.set_pv_v(0x390, p); }
    }
    // The group's pick.
    let m = &w.mobys[id];
    let (g, at, pick, frame) = (m.group, m.pv_i(0x3fc), m.pv_i(0x400), ctx.game.frame);
    if g != -1 && w.shared_i(at) != frame && !(w.shared_i(pick) != -1 && frame - 10 <= w.shared_i(at)) {
        w.set_shared_i(pick, -1);
        let mut best = 100_000.0;
        let hero = ctx.hero.pos;
        for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() {
            let o = &w.mobys[i];
            if o.o_class != CLASS || o.sub_state != 3 || o.state == 4 || o.state >= 0x80 { continue; }
            let d = dist2(o.pos, hero);
            if d < best {
                let uid = o.uid as i32;
                w.set_shared_i(pick, uid);
                best = d;
            }
        }
        w.set_shared_i(at, frame);
    }
}

/// `0x3deb50`: the timers at the end of the tick.
fn tail(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    ai::flash(m, 0x100);
    if m.state_ticks != -1 { m.state_ticks = m.state_ticks.wrapping_add(1); }
    tick_down16(w, id, 0x3d8);
    tick_down(w, id, 0x404);
    tick_down16(w, id, 0x3d6);
    let m = &mut w.mobys[id];
    if m.pv_s(0x3da) != 0 {
        m.set_pv_s(0x3da, 0);
        m.set_pv_s(0x3d6, 0x168);
    }
}

/// An idle (sequence 4 or 5) with a blend of 12, then state 1 (`0x3dcdc0`).
fn rest(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, base: u8) {
    let now = seq_b(w, id, ctx);
    let a = (ctx.game.rng.rand() & 1) as u8 + base;
    if now != a {
        let s = (ctx.game.rng.rand() & 1) as u8 + base;
        play(w, id, ctx, s, 0, 0xc);
    }
    w.mobys[id].set_state(1, 3);
}

/// Sequence 7 and the close-in (`0x3dcb14`).
fn close_in(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if seq_b(w, id, ctx) != 7 {
        let b = ctx.game.rng.rand_range(7, 0xc);
        play(w, id, ctx, 7, 0, b);
    }
    w.mobys[id].set_state(3, -1);
}

/// The spit's start (`0x3dc930`): its wait 180, the stream's reach 0.3, sequence 8.
fn spit_start(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    m.set_pv_i(0x404, 0xb4);
    m.set_pv_f(0x3f4, 0.3);
    m.set_pv_i(0x3f0, 0);
    if seq_b(w, id, ctx) != 8 {
        let b = ctx.game.rng.rand_range(4, 7);
        play(w, id, ctx, 8, 0, b);
    }
    w.mobys[id].set_state(0, 4);
}

/// `0x311f48(yaw, accel, brake, most, m, &v)`: the facing springs toward `yaw`.
fn spring_yaw(w: &mut MobyWorld, id: usize, yaw: f32, a: f32, b: f32, most: f32) {
    let m = &mut w.mobys[id];
    let e = sub_rot(yaw, m.rot[2]);
    let mut v = m.pv_f(0x274);
    v += a * (e * 6.366_197).clamp(-1.0, 1.0) - b * v;
    if most != 0.0 { v = v.clamp(-most, most); }
    v = v.clamp(-e.abs(), e.abs());
    m.set_pv_f(0x274, v);
    m.rot[2] = add_rot(m.rot[2], v);
}

/// `0x3dd520(m, no_spray)`: the spit's hits (module docs). The stream: `+0x3f0` its stood-in particle's age + 1 (0
/// none), `+0x3f4` its reach.
fn spit(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, spray: bool) {
    let Some((t, tp)) = target(w, id) else { return };
    let m = &w.mobys[id];
    let pitch_to = atan2(dist2(m.pos, tp), tp[2] - m.pos[2]);
    let pitch = add_rot(sub_rot(pitch_to, m.rot[1]).clamp(-0.349_065_9, 0.349_065_9), m.rot[1]);
    let yaw = m.rot[2];
    let Some(j) = ctx.host.joint(m, 0) else { return };
    let mouth = add(add(j, scale(m.rows[0], -0.4)), scale(m.rows[2], 0.3));
    let damage = m.pv_b(0x34) as f32;
    if w.mobys[t].o_class == 0x0b1a {
        ai::attack_sphere(w, id, ctx, 1.0, damage, 1.0, mouth, 0x400_0001, 0, 1, 0);
        return;
    }
    let age = w.mobys[id].pv_i(0x3f0);
    if age != 0 {
        if age <= 30 {
            let m = &mut w.mobys[id];
            let reach = m.pv_f(0x3f4).max(0.15 * age as f32);
            m.set_pv_f(0x3f4, reach);
            m.set_pv_i(0x3f0, age + 1);
        } else {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x3f0, 0);
            m.set_pv_f(0x3f4, 3.0);
        }
        let reach = w.mobys[id].pv_f(0x3f4);
        // `0x325100(reach, yaw, pitch)`.
        let tip = add(mouth, [yaw.cos() * reach * pitch.cos(), yaw.sin() * reach * pitch.cos(), pitch.sin() * reach]);
        line_hit(w, id, ctx, damage, mouth, tip);
        ai::attack_sphere(w, id, ctx, 0.3, damage, 1.0, tip, 0x400_0001, 0, 1, 0);
    }
    if spray && w.mobys[id].pv_i(0x3f0) == 0 { w.mobys[id].set_pv_i(0x3f0, 1); }
}

/// `0x3132c8(damage, push, m, from, to, flags, …)`: a hit along the line from `from` to `to`, to the first moby it
/// meets (`0x2d3fe8` = L00 `0x2de878` with the template).
fn line_hit(w: &mut MobyWorld, id: usize, ctx: &Ctx, damage: f32, from: V3, to: V3) {
    let m = &w.mobys[id];
    let d = if m.mode & crate::moby::mode::KEEP_ROWS == 0 { [m.rot[2].cos(), m.rot[2].sin(), 0.0] } else { m.rows[0] };
    let mut t = crate::moby::hits::HitTemplate::new(damage, Some(id), 0x400_0001, [d[0], d[1], 1.0, 5_627.987]);
    t.kind = (m.o_class as u16 as u32) << 16 | 0x100;
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, from, to, crate::collision::QueryFlags(9), Some(id)));
    if let Some(o) = hit {
        if let Some(target) = o.moby { w.deliver_hit_any(target, &t, [o.point[0], o.point[1], o.point[2], 1.0]); }
    }
}

/// 0x0b2b (module docs).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].sub_state != 0 { checks(w, id, ctx); }
    match w.mobys[id].sub_state {
        0 => setup(w, id, ctx),
        1 => waiting(w, id, ctx),
        2 => flight(w, id, ctx),
        3 => bank(w, id, ctx),
        4 => spitting(w, id, ctx),
        5 => {
            let res = ai::flight(w, id, ctx);
            if res & 1 != 0 {
                w.mobys[id].set_state(0, 6);
            } else if res & 2 != 0 {
                let m = &mut w.mobys[id];
                m.set_pv_i(0x410, -1);
                if m.pv_i(0x3c0) != -1 { m.set_pv_i(MV + 0x3c, m.pv_i(0x3c0)); }
                if m.pv_i(0x41c) != -1 { m.set_pv_i(0x3f8, m.pv_i(0x41c)); }
                m.set_pv_s(0x3d6, 600);
                m.set_pv_s(0x3d8, 0x1e);
                rest(w, id, ctx, 4);
            }
        }
        6 => {
            crate::moby::bolts::drop(w, id, 0, 3, ctx);
            w.delete(id);
            return;
        }
        _ => {}
    }
    let sub = w.mobys[id].sub_state;
    if sub != 5 && 2 < sub { tilt_to_floor(w, id, ctx, MV, 0x3ec, 0x3e8); }
    tail(w, id);
}

/// Sub-state 1: waiting (hidden or not) for Ratchet in its area.
fn waiting(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let under = w.mobys[id].pv_b(0x3d0) != 0;
    if under { hide(w, id, true); }
    if w.mobys[id].pv_i(T_FOUND) != 1 || !tick_down16(w, id, 0x3d4) { return; }
    if !under {
        let m = &mut w.mobys[id];
        if m.pv_i(0x41c) != -1 { m.set_pv_i(0x3f8, m.pv_i(0x41c)); }
        close_in(w, id, ctx);
        w.mobys[id].set_state(3, 3);
        return;
    }
    // `0x3de318`: the farthest of its cuboids from Ratchet, on the ground there.
    let hero = ctx.hero.pos;
    let mut best: Option<(V3, f32)> = None;
    for k in 0..3 {
        if let Some(c) = cuboid_at(ctx, w.mobys[id].pv_i(0x3c4 + 4 * k)) {
            let d = dist2(c, hero);
            if best.is_none_or(|b| b.1 < d) { best = Some((c, d)); }
        }
    }
    let Some((mut to, _)) = best else { return };
    to[2] = ai::ground_z(ctx, 10.0, to, 0);
    let from = w.mobys[id].pos;
    let d = sub(to, from);
    let speed = (len(d) * 0.033_333_335).min(0.133_333_34);
    let across = set_len(d, speed);
    let t = len2(d) / speed;
    let vz = if t != 0.0 { -((from[2] - to[2]) + -0.005 * t * t * 0.5) / t } else { 0.0 };
    let m = &mut w.mobys[id];
    m.set_pv_v(0x190, [across[0], across[1], vz]);
    m.set_pv_v(0x390, to);
    if seq_b(w, id, ctx) != 0xc { play(w, id, ctx, 0xc, 0, 2); }
    if 0.0 < t { ctx.host.set_anim_speed(&w.mobys[id], 51.0 / t); }
    hide(w, id, false);
    w.mobys[id].set_state(0, 2);
}

/// Sub-state 2: the leap's flight and landing.
fn flight(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    m.pos = add(m.pos, m.pv_v(0x190));
    let aim = m.pv_v(0x2e0);
    let yaw = atan2(aim[0] - m.pos[0], aim[1] - m.pos[1]);
    let (a, b, most) = (m.pv_f(MV + 0x14), m.pv_f(MV + 0x24), m.pv_f(MV + 0x1c) * 2.0);
    spring_yaw(w, id, yaw, a, b, most);
    let m = &mut w.mobys[id];
    let vz = m.pv_f(0x198) - 0.005;
    m.set_pv_f(0x198, vz);
    let land = m.pv_v(0x390);
    if 0.0 < vz || -vz <= (m.pos[2] - land[2]).abs() { return; }
    ctx.host.set_anim_speed(&w.mobys[id], 1.0);
    let m = &mut w.mobys[id];
    m.pos[2] = land[2];
    m.set_pv_f(0x198, 0.0);
    if dist2(m.pos, land) < len(m.pv_v(0x190)) {
        m.pos = land;
        m.set_pv_v(0x190, [0.0; 3]);
    }
    if !wrapped(w, id, ctx) { return; }
    let m = &mut w.mobys[id];
    let pos = m.pos;
    m.set_pv_v(WANDER, pos);
    if seq_b(w, id, ctx) != 6 { play(w, id, ctx, 6, 0, 10); }
    ctx.host.set_anim_speed(&w.mobys[id], 1.0);
    let t = ctx.game.rng.randf_signed(20.0, 45.0) * 0.017_453_294;
    let m = &mut w.mobys[id];
    m.set_pv_f(0x3dc, t);
    if m.pv_i(0x41c) != -1 { m.set_pv_i(0x3f8, m.pv_i(0x41c)); }
    if m.pv_b(0x3d3) == 0 {
        if m.pv_i(0x3c0) != -1 { m.set_pv_i(MV + 0x3c, m.pv_i(0x3c0)); }
        m.set_state(3, 3);
    } else {
        m.set_state(2, 3);
    }
}

/// Sub-state 3: on the bank.
fn bank(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        1 => {
            if !wrapped(w, id, ctx) || w.mobys[id].pv_s(0x3d8) != 0 { return; }
            let t = ctx.game.rng.rand_range(0xb4, 0x1b8);
            w.mobys[id].set_pv_s(0x3d8, t as i16);
            if seq_b(w, id, ctx) != 6 {
                let b = ctx.game.rng.rand_range(7, 0xc);
                play(w, id, ctx, 6, 7, b);
            }
            w.mobys[id].set_state(2, -1);
        }
        2 => {
            roam(w, id, ctx);
            if ctx.host.anim(&w.mobys[id]).is_some_and(|a| a.frame_a != 0) { return; }
            let m = &w.mobys[id];
            // Seeing him, or alarmed (`+0x3d6`): with a target moby it closes in; else when its group picked it and
            // its wait is over it turns to him.
            let linked = m.pv_i(0x408) != -1;
            let aware = m.pv_i(T_FOUND) == 1 || m.pv_s(0x3d6) != 0;
            let picked = m.group != -1 && w.shared_i(m.pv_i(0x400)) == m.uid as i32;
            let go = aware && (linked || (picked && m.pv_i(0x404) == 0));
            if go {
                let m = &mut w.mobys[id];
                if m.pv_i(0x3c0) != -1 { m.set_pv_i(MV + 0x3c, m.pv_i(0x3c0)); }
                m.set_pv_s(0x3d6, 0x168);
                if linked {
                    close_in(w, id, ctx);
                } else {
                    let now = seq_b(w, id, ctx);
                    let a = (ctx.game.rng.rand() & 1) as u8 + 2;
                    if now != a {
                        let s = (ctx.game.rng.rand() & 1) as u8 + 2;
                        let b = ctx.game.rng.rand_range(7, 0xc);
                        play(w, id, ctx, s, 0, b);
                    }
                    w.mobys[id].set_state(5, -1);
                }
                return;
            }
            let m = &w.mobys[id];
            if m.pv_s(0x3d8) != 0 { return; }
            if m.pv_s(0x3d6) != 0 || linked {
                if !linked { rest(w, id, ctx, 4); }
                return;
            }
            w.mobys[id].set_pv_s(0x3d8, 0x78);
            let now = seq_b(w, id, ctx);
            let a = (ctx.game.rng.rand() & 1) as u8;
            if now != a {
                let s = (ctx.game.rng.rand() & 1) as u8;
                play(w, id, ctx, s, 0, 0xc);
            }
            w.mobys[id].set_state(1, -1);
        }
        3 => {
            let goal = w.mobys[id].pv_v(0x390);
            go_to(w, id, ctx, goal);
            if !wrapped(w, id, ctx) { return; }
            let m = &w.mobys[id];
            let d = dist2(m.pos, goal);
            let at_nest = target(w, id).is_some_and(|(t, _)| w.mobys[t].o_class == 0x0b1a);
            if (at_nest && d < 1.0) || (!at_nest && d <= 3.0) {
                spit_start(w, id, ctx);
                return;
            }
            let m = &w.mobys[id];
            if m.pv_i(T_FOUND) == 1 || m.pv_s(0x3d6) != 0 || m.pv_i(0x408) != -1 { return; }
            if ctx.game.rng.randi(0x27) != 0 { return; }
            if ctx.game.rng.randi(3) == 0 {
                spit_start(w, id, ctx);
                return;
            }
            if seq_b(w, id, ctx) != 6 {
                let b = ctx.game.rng.rand_range(0xc, 0x12);
                play(w, id, ctx, 6, 0, b);
            }
            w.mobys[id].set_state(4, 3);
        }
        4 => {
            roam(w, id, ctx);
            if !wrapped(w, id, ctx) { return; }
            let m = &w.mobys[id];
            if m.pv_i(0x404) != 0 && dist2(m.pos, ctx.hero.pos) <= 7.0 { return; }
            w.mobys[id].set_pv_s(0x3d8, 0x3c);
            rest(w, id, ctx, 4);
        }
        5 => {
            let m = &w.mobys[id];
            let aim = m.pv_v(0x2e0);
            let yaw = atan2(aim[0] - m.pos[0], aim[1] - m.pos[1]);
            let (a, b, most) = (m.pv_f(MV + 0x14) * 2.0, m.pv_f(MV + 0x24), m.pv_f(MV + 0x1c) * 2.0);
            spring_yaw(w, id, yaw, a, b, most);
            if wrapped(w, id, ctx) { close_in(w, id, ctx); }
        }
        _ => {}
    }
}

/// Sub-state 4: the spit.
fn spitting(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let t = ctx.host.frame_time(&w.mobys[id]);
    if let Some((_, tp)) = target(w, id) {
        let m = &mut w.mobys[id];
        let face = atan2(tp[0] - m.pos[0], tp[1] - m.pos[1]);
        let (a, b, most) = (m.pv_f(MV + 0x14), m.pv_f(MV + 0x18), m.pv_f(MV + 0x1c));
        let (mut yaw, mut v) = (m.rot[2], m.pv_f(0x274));
        turn_ease(face, a, b, most, &mut yaw, &mut v, 0);
        m.rot[2] = yaw;
        m.set_pv_f(0x274, v);
    }
    let sa = seq_a(w, id, ctx);
    if sa == 8 && (18.0..=23.0).contains(&t) {
        ctx.host.set_anim_speed(&w.mobys[id], 0.4);
        let damage = w.mobys[id].pv_b(0x34) as f32;
        ai::attack_joint(w, id, ctx, 0.2, damage, 1.0, 0, 0x400_0001, 0, 1, 0);
        spit(w, id, ctx, true);
    } else {
        ctx.host.set_anim_speed(&w.mobys[id], 1.0);
        spit(w, id, ctx, false);
    }
    if !wrapped(w, id, ctx) { return; }
    if seq_b(w, id, ctx) != 6 {
        let b = ctx.game.rng.rand_range(0xc, 0x12);
        play(w, id, ctx, 6, 0, b);
    }
    w.mobys[id].set_state(4, 3);
}
