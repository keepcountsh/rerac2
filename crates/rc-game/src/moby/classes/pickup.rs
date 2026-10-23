//! The ammo pickups (class 0xcdb, L00 `0x3d26a8`), out of the ammo crates (`0x39d948` → [`drop_ammo`]): a shard of the
//! ammo box (crate::moby::shards, piece 0) thrown up at a random angle, tumbling on the level. Ratchet within the
//! pickup radii (2.125 across, 1.25 up) while it rests or bounces, or three ticks after he came near while it flew,
//! takes it — unless that weapon is full — its ammo added (`0x2d4358`), the pickup sound (no more than one every 11
//! ticks); then it flies into his head (state 2), speeding up and spinning, shrinking over its last 30 steps.
//!
//! **pvars** (after the shard's, crate::moby::shards): `+0x60` the ammo it gives, `+0x68` the item, `+0x6c` 0,
//! `+0x70` 0x10, `+0x74` the rise while flying in, `+0x78` the height it started at, `+0x7c` the ticks left of the
//! near-miss window.
//!
//! Not ported: the message (`0x327478` → the GUI's message box `0x42f608`), the weapon panel shown, the challenge
//! mode's doubled ammo (`0x1a7a12`), the gravity frames.

use crate::hero::physics::{add, approach, cross, dist, dist2, len, set_len, sub, V3};
use crate::moby::shards::{self, COUNT, GRAVITY, SPIN, VEL};
use crate::moby::{Ctx, MobyWorld};

/// The ammo box's class.
pub const CLASS: i16 = 0x0cdb;
/// `0x18c2c4` / `0x18c2c8`: Ratchet's pickup radii (crate::moby::classes::bolt's).
const PICK_XY: f32 = 2.125;
const PICK_Z: f32 = 1.25;

/// The items `0x31aa10` leaves out.
fn excluded(i: usize) -> bool { matches!(i, 0x1f | 0x2d | 0x3d | 0x4d) }

/// `0x31aa10`: an item Ratchet has that takes ammo, one not full first (each as likely; else any of them), and how many
/// pickups (two one time in five). `None` with none (the crate's own type then stands; not on Aranos).
fn pick(ctx: &mut Ctx) -> (Option<usize>, i32) {
    let items = &ctx.level.items;
    let g = &mut *ctx.game;
    let max = |i: usize| items.get(i).map_or(0, |r| r.max_ammo as i32);
    let has = |g: &crate::game::GameState, i: usize| g.items.get(i).is_some_and(|&b| b != 0);
    let ammo = |g: &crate::game::GameState, i: usize| g.ammo.get(i).copied().unwrap_or(0);
    let all: Vec<usize> = (0..0x38).filter(|&i| has(g, i) && max(i) != 0 && !excluded(i)).collect();
    let short: Vec<usize> = (0..0x38).filter(|&i| has(g, i) && max(i) != 0 && i != 0x1f && i != 0x3d && ammo(g, i) < max(i)).collect();
    let item = if all.iter().any(|&i| ammo(g, i) < max(i)) {
        let k = g.rng.randi(all.iter().filter(|&&i| ammo(g, i) < max(i)).count() as i32) as usize;
        short.get(k).copied()
    } else {
        let k = g.rng.randi(all.len() as i32) as usize;
        all.get(k).copied()
    };
    let n = if g.rng.randi(5) != 0 { 1 } else { 2 };
    (item, n)
}

/// `0x39d948`'s ammo crate part: [`pick`]'s pickups at the crate.
pub fn drop_ammo(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let (item, n) = pick(ctx);
    let Some(item) = item else { return };
    let at = w.mobys[id].pos;
    for _ in 0..n { spawn(w, ctx, at, item, None); }
}

/// `0x3d2d30(pos, item, amount, 0)`: a pickup of `item` (its record's `+0x7c` ammo unless `amount`) at `pos`: turned
/// to a random heading, thrown 2..4 a second at a random angle and 4..8 a second up, spinning 180..360°/s, its gravity
/// 10..15; its light from Ratchet's.
pub fn spawn(w: &mut MobyWorld, ctx: &mut Ctx, pos: V3, item: usize, amount: Option<i32>) -> Option<usize> {
    let rng = &mut ctx.game.rng;
    let yaw = rng.rand_angle();
    // `0x31bbc0(1/30, 1/15)`, then `0x326328`: the rise replaces its up component.
    let l = rng.randf(0.033_333_335, 0.066_666_67);
    let (a, b) = (rng.rand_angle(), rng.rand_angle());
    let up = rng.randf(4.0, 8.0) * 0.016_666_668;
    let vel = [a.cos() * b.sin() * l, a.sin() * b.sin() * l, up];
    let spin = rng.randf(180.0, 360.0);
    let gravity = rng.randf(10.0, 15.0);
    let hero = ctx.hero_moby;
    let id = shards::spawn_shard(w, ctx, CLASS, hero, pos, [0.0, 0.0, yaw], None, spin, gravity, 0, vel, 0xff)?;
    let give = amount.unwrap_or_else(|| ctx.level.items.get(item).map_or(0, |r| r.pickup_ammo as i32));
    let light = hero.and_then(|h| w.mobys.get(h)).map(|h| (h.light, h.ambient));
    let m = &mut w.mobys[id];
    m.state = 1;
    m.update = Some(update);
    m.set_pv_i(0x60, give);
    m.set_pv_i(0x68, item as i32);
    m.set_pv_i(0x70, 0x10);
    m.set_pv_f(0x6c, 0.0);
    m.set_pv_f(0x78, m.pos[2]);
    if let Some((l, a)) = light { (m.light, m.ambient) = (l, a); }
    w.changed = true;
    Some(id)
}

/// Within Ratchet's pickup radii (`0x2eedb0` up, `0x2ef050` across).
fn near(p: V3, hero: V3) -> bool { (p[2] - hero[2]).abs() < PICK_Z && dist2(p, hero) < PICK_XY }

/// `0x3d26a8`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => {
            // A placed pickup: its item is the one whose pickup class it is.
            let class = w.mobys[id].o_class;
            let item = (0..0x38).filter(|&i| ctx.level.items.get(i).is_some_and(|r| r.pickup_class == class)).last();
            let m = &mut w.mobys[id];
            m.state = 1;
            if let Some(i) = item { m.set_pv_i(0x68, i as i32); }
        }
        1 => {
            if !rest_or_take(w, id, ctx) { return; }
        }
        2 => {
            if !fly_in(w, id, ctx) { return; }
        }
        _ => {}
    }
    fade(w, id, ctx);
}

/// State 1: tumbling, and taken when Ratchet is near. False when it went (it is not).
fn rest_or_take(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let hero = ctx.hero.pos;
    let m = &mut w.mobys[id];
    // At rest on three points, woken once every 16 ticks (its own phase) to fall again if the ground went.
    if m.pvars[COUNT] == 3 && (ctx.game.frame & 0xf) as usize == (id & 0xf) {
        m.set_pv_v(VEL, [0.0; 3]);
        for (k, v) in [0.0, 0.0, 0.0, 1.0].iter().enumerate() { m.set_pv_f(SPIN + 4 * k, *v); }
        m.pvars[COUNT] = 0;
    }
    let before = m.pv_v(VEL);
    shards::step(w, id, ctx);
    let m = &mut w.mobys[id];
    let bounced = 2.0 * m.pv_f(GRAVITY) < dist(before, m.pv_v(VEL));
    let mut take = true;
    if m.pvars[COUNT] != 0 || bounced {
        // `0x2eeab0`: the near-miss window counts down; while it runs it is taken.
        let mut t = m.pv_i(0x7c);
        let running = !crate::moby::countdown(&mut t);
        m.set_pv_i(0x7c, t);
        if !running && !near(m.pos, hero) { take = false; }
    } else {
        take = false;
        if near(m.pos, hero) { m.set_pv_i(0x7c, 3); }
    }
    let item = m.pv_i(0x68).max(0) as usize;
    let max = ctx.level.items.get(item).map_or(0, |r| r.max_ammo as i32);
    if max <= ctx.game.ammo.get(item).copied().unwrap_or(0) { take = false; }
    if m.pos[2] < m.pv_f(0x78) - 2.0 { take = true; }
    if ctx.hero.hp == 0 { take = false; }
    if !take { return true; }
    // `0x2d4358(item, n)`: added up to the most.
    let give = m.pv_i(0x60);
    if let Some(a) = ctx.game.ammo.get_mut(item) { *a = (*a + give).min(if max == 0 { i32::MAX } else { max }); }
    m.set_state(2, -1);
    m.set_pv_f(0x74, 0.133_333_33);
    m.set_pv_v(VEL, [0.0; 3]);
    if 11 <= (ctx.game.frame - ctx.game.pickup_sound_frame).abs() {
        ctx.host.play_sound(&w.mobys[id], 0, 0);
        ctx.game.pickup_sound_frame = ctx.game.frame;
    }
    true
}

/// State 2: into Ratchet's head, faster and rising while above him, spinning 10° a tick about the line across its
/// move (its point 0.3 up kept still), shrinking for the last 30 steps; gone at 1 step from him or at no size.
fn fly_in(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) -> bool {
    let (hero, carry) = (ctx.hero.pos, ctx.hero.carry);
    let class_scale = w.class(CLASS).map_or(1.0, |c| c.scale);
    let m = &mut w.mobys[id];
    let mut rise = m.pv_f(0x74);
    approach(if m.pos[2] - hero[2] > 0.0 { 0.0 } else { 8.0 }, 0.006_666_667, &mut rise);
    m.set_pv_f(0x74, rise);
    m.pos = add(add(m.pos, [0.0, 0.0, rise]), carry);
    let mut speed = len(m.pv_v(VEL));
    approach(1.066_666_7, 0.006_666_667, &mut speed);
    let to = sub(add(hero, [0.0, 0.0, 1.0]), m.pos);
    let steps = len(to) / speed.max(1e-6);
    if steps < 30.0 { approach(0.0, class_scale * 0.033_333_335, &mut m.scale); }
    if m.scale == 0.0 || steps <= 1.0 {
        w.delete(id);
        return false;
    }
    let vel = set_len(to, speed);
    m.set_pv_v(VEL, vel);
    m.pos = add(m.pos, vel);
    let axis = cross([0.0, 0.0, 1.0], vel);
    if len(axis) > 0.0 {
        let pin = [0.0, 0.0, 0.3];
        let through = |r: &[V3; 3], v: V3| -> V3 { std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k]) };
        let before = through(&m.rows, pin);
        let q = shards::axis_quat(0.087_155_74, axis);
        m.rows = m.rows.map(|r| shards::rotate(q, r));
        let after = through(&m.rows, pin);
        m.pos = sub(m.pos, sub(after, before));
    }
    w.changed = true;
    true
}

/// While it lies about Ratchet (their spheres overlapping) it fades to 0x24; else back to full (mode 0x2000 while
/// see-through).
fn fade(w: &mut MobyWorld, id: usize, ctx: &Ctx) {
    let over = w.mobys[id].state == 1
        && ctx.hero_moby.and_then(|h| w.mobys.get(h)).is_some_and(|h| {
            let m = &w.mobys[id];
            let r = m.bsphere[3] + h.bsphere[3];
            let d = sub(m.pos, h.pos);
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < r * r
        });
    let m = &mut w.mobys[id];
    m.alpha = if over { if 0x24 < m.alpha { m.alpha - 4 } else { 0x24 } } else if 0x7b < m.alpha { 0x80 } else { m.alpha + 4 };
    if m.alpha >= 0x80 { m.mode &= !0x2000 } else { m.mode |= 0x2000 }
}
