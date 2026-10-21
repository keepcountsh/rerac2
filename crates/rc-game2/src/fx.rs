//! The mobys' draw callbacks (`0x2e7920(fn, moby)`: registered during the moby's update, called by the frame's draw
//! after every update): the quads they send to the GS (`0x2eb998`, an FX texture, ALPHA 0x48 additive or 0x44),
//! which the engine draws through its fx_draw. They run after the tick ([`callbacks`]) in the moby order, as the frame
//! calls them, so the random draws some make (a glow's roll) come after the tick's.
//!
//! | Callback | Moby | Draws |
//! |---|---|---|
//! | `0x3d8340` | 0x0b0a | its three glows ([`glow`]: the pilot light at `+0x3a0`, the nozzle `+0x3b0` and the mouth `+0x3c0`); climbing, its wire (`0x3d8498`) |
//!
//! The Lancer's shot trail (`0x3e4050`) draws from crate::moby::classes::shot. Not ported: 0x0b0a's emerge flash
//! (`0x3d8920` → `0x2ee190`, FX 0x3b), 0x0b0b's glows and wire (`0x3dc220`, `0x3db790`).

use crate::hero::physics::{add, cross, len, scale, sub, V3};
use crate::moby::MobyWorld;

/// One quad (`0x2eb998`): FX texture, additive (0x48) or normal (0x44), corners in GS strip order, ST, RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxQuad {
    pub fx: usize,
    pub additive: bool,
    pub corners: [V3; 4],
    pub st: [[f32; 2]; 4],
    pub rgba: [u32; 4],
}

const ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

fn unit(v: V3) -> V3 {
    let l = len(v);
    if 0.0 < l { scale(v, 1.0 / l) } else { v }
}

/// `0x3d86a0(size, m, point, rgb, alpha, alpha2)`: a soft glow (FX 0xb, additive) at `point` pulled 0.5 toward the
/// camera, facing it with a random roll (`0x31bb00`): the template's ±2 square × `size` in `rgb` at `alpha`, then half
/// of it at `alpha2`.
pub fn glow(out: &mut Vec<FxQuad>, rng: &mut crate::rng::Rng, size: f32, point: V3, rgb: u32, alpha: u32, alpha2: u32, cam: V3) {
    let roll = rng.rand_angle();
    let f = unit(sub(cam, point));
    let p = add(point, scale(f, 0.5));
    // A basis across the view, turned by the roll.
    let up = if f[2].abs() < 0.99 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let r0 = unit(cross(up, f));
    let u0 = cross(f, r0);
    let (s, c) = roll.sin_cos();
    let r = add(scale(r0, c), scale(u0, s));
    let u = sub(scale(u0, c), scale(r0, s));
    for (k, a) in [(1.0, alpha), (0.5, alpha2)] {
        let h = 2.0 * size * k;
        let corner = |y: f32, z: f32| add(p, add(scale(r, y * h), scale(u, z * h)));
        let rgba = (rgb & 0xff_ffff) | a << 24;
        out.push(FxQuad { fx: 0xb, additive: true, corners: [corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0)], st: ST, rgba: [rgba; 4] });
    }
}

/// `$gp+0x12f0` / `+0x12f4`: 0x0b0a's glow colours.
const GLOW_0B0A: [u32; 2] = [0x0000_36f6, 0x0010_50b0];

/// `0x3d8340` (0x0b0a, every tick): climbing (states 4, 5, 10, or sub-state 2), its wire (`0x3d8498`: FX 0xe, 0.05
/// wide, white at alpha 0x1f, from `+0x380` to `+0x390`); then the glows.
fn draw_0b0a(w: &MobyWorld, id: usize, rng: &mut crate::rng::Rng, cam: V3, out: &mut Vec<FxQuad>) {
    let m = &w.mobys[id];
    if matches!(m.state, 4 | 5 | 10) || m.sub_state == 2 {
        let (a, b) = (m.pv_v(0x380), m.pv_v(0x390));
        let side = scale(unit(cross(sub(cam, a), unit(sub(b, a)))), 0.05);
        let c = 0x1f7f_7f7f;
        out.push(FxQuad { fx: 0xe, additive: true, corners: [add(a, side), sub(a, side), add(b, side), sub(b, side)], st: ST, rgba: [c; 4] });
    }
    glow(out, rng, 0.125, m.pv_v(0x3a0), GLOW_0B0A[0], 0x20, 0x20, cam);
    glow(out, rng, 0.15, m.pv_v(0x3b0), GLOW_0B0A[1], 0x30, 0x20, cam);
    glow(out, rng, 0.15, m.pv_v(0x3c0), GLOW_0B0A[1], 0x10, 0x30, cam);
}

/// The frame's callbacks after the tick: the mobys the loop ran (they registered theirs), in the loop's order.
pub fn callbacks(w: &MobyWorld, rng: &mut crate::rng::Rng, cam: V3) -> Vec<FxQuad> {
    let mut out = Vec::new();
    for id in w.run_list(cam) {
        if 0x80 <= w.mobys[id].state { continue; }
        if w.mobys[id].o_class == 0x0b0a { draw_0b0a(w, id, rng, cam, &mut out); }
    }
    out
}
