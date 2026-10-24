//! The debris (class 0x7a, L00 `0x390888` / `0x3909f0`): a chunk thrown out of a blast, spinning and falling, leaving
//! a smoke trail (crate::particles::trail), shrinking over the last quarter of its life.
//!
//! Its pvars: `+0x00` the velocity, `+0x10` / `+0x14` its spins, `+0x18` / `+0x1a` its life and the life it had, `+0x1c`
//! its size, `+0x20` a thinner trail (kind 2). Its timer byte (`+0xbc`): bit 0 trailing, the bits above the trail's
//! colour shift (`0x31f7d0`, not ported: every caller here passes 0).

use crate::hero::physics::{add, add_rot, set_len, V3};
use crate::moby::{countdown, mode, Ctx, MobyWorld};

pub const CLASS: i16 = 0x7a;
/// `0x1aefc0`: the trail's colours.
const TRAIL_RGBA: [u32; 4] = [0x2fff_ffff, 0x2f00_ffff, 0x2f00_7fff, 0x2f00_4fff];
/// `0x326618`'s fall a tick.
const FALL: f32 = 0.004_056;

/// `0x390888(pos, vel, life, kind, 0)`: a piece of debris at `pos` moving `vel` a tick for `life` ticks, spinning 6°..12°
/// a tick on two axes, lit flat (0x7f); kind 0 is a big chunk without a trail at 0.5..0.75 of its size, kind 1 trails
/// smoke, kind 2 trails it every other tick or so.
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, pos: V3, vel: V3, life: i32, kind: u8) -> Option<usize> {
    w.class(CLASS)?;
    let id = w.create(CLASS);
    let g = &mut ctx.game.rng;
    let spins = [g.randf(0.104_719_76, 0.209_439_52), g.randf(0.104_719_76, 0.209_439_52)];
    let (kind, thin) = if kind == 2 { (1, true) } else { (kind, false) };
    let shrink = (kind == 0).then(|| g.randf(0.5, 0.75));
    let m = &mut w.mobys[id];
    m.update = Some(update);
    m.mode &= !mode::NO_UPDATE;
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.drawn = 1;
    m.alpha = 0x80;
    m.draw_dist = 0xff;
    m.update_dist = 0xff;
    m.ambient = [0x7f; 3];
    m.pos = pos;
    m.set_pv_v(0, vel);
    m.set_pv_f(0x10, spins[0]);
    m.set_pv_f(0x14, spins[1]);
    m.set_pv_s(0x1a, life as i16);
    m.set_pv_s(0x18, life as i16);
    if thin { m.set_pv_i(0x20, 1); }
    if let Some(k) = shrink { m.scale *= k; }
    m.set_pv_f(0x1c, m.scale);
    m.timer = kind;
    w.rebuild(id, &*ctx.host);
    w.changed = true;
    Some(id)
}

/// `0x3909f0`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    // The trail: a puff a tick (a thin one: when `rand % 2` is 0) going 1/60 a tick a random way.
    let smoke = w.mobys[id].pv_i(0x20) == 0 || ctx.game.rng.randi(2) == 0;
    if w.mobys[id].timer & 1 != 0 && smoke {
        let g = &mut ctx.game.rng;
        let d = [g.randf(-1.0, 1.0), g.randf(-1.0, 1.0), g.randf(-1.0, 1.0)];
        let v = set_len(d, 0.016_666_668);
        let c = TRAIL_RGBA[g.randi(4) as usize];
        if let Some(s) = ctx.parts.as_deref_mut() {
            crate::particles::trail(s, &mut ctx.game.rng, w.mobys[id].pos, v, c, 0x4fff, 0x2d, 0x32, 0x78, true);
        }
    }
    let m = &mut w.mobys[id];
    m.rot[0] = add_rot(m.rot[0], m.pv_f(0x10));
    m.rot[1] = add_rot(m.rot[1], m.pv_f(0x14));
    let mut v = m.pv_v(0);
    m.pos = add(m.pos, v);
    v[2] -= FALL;
    m.set_pv_v(0, v);
    if m.pos.iter().any(|&c| c < 0.0) {
        w.delete(id);
        return;
    }
    // The last quarter of its life: shrinking to nothing.
    let quarter = m.pv_s(0x1a) as i32 / 4;
    let mut life = m.pv_s(0x18) as i32;
    if life < quarter { m.scale = m.pv_f(0x1c) * life as f32 / quarter as f32; }
    let out = countdown(&mut life);
    m.set_pv_s(0x18, life as i16);
    if out {
        w.delete(id);
        return;
    }
    w.rebuild(id, &*ctx.host);
    w.changed = true;
}
