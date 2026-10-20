//! Aranos's small objects (L00 class updates of a few lines each; the classes have no names in any reference, so
//! they go by o_class).
//!
//! * 0x12f5 (`0x383b90`): deletes itself on its first update (a placeholder the level does not use).
//! * 0x0c8b (`0x3ec838`): the level's death height by where Ratchet is — 30 inside its cuboid (`+0x68`[0]), else 95
//!   (`0x1b1a68`).
//! * 0x0ac3 (`0x3d00a0`): waits for Ratchet within 6 across, then plays its sequence 1 (sound 0), then 2 once that
//!   wraps (sound 1); its colour word `+0x78` changes with each.
//! * 0x0ac5 / 0x0ac6 (`0x3d0888` / `0x3d0980`): when Ratchet enters the cuboid `+0x68`[0] (sound 0), slides 0.417 a
//!   tick backward / forward along its yaw for 100 ticks, then is deleted.

use crate::moby::{Ctx, MobyWorld};

/// The seq the moby's animation shows (`+0x43`), 0xff without one.
pub fn seq(w: &MobyWorld, id: usize, ctx: &Ctx) -> u8 { ctx.host.anim(&w.mobys[id]).map_or(0xff, |a| a.seq_b) }

/// `+0x60 & 2`: its sequence wrapped this tick.
pub fn wrapped(w: &MobyWorld, id: usize, ctx: &Ctx) -> bool { ctx.host.anim(&w.mobys[id]).is_some_and(|a| a.flags & 2 != 0) }

/// `0x31b5f0(m, seq, frame, blend)` unless the sequence already plays.
pub fn set_seq(w: &MobyWorld, id: usize, ctx: &mut Ctx, s: u8, frame: u8, blend: u8) {
    if seq(w, id, ctx) != s { ctx.host.set_anim(&w.mobys[id], s, frame, blend); }
}

/// 0x12f5.
pub fn update_12f5(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) { w.delete(id); }

/// 0x0c8b.
pub fn update_0c8b(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let v = w.mobys[id].pv_i(0);
    ctx.game.death_z = Some(if ctx.level.in_cuboid(v, ctx.hero.pos) { 30.0 } else { 95.0 });
}

/// 0x0ac3.
pub fn update_0ac3(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if m.state == 0 && crate::hero::physics::dist2(m.pos, ctx.hero.pos) < 6.0 {
        w.mobys[id].state = 1;
        set_seq(w, id, ctx, 1, 0, 0);
        ctx.host.play_sound(&w.mobys[id], 0, 0);
        w.mobys[id].has_collision = false;
    }
    if w.mobys[id].state == 1 && wrapped(w, id, ctx) {
        w.mobys[id].state = 2;
        set_seq(w, id, ctx, 2, 0, 0);
        ctx.host.play_sound(&w.mobys[id], 1, 0);
    }
}

/// 0x0ac5 (backward) / 0x0ac6 (forward).
pub fn update_0ac5(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) { slide(w, id, ctx, std::f32::consts::PI) }
pub fn update_0ac6(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) { slide(w, id, ctx, 0.0) }

fn slide(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, turn: f32) {
    let m = &w.mobys[id];
    if m.state == 0 && ctx.level.in_cuboid(m.pv_i(0), ctx.hero.pos) {
        let m = &mut w.mobys[id];
        m.state = 1;
        m.set_pv_i(4, 0);
        ctx.host.play_sound(&w.mobys[id], 0, 0);
    }
    if w.mobys[id].state == 1 {
        let m = &mut w.mobys[id];
        let (s, c) = (m.rot[2] + turn).sin_cos();
        m.pos[0] += c * 0.416_666_7;
        m.pos[1] += s * 0.416_666_7;
        let n = m.pv_i(4) + 1;
        m.set_pv_i(4, n);
        w.changed = true;
        if 100 < n { w.delete(id); }
    }
}

/// 0x0c4c (`0x3eb888`, the ships crossing the sky): drawn up to 2800, they fly 1.667 a tick down y from 3000 to −3000
/// and start over, the scale growing in from the ends (1 − (|y| − 2250) / 750, clamped 0..3) of the class's.
pub fn update_0c4c(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let base = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.draw_dist = 0xaf0;
        m.update_dist = 0xff;
        m.state = 1;
    }
    let _ = ctx;
    if m.state == 1 {
        m.pos[1] -= 1.666_666_7;
        let f = (1.0 - (750.0 - (3000.0 - m.pos[1].abs())) * 0.001_333_333_3).clamp(0.0, 3.0);
        m.scale = base * f;
        if m.pos[1] <= -3000.0 { m.pos[1] = 3000.0; }
    }
}

/// 0x0c63 (`0x3ec378`): a mission area — Ratchet in its cuboid (`+0x68`[4]) marks the mission (`+0xb0`) the area's;
/// the first time (`+0x68`[8] = 0, its bits clear) the sub-mission (`+0xb1`) is done: its bits set to the rank
/// (`0x2e4450`: 1 without the stats table) and its rewards (the spawn's tables `0x1c5400` XP, `0x1c54c0` bolts: not
/// read yet, none) banked.
pub fn update_0c63(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if !ctx.level.in_cuboid(m.pv_i(4), ctx.hero.pos) { return; }
    ctx.game.mission_area = m.mission;
    if m.pv_i(8) != 0 || ctx.game.mission(m.mission, m.sub_index) != 0 { return; }
    let (mission, sub) = (m.mission, m.sub_index);
    w.mobys[id].set_pv_i(8, 1);
    ctx.game.set_mission(mission, sub, 1);
}

/// 0x0ac8 (`0x3d0e00`): rocks back and forth — its pitch springs to a random 5..15° (k 0.00087, d 0.00087, at most
/// 0.00436), a new aim after 60..120 ticks or once there; the sound by the direction.
pub fn update_0ac8(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 {
        let a = ctx.game.rng.randf(5.0, 15.0);
        let t = ctx.game.rng.rand_range(0x3c, 0x78);
        let m = &mut w.mobys[id];
        m.set_pv_f(8, a);
        m.set_pv_i(0xc, t);
        m.state = 1;
        let s = if m.rot[0] < a * 0.017_453_294 { 0 } else { 1 };
        ctx.host.play_sound(&w.mobys[id], s, 0);
    }
    let m = &mut w.mobys[id];
    if m.state == 1 {
        let t = m.pv_f(8) * 0.017_453_294;
        let (mut x, mut v) = (m.rot[0], m.pv_f(0));
        crate::hero::physics::spring(t, 0.000_872_664_6, 0.000_872_664_6, 0.004_363_323, &mut x, &mut v);
        m.rot[0] = x;
        m.set_pv_f(0, v);
        let n = m.pv_i(0xc) - 1;
        m.set_pv_i(0xc, n.max(0));
        if n <= 0 || m.rot[0] == t { m.state = 0; }
    }
}

/// 0x0d1e (`0x3fa808`, the searchlights): the pitch turning 0.0524 a tick; two light cones out of it (`0x33f550`,
/// the engine's lights: not drawn yet).
pub fn update_0d1e(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    if w.mobys[id].state == 0 { w.mobys[id].set_state(1, -1); }
    let m = &mut w.mobys[id];
    if m.state == 1 { m.rot[1] = crate::hero::physics::add_rot(m.rot[1], 0.052_359_88); }
}

/// 0x12f9 (`0x383bb0`): a barrier held by two objects (the moby indices `+0x68`[4] / [8]): once either is gone (state
/// 0xfe / 0xfd) it breaks (debris `0x327498`: not drawn; sound 0) and is deleted.
pub fn update_12f9(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => w.mobys[id].set_state(1, -1),
        1 => {
            let (a, b) = (w.mobys[id].pv_i(4), w.mobys[id].pv_i(8));
            if a < 1 || b < 1 { return; }
            let alive = |i: i32| w.mobys.get(i as usize).is_some_and(|m| m.state != 0xfe && m.state != 0xfd);
            if alive(a) && alive(b) { return; }
            w.mobys[id].set_state(2, -1);
        }
        2 => {
            ctx.host.play_sound(&w.mobys[id], 0, 0);
            w.delete(id);
        }
        _ => {}
    }
}

/// The breakables (0x12fa the lamps `0x383ce0`, 0x1143 `0x414368`): a hit with the flags 0x4010000 (the wrench and
/// blasts) while `+0x68`[0] = 0 breaks them (state 2: the shards `0x32b580` and the burst `0x2ecfb0`, not drawn),
/// then they are deleted. The lamp's light (`0x33f550`) is the engine's.
pub fn update_breakable(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => w.mobys[id].set_state(1, -1),
        1 => {
            if let Some(h) = w.hit_message(id) {
                if h.tmpl.flags & 0x0401_0000 != 0 && w.mobys[id].pv_i(0) == 0 { w.mobys[id].set_state(2, -1); }
            }
            w.clear_hit(id);
        }
        2 => w.delete(id),
        _ => {}
    }
}

/// 0x132d (`0x383eb8`): the sun's lens flare (14 rings, `0x3842f8` drawn when the sun `+0x68` is on screen and not
/// hidden): not drawn yet; only its state.
pub fn update_132d(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.state = 1;
        m.update_dist = 0xff;
    }
}

/// `0x31c8e0(group, -1)`: the group's members not dead (state < 0x80).
pub fn group_alive(w: &MobyWorld, group: i32) -> usize {
    if group < 0 { return 0; }
    w.groups.get(group as usize).map_or(0, |g| g.iter().filter(|&&i| w.mobys.get(i as usize).is_some_and(|m| m.state < 0x80)).count())
}

/// 0x0919 (`0x3b9868`, a camera volume): while Ratchet is in its cuboid (`+0x68`[0]) and its group (`[0x40]`) has a
/// member alive, a camera tweak of priority 5 holds (`0x2d7468`) with the fields its pvars give (`0x3b9618`: `[4..0x1c]`
/// distance, pivot height, look height, pitch, field of view, `[0x1c]` / `[0x1e]` the springs and the stick, `[0x20..
/// 0x23]` the bytes, `[0x24]` / `[0x26]` the rest timer, `[0x30]` the direction; −1 / 0 leave the default).
pub fn update_0919(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let inside = ctx.level.in_cuboid(m.pv_i(0), ctx.hero.pos);
    let group = m.pv_i(0x40);
    let active = m.pv_i(0x28) != 0;
    if inside && !active {
        if group_alive(w, group) == 0 { return; }
        let Some(slot) = ctx.game.cam_tweaks.request(5) else { return };
        let m = &w.mobys[id];
        let t = &mut ctx.game.cam_tweaks.slots[slot];
        let f = |o: usize| m.pv_f(o);
        if f(4) != -1.0 { t.dist = f(4); }
        if f(8) != -1.0 { t.pivot_h = f(8); }
        if f(0xc) != -1.0 { t.look_h = f(0xc); }
        if f(0x10) != -1.0 { t.pitch = f(0x10); }
        if f(0x14) != -1.0 { t.fov = f(0x14); }
        if f(0x18) != -1.0 { t.g14 = f(0x18); }
        if m.pv_s(0x1c) != -1 { t.firm = m.pv_b(0x1c) != 0; }
        if m.pv_s(0x1e) != -1 { t.no_stick = m.pv_s(0x1e) != 0; }
        if m.pv_b(0x20) != 0 { t.priority = m.pv_b(0x20); }
        if m.pv_b(0x22) != 0 { t.face_once = true; }
        if m.pv_b(0x23) != 0 { t.face_rest = true; }
        if m.pv_s(0x24) != -1 { t.rest_t = m.pv_s(0x24); }
        if m.pv_s(0x26) != -1 { t.rest_len = m.pv_s(0x26); }
        if f(0x30) != -1.0 { t.dir = [f(0x30), f(0x34), f(0x38)]; }
        let m = &mut w.mobys[id];
        m.set_pv_i(0x2c, slot as i32);
        m.set_pv_i(0x28, 1);
        return;
    }
    if !active { return; }
    if !inside || group_alive(w, group) == 0 {
        ctx.game.cam_tweaks.release(w.mobys[id].pv_i(0x2c) as usize);
        w.mobys[id].set_pv_i(0x28, 0);
    }
}

/// 0x0c8c (`0x3ec880`): rooms swapped by where Ratchet is — entering its cuboid (`+0x68`[0x10]) hides the members of
/// its four groups (`[0..0x10]`, those with more than one member alive: mode |= 0x43, hidden and still), leaving it
/// shows them again.
pub fn update_0c8c(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let vol = w.mobys[id].pv_i(0x10);
    if w.mobys[id].state == 0 && ctx.level.in_cuboid(vol, ctx.hero.pos) { w.mobys[id].state = 1; }
    let swap = |w: &mut MobyWorld, hide: bool| {
        for k in 0..4 {
            let g = w.mobys[id].pv_i(4 * k);
            if group_alive(w, g) < 2 { continue; }
            let members = w.groups[g as usize].clone();
            for i in members {
                let m = &mut w.mobys[i];
                if hide { m.mode |= 0x43 } else { m.mode &= !0x43 }
            }
        }
        w.changed = true;
    };
    if w.mobys[id].state == 1 {
        swap(w, true);
        w.mobys[id].state = 2;
    }
    if w.mobys[id].state == 2 && !ctx.level.in_cuboid(vol, ctx.hero.pos) { w.mobys[id].state = 3; }
    if w.mobys[id].state == 3 {
        swap(w, false);
        w.mobys[id].state = 0;
    }
}

/// 0x0c22 (`0x3e8be8`, the tutorial's help): Ratchet in its cuboids (`+0x68`[0..8]) asks for the help texts —
/// [0] 0x9ab (slot 2), [1] 0x9ac / 0x9ad by the weapon (slots 3 / 4), [3] with its group [5] alive 0x9b0 / 0x9b2 /
/// 0x9b1 (7 / 9 / 8), [2] 0x9ae (5), [4] 0x9b3 (10), each until its slot counts a dismissal (the player stats that
/// also stop some, `0x18c7b0` / `0x18c828` / `0x18c840`, are not kept: 0); near a crate stack
/// (class 0x326 within 10, not broken) 0x9af (6) with health missing, 0x9e7 (0x3e) right at it. In [6] or [7] the
/// items are kept away (`0x18c310`).
pub fn update_0c22(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if m.state == 0 {
        let m = &mut w.mobys[id];
        m.state = 1;
        m.update_dist = 0xff;
        return;
    }
    let p = ctx.hero.pos;
    let g = &mut *ctx.game;
    let lv = ctx.level;
    let v = |k: usize| m.pv_i(4 * k);
    if g.help_count(6) == 0 && g.help_req.is_none() && g.help_state == 0 {
        for o in w.mobys.iter().filter(|o| o.o_class == 0x326 && o.state < 0x80) {
            let d = crate::hero::physics::dist(o.pos, p);
            if 10.0 <= d || o.state == 1 { continue; }
            if ctx.hero.hp < g.max_health {
                g.help(0x9af, 6);
            } else if d < 1.0 && g.help_count(0x3e) == 0 {
                g.help(0x9e7, 0x3e);
            }
        }
    }
    if lv.in_cuboid(v(0), p) && g.help_count(2) == 0 { g.help(0x9ab, 2); }
    if lv.in_cuboid(v(1), p) && g.help_count(3) == 0 && g.help_count(4) == 0 { g.help(0x9ac, 3); }
    if lv.in_cuboid(v(3), p) && g.help_count(7) == 0 && g.help_count(8) == 0 && g.help_count(9) == 0 && group_alive(w, v(5)) != 0 {
        // 0x9b0 (7) with the item 0x2a out, 0x9b1 (8) once the stat `0x18c840` counts: neither is kept here.
        g.help(0x9b2, 9);
    }
    if lv.in_cuboid(v(2), p) && g.help_count(5) == 0 { g.help(0x9ae, 5); }
    if lv.in_cuboid(v(4), p) && g.help_count(10) == 0 { g.help(0x9b3, 10); }
    g.no_item_area = lv.in_cuboid(v(6), p) || lv.in_cuboid(v(7), p);
}

/// 0x0ac0 (`0x3ce5b0`): raised 1.5 and its place kept (`+0x210`), it waits out `+0x208` ticks, then moves 1/12 a
/// tick along its yaw, back to the kept place every 0x208 ticks. With Ratchet in the cuboid `+0x220` it draws a ring
/// of 9 points 3 out (`0x3ceaa8`, the effects pass); in the cuboid `+0x20c` and touching the ground under it
/// (`0x2df8d8`) it plays Ratchet's sound 0x18 and throws sparks (not ported: the hero's sounds and the particles).
pub fn update_0ac0(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.pos[2] += 1.5;
        m.set_pv_i(0x200, 0);
        m.set_pv_v(0x210, m.pos);
        m.set_pv_i(0x204, 0);
        m.state = 1;
    }
    if m.state == 1 {
        let mut t = m.pv_i(0x208);
        if crate::moby::countdown(&mut t) { m.state = 2; }
        m.set_pv_i(0x208, t);
    }
    if m.state == 2 {
        let (s, c) = m.rot[2].sin_cos();
        m.pos[0] += c * 0.083_333_336;
        m.pos[1] += s * 0.083_333_336;
        let n = m.pv_i(0x204) + 1;
        m.set_pv_i(0x204, n);
        if 0x207 < n {
            m.set_pv_i(0x204, 0);
            m.pos = m.pv_v(0x210);
        }
        w.changed = true;
    }
}

/// `0x327898(m, o_class, update_dist)`: a moby of `o_class` in `m`'s place — its position and angles, its scale by
/// `m`'s share of its class's, drawn (`+0x31` = 1, draw distance 0xff), the mode cleared.
pub fn spawn_in_place(w: &mut MobyWorld, src: usize, o_class: i16, update_dist: u8) -> usize {
    let base = w.class(w.mobys[src].o_class).map_or(1.0, |c| c.scale);
    let (pos, rot, s) = (w.mobys[src].pos, w.mobys[src].rot, w.mobys[src].scale);
    let id = w.create(o_class);
    let m = &mut w.mobys[id];
    m.draw_dist = 0xff;
    m.drawn = 1;
    if base != 0.0 { m.scale *= s / base; }
    m.pos = pos;
    m.rot = rot;
    m.update_dist = update_dist;
    m.mode = 0;
    w.changed = true;
    id
}

/// The wrecks (`0x3d0f50`: 0x0ac9, 0x0c0a..0x0c0d, 0x0e01 / 0x0e02): a hit with the flags 0x10000 while `+0x68`[0x80]
/// = 0 breaks them — 5 pieces (14 when the frame time `0x1b194c` / `0x1b1950` is short; the class 0x43e flung out,
/// `0x39c6b8`: the effects pass), the sound 0xc0a, the sound 0xc0a, then 0x0c0a..0x0c0d leave the wreck 0x0c10..0x0c13 in their place
/// and are hidden without collision and deleted after 300 ticks; 0x0ac9 (turned 45° at the start) and the 0x0e0x
/// stay as they are (`+0x7c` = 0x8001).
pub fn update_wreck(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class = w.mobys[id].o_class;
    let keep = matches!(class, 0x0ac9 | 0x0e01 | 0x0e02);
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            if class == 0x0ac9 { m.rot[1] += std::f32::consts::FRAC_PI_4; }
            m.set_state(1, -1);
            w.changed = true;
        }
        1 => {
            if w.hit_message(id).is_some_and(|h| h.tmpl.flags & 0x10000 != 0) && w.mobys[id].pv_i(0x80) == 0 {
                w.mobys[id].set_state(2, -1);
            }
            w.clear_hit(id);
        }
        2 => {
            // The sound 0xc0a (`0x360698`, by its id, not the class's: the sound pass).
            let _ = &ctx;
            let wreck = match class { 0x0c0a => Some(0x0c10), 0x0c0b => Some(0x0c11), 0x0c0c => Some(0x0c12), 0x0c0d => Some(0x0c13), _ => None };
            if let Some(c) = wreck { spawn_in_place(w, id, c, 0); }
            w.mobys[id].set_state(3, -1);
        }
        3 => {
            let m = &mut w.mobys[id];
            if m.first() {
                if !keep {
                    m.drawn = 0;
                    m.mode |= 1;
                    m.has_collision = false;
                }
                m.set_pv_i(0x84, 300);
            }
            let mut t = m.pv_i(0x84);
            let done = crate::moby::countdown(&mut t);
            m.set_pv_i(0x84, t);
            if done && !keep { w.delete(id); }
        }
        _ => {}
    }
}

/// 0x0f13 (`0x40eb48`, a scene's props): at the start, once its uid is dead or counted, the mobys `+0x68`[0x40] /
/// [0x44] go; then, outside a scene (`0x1a8f00` ≠ 2), the moby [0x20] is a target with its class's collision. The
/// scenes' part (the scene `0x1bca70` 1 / 2 by its frame `0x1bca74`: the props hidden, the uid counted, the scene
/// mobys `0x3dbed0` / `0x3dbfb0` driven, the sounds at the frames 0xdf / 0xe2) waits for the scene system.
pub fn update_0f13(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        for k in [0x24, 0x28, 0x30, 0x34] { m.set_pv_i(k, 0); }
        m.update_dist = 0xff;
        m.state = 1;
        let uid = m.uid;
        let dead = ctx.game.uid_dead.get(uid.max(0) as usize).copied().unwrap_or(0) != 0;
        if !dead && ctx.game.uid_count(uid) == 0 { return; }
        for k in [0x44, 0x40] {
            let i = w.mobys[id].pv_i(k);
            if i != -1 && (i as usize) < w.mobys.len() { w.delete(i as usize); }
        }
        return;
    }
    if m.state != 1 { return; }
    let t = m.pv_i(0x20);
    if t != -1 && (t as usize) < w.mobys.len() {
        let coll = w.class(w.mobys[t as usize].o_class).is_some_and(|c| c.has_collision);
        let o = &mut w.mobys[t as usize];
        o.mode = (o.mode & 0xffbe) | crate::moby::mode::TARGET;
        o.has_collision = coll;
        w.changed = true;
    }
}

/// 0x1330 (`0x3846d0`, the level's arrival): at the start Ratchet is put at the cuboid `+0x68`[1] (`0x2b9e68`: its
/// place and angles, the idle). The arrival scene — Ratchet's ship (0x0d54, created at the path [5]'s start) flown
/// along the path at [7]/60 a tick under the scene camera [2], the level's name shown, then the ship hidden and
/// Ratchet walked 2 out of the cuboid along its yaw — plays only on a first visit (the save's flag, the scene
/// system: not ported), so the port takes the visit's other branch: state 4, then 5 once no walk is scripted (the
/// mobys of the list `0x220d50`, the ships, hidden in 4 and 5). Not on a restart from a checkpoint (the port's
/// restart keeps the checkpoint's place).
pub fn update_1330(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    match m.state {
        0 => {
            m.update_dist = 0xff;
            m.set_pv_i(0x20, 0);
            let at = m.pv_i(4);
            if ctx.game.checkpoint.is_none() {
                if let Some((p, r)) = ctx.level.cuboid_place(at) {
                    ctx.hero.teleport(p, r);
                    ctx.hero.request_state(0, true);
                }
            }
            m.set_state(4, -1);
        }
        4 => {
            m.first();
            if !(0x62..=0x64).contains(&ctx.hero.state) { m.set_state(5, -1); }
        }
        _ => {}
    }
}

/// Shows or hides the members of a group (more than one alive): hidden is mode |= 0x43 without collision, shown
/// mode &= !0x43 with the class's collision.
fn show_group(w: &mut MobyWorld, group: i32, show: bool) {
    if group_alive(w, group) < 2 { return; }
    for i in w.groups[group as usize].clone() {
        let coll = w.class(w.mobys[i].o_class).is_some_and(|c| c.has_collision);
        let m = &mut w.mobys[i];
        if show {
            m.mode &= 0xffbc;
            m.has_collision = coll;
        } else {
            m.mode |= 0x43;
            m.has_collision = false;
        }
    }
    w.changed = true;
}

/// 0x0ac4 (`0x3d01a0`, a gate): shut (state 0) it keeps the group `+0x68`[0x1c] hidden; Ratchet within 6 and in
/// neither cuboid [0x14] / [0x18] opens it (1: sequence 1, no collision, the group shown), in [0x14] leaves it open
/// for good (5); one of the four mobys [0..0xc] gone or set off (its sub-state) opens it too — or, with [0x10] ≠ 0,
/// blows it up (the sound 2, a blast `0x320538` and debris `0x327498`: the effects pass; deleted). Opened it shows
/// the sequence 2 (3); Ratchet through into [0x14] shuts it (4: sequence 3, then 5: its collision back, sequence 0).
/// Its colour word `+0x78` and the state 2 (a slide 0.3 a tick to its right until it touches, set by nothing here)
/// are not ported.
pub fn update_0ac4(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let p = ctx.hero.pos;
    if w.mobys[id].state == 0 {
        let m = &w.mobys[id];
        let (a, b, group) = (m.pv_i(0x14), m.pv_i(0x18), m.pv_i(0x1c));
        show_group(w, group, false);
        let near = crate::hero::physics::dist(w.mobys[id].pos, p) < 6.0;
        if near && !ctx.level.in_cuboid(a, p) && !ctx.level.in_cuboid(b, p) {
            w.mobys[id].state = 1;
            set_seq(w, id, ctx, 1, 0, 0);
            w.mobys[id].has_collision = false;
            show_group(w, group, true);
        } else if ctx.level.in_cuboid(a, p) {
            w.mobys[id].state = 5;
        }
        for k in 0..4 {
            let o = w.mobys[id].pv_i(4 * k);
            if o == -1 { continue; }
            let Some(om) = w.mobys.get(o as usize) else { continue };
            if om.sub_state == 0 && om.state < 0x80 { continue; }
            if w.mobys[id].pv_i(0x10) != 0 {
                ctx.host.play_sound(&w.mobys[id], 2, 0);
                w.delete(id);
                return;
            }
            w.mobys[id].state = 1;
            set_seq(w, id, ctx, 1, 0, 0);
            w.mobys[id].has_collision = false;
        }
    }
    if w.mobys[id].state == 1 && wrapped(w, id, ctx) {
        w.mobys[id].state = 3;
        set_seq(w, id, ctx, 2, 0, 0);
    }
    if w.mobys[id].state == 3 && ctx.level.in_cuboid(w.mobys[id].pv_i(0x14), p) {
        w.mobys[id].state = 4;
        set_seq(w, id, ctx, 3, 0, 0);
    }
    if w.mobys[id].state == 4 && wrapped(w, id, ctx) {
        w.mobys[id].state = 5;
        w.mobys[id].has_collision = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
        set_seq(w, id, ctx, 0, 0, 0);
    }
}

/// `0x324430(amp, step, base, m, &phase, &last)`: the height bobs — `base + sin(phase)·amp` (base ≤ 0: by the
/// change from the last tick's offset), the phase stepped.
fn bob(m: &mut crate::moby::Moby, amp: f32, step: f32, base: f32, phase: usize, last: usize) {
    let ph = crate::hero::physics::add_rot(m.pv_f(phase), step);
    m.set_pv_f(phase, ph);
    if 0.0 < base {
        m.pos[2] = ph.sin() * amp + base;
    } else {
        m.pos[2] -= m.pv_f(last);
        let d = ph.sin() * amp;
        m.set_pv_f(last, d);
        m.pos[2] += d;
    }
}

/// `0x3244e8(tilt, step_a, step_b, m, &a, &b)`: the pitch / roll sway (`tilt·sin a·sin b`, `tilt·sin a·cos b`), the
/// phases stepped.
fn sway(m: &mut crate::moby::Moby, tilt: f32, step_a: f32, step_b: f32, a: usize, b: usize) {
    let (pa, pb) = (m.pv_f(a), m.pv_f(b));
    m.rot[0] = tilt * pa.sin() * pb.sin();
    m.rot[1] = tilt * pa.sin() * pb.cos();
    m.set_pv_f(a, crate::hero::physics::add_rot(pa, step_a));
    m.set_pv_f(b, crate::hero::physics::add_rot(pb, step_b));
}

/// 0x0adb (`0x3d1d60`, a hovering craft): after a random 0..39 ticks it bobs 1.5 about its height (100°/s) and sways
/// 9° (10°/s, 50°/s: the tunables `$gp+0x1128..0x1138`); its thrusters' particles (`0x335bb8`, six a tick) wait for
/// the effects pass.
pub fn update_0adb(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    const DEG_S: f32 = 0.000_290_888_25;
    if w.mobys[id].state == 0 {
        let t = ctx.game.rng.randi(0x28);
        let m = &mut w.mobys[id];
        m.set_pv_i(0x10, t);
        m.update_dist = 100;
        m.state = 1;
        m.set_pv_i(0x18, 0);
        m.set_pv_f(0x28, m.pos[2]);
    }
    let m = &mut w.mobys[id];
    if m.state == 1 {
        let mut t = m.pv_i(0x10);
        if crate::moby::countdown(&mut t) { m.state = 2; }
        m.set_pv_i(0x10, t);
    } else if m.state == 2 {
        let base = m.pv_f(0x28);
        bob(m, 1.5, 100.0 * DEG_S, base, 0xc, 0);
        sway(m, 9.0 * 0.017_453_294, 10.0 * DEG_S, 50.0 * DEG_S, 4, 8);
        w.changed = true;
    }
}

/// The lifts 0x0ac2 (`0x3cf6c8`) and 0x0aca (`0x3d1458`): each runs between its height and `+0x68`[0x8c] (`[0x90]`
/// ≠ 0: starting at the other end; `[0x94]` / `[0x98]` the ends of the states 2 / 3, `[0x80]` the end it is at),
/// with a pad (0x0c16; 0x0ac2's 0.2 above it, 0x0aca's 0.25 above and 1 ahead; its light pulsing `[0xa0]`: not
/// drawn) that starts it when Ratchet stands on it — or, with him far off at the first end (0x0ac2: more than 14
/// below; 0x0aca: more than 6 off and within 25), or more than 6 off at the other (0x0ac2: above), it comes to him —
/// the pad's collision off, the looped sound 0, the sound 0x8a1 (`0x360698`, the sound pass), 0x0ac2's sequence 2.
/// It springs there (up: k 0.02, d 0.3, at most 0.1; down: 1/600 and 1/300, the fast spring with Ratchet just under
/// it, held while he is in the cuboid — 0x0ac2's `[0xa8]`, 0x0aca's `[0xa4]`, the other word the spring's velocity),
/// carrying Ratchet (`0x322668`). There (0x0ac2: within 0.14; 0x0aca: 0.25) it waits — 0x0ac2 while he is within
/// 1.2, 0x0aca while he stands on the pad or is within 2 and not below it — the sound 1 on arriving, then the pad
/// comes back (0x0ac2's sequence 1).
struct Lift {
    aca: bool,
}

pub fn update_0ac2(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) { lift(w, id, ctx, Lift { aca: false }) }
pub fn update_0aca(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) { lift(w, id, ctx, Lift { aca: true }) }

fn lift(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, l: Lift) {
    use crate::hero::physics::{dist, spring};
    let (old, old_yaw) = (w.mobys[id].pos, w.mobys[id].rot[2]);
    let hp = ctx.hero.pos;
    let (vel, vol) = if l.aca { (0xa8, 0xa4) } else { (0xa4, 0xa8) };
    let pad = |w: &MobyWorld| usize::try_from(w.mobys[id].pv_i(0x9c)).ok().filter(|&i| i < w.mobys.len());
    let on_pad = |w: &MobyWorld, ctx: &Ctx| pad(w).is_some_and(|p| ctx.hero.ground_moby == Some(p) && ctx.hero.air_ticks == 0);
    let pad_at = |m: &crate::moby::Moby, up: f32| {
        if l.aca {
            let (s, c) = m.rot[2].sin_cos();
            add3(m.pos, [c, s, 0.25])
        } else {
            add3(m.pos, [0.0, 0.0, up])
        }
    };
    // The trip's start (the states 2 / 3): the pad off, the loop on.
    let start = |w: &mut MobyWorld, ctx: &mut Ctx, state: u8| {
        if !ctx.host.sound_playing(w.mobys[id].pv_i(0x88)) {
            let h = ctx.host.play_sound(&w.mobys[id], 0, 4);
            w.mobys[id].set_pv_i(0x88, h);
        }
        w.mobys[id].set_state(state, -1);
        if !l.aca { set_seq(w, id, ctx, 2, 0, 6); }
        if let Some(p) = pad(w) { w.mobys[p].has_collision = false; }
    };
    // There: waits while Ratchet is near, then the pad back at the end `at`.
    let arrive = |w: &mut MobyWorld, ctx: &mut Ctx, at: i32| {
        let h = w.mobys[id].pv_i(0x88);
        let d = dist(w.mobys[id].pos, hp);
        let wait = if l.aca { on_pad(w, ctx) || (d <= 2.0 && w.mobys[id].pos[2] <= hp[2]) } else { d <= 1.2 };
        if h != -1 { ctx.host.stop_sound(h); }
        w.mobys[id].set_pv_i(0x88, -1);
        if wait {
            if 0 < w.mobys[id].pv_i(0x84) {
                ctx.host.play_sound(&w.mobys[id], 1, 0);
                w.mobys[id].set_pv_i(0x84, 0);
            }
            if !l.aca { set_seq(w, id, ctx, 1, 0, 6); }
            return;
        }
        w.mobys[id].set_pv_i(0x80, at);
        w.mobys[id].set_state(1, -1);
        if !l.aca { set_seq(w, id, ctx, 1, 0, 6); }
        if l.aca && at == 1 { w.mobys[id].set_pv_i(0x84, 0); }
        if let Some(p) = pad(w) { w.mobys[p].has_collision = w.class(0x0c16).is_some_and(|c| c.has_collision); }
    };
    let step = |m: &mut crate::moby::Moby, t: f32, k: f32, d: f32| {
        let (mut z, mut v) = (m.pos[2], m.pv_f(vel));
        spring(t, k, d, 0.1, &mut z, &mut v);
        m.pos[2] = z;
        m.set_pv_f(vel, v);
        m.set_pv_i(0x84, m.pv_i(0x84) + 1);
    };
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x5c, m.pv_i(0x5c) | 1);
            m.set_pv_v(0x60, m.pos);
            m.state = 1;
            let (z, other) = (m.pos[2], m.pv_f(0x8c));
            let flip = m.pv_i(0x90) != 0;
            m.set_pv_f(0x94, if flip { other } else { z });
            m.set_pv_f(0x98, if flip { z } else { other });
            m.set_pv_i(0x80, flip as i32);
            m.set_pv_i(0x84, 0);
            m.set_pv_i(0x88, -1);
            let (at, ud, dd) = (pad_at(m, 0.15), m.update_dist, m.draw_dist);
            let p = w.create(0x0c16);
            let pm = &mut w.mobys[p];
            pm.update_dist = ud;
            pm.draw_dist = dd;
            pm.drawn = 1;
            pm.pos = at;
            w.mobys[id].set_pv_i(0x9c, p as i32);
            if !l.aca { set_seq(w, id, ctx, 2, 0, 0); }
            let ph = ctx.game.rng.rand_angle();
            w.mobys[id].set_pv_f(0xa0, ph);
        }
        1 => {
            let m = &mut w.mobys[id];
            m.set_pv_f(0xa0, crate::hero::physics::add_rot(m.pv_f(0xa0), 0.104_719_76));
            if !l.aca && dist(m.pos, hp) < 6.0 && seq(w, id, ctx) == 2 { ctx.host.set_anim(&w.mobys[id], 1, 0, 6); }
            let at = w.mobys[id].pv_i(0x80);
            let z = w.mobys[id].pos[2];
            let dz = (z - hp[2]).abs();
            if on_pad(w, ctx) {
                start(w, ctx, if at == 0 { 3 } else { 2 });
            } else if l.aca {
                if 6.0 < dz && at == 0 && dist(w.mobys[id].pos, hp) < 25.0 {
                    start(w, ctx, 3);
                } else if 6.0 < dz && at == 1 {
                    start(w, ctx, 2);
                }
            } else if 14.0 < dz && at == 0 && hp[2] < z {
                start(w, ctx, 3);
            } else if 6.0 < dz && at == 1 && z < hp[2] {
                start(w, ctx, 2);
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            let t = m.pv_f(0x94);
            if (if l.aca { 0.25 } else { 0.14 }) < dist(m.pos, [m.pos[0], m.pos[1], t]) {
                step(m, t, 0.02, 0.3);
            } else {
                arrive(w, ctx, 0);
            }
        }
        3 => {
            let t = w.mobys[id].pv_f(0x98);
            let z = w.mobys[id].pos[2];
            if t < z {
                let under = z - hp[2] < 1.8 && hp[2] < z;
                if !(under && ctx.level.in_cuboid(w.mobys[id].pv_i(vol), hp)) {
                    let (k, d) = if under { (0.02, 0.3) } else { (0.001_666_667, 0.003_333_333) };
                    step(&mut w.mobys[id], t, k, d);
                }
            } else {
                arrive(w, ctx, 1);
            }
        }
        _ => {}
    }
    let m = &w.mobys[id];
    let (pos, yaw, at) = (m.pos, m.rot[2], pad_at(m, 0.2));
    crate::moby::carry_hero(ctx, id, old, old_yaw, pos, yaw);
    if let Some(p) = pad(w) {
        crate::moby::carry_hero(ctx, p, w.mobys[p].pos, yaw, at, yaw);
        w.mobys[p].pos = at;
    }
    w.changed = true;
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
