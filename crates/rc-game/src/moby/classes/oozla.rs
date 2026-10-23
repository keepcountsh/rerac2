//! Oozla's small objects (L01 class updates; no names in any reference, so they go by o_class).
//!
//! * 0x08fb (`0x41dee0`): sets mode 0x802 (no update; 0x800) and nothing else.
//! * 0x0f14 (`0x4108b0`): on its first tick state 1, update distance 0xff.
//! * 0x0a6a / 0x0a6b / 0x0aa0 / 0x0aa1 (`0x3ca620`): movers between two poses ([`update_mover`]).
//! * 0x0c58 (`0x3ed598`): moves once to a cuboid's place when poked ([`update_0c58`]).
//!
//! Not ported here: 0x0ab3 (`0x3d4098`, deleted once the level byte `0x1a7b2e` is set: not mapped), 0x0e89
//! (`0x406860`, the 0x0c58 mover started by the level bytes `0x1a7b13` / `0x1a7b32`), 0x0ca0 (`0x3ee1d0`, the
//! switch that pokes them: the use system, G-UI-001), 0x0a92 / 0x0a93 (`0x3cfcc0`, floating on the water height
//! of 0x0a17, G-REN-009).

use crate::hero::physics::{add, add_rot, scale, sub, sub_rot, V3};
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
