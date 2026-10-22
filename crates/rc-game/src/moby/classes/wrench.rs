//! The wrench, class 0x47 (L00 `0x38d318`, called by the item pass `0x2b2060`): in the hand (state 0) its sequence
//! follows Ratchet's, and during a swing's hit window it sweeps the segment from Ratchet's joint 0 to its own head
//! (joint 1, reach + 0.17) since the last tick (`0x31d648`, 5 lines) and tests a sphere at the head (0.35; the jump
//! attack 0.47) against the mobys, with the item's damage (`0x262560` / `0x262564` of its row: 1 / 2); thrown (states
//! 10 out, 11 back) it flies along the throw, comes back to the hand and hits what its spheres touch. R&C1's wrench
//! update (crate::base::hero::melee, `0x2be1c0`) is the map.
//!
//! **pvars**: +0x40 the throw's direction, +0x60 its speed, +0x64 the speed's change, +0x6c the wall-hit timer,
//! +0x70 the trail, +0x74 the swing counter (s16), +0x76 the "skimmed the floor" flag (s16), +0x7c the trail's swing.
//!
//! **Sounds** (its class sounds, once a swing or a throw: `0x18aa1e`, `MeleeBlock::wall_hit`): 2 when it strikes a wall
//! or the floor, else on a moby [`hit_sound`]'s 0 or 1 (none on a moby whose health block says so); 0xb looped while
//! it flies (started as it leaves the hand, stopped when it is caught).
//!
//! Not ported: the sparks (`0x38ce80`), the trail (`0x323c90`, `+0x70`), the look stance marker (`0x2db498`), the glow
//! (`0x2e7920`), the gravity frames, the floor skim of the throw (`0x31ce20`), the jump attack's floor strike
//! (`0x2b3aa0`: its sound 2 and sparks).

use crate::hero::items::Throw;
use crate::hero::melee::{HIT_FROM, HIT_TO, SWINGS};
use crate::hero::physics::{add, atan2, dist, len, rot_dist, scale, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{Ctx, Moby, MobyWorld};

/// The wrench's damage (row +0x60) and the jump attack's (+0x64).
const DAMAGE: f32 = 1.0;
const DAMAGE_JUMP: f32 = 2.0;
/// `t[3]` of the wrench's templates.
const NO_KNOCK_LIMIT: f32 = 5627.987;

fn pv_f(m: &Moby, o: usize) -> f32 { m.pvars.get(o..o + 4).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])) }
fn set_pv_f(m: &mut Moby, o: usize, v: f32) {
    if m.pvars.len() < o + 4 { m.pvars.resize(o + 4, 0); }
    m.pvars[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn pv_v(m: &Moby, o: usize) -> V3 { [pv_f(m, o), pv_f(m, o + 4), pv_f(m, o + 8)] }
fn set_pv_v(m: &mut Moby, o: usize, v: V3) { for k in 0..3 { set_pv_f(m, o + 4 * k, v[k]); } }
fn pv_s(m: &Moby, o: usize) -> i16 { m.pvars.get(o..o + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]])) }
fn set_pv_s(m: &mut Moby, o: usize, v: i16) {
    if m.pvars.len() < o + 2 { m.pvars.resize(o + 2, 0); }
    m.pvars[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

/// `0x2ba610`'s part on the wrench: state 10, the direction (the facing; the floor skim flag when Ratchet's floor is
/// steep and close), speed 0.383, its change 0.
pub fn start_throw(m: &mut Moby, t: Throw) {
    m.state = 10;
    set_pv_v(m, 0x40, t.dir);
    set_pv_f(m, 0x60, t.push);
    set_pv_f(m, 0x64, 0.0);
}

/// `0x31fa20(moby)`: the health block of a moby the enemy framework runs (mode 0x20; its pvars' word 0 points at it).
fn health(w: &MobyWorld, m: Option<usize>) -> Option<(usize, usize)> {
    let m = m.filter(|&m| w.mobys.get(m).is_some_and(|x| x.mode & crate::moby::ai::AI != 0))?;
    Some((m, crate::moby::ai::block(&w.mobys[m], 0)))
}

/// `0x38cbf8`: the wrench's hit sound on moby `m`: 1 when its health block's byte +9 is 1, else 0.
fn hit_sound(w: &MobyWorld, m: Option<usize>) -> i32 { health(w, m).is_some_and(|(m, b)| w.mobys[m].pv_b(b + 9) == 1) as i32 }

/// `0x38cc48`: moby `m`'s health block has bit 1 of `+0x1e` set: no hit sound.
fn silent(w: &MobyWorld, m: Option<usize>) -> bool { health(w, m).is_some_and(|(m, b)| w.mobys[m].pv_s(b + 0x1e) & 2 != 0) }

/// The class type of moby `m` (class header +0x46, `*(m + 0x24) + 0x46`) is `c`: 0x14 / 0x12 take no hit sound.
fn is_class(w: &MobyWorld, m: Option<usize>, c: i16) -> bool { m.and_then(|m| w.mobys.get(m)).and_then(|x| w.class(x.o_class)).is_some_and(|k| k.kind == c) }

/// `0x38d318`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    ctx.hero.items.flying = 0;
    if ctx.hero.items.ready == 3 { return; }
    match w.mobys[id].state {
        0 => in_hand(w, id, ctx),
        10 | 11 => flight(w, id, ctx),
        _ => {}
    }
}

/// What the wrench reads of the hero block.
#[derive(Clone, Copy)]
struct HeroView {
    state: i32,
    group: i32,
    rot: V3,
    pos: V3,
    timer: i32,
    anim_speed: f32,
    melee: crate::hero::melee::MeleeBlock,
}

/// State 0.
fn in_hand(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let hh = &*ctx.hero;
    let h = HeroView { state: hh.state, group: hh.group, rot: hh.rot, pos: hh.pos, timer: hh.timer, anim_speed: hh.anim_speed, melee: hh.melee };
    w.mobys[id].parent = ctx.hero_moby;
    // Its sequence: the hand pose 0xe until it wraps, then 1; out of 0xf / 0x10 (the look stance) back to 0xe; the
    // swings' poses 3..5 back to 1 outside the swing states.
    if let Some(a) = ctx.host.anim(&w.mobys[id]) {
        let seq = a.seq_b;
        let wrapped = a.flags & 2 != 0;
        if seq == 0xe {
            if wrapped { ctx.host.set_anim(&w.mobys[id], 1, 0, 5); }
            return;
        }
        if seq == 0xf || seq == 0x10 {
            ctx.host.set_anim(&w.mobys[id], 0xe, 0, 2);
            return;
        }
        if wrapped && h.state != 0x37 && h.group != 0x22 && seq != 1 { ctx.host.set_anim(&w.mobys[id], 1, 0, 5); }
        if !matches!(h.state, 0x13 | 0x20 | 0x2a | 0x6d | 0x14) && (3..6).contains(&seq) { ctx.host.set_anim(&w.mobys[id], 1, 0, 4); }
    }
    let v = ctx.hero_view;
    {
        let m = &mut w.mobys[id];
        let c = pv_s(m, 0x74);
        if v.seq_b != v.seq_a || 2 < v.frame_b as i32 - v.frame_a as i32 { set_pv_s(m, 0x74, c.wrapping_add(1)); }
    }
    ctx.host.set_anim_speed(&w.mobys[id], h.anim_speed);
    if h.group != 6 && h.state != 0x2a { return; }
    let sw = SWINGS[h.melee.row.clamp(0, 8) as usize];
    let frame = v.frame;
    let mut window = !(v.blending() || frame < sw[HIT_FROM] as f32 || (sw[HIT_TO] as f32) < frame);
    // The trail starts once per swing at the swing's frame (0x17 / 0x18 at 0, 0x19 at 12, the jump attack at 23).
    {
        let m = &mut w.mobys[id];
        if pv_s(m, 0x7c) != pv_s(m, 0x74) && v.seq_a == v.seq_b {
            let from = match v.seq_a { 0x17 | 0x18 => Some(0.0), 0x19 => Some(12.0), 0x2b => Some(23.0), _ => None };
            if from.is_some_and(|f| f <= frame) {
                set_pv_f(m, 0x70, 1.0);
                let c = pv_s(m, 0x74);
                set_pv_s(m, 0x7c, c);
            }
        }
    }
    // The segment: base = Ratchet's joint 0, head = the wrench's joint 1 pushed 0.17 further; last tick's kept.
    let (Some(head), Some(base)) = (ctx.host.joint(&w.mobys[id], 1), ctx.hero_moby.and_then(|hm| ctx.host.joint(&w.mobys[hm], 0))) else { return };
    let (prev_base, prev_head) = (ctx.hero.melee.sweep_base, ctx.hero.melee.sweep_head);
    let mut yaw = h.rot[2];
    if matches!(h.state, 0x13 | 0x6d | 0x2a) {
        let a = atan2(head[0] - h.pos[0], head[1] - h.pos[1]);
        let side = if sw[1] == 1 { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 };
        yaw = crate::hero::physics::add_rot(a, side);
    }
    let mut d = sub(head, base);
    let l = len(d);
    if 0.0 < l { d = scale(d, (l + 0.17) / l); }
    let mut off = 0.0;
    if h.group != 0xf && h.state != 0x6d && h.state != 0x14 {
        off = rot_dist(h.rot[2], atan2(head[0] - h.pos[0], head[1] - h.pos[1]));
        if std::f32::consts::FRAC_PI_2 < off { window = false; }
        d = reach(h.rot[2], d);
    }
    let head = add(base, d);
    ctx.hero.melee.sweep_base = base;
    ctx.hero.melee.sweep_head = head;
    // The wall test (from 10 ticks on, within 70° of the facing): a line 1.4 out at waist height.
    if !matches!(h.state, 0x2a | 0x6d) && off < 1.221_730_6 && 10 < h.timer && ctx.hero.melee.wall_hit == 0 {
        let a = atan2(head[0] - h.pos[0], head[1] - h.pos[1]);
        let from = add(h.pos, [0.0, 0.0, 0.5]);
        let out = reach(h.rot[2], [a.cos() * 1.4, a.sin() * 1.4, 0.0]);
        if let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, from, add(from, out), crate::collision::QueryFlags(2), None)) {
            let crate_hit = o.moby.is_some_and(|m| (500..=540).contains(&w.mobys.get(m).map_or(0, |x| x.o_class)) && w.mobys[m].o_class != 0x1f6);
            if o.moby.is_none() || !crate_hit {
                ctx.hero.melee.wall_hit = 1;
                ctx.host.play_sound(&w.mobys[id], 2, 0);
                if let Some(m) = o.moby {
                    let t = HitTemplate::new(1.0, Some(id), 0x10000, [yaw.cos(), yaw.sin(), 0.0, 0.0]);
                    w.deliver_hit_any(m, &t, [o.point[0], o.point[1], o.point[2], 1.0]);
                }
            }
        }
    }
    if !window { return; }
    let (damage, k) = if h.state == 0x14 { (DAMAGE_JUMP, 1.55) } else { (DAMAGE, 1.0) };
    let mut t = HitTemplate::new(damage, Some(id), 0x10000, [yaw.cos() * k, yaw.sin() * k, 1.0 * k, NO_KNOCK_LIMIT]);
    t.dir[2] = 1.0;
    t.dir[3] = NO_KNOCK_LIMIT;
    t.kind = (w.mobys[id].o_class as u16 as u32) << 16 | 0x100;
    let mut hit = false;
    // `0x31d648(base, head, prev base, prev head, Ratchet, t, 5)`: 5 lines between the last tick's segment and this one.
    for i in 1..=5 {
        let f = i as f32 / 5.0;
        let a = add(prev_base, scale(sub(base, prev_base), f));
        let b = add(prev_head, scale(sub(head, prev_head), f));
        if let Some(o) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, a, b, crate::collision::QueryFlags(0), ctx.hero_moby)) {
            if let Some(m) = o.moby { w.deliver_hit_any(m, &t, [o.point[0], o.point[1], o.point[2], 1.0]); }
            hit = true;
            // A moby struck along the sweep: its hit sound, once a swing.
            if o.moby.is_some() && !silent(w, o.moby) && ctx.hero.melee.wall_hit == 0 && !is_class(w, o.moby, 0x14) {
                ctx.hero.melee.wall_hit = 1;
                ctx.host.play_sound(&w.mobys[id], hit_sound(w, o.moby), 0);
            }
            break;
        }
    }
    let _ = hit;
    let r = if h.group == 0xf { 0.7 } else if h.state == 0x14 { 0.47 } else { 0.35 };
    let mut dd = sub(head, base);
    let l = len(dd);
    if 0.0 < l { dd = scale(dd, (l - 0.085) / l); }
    let c = add(base, dd);
    let n = w.sphere_hit(ctx.scene, c, r, 0, ctx.hero_moby, Some(&t));
    if n == 0 { return; }
    let target = w.hits.last_target;
    if is_class(w, target, 0x12) || silent(w, target) || ctx.hero.melee.wall_hit != 0 { return; }
    ctx.hero.melee.wall_hit = 1;
    ctx.host.play_sound(&w.mobys[id], hit_sound(w, target), 0);
}

/// `0x38d270(&d)`: the reach shrinks with the angle off the facing (full 0.7 + 0.30 at 0°, 0.7 at 105°).
fn reach(yaw: f32, d: V3) -> V3 {
    let lim = 1.832_596;
    let a = rot_dist(yaw, atan2(d[0], d[1])).min(lim);
    scale(d, (lim - a) * 0.163_702_22 + 0.7)
}

/// States 10 (out) and 11 (back).
fn flight(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    ctx.hero.items.flying = 2;
    w.mobys[id].parent = ctx.hero_moby;
    // Its whirr while it flies out (`0x18c250`: unless it still plays).
    if w.mobys[id].state == 10 && !ctx.hero.items.throw_sound.is_some_and(|h| ctx.host.sound_playing(h)) {
        let h = ctx.host.play_sound(&w.mobys[id], 0xb, 4);
        ctx.hero.items.throw_sound = (0 <= h).then_some(h);
    }
    let prev = w.mobys[id].pos;
    {
        let carry = scale(ctx.hero.carry, 0.98);
        let m = &mut w.mobys[id];
        m.pos = add(m.pos, carry);
        if m.state == 10 {
            let dir = pv_v(m, 0x40);
            let s = pv_f(m, 0x60);
            m.pos = add(m.pos, scale(dir, s));
            let k = pv_f(m, 0x64) + 0.000_787_037;
            set_pv_f(m, 0x64, k);
            let mut s2 = s;
            crate::hero::physics::approach(0.0, k, &mut s2);
            set_pv_f(m, 0x60, s2);
            if s2 == 0.0 { m.state = 11; }
        } else {
            // Back to the hand (Ratchet's attachment 0, `0x18b020`), faster and faster.
            let home = ctx.host.hero_attach(0).map_or(ctx.hero.pos, |(_, p)| p);
            let d = sub(home, m.pos);
            let k = pv_f(m, 0x64) + 0.000_25;
            set_pv_f(m, 0x64, k);
            let mut s = pv_f(m, 0x60) + k;
            let l = len(d);
            let arrived = l <= s;
            if arrived { s = l; }
            set_pv_f(m, 0x60, s);
            if 0.0 < l { m.pos = add(m.pos, scale(d, s / l)); }
            let far = matches!(ctx.hero.state, 0x31 | 0x61) || 32.0 < l;
            if arrived || far {
                ctx.hero.items.thrown = 0;
                m.state = 0;
                if let Some(h) = ctx.hero.items.throw_sound.take() { ctx.host.stop_sound(h); }
            }
        }
    }
    let m_state = w.mobys[id].state;
    if m_state == 0 { return; }
    // A wall on the way back sends it home.
    let pos = w.mobys[id].pos;
    if let Some(_o) = ctx.coll.and_then(|c| crate::collision::coll_line_m(c, None, prev, pos, crate::collision::QueryFlags(2), None)) {
        if ctx.hero.melee.wall_hit == 0 {
            ctx.hero.melee.wall_hit = 1;
            ctx.host.play_sound(&w.mobys[id], 2, 0);
        }
        if m_state == 10 {
            let m = &mut w.mobys[id];
            let s = pv_f(m, 0x60) * 0.5;
            set_pv_f(m, 0x60, s);
            m.state = 11;
        }
    }
    let dir = pv_v(&w.mobys[id], 0x40);
    let k = if w.mobys[id].state == 11 { -0.37 } else { 1.2 };
    let mut t = HitTemplate::new(DAMAGE, Some(id), 0x10000, [dir[0] * k, dir[1] * k, 1.0, NO_KNOCK_LIMIT]);
    t.kind = ((ctx.hero.melee.throw_toggle + 1) << 16) | 0x100;
    let at = ctx.host.joint(&w.mobys[id], 1).unwrap_or(pos);
    let mut n = w.sphere_hit(ctx.scene, at, 0.4, 0, ctx.hero_moby, Some(&t));
    if n == 0 {
        let at = ctx.host.joint(&w.mobys[id], 0).unwrap_or(pos);
        n = w.sphere_hit(ctx.scene, at, 0.4, 0, ctx.hero_moby, Some(&t));
    }
    // A moby struck in flight: its hit sound, once a throw.
    let target = w.hits.last_target;
    if 0 < n && ctx.hero.melee.wall_hit == 0 && target.is_some() && !is_class(w, target, 0x14) {
        ctx.hero.melee.wall_hit = 1;
        ctx.host.play_sound(&w.mobys[id], hit_sound(w, target), 0);
    }
    let _ = dist;
}
