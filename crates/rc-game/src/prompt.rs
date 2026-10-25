//! The action prompt (L00): the one-line "△ Activate Vendor" text an object asks for while Ratchet can use it, and
//! the lease that keeps two objects from asking at once.
//!
//! **The lease** (`0x1a8fb4` the owner, 0 free; `0x1a8fb0` the ticks it holds; `0x1b2008` the text id; `0x1a8fb8`
//! the frames since it was freed): `0x327a28(owner, text, ticks)` grants it to `owner` when free (1) or renews it
//! when `owner` holds it (2), else refuses (0); a grant with a text puts the text on the HUD (`0x30eff8` →
//! `0x42ef28`: the panel's text widget at (255, 80), font 2, for `ticks` frames, 0x168 when −1) and the hold is 2
//! ticks. `0x327ae0` is the forcing form: a refusal takes the lease over (3), the hold then `ticks`. `0x327950`
//! (the main loop, before the moby updates, not in state 0x1c) counts the hold down and frees the owner at 0,
//! counts the frames since (at most 10: the quick select opens only on a △ within them, `0x330240(0x10, …)`), and
//! keeps the prompt's GUI screen 0xc up while an owner holds a text.
//!
//! Owners: 1 the vendor moby (crate::moby::classes::vendor), 6 the vendor menu (crate::vendor), 5 the help panel
//! (crate::help, not ported there).
//!
//! Not ported: the text widget's own animator (`0x42ed58`: the line split at `\x01`, its measured width, its
//! scale 1.0 — drawn here as one centred line in the HUD font); the GUI screen 0xc (its draw `0x2f8380` does
//! nothing: the widget draws).

use crate::gui::{self, Quad};
use rc_formats::disc::boot::Glyph;

/// The widget's place (`0x42ef28`: (255, 80)) and its default show (0x168 frames).
pub const AT: [f32; 2] = [255.0, 80.0];
pub const SHOW_FRAMES: i32 = 0x168;
/// The prompt's text colour (the panel's text widgets).
const COLOUR: u32 = 0x80f0_f0f0;

/// The lease and its text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    /// `0x1a8fb4`: the owner (0 free); `0x1a8fb0`: the ticks it holds.
    pub owner: i32,
    pub hold: i32,
    /// `0x1b2008`: the text id (0: none shown).
    pub text: u32,
    /// `0x1a8fb8`: frames since the lease was freed (at most 10).
    pub free_frames: i32,
    /// The widget's frames left (`0x42ef28`'s `ticks`).
    pub show: i32,
    /// The widget shows a message of its own (`0x30efb8` → `0x42f690`: "You got the … Armor!"), outside the lease.
    pub message: bool,
}

impl Prompt {
    /// `0x327a28(owner, text, ticks)`: 2 when `owner` holds the lease (renewed), 1 when it was free, 0 refused.
    pub fn request(&mut self, owner: i32, text: u32, ticks: i32) -> i32 {
        if self.owner == owner {
            if text != 0 { self.show_text(text, ticks); }
            self.hold = 2;
            self.text = text;
            2
        } else if self.owner == 0 {
            if text != 0 { self.show_text(text, ticks); }
            self.hold = 2;
            self.owner = owner;
            self.text = text;
            1
        } else {
            0
        }
    }

    /// `0x327ae0(owner, text, ticks)`: [`request`](Self::request), else the lease taken over (3) with the hold
    /// `ticks`.
    pub fn force(&mut self, owner: i32, text: u32, ticks: i32) -> i32 {
        let r = self.request(owner, text, ticks);
        if r != 0 { return r; }
        if text != 0 { self.show_text(text, ticks); }
        self.hold = ticks;
        self.text = text;
        self.owner = owner;
        3
    }

    /// `0x30eff8(text, ticks)` → `0x42ef28`: the widget shows `text` for `ticks` frames (0x168 when −1).
    fn show_text(&mut self, text: u32, ticks: i32) {
        self.text = text;
        self.show = if ticks == -1 { SHOW_FRAMES } else { ticks };
        self.message = false;
    }

    /// `0x30efb8(text, ticks)` → `0x42f690`: the widget shows `text` for `ticks` frames (0x168 when −1) on its own
    /// (the armour vendor's "You got the … Armor!").
    pub fn announce(&mut self, text: u32, ticks: i32) {
        self.show_text(text, ticks);
        self.message = true;
    }

    /// `0x327950`: the hold counts down (the owner freed at 0), the frames since.
    pub fn tick(&mut self) {
        if self.owner == 0 {
            self.free_frames = (self.free_frames + 1).min(10);
        } else {
            self.free_frames = 0;
        }
        if self.hold != 0 {
            self.hold -= 1;
            if self.hold == 0 { self.owner = 0; }
        }
        if 0 < self.show { self.show -= 1; }
    }

    /// Screen 0xc is up: an owner holds a text.
    pub fn shown(&self) -> bool { self.owner != 0 && self.text != 0 }

    /// The widget's text, centred at [`AT`] in font 2 (`0x42ed58`'s font argument), texture slot 2.
    pub fn draw(&self, text: &dyn Fn(u32) -> Vec<u8>, glyphs: &[Glyph; 256], out: &mut Vec<Quad>) {
        if !(self.shown() || self.message) || self.show <= 0 { return; }
        let t = text(self.text);
        let t = t.split(|&c| c == 0).next().unwrap_or(&[]);
        let w = gui::text_width(glyphs, t, 1.0);
        gui::text(out, glyphs, 2, t, AT[0] - (w >> 1) as f32, AT[1], 1.0, COLOUR.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lease_is_granted_renewed_refused_and_freed() {
        let mut p = Prompt::default();
        assert_eq!(p.request(1, 0x12eb, 1), 1);
        assert_eq!(p.request(1, 0x12eb, 1), 2);
        assert_eq!(p.request(2, 0x1350, 1), 0);
        assert!(p.shown());
        // The hold is 2 ticks: freed on the second tick, then the frames since count.
        p.tick();
        assert_eq!(p.owner, 1);
        p.tick();
        assert_eq!(p.owner, 0);
        assert!(!p.shown());
        p.tick();
        assert_eq!(p.free_frames, 1);
        // The forcing form takes a held lease over.
        assert_eq!(p.request(1, 0x12eb, 1), 1);
        assert_eq!(p.force(6, 0, 1), 3);
        assert_eq!((p.owner, p.hold), (6, 1));
    }
}
