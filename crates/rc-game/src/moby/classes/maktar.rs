//! Maktar Nebula's small objects (L02 class updates; no names in any reference, so they go by o_class).
//!
//! * 0x0afc (`0x3f0618`): no update (an empty function); 0x0873 creates one in its place.
//! * 0x0aaf (`0x3eb9f8`): on its first tick sequence state 1, no collision, mode 0x43 without TARGET ([`update_0aaf`]).
//! * 0x0873 (`0x3c1d20`): on its first tick a 0x0afc in its place ([`update_0873`]).
//! * 0x0e93 (`0x42fb20`): a breakable (crate::moby::classes::props::update_breakable: a hit with the flags 0x4010000
//!   while `+0x68`[0] = 0); its start's two glows (`0x346250`), its break's shards, burst, debris and sound are not
//!   ported, nor the count of them `0x12cf` keeps (`$gp−0x3c0`; at 10 left, once: `0x367f58`, `0x32c310(0x1233)`).
//!
//! Not ported: 0x000b (`0x396448`, the ship's pad: its hologram 0x477 and the △ prompt that starts the travel, the
//! main dev's progression), 0x12cf (`0x38b458`, that count), 0x0c24 (`0x40b4c0`, the help voice lines), 0x0e96
//! (`0x4301b0`, an ambient animation tied to another moby's `+0x6c`), 0x0e9c (`0x4305f8`, a tie texture scroll).

use crate::moby::{mode, Ctx, MobyWorld};

/// 0x0afc.
pub fn update_0afc(_w: &mut MobyWorld, _id: usize, _ctx: &mut Ctx) {}

/// `0x3eb9f8`: pvars `+0x5c` |= 4, state 1, the collision blob `+0x98` cleared, mode 0x43 without TARGET.
pub fn update_0aaf(w: &mut MobyWorld, id: usize, _ctx: &mut Ctx) {
    let m = &mut w.mobys[id];
    if m.state != 0 { return; }
    m.set_pv_i(0x5c, m.pv_i(0x5c) | 4);
    m.set_state(1, -1);
    m.has_collision = false;
    m.mode = (m.mode & !mode::TARGET) | 0x43;
    w.changed = true;
}

/// `0x3c1d20`: pvars `+4` / `+8` 0, `+0x10` the yaw, `+0x18` = `+0x14`, update distance 0x80, state 1; a 0x0afc
/// (`0x315438`) at its position with its update and draw distances, mode |= 0x41, kept in `+0x1c` (index + 1).
pub fn update_0873(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if w.mobys[id].state != 0 { return; }
    let m = &mut w.mobys[id];
    m.set_pv_i(4, 0);
    m.update_dist = 0x80;
    m.state = 1;
    m.set_pv_i(8, 0);
    m.set_pv_f(0x10, m.rot[2]);
    m.set_pv_i(0x18, m.pv_i(0x14));
    let (pos, update_dist, draw_dist) = (m.pos, m.update_dist, m.draw_dist);
    let c = w.create(0x0afc);
    let child = &mut w.mobys[c];
    child.update_dist = update_dist;
    child.draw_dist = draw_dist;
    child.pos = pos;
    child.mode |= 0x41;
    w.rebuild(c, ctx.host);
    w.changed = true;
    w.mobys[id].set_pv_i(0x1c, c as i32 + 1);
}
