//! Break pieces (L00): a moby that breaks throws copies of itself, each drawn as one of its class's pieces
//! (`rc_formats::moby::MobyPiece`, the `+0x7c` mask), which tumble, bounce on the level and fade.
//!
//! * **Spawn** `0x327498(h₀, h₁, v₀, v₁, moby, mask, velocity, sound)` ([`break_into`]): for each bit k of the mask (0..14)
//!   a piece flies off at a random heading, `h₀..h₁` and `v₀..v₁` units a second across and up, plus the velocity;
//!   `0x2dbdb0` makes it ([`spawn_piece`]): a moby of the same class at the moby's place and angles, its scale, mode
//!   `& ~0x20 | 0x100` (`| 0x800` when the moby has it), no collision, its source `+0x54`, a spin of 90..270°/s about a
//!   random axis; `0x2dbff8` mode 2 its body: the velocity, the spin (a quaternion a tick), the piece's centre as the
//!   pivot (in the moby's frame), gravity 10..20 /3600 a tick², 180 ticks, the bounce sound.
//! * **Tick** `0x39c4a8` → `0x2dc1f0` → `0x2dc390` ([`update`]): with no contact it falls and spins about its pivot
//!   (`0x2dc7a8`); with one or two it pivots about them under its own weight (`0x2dc968`, two: about their line); a
//!   point of the piece that crosses the level (the segment from the pivot to it, `0x2dcb80`) pushes it out, turns the
//!   spin by `velocity × normal` (`0x2dd108`, at most 0.75° a tick, the whole spin 1.5°) and bounces the velocity off
//!   the face (`0x2dd228`: on a floor, the normal within 45° of up, it loses 0.45 of its speed, at least 0.033 a tick);
//!   a bounce that leaves no speed makes the point a contact (`0x2dcde8`: the contacts that no longer hold go,
//!   `0x2dce90`; two make a hinge, `0x2dd1d8`); three contacts: at rest. The life counts down (every fourth tick
//!   while it flies freely); at its end (or once one in ten ticks while the frame rate is low) it fades, 2 alpha a tick,
//!   and goes.
//!
//! **pvars** (`+0x68`, 0x80): `+0x00` velocity, `+0x10` the spin, `+0x20` the pivot, `+0x34` the life (s16), `+0x36`
//! the piece, `+0x38` 0.45 the floor's loss, `+0x3c` its least, `+0x40` 0.5, `+0x44` sin 0.75°, `+0x48` the gravity,
//! `+0x4c` π/1800 the weight's turn, `+0x54` the source, `+0x58` the points, `+0x59..+0x5b` the contacts, `+0x5c`
//! their count, `+0x5d` the mode (2), `+0x5e` the sound (0xff none).
//!
//! The contacts test the level's mesh with the port's segment query, as `0x2e2248` tests the triangles `0x2df8d8`
//! gathers. Not ported: the gravity frames (`0x1a8ff0`), the moby's colour taken over (`0x31fa50` / `0x3241d8`), the
//! other modes (0 and 3 the whole moby, 1 its joints' segments, 4 / 5).

use super::{mode, Ctx, MobyWorld};
use crate::hero::physics::{add, dot, len, scale, set_len, sub, V3};

fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }

pub(crate) const VEL: usize = 0x00;
pub(crate) const SPIN: usize = 0x10;
const PIVOT: usize = 0x20;
const LIFE: usize = 0x34;
const PIECE: usize = 0x36;
const LOSS: usize = 0x38;
const LEAST: usize = 0x3c;
pub(crate) const GRAVITY: usize = 0x48;
const WEIGHT: usize = 0x4c;
const SOURCE: usize = 0x54;
const POINTS: usize = 0x58;
const CONTACTS: usize = 0x59;
pub(crate) const COUNT: usize = 0x5c;
const MODE: usize = 0x5d;
const SOUND: usize = 0x5e;
const BLOCK: usize = 0x80;

type Q = [f32; 4];

fn quat(m: &super::Moby, o: usize) -> Q { [m.pv_f(o), m.pv_f(o + 4), m.pv_f(o + 8), m.pv_f(o + 12)] }
fn set_quat(m: &mut super::Moby, o: usize, q: Q) { for (k, v) in q.iter().enumerate() { m.set_pv_f(o + 4 * k, *v); } }

/// `0x321bb8(s, v)`: the quaternion of axis `v` with sin(half angle) `s`.
pub(crate) fn axis_quat(s: f32, v: V3) -> Q {
    let a = set_len(v, s);
    [a[0], a[1], a[2], (1.0 - s * s).max(0.0).sqrt()]
}

/// `0x2efa98`: `a · b`.
fn qmul(a: Q, b: Q) -> Q {
    let (av, bv) = ([a[0], a[1], a[2]], [b[0], b[1], b[2]]);
    let c = cross(av, bv);
    let v: V3 = std::array::from_fn(|k| a[3] * bv[k] + b[3] * av[k] + c[k]);
    [v[0], v[1], v[2], a[3] * b[3] - dot(av, bv)]
}

/// The rotation of quaternion `q` on `v` (the rows through `0x2efc20` / `0x2efa00`).
pub(crate) fn rotate(q: Q, v: V3) -> V3 {
    let u = [q[0], q[1], q[2]];
    let t = scale(cross(u, v), 2.0);
    add(add(v, scale(t, q[3])), cross(u, t))
}

/// `0x2ef318`: a frame vector through the rows.
fn through(rows: &[V3; 3], v: V3) -> V3 { std::array::from_fn(|k| v[0] * rows[0][k] + v[1] * rows[1][k] + v[2] * rows[2][k]) }

/// `0x327498(h₀, h₁, v₀, v₁, moby, mask, velocity, sound)` (module doc).
#[allow(clippy::too_many_arguments)]
pub fn break_into(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, h: (f32, f32), v: (f32, f32), mask: u32, velocity: V3, sound: u8) {
    for k in 0..MAX_PIECE {
        if mask >> k & 1 == 0 { continue; }
        let rng = &mut ctx.game.rng;
        let yaw = rng.rand_angle();
        let across = rng.randf(h.0, h.1) * 0.016_666_668;
        let up = rng.randf(v.0, v.1) * 0.016_666_668;
        let vel = add([yaw.cos() * across, yaw.sin() * across, up], velocity);
        let spin = rng.randf(90.0, 270.0);
        let gravity = rng.randf(10.0, 20.0);
        spawn_piece(w, id, ctx, spin, gravity, k as u8, vel, sound);
    }
}

/// The mask's bits.
const MAX_PIECE: usize = 15;

/// A piece of the source moby `src`, at its place and angles ([`spawn_shard`]).
#[allow(clippy::too_many_arguments)]
pub fn spawn_piece(w: &mut MobyWorld, src: usize, ctx: &mut Ctx, spin: f32, gravity: f32, piece: u8, vel: V3, sound: u8) -> Option<usize> {
    let s = &w.mobys[src];
    let (o_class, pos, rot, rows) = (s.o_class, s.pos, s.rot, s.rows);
    spawn_shard(w, ctx, o_class, Some(src), pos, rot, Some(rows), spin, gravity, piece, vel, sound)
}

/// `0x2dbdb0(spin, gravity, src, 2, class, piece, place, angles, velocity, sound)` and `0x2dbff8` mode 2: a moby of
/// `o_class` drawn as its piece `piece`, at `pos` with the angles `rot` (or the source's rows); from the source its
/// light and ambient, its scale ratio (its scale over its class's) and its mode 0x800.
#[allow(clippy::too_many_arguments)]
pub fn spawn_shard(w: &mut MobyWorld, ctx: &mut Ctx, o_class: i16, src: Option<usize>, pos: V3, rot: V3, rows: Option<[V3; 3]>, spin: f32, gravity: f32, piece: u8, vel: V3, sound: u8) -> Option<usize> {
    let info = w.class(o_class)?;
    let class_scale = info.scale;
    let (centre, n) = match info.pieces.get(piece as usize) { Some(Some((c, p))) => (*c, p.len()), _ => return None };
    let (ratio, src_mode, light, ambient) = match src.and_then(|s| w.mobys.get(s)) {
        Some(s) => (w.class(s.o_class).map_or(1.0, |c| if c.scale != 0.0 { s.scale / c.scale } else { 1.0 }), s.mode, s.light, s.ambient),
        None => (1.0, 0, 0, [0x40; 3]),
    };
    let sc = class_scale * ratio;
    // Its spin: sin(spin / 2 a tick) about a random axis (`0x31bbc0`: its length drawn too, from sin..sin).
    let half = (spin * 0.000_145_444_12).sin();
    let rng = &mut ctx.game.rng;
    rng.randf(half, half);
    let (az, el) = (rng.rand_angle(), rng.rand_angle());
    let axis = [az.cos() * el.sin() * half, az.sin() * el.sin() * half, el.cos() * half];
    let id = w.create(o_class);
    let m = &mut w.mobys[id];
    (m.pos, m.rot, m.scale) = (pos, rot, sc);
    m.rows = rows.unwrap_or_else(|| crate::hero::physics::euler_rows(rot));
    (m.light, m.ambient) = (light, ambient);
    m.mode = (m.mode & !0x20) | mode::KEEP_ROWS | (src_mode & 0x800) | mode::NO_MATRIX;
    m.has_collision = false;
    (m.drawn, m.draw_dist, m.update_dist) = (1, 0x1ff, 0xff);
    m.pvars.resize(BLOCK.max(m.pvars.len()), 0);
    m.set_pv_v(VEL, vel);
    set_quat(m, SPIN, [axis[0], axis[1], axis[2], (1.0 - half * half).max(0.0).sqrt()]);
    m.set_pv_f(LOSS, 0.45);
    m.set_pv_f(LEAST, 0.033_333_335);
    m.set_pv_f(0x40, 0.5);
    m.set_pv_f(0x44, 0.013_089_969);
    m.set_pv_f(GRAVITY, gravity * 0.000_277_777_8);
    m.set_pv_f(WEIGHT, 0.001_745_329_3);
    m.set_pv_s(LIFE, 0xb4);
    m.set_pv_s(PIECE, piece as i16);
    m.set_pv_i(SOURCE, src.map_or(-1, |s| s as i32));
    m.pvars[POINTS] = n as u8;
    m.pvars[COUNT] = 0;
    m.pvars[MODE] = 2;
    m.pvars[SOUND] = sound;
    m.piece = Some(piece);
    // The pivot: the piece's centre in the moby's frame (`0x311cf0`, then `0x326248`).
    let k = sc * 0.000_976_562_5;
    m.set_pv_v(PIVOT, scale(centre, k));
    m.update = Some(update);
    Some(id)
}

/// The piece's points in its frame (`0x311cf0`'s list, scaled).
fn points(w: &MobyWorld, id: usize) -> Vec<V3> {
    let m = &w.mobys[id];
    let k = m.scale * 0.000_976_562_5;
    let piece = m.pv_s(PIECE).max(0) as usize;
    w.class(m.o_class).and_then(|c| c.pieces.get(piece)).and_then(Option::as_ref).map(|(_, p)| p.iter().map(|&q| scale(q, k)).collect()).unwrap_or_default()
}

/// `0x39c4a8`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => {
            if step(w, id, ctx) {
                w.mobys[id].set_state(1, -1);
                w.mobys[id].mode |= 0x2800;
            }
        }
        1 => {
            step(w, id, ctx);
            if w.mobys[id].alpha < 2 { w.delete(id); } else { w.mobys[id].alpha -= 2; }
        }
        _ => {}
    }
}

/// `0x2dc1f0` mode 2: true when its life is over (or it left the world).
pub(crate) fn step(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let pts = points(w, id);
    let mut done = body(w, id, ctx, &pts);
    let m = &mut w.mobys[id];
    if m.pvars[COUNT] != 0 || ctx.game.frame & 3 == 0 {
        let t = m.pv_s(LIFE);
        if t <= 1 { done = true; }
        m.set_pv_s(LIFE, (t - 1).max(0));
    }
    let p = m.pos;
    let inside = |c: f32| (2.0..=1021.0).contains(&c);
    done || !(inside(p[0]) && inside(p[1]) && inside(p[2]))
}

/// `0x2dc390`: one tick of the body; true when it lies still (three contacts).
fn body(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, pts: &[V3]) -> bool {
    if 3 <= w.mobys[id].pvars[COUNT] { return true; }
    if w.mobys[id].pvars[COUNT] == 0 { fly(w, id) } else { pivot(w, id, pts) }
    let Some((k, normal)) = contact(w, id, ctx, pts) else { return false };
    let n = set_len(normal, 1.0);
    let m = &mut w.mobys[id];
    let vel = m.pv_v(VEL);
    let turn = cross(vel, n);
    let spin = add_turn(quat(m, SPIN), turn);
    set_quat(m, SPIN, spin);
    let (loss, least) = (m.pv_f(LOSS), m.pv_f(LEAST));
    let (moving, vel) = bounce(loss, least, vel, n, [0.0, 0.0, 1.0]);
    m.set_pv_v(VEL, vel);
    let sound = m.pvars[SOUND];
    let free = m.pvars[COUNT] == 0;
    if !moving {
        if sound != 0xff && free { ctx.host.play_sound(&w.mobys[id], sound as i32, 0); }
        add_contact(w, id, k, n, pts);
    } else if sound != 0xff {
        ctx.host.play_sound(&w.mobys[id], sound as i32, 0);
    }
    false
}

/// `0x2dc7a8`: falls, moves, and spins about its pivot.
fn fly(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    let mut vel = m.pv_v(VEL);
    vel[2] -= m.pv_f(GRAVITY);
    m.set_pv_v(VEL, vel);
    m.pos = add(m.pos, vel);
    let pivot = m.pv_v(PIVOT);
    let before = through(&m.rows, pivot);
    let q = quat(m, SPIN);
    m.rows = m.rows.map(|r| rotate(q, r));
    let after = through(&m.rows, pivot);
    m.pos = sub(m.pos, sub(after, before));
}

/// `0x2dc968`: tips over its contact (or the line of its two) under its weight.
fn pivot(w: &mut MobyWorld, id: usize, pts: &[V3]) {
    let m = &mut w.mobys[id];
    let n = m.pvars[COUNT];
    let c0 = m.pvars[CONTACTS] as usize;
    let c1 = m.pvars[CONTACTS + 1] as usize;
    let (Some(&p0), p1) = (pts.get(c0), pts.get(c1).copied()) else { return };
    let (at, hinge) = if n < 2 { (p0, None) } else {
        let p1 = p1.unwrap_or(p0);
        (scale(add(p0, p1), 0.5), Some(through(&m.rows, sub(p0, p1))))
    };
    let before = through(&m.rows, at);
    let arm = set_len(through(&m.rows, sub(at, m.pv_v(PIVOT))), 1.0);
    let torque = set_len(cross([0.0, 0.0, 1.0], arm), m.pv_f(WEIGHT));
    let mut spin = add_turn(quat(m, SPIN), torque);
    if let Some(h) = hinge { spin = hinge_spin(spin, h); }
    set_quat(m, SPIN, spin);
    m.rows = m.rows.map(|r| rotate(spin, r));
    let after = through(&m.rows, at);
    m.pos = sub(m.pos, sub(after, before));
}

/// `0x2dd108(moby, spin, turn)`: the spin turned by `turn` (its half angle at most sin 0.75°; the spin at most 1.5°).
fn add_turn(spin: Q, turn: V3) -> Q {
    let l = len(turn);
    if l == 0.0 { return spin; }
    let (a, b) = (0.013_089_969_f32.sin(), 0.026_179_939_f32.sin());
    let q = qmul(spin, axis_quat((l * 0.5).min(a), turn));
    if b < len([q[0], q[1], q[2]]) { axis_quat(b, [q[0], q[1], q[2]]) } else { q }
}

/// `0x2dd1d8(moby, spin, d)`: the spin kept about `d` only.
fn hinge_spin(spin: Q, d: V3) -> Q {
    let u = set_len(d, 1.0);
    let s = dot(u, [spin[0], spin[1], spin[2]]);
    axis_quat(s.abs().min(1.0), scale(u, s.signum()))
}

/// `0x2dd228(loss, least, velocity, normal, up)`: the velocity off the face (`0x2ef2a8`); on a floor (`normal · up >
/// 0.7`) it loses `loss` of its speed, at least `least`. Whether any speed is left.
fn bounce(loss: f32, least: f32, vel: V3, n: V3, up: V3) -> (bool, V3) {
    let d = dot(vel, n);
    let mut v = if d < 0.0 { sub(vel, scale(n, 2.0 * d)) } else { vel };
    let mut l = len(v);
    if 0.7 < dot(up, n) {
        let f = (l * loss).max(least);
        l = if l <= f { 0.0 } else { l - f };
        v = set_len(v, l);
    }
    (l != 0.0, v)
}

/// `0x2dcb80`: the first point (not a contact) whose segment from the pivot crosses the level: the piece pushed out
/// along the face's normal, the point and the normal; None when none does.
fn contact(w: &mut MobyWorld, id: usize, ctx: &Ctx, pts: &[V3]) -> Option<(u8, V3)> {
    let coll = ctx.coll?;
    let m = &w.mobys[id];
    let centre = add(through(&m.rows, m.pv_v(PIVOT)), m.pos);
    let held = &m.pvars[CONTACTS..CONTACTS + m.pvars[COUNT].min(3) as usize];
    for (i, &p) in pts.iter().enumerate() {
        if held.contains(&(i as u8)) { continue; }
        let at = add(through(&m.rows, p), m.pos);
        let Some(o) = crate::collision::coll_line_m(coll, None, centre, at, crate::collision::QueryFlags(0), None) else { continue };
        let mut n = set_len(o.normal, 1.0);
        // The face's side the pivot is on.
        if dot(sub(centre, o.point), n) < 0.0 { n = scale(n, -1.0); }
        let depth = dot(sub(at, o.point), n);
        let m = &mut w.mobys[id];
        m.pos = sub(m.pos, scale(n, depth));
        return Some((i as u8, n));
    }
    None
}

/// `0x2dcde8(moby, point)` with `0x2dce90`: the point a contact; the others that would lift off the face go; two make
/// a hinge.
fn add_contact(w: &mut MobyWorld, id: usize, k: u8, n: V3, pts: &[V3]) {
    let m = &mut w.mobys[id];
    let c = m.pvars[COUNT].min(2) as usize;
    m.pvars[CONTACTS + c] = k;
    m.pvars[COUNT] = c as u8 + 1;
    // `0x2dce90`: in the face's plane, a contact on the far side of the new one from it goes.
    let flat = |v: V3| sub(v, scale(n, dot(v, n)));
    let Some(&pk) = pts.get(k as usize) else { return };
    let base = flat(through(&m.rows, pk));
    let mut j = 0;
    while j < m.pvars[COUNT] as usize {
        let cj = m.pvars[CONTACTS + j];
        if cj != k {
            let q = pts.get(cj as usize).map_or([0.0; 3], |&p| flat(through(&m.rows, sub(p, pk))));
            if 0.0 < dot(q, base) {
                let last = m.pvars[COUNT] as usize - 1;
                m.pvars[CONTACTS + j] = m.pvars[CONTACTS + last];
                m.pvars[COUNT] -= 1;
                continue;
            }
        }
        j += 1;
    }
    if 1 < m.pvars[COUNT] {
        let (a, b) = (m.pvars[CONTACTS] as usize, m.pvars[CONTACTS + 1] as usize);
        if let (Some(&pa), Some(&pb)) = (pts.get(a), pts.get(b)) {
            let d = through(&m.rows, sub(pa, pb));
            let s = hinge_spin(quat(m, SPIN), d);
            set_quat(m, SPIN, s);
        }
    }
}
