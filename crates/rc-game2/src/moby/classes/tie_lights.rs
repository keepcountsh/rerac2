//! 0x0ce9 (L00 `0x3f45e0`, on levels 0, 1, 7, 9, 13, 16 and 26): a tie group's lights. Its pvars name a tie group
//! (`+0x68`[0], the groups `0x1a9014`) and one of two modes:
//!
//! * **Colour** (`[7]` ≠ 0, state 1): every tick, while the camera is in its cuboid `[0x19]` (−1: always), the phase
//!   `[3]` turns by `[1]` (degrees × π / 10800 a tick, `0x2efe60`) and the colour `[4]` follows it: with no threshold
//!   (`[2]` = 0) the colours `[5]` → `[6]` by `sin(phase) · 0.5 + 0.5` (`0x2ef460`, `0x2f0000`); with one (`[2]`°),
//!   a tenth of the way a tick toward `[6]` past it and `[5]` below. A changed colour goes to every tie of the group
//!   (`0x3701d0`: their vertex colours one flat colour; [`MobyWorld::tie_colours`]). Aranos's ship lights pulse so.
//! * **Glow** (`[7]` = 0, state 2): while the camera is in its cuboid it registers `0x3f4998` on the frame's second
//!   draw list (`0x2e7920`; crate::fx), which draws a glow at each tie of the group ([`draw`]).
//!
//! The first update (state 0) converts the angles, packs the colours from the bytes `+0x20..0x2d`, sets the moby's
//! alpha (`[0x1a]`, else 0xff), and deletes it when the group is not there.

use crate::fx::FxQuad;
use crate::hero::physics::{add, add_rot, cross, dot, scale, sub, V3};
use crate::moby::{Ctx, MobyWorld};

/// `0x3f45e0`.
pub fn update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    match w.mobys[id].state {
        0 => init(w, id),
        1 if active(w, id, ctx.level, ctx.host.camera_pos()) => pulse(w, id),
        _ => {}
    }
}

/// The camera in its cuboid `[0x19]` (−1: always; `0x321858` with the camera `0x1b8d40`).
pub fn active(w: &MobyWorld, id: usize, level: &crate::level::LevelData, cam: V3) -> bool {
    let vol = w.mobys[id].pv_i(0x64);
    vol == -1 || level.in_cuboid(vol, cam)
}

fn group(w: &MobyWorld, id: usize) -> Option<&Vec<u16>> {
    usize::try_from(w.mobys[id].pv_i(0)).ok().and_then(|g| w.tie_groups.get(g)).filter(|g| !g.is_empty())
}

fn init(w: &mut MobyWorld, id: usize) {
    if group(w, id).is_none() {
        w.delete(id);
        return;
    }
    let m = &mut w.mobys[id];
    let b = |m: &crate::moby::Moby, o: usize| m.pv_b(o) as u32;
    m.set_pv_f(8, m.pv_f(8) * 0.017_453_294);
    m.set_pv_f(4, m.pv_f(4) * 0.000_290_888_25);
    let (c5, c6) = (b(m, 0x22) << 16 | b(m, 0x21) << 8 | b(m, 0x20), b(m, 0x25) << 16 | b(m, 0x24) << 8 | b(m, 0x23));
    m.set_pv_i(0x14, c5 as i32);
    m.set_pv_i(0x18, c6 as i32);
    let a = m.pv_i(0x68);
    m.alpha = if a != 0 { a as u8 } else { 0xff };
    if m.pv_i(0x1c) != 0 {
        m.state = 1;
        return;
    }
    let (cc, cd) = (
        b(m, 0x29) << 24 | b(m, 0x28) << 16 | b(m, 0x27) << 8 | b(m, 0x26),
        b(m, 0x2d) << 24 | b(m, 0x2c) << 16 | b(m, 0x2b) << 8 | b(m, 0x2a),
    );
    m.set_pv_i(0x30, cc as i32);
    m.set_pv_i(0x34, cd as i32);
    if m.pv_v(0x40) == [0.0; 3] { m.set_pv_i(0x6c, 1); }
    // `0x2e7920(0x3f4998, m)` this tick too: crate::fx draws state 2's.
    m.state = 2;
}

fn pulse(w: &mut MobyWorld, id: usize) {
    let m = &mut w.mobys[id];
    let phase = add_rot(m.pv_f(0xc), m.pv_f(4));
    m.set_pv_f(0xc, phase);
    let old = m.pv_i(0x10) as u32;
    let (a, b) = (m.pv_i(0x14) as u32, m.pv_i(0x18) as u32);
    let threshold = m.pv_f(8);
    let new = if threshold == 0.0 {
        let s = f32::from_bits(rc_formats::moby_light::vu0_sin_cos(phase.to_bits()).0);
        crate::hud::lerp_rgba(s * 0.5 + 0.5, a, b)
    } else {
        crate::hud::lerp_rgba(0.1, old, if threshold < phase { b } else { a })
    };
    m.set_pv_i(0x10, new as i32);
    if new == old { return; }
    let rgb = [new as u8 & 0xfc, (new >> 8) as u8 & 0xfc, (new >> 16) as u8 & 0xfc];
    let ties: Vec<usize> = group(w, id).map(|g| g.iter().map(|&t| t as usize).collect()).unwrap_or_default();
    w.tie_colours.extend(ties.into_iter().map(|t| (t, rgb)));
}

/// `0x3f4998` (state 2, the camera in its cuboid): per tie of the group whose sphere, grown by the size `[0x13]`, is
/// in view within `[0x18]` (`0x2f0080` = R&C1's `FastBSphereCheck`): the point at the tie's matrix × the offset
/// `[0x14..0x16]`, in the colour `[0xc]`; with a facing `[0x10..0x12]` (through the tie's rows) only when it faces the
/// camera more than `[0xf]` (the dot of the unit vectors), its colour from `[0xd]` toward `[0xc]` by
/// `f = dot · 0.5 + 0.5` (`f²` with `[0xe]`). At most 70. Then `0x326f38` draws them.
pub fn draw(w: &MobyWorld, id: usize, view: &rc_game::particles::BSphereView, cam: V3, out: &mut Vec<FxQuad>) {
    let m = &w.mobys[id];
    let Some(g) = group(w, id) else { return };
    let (size, pull, far) = (m.pv_f(0x4c), m.pv_f(0x5c), m.pv_f(0x60));
    let (off, facing) = (m.pv_v(0x50), m.pv_v(0x40));
    let (cc, cd) = (m.pv_i(0x30) as u32, m.pv_i(0x34) as u32);
    let mut glows: Vec<(V3, u32)> = Vec::new();
    for &t in g {
        let t = t as usize;
        let (Some(s), Some(mat)) = (w.tie_spheres.get(t), w.ties.get(t)) else { break };
        if view.culled(far, [s[0], s[1], s[2], s[3] + size]) { continue; }
        let rot = |v: V3| -> V3 { std::array::from_fn(|k| mat[0][k] * v[0] + mat[1][k] * v[1] + mat[2][k] * v[2]) };
        let p = add([mat[3][0], mat[3][1], mat[3][2]], rot(off));
        let colour = if m.pv_i(0x6c) != 0 {
            cc
        } else {
            let d = dot(unit(sub(cam, p)), unit(rot(facing)));
            if d <= m.pv_f(0x3c) { continue; }
            let mut f = d * 0.5 + 0.5;
            if m.pv_i(0x38) != 0 { f *= f; }
            crate::hud::lerp_rgba(f, cd, cc)
        };
        if glows.len() < 0x46 { glows.push((p, colour)); }
    }
    glow_quads(size, pull, &glows, cam, out);
}

/// `0x2ef138(1, v)`: v over its length (0 stays 0).
fn unit(v: V3) -> V3 {
    let l = dot(v, v).sqrt();
    if l != 0.0 { scale(v, 1.0 / l) } else { [0.0; 3] }
}

/// `0x326f38(size, pull, n, points, colours)`: per point a glow quad (FX 0xb, additive) facing the camera, pulled
/// `pull` toward it: the rows (to the camera d, `up × d` = c, `d × c`) and the template corners (0, ±1, ±1) × size.
pub fn glow_quads(size: f32, pull: f32, glows: &[(V3, u32)], cam: V3, out: &mut Vec<FxQuad>) {
    const CORNERS: [[f32; 2]; 4] = [[-1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]];
    const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
    for &(p, rgba) in glows {
        let d = unit(sub(cam, p));
        let c = unit(cross([0.0, 0.0, 1.0], d));
        let b = cross(d, c);
        let p = add(p, scale(d, pull));
        let corners = CORNERS.map(|[y, z]| add(p, add(scale(c, y * size), scale(b, z * size))));
        out.push(FxQuad { fx: 0xb, additive: true, corners, st: ST, rgba: [rgba; 4] });
    }
}
