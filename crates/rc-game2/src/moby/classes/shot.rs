//! The Lancer's shot, class 0xbfe (spawned by `0x3e3600`, update L00 `0x3e3990`): a small spinning blob from the
//! muzzle at the row's speed (60 / s), toward the lock's aim point or along Ratchet's facing (`0x18a580`), for range /
//! speed ticks. Each tick in flight (state 0) it turns to the lock while the lock lives, steps, spins (its rows from
//! random angles, each scaled by 0.35 + 0.15·sin of its own phase, `0x3e4d10`) and tests the step's line (`0x2de878`,
//! Ratchet ignored) with the row's damage: a hit — a moby takes the hit — plays its class sound 0; a hit or the end of
//! its time hides it (state 1) and 6 ticks later it is deleted. Out of the world's 2..1021 box it is deleted at once.
//!
//! **pvars**: +0x04..+0x0c the scale phases, +0x1c the lock, +0x20 the velocity (per tick), +0x2c the speed, +0x30
//! the range, +0x38 the ticks left, +0x3c 1 / the ticks, +0x40 1.0, +0x44..+0x4c the spin (per tick).
//!
//! Not ported: its trail (the draw callback `0x3e4050`, the gadget trail buffer `0x32b3b0`), the sprites drawn around
//! it each tick and on a hit (`0x33ef78`, `0x3e46d8`) or a fizzle (`0x3e4ae8`), the gravity boots' bend (`0x37f538` /
//! `0x37f618`), the first-person aim (`0x380ef8`).

use crate::hero::items::{row, LANCER};
use crate::hero::physics::{add, len, scale, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{aim, countdown, mode, Ctx, MobyWorld};

/// The shot's class.
pub const SHOT_CLASS: i16 = 0xbfe;

fn normalize(v: V3) -> V3 {
    let l = len(v);
    if 0.0 < l { scale(v, 1.0 / l) } else { v }
}

/// `0x3e3600(range, at, owner, target)`: a shot from `at`.
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, range: f32, at: V3, owner: usize, target: Option<usize>) {
    if w.class(SHOT_CLASS).is_none() { return; }
    let Some(r) = row(LANCER) else { return };
    let speed = r.speed / 60.0;
    let vel = match target {
        Some(t) if w.mobys.get(t).is_some() => scale(normalize(sub(aim::aim_point(w, t), at)), speed),
        _ => scale(normalize(ctx.hero.rows[0]), speed),
    };
    let id = w.create(SHOT_CLASS);
    let class_scale = w.class(SHOT_CLASS).map_or(1.0, |c| c.scale);
    let ticks = (range / speed).round() as i32;
    {
        let m = &mut w.mobys[id];
        m.update_dist = 0xff;
        m.mode = mode::KEEP_ROWS;
        m.drawn = 1;
        m.draw_dist = 0xff;
        m.parent = Some(owner);
        m.state = 0;
        m.set_pv_f(0x30, range);
        m.set_pv_v(0x20, vel);
        m.pos = at;
        m.set_pv_f(0x2c, speed);
    }
    // The rows first (from no angles), then the random spin, angles and phases.
    spin(w, id);
    let g = &mut ctx.game.rng;
    let s = [g.randf(0.0, 0.069_813_17), g.randf(0.0, 0.139_626_34), g.randf(0.0, 0.279_252_68)];
    let rot = [g.rand_angle(), g.rand_angle(), g.rand_angle()];
    let phases = [g.rand_angle(), g.rand_angle(), g.rand_angle()];
    let m = &mut w.mobys[id];
    m.set_pv_i(0x38, ticks);
    m.set_pv_f(0x3c, 1.0 / ticks.max(1) as f32);
    m.scale = class_scale * 6.0;
    m.set_pv_i(0x1c, target.map_or(0, |t| t as i32));
    m.set_pv_f(0x40, 1.0);
    m.set_pv_v(0x44, s);
    m.rot = rot;
    m.set_pv_v(4, phases);
    w.rebuild(id, &*ctx.host);
}

/// `0x3e4d10`: the rows from the Euler angles, each scaled by 0.35 + 0.15·sin of its phase; the phases step 0.5.
fn spin(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    let mut rows = crate::hero::physics::euler_rows(m.rot);
    let ph = m.pv_v(4);
    for k in 0..3 { rows[k] = scale(rows[k], ph[k].sin() * 0.15 + 0.35); }
    m.rows = rows;
    m.set_pv_v(4, ph.map(|p| crate::hero::physics::add_rot(p, 0.5)));
}

/// `0x3e3990`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let from = w.mobys[id].pos;
    // Homing on the lock while it lives.
    if let Some(t) = w.mobys[id].pv_link(0x1c) {
        let gone = w.mobys.get(t).is_none_or(|x| x.state >= 0xfd) || aim::health(w, t).is_some_and(|(hp, _)| hp <= 0.0 && !aim::is_crate(w, t));
        if gone {
            w.mobys[id].set_pv_i(0x1c, 0);
        } else {
            let d = sub(aim::aim_point(w, t), from);
            let l = len(d);
            if l == 0.0 {
                w.mobys[id].set_pv_i(0x1c, 0);
            } else {
                let s = w.mobys[id].pv_f(0x2c);
                w.mobys[id].set_pv_v(0x20, scale(d, s / l));
            }
        }
    }
    match w.mobys[id].state {
        0 => {
            let vel = w.mobys[id].pv_v(0x20);
            let to = add(from, vel);
            {
                let m = &mut w.mobys[id];
                m.pos = to;
                let s = m.pv_v(0x44);
                m.rot = [crate::hero::physics::add_rot(m.rot[0], s[0]), crate::hero::physics::add_rot(m.rot[1], s[1]), crate::hero::physics::add_rot(m.rot[2], s[2])];
            }
            spin(w, id);
            let damage = row(LANCER).map_or(0.0, |r| r.damage);
            let d = normalize(vel);
            let mut t = HitTemplate::new(damage, Some(id), 0x10000, [d[0], d[1], d[2], crate::moby::classes::lancer::NO_KNOCK_LIMIT]);
            t.kind = (w.mobys[id].o_class as u16 as u32) << 16 | 0x101;
            let hit = ctx.coll.and_then(|c| rc_game::collision_query::coll_line_m(c, ctx.scene, from, to, rc_game::collision_query::QueryFlags(0), ctx.hero_moby));
            match hit {
                Some(o) => {
                    if let Some(m) = o.moby { w.deliver_hit_any(m, &t, [o.point[0], o.point[1], o.point[2], 1.0]); }
                    ctx.host.play_sound(&w.mobys[id], 0, 0);
                    hide(w, id);
                }
                None => {
                    let mut n = w.mobys[id].pv_i(0x38);
                    let done = countdown(&mut n);
                    w.mobys[id].set_pv_i(0x38, n);
                    if done { hide(w, id); }
                }
            }
            w.rebuild(id, &*ctx.host);
        }
        1 => {
            let mut n = w.mobys[id].pv_i(0x38);
            let done = countdown(&mut n);
            w.mobys[id].set_pv_i(0x38, n);
            if done {
                w.delete(id);
                return;
            }
        }
        _ => {}
    }
    let p = w.mobys[id].pos;
    if p.iter().any(|&c| !(2.0..=1021.0).contains(&c)) { w.delete(id); }
}

/// State 1: hidden, the lock dropped, 6 ticks left.
fn hide(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    m.state = 1;
    m.set_pv_i(0x1c, 0);
    m.mode |= 0x41;
    m.set_pv_i(0x38, 6);
}
