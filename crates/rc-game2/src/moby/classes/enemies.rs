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

// ---------------------------------------------------------------------------------------------------
// 0x0b0a (`0x3d6900`, its set-up and checks `0x3d7a48`, its tail `0x3d81e0`, its bite `0x3d89f0`): a burrower.
//
// Set up: scaled 1.1, mirrored half the time, its movement block at `+0x110` (radius 0.8, gravity 1/180, 30° of turn
// left before it walks), its reaction block at `+0x60` (hurts into state 0xf, sub-state 5). The point above it it
// digs up to (`+0x380`: 20 up from its joint 0, or what is in the way; a cuboid's `+0x288`). Starting underground
// (`+0x27c`, or a cuboid) it waits as a mound (0x0ab7, `+0x2ac`, which takes the hits), else it stands (state 1).
// Each tick: its joints' points (`+0x390..`, the bite's mouth `+0x3c0`), the damage and the wakes (its group's
// burrowers `+0x276`; a death lets the group's next burrower or 0x0b0b up, `+0x294`), its two areas (`+0x250` /
// `+0x254`, ranges `+0x268` / `+0x26c`); under z 90 it bursts with its bolts.
//
// Sub-states: 0 standing — with Ratchet in the first area at its height (within 2) it rises (0xb; from a cuboid 10,
// out of the ground); woken (`+0x276`), 9 / 3; 2 the mound — with him in the first area it comes up (5), the mound's
// hits taking its health (15-tick wait); 3 waiting — him in an area: up (0xb), else after `+0x270` back to the mound
// (2, sequence 3); 4 fighting (faster senses) — him gone from both areas: wait (9, sequence 6); 5 the hurt's flight:
// dead, the sound 0xc, the mound gone, debris (`0x3d8d90`), its bolts, deleted; over, back to the fight (0xc) or to
// wait.
//
// States: 1 still; 3 swaying; 4 / 5 digging down (3 under the ground, eased; the loop sound 0xd) facing him, then out
// (8); 6 → 7 sinking; 8 the emerge sequence, then the fight (0xc); 10 climbing to `+0x380` less 3, then 7; 0xb up,
// then the fight; 0xc the fight: within 2.8 and facing him within 22° it bites (sequence 0x1c: three spheres of 0.3
// along its joint 4 doing `+0x298`), else it walks up to 2.8 of him (its yaw offset `+0x284`) or turns to him; 0xd the
// bite's turn; 0xe after a bite: farther than 4, the fight (0x1c), else sequence 7.
//
// Not ported: its sounds but the two, the debris and the burst (`0x3d8d90`, `0x3210a8`, `0x3f8348`), the shadow, the
// colour flash, the head's and tail's aims (`0x324208` / `0x327308`, the joint records `+0x3d0` / `+0x450`), the
// draw callbacks, the gravity frames.

/// `0x3d82c0`: the moby's group's burrowers (not dead) are woken (`+0x276` = 1).
fn wake_burrowers(w: &mut MobyWorld, id: usize) {
    let g = w.mobys[id].group;
    let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() else { return };
    for i in members {
        let m = &mut w.mobys[i];
        if m.state < 0x80 && m.o_class == 0x0b0a { m.set_pv_s(0x276, 1); }
    }
}

/// `0x3d7a48`. Returns whether it is to go.
fn checks_0b0a(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    if w.mobys[id].state == 0 {
        let mirror = ctx.game.rng.randi(2) == 1;
        let m = &mut w.mobys[id];
        m.scale *= 1.1;
        m.set_pv_i(0x18, 0x110);
        m.set_pv_f(0x284, m.pv_f(0x284) * 0.017_453_294);
        m.set_pv_b(0x48, 20);
        m.set_pv_b(0x4a, 16);
        if mirror { m.mode |= crate::moby::mode::MIRROR; }
        m.set_pv_f(0x90, 0.5);
        m.set_pv_f(0x94, 0.5);
        m.set_pv_f(0x114, 0.5);
        m.set_pv_i(0x2c0, -1);
        m.set_pv_b(0x77, 0x18);
        m.set_pv_b(0x70, 0xf);
        m.set_pv_i(0xa8, 5);
        m.set_pv_b(0x71, 0x13);
        m.set_pv_b(0x72, 0x15);
        m.set_pv_b(0x76, 0x17);
        m.set_pv_f(0x7c, 0.1);
        m.set_pv_f(0x80, 0.066_666_67);
        m.set_pv_f(0x88, 0.008_333_334);
        m.set_pv_f(0x8c, 0.004_166_667);
        m.set_pv_b(0xd3, 0xff);
        m.set_pv_f(0x110, 0.8);
        m.set_pv_f(0x11c, 0.4);
        m.set_pv_f(0x120, 1.136_6);
        m.set_pv_f(0x140, 0.523_598_8);
        m.set_pv_f(0x13c, 0.005_555_556);
        m.set_pv_f(0x84, 0.004_166_667);
        m.set_pv_f(0x9c, 0.4);
        for o in [0xcc, 0xcd, 0xce, 0xcf, 0xd2] { m.set_pv_b(o, 0xff); }
        m.set_pv_f(0x118, 0.4);
        ai::senses(m, 0.004_444_445, 0.133_333_33, 0.001_163_553, 0.104_719_76);
        m.set_pv_v(0x200, m.pos);
        m.set_pv_b(0x2e, 1);
        m.set_pv_i(0x280, 0);
        m.set_pv_f(0x210, 1.0);
        m.set_pv_f(0x21c, 5.0);
        m.set_pv_f(0x220, 10.0);
        m.set_pv_f(0x298, m.pv_b(0x34) as f32);
        m.set_pv_b(0x28, 3);
        let j0 = ctx.host.joint(&w.mobys[id], 0).unwrap_or(w.mobys[id].pos);
        let top = [j0[0], j0[1], j0[2] + 20.0];
        let hit = ctx.coll.and_then(|c| rc_game::collision_query::coll_line_m(c, ctx.scene, j0, top, rc_game::collision_query::QueryFlags(0), Some(id)));
        let m = &mut w.mobys[id];
        m.set_pv_v(0x390, j0);
        m.set_pv_v(0x380, hit.map_or(top, |o| o.point));
        if m.pv_i(0x27c) == 0 && m.pv_i(0x288) == -1 {
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            m.pos[2] = g;
            m.set_state(1, 0);
            super::props::set_seq(w, id, ctx, 0, 0, 0);
        } else {
            m.set_state(3, 2);
            super::props::set_seq(w, id, ctx, 0xb, 0, 0);
            let m = &mut w.mobys[id];
            m.has_collision = false;
            m.set_pv_f(0x30, 2.5);
            let (pos, ud, dd) = (m.pos, m.update_dist, m.draw_dist);
            let h = w.create(0x0ab7);
            let hm = &mut w.mobys[h];
            hm.update_dist = ud;
            hm.draw_dist = dd;
            hm.drawn = 1;
            hm.mode |= 0x41;
            hm.pos = pos;
            w.rebuild(h, &*ctx.host);
            ai::set_link(&mut w.mobys[id], 0x2ac, Some(h));
        }
        let m = &mut w.mobys[id];
        let c = m.pv_i(0x288);
        if c != -1 {
            m.set_state(2, 0);
            if let Some((p, _)) = ctx.level.cuboid_place(c) { w.mobys[id].set_pv_v(0x380, p); }
            w.mobys[id].update_dist = 0x80;
        }
    }
    // The points of its joints 0..2 and the mouth (0.25 out along joint 3).
    for (k, o) in [(0, 0x390), (1, 0x3a0), (2, 0x3b0)] {
        if let Some(p) = ctx.host.joint(&w.mobys[id], k) { w.mobys[id].set_pv_v(o, p); }
    }
    if let Some((r, t)) = ctx.host.joint_matrix(&w.mobys[id], 3) { w.mobys[id].set_pv_v(0x3c0, add(t, scale(r[0], 0.25))); }
    let alive = w.mobys[id].pv_b(0x2e) != 2;
    let kind = ai::take_damage(w, id, ctx, false);
    if kind != 0 || !alive {
        let m = &mut w.mobys[id];
        m.set_pv_f(0x30, 0.5);
        if kind == 5 {
            m.rot[0] = 0.0;
            m.rot[1] = 0.839_724_4;
        }
        wake_burrowers(w, id);
        if matches!(kind, 4 | 5) || !alive {
            let g = w.mobys[id].group;
            if 1 < super::props::group_alive(w, g as i32) {
                let members = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default();
                let next_too = w.mobys[id].pv_i(0x29c) != 0;
                for i in members {
                    let c = w.mobys[i].o_class;
                    if (c == 0x0b0a || c == 0x0b0b) && w.mobys[i].pv_i(0x294) == 0 {
                        w.mobys[i].set_pv_i(0x294, 1);
                        if c == 0x0b0a || !next_too { break; }
                    }
                }
            }
        }
    }
    let m = &w.mobys[id];
    let (r1, a1, r2, a2) = (m.pv_f(0x268), m.pv_i(0x250), m.pv_f(0x26c), m.pv_i(0x254));
    let f1 = ai::find_target(w, id, ctx, r1, 10.0, a1, None, false);
    w.mobys[id].set_pv_i(0x258, f1 as i32);
    let f2 = ai::find_target(w, id, ctx, r2, 10.0, a2, None, false);
    w.mobys[id].set_pv_i(0x25c, f2 as i32);
    if w.mobys[id].pos[2] < 90.0 {
        crate::moby::bolts::drop(w, id, 0, 3, ctx);
        return true;
    }
    if !alive {
        // `0x3d8188`: the mound goes; the group is woken.
        if let Some(h) = ai::link(&w.mobys[id], 0x2ac).filter(|&h| w.mobys.get(h).is_some_and(|m| m.o_class == 0x0ab7)) {
            w.delete(h);
            ai::set_link(&mut w.mobys[id], 0x2ac, None);
        }
        wake_burrowers(w, id);
        return true;
    }
    false
}

/// `0x3d89f0(damage, m, 0, joint)`: three spheres of 0.3 along the joint's first row (at it, 0.85 and 1.6 out, the
/// last two 0.1 lower).
fn bite(w: &mut MobyWorld, id: usize, ctx: &Ctx, damage: f32, joint: usize) {
    let Some((r, t)) = ctx.host.joint_matrix(&w.mobys[id], joint) else { return };
    let mut b = add(t, set_len(r[1], 0.85));
    let mut c = add(t, set_len(r[1], 1.6));
    b[2] -= 0.1;
    c[2] -= 0.1;
    for p in [t, b, c] { ai::attack_sphere(w, id, ctx, 0.3, damage, 1.0, p, 1, 0, 1, 0); }
}

/// 0x0b0a (above).
pub fn update_0b0a(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if checks_0b0a(w, id, ctx) {
        w.delete(id);
        return;
    }
    let coll = w.class(0x0b0a).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    m.mode = (m.mode & 0xffbe) | crate::moby::mode::TARGET;
    m.has_collision = coll;
    let slow = |m: &mut crate::moby::Moby| ai::senses(m, 0.002_222_222_4, 0.066_666_67, 0.001_163_553, 0.104_719_76);
    let fast = |m: &mut crate::moby::Moby| ai::senses(m, 0.004_444_445, 0.133_333_33, 0.001_163_553, 0.104_719_76);
    let first = |m: &mut crate::moby::Moby| { let f = m.once & 2 == 0; m.once |= 2; f };
    let rest = |w: &mut MobyWorld, ctx: &mut Ctx, lo: i32| { let t = ctx.game.rng.rand_range(lo, 600); w.mobys[id].set_pv_s(0x270, t as i16); };
    let wait = |w: &mut MobyWorld, ctx: &mut Ctx| {
        w.mobys[id].set_state(9, 3);
        super::props::set_seq(w, id, ctx, 6, 0, 10);
        let t = ctx.game.rng.rand_range(0x244, 600);
        w.mobys[id].set_pv_s(0x270, t as i16);
    };
    let rise = |w: &mut MobyWorld, ctx: &mut Ctx| {
        w.mobys[id].set_state(0xb, 4);
        super::props::set_seq(w, id, ctx, 1, 0, 4);
    };
    let up = |w: &mut MobyWorld, ctx: &mut Ctx| {
        w.mobys[id].set_state(5, -1);
        super::props::set_seq(w, id, ctx, 0x19, 0, 4);
        w.mobys[id].set_pv_i(0x2b0, 1);
    };
    match w.mobys[id].sub_state {
        0 => {
            let m = &mut w.mobys[id];
            if first(m) { slow(m); }
            let m = &w.mobys[id];
            let close = m.pv_i(0x258) == 1 && m.pv_i(0x294) != 0 && (m.pos[2] - m.pv_f(0x238)).abs() < 2.0;
            if close {
                rise(w, ctx);
                if w.mobys[id].pv_i(0x288) != -1 {
                    w.mobys[id].set_state(10, 1);
                    super::props::set_seq(w, id, ctx, 0x1a, 0, 4);
                }
            } else if w.mobys[id].pv_s(0x276) != 0 {
                let m = &mut w.mobys[id];
                m.set_state(9, 3);
                m.set_pv_s(0x276, 0);
                if m.pv_s(0x270) < 300 { rest(w, ctx, 0x1e0); }
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            if first(m) { slow(m); }
            let m = &w.mobys[id];
            let start = if m.pv_i(0x258) == 1 { m.pv_i(0x294) != 0 && m.pv_i(0x2b0) == 0 } else { m.pv_s(0x276) != 0 && m.pv_i(0x294) != 0 && m.pv_i(0x2b0) == 0 };
            if start {
                if m.pv_i(0x258) != 1 { w.mobys[id].set_pv_s(0x276, 0); }
                up(w, ctx);
            }
            // The mound takes the hits.
            if let Some(h) = ai::link(&w.mobys[id], 0x2ac) {
                if let Some(rec) = w.hit_message(h).filter(|r| r.tmpl.flags & 0x10000 != 0) {
                    if w.mobys[id].pv_s(0x26) < 1 {
                        let m = &mut w.mobys[id];
                        m.set_pv_f(0x20, m.pv_f(0x20) - rec.tmpl.damage);
                        m.set_pv_s(0x26, 0xf);
                        w.clear_hit(h);
                        if w.mobys[id].pv_i(0x2b0) == 0 { up(w, ctx); }
                        if w.mobys[id].pv_f(0x20) <= 0.0 { w.mobys[id].set_state(0xf, 5); }
                    }
                } else {
                    w.clear_hit(h);
                }
                let p = w.mobys[id].pos;
                if let Some(hm) = w.mobys.get_mut(h) { hm.pos = p; }
            }
        }
        3 => {
            let m = &mut w.mobys[id];
            if first(m) { slow(m); }
            let m = &w.mobys[id];
            let seen = m.pv_i(0x258) == 1 || m.pv_i(0x25c) == 1;
            if seen && m.pv_i(0x294) != 0 {
                rise(w, ctx);
            } else if w.mobys[id].pv_s(0x270) == 0 {
                w.mobys[id].set_state(2, 0);
                super::props::set_seq(w, id, ctx, 3, 0, 10);
            }
        }
        4 => {
            let m = &mut w.mobys[id];
            if first(m) { fast(m); }
            let m = &w.mobys[id];
            if m.pv_i(0x258) != 1 && !(m.pv_i(0x25c) == 1 || m.state == 5 || m.state == 4) { wait(w, ctx); }
        }
        5 => {
            let res = ai::flight(w, id, ctx);
            let h = w.mobys[id].pv_i(0x2c0);
            if h != -1 { ctx.host.stop_sound(h); }
            w.mobys[id].set_pv_i(0x2c0, -1);
            if res & 1 != 0 || w.mobys[id].pv_f(0x20) <= 0.0 {
                ctx.host.play_sound(&w.mobys[id], 0xc, 0);
                if let Some(hm) = ai::link(&w.mobys[id], 0x2ac).filter(|&h| w.mobys.get(h).is_some_and(|m| m.o_class == 0x0ab7)) { w.delete(hm); }
                crate::moby::bolts::drop(w, id, 0, 3, ctx);
                w.delete(id);
                return;
            }
            if res & 2 != 0 {
                let m = &w.mobys[id];
                if m.pv_i(0x258) != 1 && m.pv_i(0x25c) != 1 {
                    wait(w, ctx);
                } else {
                    w.mobys[id].set_state(0xc, 4);
                    super::props::set_seq(w, id, ctx, 0x1c, 0, 10);
                }
            }
        }
        _ => {}
    }
    let target = ai::link(&w.mobys[id], 0x240).and_then(|t| w.mobys.get(t)).map(|t| t.pos);
    let goal = w.mobys[id].pv_v(0x230);
    let tp = target.unwrap_or(goal);
    let to_target = atan2(tp[0] - w.mobys[id].pos[0], tp[1] - w.mobys[id].pos[1]);
    let face = |w: &mut MobyWorld| ai::turn_to(&mut w.mobys[id], to_target, 0.001, 0.01, 0.1, 0x28c);
    let tilt_back = |w: &mut MobyWorld| {
        let m = &mut w.mobys[id];
        let mut v = m.pv_f(0x290);
        spring(0.0, 0.016_666_668, 0.0, 0.066_666_67, &mut m.rot[1], &mut v);
        spring(0.0, 0.016_666_668, 0.0, 0.066_666_67, &mut m.rot[0], &mut v);
        m.set_pv_f(0x290, v);
    };
    let surface = |w: &mut MobyWorld, ctx: &mut Ctx| {
        let h = w.mobys[id].pv_i(0x2c0);
        if h != -1 { ctx.host.stop_sound(h); }
        w.mobys[id].set_pv_i(0x2c0, -1);
        w.mobys[id].set_state(8, -1);
        super::props::set_seq(w, id, ctx, 0xe, 0, 3);
    };
    match w.mobys[id].state {
        1 => {
            if w.mobys[id].pv_i(0x2bc) == 0 { ai::hold(w, id, ctx); }
        }
        3 => super::props::sway(&mut w.mobys[id], 0.069_813_17, 0.052_359_88, 0.013_089_97, 0x2a4, 0x2a8),
        4 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x2a0, m.pv_i(0x2a0) + 1);
            face(w);
            tilt_back(w);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                let mut v = m.pv_f(0x280);
                ease(g - 3.0, 0.016_666_668, 0.163_475_5, 0.166_666_67, &mut m.pos[2], &mut v);
                m.set_pv_f(0x280, v);
            } else {
                surface(w, ctx);
                let coll = w.class(0x0b0a).is_some_and(|c| c.has_collision);
                w.mobys[id].has_collision = coll;
                if let Some(h) = ai::link(&w.mobys[id], 0x2ac).filter(|&h| w.mobys.get(h).is_some_and(|m| m.o_class == 0x0ab7)) { w.delete(h); }
                let m = &mut w.mobys[id];
                m.set_pv_f(0x30, 0.5);
                m.mode |= crate::moby::mode::TARGET;
            }
        }
        5 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x2a0, m.pv_i(0x2a0) + 1);
            if !ctx.host.sound_playing(m.pv_i(0x2c0)) {
                let h = ctx.host.play_sound(&w.mobys[id], 0xd, 4);
                w.mobys[id].set_pv_i(0x2c0, h);
            }
            face(w);
            tilt_back(w);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                m.pos[2] -= g * 0.000_25;
            } else {
                surface(w, ctx);
            }
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(4, -1);
                super::props::set_seq(w, id, ctx, 0x1a, 0, 0xf);
            }
        }
        6 => {
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(7, -1);
                super::props::set_seq(w, id, ctx, 0xd, 0, 0xf);
            }
        }
        7 => {
            let m = &mut w.mobys[id];
            let mut v = m.pv_f(0x290);
            spring(0.0, 0.016_666_668, 0.0, 0.066_666_67, &mut m.rot[1], &mut v);
            m.set_pv_f(0x290, v);
            face(w);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                let mut v = m.pv_f(0x280);
                ease(g - 3.0, 0.025, 0.163_475_5, 0.25, &mut m.pos[2], &mut v);
                m.set_pv_f(0x280, v);
            } else {
                w.mobys[id].set_state(8, -1);
                super::props::set_seq(w, id, ctx, 0xe, 0, 3);
            }
        }
        8 => {
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(0xc, 4);
                super::props::set_seq(w, id, ctx, 0x1c, 0, 0xf);
                let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
                w.mobys[id].pos[2] = g;
            }
        }
        10 => {
            let m = &mut w.mobys[id];
            let t = m.pv_v(0x380);
            let target = [t[0], t[1], t[2] - 3.0];
            let d = sub(target, m.pos);
            m.rot[2] = atan2(d[0], d[1]);
            m.rot[1] = atan2(len2(d), d[2]);
            let mut v = m.pv_f(0x280);
            ai::spring_to(&mut m.pos, target, 0.016_666_668, 0.0, 0.333_333_34, &mut v);
            m.set_pv_f(0x280, v);
            if dist(m.pos, target) < 0.4 {
                super::props::set_seq(w, id, ctx, 0xd, 0, 10);
                w.mobys[id].set_state(7, -1);
            }
        }
        0xb => {
            ai::hold(w, id, ctx);
            if super::props::wrapped(w, id, ctx) {
                ai::senses(&mut w.mobys[id], 0.001_111_111, 0.033_333_335, 0.001_163_553, 0.104_719_76);
                w.mobys[id].set_state(0xc, -1);
                super::props::set_seq(w, id, ctx, 0x1c, 0, 0xf);
            }
        }
        0xc => {
            ai::senses(&mut w.mobys[id], 0.001_111_111, 0.033_333_335, 0.001_163_553, 0.104_719_76);
            let m = &w.mobys[id];
            let d = dist2(m.pos, tp);
            let yaw_goal = atan2(goal[0] - m.pos[0], goal[1] - m.pos[1]);
            let facing = d < 2.8 && rot_dist(m.rot[2], yaw_goal) < 0.383_972_47;
            if !facing {
                if 2.8 < d {
                    let off = w.mobys[id].pv_f(0x284);
                    ai::move_toward(w, id, ctx, off, goal);
                } else {
                    let p = w.mobys[id].pos;
                    ai::move_step(w, id, ctx, to_target, p);
                }
            }
            if super::props::seq(w, id, ctx) == 0x1c {
                let dmg = w.mobys[id].pv_f(0x298);
                bite(w, id, ctx, dmg, 4);
            }
        }
        0xd => {
            let p = w.mobys[id].pos;
            ai::move_step(w, id, ctx, to_target, p);
        }
        0xe => {
            if super::props::wrapped(w, id, ctx) {
                if 4.0 < dist2(w.mobys[id].pos, tp) {
                    ai::senses(&mut w.mobys[id], 0.004_444_445, 0.133_333_33, 0.001_163_553, 0.104_719_76);
                    w.mobys[id].set_state(0xc, -1);
                    super::props::set_seq(w, id, ctx, 0x1c, 0, 10);
                } else {
                    ai::senses(&mut w.mobys[id], 0.002_222_222_4, 0.066_666_67, 0.001_163_553, 0.104_719_76);
                    w.mobys[id].set_state(0xc, -1);
                    super::props::set_seq(w, id, ctx, 7, 0, 10);
                }
            }
        }
        _ => {}
    }
    // `0x3d81e0`: the timers and the state's ticks.
    let m = &mut w.mobys[id];
    for o in [0x270, 0x272] {
        let mut t = m.pv_s(o) as i32;
        crate::moby::countdown(&mut t);
        m.set_pv_s(o, t as i16);
    }
    if m.state_ticks != -1 { m.state_ticks = m.state_ticks.wrapping_add(1); }
    w.changed = true;
}
