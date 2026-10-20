//! The hand item (slot 0 of the item slots `0x18b040 + 0x50·k`): its creation `0x2afc68` (the item's moby class from
//! the item record table `0x262500` row `0x139568[item]`: the wrench, item 10, is class 0x47), its placement at
//! Ratchet's attachment matrix `0x2b09e0` (`0x18aa70 + 0x40·k`, k = the record's word 0: 0 for the wrench) and the
//! requests the states make of it (its animation, the throw). The world side ([`item_pass`]) runs after the hero
//! update, as `0x2b21f0` / `0x2b09e0` do inside it.
//!
//! Not ported: the other slots (the back item `0x18b130`: Clank, the packs), item changes (`0x18c104`, the quick
//! select), the gadget class loader `0x306018` (the engine adds the class), the strafe / look stance marker.

use super::physics::V3;
use crate::moby::{mode, Ctx, MobyWorld};

/// The wrench's moby class (`0x262504` of item 10's row).
pub const WRENCH_CLASS: i16 = 0x47;
/// The wrench's item id.
pub const WRENCH: i32 = 10;

/// A throw request (`0x2ba610`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    pub dir: V3,
    pub push: f32,
}

/// The item slot block (slot 0, `0x18b040..0x18b090`) and the flags the states set.
#[derive(Clone, Copy, Debug, Default)]
pub struct ItemBlock {
    /// `0x18b040`: the hand item's moby; its class.
    pub hand: Option<usize>,
    pub hand_class: i16,
    /// `0x18b068`: the item (10 = the wrench).
    pub item: i32,
    /// `0x18b064`: 2 = in the hand, ready.
    pub ready: i32,
    /// `0x18b050`: the buttons that use it (the wrench: □ 0x80, the others ○ 0x20).
    pub trigger: u32,
    /// `0x18b059`: 2 while a swing can hit (the states set it; the wrench's update reads it).
    pub use_state: u8,
    /// `0x18b05a`: the wrench is thrown (out of the hand).
    pub thrown: u8,
    /// `0x18b05c`: the wrench's update says it is flying (2).
    pub flying: u8,
    /// `0x18c0dc`: states that keep the item away.
    pub no_use: u8,
    /// `0x18b060`: ticks the item has been in the hand.
    pub item_ticks: i32,
    /// Requests for the item pass: the hand item's sequence `(seq, frame, blend)` (`0x31b5f0`), the throw.
    pub hand_anim: Option<(u8, u8, u8)>,
    pub throw: Option<Throw>,
}

/// After the hero's transitions (`0x2b21f0`): create the hand item when there is none (`0x2afc68`: the wrench; draw
/// distance 0x20, drawn, ready, □, sequence 1), place it at Ratchet's attachment 0 while it is in the hand (`0x2b09e0`:
/// the rebuild, then the attachment matrix for its rows, mode 6: out of the moby loop), carry out the states' requests,
/// and run its update (`0x2b2060`: the item slots' updates are called here, not by the moby loop; the item tick
/// `0x18b060` counts).
pub fn item_pass(world: &mut MobyWorld, ctx: &mut Ctx) {
    // The body's hits of this tick (the Thruster-Pack's ram).
    for h in std::mem::take(&mut ctx.hero.hits_out) {
        let t = crate::moby::hits::HitTemplate::new(h.damage, ctx.hero_moby, h.flags, [h.dir[0], h.dir[1], h.dir[2], 0.0]);
        let mut c = h.centre;
        if let (Some([a, b]), Some(hm)) = (h.joints, ctx.hero_moby) {
            if let (Some(pa), Some(pb)) = (ctx.host.joint(&world.mobys[hm], a), ctx.host.joint(&world.mobys[hm], b)) {
                c = [(pa[0] + pb[0]) * 0.5, (pa[1] + pb[1]) * 0.5, (pa[2] + pb[2]) * 0.5];
            }
        }
        world.sphere_hit(ctx.scene, c, h.r, 0, ctx.hero_moby, Some(&t));
    }
    let it = &mut ctx.hero.items;
    if it.hand.is_none() && world.class(WRENCH_CLASS).is_some() {
        let id = world.create(WRENCH_CLASS);
        let m = &mut world.mobys[id];
        m.draw_dist = 0x20;
        m.drawn = 1;
        m.parent = ctx.hero_moby;
        it.hand = Some(id);
        it.hand_class = WRENCH_CLASS;
        it.item = WRENCH;
        it.ready = 2;
        it.trigger = 0x80;
        ctx.host.set_anim(&world.mobys[id], 1, 0, 1);
    }
    let Some(id) = it.hand else { return };
    if let Some((seq, frame, blend)) = it.hand_anim.take() { ctx.host.set_anim(&world.mobys[id], seq, frame, blend); }
    if let Some(t) = it.throw.take() {
        let m = &mut world.mobys[id];
        m.pvars.resize(crate::moby::PVAR_BYTES, 0);
        crate::moby::classes::wrench::start_throw(m, t);
        ctx.host.set_anim(&world.mobys[id], 6, 0, 5);
    }
    if it.thrown == 0 {
        if let Some((rows, pos)) = ctx.host.hero_attach(0) {
            {
                let m = &mut world.mobys[id];
                m.pos = pos;
                m.mode |= mode::NO_UPDATE | mode::NO_MATRIX | mode::KEEP_ROWS;
                m.rows = rows;
                // `+0xf4` / `+0xf8`: the pitch and yaw of the first row.
                let f = rows[0];
                m.rot = [0.0, -f[2].atan2((f[0] * f[0] + f[1] * f[1]).sqrt()), f[1].atan2(f[0])];
            }
            world.rebuild(id, &*ctx.host);
        }
    }
    ctx.hero.items.item_ticks += 1;
    if world.mobys[id].state < 0xfe {
        if let Some(f) = world.mobys[id].update { f(world, id, ctx); }
    }
}
