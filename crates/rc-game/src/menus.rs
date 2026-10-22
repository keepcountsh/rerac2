//! The front end's pages past the main menu (game mode 3, the page loop `0x2cb860`) and the card dialog in front of
//! them (mode 4 kind 1: update `0x287140`, draw `0x286378`): New Game's save slots, Load's, and the overwrite
//! question. Spec: docs/plan/bringup.md §7.
//!
//! **Opening** (the main menu's rows, `0x2d67c0`): New Game (kind 4) and Load (kind 5) play sound 4, raise the card
//! monitor's "a save / a load is asked" flag (`0x1a7424` 2 / 4) and open the card dialog with the page to show once
//! the card is ready ([`Menus::new_game`], [`Menus::load_game`]).
//!
//! **The card dialog** ([`Dialog`]): the monitor's status (crate::memcard) picks its text and buttons. Ready (1) or
//! data found (0x10) opens the page; the questions (no save data: create it? unformatted: format it?) set the flags
//! the monitor acts on; "continue without saving" starts a new game from the blank save; going back returns to the
//! main menu.
//!
//! **The pages** (rects `{cx, cy, w, h}` per panel, `0x2ccc18`: a panel's box is `cx − w/2, cy − h/2, w, h`):
//! * New Game `0x260ec8`: the slots (update `0x2de168`, draw `0x2de768`), the selected slot's planet (`0x2dd638`),
//!   the buttons (`0x2d9718` over the rows `0x1ab738`), the card's message (`0x2db700`). An empty slot is saved at
//!   once, a used one asks first (page `0x2610e8`); ○ starts without saving; △ goes back.
//! * Load `0x260d98`: the slots (update `0x2ddd30`), the planet, the card's message. ✕ on a used slot loads it.
//! * The overwrite question `0x2610e8` (`0x2dc8a8` / `0x2dc940`, buttons `0x2caaa8`): ○ yes, △ no.
//!
//! **The slot column** (the front end's window object `+0x3cea0`: built `0x33a678`, updated `0x33a8f0`, drawn
//! `0x33a9f8`): GUI sprites 0xc8..0xca at (256, 212), the page's title above (hidden on the overwrite question),
//! Load's two button labels under it, and the selection, sprite 0xd6 at the selected slot, pulsing.
//!
//! Not drawn yet: the dialog frame's GUI-sprite border (`0x339398`).

use crate::front_end::Assets;
use crate::gui::{self, text_box, Quad, Tex, TextBox};
use crate::memcard::{status, MemCard, Preview, TYPE_PS2};
use crate::save::{self, SaveData, Scope};

/// Pad bits (`0x138344`).
mod pad {
    pub const TRIANGLE: u16 = 0x10;
    pub const CIRCLE: u16 = 0x20;
    pub const CROSS: u16 = 0x40;
    pub const SELECT: u16 = 0x100;
    pub const START: u16 = 0x800;
    pub const UP: u16 = 0x1000;
    pub const DOWN: u16 = 0x4000;
}

/// Texts.
mod text {
    pub const EMPTY: u32 = 0x2daa;
    pub const OVERWRITE: u32 = 0x2c8f;
    pub const SAVING: u32 = 0x2c92;
    pub const LOADING: u32 = 0x2c93;
    pub const CHECKING: u32 = 0x2c96;
    pub const PLANET: u32 = 0xb44;
    pub const CONTINUE: u32 = 0x31c3;
    pub const YES_CROSS: u32 = 0x31c7;
    pub const YES_CIRCLE: u32 = 0x31c8;
    pub const NO_TRIANGLE: u32 = 0x31c4;
    pub const GO_BACK: u32 = 0x2c7e;
    pub const CONTINUE_ANYWAY: u32 = 0x2c81;
}

/// The icons (`FUN_0028edf0(id, frame)`): the slot's clock, its count and its date (frames 3, 4, 2), the bolt, the
/// page's picture.
pub const ICON_SLOT: u16 = 0xe99d;
pub const ICON_BOLT: u16 = 0x7567;
pub const ICON_PICTURE: u16 = 0xeaa9;

/// The slot list's layout (`0x1abc58`).
mod slots {
    pub const LINE: i32 = 21;
    pub const GAP: i32 = 27;
    pub const ICON: f32 = 16.0;
    pub const SECOND_ICON: i32 = 0x50;
    pub const X: i32 = 29;
    pub const Y: i32 = 2;
    pub const PICTURE: [f32; 4] = [356.0, 280.0, 104.0, 110.0];
    pub const PICTURE_ALPHA: u8 = 100;
    pub const BOLTS_X: i32 = 84;
    /// The thousands mark's offset (comma; full stop for languages 3, 4, 5) and the digits' pitch.
    pub const COMMA: (i32, i32) = (1, 2);
    pub const STOP: (i32, i32) = (0, 2);
    pub const DIGIT: f32 = 11.6;
}

/// The planets' names and places (`0x258d50`: per level the planet and the place; −1 none).
const PLANETS: [(u32, i32); 27] = [
    (0xb45, 0xb2b), (0xb46, 0xb2c), (0xb47, 0xb2d), (0xb48, 0xb2e), (0xb49, 0xb2f), (0xb4a, 0xb30), (0xb4b, 0xb31),
    (0xb4c, 0xb32), (0xb4d, 0xb33), (0xb4e, 0xb34), (0xb4f, 0xb35), (0xb50, 0xb36), (0xb51, 0xb37), (0xb52, 0xb38),
    (0xb53, 0xb39), (0xb54, 0xb3a), (0xb55, 0xb3b), (0xb56, 0xb3c), (0xb57, 0xb3d), (0xb58, 0xb3e), (0xb59, 0xb3f),
    (0xb5a, 0xb40), (0xb5b, 0xb41), (0xb5c, 0xb42), (0xb5d, 0xb43), (0x1b23, -1), (0x1b24, 0x1b22),
];

/// A panel's box from its page rect `{cx, cy, w, h}` (`0x2ccc18`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

const fn rect(cx: i32, cy: i32, w: i32, h: i32) -> Rect { Rect { x: cx - w / 2, y: cy - h / 2, w, h } }

/// The panels' rects on the save / load pages (`0x260ec8`, `0x260d98`, `0x2610e8`).
const SLOTS: Rect = rect(90, 57, 112, 23);
const PLANET: Rect = rect(490, 237, 322, 44);
const BUTTONS: Rect = rect(460, 400, 528, 250);
const MESSAGE: Rect = rect(324, 162, 307, 101);
const QUESTION: Rect = rect(420, 260, 460, 330);
const ANSWERS: Rect = rect(438, 325, 230, 200);

/// New Game's button rows (`0x1ab738`): "✕ Select file", "○ Continue without saving", "△ Exit".
const NEW_GAME_ROWS: [u32; 3] = [0x2c32, 0x2dc6, 0x2be5];

/// The slot column (`0x33a678`): its place, its sprites and their colours (`0x1adc50`, `0x1add18`), the title's
/// and the labels' offsets (`0x33c078`), the selection's sprite, offset and step (`0x1adc68`..`0x1adc70`).
mod window {
    pub const AT: [f32; 2] = [256.0, 212.0];
    pub const SPRITES: [(u32, u32); 3] = [(0xc8, 0x6044_2d00), (0xc9, 0x6024_1700), (0xca, 0x55f0_c070)];
    pub const TITLE: [f32; 2] = [64.0, -164.0];
    pub const EXIT: [f32; 2] = [10.0, 99.0];
    pub const SELECT: [f32; 2] = [10.0, 136.0];
    pub const SELECTION: u32 = 0xd6;
    pub const SELECTION_AT: [f32; 2] = [-169.0, -138.0];
    pub const SELECTION_STEP: f32 = 90.0;
    /// "Save Game", "Load Game", "✕ Select file", "△ Exit".
    pub const SAVE_GAME: u32 = 0x2c34;
    pub const LOAD_GAME: u32 = 0x2c33;
    pub const SELECT_FILE: u32 = 0x2c32;
    pub const EXIT_TEXT: u32 = 0x2be5;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    NewGame,
    Load,
    /// "A save game already exists in the file you have selected. Overwrite?" (`0x2610e8`).
    Overwrite,
}

/// What a frame asks of the front end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Sound(u32),
    /// Back to the main menu.
    MainMenu,
    /// Start / Select on a page: the menus close (the title).
    Close,
    /// `0x2f7328`: a new game starts from the save data (the blank save, saved to a slot or not).
    NewGame,
    /// A save was loaded into the save data: the game goes to its level.
    Loaded { level: i32 },
}

/// Mode 4, kind 1 (`0x286270(1, page)`): the card dialog in front of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dialog {
    /// The page shown when the card is ready (`0x1bac54`).
    next: Page,
    /// Opened from a page (drawn under it), not from the main menu.
    over_page: bool,
    /// Frames shown (`0x1bac5c`) and the failure texts' wait (`0x1bac68`).
    frames: i32,
    fail_wait: i32,
}

/// The menus' state (the page loop's globals).
pub struct Menus {
    page: Page,
    /// The page before (`0x1f28a0`; the overwrite question's answer is read by the page it returns to).
    previous: Option<Page>,
    dialog: Option<Dialog>,
    /// The page loop's sub-state 3 (the backdrop alone for one frame) before a page runs.
    fading: bool,
    /// The selected slot (`0x1a7c10`).
    pub slot: usize,
    /// The slot panel's state (`+0x4c`: 0 opened, 1 save now, 2 running).
    panel: i32,
    /// A save or a load is running (`0x1f28f8`) and its message (`0x1f28fc`).
    waiting: bool,
    message: u32,
    /// Frames the card machine has been idle (`0x1f2924`).
    idle: i32,
    /// The dialog was opened for a load (`0x1f2920`).
    load: bool,
    /// The overwrite question was answered yes (`0x261214`).
    overwrite: bool,
    /// The selection's pulse (`0x1a9e94`: one step a frame, back to 0 on Up / Down).
    pulse: u32,
}

/// The game's reset to the blank save (`0x299bf8`): the options (`0x1a7b98`, 0x28 bytes) kept, the level 0, no slot.
pub fn reset_to_blank(save: &mut SaveData, blank: &SaveData, card: &mut MemCard) {
    let options = save.bytes(Scope::Global, 0x1a7b98, 0x28).map(<[u8]>::to_vec);
    *save = blank.clone();
    if let (Some(o), Some(b)) = (options, save.bytes_mut(Scope::Global, 0x1a7b98, 0x28)) { b.copy_from_slice(&o); }
    save.set_i32(Scope::Global, 0x1a79f8, 0);
    card.checked = false;
    card.cards[0].slot = -1;
}

/// What a frame works with: the card, the save data it reads and writes, the blank save, the console clock (BCD).
pub struct Ctx<'a> {
    pub card: &'a mut MemCard,
    pub save: &'a mut SaveData,
    pub blank: &'a SaveData,
    pub clock: [u8; 8],
}

impl Menus {
    fn open(next: Page, load: bool) -> Menus {
        Menus {
            page: next,
            previous: None,
            dialog: Some(Dialog { next, over_page: false, frames: 0, fail_wait: 0 }),
            fading: false,
            slot: 0,
            panel: 0,
            waiting: false,
            message: 0,
            idle: 0,
            load,
            overwrite: false,
            pulse: 0,
        }
    }

    /// The main menu's New Game (`0x2d67c0` kind 4).
    pub fn new_game(card: &mut MemCard, slot: usize) -> (Menus, Out) {
        card.flags = (card.flags & !4) | 2;
        (Menus { slot, ..Menus::open(Page::NewGame, false) }, Out::Sound(4))
    }

    /// The main menu's Load (kind 5).
    pub fn load_game(card: &mut MemCard, slot: usize) -> (Menus, Out) {
        card.flags = (card.flags & !2) | 4;
        (Menus { slot, ..Menus::open(Page::Load, true) }, Out::Sound(4))
    }

    /// The card dialog is up over the main menu (the front end draws the menu under it).
    pub fn over_main_menu(&self) -> bool { self.dialog.is_some_and(|d| !d.over_page) }

    fn ready_card(&self, card: &MemCard) -> bool { card.idle() && 10 < self.idle && card.cards[0].ty == TYPE_PS2 }

    fn go(&mut self, page: Page, out: &mut Vec<Out>) {
        out.push(Out::Sound(if page == self.page { 0x11 } else { 0x12 }));
        self.previous = Some(self.page);
        self.page = page;
        self.fading = true;
        self.panel = 0;
    }

    /// The dialog again in front of `page` (`0x2b58f8(4, 1, 1, page)`, or the row action `0x2d67a0` again).
    fn dialog_for(&mut self, page: Page) { self.dialog = Some(Dialog { next: page, over_page: true, frames: 0, fail_wait: 0 }); }

    /// One frame: `pressed` the buttons down this frame (the slot panels' flag 1, the pad's auto-repeat `0x138334`,
    /// is clear).
    pub fn tick(&mut self, pressed: u16, cx: &mut Ctx) -> Vec<Out> {
        let mut out = Vec::new();
        if cx.card.idle() { self.idle += 1 } else { self.idle = 0 }
        if self.dialog.is_some() {
            self.dialog_tick(pressed, cx, &mut out);
            return out;
        }
        if std::mem::take(&mut self.fading) { return out; }
        // The slot column's update (`0x33a8f0`).
        self.pulse = if pressed & (pad::UP | pad::DOWN) != 0 { 0 } else { self.pulse + 1 };
        // The monitor's "card changed" while a page runs (`0x2cb860`): the dialog.
        if cx.card.flags & 1 != 0 {
            self.dialog_for(self.page);
            return out;
        }
        match self.page {
            Page::NewGame => self.new_game_tick(pressed, cx, &mut out),
            Page::Load => self.load_tick(pressed, cx, &mut out),
            Page::Overwrite => self.overwrite_tick(pressed, cx, &mut out),
        }
        out
    }

    /// Up / Down on the slots (`0x1a7c10`, 0..=3).
    fn move_slot(&mut self, keys: u16) {
        if keys & pad::UP != 0 && self.slot != 0 { self.slot -= 1; }
        if keys & pad::DOWN != 0 && self.slot < 3 { self.slot += 1; }
    }

    /// The card's status sends a page back to the dialog (or to the main menu).
    fn check_status(&mut self, cx: &mut Ctx, out: &mut Vec<Out>) -> bool {
        match cx.card.status {
            status::PRESENT => {
                // `0x2d67a0(4 / 5, page)`: the row's action again.
                cx.card.flags = if self.load { (cx.card.flags & !2) | 4 } else { (cx.card.flags & !4) | 2 };
                out.push(Out::Sound(4));
                self.dialog_for(self.page);
                true
            }
            status::READY | status::FOUND => false,
            _ => {
                self.dialog_for(self.page);
                true
            }
        }
    }

    /// `0x2de168`.
    fn new_game_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if self.panel == 0 && self.previous == Some(Page::Overwrite) && std::mem::take(&mut self.overwrite) { self.panel = 1; }
        if self.panel == 1 {
            // `0x2df710`: the blank save, stamped and written whole into the slot.
            reset_to_blank(cx.save, cx.blank, cx.card);
            save::set_clock(cx.save, cx.clock);
            if let Some(p) = cx.card.cards[0].previews.get_mut(self.slot) { p.level = 0; }
            cx.card.save_whole(cx.save, self.slot as i32);
            self.waiting = true;
            self.idle = 0;
            self.message = text::SAVING;
        }
        self.panel = 2;
        if self.waiting {
            if !(cx.card.idle() && 10 < self.idle) { return; }
            self.waiting = false;
            let card = &mut *cx.card;
            if card.error == 0 && card.cards[0].info == 0 {
                card.checked = true;
                let s = &*cx.save;
                if let Some(p) = card.cards[0].previews.get_mut(self.slot) {
                    *p = Preview {
                        level: s.i32(Scope::Global, 0x1a79f8),
                        bolts: s.i32(Scope::Global, 0x1a7a00),
                        byte_12: s.u8(Scope::Global, 0x1a7a12) as i32,
                        word_bc8: s.i32(Scope::Global, 0x1a7bc8),
                        clock: cx.clock,
                        bad: false,
                    };
                }
                card.front_end = false;
                out.push(Out::NewGame);
            } else {
                card.checked = false;
                card.flags |= 0x80;
                self.dialog_for(Page::NewGame);
            }
            return;
        }
        if keys & (pad::START | pad::SELECT) != 0 {
            out.push(Out::Close);
            return;
        }
        if keys & pad::TRIANGLE != 0 {
            out.push(Out::MainMenu);
            return;
        }
        if self.check_status(cx, out) { return; }
        if !self.ready_card(cx.card) { return; }
        let was = self.slot;
        self.move_slot(keys);
        if keys & pad::CIRCLE != 0 {
            // Continue without saving.
            reset_to_blank(cx.save, cx.blank, cx.card);
            cx.card.flags = (cx.card.flags & !6) | 0x20;
            out.push(Out::NewGame);
            return;
        }
        if keys & pad::CROSS != 0 {
            if cx.card.cards[0].previews[self.slot].level == -1 {
                self.panel = 1;
                return;
            }
            self.go(Page::Overwrite, out);
        }
        if self.slot != was { out.push(Out::Sound(0x11)); }
    }

    /// `0x2ddd30`.
    fn load_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if self.waiting {
            if !(cx.card.idle() && 10 < self.idle) { return; }
            self.waiting = false;
            let card = &mut *cx.card;
            if card.error == 0 && card.cards[0].info == 0 {
                card.checked = true;
                card.front_end = false;
                let level = cx.save.i32(Scope::Global, 0x1a79f8);
                out.push(if level < 1 && save::visited(cx.save, 0) == 0 { Out::NewGame } else { Out::Loaded { level } });
            } else {
                card.checked = false;
                card.flags |= 0x100;
                self.dialog_for(Page::Load);
            }
            return;
        }
        if keys & (pad::START | pad::SELECT) != 0 {
            out.push(Out::Close);
            return;
        }
        if keys & pad::TRIANGLE != 0 {
            out.push(Out::MainMenu);
            return;
        }
        if self.check_status(cx, out) { return; }
        if !self.ready_card(cx.card) { return; }
        let was = self.slot;
        self.move_slot(keys);
        if keys & pad::CROSS != 0 {
            if cx.card.cards[0].previews[self.slot].level >= 0 {
                out.push(Out::Sound(4));
                cx.card.load(self.slot as i32);
                self.waiting = true;
                self.idle = 0;
                self.message = text::LOADING;
            } else {
                out.push(Out::Sound(5));
            }
        }
        if self.slot != was { out.push(Out::Sound(3)); }
    }

    /// `0x2dc8a8`.
    fn overwrite_tick(&mut self, pressed: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if !cx.card.ready() {
            self.go(Page::NewGame, out);
            return;
        }
        if pressed & pad::CIRCLE != 0 {
            self.overwrite = true;
            self.go(Page::NewGame, out);
        } else if pressed & pad::TRIANGLE != 0 {
            self.overwrite = false;
            self.go(Page::NewGame, out);
        }
    }

    /// `0x287140` kind 1, as the front end runs it (`0x1a8c88` set, its own card check page `0x1a8c8c` not).
    fn dialog_tick(&mut self, pressed: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        let Some(d) = self.dialog.as_mut() else { return };
        d.frames += 1;
        let (next, load) = (d.next, self.load);
        let card = &mut *cx.card;
        let cross = pressed & pad::CROSS != 0;
        let circle = pressed & pad::CIRCLE != 0;
        let triangle = pressed & pad::TRIANGLE != 0;
        // Leave the dialog: to its page, or back to the main menu.
        let leave = |m: &mut Menus, page: Option<Page>, out: &mut Vec<Out>| {
            m.dialog = None;
            match page {
                Some(p) => {
                    m.previous = Some(m.page);
                    m.page = p;
                    m.fading = true;
                    m.panel = 0;
                }
                None => out.push(Out::MainMenu),
            }
        };
        let start_blank = |m: &mut Menus, cx: &mut Ctx, out: &mut Vec<Out>| {
            reset_to_blank(cx.save, cx.blank, cx.card);
            cx.card.flags = (cx.card.flags & !6) | 0x20;
            m.dialog = None;
            out.push(Out::NewGame);
        };
        match card.status {
            status::READY | status::FOUND => leave(self, Some(next), out),
            status::CHANGED => {
                if cross {
                    card.flags &= !1;
                    leave(self, None, out);
                }
            }
            status::RECHECK | status::UNFORMATTED => {
                if triangle {
                    card.flags &= !6;
                    leave(self, None, out);
                }
            }
            status::POLL => {
                if !load && cross {
                    start_blank(self, cx, out);
                } else if triangle {
                    card.flags = ((card.flags & !2) | 0x20) & !4;
                    leave(self, None, out);
                }
            }
            status::FORMAT_ASK => {
                if circle {
                    card.flags |= 8;
                } else if triangle {
                    card.flags = (card.flags | 0x20) & !6;
                }
            }
            status::PRESENT => {
                if card.flags & 6 == 0 { leave(self, None, out); }
            }
            status::NO_DATA if load => {
                if triangle {
                    card.flags &= !6;
                    leave(self, None, out);
                }
            }
            status::CREATE_ASK => {
                if circle {
                    card.flags |= 0x10;
                } else if triangle {
                    card.flags = ((card.flags & !2) | 0x20) & !4;
                }
            }
            status::FORMAT_FAILED | status::CREATE_FAILED | status::SAVE_FAILED => {
                let d = self.dialog.as_mut().unwrap();
                if d.fail_wait > 0 {
                    d.fail_wait -= 1;
                } else if cross {
                    card.flags &= !0x446;
                    leave(self, None, out);
                }
            }
            status::NO_SPACE => {
                if load {
                    if cross || triangle {
                        card.flags &= !6;
                        leave(self, None, out);
                    }
                } else if cross {
                    start_blank(self, cx, out);
                } else if triangle {
                    card.flags &= !6;
                    leave(self, None, out);
                }
            }
            status::LOAD_FAILED => {
                if cross {
                    card.flags &= !0x446;
                    leave(self, None, out);
                }
            }
            status::UNFORMATTED_FE | status::NO_DATA_FE => {
                if cross {
                    start_blank(self, cx, out);
                } else if triangle {
                    card.flags |= 0x20;
                    leave(self, None, out);
                }
            }
            _ => {}
        }
    }

    /// The dialog's text and buttons for the card's status (`0x286378` kind 1); None draws nothing.
    fn dialog_text(&self, card: &MemCard, a: &Assets) -> Option<(Vec<u8>, Vec<u32>)> {
        let s = |id: u32| a.strings.get(id).to_vec();
        let load = self.load;
        let (body, buttons): (Vec<u8>, Vec<u32>) = match card.status {
            status::CHANGED => (s(0x2c7b), vec![text::CONTINUE]),
            status::RECHECK | status::POLL if !load => (s(0x2c82), vec![text::YES_CROSS, text::NO_TRIANGLE]),
            status::RECHECK | status::POLL | status::UNFORMATTED => (s(0x2c7c), vec![text::GO_BACK]),
            status::FORMAT_ASK => (s(0x2c8a), vec![text::YES_CIRCLE, text::NO_TRIANGLE]),
            status::FORMAT | status::FORMATTING => (s(0x2c94), vec![]),
            status::CHECK | status::CHECKING => (s(text::CHECKING), vec![]),
            status::NO_DATA if load => (s(0x2c7c), vec![text::GO_BACK]),
            status::NO_DATA | status::CREATE_ASK => (s(0x2c86), vec![text::YES_CIRCLE, text::NO_TRIANGLE]),
            status::CREATE | status::CREATING => (s(0x2c95), vec![]),
            status::FORMAT_FAILED => (s(0x2c97), vec![text::CONTINUE]),
            status::CREATE_FAILED | status::SAVE_FAILED => (s(0x2c99), vec![text::CONTINUE]),
            status::NO_SPACE if load => (s(0x2c7c), vec![text::GO_BACK]),
            status::NO_SPACE => ([s(0x2c7f), vec![1, 1], s(text::CONTINUE_ANYWAY)].concat(), vec![text::YES_CROSS, text::NO_TRIANGLE]),
            status::LOAD_FAILED => (s(0x2c98), vec![text::CONTINUE]),
            status::UNFORMATTED_FE => (s(0x2c8c), vec![text::YES_CROSS, text::NO_TRIANGLE]),
            status::NO_DATA_FE => ([s(0x2c88), vec![1, 1], s(text::CONTINUE_ANYWAY)].concat(), vec![text::YES_CROSS, text::NO_TRIANGLE]),
            _ => return None,
        };
        Some((body, buttons))
    }

    /// The frame's 2D draws over the menu backdrop. `icon(id, frame)`: a GUI image's texture and texel size.
    pub fn draw(&self, a: &Assets, card: &MemCard, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) -> Vec<Quad> {
        let mut out = Vec::new();
        if let Some(d) = &self.dialog {
            // Mode 4 over the page loop's last picture: the page darkened.
            if d.over_page { self.draw_page(&mut out, a, card, icon); }
            gui::fill(&mut out, [0, 0, 0, 0x30]);
            if 0 < d.frames {
                if let Some((body, buttons)) = self.dialog_text(card, a) { dialog_box(&mut out, a, &body, &buttons); }
            }
            return out;
        }
        if !self.fading { self.draw_page(&mut out, a, card, icon); }
        out
    }

    fn draw_page(&self, out: &mut Vec<Quad>, a: &Assets, card: &MemCard, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) {
        self.draw_window(out, a);
        match self.page {
            Page::NewGame => {
                self.draw_slots(out, a, card, icon);
                self.draw_planet(out, a, card);
                draw_rows(out, a, &NEW_GAME_ROWS);
                self.draw_message(out, a, card);
            }
            Page::Load => {
                self.draw_slots(out, a, card, icon);
                self.draw_planet(out, a, card);
                self.draw_message(out, a, card);
            }
            Page::Overwrite => {
                self.draw_slots(out, a, card, icon);
                draw_question(out, a);
                draw_answers(out, a);
            }
        }
    }

    /// `0x33a9f8`: the slot column's sprites, its title (not on the overwrite question: `0x2caa38`), Load's labels
    /// (`0x1a8c88` hides them on New Game's page), the selection.
    fn draw_window(&self, out: &mut Vec<Quad>, a: &Assets) {
        use window::*;
        for (id, c) in SPRITES {
            if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, AT, 0.0, [1.0, 1.0], gui::rgba(c)); }
        }
        // `0x3378a0`: font 1, centred.
        let label = |out: &mut Vec<Quad>, id: u32, [dx, dy]: [f32; 2]| {
            let t = a.strings.get(id);
            let w = gui::text_width(&a.glyphs, t, 1.0);
            gui::text(out, &a.glyphs, 1, t, AT[0] + dx - (w >> 1) as f32, AT[1] + dy, 1.0, gui::rgba(0x80f0_f0f0));
        };
        let load = self.page == Page::Load || (self.page == Page::Overwrite && self.load);
        if self.page != Page::Overwrite { label(out, if load { LOAD_GAME } else { SAVE_GAME }, TITLE); }
        if self.page == Page::Load {
            label(out, SELECT_FILE, SELECT);
            label(out, EXIT_TEXT, EXIT);
        }
        // `0x2aa3f0(0x60442d00, 0x70fffeed, 20)`.
        let k = self.pulse % 40;
        let t = if k < 20 { k as f32 / 20.0 } else { 1.0 - (k - 20) as f32 / 20.0 };
        if let Some(s) = a.sprites.get(&SELECTION) {
            let at = [AT[0] + SELECTION_AT[0], AT[1] + self.slot as f32 * SELECTION_STEP + SELECTION_AT[1]];
            gui::sprite(out, s, at, 0.0, [1.0, 1.0], gui::lerp_colour(t, 0x70ff_feed, 0x6044_2d00));
        }
    }

    /// `0x2de768`: the page's picture, then per slot its play time, bolts and date (or EMPTY).
    fn draw_slots(&self, out: &mut Vec<Quad>, a: &Assets, card: &MemCard, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) {
        use slots::*;
        if card.status == status::CHECK { return; }
        let [px, py, pw, ph] = PICTURE;
        draw_icon(out, icon, ICON_PICTURE, 0, [px, py, pw, ph], PICTURE_ALPHA);
        let shown = card.idle() && 10 < self.idle && card.cards[0].ty == TYPE_PS2;
        let font = |out: &mut Vec<Quad>, x: i32, y: i32, c: u32, t: &[u8]| gui::text(out, &a.glyphs_2, 2, t, x as f32, y as f32, 1.0, gui::rgba(c));
        let icon_at = |out: &mut Vec<Quad>, id: u16, f: u16, x: i32, y: i32| draw_icon(out, icon, id, f, [x as f32, y as f32, ICON, ICON], 0x80);
        let mut y = SLOTS.y;
        for (i, p) in card.cards[0].previews.iter().enumerate() {
            if !shown {
                y += 3 * LINE;
            } else if p.level != -1 {
                let colour = if self.slot == i { 0x70ff_feed } else { 0x80f0_f0f0 };
                let mut hours = p.word_bc8 / 0x34bc0;
                let mut minutes = p.word_bc8 / 0xe10 - hours * 60;
                if 99 < hours { (hours, minutes) = (99, 59); }
                icon_at(out, ICON_SLOT, 3, X + 4, y + Y);
                font(out, X + ICON as i32 + 6, y + Y, colour, format!("{hours:02}:{minutes:02}").as_bytes());
                if p.byte_12 != 0 {
                    icon_at(out, ICON_SLOT, 4, X + SECOND_ICON, y + Y);
                    font(out, X + ICON as i32 + 2 + SECOND_ICON, y + Y, colour, p.byte_12.min(99).to_string().as_bytes());
                }
                y += LINE;
                let bolts = p.bolts.min(9_999_999);
                let digits = bolts.to_string();
                icon_at(out, ICON_BOLT, 0, X + 4, y + Y);
                let right = X + ICON as i32 + 6 + BOLTS_X;
                let w = gui::text_width(&a.glyphs_2, digits.as_bytes(), 1.0);
                font(out, right - w, y + Y, colour, digits.as_bytes());
                let (mark, (dx, dy)) = if matches!(card_language(), 3..=5) { (".", STOP) } else { (",", COMMA) };
                let mut k = 3.0;
                while k < digits.len() as f32 {
                    let x = right + dx - (DIGIT * k) as i32;
                    let w = gui::text_width(&a.glyphs_2, mark.as_bytes(), 1.0);
                    font(out, x - w, y + Y + dy, colour, mark.as_bytes());
                    k += 3.0;
                }
                y += LINE;
                let c = p.clock;
                icon_at(out, ICON_SLOT, 2, X + 4, y + Y);
                font(out, X + ICON as i32 + 6, y + Y, colour, format!("{:02x}/{:02x}/{:02x}", c[6], c[5], c[7]).as_bytes());
                y += LINE;
            } else {
                y += LINE;
                let t = a.strings.get(text::EMPTY);
                let w = gui::text_width(&a.glyphs_2, t, 1.0);
                font(out, SLOTS.x + SLOTS.w / 2 - (w >> 1), y + Y, 0x80f0_f0f0, t);
                y += 2 * LINE;
            }
            y += GAP;
        }
    }

    /// `0x2dd638`: the selected slot's planet and place, centred on the panel's left edge, or EMPTY.
    fn draw_planet(&self, out: &mut Vec<Quad>, a: &Assets, card: &MemCard) {
        if !(card.idle() && 10 < self.idle && card.cards[0].ty == TYPE_PS2) { return; }
        let level = card.cards[0].previews[self.slot].level % 0x1c;
        let centred = |out: &mut Vec<Quad>, y: i32, t: &[u8]| {
            let w = gui::text_width(&a.glyphs, t, 1.0);
            gui::text(out, &a.glyphs, 1, t, (PLANET.x - (w >> 1)) as f32, y as f32, 1.0, gui::rgba(0x80f0_f0f0));
        };
        let half = 16 / 2;
        if level == -1 {
            centred(out, PLANET.y + half, a.strings.get(text::EMPTY));
            return;
        }
        let Some(&(name, place)) = usize::try_from(level).ok().and_then(|l| PLANETS.get(l)) else { return };
        let name = if matches!(level, 2 | 5 | 10 | 0x17..=0x1a) {
            a.strings.get(name).to_vec()
        } else {
            [a.strings.get(text::PLANET), b" ", a.strings.get(name)].concat()
        };
        centred(out, PLANET.y + if place < 0 { half } else { 0 }, &name);
        if 0 <= place { centred(out, PLANET.y + 16, a.strings.get(place as u32)); }
    }

    /// `0x2db700`: while the card works (or has been idle under 11 frames), its message (checking, or the save's /
    /// load's) in orange over a black shadow, centred in the panel.
    fn draw_message(&self, out: &mut Vec<Quad>, a: &Assets, card: &MemCard) {
        if card.cards[0].ty != TYPE_PS2 || (card.idle() && 10 < self.idle) { return; }
        let t = a.strings.get(if self.waiting { self.message } else { text::CHECKING });
        let r = MESSAGE;
        let mut b = TextBox { top: r.y + 1, bottom: r.y + r.h + 1, left: r.x + 1, right: r.x + r.w + 1, x: r.x + (r.w >> 1), y: r.y + 5, line: 0x10, flags: text_box::CENTRE | text_box::MEASURE, ..Default::default() };
        gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, [0; 4]);
        b.y = r.y + ((r.h - b.height) >> 1);
        b.flags ^= text_box::MEASURE;
        gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, gui::rgba(0x8000_0000));
        (b.top, b.bottom, b.left, b.right, b.x, b.y) = (b.top - 1, b.bottom - 1, b.left - 1, b.right - 1, b.x - 1, b.y - 1);
        gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, gui::rgba(0x80ff_a888));
    }
}

/// The language (`0x1a7bbc`): English until the options are ported.
fn card_language() -> u8 { 0 }

/// `FUN_0028f0d0(frame, x, y, w, h, alpha)`: a GUI image over the box, its colour 0x7f7f7f.
fn draw_icon(out: &mut Vec<Quad>, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, id: u16, frame: u16, [x, y, w, h]: [f32; 4], alpha: u8) {
    let Some((tex, [tw, th])) = icon(id, frame) else { return };
    out.push(Quad { pos: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], tex, rgba: [0x7f, 0x7f, 0x7f, alpha] });
}

/// `0x2d9718` with New Game's rows (flags 0xc141a): font 2, line 14 + 2, centred on half the panel's width, each row
/// under the last from the panel's top + 17 − 7, white.
fn draw_rows(out: &mut Vec<Quad>, a: &Assets, rows: &[u32]) {
    let r = BUTTONS;
    let line = 0xe;
    let mut b = TextBox { top: r.y + 4, bottom: r.y + r.h - 4, left: r.x, right: r.x + r.w - 2, x: r.w >> 1, y: (line + 3) - line / 2 + r.y, line: line + 2, flags: text_box::CENTRE, ..Default::default() };
    for &id in rows {
        gui::text_in_box(out, &a.glyphs_2, 2, &mut b, a.strings.get(id), gui::rgba(0x80f0_f0f0));
        b.y += b.height;
    }
}

/// `0x2dc940`: the overwrite question, centred in its panel over a black shadow.
fn draw_question(out: &mut Vec<Quad>, a: &Assets) {
    let r = QUESTION;
    let t = a.strings.get(text::OVERWRITE);
    let mut b = TextBox { top: r.y, bottom: r.y + r.h + 1, left: r.x, right: r.x + r.w + 1, x: (r.x + r.w) >> 1, y: r.y + 5, line: 0x10, flags: text_box::CENTRE | text_box::MEASURE, ..Default::default() };
    b.width = r.w - 5;
    b.height = r.h - 5;
    gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, [0; 4]);
    b.y = (r.h - b.height) >> 1;
    b.flags ^= text_box::MEASURE;
    gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, gui::rgba(0x8000_0000));
    (b.top, b.bottom, b.left, b.right, b.x, b.y) = (b.top - 1, b.bottom - 1, b.left - 1, b.right - 1, b.x - 1, b.y - 1);
    gui::text_in_box(out, &a.glyphs_2, 2, &mut b, t, gui::rgba(0x80f0_f0f0));
}

/// `0x2caaa8`: "○ Yes" from the panel's left half-width and "△ No" right-aligned at its right.
fn draw_answers(out: &mut Vec<Quad>, a: &Assets) {
    let r = ANSWERS;
    let (x, half) = (r.x, r.w >> 1);
    let white = gui::rgba(0x80f0_f0f0);
    gui::text(out, &a.glyphs_2, 2, a.strings.get(text::YES_CIRCLE), (x - half) as f32, r.y as f32, 1.0, white);
    let no = a.strings.get(text::NO_TRIANGLE);
    let w = gui::text_width(&a.glyphs_2, no, 1.0);
    gui::text(out, &a.glyphs_2, 2, no, (x + half - w) as f32, r.y as f32, 1.0, white);
}

/// `0x338f88(…, box, 0x80f0f0f0, text, font 1, centred on the screen, −1, 0, buttons…)` with mode 4's box (x 96..416,
/// anchor 256, line 16): the text, a blank line and the buttons (two 15 spaces apart), measured, centred
/// vertically on the screen, framed (`0x27f208`: 10 pixels beyond the text's half width, 5 beyond its half height,
/// alpha 0x60, colour 0x442d00), then drawn in font 1.
fn dialog_box(out: &mut Vec<Quad>, a: &Assets, body: &[u8], buttons: &[u32]) {
    let mut t = body.to_vec();
    if !buttons.is_empty() {
        t.extend_from_slice(&[1, 1]);
        for (i, &b) in buttons.iter().enumerate() {
            if i > 0 { t.extend_from_slice(b"               "); }
            t.extend_from_slice(a.strings.get(b));
        }
    }
    let mut b = TextBox { top: 0, bottom: gui::SCREEN_H as i32, left: 0x60, right: 0x1a0, x: 0x100, y: 0x68, line: 0x10, flags: text_box::CENTRE | text_box::MIDDLE | text_box::MEASURE, ..Default::default() };
    gui::text_in_box(out, &a.glyphs, 1, &mut b, &t, [0; 4]);
    let top = (gui::SCREEN_H as i32 - b.height) >> 1;
    (b.top, b.bottom, b.y) = (top, top + b.height, top + (b.height >> 1));
    gui::text_in_box(out, &a.glyphs, 1, &mut b, &t, [0; 4]);
    let (hw, hh) = ((b.width >> 1) + 10, (b.height >> 1) + 5);
    gui::frame(out, b.y - hh, b.y + hh, b.x - hw, b.x + hw, 0x60, 0x442d00);
    b.flags &= !text_box::MEASURE;
    gui::text_in_box(out, &a.glyphs, 1, &mut b, &t, gui::rgba(0x80f0_f0f0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_boxes_come_from_centres() {
        assert_eq!(SLOTS, Rect { x: 34, y: 46, w: 112, h: 23 });
        assert_eq!(MESSAGE, Rect { x: 171, y: 112, w: 307, h: 101 });
    }
}
