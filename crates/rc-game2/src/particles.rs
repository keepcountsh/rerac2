//! Going Commando's particles: R&C1's pool and record layout (`rc_game::particles`: 2048 records of 0x40 bytes, the
//! allocator `0x330df0` = R&C1's `CreatePart`, the kill `0x330f40`, the def table `0x1fe8c0` that `0x2fe578` fills from
//! the same core-header words), so the engine's particle renderer draws them as it draws R&C1's. Only the spawners and
//! the update types differ: `UpdateParts` `0x331150` calls the type table `0x1fe6c0[type]` (set by `0x331ea0`) in pool
//! order; type 0x55's entry calls the function its spawner left at `+0x3c`.
//!
//! | Type | Spawner | Update | |
//! |---|---|---|---|
//! | 0x17 | [`puff`] `0x335bb8` | `0x335d80` = R&C1's type 23 (`rc_game::particles::type23::update`, the same steps at 60 Hz) | glow puffs |
//! | 0x2c | [`spark`] `0x339848` | `0x3399c8` ([`update_spark`]) | sparks: alpha down with the life, gravity, drag |
//! | 0x30 | [`drift`] `0x33a1c0` | `0x33a2e8` = R&C1's type 48 (`rc_game::particles::type48::update48`) | drifting smoke: fades in and out over the life |
//! | 0x55 | [`sprite`] `0x33ef78` | `+0x3c` = `0x33f120` ([`update_sprite`]) | sprites: alpha and size tweened over the life |
//! | 0x59 | [`glow`] `0x33f550` | `0x33f6e8` ([`update_glow`]) | a glow riding a moby (the hand item's), pulled toward the camera |
//!
//! Any other type kills itself on its first update. The gravity direction (`0x18a0e0`) is (0, 0, −1): normal gravity.
//! Not ported: the gravity-boots bend of the positions (`0x380e08`, `+0x1c` = 2).

use crate::rng::Rng;
use rc_game::particles::{rec, Particles};

/// `0x18a0e0`: the gravity direction.
pub const GRAVITY: [f32; 3] = [0.0, 0.0, -1.0];
/// The update a type-0x55 record carries at `+0x3c`.
const SPRITE_UPDATE: u32 = 0x33f120;

/// `0x2eeae0(rec + 0xa)`: the life down one; true when it was or reaches 0.
fn dec_life(r: &mut rc_game::particles::Record) -> bool {
    let mut t = rec::i16(r, 0xa) as i32;
    let done = crate::moby::countdown(&mut t);
    rec::set_i16(r, 0xa, t as i16);
    done
}

/// Type 0x30, the drifting particle: R&C1's type 48 (`rc_game::particles::type48`, the same record and update).
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
/// at `offset` in its rows, of `size` and colour `rgba`, its frame `def[0x59][1]`, pulled `pull` of the way toward
/// the camera, for `life` ticks (−1: while the moby lives); `facing`: faded unless it faces the camera
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

/// `0x33f6e8`: the life down (out, or its moby gone: killed); at its moby's position plus its rows × the offset, then
/// pulled toward the camera at `cam` by its pull.
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
    let pull = rec::ff(r, 0x28);
    rec::set_v3(r, 0x10, std::array::from_fn(|k| p[k] + (cam[k] - p[k]) * pull));
}

/// `UpdateParts` `0x331150`: every live record up to the high water mark at entry, in pool order (the mobys and the
/// camera for the glows that ride a moby).
pub fn update_parts(sys: &mut Particles, mobys: &crate::moby::MobyWorld, cam: [f32; 3]) {
    let end = (sys.pool.hw + 1).max(0) as usize;
    let mut none = rc_game::rng::Rng::new();
    for i in 0..end {
        let r = &sys.pool.recs[i];
        if r[1] & 0x80 != 0 { continue; }
        match r[0] {
            0x17 => rc_game::particles::type23::update(sys, i, &mut none),
            0x2c => update_spark(sys, i),
            DRIFT => rc_game::particles::type48::update48(sys, i, &mut none),
            0x55 if rec::u32(r, 0x3c) == SPRITE_UPDATE => update_sprite(sys, i),
            0x59 => update_glow(sys, i, mobys, cam),
            _ => sys.kill_part(i),
        }
    }
}
