//! Bolts out of a moby: the drop `0x31adc0(moby, flags, limit)` (the moby's bolt value `+0xb4` by the uid's break
//! count, `0x2e4670` over the percentage tables `0x1a89c8` / `0x1a89d0`, plus a fiftieth of the bank), the spawner
//! `0x322af0(height, moby, min, max, flags, −1, limit)` (the amount split into the denominations 1000 / 500 / 100 / 50
//! / 20 / 5 / 1, at most `limit` bolt mobys with flag 8, the rest into the bank `0x1a7a1c`) and the bolt's creation
//! `0x383380` (classes 13..19; one more than 64 from the camera goes to the bank instead).
//!
//! Every bolt, placed (its state 0) or made here, gets the same look ([`dress`]): half its class's size, the metal
//! pass from 20 units (`+0x63` 0x14, the gold sheen), light set 0xd and the bolt ambient, its rows kept (mode 0x100).
//!
//! Not ported: the XP of the drop (`+0xb6`, `0x18d198`/`0x18d208` rate scaling), the challenge mode multiplier
//! (`0x1a7a10`).

use super::{Ctx, Moby, MobyWorld};
use crate::hero::physics::{add, cross, scale, set_len, sub, V3};

/// `0x1a89c8` (first table, bit 3 clear) and `0x1a89d0` (bit 3 set): the percentage of the bolt value by break count.
pub const BOLT_PERCENT: [[u8; 8]; 2] = [[100, 50, 40, 30, 25, 20, 15, 10], [100; 8]];

/// The bolt classes by value (`0x383380`).
pub fn bolt_class(value: i32) -> i16 {
    match value { 5 => 0xe, 20 => 0xf, 50 => 0x10, 100 => 0x11, 500 => 0x12, 1000 => 0x13, _ => 0xd }
}

/// `$gp − 0x448 + 4·k` (L00 `0x1aeba8..`): the size factor of every bolt class, 0.5.
pub const BOLT_SCALE: f32 = 0.5;
/// `$gp − 0x418` (L00 `0x1aebd8`, the same bytes on every level): the bolts' ambient (`0x3241d8`).
pub const BOLT_AMBIENT: [u8; 3] = [0x10, 0x10, 0x18];

/// The bolt look (`0x381e60` state 0 and `0x383380`): mode 0x100, shine 0x14, the scale class × 0.5, the ambient
/// ([`BOLT_AMBIENT`] through `0x3241d8`) and light set 0xd with it kept (`0x313330` / `0x3132e0(m, 0, 0xd, 0, 0)` /
/// `0x313300`).
pub fn dress(m: &mut Moby, class_scale: f32) {
    m.mode |= super::mode::KEEP_ROWS;
    m.shine = 0x14;
    m.scale = class_scale * BOLT_SCALE;
    m.ambient = BOLT_AMBIENT;
    m.light = 0xd;
}

/// The value of a bolt class (`0x382c18`).
pub fn bolt_value(o_class: i16) -> i32 {
    match o_class { 0xe => 5, 0xf => 20, 0x10 => 50, 0x11 => 100, 0x12 => 500, 0x13 => 1000, _ => 1 }
}

/// `0x31adc0(moby, flags, limit)`.
pub fn drop(w: &mut MobyWorld, id: usize, flags: u32, limit: i32, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if m.uid < 0 || m.sub_index & 0x80 != 0 { return; }
    let (uid, base) = (m.uid, m.bolts);
    w.mobys[id].drops = w.mobys[id].drops.wrapping_add(1);
    let g = &mut *ctx.game;
    // A fiftieth of the bank (rounded up) joins the drop unless flag 0x1000.
    let mut bank = 0;
    if flags & 0x1000 == 0 {
        bank = (g.bolt_bank + 0x31) / 0x32;
        g.bolt_bank -= bank;
    }
    // The XP part (`+0xb6`): the health and XP widget shows for 720 ticks (`0x30d9f8(0x2d0)`; the XP itself is not
    // ported).
    if w.mobys[id].xp != 0 { g.show_health = Some(0x2d0); }
    let table = &BOLT_PERCENT[g.uid_flag(uid) as usize];
    let value = table[g.uid_count(uid) as usize] as i32 * base as i32 / 100;
    g.bump_uid(uid);
    let mission = w.mobys[id].mission;
    g.uid_gone(uid, mission);
    let mut f = value as f32 + bank as f32;
    let mut spawn_flags = if flags & 0x100 != 0 { 5 } else { 1 };
    if flags & 0x200 != 0 { spawn_flags |= 2; }
    if flags & 0x400 != 0 { spawn_flags |= 0x10; }
    spawn_flags |= 8;
    if w.free_slots() < 100 { spawn_flags |= 2; }
    if f < 0.0 { f = 0.0; }
    if 0.0 < f {
        let spread = if 8.0 <= f { (f * 0.25) as i32 } else if 2.0 <= f { 1 } else { 0 };
        let n = f as i32;
        spawn(w, id, 0.0, n - spread, n + spread, spawn_flags, limit, ctx);
    }
}

/// `0x322af0(height, moby, min, max, flags, −1, limit)`.
#[allow(clippy::too_many_arguments)]
pub fn spawn(w: &mut MobyWorld, src: usize, height: f32, min: i32, max: i32, flags: u32, limit: i32, ctx: &mut Ctx) {
    if flags == 0 || (min == 0 && max == 0) { return; }
    let min = min.max(1);
    let g = &mut *ctx.game;
    let mut n = g.rng.randi(max - min + 1) + min;
    const DEN: [i32; 7] = [1000, 500, 100, 50, 20, 5, 1];
    let mut counts = [0i32; 7];
    if flags & 8 == 0 {
        for (k, d) in DEN.iter().enumerate() {
            if k == 6 { counts[k] = n; break; }
            counts[k] = n / d;
            n %= d;
        }
    } else {
        // At most `limit` bolt mobys, the largest denominations first; the rest to the bank.
        let mut left = limit;
        for (k, d) in DEN.iter().enumerate() {
            if left <= 0 { break; }
            let c = (n / d).min(left);
            counts[k] = c;
            left -= c;
            n -= c * d;
        }
        g.bolt_bank += n;
    }
    let centre = { let b = w.mobys[src].bsphere; [b[0] / 1024.0, b[1] / 1024.0, b[2] / 1024.0] };
    let mut k = 0;
    while k < 7 {
        if counts[k] == 0 { k += 1; continue; }
        counts[k] -= 1;
        let g = &mut *ctx.game;
        let a = g.rng.rand_angle();
        let h = g.rng.randf(0.0, 3.0);
        let vx = a.cos() * 0.016_666_668 * h;
        let h2 = g.rng.randf(0.0, 3.0);
        let vy = a.sin() * 0.016_666_668 * h2;
        let vz = g.rng.randf(3.7, 6.0) * 0.016_666_668;
        let vel = [vx, vy, vz];
        let mut p = add(centre, scale([vx, vy, 0.0], 0.5));
        p[2] += height;
        create_bolt(w, src, p, vel, flags, DEN[k], ctx);
    }
}

/// `0x383380(source, pos, vel, flags, value, …)`: the bolt moby of `value`'s class, flying (state 1) from `pos` at
/// `vel`, drawn, update and draw distance 0x40, [`dress`]ed; its tumble a turn of 0..720°/s about `(source − pos) ×
/// vel` (`0x2eef88(out, vel, d)` = d × vel, `0x31fe98`); with flag 2 it flies to Ratchet at once (the magnet:
/// `0x3831a8` with a random lean of ±30° when flag 0x10). One farther than 64 from the camera goes to the bank instead.
pub fn create_bolt(w: &mut MobyWorld, src: usize, pos: V3, vel: V3, flags: u32, value: i32, ctx: &mut Ctx) -> Option<usize> {
    let cam = ctx.host.camera_pos();
    let d = ((pos[0] - cam[0]).powi(2) + (pos[1] - cam[1]).powi(2) + (pos[2] - cam[2]).powi(2)).sqrt();
    if 64.0 <= d {
        ctx.game.bolt_bank += value;
        return None;
    }
    let class = bolt_class(value);
    let class_scale = w.class(class)?.scale;
    let from = w.mobys[src].pos;
    // The top owner of the source (`+0xb8` chain).
    let mut parent = src;
    while let Some(p) = w.mobys.get(parent).and_then(|s| s.parent) { if p == parent { break; } parent = p; }
    let id = w.create(class);
    let g = &mut *ctx.game;
    let spin = g.rng.randi(2) != 0;
    let a = g.rng.rand_angle();
    let b = g.rng.rand_angle();
    let turn = g.rng.randf(0.0, 720.0) * 0.000_290_888_25;
    let m = &mut w.mobys[id];
    dress(m, class_scale);
    m.state = 1;
    m.drawn = 1;
    m.update_dist = 0x40;
    m.draw_dist = 0x40;
    m.parent = Some(parent);
    m.rows = super::attach::rows_of(m.rot);
    m.pos = pos;
    m.set_pv_v(0x20, vel);
    if spin { m.set_pv_b(0x55, m.pv_b(0x55) | 1); }
    m.set_pv_f(0x50, a);
    m.set_pv_f(0x58, b);
    m.set_pv_f(0x68, pos[2]);
    m.set_pv_q(0x30, crate::quat::axis_angle(turn, set_len(cross(sub(from, pos), vel), 1.0)));
    if flags & 2 != 0 {
        let lean = if flags & 0x10 != 0 { ctx.game.rng.randf(-30.0, 30.0) * 0.017_453_294 } else { 0.0 };
        super::classes::bolt::attract(w, id, lean, 0.0, ctx);
    }
    Some(id)
}
