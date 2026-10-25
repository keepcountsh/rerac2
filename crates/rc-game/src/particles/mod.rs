//! Going Commando's particles: R&C1's pool and record layout (`crate::particles`: 2048 records of 0x40 bytes, the
//! allocator `0x330df0` = R&C1's `CreatePart`, the kill `0x330f40`, the def table `0x1fe8c0` that `0x2fe578` fills from
//! the same core-header words), so the engine's particle renderer draws them as it draws R&C1's. Only the spawners and
//! the update types differ: `UpdateParts` `0x331150` calls the type table `0x1fe6c0[type]` (set by `0x331ea0`) in pool
//! order; type 0x55's entry calls the function its spawner left at `+0x3c`.
//!
//! | Type | Spawner | Update | |
//! |---|---|---|---|
//! | 0x17 | [`puff`] `0x335bb8` | `0x335d80` = R&C1's type 23 (`crate::particles::type23::update`, the same steps at 60 Hz) | glow puffs |
//! | 0x2c | [`spark`] `0x339848` | `0x3399c8` ([`update_spark`]) | sparks: alpha down with the life, gravity, drag |
//! | 0x30 | [`drift`] `0x33a1c0` | `0x33a2e8` = R&C1's type 48 (`crate::particles::type48::update48`) | drifting smoke: fades in and out over the life |
//! | 0x55 | [`sprite`] `0x33ef78` | `+0x3c` = `0x33f120` ([`update_sprite`]) | sprites: alpha and size tweened over the life |
//! | 0x59 | [`glow`] `0x33f550` | `0x33f6e8` ([`update_glow`]) | a glow riding a moby (the hand item's), pulled toward the camera |
//! | 0x5a | [`streak`] `0x33f940` | `0x33fa58` ([`update_streak`]) | streaks: a ribbon from the point back along its last move |
//! | 0x61 | [`burst`] `0x340830` | `0x3409a8` ([`update_burst`]) | the blasts' sprites: size and alpha stepped over the life |
//! | 0x3b | [`sparkle`] `0x33b468` | `0x33b588` ([`update_sparkle`]) | the nanotech healing ring's sparkles, riding Ratchet |
//! | 0x3c | [`glint`] `0x33b5e0` | `0x33b710` ([`update_glint`]) | the nanotech heal's glints: drifting, fading out |
//! | 0x3e | [`orb`] `0x33bb60` | `0x33bc70` ([`update_orb`]) | the nanotech orbs (kind 1, moved by their cluster) and their trail puffs (kinds 0, 2) |
//! | 4 | [`trail`] `0x332c10` | `0x332d28` ([`update_trail`]) | the debris' smoke trails: growing, colour eased over the life |
//! | 0xb | [`billow`] `0x333a70` | `0x333d00` ([`update_billow`]) | the Gravity Bomb's smoke: billowing, splitting, rising and fading |
//! | 0xf | [`ember`] `0x3348c0` | `0x334a18` ([`update_ember`]) | the Gravity Bomb's sparks: falling, shrinking, trailing embers |
//! | 0x22, 0x23, 0x2d, 0x2e | [`water`] | [`water`] | the water: bubbles, drops, rings and the bow spray (the hero's swim effects) |
//!
//! Any other type kills itself on its first update. The gravity direction (`0x18a0e0`) is (0, 0, −1): normal gravity.
//! Not ported: the gravity-boots bend of the positions (`0x380e08`, `+0x1c` = 2).

mod pool;
pub mod type23;
pub mod type48;
pub mod water;
pub use pool::*;
pub use water::PartAux;

use crate::hero::physics::{add, len, scale, set_len, sub};
use crate::rng::Rng;

/// `0x18a0e0`: the gravity direction.
pub const GRAVITY: [f32; 3] = [0.0, 0.0, -1.0];
/// The update a type-0x55 record carries at `+0x3c`.
const SPRITE_UPDATE: u32 = 0x33f120;

/// `0x2eeae0(rec + 0xa)`: the life down one; true when it was or reaches 0.
fn dec_life(r: &mut crate::particles::Record) -> bool {
    let mut t = rec::i16(r, 0xa) as i32;
    let done = crate::moby::countdown(&mut t);
    rec::set_i16(r, 0xa, t as i16);
    done
}

/// Type 0x30, the drifting particle: R&C1's type 48 (`crate::particles::type48`, the same record and update).
pub const DRIFT: u8 = 0x30;

/// `0x33a1c0(size, pos, vel, life, spin, rgba)`: a drifting particle (type 0x30) at `pos` moving `vel` a tick, its
/// colour fading in to `rgba` at the middle of its `life` and out again, turning `spin` a tick from a random angle;
/// ALPHA 0x44, byte 9 = 0x12. R&C1's `type48::spawn48` with Going Commando's random draw.
#[allow(clippy::too_many_arguments)]
pub fn drift(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 3], vel: [f32; 3], life: i16, spin: i16, rgba: u32) -> Option<usize> {
    let i = sys.create_part(DRIFT)?;
    let def = sys.def_first(DRIFT);
    let rot = rng.randi(0x100) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, rgba & 0xff_ffff);
    r[9] = 2 + 0x10;
    r[3] = 0x44;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210_000.0);
    r[2] = def;
    r[8] = rot;
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x30, 2.0 / life as f32);
    rec::set_i16(r, 0xa, life);
    rec::set_u32(r, 0x38, rgba);
    rec::set_i16(r, 0x3c, spin);
    rec::set_u32(r, 0x34, rgba & 0xff_ffff);
    Some(i)
}

/// `0x33ef78(size0, size1, fall, pos, vel, life, rgb, spin, alpha0, alpha1, w)`: a sprite (type 0x55) at `pos` moving
/// `vel` a tick (its z falling `fall` more each tick), its size from `size0` to `size1` and its alpha from `alpha0` to
/// `alpha1` over `life` ticks, turning `spin` a tick from a random angle, additive, near 1 / far 128.
#[allow(clippy::too_many_arguments)]
pub fn sprite(sys: &mut Particles, rng: &mut Rng, size0: f32, size1: f32, fall: f32, pos: [f32; 3], vel: [f32; 3], life: i32, rgb: u32, spin: u8, alpha0: i32, alpha1: i32, w: i32) -> Option<usize> {
    let i = sys.create_part(0x55)?;
    let def = sys.def_first(0x55);
    let rot = rng.randi(0xff) as u8;
    let mut life = life.max(1);
    let step = (alpha1 - alpha0) / life;
    if step != 0 { life = (alpha1 - alpha0) / step; }
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_ff(r, 0x1c, w as f32);
    rec::set_u32(r, 4, (alpha0 as u32) << 24 | rgb);
    r[1] = 0;
    rec::set_ff(r, 0xc, size0);
    r[3] = 0x48;
    r[9] = 0x40 + 4;
    r[8] = rot;
    r[2] = def;
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x2c, fall);
    r[0x35] = spin;
    r[0x34] = step as u8;
    rec::set_ff(r, 0x30, (size1 - size0) / life as f32);
    rec::set_i16(r, 0xa, life.abs() as i16);
    rec::set_u32(r, 0x3c, SPRITE_UPDATE);
    Some(i)
}

/// `0x33f120`: the life down (out: killed); the alpha steps, the position moves, the velocity's z falls, the angle
/// turns and the size grows.
pub fn update_sprite(sys: &mut Particles, i: usize) {
    let r = &mut sys.pool.recs[i];
    if dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let c = rec::u32(r, 4);
    let a = ((c as i32 >> 24) + r[0x34] as i32) as u32;
    rec::set_u32(r, 4, c & 0xff_ffff | a.wrapping_shl(24));
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    rec::set_ff(r, 0x28, rec::ff(r, 0x28) - rec::ff(r, 0x2c));
    if rec::ff(r, 0x1c) != 0.0 { rec::set_ff(r, 0x1c, 2.0); }
    r[8] = r[8].wrapping_add(r[0x35]);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x30));
}

/// `0x339848(size, grow, drag, fall, w, pos, vel, life, alpha, rgb, spin)`: a spark (type 0x2c) at `pos` moving `vel`,
/// pulled `fall` a tick along the gravity and slowed by `drag`, growing `grow` a tick, its alpha going down with its
/// life, additive, near 1 / far 128.
#[allow(clippy::too_many_arguments)]
pub fn spark(sys: &mut Particles, rng: &mut Rng, size: f32, grow: f32, drag: f32, fall: f32, w: f32, pos: [f32; 3], vel: [f32; 3], life: i16, alpha: i32, rgb: u32, spin: i32) -> Option<usize> {
    let i = sys.create_part(0x2c)?;
    let def = sys.def_first(0x2c);
    let rot = rng.randi(0xff) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_ff(r, 0x1c, w);
    rec::set_u32(r, 4, (alpha as u32) << 24 | rgb);
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[3] = 0x48;
    r[9] = 0x40 + 4;
    r[8] = rot;
    rec::set_i16(r, 0xa, life);
    r[2] = def;
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x2c, fall);
    r[0x30] = spin as u8;
    rec::set_ff(r, 0x34, grow);
    r[0x31] = rgb as u8;
    r[0x32] = (rgb >> 8) as u8;
    r[0x33] = (rgb >> 16) as u8;
    rec::set_i16(r, 0x3c, life);
    rec::set_i16(r, 0x3e, alpha as i16);
    rec::set_ff(r, 0x38, (drag - 1.0) + 1.0);
    Some(i)
}

/// `0x3399c8`: the life down (out: killed); alpha = start · life / start life; the position moves; the velocity
/// gains the fall along the gravity and is scaled by the drag; the angle turns, the size grows.
pub fn update_spark(sys: &mut Particles, i: usize) {
    let r = &mut sys.pool.recs[i];
    if dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let a = (rec::i16(r, 0x3e) as f32 * (rec::i16(r, 0xa) as f32 / rec::i16(r, 0x3c) as f32)) as i32 as u32;
    rec::set_u32(r, 4, a << 24 | (r[0x33] as u32) << 16 | (r[0x32] as u32) << 8 | r[0x31] as u32);
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    if rec::ff(r, 0x1c) != 0.0 { rec::set_ff(r, 0x1c, 2.0); }
    let (fall, drag) = (rec::ff(r, 0x2c), rec::ff(r, 0x38));
    rec::set_v3(r, 0x20, std::array::from_fn(|k| (v[k] + GRAVITY[k] * fall) * drag));
    r[8] = r[8].wrapping_add(r[0x30]);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x34));
}

/// `0x335bb8(jitter, grow_lo, grow_hi, size, pos, spin, vel, rgba, w)`: a glow puff (type 0x17, R&C1's type 23's
/// record): at `pos` jittered up to `jitter`, fading in over 10 ticks then out over 20, growing by a random factor in
/// `grow_lo..grow_hi` a tick, turning `spin` either way, moving `vel`; additive, near 1 / far 0x1c0.
#[allow(clippy::too_many_arguments)]
pub fn puff(sys: &mut Particles, rng: &mut Rng, jitter: f32, grow_lo: f32, grow_hi: f32, size: f32, pos: [f32; 3], spin: i32, vel: [f32; 3], rgba: u32, w: i32) -> Option<usize> {
    let i = sys.create_part(0x17)?;
    let def = sys.def_first(0x17);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, rgba | 0x2000_0000);
    r[9] = 4 + 0x70;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[8] = 0;
    r[2] = def;
    for k in 0..3 {
        let d = rng.randf_signed(0.0, jitter);
        rec::set_ff(r, 0x10 + 4 * k, rec::ff(r, 0x10 + 4 * k) + d);
    }
    rec::set_ff(r, 0x1c, w as f32);
    rec::set_i16(r, 0xa, 10);
    rec::set_u32(r, 0x24, 0);
    rec::set_u32(r, 0x20, rgba);
    let s = if rng.randi(2) != 0 { spin } else { -spin };
    rec::set_i16(r, 0x28, s as i16);
    rec::set_ff(r, 0x2c, rng.randf(grow_lo, grow_hi));
    rec::set_v3(r, 0x30, vel);
    rec::set_ff(r, 0x3c, 30.0);
    Some(i)
}

/// `0x33f550(size, pull, offset, moby, rgba, joint −1, def 1, life, facing, far)`: a glow (type 0x59) on moby `moby`
/// at `offset` in its rows, of `size` and colour `rgba`, its frame `def[0x59][1]`, pulled `pull` units toward the
/// camera, for `life` ticks (−1: while the moby lives); `facing`: faded unless it faces the camera
/// ([`update_glow`]); additive, near 1 / far `far` (the caller's, else the moby's draw distance `+0x32`) >> 5, ×32. Starts at the moby's position plus the offset (unturned): its
/// update places it the same tick.
#[allow(clippy::too_many_arguments)]
pub fn glow(sys: &mut Particles, rng: &mut Rng, size: f32, pull: f32, offset: [f32; 3], moby: usize, pos: [f32; 3], far: i16, facing: bool, rgba: u32, life: i16) -> Option<usize> {
    let i = sys.create_part(0x59)?;
    let def = sys.def_frame(0x59, 1);
    let rot = rng.randi(0xff) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, rgba);
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[3] = 0x48;
    // Near 1; far `far` >> 5 (in 32s: the caller's, else the moby's draw distance; the hand item's 0x20: 1).
    r[9] = ((far >> 5) << 4) as u8 + 4;
    r[8] = rot;
    rec::set_i16(r, 0xa, life);
    r[2] = def;
    rec::set_u32(r, 0x20, moby as u32 + 1);
    rec::set_u32(r, 0x24, u32::MAX);
    rec::set_ff(r, 0x28, pull);
    rec::set_i16(r, 0x2c, facing as i16);
    rec::set_i16(r, 0x2e, 0);
    rec::set_v3(r, 0x30, offset);
    rec::set_v3(r, 0x10, [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]]);
    Some(i)
}

/// `0x33f940(fall, pos, vel, life)`: a streak (type 0x5a, a ribbon: record kind 3) at `pos` moving `vel` a tick (its z
/// falling `fall` more each tick), its tail 0.9 of a move behind; both colours white at full alpha, fading over its last
/// `life − 15` ticks; widths 0.05 and twice that; additive, near 2 / far 64.
pub fn streak(sys: &mut Particles, fall: f32, pos: [f32; 3], vel: [f32; 3], life: i16) -> Option<usize> {
    let i = sys.create_part(0x5a)?;
    let def = sys.def_first(0x5a);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0xc, 0x7fff_ffff);
    rec::set_u32(r, 4, 0x7fff_ffff);
    r[9] = 2 + 0x20;
    r[1] = 3;
    r[3] = 0x48;
    r[2] = def;
    rec::set_v3(r, 0x30, vel);
    rec::set_ff(r, 0x3c, fall);
    rec::set_v3(r, 0x10, pos);
    rec::set_v3(r, 0x20, std::array::from_fn(|k| pos[k] - vel[k] * 0.9));
    rec::set_i16(r, 0xa, life);
    rec::set_ff(r, 0x1c, 0.05);
    rec::set_ff(r, 0x2c, 2.0);
    r[8] = (life - 15) as u8;
    Some(i)
}

/// `0x33fa58`: the life down (out: killed); one of its type's first two frames; the point moves, the tail follows 0.9
/// of the move behind, the velocity's z falls; under `+8` ticks left both alphas go down with the life.
pub fn update_streak(sys: &mut Particles, i: usize, rng: &mut Rng) {
    if dec_life(&mut sys.pool.recs[i]) {
        sys.kill_part(i);
        return;
    }
    let k = rng.randi(2) as usize;
    let def = sys.def_frame(0x5a, k);
    let r = &mut sys.pool.recs[i];
    r[2] = def;
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x30));
    let q: [f32; 3] = std::array::from_fn(|k| p[k] + v[k]);
    rec::set_v3(r, 0x10, q);
    rec::set_v3(r, 0x20, std::array::from_fn(|k| q[k] - (q[k] - p[k]) * 0.9));
    rec::set_ff(r, 0x2c, 2.0);
    rec::set_ff(r, 0x38, rec::ff(r, 0x38) - rec::ff(r, 0x3c));
    let life = rec::i16(r, 0xa);
    if life < r[8] as i16 {
        let a = ((life as f32 / r[8] as f32) * 127.0) as i32 as u32;
        for o in [4, 0xc] { rec::set_u32(r, o, a << 24 | rec::u32(r, o) & 0xff_ffff); }
    }
}

/// `0x340830(size0, size1, fall, pos, vel, life, rgb, spin, alpha0, alpha1, frame)`: a blast sprite (type 0x61) at `pos`
/// moving `vel` a tick (its z falling `fall` more each tick), its size from `size0` toward `size1` and its alpha from
/// `alpha0` toward `alpha1` in steps over `life` ticks, turning `spin` a tick from a random angle, frame `frame` of its
/// type's list; additive, near 1 / far 128.
#[allow(clippy::too_many_arguments)]
pub fn burst(sys: &mut Particles, rng: &mut Rng, size0: f32, size1: f32, fall: f32, pos: [f32; 3], vel: [f32; 3], life: i16, rgb: u32, spin: u8, alpha0: i32, alpha1: i32, frame: usize) -> Option<usize> {
    let i = sys.create_part(0x61)?;
    let rot = rng.randi(0xff) as u8;
    let def = sys.def_frame(0x61, frame);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, (alpha0 as u32) << 24 | rgb);
    r[1] = 0;
    rec::set_ff(r, 0xc, size0);
    r[3] = 0x48;
    r[9] = 4 + 0x40;
    r[8] = rot;
    rec::set_i16(r, 0xa, life);
    r[2] = def;
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x2c, fall);
    r[0x35] = spin;
    rec::set_ff(r, 0x30, (size1 - size0) / life as f32);
    r[0x34] = ((alpha1 - alpha0) / life.max(1) as i32) as u8;
    Some(i)
}

/// `0x3409a8`: the life down (out: killed); the alpha steps, the position moves, the velocity's z falls, the angle turns
/// and the size grows.
pub fn update_burst(sys: &mut Particles, i: usize) {
    let r = &mut sys.pool.recs[i];
    if dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let c = rec::u32(r, 4);
    let a = ((c as i32 >> 24) + r[0x34] as i32) as u32;
    rec::set_u32(r, 4, c & 0xff_ffff | a.wrapping_shl(24));
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    rec::set_ff(r, 0x28, rec::ff(r, 0x28) - rec::ff(r, 0x2c));
    r[8] = r[8].wrapping_add(r[0x35]);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x30));
}

/// `0x33f6e8`: the life down (out, or its moby gone: killed); at its moby's position plus its rows × the offset, then
/// pulled its pull (in units) toward the camera at `cam`.
pub fn update_glow(sys: &mut Particles, i: usize, mobys: &crate::moby::MobyWorld, cam: [f32; 3]) {
    let r = &mut sys.pool.recs[i];
    let alive = rec::i16(r, 0xa) < 0 || !dec_life(r);
    let m = rec::u32(r, 0x20).checked_sub(1).and_then(|k| mobys.mobys.get(k as usize));
    let Some(m) = m.filter(|m| alive && m.state < 0x80) else {
        sys.kill_part(i);
        return;
    };
    let o = rec::v3(r, 0x30);
    let p: [f32; 3] = std::array::from_fn(|k| m.pos[k] + (m.rows[0][k] * o[0] + m.rows[1][k] * o[1] + m.rows[2][k] * o[2]));
    // Facing (`+0x2c`): d = unit(it − camera)·unit(it − moby); turned away (d > 0) its alpha keeps only bits 0x05,
    // else it is scaled by −d. (The game reads the first vector from the stack, set only with `+0x2e`; here it is the
    // camera's, as meant.)
    if rec::i16(r, 0x2c) != 0 {
        let unit = |v: [f32; 3]| { let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if 0.0 < l { v.map(|x| x / l) } else { v } };
        let (a, b) = (unit(std::array::from_fn(|k| p[k] - cam[k])), unit(std::array::from_fn(|k| p[k] - m.pos[k])));
        let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let c = rec::u32(r, 4);
        let c = if -d < 0.0 { c & 0x05ff_ffff } else { c & 0xff_ffff | ((r[7] as f32 * -d) as i32 as u32) << 24 };
        rec::set_u32(r, 4, c);
    }
    // `0x2ef138(pull, d, d)` on `d` = camera − it: `pull` units toward the camera (a distance, not a share of the way).
    let pull = rec::ff(r, 0x28);
    let d: [f32; 3] = std::array::from_fn(|k| cam[k] - p[k]);
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let k = if 0.0 < l { pull / l } else { 0.0 };
    rec::set_v3(r, 0x10, std::array::from_fn(|i| p[i] + d[i] * k));
}

/// Type 0x3e, the nanotech orb and its trail puffs (crate::moby::classes::nanotech).
pub const ORB: u8 = 0x3e;

/// `0x33bb60(pos, life, kind, fall)`: a nanotech orb record (type 0x3e) at `pos`: colour 0x7f7f4040, size 30000,
/// additive, near 4 / far 64, a random rotation, its frame `def[0x3e][kind]` (`kind` > 1: the first), its alpha falling
/// `trunc(127 / life)` a tick (kind 1, the orb itself, never fades); `fall` (`+0x23`): the gravity boots' bend (always 0
/// here). The caller sets the colour and size it wants.
pub fn orb(sys: &mut Particles, rng: &mut Rng, pos: [f32; 3], life: i16, kind: u8) -> Option<usize> {
    let i = sys.create_part(ORB)?;
    let rot = rng.randi(0xff) as u8;
    let def = sys.def_frame(ORB, if kind > 1 { 0 } else { kind as usize });
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, 0x7f7f_4040);
    r[1] = 0;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, 30000.0);
    r[9] = 4 + 0x40;
    r[8] = rot;
    r[0x22] = kind;
    r[0x23] = 0;
    r[2] = def;
    // `trunc(127 / life)` (a life of 0, the orbs': the step is never used).
    rec::set_i16(r, 0x20, if life != 0 { (127.0 / life as f32) as i32 as i16 } else { 0 });
    rec::set_i16(r, 0xa, life);
    Some(i)
}

/// Rides moby `moby` (kind 2): `+0x24` the moby, `+0x30` the record's place from `at`, the moby's position.
pub fn orb_attach(sys: &mut Particles, i: usize, moby: usize, at: [f32; 3]) {
    let r = &mut sys.pool.recs[i];
    let p = rec::pos(r);
    rec::set_u32(r, 0x24, moby as u32 + 1);
    rec::set_v3(r, 0x30, [p[0] - at[0], p[1] - at[1], p[2] - at[2]]);
}

/// `0x33bc70`: kind 1 does nothing (its cluster moves and kills it); kind 2 follows its moby at the kept offset; kinds
/// 0 and 2 lose their alpha step a tick and die with no alpha left.
pub fn update_orb(sys: &mut Particles, i: usize, mobys: &crate::moby::MobyWorld) {
    let r = &mut sys.pool.recs[i];
    if r[0x22] == 1 { return; }
    if r[0x22] == 2 {
        if let Some(m) = rec::u32(r, 0x24).checked_sub(1).and_then(|k| mobys.mobys.get(k as usize)) {
            let o = rec::v3(r, 0x30);
            rec::set_v3(r, 0x10, [m.pos[0] + o[0], m.pos[1] + o[1], m.pos[2] + o[2]]);
        }
    }
    let c = rec::u32(r, 4);
    let a = (c as i32 >> 24) - rec::i16(r, 0x20) as i32;
    if a < 1 {
        sys.kill_part(i);
    } else {
        rec::set_u32(r, 4, c & 0xff_ffff | (a as u32) << 24);
    }
}

/// `0x33b468(size, pos, rgba, rot, def, soft, life, attach)`: a sparkle (type 0x3b) at `pos`, size `size`·210000, its
/// frame `*def[def]`, additive (0x44 with `soft`), near 1 / far 32, for `life` ticks; `attach`: it keeps its place
/// from Ratchet (`hero`, `0x189ea0`) and rides him.
#[allow(clippy::too_many_arguments)]
pub fn sparkle(sys: &mut Particles, size: f32, pos: [f32; 3], rgba: u32, rot: u8, def: u8, soft: bool, life: i16, attach: bool, hero: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(0x3b)?;
    let frame = sys.def_first(def);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0x30, attach as u32);
    if attach { rec::set_v3(r, 0x20, [pos[0] - hero[0], pos[1] - hero[1], pos[2] - hero[2]]); }
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    r[9] = 1 + 0x20;
    r[3] = if soft { 0x44 } else { 0x48 };
    r[8] = rot;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_i16(r, 0xa, life);
    r[2] = frame;
    Some(i)
}

/// `0x33b588`: riding Ratchet, at him plus the kept place; the life down (out: killed).
pub fn update_sparkle(sys: &mut Particles, i: usize, hero: [f32; 3]) {
    let r = &mut sys.pool.recs[i];
    if rec::u32(r, 0x30) == 1 {
        let o = rec::v3(r, 0x20);
        rec::set_v3(r, 0x10, [hero[0] + o[0], hero[1] + o[1], hero[2] + o[2]]);
    }
    if dec_life(r) { sys.kill_part(i); }
}

/// `0x33b5e0(size, pos, vel, rgba, life, rot, attach)`: a glint (type 0x3c) at `pos` moving `vel` a tick, size
/// `size`·210000, its frame `*def[0x35]`, additive, near 1 / far 32, for `life & 0xff` ticks; `attach`: it moves from
/// Ratchet's place (`hero`) as he moves.
#[allow(clippy::too_many_arguments)]
pub fn glint(sys: &mut Particles, size: f32, pos: [f32; 3], vel: [f32; 3], rgba: u32, life: u16, rot: u8, attach: bool, hero: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(0x3c)?;
    let frame = sys.def_first(0x35);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_v3(r, 0x20, vel);
    rec::set_u32(r, 0x3c, attach as u32);
    if attach { rec::set_v3(r, 0x30, [pos[0] - hero[0], pos[1] - hero[1], pos[2] - hero[2]]); }
    rec::set_u32(r, 4, rgba);
    r[9] = 1 + 0x20;
    r[3] = 0x48;
    r[8] = rot;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_i16(r, 0xa, (life & 0xff) as i16);
    r[2] = frame;
    Some(i)
}

/// `0x33b710`: it moves (riding Ratchet: from his place); killed outside [2, 1021]³ or when its life is out; else
/// its colour `0x2f0000((life − 1) / life, rgb, rgba)`: the alpha fades out over the life.
pub fn update_glint(sys: &mut Particles, i: usize, hero: [f32; 3]) {
    let r = &mut sys.pool.recs[i];
    let v = rec::v3(r, 0x20);
    let p = if rec::u32(r, 0x3c) == 1 {
        let o = rec::v3(r, 0x30);
        let p: [f32; 3] = std::array::from_fn(|k| hero[k] + o[k] + v[k]);
        rec::set_v3(r, 0x30, std::array::from_fn(|k| p[k] - hero[k]));
        p
    } else {
        let p = rec::pos(r);
        std::array::from_fn(|k| p[k] + v[k])
    };
    rec::set_v3(r, 0x10, p);
    if p.iter().any(|&x| !(2.0..=1021.0).contains(&x)) || dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let life = rec::i16(r, 0xa) as f32;
    let c = rec::u32(r, 4);
    rec::set_u32(r, 4, tween_color(((life - 1.0) / life).to_bits(), c & 0xff_ffff, c));
}

/// Type 4, a debris trail's puff (crate::moby::classes::debris).
pub const TRAIL: u8 = 4;

/// `0x332c10(pos, vel, c0, c1, life, base, grow, additive)`: a trail puff (type 4) at `pos` moving `vel` a tick, its
/// colour from `c0` to `c1` and its size from `base` to `base + grow` (thousands) over `life` ticks; ALPHA 0x48
/// (`additive`) or 0x44, near 4 / far 0xa, a random rotation turning a step a tick.
#[allow(clippy::too_many_arguments)]
pub fn trail(sys: &mut Particles, rng: &mut Rng, pos: [f32; 3], vel: [f32; 3], c0: u32, c1: u32, life: i32, base: i16, grow: i16, additive: bool) -> Option<usize> {
    let i = sys.create_part(TRAIL)?;
    let def = sys.def_first(TRAIL);
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_ff(r, 0x1c, 0.0);
    rec::set_u32(r, 4, c0);
    r[9] = 0xa4;
    r[3] = if additive { 0x48 } else { 0x44 };
    r[1] = 0;
    r[2] = def;
    r[8] = rot;
    rec::set_ff(r, 0xc, 0.0);
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x2c, 0.0);
    rec::set_i16(r, 0xa, life as i16);
    rec::set_i16(r, 0x3a, base);
    rec::set_u32(r, 0x3c, life as u32);
    rec::set_u32(r, 0x30, c0);
    rec::set_i16(r, 0x38, grow);
    rec::set_u32(r, 0x34, c1);
    Some(i)
}

/// `0x332d28`: size = (grow · ticks gone / life + base) · 1000; the velocity × 0.98, then the move; the turn; the colour
/// `0x2f0000(life left / life, c1, c0)`; the life down (out: killed).
pub fn update_trail(sys: &mut Particles, i: usize) {
    let r = &mut sys.pool.recs[i];
    let life0 = rec::u32(r, 0x3c) as i32;
    if life0 == 0 {
        sys.kill_part(i);
        return;
    }
    let life = rec::i16(r, 0xa) as i32;
    let size = rec::i16(r, 0x38) as i32 * (life0 - life) / life0 + rec::i16(r, 0x3a) as i32;
    rec::set_ff(r, 0xc, size as f32 * 1000.0);
    let v = rec::v3(r, 0x20).map(|x| x * 0.98);
    rec::set_v3(r, 0x20, v);
    let p = rec::v3(r, 0x10);
    rec::set_v3(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    r[8] = r[8].wrapping_add(1);
    rec::set_u32(r, 4, tween_color((life as f32 / life0 as f32).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30)));
    if dec_life(r) { sys.kill_part(i); }
}

/// Type 0xb, a billow: the Gravity Bomb's smoke.
pub const BILLOW: u8 = 0xb;

/// `0x333a70(size, speed, pos, vel, c0, c1, life, hold, gen, flags)`: a billow (type 0xb) at `pos` moving `vel` plus
/// `speed` a random way, for `life` ticks (none with 0); ALPHA 0x44 one time in five, else 0x48; near 4 / far 0xa.
/// (The frame-load throttles that drop some at a slow frame rate never fire here.)
#[allow(clippy::too_many_arguments)]
pub fn billow(sys: &mut Particles, rng: &mut Rng, size: f32, speed: f32, pos: [f32; 3], vel: [f32; 3], c0: u32, c1: u32, life: i32, hold: u8, gen: u8, flags: u8) -> Option<usize> {
    if life == 0 { return None; }
    let i = sys.create_part(BILLOW)?;
    let def = sys.def_first(BILLOW);
    let soft = rng.randi(100) < 0x14;
    let rot = rng.rand() as u8;
    let d = [rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0)];
    let d = set_len(d, speed);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_ff(r, 0x1c, 0.0);
    rec::set_u32(r, 4, c0);
    r[9] = 0xa4;
    r[1] = 0;
    rec::set_ff(r, 0xc, 0.0);
    r[3] = if soft { 0x44 } else { 0x48 };
    r[8] = rot;
    r[2] = def;
    rec::set_v3(r, 0x20, std::array::from_fn(|k| vel[k] + d[k]));
    rec::set_ff(r, 0x2c, speed * 0.75);
    rec::set_i16(r, 0xa, life as i16);
    r[0x39] = flags;
    r[0x3b] = life as u8;
    r[0x38] = hold;
    rec::set_ff(r, 0x3c, size);
    rec::set_u32(r, 0x30, c0);
    rec::set_u32(r, 0x34, c1);
    r[0x3a] = gen;
    Some(i)
}

/// `0x333d00`: a billow's tick (flags `+0x39`: 1 fading, 2 the first burst, 4 rising, 8 a long fade).
/// * Fading: it rises (1/1200 a tick more), moves and slows (× 0.98); its colour eases to the fade's over the first
///   quarter of the fade, then an additive one is gone and a blended one's alpha goes down; the life down (out: killed).
/// * Else it moves and slows (× 0.8, × 0.6 in the burst; rising: 0.0027 a tick up first); at 85% of its life a puff of
///   the first two generations not rising splits off five more (life × 0.9..1.1, speed × 0.85..1.0); not rising, its
///   size follows the curve through 0, `size` at its life's end (shrunk within 10 of the camera); the colour from `c0`
///   to `c1` (the burst: from white at a sixteenth to `c0`). Its life out: the burst rises for three lives at 1.6..2
///   × its speed; else it fades for 2..3 lives (×5 with flag 8), from `c1` to nothing (additive) or a dark grey.
pub fn update_billow(sys: &mut Particles, i: usize, cam: [f32; 3], rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(1);
    let flags = r[0x39];
    let l0 = r[0x3b] as f32;
    let (p, mut v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    if flags & 1 != 0 {
        let a = (l0 * 0.75) as i32 + 1;
        let b = (l0 * 0.25) as i32;
        v[2] += 0.000_833_333_35;
        let p = add(p, v);
        rec::set_v3(r, 0x10, p);
        rec::set_v3(r, 0x20, scale(v, 0.98));
        let life = rec::i16(r, 0xa) as i32;
        if a < life {
            rec::set_u32(r, 4, tween_color((((life - a) as f32) / (b + 1) as f32).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30)));
        } else if r[3] == 0x48 {
            rec::set_i16(r, 0xa, 0);
        } else {
            let c = rec::u32(r, 4);
            rec::set_u32(r, 4, c & 0xff_ffff | ((life * 15 / a) as u32) << 24);
        }
        if dec_life(r) { sys.kill_part(i); }
        return;
    }
    let p = add(p, v);
    rec::set_v3(r, 0x10, p);
    if flags & 4 != 0 { v[2] += 0.002_722_222_4; }
    v = scale(v, if flags & 2 != 0 { 0.6 } else { 0.8 });
    rec::set_v3(r, 0x20, v);
    if rec::i16(r, 0xa) == (l0 * 0.85) as i32 as i16 && r[0x3a] < 2 && flags & 4 == 0 {
        let (size, speed, c0, c1, hold, gen) = (rec::ff(r, 0x3c), rec::ff(r, 0x2c), rec::u32(r, 0x30), rec::u32(r, 0x34), r[0x38], r[0x3a]);
        for _ in 0..5 {
            let k = rng.randf(0.9, 1.1);
            let life = (l0 * k) as i32;
            let s = rng.randf(0.85, 1.0);
            billow(sys, rng, size, speed * s, p, v, c0, c1, life, hold, gen + 1, flags);
        }
    }
    let r = &mut sys.pool.recs[i];
    if flags & 4 == 0 {
        let s = rec::ff(r, 0x3c);
        let a8 = s * 8.0;
        let b = (l0 * 8.0 - l0 * s * 3.0) / (s * a8 + a8);
        let c = (l0 * 3.0 + b * a8) / (s * -8.0);
        let lv = rec::i16(r, 0xa) as f32;
        // `0x2eed78`: the VU0 square root (of the magnitude).
        let d = (b * b - c * (l0 - lv) * 4.0).abs().sqrt();
        let mut size = ((-b - d) / (c + c)).max((-b + d) / (c + c));
        let dist = len(sub(p, cam));
        if dist < 10.0 { size *= dist * 0.075 + 0.25; }
        rec::set_ff(r, 0xc, size);
    }
    let life = rec::i16(r, 0xa) as f32;
    let (a, b) = if flags & 2 == 0 { (rec::u32(r, 0x34), rec::u32(r, 0x30)) } else { (rec::u32(r, 0x30), 0x0fff_ffff) };
    rec::set_u32(r, 4, tween_color((life / l0).to_bits(), a, b));
    // `0x2eeb10(+0x38)`.
    r[0x38] = r[0x38].saturating_sub(1);
    if !dec_life(r) { return; }
    if flags & 2 == 0 {
        let c1 = rec::u32(r, 0x34);
        rec::set_u32(r, 0x30, c1);
        r[0x39] |= 1;
        rec::set_u32(r, 0x34, if r[3] == 0x48 { 0 } else { 0x0f2f_3f3f });
        let l = r[0x3b] as i32;
        let mut n = rng.rand_range(l * 2, l * 3) as u8;
        if r[0x39] & 8 != 0 { n = n.wrapping_mul(5); }
        r[0x3b] = n;
        rec::set_i16(r, 0xa, n as i16);
    } else {
        let l = (r[0x3b] as u16 * 3) as i16;
        rec::set_i16(r, 0xa, l);
        r[0x3b] = l as u8;
        r[0x39] = r[0x39] & 0xfd | 4;
        let k = rng.randf(1.6, 2.0);
        let r = &mut sys.pool.recs[i];
        rec::set_v3(r, 0x20, set_len(rec::v3(r, 0x20), rec::ff(r, 0x2c) * k));
    }
}

/// Type 0xf, an ember: the Gravity Bomb's sparks.
pub const EMBER: u8 = 0xf;

/// `0x3348c0(size, pos, vel, c0, c1, life, trail, def, alpha, w)`: an ember (type 0xf) at `pos` moving `vel`, falling
/// 0.004056 a tick, shrinking from `size` and going from `c0` to `c1` over `life` ticks (none with 0); `trail`: it
/// leaves embers behind every other frame; its frame `def` (None: its type's first), ALPHA `alpha` (None: 0x48), near 4
/// / far 0xa.
#[allow(clippy::too_many_arguments)]
pub fn ember(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 3], vel: [f32; 3], c0: u32, c1: u32, life: i32, trail: i16, def: Option<u8>, alpha: Option<u8>, w: i32) -> Option<usize> {
    if life == 0 { return None; }
    let i = sys.create_part(EMBER)?;
    let first = sys.def_first(EMBER);
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, c0);
    rec::set_ff(r, 0x1c, w as f32);
    r[9] = 0xa4;
    r[3] = alpha.unwrap_or(0x48);
    r[1] = 0;
    rec::set_ff(r, 0xc, size);
    r[8] = rot;
    r[2] = def.unwrap_or(first);
    rec::set_i16(r, 0xa, life as i16);
    rec::set_i16(r, 0x38, life as i16);
    rec::set_ff(r, 0x3c, size);
    rec::set_u32(r, 0x30, c0);
    rec::set_u32(r, 0x34, c1);
    rec::set_i16(r, 0x3a, trail);
    rec::set_v3(r, 0x20, vel);
    rec::set_u32(r, 0x2c, 0x3b84_e478);
    Some(i)
}

/// `0x334a18`: the life down (out: killed); size and colour by the life left; the move, then the fall; with a trail on
/// an odd frame (`frame`, `0x1b1948`) an ember behind it (its speed's 0.15..0.25 a random way added, × 0.75..0.95, 15..30
/// ticks, its own size, colour, frame and ALPHA, no trail), else the velocity × 0.96.
pub fn update_ember(sys: &mut Particles, i: usize, rng: &mut Rng, frame: i32) {
    let r = &mut sys.pool.recs[i];
    if dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let (life, l0) = (rec::i16(r, 0xa) as f32, rec::i16(r, 0x38) as f32);
    rec::set_ff(r, 0xc, life * rec::ff(r, 0x3c) / l0);
    rec::set_u32(r, 4, tween_color((life / l0).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30)));
    let p = add(rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, p);
    let mut v = rec::v3(r, 0x20);
    v[2] -= rec::ff(r, 0x2c);
    if rec::ff(r, 0x1c) != 0.0 { rec::set_ff(r, 0x1c, 2.0); }
    if rec::i16(r, 0x3a) == 0 || frame & 1 == 0 {
        rec::set_v3(r, 0x20, scale(v, 0.96));
        return;
    }
    rec::set_v3(r, 0x20, v);
    let (size, c, c1, def, alpha, w) = (rec::ff(r, 0xc), rec::u32(r, 4), rec::u32(r, 0x34), r[2], r[3], rec::ff(r, 0x1c) as i32);
    let d = [rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0)];
    let k = len(v) * rng.randf(0.15, 0.25);
    let t = add(set_len(d, k), v);
    let t = scale(t, rng.randf(0.75, 0.95));
    let life = rng.rand_range(15, 30);
    ember(sys, rng, size, p, t, c, c1, life, 0, Some(def), Some(alpha), w);
}

/// `UpdateParts` `0x331150`: every live record up to the high water mark at entry, in pool order (the mobys and the
/// camera for the glows that ride a moby, Ratchet's position `0x189ea0` for the records riding him; the game's random
/// stream; `aux`: what the water types read, [`water::PartAux`]).
pub fn update_parts(sys: &mut Particles, mobys: &crate::moby::MobyWorld, cam: [f32; 3], hero: [f32; 3], rng: &mut Rng, aux: &PartAux) {
    let end = (sys.pool.hw + 1).max(0) as usize;
    for i in 0..end {
        let r = &sys.pool.recs[i];
        if r[1] & 0x80 != 0 { continue; }
        match r[0] {
            0x17 => crate::particles::type23::update(sys, i),
            0x2c => update_spark(sys, i),
            DRIFT => crate::particles::type48::update48(sys, i),
            0x55 if rec::u32(r, 0x3c) == SPRITE_UPDATE => update_sprite(sys, i),
            0x59 => update_glow(sys, i, mobys, cam),
            0x5a => update_streak(sys, i, rng),
            0x61 => update_burst(sys, i),
            0x3b => update_sparkle(sys, i, hero),
            0x3c => update_glint(sys, i, hero),
            ORB => update_orb(sys, i, mobys),
            TRAIL => update_trail(sys, i),
            BILLOW => update_billow(sys, i, cam, rng),
            EMBER => update_ember(sys, i, rng, mobys.frame),
            water::BUBBLE => water::update_bubble(sys, i, rng, aux),
            water::DROP => water::update_drop(sys, i, rng, aux),
            water::RING => water::update_ring(sys, i, aux),
            water::SPRAY => water::update_spray(sys, i, aux),
            _ => sys.kill_part(i),
        }
    }
}
