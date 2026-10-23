//! The Gravity Bomb (item 0x2a, crate::hero::items::ROWS): the hand moby 0xce3 (L00 `0x3ef940`, called by the item pass
//! while it is the hand item), the bomb it throws 0xce4 (`0x3efda0`) and its blast 0xce5 (`0x3f0310`).
//!
//! **The hand** (pvars: `+0x00` the landing point, `+0x10` its normal, `+0x20` the cooldown in seconds, `+0x24` a
//! landing found): each tick the aim — out of the look stance, the cone search (crate::moby::aim, π/8 × π/4 to 11.1
//! from the hand along Ratchet's facing, no crates) — and the throw that reaches it ([`launch`], `0x37e328`: 15 a
//! second across, the rise worked out for the gravity 12 a second²; with nothing aimed at, 12 ahead), then the arc
//! followed to where it lands ([`landing`], `0x37e988`) for the marker ([`draw_marker`], `0x37dfc0`). Its sequence 2
//! run out: back to 1. In hand 23 ticks, the cooldown out and ○ held (in the look stance after its wait): one bomb
//! (`0x2d42d0`), the use stance (crate::hero::items::ItemBlock::use_start: Ratchet's sequence 0x36, both hands),
//! sequence 2, class sound 0, the bomb thrown ([`throw`], `0x3f0190`) and the cooldown 1 / rate (2 s); out of bombs, a
//! press plays class sound 1. The use ends after 21 ticks in use (`0x2af848`).
//!
//! **The bomb** (pvars: `+0x00` its velocity, `+0x10` / `+0x14` its spins, `+0x20` the hit's normal, `+0x30` the hand):
//! each tick a step of the arc and the sweep ([`sweep`], `0x37edb8`: a 0.25 sphere at the new place, else the line
//! from the old); a wall (normal under 0.5 up) bounces it (its normal speed reversed at a quarter), a floor stops it
//! there, as does any moby that is a target; stopped, it plays class sound 0, blows up ([`blast`], `0x3f0860`) and
//! goes. It spins (`+0xf0` / `+0xf4`) and is gone out of the world's bounds (2..1021).
//!
//! **The blast** (one at a time: a new one deletes the last; pvars `+0x00` the normal, `+0x10` its size, `+0x14` its
//! timer, `+0x18` the growth, `+0x1c` its alpha, `+0x24`): 20 ticks of hits — every moby with health or a target, not
//! Ratchet nor class 0xa5a, inside a sphere growing to 6 takes the record's blast damage (`+0x74`: 4) with flags
//! 0x830000 along the line from the centre — then 40 ticks fading.
//!
//! Not ported: the target held by `0x18b270` (nothing sets it on Aranos), the aim with L1 (`0x37e328`'s camera branch),
//! the gravity frames and moving frames of the arc (`0x37f538` / `0x37f618`, `0x380e08` / `0x380ec0`: nothing on
//! Aranos), the rumble, the statistics (`0x3f9f70`), the trail, the blast's effect (its arcs `0x3f13d8` / `0x3f1b00`,
//! debris, puffs, sparks and light `0x3f09e0`, its draw `0x3f0e80`) and its camera shake (`0x1b8d60`), the bomb's
//! explosion on a target's own line (`0x3efda0`'s `0x2de878(.., 6, target)`: here at the sweep's point).

use crate::hero::items::{row, GRAVITY_BOMB};
use crate::hero::physics::{add, add_rot, cross, dot, len, scale, set_len, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{aim, countdown, mode, Ctx, MobyWorld};

/// The hand's, the bomb's and the blast's classes.
pub const HAND_CLASS: i16 = 0xce3;
pub const BOMB_CLASS: i16 = 0xce4;
pub const BLAST_CLASS: i16 = 0xce5;

/// `gp+0x17c0..+0x17cc`: the throw's reach with nothing aimed at, the least, the gravity (a second²) and the speed
/// across (a second).
const REACH: f32 = 12.0;
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

/// `0x37e328(least, reach, speed, gravity, out, from, target, 0)` under plain gravity: the throw from `from` that comes
/// down on `target` (a moby's place), or `reach` ahead along Ratchet's facing; `speed` across, the rise for the fall.
fn launch(ctx: &Ctx, from: V3, target: Option<V3>) -> V3 {
    let to = target.unwrap_or_else(|| add(ctx.hero.pos, set_len(ctx.hero.rows[0], REACH)));
    let d = sub(to, from);
    let across = [d[0], d[1], 0.0];
    let mut v = set_len(across, SPEED);
    let t = len(across) / SPEED;
    if 0.0 < t { v[2] = (d[2] + GRAVITY * t * t * 0.5) / t; }
    v
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
/// normal speed kept); else it stops. Returns the place, the surface's normal and the moby touched.
fn sweep(w: &MobyWorld, ctx: &Ctx, id: usize, from: V3, to: V3, vel: &mut V3) -> (Sweep, V3, V3, Option<usize>) {
    let Some(c) = ctx.coll else { return (Sweep::None, to, [0.0; 3], None) };
    let q = crate::collision::QueryFlags(0);
    let hit = crate::collision::coll_sphere_m(c, ctx.scene, to, 0.25, q, Some(id)).map(|h| (h.pushed_centre.unwrap_or(h.point), h))
        .or_else(|| w.coll_line(ctx, from, to, 0, Some(id)).map(|h| (h.point, h)));
    let Some((at, h)) = hit else { return (Sweep::None, to, [0.0; 3], None) };
    if h.moby.is_some_and(|m| m == id || Some(m) == ctx.hero_moby || w.mobys.get(m).is_some_and(|x| x.o_class == HAND_CLASS || x.o_class == 0)) {
        return (Sweep::None, to, [0.0; 3], None);
    }
    let n = crate::hero::physics::unit(h.normal);
    if dot([0.0, 0.0, 1.0], n) < FLOOR {
        // `0x2ef2a8`: reflected; then the normal part a quarter.
        let r = sub(*vel, scale(n, 2.0 * dot(*vel, n)));
        let along = scale(n, dot(r, n));
        *vel = add(sub(r, along), scale(along, RESTITUTION));
        return (Sweep::Bounce, at, n, h.moby);
    }
    (Sweep::Stop, h.point, n, h.moby)
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
            let (s, at, n, touched) = sweep(w, ctx, id, from, to, &mut vel);
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
            // A target touched stops it there.
            if let Some(t) = touched {
                let target = w.mobys.get(t).is_some_and(|x| x.mode & mode::TARGET != 0) || aim::health(w, t).is_some();
                if target { w.mobys[id].state = 2; }
            }
            let m = &mut w.mobys[id];
            m.rot[0] = add_rot(m.rot[0], m.pv_f(0x10));
            m.rot[1] = add_rot(m.rot[1], m.pv_f(0x14));
            let p = m.pos;
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
    }
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
                    if !(aim::health(w, t).is_some() || x.mode & mode::TARGET != 0) || Some(t) == ctx.hero_moby || x.o_class == 0xa5a { continue; }
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
        // 6: its effect's last arcs gone (the effect is not ported: at once).
        _ => w.delete(id),
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
