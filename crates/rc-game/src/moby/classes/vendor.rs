//! The vendor machine, class 0xb (every planet's overlay has the update; level01's is `0x381c70`, read from its
//! disassembly — the engine functions it calls are named by their level00 addresses). Its pvars: `+0x00` / `+0x04`
//! cleared at init, `+0x0c` the hologram moby (class 0x477), `+0x10` / `+0x50` its two manipulator records,
//! `+0x90` the hologram's scale 0..1, `+0x94` the kind (crate::vendor::Kind::of_pvar: 0 weapons, 1 mods, 2
//! armour, 3 the Gadgetron weapons, 4 the ship), `+0x98` ≠ 0: only in the challenge mode. Globals: the glow phase
//! `gp−0x100` and the manipulator angles `gp−0x108` / `gp−0x104` (crate::game::GameState::vendor_phase).
//!
//! Every tick: the phase += 0.05, past 3.14 back to −3.14; the glow word `+0x78` = `((int)(sin φ · 12) + 0x60) ·
//! 0x20100 | 0x80000000` (the ship vendor, kind 2: `· 0x101`), the hologram's `0x80f0f0b0` (kind 2: `0x80008080`).
//! The Gadgetron kind hides itself and its hologram (mode bit 0, `+0x31` 0, `+0x98` 0). Then by state:
//! * **0** (init): a challenge-only vendor outside it deletes itself (`0x307440`). Else state 1, `+0x00` = `+0x04`
//!   = 0, the hologram created (`0x3070d8(0x477)`): mode |= 2, draw distance 0x40, at the vendor + (0, 0, 1.95)
//!   (15 lower for the Gadgetron kind), `+0x63` = 0x20, `+0x90` = 0, the manipulators attached to its joint lists
//!   0 and 1 (`0x308238`), its matrix built (`0x309b28`).
//! * **1** (far): `+0x90` −= 0.1 while above 0; at 0 the hologram hidden. Every 8th frame (`0x1b2088 & 7`): XY
//!   distance to Ratchet ≤ 16 and |Δz| ≤ 8 → state 2, the hologram shown, the kind's classes asked for
//!   (`0x372618(level, kind)`; the ship vendor also `0x369240(level)`). Then the tail (not the Gadgetron kind).
//! * **2** (near): `+0x90` += 0.1 while below 1. Every 8th frame: XY > 18 or |Δz| > 10 → state 1, return. The
//!   tail, then the prompt: Ratchet in movement group 0 or 1 (or state 3), not in state 0x1c / 0x31, his body
//!   Ratchet's (`0x18c0d4` = 0), XY ≤ 4, |Δz| ≤ 2 and facing the vendor within π/2 (`0x2e5250` of his yaw and
//!   `0x2e4818` of the offset) → the prompt lease for owner 1 with the kind's text (`0x327ae0(1, text, 1)`: "△
//!   Activate Vendor", the armour's 0x1350, the ship's 0x1351); △ pressed (`0x138324 & 0x10`) with it → state 4.
//! * **3** (the menu open; `0x374628` sets 1 again): nothing.
//! * **4** (opening): `+0x90` −= 0.1 while above 0; at 0 the hologram hidden and `0x32c5f0(5, 1, vendor)` asks for
//!   game mode 5 ([`crate::game::GameState::vendor_open`]), state 3. Else the tail.
//! * **Tail**: the draw callback `0x381568` registered (`0x2dd008`: the beam and glow points), the manipulators
//!   turned about z by the angles (`0x2e4e68(rec, angle, 2)`), the angles += 0.01 / −= 0.01 (`0x2e5168`), the
//!   hologram's scale = `+0x90` · its class scale · 2.5, its matrix built.
//!
//! Not ported: the draw callback (the beam: an engine draw), the manipulators (the hologram's two spinning joints:
//! the port's moby animation has no class manipulators), the classes' load (`0x372618`: the port has every
//! class), the mod / armour / ship kinds' menus (the prompt shows, △ says so; crate::vendor).

use crate::moby::{mode, Ctx, MobyWorld};
use crate::vendor::{Kind, HOLOGRAM_CLASS};

/// Pvar offsets.
const CHILD: usize = 0x0c;
const SCALE: usize = 0x90;
const KIND: usize = 0x94;
const CHALLENGE_ONLY: usize = 0x98;
const PVAR_SIZE: usize = 0x9c;
/// The hologram's height over the vendor and the Gadgetron kind's drop.
const HOLOGRAM_UP: f32 = 1.95;
const GADGETRON_DROP: f32 = 15.0;
/// The near and far ranges (XY, |Δz|) and the use range.
const NEAR: (f32, f32) = (16.0, 8.0);
const FAR: (f32, f32) = (18.0, 10.0);
const USE: (f32, f32) = (4.0, 2.0);
/// The prompt texts by kind: "△ Activate Vendor", "△ Activate Armor Vendor", "△ Activate Ship Vendor".
const PROMPT: u32 = 0x12eb;
const PROMPT_ARMOR: u32 = 0x1350;
const PROMPT_SHIP: u32 = 0x1351;
/// The prompt's owner: the vendor moby (`0x327ae0(1, …)`).
const OWNER: i32 = 1;
/// The phase's wrap (`0x4048f5c3`: 3.14, not π).
const PHASE_WRAP: f32 = 3.140_000_1;

fn child(w: &MobyWorld, id: usize) -> Option<usize> {
    let c = w.mobys[id].pv_link(CHILD)?;
    w.mobys.get(c).filter(|m| m.state < 0xfe && m.o_class == HOLOGRAM_CLASS).map(|_| c)
}

/// The update (level01 `0x381c70`).
pub fn update(world: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if world.mobys[id].pvars.len() < PVAR_SIZE { world.mobys[id].pvars.resize(PVAR_SIZE, 0); }
    // The glow.
    let ph = &mut ctx.game.vendor_phase;
    ph[0] += 0.05;
    if PHASE_WRAP < ph[0] { ph[0] = -PHASE_WRAP; }
    let kind = world.mobys[id].pv_i(KIND);
    let (mul, holo) = if kind == 2 { (0x101u32, 0x8080u32) } else { (0x20100, 0x80f0_f0b0) };
    let k = ((ph[0].sin() * 12.0) as i32 + 0x60) as u32;
    world.mobys[id].glow = k.wrapping_mul(mul) | 0x8000_0000;
    let c = child(world, id);
    if let Some(c) = c { world.mobys[c].glow = holo | 0x8000_0000; }
    if kind == 3 {
        let m = &mut world.mobys[id];
        m.mode |= 1;
        m.drawn = 0;
        if let Some(c) = c {
            world.mobys[c].mode |= 1;
            world.mobys[c].drawn = 0;
        }
    }
    let state = world.mobys[id].state;
    match state {
        0 => {
            if world.mobys[id].pv_i(CHALLENGE_ONLY) != 0 && !ctx.game.challenge {
                println!("rc_game::moby::classes::vendor: vendor #{id} is the challenge mode's only: deleted");
                world.delete(id);
                return;
            }
            world.mobys[id].state = 1;
            world.mobys[id].set_pv_i(0, 0);
            world.mobys[id].set_pv_i(4, 0);
            if world.class(HOLOGRAM_CLASS).is_some() {
                let c = world.create(HOLOGRAM_CLASS);
                let pos = world.mobys[id].pos;
                let m = &mut world.mobys[c];
                m.mode |= mode::NO_UPDATE;
                m.draw_dist = 0x40;
                m.drawn = 1;
                m.pos = [pos[0], pos[1], pos[2] + HOLOGRAM_UP - if kind == 3 { GADGETRON_DROP } else { 0.0 }];
                m.shine = 0x20;
                m.drive = true;
                world.mobys[id].set_pv_i(CHILD, c as i32);
                world.mobys[id].set_pv_f(SCALE, 0.0);
                // Then only the hologram's matrix (`0x38203c`: the tail's last call).
                world.rebuild(c, &*ctx.host);
            }
        }
        1 | 2 => {
            let s = world.mobys[id].pv_f(SCALE);
            if state == 1 {
                let s = if 0.0 < s { s - 0.1 } else { s };
                world.mobys[id].set_pv_f(SCALE, s);
                if s <= 0.0 {
                    if let Some(c) = c { world.mobys[c].mode |= 1; }
                }
            } else if s < 1.0 {
                world.mobys[id].set_pv_f(SCALE, s + 0.1);
            }
            let (pos, hero) = (world.mobys[id].pos, ctx.hero.pos);
            let (dx, dy) = (pos[0] - hero[0], pos[1] - hero[1]);
            let xy = (dx * dx + dy * dy).sqrt();
            let dz = (pos[2] - hero[2]).abs();
            if ctx.game.frame & 7 == 0 {
                if state == 1 {
                    if xy <= NEAR.0 && dz <= NEAR.1 {
                        world.mobys[id].state = 2;
                        if let Some(c) = c { world.mobys[c].mode &= !1; }
                    }
                } else if FAR.0 < xy || FAR.1 < dz {
                    world.mobys[id].state = 1;
                    return;
                }
            }
            if kind != 3 { tail(world, id, ctx); }
            if state != 2 { return; }
            // The prompt rule.
            let h = &*ctx.hero;
            let mut can = true;
            if h.group != 0 && h.state != 3 && h.group != 1 { can = false; }
            if h.state == 0x1c || h.state == 0x31 { can = false; }
            if h.mode != 0 { can = false; }
            if USE.0 < xy || USE.1 < dz { can = false; }
            let heading = (pos[1] - hero[1]).atan2(pos[0] - hero[0]);
            if std::f32::consts::FRAC_PI_2 < crate::hero::physics::rot_dist(h.rot[2], heading) { can = false; }
            let mut lease = false;
            if can {
                let text = match kind {
                    2 => PROMPT_ARMOR,
                    4 => PROMPT_SHIP,
                    _ => PROMPT,
                };
                lease = ctx.game.prompt.force(OWNER, text, 1) != 0;
            }
            if ctx.hero.items.pressed & crate::pad::button::TRIANGLE != 0 && can && lease {
                let (k, _) = Kind::of_pvar(kind);
                if k != Kind::Ship {
                    world.mobys[id].state = 4;
                } else {
                    eprintln!("rc_game::moby::classes::vendor: the ship vendor's menu is not ported");
                }
            }
        }
        4 => {
            let s = world.mobys[id].pv_f(SCALE);
            let s = if 0.0 < s { s - 0.1 } else { s };
            world.mobys[id].set_pv_f(SCALE, s);
            if s <= 0.0 {
                if let Some(c) = c { world.mobys[c].mode |= 1; }
                ctx.game.vendor_open = Some(id);
                world.mobys[id].state = 3;
                return;
            }
            if kind != 3 { tail(world, id, ctx); }
        }
        _ => {}
    }
}

/// The tail: the angles advanced, the hologram scaled by `+0x90` · 2.5 of its class.
fn tail(world: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    let ph = &mut ctx.game.vendor_phase;
    ph[1] = crate::hero::physics::add_rot(ph[1], 0.01);
    ph[2] = crate::hero::physics::add_rot(ph[2], -0.01);
    let s = world.mobys[id].pv_f(SCALE);
    if let Some(c) = child(world, id) {
        let cs = world.class(HOLOGRAM_CLASS).map_or(1.0, |c| c.scale);
        world.mobys[c].scale = s * cs * 2.5;
        // The moby loop's `0x313420` on it (out of the port's run list with no update of its own).
        ctx.host.advance(&world.mobys[c]);
        world.rebuild(c, &*ctx.host);
    }
}

/// `0x374628`'s write: the menu over, state 1.
pub fn on_exit(world: &mut MobyWorld, id: usize) {
    if let Some(m) = world.mobys.get_mut(id).filter(|m| m.o_class == crate::vendor::CLASS) { m.state = 1; }
}
