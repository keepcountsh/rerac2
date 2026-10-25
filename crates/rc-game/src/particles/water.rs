//! The water particles: the four types the hero's swim effects spawn (crate::hero::water), each with R&C2's
//! spawner and update (L00; the type table `0x1fe6c0[type]` that `0x331ea0` fills).
//!
//! | Type | Spawner | Update | |
//! |---|---|---|---|
//! | 0x22 | [`bubble`] `0x3378b0(size, top, pos, vel)` | `0x337a78` ([`update_bubble`]) | a bubble: rises to `top` (the water level), wobbles sideways to the camera, pops |
//! | 0x23 | [`drop`] `0x337cc0(pos, vel, kind, life)` | `0x337dd8` ([`update_drop`]) | a drop of water: falls; kind 1 rings the water face it hits, kind 3 rings the hero's water level |
//! | 0x2d | [`ring`] `0x339b60(size, grow, pos, &height, rgba)` | `0x339cc0` ([`update_ring`]) | a ring on the water: grows, fades one alpha step every 4 ticks |
//! | 0x2e | [`spray`] `0x339e10(size, spin, pos, vel, &height)` | `0x339f30` ([`update_spray`]) | the bow spray: grows, spins down, fades |
//!
//! The ring and the spray keep a pointer to the height they sit on: the hero's water level `0x18a150` on every call
//! the hero makes (here [`FOLLOW_WATER`] at that slot, read as [`PartAux::water_z`]), and `0x1aa1b4` (0 on the disc:
//! no height) for the rings a falling drop leaves. The bubbles' sideways wobble is across the camera's yaw
//! (`0x1b8d58`, [`PartAux::cam_yaw`]); a kind-1 drop tests the world with a line (flags 2, [`PartAux::coll`]).
//!
//! Not ported: a kind-2 drop's splat on a non-water face (`0x33c390`, type 0x33: the hero spawns kinds 0, 1 and 3
//! only), the splash moby 0x307 (`0x39f938`: the class is in no level's table, so `CreateMoby` returns 0 in the game
//! too).

use super::{dec_life, rec, Particles};
use crate::collision::{coll_line_m, QueryFlags};
use crate::hero::physics::{add, len2, set_len2};
use crate::rng::Rng;
use rc_formats::collision::Collision;

pub const BUBBLE: u8 = 0x22;
pub const DROP: u8 = 0x23;
pub const RING: u8 = 0x2d;
pub const SPRAY: u8 = 0x2e;

/// The height slot's value for "follow the hero's water level" (the game's pointer to `0x18a150`).
pub const FOLLOW_WATER: u32 = 1;

/// What the water updates read of the world each tick.
#[derive(Clone, Copy)]
pub struct PartAux<'a> {
    /// The camera's yaw (`0x1b8d58`).
    pub cam_yaw: f32,
    /// The hero's water level (`0x18a150`).
    pub water_z: f32,
    /// The level's collision (the drops' line test).
    pub coll: Option<&'a Collision>,
}

/// `0x3378b0(size, top, pos, vel)`: a bubble (type 0x22) at `pos` moving `vel` a tick, its size 5250 growing to
/// `randf(0.75, 1.25)·size`, rising until `top`, colour 0x88a0a0a0, def 0x22, for `rand_range(70, 140)` ticks. The
/// hero's calls pass `top` −1: the water level (his place + 0.4 in the slide 0x30), resolved by the caller.
pub fn bubble(sys: &mut Particles, rng: &mut Rng, size: f32, top: f32, pos: [f32; 3], vel: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(BUBBLE)?;
    let def = sys.def_first(BUBBLE);
    let max = rng.randf(0.75, 1.25) * size;
    let rot = (rng.rand_range(-10, 10) - 0x80) as u8;
    let life = rng.rand_range(0x46, 0x8c) as i16;
    let angle = rng.rand_angle();
    let turn = rng.randf(0.029_077_28, 0.116_309_12);
    let sway = rng.randf(0.008_333_334, 0.023_333_335);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, 0x88a0_a0a0);
    r[1] = 0;
    r[3] = 0x44;
    r[9] = 0x40 + 4;
    rec::set_ff(r, 0x2c, max);
    rec::set_ff(r, 0xc, 5250.0);
    r[8] = rot;
    r[2] = def;
    rec::set_i16(r, 0xa, life);
    rec::set_ff(r, 0x30, top);
    rec::set_ff(r, 0x34, angle);
    rec::set_ff(r, 0x38, turn);
    rec::set_ff(r, 0x3c, sway);
    rec::set_v3(r, 0x20, vel);
    rec::set_v3(r, 0x10, pos);
    Some(i)
}

/// `0x337a78`: the life down (out: killed). Above `top` the rise stops and the life is cut to 12; within 0.8667 of it
/// one in twenty ticks cuts the life to 12; under 14 the alpha scales with it. The size grows 840 a tick to its
/// maximum; the velocity shrinks to 0.97 of its length and its rise eases toward 0.02166 by 1/720; the angle turns and
/// the bubble sways `sin(angle)·sway` across the camera.
pub fn update_bubble(sys: &mut Particles, i: usize, rng: &mut Rng, aux: &PartAux) {
    let r = &mut sys.pool.recs[i];
    if dec_life(r) {
        sys.kill_part(i);
        return;
    }
    let top = rec::ff(r, 0x30);
    let z = rec::ff(r, 0x18);
    if top < z {
        rec::set_ff(r, 0x28, 0.0);
        if 0xd <= rec::i16(r, 0xa) { rec::set_i16(r, 0xa, 0xc); }
    }
    if top < z + 0.866_666_7 && rng.randi(0x14) == 0 && 0xd <= rec::i16(r, 0xa) { rec::set_i16(r, 0xa, 0xc); }
    let life = rec::i16(r, 0xa) as i32;
    if life < 0xe {
        let c = rec::u32(r, 4);
        rec::set_u32(r, 4, (((c >> 24) as i32 * life / 0xe) as u32) << 24 | c & 0xff_ffff);
    }
    let size = (rec::ff(r, 0xc) + 840.000_06).min(rec::ff(r, 0x2c));
    rec::set_ff(r, 0xc, size);
    let v = rec::v3(r, 0x20);
    let l = len2(v);
    let mut v = set_len2(v, l * 0.97);
    rec::set_v3(r, 0x10, add(rec::pos(r), v));
    crate::hero::physics::approach(0.021_660_36, 0.001_388_888_9, &mut v[2]);
    rec::set_v3(r, 0x20, v);
    let a = crate::hero::physics::add_rot(rec::ff(r, 0x34), rec::ff(r, 0x38));
    rec::set_ff(r, 0x34, a);
    let side = crate::hero::physics::add_rot(aux.cam_yaw, std::f32::consts::FRAC_PI_2);
    let k = a.sin() * rec::ff(r, 0x3c);
    rec::set_v3(r, 0x10, add(rec::pos(r), [side.cos() * k, side.sin() * k, 0.0]));
}

/// `0x337cc0(pos, vel, kind, life)`: a drop (type 0x23) at `pos` moving `vel` a tick, size `randf(10500, 16800)`,
/// colour 0x80808080, def 0x23, falling `randf(1/342.9, 1/200)` a tick² for `life` ticks or until it is under its
/// start height (kind 2: under 0). Kind 0 falls, 1 rings the water face it meets (and dies on any face), 3 rings the
/// hero's water level when it reaches it.
pub fn drop(sys: &mut Particles, rng: &mut Rng, pos: [f32; 3], vel: [f32; 3], kind: i32, life: i32) -> Option<usize> {
    let i = sys.create_part(DROP)?;
    let def = sys.def_first(DROP);
    let size = rng.randf(10_500.0, 16_800.0);
    let g = rng.randf(0.002_916_666_8, 0.005);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, 0x8080_8080);
    r[1] = 0;
    r[9] = 0x10 + 2;
    r[3] = 0x44;
    rec::set_ff(r, 0xc, size);
    r[2] = def;
    rec::set_v3(r, 0x20, vel);
    rec::set_u32(r, 0x34, life as u32);
    rec::set_u32(r, 0x30, kind as u32);
    rec::set_ff(r, 0x38, if kind == 2 { 0.0 } else { pos[2] });
    rec::set_ff(r, 0x3c, g);
    Some(i)
}

/// `0x337dd8` (the module docs).
pub fn update_drop(sys: &mut Particles, i: usize, rng: &mut Rng, aux: &PartAux) {
    let r = &mut sys.pool.recs[i];
    let p = rec::pos(r);
    let v = rec::v3(r, 0x20);
    let next = add(p, v);
    match rec::u32(r, 0x30) {
        0 => rec::set_v3(r, 0x10, next),
        1 => {
            let hit = aux.coll.and_then(|c| coll_line_m(c, None, p, next, QueryFlags(2), None));
            match hit {
                None => rec::set_v3(r, 0x10, next),
                Some(o) => {
                    if o.surface_id() == 0 { ring(sys, rng, 0.2, 5250.0, o.point, false, None); }
                    sys.kill_part(i);
                    return;
                }
            }
        }
        3 => {
            rec::set_v3(r, 0x10, next);
            if next[2] < aux.water_z {
                rec::set_ff(r, 0x18, aux.water_z);
                let at = rec::pos(r);
                ring(sys, rng, 0.2, 5250.0, at, false, None);
                sys.kill_part(i);
                return;
            }
        }
        _ => {
            // Kind 2: its splat on the face it meets (`0x33c390`) is not ported; it dies there.
            let hit = aux.coll.and_then(|c| coll_line_m(c, None, p, next, QueryFlags(2), None));
            match hit {
                None => rec::set_v3(r, 0x10, next),
                Some(_) => {
                    sys.kill_part(i);
                    return;
                }
            }
        }
    }
    let r = &mut sys.pool.recs[i];
    rec::set_ff(r, 0x28, rec::ff(r, 0x28) - rec::ff(r, 0x3c));
    r[8] = r[8].wrapping_add(2);
    let life = rec::u32(r, 0x34) as i32 - 1;
    rec::set_u32(r, 0x34, life as u32);
    if life < 1 || rec::ff(r, 0x18) < rec::ff(r, 0x38) {
        let c = (rec::u32(r, 4) as i32).wrapping_sub(0x1000_0000);
        rec::set_u32(r, 4, c as u32);
        if c < 0 { sys.kill_part(i); }
    }
}

/// `0x339b60(size, grow, pos, &height, rgba)`: a ring (type 0x2d) at `pos`, size `size`·210000 growing `grow` a tick,
/// drifting `randf(0.0015, 0.0025)` a tick at a random angle, def 0x2d, additive, alpha `randi(6) + 0x14` over grey
/// 0x808080 when `rgba` is None. `follow`: it sits 0.02 above the hero's water level.
pub fn ring(sys: &mut Particles, rng: &mut Rng, size: f32, grow: f32, pos: [f32; 3], follow: bool, rgba: Option<u32>) -> Option<usize> {
    let i = sys.create_part(RING)?;
    let def = sys.def_first(RING);
    let rgba = rgba.unwrap_or_else(|| ((rng.randi(6) + 0x14) as u32) << 24 | 0x80_8080);
    let rot = rng.rand() as u8;
    let a = rng.rand_angle();
    let k = rng.randf(0.0015, 0.0025);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    r[1] = 1;
    r[9] = 0x20 + 1;
    r[3] = 0x48;
    r[2] = def;
    r[8] = rot;
    rec::set_i16(r, 0xa, 4);
    rec::set_ff(r, 0xc, size * 210_000.0);
    rec::set_v3(r, 0x20, [a.cos() * k, a.sin() * k, 0.0]);
    rec::set_u32(r, 0x34, if follow { FOLLOW_WATER } else { 0 });
    rec::set_ff(r, 0x30, grow);
    Some(i)
}

/// `0x339cc0`: it drifts; killed outside [2, 1021]³; the angle turns one step, the size grows; on the followed height
/// + 0.02; every fourth tick one alpha step off, killed at none.
pub fn update_ring(sys: &mut Particles, i: usize, aux: &PartAux) {
    let r = &mut sys.pool.recs[i];
    let p = add(rec::pos(r), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, p);
    if p.iter().any(|&c| c < 2.0 || 1021.0 < c) {
        sys.kill_part(i);
        return;
    }
    r[8] = r[8].wrapping_add(1);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x30));
    if rec::u32(r, 0x34) == FOLLOW_WATER { rec::set_ff(r, 0x18, aux.water_z + 0.02); }
    let life = rec::i16(r, 0xa);
    rec::set_i16(r, 0xa, life - 1);
    if life == 1 {
        rec::set_i16(r, 0xa, 4);
        rec::set_u32(r, 4, rec::u32(r, 4).wrapping_sub(0x100_0000));
    }
    if rec::u32(r, 4) & 0xff00_0000 == 0 { sys.kill_part(i); }
}

/// `0x339e10(size, spin, pos, vel, &height)`: the bow spray (type 0x2e) at `pos` moving `vel` a tick, size
/// `size`·210000, def 0x2e, additive, alpha `randi(10) + 0x26` over grey, its angle `randf(0, 256)` turning `spin` a
/// tick, for 5 ticks before it fades. `follow`: snaps to 0.02 above the hero's water level while within 0.2 of it.
pub fn spray(sys: &mut Particles, rng: &mut Rng, size: f32, spin: f32, pos: [f32; 3], vel: [f32; 3], follow: bool) -> Option<usize> {
    let i = sys.create_part(SPRAY)?;
    let def = sys.def_first(SPRAY);
    let rgba = ((rng.randi(10) + 0x26) as u32) << 24 | 0x80_8080;
    let angle = rng.randf(0.0, 256.0);
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    r[9] = 0x20 + 1;
    r[3] = 0x48;
    r[1] = 1;
    rec::set_i16(r, 0xa, 5);
    r[2] = def;
    rec::set_ff(r, 0xc, size * 210_000.0);
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    rec::set_u32(r, 0x38, if follow { FOLLOW_WATER } else { 0 });
    rec::set_ff(r, 0x34, spin);
    Some(i)
}

/// `0x339f30`: it moves, grows 5880 a tick, its spin decays to 0.98 and turns the angle; on the followed height when
/// within 0.2; the life down to 0 then one alpha step off a tick, killed at none.
pub fn update_spray(sys: &mut Particles, i: usize, aux: &PartAux) {
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, add(rec::pos(r), rec::v3(r, 0x20)));
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 5880.0);
    let spin = rec::ff(r, 0x34) * 0.98;
    let angle = rec::ff(r, 0x30) + spin;
    rec::set_ff(r, 0x34, spin);
    rec::set_ff(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    if rec::u32(r, 0x38) == FOLLOW_WATER && (rec::ff(r, 0x18) - aux.water_z).abs() < 0.2 { rec::set_ff(r, 0x18, aux.water_z + 0.02); }
    let life = rec::i16(r, 0xa) - 1;
    rec::set_i16(r, 0xa, life);
    if life == 0 {
        rec::set_i16(r, 0xa, 1);
        rec::set_u32(r, 4, rec::u32(r, 4).wrapping_sub(0x100_0000));
    }
    if rec::u32(r, 4) & 0xff00_0000 == 0 { sys.kill_part(i); }
}
