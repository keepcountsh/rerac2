//! The Gravity Bomb (item 0x2a, crate::hero::items::ROWS): the hand moby 0xce3 (L00 `0x3ef940`, called by the item pass
//! while it is the hand item), the bomb it throws 0xce4 (`0x3efda0`) and its blast 0xce5 (`0x3f0310`).
//!
//! **The hand** (pvars: `+0x00` the landing point, `+0x10` its normal, `+0x20` the cooldown in seconds, `+0x24` a
//! landing found): each tick the aim — out of the look stance, the cone search (crate::moby::aim, π/8 × π/4 to 11.1
//! from the hand along Ratchet's facing, no crates) — and the throw that reaches it ([`launch`], `0x37e328`: 15 a
//! second across, the rise worked out for the gravity 12 a second²; with nothing aimed at, 12 ahead, or with L1
//! held the camera's look as far as it meets the ground under Ratchet, [`look_aim`]), then the arc
//! followed to where it lands ([`landing`], `0x37e988`) for the marker ([`draw_marker`], `0x37dfc0`). Its sequence 2
//! run out: back to 1. In hand 23 ticks, the cooldown out and ○ held (in the look stance after its wait): one bomb
//! (`0x2d42d0`), the use stance (crate::hero::items::ItemBlock::use_start: Ratchet's sequence 0x36, both hands),
//! sequence 2, class sound 0, the bomb thrown ([`throw`], `0x3f0190`) and the cooldown 1 / rate (2 s); out of bombs, a
//! press plays class sound 1. The use ends after 21 ticks in use (`0x2af848`).
//!
//! **The bomb** (pvars: `+0x00` its velocity, `+0x10` / `+0x14` its spins, `+0x20` the hit's normal, `+0x30` the hand):
//! each tick a step of the arc and the sweep ([`sweep`], `0x37edb8`: a 0.25 sphere at the new place, else the line
//! from the old); a wall (normal under 0.5 up) bounces it (its normal speed reversed at a quarter), a floor stops it
//! there, as does a moby of class type 5 or 9 or one the lines test (the blast then on the ground under it); stopped, it plays class sound 0, blows up ([`blast`], `0x3f0860`) and
//! goes. It spins (`+0xf0` / `+0xf4`), asks for its blob shadow (crate::shadows, 0.2 across) and is gone out of the
//! world's bounds (2..1021).
//!
//! **The blast** (one at a time: a new one deletes the last; pvars `+0x00` the normal, `+0x10` its size, `+0x14` its
//! timer, `+0x18` the growth, `+0x1c` its alpha, `+0x24`): at once its effect, what flies out of it ([`burst`]: debris,
//! billows, a light, embers) and a camera shake (crate::camera::Shakes); then 20 ticks of hits — every moby with
//! health or one the lines test (mode 0x4000), not Ratchet nor class 0xa5a, inside a sphere growing to 6 takes the
//! record's blast damage (`+0x74`: 4) with flags 0x830000 along the line from the centre — then 40 ticks fading.
//!
//! **The blast's effect** ([`Effect`], drawn by [`draw_blast`]): 200 flares on a ring about the normal drifting out over
//! 90 ticks and fading over 55 (`0x3f13d8` / `0x3f1618`, `0x3f1b00`), and the shockwave mesh twice (`0x3f0f18`,
//! `0x3f1180`; the tables the level's overlay carries, [`DomeMesh`]). The blast goes once its last flare has.
//!
//! Not ported: the target held by `0x18b270` (nothing sets it on Aranos), the gravity frames and moving frames of
//! the throw and the arc (`0x326448`'s other gravities, `0x37f538` / `0x37f618`, `0x380e08` / `0x380ec0`: nothing on
//! Aranos; down is `(0, 0, −1)` here), the rumble, the statistics (`0x3f9f70`), the trail, the hit the ground line
//! under a moby delivers (`0x3efda0`'s `0x2de878(.., 6, target)` passes the moby as its hit template).

use crate::hero::items::{row, GRAVITY_BOMB};
use crate::hero::physics::{add, add_rot, cross, dot, len, scale, set_len, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{aim, countdown, mode, Ctx, MobyWorld};

/// The hand's, the bomb's and the blast's classes.
pub const HAND_CLASS: i16 = 0xce3;
pub const BOMB_CLASS: i16 = 0xce4;
pub const BLAST_CLASS: i16 = 0xce5;

/// `gp+0x17c0..+0x17cc`: the throw's reach with nothing aimed at, the least (1e-5), the gravity (a second²) and
/// the speed across (a second).
const REACH: f32 = 12.0;
const LEAST: f32 = f32::from_bits(0x3727_c5ac);
const GRAVITY: f32 = 12.0 / 3600.0;
const SPEED: f32 = 15.0 / 60.0;
/// `0x3257a0`'s limits for the bomb.
const CONE: aim::Cone = aim::Cone { yaw: 0.392_699_1, pitch: 0.785_398_2, range: 11.1, near_pitch: 0.0, near_yaw: 0.0 };
/// `gp+0x17e8` / `+0x17ec`: a bounce keeps this much of the normal speed; a floor is at least this much up.
const RESTITUTION: f32 = 0.25;
const FLOOR: f32 = 0.5;
/// The record's `+0x74`: the blast's damage.
const BLAST_DAMAGE: f32 = 4.0;
/// `t[3]` of the blast's hits (`0x45afdf66`).
const NO_KNOCK_LIMIT: f32 = 5627.987;
/// `0x37dfc0`'s two quads: their FX textures and colours (`gp−0x14e0`).
const MARKER_FX: [usize; 2] = [0x11, 0x12];
const MARKER_RGBA: [u32; 2] = [0x70f0_f0b0, 0x50f0_f0b0];

/// `0x3ef940`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let Some(r) = row(GRAVITY_BOMB) else { return };
    w.mobys[id].pvars.resize(crate::moby::PVAR_BYTES, 0);
    let hand = ctx.host.joint(&w.mobys[id], 0).unwrap_or(w.mobys[id].pos);
    if let Some(a) = ctx.host.anim(&w.mobys[id]) {
        if a.seq_a == a.seq_b && a.flags & 2 != 0 { ctx.host.set_anim(&w.mobys[id], 1, 0, 2); }
    }
    // The aim: in the look stance its wait; else the search.
    let hero_state = ctx.hero.state;
    let mut target = None;
    let look_ok = if matches!(hero_state, 0x1d | 1) {
        countdown(&mut ctx.hero.items.look_wait)
    } else {
        ctx.hero.items.look_wait = 0x28;
        let (sp, cp) = ctx.hero.rot[1].sin_cos();
        let (sy, cy) = ctx.hero.rot[2].sin_cos();
        target = aim::search(w, ctx, CONE, hand, [cy * cp, sy * cp, sp], false);
        true
    };
    if hero_state == 1 { ctx.hero.request_state(0x1d, true); }
    let vel = launch(ctx, hand, target.map(|t| w.mobys[t].pos));
    let found = landing(w, ctx, id, hand, vel);
    {
        let m = &mut w.mobys[id];
        if let Some((p, n)) = found {
            m.set_pv_v(0, p);
            m.set_pv_v(0x10, n);
        }
        m.set_pv_i(0x24, found.is_some() as i32);
    }
    if w.mobys[id].state == 0 {
        w.mobys[id].state = 1;
        w.mobys[id].set_pv_f(0x20, 0.0);
    }
    if w.mobys[id].state == 1 {
        if 0x15 <= ctx.hero.items.use_ticks {
            let idle = ctx.hero.idle_seq();
            ctx.hero.items.use_end(hero_state, ctx.hero_view.seq_b, idle);
        }
        let it = ctx.hero.items;
        if w.mobys[id].pv_f(0x20) == 0.0 && it.no_use == 0 && it.held & it.trigger != 0 && look_ok && 0x17 <= it.item_ticks {
            if ctx.hero.items.take_ammo(ctx.game, 1) {
                ctx.hero.items.use_start(ctx.hero.group);
                ctx.host.set_anim(&w.mobys[id], 2, 0, 2);
                throw(w, ctx, id, hand, vel);
                ctx.host.play_sound(&w.mobys[id], 0, 0);
                w.mobys[id].set_pv_f(0x20, 1.0 / r.rate);
            } else if it.pressed & it.trigger != 0 {
                ctx.host.play_sound(&w.mobys[id], 1, 0);
            }
        }
    }
    // `0x2eeb40(+0x20)`.
    let c = w.mobys[id].pv_f(0x20);
    w.mobys[id].set_pv_f(0x20, if 1.0 / 60.0 < c { c - 1.0 / 60.0 } else { 0.0 });
    w.register_draw(id);
}

/// `0x37e328(least, reach, speed, gravity, out, from, target, 0)` under plain gravity (down `(0, 0, −1)`,
/// `0x326448`): the throw from `from` that comes down on `target` (a moby's place), or ahead of Ratchet — `reach`
/// along his facing, or with L1 held (`0x138320 & 4`) and the item not kept away (`0x18c0dc`) along the camera's
/// look ([`look_aim`]); `speed` across, the rise for the fall.
fn launch(ctx: &Ctx, from: V3, target: Option<V3>) -> V3 {
    let to = target.unwrap_or_else(|| {
        let it = ctx.hero.items;
        let (way, reach) = if it.held & crate::pad::button::L1 != 0 && it.no_use == 0 { look_aim(ctx) } else { (ctx.hero.rows[0], REACH) };
        add(ctx.hero.pos, set_len(way, reach))
    });
    let d = sub(to, from);
    let across = [d[0], d[1], 0.0];
    let mut v = set_len(across, SPEED);
    let t = len(across) / SPEED;
    if 0.0 < t { v[2] = (d[2] + GRAVITY * t * t * 0.5) / t; }
    v
}

/// `0x37e328`'s L1 branch: the way and the reach of the throw from the camera (`0x1b8f70` its rows, `0x1b8d40` its
/// place). The way: the camera's forward and its part across; the quaternion about their cross whose half angle
/// is *half their cosine* (`0x2ef460` / `0x2ef448`, the VU0 sine and cosine: the game's own arithmetic, kept)
/// turns the camera's rows (`0x2efc20` its matrix, `0x2efa00` row by row) and the first row is the way. The
/// reach: where the camera's forward ray reaches the level of Ratchet's feet (when it looks down at all), its
/// distance across from the camera held to `least`..`reach`, else `reach`; then eased by `1 − (1 − x)²`
/// (`0x31b918`).
fn look_aim(ctx: &Ctx) -> (V3, f32) {
    use crate::hero::physics::unit;
    const DOWN: V3 = [0.0, 0.0, -1.0];
    let cam = ctx.host.camera_rows();
    let fwd = unit(cam[0]);
    let across = unit(sub(fwd, scale(DOWN, dot(fwd, DOWN))));
    let axis = cross(fwd, across);
    let half = dot(fwd, across) * 0.5;
    let (s, c) = rc_formats::moby_light::vu0_sin_cos(half.to_bits());
    let a = scale(axis, f32::from_bits(s));
    let row = crate::moby::shards::rotate([a[0], a[1], a[2], f32::from_bits(c)], [1.0, 0.0, 0.0]);
    let way: V3 = std::array::from_fn(|k| row[0] * cam[0][k] + row[1] * cam[1][k] + row[2] * cam[2][k]);
    let height = -dot(ctx.hero.pos, DOWN);
    let fwd = unit(cam[0]);
    let down = dot(DOWN, fwd);
    let mut reach = REACH;
    if 0.0 < down {
        let eye = ctx.host.camera_pos();
        let k = (dot(DOWN, eye) + height) / down;
        let v = sub(sub(eye, scale(fwd, k)), eye);
        let d = len(sub(v, scale(DOWN, dot(v, DOWN))));
        reach = if REACH < d { REACH } else if d < LEAST { LEAST } else { d };
    }
    let x = reach / REACH;
    (way, REACH * (1.0 - (1.0 - x) * (1.0 - x)))
}

/// `0x37e988(gravity, from, vel, ..)`: the arc from `from` at `vel`, ten ticks between line tests (flags 0x12, not the
/// hand), up to 300; where it lands and the surface's normal, the point pulled 6% toward the camera. Nothing while
/// Ratchet dies (0x6f) or in a scene.
fn landing(w: &MobyWorld, ctx: &Ctx, id: usize, from: V3, vel: V3) -> Option<(V3, V3)> {
    if ctx.hero.state == 0x6f || ctx.game.scenes.mode != crate::scene::PLAY { return None; }
    let (mut p, mut v) = (from, vel);
    for _ in 0..300 {
        let a = p;
        for _ in 0..10 {
            p = add(p, v);
            v[2] -= GRAVITY;
        }
        if let Some(h) = w.coll_line(ctx, a, p, 0x12, Some(id)) {
            let n = crate::hero::physics::unit(h.normal);
            let cam = ctx.host.camera_pos();
            return Some((add(cam, scale(sub(h.point, cam), 0.94)), n));
        }
    }
    None
}

/// `0x3f0190`: the bomb (class 0xce4) at the hand with the throw's velocity, two spins of 3°..6° a tick, a hundredth
/// of its size.
fn throw(w: &mut MobyWorld, ctx: &mut Ctx, hand: usize, at: V3, vel: V3) {
    if w.class(BOMB_CLASS).is_none() { return; }
    let id = w.create(BOMB_CLASS);
    let g = &mut ctx.game.rng;
    let spins = [g.randf(0.052_359_88, 0.104_719_76), g.randf(0.052_359_88, 0.104_719_76)];
    let m = &mut w.mobys[id];
    m.update = Some(update_bomb);
    m.mode &= !mode::NO_UPDATE;
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.drawn = 1;
    m.state = 0;
    m.pos = at;
    m.scale = 0.01;
    m.parent = Some(hand);
    m.set_pv_v(0, vel);
    m.set_pv_f(0x10, spins[0]);
    m.set_pv_f(0x14, spins[1]);
    m.set_pv_i(0x18, 0);
    m.set_pv_i(0x30, hand as i32);
    w.rebuild(id, &*ctx.host);
    w.changed = true;
}

/// What the sweep found.
enum Sweep {
    None,
    /// Off a wall (the velocity bounced).
    Bounce,
    /// Stopped on a floor or a target.
    Stop,
}

/// `0x37edb8` for the bomb: the 0.25 sphere at `to` (its pushed-out centre), else the line `from` → `to` (its point),
/// not the bomb nor Ratchet nor another bomb. The normal under 0.5 up bounces `vel` (reflected, a quarter of the
/// normal speed kept); else it stops. Returns the place, the surface's normal, the moby touched and the query's own
/// point (`0x1bf1a0`).
fn sweep(w: &MobyWorld, ctx: &Ctx, id: usize, from: V3, to: V3, vel: &mut V3) -> (Sweep, V3, V3, Option<usize>, V3) {
    let Some(c) = ctx.coll else { return (Sweep::None, to, [0.0; 3], None, to) };
    let q = crate::collision::QueryFlags(0);
    let hit = crate::collision::coll_sphere_m(c, ctx.scene, to, 0.25, q, Some(id)).map(|h| (h.pushed_centre.unwrap_or(h.point), h))
        .or_else(|| w.coll_line(ctx, from, to, 0, Some(id)).map(|h| (h.point, h)));
    let Some((at, h)) = hit else { return (Sweep::None, to, [0.0; 3], None, to) };
    if h.moby.is_some_and(|m| m == id || Some(m) == ctx.hero_moby || w.mobys.get(m).is_some_and(|x| x.o_class == HAND_CLASS || x.o_class == 0)) {
        return (Sweep::None, to, [0.0; 3], None, to);
    }
    let n = crate::hero::physics::unit(h.normal);
    if dot([0.0, 0.0, 1.0], n) < FLOOR {
        // `0x2ef2a8`: reflected; then the normal part a quarter.
        let r = sub(*vel, scale(n, 2.0 * dot(*vel, n)));
        let along = scale(n, dot(r, n));
        *vel = add(sub(r, along), scale(along, RESTITUTION));
        return (Sweep::Bounce, at, n, h.moby, h.point);
    }
    (Sweep::Stop, h.point, n, h.moby, h.point)
}

/// `0x3efda0`.
pub fn update_bomb(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 | 1 => {
            w.mobys[id].state = 1;
            let from = w.mobys[id].pos;
            let mut vel = w.mobys[id].pv_v(0);
            let to = add(from, vel);
            vel[2] -= GRAVITY;
            let (s, at, n, touched, point) = sweep(w, ctx, id, from, to, &mut vel);
            let m = &mut w.mobys[id];
            m.set_pv_v(0, vel);
            match s {
                Sweep::None => m.pos = to,
                Sweep::Bounce => {
                    m.pos = at;
                    m.set_pv_v(0x20, n);
                }
                Sweep::Stop => {
                    m.pos = at;
                    m.set_pv_v(0x20, n);
                    m.state = 2;
                }
            }
            // A moby of class type 5 or 9, or one the lines test (mode 0x4000), stops it: at the query's point, then
            // the line 3 down from there (raised by the moby's aim height when it has one; flags 6) gives the place
            // and the normal it blows up with, so the blast lies on the ground under it.
            if let Some(t) = touched {
                let kind = w.mobys.get(t).and_then(|x| w.class(x.o_class)).map_or(0, |c| c.kind);
                if kind == 5 || kind == 9 || w.mobys.get(t).is_some_and(|x| x.mode & mode::LINE != 0) {
                    let up = [0.0, 0.0, 1.0];
                    let start = match aim::health(w, t) {
                        Some((_, h)) => add(w.mobys[id].pos, scale(up, h)),
                        None => w.mobys[id].pos,
                    };
                    let (mut p, mut nn) = (point, n);
                    if let Some(h) = w.coll_line(ctx, start, add(start, scale(up, -3.0)), 6, None) {
                        (p, nn) = (h.point, crate::hero::physics::unit(h.normal));
                    }
                    let m = &mut w.mobys[id];
                    m.pos = p;
                    m.set_pv_v(0x20, nn);
                    m.state = 2;
                }
            }
            let m = &mut w.mobys[id];
            m.rot[0] = add_rot(m.rot[0], m.pv_f(0x10));
            m.rot[1] = add_rot(m.rot[1], m.pv_f(0x14));
            // `0x31d930(0.2, bomb)`: its blob shadow, after the spins and before the bounds.
            crate::shadows::request(w, ctx, 0.2, id);
            let p = w.mobys[id].pos;
            if p.iter().any(|&c| !(2.0..=1021.0).contains(&c)) {
                w.delete(id);
                return;
            }
            w.rebuild(id, &*ctx.host);
            w.changed = true;
        }
        2 => {
            ctx.host.play_sound(&w.mobys[id], 0, 0);
            let (at, n, hand) = (w.mobys[id].pos, w.mobys[id].pv_v(0x20), w.mobys[id].pv_link(0x30));
            blast(w, ctx, at, n, hand);
            w.delete(id);
        }
        _ => {}
    }
}

/// `0x3f0860`: the blast (class 0xce5) at `at` with the surface's normal; the last one goes.
fn blast(w: &mut MobyWorld, ctx: &mut Ctx, at: V3, n: V3, hand: Option<usize>) {
    if let Some(old) = w.mobys.iter().position(|m| m.o_class == BLAST_CLASS && m.state < 0x80) { w.delete(old); }
    let id = w.create(BLAST_CLASS);
    let m = &mut w.mobys[id];
    m.update = Some(update_blast);
    m.mode = (m.mode & !mode::NO_UPDATE) | mode::KEEP_ROWS;
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.state = 0;
    m.pos = at;
    m.parent = hand;
    m.set_pv_v(0, n);
    // Its rows: Ratchet's turned so that the third is up.
    let mut rows = crate::hero::physics::euler_rows(ctx.hero.rot);
    let axis = cross(rows[2], [0.0, 0.0, 1.0]);
    if 1e-6 < len(axis) {
        let a = dot(rows[2], [0.0, 0.0, 1.0]).clamp(-1.0, 1.0).acos();
        rows = rows.map(|r| turn(r, crate::hero::physics::unit(axis), a));
    }
    m.rows = rows;
    w.changed = true;
}

/// `v` turned by `a` about the unit axis `k`.
fn turn(v: V3, k: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    let x = cross(k, v);
    let d = dot(k, v);
    std::array::from_fn(|i| v[i] * c + x[i] * s + k[i] * d * (1.0 - c))
}

/// `0x3f0310`.
pub fn update_blast(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 {
        let m = &mut w.mobys[id];
        m.state = 1;
        m.set_pv_f(0x10, 0.0);
        m.set_pv_f(0x18, 0.666_666_7);
        m.set_pv_i(0x1c, 0x7f);
        m.set_pv_i(0x24, 0x6f);
        m.set_pv_i(0x14, 0x14);
        let (at, n) = (m.pos, m.pv_v(0));
        effect_start(w, ctx, id, at, n);
        burst(w, ctx, id, at);
        // The camera shake (up and down, 0.45 over 40 ticks) when it is in view within 100.
        if !ctx.view.is_some_and(|v| v.culled(100.0, [at[0], at[1], at[2] + 1.0, 2.0])) {
            let s = &mut ctx.game.shakes.0[0];
            (s.amp, s.timer, s.max) = (0.45, 0x28, 0);
        }
    }
    effect_tick(w, id);
    w.register_draw(id);
    let m = &mut w.mobys[id];
    m.set_pv_f(0x10, m.pv_f(0x10) + m.pv_f(0x18));
    match m.state {
        1 => {
            m.set_pv_i(0x24, (m.pv_i(0x24) - 5).max(0));
            let r = (1.0 - m.pv_i(0x14) as f32 / 20.0) * 6.0 + 0.001;
            let at = m.pos;
            if let Some(sc) = ctx.scene {
                for t in crate::collision::coll_sphere_mobys(sc, at, r, crate::collision::QueryFlags(0), Some(id)) {
                    let Some(x) = w.mobys.get(t) else { continue };
                    if !(aim::health(w, t).is_some() || x.mode & mode::LINE != 0) || Some(t) == ctx.hero_moby || x.o_class == 0xa5a { continue; }
                    let d = crate::hero::physics::unit(sub(x.pos, at));
                    let mut tm = HitTemplate::new(BLAST_DAMAGE, Some(id), 0x0083_0000, [d[0], d[1], d[2], NO_KNOCK_LIMIT]);
                    tm.kind = (x.o_class as u16 as u32) << 16 | 0x0305;
                    let p = x.pos;
                    w.deliver_hit_any(t, &tm, [p[0], p[1], p[2], 1.0]);
                }
            }
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x14);
            if countdown(&mut t) {
                m.state = 2;
                t = 0x28;
            }
            m.set_pv_i(0x14, t);
        }
        2 => {
            let mut t = m.pv_i(0x14);
            let f = t as f32;
            if !countdown(&mut t) {
                m.set_pv_i(0x1c, (127.0 * f * 0.025) as i32);
            } else if m.pv_i(0x1c) == 0 {
                m.state = 6;
            } else {
                m.set_pv_i(0x1c, m.pv_i(0x1c) - 1);
            }
            m.set_pv_i(0x14, t);
        }
        // 6: gone with its effect's last flares.
        _ => {
            if w.gravity_bomb.blast != Some(id) || w.gravity_bomb.flares.is_empty() {
                if w.gravity_bomb.blast == Some(id) { w.gravity_bomb.blast = None; }
                w.delete(id);
            }
        }
    }
}

/// `0x3f0310`'s colours for the billows (`0x1b07f0` / `0x1b0808`).
const BILLOW_FROM: [u32; 6] = [0x4f00_8fff, 0x4f00_8fff, 0x4f00_7fff, 0x4f00_6fff, 0x2fff_ffff, 0x2fff_ffff];
const BILLOW_TO: [u32; 6] = [0x2f00_5f7f, 0x2f00_4f7f, 0x2f00_3f7f, 0x2f00_004f, 0x2f00_0000, 0x3f00_0000];

/// `0x3f09e0(blast, at)`: what flies out of the blast, every way turned by the blast's rows (third up):
/// * a chunk of debris (kind 0) toward the camera, 1/300 of its distance a random way plus twice its distance a
///   second toward it lifted by half the distance, at most 10 a second; 9 smoking ones (kind 1) out at 25°..45° up,
///   13..20 a second; each 60..90 ticks (crate::moby::classes::debris);
/// * 10 billows (crate::particles::billow), 12..15 a second, 20..40 ticks, from blue or white to dark blue or black;
/// * the light (crate::moby::classes::light::GRAVITY_BOMB);
/// * 50 embers (crate::particles::ember) out at 8°..35° up, 16..30 a second, 50..110 ticks, orange to dark red, trailing.
fn burst(w: &mut MobyWorld, ctx: &mut Ctx, id: usize, at: V3) {
    use super::{debris, light, shot::polar};
    let rows = w.mobys[id].rows;
    let through = |v: V3| -> V3 { std::array::from_fn(|k| v[0] * rows[0][k] + v[1] * rows[1][k] + v[2] * rows[2][k]) };
    let mut d = sub(ctx.host.camera_pos(), at);
    let dist = len(d);
    let g = &mut ctx.game.rng;
    let r = [g.randf(-1.0, 1.0), g.randf(-1.0, 1.0), g.randf(-1.0, 1.0)];
    d[2] += dist * 0.5;
    let mut v = add(set_len(r, dist * 0.003_333_333_6), set_len(d, (dist + dist) * 0.016_666_668));
    // `0x2ef1d0(1/6, v, v)`: at most that long.
    let l = len(v);
    if 0.0 < l && 0.166_666_67 <= l { v = scale(v, 0.166_666_67 / l); }
    let life = g.rand_range(0x3c, 0x5a);
    debris::spawn(w, ctx, at, through(v), life, 0);
    for _ in 0..9 {
        let g = &mut ctx.game.rng;
        let pitch = g.randf(0.436_332_3, 0.785_398_2);
        let yaw = g.rand_angle();
        let speed = g.randf(13.0, 20.0);
        let v = through(polar(speed * 0.016_666_668, yaw, pitch));
        let life = g.rand_range(0x3c, 0x5a);
        debris::spawn(w, ctx, at, v, life, 1);
    }
    for _ in 0..10 {
        let g = &mut ctx.game.rng;
        let speed = g.randf(8.0, 10.0) * 0.016_666_668 * 1.5;
        let (a, b) = (g.randi(6) as usize, g.randi(6) as usize);
        let life = g.rand_range(0x14, 0x28);
        let hold = g.rand_range(0x32, 0x46) as u8;
        if let Some(s) = ctx.parts.as_deref_mut() {
            crate::particles::billow(s, &mut ctx.game.rng, 600_000.0, speed, at, [0.0; 3], BILLOW_FROM[a], BILLOW_TO[b], life, hold, 0, 0);
        }
    }
    light::spawn(w, ctx, &light::GRAVITY_BOMB, at);
    for _ in 0..50 {
        let g = &mut ctx.game.rng;
        let pitch = g.randf(0.139_626_3, 0.610_865_3);
        let yaw = g.rand_angle();
        let speed = g.randf(16.0, 30.0);
        let v = through(polar(speed * 0.016_666_668, yaw, pitch));
        let life = g.rand_range(0x3c, 0x78) - 10;
        if let Some(s) = ctx.parts.as_deref_mut() {
            crate::particles::ember(s, &mut ctx.game.rng, 40_000.0, at, v, 0x4f00_7fff, 0x1f00_007f, life, 1, None, None, 0);
        }
    }
}

/// `0x3efd68` → `0x37dfc0(1, landing, normal, 0)`: the landing marker, two quads in the surface's plane (FX 0x11 and
/// 0x12, ALPHA 0x44), ±1 across, turning 2° a frame the one way and the other; not in the states 0x17 / 0x18 / 0x19 /
/// 0x1a / 0x1b / 0x6f / 0x62.
pub fn draw_marker(w: &MobyWorld, id: usize, hero_state: i32, frame: i32, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    if m.pv_i(0x24) == 0 || matches!(hero_state, 0x17..=0x1b | 0x6f | 0x62) { return; }
    let (at, n) = (m.pv_v(0), m.pv_v(0x10));
    // Its frame: the second axis along the normal.
    let pitch = add_rot(-crate::hero::physics::atan2((n[0] * n[0] + n[2] * n[2]).sqrt(), n[1]), std::f32::consts::FRAC_PI_2);
    let yaw = crate::hero::physics::atan2(n[2], n[0]);
    let rows = crate::hero::physics::euler_rows([pitch, yaw, 0.0]);
    let spin0 = {
        let f = (frame % 180) as f32 * 0.034_906_59 * 0.159_154_94 + 0.5;
        ((f - f.floor()) - 0.5) * std::f32::consts::TAU
    };
    for k in 0..2 {
        let s = if k == 0 { spin0 } else { -spin0 };
        let (sn, cs) = s.sin_cos();
        let corner = |x: f32, z: f32| {
            let (x, z) = (x * cs + z * sn, z * cs - x * sn);
            add(at, add(scale(rows[0], x), scale(rows[2], z)))
        };
        let c = MARKER_RGBA[k];
        out.push(crate::fx::FxQuad {
            fx: MARKER_FX[k],
            additive: false,
            corners: [corner(-1.0, 1.0), corner(1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0)],
            st: [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            rgba: [c; 4],
        });
    }
}

/// One of the blast's flares (`0x1b2b38`, 320 records of 0x80 bytes): a camera-facing sprite (FX 0x2e) that drifts
/// out from the centre in the surface's plane, growing, its colour easing from orange to nearly black, then fading.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flare {
    /// `+0x00` the place, `+0x10` the way out, `+0x24` the reach out a tick.
    pos: V3,
    dir: V3,
    out: f32,
    /// `+0x28 / +0x2c / +0x30`: the roll's speed from, now and to; `+0x34` the roll.
    spin: [f32; 3],
    roll: f32,
    /// `+0x38 / +0x3c / +0x40`: the rise along the normal a tick; `+0x44 / +0x48 / +0x4c` the size.
    rise: [f32; 3],
    size: [f32; 3],
    /// `+0x54 / +0x58`: the ticks left of the phase and its length.
    left: i32,
    ticks: i32,
    /// `+0x5c / +0x60`: the alpha from and now; `+0x64 / +0x68 / +0x6c` the colour from, now and to.
    alpha0: i32,
    alpha: i32,
    colour: [u32; 3],
    /// `+0x70`: 2 additive (ALPHA (Cs − 0)·As + Cd), 1 blended.
    blend: u8,
    /// `+0x74`: 0 growing, 1 fading, 2 going; `+0x78` its kind (2: the blast's).
    phase: u8,
    kind: u8,
}

/// The blast's mesh (the 64 quads of `{vertex, ST}` pairs at L00's `0x29b400`, the vertices `0x29af00`, the STs
/// `0x29b800`; the same tables in every level's overlay, at that level's addresses), drawn twice as its shockwave.
#[derive(Clone, Debug, PartialEq)]
pub struct DomeMesh {
    pub verts: Vec<V3>,
    pub st: Vec<[f32; 2]>,
    pub quads: Vec<[(u16, u16); 4]>,
}

impl DomeMesh {
    /// From the level's overlay through `read(address, length)` of L00's addresses (the engine maps them to the
    /// level's copy of the blast's draw, `0x3f0f18`'s `lui`/`addiu` pairs).
    pub fn read(read: impl Fn(u32, usize) -> Option<Vec<u8>>) -> Option<DomeMesh> {
        let q = read(0x0029_b400, 64 * 16)?;
        let half = |b: &[u8], i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        let quads: Vec<[(u16, u16); 4]> = (0..64).map(|i| std::array::from_fn(|k| (half(&q, i * 8 + 2 * k), half(&q, i * 8 + 2 * k + 1)))).collect();
        let nv = quads.iter().flatten().map(|&(v, _)| v as usize + 1).max()?;
        let ns = quads.iter().flatten().map(|&(_, s)| s as usize + 1).max()?;
        let f = |b: &[u8], i: usize| f32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        let v = read(0x0029_af00, nv * 16)?;
        let s = read(0x0029_b800, ns * 8)?;
        Some(DomeMesh { verts: (0..nv).map(|i| [f(&v, 4 * i), f(&v, 4 * i + 1), f(&v, 4 * i + 2)]).collect(), st: (0..ns).map(|i| [f(&s, 2 * i), f(&s, 2 * i + 1)]).collect(), quads })
    }
}

/// The blast's effect (`0x1b2b34`: one group of 0x50 bytes, its flares in the pool), and the mesh (set by the engine).
#[derive(Clone, Debug, Default)]
pub struct Effect {
    /// `+0x30`: the blast it belongs to; `+0x34` its countdown; `+0x00` the normal, `+0x10` the plane's axis, `+0x20`
    /// the centre.
    blast: Option<usize>,
    left: i32,
    normal: V3,
    axis: V3,
    centre: V3,
    flares: Vec<Flare>,
    pub mesh: Option<std::sync::Arc<DomeMesh>>,
}

/// The blast's start (`0x3f0310` state 0 clears the pool and the group) and `0x3f13d8(1/6, blast, 20, at, n)`: the
/// group and its 200 flares, two at each of 100 angles 3.6° apart from a random one (`0x3f1618` kinds 2 and 0).
fn effect_start(w: &mut MobyWorld, ctx: &mut Ctx, id: usize, at: V3, n: V3) {
    let e = &mut w.gravity_bomb;
    e.blast = Some(id);
    e.left = 0x14;
    e.normal = n;
    e.centre = at;
    e.axis = cross(n, at);
    e.flares.clear();
    let g = &mut ctx.game.rng;
    let mut a = g.rand_angle();
    for _ in 0..100 {
        for kind in [2u8, 0] {
            let alpha = g.rand_range(0x5f, 0x7f);
            g.rand();
            let dir = rotate(e.axis, a, n);
            let grey = g.rand_range(0, 3) as u32 * 0x0001_0101;
            let s0 = g.randf(0.15, 1.25) * 0.125;
            let s1 = g.randf(0.5, 1.8) * 0.7;
            let pos = add(add(at, set_len(n, 0.1)), set_len(dir, 0.75));
            let spin1 = g.randf_signed(0.052_359_88, 0.104_719_76);
            let spin0 = g.randf_signed(0.314_159_3, 0.356_047_2);
            let roll = g.rand_angle();
            e.flares.push(Flare {
                pos,
                dir,
                out: if kind == 0 { 0.075 } else { 0.208_333_3 },
                spin: [spin0, spin0, spin1],
                roll,
                rise: [0.0; 3],
                size: [s0, s0, s1],
                left: 0x5a,
                ticks: 0x5a,
                alpha0: alpha,
                alpha,
                colour: [0x0010_3080, 0x0010_3080, grey],
                blend: 2,
                phase: 0,
                kind: 2,
            });
        }
        a = add_rot(a, 0.063_466_52);
    }
}

/// `0x3f1520` / `0x3f1b00`: the group's countdown (free once out with no flares left) and each flare's tick.
fn effect_tick(w: &mut MobyWorld, id: usize) {
    let e = &mut w.gravity_bomb;
    if e.blast != Some(id) { return; }
    if countdown(&mut e.left) && e.flares.is_empty() { e.blast = None; }
    let n = e.normal;
    e.flares.retain_mut(|f| {
        let t = 1.0 - f.left as f32 / f.ticks.max(1) as f32;
        let lerps = |f: &mut Flare| {
            f.colour[1] = crate::hud::lerp_rgba(t, f.colour[0], f.colour[2]);
            f.spin[1] = f.spin[0] + (f.spin[2] - f.spin[0]) * t;
            f.rise[1] = f.rise[0] + (f.rise[2] - f.rise[0]) * t;
            f.size[1] = f.size[0] + (f.size[2] - f.size[0]) * t;
        };
        let push = match f.phase {
            0 => {
                lerps(f);
                set_len(f.dir, f.out * (1.0 - (1.0 - t) * (1.0 - t)))
            }
            1 => {
                f.alpha = (f.alpha0 as f32 - f.alpha0 as f32 * t * t) as i32;
                if f.kind != 2 {
                    lerps(f);
                    set_len(f.dir, f.out * (1.0 - (1.0 - t) * (1.0 - t)))
                } else {
                    set_len(f.dir, f.out)
                }
            }
            _ => [0.0; 3],
        };
        f.pos = add(f.pos, set_len(n, f.rise[1]));
        if f.kind != 1 { f.pos = add(f.pos, push); }
        f.roll = add_rot(f.roll, f.spin[1]);
        if countdown(&mut f.left) {
            if f.phase == 0 {
                f.phase = 1;
                f.left = if f.kind == 1 { 0x1e } else { 0x37 };
                f.ticks = f.left;
            } else if f.alpha < 1 {
                return false;
            } else {
                f.phase = 2;
                f.alpha -= 1;
            }
        }
        true
    });
}

/// `0x321c00(angle, out, v, axis)`: `v` turned `angle` about `axis`.
fn rotate(v: V3, angle: f32, axis: V3) -> V3 {
    if angle.abs() < 1e-5 { return v; }
    turn(v, crate::hero::physics::unit(axis), angle)
}

/// `0x3f0e80`: in view (100, a sphere of 2), the shockwave twice (`0x3f0f18`: the mesh 2·size across and a quarter
/// of the size high; `0x3f1180`: the size across and an eighth high; FX 0x2b additive, `0x204080` at the blast's
/// `+0x24`, on its rows a quarter up) and the flares (`0x3f1f90`: FX 0x2e, ±size, turned by their roll to face the
/// camera, a quarter toward it).
pub fn draw_blast(w: &MobyWorld, id: usize, view: &crate::particles::BSphereView, cam: V3, out: &mut Vec<crate::fx::FxQuad>) {
    let m = &w.mobys[id];
    if view.culled(100.0, [m.pos[0], m.pos[1], m.pos[2], 2.0]) { return; }
    let size = m.pv_f(0x10);
    let rgba = 0x0020_4080 | (m.pv_i(0x24).clamp(0, 0xff) as u32) << 24;
    let base = add(m.pos, [0.0, 0.0, 0.25]);
    if let Some(mesh) = &w.gravity_bomb.mesh {
        for k in [[size * 2.0, size * 2.0, size * 0.25], [size, size, size * 0.125]] {
            for q in &mesh.quads {
                let corner = |i: usize| {
                    let v = mesh.verts.get(q[i].0 as usize).copied().unwrap_or_default();
                    let v = [v[0] * k[0], v[1] * k[1], v[2] * k[2]];
                    add(base, add(add(scale(m.rows[0], v[0]), scale(m.rows[1], v[1])), scale(m.rows[2], v[2])))
                };
                let st = |i: usize| mesh.st.get(q[i].1 as usize).copied().unwrap_or_default();
                out.push(crate::fx::FxQuad { fx: 0x2b, additive: true, corners: std::array::from_fn(corner), st: std::array::from_fn(st), rgba: [rgba; 4] });
            }
        }
    }
    let e = &w.gravity_bomb;
    if e.blast != Some(id) { return; }
    for f in &e.flares {
        let d = sub(cam, f.pos);
        let yaw = crate::hero::physics::atan2(d[0], d[1]);
        let pitch = -crate::hero::physics::atan2((d[0] * d[0] + d[1] * d[1]).sqrt(), d[2]);
        let rows = crate::hero::physics::euler_rows([f.roll, pitch, yaw]);
        let at = add(f.pos, set_len(d, 0.25));
        let s = f.size[1];
        let corner = |y: f32, z: f32| add(at, add(scale(rows[1], y * s), scale(rows[2], z * s)));
        let c = f.colour[1] & 0xff_ffff | (f.alpha.clamp(0, 0xff) as u32) << 24;
        out.push(crate::fx::FxQuad {
            fx: 0x2e,
            additive: f.blend == 2,
            corners: [corner(-1.0, -1.0), corner(-1.0, 1.0), corner(1.0, -1.0), corner(1.0, 1.0)],
            st: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
            rgba: [c; 4],
        });
    }
}
