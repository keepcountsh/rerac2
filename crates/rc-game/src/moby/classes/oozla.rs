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
//! * 0x0a7b (`0x3cb158`): the conveyor platforms the Dynamo slides out ([`update_0a7b`]).
//! * 0x061c (`0x3a6fb8`): the path riders on the swamp, knocked off and sinking when hit ([`update_061c`]).
//! * 0x0ace (`0x3d42b0`): the Tractor Beam's seller ([`update_0ace`]).
//!
//! Not ported here: 0x0ab3 (`0x3d4098`, deleted once `0x1a7b2e` is set: a byte of the `0x1a7b00` table whose
//! `0x1a7b13` is the Gravity Boots, so probably an item had), 0x0e89 (`0x406860`, the 0x0c58 mover, started also by
//! `0x1a7b13` / `0x1a7b32`), 0x0ca0 (`0x3ee1d0`, the switch that pokes them: the use system, G-UI-001).

use crate::hero::physics::{add, add_rot, approach, dist2, scale, set_len, sub, sub_rot, V3};
use crate::moby::ai;
use crate::moby::attach::rows_of;
use crate::moby::hits::HitTemplate;
use crate::moby::{carry_hero, mode, Ctx, MobyWorld};
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

/// 0x0a17 (`0x3bcdd8`): the swamp's water. At its start (state 0) its colours (`+0x00` the band near `0x80103030`,
/// `+0x04` the band far and the wall's foot `0x800c1006`, `+0x0c` / `+0x10` the surface near / at two tiles
/// `0x50106060` / `0x00106060`), its reach `+0x28` 224, the layers' scroll 0 (`game.water_scroll`, `0x1b2fd0`) and the
/// camera noted (`game.water_cam`, `$gp+0x3fd0`). Its level (`game.water_base`, L01 `$gp+0xdec`) is its `+0x14`, or
/// with Ratchet in one of its eight cuboids `+0x40..+0x60` that cuboid's centre height + 0.15; with him in one of the
/// two cuboids `+0x30` / `+0x34` (dry ground) the tick ends there. Then the camera's move since the last tick turns
/// each layer's scroll by `move · 2·scale / (5·tile)` (scales `$gp+0xdc0`: 2, 3; tile `+0x18`; the whole turns
/// dropped), and while the camera is above `+0x20` the scroll drifts by the layer's speed (`$gp+0xdd0`: (0.04, 0.04)
/// and (0, −0.04)) / 60, both kept within ±1; the height (`game.water_z`, `$gp+0xde8`, what `0x3bc3b8` gives the
/// floating objects) waves 0.1 about the level, `sin(frame mod 360 · 0.017444445 − 3.14)`; and the water is drawn
/// ([`draw_0a17`], registered on the draw list before the ties `0x2dcec8`). Not ported: the camera test of the dry
/// cuboids when `0x18c0b4` is 0x1c, and the flag `0x18a01e` (0 in the dry cuboids, else 2).
pub fn update_0a17(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const SCALE: [f32; 2] = [2.0, 3.0];
    const SPEED: [[f32; 2]; 2] = [[0.04, 0.04], [0.0, -0.04]];
    let cam = ctx.host.camera_pos();
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.update_dist = 0xff;
        m.state = 1;
        for (o, c) in [(0x00, 0x8010_3030u32), (0x04, 0x800c_1006), (0x0c, 0x5010_6060), (0x10, 0x0010_6060)] { m.set_pv_i(o, c as i32); }
        m.set_pv_f(0x28, 224.0);
        ctx.game.water_scroll = [[0.0; 2]; 2];
        ctx.game.water_cam = cam;
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
    let d = sub(cam, ctx.game.water_cam);
    ctx.game.water_cam = cam;
    if m.pv_f(0x20) < cam[2] {
        let wrap = |x: f32| if 1.0 < x { x - 1.0 } else if x < -1.0 { x + 1.0 } else { x };
        let tile = m.pv_f(0x18);
        for (k, sc) in ctx.game.water_scroll.iter_mut().enumerate() {
            let f = SCALE[k] * 2.0 / (tile * 5.0);
            let step = |v: f32| if v != 1.0 { v - v.trunc() } else { v };
            sc[0] = wrap(sc[0] + step(d[0] * f));
            sc[1] = wrap(sc[1] + step(d[1] * f));
        }
        // The game's own 3.14 (not π).
        #[allow(clippy::approx_constant)]
        let phase = (ctx.game.frame % 360) as f32 * 0.017_444_445 - 3.14;
        ctx.game.water_z = ctx.game.water_base + phase.sin() * 0.1;
        for (k, sc) in ctx.game.water_scroll.iter_mut().enumerate() {
            sc[0] = wrap(sc[0] + SPEED[k][0] / 60.0);
            sc[1] = wrap(sc[1] + SPEED[k][1] / 60.0);
        }
        w.register_draw(id);
    }
}

/// `0x3bc3c0` (0x0a17's draw, crate::fx): the swamp water about the camera at the water's height (`game.water_z`).
/// F and L are the camera's forward and left rows flattened and normalised.
/// * The far band (FX 0x16, ST 0, ALPHA 0x44: the colours' own alpha, opaque here): a strip from `cam − 75F ∓ 87L` to
///   `cam + reach·F ∓ 750L` (`+0x28`) in five steps, its colour `+0x00` → `+0x04` along it.
/// * The wall (the same FX, ALPHA 0x44), with the camera more than `+0x24` above the level (`+0x14`; not in state 0x1c):
///   from the band's far edge up to `camera z − +0x24`, `+0x04` at its foot and `+0x08` at its top.
/// * The surface, two layers (FX `+0x1c + layer + 0x3f`, ALPHA 0x48: added): 4 × 4 tiles of `+0x18` about the camera
///   (`cam + (x, y) + 1`), ST `x / (2·tile) · scale + scroll`, each corner's colour `+0x0c` → `+0x10` by its distance
///   to the camera over two tiles.
///
/// The water's own fog (`0x1bdbe8` / `0x1bdbec` set to 255 / 249.9 for its draw: next to none) is the effects' (none).
/// Not ported: its draws' Z test without Z write (the effects test Z the same way), the hero state 0x1c's normal fog.
pub fn draw_0a17(w: &MobyWorld, id: usize, game: &crate::game::GameState, cam: V3, rows: [V3; 3], out: &mut Vec<crate::fx::FxQuad>) {
    use crate::fx::FxQuad;
    use crate::hud::lerp_rgba;
    const SCALE: [f32; 2] = [2.0, 3.0];
    let m = &w.mobys[id];
    let c = |o: usize| m.pv_i(o) as u32;
    let z = game.water_z;
    let flat = |v: V3| set_len([v[0], v[1], 0.0], 1.0);
    let (f, l) = (flat(rows[0]), flat(rows[1]));
    let at = |a: f32, b: f32| -> V3 { let p = add(add(scale(f, a), scale(l, b)), cam); [p[0], p[1], z] };
    let (n0, n1, f0, f1) = (at(-75.0, -87.0), at(-75.0, 87.0), at(m.pv_f(0x28), -750.0), at(m.pv_f(0x28), 750.0));
    let lerp = |a: V3, b: V3, t: f32| add(a, scale(sub(b, a), t));
    let st0 = [[0.0; 2]; 4];
    let band = |i: usize| (i as f32 * 0.2, lerp_rgba(i as f32 * 0.2, c(0x00), c(0x04)));
    for i in 0..5 {
        let ((t0, c0), (t1, c1)) = (band(i), band(i + 1));
        out.push(FxQuad { fx: 0x16, additive: false, corners: [lerp(n0, f0, t0), lerp(n1, f1, t0), lerp(n0, f0, t1), lerp(n1, f1, t1)], st: st0, rgba: [c0, c0, c1, c1] });
    }
    let top = cam[2] - m.pv_f(0x24);
    if m.pv_f(0x14) < top {
        let up = |p: V3| [p[0], p[1], top];
        out.push(FxQuad { fx: 0x16, additive: false, corners: [f0, up(f0), f1, up(f1)], st: st0, rgba: [c(0x04), c(0x08), c(0x04), c(0x08)] });
    }
    let tile = m.pv_f(0x18);
    if tile <= 0.0 { return; }
    let colour = |p: V3| lerp_rgba((crate::hero::physics::len(sub(p, cam)) / (tile * 2.0)).min(1.0), c(0x0c), c(0x10));
    for (layer, sc) in game.water_scroll.iter().enumerate() {
        let fx = (m.pv_i(0x1c) + layer as i32 + 0x3f) as usize;
        let st = |x: f32, y: f32| [x / (tile * 2.0) * SCALE[layer] + sc[0], y / (tile * 2.0) * SCALE[layer] + sc[1]];
        for row in 0..4 {
            let y = (row as f32 - 2.0) * tile;
            for col in 0..4 {
                let x = (col as f32 - 2.0) * tile;
                let p = |dx: f32, dy: f32| [cam[0] + x + dx + 1.0, cam[1] + y + dy + 1.0, z];
                let corners = [p(0.0, 0.0), p(0.0, tile), p(tile, 0.0), p(tile, tile)];
                out.push(FxQuad {
                    fx,
                    additive: true,
                    corners,
                    st: [st(x, y), st(x, y + tile), st(x + tile, y), st(x + tile, y + tile)],
                    rgba: corners.map(colour),
                });
            }
        }
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
            w.poke_group(g as i32, 2);
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
/// Its look: a green projection, shown only once poked ([`hologram`], the phase `+0xac`, on from state 2).
///
/// Not ported: its moving sound (class 0x9ea's, looped, `+0xa8`).
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
    let on = 1 < w.mobys[id].state;
    hologram(&mut w.mobys[id], 0xac, on, &mut ctx.game.rng);
}

/// `0x321ba0(m, phase, on)`: a projection's look. While it is on, or not yet faded out, the phase (a pvar) turns 0.1455
/// a tick (wrapped to ±π) and the ambient pulses between (0x20, 0xc0, 0) and (0, 0x80, 0) by `(sin + 0.5) / 2`, one
/// tick in twenty (`0x3117a0`) 0.3 of the way to (0xff, 0xff, 0x80); the alpha (`+0x23`) goes 3 a tick up to 0x40 on, down
/// to 0 off. So a platform spawned at 0x80 fades out at the start and shows only while it is out.
fn hologram(m: &mut crate::moby::Moby, phase: usize, on: bool, rng: &mut crate::rng::Rng) {
    if m.alpha == 0 && !on { return; }
    let mut p = m.pv_f(phase) + 0.145_444_1;
    if PI <= p { p = p - PI - PI; } else if p < -PI { p = p + PI + PI; }
    m.set_pv_f(phase, p);
    let lerp = |t: f32, a: [u8; 3], b: [u8; 3]| -> [u8; 3] { std::array::from_fn(|k| (a[k] as f32 * (1.0 - t) + b[k] as f32 * t) as u8) };
    let mut c = lerp((p.sin() + 0.5) * 0.5, [0x20, 0xc0, 0x00], [0x00, 0x80, 0x00]);
    if rng.randf(0.0, 1.0) < 0.05 { c = lerp(0.3, c, [0xff, 0xff, 0x80]); }
    m.ambient = c;
    m.alpha = match (on, m.alpha) {
        (false, a) if a < 4 => 0,
        (false, a) => a - 3,
        (true, a) if a < 0x3d => a + 3,
        (true, _) => 0x40,
    };
}

/// 0x0a95 (`0x3d01f8`, 1 placed): the Dynamo (item 0x24) to pick up. Not had yet it floats 0.75 over its place at
/// 1.5 times its scale (else it is gone at once), turning 4.5° a tick; Ratchet in its cuboid `+0x04` asks for the
/// scene 0x26 (`0x2f4ee8`, then `0x2f5030`: the chain started) and it waits (2), hidden while the scene plays. Once no
/// scene is asked for or playing (`0x371ba8`) Ratchet is put at the cuboid `+0x08` (`0x2b9e68`), the Dynamo
/// is his (`0x2f46e0`), asked for in his hand (`0x3227d8`) and it is gone.
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
            // `0x2e99e8(0x24)` then `0x3186d0(0x24)` (L01's `0x2f46e0` / `0x3227d8`): his, then in his hand.
            ctx.game.get_item(DYNAMO);
            let held = ctx.game.held;
            ctx.hero.items.ask(held, DYNAMO);
            w.delete(id);
        }
        _ => {}
    }
}

/// 0x0a7b (`0x3cb158`, 3 placed): a conveyor platform a Dynamo target slides out. Poked (`+0xbc`) at home (1, no
/// collision there) it goes over `+0x60` ticks (2: `t` `+0x70`, eased `(1 − cos πt)/2`) toward the cuboid `+0x94`
/// (its distance kept in `+0x98` at the set-up; −1: 10 along its first row), holds `+0x64` ticks (3), comes back (4)
/// and, home, its group's pokes cleared, waits again (1). Out (2..4) its belt pushes whoever stands on it along its
/// first row, `+0x68` · `+0x7c` a second (the mover block `+0x20`, `0x322668`: the belt's push, not the platform's
/// slide).
///
/// Out, it draws its belt ([`draw_0a7b`]). Not ported: its loop sound (`+0x90`, when on screen within 255).
pub fn update_0a7b(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let solid = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
    let cuboid = ctx.level.cuboid_place(w.mobys[id].pv_i(0x94)).map(|(p, _)| p);
    let m = &mut w.mobys[id];
    let place = |m: &mut crate::moby::Moby, t: f32| {
        let f = (1.0 - (t * PI).cos()) * 0.5;
        let home = m.pv_v(0x80);
        let d = match cuboid {
            Some(c) => set_len(sub(c, home), m.pv_f(0x98) * f),
            None => set_len(m.rows[0], f * 10.0),
        };
        m.pos = add(home, d);
    };
    match m.state {
        0 => {
            m.set_pv_i(0x90, -1);
            m.set_pv_f(0x70, 0.0);
            m.set_pv_i(0x5c, m.pv_i(0x5c) | 4);
            m.set_pv_i(0x6c, 0);
            m.set_pv_i(0x74, 0);
            m.has_collision = false;
            let pos = m.pos;
            m.set_pv_v(0x80, pos);
            if let Some(c) = cuboid { m.set_pv_f(0x98, crate::hero::physics::dist(pos, c)); }
            m.set_state(1, -1);
        }
        1 => {
            if m.timer != 0 {
                m.has_collision = solid;
                m.set_state(2, -1);
            }
        }
        2 => {
            let t = (m.pv_f(0x70) + 1.0 / m.pv_f(0x60)).clamp(0.0, 1.0);
            m.set_pv_f(0x70, t);
            place(m, t);
            if 1.0 <= t {
                m.set_pv_i(0x6c, m.pv_i(0x64));
                m.set_state(3, -1);
            }
        }
        3 => {
            let mut t = m.pv_i(0x6c);
            if crate::moby::countdown(&mut t) { m.set_state(4, -1); }
            m.set_pv_i(0x6c, t);
        }
        4 => {
            let t = (m.pv_f(0x70) - 1.0 / m.pv_f(0x60)).clamp(0.0, 1.0);
            m.set_pv_f(0x70, t);
            place(m, t);
            if t <= 0.0 {
                m.has_collision = false;
                let g = m.group;
                m.set_state(1, -1);
                if g != -1 {
                    for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() { w.mobys[i].timer = 0; }
                }
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.timer = 0;
    if (2..=4).contains(&m.state) {
        w.register_draw(id);
        let m = &mut w.mobys[id];
        let push = set_len(m.rows[0], m.pv_f(0x68) * m.pv_i(0x7c) as f32 * 0.016_666_668);
        let (pos, yaw) = (m.pos, m.rot[2]);
        carry_hero(ctx, id, sub(pos, push), yaw, pos, yaw);
    }
}

/// A belt of light (`0x3cac30` / `0x3d2ea8`): five layers of strips (ALPHA 0x44) from `base` stepping `along` (`step`
/// of the way a quad), ±`half` across, scrolling with `phase`, each edge's alpha sin(πk) × `fade` × 102.4 (the first
/// 0). Layer 0 green (0.25, 1, 0.5) `dz` up; 1 blue (0.25, 0.5, 1), half a texture on; 2 red, `dz` more, at
/// `scroll[0]` times the scroll, ±`red_half` across, its steps `shrink` as long and as many more; 3 and 4 white at
/// `scroll[1]` times the scroll, as 2 but `rail` to each side.
struct Beam {
    fx: usize,
    base: V3,
    along: V3,
    step: f32,
    half: V3,
    rail: V3,
    dz: f32,
    red_half: f32,
    shrink: f32,
    scroll: [f32; 2],
    phase: f32,
    fade: f32,
}

fn draw_beam(b: &Beam, out: &mut Vec<crate::fx::FxQuad>) {
    let rgb = |r: f32, g: f32, bl: f32| (r * 255.0) as u32 | ((g * 255.0) as u32) << 8 | ((bl * 255.0) as u32) << 16;
    let frac = |x: f32| x - x.trunc();
    let (mut base, mut along, mut half, mut step) = (b.base, b.along, b.half, b.step);
    for layer in 0..5 {
        let (start, colour, v) = match layer {
            0 => {
                base[2] += b.dz;
                (base, rgb(0.25, 1.0, 0.5), frac(b.phase))
            }
            1 => (base, rgb(0.25, 0.5, 1.0), frac(b.phase + 0.5)),
            2 => {
                base[2] += b.dz;
                half = set_len(half, b.red_half);
                along = scale(along, b.shrink);
                step *= b.shrink;
                (base, rgb(1.0, 0.0, 0.0), frac(b.phase * b.scroll[0]))
            }
            3 => (add(base, b.rail), rgb(1.0, 1.0, 1.0), frac(b.phase * b.scroll[1])),
            _ => (sub(base, b.rail), rgb(1.0, 1.0, 1.0), frac(b.phase * b.scroll[1])),
        };
        let st = [[0.0, v], [1.0, v], [0.0, v + 1.0], [1.0, v + 1.0]];
        let mut e = start;
        let mut prev = (sub(e, half), add(e, half), colour);
        e = add(e, along);
        let mut k = step;
        while k < 1.0 {
            let a = ((k * PI).sin() * b.fade * 102.4) as i32 as u32;
            let c = (colour & 0xff_ffff) | a << 24;
            let (l, r) = (sub(e, half), add(e, half));
            e = add(e, along);
            out.push(crate::fx::FxQuad { fx: b.fx, additive: false, corners: [prev.0, prev.1, l, r], st, rgba: [prev.2, prev.2, c, c] });
            prev = (l, r, c);
            k += step;
        }
    }
}

/// `0x3cac30` (0x0a7b out, registered each tick): its belt ([`draw_beam`], FX 0x3f + `+0x78`) behind it along its
/// first row, scrolling with `+0x74` (down `+0x68` / 120 a frame), fading with the slide `+0x70`. Its scale `s`
/// against the class's: the start 10·s back with the direction `+0x7c` 1 (else at it), stepping 2 × `+0x7c` at 0.2
/// a step, ±2·s across, 0.02 up a layer; the red ±0.4·s with steps 0.4 as long, the scrolls 3 and 3.5, the rails 1.6·s.
pub fn draw_0a7b(w: &mut MobyWorld, id: usize, out: &mut Vec<crate::fx::FxQuad>) {
    let class_scale = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let m = &mut w.mobys[id];
    let s = if class_scale != 0.0 { m.scale / class_scale } else { 1.0 };
    let phase = m.pv_f(0x74) - m.pv_f(0x68) * 0.016_666_668 / 2.0;
    m.set_pv_f(0x74, phase);
    let dir = m.pv_i(0x7c);
    let (row0, row1) = (m.rows[0], m.rows[1]);
    let base = if dir == 1 { add(m.pos, set_len(row0, s * -10.0)) } else { m.pos };
    let beam = Beam {
        fx: 0x3f + m.pv_i(0x78) as usize,
        base,
        along: set_len(row0, 2.0 * dir as f32),
        step: 0.2,
        half: set_len(row1, s * -2.0),
        rail: set_len(row1, 2.0 * s * 0.8),
        dz: 0.02,
        red_half: 2.0 * s * 0.2,
        shrink: 0.4,
        scroll: [3.0, 3.5],
        phase,
        fade: m.pv_f(0x70),
    };
    draw_beam(&beam, out);
}

/// `0x3d2ea8` (0x0a9d open, registered each tick): its light ([`draw_beam`], FX 0x3f + `+0xb4`) at its home `+0x60`,
/// from 1.05 back along its first row one step of 1.05 (half a step a quad), ±3.2 across, 0.02 down a layer,
/// scrolling with `+0xb8` (down 2.5 / 60 / 1.05 a frame), fading with its opening `+0xac`; the red ±1.6 with half
/// steps, the scrolls 3 and 4, the rails 1.6 (`$gp+0x1484..0x14ec`).
pub fn draw_0a9d(w: &mut MobyWorld, id: usize, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &mut w.mobys[id];
    let phase = m.pv_f(0xb8) - 2.5 * 0.016_666_668 / 1.05;
    m.set_pv_f(0xb8, phase);
    let (row0, row1) = (m.rows[0], m.rows[1]);
    let beam = Beam {
        fx: 0x3f + m.pv_i(0xb4) as usize,
        base: add(m.pv_v(0x60), set_len(row0, -1.05)),
        along: set_len(row0, 1.05),
        step: 0.5,
        half: set_len(row1, -3.2),
        rail: set_len(row1, 1.6),
        dz: -0.02,
        red_half: 1.6,
        shrink: 0.5,
        scroll: [3.0, 4.0],
        phase,
        fade: m.pv_f(0xac),
    };
    draw_beam(&beam, out);
}

/// 0x0a9d / 0x0aa7 (`0x3d33a0`): a gate or hatch the launcher 0x0aae works, between its place (kept at `+0x60` /
/// `+0x70`) and that place plus `+0x80` turned by its angles, its angles plus `+0x90`. Set up (0) it is open for
/// good (2, `+0xa8`) when its mission is done or its uid counted, else shut (1). Poked (`+0xbc`) it opens over `+0xa4`
/// ticks (2: `t` `+0xac`, eased `(1 − cos πt)/2`), holds `+0xaa` ticks (3: `+0xb0`; while poked, longer), closes (4;
/// poked, it opens again). Ratchet in its cuboid `+0xa0` opens it for good and counts its uid. It carries Ratchet
/// (`0x318560`, the block `+0x20`); a 0x0a9d open draws its light ([`draw_0a9d`]); with `+0xa9` its pokes last a
/// tick.
pub fn update_gate(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (old, old_rot) = (w.mobys[id].pos, w.mobys[id].rot);
    let inside = ctx.level.in_cuboid(w.mobys[id].pv_i(0xa0), ctx.hero.pos);
    let m = &mut w.mobys[id];
    let pose = |m: &mut crate::moby::Moby| {
        let t = m.pv_f(0xac);
        let f = (1.0 - (t * PI).cos()) * 0.5;
        let r = rows_of(m.pv_v(0x70));
        let o = scale(m.pv_v(0x80), f);
        m.pos = add(m.pv_v(0x60), add(add(scale(r[0], o[0]), scale(r[1], o[1])), scale(r[2], o[2])));
        for k in 0..3 {
            let home = m.pv_f(0x70 + 4 * k);
            m.rot[k] = lerp_rot(home, add_rot(home, m.pv_f(0x90 + 4 * k)), f);
        }
    };
    let duration = m.pv_f(0xa4);
    match m.state {
        0 => {
            let (pos, rot) = (m.pos, m.rot);
            m.set_pv_v(0x60, pos);
            m.set_pv_v(0x70, rot);
            m.set_pv_f(0xac, 0.0);
            m.set_pv_f(0xb8, 0.0);
            let done = ctx.game.missions.get(ctx.game.level.max(0) as usize * 0x10 + m.mission as usize).is_some_and(|b| b & 0x80 != 0);
            if !done && ctx.game.uid_count(m.uid) == 0 {
                m.set_state(1, -1);
            } else {
                m.set_pv_b(0xa8, 1);
                m.set_state(2, -1);
            }
        }
        1 => {
            if m.timer != 0 { m.set_state(2, -1); }
            if inside {
                ctx.game.count_uid(m.uid);
                m.set_pv_b(0xa8, 1);
                m.set_state(2, -1);
            }
        }
        2 => {
            let t = (m.pv_f(0xac) + 1.0 / duration).clamp(0.0, 1.0);
            m.set_pv_f(0xac, t);
            pose(m);
            if 1.0 <= t {
                m.set_pv_i(0xb0, m.pv_s(0xaa) as i32);
                m.set_state(3, -1);
            }
        }
        3 => {
            if inside {
                ctx.game.count_uid(m.uid);
                m.set_pv_b(0xa8, 1);
            }
            if m.pv_b(0xa8) == 0 && m.timer == 0 {
                let mut t = m.pv_i(0xb0);
                if crate::moby::countdown(&mut t) { m.set_state(4, -1); }
                m.set_pv_i(0xb0, t);
            }
        }
        4 => {
            let t = (m.pv_f(0xac) - 1.0 / duration).clamp(0.0, 1.0);
            m.set_pv_f(0xac, t);
            pose(m);
            if t <= 0.0 { m.set_state(1, -1); }
            if m.timer != 0 { m.set_state(2, -1); }
            if inside {
                m.set_pv_b(0xa8, 1);
                m.set_state(2, -1);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    let state = m.state;
    if m.o_class == 0x0a9d && (2..=4).contains(&state) { w.register_draw(id); }
    if state < 5 {
        let (pos, yaw) = (w.mobys[id].pos, w.mobys[id].rot[2]);
        carry_hero(ctx, id, old, old_rot[2], pos, yaw);
    }
    let m = &mut w.mobys[id];
    if m.pv_b(0xa9) != 0 { m.timer = 0; }
}

/// `0x338b68(step, m, path, &pos, i, dir)`: along the path from its point `i` toward the next (`dir` 1: past the
/// last the first; else back, before the first the last): `pos` put on that leg at its distance along it + `step`,
/// on to the next leg past its end. Returns the point the leg starts at.
fn path_step(points: &[[f32; 4]], step: f32, pos: &mut V3, i: usize, dir: i32) -> usize {
    let n = points.len();
    let next = |i: usize| -> usize {
        if i == n - 1 { if 1 <= dir { 0 } else { n - 2 } } else if i == 0 { if 1 <= dir { 1 } else { n - 1 } } else { (i as i32 + dir) as usize }
    };
    let p = |k: usize| -> V3 { let q = points[k]; [q[0], q[1], q[2]] };
    let mut i = i;
    let j = next(i);
    let leg = sub(p(j), p(i));
    let l = crate::hero::physics::len(leg);
    let mut d = set_len(leg, 1.0);
    let mut t = crate::hero::physics::dot(sub(*pos, p(i)), d) + step;
    if l < t {
        d = set_len(sub(p(next(j)), p(j)), 1.0);
        t -= l;
        i = j;
    }
    *pos = add(p(i), set_len(d, t));
    i
}

/// 0x0a9e / 0x0a9f (`0x3d3b20`, sent by 0x0aae): at 0.95 of its class's scale, along the path `+0x74` (from the
/// point `+0x78`) at its launcher's `+0x20` a second; going down, its launcher's third moby (`+0x0c`) is poked (and
/// the first time, a sound); at the path's last point that poke is cleared and it is gone. It carries Ratchet.
///
/// Not ported: its sounds (`0x356978`).
pub fn update_sent(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class_scale = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let launcher = w.mobys[id].pv_i(0x70) as usize;
    let Some(lm) = w.mobys.get(launcher) else { return };
    let (speed, far) = (lm.pv_f(0x20), lm.pv_i(0xc));
    let Some(points) = usize::try_from(w.mobys[id].pv_i(0x74)).ok().and_then(|p| ctx.level.paths.get(p)).cloned() else { return };
    if points.len() < 2 { return; }
    let m = &mut w.mobys[id];
    m.scale = class_scale * 0.95;
    let (old, yaw) = (m.pos, m.rot[2]);
    m.set_pv_v(0x60, old);
    let mut pos = m.pos;
    let i = path_step(&points, speed * 0.016_666_668, &mut pos, m.pv_i(0x78).max(0) as usize, 1);
    m.pos = pos;
    m.set_pv_i(0x78, i as i32);
    let poke = |w: &mut MobyWorld, v: u8| if let Some(f) = usize::try_from(far).ok().and_then(|f| w.mobys.get_mut(f)) { f.timer = v; };
    if pos[2] < old[2] {
        m.set_pv_s(0x7c, m.pv_s(0x7c) | 0x80);
        poke(w, 1);
    }
    if i == points.len() - 1 {
        poke(w, 0);
        w.delete(id);
        return;
    }
    w.rebuild(id, &*ctx.host);
    carry_hero(ctx, id, old, yaw, pos, yaw);
}

/// 0x0aae (`0x3d3cf0`, 1 placed): the launcher. Poked (1) it pokes its first moby (`+0x04`, the gate) and for
/// `+0x24` ticks (2) every `+0x00` ticks sends a 0x0a9e or 0x0a9f (`0x3d3a18`) from the cuboid `+0x18` or `+0x1c`
/// (the pattern `0x297c70`: 0 the first of each, 1 the second of each, 2 the second class from the first cuboid) on
/// the path `+0x10` + 4·cuboid, poking its second moby (`+0x08`, the hatch) for 40 ticks each time; then (3) the gate's
/// poke and its group's cleared, waiting again (1). While it runs and Ratchet is in its cuboid `+0x38`, a camera
/// tweak (priority 5, `0x2d7468`) holds: pivot 6, firm, the stick off, facing −x once.
pub fn update_0aae(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const PATTERN: [i32; 16] = [0, 1, 2, 0, 0, 1, 0, 2, 0, 0, 1, 2, 0, 0, 1, 0];
    let poke = |w: &mut MobyWorld, at: i32, v: u8| if let Some(m) = usize::try_from(at).ok().and_then(|i| w.mobys.get_mut(i)) { m.timer = v; };
    let m = &mut w.mobys[id];
    match m.state {
        0 => {
            m.set_pv_i(0x34, 0);
            m.set_state(if m.pv_i(0x10) == -1 || m.pv_i(0x14) == -1 { 4 } else { 1 }, -1);
        }
        1 => {
            if m.timer != 0 {
                let gate = m.pv_i(4);
                m.set_pv_i(0x28, 0);
                m.timer = 0;
                m.set_pv_i(0x3c, 0);
                m.set_pv_i(0x2c, m.pv_i(0x24));
                m.set_state(2, -1);
                poke(w, gate, 1);
            }
        }
        2 => {
            let mut t = m.pv_i(0x28);
            let due = crate::moby::countdown(&mut t);
            m.set_pv_i(0x28, t);
            if due {
                let k = m.pv_i(0x3c);
                match PATTERN.get(k as usize).copied().unwrap_or(-1) {
                    0 => { m.set_pv_s(0x30, 0); m.set_pv_s(0x32, 0); }
                    1 => { m.set_pv_s(0x30, 1); m.set_pv_s(0x32, 1); }
                    2 => { m.set_pv_s(0x30, 0); m.set_pv_s(0x32, 1); }
                    _ => {}
                }
                m.set_pv_i(0x3c, k + 1);
                let (sel, kind) = (m.pv_s(0x30).clamp(0, 1) as usize, m.pv_s(0x32));
                let path = m.pv_i(0x10 + 4 * sel);
                let place = ctx.level.cuboid_place(m.pv_i(0x18 + 4 * sel));
                let (period, hatch) = (m.pv_i(0), m.pv_i(8));
                m.set_pv_i(0x28, period);
                if let Some((pos, rot)) = place {
                    let o = 0x0a9e + kind;
                    let scale = w.class(o).map_or(1.0, |c| c.scale) * 0.95;
                    let s = w.create(o);
                    let sm = &mut w.mobys[s];
                    (sm.drawn, sm.update_dist, sm.draw_dist, sm.state, sm.timer) = (1, 0xff, 0xff, 0, 0);
                    sm.scale = scale;
                    sm.pos = pos;
                    sm.rot = rot;
                    sm.set_pv_v(0x60, pos);
                    sm.set_pv_i(0x70, id as i32);
                    sm.set_pv_s(0x7c, sel as i16);
                    sm.set_pv_s(0x7e, -1);
                    sm.set_pv_i(0x74, path);
                    sm.set_pv_i(0x78, 0);
                    w.rebuild(s, &*ctx.host);
                }
                poke(w, hatch, 1);
            }
            let m = &mut w.mobys[id];
            if m.pv_i(0x28) < m.pv_i(0) - 0x28 {
                let hatch = m.pv_i(8);
                poke(w, hatch, 0);
            }
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x2c);
            let over = crate::moby::countdown(&mut t);
            m.set_pv_i(0x2c, t);
            if over { m.set_state(3, -1); }
        }
        3 => {
            let (gate, g) = (m.pv_i(4), m.group);
            m.set_state(1, -1);
            poke(w, gate, 0);
            if g != -1 {
                for i in usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default() { w.mobys[i].timer = 0; }
            }
        }
        _ => {}
    }
    // The camera: `+0x34` the request + 1 (0 none).
    let m = &w.mobys[id];
    let held = m.pv_i(0x34) - 1;
    let want = m.state == 2 && ctx.level.in_cuboid(m.pv_i(0x38), ctx.hero.pos);
    if want && held < 0 {
        if let Some(slot) = ctx.game.cam_tweaks.request(5) {
            let t = &mut ctx.game.cam_tweaks.slots[slot];
            t.dir = [-1.0, 0.0, 0.0];
            t.firm = true;
            t.pivot_h = 6.0;
            t.no_stick = true;
            t.face_once = true;
            w.mobys[id].set_pv_i(0x34, slot as i32 + 1);
        }
    } else if !want && 0 <= held {
        ctx.game.cam_tweaks.release(held as usize);
        w.mobys[id].set_pv_i(0x34, 0);
    }
}

/// 0x0a1e (`0x3bd370`, Oozla's snapping flower): open (1, sequence 0) until Ratchet lands on it (his ground moby,
/// grounded), then (2, sequence 1) `+0x50` seconds (`+0x60`; not while its rest `+0x70` runs) before it snaps (3,
/// sequence 2 at 1.2 times). As its frame passes 4 it is a target (mode 0x1000); Ratchet still on it and under half a
/// unit above it is caught (4, `+0x68`, its collision off): held in 0x6f, pulled 1/15 a tick toward 2 over it, still.
/// As the frame passes 46 the caught one is bitten — under 2 health dead (`0x2b31f0`), else hit for `+0x34` (`0x31d128`
/// / `0x31d378`) — and let go, a rest of 120 ticks; its sequence over, open again (1). It is never hit itself (`+0xa8`
/// −1).
///
/// Not ported: the pad's rumbles (`0x2cc438`), the snap's sound, the hold's camera (`0x2c7c68`).
pub fn update_0a1e(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class_scale = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let frame = ctx.host.frame_time(&w.mobys[id]);
    let seq = super::props::seq(w, id, ctx);
    let on = ctx.hero.ground_moby == Some(id);
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.set_state(1, -1);
            m.coll_disable = 3;
            m.mode &= !mode::TARGET;
            if seq != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 2); }
        }
        1 => {
            if on && ctx.hero.grounded_ticks != 0 {
                let m = &mut w.mobys[id];
                m.set_state(2, -1);
                m.set_pv_i(0x60, (m.pv_f(0x50) * 60.0) as i32);
                if seq != 1 { ctx.host.set_anim(&w.mobys[id], 1, 0, 10); }
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            let mut rest = m.pv_i(0x70);
            let rested = crate::moby::countdown(&mut rest);
            m.set_pv_i(0x70, rest);
            if rested {
                let mut t = m.pv_i(0x60);
                let snap = crate::moby::countdown(&mut t);
                m.set_pv_i(0x60, t);
                if snap {
                    m.set_state(3, -1);
                    if seq != 2 { ctx.host.set_anim(&w.mobys[id], 2, 0, 6); }
                    ctx.host.set_anim_speed(&w.mobys[id], 1.2);
                }
            }
        }
        3 => {
            let m = &mut w.mobys[id];
            if m.pv_f(0x74) < 4.0 && 4.0 < frame {
                m.mode |= mode::TARGET;
                m.set_pv_i(0x64, 600);
                if on && ctx.hero.height < 0.5 {
                    m.set_pv_i(0x68, 1);
                    m.coll_disable = u32::MAX;
                } else {
                    m.coll_disable = 0x8000_0000;
                }
                m.set_state(4, -1);
            }
        }
        4 => {
            let caught = w.mobys[id].pv_i(0x68) != 0;
            if w.mobys[id].pv_f(0x74) < 46.0 && 46.0 < frame {
                if caught {
                    if ctx.hero.hp < 2 {
                        ctx.hero.die();
                    } else if let Some(h) = ctx.hero_moby {
                        let dmg = w.mobys[id].pv_b(0x34) as f32;
                        let t = HitTemplate::new(dmg, Some(id), 1, [0.0, 0.0, 0.0, 5_627.987]);
                        let p = w.mobys[id].pos;
                        w.deliver_hit_any(h, &t, [p[0], p[1], p[2], 1.0]);
                    }
                    let m = &mut w.mobys[id];
                    m.set_pv_i(0x68, 0);
                    m.set_pv_i(0x70, 0x78);
                }
                let m = &mut w.mobys[id];
                m.coll_disable = 3;
                m.mode &= !mode::TARGET;
            }
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(1, -1);
                if seq != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 10); }
            } else if w.mobys[id].pv_i(0x68) != 0 {
                if ctx.hero.state != 0x6f { ctx.hero.request_state(0x6f, true); }
                let p = w.mobys[id].pos;
                let to = [p[0], p[1], p[2] + 2.0];
                let h = &mut ctx.hero;
                h.pos = add(h.pos, scale(sub(to, h.pos), 0.066_666_67));
                (h.vel, h.disp, h.eff, h.eff_v) = ([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
                let m = &mut w.mobys[id];
                m.set_pv_i(0x64, m.pv_i(0x64).min(0x3c));
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    m.hit_slot = -1;
    m.scale = class_scale;
    m.set_pv_f(0x74, frame);
}
/// 0x0ace (`0x3d42b0`): the Tractor Beam's seller at the end of the slide tube. The price is item 0x2e's record's
/// (`0x263820 + 0xe0·0x139596`, 1000 bolts). By state:
/// * **0**: the Tractor Beam had (`0x1a7b2e`) deletes it (`0x307440`); else 1, or with scene 0x27 seen (`0x13978c`)
///   3 when the bolts are under the price, else 4; its grid cells (`0x313ae0`) and its prop 0xd7e made (`0x3070d8`:
///   its update and draw distances, drawn).
/// * **1**: Ratchet in the cuboid `+0x120` queues the talk scene 0x27 (`0x2ea1f0` → `0x2f4ee8`) and starts it (`0x2ea338`
///   → `0x2f5030`): 2.
/// * **2** and **5**: hidden with its prop while the scene plays (scene mode, `0x1a8f40` = 2); after it (`0x367d78`) shown
///   again. 2: Ratchet put at the cuboid `+0x120` (`0x2b9b38(.., 0, 1)`), then 3 or 4 by the bolts. 5: its group's
///   timers set to 1 (`0x3127b8`) and, the Tractor Beam not had yet, the bolts paid, the bolt counter shown (`0x3032e8(0xb4)`),
///   the Tractor Beam his (`0x2e99e8` / `0x3186d0`: L00's `0x2f46e0` / `0x3227d8`), the message 0x1226 (`0x31d0e0`), the
///   help 0x9bc in slot 0x13 (`0x2ea578`); the prop and the seller gone.
/// * **3** (short of bolts; more than the price → 4) and **4** (enough; fewer → the counter shown, 3): while Ratchet can
///   talk to it ([`can_talk`], 2.5) the bolt counter shows and the second message box the price in the text 0x1300
///   (3) or 0x12fe (4) (`0x325830`); in 4 a △ press (`0x138324 & 0x10`) queues the sale's scene 0x28 and starts it: 5.
///
/// Not ported: the talk record it marks (`0x325980(0x3259d8(m))`: the level's talk list `0x1c5d20` / `0x28ead0`, its
/// record `0x139a34 + 0x10·i`; the missions' kinds 4 and 5 read it), its prop's set-up and joint (`0x309b28`,
/// `0x315f20`, `0x31a700(m, prop, 2, 0)`), its head and body turning to Ratchet (the joint aims `0x31a0e0` with the
/// timers `+0x114` / `+0x118` / `+0x11e` and its idle sequence `0x311358`: no joint aims in the port), the scene flag
/// `0x1b22a8`, the camera `0x42d968` of `+0x12c` and the autosave `0x303030` after the sale (as the Dynamo's).
pub fn update_0ace(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::hero::items::TRACTOR;
    const ZONE: usize = 0x120;
    const PROP: usize = 0x128;
    const PROP_CLASS: i16 = 0x0d7e;
    let price = ctx.level.items.get(TRACTOR as usize).map_or(0, |r| r.price);
    let had = ctx.game.items.get(TRACTOR as usize).is_some_and(|&b| b != 0);
    let bolts = ctx.game.bolts as i32;
    let prop = |w: &MobyWorld| w.mobys[id].pv_link(PROP).filter(|&c| w.mobys.get(c).is_some_and(|m| m.state < 0x80));
    let show = |w: &mut MobyWorld, on: bool| {
        for i in std::iter::once(id).chain(prop(w)) {
            let m = &mut w.mobys[i];
            if on { m.drawn = 1; m.mode &= !1; } else { m.drawn = 0; m.mode |= 1; }
        }
    };
    match w.mobys[id].state {
        0 => {
            if had { return w.delete(id); }
            let next = if ctx.game.scenes_seen.get(0x27).copied().unwrap_or(false) { if bolts < price { 3 } else { 4 } } else { 1 };
            w.mobys[id].set_state(next, -1);
            w.rebuild(id, &*ctx.host);
            if w.class(PROP_CLASS).is_some() {
                let c = w.create(PROP_CLASS);
                let (pos, ud, dd) = (w.mobys[id].pos, w.mobys[id].update_dist, w.mobys[id].draw_dist);
                let m = &mut w.mobys[c];
                m.pos = pos;
                (m.update_dist, m.draw_dist, m.drawn) = (ud, dd, 1);
                w.mobys[id].set_pv_i(PROP, c as i32);
            }
        }
        1 => {
            if !ctx.level.in_cuboid(w.mobys[id].pv_i(ZONE), ctx.hero.pos) { return; }
            if ctx.game.scenes.queue.push(0x27) {
                ctx.game.scenes.start_queue(ctx.hero.hp);
                w.mobys[id].set_state(2, -1);
            }
        }
        2 | 5 => {
            let state = w.mobys[id].state;
            if ctx.game.scenes.mode == crate::scene::SCENE { show(w, false); }
            if ctx.game.scenes.in_scene() { return; }
            show(w, true);
            if state == 2 {
                if let Some((pos, rot)) = ctx.level.cuboid_place(w.mobys[id].pv_i(ZONE)) {
                    ctx.hero.teleport(pos, rot);
                    ctx.hero.request_state(0, true);
                }
                w.mobys[id].set_state(if bolts < price { 3 } else { 4 }, -1);
                return;
            }
            let group = w.mobys[id].group as i32;
            w.poke_group(group, 1);
            if had { return; }
            ctx.game.bolts = (bolts - price).max(0) as u32;
            ctx.game.show_bolts = Some(0xb4);
            ctx.game.get_item(TRACTOR);
            let held = ctx.game.held;
            ctx.hero.items.ask(held, TRACTOR);
            ctx.game.messages.push(crate::msgbox::Request::Text { text: 0x1226, ticks: None });
            ctx.game.help(0x9bc, 0x13);
            if let Some(c) = prop(w) { w.delete(c); }
            w.delete(id);
        }
        3 | 4 => {
            let state = w.mobys[id].state;
            if state == 3 && price < bolts { return w.mobys[id].set_state(4, -1); }
            if state == 4 && bolts < price {
                ctx.game.show_bolts = Some(0xb4);
                return w.mobys[id].set_state(3, -1);
            }
            if !can_talk(w, id, ctx, 2.5, 10) { return; }
            ctx.game.show_bolts = Some(0xb4);
            let text = if state == 3 { 0x1300 } else { 0x12fe };
            ctx.game.messages.push(crate::msgbox::Request::SecondFormatted { text, n: price, ticks: Some(1) });
            if state == 4 && ctx.hero.items.pressed & crate::pad::button::TRIANGLE != 0 && ctx.game.scenes.queue.push(0x28) {
                ctx.game.scenes.start_queue(ctx.hero.hp);
                w.mobys[id].set_state(5, -1);
            }
        }
        _ => {}
    }
}

/// `0x3258c0(r, moby, n)`: Ratchet can talk to `id`: not in states 0x1c / 0x31, with health, on the ground (`0x18a160`
/// ticks, no air ticks `0x18a16e`), not in movement group 3, the game in play (`$gp−0x60b0`), within 2·r of it (3D),
/// the level running `n` frames or more (`0x1b2088`, L00's `0x1b1948`).
fn can_talk(w: &MobyWorld, id: usize, ctx: &Ctx, r: f32, n: i32) -> bool {
    let h = &*ctx.hero;
    if h.state == 0x1c || h.state == 0x31 || h.hp == 0 || h.grounded_ticks == 0 || h.air_ticks != 0 || h.group == 3 { return false; }
    if ctx.game.scenes.mode != crate::scene::PLAY { return false; }
    if r + r < crate::hero::physics::dist(w.mobys[id].pos, h.pos) { return false; }
    n <= w.frame
}

/// 0x061c (`0x3a6fb8`, 22 placed): a rider on a path of the swamp (pvars: `+0x50` the path, `+0x54` its height a tick,
/// `+0x58` where it starts (a share of the points), `+0x5c` where it is, `+0x60` / `+0x68` its velocity when knocked off,
/// `+0x70` the path loops, `+0x74` its step a tick, `+0x78` its sinking ticks). By state:
/// * **0**: no path (−1) deletes it; else the hits counted (`gp+0x834`) cleared, the step 6 a second (`gp+0x82c`)
///   over the first segment's length, the path looped when that and the length from the first point to the last differ
///   by under 0.1 (as the game compares them), the start `+0x58` · the points; 1, updated at any distance, its scale
///   × 2 (`gp+0x830`).
/// * **1**: its pose on the path (crate::paths::pose: the place, the pitch halved, the yaw), raised by `+0x54`, the
///   step on (wrapping past the points); with its sequence having wrapped (`+0x60 & 2`) sequence 0 rising, else 1 (a
///   10-tick blend: the animation's trigger sounds go with them). A hit with the flags 0x10000 (`0x313d88`) knocks it
///   off: 0.05 a tick along its facing (`gp+0x82c` / 120 through its rows), 0.05 down; the fourth hit of the level on
///   Oozla shows the message 0x1233 once (`0x1a7a75`); 2.
/// * **2**: its first tick plays class sound 0 (`0x320328` = L00 `0x32a7c8`, `0x356978`); it flies (the velocity falling 0.003 a
///   tick, slowing to 0.98 across, its pitch easing to π), its animation at half speed; within 0.5 of the ground
///   under it (`0x312c20` = L00 `0x31ce20`) it sinks for 30 ticks: 3.
/// * **3**: its first tick plays class sound 1; it sinks at its fall's speed; then it is gone.
///
/// Not ported: the splash (`0x317148`: the billows, the splash drops, the light), the break's debris and drops
/// (`0x3a7428`) and the drop `0x31d020(0, rider, 1)`, the half animation speed of state 2, the call `0x356ad8(1, 0, 0)`
/// with the hint, and the hint flag's save.
pub fn update_061c(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const STEP: f32 = 6.0;
    const SCALE: f32 = 2.0;
    let path_of = |ctx: &Ctx, i: i32| usize::try_from(i).ok().and_then(|i| ctx.level.paths.get(i)).filter(|p| 2 <= p.len()).cloned();
    let state = w.mobys[id].state;
    match state {
        0 | 1 => {
            let Some(points) = path_of(ctx, w.mobys[id].pv_i(0x50)) else {
                if state == 0 { return w.delete(id); }
                return;
            };
            let n = points.len();
            if state == 0 {
                ctx.game.rider_hits = 0;
                let p3 = |k: usize| [points[k][0], points[k][1], points[k][2]];
                let first = crate::hero::physics::dist(p3(0), p3(1));
                let whole = crate::hero::physics::dist(p3(0), p3(n - 1));
                let class_scale = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
                let m = &mut w.mobys[id];
                m.set_pv_f(0x74, STEP * 0.016_666_668 / first);
                m.set_pv_i(0x70, ((first - whole).abs() < 0.1) as i32);
                m.set_pv_f(0x5c, m.pv_f(0x58) * n as f32);
                m.set_state(1, -1);
                m.update_dist = 0xff;
                m.scale = class_scale * SCALE;
            }
            let m = &mut w.mobys[id];
            let z0 = m.pos[2];
            let (pos, pitch, yaw, _) = crate::paths::pose(&points, m.pv_f(0x5c), m.pv_i(0x70) != 0);
            m.pos = pos;
            m.rot[1] = pitch * 0.5;
            m.rot[2] = yaw;
            m.pos[2] += m.pv_f(0x54);
            let mut t = m.pv_f(0x5c) + m.pv_f(0x74);
            if (n as f32) < t { t -= n as f32; }
            m.set_pv_f(0x5c, t);
            if let Some(a) = ctx.host.anim(&w.mobys[id]) {
                if a.flags & 2 != 0 {
                    let rising = z0 < w.mobys[id].pos[2];
                    if rising && a.seq_b != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 10); }
                    if !rising && a.seq_b != 1 { ctx.host.set_anim(&w.mobys[id], 1, 0, 10); }
                }
            }
            // `0x313d88(rider, 0x10000, 0)`: a hit without those flags is dropped.
            let Some(h) = w.hit_message(id) else { return };
            if h.tmpl.flags & 0x0001_0000 == 0 { return w.clear_hit(id); }
            let m = &mut w.mobys[id];
            let push = [STEP * 0.008_333_334, m.pv_f(0x64), m.pv_f(0x68)];
            let v: V3 = std::array::from_fn(|k| push[0] * m.rows[0][k] + push[1] * m.rows[1][k] + push[2] * m.rows[2][k]);
            m.set_pv_v(0x60, [v[0], v[1], -0.05]);
            ctx.game.rider_hits += 1;
            if 3 < ctx.game.rider_hits && ctx.game.level == 1 && !ctx.game.rider_hint {
                ctx.game.rider_hint = true;
                ctx.game.messages.push(crate::msgbox::Request::Text { text: 0x1233, ticks: None });
            }
            w.mobys[id].set_state(2, -1);
        }
        2 => {
            if w.mobys[id].first() { ctx.host.play_sound(&w.mobys[id], 0, 0); }
            let m = &mut w.mobys[id];
            let v = m.pv_v(0x60);
            m.pos = add(m.pos, v);
            m.set_pv_v(0x60, [v[0] * 0.98, v[1] * 0.98, v[2] - 0.003]);
            m.rot[0] = add_rot(-PI, (m.rot[0] + PI) * 0.95);
            let pos = m.pos;
            if crate::moby::ground_under(ctx, pos).is_some_and(|g| (g - pos[2]).abs() < 0.5) {
                let m = &mut w.mobys[id];
                m.set_pv_i(0x78, 0x1e);
                m.set_state(3, -1);
            }
        }
        3 => {
            if w.mobys[id].first() { ctx.host.play_sound(&w.mobys[id], 1, 0); }
            let m = &mut w.mobys[id];
            m.pos[2] += m.pv_f(0x68);
            let mut t = m.pv_i(0x78);
            let out = crate::moby::countdown(&mut t);
            m.set_pv_i(0x78, t);
            if out { w.delete(id); }
        }
        _ => {}
    }
}
