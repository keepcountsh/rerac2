//! The help panel (L00): the requests `0x2f5270` ([`GameState::help`]), the update `0x2f5680` and the draw
//! `0x2f5fb0`, the text of the level's help block (`0x1b1b00`, records `{text, id, voice, 0}`: R&C1's layout,
//! `rc_formats::strings`) laid out by the word wrap `0x2ea908`.
//!
//! **States** (`0x27d7c8`, its ticks `0x27d7cc`): 0 idle (a request opens it: `0x2f5528` measures the text — flags 7,
//! the box half its size plus 10 × 5 — and sets the shortest show, 7 ticks a character); 1 the opening; 3 the box
//! grows from 32 × 32 to its size over 8 ticks; 4 the text fades in over 4; 5 shown (until the shortest show is over);
//! 6 the text fades out over 4; 7 the box shrinks to 8 × 8 over 8 and fades. △ dismisses it from 1, 3, 4 and 5; a
//! dismissal and the end of 5 count in the slot (`0x18c958 + 0xc·slot`). The box: a dark blue rounded fill
//! (`0x2e9548`, `0x60442d00` at alpha 0x60) centred at x 256 and 60 above the bottom, the text in `0x80ffa888`.
//!
//! The panel shows from the first frame on a level (`0x27d7f8`; a level index below 0 would wait for a direction
//! and 120 ticks) with the help text on and the voice off (the options `0x1a7b9d` / `0x1a7b9c`).
//!
//! The border (`0x30d8d0` → `0x42f3b0`, `0x55f0c070`): the GUI sprites 0x57 down the sides and 0x58 along the top and
//! bottom (`0x444de0`), mirrored, their frame stretching them to the box's height and width.
//!
//! Not ported: the help voice (the dialogue player `0x1a63cc`, the voice table `0x14df60`), the prompt slot it holds
//! (`0x327ae0(5, 0, 1)`), the slot's last-shown time and level bits (`0x18c95c` / `0x18c960`), the help log
//! `0x2f6538`, the multi-page help (`0x27d800`), the shake offset `0x27d808`, PAL's height (0.9).

use crate::game::GameState;
use crate::gui::{self, Quad, Tex};
use rc_formats::rac2::boot::Glyph;
use rc_formats::rac2::gui_sprite::Sprite;
use rc_formats::strings::Message;
use std::collections::HashMap;

/// The screen height (`0x1a7344`).
const SCREEN_H: i32 = 416;
const BOX_RGBA: u32 = 0x0044_2d00;
const TEXT_RGB: u32 = 0x00ff_a888;
const BORDER_RGB: u32 = 0x00f0_c070;

/// `0x2eb050`'s layout (the words `+0..+0x16`): the lines drawn between `top` and `bottom`, the text between `x0` and
/// `x1` anchored at `x` (flags bit 0: centred), at `y` (bit 1: centred on it), `line` apart; bit 2 measures only.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub top: i32,
    pub bottom: i32,
    pub x0: i32,
    pub x1: i32,
    pub x: i32,
    pub y: i32,
    pub line: i32,
    pub flags: u16,
}

fn adv(glyphs: &[Glyph; 256], c: u8) -> i32 { glyphs[c as usize].advance as i32 }

/// `0x2ea908`: the lines `(start, end)` (inclusive) of `text` — each as long as fits, broken at a space or a code
/// below 0x10; then, while the last line is short (under a third of the width), the width narrows by 16 to even
/// the lines, back to the full width if that would add a line. Returns the lines.
pub fn wrap(text: &[u8], l: &Layout, glyphs: &[Glyph; 256]) -> Vec<(usize, usize)> {
    let at = |i: usize| text.get(i).copied().unwrap_or(0);
    let full = if l.flags & 1 != 0 { 2 * (l.x1 - l.x).min(l.x - l.x0) } else { l.x1 - l.x };
    let (mut fin, mut first, mut avail, mut last_w) = (false, 0usize, full, 0i32);
    let mut lines: Vec<(usize, usize)>;
    loop {
        lines = Vec::new();
        let mut pos = 0usize;
        let mut w_end = last_w;
        if at(0) != 0 {
            loop {
                let start = pos;
                let mut w = 0;
                let mut brk = pos;
                if 0 < avail {
                    loop {
                        let c = at(pos);
                        if c == b' ' || c < 0x10 { brk = pos; }
                        if c < 2 { break; }
                        pos += 1;
                        w += adv(glyphs, c);
                        if avail <= w { break; }
                    }
                }
                let mut end = if brk == start { pos } else { brk };
                let probe = end;
                if at(probe) == b' ' || at(probe) < 0x10 { end = end.wrapping_sub(1); }
                lines.push((start, end));
                w_end = w;
                if at(probe) == 0 { break; }
                pos = probe + 1;
                w_end = last_w;
                if at(pos) == 0 || 0x4e < lines.len() { break; }
            }
        }
        if fin { break; }
        if first == 0 { first = lines.len(); }
        if lines.len() < 2 { break; }
        last_w = w_end;
        if first < lines.len() {
            fin = true;
            avail = full;
            continue;
        }
        let third = avail / 3;
        avail -= 16;
        if third <= w_end { break; }
    }
    lines
}

/// The size `(width, height)` of the laid-out text, and its draw into `out` unless the layout measures only.
pub fn text_box(out: &mut Vec<Quad>, text: &[u8], l: &Layout, glyphs: &[Glyph; 256], rgba: [u8; 4]) -> (i32, i32) {
    let lines = wrap(text, l, glyphs);
    let mut y = l.y;
    if l.flags & 2 != 0 { y -= lines.len() as i32 * l.line >> 1; }
    let mut width = 0;
    for &(s, e) in &lines {
        if l.top <= y + l.line && y <= l.bottom && e != usize::MAX && s <= e {
            let part = &text[s..(e + 1).min(text.len())];
            let w: i32 = part.iter().map(|&c| adv(glyphs, c)).sum();
            width = width.max(w);
            if l.flags & 4 == 0 {
                let x = if l.flags & 1 != 0 { l.x - (w >> 1) } else { l.x };
                gui::text(out, glyphs, 1, part, x as f32, y as f32, 1.0, rgba);
            }
        }
        y += l.line;
    }
    (width, lines.len() as i32 * l.line)
}

/// `0x2e9548(y0, y1, x0, x1, alpha, colour)`: a filled box with its left and right edges rounded (rows of 2, 1 and 1
/// pixels, 1, 2 and 4 in from the ends).
fn rounded_box(out: &mut Vec<Quad>, y0: i32, y1: i32, x0: i32, x1: i32, alpha: i32, colour: u32) {
    let rgba = gui::rgba(((alpha as u32) << 24) | colour);
    let mut rect = |y0: i32, y1: i32, x0: i32, x1: i32| {
        let (y0, y1, x0, x1) = (y0 as f32, y1 as f32, x0 as f32, x1 as f32);
        out.push(Quad { pos: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]], uv: [[0.0; 2]; 4], tex: Tex::None, rgba });
    };
    rect(y0, y1, x0, x1);
    rect(y0 + 1, y1 - 1, x0 - 2, x0);
    rect(y0 + 2, y1 - 2, x0 - 3, x0 - 2);
    rect(y0 + 4, y1 - 4, x0 - 4, x0 - 3);
    rect(y0 + 1, y1 - 1, x1, x1 + 2);
    rect(y0 + 2, y1 - 2, x1 + 2, x1 + 3);
    rect(y0 + 4, y1 - 4, x1 + 3, x1 + 4);
}

/// `0x42f3b0(y0, y1, x0, x1, gui, alpha, colour)`: the border — the side piece 0x57 at the left (mirrored) and
/// right edges, its frame `((y1 − y0) − 26) / 112 · 0.311 + 0.0375`; the top and bottom piece 0x58 16 in from both
/// corners, mirrored to the four, its frame `((x1 − x0) − 150) / 306 · 0.433 + 0.102`.
fn border(out: &mut Vec<Quad>, sprites: &HashMap<u32, Sprite>, y0: i32, y1: i32, x0: i32, x1: i32, alpha: i32, colour: u32) {
    let rgba = gui::rgba(((alpha as u32) << 24) | colour);
    let (y0, y1, x0, x1) = (y0 as f32, y1 as f32, x0 as f32, x1 as f32);
    let fy = ((y1 - y0) - 26.0) * 0.008_928_572 * 0.311 + 0.0375;
    let fx = ((x1 - x0) - 150.0) * 0.003_267_974 * 0.433 + 0.102;
    if let Some(s) = sprites.get(&0x57) {
        gui::sprite(out, s, [x0, y0], fy, [-1.0, 1.0], rgba);
        gui::sprite(out, s, [x1, y0], fy, [1.0, 1.0], rgba);
    }
    if let Some(s) = sprites.get(&0x58) {
        gui::sprite(out, s, [x0 + 16.0, y0], fx, [-1.0, 1.0], rgba);
        gui::sprite(out, s, [x1 - 16.0, y0], fx, [1.0, 1.0], rgba);
        gui::sprite(out, s, [x0 + 16.0, y1], fx, [-1.0, -1.0], rgba);
        gui::sprite(out, s, [x1 - 16.0, y1], fx, [1.0, -1.0], rgba);
    }
}

/// The panel (`0x27d7c8..0x27d810`; its state is [`GameState::help_state`]).
#[derive(Clone, Debug, Default)]
pub struct Panel {
    /// `0x27d7cc`: the state's ticks.
    pub timer: i32,
    /// `0x27d7e8` / `0x27d7f0`: the text's index in the block and its slot.
    pub index: Option<usize>,
    pub slot: i32,
    /// `0x27d7d0` / `0x27d7d4`: the box's half width and height; `0x27d7d8` / `0x27d7dc` its centre;
    /// `0x27d7e0` / `0x27d7e4` the half sizes drawn.
    pub half: [i32; 2],
    pub centre: [i32; 2],
    pub drawn: [i32; 2],
    /// `0x27d810`: the shortest show (7 ticks a character).
    pub min_ticks: i32,
    pub measured: bool,
}

impl Panel {
    fn layout(y: i32, flags: u16) -> Layout { Layout { top: 0xf0, bottom: 0x1e0, x0: 0x2c, x1: 0x1d4, x: 0x100, y, line: 0x10, flags } }

    fn count(g: &mut GameState, slot: i32) {
        if let Some(c) = g.help_counts.get_mut(slot.max(0) as usize) {
            if *c != -1 { *c += 1; }
        }
    }

    /// `0x2f5528`: open on the text `index` (the measure waits for the draw, which has the font).
    fn open(&mut self, texts: &[Message], g: &mut GameState) {
        g.help_state = 1;
        self.timer = 0;
        let text = self.index.and_then(|i| texts.get(i)).map_or(&[][..], |m| &m.text[..]);
        self.drawn = [8, 8];
        self.min_ticks = text.len() as i32 * 7;
        self.measured = false;
    }

    /// `0x2f5528`'s measure: the box half the text's size plus 10 × 5, centred at x 256 and 60 above the bottom (12
    /// at least).
    fn measure(&mut self, texts: &[Message], glyphs: &[Glyph; 256]) {
        let text = self.index.and_then(|i| texts.get(i)).map_or(&[][..], |m| &m.text[..]);
        let (w, h) = text_box(&mut Vec::new(), text, &Self::layout(0x168, 7), glyphs, [0; 4]);
        self.half = [(w >> 1) + 10, (h >> 1) + 5];
        self.centre = [0x100, SCREEN_H - 0x3c];
        if SCREEN_H - 0xc < self.centre[1] + self.half[1] { self.centre[1] = SCREEN_H - (self.half[1] + 0xc); }
        self.measured = true;
    }

    /// `0x2f5680`, once a frame (`triangle`: △ pressed this frame).
    pub fn tick(&mut self, g: &mut GameState, triangle: bool, texts: &[Message]) {
        self.timer += 1;
        match g.help_state {
            0 => {
                if let Some((text, slot)) = g.help_req.take() {
                    self.index = rc_formats::strings::find_index(texts, text);
                    self.slot = slot;
                    if self.index.is_some() { self.open(texts, g); }
                }
            }
            1 | 2 => {
                if triangle {
                    Self::count(g, self.slot);
                    g.help_state = 7;
                    self.timer = 0;
                } else if 1 <= self.timer {
                    g.help_state = 3;
                    self.timer = 0;
                }
            }
            3 => {
                if triangle {
                    Self::count(g, self.slot);
                    g.help_state = 7;
                    self.timer = 0;
                } else if 8 <= self.timer {
                    g.help_state = 4;
                    self.timer = 0;
                }
            }
            4 => {
                if triangle {
                    Self::count(g, self.slot);
                    g.help_state = 6;
                    self.timer = 4 - self.timer;
                } else if 4 <= self.timer {
                    g.help_state = 5;
                    self.timer = 0;
                }
            }
            5 => {
                if self.min_ticks <= self.timer || triangle {
                    Self::count(g, self.slot);
                    g.help_state = 6;
                    self.timer = 0;
                }
            }
            6 => {
                if 4 <= self.timer || triangle {
                    g.help_state = 7;
                    self.timer = 0;
                }
            }
            7 => {
                if 7 < self.timer {
                    g.help_state = 0;
                    self.index = None;
                    self.timer = 0;
                }
            }
            _ => {}
        }
    }

    /// `0x2f5fb0`: the box and the text as GUI quads.
    pub fn draw(&mut self, g: &GameState, texts: &[Message], glyphs: &[Glyph; 256], sprites: &HashMap<u32, Sprite>) -> Vec<Quad> {
        let mut out = Vec::new();
        if g.help_state != 0 && !self.measured { self.measure(texts, glyphs); }
        let [x, y] = self.centre;
        let t8 = self.timer.clamp(0, 8);
        match g.help_state {
            1 => {
                self.drawn = [8, 8];
                rounded_box(&mut out, y - 8, y + 8, x - 8, x + 8, 0x60, BOX_RGBA);
                border(&mut out, sprites, y - 8, y + 8, x - 8, x + 8, 0x60, BORDER_RGB);
            }
            3 => {
                self.drawn = [(self.half[0] - 0x20) * t8 / 8 + 0x20, (self.half[1] - 0x20) * t8 / 8 + 0x20];
                let [dx, dy] = self.drawn;
                rounded_box(&mut out, y - dy, y + dy, x - dx, x + dx, 0x60, BOX_RGBA);
                border(&mut out, sprites, y - dy, y + dy, x - dx, x + dx, 0x60, BORDER_RGB);
            }
            4..=6 => {
                self.drawn = self.half;
                let [dx, dy] = self.half;
                rounded_box(&mut out, y - dy, y + dy, x - dx, x + dx, 0x60, BOX_RGBA);
                border(&mut out, sprites, y - dy, y + dy, x - dx, x + dx, 0x60, BORDER_RGB);
                let t4 = self.timer.clamp(0, 4);
                let alpha = match g.help_state { 4 => t4 * 0x20, 6 => (4 - t4) * 0x20, _ => 0x80 };
                let text = self.index.and_then(|i| texts.get(i)).map_or(&[][..], |m| &m.text[..]);
                text_box(&mut out, text, &Self::layout(y, 3), glyphs, gui::rgba(((alpha as u32) << 24) | TEXT_RGB));
            }
            7 => {
                let [dx, dy] = self.drawn;
                let (dx, dy) = (dx - (dx - 8) * t8 / 8, dy - (dy - 8) * t8 / 8);
                rounded_box(&mut out, y - dy, y + dy, x - dx, x + dx, (8 - t8) * 0xc, BOX_RGBA);
                border(&mut out, sprites, y - dy, y + dy, x - dx, x + dx, (8 - t8) * 0xc, BORDER_RGB);
            }
            _ => {}
        }
        out
    }
}
