//! The front end's 2D draws as screen quads (512×416 pixels, strip order v0 v1 v2 v3: triangles 012 and 123), in
//! draw order: the GUI sprites (`FUN_003018a0`), the font (`FUN_0027fcb0`, `FUN_0027f920`) and full-screen pictures
//! and fades. The engine draws the list as the GS would (alpha 0x80 = 1.0, `(Cs − Cd)·As/128 + Cd`).

use rc_formats::disc::boot::Glyph;
use rc_formats::disc::gui_sprite::{BoneKey, Prim, Sprite};

/// The screen (`FUN_0027baa0`, NTSC: `FUN_00284c18(0x200, 0x1a0, …)`).
pub const SCREEN_W: f32 = 512.0;
pub const SCREEN_H: f32 = 416.0;

/// The GUI projection (L25 `0x390ae0`): pixels per sprite unit × 16 are half the screen over the current view's
/// tangent `0x1bd730` across, and half its height over `0x1bd734` × 0.75 (`0x1a7bb9` set) up — the x tangent × 0.775.
/// Read live: 0.62 on the pause menu (game mode 3, [`MENU_TAN`]), 0.6299774 in the Wupash fight ([`PLAY_TAN`]).
/// Only the space fight's HUD takes [`PLAY_TAN`] so far; the rest (the on-foot HUD, the help and message boxes) keep
/// [`MENU_TAN`] until their own views are read (docs/plan/gaps.md G-REN-016).
pub const MENU_TAN: f32 = 0.62;
pub const PLAY_TAN: f32 = 0.629_977_4;
fn scales(tan: f32) -> (f32, f32) { (SCREEN_W * 0.5 / tan, SCREEN_H * 0.5 / (tan * 0.775)) }

/// What a quad samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    None,
    /// The front end's texture set, slot n (1, 2: the fonts).
    Slot(usize),
    /// A full-screen picture.
    Picture(Picture),
}

/// The pictures the front end puts in the frame buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    /// `rc_formats::disc::front_end::SPLASHES[i]`.
    Splash(usize),
    /// The menu backdrop.
    Backdrop,
    /// The movie's current picture.
    Movie,
    /// The pause page's item pictures (the panel `0x3548f0`'s, 256 × 256): two slots.
    Item(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Pixels; a corner at x covers pixels x.. like the GS's `x·16 + OF − 8` (crate docs).
    pub pos: [[f32; 2]; 4],
    /// Texels.
    pub uv: [[f32; 2]; 4],
    pub tex: Tex,
    /// GS RGBA (alpha 0x80 = 1.0); a textured quad modulates: `C = Ct·Cf >> 7`.
    pub rgba: [u8; 4],
}

/// A picture over the whole screen at its own size (texel = pixel), unmodulated (`0x80808080`).
pub fn picture(out: &mut Vec<Quad>, p: Picture) {
    let (w, h) = (SCREEN_W, SCREEN_H);
    out.push(Quad { pos: [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]], uv: [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]], tex: Tex::Picture(p), rgba: [0x80; 4] });
}

/// `FUN_0027e440(r, g, b, a)`: a full-screen untextured quad.
pub fn fill(out: &mut Vec<Quad>, rgba: [u8; 4]) {
    let (w, h) = (SCREEN_W, SCREEN_H);
    out.push(Quad { pos: [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]], uv: [[0.0; 2]; 4], tex: Tex::None, rgba });
}

/// A GS colour word (`0xAABBGGRR`) as bytes.
pub const fn rgba(w: u32) -> [u8; 4] { w.to_le_bytes() }

/// `FUN_002846e8(t, a, b)`: `a·(1 − t) + b·t` per byte, truncated.
pub fn lerp_colour(t: f32, a: u32, b: u32) -> [u8; 4] {
    let (a, b) = (rgba(a), rgba(b));
    std::array::from_fn(|i| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t) as i32 as u8)
}

/// A bone matrix: rows x, y, z and the translation (row-vector convention: `v' = v.x·r0 + v.y·r1 + v.z·r2 + t`).
type Mat = [[f32; 3]; 4];

fn apply(m: &Mat, v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + m[3][j]) }

/// `FUN_00284408`: rotation (quaternion) × scale, then the translation.
fn key_matrix(k: &BoneKey) -> Mat {
    let [x, y, z, w] = k.rotation;
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let s = k.scale;
    [r[0].map(|c| c * s[0]), r[1].map(|c| c * s[1]), r[2].map(|c| c * s[2]), k.translation]
}

/// `FUN_003015d8`: the bones at `frame` (keys `⌊frame⌋` and the next, wrapping, blended linearly when the fraction is
/// above 0.001), each under its parent.
pub fn pose(s: &Sprite, frame: f32) -> Vec<Mat> {
    let n = s.frames.len();
    if n == 0 { return Vec::new(); }
    let i = frame.max(0.0) as usize % n;
    let t = frame - frame.floor();
    let (a, b) = (&s.frames[i], &s.frames[if i + 1 >= n { 0 } else { i + 1 }]);
    let mut out: Vec<Mat> = Vec::with_capacity(a.len());
    for (k, ka) in a.iter().enumerate() {
        let key = if t > 0.001 {
            let kb = &b[k];
            let l = |p: f32, q: f32| p * (1.0 - t) + q * t;
            BoneKey {
                rotation: std::array::from_fn(|j| l(ka.rotation[j], kb.rotation[j])),
                scale: std::array::from_fn(|j| l(ka.scale[j], kb.scale[j])),
                translation: std::array::from_fn(|j| l(ka.translation[j], kb.translation[j])),
                parent: ka.parent,
            }
        } else {
            *ka
        };
        let m = key_matrix(&key);
        out.push(match key.parent.and_then(|p| out.get(p)) {
            Some(p) => [apply(&[p[0], p[1], p[2], [0.0; 3]], m[0]), apply(&[p[0], p[1], p[2], [0.0; 3]], m[1]), apply(&[p[0], p[1], p[2], [0.0; 3]], m[2]), apply(p, m[3])],
            None => m,
        });
    }
    out
}

/// `FUN_003017f8(x, y, sx, sy, frame, sprite, colour)`: the sprite's quads. A vertex lands at
/// `pos + (X·sx·scale·x, −Y·sy·scale·z) / 16` (X, Y: the projection's [`scales`] by the view's tangent; here the menus') (the GS coordinate, half a pixel right of a [`Quad`]
/// corner); its colour is `vertex × colour >> 7`. Quads after a texture select of a texture the sprite has are not
/// drawn (no textured GUI sprite is used yet). Nothing is drawn when the colour's alpha is 0 (`FUN_003017f8`).
pub fn sprite(out: &mut Vec<Quad>, s: &Sprite, pos: [f32; 2], frame: f32, scale: [f32; 2], colour: [u8; 4]) {
    sprite_with(out, s, pos, frame, scale, colour, MENU_TAN);
}

pub fn sprite_with(out: &mut Vec<Quad>, s: &Sprite, pos: [f32; 2], frame: f32, scale: [f32; 2], colour: [u8; 4], tan: f32) {
    if colour[3] == 0 { return; }
    let (x_scale, y_scale) = scales(tan);
    let bones = pose(s, frame);
    let pts: Vec<([f32; 2], [u8; 4])> = s.vertices.iter().map(|v| {
        let p = bones.get(v.bone).map_or(v.pos, |m| apply(m, v.pos));
        let x = pos[0] + x_scale * scale[0] * s.scale * p[0] / 16.0 + 0.5;
        let y = pos[1] - y_scale * scale[1] * s.scale * p[2] / 16.0 + 0.5;
        let c = std::array::from_fn(|i| ((v.rgba[i] as u32 * colour[i] as u32) >> 7).min(0xff) as u8);
        ([x, y], c)
    }).collect();
    let mut textured = false;
    for p in &s.prims {
        match *p {
            Prim::Texture(t) => textured = (t as usize) < s.textures,
            Prim::Quad([a, b, c, d]) if !textured => {
                let v = [a, b, d, c].map(|i| pts[i as usize]);
                // Flat shading: the GS takes the last vertex's colour (every GUI sprite in use has one colour).
                out.push(Quad { pos: v.map(|q| q.0), uv: [[0.0; 2]; 4], tex: Tex::None, rgba: v[3].1 });
            }
            Prim::Quad(_) => {}
        }
    }
}

/// `FUN_0027f858`: the width of `text` at `scale` (Σ advance·scale, truncated).
pub fn text_width(glyphs: &[Glyph; 256], text: &[u8], scale: f32) -> i32 {
    text.iter().map(|&c| glyphs[c as usize].advance as f32 * scale).sum::<f32>() as i32
}

/// `FUN_0027fcb0(x, y, scale, colour, text, …, texture slot, glyphs)`: one 16×16 cell per character (16·scale
/// square) at `(x, y + y offset·scale)`, then `x += advance·scale`. Characters 8..15 are colour codes (no change
/// while the colour table is off, `0x1b15e8`); below 0x20 the pad icons, 24×16, drawn grey (the colour's average);
/// 0x80..0xa7 draw the accent of character + 0x40 first, offset by its advance and y offset; a character with
/// advance 0 is not drawn.
#[allow(clippy::too_many_arguments)]
pub fn text(out: &mut Vec<Quad>, glyphs: &[Glyph; 256], slot: usize, text: &[u8], mut x: f32, y: f32, scale: f32, colour: [u8; 4]) {
    let cell = 16.0 * scale;
    let mut quad = |x: f32, y: f32, w: f32, h: f32, u: u8, v: u8, tw: f32, rgba: [u8; 4]| {
        let (u, v) = (u as f32, v as f32);
        out.push(Quad { pos: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]], uv: [[u, v], [u + tw, v], [u, v + 16.0], [u + tw, v + 16.0]], tex: Tex::Slot(slot), rgba });
    };
    for &c in text {
        if (8..16).contains(&c) { continue; }
        let g = glyphs[c as usize];
        if g.advance == 0 { continue; }
        if (0x80..0xa8).contains(&c) {
            let a = glyphs[c as usize + 0x40];
            quad(x + a.advance as f32 * scale, y + a.y as f32 * scale, cell, cell, a.u, a.v, 16.0, colour);
        }
        if c < 0x20 {
            let grey = ((colour[0] as u32 + colour[1] as u32 + colour[2] as u32) / 3) as u8;
            quad(x, y + g.y as f32 * scale, scale * 24.0, scale * 16.0, g.u, g.v, 24.0, [grey, grey, grey, colour[3]]);
        } else if c > 0x20 {
            quad(x, y + g.y as f32 * scale, cell, cell, g.u, g.v, 16.0, colour);
        }
        x += g.advance as f32 * scale;
    }
}

/// `FUN_0027e4d0(top, bottom, left, right, colour)`: an untextured rectangle.
pub fn rect(out: &mut Vec<Quad>, top: i32, bottom: i32, left: i32, right: i32, rgba: [u8; 4]) {
    let (t, b, l, r) = (top as f32, bottom as f32, left as f32, right as f32);
    out.push(Quad { pos: [[l, t], [r, t], [l, b], [r, b]], uv: [[0.0; 2]; 4], tex: Tex::None, rgba });
}

/// `FUN_0027f208(top, bottom, left, right, alpha, rgb)`: the dialogs' panel, a rectangle with three bars stepping in
/// at either side.
pub fn frame(out: &mut Vec<Quad>, top: i32, bottom: i32, left: i32, right: i32, alpha: i32, rgb: u32) {
    let c = rgba(((alpha as u32) << 24) | (rgb & 0xff_ffff));
    rect(out, top, bottom, left, right, c);
    for (dy, x0, x1) in [(1, -2, 0), (2, -3, -2), (4, -4, -3)] { rect(out, top + dy, bottom - dy, left + x0, left + x1, c); }
    for (dy, x0, x1) in [(1, 0, 2), (2, 2, 3), (4, 3, 4)] { rect(out, top + dy, bottom - dy, right + x0, right + x1, c); }
}

/// `FUN_00280550`'s box (its 0x18-byte record): the clip rows and columns, the anchor (`x` the left edge, or the
/// centre with [`text_box::CENTRE`]; `y` the first line's top, or the block's centre with [`text_box::MIDDLE`]), the
/// line height; the widest line and the block's height come back in `width` / `height`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextBox {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub line: i32,
    pub flags: u16,
}

pub mod text_box {
    /// Lines centred on `x`.
    pub const CENTRE: u16 = 1;
    /// The block centred on `y`.
    pub const MIDDLE: u16 = 2;
    /// Measure only (`width` / `height`), nothing drawn.
    pub const MEASURE: u16 = 4;
}

/// The lines of `text` as inclusive `(start, end)` indices (an empty line ends before it starts) and the last line's
/// width: a line takes characters while its width is below `budget`, then ends at the last space or control
/// character seen (else where it stopped); characters 0 and 1 end it.
fn wrap(text: &[u8], glyphs: &[Glyph; 256], budget: i32, last_w: i32) -> (Vec<(usize, isize)>, i32) {
    let at = |i: usize| text.get(i).copied().unwrap_or(0);
    let adv = |c: u8| glyphs[c as usize].advance as i8 as i32;
    let mut lines = Vec::new();
    let mut w_last = last_w;
    if at(0) == 0 { return (lines, w_last); }
    let mut i = 0usize;
    loop {
        let start = i;
        let (mut brk, mut j, mut width) = (i, i, 0);
        if 0 < budget {
            loop {
                let c = at(j);
                if c == 0x20 || c < 0x10 { brk = j; }
                if c < 2 { break; }
                j += 1;
                width += adv(c);
                if width >= budget { break; }
            }
        }
        w_last = width;
        let e = if brk == start { j } else { brk };
        let end = if at(e) == 0x20 || at(e) < 0x10 { e as isize - 1 } else { e as isize };
        lines.push((start, end));
        if at(e) == 0 { break; }
        i = e + 1;
        if at(i) == 0 || lines.len() > 0x4e { break; }
    }
    (lines, w_last)
}

/// `FUN_00280550(1.0, box, colour, text, −1, slot, glyphs)`: `text` wrapped into the box's width (twice the
/// nearer side's distance from `x` when centred), the width narrowed by 16 at a time while the last line stays under a
/// third of it (the first width again when that adds a line); each line in the box's rows drawn at the box's
/// line height.
pub fn text_in_box(out: &mut Vec<Quad>, glyphs: &[Glyph; 256], slot: usize, b: &mut TextBox, text: &[u8], colour: [u8; 4]) {
    let centred = b.flags & text_box::CENTRE != 0;
    let budget0 = if centred { 2 * (b.right - b.x).min(b.x - b.left) } else { b.right - b.x };
    let (mut budget, mut first, mut last_w, mut redo) = (budget0, 0, 0, false);
    let lines = loop {
        let (lines, w) = wrap(text, glyphs, budget, last_w);
        if redo { break lines; }
        if first == 0 { first = lines.len(); }
        if lines.len() < 2 { break lines; }
        last_w = w;
        if first < lines.len() {
            redo = true;
            budget = budget0;
            continue;
        }
        let third = budget / 3;
        budget -= 16;
        if w >= third { break lines; }
    };
    let n = lines.len() as i32;
    b.width = 0;
    b.height = n * b.line;
    let mut y = if b.flags & text_box::MIDDLE != 0 { b.y - (n * b.line >> 1) } else { b.y };
    for (s, e) in lines {
        if b.top <= y + b.line && y <= b.bottom {
            let part = &text[s..((e + 1).max(s as isize) as usize).min(text.len())];
            let w = text_width(glyphs, part, 1.0);
            b.width = b.width.max(w);
            if b.flags & text_box::MEASURE == 0 {
                let x = if centred { b.x - (w >> 1) } else { b.x };
                self::text(out, glyphs, slot, part, x as f32, y as f32, 1.0, colour);
            }
        }
        y += b.line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_box_wraps_at_spaces_and_line_breaks() {
        let mut g = [Glyph::default(); 256];
        for c in 0x20..0x80 { g[c] = Glyph { u: 0, v: 0, y: 0, advance: 10 }; }
        // 8 characters a line at most (width < 80 before the 8th is added).
        let text = b"aaa bbb ccc\x01\x01dd";
        let (lines, _) = wrap(text, &g, 80, 0);
        let s = |(a, e): (usize, isize)| String::from_utf8_lossy(&text[a..(e + 1).max(a as isize) as usize]).into_owned();
        assert_eq!(lines.into_iter().map(s).collect::<Vec<_>>(), ["aaa bbb", "ccc", "", "dd"]);
        let mut b = TextBox { top: 0, bottom: 416, left: 0, right: 80, x: 0, y: 10, line: 16, ..Default::default() };
        let mut out = Vec::new();
        text_in_box(&mut out, &g, 1, &mut b, text, [0x80; 4]);
        assert_eq!((b.width, b.height), (70, 64));
    }

    fn key(tx: f32, parent: Option<usize>) -> BoneKey { BoneKey { rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3], translation: [tx, 0.0, 0.0], parent } }

    #[test]
    fn keys_blend_and_chain() {
        let s = Sprite { scale: 1.0, vertices: vec![], prims: vec![], frames: vec![vec![key(0.0, None), key(3654.0, Some(0))], vec![key(0.0, None), key(23762.0, Some(0))]], textures: 0 };
        let m = pose(&s, 0.25);
        assert_eq!(m[1][3][0], 3654.0 + 0.25 * (23762.0 - 3654.0));
        // Frame 1 blends towards frame 0 (wrapping).
        assert_eq!(pose(&s, 1.5)[1][3][0], 0.5 * 23762.0 + 0.5 * 3654.0);
    }


    #[test]
    fn text_advances_and_skips_colour_codes() {
        let mut g = [Glyph::default(); 256];
        g[b'A' as usize] = Glyph { u: 0, v: 0, y: 0, advance: 14 };
        g[b' ' as usize] = Glyph { u: 0, v: 0, y: 0, advance: 5 };
        let mut out = Vec::new();
        text(&mut out, &g, 1, b"A\x09 A", 10.0, 20.0, 1.0, [0x80; 4]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].pos[0], [10.0 + 14.0 + 5.0, 20.0]);
        assert_eq!(text_width(&g, b"A A", 1.0), 33);
    }

    #[test]
    fn colour_lerp() {
        assert_eq!(lerp_colour(0.0, 0x7fe0e0e0, 0x5ff0c070), rgba(0x7fe0e0e0));
        assert_eq!(lerp_colour(1.0, 0x7fe0e0e0, 0x5ff0c070), rgba(0x5ff0c070));
    }
}
