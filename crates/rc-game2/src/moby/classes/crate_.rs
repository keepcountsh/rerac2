//! The crates, classes 500 (bolt crate), 501, 511 (ammo crate) (L00 `0x39c870`, one update for the three): wait for
//! a hit (`0x31dce8` with mask 0x5830001; damage above 0), break (`0x39daa0`: the stack above and below re-linked
//! and set to fall, hidden, no collision, the break counter), drop the reward (`0x39d948`: a bolt crate's bolts with
//! 1..4 bolt mobys, fewer the more crates broke lately; the others their pickups), then go (state 3: deleted, or
//! state 6 to come back for the ammo crates).
//!
//! **pvars** (0x110): +0xa0 / +0xa4 the crates above / below, +0xac flags (0x4 / 0x8 falling, 0x20 the stack broke,
//! 0x10 rests on the floor), +0xb4 the frame of the last settle, +0xc6 the ammo crate's amount, +0xc8 comes back,
//! +0xc9 its timer, +0xcb the pickup type, +0xcc rests on something, +0xfc / +0x100 counters.
//!
//! The break throws the crate's pieces (crate::moby::shards) and plays its sound.
//!
//! Not ported: the stack's fall and settle (`0x39dfd8`), the TNT crate 0x1f9 and the class 0x1f6's flag test beyond
//! the damage, the dust puffs (`0x334528`), the other classes' break sounds (`0x360698`), the pickups of the
//! non-bolt crates (`0x31aa10` / `0x3d2d30`: the ammo and nanotech pickups), the respawn's stacking search
//! (`0x31c9d8`).

use crate::moby::{bolts, mode, shards, Ctx, MobyWorld};

/// `0x5830001`: the hit flags a crate takes.
const HIT_MASK: u32 = 0x0583_0001;

/// `0x39c870`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class = w.mobys[id].o_class;
    if w.mobys[id].pv_link(0xa0).is_none() {
        let f = w.mobys[id].pv_s(0x3e);
        w.mobys[id].set_pv_s(0x3e, f | 1);
    }
    // `0x31dce8(moby, mask, 0)`: its record when the flags match (a record without them is dropped).
    let mut hit = w.hit_message(id);
    if let Some(h) = hit {
        if h.tmpl.flags & HIT_MASK == 0 {
            w.clear_hit(id);
            hit = None;
        }
    }
    if hit.is_some_and(|h| h.tmpl.flags == 0x0100_0000) { hit = None; }
    match w.mobys[id].state {
        0 => {
            let m = &mut w.mobys[id];
            m.set_pv_v(0x90, [0.0; 3]);
            m.state = 1;
            m.set_pv_i(0xa8, 0);
            if class == 0x1ff { m.set_pv_i(0xb4, m.pv_i(0xb4) + 1); }
            if m.pv_i(0xcc) == 0 { m.set_pv_i(0xac, m.pv_i(0xac) | 0x14); }
            m.set_pv_i(0xfc, 0);
            if class == 0x1ff { m.set_pv_s(0xc6, 100); }
            // Targets for the aims (the class header's mode bits give 0x1000 / 0x4000).
            return;
        }
        1 => {
            let Some(h) = hit else { return };
            if h.tmpl.damage <= 0.0 { return; }
            if class == 0x1f6 && h.tmpl.flags & 0x20000 == 0 { return; }
            break_crate(w, id, ctx);
            reward(w, id, ctx);
        }
        2 => {
            let m = &mut w.mobys[id];
            m.timer = m.timer.wrapping_sub(1);
            if m.timer != 0xff { return; }
            m.state = 1;
            w.clear_hit(id);
            return;
        }
        3 => {
            if w.mobys[id].pv_b(0xc8) == 0 {
                w.delete(id);
                return;
            }
            let m = &mut w.mobys[id];
            m.has_collision = false;
            m.mode = (m.mode & !mode::TARGET) | 0x41;
            if 1 < m.bolts { m.bolts = 1; }
            m.state = 6;
            if m.pv_b(0xc9) != 0xff {
                let t = ctx.game.rng.rand_range(0x1e, 0x3c) as u8;
                w.mobys[id].set_pv_b(0xc9, t);
            }
        }
        6 => {
            let m = &mut w.mobys[id];
            m.set_pv_i(0x100, m.pv_i(0x100) + 1);
            if m.pv_b(0xc9) == 0xff { return; }
            // `0x2eeb10`: the timer runs out (out of sight: `0x2f0080` not ported, so always) → back, resting.
            let t = m.pv_b(0xc9);
            if t > 1 {
                m.set_pv_b(0xc9, t - 1);
                return;
            }
            m.set_pv_b(0xc9, 0);
            m.state = 1;
            m.drawn = 1;
            m.mode = (m.mode & !1) | mode::TARGET;
            m.has_collision = true;
            m.set_pv_i(0xa4, 0);
            m.set_pv_i(0xa0, 0);
            w.changed = true;
        }
        _ => return,
    }
    w.clear_hit(id);
}

/// `0x39daa0(moby, explosive, hit)`: the stack re-linked (the crate above rests on the one below and both fall), the
/// crates above marked broken-under, the break counter (`$gp+0x3fc` decays with time, + the shard count), then broken:
/// state 3, timer 15, hidden (mode 1), no collision.
fn break_crate(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    // Its sound, one crate's a frame (`0x1b2794`): class sound 0 (0x1fa / 0x1ff play another class's, not ported).
    if 1 < (ctx.game.frame - ctx.game.crate_sound_frame).abs() && !matches!(w.mobys[id].o_class, 0x1fa | 0x1ff) {
        ctx.host.play_sound(&w.mobys[id], 0, 0);
        ctx.game.crate_sound_frame = ctx.game.frame;
    }
    let above = w.mobys[id].pv_link(0xa0);
    let below = w.mobys[id].pv_link(0xa4);
    if let Some(a) = above {
        w.mobys[a].set_pv_i(0xa4, below.map_or(0, |b| b as i32));
        let f = w.mobys[a].pv_i(0xac);
        w.mobys[a].set_pv_i(0xac, f | 0x24);
    }
    if let Some(b) = below {
        w.mobys[b].set_pv_i(0xa0, above.map_or(0, |a| a as i32));
        let f = w.mobys[b].pv_i(0xac);
        w.mobys[b].set_pv_i(0xac, f | 0x24);
    }
    let mut cur = Some(id);
    let mut guard = 0;
    while let Some(c) = cur {
        let f = w.mobys[c].pv_i(0xac);
        w.mobys[c].set_pv_i(0xac, f | 0x20);
        cur = w.mobys[c].pv_link(0xa0);
        guard += 1;
        if guard > 64 { break; }
    }
    // The recent-breaks count: decays by a quarter of the ticks since the last break (none within 0xf0: reset).
    let g = &mut *ctx.game;
    let since = g.frame - g.last_break_frame;
    if since < 0xf1 {
        if since != 0 { g.recent_breaks = (g.recent_breaks - (since >> 2)).max(0); }
    } else {
        g.recent_breaks = 0;
    }
    g.last_break_frame = g.frame;
    let free = w.free_slots() as i32;
    let r = g.recent_breaks;
    let shards = if 0xfb <= free && r <= 4 {
        4
    } else if 0xc9 <= free && r <= 9 {
        3
    } else if 0x97 <= free && r <= 0xe {
        2
    } else if 100 < free {
        (r < 0x14) as i32
    } else {
        0
    };
    g.recent_breaks += shards;
    // The pieces (`0x327498(3, 6, 3, 6, crate, mask, 0, 0xff)`): the bolt crate's 0, 2 and 5 by the count above, the
    // nanotech crate's first n − 1 (the platform's velocity, `0x3222a8`, not ported).
    let mask = match w.mobys[id].o_class {
        500 | 0x1ff => match shards { 0 => 0, 1 => 1, 2 => 5, _ => 0x25 },
        0x1f5 | 0x200 if 0 < shards => ((1u32 << (shards - 1)) - 1).min(0xf),
        _ => 0,
    };
    if mask != 0 { shards::break_into(w, id, ctx, (3.0, 6.0), (3.0, 6.0), mask, [0.0; 3], 0xff); }
    let m = &mut w.mobys[id];
    m.state = 3;
    m.timer = 0xf;
    m.mode |= 1;
    m.has_collision = false;
    let old = m.grid_rect;
    m.grid_rect = rc_game::collision_query::mobys::GRID_NONE;
    w.grid.update(id as u16, old, rc_game::collision_query::mobys::GRID_NONE);
    w.changed = true;
}

/// `0x39d948`: a bolt crate (no ammo amount, not 501 / 0x200) drops its bolts with `rand(2)` + 2 / 1 / 0 bolt mobys
/// (by the recent breaks: below 11 / 21 / more); the others drop their pickups (not ported) and no bolts.
fn reward(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let plain = m.pv_s(0xc6) == 0 && m.o_class != 0x1f5 && m.o_class != 0x200;
    if plain {
        let r = ctx.game.recent_breaks;
        let k = if r < 0x15 { if 10 < r { 1 } else { 2 } } else { 0 };
        let n = ctx.game.rng.randi(2);
        bolts::drop(w, id, 0x100, n + k, ctx);
    } else {
        if w.mobys[id].bolts != 0 { w.mobys[id].bolts = 0; }
        bolts::drop(w, id, 0x100, 0, ctx);
    }
}
