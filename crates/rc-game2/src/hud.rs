//! The in-game HUD's counters (L00, the GUI object `0x1a9054`): the bolt counter (part of the weapon / bolt panel,
//! `+0x376c8`, built by `0x442020`, updated by `0x443c08`, drawn by `0x444900`) and Ratchet's health (the "HudHeroXP"
//! widget, `+0x37ed0` = the panel's `+0x808`: `0x440ab0`, `0x441968`, `0x441a68`). They show for a while after a
//! change (`0x30d910(n)`: the bolt counter, n ticks; `0x30d9f8(n)`: the health) and fade in and out with the GUI's
//! animators. Screen units are the GUI's 512 × 416 (crate::gui).
//!
//! **Animators.** A *blend* (`0x4406c8`, stepped by `0x440878`) eases a parameter t between 0 and 1 at its speed
//! (0.0667 a tick) with the Hermite curve `0x445be8(t, 0, m0, m1, 1)` and writes the lerp of two keys into its
//! targets: a colour (`0x2f0000`, its own tangents), a size and a position. A *fader* (`0x440228`, stepped by
//! `0x4403d8`) runs t up or down through up to three keyed values (a colour or two floats).
//!
//! **Bolt counter** (`+0x194..+0x2a0`): the back and outline sprites 3 and 4 (their frame = the digit count − 1, at
//! most 6) at (498, 10), the count right-aligned at (440, 15) in scale 0.9 with a separator every three digits, the
//! icon 0x7567 (32 × 32) at (457, 15); shown by `0x443b80(n)` (the blends run forward), hidden when the timer runs
//! out (the blends run back).
//!
//! **Health** (at (147, 10)): pills in groups of four (quarter sprites mirrored into a ring, `0x441560`): filled
//! 0x1a / empty 0x19 up to 20 health (one row of five groups, 16.5 apart from (184, 29)), 0x1b / 0x18 above (rows of
//! ten); the back 0x11 and outline 0x12 sprites sized by the pill count (`0x441238`); a raised limit blinks the new
//! pills (180 ticks). Shown by `0x441d58(n)`.
//!
//! **Weapon** (the panel's left half, `+0x0..+0x194`, [`WeaponPanel`]): the back and outline sprites 1 and 2 at (10,
//! 10) (their frame = the ammo text's length − 3, 0..5), the item's icon (its row's `+0x2c`) 32 × 32 at (20, 15), the
//! ammo `"%d/%d"` (`0x1ae2b0`: ammo, most) at (64, 15) in scale 0.9. Each tick a held item has ammo (`0x2fa770`),
//! `0x443840(icon, 10)` keeps it up for 10 more ticks; its blends run forward from hidden and back once the timer runs
//! out (the wrench: 10 ticks after it comes out).
//!
//! **Markers** (`0x2db498` queues, `0x2db598` draws and clears; [`marker`], [`marker_quads`]): up to six a tick, each a
//! sprite of an FX texture (`0x2e8e68`), 40 × its size pixels, centred on its point's screen position (`0x2e46d8`) or
//! the screen's centre, its alpha at most `$gp−0x6670` (0x40). The Lancer's: its lock's green reticle (FX 0x26).
//!
//! Not ported: the weapon's experience bar (sprite 5 at (10, 18.5), `0x139868`), the low-ammo red pulse (the faders
//! `0x70202080` / `0x70242335` under a tenth of the most), the quick select ring, the XP bar (`+0xf4`), the vendor and
//! the other screens of the GUI object, PAL's sizes.

use crate::gui::{self, Quad, Tex};
use rc_formats::rac2::boot::Glyph;
use rc_formats::rac2::gui_sprite::Sprite;
use std::collections::HashMap;

/// A screen marker (`0x1bb000 + 0x30·i`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    /// `+0x00`: the world point (`+0x24` set), else the screen's centre.
    pub pos: Option<[f32; 3]>,
    /// `+0x10`: the size (× 40 pixels).
    pub size: f32,
    /// `+0x14`: the colour (GS RGBA, R low).
    pub colour: u32,
    /// `+0x18`: the FX texture.
    pub fx: u16,
    /// `+0x20`: the moby it belongs to (not drawn once deleted).
    pub moby: usize,
}

/// `$gp−0x6670`: the markers' most alpha.
const MARKER_ALPHA: u32 = 0x40;

/// `0x2db498(size, angle 0, step, moby, colour, pos, fx, −1)`: a marker queued for the HUD's draw, six at most. The FX
/// textures 0x1f..0x27 draw as one sprite centred on the point (mode 2), the Lancer's 0x26 among them.
pub fn marker(list: &mut Vec<Marker>, size: f32, moby: usize, colour: u32, pos: Option<[f32; 3]>, fx: u16) {
    if 6 <= list.len() { return; }
    let colour = if MARKER_ALPHA < colour >> 24 { colour & 0xff_ffff | MARKER_ALPHA << 24 } else { colour };
    list.push(Marker { pos, size, colour, fx, moby });
}

/// `0x2db598` → `0x2dab20` (mode 2) → `0x2e8e68`: each marker whose moby is alive (`alive`) as a quad, in pixels: the
/// point seen from the camera at `eye` (rows forward, left, up; half-angle tangent `tan`, y × 0.775) or the screen's
/// centre, 40 × size square, its texels 1..63 (`tex`: the FX texture's layer texture and texel size, 64 × 64 on the
/// GS). Behind the camera: not drawn.
pub fn marker_quads(markers: &[Marker], eye: [f32; 3], rows: [[f32; 3]; 3], tan: f32, tex: &dyn Fn(u16) -> Option<(Tex, [f32; 2])>, alive: &dyn Fn(usize) -> bool) -> Vec<Quad> {
    let mut out = Vec::new();
    let (cx, cy) = (gui::SCREEN_W * 0.5, gui::SCREEN_H * 0.5);
    for m in markers {
        if !alive(m.moby) { continue; }
        let Some((t, [tw, th])) = tex(m.fx) else { continue };
        let [x, y] = match m.pos {
            Some(p) => {
                let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
                let dot = |r: [f32; 3]| r[0] * d[0] + r[1] * d[1] + r[2] * d[2];
                let z = dot(rows[0]);
                if z <= 0.0 { continue; }
                [cx - dot(rows[1]) / z * (cx / tan), cy - dot(rows[2]) / z * (cy / (tan * 0.775))]
            }
            None => [cx, cy],
        };
        let h = m.size * 40.0 * 0.5;
        let (u0, u1, v0, v1) = (tw / 64.0, tw * 63.0 / 64.0, th / 64.0, th * 63.0 / 64.0);
        out.push(Quad { pos: [[x - h, y + h], [x + h, y + h], [x - h, y - h], [x + h, y - h]], uv: [[u0, v0], [u1, v0], [u0, v1], [u1, v1]], tex: t, rgba: m.colour.to_le_bytes() });
    }
    out
}

/// `0x445be8(t, p0, m0, m1, p1)`: the Hermite curve.
pub fn hermite(t: f32, p0: f32, m0: f32, m1: f32, p1: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    ((t3 + t3) - t2 * 3.0) * p0 + p0 + m0 * ((t3 - (t2 + t2)) + t) + m1 * (t3 - t2) + p1 * (t2 * 3.0 - (t3 + t3))
}

/// `0x2f0000(t, a, b)`: the colour bytes lerped.
pub fn lerp_rgba(t: f32, a: u32, b: u32) -> u32 {
    let mut out = 0u32;
    for k in 0..4 {
        let (x, y) = (((a >> (8 * k)) & 0xff) as f32, ((b >> (8 * k)) & 0xff) as f32);
        let v = (x * (1.0 - t) + y * t) as i32;
        out |= (v.clamp(0, 255) as u32) << (8 * k);
    }
    out
}

/// A two-key blend (`0x4406c8`).
#[derive(Clone, Copy, Debug)]
pub struct Blend {
    /// `+0x00` / `+0x04`: the size and position's tangents; `+0x08` / `+0x0c` the colour's.
    pub ease: [f32; 4],
    pub t: f32,
    pub dir: i32,
    pub speed: f32,
    pub active: bool,
    pub colour: [u32; 2],
    pub size: [[f32; 2]; 2],
    pub pos: [[f32; 2]; 2],
}

impl Blend {
    fn new(colour: [u32; 2], size: [[f32; 2]; 2], pos: [[f32; 2]; 2], ease: [f32; 4]) -> Blend {
        Blend { ease, t: 0.0, dir: 1, speed: 0.066_666_5, active: false, colour, size, pos }
    }
    /// `0x440810(b, 0)`: from the start, forward.
    fn forward(&mut self) { (self.t, self.dir, self.active) = (0.0, 1, true); }
    /// `0x440838(b, 0)`: from the end, back.
    fn back(&mut self) { (self.t, self.dir, self.active) = (1.0, -1, true); }
    /// `0x440878`: one tick, then the values at t.
    fn step(&mut self) {
        if !self.active { return; }
        self.t += self.speed * self.dir as f32;
        let out = if self.dir == 1 { 1.0 < self.t } else { self.t < 0.0 };
        self.t = self.t.clamp(0.0, 1.0);
        self.active = !out;
    }
    fn h(&self) -> f32 { hermite(self.t, 0.0, self.ease[0], self.ease[1], 1.0) }
    pub fn colour(&self) -> u32 { lerp_rgba(hermite(self.t, 0.0, self.ease[2], self.ease[3], 1.0), self.colour[0], self.colour[1]) }
    pub fn size(&self) -> [f32; 2] { let h = self.h(); [self.size[0][0] * (1.0 - h) + self.size[1][0] * h, self.size[0][1] * (1.0 - h) + self.size[1][1] * h] }
    pub fn pos(&self) -> [f32; 2] { let h = self.h(); [self.pos[0][0] * (1.0 - h) + self.pos[1][0] * h, self.pos[0][1] * (1.0 - h) + self.pos[1][1] * h] }
}

/// A keyed fader (`0x440228`): up to three keys `(time, value)` and their tangents.
#[derive(Clone, Copy, Debug)]
pub struct Fader {
    pub keys: [(f32, [f32; 2], u32); 3],
    pub count: usize,
    /// Per segment: the Hermite tangents (`0x440368`).
    pub tangents: [[f32; 2]; 2],
    pub t: f32,
    pub dir: i32,
    pub speed: f32,
    pub active: bool,
    /// The value is a colour (mode 0) or two floats (mode 2).
    pub colour_mode: bool,
}

impl Fader {
    fn colour(keys: [(f32, u32); 3], count: usize) -> Fader {
        Fader { keys: keys.map(|(t, c)| (t, [0.0; 2], c)), count, tangents: [[0.0, 0.0], [1.0, 1.0]], t: 0.0, dir: -1, speed: 0.116_666_6, active: false, colour_mode: true }
    }
    fn floats(keys: [(f32, [f32; 2]); 3], count: usize) -> Fader {
        Fader { keys: keys.map(|(t, v)| (t, v, 0)), count, tangents: [[0.0, 0.0], [1.0, 1.0]], t: 0.0, dir: -1, speed: 0.116_666_6, active: false, colour_mode: false }
    }
    /// `0x440388(f, dir)`.
    fn run(&mut self, dir: i32) {
        self.dir = dir;
        if !self.active {
            self.t = if dir == 1 { 0.0 } else { 1.0 };
            self.active = true;
        }
    }
    /// `0x4403d8(f, 1)`.
    fn step(&mut self) {
        if !self.active { return; }
        self.t += self.speed * self.dir as f32;
        let done = if self.dir == 1 { 1.0 < self.t } else { self.t < 0.0 };
        self.t = self.t.clamp(0.0, 1.0);
        if done { self.active = false; }
    }
    /// The value at t: the segment's lerp along its curve.
    fn sample(&self) -> ([f32; 2], u32) {
        let n = self.count.max(1);
        let mut k = 0;
        while k + 1 < n - 1 && self.keys[k + 1].0 <= self.t { k += 1; }
        if n == 1 { return (self.keys[0].1, self.keys[0].2); }
        let (a, b) = (self.keys[k], self.keys[k + 1]);
        let span = (b.0 - a.0).max(1e-6);
        let u = ((self.t - a.0) / span).clamp(0.0, 1.0);
        let tg = self.tangents[k.min(1)];
        let h = hermite(u, 0.0, tg[0], tg[1], 1.0);
        ([a.1[0] * (1.0 - h) + b.1[0] * h, a.1[1] * (1.0 - h) + b.1[1] * h], lerp_rgba(h, a.2, b.2))
    }
}

/// One draw of the HUD.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A GUI sprite (`0x381900`): id, position, scale, frame, colour (RGBA, R low, alpha 0x80 = 1).
    Sprite { id: u32, pos: [f32; 2], scale: [f32; 2], frame: f32, colour: u32 },
    /// A HUD icon (`0x2faa98` + `0x2faf68`): icon id and frame, top-left, size.
    Icon { id: u16, frame: u16, pos: [f32; 2], size: [f32; 2], colour: u32 },
    /// Text (`0x2ea068` / `0x2e9f68`): right-aligned at `pos` when `right`.
    Text { text: String, pos: [f32; 2], scale: f32, colour: u32, right: bool },
}

/// The bolt counter.
#[derive(Clone, Debug)]
pub struct BoltCounter {
    /// `+0x158c`: the show timer; `+0x1590`: hidden.
    pub timer: i32,
    pub hidden: bool,
    pub back: Blend,
    pub outline: Blend,
    pub text: Blend,
    pub icon: Blend,
}

impl Default for BoltCounter {
    fn default() -> Self {
        BoltCounter {
            timer: 0,
            hidden: true,
            back: Blend::new([0x0044_2d00, 0x6044_2d00], [[0.0, 0.0], [1.0, 1.0]], [[458.0, 25.0], [498.0, 10.0]], [1.0, 1.0, 0.0, 2.0]),
            outline: Blend::new([0x00f0_c070, 0x55f0_c070], [[1.4, 1.4], [1.0, 1.0]], [[515.0, 4.0], [498.0, 10.0]], [0.0, 3.0, 0.0, 3.0]),
            text: Blend::new([0x00f0_f0f0, 0x80f0_f0f0], [[0.9, 0.0], [0.9, 0.0]], [[440.0, 15.0], [440.0, 15.0]], [1.0, 1.0, 0.0, 3.0]),
            icon: Blend::new([0x00f0_f0b0, 0x60f0_f0b0], [[32.0, 32.0], [32.0, 32.0]], [[457.0, 15.0], [457.0, 15.0]], [1.0, 1.0, 0.0, 3.0]),
        }
    }
}

impl BoltCounter {
    /// `0x443b80(n)`.
    pub fn show(&mut self, n: i32) {
        self.timer = n;
        if self.hidden {
            self.hidden = false;
            for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.forward(); }
        }
    }

    /// `0x443c08`'s part: the blends, then the timer (the fade back once it runs out and the first blend is done).
    pub fn tick(&mut self) {
        for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.step(); }
        if self.timer != 0 {
            self.timer -= 1;
            if self.timer == 0 && !self.back.active {
                for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.back(); }
                self.hidden = true;
            }
        }
    }

    /// `0x444900`'s part.
    pub fn draw(&self, bolts: u32, separator: char, out: &mut Vec<Draw>) {
        let digits = bolts.to_string();
        let frame = (digits.len() as f32 - 1.0).clamp(0.0, 6.0);
        out.push(Draw::Sprite { id: 3, pos: self.back.pos(), scale: self.back.size(), frame, colour: self.back.colour() });
        out.push(Draw::Sprite { id: 4, pos: self.outline.pos(), scale: self.outline.size(), frame, colour: self.outline.colour() });
        // The separators every three digits from the right.
        let mut text = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i != 0 && (digits.len() - i) % 3 == 0 { text.push(separator); }
            text.push(c);
        }
        out.push(Draw::Text { text, pos: self.text.pos(), scale: self.text.size()[0], colour: self.text.colour(), right: true });
        out.push(Draw::Icon { id: 0x7567, frame: 0, pos: self.icon.pos(), size: self.icon.size(), colour: self.icon.colour() });
    }
}

/// The weapon half of the panel.
#[derive(Clone, Debug)]
pub struct WeaponPanel {
    /// `+0x1584`: the show timer; `+0x1588`: hidden.
    pub timer: i32,
    pub hidden: bool,
    /// `+0x157c` / `+0x1580`: the ammo and the most shown; the icon (`+0x158`).
    pub ammo: i32,
    pub max: i32,
    pub icon_id: u16,
    pub back: Blend,
    pub outline: Blend,
    pub text: Blend,
    pub icon: Blend,
}

impl Default for WeaponPanel {
    fn default() -> Self {
        WeaponPanel {
            timer: 0,
            hidden: true,
            ammo: 0,
            max: 0,
            icon_id: 0,
            back: Blend::new([0x0044_2d00, 0x6044_2d00], [[0.0, 0.0], [1.0, 1.0]], [[50.0, 25.0], [10.0, 10.0]], [1.0, 1.0, 0.0, 2.0]),
            outline: Blend::new([0x00f0_c070, 0x55f0_c070], [[1.2, 1.2], [1.0, 1.0]], [[0.0, 0.0], [10.0, 10.0]], [1.0, 1.0, 0.0, 3.0]),
            text: Blend::new([0x00f0_f0f0, 0x80f0_f0f0], [[0.9, 0.0], [0.9, 0.0]], [[64.0, 15.0], [64.0, 15.0]], [1.0, 1.0, 0.0, 3.0]),
            icon: Blend::new([0x00f0_f0b0, 0x60f0_f0b0], [[32.0, 32.0], [32.0, 32.0]], [[20.0, 15.0], [20.0, 15.0]], [1.0, 1.0, 0.0, 3.0]),
        }
    }
}

impl WeaponPanel {
    /// `0x443840(icon, n)`.
    pub fn show(&mut self, icon: u16, n: i32) {
        self.timer = n;
        if self.hidden {
            self.hidden = false;
            for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.forward(); }
        }
        self.icon_id = icon;
    }

    /// `0x443c08`'s part: the item with ammo held (icon, ammo, most) keeps it up; the blends; the timer.
    pub fn tick(&mut self, held: Option<(u16, i32, i32)>) {
        if let Some((icon, ammo, max)) = held {
            self.show(icon, 10);
            (self.ammo, self.max) = (ammo, max);
        }
        for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.step(); }
        if self.timer != 0 {
            self.timer -= 1;
            if self.timer == 0 && !self.back.active {
                for b in [&mut self.back, &mut self.outline, &mut self.text, &mut self.icon] { b.back(); }
                self.hidden = true;
            }
        }
    }

    /// `0x444900`'s part: the back, the outline, the icon, the text.
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let text = format!("{}/{}", self.ammo, self.max);
        let frame = (text.len() as f32 - 3.0).clamp(0.0, 5.0);
        out.push(Draw::Sprite { id: 1, pos: self.back.pos(), scale: self.back.size(), frame, colour: self.back.colour() });
        out.push(Draw::Sprite { id: 2, pos: self.outline.pos(), scale: self.outline.size(), frame, colour: self.outline.colour() });
        if self.icon_id != 0 { out.push(Draw::Icon { id: self.icon_id, frame: 0, pos: self.icon.pos(), size: self.icon.size(), colour: self.icon.colour() }); }
        out.push(Draw::Text { text, pos: self.text.pos(), scale: self.text.size()[0], colour: self.text.colour(), right: false });
    }
}

/// Ratchet's health.
#[derive(Clone, Debug)]
pub struct Health {
    /// `+0` / `+4`: health and its limit; `+0xc` shown; `+0x3f4` the show timer, `+0x3f8` the blink, `+0x3fc` the
    /// limit before it was raised, `+0x400` hidden.
    pub health: i32,
    pub max: i32,
    pub shown: bool,
    pub timer: i32,
    pub blink: i32,
    pub old_max: i32,
    pub hidden: bool,
    /// `+8`: the widget's position.
    pub pos: [f32; 2],
    /// `+0x1d4` / `+0x25c`: the back's colour and size; `+0x2e4` / `+0x36c` the outline's.
    pub back_colour: Fader,
    pub back_size: Fader,
    pub outline_colour: Fader,
    pub outline_size: Fader,
}

impl Health {
    pub fn new(max: i32) -> Health {
        let c = |a: u32, b: u32| Fader::colour([(0.0, a), (0.3, b), (1.0, b)], 3);
        let mut back_colour = c(0x0044_2d00, 0x6044_2d00);
        back_colour.tangents[1] = [0.0, 2.0];
        let mut outline_colour = c(0x00f0_c070, 0x55f0_c070);
        outline_colour.tangents[1] = [0.0, 3.0];
        Health {
            health: 0, max, shown: false, timer: 0, blink: 0, old_max: max, hidden: true, pos: [147.0, 10.0],
            back_colour,
            back_size: Fader::floats([(0.0, [0.0, 0.0]), (0.3, [1.0, 1.0]), (1.0, [1.0, 1.0])], 3),
            outline_colour,
            outline_size: Fader::floats([(0.0, [1.2, 1.2]), (0.3, [1.0, 1.0]), (1.0, [1.0, 1.0])], 3),
        }
    }

    /// `0x441d58(n)`.
    pub fn show(&mut self, n: i32) {
        self.timer = n;
        if self.hidden {
            self.hidden = false;
            self.shown = true;
            for f in [&mut self.back_colour, &mut self.back_size, &mut self.outline_colour, &mut self.outline_size] { f.run(1); }
        }
    }

    /// `0x441208(max)`: a changed limit blinks for 180 ticks and shows for 300.
    pub fn set_max(&mut self, max: i32) {
        if self.max != max {
            self.old_max = self.max;
            self.blink = 0xb4;
            self.timer = 300;
        }
        self.max = max;
    }

    /// `0x441968`.
    pub fn tick(&mut self) {
        for f in [&mut self.back_colour, &mut self.back_size, &mut self.outline_colour, &mut self.outline_size] { f.step(); }
        if self.timer != 0 {
            self.timer -= 1;
            if self.timer == 0 {
                self.shown = false;
                self.blink = 0;
                self.hidden = true;
                for f in [&mut self.back_colour, &mut self.back_size, &mut self.outline_colour, &mut self.outline_size] { f.run(-1); }
            }
        }
    }

    /// `0x441238(max)`: columns, rows, spacing, origin, the quadrant gap, the pill scale; and the back / outline
    /// sprites with their frame.
    fn layout(&self) -> Option<(i32, f32, [f32; 2], f32, f32, (u32, u32, f32))> {
        let m = self.max;
        let groups = ((m - 1).max(0) / 4) as f32;
        if (1..=20).contains(&m) {
            Some((6, 16.5, [184.0, 29.0], 0.0, 1.0, (0x11, 0x12, groups)))
        } else if (21..=40).contains(&m) {
            Some((10, 8.0, [178.0, 35.0], -0.5, 1.0, (0x13, 0x14, (groups - 5.0).max(0.0))))
        } else if (41..=80).contains(&m) {
            Some((10, 8.0, [179.0, 21.0], -0.5, 1.0, (0x15, 0x16, (groups - 10.0).max(0.0))))
        } else {
            None
        }
    }

    /// `0x441a68`.
    pub fn draw(&self, frame: i32, out: &mut Vec<Draw>) {
        if !self.shown { return; }
        let Some((cols, spacing, origin, gap, scale, (back, outline, sf))) = self.layout() else { return };
        let bc = self.back_colour.sample().1;
        out.push(Draw::Sprite { id: back, pos: self.pos, scale: self.back_size.sample().0, frame: sf, colour: bc });
        out.push(Draw::Sprite { id: outline, pos: self.pos, scale: self.outline_size.sample().0, frame: sf, colour: self.outline_colour.sample().1 });
        // `0x31dc30(−1, 0x22222222, 10)`: the blink between white and dark every 10 ticks.
        let blink_colour = if (frame / 10) % 2 == 0 { 0xffff_ffff } else { 0x2222_2222 };
        let small = self.max < 0x15;
        let (filled, empty) = if small { (0x1a, 0x19) } else { (0x1b, 0x18) };
        for i in 0..self.max {
            let g = i / 4;
            let q = i % 4;
            let mut x = (g % cols) as f32;
            let mut y = if g >= cols { 1.0 } else { 0.0 };
            x = (x * scale + x * scale) * spacing + origin[0];
            y = (y * scale + y * scale) * spacing + origin[1];
            let (mut sx, mut sy) = (scale, scale);
            match q {
                1 => { y += scale + gap; sx = -scale; }
                2 => { x += scale + gap; sy = -scale; }
                3 => { x += scale + gap; y += scale + gap; sx = -scale; sy = -scale; }
                _ => {}
            }
            if g >= cols { x += 2.0; }
            let blinking = self.blink != 0 && i >= self.old_max;
            if i < self.health {
                let c = if blinking { blink_colour } else { 0x80f0_c070 };
                out.push(Draw::Sprite { id: filled, pos: [x, y], scale: [sx, sy], frame: 0.0, colour: c });
            }
            out.push(Draw::Sprite { id: empty, pos: [x, y], scale: [sx, sy], frame: 0.0, colour: 0x55f0_c070 });
        }
    }
}

/// The weapon panel and the two counters.
#[derive(Clone, Debug)]
pub struct Hud {
    pub weapon: WeaponPanel,
    pub bolts: BoltCounter,
    pub health: Health,
    pub frame: i32,
}

impl Hud {
    pub fn new(max_health: i32) -> Hud { Hud { weapon: WeaponPanel::default(), bolts: BoltCounter::default(), health: Health::new(max_health), frame: 0 } }

    /// One tick (`0x443c08`): the requests of the tick (`0x30d910` / `0x30d9f8`), the values, the animators.
    pub fn tick(&mut self, health: i32, max_health: i32, show_bolts: Option<i32>, show_health: Option<i32>, weapon: Option<(u16, i32, i32)>) {
        self.frame += 1;
        self.weapon.tick(weapon);
        if let Some(n) = show_bolts { self.bolts.show(n); }
        if let Some(n) = show_health { self.health.show(n); }
        self.health.health = health;
        self.health.set_max(max_health);
        self.bolts.tick();
        self.health.tick();
        if self.health.blink != 0 { self.health.blink -= 1; }
    }

    pub fn draw(&self, bolts: u32) -> Vec<Draw> {
        let mut out = Vec::new();
        self.weapon.draw(&mut out);
        self.bolts.draw(bolts, '\'', &mut out);
        self.health.draw(self.frame, &mut out);
        out
    }
}

/// The draws as GUI quads (crate::gui): the sprites' shapes, the icons as textured quads (`icon` gives the layer's
/// texture of an icon frame and its texel size), the text in the font of texture slot 1.
pub fn quads(draws: &[Draw], sprites: &HashMap<u32, Sprite>, glyphs: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) -> Vec<Quad> {
    let mut out = Vec::new();
    let rgba = |c: u32| -> [u8; 4] { c.to_le_bytes() };
    for d in draws {
        match d {
            Draw::Sprite { id, pos, scale, frame, colour } => {
                if colour >> 24 == 0 { continue; }
                if let Some(s) = sprites.get(id) { gui::sprite(&mut out, s, *pos, *frame, *scale, rgba(*colour)); }
            }
            Draw::Icon { id, frame, pos, size, colour } => {
                if colour >> 24 == 0 { continue; }
                let Some((tex, [tw, th])) = icon(*id, *frame) else { continue };
                let [x, y] = *pos;
                let [w, h] = *size;
                out.push(Quad { pos: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], rgba: rgba(*colour), tex });
            }
            Draw::Text { text, pos, scale, colour, right } => {
                if colour >> 24 == 0 { continue; }
                let w = gui::text_width(glyphs, text.as_bytes(), *scale) as f32;
                let x = if *right { pos[0] - w } else { pos[0] };
                gui::text(&mut out, glyphs, 1, text.as_bytes(), x, pos[1], *scale, rgba(*colour));
            }
        }
    }
    out
}
