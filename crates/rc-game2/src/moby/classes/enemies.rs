//! Aranos's enemies on the framework ([`crate::moby::ai`]).
//!
//! **0x099c** (`0x3bb120`, its set-up and checks `0x3bbab0`, its tail `0x3bbf38`): a flying creature. Set up: its
//! movement block at `+0x110` (radius 0.85, no turn), its reaction block at `+0x60` (hurts into state 0xb, sub-state
//! 3), scaled 0.75, home at its height, a yaw offset `+0x290` (random 25°..100° to a side when −1). Each tick it takes
//! damage (a hurt, or the group's wake `+0x2e` = 2, wakes the group's others of its class: `+0x276`; a death wakes one
//! more: `+0x2ac`), and looks for Ratchet in its two areas `+0x250` / `+0x254` (ranges `+0x268` / `+0x26c`).
//!
//! Its sub-states: 0 asleep (turning 2° a tick), woken by the first area (or its group) after `+0x2a4` — onto its
//! path `+0x294` (state 4) or straight to the chase (6); 1 hovering, back to the chase when he is in either area; 2
//! chasing, back to hover when he leaves both; 3 the hurt's flight ([`crate::moby::ai::flight`]): dead, it drops its
//! bolts (`0x31adc0(m, 0, 3)`) and is deleted; over, it chases again (or hovers when he is gone).
//!
//! Its states: 1 rising to its home over 26 ticks; 4 flying its path at `+0x1c0` (slowing for the last six points, at
//! least 0.5 over the ground), then the chase; 5 the chase ([`crate::moby::ai::move_toward`] at its yaw offset, its
//! height kept) until within 1.3 of him (7); 7 a 30-tick wind-up, then 6 its attack sequence 1, 8 the bite (sequence
//! 3 at 1.75: spheres of 0.17 at its joints 0..3 doing `+0x2b0`; his hurt by it → 9; out of reach after 2 s → the
//! chase); 9 20 ticks, then 10 sequence 4 to its end, then the chase. It bobs 0.1 and sways 8° once `+0x272` is out.
//!
//! Not ported: its sounds (`0x3605f0`), the death's burst (`0x3f8348`), the shadow (`0x31da58`), the colour flash
//! (`0x3201e8`), the lighting (`0x31a8e4`).

use crate::hero::physics::*;
use crate::moby::ai;
use crate::moby::{Ctx, MobyWorld};

/// `0x3bbff8`: the moby's group's others of its class (not dead) are woken (`+0x276` = 1).
fn wake_group(w: &mut MobyWorld, id: usize) {
    let (g, class) = (w.mobys[id].group, w.mobys[id].o_class);
    let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() else { return };
    for i in members {
        let m = &mut w.mobys[i];
        if m.state < 0x80 && m.o_class == class { m.set_pv_s(0x276, 1); }
    }
}

/// `0x3bbab0`: the set-up, the damage and the wakes, the areas. Returns whether it is to go.
fn checks_099c(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        m.set_pv_i(0x18, 0x110);
        m.set_pv_f(0x28c, m.pos[2]);
        m.scale *= 0.75;
        m.set_pv_i(0x2a8, -1);
        m.set_pv_b(0x48, 10);
        m.set_pv_b(0x4a, 8);
        let mut yaw = m.pv_f(0x290);
        if yaw == -1.0 { yaw = ctx.game.rng.randf_signed(25.0, 100.0); }
        let m = &mut w.mobys[id];
        m.set_pv_f(0x290, yaw * 0.017_453_294);
        m.update_dist = 0x30;
        for o in [0xd3, 0xcc, 0xcd, 0xce, 0xcf, 0xd2] { m.set_pv_b(o, 0xff); }
        m.set_pv_f(0x1c0, m.pv_f(0x29c) / 60.0);
        // The reaction block (`+0x60`).
        m.set_pv_b(0x70, 0xb);
        m.set_pv_f(0x7c, 0.1);
        m.set_pv_f(0x88, 0.016_666_668);
        m.set_pv_f(0x8c, 0.004_166_667);
        m.set_pv_f(0x9c, 0.4);
        m.set_pv_f(0x80, 0.066_666_67);
        m.set_pv_f(0x84, 0.004_166_667);
        m.set_pv_i(0xa8, 3);
        for o in [0x71, 0x72, 0x76, 0x77] { m.set_pv_b(o, 0); }
        m.set_pv_f(0x90, 0.5);
        m.set_pv_f(0x94, 0.5);
        m.set_pv_b(0xd6, 1);
        // The movement block (`+0x110`).
        m.set_pv_f(0x110, 0.85);
        m.set_pv_f(0x118, 0.75);
        m.set_pv_f(0x120, 1.136_6);
        m.set_pv_f(0x140, 25.132_742);
        m.set_pv_i(0x298, 1);
        m.set_pv_b(0x71, 0);
        m.set_pv_b(0x72, 0);
        m.set_pv_b(0x76, 0);
        m.set_pv_b(0x77, 0);
        m.set_pv_f(0x114, 0.0);
        m.set_pv_f(0x11c, 0.5);
        m.set_pv_f(0x13c, 0.0);
        m.set_pv_i(0x148, m.pv_i(0x148) | 0x10);
        ai::senses(m, 0.006_666_667, 0.066_666_67, 0.839_724_4, 0.839_724_4);
        m.set_pv_v(0x200, m.pos);
        m.set_pv_b(0x28, 3);
        m.set_pv_b(0x2e, 1);
        m.set_pv_f(0x210, 1.0);
        m.set_pv_f(0x21c, 5.0);
        m.set_pv_f(0x220, 10.0);
        m.set_pv_f(0x2b0, m.pv_b(0x34) as f32);
        m.set_pv_f(0x30, m.pv_f(0x30) - 0.5);
        let t = ctx.game.rng.randi(10);
        let m = &mut w.mobys[id];
        m.set_pv_s(0x272, t as i16);
        m.set_state(2, 0);
        super::props::set_seq(w, id, ctx, 0, 0, 0);
    }
    let dying = w.mobys[id].pv_b(0x2e) == 2;
    let kind = ai::take_damage(w, id, ctx, false);
    if kind != 0 || dying {
        wake_group(w, id);
        if matches!(kind, 4 | 5) || dying {
            let g = w.mobys[id].group;
            if let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() {
                let class = w.mobys[id].o_class;
                if 1 < super::props::group_alive(w, g as i32) {
                    if let Some(&i) = members.iter().find(|&&i| w.mobys[i].o_class == class && w.mobys[i].pv_i(0x2ac) == 0) { w.mobys[i].set_pv_i(0x2ac, 1); }
                }
            }
        }
        if dying {
            wake_group(w, id);
            return true;
        }
    }
    let m = &w.mobys[id];
    let (r1, a1, r2, a2) = (m.pv_f(0x268), m.pv_i(0x250), m.pv_f(0x26c), m.pv_i(0x254));
    let f1 = ai::find_target(w, id, ctx, r1, 10.0, a1, None, false);
    w.mobys[id].set_pv_i(0x258, f1 as i32);
    let f2 = ai::find_target(w, id, ctx, r2, 10.0, a2, None, false);
    w.mobys[id].set_pv_i(0x25c, f2 as i32);
    false
}

/// 0x099c (module docs).
pub fn update_099c(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if checks_099c(w, id, ctx) {
        w.delete(id);
        return;
    }
    let spin = |w: &mut MobyWorld| { let m = &mut w.mobys[id]; m.rot[2] = add_rot(m.rot[2], 0.034_906_585); };
    let senses = |w: &mut MobyWorld| {
        let m = &mut w.mobys[id];
        if m.once & 2 == 0 {
            m.once |= 2;
            ai::senses(m, 0.006_666_667, 0.066_666_67, 0.839_724_4, 0.839_724_4);
        }
    };
    let rest = |w: &mut MobyWorld, ctx: &mut Ctx, lo: i32| {
        let t = ctx.game.rng.rand_range(lo, 600);
        w.mobys[id].set_pv_s(0x270, t as i16);
    };
    // The move off: its path, else the chase's attack run.
    let go = |w: &mut MobyWorld, ctx: &mut Ctx| {
        let m = &mut w.mobys[id];
        if m.pv_i(0x294) != -1 {
            m.set_state(4, 2);
        } else {
            m.set_state(6, 2);
            m.pos[2] = m.pv_f(0x28c);
            super::props::set_seq(w, id, ctx, 2, 0, 5);
        }
    };
    match w.mobys[id].sub_state {
        0 => {
            senses(w);
            let m = &mut w.mobys[id];
            let woken = m.pv_i(0x258) == 1 && {
                let mut t = m.pv_i(0x2a4);
                let ready = crate::moby::countdown(&mut t);
                m.set_pv_i(0x2a4, t);
                ready && m.pv_i(0x2ac) != 0
            };
            if woken {
                go(w, ctx);
            } else if m.pv_s(0x276) != 0 {
                m.set_state(3, 1);
                m.set_pv_s(0x276, 0);
                if m.pv_s(0x270) < 300 { rest(w, ctx, 0x1e0); }
            }
            spin(w);
        }
        1 => {
            senses(w);
            let m = &w.mobys[id];
            if (m.pv_i(0x258) == 1 || m.pv_i(0x25c) == 1) && m.pv_i(0x2ac) != 0 { go(w, ctx); }
            spin(w);
        }
        2 => {
            senses(w);
            let m = &mut w.mobys[id];
            if m.pv_i(0x258) != 1 && m.pv_i(0x25c) != 1 && m.state != 4 {
                m.set_state(1, 1);
                super::props::set_seq(w, id, ctx, 0, 0, 0x14);
                rest(w, ctx, 0x244);
            }
        }
        3 => {
            let res = ai::flight(w, id, ctx);
            if res & 1 != 0 {
                let h = w.mobys[id].pv_i(0x2a8);
                if h != -1 { ctx.host.stop_sound(h); }
                w.mobys[id].set_pv_i(0x2a8, -1);
                ctx.host.play_sound(&w.mobys[id], 0, 0);
                crate::moby::bolts::drop(w, id, 0, 3, ctx);
                w.delete(id);
                return;
            }
            if res & 2 != 0 {
                let m = &mut w.mobys[id];
                if m.pv_i(0x258) != 1 && m.pv_i(0x25c) != 1 {
                    m.set_state(3, 1);
                    rest(w, ctx, 0x244);
                } else {
                    m.set_state(5, 2);
                    super::props::set_seq(w, id, ctx, 0, 0, 5);
                    if w.mobys[id].pv_i(0x294) != -1 { w.mobys[id].set_state(4, 2); }
                }
            }
        }
        _ => {}
    }
    let target_pos = |w: &MobyWorld| -> V3 { ai::link(&w.mobys[id], 0x240).and_then(|t| w.mobys.get(t)).map_or(w.mobys[id].pv_v(0x230), |t| t.pos) };
    match w.mobys[id].state {
        1 => {
            let m = &mut w.mobys[id];
            let t = m.state_ticks;
            if t < 0x1a {
                let f = (t as f32 * 0.125_663_61).sin() * 0.02;
                if m.pv_f(0x28c) < m.pos[2] { m.pos[2] -= f } else { m.pos[2] += f }
            } else {
                m.set_state(2, -1);
                m.pos[2] = m.pv_f(0x28c);
                super::props::set_seq(w, id, ctx, 0, 0, 5);
            }
        }
        4 => {
            let m = &w.mobys[id];
            let (path, i) = (m.pv_i(0x294), m.pv_i(0x298));
            let points = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)).cloned().unwrap_or_default();
            if (i as usize) < points.len() {
                let p = points[i as usize];
                let target = [p[0], p[1], p[2]];
                let m = &mut w.mobys[id];
                let d = set_len(sub(target, m.pos), m.pv_f(0x1c0));
                m.pos = add(m.pos, d);
                if dist2(m.pos, target) < 0.5 { m.set_pv_i(0x298, i + 1); }
                if points.len() as i32 - 6 <= m.pv_i(0x298) && 0.1 <= m.pv_f(0x1c0) { m.set_pv_f(0x1c0, m.pv_f(0x1c0) - 0.002_222_222_3); }
                let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
                let m = &mut w.mobys[id];
                if m.pos[2] < g + 0.5 { m.pos[2] = g + 0.5; }
                m.set_pv_f(0x28c, m.pos[2]);
            } else {
                let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
                let m = &mut w.mobys[id];
                m.set_pv_i(0x294, -1);
                m.set_pv_f(0x28c, g + 0.5);
                m.set_state(5, -1);
            }
            spin(w);
            w.changed = true;
        }
        5 => {
            let z = w.mobys[id].pos[2];
            let (off, goal) = (w.mobys[id].pv_f(0x290), w.mobys[id].pv_v(0x230));
            ai::move_toward(w, id, ctx, off, goal);
            w.mobys[id].pos[2] = z;
            if dist2(w.mobys[id].pos, target_pos(w)) < 1.3 { w.mobys[id].set_state(7, -1); }
            spin(w);
        }
        6 => {
            if super::props::wrapped(w, id, ctx) {
                super::props::set_seq(w, id, ctx, 3, 0, 5);
                w.mobys[id].set_state(8, -1);
            }
        }
        7 => {
            if 0x1e <= w.mobys[id].state_ticks {
                w.mobys[id].set_state(6, -1);
                super::props::set_seq(w, id, ctx, 1, 0, 2);
                ctx.host.set_anim_speed(&w.mobys[id], 1.0);
            }
        }
        8 => {
            ctx.host.set_anim_speed(&w.mobys[id], 1.75);
            let dmg = w.mobys[id].pv_f(0x2b0);
            for j in 0..4 { ai::attack_joint(w, id, ctx, 0.17, dmg, 1.0, j, 1, 0, 1, 0); }
            if ctx.hero.damage.hit_by == Some(id) { w.mobys[id].set_state(9, -1); }
            let far = 1.3 < dist2(w.mobys[id].pos, target_pos(w));
            if far && 120 < w.mobys[id].state_ticks {
                super::props::set_seq(w, id, ctx, 0, 0, 0xf);
                w.mobys[id].set_state(5, -1);
                ctx.host.set_anim_speed(&w.mobys[id], 1.0);
            }
        }
        9 => {
            if 20 < w.mobys[id].state_ticks {
                w.mobys[id].set_state(10, -1);
                super::props::set_seq(w, id, ctx, 4, 0, 5);
                ctx.host.set_anim_speed(&w.mobys[id], 1.0);
            }
        }
        10 => {
            if super::props::wrapped(w, id, ctx) {
                super::props::set_seq(w, id, ctx, 0, 0, 0xf);
                w.mobys[id].set_state(5, -1);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    if m.state != 4 && m.sub_state == 2 && m.pos[2] < m.pv_f(0x28c) { m.pos[2] = m.pv_f(0x28c); }
    // `0x3bbf38`: the timers, the bob and the sway once `+0x272` is out, the state's ticks.
    let mut t = m.pv_s(0x270) as i32;
    crate::moby::countdown(&mut t);
    m.set_pv_s(0x270, t as i16);
    let mut t = m.pv_s(0x272) as i32;
    let out = crate::moby::countdown(&mut t);
    m.set_pv_s(0x272, t as i16);
    if out {
        super::props::bob(m, 0.1, 0.087_266_47, -1.0, 0x27c, 0x280);
        super::props::sway(m, 0.139_626_34, 0.026_179_939, 0.026_179_939, 0x284, 0x288);
    }
    if m.state_ticks != -1 { m.state_ticks = m.state_ticks.wrapping_add(1); }
    w.changed = true;
}
