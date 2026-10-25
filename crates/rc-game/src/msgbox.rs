//! The HUD screen's message boxes (L00, the GUI object `0x1a9054` + `0x36f28`: the first at `+0x1d8c`, the second
//! at `+0x1fdc`; built by `0x42eac8`, updated by `0x42faa0` → `0x42f758`, drawn by `0x42ffb0` → `0x42fb18`, each
//! after the HUD panel's own): a line of text in a dark box with the gold border, which fades in and out.
//!
//! **Requests.** `0x327478(text, n, ticks)` → `0x42f608`: the text (by id, `0x2f5460`) formatted with `n`
//! (`0x115da8`) in the first box at (255, 135), 120 ticks unless given; `0x327448(text, ticks)` → `0x42f690`: the text
//! as it is, at (255, 123), 180 ticks; `0x30eff8(text, ticks)` → `0x42ef28`: the second box at (255, 80), its lines
//! broken at code 1 (`0x42ed58`), 360 ticks.
//!
//! **Update** (`0x42f758`): while its timer runs the alpha rises 8 a tick to 0x60, then falls 8 a tick; at 0 the box
//! is hidden. The alpha goes into the box's fill (`0x60442d00`), the text (`0x80f0f0f0`) and the border
//! (`0x55f0c070`). Text wider than 469 drops to a smaller style (2 → 1 → 0: scale 1, 0.8, 0.7) until it fits. The
//! style's margins (0: 5 × 5, 1: 6 × 5, 2: 8 × 6) and the text's width and lines (16 apart) give the box's half
//! size; the border's frames stretch to it: the corners (sprite 0x58) by `((w − 40) / 202 · 0.5675 + 0.005)`, the
//! sides (0x57) by `((h − 14) / 152 · 0.85 + 0.03)`, the bottom corners `(h − 14) / 152 · 306 + 23.5` under the top.
//!
//! **Draw** (`0x42fb18`): the fill over the box (`0x2e94a8`), the sides at its left (mirrored) and right edges 2
//! down, the corners 15 in from them, mirrored to the four, the lines from its top left plus the margin (6 down).
//!
//! Not ported: the boxes' other kind (`+0x1c8` = 2: `0x42fa98` / `0x42ffa8`, both empty), the debug frame
//! overrides (`$gp−0x1104` / `−0x1100`, −1).

use crate::gui::{self, Quad};
use rc_formats::disc::boot::Glyph;
use rc_formats::disc::gui_sprite::Sprite;
use std::collections::HashMap;

/// `0x42f700`: the style's text scale.
fn scale(style: i32) -> f32 {
    match style {
        1 => 0.8,
        0 => 0.7,
        _ => 1.0,
    }
}

/// `0x1adf08 + 8·style`: the text's offset from the box's top left plus the margin.
const TEXT_AT: [[i32; 2]; 4] = [[0, 6], [0, 6], [0, 6], [0, 0]];
/// `0x1adef8` / `0x1adf00`: the sides' and the corners' offsets from the box's corners.
const SIDE_AT: [f32; 2] = [0.0, -2.0];
const CORNER_AT: [f32; 2] = [-15.0, -2.0];

/// One box (`0x1f0` bytes).
#[derive(Clone, Debug)]
pub struct MessageBox {
    /// `[0]`: its centre; `[1]`: shown (1) or hidden (0).
    pub pos: [f32; 2],
    pub shown: bool,
    /// `[2]` the fill, `[3]` the text, the border's (`+0x10` / `+0x5c` `+0xc`): colours, alpha high.
    pub fill: u32,
    pub ink: u32,
    pub border: u32,
    /// `+0x10` / `+0x5c`: the sides' and the corners' frames.
    pub side_frame: f32,
    pub corner_frame: f32,
    /// `+0xa8`: the text (0xff at most), and its lines (`+0x1d8`, `+0x1b0` of them).
    pub text: Vec<u8>,
    pub lines: Vec<Vec<u8>>,
    /// `+0x1a8` / `+0x1ac`: the half size; `+0x1bc` / `+0x1c0` the margins; `+0x1c4` the style; `+0x1d0` the bottom
    /// corners under the top; `+0x1d4` the text's width.
    pub half: [i32; 2],
    pub margin: [i32; 2],
    pub style: i32,
    pub drop: f32,
    pub width: i32,
    /// `+0x1b8`: the ticks left to show.
    pub timer: i32,
}

impl Default for MessageBox {
    /// `0x42eac8`.
    fn default() -> Self {
        MessageBox {
            pos: [255.0, 108.0],
            shown: false,
            fill: 0x6044_2d00,
            ink: 0x80f0_f0f0,
            border: 0x55f0_c070,
            side_frame: 0.02,
            corner_frame: 0.0,
            text: Vec::new(),
            lines: Vec::new(),
            half: [0, 0],
            margin: [0, 0],
            style: 2,
            drop: 0.0,
            width: 0x28,
            timer: 0,
        }
    }
}

/// `0x115da8(out, format, n)`: the format's `%d` / `%i` (and `%%`) filled.
fn format(fmt: &[u8], n: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c == b'%' && i + 1 < fmt.len() {
            match fmt[i + 1] {
                b'd' | b'i' | b'u' => {
                    out.extend_from_slice(n.to_string().as_bytes());
                    i += 2;
                    continue;
                }
                b'%' => {
                    out.push(b'%');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn with_alpha(c: u32, a: u32) -> u32 { (c & 0x00ff_ffff) | a << 24 }

impl MessageBox {
    /// `0x42ece8(x, y, box, style, ticks)`: one line.
    fn show(&mut self, pos: [f32; 2], style: i32, ticks: i32, glyphs: &[Glyph; 256]) {
        self.style = style;
        self.pos = pos;
        self.text.truncate(0xff);
        self.timer = ticks;
        self.lines = vec![self.text.clone()];
        self.width = gui::text_width(glyphs, &self.text, 1.0);
        self.shown = true;
    }

    /// `0x42f608(screen, text, n, ticks)`: `fmt` formatted with `n`, 120 ticks unless given.
    pub fn show_formatted(&mut self, fmt: &[u8], n: i32, ticks: Option<i32>, glyphs: &[Glyph; 256]) {
        self.text = format(fmt, n);
        self.show([255.0, 135.0], 2, ticks.unwrap_or(0x78), glyphs);
    }

    /// `0x42f690(screen, text, ticks)` → `0x42ed58` at (255, 123): 180 ticks unless given.
    pub fn show_text(&mut self, text: &[u8], ticks: Option<i32>, glyphs: &[Glyph; 256]) {
        self.text = text.iter().copied().take(0xfe).collect();
        self.show_lines([255.0, 123.0], 2, ticks.unwrap_or(0xb4), glyphs);
    }

    /// `0x42ef28(screen, text, ticks)` (the second box): 360 ticks unless given.
    pub fn show_second(&mut self, text: &[u8], ticks: Option<i32>, glyphs: &[Glyph; 256]) {
        self.text = text.iter().copied().take(0xff).collect();
        self.show_lines([255.0, 80.0], 2, ticks.unwrap_or(0x168), glyphs);
    }

    /// `0x42ed58(x, y, box, style, ticks)`: the text's lines broken at code 1; the width the widest's.
    fn show_lines(&mut self, pos: [f32; 2], style: i32, ticks: i32, glyphs: &[Glyph; 256]) {
        self.style = style;
        self.pos = pos;
        self.timer = ticks;
        let end = self.text.iter().position(|&c| c == 0).unwrap_or(self.text.len());
        self.lines = self.text[..end].split(|&c| c == 1).map(<[u8]>::to_vec).collect();
        self.width = self.lines.iter().map(|l| gui::text_width(glyphs, l, 1.0)).max().unwrap_or(0);
        self.shown = true;
    }

    /// `0x42faa0` → `0x42f758`, once a tick.
    pub fn tick(&mut self, glyphs: &[Glyph; 256]) {
        if !self.shown { return; }
        let mut a = (self.fill >> 24) as i32;
        if 0 < self.timer {
            a = (a + 8).min(0x60);
            self.timer -= 1;
        } else {
            a = (a - 8).max(0);
        }
        if a == 0 {
            self.shown = false;
            return;
        }
        let a = a as u32;
        self.fill = with_alpha(self.fill, a);
        self.ink = with_alpha(self.ink, a);
        self.border = with_alpha(self.border, a);
        // Too wide: the next smaller style.
        while 0x1d6 <= self.width && self.style != 0 {
            self.style -= 1;
            if !self.lines.is_empty() {
                let s = scale(self.style);
                self.width = self.lines.iter().map(|l| gui::text_width(glyphs, l, s)).max().unwrap_or(0);
            }
        }
        self.margin = match self.style {
            0 => [5, 5],
            1 => [6, 5],
            2 => [8, 6],
            _ => self.margin,
        };
        if self.style == 3 { return; }
        self.half = [(self.width >> 1) + self.margin[0], ((self.lines.len() as i32 * 16) >> 1) + self.margin[1]];
        self.corner_frame = (self.half[0] as f32 - 40.0) * 0.004_950_495 * 0.5675 + 0.005;
        let fy = (self.half[1] as f32 - 14.0) * 0.006_578_947_4;
        self.side_frame = fy * 0.85 + 0.03;
        self.drop = fy * 306.0 + 23.5;
    }

    /// `0x42ffb0` → `0x42fb18`: its GUI quads.
    pub fn draw(&self, out: &mut Vec<Quad>, glyphs: &[Glyph; 256], sprites: &HashMap<u32, Sprite>) {
        if !self.shown { return; }
        let [x, y] = self.pos;
        let (w, h) = (self.half[0] as f32, self.half[1] as f32);
        gui::rect(out, (y - h) as i32, (y + h) as i32, (x - w) as i32, (x + w) as i32, gui::rgba(self.fill));
        let border = gui::rgba(self.border);
        let top = y - h;
        if let Some(s) = sprites.get(&0x57) {
            gui::sprite(out, s, [x - w - SIDE_AT[0], top - SIDE_AT[1]], self.side_frame, [-1.0, 1.0], border);
            gui::sprite(out, s, [x + w + SIDE_AT[0] + 1.0, top - SIDE_AT[1]], self.side_frame, [1.0, 1.0], border);
        }
        if let Some(s) = sprites.get(&0x58) {
            let (l, r, t) = (x - w - CORNER_AT[0] + 1.0, x + w + CORNER_AT[0], top - CORNER_AT[1]);
            gui::sprite(out, s, [r, t], self.corner_frame, [1.0, 1.0], border);
            gui::sprite(out, s, [l, t], self.corner_frame, [-1.0, 1.0], border);
            gui::sprite(out, s, [r, t + self.drop], self.corner_frame, [1.0, -1.0], border);
            gui::sprite(out, s, [l, t + self.drop], self.corner_frame, [-1.0, -1.0], border);
        }
        let style = self.style.clamp(0, 3) as usize;
        let tx = ((x - w) + self.margin[0] as f32) as i32 + TEXT_AT[style][0];
        let ink = gui::rgba(self.ink);
        for (i, line) in self.lines.iter().enumerate() {
            let ty = (y - h) as i32 + TEXT_AT[style][1] + i as i32 * 16;
            gui::text(out, glyphs, 1, line, tx as f32, ty as f32, scale(self.style), ink);
        }
    }
}

/// The HUD screen's two boxes.
#[derive(Clone, Debug, Default)]
pub struct Boxes {
    pub first: MessageBox,
    pub second: MessageBox,
}

/// A message asked for this tick (the requests run with the font, after the tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// `0x327478(text, n, ticks)`.
    Formatted { text: i32, n: i32, ticks: Option<i32> },
    /// `0x327448(text, ticks)`.
    Text { text: i32, ticks: Option<i32> },
    /// `0x30eff8(text, ticks)` with the text by id.
    Second { text: i32, ticks: Option<i32> },
    /// The text by id formatted with `n` (`0x115da8`) into the second box (L01 `0x325830`: its `0x2ed8b8` →
    /// `0x3049d0` = L00 `0x30eff8`).
    SecondFormatted { text: i32, n: i32, ticks: Option<i32> },
}

impl Boxes {
    /// The requests (each text by id from the level's table, `0x2f5460`; a missing one is empty), then
    /// `0x4451e8`'s part: the first box, then the second.
    pub fn tick(&mut self, requests: &mut Vec<Request>, texts: &[rc_formats::strings::Message], glyphs: &[Glyph; 256]) {
        let text = |id: i32| rc_formats::strings::find_index(texts, id).map_or(&[][..], |i| &texts[i].text[..]);
        for r in requests.drain(..) {
            match r {
                Request::Formatted { text: t, n, ticks } => self.first.show_formatted(text(t), n, ticks, glyphs),
                Request::Text { text: t, ticks } => self.first.show_text(text(t), ticks, glyphs),
                Request::Second { text: t, ticks } => self.second.show_second(text(t), ticks, glyphs),
                Request::SecondFormatted { text: t, n, ticks } => self.second.show_second(&format(text(t), n), ticks, glyphs),
            }
        }
        self.first.tick(glyphs);
        self.second.tick(glyphs);
    }

    pub fn draw(&self, out: &mut Vec<Quad>, glyphs: &[Glyph; 256], sprites: &HashMap<u32, Sprite>) {
        self.first.draw(out, glyphs, sprites);
        self.second.draw(out, glyphs, sprites);
    }
}
