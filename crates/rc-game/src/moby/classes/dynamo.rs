//! The Dynamo, class 0x825 (L01 `0x3ad1a0`, item 0x24, crate::hero::items::ROWS; got on Oozla from 0x0a95): while it
//! is the hand item it locks the nearest Dynamo target (`0x3acff8`) and switches it on.
//!
//! **The search** (`0x3acff8`): the target list's 0x0cb5 / 0x12ea mobys not already on (state 2), on screen, seen
//! from the camera (a line with flags 6 meeting nothing else, or Ratchet, or the target), the nearest to Ratchet
//! (within `0x1af96c`, unlimited). Each tick the lock gets the reticle (FX 0x26, fainter with the camera's distance).
//!
//! **States**: 1 ready: the fire button with a lock starts the use stance and a wait of 12 ticks (4 in the air), 2;
//! at its end the lock is hit (flags 0x10000, from the Dynamo itself, so the target knows it: 0x0cb5), the wait 15
//! (3 in the air), 3; then 7 more, 4; then ready again (1). Ratchet's look stance 1 goes to its twin 0x1d, as the
//! Lancer's.
//!
//! Not ported: its three bolts of energy to the target (`0x3ad780`, animated by `0x3ada48` and drawn by `0x3ad5c8`),
//! the joint points `0x1b2f50` / `0x1b2f60`, the arm weight `0x18aa40`, the stats `0x18c510..`.

use crate::hero::items::DYNAMO;
use crate::hero::physics::{dist, len, sub};
use crate::moby::hits::HitTemplate;
use crate::moby::{countdown, mode, Ctx, MobyWorld};

/// The reticle's FX texture.
const RETICLE: u16 = 0x26;

/// `0x3acff8`: the target to lock.
fn search(w: &MobyWorld, ctx: &Ctx) -> Option<usize> {
    let hero = ctx.hero.pos;
    let cam = ctx.host.camera_pos();
    let mut best: Option<(f32, usize)> = None;
    for (i, m) in w.mobys.iter().enumerate() {
        if !(m.o_class == 0x0cb5 || m.o_class == 0x12ea) || m.state >= 0x80 || m.mode & mode::TARGET == 0 || m.state == 2 { continue; }
        if m.drawn == 0 { continue; }
        let seen = match ctx.coll.and_then(|c| crate::collision::coll_line_m(c, ctx.scene, cam, m.pos, crate::collision::QueryFlags(6), None)) {
            None => true,
            Some(o) => o.moby == Some(i) || o.moby == ctx.hero_moby || (m.o_class == 0x12ea && dist(o.point, m.pos) < 1.0),
        };
        if !seen { continue; }
        let d = dist(m.pos, hero);
        if best.is_none_or(|(b, _)| d < b) { best = Some((d, i)); }
    }
    best.map(|(_, i)| i)
}

/// 0x825 (`0x3ad1a0`).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 {
        let m = &mut w.mobys[id];
        m.state = 1;
        m.set_pv_i(0x18, 0);
    }
    if ctx.hero.state == 1 { ctx.hero.request_state(0x1d, true); }
    let muzzle = ctx.host.joint(&w.mobys[id], 0).unwrap_or(w.mobys[id].pos);
    let air = ctx.hero.grounded_ticks == 0;
    match w.mobys[id].state {
        1 => {
            if ctx.hero.items.no_use != 0 { return; }
            let lock = search(w, ctx);
            w.mobys[id].set_pv_i(0x18, lock.map_or(0, |t| t as i32 + 1));
            if let Some(t) = lock {
                let at = w.mobys[t].pos;
                let d = len(sub(ctx.host.camera_pos(), at));
                crate::hud::marker(&mut ctx.game.markers, (0.9 - d * 0.02 * 0.65).abs(), id, 0xff0f_ff0f, Some(at), RETICLE);
            }
            let it = ctx.hero.items;
            if it.pressed & it.trigger != 0 && lock.is_some() {
                let m = &mut w.mobys[id];
                m.set_pv_i(0x1c, if air { 4 } else { 12 });
                m.state = 2;
                ctx.hero.items.use_start(ctx.hero.group);
            }
        }
        2 => {
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x1c);
            let done = countdown(&mut t);
            m.set_pv_i(0x1c, t);
            if !done { return; }
            if let Some(target) = usize::try_from(m.pv_i(0x18) - 1).ok().filter(|&t| w.mobys.get(t).is_some_and(|x| x.state < 0x80)) {
                let d = sub(w.mobys[target].pos, muzzle);
                let l = len(d).max(1e-6);
                let mut tm = HitTemplate::new(0.0, Some(id), 0x10000, [d[0] / l, d[1] / l, 1.0, 5_627.987]);
                tm.kind = (0x825u32 << 16) | 0x0203;
                let p = w.mobys[target].pos;
                w.deliver_hit(target, &tm, [p[0], p[1], p[2], 1.0]);
            }
            let m = &mut w.mobys[id];
            m.state = 3;
            m.set_pv_i(0x1c, if air { 3 } else { 15 });
            m.set_pv_i(0x18, 0);
        }
        3 => {
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x1c);
            if countdown(&mut t) {
                m.state = 4;
                t = 7;
            }
            m.set_pv_i(0x1c, t);
        }
        4 => {
            let m = &mut w.mobys[id];
            let mut t = m.pv_i(0x1c);
            let done = countdown(&mut t);
            m.set_pv_i(0x1c, t);
            if done {
                m.state = 1;
                let idle = ctx.hero.idle_seq();
                let st = ctx.hero.state;
                ctx.hero.items.use_end(st, ctx.hero_view.seq_b, idle);
            }
        }
        _ => {}
    }
    let _ = DYNAMO;
}
