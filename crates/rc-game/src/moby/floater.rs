//! The floating platforms' rocking (L00 `0x32bd98` set-up, `0x32c0b0` step; L01 `0x3213a8` / `0x3216c0`): a block
//! in the moby's pvars that tilts it under Ratchet's weight, sinks it a little and wobbles it, the moby's rows built
//! here (mode 0x100). Oozla's lily pads (0x0a56) use it.
//!
//! **The block** (offsets from its start; x and y are the tilts, z the sink): `+0x00` the home place, `+0x10` the
//! pivot (a local point kept at home), `+0x20` the home angles, `+0x30` the offsets, `+0x3c` their velocities, `+0x48`
//! the springs' gains, `+0x54` their damping, `+0x60` the clamps, `+0x6c` the response to Ratchet, `+0x78` the
//! landing kick, `+0x84` the wobble phases, `+0x90` their steps, `+0x9c` their amplitudes, `+0xac` the kick's
//! 10-tick wait.
//!
//! **A step**: while Ratchet has stood on it 2 ticks or more, his offset in its frame less the pivot gives the force
//! `(−y·r.x, x·r.y, −r.z)` (a landing first kicks the velocities by force · kick, then waits 10 ticks off it); each axis
//! springs toward its force (`v += k·(f − o)`, `v −= d·v`, `o += v`) and wobbles (`a·sin` of its phase). The tilts are
//! the offsets plus wobble, clamped, turned onto the home angles; the place keeps the pivot (bobbed by the z wobble) at
//! home and drops by the clamped sink; whoever stands on it rides the move.

use crate::hero::physics::{add_rot, dot, sub, V3};
use crate::moby::attach::rows_of;
use crate::moby::{carry_hero, countdown, mode, Ctx, MobyWorld};

/// The set-up's tunables per axis (x, y tilts; z sink); [`Params::default`] is the table at L01 `0x1aa360`.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub gain: V3,
    pub damping: V3,
    pub clamp: V3,
    pub response: V3,
    pub kick: V3,
    /// Degrees a second (× 0.00029088825 a tick).
    pub speed: V3,
    pub amplitude: V3,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            gain: [0.01; 3],
            damping: [0.03; 3],
            clamp: [0.75, 0.75, 1.0],
            response: [0.1, 0.1, 0.3],
            kick: [0.05; 3],
            speed: [30.0, 60.0, 45.0],
            amplitude: [0.05; 3],
        }
    }
}

/// `0x32bd98(m, block, 0, 0, …)`: home at its place and angles, no pivot, the tunables, random phases.
pub fn init(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, b: usize, p: &Params) {
    let m = &mut w.mobys[id];
    m.mode |= mode::KEEP_ROWS;
    let (pos, rot) = (m.pos, m.rot);
    m.set_pv_v(b, pos);
    m.set_pv_v(b + 0x10, [0.0; 3]);
    m.set_pv_v(b + 0x20, rot);
    for k in 0..3 {
        let m = &mut w.mobys[id];
        m.set_pv_f(b + 0x78 + 4 * k, p.kick[k]);
        m.set_pv_f(b + 0x48 + 4 * k, p.gain[k]);
        m.set_pv_f(b + 0x60 + 4 * k, p.clamp[k]);
        m.set_pv_f(b + 0x54 + 4 * k, p.damping[k]);
        m.set_pv_f(b + 0x6c + 4 * k, p.response[k]);
        let phase = ctx.game.rng.rand_angle();
        let m = &mut w.mobys[id];
        m.set_pv_f(b + 0x84 + 4 * k, phase);
        m.set_pv_f(b + 0x9c + 4 * k, p.amplitude[k]);
        m.set_pv_f(b + 0x90 + 4 * k, p.speed[k] * 0.000_290_888_25);
    }
}

/// `0x32c0b0(m, block, 0)`: one step under Ratchet.
pub fn step(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, b: usize) {
    let on = ctx.hero.ground_moby == Some(id) && 2 <= ctx.hero.grounded_ticks;
    let hero = ctx.hero.pos;
    let m = &mut w.mobys[id];
    let old = m.pos;
    let mut f = [0.0f32; 3];
    if on {
        let d = sub(hero, m.pos);
        let pivot = m.pv_v(b + 0x10);
        let l = std::array::from_fn::<f32, 3, _>(|k| dot(d, m.rows[k]) - pivot[k]);
        let r = m.pv_v(b + 0x6c);
        f = [-l[1] * r[0], l[0] * r[1], -r[2]];
        if m.pv_i(b + 0xac) == 0 {
            for (k, fk) in f.iter().enumerate() {
                let o = b + 0x3c + 4 * k;
                m.set_pv_f(o, m.pv_f(o) + fk * m.pv_f(b + 0x78 + 4 * k));
            }
            m.set_pv_i(b + 0xac, 10);
        }
    } else {
        let mut t = m.pv_i(b + 0xac);
        countdown(&mut t);
        m.set_pv_i(b + 0xac, t);
    }
    let mut wobble = [0.0f32; 3];
    for k in 0..3 {
        let (oo, ov) = (b + 0x30 + 4 * k, b + 0x3c + 4 * k);
        let mut v = m.pv_f(ov) + m.pv_f(b + 0x48 + 4 * k) * (f[k] - m.pv_f(oo));
        v -= m.pv_f(b + 0x54 + 4 * k) * v;
        m.set_pv_f(ov, v);
        m.set_pv_f(oo, m.pv_f(oo) + v);
        let ph = add_rot(m.pv_f(b + 0x84 + 4 * k), m.pv_f(b + 0x90 + 4 * k));
        m.set_pv_f(b + 0x84 + 4 * k, ph);
        wobble[k] = m.pv_f(b + 0x9c + 4 * k) * ph.sin();
    }
    let (off, c) = (m.pv_v(b + 0x30), m.pv_v(b + 0x60));
    let tilt = [(off[0] + wobble[0]).clamp(-c[0], c[0]), (off[1] + wobble[1]).clamp(-c[1], c[1]), 0.0];
    let (t, home) = (rows_of(tilt), rows_of(m.pv_v(b + 0x20)));
    m.rows = t.map(|r| std::array::from_fn(|j| r[0] * home[0][j] + r[1] * home[1][j] + r[2] * home[2][j]));
    let mut pivot = m.pv_v(b + 0x10);
    pivot[2] += wobble[2];
    let turned: V3 = std::array::from_fn(|j| pivot[0] * m.rows[0][j] + pivot[1] * m.rows[1][j] + pivot[2] * m.rows[2][j]);
    m.pos = sub(m.pv_v(b), turned);
    // `0x31b8f8`: down by the clamped sink (the level's gravity is straight down).
    m.pos[2] -= (-off[2]).clamp(-c[2], c[2]);
    let (pos, yaw) = (m.pos, m.rot[2]);
    carry_hero(ctx, id, old, yaw, pos, yaw);
}
