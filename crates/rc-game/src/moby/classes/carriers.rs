//! The carriers of Aranos (the group attach, [`crate::moby::attach`]).
//!
//! * 0x0a38 (`0x3c4358`): takes its moby group `+0x68`[4] and tie group [0] (at most 39 members), then turns about
//!   its y axis by [8] °/s, carrying them.
//! * 0x0c66 (`0x3ec540`): takes its tie group [0], moby group [4] and shrub group [8] (at most 0x4f; none: it is
//!   deleted) and, while the level's variant (`0x1c92c0`) is its [0xc]: bobs by [0x30] at [0x34] °/s about its
//!   height (`0x324430`), sways [0x38]° at [0x3c] °/s (and 1/3 °/tick, `0x3244e8`), and spins by [0x10..0x18] °/s —
//!   itself with its frame its rows ([0x1c] = 0), else its members in its frame — carrying them.

use crate::moby::attach::{angles_of, rows_of, turn};
use crate::moby::{mode, Ctx, MobyWorld};

/// °/s as radians a tick.
const DEG_S: f32 = 0.000_290_888_25;

/// 0x0a38.
pub fn update_0a38(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        if m.pv_i(0) == -1 && m.pv_i(4) == -1 { return; }
        m.update_dist = 0xff;
        let (moby, tie) = (m.pv_i(4), m.pv_i(0));
        w.attach(id, 100, moby, tie, -1);
        w.attach_limit(id, 39);
        w.mobys[id].state = 1;
    } else if m.state == 1 {
        m.rot[1] = crate::hero::physics::add_rot(m.rot[1], m.pv_f(8) * DEG_S);
        w.carry(id, &*ctx.host);
    }
}

/// 0x0c66.
pub fn update_0c66(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state == 0 {
        if m.pv_i(0) == -1 && m.pv_i(4) == -1 { return; }
        m.update_dist = 0xff;
        let (moby, tie, shrub) = (m.pv_i(4), m.pv_i(0), m.pv_i(8));
        w.attach(id, 0x4f, moby, tie, shrub);
        if !w.attached.contains_key(&id) {
            w.delete(id);
            return;
        }
        let m = &mut w.mobys[id];
        let own = m.pv_i(0x1c) == 0;
        if own {
            m.mode |= mode::KEEP_ROWS | mode::NO_MATRIX;
            m.rows = rows_of(m.rot);
        }
        m.set_pv_f(0x18f0, m.pos[2]);
        let ph = m.pv_f(0x20);
        if ph != 0.0 { m.set_pv_f(0x20, ph * 0.017_453_294); }
        m.set_pv_f(0x24, m.pv_f(0x20).sin() * m.pv_f(0x30));
        m.state = 1;
        if own { w.attach_use_rows(id); }
        return;
    }
    if m.state != 1 || ctx.game.variant != m.pv_i(0xc) { return; }
    let own = m.pv_i(0x1c) == 0;
    if 0.0 < m.pv_f(0x34) {
        let (amp, step, base) = (m.pv_f(0x30), m.pv_f(0x34) * DEG_S, m.pv_f(0x18f0));
        super::props::bob(m, amp, step, base, 0x20, 0x24);
    }
    let sway = 0.0 < m.pv_f(0x3c);
    if sway {
        let (tilt, step) = (m.pv_f(0x38) * 0.017_453_294, m.pv_f(0x3c) * DEG_S);
        super::props::sway(m, tilt, step, 0.005_817_764, 0x28, 0x2c);
        if own { m.rows = rows_of(m.rot); }
    }
    let spin = rows_of([m.pv_f(0x10) * DEG_S, m.pv_f(0x14) * DEG_S, m.pv_f(0x18) * DEG_S]);
    if own {
        m.rows = turn(&m.rows, &spin);
        if sway { m.rot = angles_of(&m.rows); }
    } else {
        w.attach_turn(id, &spin);
    }
    w.carry(id, &*ctx.host);
}

/// 0x09c7 (`0x3c3be8`, a floating rock with a mount): in the level's variant [0x40] (or any, < 0), it takes its tie
/// group [0x10], moby group [0x14] and shrub group [0x18] (at most 100; on level 2 the mobys ride its joints —
/// not ported) and, while in view (`0x2f0080(500, sphere)`: the camera within 500 of its sphere [0..0xc] with the
/// radius [0x44]), bobs ([0x30] at [0x34] °/s about its start height), sways ([0x38]° at [0x3c] °/s) and turns
/// [0x1c] °/s, carrying them. With [0x48] ≠ 0 a hidden target (0x0e9b, mode 0x5041) sits on its tie of class
/// 0x0ab6; hit (flags 0x10000), it swells to 1.35 and blows the rock apart: 30..50 bolts (flag 2, 3..5 mobys), the
/// sound 0 of the target, its ties gone, both deleted (the debris `0x32b858`, the burst `0x3c4230` and
/// the sound 0xe9b `0x360698` wait for the effects and sound passes); the blast `0x3f8348` (crate::moby::blast).
pub fn update_09c7(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let variant = m.pv_i(0x40);
    if 0 <= variant && ctx.game.variant != variant { return; }
    let target = |w: &MobyWorld| usize::try_from(w.mobys[id].pv_i(0x4c)).ok().filter(|&t| t != 0 && t < w.mobys.len());
    if m.state == 0 {
        if m.pv_i(0x10) == -1 && m.pv_i(0x14) == -1 { return; }
        let m = &mut w.mobys[id];
        m.update_dist = 0xff;
        m.set_pv_i(0x4c, 0);
        if m.pv_i(0x48) != 0 {
            let t = w.create(0x0e9b);
            let tm = &mut w.mobys[t];
            tm.parent = Some(id);
            tm.mode |= 0x41 | 0x5000;
            tm.rot = [0.0; 3];
            w.mobys[id].set_pv_i(0x4c, t as i32);
        }
        let m = &w.mobys[id];
        let (tie, moby, shrub) = (m.pv_i(0x10), m.pv_i(0x14), m.pv_i(0x18));
        w.attach(id, 100, moby, tie, shrub);
        // The mount's tie: the group's first of class 0x0ab6.
        let mount = usize::try_from(tie).ok().and_then(|g| w.tie_groups.get(g)).and_then(|g| g.iter().find(|&&t| w.tie_classes.get(t as usize) == Some(&0x0ab6))).map_or(-1, |&t| t as i32);
        let mount = if target(w).is_some() { mount } else { -1 };
        w.mobys[id].set_pv_i(0x1fb0, mount);
        let m = &mut w.mobys[id];
        m.set_pv_v(0, m.pos);
        m.set_pv_f(0xc, m.pv_f(0x44));
        m.set_pv_f(0x1fb4, m.pos[2]);
        m.state = 1;
        return;
    }
    if m.state != 1 { return; }
    if let Some(t) = target(w) {
        let mount = w.mobys[id].pv_i(0x1fb0);
        if let Some(tm) = usize::try_from(mount).ok().and_then(|i| w.ties.get(i)).copied() {
            let o = &mut w.mobys[t];
            o.pos = [tm[3][0], tm[3][1], tm[3][2]];
            o.rot[2] = crate::hero::physics::atan2(tm[0][0], tm[0][1]);
            w.rebuild(t, &*ctx.host);
            let hit = w.hit_message(t).is_some_and(|h| h.tmpl.flags & 0x10000 != 0);
            if hit {
                let base = w.class(0x0e9b).map_or(1.0, |c| c.scale);
                w.mobys[t].scale = base * 1.35;
                let limit = ctx.game.rng.rand_range(3, 5);
                crate::moby::bolts::spawn(w, t, 0.0, 0x1e, 0x32, 2, limit, ctx);
                let at = w.mobys[t].pos;
                crate::moby::blast::burst(ctx, at, &crate::moby::blast::Blast { size: 0.85, bursts: [0x19, 0x14, 0x14], puffs: 0x14, streaks: 0x2d });
                let tie = w.mobys[id].pv_i(0x10);
                if let Some(g) = usize::try_from(tie).ok().and_then(|g| w.tie_groups.get(g)).cloned() {
                    for i in g {
                        let i = i as usize;
                        if let Some(m) = w.ties.get_mut(i) { for r in m.iter_mut().take(3) { *r = [0.0; 4]; } }
                        w.hidden_ties.insert(i);
                        if !w.moved_ties.contains(&i) { w.moved_ties.push(i); }
                    }
                }
                ctx.host.play_sound(&w.mobys[t], 0, 0);
                w.delete(t);
                w.delete(id);
                return;
            }
            w.clear_hit(t);
        }
    }
    let m = &w.mobys[id];
    let sphere = m.pv_v(0);
    let cam = ctx.host.camera_pos();
    if 500.0 + m.pv_f(0xc) < crate::hero::physics::dist(cam, sphere) { return; }
    let m = &mut w.mobys[id];
    if 0.0 < m.pv_f(0x34) {
        let (amp, step, base) = (m.pv_f(0x30), m.pv_f(0x34) * DEG_S, m.pv_f(0x1fb4));
        super::props::bob(m, amp, step, base, 0x20, 0x24);
    }
    if 0.0 < m.pv_f(0x3c) {
        let (tilt, step) = (m.pv_f(0x38) * 0.017_453_294, m.pv_f(0x3c) * DEG_S);
        super::props::sway(m, tilt, step, 0.005_817_764, 0x28, 0x2c);
    }
    let turn = m.pv_f(0x1c) * DEG_S;
    m.rot[2] = crate::hero::physics::add_rot(m.rot[2], turn);
    m.set_pv_f(0x58, crate::hero::physics::add_rot(m.pv_f(0x58), turn));
    w.carry(id, &*ctx.host);
}
