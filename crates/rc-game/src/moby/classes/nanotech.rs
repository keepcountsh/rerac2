//! The nanotech cluster (class 0x326, L00 `0x3a0800`): the health a nanotech crate holds (0x1f5: one point, 0x200:
//! four; crate::moby::classes::crate_ creates it at the crate's start, `0x39ff50`: `+0xb8` the crate, pvar `+0x57` the
//! kind). The moby is hidden (mode 0x41, no collision); what shows are 8 orbs (particle type 0x3e kind 1, size 60000)
//! with their trail puffs (kind 2, 10000, riding the crate, or the cluster once free) and its draw callback's glow
//! ([`draw`]). The first cluster to start is the **master** (`0x1b1a6c`): it keeps the 8 orbit directions
//! (`0x23a5f0`: the crate's rows and their sums and differences, 0.25 long) and turns each 4° a tick about its own
//! axis (`0x23a670`, [`Globals`]), and counts the clusters on their way to Ratchet (pvar `+0x0b`).
//!
//! * **0** (start): the bob's phase `randf(0, π)`, 4 × `randi(255)` hues, the ring's 4 timers 90; a random tilt of the
//!   whole cluster (an axis `randf(0, 2π)` / `randf(−π/2, π/2)`, an angle `±randf(π/4, π/2)`); then as in 1;
//! * **1** (in the crate): at the crate; the crate gone (deleted or in its state 6) → **2**;
//! * **2** (free): falls onto what is below (9.8/s², at most 0.3 a tick; `0x39ffe0`), the orbs breathe (radius
//!   `0.25 + 0.15·sin`); with Ratchet's health under the limit (4 as Clank) by more than the clusters on their way,
//!   within 10 of him and nothing in the line to his head (flags 0x12) → **3**: each orb flies `rand(20..80)` ticks on
//!   a cubic from where it is to his chest (0.75 up), the 8 fanned 5 out about the line; each leaves two trail puffs a
//!   tick; the **first to arrive heals** (1 point, 4 for the kind 1; at most the limit; the HUD shows the health for
//!   0x168 ticks) with sound 1 of the crate's class (`0x360698`); after that the healing ring (`0x3a0340`, type-0x3b
//!   sparkles around him) and, one tick in five while it is young, two glints (`0x3a0220`, type 0x3c); all orbs gone
//!   after 300 ticks → deleted (the master waits in 4, turning its table);
//! * in 0 / 1 / 2 the cluster bobs `0.06·sin` (2° a tick) and, in view (64), places its orbs, lays a trail puff each
//!   (life 40, 5 farther than 10 from the camera across) and registers its draw ([`draw`]).
//!
//! **pvars**: `+0x00` the bob, `+0x04` its phase, `+0x0b` (the master) the clusters on their way, `+0x0c` the moby it
//! rides (none here), `+0x10` the centre (0.5 up), `+0x20` / `+0x2c` the tilt's axis and angle, `+0x30` the 8 orbs,
//! `+0x50` the fall's speed, `+0x54` the flight's timer, `+0x56` healed, `+0x57` the kind, `+0x58` the ring's 4
//! timers, `+0x60` its 4 hues. An orb's record keeps `+0x28` its radius and `+0x2c` its breathing phase (state 2),
//! then `+0x24` the flight's control point, `+0x30` its start, `+0x3c` / `+0x3e` the flight's ticks / left.
//!
//! Not ported: the ride of a free cluster on a moving moby (`0x3222a8` / `0x3223e8` / `0x322570`: crates on movers
//! are not ported either), the loaded-area test (`0x321a78` → state 5, back to 2 when the area is loaded again),
//! the cheat `0x1a7a4e` (taken at full health while `0x189ffc` is 0, which it sets to 600; the flight held at 250),
//! the challenge mode's four times the health (`0x1a7a12`), the orbs kept on the cluster out of view (`0x1adb0c`).

use crate::fx::FxQuad;
use crate::hero::physics::{add, add_rot, cross, dist, dist2, euler_rows, len, scale, set_len, sub, V3};
use crate::moby::aim::is_crate;
use crate::moby::{countdown, Ctx, MobyWorld};
use crate::particles::{self, rec};

/// The cluster's class.
pub const CLASS: i16 = 0x0326;

/// Pvar offsets.
mod pv {
    pub const BOB: usize = 0x00;
    pub const PHASE: usize = 0x04;
    pub const PENDING: usize = 0x0b;
    pub const CENTRE: usize = 0x10;
    pub const AXIS: usize = 0x20;
    pub const TILT: usize = 0x2c;
    pub const ORBS: usize = 0x30;
    pub const FALL: usize = 0x50;
    pub const TIMER: usize = 0x54;
    pub const HEALED: usize = 0x56;
    pub const KIND: usize = 0x57;
    pub const RING_T: usize = 0x58;
    pub const HUE: usize = 0x60;
}

/// An orb record's fields.
mod orb {
    pub const CTRL: usize = 0x24;
    pub const RADIUS: usize = 0x28;
    pub const BREATH: usize = 0x2c;
    pub const START: usize = 0x30;
    pub const TICKS: usize = 0x3c;
    pub const LEFT: usize = 0x3e;
}

/// `$gp+0x408`: the master's turn a tick (4°); `+0x430` the directions' length; `+0x42c` / `+0x428` the bob and its
/// step (2°); `+0x478` the breathing; `+0x480` the orbs' breathing steps.
const TURN: f32 = f32::from_bits(0x3d8e_fa36);
const DIR_LEN: f32 = 0.25;
const BOB: f32 = 0.06;
const BOB_STEP: f32 = f32::from_bits(0x3d0e_fa36);
const BREATH: f32 = 0.15;
const BREATH_STEP: [u32; 8] = [0x3d8e_fa36, 0x3d0e_fa36, 0x3dd6_7751, 0x3d56_7751, 0x3c8e_fa36, 0x3db2_b8c4, 0x3d32_b8c4, 0x3da0_d97d];
/// `$gp+0x418` / `+0x41c`: the orbs' colour by kind; `+0x414` / `+0x410` the orbs' and trail puffs' sizes; `+0x40c`
/// an orb's turn a tick.
const ORB_RGBA: [u32; 2] = [0x7f7f_4040, 0x7f7f_407f];
const ORB_SIZE: f32 = 60000.0;
const TRAIL_SIZE: f32 = 10000.0;
const ORB_SPIN: u8 = 4;
/// `$gp+0x458` the flight's ticks; `+0x460` / `+0x464` an orb's; `+0x470` the chest's height; `+0x46c` the fan.
const FLIGHT: i16 = 300;
const ORB_FLIGHT: (i32, i32) = (20, 80);
const CHEST: f32 = 0.75;
const FAN: f32 = 5.0;
/// `$gp+0x474`: the ring's timers; `+0x4e0` / `+0x4f0` / `+0x500` its hue speeds, sizes and heights; `+0x4c4..` its
/// colours by kind; `+0x4d8` the sparkles' frame list (53, then type 0x3e's first frame put in).
const RING_TICKS: i16 = 90;
const RING_HUE: [f32; 4] = [-2.0, -4.25, 1.25, 3.5];
const RING_SIZE: [f32; 4] = [4.0, 5.0, 3.0, 6.0];
const RING_Z: [f32; 4] = [-0.25, -0.1, 0.15, 0.3];
const RING_RGBA: [[u32; 2]; 2] = [[0x407f_2020, 0x107f_2020], [0x407f_207f, 0x107f_207f]];
const RING_DEF: u8 = 0x35;
/// `$gp+0x4a4` / `+0x4a8` the glints' speed (a second), `+0x4ac` their life, `+0x4b0` / `+0x4b8` / `+0x4b4` their
/// colours, `+0x4bc` / `+0x4c0` their sizes.
const GLINT_SPEED: (f32, f32) = (1.0, 2.0);
const GLINT_LIFE: u16 = 60;
const GLINT_RGBA: [u32; 2] = [0x5080_4040, 0x5080_4080];
const GLINT_WHITE: u32 = 0x407f_7f7f;
const GLINT_SIZE: (f32, f32) = (0.4, 0.2);

/// The draw's tables (L00's overlay data, [`GlowTables::read`]) and constants (`$gp+0x510..0x564`).
#[derive(Clone, Debug, PartialEq)]
pub struct GlowTables {
    /// `0x292330`: the sphere's 290 points (0.3 across), `0x293550` their ST.
    pub verts: Vec<V3>,
    pub st: Vec<[f32; 2]>,
    /// `0x293e70` / `0x294470`: the halo's points (32 inner, 1 long, then the outer) and its 32 quads.
    pub halo_pts: Vec<V3>,
    pub halo_quads: Vec<[u16; 4]>,
    /// `0x294670` (0x1f5) / `0x294570` (0x200): the crate glass's 16 points in the crate's frame.
    pub glass: [Vec<V3>; 2],
}

/// `$gp+0x510`: the glass's quads; `+0x544` / `+0x55c` the sphere's and the glass's FX textures; `+0x548` the sphere's
/// colours by kind; `+0x550` / `+0x564` the halo's size and its outer corners' factor; `+0x554` its colours; `+0x560` the
/// glass's sphere-map length.
const GLASS_QUADS: [[usize; 4]; 4] = [[0, 1, 3, 2], [4, 5, 7, 6], [8, 9, 11, 10], [12, 13, 15, 14]];
const SPHERE_FX: usize = 0x15;
const GLASS_FX: usize = 0x15;
const HALO_FX: usize = 0xb;
const SPHERE_RGBA: [u32; 2] = [0x5080_4040, 0x5080_4080];
const HALO_SIZE: f32 = 0.3;
const HALO_OUTER: f32 = 1.125;
const HALO_RGB: [u32; 2] = [0x0080_2020, 0x0080_2080];
const GLASS_RGBA: u32 = 0x107f_7f7f;
const GLASS_ENV: f32 = 2.0;
/// The sphere's three strips (`0x2ec618`): first point and count.
const STRIPS: [(usize, usize); 3] = [(0, 0x94), (146, 0x6d), (254, 0x24)];

impl GlowTables {
    /// From the overlay through `read(address, length)` (L00's addresses).
    pub fn read(read: impl Fn(u32, usize) -> Option<Vec<u8>>) -> Option<GlowTables> {
        let floats = |a: u32, n: usize, stride: usize, k: usize| -> Option<Vec<Vec<f32>>> {
            let b = read(a, n * stride)?;
            Some((0..n).map(|i| (0..k).map(|j| f32::from_le_bytes(b[i * stride + 4 * j..i * stride + 4 * j + 4].try_into().unwrap())).collect()).collect())
        };
        let v3 = |a: u32, n: usize| -> Option<Vec<V3>> { Some(floats(a, n, 16, 3)?.into_iter().map(|v| [v[0], v[1], v[2]]).collect()) };
        let q = read(0x0029_4470, 32 * 8)?;
        let halo_quads: Vec<[u16; 4]> = (0..32).map(|i| std::array::from_fn(|k| u16::from_le_bytes([q[8 * i + 2 * k], q[8 * i + 2 * k + 1]]))).collect();
        let halo_n = halo_quads.iter().flatten().map(|&i| i as usize + 1).max()?;
        Some(GlowTables {
            verts: v3(0x0029_2330, 290)?,
            st: floats(0x0029_3550, 290, 8, 2)?.into_iter().map(|v| [v[0], v[1]]).collect(),
            halo_pts: v3(0x0029_3e70, halo_n)?,
            halo_quads,
            glass: [v3(0x0029_4670, 16)?, v3(0x0029_4570, 16)?],
        })
    }
}

/// The clusters' globals: the master (`0x1b1a6c`), its directions (`0x23a5f0`) and their axes (`0x23a670`); the draw's
/// tables (set by the engine).
#[derive(Clone, Debug, Default)]
pub struct Globals {
    pub master: Option<usize>,
    pub dirs: [V3; 8],
    pub axes: [V3; 8],
    pub tables: Option<std::sync::Arc<GlowTables>>,
}

/// `0x321c00(angle, out, v, axis)`: `v` turned `angle` about `axis` (an angle under 1e-5 keeps it).
fn rotate(v: V3, angle: f32, axis: V3) -> V3 {
    if angle.abs() < 1e-5 { return v; }
    let k = set_len(axis, 1.0);
    let (s, c) = angle.sin_cos();
    let d = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
    let x = cross(k, v);
    std::array::from_fn(|i| v[i] * c + x[i] * s + k[i] * d * (1.0 - c))
}

/// `0x31bc68(a, b, c, d, t)`: the cubic from `b` (t = 0) to `c` (t = 1).
fn cubic(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let f = (d - c) - (a - b);
    f * t * t * t + ((a - b) - f) * t * t + (c - a) * t + b
}

fn up(v: V3, h: f32) -> V3 { [v[0], v[1], v[2] + h] }

/// `0x39ff50(crate, kind)`: a cluster for the crate (none when the crate is gone): hidden, no collision, update
/// distance 64, at the crate.
pub fn create(w: &mut MobyWorld, crate_: usize, kind: u8) -> Option<usize> {
    if w.mobys[crate_].state >= 0x80 { return None; }
    let at = w.mobys[crate_].pos;
    let id = w.create(CLASS);
    let m = &mut w.mobys[id];
    m.update = Some(update);
    m.mode &= !crate::moby::mode::NO_UPDATE;
    m.update_dist = 0x40;
    m.state = 0;
    m.parent = Some(crate_);
    m.set_pv_b(pv::KIND, kind);
    m.has_collision = false;
    m.mode |= 0x41;
    m.pos = at;
    w.changed = true;
    Some(id)
}

/// `0x3a0800`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => {
            start(w, id, ctx);
            follow(w, id);
            common(w, id, ctx);
        }
        1 => {
            follow(w, id);
            common(w, id, ctx);
        }
        2 => common(w, id, ctx),
        3 => fly(w, id, ctx),
        4 if w.nanotech.master == Some(id) => turn(&mut w.nanotech),
        _ => {}
    }
}

/// State 0.
fn start(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let c = w.mobys[id].parent.unwrap_or(id);
    let (at, rot) = (w.mobys[c].pos, w.mobys[c].rot);
    let rng = &mut ctx.game.rng;
    let m = &mut w.mobys[id];
    m.state = 1;
    m.mode |= 0x41;
    m.has_collision = false;
    m.timer = 0;
    m.pos = at;
    m.rot = rot;
    m.set_pv_f(pv::BOB, 0.0);
    m.set_pv_s(0x08, 0);
    m.set_pv_f(pv::PHASE, rng.randf(0.0, std::f32::consts::PI));
    m.set_pv_b(pv::HEALED, 0);
    m.set_pv_i(0x0c, 0);
    for k in 0..4 { m.set_pv_f(pv::HUE + 4 * k, rng.randi(0xff) as f32); }
    for k in 0..4 { m.set_pv_s(pv::RING_T + 2 * k, RING_TICKS); }
    for k in 0..8 { m.set_pv_i(pv::ORBS + 4 * k, 0); }
    if w.nanotech.master.is_none() {
        // `0x2ef6b8`: its rows from the crate's angles.
        let r = euler_rows(rot);
        let m = &mut w.mobys[id];
        m.rows = r;
        m.update_dist = 0xff;
        m.set_pv_b(pv::PENDING, 0);
        let g = &mut w.nanotech;
        g.master = Some(id);
        let dirs = [r[0], r[1], r[2], add(r[0], r[2]), add(r[0], r[1]), sub(r[1], r[2]), sub(r[0], r[2]), sub(r[0], r[1])].map(|d| set_len(d, DIR_LEN));
        g.dirs = dirs;
        g.axes[0] = r[2];
        g.axes[1] = r[0];
        g.axes[2] = r[1];
        // `0x2eef88(out, a, b)` = b × a.
        for (k, other) in [r[1], r[2], r[0], r[1], r[2]].into_iter().enumerate() { g.axes[3 + k] = cross(other, dirs[3 + k]); }
    }
    let yaw = rng.randf(0.0, std::f32::consts::TAU);
    let pitch = rng.randf(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    let tilt = rng.randf_signed(f32::from_bits(0x3f49_0fd8), std::f32::consts::FRAC_PI_2);
    let m = &mut w.mobys[id];
    m.set_pv_v(pv::AXIS, [yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin()]);
    m.set_pv_f(pv::TILT, tilt);
}

/// State 1's part: at the crate; the crate gone (deleted, or in its state 6: `0x39e530`) → 2.
fn follow(w: &mut MobyWorld, id: usize) {
    let c = w.mobys[id].parent.unwrap_or(id);
    let (at, state) = (w.mobys[c].pos, w.mobys[c].state);
    let m = &mut w.mobys[id];
    m.pos = at;
    m.set_pv_v(pv::CENTRE, up(at, 0.5));
    if state >= 0x80 || state == 6 || c == id {
        m.state = 2;
        m.set_pv_f(pv::FALL, 0.0);
    }
}

/// `0x3a0730`: the master turns each direction about its axis.
fn turn(g: &mut Globals) {
    for k in 0..8 { g.dirs[k] = set_len(rotate(g.dirs[k], TURN, g.axes[k]), DIR_LEN); }
}

fn orb_of(w: &MobyWorld, id: usize, k: usize) -> Option<usize> { usize::try_from(w.mobys[id].pv_i(pv::ORBS + 4 * k) - 1).ok() }
fn set_orb(w: &mut MobyWorld, id: usize, k: usize, o: Option<usize>) { w.mobys[id].set_pv_i(pv::ORBS + 4 * k, o.map_or(0, |o| o as i32 + 1)); }

/// `0x3a0678`: orb `k`'s place: its direction tilted, at its radius (state 2), from the centre, bobbing.
fn orb_pos(w: &MobyWorld, id: usize, k: usize, parts: Option<&particles::Particles>) -> V3 {
    let m = &w.mobys[id];
    let mut v = rotate(w.nanotech.dirs[k], m.pv_f(pv::TILT), m.pv_v(pv::AXIS));
    if let (Some(o), Some(s), 2) = (orb_of(w, id, k), parts, m.state) { v = set_len(v, rec::ff(&s.pool.recs[o], orb::RADIUS)); }
    let mut q = add(m.pv_v(pv::CENTRE), v);
    q[2] += m.pv_f(pv::BOB);
    q
}

fn kind(w: &MobyWorld, id: usize) -> usize { (w.mobys[id].pv_b(pv::KIND) != 0) as usize }

/// States 0 / 1 / 2 after their own part: the fall and the breathing (2), the heal's test, the bob, the master's
/// turn, and in view the orbs, their trail puffs and the draw.
fn common(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 2 {
        let at = w.mobys[id].pos;
        w.mobys[id].set_pv_v(pv::CENTRE, up(at, 0.5));
        fall(w, id, ctx);
        for (k, step) in BREATH_STEP.iter().enumerate() {
            let (Some(o), Some(s)) = (orb_of(w, id, k), ctx.parts.as_deref_mut()) else { continue };
            let r = &mut s.pool.recs[o];
            let b = rec::ff(r, orb::BREATH);
            rec::set_ff(r, orb::RADIUS, BREATH * b.sin() + DIR_LEN);
            rec::set_ff(r, orb::BREATH, add_rot(b, f32::from_bits(*step)));
        }
    }
    heal_test(w, id, ctx);
    let m = &mut w.mobys[id];
    let ph = m.pv_f(pv::PHASE);
    m.set_pv_f(pv::BOB, BOB * ph.sin());
    m.set_pv_f(pv::PHASE, add_rot(ph, BOB_STEP));
    if w.nanotech.master == Some(id) { turn(&mut w.nanotech); }
    // `0x2f0080(64, sphere)`: the centre in view.
    let at = w.mobys[id].pos;
    let c = up(at, 0.5);
    if ctx.view.is_some_and(|v| v.culled(64.0, [c[0], c[1], c[2], 1.0])) { return; }
    w.register_draw(id);
    let life = if 10.0 < dist2(at, ctx.host.camera_pos()) { 5 } else { 0x28 };
    let state = w.mobys[id].state;
    let anchor = if state == 2 { id } else { w.mobys[id].parent.unwrap_or(id) };
    let k_ = kind(w, id);
    let Some(s) = ctx.parts.as_deref_mut() else { return };
    for k in 0..8 {
        let q = orb_pos(w, id, k, Some(&*s));
        if let Some(t) = particles::orb(s, &mut ctx.game.rng, q, life, 2) {
            particles::orb_attach(s, t, anchor, at);
            let r = &mut s.pool.recs[t];
            rec::set_u32(r, 4, ORB_RGBA[k_]);
            rec::set_ff(r, 0xc, TRAIL_SIZE);
        }
        match orb_of(w, id, k) {
            None => {
                let o = particles::orb(s, &mut ctx.game.rng, q, 0, 1);
                if let Some(o) = o {
                    let r = &mut s.pool.recs[o];
                    rec::set_ff(r, orb::BREATH, 0.0);
                    rec::set_ff(r, orb::RADIUS, DIR_LEN);
                    rec::set_ff(r, 0xc, ORB_SIZE);
                    rec::set_u32(r, 4, ORB_RGBA[k_]);
                }
                set_orb(w, id, k, o);
            }
            Some(o) => {
                let r = &mut s.pool.recs[o];
                rec::set_v3(r, 0x10, q);
                r[8] = r[8].wrapping_add(ORB_SPIN);
            }
        }
    }
}

/// `0x39ffe0`: the free cluster falls onto what is below it: the line from 0.5 above to 1 below; on a crate it sits
/// on its top, at its middle.
fn fall(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let p = w.mobys[id].pos;
    let mut floor = p[2] - 1.0;
    if let Some(h) = w.coll_line(ctx, up(p, 0.5), up(p, -1.0), 2, Some(id)) {
        floor = h.point[2];
        if let Some(o) = h.moby.filter(|&o| is_crate(w, o)) {
            let q = [w.mobys[o].pos[0], w.mobys[o].pos[1], h.point[2]];
            let m = &mut w.mobys[id];
            m.pos = q;
            m.set_pv_v(pv::CENTRE, up(q, 0.5));
        }
    }
    let m = &mut w.mobys[id];
    let z = m.pos[2];
    if z <= floor {
        m.pos[2] = floor;
        m.timer = 0;
        m.set_pv_f(pv::FALL, 0.0);
    } else {
        m.timer = 1;
        let v = m.pv_f(pv::FALL);
        m.pos[2] = z - v;
        m.set_pv_f(pv::FALL, (v + 0.002_722_222_4).min(0.3));
    }
    w.changed = true;
}

/// The health's limit (`0x1a7a08`; 4 as Clank).
fn max_health(ctx: &Ctx) -> i32 { if ctx.hero.mode == 1 { 4 } else { ctx.game.max_health } }

/// Ratchet's chest (`0x189ea0 − 0.75 · gravity`).
fn chest(ctx: &Ctx) -> V3 { up(ctx.hero.pos, CHEST) }

/// The heal's test of the states 0 / 1 / 2 (→ 3) and the orbs' flights.
fn heal_test(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 1 { return; }
    let Some(master) = w.nanotech.master else { return };
    let hp = ctx.hero.hp;
    if hp == 0 { return; }
    if max_health(ctx) <= hp + w.mobys[master].pv_b(pv::PENDING) as i32 { return; }
    if (0..8).all(|k| orb_of(w, id, k).is_none()) { return; }
    let at = w.mobys[id].pos;
    if 10.0 <= dist(at, ctx.hero.pos) { return; }
    // `0x3a07a8`: nothing in the line from 0.5 above it to his head (`0x189f00`).
    if w.coll_line(ctx, up(at, 0.5), up(ctx.hero.pos, 1.0), 0x12, None).is_some() { return; }
    w.mobys[id].state = 3;
    let n = w.mobys[master].pv_b(pv::PENDING).wrapping_add(1);
    w.mobys[master].set_pv_b(pv::PENDING, n);
    w.mobys[id].set_pv_s(pv::TIMER, FLIGHT);
    let mut d = sub(chest(ctx), at);
    d[2] -= 0.5;
    let c = cross([0.0, 0.0, FAN], d);
    let fan = set_len(rotate(cross(d, c), -std::f32::consts::FRAC_PI_2, d), FAN);
    let half = scale(d, 0.5);
    let Some(s) = ctx.parts.as_deref_mut() else { return };
    for k in 0..8 {
        let Some(o) = orb_of(w, id, k) else { continue };
        let ctrl = add(half, rotate(fan, k as f32 * 0.448_549_7, d));
        let n = ctx.game.rng.rand_range(ORB_FLIGHT.0, ORB_FLIGHT.1) as i16;
        let r = &mut s.pool.recs[o];
        rec::set_v3(r, orb::CTRL, ctrl);
        let p = rec::pos(r);
        rec::set_v3(r, orb::START, p);
        rec::set_i16(r, orb::LEFT, n);
        rec::set_i16(r, orb::TICKS, n);
    }
}

/// State 3: the orbs fly to Ratchet; the first to arrive heals.
fn fly(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let master = w.nanotech.master;
    if master == Some(id) { turn(&mut w.nanotech); }
    if w.mobys[id].pv_b(pv::HEALED) != 0 {
        w.mobys[id].pos = chest(ctx);
        if ring(w, id, ctx) && ctx.game.rng.randi(5) == 0 { glints(w, id, ctx); }
    }
    let mut t = w.mobys[id].pv_s(pv::TIMER) as i32;
    let out = countdown(&mut t);
    w.mobys[id].set_pv_s(pv::TIMER, t as i16);
    if out {
        for k in 0..8 {
            if let (Some(o), Some(s)) = (orb_of(w, id, k), ctx.parts.as_deref_mut()) { s.kill_part(o); }
            set_orb(w, id, k, None);
        }
        if master == Some(id) { w.mobys[id].state = 4; } else { w.delete(id); }
        return;
    }
    let target = chest(ctx);
    let k_ = kind(w, id);
    for k in 0..8 {
        let Some(o) = orb_of(w, id, k) else { continue };
        let Some(s) = ctx.parts.as_deref_mut() else { break };
        let r = &mut s.pool.recs[o];
        let mut left = rec::i16(r, orb::LEFT) as i32;
        let arrived = countdown(&mut left);
        rec::set_i16(r, orb::LEFT, left as i16);
        if !arrived {
            let prev = rec::pos(r);
            let tt = 1.0 - left as f32 / rec::i16(r, orb::TICKS) as f32;
            let (start, ctrl) = (rec::v3(r, orb::START), rec::v3(r, orb::CTRL));
            let q: V3 = std::array::from_fn(|i| cubic(start[i] - ctrl[i], start[i], target[i], target[i] - ctrl[i], tt));
            rec::set_v3(r, 0x10, q);
            for at in [q, scale(add(prev, q), 0.5)] {
                let rng = &mut ctx.game.rng;
                let j: V3 = std::array::from_fn(|_| rng.randf_signed(0.0, 0.01));
                if let Some(p) = particles::orb(s, rng, add(at, j), 0x14, 0) {
                    let r = &mut s.pool.recs[p];
                    rec::set_u32(r, 4, ORB_RGBA[k_]);
                    rec::set_ff(r, 0xc, TRAIL_SIZE);
                }
            }
            continue;
        }
        if w.mobys[id].pv_b(pv::HEALED) == 0 {
            let hp = ctx.hero.hp;
            if hp != 0 {
                let new = (hp + if k_ == 0 { 1 } else { 4 }).min(max_health(ctx));
                if new != hp { ctx.game.show_health = Some(0x168); }
                ctx.hero.hp = new;
            }
            if let Some(m) = master {
                let n = (w.mobys[m].pv_b(pv::PENDING) as i8).wrapping_sub(1).max(0);
                w.mobys[m].set_pv_b(pv::PENDING, n as u8);
            }
            let class = if k_ == 0 { 0x1f5 } else { 0x200 };
            ctx.host.play_sound_of(&w.mobys[id], class, 1, 0x20);
            w.mobys[id].set_pv_b(pv::HEALED, 1);
        }
        if let Some(s) = ctx.parts.as_deref_mut() { s.kill_part(o); }
        set_orb(w, id, k, None);
    }
}

/// `0x3a0340`: the healing ring, 4 pairs of sparkles riding Ratchet (0.3 toward the camera from him, 0.1 apart),
/// each while its timer runs; true while one is above 0x30.
fn ring(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let at = w.mobys[id].pos;
    let away = set_len(sub(ctx.host.camera_pos(), at), -0.3);
    let step = set_len(away, 0.1);
    let mut p = add(away, at);
    let z0 = p[2];
    let k_ = kind(w, id);
    let hero = ctx.hero.pos;
    let mut young = false;
    for k in 0..4 {
        let m = &mut w.mobys[id];
        let mut t = m.pv_s(pv::RING_T + 2 * k) as i32;
        let done = countdown(&mut t);
        m.set_pv_s(pv::RING_T + 2 * k, t as i16);
        if !done {
            let mut hue = m.pv_f(pv::HUE + 4 * k) + RING_HUE[k];
            if 0x30 < t { young = true; }
            if 255.0 <= hue { hue -= 255.0; } else if hue <= 0.0 { hue += 255.0; }
            m.set_pv_f(pv::HUE + 4 * k, hue);
            let f = (0.5 - (0xff - t) as f32 * 0.003_921_569).abs();
            let c = particles::tween_color(f.to_bits(), RING_RGBA[k_][0], RING_RGBA[k_][1]);
            let life = 1.0 - t as f32 / RING_TICKS as f32;
            let a = (((c >> 24) as f32) * (1.0 - life)) as i32 as u32;
            let q = [p[0], p[1], z0 + RING_Z[k]];
            let rot = hue as i32 as u8;
            if let Some(s) = ctx.parts.as_deref_mut() {
                let frame = s.def_first(particles::ORB);
                for (size, rgba) in [(RING_SIZE[k] * life, a << 24 | c & 0xff_ffff), (RING_SIZE[k] * 0.7 * life, a << 24 | 0xff_ffff)] {
                    if let Some(r) = particles::sparkle(s, size, q, rgba, rot, RING_DEF, false, 2, true, hero) { s.pool.recs[r][2] = frame; }
                }
            }
        }
        p = add(p, step);
    }
    young
}

/// `0x3a0220`: two glints at the cluster riding Ratchet, drifting `randf(1, 2)` a second along the gravity turned a
/// random angle about the line from it to the camera.
fn glints(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let at = w.mobys[id].pos;
    let d = sub(ctx.host.camera_pos(), at);
    let rng = &mut ctx.game.rng;
    let speed = rng.randf(GLINT_SPEED.0 * 0.016_666_668, GLINT_SPEED.1 * 0.016_666_668);
    let v = rotate(scale(particles::GRAVITY, speed), rng.rand_angle(), d);
    let hero = ctx.hero.pos;
    let rgba = GLINT_RGBA[kind(w, id)];
    let rot = rng.randi(0xff) as u8;
    if let Some(s) = ctx.parts.as_deref_mut() { particles::glint(s, GLINT_SIZE.0, at, v, rgba, GLINT_LIFE, rot, true, hero); }
    let rot = ctx.game.rng.randi(0xff) as u8;
    if let Some(s) = ctx.parts.as_deref_mut() { particles::glint(s, GLINT_SIZE.1, at, v, GLINT_WHITE, GLINT_LIFE, rot, true, hero); }
}

/// The quads of a GS triangle strip (two triangles a quad, the last one alone doubled).
fn strip(out: &mut Vec<FxQuad>, fx: usize, additive: bool, pts: &[(V3, [f32; 2], u32)]) {
    let mut i = 0;
    while i + 2 < pts.len() {
        let c = [pts[i], pts[i + 1], pts[i + 2], pts[if i + 3 < pts.len() { i + 3 } else { i + 2 }]];
        out.push(FxQuad { fx, additive, corners: c.map(|p| p.0), st: c.map(|p| p.1), rgba: c.map(|p| p.2) });
        i += 2;
    }
}

/// `v − m·trunc(v / m)` (`0x2effc8`).
fn wrap(v: f32, m: f32) -> f32 { let q = v / m; (q - q.trunc()) * m }

/// `0x3a17f0` (the frame's first list, `0x2e7718`): at the crate again (state 1), with c = the centre bobbing and the
/// camera-facing frame F = unit(camera − c), R = unit(crate up × F), U = F × R: the sphere (FX 0x15, ALPHA 0x44, the
/// kind's colour; Z tested, not written), the halo (32 quads, FX 0xb, additive, the alpha `128 + 64·bob/0.06` inside
/// and 0 outside) and, in the crate (state 1), its glass's sheen (4 quads of the crate's box in its frame, FX 0x15 at
/// alpha 0x10, sphere-mapped by the view's (x, z) set 2 long). The offsets `0x1b27a0` (the centre) and `0x1b27b0`
/// (the halo's alpha) are taken as 0.
pub fn draw(w: &mut MobyWorld, id: usize, cam: V3, out: &mut Vec<FxQuad>) {
    let Some(t) = w.nanotech.tables.clone() else { return };
    let c_ = w.mobys[id].parent.unwrap_or(id);
    let (crate_pos, crate_rows, crate_class) = (w.mobys[c_].pos, w.mobys[c_].rows, w.mobys[c_].o_class);
    let m = &mut w.mobys[id];
    if m.state == 1 {
        m.pos = crate_pos;
        m.set_pv_v(pv::CENTRE, up(crate_pos, 0.5));
    }
    let bob = m.pv_f(pv::BOB);
    let k_ = (m.pv_b(pv::KIND) != 0) as usize;
    let c = up(m.pos, 0.5 + bob);
    let f = set_len(sub(cam, c), 1.0);
    let r = set_len(cross(crate_rows[2], f), 1.0);
    let u = cross(f, r);
    let xform = |v: V3| add(add(add(c, scale(f, v[0])), scale(r, v[1])), scale(u, v[2]));
    for (a, n) in STRIPS {
        let pts: Vec<(V3, [f32; 2], u32)> = (a..(a + n).min(t.verts.len())).map(|i| (xform(t.verts[i]), t.st[i], SPHERE_RGBA[k_])).collect();
        strip(out, SPHERE_FX, false, &pts);
    }
    let a = ((bob / BOB * 64.0 + 128.0) as i32) as u32;
    let rgba = a << 24 | HALO_RGB[k_];
    for q in &t.halo_quads {
        let corners: [V3; 4] = std::array::from_fn(|k| {
            let s = if k < 2 { HALO_SIZE } else { HALO_SIZE * HALO_OUTER };
            xform(scale(t.halo_pts.get(q[k] as usize).copied().unwrap_or_default(), s))
        });
        out.push(FxQuad { fx: HALO_FX, additive: true, corners, st: [[0.5, 0.5], [0.5, 0.5], [0.5, 1.0], [0.5, 1.0]], rgba: [rgba, rgba, 0, 0] });
    }
    if w.mobys[id].state != 1 { return; }
    let at = [c[0], c[1], w.mobys[id].pos[2]];
    let glass = &t.glass[(crate_class != 0x1f5) as usize];
    for q in GLASS_QUADS {
        let corners: [V3; 4] = q.map(|i| {
            let p = glass.get(i).copied().unwrap_or_default();
            add(at, add(add(scale(crate_rows[0], p[0]), scale(crate_rows[1], p[1])), scale(crate_rows[2], p[2])))
        });
        let st = corners.map(|p| {
            let e = sub(p, cam);
            let l = len([e[0], e[2], 0.0]);
            let k = if l != 0.0 { GLASS_ENV / l } else { 0.0 };
            [wrap(e[0] * k * 0.5 + 0.5, 8.0), wrap(e[2] * k * 0.5 + 0.5, 8.0)]
        });
        out.push(FxQuad { fx: GLASS_FX, additive: false, corners, st, rgba: [GLASS_RGBA; 4] });
    }
}
