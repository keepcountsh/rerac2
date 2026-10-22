//! Particle types 48 and 50, level 5's glints and falling sparks (level01 updates 0x286e20 (48) and 0x286f90 (50):
//! no Ghidra function until 2026-09-29, both the same code on all 19 levels, `overlay-diff`; spawners level05
//! `0x29bbe0` / `0x29be70`, read from the decomp). Their callers are level 5's unported classes (G-PRT-001).
//!
//! **48, spawn** `(size f12, pos a0, vel a1, life a2, spin a3, rgba t0)`: RGBA `rgba & 0xffffff` (alpha 0), byte9
//! `trunc(2) + 0x10` = 0x12, ALPHA 0x44, byte1 0, size·210000, texture `def[48][0]`, rotation `randi(256)` (the one
//! draw), velocity +0x20 (xyzw), +0x30 = 2/life, timer = life, +0x38 = rgba, +0x34 = rgba at alpha 0, +0x3c = spin.
//! **48, update**: rotation += spin; pos += vel; colour = `FastTweenColor(|t·(2/life) − 1|, rgba, rgba at alpha 0)`
//! (it fades in to the middle of its life and out again); the timer fires → killed.
//!
//! **50, spawn** `(size f12, floor f13, g f14, pos a0, vel a1, rgba a2, frame a3)`: byte9 `trunc(1) + 0x40` = 0x41,
//! ALPHA 0x44, byte1 0, texture `def[50][frame]`, RGBA, size·210000, rotation `trunc(randf(0, 255))`, timer
//! `ticks(15)`, velocity +0x20 (xyzw), floor +0x30, gravity +0x38, spin +0x34 = `trunc(randf(−3, 3))` (two draws).
//! **50, update**: once the timer has run out (it stays 0 and fires every tick) vel.z −= g; pos += vel; rotation +=
//! spin; frame `def[50][0]` grows by [0x160210]·dt·210000 (0.25 in the image, never written); below the floor →
//! killed. No RNG in either update.

use super::{fast_dec_timer, rec, tween_color, Particles};

/// Update 0x286e20 (no RNG).
pub(crate) fn update48(sys: &mut Particles, i: usize) {
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x3c]);
    let (p, v) = (rec::v4(r, 0x10), rec::v4(r, 0x20));
    rec::set_v4(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    let f = (rec::i16(r, 10) as f32 * rec::ff(r, 0x30) - 1.0).abs();
    rec::set_u32(r, 4, tween_color(f.to_bits(), rec::u32(r, 0x38), rec::u32(r, 0x34)));
    if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}
