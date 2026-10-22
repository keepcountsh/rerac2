//! The gates (class 0xac7, L00 `0x3d0a60`): closed until the enemies of their group (`+0x68`[0]) are gone, then
//! open — turning 1° a tick for 135 ticks (sound 0 at the start, 1 at the end). The first time (the mission's flag
//! clear) with `[2]` set the opening is shown: the fly-by camera `[4]` armed (`0x42a4e0`, crate::cameras), the
//! mission's flag, the checkpoint at the cuboid `[3]`, and once the fly-by is over (`0x42a508`) Ratchet placed
//! there. With no group and `[5]` it starts open and closes behind Ratchet (state 4: he is put 2.5 back and walks
//! in).

use crate::moby::{Ctx, MobyWorld};

pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pvars.is_empty() { return; }
    let lv = ctx.game.level.max(0) as usize * 0x10;
    let flag = |g: &crate::game::GameState, mission: u8| g.missions.get(lv + mission as usize).copied().unwrap_or(0) & 0x80 != 0;
    let m = &w.mobys[id];
    let (group, mission) = (m.pv_i(0), m.mission);
    if m.state == 0 {
        let has_group = 0 <= group && (group as usize) < w.groups.len() && !w.groups[group as usize].is_empty();
        if !has_group {
            if m.pv_i(8) == 0 && m.pv_i(0x14) != 0 {
                let m = &mut w.mobys[id];
                m.set_pv_f(0x18, m.rot[1]);
                m.rot[1] = crate::hero::physics::add_rot(m.rot[1], -std::f32::consts::FRAC_PI_2);
                m.set_state(4, -1);
                return;
            }
            w.mobys[id].state = if m.pv_i(8) != 0 { 3 } else { 2 };
            return;
        }
        let alive = super::props::group_alive(w, group);
        if alive == 0 {
            if !flag(ctx.game, mission) {
                w.mobys[id].state = 1;
                let m = &w.mobys[id];
                if m.pv_i(8) == 0 { return; }
                ctx.game.cameras.arm(m.pv_i(0x10));
                if let Some(b) = ctx.game.missions.get_mut(lv + mission as usize) { *b |= 0x80; }
                if let Some((p, r)) = ctx.level.cuboid_place(m.pv_i(0xc)) { ctx.game.save_checkpoint(p, r); }
                return;
            }
            w.mobys[id].state = 3;
        } else if flag(ctx.game, mission) {
            w.mobys[id].state = 3;
        }
    }
    if w.mobys[id].state == 1 && w.mobys[id].pv_i(0x10) != 0 && ctx.game.cameras.flyby_over(ctx.game.scenes.fade) {
        // `0x42a508`: the fly-by is over (or black in its flight): Ratchet at the cuboid.
        if let Some((p, r)) = ctx.level.cuboid_place(w.mobys[id].pv_i(0xc)) { ctx.hero.teleport(p, r); }
        let m = &mut w.mobys[id];
        m.state = 3;
        m.once |= 4;
    }
    if w.mobys[id].state == 3 {
        let m = &mut w.mobys[id];
        if m.pv_f(4) == 0.0 { ctx.host.play_sound(&w.mobys[id], 0, 0); }
        let m = &mut w.mobys[id];
        m.rot[1] -= 0.017_453_294;
        let t = m.pv_f(4) + 1.0;
        m.set_pv_f(4, t);
        w.changed = true;
        if 135.0 < t {
            ctx.host.play_sound(&w.mobys[id], 1, 0);
            w.mobys[id].state = 2;
        }
    }
    if w.mobys[id].state != 4 { return; }
    if w.mobys[id].first() {
        ctx.host.play_sound(&w.mobys[id], 1, 0);
        let h = &mut *ctx.hero;
        let back = [h.pos[0] - h.rows[0][0] * 2.5, h.pos[1] - h.rows[0][1] * 2.5, h.pos[2] - h.rows[0][2] * 2.5];
        let (at, rot) = (h.pos, h.rot);
        h.teleport(back, rot);
        h.script_walk_to(rot[2], at, true);
    }
    let m = &mut w.mobys[id];
    let to = m.pv_f(0x18);
    let d = crate::hero::physics::angle_diff(to, m.rot[1], 0).clamp(-0.026_179_94, 0.026_179_94);
    m.rot[1] = crate::hero::physics::add_rot(m.rot[1], d);
    w.changed = true;
    if crate::hero::physics::rot_dist(m.rot[1], to) < 0.001 { m.state = 2; }
}
