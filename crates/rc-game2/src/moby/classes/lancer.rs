//! The Lancer, class 0x5d2 (L00 `0x3a8060`, called by the item pass `0x2b2060` while it is the hand item; item 0x1e,
//! crate::hero::items::ROWS). Created in state 0, it starts up (`0x3a8f20`) into state 2, which waits out the cooldown
//! into state 1, ready. Every tick: its muzzle (joint 0) and the aim — the search (crate::moby::aim, a 20 range cone
//! from Ratchet's chest along his yaw and pitch) must pick the same moby 5 ticks running to lock it, a lock then holds 5
//! ticks before the next search, and a lock that dies, is hidden or deleted is dropped. Ready, ○ held fires once the
//! cooldown (1 / 7 s) is out: one ammo, the use stance on the first shot (crate::hero::items::ItemBlock::use_start),
//! its sequence 2, its class sound 0, a shot (class 0xbfe, crate::moby::classes::shot) toward the lock, and the mobys
//! within 0.5 of the muzzle hit outright. Out of ammo, a press plays its class sound 1 instead. Fired from the look
//! stance 1, Ratchet goes to its scripted twin 0x1d, where the first shot waits 0x28 ticks. The use ends when ○ is let
//! go (or the ammo is out) and it has been in use 10 ticks.
//!
//! **pvars**: +0x00 the lock, +0x04 its class (s16, −1 none), +0x06 the lock's hold (s16), +0x08 its aim height,
//! +0x0c the cooldown (seconds), +0x10 a use started (s16), +0x14 the muzzle, +0x28 the flash ticks, +0x30 the last
//! search's pick, +0x34 the ticks it must stay picked.
//!
//! The muzzle flash (`0x3a8ce0`, [`muzzle_flash`]) and a point-blank hit's smoke and sparks (`0x3e46d8`) are particles.
//!
//! The markers (`0x2db498`): the lock's green reticle and, in the look stance's twin, the crosshair.
//!
//! Not ported: the flash light (`0x2fda20`, `+0x24`), the pad rumble (`0x2d6d60`), the
//! stats (`0x18c8dc..`), the first-person aim (`0x18b320`).

use crate::hero::items::{row, LANCER};
use crate::hero::physics::{add, len, scale, sub, V3};
use crate::moby::hits::HitTemplate;
use crate::moby::{aim, countdown, Ctx, MobyWorld};

/// `$gp+0x37fc` for the Lancer: the shots' range; the lock delay `$gp+0x684` and hold `$gp+0x680`.
pub const RANGE: f32 = 20.0;
const LOCK_DELAY: i32 = 5;
const LOCK_HOLD: i16 = 5;
/// `0x1b27e8` / `0x1b27f0`: the search's limits at the range (6°); 45° / 35° near Ratchet.
const CONE: aim::Cone = aim::Cone { yaw: 0.104_719_76, pitch: 0.104_719_76, range: RANGE, near_pitch: 0.785_398_2, near_yaw: 0.610_865_2 };
/// The reticle's FX texture.
const RETICLE: u16 = 0x26;
/// `t[3]` of a shot's template.
pub const NO_KNOCK_LIMIT: f32 = 5627.987;

/// `0x3a8f20`: the lock cleared, the cooldown 0.1 s (the equip before the first shot), the glow word 0xff0020ff.
fn start(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    m.pvars.resize(crate::moby::PVAR_BYTES, 0);
    m.set_pv_s(4, -1);
    m.set_pv_i(0, 0);
    m.set_pv_s(6, 0);
    m.set_pv_s(0x10, 0);
    m.set_pv_i(0x34, LOCK_DELAY);
    // The first shot waits 0.1 s (the equip: its sequence 0 plays meanwhile); no flash light or rumble yet.
    m.set_pv_f(0xc, 0.1);
    m.set_pv_i(0x24, -1);
    m.set_pv_i(0x3c, -1);
    m.set_pv_i(0x28, 0);
    m.set_pv_i(0x2c, 0);
    m.set_pv_i(0x30, 0);
    // Its glow word, which the hand item's glow (crate::hero::items::hand_glow) then pulses.
    m.glow = 0xff00_20ff;
}

/// `0x3a8060`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 {
        w.mobys[id].state = 2;
        start(w, id);
    }
    let Some(r) = row(LANCER) else { return };
    let muzzle = ctx.host.joint(&w.mobys[id], 0).unwrap_or(w.mobys[id].pos);
    w.mobys[id].set_pv_v(0x14, muzzle);
    let (hero_pos, hero_rows, hero_rot, hero_state) = (ctx.hero.pos, ctx.hero.rows, ctx.hero.rot, ctx.hero.state);
    let chest = add(hero_pos, scale(hero_rows[2], 0.75));
    {
        let m = &mut w.mobys[id];
        m.set_pv_i(0x2c, 0);
        let mut hold = m.pv_s(6) as i32;
        countdown(&mut hold);
        m.set_pv_s(6, hold as i16);
    }
    // The lock: dropped once dead, gone or hidden.
    if let Some(t) = w.mobys[id].pv_link(0) {
        let ok = w.mobys.get(t).is_some_and(|x| x.state < 0xfd && x.drawn != 0) && aim::health(w, t).is_none_or(|(hp, _)| 0.0 < hp || aim::is_crate(w, t));
        if !ok {
            let m = &mut w.mobys[id];
            m.set_pv_s(4, -1);
            m.set_pv_i(0, 0);
            m.set_pv_f(8, 0.0);
        }
    }
    // `0x18aa18`: Ratchet aims at a live lock this tick.
    if let Some(t) = w.mobys[id].pv_link(0) { ctx.hero.items.aim_target = Some((t, w.mobys[id].pv_f(8))); }
    // The search, ready and between holds.
    if w.mobys[id].state == 1 && w.mobys[id].pv_s(6) == 0 {
        let (sp, cp) = hero_rot[1].sin_cos();
        let (sy, cy) = hero_rot[2].sin_cos();
        let found = aim::search(w, ctx, CONE, chest, [cy * cp, sy * cp, sp], true);
        let last = w.mobys[id].pv_link(0x30);
        match found {
            None => {
                let m = &mut w.mobys[id];
                m.set_pv_i(0, 0);
                m.set_pv_s(4, -1);
                m.set_pv_f(8, 0.0);
                m.set_pv_i(0x34, LOCK_DELAY);
            }
            Some(f) if Some(f) != last => w.mobys[id].set_pv_i(0x34, LOCK_DELAY),
            Some(f) => {
                let mut d = w.mobys[id].pv_i(0x34);
                let done = countdown(&mut d);
                w.mobys[id].set_pv_i(0x34, d);
                if done {
                    let (class, up) = (w.mobys[f].o_class, aim::health(w, f).map_or(0.0, |(_, a)| a));
                    let m = &mut w.mobys[id];
                    m.set_pv_i(0, f as i32);
                    m.set_pv_s(4, class);
                    m.set_pv_f(8, up);
                    m.set_pv_s(6, LOCK_HOLD);
                }
            }
        }
        w.mobys[id].set_pv_i(0x30, found.map_or(0, |f| f as i32));
    }
    // The markers (crate::hud::marker): in the look stance's twin the crosshair at the screen's centre (red while the
    // cooldown runs with ammo left, else green); a lock's green reticle on its aim point, smaller with the distance.
    if hero_state == 0x1d {
        let red = 0.0 < w.mobys[id].pv_f(0xc) && ctx.hero.items.has_ammo(ctx.game);
        crate::hud::marker(&mut ctx.game.markers, 1.0, id, if red { 0x6000_00ff } else { 0x600f_ff0f }, None, RETICLE);
    }
    if let Some(t) = w.mobys[id].pv_link(0) {
        let at = aim::aim_point(w, t);
        let d = len(sub(at, ctx.host.camera_pos()));
        crate::hud::marker(&mut ctx.game.markers, (0.9 - d * 0.006_666_667 * 0.65).abs(), id, 0xff0f_ff0f, Some(at), RETICLE);
    }
    let it = ctx.hero.items;
    let fire_held = it.held & it.trigger != 0;
    // The flash: from 10 down, held at 1..8 while ○ is held with ammo.
    {
        let fl = w.mobys[id].pv_i(0x28);
        if 0 < fl && (8 < fl || !fire_held || !it.has_ammo(ctx.game)) { w.mobys[id].set_pv_i(0x28, fl - 1); }
    }
    // Its fire sequence 2 run out: back to 1.
    if let Some(a) = ctx.host.anim(&w.mobys[id]) {
        if a.seq_a == a.seq_b && a.flags & 2 != 0 { ctx.host.set_anim(&w.mobys[id], 1, 0, 2); }
    }
    match w.mobys[id].state {
        1 => {
            if hero_state == 1 { ctx.hero.request_state(0x1d, true); }
            let ok = if hero_state == 0x1d { countdown(&mut ctx.hero.items.look_wait) } else { ctx.hero.items.look_wait = 0x28; true };
            if cooldown(w, id) && ctx.hero.items.no_use == 0 && fire_held && ok {
                if ctx.hero.items.take_ammo(ctx.game, 1) {
                    fire(w, id, ctx, muzzle, r.damage, r.speed);
                } else if it.pressed & it.trigger != 0 {
                    ctx.host.play_sound(&w.mobys[id], 1, 0);
                }
                w.mobys[id].set_pv_f(0xc, 1.0 / r.rate);
            }
            let it = &ctx.hero.items;
            let keep = (fire_held && it.has_ammo(ctx.game)) || (it.use_ticks < 10 && it.using);
            if !keep {
                let idle = ctx.hero.idle_seq();
                ctx.hero.items.use_end(hero_state, ctx.hero_view.seq_b, idle);
                w.mobys[id].set_pv_s(0x10, 0);
            }
        }
        2 => {
            let idle = ctx.hero.idle_seq();
            ctx.hero.items.use_end(hero_state, ctx.hero_view.seq_b, idle);
            if cooldown(w, id) { w.mobys[id].state = 1; }
        }
        _ => {}
    }
}

/// `0x2eeb40(+0x0c)`: the cooldown down a frame (1/60 s); true when out.
fn cooldown(w: &mut MobyWorld, id: usize) -> bool {
    let c = w.mobys[id].pv_f(0xc);
    if 1.0 / 60.0 < c {
        w.mobys[id].set_pv_f(0xc, c - 1.0 / 60.0);
        false
    } else {
        w.mobys[id].set_pv_f(0xc, 0.0);
        true
    }
}

/// `0x3a8ce0`: the muzzle flash, nine steps down the Lancer's third row from 0.1 back along its second and 0.15 up its
/// third: a white sprite (36750, 4 ticks) jittered 0.1 and an orange glow (262500 at alpha 0x10, `def[0x35]`).
fn muzzle_flash(sys: &mut rc_game::particles::Particles, g: &mut crate::rng::Rng, muzzle: V3, rows: [V3; 3]) {
    use crate::particles::sprite;
    let mut base = add(add(muzzle, scale(rows[1], -0.1)), scale(rows[2], 0.15));
    let step = scale(rows[2], -0.033_333_335);
    let glow = sys.def_first(0x35);
    for _ in 0..9 {
        let p: V3 = std::array::from_fn(|k| base[k] + g.randf_signed(0.0, 0.1));
        let spin = g.randi(0xff) as u8;
        sprite(sys, g, 36_750.0, 36_750.0, 0.0, p, [0.0; 3], 4, 0x80_8080, spin, 0x7f, 0x7f, 1);
        if let Some(i) = sprite(sys, g, 262_500.0, 262_500.0, 0.0, base, [0.0; 3], 4, 0x20_3080, spin, 0x10, 0x10, 1) { sys.pool.recs[i][2] = glow; }
        base = add(base, step);
    }
}

/// One shot: the muzzle flash, the use stance on the first, sequence 2, the flash, class sound 0, the shot and the
/// point-blank hits (each with the impact's smoke and sparks).
fn fire(w: &mut MobyWorld, id: usize, ctx: &mut Ctx, muzzle: V3, damage: f32, speed: f32) {
    if let Some(sys) = ctx.parts.as_deref_mut() { muzzle_flash(sys, &mut ctx.game.rng, muzzle, w.mobys[id].rows); }
    if w.mobys[id].pv_s(0x10) == 0 {
        ctx.hero.items.use_start(ctx.hero.group);
        w.mobys[id].set_pv_s(0x10, 1);
    }
    if ctx.host.anim(&w.mobys[id]).is_some_and(|a| a.seq_b != 2) { ctx.host.set_anim(&w.mobys[id], 2, 0, 0); }
    w.mobys[id].set_pv_i(0x28, 10);
    ctx.host.play_sound(&w.mobys[id], 0, 0);
    let target = w.mobys[id].pv_link(0);
    crate::moby::classes::shot::spawn(w, ctx, RANGE, muzzle, id, target);
    // `0x2e14b0(0.5, muzzle)`: the mobys at the muzzle, each hit along muzzle → it.
    let Some(sc) = ctx.scene else { return };
    for m in rc_game::collision_query::coll_sphere_mobys(sc, muzzle, 0.5, rc_game::collision_query::QueryFlags(0), Some(id)) {
        if Some(m) == ctx.hero_moby || aim::health(w, m).is_none() { continue; }
        let d = scale(sub(w.mobys[m].pos, muzzle), speed / 60.0);
        let l = len(d);
        let d = if 0.0 < l { scale(d, 1.0 / l) } else { d };
        let mut t = HitTemplate::new(damage, Some(id), 0x10000, [d[0], d[1], d[2], NO_KNOCK_LIMIT]);
        t.kind = (w.mobys[id].o_class as u16 as u32) << 16 | 0x101;
        let p = w.mobys[m].pos;
        w.deliver_hit_any(m, &t, [p[0], p[1], p[2], 1.0]);
        let at = aim::aim_point(w, m);
        let cam = ctx.host.camera_pos();
        if let Some(sys) = ctx.parts.as_deref_mut() { crate::moby::classes::shot::impact(sys, &mut ctx.game.rng, at, cam, true); }
    }
}
