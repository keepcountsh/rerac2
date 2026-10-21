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

// ---------------------------------------------------------------------------------------------------
// 0x0b0b (`0x3d9020`, its set-up and checks `0x3dac08`, its tail `0x3db4b8`): the burrower's bigger kin, with two
// spinning parts (0x0aa2, `+0x2a0` / `+0x2a4`, at its joints 0 and 2).
//
// Set up as 0x0b0a (scaled 1.1, mirrored half the time, the movement block at `+0x110`: radius 0.8, gravity 1/180, 30°
// of turn left; hurts into state 0xf, sub-state 5); its wander (`+0x200`: home, radius 1, 3 from home, 10 from
// Ratchet), its wander's rest `+0x2d8` (120..700 ticks), the bite's damage `+0x2cc`. On the ground (state 1; `+0x2dc`:
// wandering, 2), else as a mound (0x0ab7, `+0x2e0`; state 3, sub-state 2). Each tick: the damage and the wakes (its
// group's 0x0b0b `+0x276`; a death, or more than 20 over the ground once, lets the group's next 0x0b0a or 0x0b0b up,
// `+0x294`); `+0x2ec`: west of x 170.7 it falls away (state 0x10, sub-state 6); its two areas; under z 90 it bursts.
//
// Sub-states: 0 on the ground — him in the first area after `+0x2d0`, or within 3.4, up (0xb, sequence 1 or 5; a
// cuboid: 10); woken, 9 / 3; 2 the mound — him in the first area after `+0x2d0`, or woken: out (5, sequence 0x19),
// the mound's hits taking its health; 3 waiting — him in an area and less than 3.25 above: up, else after `+0x270` the
// wander (2, sequence 4); 4 fighting — him gone from the areas (or above): rest (9, sequence 7); 5 the hurt's flight:
// dead, its parts, its mound, its bolts, deleted; over, the fight (0xc) or rest; with `+0x2ec`, a sphere of 0.9
// around it; 6 the fall away: 0.4 a tick down y until the flight ends or under z 100.
//
// States: 1 (`+0x2f0`) holding; 2 / 9 wandering ([`crate::moby::ai::wander`], sequence 8 / 0xb, with rests 4 / 2 and
// 6 / 7); 3 swaying; 4 / 5 down out of the mound facing him, 7 down to the ground, 8 the emerge (sequence 0xf) then
// the fight; 10 the climb to `+0x300`; 0xb up, then the fight; 0xc the fight: along its path (`+0x2f8`) first, within
// 2.8 facing him within 12° the bite (0xd, sequence 0x1b or 0x10), else up to him; 0xd the bite: frames 19..25 four
// spheres at its joints 0, 1, 2, 9 doing `+0x2cc`, its parts spinning faster (25°) from frame 17; his hurt by it → 0xe
// (sequence 0x11, its parts still through frames 12..15). The parts spin 8° a tick but on the ground, as a mound or
// in the flight.
//
// Not ported: its sounds but the two, the debris and the bursts (`0x3210a8`, `0x3f8348`, `0x327498`), the parts'
// sparks (`0x3dbc20`), the bite's trail (`0x3dbde8`, `0x3dbe88`, `0x3dc100`), the shadow, the colour flash, the head's
// aims (`0x324208`, `0x327308`), the draw callbacks, the scene's hiding (`0x1a8f00` = 2).

const PART: i16 = 0x0aa2;
const PART_JOINTS: [usize; 2] = [0, 2];

/// `0x3db680`: the group's 0x0b0b (not dead) are woken.
fn wake_0b0b(w: &mut MobyWorld, id: usize) {
    let g = w.mobys[id].group;
    let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() else { return };
    for i in members {
        let m = &mut w.mobys[i];
        if m.state < 0x80 && m.o_class == 0x0b0b { m.set_pv_s(0x276, 1); }
    }
}

/// The group's first 0x0b0a or 0x0b0b still held (`+0x294` = 0) is let up, while more than one of the group lives.
fn let_next_up(w: &mut MobyWorld, id: usize) {
    let g = w.mobys[id].group;
    if super::props::group_alive(w, g as i32) < 2 { return; }
    let members = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default();
    if let Some(&i) = members.iter().find(|&&i| matches!(w.mobys[i].o_class, 0x0b0a | 0x0b0b) && w.mobys[i].pv_i(0x294) == 0) {
        w.mobys[i].set_pv_i(0x294, 1);
    }
}

/// `0x3253f0(m, part, joint, 0)`: the part at the joint's frame (its rows made unit), shown as `m` is.
fn place_part(w: &mut MobyWorld, id: usize, p: usize, joint: usize, ctx: &Ctx) {
    let Some((r, t)) = ctx.host.joint_matrix(&w.mobys[id], joint) else { return };
    let hidden = w.mobys[id].mode & 1 != 0;
    let pm = &mut w.mobys[p];
    pm.pos = t;
    pm.rows = r.map(|row| set_len(row, 1.0));
    pm.mode = if hidden { pm.mode | 0x41 } else { pm.mode & 0xffbe };
    pm.mode |= crate::moby::mode::NO_UPDATE | crate::moby::mode::NO_MATRIX | crate::moby::mode::KEEP_ROWS;
}

/// `0x3dbed0(m, +0x2a0, 0, 2)`: the two parts made (`0x32a748`: at it, its draw distance; scaled 0.75) at its joints.
fn make_parts(w: &mut MobyWorld, id: usize, ctx: &Ctx) {
    for (k, j) in PART_JOINTS.into_iter().enumerate() {
        let (pos, rot, dd) = (w.mobys[id].pos, w.mobys[id].rot, w.mobys[id].draw_dist);
        let p = w.create(PART);
        let pm = &mut w.mobys[p];
        pm.update_dist = 0xff;
        pm.drawn = 1;
        pm.parent = Some(id);
        pm.pos = pos;
        pm.rot = rot;
        pm.mode &= 0xfff8;
        pm.scale *= 0.75;
        pm.draw_dist = dd;
        w.rebuild(p, &*ctx.host);
        place_part(w, id, p, j, ctx);
        w.rebuild(p, &*ctx.host);
        ai::set_link(&mut w.mobys[id], 0x2a0 + 4 * k, Some(p));
    }
}

/// The parts (`+0x2a0` / `+0x2a4`) that are still there.
fn parts(w: &MobyWorld, id: usize) -> [Option<usize>; 2] {
    [0, 1].map(|k| ai::link(&w.mobys[id], 0x2a0 + 4 * k).filter(|&p| w.mobys.get(p).is_some_and(|m| m.state < 0x80 && m.o_class == PART)))
}

/// `0x3dc090`: the parts go.
fn delete_parts(w: &mut MobyWorld, id: usize) {
    for p in parts(w, id).into_iter().flatten() { w.delete(p); }
    ai::set_link(&mut w.mobys[id], 0x2a0, None);
    ai::set_link(&mut w.mobys[id], 0x2a4, None);
}

/// `0x3dbfb0(angle, m, parts, _)`: each part at its joint, turned `angle` about its own z.
fn spin_parts(w: &mut MobyWorld, id: usize, ctx: &Ctx, angle: f32) {
    let rz = crate::moby::attach::rows_of([0.0, 0.0, angle]);
    for (p, j) in parts(w, id).into_iter().zip(PART_JOINTS) {
        let Some(p) = p else { continue };
        place_part(w, id, p, j, ctx);
        let r = w.mobys[p].rows;
        w.mobys[p].rows = rz.map(|z| add(add(scale(r[0], z[0]), scale(r[1], z[1])), scale(r[2], z[2])));
        w.rebuild(p, &*ctx.host);
    }
}

/// The mound (`+0x2e0`) goes.
fn delete_mound(w: &mut MobyWorld, id: usize) {
    if let Some(h) = ai::link(&w.mobys[id], 0x2e0).filter(|&h| w.mobys.get(h).is_some_and(|m| m.o_class == 0x0ab7)) { w.delete(h); }
    ai::set_link(&mut w.mobys[id], 0x2e0, None);
}

/// `0x3dac08`. Returns whether it is to go.
fn checks_0b0b(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    if w.mobys[id].state == 0 {
        let mirror = ctx.game.rng.randi(2) == 1;
        let m = &mut w.mobys[id];
        m.scale *= 1.1;
        m.set_pv_b(0x48, 20);
        m.set_pv_b(0x4a, 16);
        if mirror { m.mode |= crate::moby::mode::MIRROR; }
        m.set_pv_i(0x18, 0x110);
        m.set_pv_i(0x2ac, 0);
        m.set_pv_i(0x2bc, 0);
        m.set_pv_i(0x2c0, -1);
        m.set_pv_f(0x284, m.pv_f(0x284) * 0.017_453_294);
        m.set_pv_v(0x520, m.pos);
        m.set_pv_f(0x2e8, 8.0);
        m.set_pv_b(0x70, 0xf);
        m.set_pv_i(0xa8, 5);
        m.set_pv_f(0x9c, 0.4);
        m.set_pv_i(0x2c4, 0);
        m.set_pv_i(0x2c8, 0);
        m.set_pv_f(0x90, if m.pv_i(0x2ec) != 0 { 0.07 } else { 0.5 });
        m.set_pv_f(0x94, 0.9);
        m.set_pv_f(0x114, 0.5);
        m.set_pv_f(0x110, 0.8);
        m.set_pv_f(0x11c, 0.9);
        m.set_pv_f(0x120, 1.553_343);
        m.set_pv_f(0x140, 0.523_598_8);
        m.set_pv_f(0x13c, 0.005_555_556);
        m.set_pv_f(0x118, 0.9);
        ai::senses(m, 0.003_333_333_5, 0.100_000_01, 0.001_163_553, 0.104_719_76);
        m.set_pv_v(0x200, m.pos);
        m.set_pv_f(0x21c, 3.0);
        m.set_pv_f(0x220, 10.0);
        m.set_pv_f(0x210, 1.0);
        let t = ctx.game.rng.rand_range(120, 700);
        let m = &mut w.mobys[id];
        m.set_pv_b(0x2e, 1);
        m.set_pv_i(0x280, 0);
        m.set_pv_i(0x2e4, 0);
        m.set_pv_b(0x28, 3);
        m.set_pv_i(0x2d8, t);
        m.set_pv_f(0x2cc, m.pv_b(0x34) as f32);
        // The point above it it climbs to: 20 up from its joint 3, or what is in the way.
        let j3 = ctx.host.joint(&w.mobys[id], 3).unwrap_or(w.mobys[id].pos);
        let mut top = [j3[0], j3[1], j3[2] + 20.0];
        let hit = ctx.coll.and_then(|c| rc_game::collision_query::coll_line_m(c, ctx.scene, j3, top, rc_game::collision_query::QueryFlags(0), Some(id)));
        if let Some(h) = hit { top[2] = h.point[2]; }
        let m = &mut w.mobys[id];
        m.set_pv_v(0x300, top);
        m.set_pv_v(0x310, j3);
        if m.pv_i(0x27c) == 0 && m.pv_i(0x288) == -1 {
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            m.pos[2] = g;
            m.set_state(1, 0);
            super::props::set_seq(w, id, ctx, 0, 0, 0);
            if w.mobys[id].pv_i(0x2dc) != 0 {
                w.mobys[id].set_state(2, 0);
                super::props::set_seq(w, id, ctx, 4, 0, 10);
            }
        } else {
            m.set_state(3, 2);
            super::props::set_seq(w, id, ctx, 0xc, 0, 0);
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
            ai::set_link(&mut w.mobys[id], 0x2e0, Some(h));
        }
        let c = w.mobys[id].pv_i(0x288);
        if c != -1 {
            w.mobys[id].set_state(2, 0);
            if let Some((p, _)) = ctx.level.cuboid_place(c) { w.mobys[id].set_pv_v(0x300, p); }
            w.mobys[id].update_dist = 0x80;
        }
        make_parts(w, id, ctx);
    }
    let alive = w.mobys[id].pv_b(0x2e) != 2;
    if let Some(p) = ctx.host.joint(&w.mobys[id], 3) { w.mobys[id].set_pv_v(0x310, p); }
    let kind = ai::take_damage(w, id, ctx, false);
    if kind != 0 || !alive {
        let m = &mut w.mobys[id];
        m.set_pv_f(0x30, 0.5);
        if kind == 5 {
            m.rot[0] = 0.0;
            m.rot[1] = 0.839_724_4;
        }
        wake_0b0b(w, id);
        if matches!(kind, 4 | 5) || !alive { let_next_up(w, id); }
    }
    // Once more than 20 over the ground, the group's next is let up.
    let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
    let m = &mut w.mobys[id];
    if 20.0 < m.pos[2] - g && 0 <= m.group && m.pv_i(0x2c8) == 0 {
        m.set_pv_i(0x2c8, 1);
        let_next_up(w, id);
    }
    let m = &mut w.mobys[id];
    if m.pv_i(0x2ec) != 0 && m.pos[0] < 170.7 && m.sub_state != 6 { m.set_state(0x10, 6); }
    let m = &w.mobys[id];
    let (r1, a1, r2, a2) = (m.pv_f(0x268), m.pv_i(0x250), m.pv_f(0x26c), m.pv_i(0x254));
    let f1 = ai::find_target(w, id, ctx, r1, 10.0, a1, None, false);
    w.mobys[id].set_pv_i(0x258, f1 as i32);
    let f2 = ai::find_target(w, id, ctx, r2, 10.0, a2, None, false);
    w.mobys[id].set_pv_i(0x25c, f2 as i32);
    if w.mobys[id].pos[2] < 90.0 {
        delete_parts(w, id);
        crate::moby::bolts::drop(w, id, 0, 3, ctx);
        return true;
    }
    if !alive {
        // `0x3db458`.
        delete_parts(w, id);
        delete_mound(w, id);
        wake_0b0b(w, id);
        return true;
    }
    false
}

/// 0x0b0b (above).
pub fn update_0b0b(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if checks_0b0b(w, id, ctx) {
        w.delete(id);
        return;
    }
    if w.mobys[id].state != 0xc && w.mobys[id].sub_state != 2 { ai::hold(w, id, ctx); }
    let coll = w.class(0x0b0b).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    m.mode = (m.mode & 0xffbe) | crate::moby::mode::TARGET;
    m.has_collision = coll;
    let part_coll = w.class(PART).is_some_and(|c| c.has_collision);
    for p in parts(w, id).into_iter().flatten() {
        let pm = &mut w.mobys[p];
        pm.mode = (pm.mode & 0xffbe) | crate::moby::mode::TARGET;
        pm.has_collision = part_coll;
    }
    let slow = |m: &mut crate::moby::Moby| ai::senses(m, 0.002_222_222_4, 0.066_666_67, 0.001_163_553, 0.104_719_76);
    let fast = |m: &mut crate::moby::Moby| ai::senses(m, 0.003_333_333_5, 0.100_000_01, 0.001_163_553, 0.104_719_76);
    let first = |m: &mut crate::moby::Moby| { let f = m.once & 2 == 0; m.once |= 2; f };
    let seq_a = |w: &MobyWorld, ctx: &Ctx| ctx.host.anim(&w.mobys[id]).map_or(0xff, |a| a.seq_a);
    let seq_b = |w: &MobyWorld, ctx: &Ctx| super::props::seq(w, id, ctx);
    // Up: sequence 1 from its first, else 5 at a random frame.
    let up_seq = |w: &mut MobyWorld, ctx: &mut Ctx| {
        if seq_a(w, ctx) == 0 {
            super::props::set_seq(w, id, ctx, 1, 0, 4);
        } else if seq_b(w, ctx) != 5 {
            let f = ctx.game.rng.randi(2) as u8;
            let b = ctx.game.rng.rand_range(10, 20) as u8;
            ctx.host.set_anim(&w.mobys[id], 5, f, b);
        }
    };
    let out_of_mound = |w: &mut MobyWorld, ctx: &mut Ctx| {
        w.mobys[id].set_state(5, -1);
        super::props::set_seq(w, id, ctx, 0x19, 0, 4);
        w.mobys[id].set_pv_i(0x2e4, 1);
    };
    let target = ai::link(&w.mobys[id], 0x240).and_then(|t| w.mobys.get(t)).map(|t| t.pos);
    let goal = w.mobys[id].pv_v(0x230);
    let tp = target.unwrap_or(goal);
    match w.mobys[id].sub_state {
        0 => {
            let m = &mut w.mobys[id];
            if first(m) { slow(m); }
            let m = &mut w.mobys[id];
            if m.pv_i(0x258) == 1 && m.pv_i(0x294) != 0 {
                let mut t = m.pv_i(0x2d0);
                let ready = crate::moby::countdown(&mut t);
                m.set_pv_i(0x2d0, t);
                if ready {
                    m.set_state(0xb, 4);
                    up_seq(w, ctx);
                    if w.mobys[id].pv_i(0x288) != -1 {
                        w.mobys[id].set_state(10, 1);
                        super::props::set_seq(w, id, ctx, 0x1a, 0, 4);
                    }
                }
            } else if target.is_some_and(|p| dist2(m.pos, p) <= 3.4) {
                m.set_state(0xb, 4);
                up_seq(w, ctx);
                w.mobys[id].set_pv_i(0x294, 1);
            } else if m.pv_s(0x276) != 0 && m.pv_i(0x294) != 0 {
                m.set_state(9, 3);
                m.set_pv_s(0x276, 0);
                if m.pv_s(0x270) < 300 {
                    let t = ctx.game.rng.rand_range(0x1e0, 600);
                    w.mobys[id].set_pv_s(0x270, t as i16);
                }
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            if first(m) { slow(m); }
            m.has_collision = false;
            let held = m.pv_i(0x294) == 0 || m.pv_i(0x2e4) != 0;
            if m.pv_i(0x258) == 1 && !held {
                let mut t = m.pv_i(0x2d0);
                let ready = crate::moby::countdown(&mut t);
                m.set_pv_i(0x2d0, t);
                if ready { out_of_mound(w, ctx); }
            } else if m.pv_s(0x276) != 0 && !held {
                m.set_pv_s(0x276, 0);
                out_of_mound(w, ctx);
            }
            // The mound takes the hits.
            if let Some(h) = ai::link(&w.mobys[id], 0x2e0).filter(|&h| h < w.mobys.len()) {
                if let Some(rec) = w.hit_message(h).filter(|r| r.tmpl.flags & 0x10000 != 0) {
                    if w.mobys[id].pv_s(0x26) < 1 {
                        let m = &mut w.mobys[id];
                        m.set_pv_f(0x20, m.pv_f(0x20) - rec.tmpl.damage);
                        m.set_pv_s(0x26, 0xf);
                        w.clear_hit(h);
                        if w.mobys[id].pv_i(0x2e4) == 0 { out_of_mound(w, ctx); }
                        if w.mobys[id].pv_f(0x20) <= 0.0 { w.mobys[id].set_state(0xf, 5); }
                    }
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
            if seen && m.pv_i(0x294) != 0 && ctx.hero.pos[2] - m.pos[2] < 3.25 {
                w.mobys[id].set_state(0xb, 4);
                up_seq(w, ctx);
            } else if m.pv_s(0x270) == 0 {
                w.mobys[id].set_state(2, 0);
                super::props::set_seq(w, id, ctx, 4, 0, 10);
            }
        }
        4 => {
            let m = &mut w.mobys[id];
            if first(m) { fast(m); }
            let m = &w.mobys[id];
            let seen = m.pv_i(0x258) == 1 || m.pv_i(0x25c) == 1;
            if !(seen && m.pv_f(0x238) - m.pos[2] < 3.25) && m.state != 5 && m.state != 4 {
                w.mobys[id].set_state(9, 3);
                if seq_b(w, ctx) != 7 {
                    let f = ctx.game.rng.randi(2) as u8;
                    let b = ctx.game.rng.rand_range(10, 20) as u8;
                    ctx.host.set_anim(&w.mobys[id], 7, f, b);
                }
                let t = ctx.game.rng.rand_range(0x244, 600);
                w.mobys[id].set_pv_s(0x270, t as i16);
            }
        }
        5 => {
            let res = ai::flight(w, id, ctx);
            if res & 1 != 0 || (w.mobys[id].pv_f(0x20) <= 0.0 && w.mobys[id].pv_i(0x2ec) == 0) {
                ctx.host.play_sound(&w.mobys[id], 0xd, 0);
                delete_parts(w, id);
                delete_mound(w, id);
                crate::moby::bolts::drop(w, id, 0, 3, ctx);
                w.delete(id);
                return;
            }
            if res & 2 != 0 {
                let m = &w.mobys[id];
                if m.pv_i(0x258) == 1 || m.pv_i(0x25c) == 1 {
                    w.mobys[id].set_state(0xc, 4);
                    super::props::set_seq(w, id, ctx, 0xb, 0, 10);
                } else {
                    w.mobys[id].set_state(9, 3);
                    super::props::set_seq(w, id, ctx, 7, 0, 10);
                    let t = ctx.game.rng.rand_range(0x244, 600);
                    w.mobys[id].set_pv_s(0x270, t as i16);
                }
                w.mobys[id].set_pv_i(0x294, 1);
            }
            if w.mobys[id].pv_i(0x2ec) != 0 {
                let p = w.mobys[id].pos;
                ai::attack_sphere(w, id, ctx, 0.9, 0.1, 1.0, p, 0x10000, 0, 1, 0);
            }
        }
        6 => {
            ai::flight(w, id, ctx);
            if super::props::wrapped(w, id, ctx) || w.mobys[id].pos[2] < 100.0 {
                delete_parts(w, id);
                crate::moby::bolts::drop(w, id, 0, 3, ctx);
                w.delete(id);
                return;
            }
            w.mobys[id].pos[1] -= 0.4;
        }
        _ => {}
    }
    let to_target = atan2(tp[0] - w.mobys[id].pos[0], tp[1] - w.mobys[id].pos[1]);
    let face = |w: &mut MobyWorld| ai::turn_to(&mut w.mobys[id], to_target, 0.001, 0.01, 0.1, 0x28c);
    let tilt_back = |w: &mut MobyWorld, both: bool| {
        let m = &mut w.mobys[id];
        let mut v = m.pv_f(0x290);
        spring(0.0, 0.016_666_668, 0.0, 0.066_666_67, &mut m.rot[1], &mut v);
        if both { spring(0.0, 0.016_666_668, 0.0, 0.066_666_67, &mut m.rot[0], &mut v); }
        m.set_pv_f(0x290, v);
    };
    let emerge = |w: &mut MobyWorld, ctx: &mut Ctx| {
        w.mobys[id].set_state(8, -1);
        super::props::set_seq(w, id, ctx, 0xf, 0, 3);
    };
    let stop_loop = |w: &mut MobyWorld, ctx: &mut Ctx| {
        let h = w.mobys[id].pv_i(0x2c0);
        if h != -1 { ctx.host.stop_sound(h); }
        w.mobys[id].set_pv_i(0x2c0, -1);
    };
    // After the bite or its hurt: the fight again, at a run when he is farther than 4.
    let back_to_fight = |w: &mut MobyWorld, ctx: &mut Ctx| {
        if 4.0 < dist2(w.mobys[id].pos, tp) {
            fast(&mut w.mobys[id]);
            w.mobys[id].set_state(0xc, -1);
            super::props::set_seq(w, id, ctx, 0xb, 0, 10);
        } else {
            slow(&mut w.mobys[id]);
            w.mobys[id].set_state(0xc, -1);
            super::props::set_seq(w, id, ctx, 8, 0, 10);
        }
    };
    // The wander with its rests (`walk` the walk sequence, `rests` the two rest sequences, `speed` the wander's).
    let roam = |w: &mut MobyWorld, ctx: &mut Ctx, walk: u8, rests: [u8; 2], speed: f32, step: f32, hi: i32| {
        let m = &mut w.mobys[id];
        let mut t = m.pv_i(0x2d8);
        crate::moby::countdown(&mut t);
        m.set_pv_i(0x2d8, t);
        let b = seq_b(w, ctx);
        if b != walk && !rests.contains(&b) { super::props::set_seq(w, id, ctx, walk, 0, 0xe); }
        let m = &mut w.mobys[id];
        let mut s = m.pv_f(0x214);
        approach(if m.pv_i(0x2d8) < 0x15 { 0.0 } else { speed }, step, &mut s);
        m.set_pv_f(0x214, s);
        let b = seq_b(w, ctx);
        if b != walk {
            if rests.contains(&b) && super::props::wrapped(w, id, ctx) {
                let t = ctx.game.rng.rand_range(120, hi);
                w.mobys[id].set_pv_i(0x2d8, t);
                super::props::set_seq(w, id, ctx, walk, 0, 0xc);
            }
            return;
        }
        if w.mobys[id].pv_i(0x2d8) == 0 {
            ctx.host.set_anim(&w.mobys[id], rests[1], 0, 0xc);
            if ctx.game.rng.randi(3) == 1 { ctx.host.set_anim(&w.mobys[id], rests[0], 0, 0xc); }
        }
        ai::wander(w, id, ctx, 0.35, 0.25, 0x200);
    };
    match w.mobys[id].state {
        1 => {
            if w.mobys[id].pv_i(0x2f0) != 0 { ai::hold(w, id, ctx); }
        }
        2 => roam(w, ctx, 8, [2, 4], 0.05, 0.000_833_333_36, 700),
        3 => super::props::sway(&mut w.mobys[id], 0.069_813_17, 0.052_359_88, 0.013_089_97, 0x298, 0x29c),
        4 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x2bc, m.pv_i(0x2bc) + 1);
            face(w);
            tilt_back(w, true);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                let mut v = m.pv_f(0x280);
                ease(g - 3.0, 0.016_666_668, 0.163_475_5, 0.166_666_67, &mut m.pos[2], &mut v);
                m.set_pv_f(0x280, v);
            } else {
                stop_loop(w, ctx);
                emerge(w, ctx);
                let coll = w.class(0x0b0b).is_some_and(|c| c.has_collision);
                w.mobys[id].has_collision = coll;
                delete_mound(w, id);
                let m = &mut w.mobys[id];
                m.set_pv_f(0x30, 0.5);
                m.mode |= crate::moby::mode::TARGET;
            }
        }
        5 => {
            if !ctx.host.sound_playing(w.mobys[id].pv_i(0x2c0)) {
                let h = ctx.host.play_sound(&w.mobys[id], 0xb, 4);
                w.mobys[id].set_pv_i(0x2c0, h);
            }
            let m = &mut w.mobys[id];
            m.set_pv_i(0x2bc, m.pv_i(0x2bc) + 1);
            face(w);
            tilt_back(w, true);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                m.pos[2] -= g * 0.000_25;
            } else {
                stop_loop(w, ctx);
                emerge(w, ctx);
            }
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(4, -1);
                super::props::set_seq(w, id, ctx, 0x1a, 0, 0xf);
            }
        }
        6 => {
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(7, -1);
                super::props::set_seq(w, id, ctx, 0xe, 0, 0xf);
            }
        }
        7 => {
            tilt_back(w, false);
            face(w);
            let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
            let m = &mut w.mobys[id];
            if 0.05 < m.pos[2] - g {
                let mut v = m.pv_f(0x280);
                ease(g - 3.0, 0.025, 0.163_475_5, 0.25, &mut m.pos[2], &mut v);
                m.set_pv_f(0x280, v);
            } else {
                emerge(w, ctx);
            }
        }
        8 => {
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_state(0xc, 4);
                super::props::set_seq(w, id, ctx, 8, 0, 0xf);
                let coll = w.class(0x0b0b).is_some_and(|c| c.has_collision);
                w.mobys[id].has_collision = coll;
                let g = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
                w.mobys[id].pos[2] = g;
            }
        }
        9 => roam(w, ctx, 0xb, [7, 6], 0.1, 0.001_666_666_7, 200),
        10 => {
            let m = &mut w.mobys[id];
            let t = m.pv_v(0x300);
            let target = [t[0], t[1], t[2] - 3.0];
            let d = sub(target, m.pos);
            m.rot[2] = atan2(d[0], d[1]);
            m.rot[1] = atan2(len2(d), d[2]);
            let mut v = m.pv_f(0x280);
            ai::spring_to(&mut m.pos, target, 0.016_666_668, 0.0, 0.333_333_34, &mut v);
            m.set_pv_f(0x280, v);
            if dist(m.pos, target) < 0.4 {
                super::props::set_seq(w, id, ctx, 0xe, 0, 10);
                w.mobys[id].set_state(7, -1);
            }
        }
        0xb => {
            ai::hold(w, id, ctx);
            if super::props::wrapped(w, id, ctx) {
                fast(&mut w.mobys[id]);
                w.mobys[id].set_state(0xc, -1);
                super::props::set_seq(w, id, ctx, 0xb, 0, 0xf);
            }
        }
        0xc => {
            let path = w.mobys[id].pv_i(0x2f8);
            if path == -1 {
                let m = &w.mobys[id];
                let d = dist2(m.pos, tp);
                let yaw_goal = atan2(goal[0] - m.pos[0], goal[1] - m.pos[1]);
                if d < 2.8 && rot_dist(m.rot[2], yaw_goal) < 0.209_439_53 {
                    w.mobys[id].set_state(0xd, -1);
                    super::props::set_seq(w, id, ctx, 0x1b, 0, 6);
                    if ctx.game.rng.randi(2) == 1 { ctx.host.set_anim(&w.mobys[id], 0x10, 0, 6); }
                    w.mobys[id].set_pv_i(0x2b0, 0);
                } else if d <= 2.8 {
                    let p = w.mobys[id].pos;
                    ai::move_step(w, id, ctx, to_target, p);
                } else {
                    let off = w.mobys[id].pv_f(0x284);
                    ai::move_toward(w, id, ctx, off, goal);
                }
            } else if ai::follow_path(w, id, ctx, path) || dist2(w.mobys[id].pos, tp) <= 3.4 {
                w.mobys[id].set_pv_i(0x2f8, -1);
            }
            if 5.0 < dist2(w.mobys[id].pos, tp) && seq_a(w, ctx) == 8 {
                super::props::set_seq(w, id, ctx, 0xb, 0, 4);
                fast(&mut w.mobys[id]);
            }
        }
        0xd => {
            if super::props::wrapped(w, id, ctx) {
                w.mobys[id].set_pv_f(0x2e8, 8.0);
                if ctx.hero.damage.hit_by == Some(id) {
                    w.mobys[id].set_state(0xe, -1);
                    super::props::set_seq(w, id, ctx, 0x11, 0, 7);
                } else {
                    back_to_fight(w, ctx);
                }
            }
            ai::hold(w, id, ctx);
            let f = ctx.host.frame_time(&w.mobys[id]);
            if matches!(seq_a(w, ctx), 0x1b | 0x10) {
                if (19.0..=25.0).contains(&f) {
                    let dmg = w.mobys[id].pv_f(0x2cc);
                    for (r, j) in [(0.35, 0), (0.15, 1), (0.15, 2), (0.35, 9)] { ai::attack_joint(w, id, ctx, r, dmg, 1.0, j, 1, 0, 1, 0); }
                }
                if (17.0..=25.0).contains(&f) { w.mobys[id].set_pv_f(0x2e8, 25.0); }
            }
        }
        0xe => {
            if super::props::wrapped(w, id, ctx) { back_to_fight(w, ctx); }
            ai::hold(w, id, ctx);
            let f = ctx.host.frame_time(&w.mobys[id]);
            let still = seq_a(w, ctx) == 0x11 && !(5.0 < f && (f < 12.0 || 15.0 < f));
            w.mobys[id].set_pv_i(0x2c4, still as i32);
        }
        _ => {}
    }
    // `0x3db4b8`: the parts spun (not on the ground, as a mound, in the flight or held still), the timers, the
    // state's ticks.
    let m = &mut w.mobys[id];
    let mut angle = m.pv_f(0x2ac);
    if m.sub_state != 0 && m.state != 3 && m.sub_state != 5 && m.pv_i(0x2c4) == 0 {
        angle = add_rot(angle, m.pv_f(0x2e8) * 0.017_453_294);
        m.set_pv_f(0x2ac, angle);
    }
    spin_parts(w, id, ctx, angle);
    let m = &mut w.mobys[id];
    for o in [0x270, 0x272, 0x26] {
        let mut t = m.pv_s(o) as i32;
        crate::moby::countdown(&mut t);
        m.set_pv_s(o, t as i16);
    }
    if m.state_ticks != -1 { m.state_ticks = m.state_ticks.wrapping_add(1); }
    w.changed = true;
}
