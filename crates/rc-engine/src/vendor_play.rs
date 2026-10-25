//! The vendor's menu in the engine (game mode 5; the menu itself is rc_game::vendor): the open `0x374038` on a
//! vendor moby's request, the frame `0x375f40` in place of the main loop's (the world stands: no moby or hero
//! update), the exit `0x374628`, the preview moby the menu asks for, and the camera at the vendor's front.
//!
//! **Open**: Ratchet in state 0x61 with the items kept away, his worn items hidden (`0x2d3208`: the hand, head and
//! feet mobys' mode bit 0), the help panel closed (`0x2f5310`), the bolt counter up (`0x30d910(18000000)`), the
//! vendor's class sound 3. **Frame**: the menu's frame; its sounds on the vendor moby (`0x3605f0(n, 0x11,
//! vendor)`); the preview moby made, placed and faded as the menu says (`0x37a8e8` / `0x37b728`: a created moby
//! with the game's init `0x311418`: mode 0x2010, draw distance 0x40, glow `0x80808080`, lit `0x3132e0(0x202020,
//! 0xe, 0xe)`; its position the vendor's rows · the offset, then centred on its bounding sphere (`0x2eeee0`)); the
//! prompt lease held for owner 6 (`0x327ae0(6, 0, 1)`); the HUD ticked with the bolt counter kept up (`0x30d910(0xb4)`)
//! and the selected ammo's panel; the view from the vendor's rows · (3.8, 0, 1.5 | 1.133) facing yaw + π.
//! **Exit**: state 0, Ratchet put at the vendor's rows · (3.5, 0, 0) facing it (`0x2b9e68`), the follow camera reset
//! (`0x4289f0(2)`: the caller's), the vendor's state 1, the preview gone.
//!
//! Not ported: the music pause and resume (`0x1a641a`), the GUI lump 0x5d's load, the exit's single world tick
//! (`0x375f40`'s substate 2 end: the next tick runs anyway), the hero's `0x18c0d5`.

use rc_game::game::GameState;
use rc_game::hero::Hero;
use rc_game::level::LevelData;
use rc_game::moby::{mode, MobyHost, MobyWorld};
use rc_game::pad::PadState;
use rc_game::vendor::{self, Out, Vendor};
use rc_formats::strings::Message;

/// The menu while open, its vendor moby and the preview moby it made.
#[derive(Default)]
pub(crate) struct VendorPlay {
    pub menu: Option<Vendor>,
    pub moby: Option<usize>,
    pub preview: Option<usize>,
    /// A preview class this level has not got (said once).
    preview_missing: Option<i16>,
}

/// What a frame leaves the caller: the view (eye, rows) while the menu is up; `closed` when it ended this frame
/// (the follow camera to reset).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    pub view: Option<([f32; 3], [[f32; 3]; 3])>,
    pub closed: bool,
}

/// The game's parts the menu works on.
pub(crate) struct Parts<'a> {
    pub world: &'a mut MobyWorld,
    pub level: &'a LevelData,
    pub game: &'a mut GameState,
    pub hero: &'a mut Hero,
    pub hud: &'a mut rc_game::hud::Hud,
    pub help: &'a mut rc_game::help::Panel,
    pub pad: &'a PadState,
    pub texts: &'a [Message],
    pub host: &'a mut dyn MobyHost,
}

/// `v + rows · d` (the vendor's rows, as `0x2ef318` / `0x2eee70` place the camera and the preview).
fn in_rows(pos: [f32; 3], rows: [[f32; 3]; 3], d: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| pos[k] + rows[0][k] * d[0] + rows[1][k] * d[1] + rows[2][k] * d[2])
}

/// The view's rows facing `yaw` (level): forward, left, up.
fn facing(yaw: f32) -> [[f32; 3]; 3] {
    let (s, c) = yaw.sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

impl VendorPlay {
    /// `0x374038(vendor)`: the menu opened on moby `id` (its kind from its pvar `+0x94`).
    pub(crate) fn open(&mut self, p: &mut Parts, id: usize, widescreen: bool) {
        let Some(v) = p.world.mobys.get(id) else { return };
        let kind = v.pv_i(0x94);
        let mut outs = Vec::new();
        let menu = Vendor::open(kind, p.game, &p.level.items, widescreen, &mut outs);
        println!("play: vendor {id}: menu open (kind {kind}, {} entries {:?}, selection {})", menu.entries.len(), menu.entries.iter().map(|e| (e.item, e.ammo)).collect::<Vec<_>>(), menu.sel);
        // `0x2c4560(0x61, 1)`, the items kept away, the worn items hidden (`0x2d3208`).
        p.hero.request_state(0x61, true);
        p.hero.items.no_use = 1;
        for slot in [p.hero.items.hand, p.hero.items.head, p.hero.items.feet[0], p.hero.items.feet[1]].into_iter().flatten() {
            if let Some(w) = p.world.mobys.get_mut(slot) { w.mode |= 1; }
        }
        // `0x2f5310`: the help panel closed.
        p.game.help_state = 0;
        p.help.index = None;
        p.help.timer = 0;
        // `0x30d910(18000000)`: the bolt counter up for good.
        p.game.show_bolts = Some(18_000_000);
        p.game.prompt.force(vendor::PROMPT_OWNER, 0, 1);
        self.menu = Some(menu);
        self.moby = Some(id);
        for o in outs { self.apply(p, o); }
    }

    fn apply(&mut self, p: &mut Parts, o: Out) -> bool {
        match o {
            Out::Sound(n) => {
                if let Some(v) = self.moby.and_then(|id| p.world.mobys.get(id)) { p.host.play_sound(v, n, 0x11); }
                false
            }
            Out::Equip(item) => {
                crate::play::equip(p.game.held, p.hero, item);
                false
            }
            Out::Help(text, slot) => {
                p.game.help(text, slot);
                false
            }
            Out::Exit => {
                self.exit(p);
                true
            }
        }
    }

    /// `0x375f40`: one frame while the menu is open.
    pub(crate) fn frame(&mut self, p: &mut Parts) -> Frame {
        let mut out = Frame::default();
        let outs = {
            let texts_src = p.texts;
            let texts = move |id: u32| rc_formats::strings::find_index(texts_src, id as i32).map(|i| texts_src[i].text.clone()).unwrap_or_default();
            let Some(menu) = self.menu.as_mut() else { return out };
            menu.frame(p.pad, p.game, &p.level.items, &texts)
        };
        p.game.prompt.force(vendor::PROMPT_OWNER, 0, 1);
        for o in outs {
            if self.apply(p, o) { out.closed = true; }
        }
        let (Some(menu), Some(id)) = (self.menu.as_ref(), self.moby) else { return out };
        let Some(v) = p.world.mobys.get(id) else { return out };
        let (vpos, vrows, vyaw) = (v.pos, v.rows, v.rot[2]);
        // The preview moby.
        let want = menu.preview();
        self.preview(p, want, vpos, vrows);
        // The HUD: the bolt counter kept up, the selected ammo entry's panel (`0x30d9c0` / `0x30d948`).
        let ammo = self.menu.as_ref().and_then(|m| m.hud_ammo(p.game, &p.level.items));
        p.hud.tick(p.hero.hp, p.game.max_health, Some(0xb4), None, ammo);
        let menu = self.menu.as_ref().expect("the menu");
        let eye = in_rows(vpos, vrows, menu.camera());
        let yaw = rc_game::hero::physics::add_rot(vyaw, std::f32::consts::PI);
        out.view = Some((eye, facing(yaw)));
        out
    }

    /// The preview moby as the menu wants it (module docs), or none.
    fn preview(&mut self, p: &mut Parts, want: Option<vendor::PreviewPose>, vpos: [f32; 3], vrows: [[f32; 3]; 3]) {
        let Some(pp) = want else {
            if let Some(id) = self.preview.take() { p.world.delete(id); }
            return;
        };
        let class = pp.class as i16;
        if let Some(id) = self.preview {
            if p.world.mobys.get(id).is_none_or(|x| x.o_class != class || x.state >= 0xfe) {
                p.world.delete(id);
                self.preview = None;
            }
        }
        if self.preview.is_none() {
            if p.world.class(class).is_none() {
                if self.preview_missing != Some(class) {
                    eprintln!("play: vendor: the preview's class {class:#x} is not on this level: no preview");
                    self.preview_missing = Some(class);
                }
                return;
            }
            let id = p.world.create(class);
            let x = &mut p.world.mobys[id];
            x.mode |= mode::NO_UPDATE | mode::GLOW | 0x2000;
            x.draw_dist = 0x40;
            x.drawn = 1;
            x.glow = 0x8080_8080;
            x.ambient = [0x20; 3];
            x.light = 0x0e0e;
            x.bangles = pp.bangles;
            x.drive = true;
            self.preview = Some(id);
            if pp.anim { p.host.set_anim(&p.world.mobys[id], 1, 0, 0); }
        }
        let Some(id) = self.preview else { return };
        let class_scale = p.world.class(class).map_or(1.0, |c| c.scale);
        {
            let x = &mut p.world.mobys[id];
            x.pos = in_rows(vpos, vrows, pp.offset);
            x.rot = pp.rot;
            x.scale = class_scale * pp.scale;
            x.alpha = pp.alpha;
        }
        // Its animation steps with the draw (`0x312ed8`'s list).
        p.host.advance(&p.world.mobys[id]);
        p.world.rebuild(id, &*p.host);
        // `0x2eeee0(1/1024, c, moby)`: centred on its bounding sphere across (x and y only: its height stays the
        // table's).
        let x = &mut p.world.mobys[id];
        let c = [x.bsphere[0] / 1024.0, x.bsphere[1] / 1024.0];
        x.pos = [x.pos[0] + (x.pos[0] - c[0]), x.pos[1] + (x.pos[1] - c[1]), x.pos[2]];
        p.world.rebuild(id, &*p.host);
    }

    /// `0x374628`: the menu over.
    fn exit(&mut self, p: &mut Parts) {
        let moby = self.moby.take();
        self.menu = None;
        self.preview(p, None, [0.0; 3], [[0.0; 3]; 3]);
        p.hero.request_state(0, true);
        p.hero.items.no_use = 0;
        if let Some(id) = moby {
            if let Some(v) = p.world.mobys.get(id) {
                // `0x2b9e68`: Ratchet 3.5 in front of the vendor facing it.
                let pos = in_rows(v.pos, v.rows, vendor::EXIT_AT);
                let yaw = rc_game::hero::physics::add_rot(v.rot[2], std::f32::consts::PI);
                p.hero.teleport(pos, [0.0, 0.0, yaw]);
                println!("play: vendor {id}: menu closed; Ratchet at ({:.1}, {:.1}, {:.1}) facing {yaw:.2}", pos[0], pos[1], pos[2]);
            }
            rc_game::moby::classes::vendor::on_exit(p.world, id);
        }
    }
}
