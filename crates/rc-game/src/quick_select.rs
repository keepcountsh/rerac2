//! The quick select (L00): the ring of eight items △ brings up. Three parts of the game make it.
//!
//! **The hold** (`0x2fcc88`, run first in the frame by `0x3706f0`; its state `gp−0x5c80`): a request (`0x2fcc60`)
//! starts it, the next frame it waits [`HOLD_TICKS`] (`0x1a9374`) more, and △ still held then makes it live (3,
//! `0x2fcc78`); △ let go before that drops it.
//!
//! **The screen** (GUI screen 0x23: init `0x2f8498` / `0x2f83d0`, update `0x2f84e8`, draw `0x2f8db8`): opened by △
//! (the hero's transitions `0x2cb650` with Quick Select Pause off; with it on, the game's GUI mode 4 kind 7 the main
//! loop enters once the hold is live). Its table (`0x27dac0`, 8 × 0x1c: the icon, the d-pad's neighbours, the item)
//! holds the saved slots (`0x1a73b8`, crate::game::GameState::ring). With the ring up (the hold live, or the paused
//! mode) the left stick past 0.9 picks the slot its angle points at (slot 0 at the top, clockwise; the cardinal slots
//! narrower, the slot held while the stick stays within 33.75° of it, an empty slot giving way to its neighbour on
//! the stick's side), the d-pad walks the neighbours (with the stick centred). Each frame △ is held the text fades in
//! (`+0x70`, 8 steps); let go, it fades out, and at the end the slot's item is asked for (`0x18c0e8`, its ammo cut to
//! the most) and the screen closes. A new slot restarts the name's fade (`+0x71`) and plays sound 3.
//!
//! **The ring** (the HUD panel's widgets, built `0x442020`, updated `0x443c08`, drawn `0x444900`): kept up by the
//! screen each frame (`0x4438e0(n)`: n frames, 2 or 15 paused), its six faders run forward from closed and back n
//! frames after the last keep. The ring's two halves (sprites 7 and 6, drawn again mirrored), a back per slot
//! (sprites 8..15, slot k's is k − 1: red `0x70202080` for an item out of ammo), the items' icons (32 × 32 on an
//! ellipse 82.14 × 76.442 about (109, 189)) and the selected slot's pulsing highlight (sprite 0x10, frame k − 1). The
//! screen draws the item's name (split at its first '-' or ' ' into two lines) and its ammo under the ring.
//!
//! Not ported: the classes' load wait (`0x305cc0` / `0x305ab0`: the port has every class), the game statistics
//! counters, the d-pad's rumble (`0x31dc30` with its last flag), the dim layer `0x1a9054+0x3f9dc`, the items' tints
//! (`0x42ddd8`) and XP bars (`+0x36f28`), Clank's ring (`0x2f9380` / `0x2f93c8` / `0x2f98d0`), the message box's veto
//! (`0x1a8fb4`).

use crate::hud::{Align, Draw, Fader};
use crate::pad::{button, PadState};

/// `0x1a9374`: the frames △ is held before the ring comes up.
pub const HOLD_TICKS: i32 = 4;
/// `gp+0x2ba8`: the slots.
pub const SLOTS: usize = 8;
/// `0x4438e0`'s frames: the ring stays up this long after the screen's last keep (`gp−0x5f70` paused).
const KEEP: i32 = 2;
const KEEP_PAUSED: i32 = 15;
/// `0x27dac0` (`+0x08 / +0x0c / +0x10 / +0x14`): each slot's d-pad neighbours, left, right, up, down.
const NEIGHBOURS: [[i32; 4]; SLOTS] = [[7, 1, 0, 4], [0, 2, 0, 2], [6, 2, 1, 3], [4, 2, 2, 4], [5, 3, 0, 4], [6, 4, 6, 4], [6, 2, 7, 5], [6, 0, 0, 6]];
/// `0x1a90e8..0x1a90f4`: the slot a d-pad press picks with none selected, left, right, up, down.
const DPAD_FIRST: [i32; 4] = [6, 2, 0, 4];
/// The ring's centre (all its sprites) and the texts' anchor (screen slot 3 at (20, 208), 210 × 200, flag 4:
/// `0x2f7d48` takes half the height).
const RING_AT: [f32; 2] = [125.0, 208.0];
const TEXT_AT: [f32; 2] = [20.0, 108.0];

/// One slot as the screen and the ring read it: the item (0 empty), its record's icon (`+0x2c`), name (`−0x08`), its
/// ammo, whether it uses ammo and the most (`+0x78`, `+0x7e`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub item: i32,
    pub icon: u16,
    pub name: u32,
    pub ammo: i32,
    pub uses_ammo: bool,
    pub max: i32,
}

impl Slot {
    fn used(&self) -> bool { self.icon != 0 }
}

/// What a frame of the screen asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// The item to hold (`0x18c0e8`), its ammo cut to the most when it has more.
    Equip(i32),
    /// The paused mode ends (`0x2f41f0`).
    Resume,
    /// Sound 3 of the GUI's set (`0x360750(3, 0, 0)`): a new slot.
    Sound(u32),
}

/// Screen 0x23's record (`+0x70..+0x7b`).
#[derive(Clone, Copy, Debug)]
struct Screen {
    /// `+0x74`: the selected slot (−1 none; −2 before the first reset).
    cursor: i32,
    /// `+0x78`: until its top byte is 1, the reset's countdown (the stick held from before the ring waits 30 frames).
    reset: i32,
    /// `+0x70` / `+0x71`: the text's fade and the name's (0..8).
    fade: u8,
    name_fade: u8,
}

/// The HUD panel's ring (`+0x2e0..+0x804`, `+0xc0c..+0xeb4`, `+0x1594` / `+0x1598`).
#[derive(Clone, Debug)]
struct Ring {
    /// `+0xc0c`: the backs' colour; `+0xc94` their scale (and sprite 7's); `+0xd1c` / `+0xda4`: sprite 6's colour and
    /// scale; `+0xe2c`: the icons' colour; `+0xeb4`: the shared transform's scale (`+0x804`).
    back_colour: Fader,
    back_size: Fader,
    glow_colour: Fader,
    glow_size: Fader,
    icon_colour: Fader,
    scale: Fader,
    /// `+0x1594`: frames left before it closes; `+0x1598`: closed.
    keep: i32,
    closed: bool,
    /// `+0x378 + 0x4c·k`: the backs' colours.
    backs: [u32; SLOTS],
    /// `+0x5d8`: the highlight's frame when shown, and its colour.
    highlight: Option<f32>,
    highlight_colour: u32,
    /// `gp−0x4e1c`: the pulse's frame counter (`0x31dc30`).
    pulse: i32,
}

impl Default for Ring {
    fn default() -> Ring {
        let mut back_colour = Fader::colour([(0.0, 0x0044_2d00), (0.3, 0x6044_2d00), (1.0, 0x6044_2d00)], 3);
        back_colour.tangents[1] = [0.0, 2.0];
        let mut glow_colour = Fader::colour([(0.0, 0x00f0_c070), (0.3, 0x55f0_c070), (1.0, 0x55f0_c070)], 3);
        glow_colour.tangents[1] = [0.0, 3.0];
        let mut icon_colour = Fader::colour([(0.0, 0x00f0_f0b0), (0.5, 0x00f0_f0b0), (1.0, 0x60f0_f0b0)], 3);
        icon_colour.tangents = [[0.0, 0.0], [0.0, 0.0]];
        Ring {
            back_colour,
            back_size: Fader::floats([(0.0, [0.0, 0.0]), (0.3, [1.0, 1.0]), (1.0, [1.0, 1.0])], 3),
            glow_colour,
            glow_size: Fader::floats([(0.0, [1.2, 1.2]), (0.3, [1.0, 1.0]), (1.0, [1.0, 1.0])], 3),
            icon_colour,
            scale: Fader::floats([(0.0, [0.0, 0.0]), (0.01, [1.0, 1.0]), (1.0, [1.0, 1.0])], 3),
            keep: 0,
            closed: true,
            backs: [0x0044_2d00; SLOTS],
            highlight: None,
            highlight_colour: 0x70ff_feed,
            pulse: 0,
        }
    }
}

impl Ring {
    fn faders(&mut self) -> [&mut Fader; 6] {
        [&mut self.back_colour, &mut self.back_size, &mut self.glow_colour, &mut self.glow_size, &mut self.icon_colour, &mut self.scale]
    }

    /// `0x4438e0(n)`: up for `n` more frames; from closed the faders run forward.
    fn keep(&mut self, n: i32) {
        self.keep = n;
        if self.closed {
            self.closed = false;
            self.highlight = None;
            for f in self.faders() { f.run(1); }
        }
    }

    /// `0x443ac8(frame, 1)`: the highlight at `frame`, its colour the pulse (`0x31dc30(0x60442d00, 0x70fffeed, 20)`:
    /// a triangle over 40 frames, from the second colour; a d-pad press starts it over).
    fn highlight(&mut self, frame: f32, dpad: bool) {
        if dpad { self.pulse = 0; }
        self.pulse += 1;
        let i = self.pulse % 40;
        let t = if i < 20 { i as f32 / 20.0 } else { 1.0 - (i - 20) as f32 / 20.0 };
        self.highlight_colour = crate::hud::lerp_rgba(t, 0x70ff_feed, 0x6044_2d00);
        self.highlight = Some(frame);
    }

    /// `0x443c08`'s part: the backs' colours by their slots, the faders, the close.
    fn tick(&mut self, slots: &[Slot; SLOTS]) {
        for (k, s) in slots.iter().enumerate() {
            if !s.used() { continue; }
            let empty = s.uses_ammo && s.max != 0 && s.ammo == 0;
            self.backs[(k + SLOTS - 1) % SLOTS] = if empty { 0x7020_2080 } else { 0x6044_2d00 };
        }
        for f in self.faders() { f.step(); }
        if self.keep != 0 {
            self.keep -= 1;
            if self.keep == 0 && !self.closed {
                self.closed = true;
                self.highlight = None;
                for f in self.faders() { f.run(-1); }
            }
        }
    }

    /// `0x444900`'s part.
    fn draw(&self, slots: &[Slot; SLOTS], out: &mut Vec<Draw>) {
        let on = self.scale.sample().0;
        let (back_size, back_alpha) = (self.back_size.sample().0, self.back_colour.sample().1 & 0xff00_0000);
        let (glow_size, glow_colour) = (self.glow_size.sample().0, self.glow_colour.sample().1);
        let scaled = |s: [f32; 2]| [s[0] * on[0], s[1] * on[1]];
        let (bs, gs) = (scaled(back_size), scaled(glow_size));
        let ring = 0x0044_2d00 | back_alpha;
        for mirror in [1.0, -1.0] {
            out.push(Draw::Sprite { id: 7, pos: RING_AT, scale: [bs[0] * mirror, bs[1]], frame: 0.0, colour: ring });
            out.push(Draw::Sprite { id: 6, pos: RING_AT, scale: [gs[0] * mirror, gs[1]], frame: 0.0, colour: glow_colour });
        }
        for (j, &c) in self.backs.iter().enumerate() {
            out.push(Draw::Sprite { id: 8 + j as u32, pos: RING_AT, scale: bs, frame: 0.0, colour: c & 0xff_ffff | back_alpha });
        }
        let icon_colour = self.icon_colour.sample().1;
        for (k, s) in slots.iter().enumerate() {
            if !s.used() { continue; }
            let a = crate::hero::physics::add_rot(k as f32 * 2.0 * 0.392_699_1 - std::f32::consts::PI, std::f32::consts::FRAC_PI_2);
            let pos = [((a.cos() * 82.14) as i32 + 0x6d) as f32, ((a.sin() * 76.442) as i32 + 0xbd) as f32];
            out.push(Draw::Icon { id: s.icon, frame: 0, pos, size: [32.0, 32.0], colour: icon_colour });
        }
        if let Some(frame) = self.highlight {
            out.push(Draw::Sprite { id: 0x10, pos: RING_AT, scale: [1.0, 1.0], frame, colour: self.highlight_colour });
        }
    }
}

/// The quick select: the hold, the screen and the ring.
#[derive(Clone, Debug, Default)]
pub struct QuickSelect {
    /// `gp−0x5c80` and its countdown `gp+0x2b38`.
    hold: u8,
    hold_left: i32,
    screen: Option<Screen>,
    ring: Ring,
}

impl QuickSelect {
    /// `0x2fcc60`: the hold asked for.
    pub fn request(&mut self) {
        if self.hold == 0 { self.hold = 1; }
    }

    /// `0x2fcc78`: the hold is live.
    pub fn live(&self) -> bool { self.hold == 3 }

    /// `0x3706f0`'s part, first in the frame: the hold's frame; then with Quick Select Pause on (`pause`), △ within
    /// the last 10 frames (`0x330240(0x10, gp−0x6038)`, the frames since a message box, at most 10) with the game free
    /// for it (`ok`: past its first 8 frames, not dying or in a vehicle, the wrench not flying, not in a scene) asks
    /// for the hold, and once it is live the game stops in the ring's mode (GUI mode 4 kind 7): true.
    pub fn main_loop(&mut self, pad: &PadState, pause: bool, ok: bool) -> bool {
        self.hold_tick(pad.held);
        if !(pause && ok && pad.pressed_within(button::TRIANGLE, 10).is_some()) { return false; }
        self.request();
        if !self.live() { return false; }
        self.open();
        true
    }

    /// `0x2fcc88`: the hold's frame (`held`: the pad's buttons).
    pub fn hold_tick(&mut self, held: u32) {
        match self.hold {
            1 => (self.hold, self.hold_left) = (2, HOLD_TICKS),
            2 => {
                self.hold_left -= 1;
                // △ held: live once the countdown is out, else still waiting; let go: dropped.
                if held & button::TRIANGLE != 0 {
                    if self.hold_left == 0 { self.hold = 3; }
                    return;
                }
                self.hold = 0;
            }
            _ => {}
        }
    }

    /// `0x2f79d8(0x23, …)` with the init `0x2f8498`: the screen opens (once).
    pub fn open(&mut self) {
        if self.screen.is_none() { self.screen = Some(Screen { cursor: -2, reset: 0x1e, fade: 0, name_fade: 0 }); }
    }

    pub fn is_open(&self) -> bool { self.screen.is_some() }

    /// `0x2f84e8`: the screen's frame (`paused`: Quick Select Pause is on and the game's paused mode runs it; `pause`:
    /// the option).
    pub fn update(&mut self, pad: &PadState, slots: &[Slot; SLOTS], pause: bool, paused: bool) -> Vec<Out> {
        let mut out = Vec::new();
        let Some(mut s) = self.screen else { return out };
        let active = if pause { paused } else {
            self.request();
            self.live()
        };
        if active { self.ring.keep(if pause { KEEP_PAUSED } else { KEEP }); }
        let (held, pressed) = (pad.held_unmirrored, pad.pressed_unmirrored);
        let dpad = pressed & (button::UP | button::DOWN | button::LEFT | button::RIGHT);
        let [x, y] = [pad.analog_copy[2].to_f32(), pad.analog_copy[3].to_f32()];
        let len = (x * x + y * y).sqrt();
        let (ux, uy) = if len != 0.0 { (x / len, y / len) } else { (x, y) };
        let angle = crate::hero::physics::atan2(ux, uy);
        if s.reset >> 24 == 0 && (dpad != 0 || len < 0.5 || { s.reset -= 1; s.reset == -1 }) {
            s.cursor = -1;
            s.reset = 0x0100_00ff;
        }
        let prev = s.cursor;
        let used = |k: i32| k >= 0 && slots[k as usize % SLOTS].used();
        if (s.reset >> 24) & 0xff == 1 && held & button::TRIANGLE != 0 && active {
            let mut side = 0;
            let n = SLOTS as f32;
            let tau = std::f32::consts::TAU;
            if 0.9 < len {
                use std::f32::consts::{FRAC_PI_2, PI};
                let f15 = angle + PI + PI + FRAC_PI_2 + PI / n;
                let f11 = f15 * 0.159_154_94 * n;
                let near = prev != -1 && crate::hero::physics::rot_dist(prev as f32 * tau / n - PI, sub_rot(angle, FRAC_PI_2)) <= 0.589_048_7;
                if !near {
                    let mut k = (f11 as i32) % SLOTS as i32;
                    if k & 1 == 0 {
                        let frac = f11 - (f11 as i32) as f32;
                        if 0.8 < frac { k += 1 } else if frac < 0.2 { k -= 1 }
                        k = (k + SLOTS as i32) % SLOTS as i32;
                    }
                    s.cursor = k;
                    side = if 0.0 < wrap(f15 - (k as f32 * 0.785_398_24 + 0.392_699_12)) { 1 } else { -1 };
                }
            } else if !pad.stick_active && dpad != 0 {
                // Up, down, left, right in turn, each from where the last left it.
                let mut c = s.cursor;
                for (bit, dir) in [(button::UP, 2), (button::DOWN, 3), (button::LEFT, 0), (button::RIGHT, 1)] {
                    if pressed & bit == 0 { continue; }
                    c = if c == -1 { DPAD_FIRST[dir] } else { NEIGHBOURS[c as usize][dir] };
                }
                s.cursor = c;
            }
            if s.cursor >= 0 && !used(s.cursor) && side != 0 {
                let alt = (s.cursor + side).rem_euclid(SLOTS as i32);
                if used(alt) { s.cursor = alt; }
            }
        }
        if s.cursor == prev {
            if s.name_fade < 8 { s.name_fade += 1; }
        } else {
            s.name_fade = 0;
            out.push(Out::Sound(3));
        }
        if held & button::TRIANGLE == 0 {
            let done = s.fade == 0 || { s.fade -= 1; s.fade == 0 };
            if done && s.cursor >= 0 && slots[s.cursor as usize].item != 0 { out.push(Out::Equip(slots[s.cursor as usize].item)); }
            if pause { out.push(Out::Resume) } else { self.hold = 0; }
        } else if s.fade < 8 {
            s.fade += 1;
        }
        self.screen = if s.fade == 0 { None } else { Some(s) };
        out
    }

    /// The ring's frame (with the HUD's, `0x443c08`).
    pub fn ring_tick(&mut self, slots: &[Slot; SLOTS]) { self.ring.tick(slots); }

    /// `0x2f8db8` (the highlight, the name and the ammo), then the ring (`0x444900`). `name` gives a text id's
    /// bytes.
    pub fn draw(&mut self, slots: &[Slot; SLOTS], pad: &PadState, name: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Draw>) {
        let mut texts = Vec::new();
        if let Some(s) = self.screen {
            let a = (s.fade as f32 / 8.0).clamp(0.0, 1.0);
            let b = (s.name_fade as f32 / 8.0).clamp(0.0, 1.0);
            let dpad = pad.pressed_unmirrored & (button::UP | button::DOWN | button::LEFT | button::RIGHT) != 0;
            if s.cursor >= 0 && slots[s.cursor as usize].used() {
                self.ring.highlight(((s.cursor + 7) % 8) as f32, dpad);
                let slot = slots[s.cursor as usize];
                let f = a * b;
                let white = crate::hud::lerp_rgba(f, 0x00f0_f0f0, 0x80f0_f0f0);
                let x = TEXT_AT[0] + 0x69 as f32;
                if slot.uses_ammo && slot.max != 0 {
                    let colour = if slot.ammo == 0 { crate::hud::lerp_rgba(f, 0x0040_40ff, 0x8040_40ff) } else { white };
                    texts.push(Draw::Text { text: format!("{}/{}", slot.ammo, slot.max), pos: [x, TEXT_AT[1] + 0x70 as f32], scale: 1.0, colour, align: Align::Centre });
                }
                let text = name(slot.name);
                let (first, second) = split_name(&text);
                let y = TEXT_AT[1] + if slot.uses_ammo && slot.max != 0 { 0x4b } else if second.is_some() { 0x55 } else { 100 } as f32;
                let line = |t: &[u8]| String::from_utf8_lossy(t).into_owned();
                texts.push(Draw::Text { text: line(first), pos: [x, y], scale: 1.0, colour: white, align: Align::Centre });
                if let Some(t) = second { texts.push(Draw::Text { text: line(t), pos: [x, y + 16.0], scale: 1.0, colour: white, align: Align::Centre }); }
            }
        }
        self.ring.draw(slots, out);
        out.extend(texts);
    }
}

/// The slots (`0x2f83d0`): the saved items with their records' icons, names and ammo (the record is the item's row:
/// the port keeps no upgrades' rows).
pub fn slots(game: &crate::game::GameState, records: &[rc_formats::disc::menu_tables::ItemRecord]) -> [Slot; SLOTS] {
    std::array::from_fn(|k| {
        let item = game.ring[k];
        let Some(r) = usize::try_from(item).ok().filter(|_| item != 0).and_then(|i| records.get(i)) else { return Slot::default() };
        let ammo = game.ammo.get(item as usize).copied().unwrap_or(0);
        Slot { item, icon: r.icon, name: r.name, ammo, uses_ammo: r.uses_ammo, max: r.max_ammo as i32 }
    })
}

/// `0x2efea8`: `a − b` wrapped to (−π, π].
fn sub_rot(a: f32, b: f32) -> f32 {
    use std::f32::consts::PI;
    let mut s = a - b;
    let low = s < -PI;
    if PI <= s { s = s - PI - PI; }
    if low { s = s + PI + PI; }
    s
}

/// `0x2efef0`: `a` wrapped to [−π, π) by its fraction of a turn.
fn wrap(a: f32) -> f32 {
    let f = a * 0.159_154_94 + 0.5;
    ((f - f.floor()) - 0.5) * std::f32::consts::TAU
}

/// The name's two lines (`0x2f8db8`): at the first '-' after the first character (kept on the first line), else at
/// the first ' ' (dropped); one line without either.
fn split_name(t: &[u8]) -> (&[u8], Option<&[u8]>) {
    let t = t.split(|&c| c == 0).next().unwrap_or(&[]);
    if let Some(i) = (1..t.len()).find(|&i| t[i] == b'-') { return (&t[..i + 1], Some(&t[i + 1..])); }
    if let Some(i) = (1..t.len()).find(|&i| t[i] == b' ') { return (&t[..i], Some(&t[i + 1..])); }
    (t, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_like_the_game() {
        assert_eq!(split_name(b"Lancer"), (&b"Lancer"[..], None));
        assert_eq!(split_name(b"Gravity Bomb"), (&b"Gravity"[..], Some(&b"Bomb"[..])));
        assert_eq!(split_name(b"Synth-o-Matic"), (&b"Synth-"[..], Some(&b"o-Matic"[..])));
    }
}
