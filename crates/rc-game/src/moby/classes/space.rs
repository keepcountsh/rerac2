//! The space fights' enemies (L25 Wupash Nebula; the space-AI library `0x374b10..0x378be0` all the ship levels
//! share). One class so far: the fighter 0x0957 (`0x3ca118`).
//!
//! **The AI block** (the moby's `+0x68`[`0x14`] with mode 0x20; kept here in the pvars from [`AI`]): `+0x60` the
//! goal, `+0x80` toward it, `+0x90` the velocity a tick, `+0xa0` the last one, `+0xb0` the last place, `+0xc0` this
//! tick's push, `+0x30` / `+0x34` / `+0x38` the roll, pitch and yaw, `+0x100` the state, `+0x104` in the gun's cone,
//! `+0x108` in range, `+0x110` active, `+0x114` the attack kind, `+0x118` attacking, `+0x120` the target (the
//! player's ship), `+0x124` the speed, `+0x130` the goal's timer.
//!
//! **A tick** (`0x377278`): the step (`+0x90`, not in the circling 8); kept inside the fight's sphere (`0x374f50`: the
//! mission object's `+0x68`[0], radius 256 round (512, 512, 512) on Wupash; less its own size there); the push (`0x377a58`:
//! toward the goal at the state's pull, then sped up below half the state's top speed and slowed above it, at most
//! pull · top); the top speed (`0x377e30`); the angles from the velocity (`0x377820`: the yaw and pitch along it,
//! the roll leaning into the turn by `0.98·roll + turned·pull'·1.41·8`, at most 81°); then the state:
//! * 1 wandering: at the goal (within 1) or its timer out, a new one (`0x3783a0`): a random point of the sphere
//!   across, ±64 up from it, 300 ticks.
//! * 4 closing: the goal 25 ahead of the target; there → 5 for 300..600 ticks.
//! * 5 on its tail: 5 ahead of the target; there or the timer out → the attack's end (`0x376950`), 1.
//!
//! The state's pull, push and top speed (`0x377bd0`, `$gp−0x2ab0..`): 1 and 0xd 0.02 / 0.1 / 0.4, 3 0.02 / 0.1 / 0.417,
//! 4 0.5 top, 5 0.04 / 0.2 / 0.417, 6 0.03 / 0.15 / 0.367, 7 0.167 top, 8 0.417 top; in range of the target 0.7 of
//! the pull and push.
//!
//! **The fighter** (`0x3ca118`): its start (`0x3ca4a8`, `0x376068`): active, health 1, scale 0.06, at a random
//! point of the sphere, wandering. Each tick: the AI tick, its hits (`0x376650` → the mission's handler `0x40f270`:
//! a hit with the flags 0x230000 takes its damage from the health; at 0 it bursts and is put away, `0x375fe8`:
//! hidden, no collision, not active), the range test (`0x376a60`: the target within 176 with a clear line → in
//! range; within 40.5° across and up of its facing → in the gun's cone), and the attack's start (`0x3769a0` /
//! `0x3767e8`: not attacking while fewer than 4 attack (`0x221820`), → 4 with a timer of 120..500).
//!
//! Its gun (`0x3ca5a8`): in range and in the cone, every 20 ticks a shot ([`crate::ship::spawn_enemy_shot`]).
//!
//! Not ported: the formations (3: the leader `+0x10`'s place),
//! the states 2, 6, 7, 8, 0xb, 0xc, 0xd, the leaders' list (`0x3750e8`: only those classes attack), the goal's and
//! the step's collision (`0x378570`, `0x376650`'s push), the burst (`0x4184c8`) and the bolts (`0x40fbc8`), the
//! exhaust (`0x3ca6c0`), the sounds; the waves that bring them back (the mission object 0x0eee).

use crate::hero::physics::{add, atan2, dist, len, len2, rot_dist, set_len, sub, sub_rot, V3};
use crate::moby::{mode, Ctx, MobyWorld};

/// The fighter's class.
pub const FIGHTER: i16 = 0x0957;

/// Where the AI block sits in the pvars, and the fighter's own words.
const AI: usize = 0x200;
const HEALTH: usize = 0x1f0;
const STARTED: usize = 0x1f4;
/// A wave's return asked (`0x376428`): gather round `+0x1e0`, go for `+0x1e8`... (the points kept in the pvars).
const REVIVE: usize = 0x1f8;
const REVIVE_AT: usize = 0x1d0;
const REVIVE_GOAL: usize = 0x1e0;
/// The mission object's words the flight keeps (`0x22182c` the quiet, `0x221810` the active count).
pub const PASSIVE: usize = 0x100;
pub const ACTIVE_COUNT: usize = 0x104;
/// The gun's wait (`+0x1b8`).
const GUN: usize = 0x1b8;

/// The AI block's words.
mod a {
    pub const GOAL: usize = 0x60;
    pub const VEL: usize = 0x90;
    pub const ROLL: usize = 0x30;
    pub const PITCH: usize = 0x34;
    pub const YAW: usize = 0x38;
    pub const STATE: usize = 0x100;
    pub const CONE: usize = 0x104;
    pub const RANGE: usize = 0x108;
    pub const ACTIVE: usize = 0x110;
    pub const ATTACKING: usize = 0x118;
    pub const TARGET: usize = 0x120;
    pub const SPEED: usize = 0x124;
    pub const TIMER: usize = 0x130;
}

fn ai_i(w: &MobyWorld, id: usize, o: usize) -> i32 { w.mobys[id].pv_i(AI + o) }
fn ai_f(w: &MobyWorld, id: usize, o: usize) -> f32 { w.mobys[id].pv_f(AI + o) }
fn ai_v(w: &MobyWorld, id: usize, o: usize) -> V3 { w.mobys[id].pv_v(AI + o) }
fn set_ai_i(w: &mut MobyWorld, id: usize, o: usize, v: i32) { w.mobys[id].set_pv_i(AI + o, v) }
fn set_ai_f(w: &mut MobyWorld, id: usize, o: usize, v: f32) { w.mobys[id].set_pv_f(AI + o, v) }
fn set_ai_v(w: &mut MobyWorld, id: usize, o: usize, v: V3) { w.mobys[id].set_pv_v(AI + o, v) }

/// The fight's sphere (`0x1b2114 + 0x221800·0x80`: the mission object 0x0eee's `+0x68`[0]): its centre and radius.
fn arena(w: &MobyWorld, ctx: &Ctx) -> (V3, f32) {
    let i = w.mobys.iter().find(|m| m.o_class == 0x0eee).map_or(0, |m| m.pv_i(0));
    let Some(v) = usize::try_from(i).ok().and_then(|i| ctx.level.spheres.get(i)) else { return ([512.0; 3], 256.0) };
    let m = &v.matrix;
    ([m[3][0], m[3][1], m[3][2]], len([m[0][0], m[0][1], m[0][2]]))
}

/// The target (`+0x120`, index + 1).
fn target(w: &MobyWorld, id: usize) -> Option<usize> {
    usize::try_from(ai_i(w, id, a::TARGET) - 1).ok().filter(|&t| w.mobys.get(t).is_some_and(|m| m.state < 0x80))
}

/// `0x377bd0`: the state's pull, push and top speed (a tick).
fn tuning(state: i32, in_range: bool) -> (f32, f32, f32) {
    let (mut k, mut kd, top) = match state {
        2 => (0.005, 0.025, 0.133_333_34),
        3 => (0.02, 0.1, 0.416_666_7),
        4 | 0xb => (0.02, 0.1, 0.5),
        5 => (0.04, 0.2, 0.416_666_7),
        6 => (0.03, 0.15, 0.366_666_67),
        7 => (0.02, 0.1, 0.166_666_67),
        8 => (0.02, 0.1, 0.416_666_7),
        _ => (0.02, 0.1, 0.4),
    };
    if in_range && !matches!(state, 0xb | 2 | 0xc) {
        k *= 0.7;
        kd *= 0.7;
    }
    (k, kd, top)
}

/// `0x378570`: the goal kept inside the sphere.
fn set_goal(w: &mut MobyWorld, id: usize, ctx: &Ctx, goal: V3) {
    let (c, r) = arena(w, ctx);
    let d = sub(goal, c);
    let goal = if r < len(d) { add(c, set_len(d, r)) } else { goal };
    set_ai_v(w, id, a::GOAL, goal);
}

/// `0x3783a0`: a random goal in the sphere, ±64 up from the fighter, for 300 ticks.
fn wander_goal(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (c, r) = arena(w, ctx);
    let g = &mut ctx.game.rng;
    let x = g.rand_angle().cos() * g.randf(0.0, r);
    let y = g.rand_angle().sin() * g.randf(0.0, r);
    let z = g.randf(-64.0, 64.0) + w.mobys[id].pos[2];
    set_goal(w, id, ctx, [c[0] + x, c[1] + y, z]);
    set_ai_i(w, id, a::TIMER, 300);
}

/// `0x3786e8`: at the goal (within 1; 8 in state 2), or the goal's timer out.
fn arrived(w: &mut MobyWorld, id: usize) -> bool {
    let state = ai_i(w, id, a::STATE);
    let near = if state == 2 { 8.0 } else { 1.0 };
    let at = dist(w.mobys[id].pos, ai_v(w, id, a::GOAL)) < near && state != 8;
    let mut t = ai_i(w, id, a::TIMER);
    let out = crate::moby::countdown(&mut t);
    set_ai_i(w, id, a::TIMER, t);
    at || out
}

/// `0x376950`: the attack over.
fn end_attack(w: &mut MobyWorld, id: usize) {
    set_ai_i(w, id, a::ATTACKING, 0);
    set_ai_i(w, id, a::STATE, 1);
}

/// `0x3ca4a8` / `0x376068`: the start.
fn start(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (c, r) = arena(w, ctx);
    let ship = w.mobys.iter().position(|m| m.o_class == crate::ship::SHIP_CLASS && m.state < 0x80);
    {
        let g = &mut ctx.game.rng;
        let (d, yaw, pitch) = (g.randf(0.0, r), g.rand_angle(), g.rand_angle());
        let p = super::shot::polar(d, yaw, pitch);
        let m = &mut w.mobys[id];
        m.state = 1;
        m.mode = (m.mode | mode::KEEP_ROWS) & !3;
        m.update_dist = 0xff;
        m.draw_dist = 0x1ff;
        m.scale = 0.06;
        m.pos = add(c, p);
        m.set_pv_f(HEALTH, 1.0);
        m.set_pv_i(STARTED, 1);
    }
    for o in (0..0x160).step_by(4) { set_ai_i(w, id, o, 0); }
    set_ai_i(w, id, a::ACTIVE, 1);
    set_ai_i(w, id, a::STATE, 1);
    set_ai_i(w, id, a::TARGET, ship.map_or(0, |s| s as i32 + 1));
    wander_goal(w, id, ctx);
    w.changed = true;
}

/// `0x377278`: the AI tick.
fn think(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let state = ai_i(w, id, a::STATE);
    let in_range = ai_i(w, id, a::RANGE) != 0;
    let (k, kd, top) = tuning(state, in_range);
    // The step, kept inside the sphere (the game less the fighter's size).
    let vel = ai_v(w, id, a::VEL);
    if state != 8 { w.mobys[id].pos = add(w.mobys[id].pos, vel); }
    let (c, r) = arena(w, ctx);
    let d = sub(w.mobys[id].pos, c);
    if r < len(d) { w.mobys[id].pos = add(c, set_len(d, r)); }
    // `0x377a58`: the push.
    let pos = w.mobys[id].pos;
    let mut push = set_len(sub(ai_v(w, id, a::GOAL), pos), k);
    let speed = ai_f(w, id, a::SPEED);
    let f = (speed - top * 0.5) / top;
    let mut m = f.abs().clamp(k, kd);
    if 0.0 < f { m = -m; }
    push = add(push, set_len(vel, m));
    if k * top < len(push) { push = set_len(push, k * top); }
    let mut vel = add(vel, push);
    // `0x377e30`: the top speed.
    let mut sp = len(vel);
    if top < sp {
        vel = set_len(vel, top);
        sp = top;
    }
    set_ai_v(w, id, a::VEL, vel);
    set_ai_f(w, id, a::SPEED, sp);
    // `0x377820`: the angles.
    let old_yaw = ai_f(w, id, a::YAW);
    let yaw = atan2(vel[0], vel[1]);
    let pitch = -atan2(len2(vel), vel[2]);
    let side = if sub_rot(yaw, old_yaw) < 0.0 { -1.0 } else { 1.0 };
    let turned = rot_dist(old_yaw, yaw);
    let roll = (ai_f(w, id, a::ROLL) * 0.98 + turned * -side * kd * 1.413_716_7 * 8.0).clamp(-1.413_716_7, 1.413_716_7);
    (set_ai_f(w, id, a::ROLL, roll), set_ai_f(w, id, a::PITCH, pitch), set_ai_f(w, id, a::YAW, yaw));
    w.mobys[id].rows = crate::moby::attach::rows_of([roll, pitch, yaw]);
    // The state.
    match state {
        1 => {
            if arrived(w, id) { wander_goal(w, id, ctx); }
        }
        4 | 5 => match target(w, id) {
            Some(t) => {
                let ahead = if state == 4 { 25.0 } else { 5.0 };
                let tm = &w.mobys[t];
                let goal = add(tm.pos, set_len(tm.rows[0], ahead));
                set_goal(w, id, ctx, goal);
                if arrived(w, id) {
                    if state == 4 {
                        set_ai_i(w, id, a::STATE, 5);
                        let t = ctx.game.rng.rand_range(300, 600);
                        set_ai_i(w, id, a::TIMER, t);
                    } else {
                        end_attack(w, id);
                    }
                }
            }
            None => end_attack(w, id),
        },
        _ => {
            end_attack(w, id);
        }
    }
}

/// `0x40f270` (the mission's hit handler): a hit with the flags 0x230000 takes its damage; at 0 the fighter is put
/// away (`0x375fe8`).
fn hits(w: &mut MobyWorld, id: usize) {
    let Some(h) = w.hit_message(id) else { return };
    w.clear_hit(id);
    if h.tmpl.flags & 0x0023_0000 == 0 { return; }
    let hp = (w.mobys[id].pv_f(HEALTH) - h.tmpl.damage).max(0.0);
    w.mobys[id].set_pv_f(HEALTH, hp);
    if hp <= 0.0 {
        set_ai_i(w, id, a::ATTACKING, 0);
        set_ai_i(w, id, a::ACTIVE, 0);
        let m = &mut w.mobys[id];
        m.has_collision = false;
        m.draw_dist = 0;
        m.drawn = 0;
        m.mode |= 3;
        w.changed = true;
    }
}

/// `0x376a60(176, …)`: in range (the target within 176, the line clear) and in the gun's cone (40.5° across and up).
fn range(w: &mut MobyWorld, id: usize, ctx: &Ctx) {
    set_ai_i(w, id, a::CONE, 0);
    set_ai_i(w, id, a::RANGE, 0);
    if passive(w) { return; }
    let Some(t) = target(w, id) else { return };
    let (p, q) = (w.mobys[id].pos, w.mobys[t].pos);
    if 176.0 < dist(p, q) { return; }
    // The line clear, or ending on a moby of the target's class.
    let hit = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, p, q, crate::collision::QueryFlags(0), Some(id)));
    if hit.is_some_and(|h| h.moby.is_none_or(|m| w.mobys.get(m).is_none_or(|m| m.o_class != w.mobys[t].o_class))) { return; }
    let d = sub(q, p);
    let across = rot_dist(atan2(d[0], d[1]), ai_f(w, id, a::YAW));
    let up = rot_dist(atan2(len2(d), d[2]), 0.0);
    if across <= 0.706_858_3 && up <= 0.706_858_3 { set_ai_i(w, id, a::CONE, 1); }
    set_ai_i(w, id, a::RANGE, 1);
}

/// `0x3769a0` / `0x3767e8`: the attack's start (not attacking, fewer than 4 attacking, not in a formation or dodging).
/// `0x3ca5a8`: in range and in the cone with the gun ready (`+0x1b8`), a shot at the point 2 ahead of the target
/// (`0x375198`), the gun ready again in 20 ticks.
fn fire(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let mut wait = w.mobys[id].pv_i(GUN);
    crate::moby::countdown(&mut wait);
    w.mobys[id].set_pv_i(GUN, wait);
    if wait != 0 || ai_i(w, id, a::RANGE) == 0 || ai_i(w, id, a::CONE) == 0 { return; }
    let Some(t) = target(w, id) else { return };
    if w.mobys[t].draw_dist == 0 { return; }
    let at = add(w.mobys[t].pos, set_len(w.mobys[t].rows[0], 2.0));
    let from = w.mobys[id].pos;
    crate::ship::spawn_enemy_shot(w, &*ctx.host, from, at, t);
    w.mobys[id].set_pv_i(GUN, 0x14);
}

fn maybe_attack(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if target(w, id).is_none() || ai_i(w, id, a::ATTACKING) == 1 || matches!(ai_i(w, id, a::STATE), 3 | 0xd) { return; }
    let attacking = w.mobys.iter().filter(|m| m.o_class == FIGHTER && m.state < 0x80 && m.pv_i(AI + a::ATTACKING) == 1).count();
    if 4 <= attacking { return; }
    set_ai_i(w, id, a::ATTACKING, 1);
    set_ai_i(w, id, a::STATE, 4);
    let t = ctx.game.rng.rand_range(0x78, 500);
    set_ai_i(w, id, a::TIMER, t);
}

/// A dev aid (`RC_SPACE_CLEAR`): every active fighter put away as if shot.
pub fn clear_all(w: &mut MobyWorld) {
    let ids: Vec<usize> = (0..w.mobys.len()).filter(|&i| w.mobys[i].o_class == FIGHTER && w.mobys[i].state < 0x80 && ai_i(w, i, a::ACTIVE) == 1).collect();
    for id in ids {
        set_ai_i(w, id, a::ACTIVE, 0);
        set_ai_i(w, id, a::ATTACKING, 0);
        let m = &mut w.mobys[id];
        (m.has_collision, m.draw_dist, m.drawn) = (false, 0, 0);
        m.mode |= 3;
    }
    w.changed = true;
}

/// The fighters active (`0x221810`).
pub fn active_count(w: &MobyWorld) -> usize {
    w.mobys.iter().filter(|m| m.o_class == FIGHTER && m.state < 0x80 && m.pv_i(STARTED) != 0 && m.pv_i(AI + a::ACTIVE) == 1).count()
}

/// `0x22182c`: the quiet (the mission object's word).
fn passive(w: &MobyWorld) -> bool { w.mobys.iter().find(|m| m.o_class == 0x0eee && m.state < 0x80).is_some_and(|m| m.pv_i(PASSIVE) != 0) }

/// `0x376548(1)` / `0x378250(10, at)` / `0x378878(10, goal)`: every fighter back (`0x376068`), within 10 of `at`,
/// going for a point within 10 of `goal`.
pub fn next_wave(w: &mut MobyWorld, at: V3, goal: V3) {
    for m in w.mobys.iter_mut().filter(|m| m.o_class == FIGHTER && m.state < 0x80) {
        // Updated again (`0x376068` clears the mode's 3); the rest of the return is its next update's.
        m.mode &= !3;
        m.set_pv_i(REVIVE, 1);
        m.set_pv_v(REVIVE_AT, at);
        m.set_pv_v(REVIVE_GOAL, goal);
    }
}

/// The return: active again (`0x376068`), shown, its health back, within 10 of the gathering point, its goal within 10
/// of the wave's.
fn revive(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (at, goal) = (w.mobys[id].pv_v(REVIVE_AT), w.mobys[id].pv_v(REVIVE_GOAL));
    let g = &mut ctx.game.rng;
    let p = super::shot::polar(g.randf(0.0, 10.0), g.rand_angle(), g.rand_angle());
    let q = super::shot::polar(g.randf(0.0, 10.0), g.rand_angle(), g.rand_angle());
    let ship = w.mobys.iter().position(|m| m.o_class == crate::ship::SHIP_CLASS && m.state < 0x80);
    {
        let m = &mut w.mobys[id];
        m.set_pv_i(REVIVE, 0);
        m.mode &= !3;
        m.draw_dist = 0x1ff;
        m.has_collision = true;
        m.pos = add(at, p);
        m.set_pv_f(HEALTH, 1.0);
    }
    for o in (0..0x160).step_by(4) { set_ai_i(w, id, o, 0); }
    set_ai_i(w, id, a::ACTIVE, 1);
    set_ai_i(w, id, a::STATE, 1);
    set_ai_i(w, id, a::TARGET, ship.map_or(0, |s| s as i32 + 1));
    set_goal(w, id, ctx, add(goal, q));
    set_ai_i(w, id, a::TIMER, 300);
    w.changed = true;
}

/// `0x3ca118`.
pub fn update_fighter(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pv_i(STARTED) == 0 { return start(w, id, ctx); }
    if w.mobys[id].pv_i(REVIVE) != 0 { revive(w, id, ctx); }
    if ai_i(w, id, a::ACTIVE) != 1 { return; }
    think(w, id, ctx);
    hits(w, id);
    if ai_i(w, id, a::ACTIVE) != 1 { return; }
    range(w, id, ctx);
    fire(w, id, ctx);
    maybe_attack(w, id, ctx);
    w.rebuild(id, &*ctx.host);
}
