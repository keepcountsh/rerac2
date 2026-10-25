//! The explosion flash (class 0x70, L00 `0x38e5a8` / `0x38e768`; the level's class table gives class 0x4a8 the same
//! update): a drawn ball tinted by its ambient that grows from a quarter of its full size to the whole of it over its
//! life, turns π/60 a tick about every axis, and fades out over its second half. It is what the blasts
//! (crate::moby::blast, their `+0x34` "light") and the enemies' deaths leave at the centre — a mesh, not one of the
//! point lights (crate::point_lights; the Lancer's flash and the light moby 0x27f are the point-light users).
//!
//! Its pvars: `+0x00` the vector the spawner is given (nothing reads it), `+0x10` the spawner's `k` (nothing reads it),
//! `+0x14` its life T, `+0x18` its full size (the class's scale · the spawner's size), `+0x1c` its timer (s16),
//! `+0x1e` its start alpha.

use crate::hero::physics::{add_rot, V3};
use crate::moby::{countdown, mode, Ctx, MobyWorld};

pub const CLASS: i16 = 0x70;
/// The second class the table maps to `0x38e768` (no spawner of it is ported).
pub const BOMB_CLASS: i16 = 0x4a8;

/// `0x3d567752`: the turn a tick about each axis (π/60).
const STEP: f32 = 0.052_359_88;
/// The head start of the timer: it is spawned `T − 4`, so the ball starts at 4/T of its full size.
const HEAD: i32 = 4;

/// `0x38e5a8(size, k, pos, vec, T, r, g, b, a)`: a flash at `pos`, three random Euler angles (`randf(−π, π)` for x,
/// y, z), state 0, alpha `a`, drawn, updated and drawn at any distance, its ambient `(r, g, b)` (`0x313300`), its
/// full size the class's scale · `size`, its timer `T − 4` and its size already `4 · full / T`; then its matrix
/// (`0x313dc0`). None when the level has no class 0x70.
#[allow(clippy::too_many_arguments)]
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, size: f32, k: f32, pos: V3, vec: V3, life: i32, rgb: [u8; 3], alpha: u8) -> Option<usize> {
    w.class(CLASS)?;
    let id = w.create(CLASS);
    let g = &mut ctx.game.rng;
    let rot = [g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI), g.randf(-std::f32::consts::PI, std::f32::consts::PI)];
    let m = &mut w.mobys[id];
    m.update = Some(update);
    m.mode &= !mode::NO_UPDATE;
    m.state = 0;
    m.alpha = alpha;
    m.drawn = 1;
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.ambient = rgb;
    let full = m.scale * size;
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.set_pv_f(0x18, full);
    m.set_pv_f(0x10, k);
    m.pos = pos;
    m.set_pv_v(0x00, vec);
    m.rot = rot;
    m.set_pv_i(0x14, life);
    let timer = (life - HEAD) as i16;
    m.set_pv_s(0x1c, timer);
    m.scale = size_at(life, timer, full);
    m.set_pv_s(0x1e, alpha as i16);
    w.rebuild(id, &*ctx.host);
    w.changed = true;
    Some(id)
}

/// `(T − timer) · full / T`.
fn size_at(life: i32, timer: i16, full: f32) -> f32 { (life - timer as i32) as f32 * full / life as f32 }

/// `0x38e768`: its size from its timer, each Euler angle on by π/60 (`0x2efe60`), the timer down (`0x2eeae0`: out,
/// the moby goes), and past the half-way mark its alpha `timer · a₀ / (T / 2)` (integer arithmetic).
pub fn update(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    let life = m.pv_i(0x14);
    let mut timer = m.pv_s(0x1c) as i32;
    m.scale = size_at(life, timer as i16, m.pv_f(0x18));
    for k in 0..3 { m.rot[k] = add_rot(m.rot[k], STEP); }
    let out = countdown(&mut timer);
    m.set_pv_s(0x1c, timer as i16);
    if out {
        w.delete(id);
        return;
    }
    let half = life / 2;
    if half != 0 && timer < half { m.alpha = (timer * m.pv_s(0x1e) as i32 / half) as u8; }
}
