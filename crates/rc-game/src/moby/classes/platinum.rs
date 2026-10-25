//! The platinum bolt, class 0x046e (L01 `0x398790`; 2 placed on Oozla): one of the level's four (`+0x68`[0] its
//! number k, the save's byte `0x19b278 + 4·level + k`, crate::game::GameState::platinum).
//!
//! * **0**: found already (or no number) it goes; else its home `[0x10]` is where it is (`[0x1c]` its yaw), its angles
//!   at random, draw distance at least 0x40, 1.
//! * **1**: it floats 1 above its home bobbing 0.3 (`$gp+0x4e0` / `+0x4e4`, the sine of its fourth angle), turning
//!   120°, 40° and 20° a second about x, y and z and its bob 90° (`$gp+0x4ec..+0x4f8`). Ratchet within 3 across and 2
//!   up, alive and on foot (or in mode 3) takes it: with `[0xc]` (16 bits) set or Ratchet in state 0x31 at once (the
//!   message 0x124b, found), else the show (`0x398500`; with `[0x78]` set only once he lands, state 3).
//! * **The show** (`0x398500`): Ratchet held (0x6f) 2.5 out along its yaw facing it, its sound 0, the script camera
//!   snapped 1.25 out and 4 to the left looking back across (yaw − 90°, 1 up), its sequence 1, Ratchet's 0x7b; 2,
//!   `[0x6c]` 60.
//! * **2**: it sits at Ratchet (his place and angles); the camera (`0x399088`) swings with its sequence's frame (/170)
//!   from 20° one side of Ratchet to 20° the other, 5 out and 1 up, pitching down to 0.17, or between the cuboids
//!   `[4]` / `[8]` when it has both; at the end of its sequence 1 it goes back to 0 hidden; Ratchet's sequence back
//!   to idle (30) at its end; then 60 ticks and it is found: the message, Ratchet let go where he is, the camera
//!   behind him, deleted.
//!
//! Not ported: its four glow rings (`0x398e18`, `0x3311b0` by `[0x20..0x5c]`) and sparkles (`0x3992a0`), the count
//! `$gp+0x520`, the autosave (`0x303030`), `0x2dd378(10)`, the flags `0x1b1fa8` / `0x18c0df` / `0x1b22a8` and the
//! mirror bit `0x1a7a4d`.

use crate::hero::physics::{add, add_rot, dist2, sub_rot, V3};
use crate::moby::{Ctx, MobyWorld};
use std::f32::consts::{FRAC_PI_2, PI};

pub const CLASS: i16 = 0x046e;

/// `$gp+0x4e0` / `+0x4e4`: the height over the home and the bob.
const RISE: f32 = 1.0;
const BOB: f32 = 0.3;
/// `$gp+0x4ec..+0x4f8`: the turns in degrees a second (x, y, z, the bob's phase).
const TURN: [f32; 4] = [120.0, 40.0, 20.0, 90.0];
const DEG_S: f32 = 0.000_290_888_25;
/// The message `0x31d0e0(0x124b)`.
const FOUND_TEXT: i32 = 0x124b;

fn slot(ctx: &Ctx, k: i32) -> Option<usize> {
    let l = usize::try_from(ctx.game.level).ok()?;
    let k = usize::try_from(k).ok()?;
    Some(l * 4 + k).filter(|&i| i < ctx.game.platinum.len())
}

fn found(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    ctx.game.messages.push(crate::msgbox::Request::Text { text: FOUND_TEXT, ticks: None });
    if let Some(i) = slot(ctx, w.mobys[id].pv_i(0)) {
        if ctx.game.platinum[i] == 0 { ctx.game.platinum_found += 1; }
        ctx.game.platinum[i] = 1;
    }
}

/// `0x398790`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => {
            let k = w.mobys[id].pv_i(0);
            let new = k != -1 && slot(ctx, k).is_some_and(|i| ctx.game.platinum[i] == 0);
            if !new { return w.delete(id); }
            let rng = &mut ctx.game.rng;
            let a: [f32; 4] = std::array::from_fn(|_| rng.rand_angle());
            let m = &mut w.mobys[id];
            let (pos, yaw) = (m.pos, m.rot[2]);
            m.set_pv_v(0x10, pos);
            m.set_pv_f(0x1c, yaw);
            m.rot = [a[0], a[1], a[2]];
            // Its fourth angle (`+0xfc`, the bob's phase) kept at `[0x7c]`.
            m.set_pv_f(0x7c, a[3]);
            if m.draw_dist < 0x40 { m.draw_dist = 0x40; }
            m.state = 1;
        }
        1 => {
            let class_scale = w.class(CLASS).map_or(1.0, |c| c.scale);
            let m = &mut w.mobys[id];
            m.scale = class_scale;
            let phase = m.pv_f(0x7c);
            m.pos[2] = m.pv_f(0x18) + RISE + BOB * phase.sin();
            for (r, t) in m.rot.iter_mut().zip(TURN) { *r = add_rot(*r, t * DEG_S); }
            m.set_pv_f(0x7c, add_rot(phase, TURN[3] * DEG_S));
            let h = &*ctx.hero;
            if 3.0 <= dist2(m.pos, h.pos) || 2.0 <= (m.pos[2] - h.pos[2]).abs() || h.hp < 1 { return; }
            if m.pv_s(0xc) != 0 || h.state == 0x31 {
                found(w, id, ctx);
                return w.delete(id);
            }
            if m.pv_i(0x78) != 0 {
                m.mode |= 0x41;
                return m.set_state(3, -1);
            }
            show(w, id, ctx);
        }
        2 => held(w, id, ctx),
        3 if ctx.hero.air_ticks == 0 => show(w, id, ctx),
        _ => {}
    }
}

/// `0x398500`: the show's start.
fn show(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    use crate::script_camera::MODE_SNAP;
    let m = &w.mobys[id];
    let (home, yaw) = (m.pv_v(0x10), m.pv_f(0x1c));
    let at = add(home, [yaw.cos() * 2.5, yaw.sin() * 2.5, 0.0]);
    ctx.hero.teleport(at, [0.0, 0.0, add_rot(yaw, PI)]);
    ctx.hero.request_state(0x6f, false);
    ctx.host.play_sound(&w.mobys[id], 0, 0);
    let side = add_rot(yaw, FRAC_PI_2);
    let eye = add(home, [yaw.cos() * 1.25 + side.cos() * 4.0, yaw.sin() * 1.25 + side.sin() * 4.0, 1.0]);
    let euler = [0.0, 0.0, sub_rot(yaw, FRAC_PI_2)];
    ctx.game.cameras.camera_script(eye, euler, MODE_SNAP, 0, false, ctx.hero);
    ctx.game.cameras.camera_script_targets(Some(eye), Some(euler));
    let (hp, hr) = (ctx.hero.pos, ctx.hero.rot);
    let m = &mut w.mobys[id];
    (m.pos, m.rot) = (hp, hr);
    if ctx.host.anim(&w.mobys[id]).is_none_or(|a| a.seq_b != 1) { ctx.host.set_anim(&w.mobys[id], 1, 0, 0); }
    ctx.hero.anim_req = Some((0.0, 0x7b, 0));
    let m = &mut w.mobys[id];
    m.state = 2;
    m.set_pv_i(0x6c, 0x3c);
}

/// State 2 (and `0x399088`, the camera).
fn held(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    ctx.hero.request_state(0x6f, false);
    let (hp, hr) = (ctx.hero.pos, ctx.hero.rot);
    w.mobys[id].pos = hp;
    w.mobys[id].rot = hr;
    let a = ctx.host.anim(&w.mobys[id]);
    let playing = a.is_some_and(|a| a.seq_a == 1);
    camera(w, id, ctx, playing);
    if playing && a.is_some_and(|a| a.flags & 2 != 0) {
        if a.is_some_and(|a| a.seq_b != 0) { ctx.host.set_anim(&w.mobys[id], 0, 0, 0); }
        let m = &mut w.mobys[id];
        m.drawn = 0;
        m.mode |= 1;
    }
    if ctx.hero_view.flags & 2 != 0 { ctx.hero.anim_req = Some((30.0, 0, 0)); }
    if playing { return; }
    let mut t = w.mobys[id].pv_i(0x6c);
    let out = crate::moby::countdown(&mut t);
    w.mobys[id].set_pv_i(0x6c, t);
    if !out { return; }
    found(w, id, ctx);
    ctx.hero.request_state(0, false);
    ctx.game.cameras.camera_script2(crate::script_camera::RELEASE_CUT_BEHIND, ctx.hero);
    w.delete(id);
}

/// `0x399088`: the camera's targets by the share `t` of the bolt's sequence played (its frame / 170; 1 once over).
fn camera(w: &MobyWorld, id: usize, ctx: &mut Ctx, playing: bool) {
    let m = &w.mobys[id];
    let t = if playing { ctx.host.frame_time(m) } else { 170.0 } * 0.005_882_353;
    let places = (ctx.level.cuboid_place(m.pv_i(4)), ctx.level.cuboid_place(m.pv_i(8)));
    let (eye, euler): (V3, V3) = if let (Some((p0, r0)), Some((p1, r1))) = places {
        (std::array::from_fn(|k| p0[k] + (p1[k] - p0[k]) * t), std::array::from_fn(|k| add_rot(sub_rot(r1[k], r0[k]) * t, r0[k])))
    } else {
        let a = add_rot(t * 0.698_131_8 - 0.349_065_9, ctx.hero.rot[2]);
        let h = ctx.hero.pos;
        ([h[0] + a.cos() * 5.0, h[1] + a.sin() * 5.0, h[2] + 1.0], [0.0, t * -0.17, add_rot(a, PI)])
    };
    ctx.game.cameras.camera_script_targets(Some(eye), Some(euler));
}
