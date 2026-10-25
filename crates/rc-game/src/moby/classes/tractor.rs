//! The Tractor Beam, class 0xbc (L01 `0x38d4f0`, item 0x2e, crate::hero::items::ROWS; sold on Oozla), and the boxes it
//! drags, 0x0acb / 0x0acc (`0x3d40e8` with the shared drag `0x31bd40`).
//!
//! **The beam** (`0x38d4f0`): set up (0) it waits 16 ticks (1, `+0x2c`); then with a target held (`0x18a3e0`) it is
//! in use (3: the use stance, `0x2b0198` / `0x2b0400`) until the target or the button goes. While it holds, its start
//! `+0x00` is its place 0.7 up, its end `+0x10` its joint 0 out along Ratchet's facing by the reach `+0x24` (0.8 at
//! the press, easing to 12 by 0.07 a tick), and it draws ([`draw`], `0x38e4c8`).
//!
//! **The pick**: the game's writer of `0x18a3e0` is not found (only two reads in the level's code; the PS2 shows it
//! set while ○ holds a box of the target list `0x1fb3e0`, cleared at the release). The port's: at the press, the
//! nearest box in the target list within its range ahead of Ratchet (±60°) with a clear line; kept while ○ is held.
//!
//! **A box** (`0x3d40e8`; `+0x50` its drag block, `0x31bd40`): every 16 ticks a target (mode 0x1000) while Ratchet
//! is within `+0x88`. Held, a line to Ratchet that meets anything lets it go; it is pulled toward the point `+0x84`
//! (4) from Ratchet halfway between its side and his facing, at `dist / 15` a tick (at most 2/15, flag 4: `dist·8/15`,
//! 4/15) with the change capped at 1/180 a tick (flag 4: 1/72); let go, its speed falls by 1/360 a tick. Moving, it
//! falls (1/180 a tick², onto the ground a line from 0.25 over to 1 under finds; not with flag 2), keeps out of
//! Ratchet's way (within its radius + 2.5, not with flag 4), moves, is pushed out of walls by eight lines at 45° steps
//! (`0x31c520`), hits what is ahead (`0x31d180`, flags 0x4000000) and stays in its path `+0xe0` (`0x343678`). Its
//! radius / heights are `+0xe4` / `+0xe8` / `+0xec` × its scale over its class's. It carries Ratchet.
//!
//! Not ported: the beam's rope (`0x38dd68`: 20 points, springy and waving; here a straight line with a slight wave),
//! its sparks (`0x38ef20`, `0x3303e8`), its light (`0x2f33f8`); Clank's and the strafe's hold points; a box's
//! cuboid `+0x80` (none on Oozla), its cylinder test (`0x2d5d38`), its glow pulse (`+0x78`).
//!
//! **Sounds**: the beam's class sound 1 when it is taken out, 2 at a press with nothing held, 0 looped while it holds
//! (`+0x40`); a box's 0 looped while it moves (`+0x90`).

use crate::hero::physics::{add, dist, dot, len, scale, set_len, sub, V3};
use crate::moby::{carry_hero, mode, Ctx, MobyWorld};

/// The boxes' classes.
const BOXES: [i16; 2] = [0x0acb, 0x0acc];
/// The beam's pvar holding its target + 1 (the port's `0x18a3e0`).
const TARGET: usize = 0x60;

/// The beam in Ratchet's hand.
fn beam(w: &MobyWorld) -> Option<usize> {
    w.mobys.iter().position(|m| m.o_class == crate::hero::items::TRACTOR_CLASS && m.state < 0x80)
}

/// `0x18a3e0`: the box held.
pub fn held(w: &MobyWorld) -> Option<usize> {
    beam(w).and_then(|b| usize::try_from(w.mobys[b].pv_i(TARGET) - 1).ok())
}

fn release(w: &mut MobyWorld) {
    if let Some(b) = beam(w) { w.mobys[b].set_pv_i(TARGET, 0); }
}

/// The port's pick (see the module).
fn pick(w: &MobyWorld, ctx: &Ctx) -> Option<usize> {
    let hero = ctx.hero.pos;
    let fwd = ctx.hero.rows[0];
    let mut best: Option<(f32, usize)> = None;
    for (i, m) in w.mobys.iter().enumerate() {
        if !BOXES.contains(&m.o_class) || m.state >= 0x80 || m.mode & mode::TARGET == 0 { continue; }
        let mut d = sub(m.pos, hero);
        d[2] = 0.0;
        let l = len(d);
        if m.pv_f(0x88) < l || (0.1 < l && dot(d, fwd) < 0.5 * l) { continue; }
        let from = [hero[0], hero[1], hero[2] + 1.0];
        if w.coll_line(ctx, from, [m.pos[0], m.pos[1], m.pos[2] + 0.5], 2, Some(i)).is_some_and(|o| o.moby != ctx.hero_moby) { continue; }
        if best.is_none_or(|(b, _)| l < b) { best = Some((l, i)); }
    }
    best.map(|(_, i)| i)
}

/// 0xbc (`0x38d4f0`).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let it = ctx.hero.items;
    let pressed = it.pressed & it.trigger != 0 && it.no_use == 0;
    let down = it.held & it.trigger != 0;
    if pressed { w.mobys[id].set_pv_f(0x24, 0.8); }
    // Out of its unfold (sequence 0) it holds sequence 1, as the live one does (the Dynamo's way).
    if let Some(a) = ctx.host.anim(&w.mobys[id]) {
        if a.seq_b == 0 && a.seq_a == a.seq_b && a.flags & 2 != 0 { ctx.host.set_anim(&w.mobys[id], 1, 0, 2); }
    }
    if ctx.hero.state == 1 { ctx.hero.request_state(0x1d, true); }
    // The target: kept while ○ is held, picked at the press.
    let mut target = usize::try_from(w.mobys[id].pv_i(TARGET) - 1).ok().filter(|&t| w.mobys.get(t).is_some_and(|m| m.state < 0x80));
    if !down || it.no_use != 0 {
        target = None;
    } else if target.is_none() && pressed {
        target = pick(w, ctx);
    }
    w.mobys[id].set_pv_i(TARGET, target.map_or(0, |t| t as i32 + 1));
    let state = w.mobys[id].state;
    let holding = target.is_some() && state != 0 && state != 4 && ctx.hero.state != 0x6f;
    if holding && w.mobys[id].pv_i(0x2c) == 0 {
        let muzzle = ctx.host.joint(&w.mobys[id], 0).unwrap_or(w.mobys[id].pos);
        let m = &mut w.mobys[id];
        let reach = m.pv_f(0x24) + (12.0 - m.pv_f(0x24)) * 0.07;
        m.set_pv_f(0x24, reach);
        let p = m.pos;
        m.set_pv_v(0, [p[0], p[1], p[2] + 0.7]);
        m.set_pv_v(0x10, add(muzzle, set_len(ctx.hero.rows[0], reach)));
        m.set_pv_v(0x30, muzzle);
        w.register_draw(id);
    }
    let m = &mut w.mobys[id];
    match state {
        0 => {
            m.set_pv_i(0x2c, 16);
            m.set_pv_i(0x40, -1);
            m.set_state(1, -1);
            ctx.host.play_sound(&w.mobys[id], 1, 0);
        }
        1 => {
            let h = m.pv_i(0x40);
            if h != -1 { ctx.host.stop_sound(h); }
            let m = &mut w.mobys[id];
            m.set_pv_i(0x40, -1);
            if pressed && !holding { ctx.host.play_sound(&w.mobys[id], 2, 0); }
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x2c);
            crate::moby::countdown(&mut t);
            m.set_pv_i(0x2c, t);
            if holding {
                m.set_state(3, -1);
                ctx.hero.items.use_start(ctx.hero.group);
            }
        }
        3 => {
            let h = m.pv_i(0x40);
            if h == -1 || !ctx.host.sound_playing(h) {
                let h = ctx.host.play_sound(&w.mobys[id], 0, 4);
                w.mobys[id].set_pv_i(0x40, h);
            }
            let m = &mut w.mobys[id];
            if !holding {
                m.set_pv_f(0x24, 0.0);
                m.set_state(1, -1);
                let idle = ctx.hero.idle_seq();
                let st = ctx.hero.state;
                ctx.hero.items.use_end(st, ctx.hero_view.seq_b, idle);
            }
        }
        _ => {}
    }
}

/// `0x38e4c8` (registered while it holds): two ribbons along the beam from its joint to the box (FX 0xe ±0.05 in
/// `0x80` `0x12cdef`, FX 0x10 ±0.4 in `0x40` `0x1232ef`, additive; grey at the ends), scrolling 0.3 a frame.
pub fn draw(w: &MobyWorld, id: usize, cam: V3, phase: f32, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    let a = m.pv_v(0x30);
    let b = match held(w) {
        Some(t) => { let p = w.mobys[t].pos; [p[0], p[1], p[2] + 0.5] }
        None => m.pv_v(0x10),
    };
    const N: usize = 20;
    let along = sub(b, a);
    let up = [0.0, 0.0, 1.0];
    let pts: Vec<V3> = (0..N)
        .map(|i| {
            let t = i as f32 / (N - 1) as f32;
            let wave = (t * std::f32::consts::PI).sin() * 0.15 * (phase * 2.0 + t * 9.0).sin();
            add(add(a, scale(along, t)), scale(up, wave))
        })
        .collect();
    for (fx, half, rgb, alpha) in [(0xe, 0.05, 0x0012_cdef, 0x80u32), (0x10, 0.4, 0x0012_32ef, 0x40)] {
        for i in 0..N - 1 {
            let (p, q) = (pts[i], pts[i + 1]);
            let seg = sub(q, p);
            let side = set_len(crate::hero::physics::cross(seg, sub(cam, p)), half);
            let c = |k: usize| if k == 0 || k == N - 1 { 0x7f7f7f | alpha << 24 } else { rgb | alpha << 24 };
            let v0 = phase + i as f32 * 0.5;
            let st = [[0.0, v0], [1.0, v0], [0.0, v0 + 0.5], [1.0, v0 + 0.5]];
            out.push(crate::fx::FxQuad { fx, additive: true, corners: [add(p, side), sub(p, side), add(q, side), sub(q, side)], st, rgba: [c(i), c(i), c(i + 1), c(i + 1)] });
        }
    }
}

/// `0x31c520(m, v, from, to)`: a line meeting a wall (not Ratchet, not class 0x881) pushes the box back out by how
/// far `to` went past it along the wall's normal (its height kept).
fn push_out(w: &mut MobyWorld, id: usize, ctx: &Ctx, from: V3, to: V3) {
    let Some(o) = w.coll_line(ctx, from, to, 0x10, Some(id)) else { return };
    if let Some(m) = o.moby {
        if Some(m) == ctx.hero_moby || w.mobys.get(m).is_some_and(|x| x.o_class == 0x881 || x.o_class == 0) { return; }
    }
    let n = set_len(o.normal, 1.0);
    let d = dot(sub(to, o.point), n).abs();
    let m = &mut w.mobys[id];
    let z = m.pos[2];
    m.pos = add(m.pos, scale(n, d));
    m.pos[2] = z;
}

/// `0x31bd40(height, r, h2, m, block, path)`: the drag (see the module).
fn drag(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, r: f32, path: i32) {
    let hero = ctx.hero.pos;
    let m = &mut w.mobys[id];
    if (ctx.game.frame & 0xf) as usize == id & 0xf {
        let near = dist(hero, m.pos) < m.pv_f(0x88);
        if near { m.mode |= mode::TARGET } else { m.mode &= !mode::TARGET }
    }
    let start = w.mobys[id].pos;
    let mut v = w.mobys[id].pv_v(0x50);
    let flags = w.mobys[id].pv_i(0x70);
    if held(w) == Some(id) {
        let p = w.mobys[id].pos;
        if w.coll_line(ctx, [p[0], p[1], p[2] + 0.5], [hero[0], hero[1], hero[2] + 1.0], 2, Some(id)).is_some_and(|o| o.moby != ctx.hero_moby) {
            release(w);
        }
        let d = match w.mobys[id].pv_f(0x84) { x if x != 0.0 => x, _ => 4.0 };
        let a = add(hero, set_len(sub(p, hero), d));
        let mut b = add(hero, set_len(ctx.hero.rows[0], d));
        b[2] = a[2];
        let mid = add(a, scale(sub(b, a), 0.5));
        let goal = add(hero, set_len(sub(mid, hero), d));
        let (k, cap, acc) = if flags & 4 == 0 { (dist(p, goal) * 0.066_666_67, 0.133_333_34, 0.005_555_556) } else { (dist(p, goal) * 0.533_333_36, 0.266_666_68, 0.013_888_889) };
        let mut f = sub(goal, p);
        f[2] = 0.0;
        let want = set_len(f, k.min(cap));
        let mut dv = sub(want, v);
        if acc < len(dv) { dv = set_len(dv, acc); }
        v[0] += dv[0];
        v[1] += dv[1];
    } else {
        let l = len(v);
        let l2 = (l - 0.002_777_778).max(0.0);
        v = if l == 0.0 { v } else { scale(v, l2 / l) };
    }
    if 0.001 < len(v) {
        let p = w.mobys[id].pos;
        if flags & 2 == 0 {
            let floor = w.coll_line(ctx, [p[0], p[1], p[2] + 0.25], [p[0], p[1], p[2] - 1.0], 0x10, Some(id)).map_or(0.0, |o| o.point[2]);
            if p[2] < floor {
                w.mobys[id].pos[2] = floor;
                v[2] = 0.0;
            } else {
                v[2] -= 0.005_555_556;
            }
        }
        let p = w.mobys[id].pos;
        if flags & 4 == 0 && dist(p, hero) < r + 2.5 {
            let mut to = sub(hero, p);
            to[2] = 0.0;
            let to = set_len(to, 1.0);
            let c = dot(to, v);
            if 0.0 < c { v = sub(v, scale(to, c)); }
        }
        w.mobys[id].pos = add(w.mobys[id].pos, v);
        let p = w.mobys[id].pos;
        let from = [p[0], p[1], p[2] + 0.25];
        let mut probe = set_len(v, r);
        let (s, c) = std::f32::consts::FRAC_PI_4.sin_cos();
        for _ in 0..8 {
            probe = [probe[0] * c - probe[1] * s, probe[0] * s + probe[1] * c, probe[2]];
            let p = w.mobys[id].pos;
            let mut to = add(p, probe);
            to[2] += 0.25;
            push_out(w, id, ctx, from, to);
        }
        let p = w.mobys[id].pos;
        let ahead = add(add(p, scale(v, 5.0)), [0.0, 0.0, r]);
        crate::moby::ai::attack_sphere(w, id, ctx, r + 0.2, 1.0, 1.0, ahead, 0x400_0000, 0, 1, 0);
        if let Some(points) = usize::try_from(path).ok().and_then(|i| ctx.level.paths.get(i)) {
            if let Some(q) = crate::moby::ai::fence(points, r, w.mobys[id].pos) { w.mobys[id].pos = q; }
        }
        let moved = sub(w.mobys[id].pos, start);
        if dot(moved, moved) < dot(v, v) {
            v[0] = moved[0];
            v[1] = moved[1];
        }
    }
    // Its slide's sound (`+0x90`): looped while it moves.
    let h = w.mobys[id].pv_i(0x90);
    if [v[0], v[1]] != [0.0, 0.0] && 0.001 < len(v) {
        if h == -1 || !ctx.host.sound_playing(h) {
            let h = ctx.host.play_sound(&w.mobys[id], 0, 4);
            w.mobys[id].set_pv_i(0x90, h);
        }
    } else if h != -1 {
        ctx.host.stop_sound(h);
        w.mobys[id].set_pv_i(0x90, -1);
    }
    w.mobys[id].set_pv_v(0x50, v);
}

/// 0x0acb / 0x0acc (`0x3d40e8`): see the module.
pub fn update_box(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class_scale = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let (old, yaw) = (w.mobys[id].pos, w.mobys[id].rot[2]);
    let m = &mut w.mobys[id];
    match m.state {
        0 => {
            m.mode |= mode::TARGET;
            m.set_pv_i(0xdc, m.pv_i(0xdc) | 1);
            m.set_pv_i(0x90, -1);
            m.set_state(1, -1);
            if m.o_class == 0x0e2a { m.set_pv_f(0x88, 12.0); }
        }
        1 => {
            let s = if class_scale != 0.0 { m.scale / class_scale } else { 1.0 };
            let (r, path) = (m.pv_f(0xe4) * s, m.pv_i(0xe0));
            drag(w, id, ctx, r, path);
        }
        _ => {}
    }
    w.rebuild(id, &*ctx.host);
    let p = w.mobys[id].pos;
    carry_hero(ctx, id, old, yaw, p, yaw);
}
