//! Oozla's leaping swamp creature, **0x09a2** (L01 `0x3b6c80`; its set-up `0x3b6060`, checks `0x3b67d0`, tail
//! `0x3b6718`), on the framework ([`crate::moby::ai`]): health block `+0x20`, target `+0x230`, reaction `+0x60`
//! (a hurt goes to sub-state 5), movement `+0x110`, the wander block `+0x200` (home, radius 1, speed `$gp+0xd90`/60,
//! turn `$gp+0xda0`°/s, 5 from home, 6 from Ratchet).
//!
//! **Sub-states** (`+0x95`): 0 the set-up, once the swamp's water is up (`game.water_z`, kept at `+0x34c`): with a path
//! `+0x308` it swims (1, at the water), else it lurks on the bank (3); `+0x374` puts it 2 under the water hidden (5),
//! a cuboid `+0x380` sends it walking there (sub 3, state 6). 1 swimming: it wanders (state 0, its speed easing to
//! 2/60 for `+0x32c` ticks) and turns (state 1, sequence 4 to its end); seeing Ratchet (or alarmed by its group,
//! `+0x330`) it readies (3) and leaps (`0x3b6bb0`: to the one of its cuboids `+0x310..+0x31c` farthest from him, the
//! jump solved by `0x31e048` at most 0.133 a tick, gravity 0.005, sound 5, sequence 3 at 38 frames over the flight).
//! 2 the leap: once sequence 3 is 5 frames in, it flies, turning to the landing, which makes it solid, homes it there,
//! alarms it (`+0x330` = 240) and stands it (3, sequence 8). 3 on the bank: idle (0) and turning (1) about home; when
//! it sees him (or is alarmed), and it is its group's nearest or second nearest (the group's shared words, below) and
//! within 1 in height, it charges (2) unless two of its group already do, within 14; within 3.5 (0.5 in height)
//! facing him within 30° it bites (sub 4); 4 going home, 6 walking to `+0x380`'s cuboid until it sees him (7: to it,
//! deleted there unseen). 4 the bite (sequence 10: frames 18..25 hit with joints 0..4, radius 0.5, its damage
//! `+0x34`), then back off (sequence 8, or 0xb after hitting him). 5 the knock (the framework's flight): dead → 7;
//! over, solid again, into water (the floor's surface 3) → 6, else on the bank (4, or 1 inside its fence path).
//! 6 / 7 dying: the group alarmed, its bolts dropped, deleted.
//!
//! **The group's shared words** (gameplay section 21, [`MobyWorld::shared_i`]): `+0x33c` the charges under way,
//! `+0x368` the frame the next two were picked (every 10 frames, of its class in the group on the bank and not
//! busy: the nearest Ratchet's uid `+0x36c`, the second `+0x370`).
//!
//! Every tick (`0x3b67d0`): the target (range `+0x388`, height 0.5, area `+0x398`; one already had is kept while
//! within `+0x39c`), the damage (a hurt in the water swims on: sub 1), the group's pick; at the end (`0x3b6718`) its
//! timers, and on the bank its body tilts to the floor (40° at most).
//!
//! Not ported: its shadow (`0x313af8` = L00 `0x31da58`), the head's and tail's aims (`0x31a0e0`, `0x31cfa0`), the
//! splashes, ripples and their sounds (`0x3b5608`, `0x3b5888`, `0x3b64d0`), the companion moby 0x660 it makes at the
//! set-up (`0x3a7680`), the death's fade (`0x3eb1d0`, the class 0xc41 manager: it is deleted at once), the sphere
//! query `0x2d6c20` (a scan of the mobys within 3.5).

use crate::hero::physics::*;
use crate::moby::{ai, countdown, Ctx, MobyWorld};
use super::props::{set_seq, wrapped};

/// The class.
const CLASS: i16 = 0x09a2;
/// The movement block and the wander block.
const MV: usize = 0x110;
const WANDER: usize = 0x200;
/// The target block's moby and found words.
const T_MOBY: usize = 0x240;
const T_FOUND: usize = 0x244;
/// Ticks to the tuning constants (`$gp+0xd84..0xda0` on Oozla).
const SWIM_SPEED: f32 = 2.0 / 60.0;
const TURN: f32 = 120.0 * 0.000_290_888_25;

/// The sequence the moby blends from (`+0x42`).
pub(super) fn seq_a(w: &MobyWorld, id: usize, ctx: &Ctx) -> u8 { ctx.host.anim(&w.mobys[id]).map_or(0xff, |a| a.seq_a) }
pub(super) fn seq_b(w: &MobyWorld, id: usize, ctx: &Ctx) -> u8 { super::props::seq(w, id, ctx) }

/// `0x311358(m, s, frame, blend)` unless `s` plays.
pub(super) fn play(w: &MobyWorld, id: usize, ctx: &mut Ctx, s: u8, frame: u8, blend: i32) { set_seq(w, id, ctx, s, frame, blend.clamp(0, 255) as u8); }

/// A countdown on a pvar word (`0x2e3de8`).
pub(super) fn tick_down(w: &mut MobyWorld, id: usize, off: usize) -> bool {
    let m = &mut w.mobys[id];
    let mut t = m.pv_i(off);
    let r = countdown(&mut t);
    m.set_pv_i(off, t);
    r
}

/// The target moby (`+0x240`) and its place, when it has one.
pub(super) fn target(w: &MobyWorld, id: usize) -> Option<(usize, V3)> {
    ai::link(&w.mobys[id], T_MOBY).and_then(|t| w.mobys.get(t).map(|m| (t, m.pos)))
}

/// `0x3b6028`: it takes one of its group's charges (`+0x340` marks it holds one).
fn claim(w: &mut MobyWorld, id: usize) {
    let m = &w.mobys[id];
    if m.group != -1 && m.pv_i(0x340) == 0 {
        let p = m.pv_i(0x33c);
        w.set_shared_i(p, w.shared_i(p) + 1);
    }
    w.mobys[id].set_pv_i(0x340, 1);
}

/// `0x3b5fe0`: it gives its charge back.
fn release(w: &mut MobyWorld, id: usize) {
    let m = &w.mobys[id];
    if m.group != -1 && m.pv_i(0x340) != 0 {
        let p = m.pv_i(0x33c);
        let mut n = w.shared_i(p);
        countdown(&mut n);
        w.set_shared_i(p, n);
    }
    w.mobys[id].set_pv_i(0x340, 0);
}

/// `0x3b6698`: its group's creatures (alive) are alarmed (`+0x350`).
fn alarm_group(w: &mut MobyWorld, id: usize) {
    let g = w.mobys[id].group;
    let Some(members) = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned() else { return };
    for i in members {
        let m = &mut w.mobys[i];
        if m.state < 0x80 && m.o_class == CLASS { m.set_pv_i(0x350, 1); }
    }
}

/// `0x3b59e0`: the 0x0a26 mobys within 3.5 (16 at most) are made passable and stirred (`+0x9c`, `+0xbc`).
fn part_reeds(w: &mut MobyWorld, id: usize) -> Vec<usize> {
    let at = w.mobys[id].pos;
    let near: Vec<usize> = (0..w.mobys.len())
        .filter(|&i| i != id && w.mobys[i].state < 0x80 && w.mobys[i].o_class == 0x0a26 && dist(w.mobys[i].pos, at) < 3.5)
        .take(16)
        .collect();
    for &i in &near {
        let m = &mut w.mobys[i];
        m.coll_disable = 1;
        m.timer = 1;
    }
    near
}

/// `0x3b5b08`: standing on a 0x0a56 or 0x0a53 it keeps hold of it (`+0x384`, its moby): 0x0a56 it stays within 1 of
/// and above, 0x0a53 within its frame's ±2 × ±5 above it; 0.1 a tick at most.
fn cling(w: &mut MobyWorld, id: usize) {
    let m = &w.mobys[id];
    let held = m.pv_i(0x384);
    if held == -1 {
        if let Some(f) = ai::link(m, MV + 0x88).filter(|&f| matches!(w.mobys.get(f).map(|x| x.o_class), Some(0x0a56 | 0x0a53))) {
            w.mobys[id].set_pv_i(0x384, f as i32);
        }
        return;
    }
    let Some(h) = usize::try_from(held).ok().and_then(|h| w.mobys.get(h)) else { return };
    let pos = m.pos;
    let goal = match h.o_class {
        0x0a56 => {
            let mut g = if 1.0 < dist2(pos, h.pos) { add(h.pos, set_len(sub(pos, h.pos), 1.0)) } else { pos };
            if g[2] < h.pos[2] + 0.01 { g[2] = h.pos[2] + 0.011; }
            g
        }
        0x0a53 => {
            let rows = h.rows;
            let d = sub(pos, h.pos);
            let l = [dot(d, rows[0]).clamp(-2.0, 2.0), dot(d, rows[1]).clamp(-5.0, 5.0), dot(d, rows[2]).max(0.01)];
            add(h.pos, std::array::from_fn(|k| l[0] * rows[0][k] + l[1] * rows[1][k] + l[2] * rows[2][k]))
        }
        _ => pos,
    };
    let mut step = sub(goal, pos);
    if 0.1 < len(step) { step = set_len(step, 0.1); }
    w.mobys[id].pos = add(pos, step);
}

/// The moves' frame (`0x3b5de0` / `0x3b5ed8`): passing reeds (`+0x35d`) doubles its walk accel and most for the move;
/// after it the hold of a 0x0a56 / 0x0a53, and its velocity's z at most 1/12.
fn moving(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, mv: impl FnOnce(&mut MobyWorld, &mut Ctx) -> i32) -> i32 {
    let reeds = if w.mobys[id].pv_b(0x35d) != 0 { part_reeds(w, id) } else { Vec::new() };
    let (a, most) = (w.mobys[id].pv_f(MV + 0x20), w.mobys[id].pv_f(MV + 0x28));
    if !reeds.is_empty() {
        w.mobys[id].set_pv_f(MV + 0x28, most + most);
        w.mobys[id].set_pv_f(MV + 0x20, a + a);
    }
    let flags = mv(w, ctx);
    cling(w, id);
    let m = &mut w.mobys[id];
    let vz = m.pv_f(MV + 0xa8);
    if 0.083_333_336 < vz { m.set_pv_f(MV + 0xa8, (vz * 0.9).min(0.083_333_336)); }
    m.set_pv_f(MV + 0x20, a);
    m.set_pv_f(MV + 0x28, most);
    for i in reeds { w.mobys[i].coll_disable = 0; }
    flags
}

/// `0x319858(m, block, mv)`: the wander about home (the block `WANDER`): a new heading 45°..150° to a side held for
/// `+0x2c..+0x2e` ticks, turned to at the block's turn; each tick a move 2 ahead (`0x322450`), on the ground; off
/// home's radius `+0x1c` (or stopped) it heads home for 30..90 ticks, near Ratchet (`+0x20`) it turns away from him.
pub(super) fn wander(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, b: usize) -> i32 {
    let m = &w.mobys[id];
    if m.pv_s(b + 0x28) == 0 {
        let turn = ctx.game.rng.randf_signed(std::f32::consts::FRAC_PI_4, 2.617_994);
        let (lo, hi) = (m.pv_s(b + 0x2c) as i32, m.pv_s(b + 0x2e) as i32);
        let t = ctx.game.rng.rand_range(lo, hi);
        let m = &mut w.mobys[id];
        m.set_pv_f(b + 0x24, m.pv_f(b + 0x24) + turn);
        m.set_pv_s(b + 0x2a, t as i16);
        m.set_pv_s(b + 0x28, 1);
    } else {
        let m = &mut w.mobys[id];
        let mut yaw = m.rot[2];
        approach_angle(m.pv_f(b + 0x24), m.pv_f(b + 0x18), &mut yaw, 0);
        m.rot[2] = yaw;
        let mut t = m.pv_s(b + 0x2a) as i32;
        if countdown(&mut t) { m.set_pv_s(b + 0x28, 0); }
        m.set_pv_s(b + 0x2a, t as i16);
    }
    let m = &w.mobys[id];
    let (s, c) = m.rot[2].sin_cos();
    let ahead = [m.pos[0] + c + c, m.pos[1] + s + s, m.pos[2]];
    let yaw = atan2(ahead[0] - m.pos[0], ahead[1] - m.pos[1]);
    let flags = ai::move_step(w, id, ctx, yaw, ahead);
    let z = ai::ground_z(ctx, 0.5, w.mobys[id].pos, 0);
    let m = &mut w.mobys[id];
    m.pos[2] = z;
    let home = m.pv_v(b);
    if flags != 0 || m.pv_f(b + 0x1c) < dist2(m.pos, home) {
        let t = ctx.game.rng.rand_range(0x1e, 0x5a);
        let m = &mut w.mobys[id];
        m.set_pv_f(b + 0x24, atan2(home[0] - m.pos[0], home[1] - m.pos[1]));
        m.set_pv_s(b + 0x2a, t as i16);
        m.set_pv_s(b + 0x28, 1);
    } else {
        let hero = ctx.hero.pos;
        let (d, near) = (dist2(m.pos, hero), m.pv_f(b + 0x20));
        if d < near {
            let away = atan2(m.pos[0] - hero[0], m.pos[1] - hero[1]);
            m.set_pv_f(b + 0x24, add_rot(m.pv_f(b + 0x24), sub_rot(away, m.pv_f(b + 0x24)) * (d / near)));
        }
    }
    flags
}

/// `0x3b5de0`: the wander's frame.
fn roam(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) { moving(w, id, ctx, |w, ctx| wander(w, id, ctx, WANDER)); }

/// `0x3b5ed8(m, goal, &offset)`: the move toward `goal` at a yaw offset (`0x3224a8` = the framework's move toward).
fn go_to(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, goal: V3, offset: f32) -> i32 { moving(w, id, ctx, |w, ctx| ai::move_toward(w, id, ctx, offset, goal)) }

/// The weave of its charge: 50° side to side with the level's frame (`0x3164e0` wraps the phase).
fn weave(ctx: &Ctx) -> f32 {
    let ph = ctx.game.frame as f32 * 0.011_635_531;
    let wrapped = ((ph + std::f32::consts::PI) * 0.159_154_94).fract() * std::f32::consts::TAU - std::f32::consts::PI;
    wrapped.sin() * 0.872_664_7
}

/// The cuboid's centre (`+0x30` of its record).
pub(super) fn cuboid_at(ctx: &Ctx, i: i32) -> Option<V3> { ctx.level.cuboid_place(i).map(|(c, _)| c) }

/// `0x3b6060`: the set-up (module docs). False while the water is not up yet.
fn setup(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let frame = ctx.game.frame;
    let m = &w.mobys[id];
    if frame == 0 && m.group != -1 { let p = m.pv_i(0x33c); w.set_shared_i(p, 0); }
    let water = ctx.game.water_z;
    if water <= 0.0 && frame < 4 { return false; }
    let m = &w.mobys[id];
    if m.group != -1 {
        let (a, b, c) = (m.pv_i(0x368), m.pv_i(0x36c), m.pv_i(0x370));
        w.set_shared_i(a, 0);
        w.set_shared_i(b, -1);
        w.set_shared_i(c, -1);
    }
    let r32c = ctx.game.rng.rand_range(0x78, 0);
    let m = &mut w.mobys[id];
    if 0.0 < water { m.set_pv_f(0x34c, water); }
    m.set_pv_i(0x378, 0);
    let pos = m.pos;
    m.set_pv_v(0x2d0, pos);
    // The reaction block (`+0x60`): a hurt goes to sub-state 5.
    m.set_pv_b(0x48, 10);
    m.set_pv_b(0x4a, 8);
    m.set_pv_i(0xa8, 5);
    m.set_pv_f(0x9c, 0.4);
    m.set_pv_b(0x79, 1);
    m.set_pv_b(0x70, 0);
    m.set_pv_f(0x90, 0.5);
    m.set_pv_f(0x94, 0.5);
    if m.pv_f(0x37c) == 0.0 { m.set_pv_f(0x37c, 1.0); }
    m.set_pv_b(0x2a, 6);
    m.set_pv_b(0x28, 3);
    m.set_pv_f(0x30, 0.5);
    // The wander block.
    m.set_pv_f(WANDER + 0x10, 1.0);
    m.set_pv_f(WANDER + 0x1c, 5.0);
    m.set_pv_f(WANDER + 0x20, 6.0);
    m.set_pv_f(WANDER + 0x18, TURN);
    m.set_pv_f(0x214, SWIM_SPEED);
    m.set_pv_i(0x32c, r32c);
    // The movement block.
    m.set_pv_f(MV, 1.0);
    m.set_pv_f(MV + 4, 0.2);
    m.set_pv_f(MV + 0xc, 0.25);
    m.set_pv_f(MV + 0x30, std::f32::consts::FRAC_PI_6);
    m.set_pv_f(MV + 0x2c, 0.005);
    m.set_pv_f(MV + 8, 0.25);
    m.set_pv_f(MV + 0x10, std::f32::consts::FRAC_PI_6);
    // `0x3235c8`: the walk and turn limits (`$gp+0xd84..0xd98` × `+0x37c`).
    let k = m.pv_f(0x37c);
    let (walk_a, walk_most, turn_a, turn_most) = (6.0 * k / 3600.0, 3.0 * k / 60.0, 180.0 * k * 4.848_137_5e-6, 180.0 * k * 0.000_290_888_25);
    m.set_pv_f(MV + 0x1c, turn_most);
    m.set_pv_f(MV + 0x24, walk_a);
    m.set_pv_f(MV + 0x28, walk_most);
    m.set_pv_f(MV + 0x18, turn_a);
    m.set_pv_f(MV + 0x20, walk_a);
    m.set_pv_f(MV + 0x14, turn_a);
    let (path, alt, ambush, walk_to) = (m.pv_i(0x308), m.pv_i(0x364), m.pv_i(0x374), m.pv_i(0x380));
    let hidden = |w: &mut MobyWorld, ctx: &mut Ctx| {
        let m = &mut w.mobys[id];
        m.has_collision = false;
        play(w, id, ctx, 4, 0, 2);
    };
    if path == -1 {
        if ambush == 0 {
            let z = ai::ground_z(ctx, 0.5, pos, 0);
            let m = &mut w.mobys[id];
            m.pos[2] = z;
            let fence = if alt == -1 { m.pv_i(0x30c) } else { alt };
            m.set_pv_i(MV + 0x3c, fence);
            m.timer = 0;
            m.set_pv_i(0x398, m.pv_i(0x394));
            m.set_pv_i(0x39c, m.pv_i(0x388));
            m.set_state(1, 3);
            play(w, id, ctx, 0, 0, 2);
        } else {
            let m = &mut w.mobys[id];
            m.set_state(5, 1);
            m.pos[2] = m.pv_f(0x34c) - 2.0;
            hidden(w, ctx);
        }
    } else {
        let m = &mut w.mobys[id];
        m.set_pv_i(MV + 0x3c, path);
        m.set_state(1, 1);
        m.pos[2] = m.pv_f(0x34c);
        if ambush != 0 {
            m.set_state(5, 1);
            m.pos[2] = m.pv_f(0x34c) - 2.0;
        }
        hidden(w, ctx);
    }
    if walk_to != -1 {
        play(w, id, ctx, 0x14, 0, 2);
        let m = &mut w.mobys[id];
        m.set_pv_i(MV + 0x3c, m.pv_i(0x30c));
        m.set_pv_i(0x398, m.pv_i(0x390));
        m.set_pv_i(0x39c, m.pv_i(0x388));
        m.set_state(6, 3);
    }
    let m = &mut w.mobys[id];
    let pos = m.pos;
    m.set_pv_v(WANDER, pos);
    m.set_pv_i(0x39c, 0);
    m.set_pv_i(0x398, m.pv_i(0x390));
    true
}

/// `0x3b67d0`: the target, the damage, the group's pick (module docs).
fn checks(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let t = 0x230;
    let kept: Vec<u8> = m.pvars[t..t + 0x20].to_vec();
    let had = m.pv_i(T_FOUND) == 1;
    let (range, area, keep) = (m.pv_f(0x388), m.pv_i(0x398), m.pv_f(0x39c));
    let found = ai::find_target(w, id, ctx, range, 0.5, area, None, false);
    if !found && had {
        let m = &w.mobys[id];
        let old = usize::try_from(i32::from_le_bytes([kept[0x10], kept[0x11], kept[0x12], kept[0x13]])).ok().filter(|&v| v != 0).map(|v| v - 1);
        if let Some(o) = old.filter(|&o| w.mobys.get(o).is_some_and(|x| x.state != 0xfe && x.state != 0xfd && dist2(m.pos, x.pos) < keep)) {
            let point = if Some(o) == ctx.hero_moby { ctx.hero.aim_point } else {
                let mut p = w.mobys[o].pos;
                if let Some((_, h)) = crate::moby::aim::health(w, o) { p[2] += h; }
                p
            };
            let m = &mut w.mobys[id];
            m.pvars[t..t + 0x20].copy_from_slice(&kept);
            m.set_pv_v(t, point);
            m.set_pv_i(T_FOUND, 1);
        }
    }
    let kind = ai::take_damage(w, id, ctx, false);
    if kind != 0 {
        let m = &mut w.mobys[id];
        let fence = if 0.0 < m.pv_f(0x20) { m.pv_i(MV + 0x3c) } else { -1 };
        m.set_pv_i(0xdc, fence);
        release(w, id);
        alarm_group(w, id);
        if w.mobys[id].sub_state < 3 {
            w.mobys[id].set_pv_b(0x7a, 2);
            play(w, id, ctx, 4, 0, 0xf);
            let m = &mut w.mobys[id];
            m.set_pv_i(0x320, 0);
            m.set_state(1, 1);
        }
    }
    if w.mobys[id].drawn != 0 {
        if let Some((_, tp)) = target(w, id) {
            let m = &mut w.mobys[id];
            m.set_pv_f(0x2b8, sub_rot(atan2(tp[0] - m.pos[0], tp[1] - m.pos[1]), m.rot[2]));
        }
    }
    // The group's pick, every 10 frames (or when it has none).
    let m = &w.mobys[id];
    let g = m.group;
    let frame = ctx.game.frame;
    let (at, first, second) = (m.pv_i(0x368), m.pv_i(0x36c), m.pv_i(0x370));
    if g != -1 && w.shared_i(at) != frame && (w.shared_i(first) == -1 || w.shared_i(at) < frame - 10) {
        w.set_shared_i(first, -1);
        w.set_shared_i(second, -1);
        let mut best = 100_000.0;
        let members = usize::try_from(g).ok().and_then(|g| w.groups.get(g)).cloned().unwrap_or_default();
        let hero = ctx.hero.pos;
        for i in members {
            let o = &w.mobys[i];
            if o.o_class != CLASS || o.sub_state != 3 || 3 <= o.state || o.state >= 0x80 { continue; }
            let d = dist2(o.pos, hero);
            if d < best {
                let uid = o.uid as i32;
                w.set_shared_i(second, w.shared_i(first));
                w.set_shared_i(first, uid);
                best = d;
            }
        }
        w.set_shared_i(at, frame);
    }
}

/// `0x3b6718`: the timers at the end of the tick.
fn tail(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pv_i(0x350) != 0 {
        w.mobys[id].set_pv_i(0x350, 0);
        if w.mobys[id].pv_i(0x330) < 200 {
            let t = ctx.game.rng.rand_range(300, 360);
            w.mobys[id].set_pv_i(0x330, t);
        }
    }
    if w.mobys[id].pv_i(T_FOUND) != 1 { tick_down(w, id, 0x330); }
    let m = &mut w.mobys[id];
    if (m.state_ticks as u16) < 0xfffe { m.state_ticks = m.state_ticks.wrapping_add(1); }
    tick_down(w, id, 0x32c);
    tick_down(w, id, 0x334);
}

/// Into the bite (sub-state 4): stopped, sequence 10 from `$gp+0xd80`.
fn bite_start(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, blend: (i32, i32)) {
    let m = &mut w.mobys[id];
    m.set_pv_v(MV + 0xa0, [0.0; 3]);
    m.set_pv_f(MV + 0xac, 0.0);
    m.set_pv_b(0x35c, 0);
    if seq_b(w, id, ctx) != 10 {
        let b = ctx.game.rng.rand_range(blend.0, blend.1);
        play(w, id, ctx, 10, 10, b);
    }
    w.mobys[id].set_state(1, 4);
}

/// A random idle (sequence 0 or 1) unless one plays.
fn idle(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, base: u8) {
    let now = seq_b(w, id, ctx);
    let a = (ctx.game.rng.rand() & 1) as u8 + base;
    if now != a {
        let s = (ctx.game.rng.rand() & 1) as u8 + base;
        let b = ctx.game.rng.rand_range(0x1e, 0x28);
        play(w, id, ctx, s, 0, b);
    }
}

/// The walk sequence (8, or 0x13 for an even uid) with a blend in `lo..=hi`.
fn walk_seq(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, lo: i32, hi: i32) {
    let s = if w.mobys[id].uid & 1 != 0 { 8 } else { 0x13 };
    let b = ctx.game.rng.rand_range(lo, hi);
    play(w, id, ctx, s, 0, b);
}

/// The creature dies (sub-states 6 / 7): its group alarmed, its bolts, gone.
fn die(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    alarm_group(w, id);
    crate::moby::bolts::drop(w, id, 0, 3, ctx);
    release(w, id);
    w.delete(id);
}

/// 0x09a2 (module docs).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].sub_state != 0 { checks(w, id, ctx); }
    match w.mobys[id].sub_state {
        0 => { setup(w, id, ctx); }
        1 => swim(w, id, ctx),
        2 => leap(w, id, ctx),
        3 => bank(w, id, ctx),
        4 => bite(w, id, ctx),
        5 => {
            let res = ai::flight(w, id, ctx);
            if res & 1 != 0 {
                w.mobys[id].set_state(0, 7);
            } else if res & 2 != 0 {
                let solid = w.class(CLASS).is_some_and(|c| c.has_collision);
                w.mobys[id].has_collision = solid;
                release(w, id);
                if w.mobys[id].pv_i(MV + 0xd8) & 0x1f == 3 {
                    play(w, id, ctx, 4, 0, 0x6e);
                    w.mobys[id].set_state(0, 6);
                } else {
                    let m = &mut w.mobys[id];
                    m.set_pv_i(0x398, m.pv_i(0x394));
                    m.set_pv_i(0x39c, m.pv_i(0x388));
                    m.set_pv_i(0x384, -1);
                    walk_seq(w, id, ctx, 0x12, 0x18);
                    let home = inside_fence(w, id, ctx);
                    w.mobys[id].set_state(if home { 1 } else { 4 }, 3);
                }
            }
        }
        6 | 7 => {
            die(w, id, ctx);
            return;
        }
        _ => {}
    }
    if w.mobys[id].state >= 0x80 { return; }
    if matches!(w.mobys[id].sub_state, 3 | 4) { tilt(w, id, ctx); }
    tail(w, id, ctx);
}

/// `0x3236c0`: inside its fence path's polygon and clear of its walls (else the movement block's `+0x9c` bit 1).
fn inside_fence(w: &mut MobyWorld, id: usize, ctx: &Ctx) -> bool {
    let m = &w.mobys[id];
    let path = m.pv_i(MV + 0x3c);
    if path == -1 { return false; }
    if ctx.level.in_polygon(path, m.pos) {
        let pts: Vec<[f32; 4]> = usize::try_from(path).ok().and_then(|p| ctx.level.paths.get(p)).map(|v| v.iter().map(|p| [p[0], p[1], p[2], 0.0]).collect()).unwrap_or_default();
        if ai::fence(&pts, m.pv_f(MV), m.pos).is_none() { return true; }
    }
    let m = &mut w.mobys[id];
    m.set_pv_i(MV + 0x9c, m.pv_i(MV + 0x9c) | 1);
    false
}

/// Sub-state 1: swimming.
fn swim(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        1 => {
            if wrapped(w, id, ctx) {
                let t = ctx.game.rng.rand_range(0x78, 400);
                w.mobys[id].set_pv_i(0x32c, t);
                if seq_b(w, id, ctx) != 4 {
                    let b = ctx.game.rng.rand_range(0x14, 0x1e);
                    play(w, id, ctx, 4, 0, b);
                }
                w.mobys[id].set_state(0, -1);
            }
        }
        0 => {
            let m = &mut w.mobys[id];
            let goal = if m.pv_i(0x32c) < 0x15 { 0.0 } else { SWIM_SPEED };
            let mut v = m.pv_f(0x214);
            approach(goal, 0.001_111_111_2, &mut v);
            m.set_pv_f(0x214, v);
            if tick_down(w, id, 0x32c) {
                if seq_b(w, id, ctx) != 4 {
                    let b = ctx.game.rng.rand_range(0x12, 0x18);
                    play(w, id, ctx, 4, 0, b);
                }
                w.mobys[id].set_state(1, -1);
            }
            if w.mobys[id].pv_i(0x308) != -1 { roam(w, id, ctx); }
        }
        2 => {
            let m = &mut w.mobys[id];
            let at = m.pv_v(0x2d0);
            m.set_pv_v(0x2e0, at);
        }
        3 => {
            if tick_down(w, id, 0x360) { jump(w, id, ctx); }
            return;
        }
        // The hidden one (5) only waits for the check below.
        _ => {}
    }
    let m = &mut w.mobys[id];
    if m.state != 3 && m.pv_i(0x310) != -1 && m.sub_state == 1 {
        m.set_pv_i(0x398, m.pv_i(0x390));
        m.set_pv_i(0x39c, 0);
        if m.pv_i(T_FOUND) == 1 || m.pv_i(0x330) != 0 {
            m.pos[2] = m.pv_f(0x34c);
            m.set_state(3, 1);
        }
    }
}

/// `0x3b6bb0` and the leap's start: to the farthest of its cuboids from Ratchet, on the ground there.
fn jump(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let hero = ctx.hero.pos;
    let mut best: Option<(V3, f32)> = None;
    for k in 0..4 {
        if let Some(c) = cuboid_at(ctx, m.pv_i(0x310 + 4 * k)) {
            let d = dist2(c, hero);
            if best.is_none_or(|b| b.1 < d) { best = Some((c, d)); }
        }
    }
    let Some((mut to, _)) = best else { return };
    to[2] = ai::ground_z(ctx, 10.0, to, 0);
    let from = m.pos;
    let d = sub(to, from);
    let speed = (len(d) * 0.033_333_335).min(0.133_333_34);
    let across = set_len(d, speed);
    // `0x31e048(speed, g, from, to, &t)`.
    let t = len2(d) / speed;
    let vz = if t != 0.0 { -((from[2] - to[2]) + -0.005 * t * t * 0.5) / t } else { 0.0 };
    let m = &mut w.mobys[id];
    m.set_pv_v(0x2f0, [across[0], across[1], vz]);
    m.set_pv_v(0x2e0, to);
    ctx.host.play_sound(&w.mobys[id], 5, 0);
    play(w, id, ctx, 3, 0, 10);
    if 0.0 < t { ctx.host.set_anim_speed(&w.mobys[id], 38.0 / t); }
    w.mobys[id].set_state(1, 2);
}

/// Sub-state 2: the leap's flight.
fn leap(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if seq_a(w, id, ctx) != 3 || ctx.host.frame_time(&w.mobys[id]) <= 5.0 { return; }
    let m = &mut w.mobys[id];
    m.pos = add(m.pos, m.pv_v(0x2f0));
    let to = m.pv_v(0x2e0);
    let yaw = atan2(to[0] - m.pos[0], to[1] - m.pos[1]);
    // `0x311f48(yaw, accel, brake, most, m, &v)`: the facing springs toward the landing.
    let (a, b, most) = (m.pv_f(MV + 0x14) * 10.0, m.pv_f(MV + 0x24) * 10.0, m.pv_f(MV + 0x1c) * 10.0);
    let e = sub_rot(yaw, m.rot[2]);
    let mut v = m.pv_f(0x1c4);
    v += a * (e * 6.366_197).clamp(-1.0, 1.0) - b * v;
    if most != 0.0 { v = v.clamp(-most, most); }
    v = v.clamp(-e.abs(), e.abs());
    m.set_pv_f(0x1c4, v);
    m.rot[2] = add_rot(m.rot[2], v);
    let vz = m.pv_f(0x2f8) - 0.005;
    m.set_pv_f(0x2f8, vz);
    if 0.0 <= vz || -vz <= (m.pos[2] - to[2]).abs() { return; }
    // Landed.
    let solid = w.class(CLASS).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    m.has_collision = solid;
    m.set_pv_i(MV + 0x3c, m.pv_i(0x30c));
    let pos = m.pos;
    m.set_pv_v(WANDER, pos);
    m.timer = 1;
    m.set_pv_i(0x330, 0xf0);
    inside_fence(w, id, ctx);
    if seq_b(w, id, ctx) != 8 { play(w, id, ctx, 8, 0, 10); }
    let m = &mut w.mobys[id];
    m.set_pv_i(0x398, m.pv_i(0x394));
    m.set_pv_i(0x39c, m.pv_i(0x388));
    m.set_state(1, 3);
}

/// Sub-state 3: on the bank.
fn bank(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let state = w.mobys[id].state;
    match state {
        0 => {
            let m = &mut w.mobys[id];
            let goal = if m.pv_i(0x32c) < 0x15 { 0.0 } else { SWIM_SPEED };
            let mut v = m.pv_f(0x214);
            approach(goal, 0.001_111_111_2, &mut v);
            m.set_pv_f(0x214, v);
            if tick_down(w, id, 0x32c) {
                if w.mobys[id].timer == 0 {
                    w.mobys[id].set_pv_i(0x32c, 0xb4);
                    idle(w, id, ctx, 0);
                } else {
                    w.mobys[id].set_pv_i(0x32c, 0x78);
                    idle(w, id, ctx, 6);
                }
                w.mobys[id].set_state(1, -1);
            }
            roam(w, id, ctx);
        }
        1 => {
            if ctx.game.rng.randi(7) == 0 {
                let yaw = w.mobys[id].rot[2];
                let at = w.mobys[id].pos;
                ai::move_step(w, id, ctx, yaw, at);
            }
            if tick_down(w, id, 0x32c) {
                if !wrapped(w, id, ctx) { return; }
                let t = ctx.game.rng.rand_range(0x78, 400);
                w.mobys[id].set_pv_i(0x32c, t);
                walk_seq(w, id, ctx, 0x19, 0x23);
                w.mobys[id].set_state(0, -1);
            }
        }
        2 => {
            let m = &w.mobys[id];
            let mut off = weave(ctx);
            if dist2(m.pos, m.pv_v(0x230)) < 4.75 { off = 0.0; }
            let goal = m.pv_v(0x2e0);
            go_to(w, id, ctx, goal, off);
            if let Some((_, tp)) = target(w, id) {
                let m = &w.mobys[id];
                if dist2(m.pos, tp) < 3.75 && (m.pos[2] - tp[2]).abs() < 0.5 {
                    let face = atan2(tp[0] - m.pos[0], tp[1] - m.pos[1]);
                    if rot_dist(m.rot[2], face) < 0.174_532_95 { bite_start(w, id, ctx, (0x10, 0x16)); }
                }
            }
        }
        4 => {
            let m = &w.mobys[id];
            let goal = cuboid_at(ctx, m.pv_i(0x310)).unwrap_or(m.pv_v(0x2d0));
            go_to(w, id, ctx, goal, 0.0);
            if wrapped(w, id, ctx) && (w.mobys[id].pv_i(MV + 0x94) & 0x180) != 0x100 {
                idle(w, id, ctx, 0);
                w.mobys[id].set_state(1, -1);
            }
        }
        6 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x398, m.pv_i(0x390));
            m.set_pv_i(0x39c, m.pv_i(0x388));
            roam(w, id, ctx);
            let m = &w.mobys[id];
            if m.pv_i(T_FOUND) == 1 {
                let poked = usize::try_from(m.pv_i(0x38c)).ok().and_then(|i| w.mobys.get(i)).is_none_or(|o| o.timer != 0);
                if poked { w.mobys[id].set_state(7, -1); }
            }
        }
        7 => {
            let off = weave(ctx);
            let Some(goal) = cuboid_at(ctx, w.mobys[id].pv_i(0x380)) else { return };
            w.mobys[id].set_pv_v(0x2e0, goal);
            go_to(w, id, ctx, goal, off);
            if w.mobys[id].pv_i(MV + 0x94) & 0x80 != 0 && w.mobys[id].drawn == 0 {
                w.delete(id);
                return;
            }
        }
        _ => {}
    }
    let state = w.mobys[id].state;
    if matches!(state, 4 | 6 | 7) {
        approach_check(w, id, ctx);
        return;
    }
    // Seeing Ratchet (or alarmed): the group's two nearest charge.
    let m = &mut w.mobys[id];
    m.set_pv_i(0x398, m.pv_i(0x394));
    m.set_pv_i(0x39c, m.pv_i(0x388));
    let seen = m.pv_i(T_FOUND) == 1 || m.pv_i(0x330) != 0;
    let (grouped, uid, first, second) = (m.group != -1, m.uid as i32, m.pv_i(0x36c), m.pv_i(0x370));
    let picked = !grouped || w.shared_i(first) == uid || w.shared_i(second) == uid;
    let near_height = target(w, id).is_some_and(|(_, tp)| (w.mobys[id].pos[2] - tp[2]).abs() < 1.0);
    if seen && picked && near_height {
        let m = &mut w.mobys[id];
        m.set_pv_i(0x330, 0xf0);
        m.timer = 1;
        m.set_pv_i(MV + 0x3c, m.pv_i(0x30c));
        let tp = target(w, id).map(|t| t.1).unwrap_or(w.mobys[id].pos);
        if w.mobys[id].state == 2 {
            w.mobys[id].set_pv_v(0x2e0, tp);
        } else {
            let m = &w.mobys[id];
            let busy = if m.group == -1 { m.pv_i(0x334) != 0 } else { 1 < w.shared_i(m.pv_i(0x33c)) || m.pv_i(0x334) != 0 };
            if !busy && dist2(m.pos, tp) < 14.0 {
                w.mobys[id].set_pv_v(0x2e0, tp);
                walk_seq(w, id, ctx, 0x14, 0x1e);
                claim(w, id);
                w.mobys[id].set_state(2, -1);
            }
        }
    } else if w.mobys[id].state == 2 {
        release(w, id);
        w.mobys[id].timer = 0;
        idle(w, id, ctx, 0);
        w.mobys[id].set_state(1, -1);
    }
    approach_check(w, id, ctx);
}

/// Within 3.5 of the target (0.5 in height): facing it within 30° it bites, else it turns to it.
fn approach_check(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let Some((_, tp)) = target(w, id) else { return };
    let m = &w.mobys[id];
    if 3.5 <= dist2(m.pos, tp) || 0.5 <= (m.pos[2] - tp[2]).abs() { return; }
    let face = atan2(tp[0] - m.pos[0], tp[1] - m.pos[1]);
    if rot_dist(m.rot[2], face) < std::f32::consts::FRAC_PI_6 {
        bite_start(w, id, ctx, (8, 0xc));
    } else {
        let m = &mut w.mobys[id];
        let (a, b, most) = (m.pv_f(MV + 0x14), m.pv_f(MV + 0x18), m.pv_f(MV + 0x1c));
        let (mut yaw, mut v) = (m.rot[2], m.pv_f(0x1c4));
        turn_ease(face, a, b, most, &mut yaw, &mut v, 0);
        m.rot[2] = yaw;
        m.set_pv_f(0x1c4, v);
    }
}

/// Sub-state 4: the bite.
fn bite(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let rand7 = ctx.game.rng.randi(7);
    match target(w, id) {
        None => {
            if rand7 == 0 {
                let (yaw, at) = (w.mobys[id].rot[2], w.mobys[id].pos);
                ai::move_step(w, id, ctx, yaw, at);
            }
        }
        Some((_, tp)) => {
            let m = &w.mobys[id];
            let face = atan2(tp[0] - m.pos[0], tp[1] - m.pos[1]);
            if rand7 == 0 {
                let at = m.pos;
                ai::move_step(w, id, ctx, face, at);
            } else {
                let m = &mut w.mobys[id];
                let (a, b, most) = (m.pv_f(MV + 0x14), m.pv_f(MV + 0x18), m.pv_f(MV + 0x1c));
                let (mut yaw, mut v) = (m.rot[2], m.pv_f(0x1c4));
                turn_ease(face, a, b, most, &mut yaw, &mut v, 0);
                m.rot[2] = yaw;
                m.set_pv_f(0x1c4, v);
            }
        }
    }
    let t = ctx.host.frame_time(&w.mobys[id]);
    let biting = seq_a(w, id, ctx) == 10;
    if biting && (18.0..=25.0).contains(&t) {
        let damage = w.mobys[id].pv_b(0x34) as f32;
        for j in 0..5 {
            ai::attack_joint(w, id, ctx, 0.5, damage, 1.0, j, 0x400_0001, 0, 1, 0);
            // 0x1bf918: the last moby the sphere touched.
            let hit_him = ctx.hero_moby.is_some() && w.hits.last_target == ctx.hero_moby;
            if hit_him { w.mobys[id].set_pv_b(0x35c, 1); }
        }
    }
    if t <= 30.0 && (biting || seq_b(w, id, ctx) == 10) { return; }
    if w.mobys[id].pv_b(0x35c) == 0 {
        w.mobys[id].set_pv_i(0x334, 0x28);
        if seq_b(w, id, ctx) != 8 {
            let b = ctx.game.rng.rand_range(0x19, 0x1e);
            play(w, id, ctx, 8, 0, b);
        }
    } else {
        w.mobys[id].set_pv_i(0x334, 100);
        if seq_b(w, id, ctx) != 0xb {
            let b = ctx.game.rng.rand_range(0x19, 0x1e);
            play(w, id, ctx, 0xb, 0, b);
        }
    }
    w.mobys[id].set_pv_b(0x35c, 0);
    release(w, id);
    let m = &mut w.mobys[id];
    m.set_pv_i(0x398, m.pv_i(0x394));
    m.set_pv_i(0x39c, m.pv_i(0x388));
    m.set_state(1, 3);
}

/// On the bank its body tilts to the floor's normal (up when it stands on a moby without solid collision), 40° at
/// most, each axis eased (`0x3152d0`: 0.1°/tick², at most 6°/tick).
fn tilt(w: &mut MobyWorld, id: usize, ctx: &Ctx) { tilt_to_floor(w, id, ctx, MV, 0x358, 0x354); }

/// The tilt for a movement block `mv` and the pitch / roll springs' velocity words.
pub(super) fn tilt_to_floor(w: &mut MobyWorld, id: usize, ctx: &Ctx, mv: usize, v_pitch: usize, v_roll: usize) {
    let _ = ctx;
    let m = &w.mobys[id];
    let on_moby = ai::link(m, mv + 0x88).and_then(|f| w.mobys.get(f));
    let n = match on_moby {
        Some(f) if !f.has_collision => [0.0, 0.0, 1.0],
        _ => m.pv_v(mv + 0x50),
    };
    // Into the frame of its yaw (the transpose of the yaw's rows).
    let (s, c) = m.rot[2].sin_cos();
    let l = [n[0] * c + n[1] * s, -n[0] * s + n[1] * c, n[2]];
    let pitch = atan2(l[2], l[0]).clamp(-0.698_131_8, 0.698_131_8);
    let roll = (-atan2(l[2], l[1])).clamp(-0.698_131_8, 0.698_131_8);
    let m = &mut w.mobys[id];
    let (mut a, mut v) = (m.rot[1], m.pv_f(v_pitch));
    turn_ease(pitch, 0.001_745_329_3, 0.001_745_329_3, 0.104_719_76, &mut a, &mut v, 0);
    m.rot[1] = a;
    m.set_pv_f(v_pitch, v);
    let (mut a, mut v) = (m.rot[0], m.pv_f(v_roll));
    turn_ease(roll, 0.001_745_329_3, 0.001_745_329_3, 0.104_719_76, &mut a, &mut v, 0);
    m.rot[0] = a;
    m.set_pv_f(v_roll, v);
}
