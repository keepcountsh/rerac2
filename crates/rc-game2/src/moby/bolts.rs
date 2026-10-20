//! Bolts out of a moby: the drop `0x31adc0(moby, flags, limit)` (the moby's bolt value `+0xb4` by the uid's break
//! count, `0x2e4670` over the percentage tables `0x1a89c8` / `0x1a89d0`, plus a fiftieth of the bank), the spawner
//! `0x322af0(height, moby, min, max, flags, −1, limit)` (the amount split into the denominations 1000 / 500 / 100 / 50
//! / 20 / 5 / 1, at most `limit` bolt mobys with flag 8, the rest into the bank `0x1a7a1c`) and the bolt's creation
//! `0x383380` (classes 13..19; one more than 64 from the camera goes to the bank instead).
//!
//! Not ported: the XP of the drop (`+0xb6`, `0x18d198`/`0x18d208` rate scaling), the challenge mode multiplier
//! (`0x1a7a10`), the bolt magnet (`0x3831a8` from the spawn, flag 2), the bolt's light (`0x3132e0`).

use super::{Ctx, MobyWorld};
use crate::hero::physics::{add, scale, V3};

/// `0x1a89c8` (first table, bit 3 clear) and `0x1a89d0` (bit 3 set): the percentage of the bolt value by break count.
pub const BOLT_PERCENT: [[u8; 8]; 2] = [[100, 50, 40, 30, 25, 20, 15, 10], [100; 8]];

/// The bolt classes by value (`0x383380`).
pub fn bolt_class(value: i32) -> i16 {
    match value { 5 => 0xe, 20 => 0xf, 50 => 0x10, 100 => 0x11, 500 => 0x12, 1000 => 0x13, _ => 0xd }
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
/// `vel`, update and draw distance 0x40, scale class × the value's factor; one farther than 64 from the camera goes
/// to the bank instead.
pub fn create_bolt(w: &mut MobyWorld, src: usize, pos: V3, vel: V3, _flags: u32, value: i32, ctx: &mut Ctx) -> Option<usize> {
    let cam = ctx.host.camera_pos();
    let d = ((pos[0] - cam[0]).powi(2) + (pos[1] - cam[1]).powi(2) + (pos[2] - cam[2]).powi(2)).sqrt();
    if 64.0 <= d {
        ctx.game.bolt_bank += value;
        return None;
    }
    let class = bolt_class(value);
    let class_scale = w.class(class)?.scale;
    // The top owner of the source (`+0xb8` chain).
    let mut parent = src;
    while let Some(p) = w.mobys.get(parent).and_then(|s| s.parent) { if p == parent { break; } parent = p; }
    let id = w.create(class);
    let g = &mut *ctx.game;
    let spin = g.rng.randi(2) != 0;
    let a = g.rng.rand_angle();
    let b = g.rng.rand_angle();
    let m = &mut w.mobys[id];
    // `$gp − 0x448 + 4·k`: every denomination's factor is 0.5.
    m.scale = class_scale * 0.5;
    m.mode |= super::mode::KEEP_ROWS;
    m.state = 1;
    m.drawn = 1;
    m.update_dist = 0x40;
    m.draw_dist = 0x40;
    m.parent = Some(parent);
    m.pos = pos;
    m.set_pv_v(0x20, vel);
    if spin { m.set_pv_b(0x55, m.pv_b(0x55) | 1); }
    m.set_pv_f(0x50, a);
    m.set_pv_f(0x58, b);
    m.set_pv_f(0x68, pos[2]);
    Some(id)
}
