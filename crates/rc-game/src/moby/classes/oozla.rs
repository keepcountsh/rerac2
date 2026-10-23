//! Oozla's small objects (L01 class updates; no names in any reference, so they go by o_class).
//!
//! * 0x08fb (`0x41dee0`): sets mode 0x802 (no update; 0x800) and nothing else.
//! * 0x0f14 (`0x4108b0`): on its first tick state 1, update distance 0xff.
//! * 0x0a6a / 0x0a6b / 0x0aa0 / 0x0aa1 (`0x3ca620`): movers between two poses ([`update_mover`]).
//! * 0x0c58 (`0x3ed598`): moves once to a cuboid's place when poked ([`update_0c58`]).
//! * 0x0a17 (`0x3bcdd8`): the swamp's water level ([`update_0a17`]).
//! * 0x0a92 / 0x0a93 (`0x3cfcc0`): float on that water ([`update_float`]).
//!
//! Not ported here: 0x0ab3 (`0x3d4098`, deleted once `0x1a7b2e` is set: a byte of the `0x1a7b00` table whose
//! `0x1a7b13` is the Gravity Boots, so probably an item had), 0x0e89 (`0x406860`, the 0x0c58 mover, started also by
//! `0x1a7b13` / `0x1a7b32`), 0x0ca0 (`0x3ee1d0`, the switch that pokes them: the use system, G-UI-001).

use crate::hero::physics::{add, add_rot, dist2, scale, sub, sub_rot, V3};
use crate::moby::attach::rows_of;
use crate::moby::{carry_hero, Ctx, MobyWorld};
use std::f32::consts::PI;

/// 0x08fb.
pub fn update_08fb(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) { w.mobys[id].mode |= 0x802; }

/// 0x0f14.
pub fn update_0f14(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.state = 1;
        m.update_dist = 0xff;
    }
}

/// `0x314a20(a, b, t)`: the angle `t` of the way from `a` to `b` (the short way round).
fn lerp_rot(a: f32, b: f32, t: f32) -> f32 { add_rot(a, sub_rot(b, a) * t) }

/// `(1 − cos(p·π)) / 2`: the movers' ease of their progress `p` (0..1).
fn ease(p: f32) -> f32 { (1.0 - (p * PI).cos()) * 0.5 }

/// `0x2ef318`: a frame vector through the rows.
fn through(rows: &[V3; 3], v: V3) -> V3 { std::array::from_fn(|k| v[0] * rows[0][k] + v[1] * rows[1][k] + v[2] * rows[2][k]) }

/// The movers' other moby `+0xb0` (`$gp+0x356c` + index · 0x100): `None` for −1, a free slot (state 0xfe) or 0xfd.
fn other(w: &MobyWorld, i: i32) -> Option<usize> {
    let i = usize::try_from(i).ok()?;
    w.mobys.get(i).filter(|m| m.state != 0xfe && m.state != 0xfd).map(|_| i)
}

/// 0x0a6a / 0x0a6b / 0x0aa0 / 0x0aa1 (`0x3ca620`): a mover. Its pvars: `+0x20` the carry block, `+0x60` / `+0x70` the
/// home position and angles (kept on the first tick), `+0x80` the offset (in the home frame) and `+0x90` the angles it
/// turns by, `+0xa0` the cuboid, `+0xa4` the ticks a move takes, `+0xa8` stay open, `+0xac` the progress, `+0xb0` a
/// moby that also opens it (−1 none), `+0xb8` silent. Ratchet (or that moby) in the cuboid opens it (state 2, the
/// class sound 0 unless silent), eased over `+0xa4` ticks; open (3) it waits while he (or the moby) is still there,
/// then closes (4) the same way and opens again if he comes back. The game ends the close only at a progress of 1
/// (`c.le.S 1.0, +0xac` at `0x3caab8`), which a close never reaches: a closed mover stays in state 4, which behaves
/// as state 1. It carries Ratchet with it (`0x318560`).
pub fn update_mover(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (old, old_yaw) = (w.mobys[id].pos, w.mobys[id].rot[2]);
    w.mobys[id].state_ticks = w.mobys[id].state_ticks.wrapping_add(1);
    let vol = w.mobys[id].pv_i(0xa0);
    let hero_in = |ctx: &Ctx| ctx.level.in_cuboid(vol, ctx.hero.pos);
    let sound = |w: &MobyWorld, ctx: &mut Ctx| if w.mobys[id].pv_i(0xb8) == 0 { ctx.host.play_sound(&w.mobys[id], 0, 0); };
    // The other moby in the cuboid: Some(true) in, Some(false) out, None no moby (or a free / 0xfd slot).
    let other_in = |w: &MobyWorld, ctx: &Ctx| other(w, w.mobys[id].pv_i(0xb0)).map(|o| ctx.level.in_cuboid(vol, w.mobys[o].pos));
    // States 1 and 4: Ratchet or the other moby in the cuboid opens it.
    let watch = |w: &mut MobyWorld, ctx: &mut Ctx| {
        if hero_in(ctx) {
            sound(w, ctx);
            w.mobys[id].set_state(2, -1);
        }
        if other_in(w, ctx) == Some(true) {
            sound(w, ctx);
            w.mobys[id].set_state(2, -1);
        }
    };
    let step = |w: &mut MobyWorld, d: f32| {
        let m = &mut w.mobys[id];
        let p = (m.pv_f(0xac) + d / m.pv_f(0xa4)).clamp(0.0, 1.0);
        m.set_pv_f(0xac, p);
        let e = ease(p);
        let (home, angles) = (m.pv_v(0x60), m.pv_v(0x70));
        m.pos = add(home, through(&rows_of(angles), scale(m.pv_v(0x80), e)));
        for k in [2, 1, 0] {
            m.rot[k] = lerp_rot(angles[k], add_rot(angles[k], m.pv_f(0x90 + 4 * k)), e);
        }
        p
    };
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.mode |= 0x800;
            let (pos, rot) = (m.pos, m.rot);
            m.set_pv_v(0x60, pos);
            m.set_pv_v(0x70, rot);
            m.set_pv_f(0xac, 0.0);
            m.set_state(1, -1);
        }
        1 => watch(w, ctx),
        2 => {
            if 1.0 <= step(w, 1.0) { w.mobys[id].set_state(3, -1); }
        }
        3 => {
            if w.mobys[id].pv_i(0xa8) == 0 && !hero_in(ctx) {
                let close = match w.mobys[id].pv_i(0xb0) {
                    -1 => true,
                    _ => other_in(w, ctx) == Some(false),
                };
                if close {
                    sound(w, ctx);
                    w.mobys[id].set_state(4, -1);
                }
            }
        }
        4 => {
            if 1.0 <= step(w, -1.0) { w.mobys[id].set_state(1, -1); }
            watch(w, ctx);
        }
        _ => {}
    }
    w.mobys[id].timer = 0;
    if other(w, w.mobys[id].pv_i(0xb0)).is_none() { w.mobys[id].set_pv_i(0xb0, -1); }
    let m = &w.mobys[id];
    if m.state < 5 { carry_hero(ctx, id, old, old_yaw, m.pos, m.rot[2]); }
}

/// 0x0ca0 (`0x3ee1d0`): a pressure plate. Ratchet standing on it (his ground moby `0x18a15c`, grounded) within 0.5
/// across of its joint 0 counts its uid in the level's save bits; with the count (or the uid gone for good) and a
/// mission (`+0xb0` ≠ −1) it is pressed (state 1) and pokes its moby `+0x68`[0] (`+0xbc`: Oozla's ladder, 0x0c58,
/// swings down), so a save keeps it pressed. Its glow pulses (`+0x78`: grey `(4·sin p − 3)·128`, 0x20..0x80; p `[1]`
/// 6°/tick); pressed it glows 0x80208010. Not ported: the sound 0x8a1 when pressed (`0x356a20`).
pub fn update_0ca0(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    let p = add_rot(m.pv_f(4), 0.104_719_76);
    m.set_pv_f(4, p);
    if m.state != 0 {
        m.set_pv_i(8, 1);
        m.glow = 0x8020_8010;
        return;
    }
    m.set_pv_i(8, 0);
    let on = ctx.hero.ground_moby == Some(id) && ctx.hero.air_ticks == 0;
    let near = ctx.host.joint(&w.mobys[id], 0).is_some_and(|j| dist2(ctx.hero.pos, j) < 0.5);    let uid = w.mobys[id].uid;
    if on && near { ctx.game.count_uid(uid); }
    let m = &w.mobys[id];
    let pressed = m.mission != 0xff && (ctx.game.uid_dead.get(uid.max(0) as usize).is_some_and(|&d| d != 0) || ctx.game.uid_count(uid) != 0);
    if pressed {
        let target = usize::try_from(m.pv_i(0)).ok();
        let m = &mut w.mobys[id];
        m.state = 1;
        m.timer = 1;
        if let Some(t) = target.and_then(|t| w.mobys.get_mut(t)) { t.timer = 1; }
    }
    let m = &mut w.mobys[id];
    let v = ((m.pv_f(4).sin() * 4.0 - 3.0) * 128.0) as i32;
    let v = v.clamp(0x20, 0x80) as u32;
    m.glow = 0x8000_0000 | v << 16 | v << 8 | v;
}

/// 0x0c58 (`0x3ed598`): poked (`+0xbc`, by a switch) it plays its looped sound 0 (handle `+0x90`) and moves over
/// `+0x84` ticks (progress `+0x88`, eased as the movers) from its place (kept at `+0x60` / `+0x70` on the first tick)
/// to the cuboid `+0x80`'s place and angles (its `+0x30` / `+0x70`), carrying Ratchet; there the loop stops, the sound
/// 1 plays and it stays (state 3).
pub fn update_0c58(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (old, old_yaw) = (w.mobys[id].pos, w.mobys[id].rot[2]);
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x90, -1);
            m.set_pv_f(0x88, 0.0);
            m.set_pv_i(0x5c, m.pv_i(0x5c) | 1);
            let (pos, rot) = (m.pos, m.rot);
            m.set_pv_v(0x60, pos);
            m.set_pv_v(0x70, rot);
            m.set_state(1, -1);
        }
        1 => {
            if w.mobys[id].timer != 0 {
                let h = ctx.host.play_sound(&w.mobys[id], 0, 4);
                w.mobys[id].set_pv_i(0x90, h);
                w.mobys[id].set_state(2, -1);
            }
            w.mobys[id].hit_slot = -1;
        }
        2 => {
            let Some((to, to_rot)) = ctx.level.cuboid_place(w.mobys[id].pv_i(0x80)) else { return };
            let m = &mut w.mobys[id];
            let p = (m.pv_f(0x88) + 1.0 / m.pv_f(0x84)).min(1.0);
            m.set_pv_f(0x88, p);
            let e = ease(p);
            let (home, angles) = (m.pv_v(0x60), m.pv_v(0x70));
            m.pos = add(home, scale(sub(to, home), e));
            for k in [2, 1, 0] { m.rot[k] = lerp_rot(angles[k], to_rot[k], e); }
            if 1.0 <= p {
                let h = m.pv_i(0x90);
                if h != -1 {
                    ctx.host.stop_sound(h);
                    w.mobys[id].set_pv_i(0x90, -1);
                }
                ctx.host.play_sound(&w.mobys[id], 1, 0);
                w.mobys[id].set_state(3, -1);
            }
        }
        _ => {}
    }
    let m = &w.mobys[id];
    if m.state < 3 { carry_hero(ctx, id, old, old_yaw, m.pos, m.rot[2]); }
}

/// 0x0a17 (`0x3bcdd8`): the swamp's water. Its level (`game.water_base`, L01 `$gp+0xdec`) is its `+0x14`, or with
/// Ratchet in one of its eight cuboids `+0x40..+0x60` that cuboid's centre height + 0.15; with him in one of the two
/// cuboids `+0x30` / `+0x34` (dry ground) it is left as it was. While the camera is above `+0x20` the water is
/// drawn (`0x3bc3c0`, through the first draw-callback list, with its texture scroll: not ported, G-REN-009) and its
/// height (`game.water_z`, `$gp+0xde8`, what `0x3bc3b8` gives the floating objects) waves 0.1 about the level,
/// `sin(frame mod 360 · 0.017444445 − 3.14)`. Not ported: the camera test of the dry cuboids when `0x18c0b4` is 0x1c,
/// and the flag `0x18a01e` (0 in the dry cuboids, else 2).
pub fn update_0a17(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.update_dist = 0xff;
        m.state = 1;
        ctx.game.water_base = m.pv_f(0x14);
        return;
    }
    let hero = ctx.hero.pos;
    if (0..2).any(|k| ctx.level.in_cuboid(m.pv_i(0x30 + 4 * k), hero)) { return; }
    ctx.game.water_base = (0..8)
        .map(|k| m.pv_i(0x40 + 4 * k))
        .find(|&c| ctx.level.in_cuboid(c, hero))
        .and_then(|c| ctx.level.cuboid_place(c))
        .map_or(m.pv_f(0x14), |(centre, _)| centre[2] + 0.15);
    if m.pv_f(0x20) < ctx.host.camera_pos()[2] {
        // The game's own 3.14 (not π).
        #[allow(clippy::approx_constant)]
        let phase = (ctx.game.frame % 360) as f32 * 0.017_444_445 - 3.14;
        ctx.game.water_z = ctx.game.water_base + phase.sin() * 0.1;
    }
}

/// `0x3cfc30(home, gain, damping, radius, &x, &v)`: one axis of a floater's drift about its home, the pull growing
/// with the square of the distance up to `radius`.
fn drift(home: f32, gain: f32, damping: f32, radius: f32, x: &mut f32, v: &mut f32) {
    let r2 = radius * radius;
    *v *= 1.0 - damping;
    let pull = (*x - home).powi(2).min(r2) * gain / r2;
    if home < *x { *v -= pull } else { *v += pull }
    *x += *v;
}

/// 0x0a92 / 0x0a93 (`0x3cfcc0`): float on the water. Once there is water (`game.water_z` above 0) it keeps its
/// place (`+0x00`) and random sway phases (`+0x20` / `+0x24`, the float height `+0x28` = 0.02). Then each tick it
/// sways (0.04, 30 and 40 °/s: `$gp+0x13d0..0x13d8`), drifts about its place across (`+0x10` / `+0x14` the
/// velocities: 10/3600, damping 0.1, within 3), is pushed off by Ratchet within 1.2 across unless he is 0.8 above it
/// (by 0.08 of the overlap), and rises or sinks to the water + 0.02, at most 0.05 a tick.
pub fn update_float(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const STEP: f32 = 0.000_290_888_25;
    let water = ctx.game.water_z;
    match w.mobys[id].state {
        0 => {
            if water <= 0.0 { return; }
            let (a, b) = (ctx.game.rng.rand_angle(), ctx.game.rng.rand_angle());
            let m = &mut w.mobys[id];
            m.set_pv_f(0x20, a);
            m.set_pv_f(0x24, b);
            m.set_pv_f(0x28, 0.02);
            let pos = m.pos;
            m.set_pv_v(0, pos);
            m.set_state(1, -1);
        }
        1 => {
            let m = &mut w.mobys[id];
            super::props::sway(m, 0.0402, 30.0 * STEP, 40.0 * STEP, 0x20, 0x24);
            let gain = 10.0 / 3600.0;
            for k in 0..2 {
                let (mut x, mut v) = (m.pos[k], m.pv_f(0x10 + 4 * k));
                drift(m.pv_f(4 * k), gain, 0.1, 3.0, &mut x, &mut v);
                m.pos[k] = x;
                m.set_pv_f(0x10 + 4 * k, v);
            }
            let hero = ctx.hero.pos;
            let d = dist2(m.pos, hero);
            if d < 1.2 && hero[2] < m.pos[2] + 0.8 {
                let away = sub(m.pos, hero);
                let push = scale([away[0], away[1], 0.0], (1.2 - d) * 0.08);
                m.pos = add(m.pos, push);
            }
            let dz = (water + m.pv_f(0x28) - m.pos[2]).clamp(-0.05, 0.05);
            m.pos[2] += dz;
        }
        _ => {}
    }
}
