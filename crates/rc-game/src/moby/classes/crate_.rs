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
//! The stacks are found when the crates start ([`stack_search`]) and fall when a crate under them goes ([`fall`]).
//!
//! Not ported: the TNT crate 0x1f9 and the class 0x1f6's flag test beyond
//! the damage, the dust puffs (`0x334528`), the other classes' break sounds (`0x360698`), the nanotech
//! crates' pickups, the respawn's stacking search
//! (`0x31c9d8`).

use crate::hero::physics::{add, sub, V3};
use crate::moby::aim::is_crate;
use crate::moby::{bolts, mode, shards, Ctx, MobyWorld};

/// `0x5830001`: the hit flags a crate takes.
const HIT_MASK: u32 = 0x0583_0001;

/// `0x39c870`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let class = w.mobys[id].o_class;
    if w.mobys[id].pv_i(0xac) & 0xc != 0 && w.mobys[id].pv_i(0xb4) != ctx.game.frame { fall(w, id, ctx); }
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
            for off in [0xa0, 0xa4] {
                if w.mobys[id].pv_link(off).is_some_and(|o| !is_crate(w, o)) { w.mobys[id].set_pv_i(off, 0); }
            }
            let m = &mut w.mobys[id];
            m.set_pv_v(0x90, [0.0; 3]);
            m.state = 1;
            m.set_pv_i(0xa8, 0);
            if class == 0x1ff { m.set_pv_i(0xb4, m.pv_i(0xb4) + 1); }
            if m.pv_i(0xcc) == 0 {
                let f = m.pv_i(0xac) | 0x14;
                m.set_pv_i(0xac, f);
                if f & 1 == 0 { stack_search(w, id, ctx); }
            }
            let m = &mut w.mobys[id];
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
    m.grid_rect = crate::collision::mobys::GRID_NONE;
    w.grid.update(id as u16, old, crate::collision::mobys::GRID_NONE);
    w.changed = true;
}

/// `0x39d948`: a bolt crate (no ammo amount, not 501 / 0x200) drops its bolts with `rand(2)` + 2 / 1 / 0 bolt mobys
/// (by the recent breaks: below 11 / 21 / more); an ammo crate drops ammo pickups (crate::moby::classes::pickup), and
/// no bolts.
fn reward(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    let plain = m.pv_s(0xc6) == 0 && m.o_class != 0x1f5 && m.o_class != 0x200;
    if plain {
        let r = ctx.game.recent_breaks;
        let k = if r < 0x15 { if 10 < r { 1 } else { 2 } } else { 0 };
        let n = ctx.game.rng.randi(2);
        bolts::drop(w, id, 0x100, n + k, ctx);
    } else {
        if !matches!(w.mobys[id].o_class, 0x1f5 | 0x200) { super::pickup::drop_ammo(w, id, ctx); }
        if w.mobys[id].bolts != 0 { w.mobys[id].bolts = 0; }
        bolts::drop(w, id, 0x100, 0, ctx);
    }
}

/// The line down from `at` to just above the world's floor (z 0.1), as `0x2de878(a, b, 2, 0, 0)`: the hit's z and
/// moby.
fn down(w: &MobyWorld, ctx: &Ctx, from: V3, to: V3) -> Option<(f32, Option<usize>)> {
    w.coll_line(ctx, from, to, 2, None).map(|h| (h.point[2], h.moby))
}

/// `0x39d0c0`: the stack search (once, by the group's first crate to start): each crate of the group (a crate
/// without a group alone) is set to size 1, its key kept (`+0xc4`), and looks straight down from half a unit up.
/// On the world's floor it drops onto it; on a crate of its group it stands on its top (its position plus its up
/// row) and the two are linked (`+0xa4` below, `+0xa0` above); on another moby it rests there (flag 0x8, always
/// updated) — classes 0x128 / 0x129 aside, which it drops onto. A bolt crate standing upright turns a quarter turn
/// one time in two.
///
/// Not ported: the crates on movers (`0x322028` / `0x322570`, the second pass: carried with them, `+0xd0..+0xf4`),
/// the nanotech crates' removal by the save's counter (`0x1a73e0[+0xb0] < +0xf8` → `0x311688`), the third pass
/// (distances between the group's crates, unused), the effect slots (`+0xc0`, `0x1fda00`).
fn stack_search(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let f = w.mobys[id].pv_i(0xac);
    w.mobys[id].set_pv_i(0xac, f | 4);
    let group = w.mobys[id].group;
    let members: Vec<usize> = if group < 0 {
        vec![id]
    } else {
        w.groups.get(group as usize).map_or(Vec::new(), |g| g.iter().copied().filter(|&i| w.mobys.get(i).is_some_and(|m| m.state < 0x80)).collect())
    };
    for c in members {
        if !is_crate(w, c) { continue; }
        if w.mobys[c].o_class == 500 && ctx.game.rng.randi(2) != 0 && w.mobys[c].rot[0] == 0.0 {
            let m = &mut w.mobys[c];
            if m.rot[1] == 0.0 { m.rot[2] = crate::hero::physics::add_rot(m.rot[2], std::f32::consts::FRAC_PI_2); }
        }
        let m = &mut w.mobys[c];
        m.set_pv_s(0xc4, m.rebuilds as i16);
        m.set_pv_f(0xb0, 1.0);
        m.set_pv_i(0xac, m.pv_i(0xac) | 1);
        let p = m.pos;
        let Some((z, hit)) = down(w, ctx, [p[0], p[1], p[2] + 0.5], [p[0], p[1], 0.1]) else {
            w.mobys[c].set_pv_i(0xa4, 0);
            continue;
        };
        let Some(b) = hit else {
            let m = &mut w.mobys[c];
            m.pos[2] = z;
            m.set_pv_i(0xa4, 0);
            continue;
        };
        let alone = group < 0;
        if !is_crate(w, b) && (alone || !matches!(w.mobys[b].o_class, 0x128 | 0x129)) {
            let m = &mut w.mobys[c];
            m.set_pv_i(0xa4, 0);
            m.set_pv_i(0xac, m.pv_i(0xac) | 8);
            if !alone { m.update_dist = 0xff; }
            continue;
        }
        if alone { continue; }
        if w.mobys[b].group != w.mobys[c].group {
            w.mobys[c].set_pv_i(0xa4, 0);
            continue;
        }
        if !is_crate(w, b) {
            w.mobys[c].set_pv_i(0xa4, 0);
            w.mobys[c].pos[2] = z;
            continue;
        }
        let top = add(w.mobys[b].pos, w.mobys[b].rows[2]);
        let m = &mut w.mobys[c];
        m.set_pv_i(0xa4, b as i32);
        m.pos = top;
        w.mobys[b].set_pv_i(0xa0, c as i32);
    }
    w.changed = true;
}

/// `0x39dfd8`: the fall of the stack `id` is in, once a frame (`+0xb4`), from the bottom crate up. Each crate falls
/// (gravity 9.8 a second², at most 0.3 a tick either way, `+0x98`) until it reaches the floor (the line from three
/// quarters of its size up to its next place) or, above another, that one's top (1 up); there it stops. While it
/// falls, Ratchet under it (within 0.55 across in its frame, below it by less than his capsule's top and radius)
/// breaks it as a hit would.
///
/// Not ported: the crates on movers (`+0xf0`: carried by `0x3223e8` / `0x322668`), the TNT crates' (0x1f9) fuse when
/// they land hard on a broken stack (state 5).
fn fall(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state == 0 { return; }
    let mut bottom = id;
    for _ in 0..64 {
        match w.mobys[bottom].pv_link(0xa4) {
            Some(b) => bottom = b,
            None => break,
        }
    }
    let mut next = Some(bottom);
    let mut guard = 0;
    while let Some(c) = next {
        guard += 1;
        if guard > 64 { return; }
        next = w.mobys[c].pv_link(0xa0);
        let mut f = w.mobys[c].pv_i(0xac);
        if f & 4 == 0 { f &= !0x10; }
        f &= !4;
        w.mobys[c].set_pv_i(0xac, f);
        if !is_crate(w, c) || f & 2 != 0 { return; }
        w.mobys[c].set_pv_i(0xb4, ctx.game.frame);
        if w.mobys[c].state == 0 { continue; }
        // `0x2ef318`: Ratchet's place from the crate, through its rows.
        let m = &w.mobys[c];
        let d = sub(ctx.hero.pos, m.pos);
        let l: V3 = std::array::from_fn(|k| d[0] * m.rows[0][k] + d[1] * m.rows[1][k] + d[2] * m.rows[2][k]);
        if l[0].abs() < 0.55 && l[1].abs() < 0.55 && l[2] < 0.0 && -(ctx.hero.cap_top + ctx.hero.cap_radius) < l[2] && matches!(m.state, 1 | 5) {
            break_crate(w, c, ctx);
            reward(w, c, ctx);
        }
        let vz = (w.mobys[c].pv_f(0x98) - 0.002_722_222_4).clamp(-0.3, 0.3);
        w.mobys[c].set_pv_f(0x98, vz);
        if w.mobys[c].pv_i(0xf0) != 0 { continue; }
        let m = &w.mobys[c];
        let p = m.pos;
        let land = match m.pv_link(0xa4) {
            None => down(w, ctx, [p[0], p[1], p[2] + m.pv_f(0xb0) * 0.75], [p[0], p[1], p[2] + vz]).map(|(z, _)| z),
            Some(b) => {
                let top = w.mobys[b].pos[2] + 1.0;
                (p[2] + vz < top).then_some(top)
            }
        };
        let m = &mut w.mobys[c];
        match land {
            None => {
                m.set_pv_i(0xac, m.pv_i(0xac) | 4);
                m.pos = add(m.pos, m.pv_v(0x90));
            }
            Some(z) => {
                m.pos[2] = z;
                m.set_pv_f(0x98, 0.0);
            }
        }
        w.changed = true;
    }
}
