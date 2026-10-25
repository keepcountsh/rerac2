//! Oozla's breakables (L01 class updates; no names in any reference, so they go by o_class): the store's wares and
//! the boxes by the ship's pad. Each waits in state 1 for a hit with the flags 0x4010000 (`0x313d88`, the wrench and
//! blasts) while `+0x68`[0] = 0, then breaks in state 2: its pieces thrown (crate::moby::shards), its class sound 0
//! (`0x356978`), the level's bolts (crate::moby::bolts::level_drop). Most then wait hidden for 120 ticks in state 3
//! ([`linger`]) and go.
//!
//! * 0x0f02 (`0x410440`), 0x0f08 (`0x410688`), 0x0f2e (`0x4108d0`), 0x0f33 (`0x410ec8`): pieces by `0x31d130`
//!   ([`Pieces`]).
//! * 0x0f32 (`0x410b18`): every piece by `0x3210c0` ([`throw_all`]), the first flung up on its own ([`update_0f32`]).
//! * 0x0af7 (`0x3d7d20`): leaves its wreck, a 0x0f94 (`0x31d530`) ([`update_0af7`]).
//! * 0x10ef (`0x414388`): every piece by `0x3210c0`, gone at once ([`update_10ef`]).
//! * 0x10fa (`0x414498`): every piece but the box's middle, more bolts and a blast ([`update_10fa`]).
//! * 0x12dc (`0x379e00`): the count of them all ([`update_12dc`]).
//!
//! The count (`$gp−0x400`, crate::game::GameState::store_left): each one but 0x10fa adds one at its set-up and takes
//! one when hit. Fewer pieces when the moby was not drawn last frame or six are breaking (`0x1b1570` against
//! `$gp+0x4478`; the port counts the ones in state 3), or the frame rate is low (`$gp+0x309c` / `+0x30a0`: not
//! ported).
//!
//! Not ported: the dust of 0x0af7 (`0x3d7f60`: 6 or 15 puffs `0x31d2e0`), the sparkles of 0x0f32 / 0x0f33 (`0x31b4f0`,
//! `0x2e2388`), 0x10fa's glows at its start (`0x335530`), its sparks (`0x3165d8`, no damage) and its four puffs, the
//! wares' count `0x1b3468`, 0x12dc's sound (`0x356ad8(1, 0, 0)`) and `0x1a907c`.

use crate::hero::physics::{add, add_rot, scale, V3};
use crate::moby::{bolts, shards, Ctx, MobyWorld};
use std::f32::consts::PI;

/// The classes that keep the count.
const COUNTED: [i16; 7] = [0x0f02, 0x0f08, 0x0f2e, 0x0f32, 0x0f33, 0x0af7, 0x10ef];

/// `0x1b1570`: the breaks under way past which the pieces are fewer.
const BREAKING: usize = 6;

/// State 0: counted, state 1.
fn set_up(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    ctx.game.store_left += 1;
    w.mobys[id].set_state(1, -1);
}

/// State 1: a hit with the flags 0x4010000 while `+0x68`[0] = 0 takes one off the count (with `counted`) and breaks
/// it.
fn wait_hit(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, counted: bool) {
    if let Some(h) = w.hit_message(id) {
        if h.tmpl.flags & 0x0401_0000 != 0 && w.mobys[id].pv_i(0) == 0 {
            if counted { ctx.game.store_left -= 1; }
            w.mobys[id].set_state(2, -1);
        }
    }
    w.clear_hit(id);
}

/// The full break or the lesser one (module doc).
fn full(w: &MobyWorld, id: usize) -> bool {
    let breaking = w.mobys.iter().filter(|m| m.state == 3 && COUNTED.contains(&m.o_class)).count();
    w.mobys[id].drawn != 0 && breaking < BREAKING
}

/// State 3 (the timer at `+0x68`[`at`]): its first tick hides it without collision for 120 ticks; then it is
/// deleted.
fn linger(w: &mut MobyWorld, id: usize, at: usize) {
    if w.mobys[id].first() {
        let m = &mut w.mobys[id];
        m.drawn = 0;
        m.mode |= 1;
        m.has_collision = false;
        m.set_pv_i(at, 0x78);
        let old = m.grid_rect;
        m.grid_rect = crate::collision::mobys::GRID_NONE;
        w.grid.update(id as u16, old, crate::collision::mobys::GRID_NONE);
        w.changed = true;
    }
    let mut t = w.mobys[id].pv_i(at);
    let out = crate::moby::countdown(&mut t);
    w.mobys[id].set_pv_i(at, t);
    if out { w.delete(id); }
}

/// The pieces of a `0x31d130` break: the ranges across and up, the push along its `+0xd0` row, the masks (lesser,
/// full) and the bolts' multiple.
struct Pieces {
    h: (f32, f32),
    v: (f32, f32),
    push: f32,
    masks: (u32, u32),
    bolts: i32,
}

fn pieces_of(o_class: i16) -> Option<Pieces> {
    Some(match o_class {
        0x0f02 => Pieces { h: (1.0, 3.0), v: (2.0, 4.0), push: -0.1, masks: (0xf, 0x3ff), bolts: 4 },
        0x0f08 => Pieces { h: (3.0, 6.0), v: (3.0, 6.0), push: 0.0, masks: (0xd, 0x7f), bolts: 4 },
        0x0f2e => Pieces { h: (1.0, 3.0), v: (2.0, 5.0), push: -0.066_666_67, masks: (5, 0x1f), bolts: 1 },
        0x0f33 => Pieces { h: (1.0, 3.0), v: (2.0, 5.0), push: -0.066_666_67, masks: (2, 0x3f), bolts: 1 },
        _ => return None,
    })
}

/// 0x0f02 / 0x0f08 / 0x0f2e / 0x0f33.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let Some(p) = pieces_of(w.mobys[id].o_class) else { return };
    match w.mobys[id].state {
        0 => set_up(w, id, ctx),
        1 => wait_hit(w, id, ctx, true),
        2 => {
            let mask = if full(w, id) { p.masks.1 } else { p.masks.0 };
            let push = scale(w.mobys[id].rows[1], p.push);
            shards::break_into(w, id, ctx, p.h, p.v, mask, push, 0xff);
            bolts::level_drop(w, id, 0.0, p.bolts, ctx);
            ctx.host.play_sound(&w.mobys[id], 0, 0);
            w.mobys[id].set_state(3, -1);
        }
        3 => linger(w, id, 4),
        _ => {}
    }
}

/// `0x3210c0(moby, h, spin, gravity, 0xffff, sound, skip, flags, mask, 0xff, 0)`: the bolts (flag 1 clear: the level's
/// once; 0x10: nine times, else 0x20: twice), class sound `sound` (−1 none), then each piece of the class (`0x3078b0`)
/// in `mask` but `skip` thrown (`0x2d1520` mode 2): its gravity and spin drawn from their ranges, its velocity from
/// `h` (x from `h[0]..h[4]`, y `h[1]..h[5]`, z `h[2]..h[6]`; flags 2 / 4: x / y of either sign), a sixtieth a tick,
/// through its rows unless flag 8.
#[allow(clippy::too_many_arguments)]
fn throw_all(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, h: [f32; 8], spin: (f32, f32), gravity: (f32, f32), sound: i32, skip: i32, flags: u32, mask: u32) {
    if flags & 1 == 0 { bolts::level_drop(w, id, 0.0, 1, ctx); }
    if flags & 0x10 != 0 {
        bolts::level_drop(w, id, 0.0, 9, ctx);
    } else if flags & 0x20 != 0 {
        bolts::level_drop(w, id, 0.0, 2, ctx);
    }
    w.mobys[id].piece = None;
    if 0 <= sound { ctx.host.play_sound(&w.mobys[id], sound, 0); }
    let n = w.class(w.mobys[id].o_class).map_or(0, |c| c.pieces.len());
    for k in 0..n {
        if k as i32 == skip || mask >> k & 1 == 0 { continue; }
        let rng = &mut ctx.game.rng;
        let g = rng.randf(gravity.0, gravity.1);
        let s = rng.randf(spin.0, spin.1);
        let x = if flags & 2 == 0 { rng.randf(h[0], h[4]) } else { rng.randf_signed(h[0], h[4]) };
        let y = if flags & 4 == 0 { rng.randf(h[1], h[5]) } else { rng.randf_signed(h[1], h[5]) };
        let z = rng.randf(h[2], h[6]);
        let mut v: V3 = scale([x, y, z], 0.016_666_668);
        if flags & 8 == 0 {
            let r = w.mobys[id].rows;
            v = std::array::from_fn(|i| v[0] * r[0][i] + v[1] * r[1][i] + v[2] * r[2][i]);
        }
        shards::spawn_piece(w, id, ctx, s, g, k as u8, v, 0xff);
    }
}

/// 0x0f32: the lesser break is a `0x31d130` one (1..3 across, 2..5 up, pieces 0 and 4) with the level's bolts; the
/// full one throws every piece but the first (`0x1b1590`: x and y ±0.75..2.5, z 2..5; spin 90..360, gravity 5..10)
/// and flings the first up (spin 180..360, gravity 15; ±1/60 across, 1/12 up). Its timer is `+0x68`[8].
pub fn update_0f32(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => set_up(w, id, ctx),
        1 => wait_hit(w, id, ctx, true),
        2 => {
            if full(w, id) {
                throw_all(w, id, ctx, [0.75, 0.75, 2.0, 0.0, 2.5, 2.5, 5.0, 0.0], (90.0, 360.0), (5.0, 10.0), 0, 0, 6, 0xffff);
                let rng = &mut ctx.game.rng;
                let x = rng.randf(-1.0, 1.0) * 0.016_666_668;
                let y = rng.randf(-1.0, 1.0) * 0.016_666_668;
                let s = rng.randf(180.0, 360.0);
                shards::spawn_piece(w, id, ctx, s, 15.0, 0, [x, y, 0.083_333_336], 0xff);
            } else {
                shards::break_into(w, id, ctx, (1.0, 3.0), (2.0, 5.0), 0x11, [0.0; 3], 0xff);
                bolts::level_drop(w, id, 0.0, 1, ctx);
                ctx.host.play_sound(&w.mobys[id], 0, 0);
            }
            w.mobys[id].set_state(3, -1);
        }
        3 => linger(w, id, 8),
        _ => {}
    }
}

/// 0x0af7: mode 0x800 at its set-up; its break plays its sound, drops the level's bolts and leaves a 0x0f94 in its
/// place (`0x31d530(moby, 0x0f94, 0)`: its place, angles, light and scale ratio, drawn, draw distance 0xff, update
/// distance 0, mode 0).
pub fn update_0af7(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const WRECK: i16 = 0x0f94;
    match w.mobys[id].state {
        0 => {
            w.mobys[id].mode |= 0x800;
            set_up(w, id, ctx);
        }
        1 => wait_hit(w, id, ctx, true),
        2 => {
            ctx.host.play_sound(&w.mobys[id], 0, 0);
            bolts::level_drop(w, id, 0.0, 1, ctx);
            if let Some(class_scale) = w.class(WRECK).map(|c| c.scale) {
                let m = &w.mobys[id];
                let ratio = w.class(m.o_class).map_or(1.0, |c| if c.scale != 0.0 { m.scale / c.scale } else { 1.0 });
                let (pos, rot, light, ambient) = (m.pos, m.rot, m.light, m.ambient);
                let c = w.create(WRECK);
                let n = &mut w.mobys[c];
                (n.pos, n.rot, n.scale) = (pos, rot, class_scale * ratio);
                (n.light, n.ambient) = (light, ambient);
                (n.drawn, n.draw_dist, n.update_dist, n.mode) = (1, 0xff, 0, 0);
                w.rebuild(c, ctx.host);
                w.changed = true;
            }
            w.mobys[id].set_state(3, -1);
        }
        3 => linger(w, id, 4),
        _ => {}
    }
}

/// 0x10ef: its break throws every piece (`0x1b18f0`: x and y −3..3, z 1..7; spin 90..360, gravity 10..20) with the
/// level's bolts and its sound, and it is deleted.
pub fn update_10ef(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => set_up(w, id, ctx),
        1 => wait_hit(w, id, ctx, true),
        2 => {
            throw_all(w, id, ctx, [-3.0, -3.0, 1.0, 0.0, 3.0, 3.0, 7.0, 0.0], (90.0, 360.0), (10.0, 20.0), 0, -1, 0, 0xffff);
            w.delete(id);
        }
        _ => {}
    }
}

/// 0x10fa (not counted): its break throws its pieces but 7..9 (mask 0xc7f, the ranges of 0x10ef's, no bolts of its
/// own), then three times the level's bolts 1.1 up, then the blast `0x3f9d38` at 21.5 × its scale up its frame (its
/// scale the size; two of each sprite, puffs and streaks, the flash, no shake), and it is deleted (`$gp+0x2950` −1).
pub fn update_10fa(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => w.mobys[id].set_state(1, -1),
        1 => wait_hit(w, id, ctx, false),
        2 => {
            throw_all(w, id, ctx, [-3.0, -3.0, 1.0, 0.0, 3.0, 3.0, 7.0, 0.0], (90.0, 360.0), (10.0, 20.0), 0, -1, 1, 0xc7f);
            bolts::level_drop(w, id, 1.1, 3, ctx);
            let m = &w.mobys[id];
            let at = add(m.pos, scale(m.rows[2], m.scale * 21.5));
            let b = crate::moby::blast::Blast { size: m.scale, bursts: [2; 3], puffs: 2, streaks: 2, light: true, shake: false };
            crate::moby::blast::burst(w, ctx, at, &b);
            w.delete(id);
        }
        _ => {}
    }
}

/// 0x12dc: the count set to 66 on the level's second frame; from the sixth, once none is left (and `0x1a7a77` not
/// set), the message 0x1233.
pub fn update_12dc(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 { w.mobys[id].update_dist = 0xff; }
    let g = &mut *ctx.game;
    if w.frame == 2 { g.store_left = 0x42; }
    if 5 < w.frame && g.store_left < 1 && !g.store_done {
        g.store_done = true;
        g.messages.push(crate::msgbox::Request::Text { text: 0x1233, ticks: None });
    }
}

/// 0x0a26 (`0x3bd948`, 29 placed): the swamp's plants, which the leapers and spitters stir (`+0xbc`) and push through.
/// * **0**: its scale × 0.8..1.1; with a mission (`+0xb0` ≠ −1) one gone since the checkpoint or broken before
///   (`0x21e944`, the uid's count) leaves its stump (0x0a29) and goes; else `+0x68`[0x50] > 0 plants more along its
///   facing every `[0x54]` up to that length (`0x3bd898`: 0x0a26s in state 1, any yaw); 1.
/// * **1**: a hit with the flags 0x4010000 (`0x313d88`) and some damage breaks it: its sound 0, three leaves 0x0a28
///   about each of its joints 0..3 (120° apart from a random yaw, each joint's 0.85 the last's size) and three tops
///   0x0a27 at joint 4 ([`throw_plant_piece`]), its stump, its drop (`0x310b28(moby, 0, 3)`), the uid's count up once
///   more, deleted. Unhit: stirred it plays sequence 1 (a 10-tick blend), and back in sequence 0 (60) once that
///   wraps.
pub fn update_0a26(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 {
        let k = ctx.game.rng.randf(0.8, 1.1);
        let m = &mut w.mobys[id];
        m.scale *= k;
        let (mission, uid) = (m.mission, m.uid);
        let gone = mission != 0xff
            && (ctx.game.uid_dead.get(uid.max(0) as usize).copied().unwrap_or(0) != 0 || ctx.game.uid_count(uid) != 0);
        if gone {
            leave(w, id, ctx, 0x0a29);
            return w.delete(id);
        }
        let (length, step) = (m.pv_f(0x50), m.pv_f(0x54));
        if 0.0 < length {
            let f = length / step;
            let d = scale(m.rows[0], f);
            let mut at = add(m.pos, d);
            let mut t = f;
            while t < length {
                plant(w, id, ctx, at);
                t += f;
                at = add(at, d);
            }
        }
        w.mobys[id].state = 1;
        return;
    }
    let hit = w.hit_message(id).filter(|h| h.tmpl.flags & 0x0401_0000 != 0);
    w.clear_hit(id);
    if hit.is_some_and(|h| 0.0 < h.tmpl.damage) {
        let m = &w.mobys[id];
        let ratio = w.class(m.o_class).map_or(1.0, |c| if c.scale != 0.0 { m.scale / c.scale } else { 1.0 });
        ctx.host.play_sound(&w.mobys[id], 0, 0);
        let rot = w.mobys[id].rot;
        let mut size = ratio;
        for j in 0..4 {
            let at = ctx.host.joint(&w.mobys[id], j).unwrap_or(w.mobys[id].pos);
            let mut yaw = ctx.game.rng.rand_angle();
            for _ in 0..3 {
                if let Some(p) = throw_plant_piece(w, id, ctx, 0x0a28, at, [rot[0], rot[1], yaw]) { w.mobys[p].scale *= size; }
                yaw = add_rot(yaw, 2.094_395_2);
            }
            size *= 0.85;
        }
        let at = ctx.host.joint(&w.mobys[id], 4).unwrap_or(w.mobys[id].pos);
        let mut yaw = rot[2];
        for _ in 0..3 {
            if let Some(p) = throw_plant_piece(w, id, ctx, 0x0a27, at, [rot[0], rot[1], yaw]) { w.mobys[p].scale *= ratio; }
            yaw = add_rot(yaw, 2.094_395_2);
        }
        leave(w, id, ctx, 0x0a29);
        bolts::drop(w, id, 0, 3, ctx);
        ctx.game.bump_uid(w.mobys[id].uid);
        return w.delete(id);
    }
    let Some(a) = ctx.host.anim(&w.mobys[id]) else { return };
    if w.mobys[id].timer == 0 {
        if a.flags & 2 != 0 && a.seq_b != 0 { ctx.host.set_anim(&w.mobys[id], 0, 0, 0x3c); }
    } else {
        if a.seq_b != 1 { ctx.host.set_anim(&w.mobys[id], 1, 0, 10); }
        w.mobys[id].timer = 0;
    }
}

/// `0x31d530(moby, class, 0)`: a moby of `class` in `id`'s place (its angles, light and scale ratio), drawn, draw
/// distance 0xff, update distance 0, mode 0.
fn leave(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, class: i16) -> Option<usize> {
    let class_scale = w.class(class)?.scale;
    let m = &w.mobys[id];
    let ratio = w.class(m.o_class).map_or(1.0, |c| if c.scale != 0.0 { m.scale / c.scale } else { 1.0 });
    let (pos, rot, light, ambient) = (m.pos, m.rot, m.light, m.ambient);
    let c = w.create(class);
    let n = &mut w.mobys[c];
    (n.pos, n.rot, n.scale) = (pos, rot, class_scale * ratio);
    (n.light, n.ambient) = (light, ambient);
    (n.drawn, n.draw_dist, n.update_dist, n.mode) = (1, 0xff, 0, 0);
    w.rebuild(c, ctx.host);
    w.changed = true;
    Some(c)
}

/// `0x3bd898(plant, at)`: another 0x0a26 at `at` in state 1, its distances, pitch and roll, light, any yaw.
fn plant(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, at: V3) {
    let m = &w.mobys[id];
    let (update_dist, draw_dist, rot, light, ambient) = (m.update_dist, m.draw_dist, m.rot, m.light, m.ambient);
    let yaw = ctx.game.rng.randf(-PI, PI);
    let c = w.create(0x0a26);
    let n = &mut w.mobys[c];
    (n.update_dist, n.draw_dist, n.drawn, n.state) = (update_dist, draw_dist, 1, 1);
    n.pos = at;
    n.rot = [rot[0], rot[1], yaw];
    (n.light, n.ambient) = (light, ambient);
    w.rebuild(c, ctx.host);
    w.changed = true;
}

/// `0x3bde40(0, plant, class, at, angles, 0, 1, 0, 0)`: a leaf (0x0a28) or top (0x0a27) of plant `id` at `at`, state 1
/// ([`update_plant_piece`]): its scale ratio and light, update distance 0xff, draw distance 0x80, no collision. Its
/// pvars: `+0x00` the velocity, 0.7..1.8 a second at a random heading; `+0x18` its yaw's sway 0.013..0.032; `+0x30`
/// its life, 180..240 ticks; `+0x34` 1; `+0x38` its gravity (9..15 a leaf, 9..12 a top, /3600 a tick²); `+0x3c` its
/// least fall (a leaf −3..−1.5, a top −100..−200, a sixtieth a tick); `+0x40` the sway's phase; `+0x44` the pitch it
/// eases to (20° a leaf, ±90° a top).
fn throw_plant_piece(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, class: i16, at: V3, rot: V3) -> Option<usize> {
    let class_scale = w.class(class)?.scale;
    let m = &w.mobys[id];
    let ratio = w.class(m.o_class).map_or(1.0, |c| if c.scale != 0.0 { m.scale / c.scale } else { 1.0 });
    let (light, ambient) = (m.light, m.ambient);
    let rng = &mut ctx.game.rng;
    let phase = rng.rand_angle();
    let (pitch, gravity, least) = if class == 0x0a28 {
        let g = rng.randf(9.0, 15.0) * 0.000_277_777_8;
        (0.349_065_9, g, rng.randf(-3.0, -1.5))
    } else {
        let p = if rng.randi(2) == 0 { -1.570_796_5 } else { 1.570_796_5 };
        let g = rng.randf(9.0, 12.0) * 0.000_277_777_8;
        (p, g, rng.randf(-100.0, -200.0))
    };
    let life = rng.randf(180.0, 240.0) as i32;
    let heading = rng.rand_angle();
    let speed = rng.randf(0.7, 1.8) * 0.016_666_668;
    let sway = rng.randf(0.013_089_97, 0.031_997_71);
    let c = w.create(class);
    let n = &mut w.mobys[c];
    (n.pos, n.rot, n.scale, n.state, n.drawn) = (at, rot, class_scale * ratio, 1, 1);
    (n.light, n.ambient) = (light, ambient);
    (n.update_dist, n.draw_dist) = (0xff, 0x80);
    n.has_collision = false;
    n.pvars.resize(n.pvars.len().max(0x48), 0);
    n.set_pv_v(0, [heading.cos() * speed, heading.sin() * speed, 0.0]);
    n.set_pv_f(0x18, sway);
    n.set_pv_i(0x30, life);
    n.set_pv_f(0x34, 1.0);
    n.set_pv_f(0x38, gravity);
    n.set_pv_f(0x3c, least * 0.016_666_668);
    n.set_pv_f(0x40, phase);
    n.set_pv_f(0x44, pitch);
    w.rebuild(c, ctx.host);
    w.changed = true;
    Some(c)
}

/// 0x0a27 / 0x0a28 (`0x3be2d0`): a plant's piece. Its sway's phase on 2° a tick, its yaw on by `+0x18` × its sine, its
/// pitch eased 0.02 toward `+0x44`. Falling (state 1): its roll the phase's cosine × 10°, its velocity 0.98 across,
/// its fall `+0x38` more a tick down to `+0x3c`; meeting the level (a line from 1 above where it was, `0x2d3fe8` flags
/// 2) it stops there (state 2, 10 ticks, mode 0x2000), as at its life's end. State 2: its roll eased 0.1 to 10°, its
/// alpha 12.8 a tick left; gone at 0.
pub fn update_plant_piece(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    let was = m.pos;
    let mut t = m.pv_i(0x30);
    let out = crate::moby::countdown(&mut t);
    m.set_pv_i(0x30, t);
    match m.state {
        0 => return w.delete(id),
        1 => if out { settle(m) },
        _ => {
            if out { return w.delete(id); }
            m.alpha = (t as f32 * 12.8) as u8;
        }
    }
    let phase = add_rot(m.pv_f(0x40), 0.034_906_58);
    m.set_pv_f(0x40, phase);
    m.rot[2] = add_rot(m.rot[2], m.pv_f(0x18) * phase.sin());
    m.rot[0] += (m.pv_f(0x44) - m.rot[0]) * 0.02;
    if m.state == 2 {
        m.rot[1] += (0.174_532_95 - m.rot[1]) * 0.1;
    } else {
        m.rot[1] = phase.cos() * 0.174_532_95;
        let v = m.pv_v(0);
        let v = [v[0] * 0.98, v[1] * 0.98, v[2]];
        m.pos = add(m.pos, v);
        let fall = (v[2] - m.pv_f(0x38)).max(m.pv_f(0x3c));
        m.set_pv_v(0, [v[0], v[1], fall]);
    }
    let pos = m.pos;
    let Some(c) = ctx.coll else { return };
    let Some(hit) = crate::collision::coll_line(c, [was[0], was[1], was[2] + 1.0], pos, crate::collision::QueryFlags(2)) else { return };
    let m = &mut w.mobys[id];
    m.pos = hit.point;
    m.set_pv_v(0, [0.0; 3]);
    m.set_pv_f(0x38, 0.0);
    m.set_pv_v(0x10, [0.0; 3]);
    if m.state != 2 { settle(m); }
}

/// State 2 for 10 ticks, mode 0x2000.
fn settle(m: &mut crate::moby::Moby) {
    m.state = 2;
    m.set_pv_i(0x30, 10);
    m.mode |= 0x2000;
}

/// 0x0a29 (`0x3be550`): the stump; an empty update.
pub fn update_0a29(_w: &mut MobyWorld, _id: usize, _ctx: &mut Ctx) {}
