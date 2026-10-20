//! The bolts, classes 13..19 (L00 `0x381e60`): a bolt flies (state 1: gravity 0.003 a tick, a 0.25 sphere against
//! the world and the mobys, flags 0x22), comes to rest (states 2 / 3), and once Ratchet is within the pickup radii
//! (`0x18c2c4` 2.125 across, `0x18c2c8` 1.25 up) flies to him (state 4, `0x3831a8`: speed toward 0.533 by 0.0067 a
//! tick) and is collected (`0x382c18`: its value to the bolt count `0x1a7a00`, the counter shown 180 ticks). A resting
//! bolt more than 32 from Ratchet that is not drawn goes to the bank.
//!
//! Not ported: the bolt's spin (`+0x30` axis, `+0x50` / `+0x58` angles), the ground settle arc (`0x382e20`), the
//! platforms a bolt rests on (`+0x5c`), the sparkle (`0x33a6a0`), the sound (`0x3605f0`), the per-level tally
//! (`0x18d208`).

use crate::hero::physics::{add, dist, dist2, len, scale, sub, V3};
use crate::moby::bolts::bolt_value;
use crate::moby::{Ctx, MobyWorld};

/// `0x18c2c4` / `0x18c2c8` (Ratchet's pickup radii without the Bolt Grabber).
const PICK_XY: f32 = 2.125;
const PICK_Z: f32 = 1.25;

fn near(pos: V3, hero: V3) -> bool { dist2(pos, hero) < PICK_XY && (pos[2] - hero[2]).abs() < PICK_Z }

/// `0x381e60`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let hero = ctx.hero.pos;
    let state = w.mobys[id].state;
    match state {
        0 => {
            // A placed bolt: rests where it is.
            let m = &mut w.mobys[id];
            m.state = 3;
            m.update_dist = 0x20;
            m.draw_dist = 0x20;
            m.set_pv_f(0x68, m.pos[2]);
        }
        1 => {
            let m = &mut w.mobys[id];
            if m.pv_b(0x42) == 0 && near(m.pos, hero) { m.set_pv_b(0x42, 1); }
            let mut vel = m.pv_v(0x20);
            vel[2] -= 0.003;
            m.pos = add(m.pos, vel);
            m.set_pv_v(0x20, vel);
            let low = m.pv_f(0x68) - 2.0 <= m.pos[2] || hero[2] - 2.0 <= m.pos[2];
            if !low {
                // Fell past where it started and below Ratchet: off to him.
                attract(w, id, ctx);
                return;
            }
            let pos = m.pos;
            let Some(o) = ctx.coll.and_then(|c| rc_game::collision_query::coll_sphere_m(c, ctx.scene, pos, 0.25, rc_game::collision_query::QueryFlags(0x22), Some(id))) else { return };
            let m = &mut w.mobys[id];
            if let Some(pc) = o.pushed_centre { m.pos = pc; }
            // Bounce: the velocity reflected off the contact (`0x2ef2a8`).
            let n = sub(m.pos, o.point);
            let l = len(n);
            if 0.0 < l {
                let n = scale(n, 1.0 / l);
                let d = vel[0] * n[0] + vel[1] * n[1] + vel[2] * n[2];
                vel = sub(vel, scale(n, 2.0 * d));
            }
            m.set_pv_v(0x20, vel);
            if ctx.hero.hp != 0 && (m.pv_b(0x42) != 0 || near(m.pos, hero)) {
                m.set_pv_v(0x20, scale(vel, 0.133_333_34));
                attract(w, id, ctx);
                return;
            }
            // A floor within 45°: it rests (the settle's 30..40 ticks are not ported).
            if 0.0 < l && (n[2] / l) > 0.707_106_77 {
                let t = ctx.game.rng.randf(30.0, 40.0) as u8;
                let m = &mut w.mobys[id];
                m.state = 3;
                m.set_pv_b(0x56, t);
                m.set_pv_b(0x57, t);
                m.set_pv_v(0x20, [0.0; 3]);
            }
        }
        2 | 3 => {
            let m = &w.mobys[id];
            if m.pv_b(0x42) != 0 || near(m.pos, hero) {
                attract(w, id, ctx);
                return;
            }
            if 32.0 < dist(m.pos, hero) && m.drawn == 0 && m.instance.is_none() {
                ctx.game.bolt_bank += bolt_value(m.o_class);
                w.delete(id);
            }
        }
        4 => {
            // Toward Ratchet's head (`0x189f00`): the speed eases to 0.533.
            let target = add(hero, [0.0, 0.0, 1.0]);
            let m = &mut w.mobys[id];
            let mut s = m.pv_f(0x64);
            crate::hero::physics::approach(0.533_333_3, 0.006_666_667, &mut s);
            m.set_pv_f(0x64, s);
            let d = sub(target, m.pos);
            let l = len(d);
            if l <= s.max(0.0) {
                collect(w, id, ctx);
                return;
            }
            if 0.0 < l { m.pos = add(m.pos, scale(d, s / l)); }
            // Ratchet's own move carries it.
            let c = ctx.hero.carry;
            m.pos = add(m.pos, c);
        }
        _ => {}
    }
}

/// `0x3831a8`: state 4, its speed from 0.
fn attract(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 4 { return; }
    m.set_pv_f(0x64, 0.0);
    m.state = 4;
}

/// `0x382c18`: the value to the bolt count, the counter shown, the bolt gone.
fn collect(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let v = bolt_value(w.mobys[id].o_class);
    ctx.game.bolts = ctx.game.bolts.saturating_add(v as u32);
    ctx.game.show_bolts = Some(0xb4);
    w.delete(id);
}
