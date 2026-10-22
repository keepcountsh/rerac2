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
//! **Options** (the main menu's kind 3 row, page `0x259568`; its list widget `0x343028`: frame sprites 0xcd / 0xce,
//! the selection 0x95, the rows `0x261570`, an icon per row): 16:9, Vibration and Subtitles, each a toggle page
//! (`0x33d1f8`, `0x33eb58`, `0x33e4c0`: a window of sprites 0x8b..0x8d, its title, "✕ Toggle" / "△ Exit" and the
//! setting's line), then the port's own Port Options ([`port`]). Progressive Scan (a PS2 video-mode test) is left
//! out; 16:9 is the port's aspect (4:3 or 16:9) as well as the game's byte `0x1a7bb9`.
//!
//! Not drawn yet: the dialog frame's GUI-sprite border (`0x339398`).

pub mod port;

use crate::front_end::Assets;
use crate::gui::{self, text_box, Quad, Tex, TextBox};
use crate::memcard::{status, MemCard, Preview, TYPE_PS2};
use crate::save::{self, SaveData, Scope};
use port::{PortValues, Setting, ENTRIES};
use rc_formats::disc::boot::MenuRow;

/// Pad bits (`0x138344`).
mod pad {
    pub const TRIANGLE: u16 = 0x10;
    pub const CIRCLE: u16 = 0x20;
    pub const CROSS: u16 = 0x40;
    pub const SELECT: u16 = 0x100;
    pub const START: u16 = 0x800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const LEFT: u16 = 0x8000;
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
    Options,
    Toggle(Toggle),
    Port,
    /// In the game: the pause list (mode 3 kind 1, level 00 page `0x281d48`), its Options (`0x2844b0`), the save
    /// page (kind 4) and the quit question (mode 4 kind 9).
    Pause,
    PauseOptions,
    Save,
    Quit,
    /// The in-game Options' setting pages (level 00 `0x433e28` Sound, `0x4334c8` Camera, `0x4347a0` Controls).
    Sound,
    Camera,
    Controls,
}

/// The game's options in the save data (`0x1a7b98`.., kept across a new game): words up to the camera speed, then
/// bytes.
pub mod opt {
    /// 1: stereo (`0x132938`).
    pub const STEREO: u32 = 0x1a7ba0;
    /// The volumes, 0..=1024: music, sound effects.
    pub const MUSIC: u32 = 0x1a7ba4;
    pub const SFX: u32 = 0x1a7ba8;
    /// 1: normal (not reversed).
    pub const PITCH_NORMAL: u32 = 0x1a7bac;
    pub const YAW_NORMAL: u32 = 0x1a7bb0;
    /// 0..=2: the camera's yaw rate (`gp+0x2568` + 4·n).
    pub const CAM_SPEED: u32 = 0x1a7bb4;
    pub const WIDESCREEN: u32 = 0x1a7bb9;
    pub const VIBRATION: u32 = 0x1a7bba;
    pub const QS_PAUSE: u32 = 0x1a7bbb;
}

/// The setting pages' window (`0x431c38` with the gp tables: sprites 0x8b..0x8d) and rows.
mod settings {
    pub const SOUND_FRAME: f32 = 0.67;
    pub const CAMERA_FRAME: f32 = 0.65;
    pub const CONTROLS_FRAME: f32 = 0.4;
    /// Sound: the labels right-aligned at x 256 (font 2), the sliders 10 right of the window's centre.
    pub const SOUND_ROWS: [f32; 3] = [145.0, 190.0, 225.0];
    pub const SLIDER_AT: [f32; 3] = [10.0, -49.0, -4.0];
    pub const SLIDER: [f32; 2] = [150.0, 16.0];
    pub const SLIDER_BACK: u32 = 0x20ff_ffff;
    pub const SLIDER_FILL: u32 = 0x55f0_c070;
    /// Camera: four rows centred at x 256 (font 2).
    pub const CAMERA_ROWS: [f32; 4] = [120.0, 155.0, 190.0, 225.0];
    /// Controls: two rows centred (font 1).
    pub const CONTROLS_ROWS: [f32; 2] = [150.0, 190.0];
    pub const SELECTED: u32 = 0x80d0_d0d0;
    pub const OTHER: u32 = 0x7080_8080;
    /// Texts: "Sound effects volume", "Music volume", "Toggle stereo/mono", MONO, STEREO; "Up/Down movement",
    /// "Left/Right movement", "Change camera rotation speed", Reversed, Normal, Slow, Medium, Fast; "Vibration",
    /// "Quick Select Pause"; the titles Sound, Camera, Controls.
    pub const SFX: u32 = 0x2da5;
    pub const MUSIC: u32 = 0x2da6;
    pub const STEREO: u32 = 0x2da7;
    pub const MONO_STEREO: [u32; 2] = [0x2da8, 0x2da9];
    pub const UP_DOWN: u32 = 0x2c78;
    pub const LEFT_RIGHT: u32 = 0x2c77;
    pub const SPEED: u32 = 0x2dae;
    pub const REVERSED_NORMAL: [u32; 2] = [0x2c7a, 0x2c79];
    pub const SPEEDS: [u32; 3] = [0x2dad, 0x2dac, 0x2dab];
    pub const VIBRATION: u32 = 0x2c30;
    pub const QS_PAUSE: u32 = 0x3079;
    pub const SOUND_TITLE: u32 = 0x2c2d;
    pub const CAMERA_TITLE: u32 = 0x2c2e;
    pub const CONTROLS_TITLE: u32 = 0x2bf7;
}

/// The pause list's rows (`0x345740`, on foot: the texts `0x1abb30`) and the port's Port Options after Options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseRow {
    Weapons,
    Gadgets,
    QuickSelect,
    Items,
    Help,
    Options,
    Port,
    Special,
}

impl PauseRow {
    pub const ALL: [PauseRow; 8] =
        [PauseRow::Weapons, PauseRow::Gadgets, PauseRow::QuickSelect, PauseRow::Items, PauseRow::Help, PauseRow::Options, PauseRow::Port, PauseRow::Special];
    fn text(self) -> u32 {
        match self {
            PauseRow::Weapons => 0x2be7,
            PauseRow::Gadgets => 0x2be8,
            PauseRow::QuickSelect => 0x2be9,
            PauseRow::Items => 0x2bea,
            PauseRow::Help => 0x2beb,
            PauseRow::Options | PauseRow::Port => 0x2bec,
            PauseRow::Special => 0x2c49,
        }
    }
}

/// The in-game Options rows (`0x28a0b8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseOption {
    HelpDesk,
    Save,
    Load,
    Controls,
    Sound,
    Camera,
    Subtitles,
    Quit,
}

impl PauseOption {
    pub const ALL: [PauseOption; 8] =
        [PauseOption::HelpDesk, PauseOption::Save, PauseOption::Load, PauseOption::Controls, PauseOption::Sound, PauseOption::Camera, PauseOption::Subtitles, PauseOption::Quit];
    fn text(self) -> u32 {
        match self {
            PauseOption::HelpDesk => 0x2c29,
            PauseOption::Save => 0x2c2a,
            PauseOption::Load => 0x2c2b,
            PauseOption::Controls => 0x2bf7,
            PauseOption::Sound => 0x2c2d,
            PauseOption::Camera => 0x2c2e,
            PauseOption::Subtitles => 0x2c2f,
            PauseOption::Quit => 0x2c31,
        }
    }
}

/// The pause list (`0x43f3f8` / `0x43fc18`, button rows) and the in-game Options list (`0x438148` / `0x438b48`).
mod pause {
    pub const AT: [f32; 2] = [250.0, 190.0];
    /// Row i's button at (x, y0 + step·i + dy), keyframe 0.37; its label 11 lower; y0 centres the rows on 208 with a
    /// row height of 46.
    pub const BUTTON: [f32; 3] = [-126.0, -11.0, 50.0];
    pub const LABEL: [f32; 3] = [0.0, 0.0, 50.0];
    pub const BUTTON_KEY: f32 = 0.37;
    pub const ROW_HEIGHT: f32 = 46.0;
    pub const FILL: (u32, u32) = (0x73, 0x6044_2d00);
    pub const FRAME: u32 = 0x74;
    pub const OPTIONS_AT: [f32; 2] = [251.0, 170.0];
    /// The frame sprites (keyframe 1: eight rows, but 0x94's) and the title's offset.
    pub const OPTIONS_SPRITES: [(u32, u32, f32); 4] = [(0x91, 0x6044_2d00, 1.0), (0x94, 0x80ff_de8d, 0.0), (0x92, 0x55f0_c070, 1.0), (0x93, 0x55f0_c070, 1.0)];
    pub const OPTIONS_TITLE: [f32; 2] = [0.0, -144.0];
    /// Options' row icons: 0xea9e frames 0..7 (`0x28edf0`).
    pub const OPTIONS_ICON: u16 = 0xea9e;
    /// "Options" (`0x2bec`), "Are you sure you want to quit your current game?" (`0x2c63`).
    pub const OPTIONS_TEXT: u32 = 0x2bec;
    pub const QUIT_TEXT: u32 = 0x2c63;
}

/// The Options page's toggles: the game's option byte, the page's title, the setting's text, the window's keyframe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Widescreen,
    Vibration,
    Subtitles,
}

impl Toggle {
    /// The option byte (`0x1a7bb9` / `0x1a7bba` / `0x1a7b9e`).
    pub fn byte(self) -> u32 {
        match self {
            Toggle::Widescreen => 0x1a7bb9,
            Toggle::Vibration => 0x1a7bba,
            Toggle::Subtitles => 0x1a7b9e,
        }
    }
    fn title(self) -> u32 {
        match self {
            Toggle::Widescreen => 0x307a,
            Toggle::Vibration => 0x2bf7,
            Toggle::Subtitles => 0x2c2f,
        }
    }
    fn text(self) -> u32 {
        match self {
            Toggle::Widescreen => 0x307a,
            Toggle::Vibration => 0x2c30,
            Toggle::Subtitles => 0x2c2f,
        }
    }
    fn frame(self) -> f32 { if self == Toggle::Widescreen { 0.65 } else { 0.3 } }
    /// The Options rows' pages (`0x261570`).
    fn of_page(target: u32) -> Option<Toggle> {
        match target {
            0x2597c8 => Some(Toggle::Widescreen),
            0x25d710 => Some(Toggle::Vibration),
            0x25d5e0 => Some(Toggle::Subtitles),
            _ => None,
        }
    }
}

/// The Options list (`0x343028`) and the toggle windows (`0x33bc20` with `0x1addb8`).
mod options {
    pub const AT: [f32; 2] = [253.0, 200.0];
    pub const FRAMES: [(u32, u32); 2] = [(0xcd, 0x6044_2d00), (0xce, 0x55f0_c070)];
    /// The frames' keyframe by row count (`0x1ae180`).
    pub const KEYFRAME: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 2.0, 0.0];
    pub const SELECTION: u32 = 0x95;
    pub const SELECTION_AT: [f32; 3] = [-102.0, -85.7, 35.8];
    pub const LABELS_AT: [f32; 3] = [-114.0, -94.0, 36.0];
    pub const ICONS_AT: [f32; 3] = [-22.0, -102.0, 36.0];
    /// Per row of `0x261570`: its icon and frame (`0x1ae110`; row 3 is `0xea9e`).
    pub const ICONS: [(u16, u16); 4] = [(0xeaa6, 1), (0xeaa6, 2), (0xeaa6, 3), (0xea9e, 6)];
    pub const ICON_RGBA: u32 = 0x60f0_f0b0;
    pub const TOGGLE_AT: [f32; 2] = [255.0, 195.0];
    pub const TOGGLE_SPRITES: [(u32, u32); 3] = [(0x8b, 0x6044_2d00), (0x8c, 0x55f0_c070), (0x8d, 0x80ff_de8d)];
    pub const TOGGLE_TITLE: [f32; 2] = [0.0, -143.0];
    pub const TOGGLE_TOGGLE: [f32; 2] = [0.0, 114.0];
    pub const TOGGLE_EXIT: [f32; 2] = [0.0, 140.0];
    /// "✕ Toggle", "△ Exit", "on", "off".
    pub const TOGGLE_TEXT: u32 = 0x2be4;
    pub const EXIT_TEXT: u32 = 0x2be5;
    pub const ON: u32 = 0x2c5c;
    pub const OFF: u32 = 0x2c5d;
    /// The Port Options rows shown at once (the frame's five-row keyframe); the list scrolls.
    pub const PORT_VISIBLE: usize = 5;
    /// The values' centre: the frame's right panel.
    pub const PORT_VALUE_X: f32 = 124.0;
}

/// A row of the Options page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionRow {
    Game { text: u32, scale: f32, toggle: Toggle, icon: (u16, u16) },
    Port,
}

/// The Options page's rows: the disc's (`0x261570`) without Progressive Scan, then Port Options.
pub fn option_rows(menu: &[MenuRow]) -> Vec<OptionRow> {
    let mut rows: Vec<OptionRow> = menu
        .iter()
        .enumerate()
        .filter_map(|(i, r)| Some(OptionRow::Game { text: r.text, scale: r.scale, toggle: Toggle::of_page(r.target)?, icon: *options::ICONS.get(i)? }))
        .collect();
    rows.push(OptionRow::Port);
    rows
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
    /// A Port Options row (or the 16:9 toggle) changed its setting.
    Port(Setting, u8),
    /// In the game: the pause closes (the game goes on).
    Resume,
    /// In the game: Quit Game was answered yes (back to the front end).
    Quit,
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
    /// The Options list's row (`+0x60`), the Port Options row and the first one shown.
    row: usize,
    port_row: usize,
    port_top: usize,
    /// The pages run in the game (the pause), not the front end: the parents and the card's variants differ.
    in_game: bool,
    /// The pause list's and its Options list's rows.
    pause_row: usize,
    options_row: usize,
    /// The setting page's row.
    setting_row: usize,
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
    /// The port's settings (the engine's).
    pub port: &'a mut PortValues,
    /// The buttons held (`0x138340`: the sliders move while ← / → are held).
    pub held: u16,
    /// The Options page's rows ([`option_rows`]).
    pub option_rows: &'a [OptionRow],
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
            row: 0,
            port_row: 0,
            port_top: 0,
            in_game: false,
            pause_row: 0,
            options_row: 0,
            setting_row: 0,
        }
    }

    /// In the game, Start (level 00 `0x3706f0`: mode 3 kind 1): the pause list.
    pub fn pause(slot: usize) -> (Menus, Out) {
        let mut m = Menus::open(Page::Pause, false);
        m.dialog = None;
        m.fading = true;
        m.in_game = true;
        m.slot = slot;
        (m, Out::Sound(0x12))
    }

    /// The page a page's △ goes back to (None: out of the menus: the main menu, or the game).
    fn parent(&self, page: Page) -> Option<Page> {
        if self.in_game {
            match page {
                Page::Pause => None,
                Page::PauseOptions | Page::Port => Some(Page::Pause),
                _ => Some(Page::PauseOptions),
            }
        } else {
            match page {
                Page::Toggle(_) | Page::Port => Some(Page::Options),
                Page::Overwrite => Some(Page::NewGame),
                _ => None,
            }
        }
    }

    /// △: the parent page, or out of the menus.
    fn back(&mut self, out: &mut Vec<Out>) {
        match self.parent(self.page) {
            Some(p) => self.go(p, out),
            None => out.push(if self.in_game { Out::Resume } else { Out::MainMenu }),
        }
    }

    /// Start / Select: the menus close (the title, or back to the game).
    fn close(&self, out: &mut Vec<Out>) { out.push(if self.in_game { Out::Resume } else { Out::Close }); }

    /// The main menu's Options (kind 3, page `0x259568`).
    pub fn options() -> (Menus, Out) {
        let mut m = Menus::open(Page::Options, false);
        m.dialog = None;
        m.fading = true;
        (m, Out::Sound(0x12))
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
            Page::Options => self.options_tick(pressed, cx, &mut out),
            Page::Toggle(t) => self.toggle_tick(t, pressed, cx, &mut out),
            Page::Port => self.port_tick(pressed, cx, &mut out),
            Page::Pause => self.pause_tick(pressed, &mut out),
            Page::PauseOptions => self.pause_options_tick(pressed, cx, &mut out),
            Page::Save => self.save_tick(pressed, cx, &mut out),
            Page::Quit => self.quit_tick(pressed, &mut out),
            Page::Sound | Page::Camera | Page::Controls => self.setting_tick(pressed, cx, &mut out),
        }
        out
    }

    /// The pause list (`0x347938` / `0x43f740`): Up / Down wrap past disabled rows (sound 3), ✕ the row's page
    /// (Options, Port Options; the others are not ported: nothing happens); △, Start, Select back to the game.
    fn pause_tick(&mut self, keys: u16, out: &mut Vec<Out>) {
        let rows = pause_rows();
        let n = rows.len();
        if keys & (pad::TRIANGLE | pad::START | pad::SELECT) != 0 {
            out.push(Out::Resume);
        } else if keys & (pad::UP | pad::DOWN) != 0 {
            out.push(Out::Sound(3));
            let step = if keys & pad::UP != 0 { n - 1 } else { 1 };
            let start = self.pause_row;
            loop {
                self.pause_row = (self.pause_row + step) % n;
                if pause_enabled(rows[self.pause_row]) || self.pause_row == start { break; }
            }
        } else if keys & pad::CROSS != 0 {
            match rows[self.pause_row] {
                PauseRow::Options => self.go(Page::PauseOptions, out),
                PauseRow::Port => {
                    self.port_row = 0;
                    self.port_top = 0;
                    self.go(Page::Port, out);
                }
                _ => {}
            }
        }
    }

    /// The in-game Options list: Save (the card dialog, then the save page), Load, Subtitles, Quit Game; the others
    /// are not ported.
    fn pause_options_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        let n = PauseOption::ALL.len();
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
        } else if keys & (pad::START | pad::SELECT) != 0 {
            self.close(out);
        } else if keys & (pad::UP | pad::DOWN) != 0 {
            out.push(Out::Sound(3));
            self.options_row = if keys & pad::UP != 0 { (self.options_row + n - 1) % n } else { (self.options_row + 1) % n };
        } else if keys & pad::CROSS != 0 {
            match PauseOption::ALL[self.options_row] {
                PauseOption::Save => {
                    // `0x2d67c0` kind 4.
                    out.push(Out::Sound(4));
                    cx.card.flags = (cx.card.flags & !4) | 2;
                    self.load = false;
                    self.previous = Some(self.page);
                    self.page = Page::Save;
                    self.dialog = Some(Dialog { next: Page::Save, over_page: false, frames: 0, fail_wait: 0 });
                }
                PauseOption::Load => {
                    out.push(Out::Sound(4));
                    cx.card.flags = (cx.card.flags & !2) | 4;
                    self.load = true;
                    self.previous = Some(self.page);
                    self.page = Page::Load;
                    self.dialog = Some(Dialog { next: Page::Load, over_page: false, frames: 0, fail_wait: 0 });
                }
                PauseOption::Subtitles => self.go(Page::Toggle(Toggle::Subtitles), out),
                PauseOption::Sound | PauseOption::Camera | PauseOption::Controls => {
                    self.setting_row = 0;
                    let page = match PauseOption::ALL[self.options_row] {
                        PauseOption::Sound => Page::Sound,
                        PauseOption::Camera => Page::Camera,
                        _ => Page::Controls,
                    };
                    self.go(page, out);
                }
                PauseOption::Quit => {
                    out.push(Out::Sound(4));
                    self.previous = Some(self.page);
                    self.page = Page::Quit;
                }
                _ => {}
            }
        }
    }

    /// The setting pages (`0x434088` Sound, `0x4335f0` Camera, `0x4348c8` Controls): Up / Down through the rows
    /// (sound 3), ✕ the row's toggle (sound 4; 5 on a slider), ← / → held move a volume by 6 (0..=1024).
    fn setting_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if keys & pad::TRIANGLE != 0 {
            return self.back(out);
        }
        if keys & (pad::START | pad::SELECT) != 0 {
            return self.close(out);
        }
        let n = match self.page {
            Page::Sound => 3,
            Page::Camera => 4,
            _ => 2,
        };
        if keys & (pad::UP | pad::DOWN) != 0 {
            out.push(Out::Sound(3));
            self.setting_row = if keys & pad::UP != 0 { (self.setting_row + n - 1) % n } else { (self.setting_row + 1) % n };
            return;
        }
        let save = &mut *cx.save;
        let flip = |save: &mut SaveData, addr: u32, word: bool| {
            if word {
                let v = save.i32(Scope::Global, addr);
                save.set_i32(Scope::Global, addr, (v == 0) as i32);
            } else {
                let v = save.u8(Scope::Global, addr);
                save.set_u8(Scope::Global, addr, (v == 0) as u8);
            }
        };
        match (self.page, self.setting_row) {
            (Page::Sound, r @ (0 | 1)) => {
                let addr = if r == 0 { opt::SFX } else { opt::MUSIC };
                let d = if cx.held & pad::LEFT != 0 { -6 } else if cx.held & pad::RIGHT != 0 { 6 } else { 0 };
                if d != 0 { save.set_i32(Scope::Global, addr, (save.i32(Scope::Global, addr) + d).clamp(0, 0x400)); }
                if keys & pad::CROSS != 0 { out.push(Out::Sound(5)); }
            }
            _ if keys & pad::CROSS == 0 => {}
            (Page::Sound, _) => {
                out.push(Out::Sound(4));
                flip(save, opt::STEREO, true);
            }
            (Page::Camera, 0) => {
                out.push(Out::Sound(4));
                flip(save, opt::PITCH_NORMAL, true);
            }
            (Page::Camera, 1) => {
                out.push(Out::Sound(4));
                flip(save, opt::YAW_NORMAL, true);
            }
            (Page::Camera, 2) => {
                out.push(Out::Sound(4));
                let v = save.i32(Scope::Global, opt::CAM_SPEED);
                save.set_i32(Scope::Global, opt::CAM_SPEED, (v + 1).rem_euclid(3));
            }
            (Page::Camera, _) => {
                out.push(Out::Sound(4));
                let on = cx.port.get(Setting::Aspect) == 0;
                save.set_u8(Scope::Global, opt::WIDESCREEN, on as u8);
                out.push(Out::Port(Setting::Aspect, if on { 2 } else { 0 }));
            }
            (_, 0) => {
                out.push(Out::Sound(4));
                flip(save, opt::VIBRATION, false);
            }
            _ => {
                out.push(Out::Sound(4));
                flip(save, opt::QS_PAUSE, false);
            }
        }
    }

    /// Mode 4 kind 9 (`0x287140`): ✕ yes, △ no.
    fn quit_tick(&mut self, keys: u16, out: &mut Vec<Out>) {
        if keys & pad::CROSS != 0 {
            out.push(Out::Quit);
        } else if keys & pad::TRIANGLE != 0 {
            self.page = Page::PauseOptions;
        }
    }

    /// The in-level save page's slots (`0x2dd888`): an empty slot is saved at once, a used one asks first; the
    /// game's state (stored into the save data when the pause opened) is written whole into the slot.
    fn save_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if self.panel == 0 && self.previous == Some(Page::Overwrite) && std::mem::take(&mut self.overwrite) { self.panel = 1; }
        if self.panel == 1 {
            // `0x2df668`: the clock, the save written whole.
            save::set_clock(cx.save, cx.clock);
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
            } else {
                card.flags |= 0x80;
                card.checked = false;
                self.dialog_for(Page::Save);
            }
            return;
        }
        if keys & (pad::START | pad::SELECT) != 0 {
            self.close(out);
            return;
        }
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
            return;
        }
        if self.check_status(cx, out) { return; }
        if !self.ready_card(cx.card) { return; }
        let was = self.slot;
        self.move_slot(keys);
        if keys & pad::CROSS != 0 {
            out.push(Out::Sound(4));
            if cx.card.cards[0].previews[self.slot].level == -1 {
                self.panel = 1;
                return;
            }
            self.go(Page::Overwrite, out);
        }
        if self.slot != was { out.push(Out::Sound(3)); }
    }

    /// `0x2d6408` / `0x348cb8`: Up / Down wrap (sound 3), ✕ opens the row's page, △ the main menu, Start / Select close.
    fn options_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        let n = cx.option_rows.len();
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
        } else if keys & (pad::START | pad::SELECT) != 0 {
            out.push(Out::Close);
        } else if keys & (pad::UP | pad::DOWN) != 0 && n > 0 {
            out.push(Out::Sound(3));
            self.row = if keys & pad::UP != 0 { (self.row + n - 1) % n } else { (self.row + 1) % n };
        } else if keys & pad::CROSS != 0 {
            match cx.option_rows.get(self.row) {
                Some(&OptionRow::Game { toggle, .. }) => self.go(Page::Toggle(toggle), out),
                Some(OptionRow::Port) => {
                    self.port_row = 0;
                    self.port_top = 0;
                    self.go(Page::Port, out);
                }
                None => {}
            }
        }
    }

    /// The toggle pages' update (`0x33d320` …): ✕ flips the byte (sound 4); △ back to Options.
    fn toggle_tick(&mut self, t: Toggle, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
        } else if keys & (pad::START | pad::SELECT) != 0 {
            self.close(out);
        } else if keys & pad::CROSS != 0 {
            out.push(Out::Sound(4));
            let on = !toggle_on(t, cx);
            cx.save.set_u8(Scope::Global, t.byte(), on as u8);
            if t == Toggle::Widescreen { out.push(Out::Port(Setting::Aspect, if on { 2 } else { 0 })); }
        }
    }

    /// Port Options: Up / Down wrap (sound 3), ✕ steps the row's value (sound 4), △ back to Options.
    fn port_tick(&mut self, keys: u16, cx: &mut Ctx, out: &mut Vec<Out>) {
        let n = ENTRIES.len();
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
        } else if keys & (pad::START | pad::SELECT) != 0 {
            self.close(out);
        } else if keys & (pad::UP | pad::DOWN) != 0 {
            out.push(Out::Sound(3));
            self.port_row = if keys & pad::UP != 0 { (self.port_row + n - 1) % n } else { (self.port_row + 1) % n };
            let v = options::PORT_VISIBLE;
            if self.port_row < self.port_top { self.port_top = self.port_row; }
            if self.port_row >= self.port_top + v { self.port_top = self.port_row + 1 - v; }
        } else if keys & pad::CROSS != 0 {
            let v = cx.port.next(self.port_row);
            if v != cx.port.values[self.port_row] {
                out.push(Out::Sound(4));
                cx.port.values[self.port_row] = v;
                out.push(Out::Port(ENTRIES[self.port_row].setting, v));
            }
        }
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
            self.close(out);
            return;
        }
        if keys & pad::TRIANGLE != 0 {
            self.back(out);
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
        // The page it asks for (in the game, the save page `0x25e850`'s).
        let slots = if self.in_game { Page::Save } else { Page::NewGame };
        if !cx.card.ready() {
            self.go(slots, out);
            return;
        }
        if pressed & pad::CIRCLE != 0 {
            self.overwrite = true;
            self.go(slots, out);
        } else if pressed & pad::TRIANGLE != 0 {
            self.overwrite = false;
            self.go(slots, out);
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
                None if m.in_game => {
                    m.page = Page::PauseOptions;
                    m.fading = true;
                }
                None => out.push(Out::MainMenu),
            }
        };
        // Continue without saving: a new game from the blank save (the front end only; in the game it goes back).
        let start_blank = |m: &mut Menus, cx: &mut Ctx, out: &mut Vec<Out>| {
            if m.in_game {
                cx.card.flags &= !6;
                m.dialog = None;
                m.page = Page::PauseOptions;
                m.fading = true;
                return;
            }
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
    pub fn draw(&self, a: &Assets, v: &View, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) -> Vec<Quad> {
        let mut out = Vec::new();
        let card = v.card;
        if let Some(d) = &self.dialog {
            // Mode 4 over the page loop's last picture: the page darkened.
            if d.over_page { self.draw_page(&mut out, a, v, icon); }
            gui::fill(&mut out, [0, 0, 0, 0x30]);
            if 0 < d.frames {
                if let Some((body, buttons)) = self.dialog_text(card, a) { dialog_box(&mut out, a, &body, &buttons); }
            }
            return out;
        }
        if !self.fading { self.draw_page(&mut out, a, v, icon); }
        out
    }

    fn draw_page(&self, out: &mut Vec<Quad>, a: &Assets, v: &View, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) {
        let card = v.card;
        match self.page {
            Page::Options => return self.draw_options(out, a, v, icon),
            Page::Toggle(t) => return draw_toggle(out, a, t, toggle_shown(t, v)),
            Page::Port => return self.draw_port(out, a, v),
            Page::Pause => return self.draw_pause(out, a),
            Page::PauseOptions => return self.draw_pause_options(out, a, icon),
            Page::Sound | Page::Camera | Page::Controls => return self.draw_setting(out, a, v),
            Page::Quit => {
                self.draw_pause_options(out, a, icon);
                gui::fill(out, [0, 0, 0, 0x30]);
                return dialog_box(out, a, a.strings.get(pause::QUIT_TEXT), &[text::YES_CROSS, text::NO_TRIANGLE]);
            }
            _ => {}
        }
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
            Page::Save => {
                self.draw_slots(out, a, card, icon);
                self.draw_planet(out, a, card);
                self.draw_message(out, a, card);
            }
            Page::Options | Page::Toggle(_) | Page::Port | Page::Pause | Page::PauseOptions | Page::Quit | Page::Sound | Page::Camera | Page::Controls => {}
        }
    }

    /// The setting pages' draws (`0x434320` Sound, `0x433798` Camera, `0x4349d0` Controls): the window, then the rows
    /// ("<setting>: <value>"; the selected row light, the others grey).
    fn draw_setting(&self, out: &mut Vec<Quad>, a: &Assets, v: &View) {
        use settings::*;
        let (frame, title) = match self.page {
            Page::Sound => (SOUND_FRAME, SOUND_TITLE),
            Page::Camera => (CAMERA_FRAME, CAMERA_TITLE),
            _ => (CONTROLS_FRAME, CONTROLS_TITLE),
        };
        draw_window_page(out, a, frame, title);
        let colour = |row: usize| gui::rgba(if row == self.setting_row { SELECTED } else { OTHER });
        let word = |addr: u32| v.save.i32(Scope::Global, addr);
        let byte = |addr: u32| v.save.u8(Scope::Global, addr);
        let line = |label: u32, value: u32| [a.strings.get(label), b": ", a.strings.get(value)].concat();
        // Font 2 (`0x2ea608` centred, `0x2ea4d8` right-aligned), font 1 (`0x2ea570` centred).
        let font2 = |out: &mut Vec<Quad>, t: &[u8], x: f32, y: f32, c: [u8; 4], right: bool| {
            let w = gui::text_width(&a.glyphs_2, t, 1.0);
            let x = if right { x - w as f32 } else { x - (w >> 1) as f32 };
            gui::text(out, &a.glyphs_2, 2, t, x, y, 1.0, c);
        };
        match self.page {
            Page::Sound => {
                let at = options::TOGGLE_AT;
                for (row, (label, addr)) in [(SFX, opt::SFX), (MUSIC, opt::MUSIC)].into_iter().enumerate() {
                    let y = SOUND_ROWS[row];
                    font2(out, &[a.strings.get(label), b": "].concat(), 256.0, y, colour(row), true);
                    // `0x42d360`: the bar, then the value's part of it.
                    let (x0, y0) = (at[0] + SLIDER_AT[0], at[1] + SLIDER_AT[1 + row]);
                    let [w, h] = SLIDER;
                    gui::rect(out, y0 as i32, (y0 + h) as i32, x0 as i32, (x0 + w) as i32, gui::rgba(SLIDER_BACK));
                    let fill = word(addr).clamp(0, 0x400) as f32 / 1024.0 * w;
                    gui::rect(out, y0 as i32, (y0 + h) as i32, x0 as i32, (x0 + fill) as i32, gui::rgba(SLIDER_FILL));
                }
                let mode = MONO_STEREO[(word(opt::STEREO) != 0) as usize];
                font2(out, &line(STEREO, mode), 256.0, SOUND_ROWS[2], colour(2), false);
            }
            Page::Camera => {
                let rows = [
                    line(UP_DOWN, REVERSED_NORMAL[(word(opt::PITCH_NORMAL) != 0) as usize]),
                    line(LEFT_RIGHT, REVERSED_NORMAL[(word(opt::YAW_NORMAL) != 0) as usize]),
                    line(SPEED, SPEEDS[word(opt::CAM_SPEED).clamp(0, 2) as usize]),
                    line(0x307a, if v.port.get(Setting::Aspect) != 0 { options::ON } else { options::OFF }),
                ];
                for (row, t) in rows.iter().enumerate() { font2(out, t, 256.0, CAMERA_ROWS[row], colour(row), false); }
            }
            _ => {
                let rows = [
                    line(VIBRATION, if byte(opt::VIBRATION) != 0 { options::ON } else { options::OFF }),
                    line(QS_PAUSE, if byte(opt::QS_PAUSE) != 0 { options::ON } else { options::OFF }),
                ];
                for (row, t) in rows.iter().enumerate() {
                    let w = gui::text_width(&a.glyphs, t, 1.0);
                    gui::text(out, &a.glyphs, 1, t, 256.0 - (w >> 1) as f32, CONTROLS_ROWS[row], 1.0, colour(row));
                }
            }
        }
    }

    /// The pause list (`0x43fc18`, button rows): per row the main menu's two button sprites (keyframe 0.37; the
    /// selected frame pulsing `0x20fffeed`..`0x70fffeed`) and the label centred 11 below (font 1: grey when
    /// disabled, yellow when selected); the rows centred on the screen.
    fn draw_pause(&self, out: &mut Vec<Quad>, a: &Assets) {
        use pause::*;
        let rows = pause_rows();
        let n = rows.len() as f32;
        let y0 = (208.0 - (ROW_HEIGHT + 0.5).floor() * n * 0.5) as i32 as f32;
        let k = self.pulse % 40;
        let t = if k < 20 { k as f32 / 20.0 } else { 1.0 - (k - 20) as f32 / 20.0 };
        for (i, &row) in rows.iter().enumerate() {
            let y = y0 + BUTTON[1] + BUTTON[2] * i as f32;
            let at = [AT[0] + BUTTON[0], y];
            if let Some(s) = a.sprites.get(&FILL.0) { gui::sprite(out, s, at, BUTTON_KEY, [1.0, 1.0], gui::rgba(FILL.1)); }
            let frame = if i == self.pause_row { gui::lerp_colour(t, 0x70ff_feed, 0x20ff_feed) } else { gui::rgba(0x55f0_c070) };
            if let Some(s) = a.sprites.get(&FRAME) { gui::sprite(out, s, at, BUTTON_KEY, [1.0, 1.0], frame); }
            let colour = if !pause_enabled(row) { 0x8080_8080 } else if i == self.pause_row { 0x70ff_feed } else { 0x80f0_f0f0 };
            let label: &[u8] = if row == PauseRow::Port { b"Port Options" } else { a.strings.get(row.text()) };
            let w = gui::text_width(&a.glyphs, label, 1.0);
            gui::text(out, &a.glyphs, 1, label, AT[0] + LABEL[0] - (w >> 1) as f32, y0 + LABEL[1] + LABEL[2] * i as f32, 1.0, gui::rgba(colour));
        }
    }

    /// The in-game Options list (`0x438b48`): the frame (sprites 0x91..0x94), the selection (0x95), the title, the
    /// labels (the front end's Options list's places) and the row icons.
    fn draw_pause_options(&self, out: &mut Vec<Quad>, a: &Assets, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) {
        use options::{ICONS_AT, ICON_RGBA, LABELS_AT, SELECTION, SELECTION_AT};
        use pause::*;
        let at = OPTIONS_AT;
        for (id, c, key) in OPTIONS_SPRITES {
            if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, at, key, [1.0, 1.0], gui::rgba(c)); }
        }
        if let Some(s) = a.sprites.get(&SELECTION) {
            let p = [at[0] + SELECTION_AT[0], at[1] + SELECTION_AT[1] + SELECTION_AT[2] * self.options_row as f32];
            gui::sprite(out, s, p, 0.0, [1.0, 1.05], self.pulse_colour());
        }
        let centred = |out: &mut Vec<Quad>, t: &[u8], x: f32, y: f32, c: u32| {
            let w = gui::text_width(&a.glyphs, t, 1.0);
            gui::text(out, &a.glyphs, 1, t, x - (w >> 1) as f32, y, 1.0, gui::rgba(c));
        };
        centred(out, a.strings.get(OPTIONS_TEXT), at[0] + OPTIONS_TITLE[0], at[1] + OPTIONS_TITLE[1], 0x80f0_f0f0);
        for (i, o) in PauseOption::ALL.iter().enumerate() {
            let c = if i == self.options_row { 0x70ff_feed } else { 0x80f0_f0f0 };
            centred(out, a.strings.get(o.text()), at[0] + LABELS_AT[0], at[1] + LABELS_AT[1] + LABELS_AT[2] * i as f32, c);
            let (x, y) = (at[0] + ICONS_AT[0], at[1] + ICONS_AT[1] + ICONS_AT[2] * i as f32);
            if let Some((tex, [tw, th])) = icon(OPTIONS_ICON, i as u16) {
                let rgba = gui::rgba(if i == self.options_row { 0x70ff_feed } else { ICON_RGBA });
                out.push(Quad { pos: [[x, y], [x + 32.0, y], [x, y + 32.0], [x + 32.0, y + 32.0]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], tex, rgba });
            }
        }
    }

    /// The selection's colour (`0x2aa3f0(0x60442d00, 0x70fffeed, 20)`).
    fn pulse_colour(&self) -> [u8; 4] {
        let k = self.pulse % 40;
        let t = if k < 20 { k as f32 / 20.0 } else { 1.0 - (k - 20) as f32 / 20.0 };
        gui::lerp_colour(t, 0x70ff_feed, 0x6044_2d00)
    }

    /// The Options list's frame (`0x343668`): the two frame sprites at the row count's keyframe and the selection.
    fn draw_list_frame(&self, out: &mut Vec<Quad>, a: &Assets, rows: usize, selected: usize) {
        use options::*;
        let key = KEYFRAME.get(rows.wrapping_sub(1)).copied().unwrap_or(0.0);
        for (id, c) in FRAMES {
            if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, AT, key, [1.0, 1.0], gui::rgba(c)); }
        }
        if let Some(s) = a.sprites.get(&SELECTION) {
            let at = [AT[0] + SELECTION_AT[0], AT[1] + SELECTION_AT[1] + SELECTION_AT[2] * selected as f32];
            gui::sprite(out, s, at, 0.0, [1.0, 1.05], self.pulse_colour());
        }
    }

    /// `0x343668`: the frame, the rows' labels (`0x348e70`: font 1 at the row's scale, centred; the selected one
    /// yellow; 16:9's 3 lower) and the rows' icons (32×32).
    fn draw_options(&self, out: &mut Vec<Quad>, a: &Assets, v: &View, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>) {
        use options::*;
        self.draw_list_frame(out, a, v.option_rows.len(), self.row);
        for (i, row) in v.option_rows.iter().enumerate() {
            let colour = gui::rgba(if i == self.row { 0x70ff_feed } else { 0x80f0_f0f0 });
            let y = AT[1] + LABELS_AT[1] + LABELS_AT[2] * i as f32;
            let (t, scale, dy): (&[u8], f32, f32) = match *row {
                OptionRow::Game { text, scale, .. } => (a.strings.get(text), scale, if text == 0x307a { 3.0 } else { 0.0 }),
                OptionRow::Port => (b"Port Options", 1.0, 0.0),
            };
            let w = gui::text_width(&a.glyphs, t, scale);
            gui::text(out, &a.glyphs, 1, t, AT[0] + LABELS_AT[0] - (w >> 1) as f32, y + dy, scale, colour);
            if let OptionRow::Game { icon: (id, f), .. } = *row {
                let (x, y) = (AT[0] + ICONS_AT[0], AT[1] + ICONS_AT[1] + ICONS_AT[2] * i as f32);
                if let Some((tex, [tw, th])) = icon(id, f) {
                    out.push(Quad { pos: [[x, y], [x + 32.0, y], [x, y + 32.0], [x + 32.0, y + 32.0]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], tex, rgba: gui::rgba(ICON_RGBA) });
                }
            }
        }
    }

    /// Port Options in the Options list's frame (five rows at a time): the title above, each row's label in the row's
    /// slot and its value in the frame's right panel, the selected row yellow.
    fn draw_port(&self, out: &mut Vec<Quad>, a: &Assets, v: &View) {
        use options::*;
        let shown = PORT_VISIBLE.min(ENTRIES.len());
        self.draw_list_frame(out, a, shown, self.port_row - self.port_top);
        let white = gui::rgba(0x80f0_f0f0);
        let title = b"Port Options";
        let w = gui::text_width(&a.glyphs, title, 1.0);
        gui::text(out, &a.glyphs, 1, title, 256.0 - (w >> 1) as f32, 52.0, 1.0, white);
        // The labels where the Options rows' are; the values centred in the frame's right panel.
        let centred = |out: &mut Vec<Quad>, t: &[u8], x: f32, y: f32, c: [u8; 4]| {
            let w = gui::text_width(&a.glyphs, t, 1.0);
            gui::text(out, &a.glyphs, 1, t, x - (w >> 1) as f32, y, 1.0, c);
        };
        for (k, e) in ENTRIES.iter().enumerate().skip(self.port_top).take(shown) {
            let colour = gui::rgba(if k == self.port_row { 0x70ff_feed } else { 0x80f0_f0f0 });
            let y = AT[1] + LABELS_AT[1] + LABELS_AT[2] * (k - self.port_top) as f32;
            centred(out, e.label.as_bytes(), AT[0] + LABELS_AT[0], y, colour);
            let value = e.values.get(v.port.values.get(k).copied().unwrap_or(0) as usize).copied().unwrap_or("");
            centred(out, value.as_bytes(), AT[0] + PORT_VALUE_X, y, colour);
        }
        for (id, at) in [(options::TOGGLE_TEXT, 309.0), (options::EXIT_TEXT, 335.0)] {
            let t = a.strings.get(id);
            let w = gui::text_width(&a.glyphs, t, 1.0);
            gui::text(out, &a.glyphs, 1, t, 256.0 - (w >> 1) as f32, at, 1.0, white);
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
        // `0x33a9f8`: the labels show unless the front end hides them (its New Game page).
        if self.page == Page::Load || (self.in_game && self.page == Page::Save) {
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

/// The pause list's rows on foot.
pub fn pause_rows() -> &'static [PauseRow] { &PauseRow::ALL }

/// A pause row can be taken: Special once its menu is unlocked (`0x359888`, not ported: never yet).
fn pause_enabled(r: PauseRow) -> bool { r != PauseRow::Special }

/// What the pages show besides their own state: the card, the save data's options, the port's settings, the Options
/// rows.
pub struct View<'a> {
    pub card: &'a MemCard,
    pub save: &'a SaveData,
    pub port: &'a PortValues,
    pub option_rows: &'a [OptionRow],
}

/// A toggle page's state: 16:9 is the port's aspect being wide; the others their option byte.
fn toggle_on(t: Toggle, cx: &Ctx) -> bool {
    if t == Toggle::Widescreen { cx.port.get(Setting::Aspect) != 0 } else { cx.save.u8(Scope::Global, t.byte()) != 0 }
}

fn toggle_shown(t: Toggle, v: &View) -> bool {
    if t == Toggle::Widescreen { v.port.get(Setting::Aspect) != 0 } else { v.save.u8(Scope::Global, t.byte()) != 0 }
}

/// A toggle page (`0x33d3c0` …): the window (`0x33bf90`: sprites 0x8b..0x8d at the page's keyframe, the title,
/// "✕ Toggle", "△ Exit"), then "<setting>: on / off" centred at (256, 170) in font 1.
fn draw_toggle(out: &mut Vec<Quad>, a: &Assets, t: Toggle, on: bool) {
    use options::*;
    for (id, c) in TOGGLE_SPRITES {
        if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, TOGGLE_AT, t.frame(), [1.0, 1.0], gui::rgba(c)); }
    }
    let centred = |out: &mut Vec<Quad>, text: &[u8], x: f32, y: f32, c: u32| {
        let w = gui::text_width(&a.glyphs, text, 1.0);
        gui::text(out, &a.glyphs, 1, text, x - (w >> 1) as f32, y, 1.0, gui::rgba(c));
    };
    for (id, [dx, dy]) in [(t.title(), TOGGLE_TITLE), (TOGGLE_TEXT, TOGGLE_TOGGLE), (EXIT_TEXT, TOGGLE_EXIT)] {
        centred(out, a.strings.get(id), TOGGLE_AT[0] + dx, TOGGLE_AT[1] + dy, 0x80f0_f0f0);
    }
    let line = [a.strings.get(t.text()), b": ", a.strings.get(if on { ON } else { OFF })].concat();
    centred(out, &line, 256.0, 170.0, if t == Toggle::Subtitles { 0x80d0_d0d0 } else { 0x80f0_f0f0 });
}

/// A setting page's window (`0x431fa8`): sprites 0x8b..0x8d at (255, 195) at the page's keyframe, the title,
/// "✕ Toggle" and "△ Exit".
fn draw_window_page(out: &mut Vec<Quad>, a: &Assets, frame: f32, title: u32) {
    use options::*;
    for (id, c) in TOGGLE_SPRITES {
        if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, TOGGLE_AT, frame, [1.0, 1.0], gui::rgba(c)); }
    }
    for (id, [dx, dy]) in [(title, TOGGLE_TITLE), (TOGGLE_TEXT, TOGGLE_TOGGLE), (EXIT_TEXT, TOGGLE_EXIT)] {
        let t = a.strings.get(id);
        let w = gui::text_width(&a.glyphs, t, 1.0);
        gui::text(out, &a.glyphs, 1, t, TOGGLE_AT[0] + dx - (w >> 1) as f32, TOGGLE_AT[1] + dy, 1.0, gui::rgba(0x80f0_f0f0));
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
