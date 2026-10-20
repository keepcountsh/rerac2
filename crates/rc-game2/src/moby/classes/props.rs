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
