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
