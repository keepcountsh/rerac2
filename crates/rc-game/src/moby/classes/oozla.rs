//! Oozla's small objects (L01 class updates; no names in any reference, so they go by o_class).
//!
//! * 0x08fb (`0x41dee0`): sets mode 0x802 (no update; 0x800) and nothing else.
//! * 0x0f14 (`0x4108b0`): on its first tick state 1, update distance 0xff.
//! * 0x0a6a / 0x0a6b / 0x0aa0 / 0x0aa1 (`0x3ca620`): movers between two poses ([`update_mover`]).
//! * 0x0c58 (`0x3ed598`): moves once to a cuboid's place when poked ([`update_0c58`]).
//! * 0x0a17 (`0x3bcdd8`): the swamp's water level ([`update_0a17`]).
//! * 0x0a92 / 0x0a93 (`0x3cfcc0`): float on that water ([`update_float`]).
//! * 0x0a53 (`0x41def0`): the floating bridges, tilting under Ratchet or sinking with him ([`update_0a53`]).
//! * 0x0a56 (`0x3c3ad8`): the lily pads, on the floating platforms' rocking ([`update_0a56`]).
//! * 0x0cb5 (`0x3efbc8`): the targets the Dynamo's shot (class 0x825) switches on ([`update_0cb5`]).
//! * 0x0936 / 0x08fa / 0x0a00 (`0x3b3310`): the platforms those targets move ([`update_dynamo_platform`]).
//! * 0x0a95 (`0x3d01f8`): the Dynamo to pick up ([`update_0a95`]).
//!
//! Not ported here: 0x0ab3 (`0x3d4098`, deleted once `0x1a7b2e` is set: a byte of the `0x1a7b00` table whose
//! `0x1a7b13` is the Gravity Boots, so probably an item had), 0x0e89 (`0x406860`, the 0x0c58 mover, started also by
//! `0x1a7b13` / `0x1a7b32`), 0x0ca0 (`0x3ee1d0`, the switch that pokes them: the use system, G-UI-001).

use crate::hero::physics::{add, add_rot, approach, dist2, scale, set_len, sub, sub_rot, V3};
use crate::moby::ai;
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

/// The teleporter's pads (`[5..8]`): at its place 0.4 lower, out along their facing by `[8]`·2.4 − 1.2, tilted by
/// `[9]`·−60° − 20°, their rows rebuilt (`0x399d28`).
fn teleporter_pads(w: &mut MobyWorld, id: usize, ctx: &Ctx) {
    let m = &w.mobys[id];
    let (pos, out, tilt) = (m.pos, m.pv_f(0x20) * 2.4 - 1.2, m.pv_f(0x24) * -1.047_197_7 - 0.349_065_9);
    for k in 0..3 {
        let Some(p) = usize::try_from(w.mobys[id].pv_i(0x14 + 4 * k)).ok().filter(|&p| p < w.mobys.len()) else { continue };
        let pm = &mut w.mobys[p];
        pm.rot[1] = -20.0 * 0.017_453_294;
        let rows = rows_of(pm.rot);
        pm.pos = add([pos[0], pos[1], pos[2] - 0.4], set_len(rows[0], out));
        pm.rot[1] = tilt;
        pm.drive = true;
        w.rebuild(p, &*ctx.host);
    }
}

/// Its pads shown (with collision) or hidden (`0x39a4d8` / `0x39a488`).
fn teleporter_show(w: &mut MobyWorld, id: usize, show: bool) {
    let solid = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
    let pads: Vec<usize> = (0..3).filter_map(|k| usize::try_from(w.mobys[id].pv_i(0x14 + 4 * k)).ok()).filter(|&p| p < w.mobys.len()).collect();
    for i in std::iter::once(id).chain(pads) {
        let m = &mut w.mobys[i];
        if show { (m.mode, m.drawn) = (m.mode & !1, 1) } else { (m.mode, m.drawn) = (m.mode | 1, 0) }
        if i == id { m.has_collision = show && solid; }
    }
}

/// 0x046f (`0x3994c0`, levels 1, 8, 9, 12, 17): a teleporter to its partner `+0x68`[0] (on Oozla to the Megacorp
/// store and back). Hidden until Ratchet first enters its cuboid `[1]` (its uid counted in the save; kept for good),
/// with its three pads (class 0x13b, made at the set-up). Standing on it (his ground moby, grounded) he gets its help
/// text once (not ported: `0x399e80`, the level's text `$gp+0x568`[`[0x10]`], 0x12e5 on Oozla); △ (and a 1.5 sphere
/// pushing 20 off it) walks him to its centre (`0x2d4440`, facing its yaw) while it powers up (`[8]` / `[9]` eased
/// to 1: 12/3600, at most 32/60: `$gp+0x558..0x560`); once he is there (his state 0x64; or after 360 ticks) the beam
/// `[0xd]` fades from 1.95 and he is put on the partner 0.2 up, facing its way (`0x2b9e68`); the partner powers down
/// (4). Not ported: the beam's draw (`0x2dce00`), the help text, the cameras' snap (`0x304200`, `0x3245e8`,
/// `0x42b9f0`), the item gate `[0xf]` (`0x1a7a12`), the mission gate `[2]`, `0x320328`'s pad flag.
pub fn update_046f(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::hero::physics::ease;
    let uid = w.mobys[id].uid;
    let mut on = ctx.game.uid_dead.get(uid.max(0) as usize).is_some_and(|&d| d != 0) || ctx.game.uid_count(uid) != 0;
    if !on && w.mobys[id].pv_i(4) != -1 && ctx.level.in_cuboid(w.mobys[id].pv_i(4), ctx.hero.pos) {
        on = true;
        ctx.game.count_uid(uid);
        teleporter_show(w, id, true);
        if w.mobys[id].state == 5 { w.mobys[id].set_state(1, -1); }
    }
    let m = &mut w.mobys[id];
    if (m.state_ticks as u16) < 0x7ffd { m.state_ticks += 1; }
    let (a, b, most) = (12.0 / 3600.0, 12.0 / 3600.0, 32.0 / 60.0);
    let ease_to = |m: &mut crate::moby::Moby, t: f32, x: usize, v: usize| {
        let (mut p, mut s) = (m.pv_f(x), m.pv_f(v));
        ease(t, a, b, most, &mut p, &mut s);
        m.set_pv_f(x, p);
        m.set_pv_f(v, s);
    };
    match m.state {
        0 => {
            m.update_dist = 0xff;
            let (pos, rot, ud) = (m.pos, m.rot, m.update_dist);
            for k in 0..3 {
                let p = w.create(0x013b);
                let pm = &mut w.mobys[p];
                pm.draw_dist = 0x40;
                pm.drawn = 1;
                pm.pos = pos;
                pm.rot = [rot[0], rot[1], add_rot(rot[2], k as f32 * 2.094_395_4)];
                pm.update_dist = ud;
                pm.set_pv_i(0, id as i32);
                w.mobys[id].set_pv_i(0x14 + 4 * k, p as i32);
            }
            if w.mobys[id].pv_i(0x38) == 0 && !on {
                w.mobys[id].set_state(5, -1);
                teleporter_show(w, id, false);
            } else {
                w.mobys[id].set_state(6, -1);
            }
        }
        6 => w.mobys[id].set_state(1, -1),
        1 => {
            let m = &w.mobys[id];
            if m.pv_i(0) == -1 || ctx.hero.ground_moby != Some(id) || ctx.hero.air_ticks != 0 { teleporter_pads(w, id, ctx); return; }
            if m.pv_i(0x50) != 0 {
                let idle = ctx.hero.state == 0;
                w.mobys[id].set_pv_i(0x50, idle as i32);
            }
            if ctx.hero.items.pressed & crate::pad::button::TRIANGLE != 0 {
                ctx.host.play_sound(&w.mobys[id], 0, 0);
                let m = &mut w.mobys[id];
                m.set_state(2, -1);
                m.set_pv_i(0x28, 0);
                m.set_pv_i(0x30, -1);
                m.set_pv_i(0x2c, 0);
                let (yaw, at) = (m.rot[2], m.pos);
                ctx.hero.script_walk_to(yaw, at, false);
                ai::attack_sphere(w, id, ctx, 1.5, 20.0, 1.0, at, 0x10000, 0, 1, 0);
                return;
            }
        }
        2 => {
            let walk = match ctx.hero.state { 0x62 | 0x63 => 1, 0x64 => 2, _ => 0 };
            if m.pv_f(0x20) < 1.0 {
                ease_to(m, 1.0, 0x20, 0x28);
            } else if m.pv_f(0x24) < 1.0 {
                ease_to(m, 1.0, 0x24, 0x2c);
            } else if walk == 2 || (walk != 0 && 0x168 < m.state_ticks) {
                if walk != 2 { ctx.hero.request_state(0, true); }
                m.set_pv_f(0x34, 1.95);
                m.set_state(3, -1);
            } else if walk == 0 {
                m.set_state(4, -1);
            }
        }
        3 => {
            if m.pv_f(0x34) != 0.0 {
                let mut f = m.pv_f(0x34);
                approach(0.0, 0.066_666_67, &mut f);
                m.set_pv_f(0x34, f);
                return;
            }
            let Some(p) = usize::try_from(m.pv_i(0)).ok().filter(|&p| p < w.mobys.len()) else { return };
            let (to, rot) = (w.mobys[p].pos, w.mobys[p].rot);
            ctx.hero.teleport([to[0], to[1], to[2] + 0.2], rot);
            // His walk to the old pad's centre ends there (he stands on the new pad).
            ctx.hero.request_state(0, true);
            let pm = &mut w.mobys[p];
            pm.set_state(4, -1);
            for o in [0x28, 0x2c, 0x34] { pm.set_pv_i(o, 0); }
            pm.set_pv_f(0x24, 1.0);
            pm.set_pv_f(0x20, 1.0);
            pm.set_pv_i(0x50, 1);
            ctx.host.play_sound(&w.mobys[p], 1, 0);
            let m = &mut w.mobys[id];
            m.set_state(1, -1);
            for o in [0x2c, 0x20, 0x24, 0x28] { m.set_pv_i(o, 0); }
            m.set_pv_f(0x34, 1.95);
        }
        4 | 8 => {
            if m.pv_f(0x34) < 1.95 {
                let mut f = m.pv_f(0x34);
                approach(1.95, 0.066_666_67, &mut f);
                m.set_pv_f(0x34, f);
            } else if 0.0 < m.pv_f(0x24) {
                ease_to(m, 0.0, 0x24, 0x28);
            } else if m.pv_f(0x20) <= 0.0 {
                m.set_state(1, -1);
            } else {
                ease_to(m, 0.0, 0x20, 0x2c);
            }
        }
        7 => {
            if m.pv_f(0x20) < 1.0 {
                ease_to(m, 1.0, 0x20, 0x28);
            } else if m.pv_f(0x24) < 1.0 {
                ease_to(m, 1.0, 0x24, 0x2c);
            } else {
                ctx.host.play_sound(&w.mobys[id], 1, 0);
                let m = &mut w.mobys[id];
                m.set_state(8, -1);
                m.set_pv_f(0x34, 0.0);
            }
        }
        _ => return,
    }
    teleporter_pads(w, id, ctx);
}

/// 0x0a53 (`0x41def0`, 4 placed): a floating bridge on the swamp. Once there is water (above 100) it keeps its height over the
/// water (`+0x6c`, the lowest `+0x64` = 3 under it), its angles (`+0x80..`) and two rocking phases (`+0x78` / `+0x7c`);
/// `+0xa0` set makes it a sinker (1), else it tilts (2).
///
/// * 1, the sinker: with Ratchet grounded on it, it sinks toward the lowest at 0.4 a second eased out over the 3
///   (`0x31b918`: `1 − (1 − x)²` of what is left); without him it rises back at 0.6 a second the same way.
/// * 2, the tilting pad: with Ratchet grounded on it, his offset along its rows springs its roll to −0.01 × the side
///   offset and its pitch to 0.025 × the forward one (`0x31f0c8`: 0.01, 0.3, at most 0.1); without him both spring
///   back to 0.
///
/// Both rock 1.15° on the water (`0x31a3c0`: 30 and 40 °/s, the tunables `$gp+0x2adc..0x2ae4`), the tilt on top, and
/// carry him (`0x322668`, the block `+0x20`).
///
/// Not ported: the zone test `0x321a78` (the scheduler's distances stand in), the sinker giving way under a swimming
/// Ratchet (his state 0x19 under it: its collision off), the pad's colour pulse when drawn (`+0x78` of the moby,
/// `+0xac` 3°/tick), the unused carried point `0x317e68`.
pub fn update_0a53(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::hero::physics::{dot, turn_spring};
    const STEP: f32 = 0.000_290_888_25;
    const DEPTH: f32 = 3.0;
    let water = ctx.game.water_z;
    let on = ctx.hero.ground_moby == Some(id) && ctx.hero.air_ticks == 0;
    let solid = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    let (old, old_yaw) = (m.pos, m.rot[2]);
    match m.state {
        0 => {
            if water <= 100.0 { return; }
            let (a, b) = (ctx.game.rng.rand_angle(), ctx.game.rng.rand_angle());
            m.set_pv_f(0x78, a);
            m.set_pv_f(0x7c, b);
            m.set_pv_f(0x68, 0.0);
            let h = m.pos[2] - water;
            m.set_pv_f(0x6c, h);
            m.set_pv_f(0x60, h);
            m.set_pv_f(0x64, h - DEPTH);
            let rot = m.rot;
            m.set_pv_v(0x80, rot);
            m.set_pv_v(0x90, [0.0; 3]);
            let s = if m.pv_i(0xa0) == 0 { 2 } else { 1 };
            m.set_state(s, -1);
            return;
        }
        1 => {
            m.has_collision = solid;
            let sunk = m.pv_f(0x6c) - m.pv_f(0x64);
            let ease_out = |x: f32| 1.0 - (1.0 - x) * (1.0 - x);
            let v = if on { -(0.4 / 60.0) * ease_out(sunk / DEPTH) } else { 0.6 * ease_out((DEPTH - sunk) / DEPTH) / 60.0 };
            m.set_pv_f(0x68, v);
            let h = m.pv_f(0x6c) + v;
            m.set_pv_f(0x6c, h);
            m.pos[2] = water + h;
            super::props::sway(m, 0.020_071_289, 30.0 * STEP, 40.0 * STEP, 0x78, 0x7c);
        }
        2 => {
            let (mut rx, mut vx, mut ry, mut vy) = (m.pv_f(0x80), m.pv_f(0x90), m.pv_f(0x84), m.pv_f(0x94));
            if on {
                let d = sub(ctx.hero.pos, m.pos);
                let (fwd, side) = (dot(d, m.rows[0]), dot(d, m.rows[1]));
                turn_spring(-0.01 * side, 0.01, 0.3, 0.1, &mut rx, &mut vx, 0);
                turn_spring(0.025 * fwd, 0.01, 0.3, 0.1, &mut ry, &mut vy, 0);
            } else {
                turn_spring(0.0, 0.01, 0.3, 0.1, &mut rx, &mut vx, 0);
                turn_spring(0.0, 0.01, 0.3, 0.1, &mut ry, &mut vy, 0);
            }
            m.set_pv_f(0x80, rx);
            m.set_pv_f(0x90, vx);
            m.set_pv_f(0x84, ry);
            m.set_pv_f(0x94, vy);
            m.pos[2] = water + m.pv_f(0x6c);
            super::props::sway(m, 0.020_071_289, 30.0 * STEP, 40.0 * STEP, 0x78, 0x7c);
            m.rot[0] = add_rot(rx, m.rot[0]);
            m.rot[1] = add_rot(ry, m.rot[1]);
        }
        _ => return,
    }
    let (pos, yaw) = (m.pos, m.rot[2]);
    carry_hero(ctx, id, old, old_yaw, pos, yaw);
}

/// 0x0a56 (`0x3c3ad8`, 8 placed): a lily pad. Once there is water it keeps two spare phases (`+0x120` / `+0x124`,
/// `+0x128` = 0.02) and its angles (`+0x130`), sets up the floating platforms' rocking at `+0x60`
/// ([`crate::moby::floater`], with its own tunables, as the live block shows: tilts clamped to 0.17, sink 0.19,
/// response 0.027 / 0.027 / 0.05, wobble 0.03 / 0.03 / 0.057) and goes to 1. In 1, drawn and within 32 of the camera,
/// its home height follows the water (`$gp+0xf24` = 0 above it) and it rocks.
pub fn update_0a56(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::moby::floater;
    const BLOCK: usize = 0x60;
    let water = ctx.game.water_z;
    match w.mobys[id].state {
        0 => {
            if water <= 0.0 { return; }
            let (a, b) = (ctx.game.rng.rand_angle(), ctx.game.rng.rand_angle());
            let m = &mut w.mobys[id];
            m.set_pv_f(0x120, a);
            m.set_pv_f(0x124, b);
            m.set_pv_f(0x128, 0.02);
            let rot = m.rot;
            m.set_pv_v(0x130, rot);
            m.set_pv_v(0x140, [0.0; 3]);
            m.set_state(1, -1);
            let p = floater::Params {
                clamp: [0.17, 0.17, 0.19],
                response: [0.027, 0.027, 0.05],
                amplitude: [0.03, 0.03, 0.057],
                ..Default::default()
            };
            floater::init(w, id, ctx, BLOCK, &p);
        }
        1 => {
            let m = &w.mobys[id];
            if m.drawn == 0 || 32.0 <= dist2(m.pos, ctx.host.camera_pos()) { return; }
            w.mobys[id].set_pv_f(BLOCK + 8, water);
            floater::step(w, id, ctx, BLOCK);
        }
        _ => {}
    }
}

/// 0x0cb5 (`0x3efbc8`, 12 placed): a target that switches its group on. At its set-up it rises 1 over its place,
/// keeps its pitch (`+0x14`) and goes in the target list (mode 0x1000), its sequence 0. Each tick its hit check
/// (`0x3efdb8`): a hit from the Dynamo's shot (class 0x825) pokes its group (`0x31c970`: each member's `+0xbc` = 2,
/// itself too). Poked (state 1) it leaves the list, sequence 1, and stays on (state 2): with a time `+0x00` it blinks
/// between its two glows (`$gp+0x1cc0` / `+0x1cc4`) every 10 ticks for `+0x04` ticks, then (its `+0xbc` cleared,
/// sequence 3) back in the list, waiting again (1); without one it keeps the "on" glow. Its group's platforms:
/// [`update_dynamo_platform`].
pub fn update_0cb5(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const GLOW_OFF: u32 = 0x8000_30e1;
    const GLOW_ON: u32 = 0x8000_df2a;
    use super::props::{seq, wrapped};
    if let Some(h) = w.hit_message(id).filter(|h| h.tmpl.flags & 0x10000 != 0) {
        let dynamo = h.tmpl.source.and_then(|s| w.mobys.get(s)).is_some_and(|s| s.o_class == 0x825);
        let g = w.mobys[id].group;
        if dynamo && g != -1 {
            for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() { w.mobys[i].timer = 2; }
        }
    }
    w.clear_hit(id);
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.pos[2] += 1.0;
            let pitch = m.rot[0];
            m.set_pv_f(0x14, pitch);
            m.state = 1;
            m.set_pv_i(0x50, 0);
            m.mode |= crate::moby::mode::TARGET;
            if seq(w, id, ctx) != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 0xc); }
        }
        1 => {
            if wrapped(w, id, ctx) && seq(w, id, ctx) != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 0); }
            if w.mobys[id].timer == 0 { return; }
            w.mobys[id].mode &= !crate::moby::mode::TARGET;
            ctx.host.set_anim(&w.mobys[id], 1, 0, 0);
            let m = &mut w.mobys[id];
            m.state = 2;
            if m.pv_i(0) != 0 {
                m.set_pv_i(0x10, 10);
                m.set_pv_i(0xc, m.pv_i(4));
            } else {
                m.glow = GLOW_ON;
                m.set_pv_i(0xc, 0);
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            if m.pv_i(0) != 0 {
                let mut t = m.pv_i(0x10);
                if crate::moby::countdown(&mut t) {
                    m.glow = if m.glow == GLOW_OFF { GLOW_ON } else { GLOW_OFF };
                    t = 10;
                }
                m.set_pv_i(0x10, t);
                let mut left = m.pv_i(0xc);
                if crate::moby::countdown(&mut left) { m.timer = 0; }
                m.set_pv_i(0xc, left);
            }
            let wrap = wrapped(w, id, ctx);
            if wrap && seq(w, id, ctx) == 1 { ctx.host.set_anim(&w.mobys[id], 2, 0, 0); }
            if w.mobys[id].timer == 0 && wrap {
                w.mobys[id].mode |= crate::moby::mode::TARGET;
                ctx.host.set_anim(&w.mobys[id], 3, 0, 0);
                let m = &mut w.mobys[id];
                m.state = 1;
                m.glow = GLOW_OFF;
            }
        }
        _ => {}
    }
}

/// 0x0936 / 0x08fa / 0x0a00 (`0x3b3310`, 8 placed): a platform the Dynamo's targets move (their group's poke,
/// `+0xbc`), between its place (kept at `+0x60` / `+0x70`) and the cuboid `+0x80`'s place and angles. Poked at home
/// (1, no collision there) it waits `+0x84` ticks (2) and goes over `+0x88` ticks (3: `t` `+0x9c`, eased
/// `(1 − cos πt)/2`, each angle turned the short way, `0x31e980`); there it holds `+0x8c` ticks (4) or, with −1, until
/// poked again (5: then `+0x84` more); back the same way (6); then, `+0x98` −1, it waits `+0x94` (7) and goes again;
/// else it counts its trips down (`+0xa4`): with some left it waits `+0x94` and goes again, at the last its group's
/// pokes cleared (`0x31c970(group, 0)`) it is home again (1). It carries Ratchet (`0x322668`, the block `+0x20`).
///
/// Not ported: its moving sound (class 0x9ea's, looped, `+0xa8`), its glow while it is out (`0x321ba0`, `+0xac` /
/// the moby's `+0x23`).
#[allow(clippy::collapsible_match)]
pub fn update_dynamo_platform(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let solid = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    let (old, old_rot) = (m.pos, m.rot);
    let poked = m.timer != 0;
    let place = ctx.level.cuboid_place(m.pv_i(0x80));
    let tick_down = |m: &mut crate::moby::Moby, off: usize| {
        let mut t = m.pv_i(off);
        let done = crate::moby::countdown(&mut t);
        m.set_pv_i(off, t);
        done
    };
    let pose = |m: &mut crate::moby::Moby, t: f32| {
        let Some((to, angles)) = place else { return };
        let f = (1.0 - (t * PI).cos()) * 0.5;
        let home = m.pv_v(0x60);
        m.pos = add(home, scale(sub(to, home), f));
        for (k, a) in angles.iter().enumerate() { m.rot[k] = lerp_rot(m.pv_f(0x70 + 4 * k), *a, f); }
    };
    match m.state {
        0 => {
            m.mode |= 0x2000;
            m.set_pv_i(0xa8, -1);
            let (pos, rot) = (m.pos, m.rot);
            m.set_pv_v(0x60, pos);
            m.set_pv_v(0x70, rot);
            m.set_state(1, -1);
            m.has_collision = false;
        }
        1 => {
            if poked {
                m.set_pv_i(0xa4, m.pv_i(0x98));
                m.set_pv_i(0xa0, m.pv_i(0x84));
                m.set_state(2, -1);
                m.has_collision = solid;
            }
        }
        2 => {
            if tick_down(m, 0xa0) {
                m.set_pv_f(0x9c, 0.0);
                m.set_state(3, -1);
            }
        }
        3 => {
            let t = (m.pv_f(0x9c) + 1.0 / m.pv_f(0x88)).min(1.0);
            m.set_pv_f(0x9c, t);
            pose(m, t);
            if 1.0 <= t {
                if m.pv_i(0x8c) != -1 {
                    m.set_pv_i(0xa0, m.pv_i(0x8c));
                    m.set_state(4, -1);
                } else {
                    m.set_state(5, -1);
                }
            }
        }
        4 => {
            if tick_down(m, 0xa0) {
                m.set_pv_f(0x9c, 1.0);
                m.set_state(6, -1);
            }
        }
        5 => {
            if poked {
                m.set_pv_i(0xa0, m.pv_i(0x84));
                m.timer = 0;
                m.set_state(4, -1);
            }
        }
        6 => {
            let t = (m.pv_f(0x9c) - 1.0 / m.pv_f(0x88)).max(0.0);
            m.set_pv_f(0x9c, t);
            pose(m, t);
            if t <= 0.0 {
                if m.pv_i(0x98) == -1 || !tick_down(m, 0xa4) {
                    m.set_pv_i(0xa0, m.pv_i(0x94));
                    m.set_state(7, -1);
                } else {
                    let g = m.group;
                    m.set_state(1, -1);
                    m.has_collision = false;
                    if g != -1 {
                        for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() { w.mobys[i].timer = 0; }
                    }
                }
            }
        }
        7 => {
            if tick_down(m, 0xa0) {
                m.set_pv_f(0x9c, 0.0);
                m.set_state(3, -1);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.timer = 0;
    if m.state < 8 {
        let (pos, yaw) = (m.pos, m.rot[2]);
        carry_hero(ctx, id, old, old_rot[2], pos, yaw);
    }
}

/// 0x0a95 (`0x3d01f8`, 1 placed): the Dynamo (item 0x24) to pick up. Not had yet it floats 0.75 over its place at
/// 1.5 times its scale (else it is gone at once), turning 4.5° a tick; Ratchet in its cuboid `+0x04` asks for the
/// scene 0x26 (`0x2f4ee8`, then `0x2f5030`: the chain started) and it waits (2), hidden while the scene plays. Once no
/// scene is asked for or playing (`0x371ba8`) Ratchet is put at the cuboid `+0x08` (`0x2b9e68`), the Dynamo
/// is his (`0x2f46e0`, in the quick select `0x3227d8`) and it is gone.
///
/// Not ported: its sparkle (`0x3cff58`), the message 0x1225 (`0x327448`), the autosave (`0x30d658`), the camera of
/// the moby `+0x0c` (`0x42d968`), the help 0x9b9 after three minutes.
pub fn update_0a95(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::hero::items::DYNAMO;
    let scale_of = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    match w.mobys[id].state {
        0 => {
            if ctx.game.items.get(DYNAMO as usize).copied().unwrap_or(0) != 0 {
                w.delete(id);
                return;
            }
            let m = &mut w.mobys[id];
            m.pos[2] += 0.75;
            m.scale = scale_of * 1.5;
            m.set_state(1, -1);
        }
        1 => {
            let m = &mut w.mobys[id];
            m.rot[2] = add_rot(m.rot[2], 0.078_539_82);
            if !ctx.level.in_cuboid(m.pv_i(4), ctx.hero.pos) { return; }
            if !ctx.game.scenes.queue.push(0x26) { return; }
            ctx.game.scenes.start_queue(ctx.hero.hp);
            w.mobys[id].set_state(2, -1);
        }
        2 => {
            let m = &mut w.mobys[id];
            m.state_ticks = m.state_ticks.wrapping_add(1);
            if ctx.game.scenes.mode == crate::scene::SCENE {
                m.drawn = 0;
                m.mode |= 1;
            }
            if ctx.game.scenes.in_scene() { return; }
            if let Some((pos, rot)) = ctx.level.cuboid_place(m.pv_i(8)) {
                ctx.hero.teleport(pos, rot);
                // The game holds him in 0x6f for the camera of `+0x0c` (`0x42d968`, not ported): idle at once here.
                ctx.hero.request_state(0, true);
            }
            ctx.game.get_item(DYNAMO);
            ctx.game.give_item(DYNAMO);
            w.delete(id);
        }
        _ => {}
    }
}
