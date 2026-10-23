//! R&C2's enemy framework (L00): the blocks an enemy's pvars hold and the shared routines that work on them. The
//! enemies address everything as offsets into their pvars, and the framework finds its blocks through the pointer
//! words at the pvars' start (kept relative here: [`Block`]): `[0]` the health block, `[0xc]` the target block,
//! `[0x10]` the reaction block, `[0x18]` the movement block. Only mobys with mode 0x20 take part.
//!
//! * **Senses** `0x32dcf0`: the movement block's turn (`+0x14` accel, `+0x18` brake, `+0x1c` most) and walk (`+0x20` /
//!   `+0x24` accel, `+0x28` most) limits.
//! * **Target** `0x32ad38` ([`find_target`]): Ratchet within a range across and a height (scored by the distance,
//!   more behind the facing, `0x32aa58`), or the nearest occupant of a trigger area (`0x32ab30`, the area list the
//!   hero's update fills, [`crate::game::GameState::area_has_hero`]). The target block: `+0x00` the point (Ratchet's
//!   aim point `0x189ef0`), `+0x10` the moby, `+0x14` found, `+0x1c` its kind.
//! * **Movement** `0x32cc30` / `0x32cd20` / `0x32ce98` ([`move_toward`], [`move_step`], [`hold`]): once a frame, the
//!   floor line `0x32d800`, the turn `0x31f230` and the walk speed `0x32cee0` toward the goal, the move and up to eight
//!   push-outs `0x32d2b8` (the sphere `0x32d978`, the fence path `0x32da88`), the ground and the flags `0x32d468`.
//!   The flags (`+0x94`): 1 a drop ahead, 2 sunk under the floor, 4 too steep, 0x10 fenced in, 0x20 in the air, 0x40
//!   on a dangerous moby, 0x80 at the goal, 0x100 out of its path.
//!
//! The movement block (`+0x110` on 0x099c): `+0x00` radius, `+0x04` its height, `+0x08` / `+0x0c` the step up / down,
//! `+0x10` the steepest slope, `+0x14..0x28` the limits, `+0x2c` the gravity, `+0x30` the turn left before it walks,
//! `+0x34` the stop distance, `+0x38` options (1 no ledge test, 2 the push-out's flags, 4 no gravity, 0x10 no turn, 0x40
//! the turn sequences, 0x80 the walk speed on the sequence, 0x100 no floor test, 0x200 the fall kill), `+0x3c` the
//! fence path (−1), `+0x48` / `+0x4c` aim joints, `+0x50` the floor's normal, `+0x60` the last push point, `+0x70` the
//! push-out, `+0x80` the stuck ticks, `+0x84` the pushing moby, `+0x88` the floor moby, `+0x8c` its kind, `+0x90` the
//! floor's height, `+0x94` / `+0x98` the flags now and before, `+0x9c` the state (1 fenced, 2 turning in place),
//! `+0xa0` the velocity, `+0xb0` its speed across, `+0xb4` the turn speed, `+0xbc` the frame it last moved, `+0xc0` /
//! `+0xc2` ground / air ticks, `+0xcc` the goal's distance, `+0xd0` / `+0xd4` the sequence speed's base and limit,
//! `+0xd8` the floor's surface, `+0xdc` / `+0xe0` / `+0xe2` / `+0xe4` the weave, `+0xe6` / `+0xe7` the turn sequences,
//! `+0xe8` / `+0xe9` the walk sequences, `+0xea` the one turned from.
//!
//! Not ported: the gravity frames (`$gp − 0x6000`), the joint aims (`0x32db38`, `+0x48`), the dangerous floor
//! (`0x322058`), the helpers and decoys as targets.

use super::{Ctx, Moby, MobyWorld};
use crate::hero::physics::*;

/// `RC_DEATH_LOG=1` (dev): the enemies' deaths on stdout — each hit that leaves one without health ([`take_damage`]),
/// each tick of a dying flight ([`flight`]), and one dump of any enemy still about 300 ticks after its health ran
/// out ([`MobyWorld::death_watch`]): a death that never ends.
pub fn death_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("RC_DEATH_LOG").is_ok_and(|v| v != "0"))
}

/// The death log's view of a moby: its states, place, collision, reaction block and animation.
pub fn death_line(w: &MobyWorld, id: usize, ctx: &Ctx) -> String {
    let m = &w.mobys[id];
    let r = react(m);
    let hp = if m.mode & AI != 0 { m.pv_f(health(m)) } else { f32::NAN };
    let a = ctx.host.anim(m).map_or("no anim".to_string(), |a| format!("seq {}/{} frame {}/{} t {:.2} speed {:.2} flags {:#x}", a.seq_a, a.seq_b, a.frame_a, a.frame_b, a.t, a.speed, a.flags));
    format!(
        "#{id} class {:#06x} state {} sub {} ticks {} pos ({:.2}, {:.2}, {:.2}) hp {hp:.2} mode {:#x} coll {} upd {} | react: hurt state {} sub {} opts {:#x} phase {} vel ({:.3}, {:.3}, {:.3}) land seq {} keys {} / {} g {:.4} / {:.4} | {a}",
        m.o_class, m.state, m.sub_state, m.state_ticks, m.pos[0], m.pos[1], m.pos[2], m.mode, m.has_collision, m.update.is_some(),
        m.pv_b(r + 0x10), m.pv_i(r + 0x48), m.pv_s(r + 0x38), m.pv_b(r + 0x1a), m.pv_f(r + 0x80), m.pv_f(r + 0x84), m.pv_f(r + 0x88),
        m.pv_b(r + 0x15), m.pv_f(r + 0x54), m.pv_f(r + 0x60), m.pv_f(r + 0x4c), m.pv_f(r + 0x50),
    )
}

/// The mode bit of the mobys the framework works on.
pub const AI: u16 = 0x20;

/// A block's base in the pvars (the pointer word at `ptr`, relative).
pub fn block(m: &Moby, ptr: usize) -> usize { m.pv_i(ptr).max(0) as usize }
pub fn health(m: &Moby) -> usize { block(m, 0) }
pub fn target(m: &Moby) -> usize { block(m, 0xc) }
pub fn react(m: &Moby) -> usize { block(m, 0x10) }
pub fn mv(m: &Moby) -> usize { block(m, 0x18) }

/// A moby word in the pvars: the index + 1 (0 none), as the game's pointers.
pub fn link(m: &Moby, off: usize) -> Option<usize> { usize::try_from(m.pv_i(off)).ok().filter(|&v| v != 0).map(|v| v - 1) }
pub fn set_link(m: &mut Moby, off: usize, id: Option<usize>) { m.set_pv_i(off, id.map_or(0, |i| i as i32 + 1)); }

/// `0x32dcf0(a, b, c, d, m)`: the limits (module docs).
pub fn senses(m: &mut Moby, a: f32, b: f32, c: f32, d: f32) {
    let o = mv(m);
    m.set_pv_f(o + 0x1c, d);
    m.set_pv_f(o + 0x24, a);
    m.set_pv_f(o + 0x28, b);
    m.set_pv_f(o + 0x18, c);
    m.set_pv_f(o + 0x20, a);
    m.set_pv_f(o + 0x14, c);
}

/// `0x32aa58(m, who)`: the distance across, more by the share of it the turn to face him is, plus 42 for Ratchet.
fn score(m: &Moby, at: V3, hero: bool) -> f32 {
    let d = dist2(m.pos, at);
    let turn = rot_dist(atan2(at[0] - m.pos[0], at[1] - m.pos[1]), m.rot[2]);
    d + turn * d + if hero { 42.0 } else { 0.0 }
}

/// `0x32ad38(range, height, m, area, from, any)`: Ratchet (or, with `area` ≥ 0, the area's nearest occupant) as the
/// target; returns whether one was found. `from` the point measured from (the moby's own place).
pub fn find_target(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, range: f32, height: f32, area: i32, from: Option<V3>, any: bool) -> bool {
    let m = &w.mobys[id];
    if m.mode & AI == 0 { return false; }
    let from = from.unwrap_or(m.pos);
    let t = target(m);
    let hero_pos = ctx.hero.pos;
    let aim = ctx.hero.aim_point;
    let m = &mut w.mobys[id];
    set_link(m, t + 0x10, ctx.hero_moby);
    m.set_pv_v(t, aim);
    m.set_pv_i(t + 0x1c, 0);
    m.set_pv_i(t + 0x14, 0);
    if !any && ctx.hero.state == 0x6f { return false; }
    let found = if area == -1 {
        dist2(from, hero_pos) < range && (from[2] - hero_pos[2]).abs() < height && score(m, hero_pos, true) < 9_999_999.0
    } else {
        ctx.game.area_has_hero(area)
    };
    if found {
        m.set_pv_i(t + 0x1c, 1);
        m.set_pv_i(t + 0x14, 1);
        set_link(m, t + 0x10, ctx.hero_moby);
    }
    found
}

/// `0x32d800(drop, m, block)`: the floor — a line from the step up above the feet to the step down (or `drop`)
/// below: its height `+0x90`, its moby `+0x88` and kind `+0x8c`, its surface `+0xd8` and normal `+0x50`; none: all
/// cleared, the surface −1.
fn floor(w: &mut MobyWorld, id: usize, ctx: &Ctx, drop: f32) {
    let m = &w.mobys[id];
    let o = mv(m);
    let a = [m.pos[0], m.pos[1], m.pos[2] + m.pv_f(o + 8) + 0.1];
    let b = [m.pos[0], m.pos[1], m.pos[2] - if drop == 0.0 { m.pv_f(o + 0xc) + 0.1 } else { drop }];
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(0), Some(id)));
    let m = &mut w.mobys[id];
    match hit {
        None => {
            m.set_pv_f(o + 0x90, 0.0);
            m.set_pv_i(o + 0x88, 0);
            m.set_pv_i(o + 0x8c, 0);
            m.set_pv_i(o + 0xd8, -1);
            m.set_pv_v(o + 0x50, [0.0; 3]);
        }
        Some(h) => {
            m.set_pv_f(o + 0x90, h.point[2]);
            set_link(m, o + 0x88, h.moby);
            m.set_pv_i(o + 0x8c, h.kind);
            m.set_pv_i(o + 0xd8, h.surface_id());
            m.set_pv_v(o + 0x50, set_len(h.normal, 1.0));
        }
    }
}

/// `0x32cee0(yaw, m, block, goal)`: the facing eased toward `yaw` (unless option 0x10); with the turn left within
/// `+0x30` (or no turn), the walk toward the goal at `min(√(2·accel·(d − stop)), most)` (away from the last pushing
/// point by at most `$gp − 0x4ba8` while stuck), the velocity's change at most `+0x20` speeding up, `+0x24` slowing;
/// then the vertical: in the air the fall (`+0x2c`), held off steep floors just under; on the floor the spring up to
/// it (`0x31f4b0`).
fn walk(w: &mut MobyWorld, id: usize, yaw: f32, goal: V3) {
    let m = &mut w.mobys[id];
    let o = mv(m);
    let opts = m.pv_i(o + 0x38) as u32;
    if opts & 0x10 == 0 {
        let (mut a, mut v) = (m.rot[2], m.pv_f(o + 0xb4));
        turn_ease(yaw, m.pv_f(o + 0x14), m.pv_f(o + 0x18), m.pv_f(o + 0x1c), &mut a, &mut v, 0);
        m.rot[2] = a;
        m.set_pv_f(o + 0xb4, v);
    }
    let left = rot_dist(m.rot[2], yaw);
    let mut want = [0.0f32; 3];
    let flags9c = m.pv_i(o + 0x9c);
    if opts & 0x10 == 0 && m.pv_f(o + 0x30) < left {
        if m.pv_f(o + 0xb4) * 30.0 < left - m.pv_f(o + 0x30) { m.set_pv_i(o + 0x9c, flags9c | 2); }
    } else {
        m.set_pv_i(o + 0x9c, flags9c & !2);
        let d = dist2(m.pos, goal);
        m.set_pv_f(o + 0xcc, d);
        if m.pv_f(o + 0x34) < d {
            let s = (2.0 * (d - m.pv_f(o + 0x34)) * m.pv_f(o + 0x24)).abs().sqrt().min(m.pv_f(o + 0x28));
            let mut dir = yaw;
            if m.pv_i(o + 0x80) != 0 {
                let p = m.pv_v(o + 0x60);
                let away = add_rot(atan2(p[0] - m.pos[0], p[1] - m.pos[1]), PI);
                // `$gp − 0x4ba8`: 1.9.
                let lim = 1.9;
                dir = add_rot(sub_rot(yaw, away).clamp(-lim, lim), away);
            }
            want = [dir.cos() * s, dir.sin() * s, 0.0];
        }
    }
    let vel = m.pv_v(o + 0xa0);
    let mut dv = [want[0] - vel[0], want[1] - vel[1], 0.0];
    let lim = if 0.0 < want[0] * vel[0] + want[1] * vel[1] + want[2] * vel[2] { m.pv_f(o + 0x20) } else { m.pv_f(o + 0x24) };
    if lim < len(dv) { dv = set_len(dv, lim); }
    let vel = add(vel, dv);
    m.set_pv_v(o + 0xa0, vel);
    m.set_pv_f(o + 0xb0, len2(vel));
    if opts & 4 != 0 { return; }
    let fl = m.pv_f(o + 0x90);
    let z = m.pos[2];
    let mut vz = m.pv_f(o + 0xa8);
    if fl < z {
        if m.pv_s(o + 0xc2) < 2 && z - fl < 0.15 {
            let n = m.pv_v(o + 0x50);
            let tilt = atan2(len2(n), n[2]);
            let into = -(vel[0] * n[0] + vel[1] * n[1] + vel[2] * n[2]);
            if 0.087_266_475 < tilt && into < vz { vz = into; }
        }
        vz -= m.pv_f(o + 0x2c);
        m.set_pv_f(o + 0xa8, vz);
        if m.pv_s(o + 0xc0) == 0 { let f = m.pv_i(o + 0x94); m.set_pv_i(o + 0x94, f | 0x20); }
        return;
    }
    if vz <= fl - z {
        if vz < 0.0 { vz = 0.0; }
    } else {
        vz = fl - z;
    }
    ease_speed(fl - z, 0.002_777_778, 0.002_777_778, 1.0, &mut vz);
    m.set_pv_f(o + 0xa8, vz);
}

/// `0x32d978(m, block)`: the sphere at the radius above the feet pushed out of the world and the mobys (flags 6 with
/// option 2, else 4); the pushing moby `+0x84`, the touch `+0x60`; a moby's primitive does not lift it.
fn push_out(w: &mut MobyWorld, id: usize, ctx: &Ctx) -> bool {
    let m = &w.mobys[id];
    let o = mv(m);
    let (r, h) = (m.pv_f(o), m.pv_f(o + 4));
    let up = [0.0, 0.0, h + r];
    let c = add(m.pos, up);
    let flags = if m.pv_i(o + 0x38) & 2 != 0 { 6 } else { 4 };
    let Some(hit) = ctx.coll.and_then(|cl| crate::collision::coll_sphere_m(cl, ctx.scene, c, r, crate::collision::QueryFlags(flags), Some(id))) else { return false };
    let m = &mut w.mobys[id];
    set_link(m, o + 0x84, hit.moby);
    m.set_pv_v(o + 0x60, hit.point);
    let mut pc = hit.pushed_centre.unwrap_or(c);
    if hit.moby.is_some() && hit.kind < 0 && c[2] < pc[2] { pc[2] = c[2]; }
    m.pos = sub(pc, up);
    true
}

/// `0x343678(r, path, p, &out)`: `p` kept `r` across from the path's walls (its segments with a length): a point
/// beyond a wall's line, or within `r` of it, moves to `r` from it. Returns whether it moved.
pub fn fence(points: &[[f32; 4]], r: f32, p: V3) -> Option<V3> {
    let mut q = p;
    let mut moved = false;
    for i in 0..points.len().saturating_sub(1) {
        if points[i][3] == 0.0 { continue; }
        let a = [points[i][0], points[i][1], 0.0];
        let b = [points[i + 1][0], points[i + 1][1], 0.0];
        let ap = [q[0] - a[0], q[1] - a[1], 0.0];
        let ab = sub(b, a);
        let n = set_len(ab, 1.0);
        // The side: the cross of the wall and the point (z only).
        let side = ap[0] * n[1] - ap[1] * n[0];
        if r < side.abs() { continue; }
        let along = ap[0] * n[0] + ap[1] * n[1];
        if len(ab) < along || along < 0.0 {
            if r <= len(ap) { continue; }
            let o = set_len(ap, r);
            q = [a[0] + o[0], a[1] + o[1], p[2]];
        } else {
            let foot = add(a, scale(n, along));
            let off = sub(ap, scale(n, along));
            let o = set_len(off, r);
            q = [foot[0] + o[0], foot[1] + o[1], p[2]];
        }
        moved = true;
    }
    moved.then_some(q)
}

/// `0x32da88(m, block)`: kept inside its fence path (`+0x3c`, unless out of it: `+0x9c` & 1).
fn keep_in(w: &mut MobyWorld, id: usize, ctx: &Ctx) -> bool {
    let m = &w.mobys[id];
    let o = mv(m);
    let path = m.pv_i(o + 0x3c);
    if path == -1 || m.pv_i(o + 0x9c) & 1 != 0 { return false; }
    let Some(points) = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)) else { return false };
    let r = m.pv_f(o);
    let Some(q) = fence(points, r, m.pos) else { return false };
    let m = &mut w.mobys[id];
    let d = set_len(sub(q, m.pos), r);
    m.set_pv_v(o + 0x60, d);
    m.pos = q;
    let f = m.pv_i(o + 0x94);
    m.set_pv_i(o + 0x94, f | 0x10);
    true
}

/// `0x32d2b8(m, block, from)`: the move by the velocity, up to eight rounds of push-outs; pushed, the velocity is
/// the move made (at most the speed it had), the stuck ticks count while it makes under half its top speed; kept on
/// a floor it is over.
fn move_body(w: &mut MobyWorld, id: usize, ctx: &Ctx, from: V3) {
    let m = &mut w.mobys[id];
    let o = mv(m);
    m.pos = add(m.pos, m.pv_v(o + 0xa0));
    let start = m.pos;
    set_link(m, o + 0x84, None);
    m.set_pv_v(o + 0x60, [0.0; 3]);
    let mut any = false;
    for _ in 0..8 {
        let a = push_out(w, id, ctx);
        let b = keep_in(w, id, ctx);
        any |= a | b;
        if !(a | b) { break; }
    }
    let m = &mut w.mobys[id];
    if !any {
        let mut t = m.pv_i(o + 0x80);
        crate::moby::countdown(&mut t);
        m.set_pv_i(o + 0x80, t);
    } else {
        let mut v = sub(m.pos, from);
        let l = len2(v);
        // `0x2ef188`: the move across held to the speed it had, the fall kept.
        if m.pv_f(o + 0xb0) < l {
            v = set_len2(v, m.pv_f(o + 0xb0));
        } else {
            m.set_pv_f(o + 0xb0, l);
        }
        m.set_pv_v(o + 0xa0, v);
        if m.pos[2] <= m.pv_f(o + 0x90) {
            if m.pos[2] < from[2] { m.pos[2] = from[2]; }
            if m.pv_f(o + 0xa8) < 0.0 { m.set_pv_f(o + 0xa8, 0.0); }
        }
        if m.pv_f(o + 0x28) * 0.5 <= m.pv_f(o + 0xb0) {
            let mut t = m.pv_i(o + 0x80);
            crate::moby::countdown(&mut t);
            m.set_pv_i(o + 0x80, t);
        } else {
            m.set_pv_i(o + 0x80, m.pv_i(o + 0x80) + 1);
        }
    }
    let m = &mut w.mobys[id];
    m.set_pv_v(o + 0x70, sub(m.pos, start));
}

/// `0x32d468(m, block, goal, from)`: the ground and the flags — sunk under the floor, a drop ahead (unless option 1,
/// in the first 5 air ticks: more than the step down), too steep a floor: back to `from`; on the floor the ground
/// ticks count (snapped to it), else the air ticks; at the goal; the walk sequence's speed (option 0x80) and the turn
/// sequences (option 0x40); the fall kill (option 0x200: under the react block's `+0x40` less 0.5, a 255 hit).
fn ground(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, goal: V3, from: V3) {
    floor(w, id, ctx, 0.0);
    let m = &mut w.mobys[id];
    let o = mv(m);
    let opts = m.pv_i(o + 0x38) as u32;
    let fl = m.pv_f(o + 0x90);
    let mut back = m.pos[2] < fl - m.pv_f(o + 8);
    let mut flags = m.pv_i(o + 0x94);
    if back { flags |= 2; }
    if opts & 1 == 0 && m.pv_s(o + 0xc2) < 5 && fl + m.pv_f(o + 0xc) < m.pos[2] {
        back = true;
        flags |= 1;
    }
    if m.pv_s(o + 0xc0) != 0 {
        let n = m.pv_v(o + 0x50);
        if m.pv_f(o + 0x10) < atan2(n[2], len2(n)) {
            back = true;
            flags |= 4;
        }
    }
    m.set_pv_i(o + 0x94, flags);
    if back {
        m.pos = from;
        floor(w, id, ctx, 0.0);
    }
    let m = &mut w.mobys[id];
    let fl = m.pv_f(o + 0x90);
    if m.pos[2] <= fl {
        if fl <= m.pos[2] - m.pv_f(o + 0xa8) { m.pos[2] = fl; }
        m.set_pv_s(o + 0xc2, 0);
        m.set_pv_s(o + 0xc0, m.pv_s(o + 0xc0).wrapping_add(1));
    } else {
        m.set_pv_s(o + 0xc0, 0);
        m.set_pv_s(o + 0xc2, m.pv_s(o + 0xc2).wrapping_add(1));
    }
    let d = dist2(m.pos, goal);
    m.set_pv_f(o + 0xcc, d);
    if d <= m.pv_f(o + 0x34) { let f = m.pv_i(o + 0x94); m.set_pv_i(o + 0x94, f | 0x80); }
    if m.pv_i(o + 0x9c) & 1 != 0 {
        let path = m.pv_i(o + 0x3c);
        let inside = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)).is_some_and(|pts| ctx.level.in_polygon(path, m.pos) && fence(pts, m.pv_f(o), m.pos).is_none());
        if inside { m.set_pv_i(o + 0x9c, m.pv_i(o + 0x9c) & !1); } else { let f = m.pv_i(o + 0x94); m.set_pv_i(o + 0x94, f | 0x100); }
    }
    let seq = super::classes::props::seq(w, id, ctx);
    let m = &mut w.mobys[id];
    if opts & 0x80 != 0 && (seq == m.pv_b(o + 0xe8) || seq == m.pv_b(o + 0xe9)) {
        let base = if m.pv_f(o + 0xd0) == 0.0 { m.pv_f(o + 0x28) } else { m.pv_f(o + 0xd0) };
        let lim = m.pv_f(o + 0xd4);
        let s = (m.pv_f(o + 0xb0) / base).min(lim).max(1.0 / lim);
        ctx.host.set_anim_speed(&w.mobys[id], s);
    }
    let m = &mut w.mobys[id];
    if opts & 0x40 != 0 {
        let walk = seq == m.pv_b(o + 0xe8) || seq == m.pv_b(o + 0xe9);
        if m.pv_i(o + 0x9c) & 2 != 0 && m.pv_f(o + 0xb4) != 0.0 && walk {
            m.set_pv_b(o + 0xea, seq);
            let s = if 0.0 < m.pv_f(o + 0xb4) { m.pv_b(o + 0xe6) } else { m.pv_b(o + 0xe7) };
            ctx.host.set_anim(&w.mobys[id], s, 0, 10);
        } else if (seq == m.pv_b(o + 0xe6) || seq == m.pv_b(o + 0xe7)) && m.pv_b(o + 0xea) != 0xff {
            let s = m.pv_b(o + 0xea);
            ctx.host.set_anim(&w.mobys[id], s, 0, 10);
        }
    }
    let m = &w.mobys[id];
    if opts & 0x200 != 0 && m.pv_f(o + 0x90) < m.pos[2] {
        let r = react(m);
        if m.pos[2] < m.pv_f(r + 0x40) - 0.5 {
            let t = super::hits::HitTemplate::new(255.0, None, 0x10000, [0.0; 4]);
            w.deliver_hit_any(id, &t, [m.pos[0], m.pos[1], m.pos[2], 1.0]);
        }
    }
}

/// `0x32cd20(yaw, m, goal)`: the frame's move (module docs); a second call the same frame returns the flags.
pub fn move_step(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, yaw: f32, goal: V3) -> i32 {
    let frame = ctx.game.frame;
    let m = &mut w.mobys[id];
    if m.mode & AI == 0 { return 8; }
    let o = mv(m);
    if frame == m.pv_i(o + 0xbc) { return m.pv_i(o + 0x94); }
    m.set_pv_i(o + 0x98, m.pv_i(o + 0x94));
    m.set_pv_i(o + 0x94, 0);
    let from = m.pos;
    let last = m.pv_i(o + 0xbc);
    if 1 < frame - last || last == 0 {
        floor(w, id, ctx, 0.0);
        let m = &mut w.mobys[id];
        m.set_pv_b(o + 0xea, 0xff);
        m.set_pv_f(o + 0xb4, 0.0);
        m.set_pv_i(o + 0x80, 0);
        m.set_pv_i(o + 0x9c, m.pv_i(o + 0x9c) & !2);
        m.set_pv_v(o + 0xa0, [0.0; 3]);
    }
    w.mobys[id].set_pv_i(o + 0xbc, frame);
    walk(w, id, yaw, goal);
    move_body(w, id, ctx, from);
    ground(w, id, ctx, goal, from);
    w.changed = true;
    w.mobys[id].pv_i(o + 0x94)
}

/// `0x32cc30(offset, m, goal)`: the move toward the goal, its yaw turned by `offset` and the weave (`+0xdc`, its
/// side swapped every `+0xe2..+0xe4` ticks).
pub fn move_toward(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, offset: f32, goal: V3) -> i32 {
    let m = &mut w.mobys[id];
    if m.mode & AI == 0 { return 8; }
    let o = mv(m);
    let mut yaw = add_rot(atan2(goal[0] - m.pos[0], goal[1] - m.pos[1]), offset);
    if m.pv_f(o + 0xdc) != 0.0 {
        yaw = add_rot(yaw, m.pv_f(o + 0xdc));
        let mut t = m.pv_s(o + 0xe0) as i32;
        let done = crate::moby::countdown(&mut t);
        m.set_pv_s(o + 0xe0, t as i16);
        if done {
            m.set_pv_f(o + 0xdc, -m.pv_f(o + 0xdc));
            let (a, b) = (m.pv_s(o + 0xe2) as i32, m.pv_s(o + 0xe4) as i32);
            let n = ctx.game.rng.rand_range(a, b);
            w.mobys[id].set_pv_s(o + 0xe0, n as i16);
        }
    }
    move_step(w, id, ctx, yaw, goal)
}

/// The enemies' scene rule (`0x1a8f00` = 2, after their checks): no collision, hidden and not a target
/// (`+0x98` = 0, mode `& 0xefff | 0x41`), and nothing more this tick. Outside a scene their update shows them again.
/// Returns whether a scene plays.
pub fn scene_hide(m: &mut super::Moby, ctx: &Ctx) -> bool {
    if ctx.game.scenes.mode != crate::scene::SCENE { return false; }
    m.has_collision = false;
    m.mode = (m.mode & 0xefff) | 0x41;
    true
}

/// `0x32ce98(m)`: the frame's move in place, facing as it is (it slows to a stop).
pub fn hold(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> i32 {
    let (yaw, pos) = (w.mobys[id].rot[2], w.mobys[id].pos);
    move_step(w, id, ctx, yaw, pos)
}

/// `0x32dd60(m, path)`: the walk along a path (the movement block's `+0xc4` the path + 1, `+0xc8` the point it heads
/// for, `+0xca` the last; a new path starts at its first point, `0x32de68`): each frame toward the point, the goal its
/// last; within the stop distance of it the next (or back), at the last 1.
pub fn follow_path(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, path: i32) -> bool {
    let Some(points) = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)).cloned() else { return false };
    if points.is_empty() { return false; }
    let m = &mut w.mobys[id];
    let o = mv(m);
    if m.pv_i(o + 0xc4) != path + 1 {
        m.set_pv_i(o + 0xc4, path + 1);
        m.set_pv_s(o + 0xca, points.len() as i16 - 1);
        m.set_pv_s(o + 0xc8, 0);
    }
    let at = |i: i16| { let p = points[(i.max(0) as usize).min(points.len() - 1)]; [p[0], p[1], p[2]] };
    let (i, last) = (m.pv_s(o + 0xc8), m.pv_s(o + 0xca));
    let p = at(i);
    let yaw = atan2(p[0] - m.pos[0], p[1] - m.pos[1]);
    move_step(w, id, ctx, yaw, at(last));
    let m = &mut w.mobys[id];
    if m.pv_f(o + 0x34) < dist2(m.pos, at(i)) { return false; }
    if i == last { return true; }
    m.set_pv_s(o + 0xc8, if i <= last { i + 1 } else { i - 1 });
    false
}

/// `0x31c278(up, r, step, slope, m, &pos, &target, flags)`: a step from `pos` toward `target` that keeps to walkable
/// ground. A target whose ground (`0x31ce98`, from `up` above it) is more than `step` off (flag 1: only higher) is not
/// taken; the line `up` above the feet `1.2·r` ahead (flags 0x24, flag 2 kept) meeting a face steeper than `slope`
/// slides the step along it; then (without flag 2) up to six spheres of `r` at `up + r` above the target, a touch by a
/// moby or above `slope` from the feet pushing the target out. The new ground is tested again, and `pos` takes the
/// target's x and y. Returns whether the step went as asked.
pub fn step_walk(ctx: &Ctx, id: usize, up: f32, r: f32, step: f32, slope: f32, pos: &mut V3, target: &mut V3, flags: u32) -> bool {
    use crate::collision::{coll_line_m, coll_sphere_m, QueryFlags};
    let mut ok = true;
    let off = |g: f32, p: V3| step < (g - p[2]).abs() && ((flags & 1 == 0 && g < p[2]) || p[2] < g);
    let g = ground_z(ctx, 0.5, [target[0], target[1], target[2] + up], 0x20);
    if off(g, *pos) {
        *target = *pos;
        ok = false;
    }
    let mut d = sub(*target, *pos);
    let probe = set_len(d, r * 1.2);
    let a = [pos[0], pos[1], pos[2] + up];
    let b = add(a, probe);
    if ok {
        let hit = ctx.coll.and_then(|c| coll_line_m(c, ctx.scene, a, b, QueryFlags(flags & 2 | 0x24), Some(id)));
        if let Some(h) = hit.filter(|h| 0 < h.kind) {
            let n = h.normal;
            if slope <= atan2(n[2], len2(n)) {
                let n = set_len([n[0], n[1], 0.0], 1.0);
                let into = -d[0] * n[0] - d[1] * n[1];
                if 0.0 < into {
                    d = add(d, scale(n, into));
                    ok = false;
                }
            }
            *target = add(*pos, d);
        }
    }
    if flags & 2 == 0 {
        let lift = up + r;
        for _ in 0..6 {
            let c = [target[0], target[1], target[2] + lift];
            let Some(h) = ctx.coll.and_then(|cl| coll_sphere_m(cl, ctx.scene, c, r, QueryFlags(0x24), Some(id))) else { break };
            let rise = atan2(dist2(*target, h.point), h.point[2] - target[2]);
            if h.moby.is_some() || slope < rise {
                let pc = h.pushed_centre.unwrap_or(c);
                *target = [pc[0], pc[1], pc[2] - lift];
                ok = false;
            }
        }
    }
    let g = ground_z(ctx, 0.5, [target[0], target[1], target[2] + up], 0x20);
    if off(g, *pos) {
        *target = *pos;
        ok = false;
    }
    pos[0] = target[0];
    pos[1] = target[1];
    ok
}

/// `0x323958(up, step, m, block)`: the wander about a home. The block: `+0x00` home, `+0x10` the radius, `+0x14` the
/// speed, `+0x18` the turn, `+0x1c` how far from home, `+0x20` how near Ratchet, `+0x24` the heading, (s16) `+0x28`
/// turning, `+0x2a` its ticks, `+0x2c` / `+0x2e` their range. A new heading (45°..150° to a side) is held for a while;
/// the step ([`step_walk`]) along the facing, on the ground; blocked or too far out, home is the heading for 30..90
/// ticks; near Ratchet the heading turns away from him (the nearer, the less).
pub fn wander(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, up: f32, step: f32, b: usize) {
    if w.mobys[id].pv_s(b + 0x28) == 0 {
        let t = ctx.game.rng.randf_signed(0.785_398_2, 2.617_994);
        let m = &mut w.mobys[id];
        m.set_pv_f(b + 0x24, m.pv_f(b + 0x24) + t);
        let (lo, hi) = (m.pv_s(b + 0x2c) as i32, m.pv_s(b + 0x2e) as i32);
        let n = ctx.game.rng.rand_range(lo, hi);
        let m = &mut w.mobys[id];
        m.set_pv_s(b + 0x2a, n as i16);
        m.set_pv_s(b + 0x28, 1);
    } else {
        let m = &mut w.mobys[id];
        let mut yaw = m.rot[2];
        approach_angle(m.pv_f(b + 0x24), m.pv_f(b + 0x18), &mut yaw, 0);
        m.rot[2] = yaw;
        let mut t = m.pv_s(b + 0x2a) as i32;
        let done = crate::moby::countdown(&mut t);
        m.set_pv_s(b + 0x2a, t as i16);
        if done { m.set_pv_s(b + 0x28, 0); }
    }
    let m = &w.mobys[id];
    let mut pos = m.pos;
    let s = m.pv_f(b + 0x14);
    let mut target = [pos[0] + m.rot[2].cos() * s, pos[1] + m.rot[2].sin() * s, pos[2]];
    let ok = step_walk(ctx, id, up, m.pv_f(b + 0x10), step, 0.523_598_8, &mut pos, &mut target, 0);
    pos[2] = ground_z(ctx, 0.5, pos, 0);
    let hero = ctx.hero.pos;
    let m = &mut w.mobys[id];
    m.pos = pos;
    let home = m.pv_v(b);
    if !ok || m.pv_f(b + 0x1c) < dist2(pos, home) {
        m.set_pv_f(b + 0x24, atan2(home[0] - pos[0], home[1] - pos[1]));
        let n = ctx.game.rng.rand_range(0x1e, 0x5a);
        let m = &mut w.mobys[id];
        m.set_pv_s(b + 0x2a, n as i16);
        m.set_pv_s(b + 0x28, 1);
    } else {
        let d = dist2(pos, hero);
        let near = m.pv_f(b + 0x20);
        if d < near {
            let away = atan2(pos[0] - hero[0], pos[1] - hero[1]);
            let h = m.pv_f(b + 0x24);
            m.set_pv_f(b + 0x24, add_rot(h, sub_rot(away, h) * (d / near)));
        }
    }
    w.changed = true;
}

// ---------------------------------------------------------------------------------------------------
// Hurt: the damage intake, the reaction, the knock-back flight, the attack sphere.
//
// The health block: `+0x00` health, `+0x04` (s16) the most, `+0x06` (s16) the ticks hits are not taken, `+0x17` the
// weapon that hit last (its item, `0x3228f8`), `+0x18` the moby that hit last, `+0x1c` (s16) the ticks since.
// The reaction block (`+0x60` on 0x099c): `+0x07` the reaction's sequence key, `+0x10` the state to hurt into, `+0x11`
// / `+0x12` / `+0x15` / `+0x16` / `+0x17` the fallback sequences (hurt, big hit, landing, death, spin), `+0x18` /
// `+0x19` the sparks, `+0x1a` the flight's phase (0 rising, 1 falling, 2 sliding, ≥ 3 none), `+0x1b` the water count,
// `+0x1c` / `+0x20` the knock's speeds across and up, `+0x24` / `+0x28` the gravities up and down, `+0x2c` the slide's
// brake, `+0x30` / `+0x34` the body's radius and centre height, `+0x38` options (1 dying, 2 spinning), `+0x3c` the
// bounce, `+0x40` the kill height, `+0x44` the frame of the last hurt, `+0x48` the sub-state hurt into (−1 none),
// `+0x4c` / `+0x50` the flight's gravities, `+0x54` / `+0x60` the sequence's keys to reach, `+0x58` / `+0x5c` the
// shares of the most health that hurt and that knock far, `+0x64` / `+0x68` the yaw to face and its speed, `+0x6c..
// +0x73` the fallback keys, `+0x70` (s16) the hitter, `+0x75` / `+0x76` / `+0x77` options, `+0x7c` the fence path,
// `+0x80` the velocity, `+0x90` the spin, `+0xa8` / `+0xac` the damage shares for a hit of its own class and for the
// flag.

/// A sequence's reaction record (sequence `+0x14`): its kind (0 hurt, 1 big hit, 3 death, 4 spin, 5 the flag's),
/// the knock's speeds across and up (per second), the gravities and the brake (per second²), the keys to reach.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reaction {
    pub kind: u8,
    pub brake: f32,
    pub across: f32,
    pub up: f32,
    pub g_up: f32,
    pub g_down: f32,
    pub key_a: f32,
    pub key_b: f32,
}

impl Reaction {
    /// The record's bytes: `u8 kind`, then floats at `+4` (the brake), `+8` / `+0xc` the speeds, `+0x10` / `+0x14` the
    /// gravities, `+0x18` / `+0x1c` the keys.
    pub fn parse(b: &[u8]) -> Option<Reaction> {
        let f = |o: usize| b.get(o..o + 4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]));
        Some(Reaction { kind: *b.first()?, brake: f(4)?, across: f(8)?, up: f(0xc)?, g_up: f(0x10)?, g_down: f(0x14)?, key_a: f(0x18)?, key_b: f(0x1c)? })
    }
}

/// `0x3289e0(m, kind, …)`: one of the class's sequences with a reaction of `kind`, picked at random: the sequence
/// and the reaction (its speeds per tick, its gravities and brake per tick²).
fn reaction(w: &MobyWorld, id: usize, ctx: &mut Ctx, kind: u8) -> Option<(u8, Reaction)> {
    let c = w.class(w.mobys[id].o_class)?;
    let all: Vec<(usize, Reaction)> = c.reactions.iter().enumerate().filter_map(|(s, r)| r.filter(|r| r.kind == kind).map(|r| (s, r))).collect();
    if all.is_empty() { return None; }
    let (s, r) = all[ctx.game.rng.randi(all.len() as i32) as usize];
    let k = 1.0 / 60.0;
    let kk = 1.0 / 3600.0;
    Some((s as u8, Reaction { kind, brake: r.brake * kk, across: r.across * k, up: r.up * k, g_up: r.g_up * kk, g_down: r.g_down * kk, key_a: r.key_a, key_b: r.key_b }))
}

/// `0x31dd48(m, hit)`: the damage a hit does — none for the same hitter again within 10 ticks (60 for the kind 9), a
/// new hitter's weapon kept; under 1 on a moby of at most 1 health, 1; the hit's flag 0x100000 into the reaction's
/// `+0x18`.
fn damage_of(m: &mut Moby, hit: Option<&super::hits::HitRecord>, wrench: bool) -> f32 {
    let h = health(m);
    let mut t = m.pv_s(h + 0x1c);
    if t != 0 {
        t += 1;
        m.set_pv_s(h + 0x1c, t);
        if 0x78 < t {
            m.set_pv_i(h + 0x18, 0);
            m.set_pv_s(h + 0x1c, 0);
        }
    }
    let Some(hit) = hit else { return 0.0 };
    let mut dmg = hit.tmpl.damage;
    if let Some(src) = hit.tmpl.source {
        if link(m, h + 0x18) == Some(src) {
            let ticks = m.pv_s(h + 0x1c);
            if hit.tmpl.kind & 0xff == 9 {
                if ticks < 0x3c { dmg = 0.0; }
            } else if ticks < 10 {
                dmg = 0.0;
            }
            m.set_pv_s(h + 0x1c, 1);
        } else {
            set_link(m, h + 0x18, Some(src));
            // `0x3228f8`: the hitter's item (the wrench: 10; the classes with their own kinds are not on Aranos).
            m.set_pv_b(h + 0x17, if wrench { 10 } else { 0xff });
            m.set_pv_s(h + 0x1c, 1);
        }
    }
    if m.pv_s(h + 4) as f32 <= 1.0 && 0.0 < dmg && dmg < 1.0 { dmg = 1.0; }
    let r = react(m);
    m.set_pv_b(r + 0x18, (hit.tmpl.flags & 0x100000 != 0) as u8);
    dmg
}

/// `0x3201e8(m, b)`: a colour flash's tick. The block: `+0` the ticks left, `+2` the phase, `+4..+6` the colour it
/// comes back to, `+7..+9` the flash's colour (0: that channel stays), `+0xc` / `+0xe` the ticks in and out. The
/// moby's ambient (`+0x3c`) eases to the flash's colour, then back; when it is over, the colour it came back to.
pub fn flash(m: &mut Moby, b: usize) {
    if m.pv_s(b) == 0 { return; }
    let mut t = m.pv_s(b) as i32;
    let over = crate::moby::countdown(&mut t);
    m.set_pv_s(b, t as i16);
    let base = [m.pv_b(b + 4), m.pv_b(b + 5), m.pv_b(b + 6)];
    let lit = [m.pv_b(b + 7), m.pv_b(b + 8), m.pv_b(b + 9)];
    if over {
        if m.pv_s(b + 2) != 0 {
            m.ambient = base;
            return;
        }
        m.set_pv_s(b + 2, 1);
        m.set_pv_s(b, m.pv_s(b + 0xe));
    }
    let back = m.pv_s(b + 2) != 0;
    let n = m.pv_s(b + if back { 0xe } else { 0xc }) as f32;
    let f = (n - m.pv_s(b) as f32) / n;
    let (from, to) = if back { (lit, base) } else { (base, lit) };
    m.ambient = std::array::from_fn(|k| if lit[k] == 0 { base[k] } else { (from[k] as f32 + (to[k] as i32 - from[k] as i32) as f32 * f) as u8 });
}

/// `0x320128(m, b)`: a colour flash starts (or keeps going in): from the moby's ambient now, or, when it was easing
/// back, in again from as far as it had come.
pub fn flash_start(m: &mut Moby, b: usize) {
    let t = m.pv_s(b);
    if t != 0 && m.pv_s(b + 2) == 0 { return; }
    m.set_pv_s(b + 2, 0);
    m.set_pv_s(b, m.pv_s(b + 0xc));
    if t == 0 {
        let [r, g, bl] = m.ambient;
        m.set_pv_b(b + 4, r);
        m.set_pv_b(b + 5, g);
        m.set_pv_b(b + 6, bl);
    } else {
        let k = (m.pv_s(b + 0xc) as f32 * (t as f32 / m.pv_s(b + 0xe) as f32)) as i32;
        m.set_pv_s(b, k.max(1) as i16);
    }
}

/// `0x328c28(range, cone, _, m, out, dir)`: the knock aimed at what it may fly into — the crates and the targets the
/// aims prefer (class type 0x16) about the point `range` ahead along `dir` across (the first 16 the sphere of `range`
/// there touches, `0x2e14b0`), those within `cone` of its heading: across it turns to their median heading, its
/// length kept. None: `dir` as it is. (Its third argument, 4, is not read.)
fn knock_aim(w: &MobyWorld, id: usize, ctx: &Ctx, range: f32, cone: f32, dir: V3) -> V3 {
    let Some(sc) = ctx.scene else { return dir };
    let m = &w.mobys[id];
    let ahead = set_len([dir[0], dir[1], 0.0], range);
    let heading = atan2(ahead[0], ahead[1]);
    let near = crate::collision::coll_sphere_mobys(sc, add(m.pos, ahead), range, crate::collision::QueryFlags(0), Some(id));
    let mut hs: Vec<f32> = near
        .into_iter()
        .take(16)
        .filter(|&o| super::aim::is_crate(w, o) || w.mobys.get(o).and_then(|x| w.class(x.o_class)).is_some_and(|c| c.kind == 0x16))
        .map(|o| { let p = w.mobys[o].pos; atan2(p[0] - m.pos[0], p[1] - m.pos[1]) })
        .filter(|&h| rot_dist(heading, h) < cone)
        .collect();
    if hs.is_empty() { return dir; }
    hs.sort_by(f32::total_cmp);
    let h = hs[hs.len() >> 1];
    let l = len2(dir);
    [h.cos() * l, h.sin() * l, dir[2]]
}

/// `0x3282b0(m, react, dir, kind)`: the reaction's start — its sequence (a class reaction of the kind, else the
/// block's fallback with the speeds scaled), dying (kind 4) or spinning (5) without collision and out of the target
/// list; the knock along `dir` (at least 1 across and 1 up) at the reaction's speeds, facing the hit; the flight's
/// gravities and keys. Returns whether it reacts (not for a graze).
fn react_start(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, dir: [f32; 4], kind: i32) -> bool {
    let r = react(&w.mobys[id]);
    let m = &w.mobys[id];
    let mut x = Reaction { kind: 0, brake: m.pv_f(r + 0x2c), across: m.pv_f(r + 0x1c), up: m.pv_f(r + 0x20), g_up: m.pv_f(r + 0x24), g_down: m.pv_f(r + 0x28), key_a: 0.0, key_b: 0.0 };
    let mut ok = true;
    let mut seq: Option<(u8, u8)> = None;
    let fallback = |m: &Moby, s: usize, keys: usize, k: f32, x: &mut Reaction| -> Option<(u8, u8)> {
        let q = m.pv_b(r + s);
        if q == 0xff { return None; }
        x.across *= k;
        x.up *= k;
        x.key_a = m.pv_b(r + keys) as i8 as f32;
        x.key_b = m.pv_b(r + keys + 1) as i8 as f32;
        Some((q, 3))
    };
    w.mobys[id].set_pv_b(r + 7, 0xff);
    match kind {
        2 | 3 => {
            let (first, second) = if kind == 2 { (0, 1) } else { (1, 0) };
            if let Some((s, rx)) = reaction(w, id, ctx, first).or_else(|| reaction(w, id, ctx, second)) {
                x = rx;
                seq = Some((s, 3));
            } else {
                let m = &w.mobys[id];
                seq = if kind == 2 { fallback(m, 0x11, 0x6c, 0.5, &mut x) } else { fallback(m, 0x12, 0x6e, 0.75, &mut x) };
                ok = seq.is_some();
            }
        }
        4 | 5 => {
            let m = &mut w.mobys[id];
            let opt = m.pv_i(r + 0x38) as u16 as u32;
            m.set_pv_s(r + 0x38, (opt | if kind == 4 { 1 } else { 2 }) as i16);
            m.has_collision = false;
            m.mode &= !super::mode::TARGET;
            if let Some((s, rx)) = reaction(w, id, ctx, if kind == 4 { 3 } else { 4 }) {
                x = rx;
                seq = Some((s, 3));
            } else {
                let m = &w.mobys[id];
                seq = if kind == 4 { fallback(m, 0x16, 0x72, 1.0, &mut x) } else { fallback(m, 0x17, 0x72, 1.5, &mut x) };
                ok = seq.is_some();
            }
            if kind == 5 {
                let m = &mut w.mobys[id];
                m.set_pv_f(r + 0x90, 0.104_719_76);
                m.set_pv_f(r + 0x94, 0.418_879);
            }
        }
        6 => match reaction(w, id, ctx, 5) {
            Some((s, rx)) => {
                x = rx;
                seq = Some((s, 3));
            }
            None => ok = false,
        },
        _ => {}
    }
    if let Some((s, blend)) = seq { ctx.host.set_anim(&w.mobys[id], s, 0, blend); }
    // The block's flash (its colour: red, `+7`).
    flash_start(&mut w.mobys[id], r);
    if kind == 1 || !ok { return false; }
    // Aimed at what it may fly into (`0x328c28`), at least 1 across (`0x2ef188`: the fall kept) and 1 up.
    let mut d = knock_aim(w, id, ctx, 5.0, 0.785_398_2, [dir[0], dir[1], dir[2]]);
    if len2(d) < 1.0 { d = set_len2(d, 1.0); }
    if d[2] < 1.0 { d[2] = 1.0; }
    let m = &mut w.mobys[id];
    m.set_pv_v(r + 0x80, [d[0] * x.across, d[1] * x.across, d[2] * x.up]);
    m.set_pv_f(r + 0x64, add_rot(atan2(d[0], d[1]), PI));
    if m.pv_b(r + 0x76) == 0 || 3 < kind {
        m.set_pv_f(r + 0x54, x.key_a);
        m.set_pv_f(r + 0x2c, x.brake);
        m.set_pv_f(r + 0x60, x.key_b);
        m.set_pv_f(r + 0x4c, x.g_up);
        m.set_pv_f(r + 0x50, x.g_down);
    } else {
        m.set_pv_f(r + 0x4c, 0.0);
        m.set_pv_f(r + 0x60, -1.0);
        m.set_pv_f(r + 0x50, 0.0);
        m.set_pv_f(r + 0x54, -1.0);
    }
    true
}

/// `0x327e38(m, react, flag)`: the frame's damage — the hit (flags 0x10000, while the health block's wait is over)
/// scaled for its own class's and for the flag, its damage taken off the health; the kind: 4 dead, 3 a big hit
/// (at least `+0x5c` of the most), 2 a hurt (at least `+0x58`, or a wrench hit), 1 a graze, 6 the flag's; a hurt
/// not long after the last (90 / 300 ticks) does at least the hurt's share. Unless dying or spinning (or already
/// hurting with `+0x75` / mid-flight), the reaction starts (the hit's direction, else straight back) and the moby
/// goes into its hurt state. Returns the kind.
pub fn take_damage(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, flag: bool) -> i32 {
    if !death_log() { return damage_tick(w, id, ctx, flag); }
    let hp = |w: &MobyWorld| if w.mobys[id].mode & AI != 0 { w.mobys[id].pv_f(health(&w.mobys[id])) } else { 1.0 };
    let before = hp(w);
    let kind = damage_tick(w, id, ctx, flag);
    let after = hp(w);
    if after <= 0.0 && after != before {
        println!("death hit tick {}: kind {kind} (hp {before:.2} → {after:.2}) {}", ctx.game.frame, death_line(w, id, ctx));
        let serial = w.mobys[id].serial;
        w.death_watch.entry(id).or_insert((serial, 0));
    }
    kind
}

fn damage_tick(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, flag: bool) -> i32 {
    let m = &w.mobys[id];
    if m.mode & AI == 0 { return 0; }
    let (h, r) = (health(m), react(m));
    flash(&mut w.mobys[id], r);
    let hit = w.hit_message(id).filter(|h| h.tmpl.flags & 0x10000 != 0);
    if w.hit_message(id).is_some() && hit.is_none() { w.clear_hit(id); }
    let m = &mut w.mobys[id];
    let mut wait = m.pv_s(h + 6) as i32;
    let waiting = !crate::moby::countdown(&mut wait);
    m.set_pv_s(h + 6, wait as i16);
    let mut hit = if waiting { None } else { hit };
    let mut graze = 0.0;
    if let Some(hr) = hit.as_mut() {
        graze = hr.tmpl.damage;
        let own = hr.tmpl.source.and_then(|s| w.mobys.get(s)).is_some_and(|s| s.o_class == w.mobys[id].o_class);
        let m = &w.mobys[id];
        if m.pv_f(r + 0xa8) != 0.0 && own { hr.tmpl.damage *= m.pv_f(r + 0xa8); }
        if flag { hr.tmpl.damage *= m.pv_f(r + 0xac); }
    }
    let wrench = hit.and_then(|h| h.tmpl.source).and_then(|s| w.mobys.get(s)).is_some_and(|s| s.o_class == crate::hero::items::WRENCH_CLASS);
    let mut dmg = damage_of(&mut w.mobys[id], hit.as_ref(), wrench);
    w.clear_hit(id);
    let m = &mut w.mobys[id];
    let hp = m.pv_f(h) - dmg;
    m.set_pv_f(h, hp);
    if dmg == 0.0 { return 0; }
    m.set_pv_s(r + 0x70, hit.and_then(|h| h.tmpl.source).map_or(-1, |s| s as i16));
    let most = m.pv_s(h + 4) as f32;
    // The wrench's stagger factor (`0x262544` of its row) is 0: the window is 300 ticks.
    let window = if most * m.pv_f(r + 0x58) <= 0.0 { 0x5a } else { 300 };
    if m.pv_b(r + 0x77) == 0 && window < (ctx.game.frame - m.pv_i(r + 0x44)).abs() {
        dmg = dmg.max(most * m.pv_f(r + 0x58));
    }
    let hp = m.pv_f(h);
    let mut kind = if hp <= 0.0 {
        4
    } else if most * m.pv_f(r + 0x5c) <= dmg {
        3
    } else if dmg < most * m.pv_f(r + 0x58) {
        if dmg != 0.0 && m.pv_b(r + 0x77) == 0 && wrench { 2 } else if graze != 0.0 { 1 } else { 0 }
    } else {
        2
    };
    if flag && (1..=3).contains(&kind) { kind = 6; }
    if kind == 0 { return 0; }
    if m.pv_s(r + 0x38) & 3 != 0 { return 0; }
    if m.sub_state as i32 == m.pv_i(r + 0x48) && kind < 4 {
        if m.pv_b(r + 0x75) != 0 { return 0; }
        if m.pv_b(r + 0x1a) < 2 { return 0; }
    }
    let dir = match hit {
        // The record's `+0x30`: its direction is longer than 0.0001.
        Some(hr) if 0.000_1 < len([hr.tmpl.dir[0], hr.tmpl.dir[1], hr.tmpl.dir[2]]) => hr.tmpl.dir,
        _ => {
            let f = m.rows[0];
            [-f[0], -f[1], 1.0, 5_627.987]
        }
    };
    if react_start(w, id, ctx, dir, kind) {
        let m = &mut w.mobys[id];
        m.set_pv_i(r + 0x44, ctx.game.frame);
        let st = m.pv_b(r + 0x10);
        let sub = m.pv_i(r + 0x48);
        m.set_state(st, sub);
        m.set_pv_b(r + 0x1a, 0);
    }
    kind
}

/// `0x31d180(r, damage, push, m, centre, flags, b7, b8, query)`: a hit from the moby: a sphere at `centre` hits every
/// moby it touches (not itself), along its facing (`push` times; the rows' with mode 0x100).
#[allow(clippy::too_many_arguments)]
pub fn attack_sphere(w: &mut MobyWorld, id: usize, ctx: &Ctx, r: f32, damage: f32, push: f32, centre: V3, flags: u32, b7: u8, b8: u8, query: u32) -> usize {
    let m = &w.mobys[id];
    let d = if m.mode & super::mode::KEEP_ROWS == 0 {
        [m.rot[2].cos(), m.rot[2].sin(), 1.0]
    } else {
        add(scale(m.rows[2], 0.25), m.rows[0])
    };
    let d = scale(d, push);
    let mut t = super::hits::HitTemplate::new(damage, Some(id), flags, [d[0], d[1], d[2], 5_627.987]);
    t.kind = (m.o_class as u16 as u32) << 16 | (b8 as u32) << 8 | b7 as u32;
    w.sphere_hit(ctx.scene, centre, r, query, Some(id), Some(&t))
}

/// `0x31d2c8(r, damage, push, m, joint, …)`: [`attack_sphere`] at a joint of the moby.
#[allow(clippy::too_many_arguments)]
pub fn attack_joint(w: &mut MobyWorld, id: usize, ctx: &Ctx, r: f32, damage: f32, push: f32, joint: usize, flags: u32, b7: u8, b8: u8, query: u32) -> usize {
    let Some(c) = ctx.host.joint(&w.mobys[id], joint) else { return 0 };
    attack_sphere(w, id, ctx, r, damage, push, c, flags, b7, b8, query)
}

/// `0x31ce20(up, p, flags)`: the ground's height under `p` (a line from `up` above to 0.01; 0 none).
pub fn ground_z(ctx: &Ctx, up: f32, p: V3, flags: u32) -> f32 {
    let a = [p[0], p[1], p[2] + up];
    let b = [p[0], p[1], 0.01];
    ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(flags | 2), None)).map_or(0.0, |o| o.point[2])
}

/// `0x328b80(m, react)` → `0x328e90`: the knock-back flight (while its phase is under 3); the flags: 1 dead (or out of
/// the world, or under the kill height), 2 over, 4 it hit something.
pub fn flight(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> u32 {
    let r = react(&w.mobys[id]);
    let res = if w.mobys[id].pv_b(r + 0x1a) < 3 { fly(w, id, ctx) } else { 0 };
    if res & 2 != 0 { w.mobys[id].set_pv_b(r + 0x18, 0); }
    if death_log() && w.mobys[id].pv_s(r + 0x38) & 1 != 0 { println!("dying flight tick {}: res {res:#x} {}", ctx.game.frame, death_line(w, id, ctx)); }
    res
}

fn fly(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> u32 {
    let mut res = 0u32;
    let m = &mut w.mobys[id];
    let r = react(m);
    let phase = m.pv_b(r + 0x1a);
    let g = if phase == 0 { m.pv_f(r + 0x4c) } else { m.pv_f(r + 0x50) };
    let mut v = m.pv_v(r + 0x80);
    v[2] -= g;
    let mut s = len2(v);
    approach(if 1 < phase { 0.0 } else { 0.016_666_668 }, m.pv_f(r + 0x2c), &mut s);
    // `0x2ef188(s, v, v)`: the speed across set, the fall kept.
    v = set_len2(v, s);
    m.set_pv_v(r + 0x80, v);
    let up = scale(m.rows[2], m.pv_f(r + 0x34));
    let centre = add(m.pos, up);
    let mut gz = ground_z(ctx, 0.5, centre, 0);
    // A liquid under it: its count slows it.
    if gz <= centre[2] && centre[2] + v[2] < gz && m.pv_b(r + 0x1b) == 0 && ctx.coll.is_some() {
        let a = [centre[0], centre[1], centre[2] + 0.5];
        let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, [centre[0], centre[1], 0.01], crate::collision::QueryFlags(2), None));
        if hit.is_some_and(|o| o.surface_id() == 0) {
            m.set_pv_b(r + 0x1b, 1);
            gz = ground_z(ctx, 0.5, m.pos, 0x20);
        }
    }
    let wet = m.pv_b(r + 0x1b);
    if wet != 0 {
        if wet < 4 { m.set_pv_v(r + 0x80, scale(m.pv_v(r + 0x80), 0.7)); }
        if wet < 7 {
            m.set_pv_b(r + 0x1b, wet + 1);
            m.set_pv_f(r + 0x50, m.pv_f(r + 0x50) * 0.75);
        }
    }
    if m.pos.iter().any(|&c| !(2.0..=1021.0).contains(&c)) { res = 1; }
    let phase = m.pv_b(r + 0x1a);
    let keyed = |m: &Moby, seq: u8| seq != m.pv_b(r + 0x15) && m.pv_f(r + 0x60) != -1.0 && m.pv_f(r + 0x54) != -1.0;
    let seq = super::classes::props::seq(w, id, ctx);
    let m = &w.mobys[id];
    let go = if phase == 0 { true } else if m.pv_f(r + 0x40) <= m.pos[2] { true } else { res = 1; false };
    if go && keyed(m, seq) {
        let now = ctx.host.frame_time(m);
        let speed = ctx.host.anim(m).map_or(1.0, |a| a.speed);
        match phase {
            1 => {
                let mut s = speed;
                if m.pv_f(r + 0x60) <= now {
                    s = 0.0;
                } else {
                    let g = m.pv_f(r + 0x50);
                    let solve = m.pv_b(r + 0x15) == 0xff || (gz != 0.0 && m.pv_f(r + 0x40) <= gz && m.pos[2] - gz <= 10.0);
                    if solve {
                        // The ticks to land: `0x31cd18(−g/2, vz + g/2, z − ground)`.
                        let (a, b, c) = (g * -0.5, m.pv_f(r + 0x88) - g * -0.5, m.pos[2] - gz);
                        let disc = b * b - 4.0 * a * c;
                        let mut t = 60.0;
                        if a != 0.0 && 0.0 <= disc {
                            let q = disc.sqrt();
                            let (t1, t2) = ((-b + q) / (2.0 * a), (-b - q) / (2.0 * a));
                            let tt = t1.max(t2);
                            if 0.0 < tt && tt < 60.0 { t = tt; }
                        }
                        let t = t.max(0.0001);
                        let want = (2.0 * (m.pv_f(r + 0x60) - now) / t).clamp(0.0, 2.0);
                        approach(want, 0.2, &mut s);
                    } else {
                        if seq != m.pv_b(r + 0x15) { ctx.host.set_anim(m, m.pv_b(r + 0x15), 0, 5); }
                        if 0.004_166_667 < m.pv_f(r + 0x50) { w.mobys[id].set_pv_f(r + 0x50, 0.004_166_667); }
                    }
                }
                ctx.host.set_anim_speed(&w.mobys[id], s);
            }
            0 => {
                let k = (m.pv_f(r + 0x88) / m.pv_f(r + 0x4c)).abs();
                if 2.0 < k {
                    let want = 2.0 * (m.pv_f(r + 0x54) - now) / k;
                    if 0.0 < want {
                        let mut s = speed;
                        approach(want, 0.2, &mut s);
                        ctx.host.set_anim_speed(m, s);
                    }
                }
            }
            2 => {
                ctx.host.set_anim_speed(m, 1.0);
                let a = add(m.pos, [0.0, 0.0, 0.2]);
                let b = add(m.pos, [0.0, 0.0, -1.0]);
                if ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(2), Some(id))).is_none() {
                    w.mobys[id].set_pv_b(r + 0x1a, 1);
                }
            }
            _ => {}
        }
    }
    let m = &mut w.mobys[id];
    let (mut a, mut va) = (m.rot[2], m.pv_f(r + 0x68));
    turn_ease(m.pv_f(r + 0x64), 0.013_962_634, 0.003_490_658_6, 3.14, &mut a, &mut va, 0);
    m.rot[2] = a;
    m.set_pv_f(r + 0x68, va);
    let v = m.pv_v(r + 0x80);
    m.pos = add(m.pos, v);
    let mut next = add(centre, v);
    let rad = m.pv_f(r + 0x30);
    let ahead = add(scale(v, 3.0), next);
    attack_sphere(w, id, ctx, rad, 1.0, 1.0, ahead, 0x4000000, 0, 1, 0);
    let m = &w.mobys[id];
    let q4 = crate::collision::QueryFlags(0x24);
    if rad < len(v) {
        if let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, centre, next, q4, Some(id))) { next = o.point; }
    }
    let mut line = false;
    let mut hit = ctx.coll.and_then(|c| crate::collision::coll_sphere_m(c, ctx.scene, next, rad, q4, Some(id)));
    if hit.is_none() {
        line = true;
        hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, next, m.pos, q4, Some(id)));
    }
    if let Some(o) = hit {
        res |= 4;
        let m = &mut w.mobys[id];
        m.pos = if line { o.point } else { sub(o.pushed_centre.unwrap_or(next), up) };
        let n = set_len(o.normal, 1.0);
        let v = m.pv_v(r + 0x80);
        let d = dot(v, n);
        if 0.707 <= n[2] {
            if d < 0.0 {
                if o.moby.is_none() || 0 < o.kind {
                    let a = add(m.pos, [0.0, 0.0, 0.2]);
                    let b = add(m.pos, [0.0, 0.0, -2.0]);
                    if ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(2), Some(id))).is_some() {
                        m.set_pv_b(r + 0x1a, 2);
                    }
                }
                let mut v = sub(v, scale(n, d));
                if m.pv_b(r + 0x1a) == 2 {
                    if o.moby.is_some() && o.kind < 0 { res |= (m.pv_s(r + 0x38) & 1) as u32; }
                    let mut s = len(v);
                    approach(0.0, m.pv_f(r + 0x50), &mut s);
                    v = set_len(v, s);
                }
                m.set_pv_v(r + 0x80, v);
            }
        } else if d < 0.0 {
            let k = d * -(m.pv_f(r + 0x3c) + 1.0);
            m.set_pv_v(r + 0x80, add(v, scale(n, k)));
        }
    }
    let m = &mut w.mobys[id];
    let path = m.pv_i(r + 0x7c);
    if path != -1 {
        if let Some(q) = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)).and_then(|pts| fence(pts, m.pv_f(r + 0x30), m.pos)) { m.pos = q; }
    }
    let phase = m.pv_b(r + 0x1a);
    if phase != 0 || 0.0 <= m.pv_f(r + 0x88) {
        if m.pv_s(r + 0x38) & 2 == 0 {
            let wrapped = super::classes::props::wrapped(w, id, ctx);
            let seq = super::classes::props::seq(w, id, ctx);
            let m = &w.mobys[id];
            if (wrapped && seq != m.pv_b(r + 0x15)) || (phase == 2 && seq == m.pv_b(r + 0x15)) {
                res |= if m.pv_s(r + 0x38) & 1 == 0 { 2 } else { 3 };
            }
        } else {
            if res & 4 != 0 && m.pv_f(r + 0x88) <= 0.0 { res |= 1; }
            let spin = m.pv_v(r + 0x90);
            m.rot = [add_rot(m.rot[0], spin[0]), add_rot(m.rot[1], spin[1]), add_rot(m.rot[2], spin[2])];
            let old_up = up;
            let rows = super::attach::rows_of(m.rot);
            m.rows = rows;
            let new_up = scale(rows[2], m.pv_f(r + 0x34));
            m.pos = add(m.pos, sub(old_up, new_up));
        }
    } else {
        m.set_pv_b(r + 0x1a, 1);
    }
    w.changed = true;
    res
}

/// `0x31c078(yaw, k, d, max, m, &v)`: the facing springs toward `yaw` (the push the difference over 9°, at most 1;
/// the speed at most `max` and the difference).
pub fn turn_to(m: &mut Moby, yaw: f32, k: f32, d: f32, max: f32, v_off: usize) {
    let e = sub_rot(yaw, m.rot[2]);
    let f = (e * 6.366_197).clamp(-1.0, 1.0);
    let mut v = m.pv_f(v_off);
    v += k * f - d * v;
    if max != 0.0 { v = v.clamp(-max, max); }
    v = v.clamp(-e.abs(), e.abs());
    m.set_pv_f(v_off, v);
    m.rot[2] = add_rot(m.rot[2], v);
}

/// `0x31ec88(k, d, max, &p, target, &v)`: `p` springs toward `target` along the line between them ([`spring_vel`]
/// on the distance), landing on it within `0.01·max`. Returns the distance left.
pub fn spring_to(p: &mut V3, target: V3, k: f32, d: f32, max: f32, v: &mut f32) -> f32 {
    let diff = sub(target, *p);
    let l = len(diff);
    spring_vel(l, k, d, max, v);
    *p = add(*p, set_len(diff, *v));
    let left = l - *v;
    if left.abs() < max * 0.01 {
        *p = target;
        *v = 0.0;
    }
    left
}
