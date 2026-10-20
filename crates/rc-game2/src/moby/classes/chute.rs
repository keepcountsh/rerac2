//! The chutes (class 0x2a7, L00 `0x39f588`): while Ratchet slides (group 0x10), the closest of its paths (`+0x68`
//! [0..0x1c] path indices, `[0x1c]` their count) within its range (`[0x19]`) steers him: the point `3.7 − 0.02·t` (at
//! least 0.7) ahead along it gives the slide's yaw (`0x18a960`) and bank (`0x18a964`), the push (`0x189f20`) toward it
//! at a speed easing to `[0x18]` by 1/277, and holds the slide (`0x18a020` ≥ 30). Past the path's end the push fades
//! (×0.99 a tick).

use crate::hero::physics::{atan2, dist, len, len2, set_len, sub, V3};
use crate::moby::{Ctx, MobyWorld};
use crate::paths::{advance, closest};

pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let m = &w.mobys[id];
    if m.pvars.is_empty() || m.pv_i(0) == -1 || m.pv_i(0x70) == 0 { return; }
    if m.pv_i(0x68) == 0 {
        let m = &mut w.mobys[id];
        m.set_pv_i(0x68, 1);
        m.update_dist = 0xff;
    }
    if ctx.hero.group != 0x10 { return; }
    let m = &w.mobys[id];
    let n = m.pv_i(0x70).clamp(0, 0x1c) as usize;
    let pos = ctx.hero.pos;
    let mut best: Option<(Vec<[f32; 4]>, V3, usize, f32, f32)> = None;
    for k in 0..n {
        let pts = ctx.level.walked_path(m.pv_i(4 * k));
        let Some((q, seg, t)) = closest(&pts, 1000.0, 5.0, 0.0, pos, false) else { continue };
        let d = dist(pos, q);
        if best.as_ref().map_or(true, |b| d < b.4) { best = Some((pts, q, seg, t, d)); }
    }
    let Some((pts, q, mut seg, mut t, d)) = best else { return };
    let ahead = (3.7 - ctx.hero.timer as f32 * 0.02).max(0.7);
    let (p, end) = advance(&pts, ahead, &mut seg, &mut t, false);
    if m.pv_f(0x64) < d { return; }
    let speed_to = m.pv_f(0x60);
    let m = &mut w.mobys[id];
    if !end {
        let a = sub(p, q);
        let mut to = sub(p, pos);
        ctx.hero.slide_bank = atan2(len2(a), a[2]);
        ctx.hero.slide_yaw = atan2(to[0], to[1]);
        to[2] = 0.0;
        let mut s = ctx.hero.slide_speed;
        crate::hero::physics::approach(speed_to, 0.003_611_111, &mut s);
        ctx.hero.slide_speed = s;
        if 0.001 < len(to) { ctx.hero.push = set_len(to, s); }
        ctx.hero.push[2] = 0.0;
        let pu = ctx.hero.push;
        m.set_pv_v(0x50, pu);
        if ctx.hero.slide_hold < 0x1e { ctx.hero.slide_hold = 0x1e; }
    } else {
        let v = m.pv_v(0x50);
        let v = [v[0] * 0.99, v[1] * 0.99, v[2] * 0.99];
        m.set_pv_v(0x50, v);
        ctx.hero.push = v;
    }
}
