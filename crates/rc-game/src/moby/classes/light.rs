//! The explosion light (class 0x27f, L00 `0x39e7f0` / `0x39e9c8`): an unseen moby that owns one of the point lights
//! (crate::point_lights, the bank `MobyWorld::point_lights`) for a while, its radius and colour ramping per its template.
//!
//! Its pvars (the template's record): `+0x00` the offset from its place, `+0x10` its place, `+0x24` the intensity byte
//! and `+0x25..+0x33` per channel (r, g, b) the start, top, bottom, step up and step down (hundredths), `+0x34..+0x44`
//! the radius's start, top, bottom, step up and step down, `+0x48` its life, `+0x4c` the flags, `+0x50` its slot (−1:
//! none), `+0x54..+0x5c` the colour, `+0x60` the radius, `+0x68..+0x6b` the hold times (r, g, b, radius), `+0x6c..+0x72`
//! the periods and `+0x74..+0x7a` their timers, `+0x7c..+0x7f` the hold timers.
//!
//! The flags, per channel (r, g, b, radius): random (0x10000, 0x20000, 0x40000, 0x80000), hold at the top (0x10,
//! 0x100, 0x1000, 1), holding (0x20, 0x200, 0x2000, 2), repeat (0x40, 0x400, 0x4000, 4), going down (0x80, 0x800,
//! 0x8000, 8); the offset fixed, scaled by `randf(0, 1)` (0x100000) or random within it (0x300000); 0x800000 no offset.
//!
//! Not ported: a light riding a moby (`+0x20` / `+0x64`, its rows with 0x400000): no caller here asks for one.

use crate::hero::physics::{add, scale, V3};
use crate::moby::{countdown, mode, Ctx, MobyWorld};
use crate::point_lights::PointLight;

pub const CLASS: i16 = 0x27f;

/// A light's template (the record `0x39e7f0` copies into the pvars).
#[derive(Clone, Copy, Debug)]
pub struct Template {
    pub offset: V3,
    /// The intensity byte, then r, g, b: start, top, bottom, up, down.
    pub bytes: [u8; 16],
    /// The radius's start, top and bottom.
    pub radius: [f32; 3],
    pub grow: f32,
    pub shrink: f32,
    pub life: i32,
    pub flags: u32,
    pub draw: u8,
    /// The hold times: r, g, b, radius.
    pub holds: [u8; 4],
    /// The periods: r, g, b, radius.
    pub periods: [u16; 4],
    pub delay: u8,
}

/// `0x29aeb0`, the Gravity Bomb blast's light, with the radius 40 `0x3f09e0` writes into it: yellow-white flaring up
/// and fading over 45 ticks, its radius 40 (the random radius between equal bounds).
pub const GRAVITY_BOMB: Template = Template {
    offset: [0.0, 0.0, 1.3],
    bytes: [0x00, 0x00, 0xff, 0x00, 0x13, 0x0f, 0x00, 0xff, 0x00, 0x13, 0x11, 0x00, 0x64, 0x00, 0x13, 0x13],
    radius: [40.0; 3],
    grow: 0.01,
    shrink: 0.01,
    life: 45,
    flags: 0x0008_0000,
    draw: 0xff,
    holds: [0; 4],
    periods: [0; 4],
    delay: 0,
};

mod pv {
    pub const OFFSET: usize = 0x00;
    pub const BASE: usize = 0x10;
    pub const R_MAX: usize = 0x38;
    pub const R_MIN: usize = 0x3c;
    pub const R_UP: usize = 0x40;
    pub const R_DOWN: usize = 0x44;
    pub const LIFE: usize = 0x48;
    pub const FLAGS: usize = 0x4c;
    pub const SLOT: usize = 0x50;
    pub const RGB: usize = 0x54;
    pub const RADIUS: usize = 0x60;
}

/// `0x39e7f0(template, pos, 0, 0)`: the light at `pos`, unseen (mode 1), updated at any distance.
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, t: &Template, pos: V3) -> Option<usize> {
    w.class(CLASS)?;
    let id = w.create(CLASS);
    let m = &mut w.mobys[id];
    m.update = Some(update);
    m.mode = (m.mode & !mode::NO_UPDATE) | 1;
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.update_dist = 0xff;
    m.drawn = 1;
    m.state = 0;
    m.timer = 0;
    m.pos = pos;
    m.draw_dist = t.draw as i16;
    m.set_pv_v(pv::OFFSET, t.offset);
    m.set_pv_i(0x20, 0);
    m.pvars[0x24..0x34].copy_from_slice(&t.bytes);
    m.set_pv_f(0x34, t.radius[0]);
    m.set_pv_f(pv::R_MAX, t.radius[1]);
    m.set_pv_f(pv::R_MIN, t.radius[2]);
    m.set_pv_f(pv::R_UP, t.grow);
    m.set_pv_f(pv::R_DOWN, t.shrink);
    m.set_pv_i(pv::LIFE, t.life);
    m.set_pv_i(pv::FLAGS, t.flags as i32);
    m.pvars[0x68..0x6c].copy_from_slice(&t.holds);
    for (k, p) in t.periods.iter().enumerate() { m.set_pv_s(0x6c + 2 * k, *p as i16); }
    m.set_pv_i(0x64, -1);
    m.set_pv_b(0x1c, t.delay);
    w.rebuild(id, &*ctx.host);
    w.changed = true;
    Some(id)
}

/// Its slot back to the bank.
fn free_slot(w: &mut MobyWorld, id: usize) {
    let s = w.mobys[id].pv_i(pv::SLOT);
    if s != -1 {
        w.point_lights.free(s as usize);
        w.mobys[id].set_pv_i(pv::SLOT, -1);
    }
}

/// `0x39e9c8`: state 0 keeps its place, its start colour and radius; then a tick of its start delay, its life (out:
/// the slot freed, the moby gone), the view test (`0x2f0080(255, its sphere)`: out of view the slot is freed and
/// nothing else runs), its place, its radius and channels (module doc), and its slot: taken (`0x2fdb60`) when it has
/// none, else rewritten with the colour, the intensity, the place and 0.8 of the radius.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.update_dist = 0xff;
        m.timer = m.pv_b(0x1c);
        let p = m.pos;
        m.set_pv_v(pv::BASE, p);
        m.set_pv_i(pv::SLOT, -1);
        m.set_pv_s(0x76, m.pv_s(0x6e));
        m.set_pv_s(0x74, m.pv_s(0x6c));
        m.set_pv_s(0x78, m.pv_s(0x70));
        m.set_pv_s(0x7a, m.pv_s(0x72));
        for (k, o) in [0x25, 0x2a, 0x2f].into_iter().enumerate() { m.set_pv_f(pv::RGB + 4 * k, m.pv_b(o) as f32 * 0.01); }
        m.set_pv_f(pv::RADIUS, m.pv_f(0x34));
        m.set_pv_f(pv::R_UP, m.pv_f(pv::R_UP).abs());
        m.set_pv_f(pv::R_DOWN, m.pv_f(pv::R_DOWN).abs());
        m.state = 1;
    }
    if m.timer != 0 {
        m.timer -= 1;
        return;
    }
    let mut life = m.pv_i(pv::LIFE);
    if life != -1 && countdown(&mut life) {
        w.mobys[id].set_pv_i(pv::LIFE, life);
        free_slot(w, id);
        w.delete(id);
        return;
    }
    m.set_pv_i(pv::LIFE, life);
    let (p, r) = (m.pos, m.pv_f(pv::RADIUS));
    if ctx.view.is_some_and(|v| v.culled(255.0, [p[0], p[1], p[2], r])) {
        free_slot(w, id);
        return;
    }
    let g = &mut ctx.game.rng;
    let mut flags = m.pv_i(pv::FLAGS) as u32;
    let base = m.pv_v(pv::BASE);
    m.pos = if flags & 0x80_0000 != 0 {
        base
    } else {
        let o = m.pv_v(pv::OFFSET);
        let o = if flags & 0x10_0000 == 0 {
            o
        } else if flags & 0x20_0000 == 0 {
            scale(o, g.randf(0.0, 1.0))
        } else {
            [g.randf(-o[0], o[0]), g.randf(-o[1], o[1]), g.randf(-o[2], o[2])]
        };
        add(base, o)
    };
    // The radius: its period restarts it at the bottom going up (with repeat).
    let period = m.pv_s(0x72);
    let mut restarted = false;
    if period != 0 {
        let mut t = m.pv_s(0x7a) as i32;
        let out = countdown(&mut t);
        m.set_pv_s(0x7a, t as i16);
        if out {
            restarted = flags & 4 != 0;
            m.set_pv_s(0x7a, period);
            if restarted {
                m.set_pv_f(pv::RADIUS, m.pv_f(pv::R_MIN));
                flags &= !8;
            }
        }
    }
    if flags & 0x8_0000 != 0 {
        if period == 0 || restarted { m.set_pv_f(pv::RADIUS, g.randf(m.pv_f(pv::R_MIN), m.pv_f(pv::R_MAX))); }
    } else {
        if flags & 3 == 2 { flags ^= 2; }
        let mut v = m.pv_f(pv::RADIUS);
        if flags & 2 != 0 {
            if m.pv_b(0x6b) != 0 {
                let mut h = m.pv_b(0x7f) as i32;
                let out = countdown(&mut h);
                m.set_pv_b(0x7f, h as u8);
                if out { flags ^= 2; }
            }
        } else if flags & 8 == 0 {
            v += m.pv_f(pv::R_UP);
            if m.pv_f(pv::R_MAX) <= v {
                v = m.pv_f(pv::R_MAX);
                if flags & 1 == 0 {
                    flags ^= 8;
                } else {
                    flags = (flags | 2) ^ 8;
                    m.set_pv_b(0x7f, m.pv_b(0x6b));
                }
            }
        } else {
            v -= m.pv_f(pv::R_DOWN);
            if v <= m.pv_f(pv::R_MIN) {
                v = m.pv_f(pv::R_MIN);
                if flags & 4 != 0 && period == 0 { flags ^= 8; }
            }
        }
        m.set_pv_f(pv::RADIUS, v);
    }
    for k in 0..3 {
        channel(m, g, &mut flags, k);
    }
    m.set_pv_i(pv::FLAGS, flags as i32);
    let rgb = [m.pv_f(pv::RGB), m.pv_f(pv::RGB + 4), m.pv_f(pv::RGB + 8)];
    let l = PointLight { color: rgb, intensity: m.pv_b(0x24) as f32 * 0.01, pos: m.pos, radius: m.pv_f(pv::RADIUS) * 0.8 };
    let slot = m.pv_i(pv::SLOT);
    if slot == -1 {
        let s = w.point_lights.alloc(l).map_or(-1, |s| s as i32);
        w.mobys[id].set_pv_i(pv::SLOT, s);
    } else {
        w.point_lights.set(slot as usize, l);
    }
}

/// One colour channel `k` (r, g, b): its bytes `+0x25 + 5k` (start, top, bottom, up, down), its hold `+0x68 + k` and
/// hold timer `+0x7c + k`, its period `+0x6c + 2k` and timer `+0x74 + 2k`, its flags (module doc). The period restarts
/// it at its bottom *byte* (unscaled, as the game does) going up; random: `randf(bottom, top)` (without a period); else
/// it ramps up to the top (then holds, with the hold flag), down to the bottom (turning up again with repeat and no
/// period).
fn channel(m: &mut crate::moby::Moby, g: &mut crate::rng::Rng, flags: &mut u32, k: usize) {
    let b = 0x25 + 5 * k;
    let (top, bottom, up, down) = (m.pv_b(b + 1), m.pv_b(b + 2), m.pv_b(b + 3), m.pv_b(b + 4));
    let sh = 4 * k as u32;
    let (hold, holding, repeat, going_down, random) = (0x10 << sh, 0x20 << sh, 0x40 << sh, 0x80 << sh, 0x1_0000 << k);
    let at = pv::RGB + 4 * k;
    let period = m.pv_s(0x6c + 2 * k);
    if period != 0 {
        let mut t = m.pv_s(0x74 + 2 * k) as i32;
        let out = countdown(&mut t);
        m.set_pv_s(0x74 + 2 * k, t as i16);
        if out {
            m.set_pv_s(0x74 + 2 * k, period);
            m.set_pv_f(at, bottom as f32);
            *flags &= !going_down;
        }
    }
    if *flags & random != 0 {
        if period == 0 { m.set_pv_f(at, g.randf(bottom as f32 * 0.01, top as f32 * 0.01)); }
        return;
    }
    if *flags & (hold | holding) == holding { *flags ^= holding; }
    let mut v = m.pv_f(at);
    if *flags & holding != 0 {
        if m.pv_b(0x68 + k) != 0 {
            let mut h = m.pv_b(0x7c + k) as i32;
            let out = countdown(&mut h);
            m.set_pv_b(0x7c + k, h as u8);
            if out { *flags ^= holding; }
        }
        return;
    }
    if *flags & going_down == 0 {
        v += up as f32 * 0.01;
        let t = top as f32 * 0.01;
        if t <= v {
            v = t;
            if *flags & hold == 0 {
                *flags ^= going_down;
            } else {
                *flags = (*flags | holding) ^ going_down;
                m.set_pv_b(0x7c + k, m.pv_b(0x68 + k));
            }
        }
    } else {
        v -= down as f32 * 0.01;
        let t = bottom as f32 * 0.01;
        if v <= t {
            v = t;
            if *flags & repeat != 0 && period == 0 { *flags ^= going_down; }
        }
    }
    m.set_pv_f(at, v);
}
