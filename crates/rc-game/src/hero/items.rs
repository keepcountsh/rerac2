//! The hand item (slot 0 of the item slots `0x18b040 + 0x50·k`): its creation `0x2afc68` (the item's moby class from
//! the item record table `0x262500` row `0x139568[item]`, [`ItemRow`]), its placement at Ratchet's attachment matrix
//! `0x2b09e0` (`0x18aa70 + 0x40·k`, k = the record's word 0), the item change `0x2b13c8` (□ takes the wrench out, ○
//! the last weapon back; the old item is deleted on the next pass and the new one created two frames on, `0x2b12a8`),
//! the use stance (`0x2af5e0` / `0x2af848` / `0x2af8d8`: Ratchet's sequence while a weapon fires) and the requests
//! the states make of it (its animation, the throw). The world side ([`item_pass`]) runs after the hero update, as
//! `0x2b21f0` / `0x2b09e0` do inside it.
//!
//! A game starts with the Lancer (crate::game::GameState::start_items), so Ratchet arrives holding it.
//!
//! Not ported: the other slots (the back item `0x18b130`: Clank, the packs), the quick select (`0x3227d8`), the
//! gadgets a request uses instead of holding (0x1f, 0x3d, 9, 0x2d), the weapon before the last (`0x1a7a0c`), the item a forced wrench gives back (`0x18c120`), the class streaming (`0x305cc0` /
//! `0x305ab0`: the engine loads every hand class with the level), the upper-body layers of a use (`0x311e10`, rows
//! `+0x1c` / `+0x20`: the Lancer has none), the L2 + R2 weapon pull (`0x1a7a88` bit 0x20, `0x2d67b8`), the strafe /
//! look stance marker.

use super::physics::V3;
use crate::moby::{mode, Ctx, MobyWorld};

/// The wrench's moby class (`0x262504` of item 10's row).
pub const WRENCH_CLASS: i16 = 0x47;
/// The wrench's item id.
pub const WRENCH: i32 = 10;
/// The Lancer: item 0x1e, class 0x5d2 (`0x262504` of its row; a gadget, `G/GADGET.WAD`).
pub const LANCER: i32 = 0x1e;
pub const LANCER_CLASS: i16 = 0x5d2;

/// An item record (`0x262500 + 0xe0·row`, row `0x139568[item]` = the item): the fields the port reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemRow {
    pub item: i32,
    /// `+0x00`: Ratchet's attachment it sits at; `+0x04` its moby class.
    pub attach: usize,
    pub class: i16,
    /// `+0x0c`: its stance type (`0x18c0db`; 0 the wrench's).
    pub stance: u8,
    /// `+0x2c`: its HUD icon (the weapon panel, crate::hud::WeaponPanel).
    pub icon: u16,
    /// `+0x18`: Ratchet's sequence while it is used standing; `+0x24`: the use keeps his stance (`0x18c0da`).
    pub use_seq: i32,
    pub keep: bool,
    /// `+0x40`: its shots' speed (per second); `+0x44` the shots a second; `+0x60` the damage.
    pub speed: f32,
    pub rate: f32,
    pub damage: f32,
    /// `+0x78` ≠ 0: it has ammo; `+0x7e` the most; `+0x82` what it comes with.
    pub ammo: bool,
    pub max_ammo: i32,
    pub start_ammo: i32,
}

/// The rows the port has (L00 `0x262dc0`, `0x263f40`).
pub const ROWS: [ItemRow; 2] = [
    ItemRow { item: WRENCH, attach: 0, class: WRENCH_CLASS, stance: 0, icon: 0, use_seq: 0, keep: false, speed: 0.0, rate: 0.0, damage: 1.0, ammo: false, max_ammo: 0, start_ammo: 0 },
    ItemRow { item: LANCER, attach: 0, class: LANCER_CLASS, stance: 1, icon: 0xea64, use_seq: 0x38, keep: true, speed: 60.0, rate: 7.0, damage: 0.5, ammo: true, max_ammo: 200, start_ammo: 200 },
];

/// Item `item`'s row.
pub fn row(item: i32) -> Option<&'static ItemRow> { ROWS.iter().find(|r| r.item == item) }

/// The head item's moby class (slot 2, `0x18b0e0`: item 0x2f, the pilot's helmet; `0x2d5138` names it).
pub const HEAD_CLASS: i16 = 0x50a;
/// `0x1a7d08` + 4: the joints of Ratchet whose pose the head item's joints 1 and 2 take (`0x2aa278`).
pub const HEAD_JOINTS: [u8; 2] = [8, 10];
/// The head item's attachment (its row's word 0; `HERO_LISTS[4]`: joints 0..7, the head).
pub const HEAD_ATTACH: usize = 4;
/// The worn feet item (slot 1, `0x18b090` / `0x18b094`): `0x1a739c`, 0 for the default 0x34 (its row: attachment 2,
/// class 0xa73 for both feet).
pub const FEET_ITEM: i32 = 0x34;
/// `0x2b09e0`: the second foot sits at attachment 3 whatever the row says.
pub const FEET_SECOND_ATTACH: usize = 3;
/// `0x1a7d18` + 4 / `0x1a7d30` + 4: the joints of Ratchet whose pose each foot's joints 1..4 take (`0x2aa278`): his
/// left foot 100..103, his right 94..97.
pub const FEET_JOINTS: [[u8; 4]; 2] = [[100, 101, 102, 103], [94, 95, 96, 97]];

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
    /// `0x18b0e0`: the head item's moby ([`head_pass`]).
    pub head: Option<usize>,
    /// `0x18b090` / `0x18b094`: the worn feet's mobys ([`feet_pass`]).
    pub feet: [Option<usize>; 2],
    /// `0x18c250`: the thrown wrench's loop sound (its class sound 0xb).
    pub throw_sound: Option<i32>,
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
    /// `0x18b06c`: the hand item's glow phase ([`hand_glow`]).
    pub glow_phase: f32,
    /// Requests for the item pass: the hand item's sequence `(seq, frame, blend)` (`0x31b5f0`), the throw.
    pub hand_anim: Option<(u8, u8, u8)>,
    pub throw: Option<Throw>,
    /// `0x18c104`: the item to hold (0: the default, [`item_pass`]); `0x18c0d6`: the wrench asked for (the items
    /// locked away); `0x18b054`: the frame the next item may be created after.
    pub selected: i32,
    pub wrench_req: bool,
    /// `0x18c0e8`: an item asked for (the pause's Weapons page, `0x344dd0`; 0x77: none), taken by the next change.
    pub request: i32,
    pub next_create: i32,
    /// `0x18b058`: 0xff when the item has ammo and none is left.
    pub empty: u8,
    /// `0x18c0db`: the held item's stance type (its row's `+0x0c`: 1 one hand, 2 both; crate::hero::layers).
    pub stance: u8,
    /// `0x18c0df`: the hand item tucked away (some states' entries; every change clears it).
    pub tucked: bool,
    /// The use stance: `0x18c0d8` in use, `0x18c0da` it keeps Ratchet's stance, `0x18c2d0` his sequence, `0x18c2d4` the
    /// item, `0x189ff8` the ticks in use.
    pub using: bool,
    pub keep: bool,
    pub use_seq: i32,
    pub use_item: i32,
    pub use_ticks: i32,
    /// `0x18c354`: in the scripted look stance 0x1d, the ticks before a weapon may fire (0x28).
    pub look_wait: i32,
    /// The pad as the hero tick read it (`0x138320` held, `0x138324` pressed; □ pressed within 3 ticks,
    /// `0x330240(0x80, 3)`).
    pub held: u32,
    pub pressed: u32,
    pub square_recent: bool,
    /// `0x18aa18`: the moby a weapon aims at (its lock) and its aim height, set by the weapon's update each tick it
    /// holds one and read once by the posture (crate::hero::posture, `0x2abc18`).
    pub aim_target: Option<(usize, f32)>,
    /// A sequence for Ratchet `(blend, seq)` the use stance asks for (`0x2d2508`; the engine plays it after the pass).
    pub hero_anim: Option<(f32, u8)>,
}

impl ItemBlock {
    /// `0x2d43e0(-1)`: the held item's ammo is not out (an item without ammo always can).
    pub fn has_ammo(&self, game: &crate::game::GameState) -> bool {
        match row(self.item) { Some(r) if r.ammo => game.ammo.get(self.item.max(0) as usize).is_some_and(|&a| a != 0), _ => true }
    }

    /// `0x2d42d0(-1, n)`: take `n` of the held item's ammo; false when there is too little.
    pub fn take_ammo(&self, game: &mut crate::game::GameState, n: i32) -> bool {
        if !row(self.item).is_some_and(|r| r.ammo) { return true; }
        let Some(a) = game.ammo.get_mut(self.item.max(0) as usize) else { return false };
        if n <= *a { *a -= n; true } else { false }
    }

    /// `0x2af5e0`: the held item in use. Its row's stance: kept (`+0x24`); standing (group 0), Ratchet takes its
    /// sequence (`+0x18`) blended over 0xb ticks (stance type 0), 2 (sequence 0x36, items 0x24 / 0x1a) or 5 (items
    /// 0x1e / 0x19 / 0x46). Moving, its upper-body layer (`+0x1c`, not ported; the Lancer has none).
    pub fn use_start(&mut self, group: i32) {
        if self.ready != 2 { return; }
        let Some(r) = row(self.item) else { return };
        self.using = true;
        self.keep = r.keep;
        let blend = if r.stance == 0 {
            0xb
        } else if r.use_seq == 0x36 || matches!(self.item, 0x24 | 0x1a) {
            2
        } else if matches!(self.item, 0x1e | 0x19 | 0x46) {
            5
        } else {
            -1
        };
        self.use_item = self.item;
        if group == 0 {
            self.use_seq = r.use_seq;
            self.hero_anim = Some((blend as f32, r.use_seq as u8));
        }
    }

    /// `0x2cb650`'s standing case: while a use keeps the stance and the hand item is still the one used, Ratchet's
    /// sequence is its use sequence (set again, blend −4, when it is not and no blend runs); another item ends it.
    pub fn use_stance(&mut self, state: i32, seq_b: u8, blending: bool, idle: u8) {
        if !self.using || blending || !self.keep { return; }
        if self.ready == 2 && self.item == self.use_item {
            let s = row(self.item).map_or(0, |r| r.use_seq);
            if seq_b as i32 != s {
                self.use_seq = s;
                self.hero_anim = Some((-4.0, s as u8));
            }
        } else {
            self.use_end(state, seq_b, idle);
        }
    }

    /// `0x2af848`: the use over; standing (state 0) in the use's sequence, Ratchet's idle back (blend −2).
    pub fn use_end(&mut self, state: i32, seq_b: u8, idle: u8) {
        if !self.using { return; }
        if state == 0 && seq_b as i32 == self.use_seq { self.hero_anim = Some((-2.0, idle)); }
        self.keep = false;
        self.using = false;
    }
}

/// After the hero's transitions (`0x2b21f0`): create the hand item when there is none ([`create_hand`]), place it at
/// its row's attachment of Ratchet while it is in the hand (`0x2b09e0`: the rebuild, then the attachment matrix for its
/// rows, mode 6: out of the moby loop), carry out the states' requests, test for a change ([`switch`]) and run its
/// update (`0x2b2060`: the item slots' updates are called here, not by the moby loop; the item tick `0x18b060`
/// counts).
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
    // `0x2b21f0`'s head: in water (`0x2adff8`: the swim groups 0x11 / 0x12, the states 0x67 / 0x7f / 0x72 / 0x73),
    // the groups 0x15 / 0x16 / 0x1f keep the items away and the △ menu shut (`0x18c0dc`, `0x18c0d7`); the wrench
    // is put away (`0x18c0de`) in water, on a ledge (group 3) and in the states 0x31 / 0x12 / 0x82.
    let h = &mut *ctx.hero;
    let in_water = (0x11..=0x12).contains(&h.group) || matches!(h.state, 0x67 | 0x7f | 0x72 | 0x73);
    if matches!(h.group, 0x15 | 0x16 | 0x1f) || in_water {
        h.items.no_use = 1;
        h.no_menu = 1;
    }
    let away = in_water || h.group == 3 || matches!(h.state, 0x31 | 0x12 | 0x82);
    head_pass(world, ctx);
    feet_pass(world, ctx);
    look_kind(world, ctx);
    create_hand(world, ctx);
    let Some(id) = ctx.hero.items.hand else {
        switch(world, ctx);
        return;
    };
    let it = &mut ctx.hero.items;
    if let Some((seq, frame, blend)) = it.hand_anim.take() { ctx.host.set_anim(&world.mobys[id], seq, frame, blend); }
    if let Some(t) = it.throw.take() {
        let m = &mut world.mobys[id];
        m.pvars.resize(crate::moby::PVAR_BYTES, 0);
        crate::moby::classes::wrench::start_throw(m, t);
        ctx.host.set_anim(&world.mobys[id], 6, 0, 5);
    }
    // `0x313420`: the hand item is out of the moby loop; its placement `0x2b09e0` advances its animation.
    ctx.host.advance(&world.mobys[id]);
    if it.thrown == 0 {
        let attach = row(it.item).map_or(0, |r| r.attach);
        if let Some((rows, pos)) = ctx.host.hero_attach(attach) {
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
    // `0x2d3408` (from Ratchet's update): the hand item shown, the wrench hidden while put away.
    {
        let m = &mut world.mobys[id];
        m.mode &= !1;
        if away && ctx.hero.items.item == WRENCH { m.mode |= 0x41; }
    }
    // `0x2af8d8`: a kept stance ends when the ammo runs out.
    {
        let h = &mut *ctx.hero;
        if h.items.using && h.items.keep && !h.items.has_ammo(ctx.game) {
            let idle = h.idle_seq();
            h.items.use_end(h.state, ctx.hero_view.seq_b, idle);
        }
        // `0x2c3330`: the ticks in use.
        h.items.use_ticks = if h.items.using { h.items.use_ticks + 1 } else { 0 };
    }
    hand_glow(world, ctx, id);
    // `0x2b2060`: ready (2), the change test then the update; changing (3), the old item goes (`0x2b12a8`).
    match ctx.hero.items.ready {
        2 => {
            ctx.hero.items.item_ticks += 1;
            switch(world, ctx);
        }
        3 => {
            let it = &mut ctx.hero.items;
            it.next_create = ctx.game.frame + 2;
            it.ready = 0;
            it.item = 0;
            it.hand = None;
            if let Some(h) = it.throw_sound.take() { ctx.host.stop_sound(h); }
            world.delete(id);
            return;
        }
        _ => {}
    }
    if world.mobys[id].state < 0xfe {
        if let Some(f) = world.mobys[id].update { f(world, id, ctx); }
    }
}

/// `0x2a7ed8`: the hand item's glow, once it has been in the hand 0x17 ticks (not while hidden or in state 0x61). By
/// item: the Lancer (0x1e) eases its glow word (`+0x78`, its glow vertices' colour) 0.15 a tick toward a colour that
/// pulses with its phase (0.0407 a tick, `sin`) between 0xff0028c8 and 0xff10b4b4 — in use 0.22 toward 0xff0040ff ..
/// 0xff10bebe, the phase 0.116 a tick — and sheds a glow sprite of that colour on it (`0x33f550`: type 0x59, 50400,
/// at (0.195, −0.317, 0) in its rows, pulled 0.15 toward the camera, 2 ticks), its alpha the ticks in the hand − 15
/// (at most 0x38; in use 0x32). Not ported: the other items' cases and the upgraded Lancer's (row `+0x04`).
fn hand_glow(world: &mut MobyWorld, ctx: &mut Ctx, id: usize) {
    let it = &mut ctx.hero.items;
    if ctx.hero.state == 0x61 || world.mobys[id].mode & 1 != 0 || it.item_ticks < 0x17 { return; }
    let ramp = ((it.item_ticks - 0xf).clamp(0, 0x40) as u32) << 24 | 0xff_ffff;
    if it.item != LANCER { return; }
    let lerp = crate::hud::lerp_rgba;
    let (step, keys, ease, cap) = if it.using { (0.116_352_08, [0xff00_40ff, 0xff10_bebe], 0.22, 0x32ff_ffffu32) } else { (0.040_712_8, [0xff00_28c8, 0xff10_b4b4], 0.15, 0x38ff_ffff) };
    it.glow_phase = crate::hero::physics::add_rot(it.glow_phase, step);
    let target = lerp(it.glow_phase.sin() * 0.5 + 0.5, keys[0], keys[1]);
    let m = &mut world.mobys[id];
    m.glow = lerp(ease, m.glow, target);
    let colour = m.glow & if (ramp as i32) < cap as i32 { ramp } else { cap };
    let (pos, draw_dist) = (m.pos, m.draw_dist);
    if let Some(sys) = ctx.parts.as_deref_mut() { crate::particles::glow(sys, &mut ctx.game.rng, 50_400.0, 0.15, [0.195, -0.317, 0.0], id, pos, draw_dist, false, colour, 2); }
}

/// `0x2aeb90`'s read of the enemy the head looks at (`0x18c2b0`): its health block's look kind (`+0x2c`).
fn look_kind(world: &MobyWorld, ctx: &mut Ctx) {
    let h = &mut *ctx.hero;
    h.layers.look = h.posture.look_at.and_then(|t| world.mobys.get(t)).filter(|m| m.mode & crate::moby::ai::AI != 0).map(|m| m.pv_b(crate::moby::ai::health(m) + 0x2c));
}

/// `0x2afc68`'s slot 0: with no hand item, once the frame passed `0x18b054` — the item to hold (`0x18c104`; else the
/// wrench when it was out, else the last weapon `0x1a7398`, else the wrench), its row's class: draw distance 0x20,
/// drawn, ready; the wrench takes □ (0x80) and its sequence 1, the others ○ (0x20); an item with ammo and none left
/// is marked empty.
fn create_hand(world: &mut MobyWorld, ctx: &mut Ctx) {
    let it = &mut ctx.hero.items;
    if it.hand.is_some() || ctx.game.frame <= it.next_create { return; }
    let mut item = it.selected;
    if item == 0 { item = if ctx.game.wrench_out || ctx.game.held == 0 { WRENCH } else { ctx.game.held }; }
    let Some(r) = row(item) else { return };
    if world.class(r.class).is_none() {
        if it.next_create != i32::MAX { eprintln!("rc_game::hero::items: item {item:#x}'s class {:#x} is not loaded: no hand item", r.class); }
        it.next_create = i32::MAX;
        return;
    }
    let id = world.create(r.class);
    let m = &mut world.mobys[id];
    m.draw_dist = 0x20;
    m.drawn = 1;
    if m.shine != 0 { m.shine = 0x10; }
    m.parent = ctx.hero_moby;
    it.hand = Some(id);
    it.hand_class = r.class;
    it.item = item;
    it.ready = 2;
    it.empty = if r.ammo && ctx.game.ammo.get(item as usize).is_some_and(|&a| a == 0) { 0xff } else { 0 };
    if item == WRENCH {
        it.trigger = 0x80;
        ctx.host.set_anim(&world.mobys[id], 1, 0, 1);
    } else {
        it.trigger = 0x20;
    }
    it.stance = r.stance;
}

/// `0x2b13c8(0)`: the hand item's change. Locked away (`0x18c0d7`), a weapon asks for the wrench; with the items
/// kept away (`0x18c0dc`) nothing changes unless that. □ within 3 ticks (or the wrench asked for) takes the wrench
/// out; ○ pressed brings the last weapon (`0x1a7398`) back. Not while the thrown wrench flies or a swing's window
/// (`0x18b059`, counted down here) is open. A change: the use ends, the wrench-out flag follows, the item plays its
/// sequence 2 and goes out on the next pass (ready 3).
fn switch(world: &mut MobyWorld, ctx: &mut Ctx) {
    let h = &mut *ctx.hero;
    let idle = h.idle_seq();
    let it = &mut h.items;
    if h.no_menu != 0 && it.item != WRENCH { it.wrench_req = true; }
    if it.no_use != 0 && (!it.wrench_req || it.item == WRENCH) {
        it.wrench_req = false;
        return;
    }
    let mut changed = false;
    if (it.square_recent || it.wrench_req) && it.item != WRENCH {
        it.selected = WRENCH;
        changed = true;
    }
    if it.pressed & crate::pad::button::CIRCLE != 0 && h.no_menu == 0 && ctx.game.held != it.item && ctx.game.held != 0 {
        it.selected = ctx.game.held;
        changed = true;
    }
    if it.request != 0 {
        if it.request == it.selected {
            it.request = 0;
        } else {
            it.selected = if it.request == 0x77 { 0 } else { it.request };
            ctx.game.held = it.selected;
            changed = true;
        }
    }
    if it.flying == 2 { changed = false; }
    if it.use_state != 0 {
        it.use_state -= 1;
        if it.use_state != 0 { changed = false; }
    }
    if !changed { return; }
    it.request = 0;
    it.item_ticks = 3;
    it.use_end(h.state, ctx.hero_view.seq_b, idle);
    ctx.game.wrench_out = it.selected == WRENCH;
    if it.selected != WRENCH { ctx.game.held = it.selected; }
    it.thrown = 0;
    it.wrench_req = false;
    if let Some(id) = it.hand { ctx.host.set_anim(&world.mobys[id], 2, 0, 2); }
    it.ready = 3;
    // A new stance type (`0x18c0db` against the new item's row `+0x0c`): the holding layers go for good (`+0x34`).
    if row(it.selected).map_or(0, |r| r.stance) != it.stance {
        for l in h.layers.hold.iter_mut().flatten() { l.kill = true; }
    }
}

/// The worn feet (slot 1): `0x2afc68` creates both when there are none — the item [`FEET_ITEM`]'s row's classes
/// `+0x04` / `+0x08` (draw distance 0x20, drawn, lit as Ratchet, ready) — not on armour 4 or with the cheats' bodies
/// (the port wears armour 0). `0x2b09e0` then puts the first at the row's attachment and the second at
/// [`FEET_SECOND_ATTACH`], each posed from his feet ([`FEET_JOINTS`], `0x2aa278`: the engine draws them so). While they
/// are worn his own feet are scaled to 0.01 (`0x2ac218`, crate::hero::posture).
///
/// Not ported: the Charge Boots' feet (the item `0x1a739c` = 0x36).
pub fn feet_pass(world: &mut MobyWorld, ctx: &mut Ctx) {
    let Some(row) = ctx.level.items.get(FEET_ITEM as usize).copied() else { return };
    let it = &mut ctx.hero.items;
    for (k, slot) in it.feet.iter_mut().enumerate() {
        let class = row.classes[k] as i16;
        if slot.is_some_and(|id| world.mobys.get(id).is_none_or(|m| m.state >= 0xfe || m.o_class != class)) { *slot = None; }
    }
    if it.feet[0].is_none() {
        for k in 0..2 {
            let class = row.classes[k] as i16;
            if class == 0 || world.class(class).is_none() { continue; }
            let id = world.create(class);
            let m = &mut world.mobys[id];
            m.draw_dist = 0x20;
            m.drawn = 1;
            if m.shine != 0 { m.shine = 0x10; }
            it.feet[k] = Some(id);
        }
    }
    let light = ctx.hero_moby.and_then(|h| world.mobys.get(h)).map(|h| (h.light, h.ambient));
    for k in 0..2 {
        let Some(id) = ctx.hero.items.feet[k] else { continue };
        let attach = if k == 1 { FEET_SECOND_ATTACH } else { row.attach.max(0) as usize };
        let Some((rows, pos)) = ctx.host.hero_attach(attach) else { continue };
        {
            let m = &mut world.mobys[id];
            m.parent = ctx.hero_moby;
            (m.pos, m.rows) = (pos, rows);
            m.mode = (m.mode | mode::NO_UPDATE | mode::NO_MATRIX | mode::KEEP_ROWS) & !1;
            let f = rows[0];
            m.rot = [0.0, -f[2].atan2((f[0] * f[0] + f[1] * f[1]).sqrt()), f[1].atan2(f[0])];
            if let Some((l, a)) = light { (m.light, m.ambient) = (l, a); }
        }
        world.rebuild(id, &*ctx.host);
    }
}

/// The head item (slot 2): `0x2afc68` creates it when there is none (item `0x1a73a0`, else 0x2f: class
/// [`HEAD_CLASS`]; draw distance 0x20, drawn, ready), in play and, through `0x2d4fc8`, in a scene (on the scene's
/// Ratchet: the host's hero pose). Not on armour 4 or with the cheats' bodies (`0x1a7a51` / `0x4a` / `0x53` / `0x52`):
/// the port wears armour 0. `0x2b09e0` then puts it at Ratchet's attachment [`HEAD_ATTACH`] (his head), out of the
/// moby loop; its pose is built from his (`0x2aa278`: joint 0 at rest, joints 1 and 2 his [`HEAD_JOINTS`]; the
/// engine draws it so). Shown (`0x2d4fc8` clears bit 0 in a scene). Its visor (`0x2b13c8`, slot 2: `+0x7c` = 1, its
/// piece 0 drawn on top, crate::moby::Moby::bangles) is down while Ratchet swims (movement group 0x11), in the
/// states 0x73 / 0x67 / 0x7f, or in one of the tutorial's no-item areas (`0x18c310`: outside the ship).
pub fn head_pass(world: &mut MobyWorld, ctx: &mut Ctx) {
    let it = &mut ctx.hero.items;
    if it.head.is_some_and(|id| world.mobys.get(id).is_none_or(|m| m.state >= 0xfe || m.o_class != HEAD_CLASS)) { it.head = None; }
    if it.head.is_none() && world.class(HEAD_CLASS).is_some() {
        let id = world.create(HEAD_CLASS);
        let m = &mut world.mobys[id];
        m.draw_dist = 0x20;
        m.drawn = 1;
        if m.shine != 0 { m.shine = 0x10; }
        it.head = Some(id);
    }
    let Some(id) = it.head else { return };
    let visor = ctx.hero.group == 0x11 || matches!(ctx.hero.state, 0x73 | 0x67 | 0x7f) || ctx.game.no_item_area;
    world.mobys[id].bangles = visor as u16;
    let Some((rows, pos)) = ctx.host.hero_attach(HEAD_ATTACH) else { return };
    {
        let m = &mut world.mobys[id];
        m.parent = ctx.hero_moby;
        (m.pos, m.rows) = (pos, rows);
        m.mode = (m.mode | mode::NO_UPDATE | mode::NO_MATRIX | mode::KEEP_ROWS) & !1;
        let f = rows[0];
        m.rot = [0.0, -f[2].atan2((f[0] * f[0] + f[1] * f[1]).sqrt()), f[1].atan2(f[0])];
    }
    world.rebuild(id, &*ctx.host);
}
