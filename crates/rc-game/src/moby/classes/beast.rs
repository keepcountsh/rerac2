//! Oozla's swamp beast, **0x09fc** (L01 `0x3bb338`, 15 placed): the big creature Ratchet rides across the swamp. It
//! waits at the start of its ride path `+0x290` (the level's paths), carries him along it when he stands on it (or
//! when something hits it), dives at the end, swims back under the water along its return path `+0x294` and comes up
//! at the start again.
//!
//! **States**: 0 the set-up (once the water has a height): at the ride path's first point facing the second, the
//! paths' first segment lengths inverted (`+0x2b4` / `+0x2b8`), the path height `+0x2d4`; hidden (1) with `+0x2d8`
//! set until the missions `+0x2dc..+0x2ec` (−1 none) are all done on this level, else waiting (2). 2 waiting: idles
//! (sequences 0 / 1, now and then at 0.8..1.2 speed); Ratchet on it (grounded on it a tick) or a hit (not from a
//! 0x0ce7) wakes it (sequence 2 from frame 5; 3). 3 waking: at its sequence's end, off (sequence 3, the speed 10;
//! 4). 4 the ride: the speed toward its point's `w` (when above 0) by 1/3 a tick, its place `+0x2b0` along the path
//! eased toward the last point (`0x31ed78`: 0.005 / 0.00167 × the inverse length, at most speed / 60 of it), its yaw
//! half way to the path's each tick; while he rides, a camera tweak (priority 5, 3°) faces the point 10 ahead, or
//! the moby `+0x298` within `+0x29c`. At the end: sequence 6, the dive (5). 5 / 7: the dive toward the return path's
//! start 1.5 under (5) or the rise at the ride path's start (7), eased in (`0x31f4b0`, at most 0.1 a tick), its
//! height the ride path start's plus [`DIVE`] / [`RISE`] over 180 ticks; then it turns to the path (`0x31f230`) and
//! swims back (6) or waits (2). 6 back along the return path 1.5 under the water (its angles from the path).
//!
//! Every state from 2: its path height is kept in `+0x2a8` and it floats with the water (`water_z` minus the path's
//! start height), rocking up to 2° (`0x31a3c0`: `+0x2a0` by 0.05, `+0x2a4` by 0.01 a tick); above the water (−0.2) it
//! carries Ratchet (`0x322668`, without its turn).
//!
//! Not ported: the neck's and tail's aims (`0x324208` at the joint blocks `+0x90..`, `0x327308` at `+0x300`), the
//! splashes and ripples (`0x339b60`), the pad rumble (`0x2d6d60` / `0x2d6f38`, its handle `+0x2f0`).

use crate::hero::physics::{add, add_rot, approach, atan2, dist, dist2, ease, ease_speed, len, len2, rot_dist, scale, sub, sub_rot, turn_ease, V3};
use crate::moby::{carry_hero, Ctx, MobyWorld};
use super::leaper::{play, seq_a};
use super::props::{seq, wrapped};

/// The ride and return paths, the target moby and its range.
const RIDE: usize = 0x290;
const BACK: usize = 0x294;
const TARGET: usize = 0x298;
const RANGE: usize = 0x29c;
/// The place along the current path and its speed.
const T: usize = 0x2b0;
const TV: usize = 0x2bc;
/// The camera tweak's slot + 1 (0: none).
const CAM: usize = 0x2ac;

/// A path's points.
fn path(ctx: &Ctx, i: i32) -> Option<Vec<[f32; 4]>> { usize::try_from(i).ok().and_then(|i| ctx.level.paths.get(i)).filter(|p| 2 <= p.len()).cloned() }

fn point(p: &[[f32; 4]], k: usize) -> V3 { [p[k][0], p[k][1], p[k][2]] }

/// `0x343fb8(t, path, 0, &pos, &angles, 0)`: the place at `t` along an open path (between points ⌊t⌋ and ⌊t⌋ + 1, the
/// last segment held up to its end) and its pitch (−atan2(dz, |dxy|)) and yaw of the direction, blended from this
/// segment's into the next's.
fn along(p: &[[f32; 4]], t: f32) -> (V3, f32, f32) {
    let n = p.len() as i32;
    let mut i = t as i32;
    let end = n - 2 <= i;
    if end { i = n - 2; }
    let mut f = t - i as f32;
    if end && 1.0 < f { f = 1.0; }
    let at = |k: i32| point(p, k.rem_euclid(n) as usize);
    let (a, b) = (at(i), at(i + 1));
    let pos = add(a, scale(sub(b, a), f));
    let d0 = sub(b, a);
    let dir = if end { d0 } else { add(d0, scale(sub(sub(at(i + 2), b), d0), f)) };
    (pos, -atan2(len2(dir), dir[2]), atan2(dir[0], dir[1]))
}

fn show(w: &mut MobyWorld, id: usize, on: bool) {
    let solid = w.class(w.mobys[id].o_class).is_some_and(|c| c.has_collision);
    let m = &mut w.mobys[id];
    if on { (m.mode, m.drawn, m.has_collision) = (m.mode & !1, 1, solid) } else { (m.mode, m.drawn, m.has_collision) = (m.mode | 1, 0, false) }
}

fn release_cam(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let slot = w.mobys[id].pv_i(CAM);
    if slot != 0 {
        ctx.game.cam_tweaks.release(slot as usize - 1);
        w.mobys[id].set_pv_i(CAM, 0);
    }
}

/// 0x09fc (`0x3bb338`).
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let water = ctx.game.water_z;
    let m = &w.mobys[id];
    let (Some(ride), Some(back)) = (path(ctx, m.pv_i(RIDE)), path(ctx, m.pv_i(BACK))) else { return };
    let (old, state) = (m.pos, m.state);
    if 1 < state {
        let z = m.pv_f(0x2a8);
        w.mobys[id].pos[2] = z;
    }
    let riding = ctx.hero.ground_moby == Some(id) && ctx.hero.grounded_ticks != 0;
    match state {
        0 => {
            if water <= 0.0 { return; }
            let m = &mut w.mobys[id];
            m.set_pv_f(0x2b4, 1.0 / dist(point(&ride, 0), point(&ride, 1)));
            m.set_pv_i(0x2f0, -1);
            m.set_pv_f(0x2b8, 1.0 / dist(point(&back, 0), point(&back, 1)));
            m.pos = point(&ride, 0);
            m.rot[2] = atan2(ride[1][0] - m.pos[0], ride[1][1] - m.pos[1]);
            m.set_pv_f(0x2d4, m.pos[2]);
            m.set_pv_i(CAM, 0);
            let s = (ctx.game.rng.rand() & 1) as u8;
            if seq(w, id, ctx) != s {
                let (s, blend) = ((ctx.game.rng.rand() & 1) as u8, ctx.game.rng.rand_range(10, 20));
                ctx.host.set_anim(&w.mobys[id], s, 0, blend as u8);
            }
            let m = &mut w.mobys[id];
            m.set_pv_i(0x8c, m.pv_i(0x8c) | 0x10);
            if m.pv_i(0x2d8) == 0 {
                m.set_state(2, -1);
            } else {
                show(w, id, false);
                w.mobys[id].set_state(1, -1);
            }
        }
        1 => {
            show(w, id, false);
            let level = ctx.game.level.max(0) as usize;
            let done = (0..5).all(|k| {
                let i = w.mobys[id].pv_i(0x2dc + 4 * k);
                i == -1 || ctx.game.missions.get(level * 0x10 + i.max(0) as usize).is_some_and(|b| b & 0x80 != 0)
            });
            if !done { return; }
            show(w, id, true);
            w.mobys[id].set_state(2, -1);
        }
        2 => {
            // `0x31dce8(m, 0x10000, 0)`.
            let hit = w.hit_message(id).filter(|h| h.tmpl.flags & 0x10000 != 0);
            let woken = hit.is_some_and(|h| h.tmpl.source.and_then(|s| w.mobys.get(s)).is_none_or(|s| s.o_class != 0x0ce7));
            if !woken {
                if dist2(w.mobys[id].pos, ctx.hero.pos) < 10.0 {
                    if seq(w, id, ctx) != 0 {
                        let blend = ctx.game.rng.rand_range(10, 20);
                        play(w, id, ctx, 0, 0, blend);
                    }
                } else if wrapped(w, id, ctx) && ctx.game.rng.randi(1) == 0 {
                    match seq_a(w, id, ctx) {
                        0 => {
                            if ctx.game.rng.randi(2) == 0 { play(w, id, ctx, 1, 0, 10); }
                        }
                        1 => play(w, id, ctx, 0, 0, 10),
                        _ => {}
                    }
                    let speed = ctx.game.rng.randf(0.8, 1.2);
                    ctx.host.set_anim_speed(&w.mobys[id], speed);
                }
            }
            if woken || riding {
                play(w, id, ctx, 2, 5, 3);
                w.mobys[id].set_state(3, -1);
            }
            w.clear_hit(id);
        }
        3 => {
            if wrapped(w, id, ctx) {
                play(w, id, ctx, 3, 0, 6);
                let m = &mut w.mobys[id];
                m.set_pv_f(0x2cc, 10.0);
                m.set_pv_f(0x2d0, 10.0);
                m.set_state(4, -1);
                w.clear_hit(id);
            }
        }
        4 => {
            let n = ride.len();
            let m = &mut w.mobys[id];
            let w_at = ride[(m.pv_f(T) as usize).min(n - 1)][3];
            if 0.0 < w_at { m.set_pv_f(0x2cc, w_at); }
            let mut speed = m.pv_f(0x2d0);
            approach(m.pv_f(0x2cc), 0.333_333_34, &mut speed);
            m.set_pv_f(0x2d0, speed);
            let inv = m.pv_f(0x2b4);
            let (mut t, mut v) = (m.pv_f(T), m.pv_f(TV));
            ease((n - 1) as f32, inv * 0.005, inv * 0.001_666_666_7, speed * inv * 0.016_666_668, &mut t, &mut v);
            let (pos, _, yaw) = along(&ride, t);
            m.pos = pos;
            m.rot[2] = add_rot(m.rot[2], sub_rot(yaw, m.rot[2]) * 0.5);
            let (s, c) = m.rot[2].sin_cos();
            let ahead = add(m.pos, [c * 10.0, s * 10.0, 0.0]);
            m.set_pv_f(T, t);
            m.set_pv_f(TV, v);
            if (n - 1) as f32 <= t {
                play(w, id, ctx, 6, 0, 10);
                let m = &mut w.mobys[id];
                m.set_pv_i(0x2f0, -1);
                m.state_ticks = 0;
                m.state = 5;
                m.set_pv_i(0x2c0, 0x14);
                m.set_pv_f(T, 0.0);
            }
            if riding {
                let m = &w.mobys[id];
                let mut look = ahead;
                if let Some(o) = usize::try_from(m.pv_i(TARGET)).ok().and_then(|o| w.mobys.get(o)) {
                    if dist2(m.pos, o.pos) < m.pv_f(RANGE) { look = o.pos; }
                }
                let mut slot = m.pv_i(CAM);
                if slot == 0 {
                    let Some(s) = ctx.game.cam_tweaks.request(5) else { return };
                    slot = s as i32 + 1;
                    w.mobys[id].set_pv_i(CAM, slot);
                }
                let tw = &mut ctx.game.cam_tweaks.slots[slot as usize - 1];
                tw.dir = sub(look, ctx.hero.pos);
                tw.rate_deg = 3;
                tw.face_rest = true;
            } else {
                release_cam(w, id, ctx);
            }
        }
        5 | 7 => {
            let diving = state == 5;
            let (dest, to) = if diving { (sub(point(&back, 0), [0.0, 0.0, 1.5]), point(&back, 1)) } else { (point(&ride, 0), point(&ride, 1)) };
            let yaw_t = atan2(to[0] - dest[0], to[1] - dest[1]);
            let m = &mut w.mobys[id];
            let mut d = sub(dest, m.pos);
            let mut v = m.pv_f(0x2c4);
            ease_speed(len(d), 0.001_666_666_7, 0.001_666_666_7, 0.100_000_01, &mut v);
            m.set_pv_f(0x2c4, v);
            if v < len(d) { d = scale(d, v / len(d)); }
            m.pos = add(m.pos, d);
            let curve = if diving { &DIVE } else { &RISE };
            let k = (m.state_ticks.max(0) as usize).min(180);
            m.pos[2] = ride[0][2] + curve[k];
            if k < 180 {
                m.state_ticks += 1;
            } else {
                play(w, id, ctx, 0, 0, 10);
                let m = &mut w.mobys[id];
                let (mut yaw, mut yv) = (m.rot[2], m.pv_f(0x2c8));
                turn_ease(yaw_t, 0.000_387_851, 0.000_339_369_6, 0.026_179_943, &mut yaw, &mut yv, 0);
                m.rot[2] = yaw;
                m.set_pv_f(0x2c8, yv);
                if rot_dist(yaw, yaw_t) < 0.001 {
                    m.state = if diving { 6 } else { 2 };
                    m.set_pv_f(T, 0.0);
                    w.clear_hit(id);
                }
            }
            release_cam(w, id, ctx);
        }
        6 => {
            let n = back.len();
            let m = &mut w.mobys[id];
            let inv = m.pv_f(0x2b8);
            let (mut t, mut v) = (m.pv_f(T), m.pv_f(TV));
            ease((n - 1) as f32, inv * 0.002_222_222_3, inv * 0.002_222_222_3, inv * 0.166_666_67, &mut t, &mut v);
            m.set_pv_f(T, t);
            m.set_pv_f(TV, v);
            let (pos, pitch, yaw) = along(&back, t);
            m.pos = sub(pos, [0.0, 0.0, 1.5]);
            (m.rot[1], m.rot[2]) = (pitch, yaw);
            if (n - 1) as f32 <= t {
                play(w, id, ctx, 7, 0, 10);
                let m = &mut w.mobys[id];
                m.state_ticks = 0;
                m.state = 7;
                m.set_pv_f(T, 0.0);
            }
        }
        _ => {}
    }
    let m = &mut w.mobys[id];
    if m.state < 2 { return; }
    m.set_pv_f(0x2a8, m.pos[2]);
    m.pos[2] = m.pos[2] - m.pv_f(0x2d4) + water;
    // `0x31a3c0(2°, 0.05, 0.01, m, +0x2a0, +0x2a4)`: the rocking.
    let (a, b) = (m.pv_f(0x2a0), m.pv_f(0x2a4));
    m.rot[0] = 0.034_906_59 * a.sin() * b.sin();
    m.rot[1] = 0.034_906_59 * a.sin() * b.cos();
    m.set_pv_f(0x2a0, add_rot(a, 0.05));
    m.set_pv_f(0x2a4, add_rot(b, 0.01));
    let (pos, yaw) = (m.pos, m.rot[2]);
    if water - 0.2 < pos[2] { carry_hero(ctx, id, old, yaw, pos, yaw); }
}

/// `0x2a4690`: the height under the ride path's start over the 180 ticks of the rise (state 7).
const RISE: [f32; 181] = [
    -9.397, -9.2537, -9.109, -8.963, -8.8158, -8.6675, -8.518, -8.3675, -8.2161, -8.0638,
    -7.9107, -7.7569, -7.6024, -7.4474, -7.2918, -7.1358, -6.9794, -6.8228, -6.6659, -6.5089,
    -6.3518, -6.1947, -6.0377, -5.8808, -5.7242, -5.5678, -5.4118, -5.2563, -5.1012, -4.9467,
    -4.7929, -4.6398, -4.4875, -4.3361, -4.1856, -4.0362, -3.8878, -3.7406, -3.5946, -3.45,
    -3.3067, -3.1649, -3.0245, -2.8858, -2.7488, -2.6135, -2.48, -2.3484, -2.2188, -2.0912,
    -1.9657, -1.8424, -1.7214, -1.6026, -1.4863, -1.3724, -1.2611, -1.1524, -1.0464, -0.9431,
    -0.8427, -0.7465, -0.6555, -0.5698, -0.4891, -0.4133, -0.3423, -0.2759, -0.2139, -0.1562,
    -0.1027, -0.0533, -0.0076, 0.0343, 0.0726, 0.1075, 0.1391, 0.1676, 0.1931, 0.2158,
    0.2357, 0.2532, 0.2682, 0.281, 0.2917, 0.3004, 0.3074, 0.3127, 0.3164, 0.3188,
    0.32, 0.3201, 0.3193, 0.3177, 0.3155, 0.3128, 0.3098, 0.3066, 0.3033, 0.2991,
    0.2931, 0.2854, 0.2761, 0.2654, 0.2535, 0.2404, 0.2263, 0.2114, 0.1957, 0.1795,
    0.1628, 0.1459, 0.1287, 0.1116, 0.0946, 0.0778, 0.0615, 0.0457, 0.0306, 0.0163,
    0.003, -0.0093, -0.0202, -0.0298, -0.0379, -0.0444, -0.049, -0.0525, -0.0556, -0.0585,
    -0.061, -0.0632, -0.0652, -0.0668, -0.0682, -0.0693, -0.0702, -0.0708, -0.0711, -0.0713,
    -0.0712, -0.0709, -0.0704, -0.0697, -0.0689, -0.0678, -0.0666, -0.0653, -0.0638, -0.0622,
    -0.0604, -0.0586, -0.0566, -0.0546, -0.0524, -0.0502, -0.0479, -0.0456, -0.0432, -0.0407,
    -0.0383, -0.0358, -0.0333, -0.0308, -0.0284, -0.0259, -0.0235, -0.0211, -0.0187, -0.0165,
    -0.0142, -0.0121, -0.01, -0.0081, -0.0062, -0.0045, -0.0028, -0.0014, 0.0, 0.0,
    0.0,
];
/// `0x2a4970`: the same over the dive (state 5).
const DIVE: [f32; 181] = [
    0.0, -0.0255, -0.0543, -0.0855, -0.1181, -0.1514, -0.1844, -0.2161, -0.2458, -0.2725,
    -0.2954, -0.3134, -0.3258, -0.3316, -0.3299, -0.3231, -0.314, -0.3029, -0.2899, -0.2751,
    -0.2586, -0.2407, -0.2214, -0.201, -0.1794, -0.157, -0.1339, -0.1101, -0.0858, -0.0613,
    -0.0365, -0.0117, 0.0129, 0.0373, 0.0613, 0.0847, 0.1075, 0.1294, 0.1503, 0.17,
    0.1885, 0.2056, 0.2211, 0.2349, 0.2468, 0.2567, 0.2644, 0.2699, 0.2729, 0.2748,
    0.2769, 0.2791, 0.2816, 0.2841, 0.2867, 0.2893, 0.2919, 0.2944, 0.2968, 0.2991,
    0.3011, 0.3029, 0.3044, 0.3056, 0.3064, 0.3068, 0.3068, 0.3062, 0.3052, 0.3035,
    0.3012, 0.2982, 0.2945, 0.29, 0.2848, 0.2787, 0.2717, 0.2638, 0.2549, 0.245,
    0.234, 0.222, 0.2088, 0.1944, 0.1787, 0.1618, 0.1436, 0.124, 0.103, 0.0806,
    0.0567, 0.0312, 0.0042, -0.0244, -0.0547, -0.0868, -0.1205, -0.1561, -0.1934, -0.2327,
    -0.2739, -0.317, -0.3622, -0.4109, -0.4647, -0.5234, -0.5871, -0.6554, -0.7285, -0.8061,
    -0.8882, -0.9747, -1.0654, -1.1603, -1.2593, -1.3622, -1.469, -1.5796, -1.6939, -1.8117,
    -1.933, -2.0577, -2.1857, -2.3168, -2.451, -2.5882, -2.7283, -2.8712, -3.0167, -3.1648,
    -3.3154, -3.4684, -3.6237, -3.7811, -3.9407, -4.1022, -4.2656, -4.4308, -4.5977, -4.7662,
    -4.9361, -5.1075, -5.2801, -5.454, -5.6289, -5.8048, -5.9817, -6.1593, -6.3376, -6.5165,
    -6.6959, -6.8757, -7.0558, -7.2361, -7.4164, -7.5968, -7.7771, -7.9572, -8.137, -8.3164,
    -8.4953, -8.6736, -8.8512, -9.028, -9.204, -9.3789, -9.5527, -9.7254, -9.8967, -10.0667,
    -10.2352, -10.4021, -10.5673, -10.7307, -10.8922, -11.0517, -11.2092, -11.3645, -11.5175, -11.5175,
    -11.5175,
];
