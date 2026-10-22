//! The spawn points (class 0x325, L00 `0x39fbd8`): the checkpoints. Ratchet inside its cuboid (`+0x68`[0]) — or its
//! uid already counted in the save — makes it the checkpoint (`0x359b20`): its position (on the ground below, within
//! 2) and angles, what happened since the last one made permanent; its mission's flag set (`0x3132a8`), a message
//! (`0x327448(0x2dcc, 90)`, the "checkpoint" text, with `[9]`, once per spawn point in a row: not shown yet), and 900
//! ticks before it can take over again.
//!
//! **pvars:** `[0]` the cuboid, `[2]` the flags (8 off; 2 Clank only; 4 Ratchet only; 0x10 standing, walking or a
//! script; 0x20 not in 0x31), `[3]` the state (0 waiting, 1 the checkpoint, 2 left), `[4]` take over only once, `[6]`
//! the mission whose flag must be set (−1 none), `[8]` the rest timer, `[9]` the message.

use crate::moby::{countdown, Ctx, MobyWorld};

pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].update_dist < 0x20 { w.mobys[id].update_dist = 0x20; }
    let m = &w.mobys[id];
    let flags = m.pv_i(8);
    if flags & 8 != 0 { return; }
    if m.timer == 0 {
        let m = &mut w.mobys[id];
        m.timer = 1;
        m.set_pv_i(0x20, 0);
        m.set_pv_i(0x1c, 0);
        return;
    }
    let need = m.pv_i(0x18);
    if need != -1 && ctx.game.missions.get(ctx.game.level.max(0) as usize * 0x10 + need as usize).copied().unwrap_or(0) & 0x80 == 0 { return; }
    let h = &*ctx.hero;
    if flags & 2 != 0 && h.mode != 1 { return; }
    if flags & 4 != 0 && h.mode != 3 && h.mode != 0 { return; }
    if flags & 0x10 != 0 && 1 < h.group && h.group != 9 { return; }
    if flags & 0x20 != 0 && h.state == 0x31 { return; }
    if m.pv_i(0xc) == 2 {
        let mut t = m.pv_i(0x20);
        let done = countdown(&mut t);
        w.mobys[id].set_pv_i(0x20, t);
        if done { w.mobys[id].set_pv_i(0xc, 0); }
    }
    let m = &w.mobys[id];
    let inside = ctx.level.in_cuboid(m.pv_i(0), ctx.hero.pos);
    let counted = ctx.game.uid_count(m.uid) != 0 || ctx.game.uid_dead.get(m.uid.max(0) as usize).copied().unwrap_or(0) != 0;
    let st = m.pv_i(0xc);
    let take = if inside || counted { !(st != 0 && (m.pv_i(0x10) == 0 || m.pv_i(0x20) != 0)) } else {
        if st != 0 { w.mobys[id].set_pv_i(0xc, 2); }
        false
    };
    if take {
        let m = &mut w.mobys[id];
        m.set_pv_i(0xc, 1);
        let (mission, uid, rot) = (m.mission, m.uid, m.rot);
        let mut pos = m.pos;
        if let Some(c) = ctx.coll {
            let a = [pos[0], pos[1], pos[2] + 0.5];
            if let Some(o) = crate::collision::coll_line(c, a, [pos[0], pos[1], 0.01], crate::collision::QueryFlags(2)) {
                if (pos[2] - o.point[2]).abs() <= 2.0 { pos[2] = o.point[2]; }
            }
        }
        if mission != 0xff {
            let i = ctx.game.level.max(0) as usize * 0x10 + mission as usize;
            if let Some(b) = ctx.game.missions.get_mut(i) { *b |= 0x80; }
        }
        ctx.game.save_checkpoint(pos, rot);
        let u = uid.max(0) as usize;
        if let Some(p) = ctx.game.uid_pending.get_mut(u) { *p = 0; }
        if let Some(d) = ctx.game.uid_dead.get_mut(u) { *d = 0; }
        ctx.game.clear_uid(uid);
        if m.pv_i(0x24) != 0 && ctx.game.last_checkpoint != Some(id) {
            ctx.game.checkpoint_message = Some(0x5a);
            ctx.game.last_checkpoint = Some(id);
        }
        w.mobys[id].set_pv_i(0x20, 900);
    }
    let m = &mut w.mobys[id];
    if m.state == 0 && m.mission != 0xff {
        let i = ctx.game.level.max(0) as usize * 0x10 + m.mission as usize;
        if ctx.game.missions.get(i).copied().unwrap_or(0) & 0x80 != 0 { m.state = 1; }
    }
}
