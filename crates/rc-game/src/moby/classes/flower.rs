//! Oozla's flower monster: its arms **0x0a8d** (L01 `0x3cdd88`; checks `0x3cd6d8`, set-up `0x3cd448`, tail
//! `0x3cd668`) and its core **0x0a8e** (`0x3cf680`; checks `0x3cf300`, set-up `0x3cf158`, the arms' link `0x3cef98`,
//! tail `0x3cf2b0`). On Oozla group 52 is the boss by the yellow water (two arms and the core); a third arm (group
//! 61, by (325, 470)) is on its own.
//!
//! **An arm** (sub-state `+0x95`), on the framework's blocks (health `+0x20`: `+0x24` the most, `+0x26` a 16-bit
//! wait; the colour flash `+0x50`; the target `+0x60`, found `+0x74` / `+0x7c`, its moby `+0x70`):
//! * 1, holding a captive (`+0xb4`, a 0x0b1a): the captive at its joint 8 (`+0xc8` set: joint 7) with its angles.
//!   Ratchet found in the area `+0xe0`, a hit on it or on the captive (or `+0xc0` running) wakes it: sequence 6 (10,
//!   the captive's 8, with `+0xc8`) and the captive's cry; at its end the captive goes (its `+0x95` = 1) and it sinks
//!   to wait (4), or with no area `+0xec` it goes too.
//! * 4, under the water: once Ratchet is found in its area (`+0xe8`) it waits 120 ticks and rises (3: sequence 0;
//!   at its end sequence 1, 240 ticks of `+0xc0`, the area `+0xec`, 5).
//! * 5, up: it slides toward Ratchet's place held to its path's polygon (`+0xb8`; home `+0x80` while he is lost),
//!   0.05 a tick, turning to him 5.5° off (`+0xdd` mirrors it); while `+0xc0` runs, Ratchet is in reach (35), it
//!   faces him within 2°, no other arm of the group swings (the shared count `+0xd8`) and its wait `+0xc4` is over,
//!   it swings (6: sequence 2), else once `+0xc0` is out it sinks (2: sequence 6, then 4).
//! * 6, the swing: frames 46..56 hit along its arm (the joints 1..8, two lines 0.3 apart, its damage `+0x34`) and
//!   a line down from its tip; it keeps aiming (at Ratchet's place 40 ticks of his move ahead) until a frame by the
//!   mission's rank (37 / 42 / 46) or 20 while he is in the air; frame 56 its thud. At the end the count back, a wait
//!   of 180 and up again (5).
//! * 7, hurt: knocked off Ratchet (`+0xa0`, losing 0.003 a tick) inside its polygon; then up (5), or dead (9).
//! * 8 / 9, dying: its bolts, the mission `+0xb0` done with the checkpoint at the cuboid `+0xe4` (when it has one).
//!
//! A hit (5, 6, or 1) takes its damage off the core's health and every member of its group (`+0xb0` names the core;
//! without one its own), a hurt of 1 or more knocks it (7). With the core dead it dies too.
//!
//! **The core** (0x0a8e): up (5) while any arm's `+0xbc` / `+0xc0` runs, sliding to its path's point nearest the
//! arm's target, turned to it; it shows the boss bar ([`crate::game::GameState::boss_bar`]); hurt (6) knocked like
//! the arms; dead (7): its bolts, the moby `+0xcc` deleted. Its arms are `+0xa0` / `+0xa4`.
//!
//! Not ported: the splashes and ripples (`0x3cd130`, `0x339b60`), the swing's dust ring and the camera shake
//! (`0x1b94e0`), the shadow, the music (`0x3245e8`: the tracks `+0xd0..+0xdc`), `0x1b2f40`,
//! the water clip `+0x70` / `+0x74` of the moby.

use crate::hero::physics::{add, add_rot, atan2, dist2, len, rot_dist, scale, set_len, sub, sub_rot, V3};
use crate::moby::{ai, countdown, mode, Ctx, MobyWorld};
use super::flyer::into_polygon;
use super::leaper::{play, seq_a};
use super::props::wrapped;
use super::spitter::line_hit;

const HP: usize = 0x20;
const FLASH: usize = 0x50;
const T_FOUND: usize = 0x74;
const T_SEEN: usize = 0x7c;
const T_MOBY: usize = 0x70;
const HOME: usize = 0x80;
const GOAL: usize = 0x90;
const KNOCK: usize = 0xa0;
const CORE: usize = 0xb0;
const CAPTIVE: usize = 0xb4;
const PATH: usize = 0xb8;
const AREA: usize = 0xf0;
/// The 5.5° the arm aims off.
const OFF: f32 = 0.096_000_75;

fn moby_at(w: &MobyWorld, i: i32) -> Option<usize> {
    usize::try_from(i).ok().filter(|&i| w.mobys.get(i).is_some_and(|m| m.state != 0xfe && m.state != 0xfd))
}

fn countdown_pv(w: &mut MobyWorld, id: usize, off: usize) {
    let m = &mut w.mobys[id];
    let mut t = m.pv_i(off);
    countdown(&mut t);
    m.set_pv_i(off, t);
}

fn countdown_pv16(w: &mut MobyWorld, id: usize, off: usize) {
    let m = &mut w.mobys[id];
    let mut t = m.pv_s(off) as i32;
    countdown(&mut t);
    m.set_pv_s(off, t as i16);
}

/// `0x311f48(yaw, accel, brake, most, m, &v)`: the facing springs toward `yaw`.
fn spring_yaw(w: &mut MobyWorld, id: usize, yaw: f32, a: f32, b: f32, most: f32, v_off: usize) {
    let m = &mut w.mobys[id];
    let e = sub_rot(yaw, m.rot[2]);
    let mut v = m.pv_f(v_off);
    v += a * (e * 6.366_197).clamp(-1.0, 1.0) - b * v;
    if most != 0.0 { v = v.clamp(-most, most); }
    v = v.clamp(-e.abs(), e.abs());
    m.set_pv_f(v_off, v);
    m.rot[2] = add_rot(m.rot[2], v);
}

/// Toward `goal` at most 0.05 a tick (`0x2e44d8`); an arm's by a twentieth of the way, keeping its height.
fn slide(w: &mut MobyWorld, id: usize, goal: V3, arm: bool) {
    let m = &mut w.mobys[id];
    let mut d = sub(goal, m.pos);
    if arm { d = scale(d, 0.05); }
    let l = len(d);
    if 0.05 < l { d = scale(d, 0.05 / l); }
    if arm { d[2] = 0.0; }
    m.pos = add(m.pos, d);
}

/// The target's place: Ratchet's `ahead` ticks of his move ahead (`0x189f30`), else the moby's.
fn aim_point(w: &MobyWorld, id: usize, ctx: &Ctx, ahead: f32) -> Option<V3> {
    let t = ai::link(&w.mobys[id], T_MOBY)?;
    if Some(t) == ctx.hero_moby { return Some(add(scale(ctx.hero.disp, ahead), ctx.hero.pos)); }
    w.mobys.get(t).map(|m| m.pos)
}

fn yaw_to(m: &crate::moby::Moby, p: V3) -> f32 { atan2(p[0] - m.pos[0], p[1] - m.pos[1]) }

/// The swing's turn: aimed `OFF` aside while it springs (0.00233, 0.1, at most 8°: `+0xcc`).
fn aim(w: &mut MobyWorld, id: usize, at: V3) {
    let off = if w.mobys[id].pv_b(0xdd) != 0 { -OFF } else { OFF };
    let m = &mut w.mobys[id];
    m.rot[2] = add_rot(m.rot[2], off);
    let yaw = yaw_to(m, at);
    spring_yaw(w, id, yaw, 0.002_327_102_7, 0.1, 0.139_626_35, 0xcc);
    let m = &mut w.mobys[id];
    m.rot[2] = sub_rot(m.rot[2], off);
}

/// `0x3cd400`: an arm that swings gives the group's count back.
fn release(w: &mut MobyWorld, id: usize) {
    let m = &w.mobys[id];
    if m.group != -1 && m.pv_b(0xdc) != 0 {
        let p = m.pv_i(0xd8);
        let mut n = w.shared_i(p);
        countdown(&mut n);
        w.set_shared_i(p, n);
    }
    w.mobys[id].set_pv_b(0xdc, 0);
}

/// A hit's damage on this moby or its group (`0x3cd6d8` / `0x3cf300`): `None` without one.
/// `apply`: 0 the damage is not taken, 1 off its own health, 2 off its group's.
fn take_hit(w: &mut MobyWorld, id: usize, ctx: &Ctx, apply: u8) -> Option<(f32, Option<usize>)> {
    let hit = w.hit_message(id).filter(|h| h.tmpl.flags & 0x10000 != 0);
    let m = &mut w.mobys[id];
    let rec = if m.pv_s(HP + 6) < 1 { hit } else { None };
    let wrench = rec.and_then(|h| h.tmpl.source).and_then(|s| w.mobys.get(s)).is_some_and(|s| s.o_class == crate::hero::items::WRENCH_CLASS);
    let source = rec.and_then(|h| h.tmpl.source);
    let m = &mut w.mobys[id];
    let dmg = ai::damage_of(m, rec.as_ref(), wrench);
    let _ = ctx;
    if rec.is_some() && 0.0 < dmg && apply != 0 {
        if apply == 1 {
            m.set_pv_f(HP, m.pv_f(HP) - dmg);
        } else if let Some(members) = usize::try_from(m.group).ok().and_then(|g| w.groups.get(g)).cloned() {
            let (b37, f38, s3c) = (m.pv_b(0x37), m.pv_f(0x38), m.pv_s(0x3c));
            let wait = if source.and_then(|s| w.mobys.get(s)).is_some_and(|s| (0xce4..=0xce5).contains(&s.o_class)) { 0x3c } else { 0 };
            for o in members {
                let om = &mut w.mobys[o];
                if om.pvars.len() < 0x40 { continue; }
                let h = ai::health(om);
                om.set_pv_s(h + 6, wait);
                om.set_pv_f(h, om.pv_f(h) - dmg);
                om.set_pv_b(h + 0x17, b37);
                om.set_pv_f(h + 0x18, f38);
                om.set_pv_s(h + 0x1c, s3c);
                w.clear_hit(o);
            }
        }
    }
    w.clear_hit(id);
    if 0.0 < dmg {
        let m = &mut w.mobys[id];
        m.set_pv_b(FLASH + 7, ((dmg * 20.0) as i32 + 100) as u8);
        ai::flash_start(m, FLASH);
    }
    rec.map(|_| (dmg, source))
}

/// The knock away from Ratchet a hurt of more than 0.1 gives (`(2·d + 3)/60`, at most 0.2).
fn knock(w: &mut MobyWorld, id: usize, ctx: &Ctx, dmg: f32, off: usize) {
    let m = &mut w.mobys[id];
    let mut v = set_len(sub(m.pos, ctx.hero.pos), ((dmg + dmg + 3.0) * 0.016_666_668).min(0.200_000_02));
    v[2] = 0.0;
    m.set_pv_v(off, v);
}

/// The hurt's slide: by the knock (losing 0.003 a tick), held in the path's polygon and pulled home 1/30 when out.
fn knocked(w: &mut MobyWorld, id: usize, ctx: &Ctx, knock_off: usize, path_off: usize, home_off: usize) {
    let m = &w.mobys[id];
    let p = add(m.pos, m.pv_v(knock_off));
    let path = m.pv_i(path_off);
    let held = into_polygon(ctx, path, p);
    let m = &mut w.mobys[id];
    if held == p {
        m.pos = p;
    } else {
        m.pos = held;
        let d = scale(sub(m.pv_v(home_off), m.pos), 0.033_333_335);
        m.pos = add(m.pos, d);
    }
    let v = m.pv_v(knock_off);
    let l = (len(v) - 0.003_055_555_7).max(0.0);
    m.set_pv_v(knock_off, set_len(v, l));
}

/// `0x3cd6d8`: an arm's hits, the colour flash, the search.
fn arm_checks(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let sub_state = w.mobys[id].sub_state;
    let live = sub_state == 1 || (matches!(sub_state, 5 | 6) && 0.0 < w.mobys[id].pv_f(HP));
    'hit: {
        if !live {
            w.clear_hit(id);
            break 'hit;
        }
        let grouped = w.mobys[id].pv_i(CORE) != -1;
        let apply = if sub_state <= 4 { 0 } else if grouped { 2 } else { 1 };
        let hit = take_hit(w, id, ctx, apply);
        if grouped && hit.is_some() && 4 < sub_state { w.mobys[id].set_pv_i(0xc0, 0xf0); }
        let (dmg, source) = hit.unwrap_or((0.0, None));
        if sub_state == 1 {
            if hit.is_some() {
                let m = &mut w.mobys[id];
                m.set_pv_i(0xbc, 0);
                if m.state == 0 { m.set_state(1, -1); }
            }
            break 'hit;
        }
        let core_dead = moby_at(w, w.mobys[id].pv_i(CORE)).is_none() && w.mobys[id].pv_i(CORE) != -1;
        if w.mobys[id].pv_f(HP) < 0.0 || core_dead {
            release(w, id);
            play(w, id, ctx, 4, 0, 10);
            let m = &mut w.mobys[id];
            m.set_pv_f(HP, 0.0);
            m.mode &= !mode::TARGET;
            m.set_state(0, 7);
            break 'hit;
        }
        if source.and_then(|s| w.mobys.get(s)).is_some_and(|s| s.o_class == crate::hero::items::WRENCH_CLASS) { break 'hit; }
        if 1.0 <= dmg {
            release(w, id);
            match seq_a(w, id, ctx) {
                1 => play(w, id, ctx, 3, 5, 5),
                2 => play(w, id, ctx, 3, 0, 0x14),
                _ => play(w, id, ctx, 3, 2, 10),
            }
            let m = &mut w.mobys[id];
            m.mode &= !mode::TARGET;
            m.set_state(0, 7);
        }
        if 0.1 < dmg { knock(w, id, ctx, dmg, KNOCK); }
    }
    ai::flash(&mut w.mobys[id], FLASH);
    let area = w.mobys[id].pv_i(AREA);
    let any = w.mobys[id].sub_state < 5;
    ai::find_target(w, id, ctx, 0.0, 0.0, area, None, any);
}

/// `0x3cd668`: the timers.
fn arm_tail(w: &mut MobyWorld, id: usize) {
    if w.mobys[id].pv_i(T_FOUND) == 0 { countdown_pv(w, id, 0xc0); }
    let m = &mut w.mobys[id];
    if (m.state_ticks as u16) < 0xfffe { m.state_ticks = m.state_ticks.wrapping_add(1); }
    countdown_pv(w, id, 0xbc);
    countdown_pv(w, id, 0xc4);
    countdown_pv16(w, id, HP + 6);
}

/// `0x3cd448`: the set-up.
fn arm_setup(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let scale_of = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let frame = ctx.game.frame;
    let m = &mut w.mobys[id];
    m.set_pv_b(0x48, 5);
    m.set_pv_b(0x4a, 0x20);
    m.set_pv_b(0x29, 1);
    m.scale = scale_of * 2.5;
    let pos = m.pos;
    m.set_pv_v(HOME, pos);
    let (g, p) = (m.group, m.pv_i(0xd8));
    if frame == 0 && g != -1 { w.set_shared_i(p, 0); }
    let m = &mut w.mobys[id];
    m.set_pv_f(0x30, 2.0);
    m.set_pv_b(0x2a, 8);
    m.set_pv_b(0x2e, 3);
    m.set_pv_b(0x28, 3);
    m.set_pv_i(0xbc, 0);
    m.set_pv_i(AREA, m.pv_i(0xe8));
    if m.pv_b(0xdd) != 0 { m.mode |= mode::MIRROR; }
    if m.pv_i(CAPTIVE) != -1 {
        m.set_pv_i(AREA, m.pv_i(0xe0));
        if moby_at(w, w.mobys[id].pv_i(CAPTIVE)).is_none() {
            w.mobys[id].set_pv_i(CAPTIVE, -1);
        } else {
            let m = &mut w.mobys[id];
            m.draw_dist = 0x3ff;
            m.update_dist = 0xff;
            if 0.0 < m.pv_f(0xd0) { m.pos[2] = m.pv_f(0xd0); }
            let s = if m.pv_i(0xc8) == 0 { 5 } else { 8 };
            ctx.host.set_anim(&w.mobys[id], s, 0, 0);
            w.mobys[id].set_state(0, 1);
            return;
        }
    }
    let m = &mut w.mobys[id];
    if 0.0 < m.pv_f(0xd4) { m.pos[2] = m.pv_f(0xd4); }
    m.set_pv_i(0xbc, 0x78);
    ctx.host.set_anim(&w.mobys[id], 0, 0, 0);
    ctx.host.set_anim_speed(&w.mobys[id], 0.0);
    let m = &mut w.mobys[id];
    m.mode &= !mode::TARGET;
    m.set_state(0, 4);
}

/// A random 0.8..1.2 playback speed.
fn random_speed(w: &MobyWorld, id: usize, ctx: &mut Ctx) {
    let s = ctx.game.rng.randf(0.8, 1.2);
    ctx.host.set_anim_speed(&w.mobys[id], s);
}

/// Back up from the water's level, idle (sequence 1), 240 ticks of `+0xc0`, in `area`, to 5.
fn up(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, area_off: usize) {
    let b = ctx.game.rng.rand_range(10, 15);
    play(w, id, ctx, 1, 0, b);
    random_speed(w, id, ctx);
    let m = &mut w.mobys[id];
    m.set_pv_i(AREA, m.pv_i(area_off));
    m.set_state(0, 5);
}

/// 0x0a8d (`0x3cdd88`).
pub fn update_arm(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    arm_checks(w, id, ctx);
    match w.mobys[id].sub_state {
        0 => {
            if ctx.game.frame < 2 { return; }
            arm_setup(w, id, ctx);
        }
        1 => hold(w, id, ctx),
        2 => {
            if matches!(seq_a(w, id, ctx), 6 | 10) && wrapped(w, id, ctx) {
                let m = &mut w.mobys[id];
                if 0.0 < m.pv_f(0xd4) { m.pos[2] = m.pv_f(0xd4); }
                play(w, id, ctx, 0, 0, 0);
                ctx.host.set_anim_speed(&w.mobys[id], 0.0);
                let m = &mut w.mobys[id];
                m.mode &= !mode::TARGET;
                m.set_state(0, 4);
            }
        }
        3 => {
            if wrapped(w, id, ctx) {
                let m = &mut w.mobys[id];
                m.mode |= mode::TARGET;
                m.set_pv_i(0xc0, 0xf0);
                up(w, id, ctx, 0xec);
            }
        }
        4 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(AREA, m.pv_i(0xe8));
            if m.pv_i(T_FOUND) == 1 && m.pv_i(T_SEEN) == 1 {
                if m.pv_i(0xbc) == 0 { m.set_pv_i(0xbc, 0x78); }
                if m.pv_i(0xbc) == 1 {
                    if 0.0 < m.pv_f(0xd4) { m.pos[2] = m.pv_f(0xd4); }
                    ctx.host.set_anim_speed(&w.mobys[id], 1.0);
                    let b = ctx.game.rng.rand_range(10, 15);
                    play(w, id, ctx, 0, 0, b);
                    w.mobys[id].set_state(0, 3);
                }
            }
        }
        5 => track(w, id, ctx),
        6 => swing(w, id, ctx),
        7 => {
            w.clear_hit(id);
            knocked(w, id, ctx, KNOCK, PATH, HOME);
            if !wrapped(w, id, ctx) { arm_tail(w, id); return; }
            w.mobys[id].set_pv_v(KNOCK, [0.0; 3]);
            if w.mobys[id].pv_f(HP) <= 0.0 {
                if seq_a(w, id, ctx) == 3 { play(w, id, ctx, 4, 0, 10); } else { w.mobys[id].set_state(0, 9); }
            } else {
                let m = &mut w.mobys[id];
                m.mode |= mode::TARGET;
                m.set_pv_i(0xc0, 0xf0);
                m.set_pv_i(0xc4, 0);
                up(w, id, ctx, AREA);
            }
        }
        8 => {
            if wrapped(w, id, ctx) { w.mobys[id].set_state(0, 9); }
        }
        9 => {
            die(w, id, ctx);
            return;
        }
        _ => {}
    }
    if w.mobys[id].state < 0x80 { arm_tail(w, id); }
}

/// Sub-state 9: the bolts, the mission and its checkpoint, gone.
fn die(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    crate::moby::bolts::drop(w, id, 0x200, 3, ctx);
    let m = &w.mobys[id];
    let (mission, cp) = (m.mission, m.pv_i(0xe4));
    let level = ctx.game.level.max(0) as usize;
    let done = ctx.game.missions.get(level * 0x10 + mission as usize).is_some_and(|b| b & 0x80 != 0);
    if cp != -1 && mission != 0xff && !done {
        ctx.game.mission_done(mission);
        if let Some((pos, rot)) = ctx.level.cuboid_place(cp) { ctx.game.save_checkpoint(pos, rot); }
    }
    w.delete(id);
}

/// Sub-state 1: holding its captive.
fn hold(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let Some(c) = moby_at(w, w.mobys[id].pv_i(CAPTIVE)) else { return };
    let j = if w.mobys[id].pv_i(0xc8) == 0 { 8 } else { 7 };
    if let Some(p) = ctx.host.joint(&w.mobys[id], j) { w.mobys[c].pos = p; }
    w.mobys[c].rot = w.mobys[id].rot;
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(AREA, m.pv_i(0xe0));
            let mut woken = m.pv_i(T_FOUND) == 1 && m.pv_i(T_SEEN) == 1;
            if 0 < m.pv_i(0xc0) { woken = true; }
            if w.hit_message(c).is_some_and(|h| h.tmpl.flags & 0x10001 != 0) { woken = true; }
            if woken {
                let m = &mut w.mobys[id];
                m.mode &= !mode::TARGET;
                m.set_pv_i(0xbc, 0);
                m.set_state(1, -1);
            }
        }
        1 => {
            if w.mobys[id].pv_i(0xbc) != 0 { return; }
            if w.mobys[id].pv_i(0xc8) == 0 {
                play(w, id, ctx, 6, 0, 7);
            } else {
                play(w, id, ctx, 10, 0, 7);
                ctx.host.set_anim(&w.mobys[c], 8, 0, 7);
            }
            ctx.host.play_sound(&w.mobys[c], 0, 0);
            let m = &mut w.mobys[id];
            m.mode &= !mode::TARGET;
            m.set_state(2, 1);
        }
        2 => {
            if !wrapped(w, id, ctx) { return; }
            w.mobys[c].sub_state = 1;
            if w.mobys[id].pv_i(0xec) == -1 {
                w.delete(id);
                return;
            }
            let m = &mut w.mobys[id];
            if 0.0 < m.pv_f(0xd4) { m.pos[2] = m.pv_f(0xd4); }
            m.mode &= !mode::TARGET;
            m.set_pv_i(0xbc, 0x28);
            play(w, id, ctx, 0, 0, 1);
            ctx.host.set_anim_speed(&w.mobys[id], 0.0);
            let m = &mut w.mobys[id];
            m.set_pv_i(AREA, m.pv_i(0xe8));
            m.set_state(0, 4);
        }
        _ => {}
    }
}

/// Sub-state 5: up, sliding toward Ratchet, maybe swinging.
fn track(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let goal = if m.pv_i(T_FOUND) == 1 {
        let t = m.pv_v(0x60);
        w.mobys[id].set_pv_i(0xc0, 0xf0);
        into_polygon(ctx, w.mobys[id].pv_i(PATH), t)
    } else {
        m.pv_v(HOME)
    };
    w.mobys[id].set_pv_v(GOAL, goal);
    let at = aim_point(w, id, ctx, 40.0).unwrap_or(goal);
    slide(w, id, goal, true);
    aim(w, id, at);
    if wrapped(w, id, ctx) { random_speed(w, id, ctx); }
    let m = &w.mobys[id];
    let shared = if m.group != -1 { w.shared_i(m.pv_i(0xd8)) } else { 0 };
    let swing = ctx.game.scenes.mode != crate::scene::SCENE
        && ctx.hero.state != 0x6f
        && m.pv_i(0xc4) == 0
        && !(m.group != -1 && 0 < shared)
        && dist2(m.pos, m.pv_v(0x60)) < 35.0
        && rot_dist(m.rot[2], yaw_to(m, at)) < 0.034_906_59;
    if swing {
        let m = &mut w.mobys[id];
        m.set_pv_b(0xdc, 1);
        if m.group != -1 {
            let p = m.pv_i(0xd8);
            w.set_shared_i(p, w.shared_i(p) + 1);
        }
        let b = ctx.game.rng.rand_range(7, 12);
        play(w, id, ctx, 2, 0, b);
        let m = &mut w.mobys[id];
        m.set_state(0, 6);
        m.set_pv_b(0xde, 0);
    } else if w.mobys[id].pv_i(0xc0) == 0 {
        play(w, id, ctx, 6, 0, 0x14);
        let m = &mut w.mobys[id];
        m.mode &= !mode::TARGET;
        m.set_state(0, 2);
    }
}

/// Sub-state 6: the swing.
fn swing(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let f = ctx.host.frame_time(&w.mobys[id]);
    let goal = w.mobys[id].pv_v(GOAL);
    slide(w, id, goal, true);
    if 46.0 < f && f < 56.0 {
        let m = &w.mobys[id];
        let damage = m.pv_b(0x34) as f32;
        let side = scale(m.rows[1], 0.3);
        let joints: Vec<Option<V3>> = (0..9).map(|j| ctx.host.joint(m, j)).collect();
        for j in 1..8 {
            let (Some(a), Some(b)) = (joints[j], joints[j + 1]) else { continue };
            line_hit(w, id, ctx, damage, add(a, side), add(b, side));
            line_hit(w, id, ctx, damage, sub(a, side), sub(b, side));
        }
        if let Some(t) = joints[6] {
            line_hit(w, id, ctx, damage, add(t, [0.0, 0.0, 0.5]), add(t, [0.0, 0.0, 2.0]));
        }
    }
    if 56.0 <= f && w.mobys[id].pv_b(0xde) == 0 {
        ctx.host.play_sound(&w.mobys[id], 2, 0);
        w.mobys[id].set_pv_b(0xde, 1);
    }
    let mission = w.mobys[id].mission;
    let rank = ctx.game.mission(mission, 0).max(1) as usize;
    let limit = [37.0, 42.0, 46.0].get(rank - 1).copied().unwrap_or(46.0);
    let grounded = ctx.hero.grounded_ticks != 0 && ctx.hero.air_ticks == 0 && ctx.hero.state != 4;
    if f < limit && (f < 20.0 || grounded) {
        if let Some(at) = aim_point(w, id, ctx, 55.0 - f) { aim(w, id, at); }
    }
    if !wrapped(w, id, ctx) { return; }
    release(w, id);
    w.mobys[id].set_pv_i(0xc4, 0xb4);
    up(w, id, ctx, AREA);
}


/// 0x0a8e (`0x3cf680`).
pub fn update_core(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    core_checks(w, id, ctx);
    match w.mobys[id].sub_state {
        0 => core_setup(w, id, ctx),
        1 | 5 => {
            if wrapped(w, id, ctx) {
                if seq_a(w, id, ctx) == 2 || ctx.game.rng.randi(0xf) != 0 {
                    play(w, id, ctx, 1, 0, 10);
                } else {
                    play(w, id, ctx, 2, 0, 10);
                }
            }
            if w.mobys[id].sub_state == 1 {
                if w.mobys[id].pv_i(0xbc) != 0 {
                    play(w, id, ctx, 1, 0, 10);
                    w.mobys[id].set_state(0, 5);
                }
            } else {
                let m = &mut w.mobys[id];
                if m.pv_i(0xbc) == 0 { let home = m.pv_v(0x60); m.set_pv_v(0x70, home); }
                let goal = m.pv_v(0x70);
                slide(w, id, goal, false);
                let m = &w.mobys[id];
                let yaw = yaw_to(m, m.pv_v(0x80));
                spring_yaw(w, id, yaw, 0.001_163_551_4, 0.1, 0.104_719_76, 0xb8);
                if w.mobys[id].pv_i(0xbc) == 0 {
                    w.mobys[id].mode &= !mode::TARGET;
                    play(w, id, ctx, 3, 0, 0x14);
                    w.mobys[id].set_state(0, 2);
                }
            }
        }
        2 => {
            if wrapped(w, id, ctx) {
                w.mobys[id].mode &= !mode::TARGET;
                play(w, id, ctx, 0, 0, 10);
                ctx.host.set_anim_speed(&w.mobys[id], 0.0);
                w.mobys[id].set_state(0, 4);
            }
        }
        3 => {
            if wrapped(w, id, ctx) {
                w.mobys[id].mode |= mode::TARGET;
                play(w, id, ctx, 1, 0, 10);
                w.mobys[id].set_state(0, 5);
            }
        }
        4 => {
            if 0 < w.mobys[id].pv_i(0xbc) {
                ctx.host.set_anim_speed(&w.mobys[id], 1.0);
                play(w, id, ctx, 0, 0, 10);
                w.mobys[id].set_state(0, 3);
            }
        }
        6 => {
            knocked(w, id, ctx, 0x90, 0xa8, 0x60);
            if wrapped(w, id, ctx) {
                if w.mobys[id].pv_f(HP) <= 0.0 {
                    w.mobys[id].mode &= !mode::TARGET;
                    w.mobys[id].set_state(0, 7);
                } else {
                    w.mobys[id].mode |= mode::TARGET;
                    play(w, id, ctx, 1, 0, 10);
                    w.mobys[id].set_state(0, 5);
                }
            }
        }
        7 => {
            crate::moby::bolts::drop(w, id, 0x200, 3, ctx);
            if let Some(o) = moby_at(w, w.mobys[id].pv_i(0xcc)) { w.delete(o); }
            w.mobys[id].set_pv_i(0xcc, -1);
            w.delete(id);
            return;
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    if (m.state_ticks as u16) < 0xfffe { m.state_ticks = m.state_ticks.wrapping_add(1); }
    countdown_pv(w, id, 0xbc);
    countdown_pv(w, id, 0xb4);
    countdown_pv16(w, id, HP + 6);
}

/// `0x3cf158`: the core's set-up.
fn core_setup(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let scale_of = w.class(w.mobys[id].o_class).map_or(1.0, |c| c.scale);
    let m = &mut w.mobys[id];
    m.set_pv_b(0x48, 30);
    m.set_pv_b(0x4a, 24);
    m.scale = scale_of;
    let pos = m.pos;
    m.set_pv_v(0x60, pos);
    m.set_pv_b(0x2e, 3);
    m.set_pv_f(0x30, 3.0);
    m.set_pv_b(0x2a, 8);
    m.set_pv_b(0x28, 3);
    m.set_pv_i(0xb4, 0);
    m.set_pv_i(0xc0, -1);
    let z = m.pos[2];
    if let Some(o) = moby_at(w, w.mobys[id].pv_i(0xcc)) { w.mobys[o].pos[2] = z; }
    if w.mobys[id].pv_i(0xac) == 0 {
        w.mobys[id].mode |= mode::TARGET;
        ctx.host.set_anim(&w.mobys[id], 1, 0, 0);
        w.mobys[id].set_state(0, 1);
    } else {
        w.mobys[id].mode &= !mode::TARGET;
        ctx.host.set_anim(&w.mobys[id], 0, 0, 0);
        ctx.host.set_anim_speed(&w.mobys[id], 0.0);
        w.mobys[id].set_state(0, 4);
    }
}

/// `0x3cf300`: the core's hits, then its arms (`0x3cef98`) and the boss bar.
fn core_checks(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let sub_state = w.mobys[id].sub_state;
    if !matches!(sub_state, 5 | 6) || w.mobys[id].pv_f(HP) <= 0.0 {
        w.clear_hit(id);
    } else {
        let grouped = w.mobys[id].group != -1 && w.mobys[id].pv_i(0xa0) != -1;
        let (dmg, _) = take_hit(w, id, ctx, if grouped { 2 } else { 1 }).unwrap_or((0.0, None));
        if w.mobys[id].pv_f(HP) < 0.0 {
            if let Some(o) = moby_at(w, w.mobys[id].pv_i(0xcc)) { w.delete(o); }
            w.mobys[id].set_pv_i(0xcc, -1);
            play(w, id, ctx, 5, 0, 10);
            let m = &mut w.mobys[id];
            m.set_pv_f(HP, 0.0);
            m.set_state(0, 6);
            m.mode &= !mode::TARGET;
        } else {
            if 1.0 <= dmg {
                w.mobys[id].set_pv_i(0xbc, 0xf0);
                play(w, id, ctx, 4, 0, 3);
                let m = &mut w.mobys[id];
                m.mode &= !mode::TARGET;
                m.set_state(0, 6);
            }
            if 0.1 < dmg { knock(w, id, ctx, dmg, 0x90); }
        }
    }
    core_arms_tick(w, id, ctx);
    let m = &w.mobys[id];
    if matches!(m.sub_state, 5 | 6) {
        let most = m.pv_s(0x24).max(1) as f32;
        ctx.game.boss_bar = Some(((m.pv_f(HP) / most).clamp(0.0, 1.0), 5));
        ctx.game.boss_icon = 1;
    }
}

/// `0x3cef98`: the core's arms (`+0xa0`, `+0xa4`): their waits shared, a dead core takes them down (8), the arm's
/// target is where the core slides to (its path's nearest point) and turns to.
fn core_arms_tick(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].sub_state == 6 { return; }
    for k in 0..2 {
        let Some(a) = moby_at(w, w.mobys[id].pv_i(0xa0 + 4 * k)) else { continue };
        if w.mobys[a].sub_state == 8 { continue; }
        let t = w.mobys[id].pv_i(0xbc).max(w.mobys[a].pv_i(0xc0));
        w.mobys[id].set_pv_i(0xbc, t);
        w.mobys[a].set_pv_i(0xc0, t);
        if w.mobys[id].pv_f(HP) <= 0.0 {
            play(w, id, ctx, 5, 0, 10);
            w.mobys[id].set_pv_f(HP, 0.0);
            w.mobys[a].set_pv_f(HP, 0.0);
            if seq_a(w, a, ctx) != 4 { play(w, a, ctx, 4, 0, 10); }
            w.mobys[a].sub_state = 8;
            w.mobys[id].set_state(0, 6);
        }
        let target = w.mobys[a].pv_v(0x60);
        w.mobys[id].set_pv_v(0x80, target);
        let path = w.mobys[id].pv_i(0xa8);
        let near = into_polygon(ctx, path, target);
        w.mobys[id].set_pv_v(0x70, near);
    }
}

/// 0x0b1a of kind 0 (`+0x50`), a captive (`0x3da2e0`): set up from the moby holding it (`+0x54`: sequence 9, or 6
/// while it holds with `+0xc8`; its lights), then idle; let go (`+0x95` set by its holder) it is counted and gone.
/// Its other kinds are not ported.
pub fn update_0b1a(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].pv_i(0x50) != 0 { return; }
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.draw_dist = 0x3ff;
            m.update_dist = 0xff;
            let holder = moby_at(w, w.mobys[id].pv_i(0x54));
            let s = match holder {
                Some(h) if w.mobys[h].pv_i(0xc8) != 0 => 6,
                Some(_) => 9,
                None => 9,
            };
            ctx.host.set_anim(&w.mobys[id], s, 0, 0);
            if let Some(h) = holder { (w.mobys[id].light, w.mobys[id].ambient) = (w.mobys[h].light, w.mobys[h].ambient); }
            w.mobys[id].set_state(1, -1);
        }
        _ => {
            if w.mobys[id].sub_state != 0 {
                let (uid, mission) = (w.mobys[id].uid, w.mobys[id].mission);
                if w.mobys[id].pv_i(0x60) == 0 { ctx.game.uid_gone(uid, mission); }
                w.delete(id);
            }
        }
    }
}
