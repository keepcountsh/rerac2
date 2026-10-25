//! The blob shadows (`0x31d930(size, moby)`): the soft dark disc on the ground under a moby, asked for by its update
//! each tick and drawn by the frame. The frame's list (`0x1be840`, up to 32 records of 0x20 bytes counted by
//! `0x1b18fc`) is cleared with its other draw lists (`0x2e60c8`) and sent as one packet by `0x2e7ab0`, after the
//! first callback list and before the particles and the mobys' own callbacks (`0x2e6658`): here [`draw`] puts the
//! quads first among crate::fx's.
//!
//! **The request** ([`request`], `0x31d930`): only for a moby drawn last frame (`+0x31`) while the list has room. The
//! ground is the line from 0.5 above the moby straight down to z = 0.01 (`0x31ce20(0.5, pos, 0)`: `0x2de878` with
//! flags 2, the world and the mobys' meshes); nothing when it misses or the surface id is 0 (`0x2e23e0`). The record
//! holds the moby's x and y with the ground 0.025 up, the hit's normal, and the size × (8 − the height above the
//! ground) / 8, at least a quarter of the size.
//!
//! **The draw** (`0x2e7ab0`): FX texture 0 (`0x2e71b8(0)`), ALPHA 0x44, every corner 0x40808080; each record's quad is
//! the ±1 template (`gp−0x6500`) laid onto the ground: corner = centre + size × (t − n (n·t)) with the normal n made
//! unit (`0x2ef138`), so a slope shrinks the disc along its fall line rather than tilting it.
//!
//! Callers on Aranos: the Gravity Bomb's bomb (`0x3efda0`, 0.2; crate::moby::classes::gravity_bomb), and the updates
//! `0x392ce0` (0.2), `0x3ab088`, `0x3abb40`, `0x3afd40`, `0x3d5e70` (0.3), `0x3deaf8`, `0x3e86f0`, `0x3ed5c0`,
//! `0x4003a0`, `0x4010a0`, `0x404338`, `0x409328`, `0x40abb0`, which add their own calls as they are ported.
//!
//! Not ported: the moby's own shadow (`0x31da58`: `+0x70` / `+0x74` for the shadow-volume pass), a different system.

use crate::hero::physics::{add, scale, unit, V3};
use crate::moby::{Ctx, MobyWorld};

/// The list's room (`0x31d930`'s `< 0x20`).
pub const MAX: usize = 0x20;
/// The draw's texture and colour (`0x2e7ab0`).
pub const FX: usize = 0;
pub const RGBA: u32 = 0x4080_8080;
/// `0x31ce20(0.5, ..)`: the line's start above the moby and its end (`0x3c23d70a`); the record's lift off the ground.
const ABOVE: f32 = 0.5;
const FLOOR: f32 = 0.01;
const LIFT: f32 = 0.025;
/// `gp−0x6500`: the four corners (x, y) and their ST, in GS strip order.
const TEMPLATE: [([f32; 2], [f32; 2]); 4] = [([1.0, -1.0], [0.0, 1.0]), ([-1.0, -1.0], [0.0, 0.0]), ([1.0, 1.0], [1.0, 1.0]), ([-1.0, 1.0], [1.0, 0.0])];

/// One record (0x20 bytes): `+0x00` the centre (the moby's x and y, the ground 0.025 up), `+0x0c` the size as drawn,
/// `+0x10` the ground's normal as the collision left it (unnormalised).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub centre: V3,
    pub size: f32,
    pub normal: V3,
}

/// The frame's list (`0x1be840` / `0x1b18fc`).
#[derive(Clone, Debug, Default)]
pub struct Shadows {
    pub list: Vec<Shadow>,
}

impl Shadows {
    /// `0x2e60c8`: emptied with the frame's other draw lists.
    pub fn clear(&mut self) { self.list.clear(); }
}

/// `0x31d930(size, moby)`: the shadow under the moby `id`, `size` across at ground level, shrinking with its height
/// (to a quarter at 6 up); none unless the moby was drawn, the list has room, the line down finds ground and the
/// ground's surface id is not 0.
pub fn request(w: &mut MobyWorld, ctx: &Ctx, size: f32, id: usize) {
    let m = &w.mobys[id];
    if m.drawn == 0 || MAX <= w.shadows.list.len() { return; }
    let p = m.pos;
    let Some(h) = w.coll_line(ctx, [p[0], p[1], p[2] + ABOVE], [p[0], p[1], FLOOR], 2, None) else { return };
    if h.surface_id() == 0 { return; }
    let ground = h.point[2];
    let f = ((8.0 - (p[2] - ground).abs()) * 0.125).max(0.25);
    w.shadows.list.push(Shadow { centre: [p[0], p[1], ground + LIFT], size: size * f, normal: h.normal });
}

/// `0x2e7ab0`: the list's quads (FX 0, ALPHA 0x44, 0x40808080), each the template laid onto its ground.
pub fn draw(s: &Shadows, out: &mut Vec<crate::fx::FxQuad>) {
    for sh in &s.list {
        let n = unit(sh.normal);
        let corner = |t: [f32; 2]| {
            let d = n[0] * t[0] + n[1] * t[1];
            add(sh.centre, scale([t[0] - n[0] * d, t[1] - n[1] * d, -n[2] * d], sh.size))
        };
        out.push(crate::fx::FxQuad {
            fx: FX,
            additive: false,
            corners: TEMPLATE.map(|(t, _)| corner(t)),
            st: TEMPLATE.map(|(_, st)| st),
            rgba: [RGBA; 4],
        });
    }
}
