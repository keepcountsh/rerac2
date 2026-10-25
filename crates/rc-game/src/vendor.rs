//! The Megacorp vendor's menu (game mode 5, L00) and the rules of what it sells. Three parts of the game make it.
//!
//! **The stock** (`0x1a7b70`, 32 bytes of the save, 0xff free: an item in the low six bits, 0x40 when it was owned
//! as it went in). `0x2f47c0(item)` puts a weapon in (its record's `−0x10` set, `−0x0c` 0, a price or an ammo
//! price): the slot that holds it already, else the first free one. Every level start (`0x2fcf60`) gives the
//! starting items (`0x2f46e0`, each into the stock), drops the slots whose item no planet sells (`0x2fceb8`:
//! [`GameState::stock_clean`]) and adds the planet's weapons (`0x2fd028`: every item `0x2f4aa0` says the level
//! sells: the table `0x268d10` of `{item, first level}`, [`STOCK_LEVELS`], on the levels below 0x15; item 9 in the
//! challenge mode `0x1a7a12`). The list a vendor shows ([`build_list`], `0x3724c8`) walks the stock in order: an
//! item not owned is a weapon for sale, an owned one with an ammo price its ammo; then the ammo of every owned
//! item with an ammo price not listed yet; in the challenge mode a Mega upgrade of each owned weapon whose current
//! row has none (and no experience to earn). Barlow's Gadgetron vendor (the moby's kind 3) lists instead the R&C1
//! weapons 0xc, 0xe, 0x11, 0x12, 0x35 and 0x2c (owned ones as ammo). Duplicates go.
//!
//! **The menu** (opened by `0x32c5f0(5, 1, vendor)` → `0x374038(vendor)`; its kind from the moby's pvar `+0x94`:
//! 0 weapons, 1 mods, 2 armour, 3 the Gadgetron weapons, 4 the ship; classes 0xddc / 0x1309 are the mod and ship
//! vendors). The open: the list, the selection in its middle, the strip's offset, sound 3, the bolt counter up
//! (`0x30d910(18000000)`), Ratchet in state 0x61 with the items locked (`0x18c0d5`, `0x18c0dc`) and hidden
//! (`0x2d3208`), the help panel closed (`0x2f5310`), the camera at the vendor's rows · (3.8, 0, 1.5) — (3.8, 0,
//! 1.133) in 16:9 — facing back at it (yaw + π; [`Vendor::camera`]). The frame `0x375f40`: substate 0 fades the
//! frame in (0.1 a tick), 1 runs the menu, 2 fades out and leaves (`0x32c780(0, 0)` → `0x374628`: state 0, the
//! items back, Ratchet put 3.5 in front of the vendor facing it (`0x2b9e68`), the follow camera reset (`0x4289f0(2)`),
//! the vendor's state 1). The menu (`0x375ad8`): ← / → (auto-repeat past 15 held frames every tenth) move the
//! selection with sound 1 and slide the strip 0x38; ✕ asks to buy (sound 0; sound 2 refused: a weapon too dear, ammo
//! full or its unit too dear); △ leaves (`0x375820`: sound 5, the frame fading out). **The buy** (`0x3747e8`, by the
//! popup state `0x22f7c4`): 1 sets the quantity (the bolts over the unit price, at most the room) and asks — "How
//! many?" for ammo, "Purchase?" for a weapon, else why not; 2 reads ← / → on the quantity (auto-repeat past 15
//! held every eighth, past 47 every frame), ✕ confirms and △ cancels; 4 pays: ammo `0x2d4358` (the overflow
//! given back), a weapon `0x2f46e0` (owned, in the stock as owned, its hand request `0x3227d8` unless 0x1f / 0x3d),
//! "THANK YOU" on the ticker and sound 7, the list rebuilt; too few bolts: "You can't afford that", sound 0.
//!
//! **The draw** (`0x37ba50`): the frame's four GUI sprites 0x67..0x6a at the screen's centre, scaled 0.8 → 1 as it
//! fades in (1 → 1.1 out), their alphas 0 → 0x60 / 0x60 / 0x55; then (once faded in) the selection's name and cost
//! (`0x3781c8`), the buttons (`0x379000`), the question box (`0x379c50`), the strip of icons (`0x376d40`: 40 × 40,
//! 0x38 apart, the selection at x 256 over a pulsing box, fading 0.75 alpha a pixel from the centre, "Ammo" /
//! "Mega" under the ammo and Mega entries), the ticker (`0x376a58`: the selection's description, or an idle
//! Megacorp line, 50 spaces ahead, scrolling 2 pixels a frame in the vendor's own glyph drawer `0x376638` at 1.5),
//! and the preview: the weapon's hand moby (an ammo entry: the crate 0xcdb) floating at the vendor's rows · its row
//! of `0x28d8c0` ([`PREVIEW`]: rotation, offset, scale; `(−10, 0, 1.9)` × 3.5 for ammo), turning 0.01 a tick,
//! fading in 3 alpha a tick, out 0x10 (scaled by 0.9) when the selection moves, rebuilt once the strip has settled
//! ([`Vendor::preview`]). Texts in font 2 (`0x2ea608` / `0x2e9f68` / `0x2ea4d8` with `0x2e71b8(2)`).
//!
//! **The armour vendor** (kind 2, mode 3): the list `0x373078` offers the levels past the one worn (`0x1a7a20`;
//! 1 Tetrafiber .. 4 Carbonox) that the planet's byte `0x139645` allows (every one in the challenge mode), priced
//! `0x1ad628` ([`armor::PRICES`]); the buy `0x375068` asks "Purchase?" (sound 2 when the bolts fall short), and a
//! purchase ("THANK YOU", sound 7) leaves the menu at once: `0x375820` then sets the armour, applies it to Ratchet
//! (`0x2fecd0`) and announces it ("You got the … Armor!", `0x30efb8`). Its draws: the name and cost (`0x378678`),
//! the buttons (`0x379560`), the question (`0x37a050`), the strip of armour icons on the left (`0x377948`).
//! **The mod vendor** (kind 1, class 0xddc, mode 1): the list `0x373268` first marks the mods this planet sells
//! (`0x28e8c0`, [`mods::PLANETS`]; every mod a record prices, in the challenge mode) in `0x1a7ac0`, then lists each
//! owned weapon's marked mods not yet bought (`0x1a7a88`), priced in platinum bolts (the record's `+0x88..+0x90`);
//! the buy `0x375270` pays platinum bolts (`0x327c20`: found − spent, at most 40; `0x327bc8`) and sets the bought
//! bits. Its draws: `0x3788c8` (the title "Mod for Weapon", the cost and the platinum bolts had), `0x379738`
//! (the buttons, which test the *bolts* against the price: the game's), `0x37a218` (the question), `0x377338`
//! (the strip: the weapon's icon with the mod's icon 0xeaa3 over it).
//!
//! Not ported: the ship vendor (kind 4, class 0x1309, mode 2: `0x3735f8` / `0x375670` and its draws: the port has
//! no ship upgrades, paint jobs or skill points); the armour vendor's Ratchet preview (`0x37a8e8`'s mode 3: his
//! model in the armour, the helmet 0x50a and the boots 0xa73 posed on it) and the armour's change on him until the
//! next level load (the engine attaches his armour row then); the mods' effect on the weapons (the port's weapons
//! have none); the Mega purchase's row switch (`0x2f4bf8`: the port keeps no
//! upgrade rows; the challenge mode never runs); the Gadgetron vendor's R&C1-save talk (`0x37a5d0`, the free
//! weapons `0x37a518` of `0x1a7a28`); the items 0x17 / 0x19's help after a purchase (`0x2f6638`); the preview's
//! draw over the panel (`0x37ba50` draws it after the frame sprites and the HUD, clipped to 158..360 × 115..206:
//! the port's preview is a world moby under the HUD layer, so the centre box covers it until the engine draws
//! mobys over the HUD), the class load wait (`0x306258` / `0x3062d0`: the engine loads the weapons the level
//! sells with it) and the world snapshot the menu draws over (the world keeps rendering, frozen); the
//! PDA's vendor (`0x374038` with a kind, no moby); PAL's vertical scale `0x1b2560`; the dead voice line
//! `0x22f7b8`; the icons' and glyphs' left-to-right alpha gradient (one alpha each here, the mean of the sides).

use crate::front_end::Assets;
use crate::game::GameState;
use crate::gui::{self, text_box, Quad, Tex, TextBox};
use crate::pad::{button, PadState};
use rc_formats::disc::boot::Glyph;
use rc_formats::disc::menu_tables::ItemRecord;

/// The vendor's moby class, the hologram's (`0x477`) and the ammo preview's crate (`0xcdb`).
pub const CLASS: i16 = 0xb;
pub const HOLOGRAM_CLASS: i16 = 0x477;
pub const AMMO_PREVIEW_CLASS: i16 = 0xcdb;
/// The stock's slots (`0x1a7b70..0x1a7b90`).
pub const STOCK_SLOTS: usize = 0x20;
/// The last level with a weapon stock (`0x2f4aa0`: levels below 0x15).
pub const LAST_STOCK_LEVEL: i32 = 0x15;
/// `0x268d10`: `{item, the first level it is sold on}`, −2 ends.
pub const STOCK_LEVELS: [(i32, i32); 16] = [
    (0x16, 1), (0x17, 3), (0x18, 4), (0x1a, 1), (0x1b, 9), (0x1c, 11), (0x1d, 8), (0x1e, 0), (0x1f, 6), (0x20, 11), (0x29, 3), (0x2a, 0),
    (0x2b, 14), (0x2d, 14), (0x25, 8), (0x19, 12),
];
/// The R&C1 weapons (`0x3724c8`'s cases): their Mega row is their second, not their third.
pub const SPECIALS: [i32; 5] = [0xc, 0xe, 0x11, 0x12, 0x35];
/// The Gadgetron vendor's list.
pub const GADGETRON_ITEMS: [i32; 6] = [0xc, 0xe, 0x11, 0x12, 0x35, 0x2c];
/// The prompt lease's owner (`0x327ae0(6, 0, 1)` every frame).
pub const PROMPT_OWNER: i32 = 6;
/// The bolt icon (`0x7567`).
pub const BOLT_ICON: u16 = 0x7567;

/// Texts.
pub mod text {
    pub const THANK_YOU: u32 = 0x2c61;
    pub const CANT_AFFORD_THAT: u32 = 0x2c62;
    pub const AMMO: u32 = 0x2c5f;
    pub const MEGA: u32 = 0x30f5;
    pub const BACK: u32 = 0x31b0;
    pub const BUY: u32 = 0x31b1;
    pub const MAXED_OUT: u32 = 0x31b2;
    pub const HOW_MANY: u32 = 0x31b3;
    pub const CANT_AFFORD_IT: u32 = 0x31b4;
    pub const PURCHASE: u32 = 0x31b5;
    pub const NO: u32 = 0x31c4;
    pub const YES: u32 = 0x31c7;
    pub const COST: u32 = 0x31da;
    pub const EXIT: u32 = 0x2be5;
    /// The idle ticker: "Megacorp Weapons Emporium" (`gp−0x18e0`) and the six lines `gp−0x18d8` (a random draw
    /// of 12: 0..5 a line, else the title).
    pub const EMPORIUM: u32 = 0x31d9;
    pub const LINES: [u32; 6] = [0x2e5f, 0x2e60, 0x2e61, 0x2e62, 0x2e63, 0x2e64];
}

/// The vendor's class sounds (`0x3605f0(n, 0x11, vendor)`).
pub mod sound {
    pub const SELECT: i32 = 0;
    pub const CURSOR: i32 = 1;
    pub const DENIED: i32 = 2;
    pub const OPEN: i32 = 3;
    pub const EXIT: i32 = 5;
    pub const PURCHASE: i32 = 7;
}

/// The frame's GUI sprites (`0x37ba50`, mode 0) and their colours.
const FRAME_SPRITES: [u32; 4] = [0x67, 0x68, 0x69, 0x6a];
const FRAME_RGB: [u32; 4] = [0x442d00, 0x241700, 0xf0c070, 0xf0c070];
/// The alphas the four sprites fade to (the third and fourth share one).
const FRAME_ALPHA: [i32; 3] = [0x60, 0x60, 0x55];
const SCREEN_CENTRE: [f32; 2] = [256.0, 208.0];
/// The strip (`0x376d40`): the icons' size, pitch, the selection's x, the highlight box and the labels.
const STRIP_Y: f32 = 329.0;
const STRIP_ICON: f32 = 40.0;
const STRIP_PITCH: i32 = 0x38;
const STRIP_FADE: f32 = 0.75;
const LABEL_Y: f32 = 366.0;
/// The camera (`gp−0x1a50` / `gp−0x1a40`) in the vendor's rows, the exit's place (`gp−0x1a30`) and the preview's
/// ammo offset (`gp−0x1a60`).
pub const CAMERA_43: [f32; 3] = [3.8, 0.0, 1.5];
pub const CAMERA_169: [f32; 3] = [3.8, 0.0, 1.133];
pub const EXIT_AT: [f32; 3] = [3.5, 0.0, 0.0];
const AMMO_PREVIEW_AT: [f32; 3] = [-10.0, 0.0, 1.9];
const AMMO_PREVIEW_SCALE: f32 = 3.5;
/// 16:9 lowers the preview 0.55.
const WIDESCREEN_DROP: f32 = 0.55;
/// `0x37ca08(0x9e, 0x168, 0x73, 0xce)`: the window the preview is drawn in (game pixels, inclusive).
pub const PREVIEW_WINDOW: [i32; 4] = [0x9e, 0x168, 0x73, 0xce];

/// The armour vendor's tables.
pub mod armor {
    /// The levels (0 the Commando Suit, 1..4 the armours).
    pub const MAX: i32 = 4;
    /// `0x1ad628`: the prices by level.
    pub const PRICES: [i32; 5] = [0, 25_000, 100_000, 250_000, 1_000_000];
    /// `0x1a8eb0` (`0x2f4bb0`): the names by level.
    pub const NAMES: [u32; 5] = [0x331d, 0x3318, 0x3319, 0x331a, 0x331b];
    /// `0x1ad6c8`: "You got the … Armor!" by level.
    pub const GOT: [u32; 5] = [0, 0x129b, 0x129c, 0x129d, 0x129e];
    /// `0x1ad610` (`gp−0x19e0`): the descriptions by level.
    pub const DESCRIPTIONS: [u32; 5] = [0, 0x2e68, 0x2e69, 0x2e6a, 0x2e6b];
    /// The icons: `0xea92 + level`.
    pub const ICON: u16 = 0xea92;
    /// "Can't buy any armor".
    pub const NONE: u32 = 0x301e;
}

/// The mod vendor's tables.
pub mod mods {
    /// `0x1a8e40` / `0x1a8e50`: the mods' names and descriptions by slot (Shock, Acid, Lock On).
    pub const NAMES: [u32; 3] = [0x3065, 0x3064, 0x3066];
    pub const DESCRIPTIONS: [u32; 3] = [0x2e66, 0x2e65, 0x2e67];
    /// The mods' icon (frame = the slot; frame 3 over it) and the platinum bolt's.
    pub const ICON: u16 = 0xeaa3;
    pub const PLATINUM_ICON: u16 = 0xeaa2;
    /// "Platinum", "You've got:".
    pub const PLATINUM: u32 = 0x30f8;
    pub const YOU_HAVE: u32 = 0x2dc3;
    /// `0x327c20`: the platinum bolts that can be spent, at most 40.
    pub const MAX_PLATINUM: i32 = 40;
    /// `0x28e8c0` (`0x3731e0`): per item, the planet each mod slot is first sold on (−1 never).
    pub const PLANETS: [(i32, [i32; 3]); 14] = [
        (0x1e, [3, -1, 8]), (0x2a, [-1, 3, 3]), (0x16, [-1, 8, 3]), (0x1a, [-1, 8, 3]), (0x17, [8, -1, -1]), (0x29, [8, -1, -1]), (0x18, [3, -1, 3]),
        (0x2c, [-1, -1, 13]), (0x1d, [-1, 13, 13]), (0x25, [-1, 13, 8]), (0x1b, [-1, 8, 13]), (0x1c, [-1, 13, 13]), (0x20, [-1, 13, -1]), (0x19, [-1, 13, 13]),
    ];
    /// `0x1ad570` (`gp−0x1a80`): by the entry index, 2 asks for the help text 0xa01 in slot 0x58 after a purchase
    /// (`0x2f6688`; never 2 in this build).
    pub const HELP: [i32; 4] = [3, 3, 0, 0];
}

/// The frame's GUI sprites by menu (`0x37ba50`: the mod menu draws three).
const ARMOR_SPRITES: [u32; 4] = [0x75, 0x76, 0x77, 0x78];
const MODS_SPRITES: [u32; 3] = [0x79, 0x7a, 0x7b];
/// The vendor's light (`0x374038`'s write of the light bank's set 14, `0x1c6e40`: the preview's light word
/// `0x0e0e`): colour A `gp−0x1a10` (w the back-face factor), direction A `gp−0x1a20` in the vendor's rows, light B
/// dark.
pub const LIGHT_SET: usize = 0xe;
pub const LIGHT_COLOUR: [f32; 4] = [0.9, 0.9, 0.6, -0.25];
pub const LIGHT_DIR: [f32; 3] = [-0.42, -0.7, -0.577];

/// The light set the open writes for a vendor with rows `rows`: `[colour A, direction A, colour B, direction B]`.
pub fn light_set(rows: [[f32; 3]; 3]) -> [[f32; 4]; 4] {
    let d: [f32; 3] = std::array::from_fn(|k| rows[0][k] * LIGHT_DIR[0] + rows[1][k] * LIGHT_DIR[1] + rows[2][k] * LIGHT_DIR[2]);
    [LIGHT_COLOUR, [d[0], d[1], d[2], 0.0], [0.0; 4], [0.0; 4]]
}

/// `0x28d8c0`: per item, `[rotation (x, y, z, 0), (offset x, y, z, scale)]` for its base row and again for an
/// upgraded one (the record's `−0x0c` ≠ 0).
pub const PREVIEW: [[[f32; 4]; 4]; 0x38] = [
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, 0.1, 2.8, 9.0], [0.0, 0.0, 0.0, 0.0], [-10.0, 0.1, 2.6, 3.3]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.8, 4.4], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.8, 4.4]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.03, 2.72, 3.3], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.03, 2.72, 3.3]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.6, 3.1], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.6, 2.8]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.9, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.9, 3.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.65, 2.6], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.65, 2.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.5, 7.0], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.5, 7.0]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.45, 2.9], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.45, 2.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 3.3, 2.2], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 3.3, 2.2]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.67, 3.0], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.67, 2.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.0], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.5, 2.5]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.4, 2.5], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.4, 2.5]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 2.3, 2.85], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 2.2, 2.5]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 2.4, 2.6], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.1, 2.6, 2.75]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.6, 2.8], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 3.0, 2.8]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.0, 3.4], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.0, 3.4]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.35, 4.8], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.35, 4.8]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.5, 3.6], [0.0, 1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 3.68, 4.5], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 3.68, 4.5]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.35, 2.3], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.35, 2.3]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.75, 3.3], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 2.75, 3.3]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.6, 4.9], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.6, 4.9]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.4, 2.4], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.2, 2.3]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 3.35, 2.3], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, -0.0, 3.35, 2.3]],
    [[0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.3, 2.35], [0.0, -1.570_796_4, 0.0, 0.0], [-10.0, 0.2, 2.3, 2.35]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, 0.0, 2.8, 3.4], [0.0, 0.0, 0.0, 0.0], [-10.0, 0.0, 2.8, 2.8]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.7, 4.0], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.0, 2.7, 4.0]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
    [[0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6], [0.0, 0.0, 0.0, 0.0], [-10.0, -0.1, 2.5, 3.6]],
];

/// The icons' colours by the record's class (`0x42ddb0`: `gp−0x11d8`; a Mega entry draws as class 2).
const ICON_COLOURS: [u32; 4] = [0x60f0_f0b0, 0x6029_a1ff, 0x6040_d7d7, 0x60f0_8040];
const TEXT_WHITE: u32 = 0x80f0_f0f0;
const TEXT_RED: u32 = 0x8020_20f0;
const BOLT_ICON_RGBA: u32 = 0x60f0_f0b0;
const TICKER_RGB: u32 = 0xf0c070;
/// The ticker's lead (`0x1ad678`: 50 spaces), its scroll a frame and the width a character counts for.
const TICKER_LEAD: usize = 50;
const TICKER_STEP: i32 = 2;
const TICKER_CHAR: i32 = 0x14;

/// The vendor's kind (the moby's pvar `+0x94`): what the menu sells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Weapons,
    Mods,
    Armor,
    Ship,
}

impl Kind {
    /// `0x374038`'s read of class 0xb's pvar: 0 weapons, 1 mods, 2 armour, 3 the Gadgetron weapons, 4 the ship.
    pub fn of_pvar(kind: i32) -> (Kind, bool) {
        match kind {
            1 => (Kind::Mods, false),
            2 => (Kind::Armor, false),
            3 => (Kind::Weapons, true),
            4 => (Kind::Ship, false),
            _ => (Kind::Weapons, false),
        }
    }
}

/// One entry of the list (`0x22f880`, 0x18 bytes: the item, whether its ammo (`+4`), its hand class (`+8`), a free
/// one (`+0x10`), a Mega upgrade (`+0x14`)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub item: i32,
    pub ammo: bool,
    pub mega: bool,
    pub class: i32,
    pub free: bool,
    /// The armour menu's level and price (`0x22f888` / `0x22f884`, 0xc a row) and icon (`0x22f880`).
    pub level: i32,
    pub price: i32,
    pub icon: u16,
    /// The mod menu's slot (`0x22f880`, 0x10 a row: with `item` the weapon and `price` in platinum bolts).
    pub slot: i32,
}

/// Item `item`'s row (`0x139568[item]`: the port keeps no upgrade rows, so the item's own).
fn record(records: &[ItemRecord], item: i32) -> Option<&ItemRecord> { usize::try_from(item).ok().and_then(|i| records.get(i)) }

/// `0x2f4338(item)`: the upgrades past the base row (the `+0x3c` chain down to the base, the `+0x3a` chain up).
pub fn upgrades(records: &[ItemRecord], item: i32) -> i32 {
    let mut row = item.max(0) as usize;
    for _ in 0..0x60 {
        match records.get(row).map(|r| r.base) {
            Some(b) if b != 0 => row = b as usize,
            _ => break,
        }
    }
    let mut n = 0;
    for _ in 0..0x60 {
        match records.get(row).map(|r| r.next) {
            Some(x) if x != 0 => {
                row = x as usize;
                n += 1;
            }
            _ => break,
        }
    }
    n
}

/// `0x2f4cb8(_, item, n)`: the row `n` upgrades past the base, when its class byte is `n`.
pub fn record_at(records: &[ItemRecord], item: i32, n: i32) -> Option<&ItemRecord> {
    if n >= 0xff { return None; }
    let mut row = item.max(0) as usize;
    for _ in 0..0x60 {
        match records.get(row).map(|r| r.base) {
            Some(b) if b != 0 => row = b as usize,
            _ => break,
        }
    }
    for _ in 0..n {
        match records.get(row).map(|r| r.next) {
            Some(x) if x != 0 => row = x as usize,
            _ => break,
        }
    }
    records.get(row).filter(|r| r.class as i32 == n)
}

/// The Mega row of `item`: the second for the R&C1 weapons, else the third (`0x3724c8`'s cases).
fn mega_level(item: i32) -> i32 { if SPECIALS.contains(&item) { 1 } else { 2 } }

/// `0x3724c8`'s test of a record for a Mega upgrade to sell: a vendor record (`−0x10`) whose upgrade count (or
/// class byte, for the current row) reaches the Mega level.
fn reaches_mega(item: i32, level: i32) -> bool { 2 <= level || (level == 1 && SPECIALS.contains(&item)) }

impl GameState {
    /// The stock's used slots (up to the first free one).
    pub fn stock_len(&self) -> usize { self.stock.iter().position(|&b| b == 0xff).unwrap_or(STOCK_SLOTS) }

    /// `0x2f47c0(item)`: a weapon record (`−0x10` set, `−0x0c` 0, a price or an ammo price) into the stock — the
    /// slot that holds the item already, else the first free one (none: nothing) — with 0x40 when it is owned.
    pub fn stock_add(&mut self, item: i32, r: Option<&ItemRecord>) -> bool {
        let Some(r) = r else { return false };
        if !(r.mega && r.class == 0 && (r.price != 0 || r.ammo_price != 0)) { return false; }
        self.stock_add_raw(item)
    }

    /// [`stock_add`](Self::stock_add) without the record's test (`0x2f46e0` has no record here: the level start's
    /// clean drops what no planet sells, and the list reads only weapons).
    pub fn stock_add_raw(&mut self, item: i32) -> bool {
        if !(0..0x40).contains(&item) { return false; }
        let Some(k) = self.stock.iter().position(|&b| (b & 0x3f) as i32 == item || b == 0xff) else { return false };
        let owned = self.items.get(item as usize).is_some_and(|&b| b != 0);
        self.stock[k] = item as u8 | if owned { 0x40 } else { 0 };
        true
    }

    /// `0x2fceb8`: the slots whose item no planet sells (not in [`STOCK_LEVELS`]; the Lancer is never asked) and the
    /// item-0 slots freed. Whether none was.
    pub fn stock_clean(&mut self) -> bool {
        let mut clean = true;
        for b in self.stock.iter_mut() {
            if *b == 0xff { continue; }
            let item = (*b & 0x3f) as i32;
            if item == 0x1e { continue; }
            if !STOCK_LEVELS.iter().any(|&(i, _)| i == item) || item == 0 { *b = 0xff; }
            clean = false;
        }
        clean
    }

    /// `0x2f4aa0(item, level)`: `level` sells `item` — its first level reached, below [`LAST_STOCK_LEVEL`]; item 9 in
    /// the challenge mode.
    pub fn sold_on(&self, item: i32, level: i32) -> bool {
        if item == 9 && self.challenge { return true; }
        STOCK_LEVELS.iter().any(|&(i, first)| i == item && first <= level && level < LAST_STOCK_LEVEL)
    }

    /// `0x2fd028(level)`: every item the level sells into the stock (never item 0).
    pub fn stock_for_level(&mut self, level: i32, records: &[ItemRecord]) {
        for item in 1..0x38 {
            if self.sold_on(item, level) { self.stock_add(item, record(records, item)); }
        }
    }

    /// Item `item` is owned (`0x1a7b00`).
    fn owns(&self, item: i32) -> bool { self.items.get(item.max(0) as usize).is_some_and(|&b| b != 0) }

    /// `0x327c20`: the platinum bolts to spend (found − spent, 0..40).
    pub fn platinum(&self) -> i32 { (self.platinum_found.min(mods::MAX_PLATINUM) - self.platinum_spent as i32).clamp(0, mods::MAX_PLATINUM) }

    /// `0x327bc8(n)`: `n` platinum bolts spent when they are there.
    pub fn spend_platinum(&mut self, n: i32) -> bool {
        let can = n <= self.platinum_found.min(mods::MAX_PLATINUM);
        if can { self.platinum_spent = self.platinum_spent.wrapping_add(n as u8); }
        can
    }
}

/// `0x373078`: the armour menu's list — the levels past the one worn that the planet offers (`0x139645`'s bits;
/// every one in the challenge mode), each its price and icon.
pub fn armor_list(game: &GameState) -> Vec<Entry> {
    let first = if game.armor != 0 { game.armor as i32 + 1 } else { 1 };
    (first..=armor::MAX)
        .filter(|&l| (l < 8 && game.armor_offer >> l & 1 != 0) || game.challenge)
        .map(|l| Entry { level: l, price: armor::PRICES[l as usize], icon: armor::ICON + l as u16, ..Default::default() })
        .collect()
}

/// `0x3731e0(level, item, slot)`: the planet sells the mod.
fn mod_sold_on(level: i32, item: i32, slot: usize) -> bool { mods::PLANETS.iter().any(|&(i, p)| i == item && p[slot] == level) }

/// `0x373268`: the mod menu's list — first the mods this planet sells (or any a record prices, in the challenge
/// mode) marked on offer (`0x1a7ac0`), then each owned weapon's mods on offer, priced and not bought (`0x1a7a88`).
pub fn mods_list(game: &mut GameState, records: &[ItemRecord], level: i32) -> Vec<Entry> {
    for item in 0..0x38 {
        let Some(r) = record(records, item) else { continue };
        if !r.mega { continue; }
        for slot in 0..3 {
            if mod_sold_on(level, item, slot) || (r.mods[slot] != 0 && game.challenge) { game.mods_available[item as usize] |= 1 << slot; }
        }
    }
    let mut out = Vec::new();
    for item in 1..0x38 {
        let Some(r) = record(records, item) else { continue };
        if !(r.mega && game.owns(item)) { continue; }
        let on = game.mods_available[item as usize];
        for slot in 0..3 {
            if on & (1 << slot) == 0 || r.mods[slot] == 0 { continue; }
            if game.mods_bought[item as usize] & (1 << (2 * slot)) != 0 { continue; }
            out.push(Entry { item, slot: slot as i32, price: r.mods[slot], class: r.classes[0], ..Default::default() });
        }
    }
    out
}

/// `0x3724c8(ammo_only)`: the list (module docs). `gadgetron`: the Gadgetron vendor's (`0x22fe84`).
pub fn build_list(game: &mut GameState, records: &[ItemRecord], gadgetron: bool, ammo_only: bool) -> Vec<Entry> {
    // The challenge mode: every vendor record into the stock.
    if game.challenge {
        for item in (0..0x38).filter(|&i| i != 10) {
            if record(records, item).is_some_and(|r| r.mega) { game.stock_add(item, record(records, item)); }
        }
    }
    let mut out: Vec<Entry> = Vec::new();
    // A Mega entry of an owned weapon (its base row reaches the Mega level, its current row not, no experience
    // left to earn, the challenge mode on).
    let mega_entry = |game: &GameState, item: i32| -> Option<Entry> {
        let r = record(records, item)?;
        if !(r.mega && game.owns(item)) { return None; }
        if !reaches_mega(item, upgrades(records, item)) || item == 0x10 { return None; }
        if !r.mega { return None; }
        if reaches_mega(item, r.class as i32) { return None; }
        if 0 < r.xp_max || !game.challenge { return None; }
        Some(Entry { item, ammo: false, mega: true, class: r.classes[0], ..Default::default() })
    };
    let hand_class = |item: i32| record(records, item).map_or(0, |r| r.classes[0]);
    if !gadgetron {
        for k in 0..game.stock_len() {
            let b = game.stock[k];
            if b == 0xff { continue; }
            let item = (b & 0x3f) as i32;
            if game.owns(item) {
                if let Some(e) = mega_entry(game, item) { out.push(e); }
            }
            if b & 0x40 == 0 {
                if !ammo_only && !game.owns(item) {
                    let class = if item == 9 || item == 0x49 { 0x259 } else { hand_class(item) };
                    out.push(Entry { item, ammo: false, mega: false, class, ..Default::default() });
                }
            } else if record(records, item).is_some_and(|r| r.ammo_price != 0) {
                out.push(Entry { item, ammo: true, mega: false, class: hand_class(item), ..Default::default() });
            }
        }
        for item in 0..0x38 {
            let Some(r) = record(records, item) else { continue };
            if r.mega && game.owns(item) {
                if let Some(e) = mega_entry(game, item) {
                    // The item listed already as a Mega entry: nothing more for it.
                    if out.iter().find(|x| x.item == item).is_some_and(|x| x.mega) { continue; }
                    out.push(e);
                }
            }
            if game.owns(item) && r.ammo_price != 0 {
                // The item listed already as a plain entry: nothing more for it.
                if out.iter().find(|x| x.item == item).is_some_and(|x| !x.mega) { continue; }
                out.push(Entry { item, ammo: true, mega: false, class: hand_class(item), ..Default::default() });
            }
        }
    } else {
        for item in GADGETRON_ITEMS {
            let owned = game.owns(item);
            if !owned || record(records, item).is_some_and(|r| r.ammo_price != 0) {
                out.push(Entry { item, ammo: owned, mega: false, class: hand_class(item), ..Default::default() });
            }
        }
        // `0x37a518`: the R&C1 save's weapons (`0x1a7a28` bits 0..4: 0xc, 0x35, 0x11, 0xe, 0x12) are free.
        for (bit, item) in [(1, 0xc), (2, 0x35), (4, 0x11), (8, 0xe), (0x10, 0x12)] {
            if game.rac1_weapons & bit != 0 {
                for e in out.iter_mut().filter(|e| e.item == item) { e.free = true; }
            }
        }
        game.rac1_weapons &= !0x80;
    }
    // The duplicates (the same item, ammo and Mega flags) after the first go.
    let mut k = 0;
    while k < out.len() {
        let e = out[k];
        let mut j = k + 1;
        while j < out.len() {
            if out[j].item == e.item && out[j].ammo == e.ammo && out[j].mega == e.mega { out.remove(j); } else { j += 1; }
        }
        k += 1;
    }
    out
}

/// What a frame asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// A vendor class sound (`0x3605f0(n, 0x11, vendor)`).
    Sound(i32),
    /// `0x3227d8(item)`: the hand item asked for.
    Equip(i32),
    /// `0x374628`: the menu is over (mode 0).
    Exit,
    /// `0x2f6688` → `0x2f5270(text, slot)`: a help text asked for after a mod purchase.
    Help(i32, i32),
}

/// The preview moby as the frame wants it: its class, its place in the vendor's rows, its Euler angles, the scale
/// factor on the class's and its alpha (`+0x23`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewPose {
    pub class: i32,
    pub offset: [f32; 3],
    pub rot: [f32; 3],
    pub scale: f32,
    pub alpha: u8,
    /// The class's sequence 1 when it has more than one (`0x37b728`).
    pub anim: bool,
    /// `+0x7c`: the ammo crate's piece 0 drawn on top (`0x37a8e8`).
    pub bangles: u16,
}

/// The preview (`0x22f7c8` shown, `0x22f7d0` its fade, `0x22f7d4` its alpha, `0x22f7d8` its scale, `0x22f7dc`
/// placed, `0x22f7b0` the entry it was built for, `0x22f800` its offset, `0x22f76c + 0xf0` its angles).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Preview {
    shown: bool,
    /// 0 fading in, 1 out, 2 shown, 3 gone.
    fade: u8,
    alpha: i32,
    scale: f32,
    placed: bool,
    entry: usize,
    class: i32,
    offset: [f32; 3],
    rot: [f32; 3],
    anim: bool,
}

/// The menu's state (`0x22f740..`).
#[derive(Clone, Debug)]
pub struct Vendor {
    pub kind: Kind,
    pub gadgetron: bool,
    /// `0x22f740`: 0 fading in, 1 the menu, 2 fading out; `gp−0x1a00` the fade; `0x22f744` the frames in the menu.
    pub sub: u8,
    pub fade: f32,
    pub ticks: i32,
    pub entries: Vec<Entry>,
    /// `0x22f7c0` the selection; `0x22f7ac` the strip's offset.
    pub sel: usize,
    pub scroll: i32,
    /// `0x22f7c4`: the buy (0 none, 1 asked, 2 the question, 4 answered); `gp+0x3520` confirmed, `gp+0x3524` a
    /// quantity asked; `0x1b2528` the quantity.
    pub buy: u8,
    confirmed: bool,
    qty_mode: bool,
    pub qty: i32,
    /// `0x1b251c` / `0x1b2520`: frames ← / → held; `gp+0x3534`: the last frame's presses.
    held_l: i32,
    held_r: i32,
    prev_pressed: u32,
    /// `0x22f77c` the ticker's text; `0x22f7a4` its scroll.
    pub ticker: Vec<u8>,
    pub ticker_scroll: i32,
    preview: Preview,
    /// The frame sprites' scale (`gp−0x19fc` → `gp−0x19f8`) and alphas (`0x1b2564..` → `0x1b2570..`).
    scale: [f32; 2],
    alphas: [[i32; 3]; 2],
    /// 16:9 (`0x1a7bb9`).
    pub widescreen: bool,
    /// The armour menu: the level worn at the open (`gp−0x1a70`) and the one bought (`gp−0x1a74`, `gp−0x1a78`).
    armor_old: i16,
    armor_bought: Option<i32>,
    /// The planet (`0x1a79f8`, the mod menu's planet rule).
    level: i32,
}

impl Vendor {
    /// `0x374038(vendor)`'s menu part: the list, the selection in the middle (`0x22fe80 / 2`; the strip starts
    /// `(1 − sel) · 0x28` off with fewer than three entries), sound 3, the frame's fade from 0.8.
    pub fn open(kind: i32, game: &mut GameState, records: &[ItemRecord], widescreen: bool, out: &mut Vec<Out>) -> Vendor {
        let (kind, gadgetron) = Kind::of_pvar(kind);
        let entries = match kind {
            Kind::Weapons => build_list(game, records, gadgetron, false),
            Kind::Armor => armor_list(game),
            Kind::Mods => mods_list(game, records, game.level),
            Kind::Ship => Vec::new(),
        };
        let n = entries.len() as i32;
        let sel = n / 2;
        let base = if n < 3 { 1 } else { sel };
        out.push(Out::Sound(sound::OPEN));
        Vendor {
            kind,
            gadgetron,
            sub: 0,
            fade: 0.0,
            ticks: 0,
            entries,
            sel: sel as usize,
            scroll: (base - sel) * 0x28,
            buy: 0,
            confirmed: false,
            qty_mode: false,
            qty: 0,
            held_l: 0,
            held_r: 0,
            prev_pressed: 0,
            ticker: Vec::new(),
            ticker_scroll: 0,
            preview: Preview { fade: 3, ..Preview::default() },
            scale: [0.8, 1.0],
            alphas: [[0; 3], FRAME_ALPHA],
            widescreen,
            armor_old: game.armor,
            armor_bought: None,
            level: game.level,
        }
    }

    pub fn current(&self) -> Option<Entry> { self.entries.get(self.sel).copied() }

    /// `0x3723d0`: the selection's price — a Mega entry's row's, a free one 0, else the record's.
    pub fn price(&self, records: &[ItemRecord]) -> i32 {
        let Some(e) = self.current() else { return 0 };
        if e.mega {
            record_at(records, e.item, mega_level(e.item)).map_or(0, |r| r.price)
        } else if e.free {
            0
        } else {
            record(records, e.item).map_or(0, |r| r.price)
        }
    }

    fn ammo_of(game: &GameState, item: i32) -> i32 { game.ammo.get(item.max(0) as usize).copied().unwrap_or(0) }

    /// The camera's eye in the vendor's rows (`gp−0x1a50`, 16:9 `gp−0x1a40`).
    pub fn camera(&self) -> [f32; 3] { if self.widescreen { CAMERA_169 } else { CAMERA_43 } }

    /// `0x3747a8(text)`: the ticker shows `text` after its lead, from the start.
    pub fn set_ticker(&mut self, text: &[u8]) {
        self.ticker = vec![b' '; TICKER_LEAD];
        self.ticker.extend(text.iter().copied().take_while(|&c| c != 0));
        self.ticker_scroll = 0;
    }

    /// `0x37a8e8`'s mode-0 part: the preview of the selection — an ammo entry's crate, else the weapon's hand
    /// class (its description on the ticker), placed by [`PREVIEW`] (`0x37b728`, at once: the port has every
    /// class).
    fn rebuild_preview(&mut self, _game: &GameState, records: &[ItemRecord], texts: &dyn Fn(u32) -> Vec<u8>) {
        if self.kind == Kind::Armor {
            // Mode 3: the armour's description ("Can't buy any armor" without one); Ratchet's model in it is not ported.
            let text = match self.current().filter(|e| e.level != 0) {
                Some(e) => armor::DESCRIPTIONS[e.level.clamp(0, armor::MAX) as usize],
                None => armor::NONE,
            };
            self.set_ticker(&texts(text));
            self.preview.fade = 2;
            return;
        }
        let Some(e) = self.current() else { return };
        let drop = if self.widescreen { WIDESCREEN_DROP } else { 0.0 };
        if self.kind == Kind::Mods {
            // Mode 1: the mod's description and the weapon's hand moby (`0x37b728` places it by the weapon).
            self.set_ticker(&texts(mods::DESCRIPTIONS[(e.slot.clamp(0, 2)) as usize]));
            let p = &mut self.preview;
            p.entry = self.sel;
            p.class = e.class;
            let row = PREVIEW.get(e.item.max(0) as usize).copied().unwrap_or(PREVIEW[0]);
            let upgraded = record(records, e.item).is_some_and(|r| r.class != 0);
            let (rot, place) = if upgraded { (row[2], row[3]) } else { (row[0], row[1]) };
            p.rot = [rot[0], rot[1], rot[2]];
            p.offset = [place[0], place[1], place[2] - drop];
            p.scale = place[3];
            p.anim = true;
            p.placed = true;
            p.shown = true;
            if self.buy == 0 { p.fade = 0; }
            return;
        }
        let p = &mut self.preview;
        p.entry = self.sel;
        if e.ammo {
            p.class = AMMO_PREVIEW_CLASS as i32;
            p.offset = [AMMO_PREVIEW_AT[0], AMMO_PREVIEW_AT[1], AMMO_PREVIEW_AT[2] - drop];
            p.rot = [0.0; 3];
            p.scale = AMMO_PREVIEW_SCALE;
            p.anim = false;
            (p.shown, p.fade, p.alpha, p.placed) = (true, 0, 0, true);
            return;
        }
        let text = if e.mega { record_at(records, e.item, mega_level(e.item)).map_or(0, |r| r.vendor_text) } else { record(records, e.item).map_or(0, |r| r.vendor_text) };
        self.set_ticker(&texts(text.max(0) as u32));
        let p = &mut self.preview;
        p.class = e.class;
        let row = PREVIEW.get(e.item.max(0) as usize).copied().unwrap_or(PREVIEW[0]);
        let upgraded = record(records, e.item).is_some_and(|r| r.class != 0);
        let (rot, place) = if upgraded { (row[2], row[3]) } else { (row[0], row[1]) };
        p.rot = [rot[0], rot[1], rot[2]];
        p.offset = [place[0], place[1], place[2] - drop];
        p.scale = place[3];
        p.anim = true;
        p.placed = true;
        p.shown = true;
        if self.buy == 0 { p.fade = 0; }
    }

    /// The preview moby the frame wants (`0x375f40`'s placement: the class, its offset in the vendor's rows, its
    /// angles, the scale factor, the alpha).
    pub fn preview(&self) -> Option<PreviewPose> {
        let p = &self.preview;
        // Drawn with the content (`0x37ba50`: once the frame has faded in; not while it fades out).
        if self.sub != 1 || !(p.shown && p.placed) { return None; }
        Some(PreviewPose { class: p.class, offset: p.offset, rot: p.rot, scale: p.scale, alpha: p.alpha.clamp(0, 0x80) as u8, anim: p.anim, bangles: if p.anim { 0 } else { 1 } })
    }

    /// The HUD's ammo panel (`0x30d9c0` / `0x30d948(icon, 10)` each frame): the selected ammo entry's icon,
    /// ammo and most.
    pub fn hud_ammo(&self, game: &GameState, records: &[ItemRecord]) -> Option<(u16, i32, i32)> {
        if self.kind != Kind::Weapons { return None; }
        let e = self.current().filter(|e| e.ammo)?;
        let r = record(records, e.item)?;
        Some((r.icon, Self::ammo_of(game, e.item), r.max_ammo as i32))
    }

    /// `0x375820`: △ — sound 5, the frame fading out from scale 1 to 1.1. The armour menu sets the armour worn (the
    /// one bought, else the old) and announces a bought one ("You got the … Armor!", `0x30efb8`).
    fn leave(&mut self, game: &mut GameState, out: &mut Vec<Out>) {
        out.push(Out::Sound(sound::EXIT));
        if self.kind == Kind::Armor {
            game.armor = self.armor_bought.map_or(self.armor_old, |l| l as i16);
            if let Some(l) = self.armor_bought { game.prompt.announce(armor::GOT[l.clamp(0, armor::MAX) as usize], -1); }
        }
        self.sub = 2;
        self.scale = [1.0, 1.1];
        self.alphas = [FRAME_ALPHA, [0; 3]];
        self.fade = 0.0;
    }

    /// `0x375f40`: one frame of mode 5. `texts` gives a text id's bytes (the game's stream draws the idle ticker line).
    pub fn frame(&mut self, pad: &PadState, game: &mut GameState, records: &[ItemRecord], texts: &dyn Fn(u32) -> Vec<u8>) -> Vec<Out> {
        let mut out = Vec::new();
        match self.sub {
            0 => {
                // `0x37ba50`'s first draw builds the preview (and the ticker) while the frame fades in.
                if self.preview.fade == 3 && !self.preview.shown { self.rebuild_preview(game, records, texts); }
                self.fade += 0.1;
                if 1.0 <= self.fade {
                    self.sub = 1;
                    self.fade = 1.0;
                    self.ticks = 0;
                }
            }
            1 => self.menu_frame(pad, game, records, texts, &mut out),
            _ => {
                self.fade += 0.1;
                if 1.0 <= self.fade {
                    self.fade = 1.0;
                    self.ticks = 0;
                    out.push(Out::Exit);
                }
            }
        }
        out
    }

    /// Substate 1: the held counters, the preview's fade, the rebuild, the pad, the buy.
    fn menu_frame(&mut self, pad: &PadState, game: &mut GameState, records: &[ItemRecord], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Out>) {
        self.held_l = if pad.held & button::LEFT != 0 { self.held_l + 1 } else { 0 };
        self.held_r = if pad.held & button::RIGHT != 0 { self.held_r + 1 } else { 0 };
        {
            let p = &mut self.preview;
            if p.fade == 0 {
                p.alpha += 3;
                if 0x7f < p.alpha {
                    p.fade = 2;
                    p.alpha = 0x80;
                }
            } else if p.fade == 1 {
                p.scale *= 0.9;
                p.alpha -= 0x10;
                if p.alpha < 1 {
                    p.alpha = 0;
                    p.fade = 3;
                }
            }
            if p.shown {
                // The weapon turns 0.01 a tick.
                p.rot[2] = crate::hero::physics::add_rot(p.rot[2], 0.01);
            }
        }
        if self.preview.fade == 3 && self.buy == 0 && self.scroll == 0 { self.rebuild_preview(game, records, texts); }
        self.ticks += 1;
        // `0x376d40`'s ease of the strip: 4 a frame back to 0.
        if self.scroll < 0 { self.scroll += 4 } else if 0 < self.scroll { self.scroll -= 4 }
        self.pad_frame(pad, game, records, out);
        if self.buy != 0 {
            match self.kind {
                Kind::Weapons => self.buy_frame(pad, game, records, texts, out),
                Kind::Armor => self.buy_armor(pad, game, texts, out),
                Kind::Mods => self.buy_mods(pad, game, records, texts, out),
                Kind::Ship => self.buy = 0,
            }
        }
        self.prev_pressed = pad.pressed;
        // `0x376a58`'s idle line: once the text has scrolled past (a character every 0x14 pixels), not the
        // Gadgetron vendor's, a draw of 12: 0..5 a Megacorp line, else the title.
        let scrolled = self.ticker.len() as i32 - self.ticker_scroll / TICKER_CHAR < 1;
        if scrolled && !self.gadgetron && matches!(self.kind, Kind::Weapons | Kind::Armor) {
            let r = game.rng.randi(12);
            let id = if r < 6 { text::LINES[r as usize] } else { text::EMPORIUM };
            self.set_ticker(&texts(id));
        } else {
            self.ticker_scroll += TICKER_STEP;
        }
    }

    /// `0x375ad8`: ← / →, ✕, △ (not while a buy runs). ✕ in the armour menu needs an entry with a price; in the
    /// other menus it starts the buy, which decides.
    fn pad_frame(&mut self, pad: &PadState, game: &mut GameState, records: &[ItemRecord], out: &mut Vec<Out>) {
        if self.buy != 0 { return; }
        let n = self.entries.len();
        let repeat = |held: i32| 0xf < held && held % 10 == 1;
        if (pad.pressed & button::RIGHT != 0 || repeat(self.held_r)) && self.scroll < 0x39 && self.sel + 1 < n {
            self.sel += 1;
            if self.preview.shown { self.preview.fade = 1; }
            self.scroll += STRIP_PITCH;
            out.push(Out::Sound(sound::CURSOR));
        }
        if (pad.pressed & button::LEFT != 0 || repeat(self.held_l)) && -0x39 < self.scroll && 0 < self.sel {
            self.sel -= 1;
            if self.preview.shown { self.preview.fade = 1; }
            self.scroll -= STRIP_PITCH;
            out.push(Out::Sound(sound::CURSOR));
        }
        if pad.pressed & button::CROSS != 0 && self.prev_pressed & button::CROSS == 0 && (self.kind != Kind::Armor || self.current().is_some_and(|e| e.price != 0)) {
            let e = self.current().unwrap_or_default();
            let refused = self.kind == Kind::Weapons && {
                let (ammo, max, unit) = record(records, e.item).map_or((0, 0, 0), |r| (Self::ammo_of(game, e.item), r.max_ammo as i32, r.ammo_price as i32));
                let bolts = game.bolts as i32;
                (e.ammo || bolts < self.price(records)) && (!e.ammo || max <= ammo || bolts < unit)
            };
            if refused {
                out.push(Out::Sound(sound::DENIED));
            } else {
                if self.preview.shown { self.preview.fade = 1; }
                self.buy = 1;
                out.push(Out::Sound(sound::SELECT));
            }
        }
        if pad.pressed & button::TRIANGLE != 0 { self.leave(game, out); }
    }

    /// `0x375068`: the armour menu's buy — 1 asks (sound 2 when the bolts fall short), 2 reads ✕ / △, 3 pays: too
    /// few bolts (or Carbonox worn) "You can't afford that", else the level bought, the bolts taken, "THANK YOU",
    /// sound 7 and the menu left at once.
    fn buy_armor(&mut self, pad: &PadState, game: &mut GameState, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Out>) {
        let e = self.current().unwrap_or_default();
        let bolts = game.bolts as i32;
        match self.buy {
            1 => {
                self.confirmed = false;
                if bolts < e.price { out.push(Out::Sound(sound::DENIED)); }
                self.buy = 2;
            }
            2 => {
                if pad.pressed & button::TRIANGLE != 0 && self.prev_pressed & button::TRIANGLE == 0 {
                    self.confirmed = false;
                    self.buy = 3;
                } else if pad.pressed & button::CROSS != 0 && self.prev_pressed & button::CROSS == 0 {
                    self.confirmed = true;
                    self.buy = 3;
                }
            }
            3 => {
                if !self.confirmed {
                    out.push(Out::Sound(sound::SELECT));
                } else if bolts < e.price || self.armor_old == armor::MAX as i16 {
                    self.set_ticker(&texts(text::CANT_AFFORD_THAT));
                    out.push(Out::Sound(sound::SELECT));
                } else {
                    self.armor_bought = Some(e.level);
                    out.push(Out::Sound(sound::PURCHASE));
                    game.bolts = (bolts - e.price).max(0) as u32;
                    self.set_ticker(&texts(text::THANK_YOU));
                    out.push(Out::Sound(sound::PURCHASE));
                    self.leave(game, out);
                }
                self.buy = 0;
                self.rebuild_preview(game, &[], texts);
            }
            _ => self.buy = 0,
        }
    }

    /// `0x375270`: the mod menu's buy — 1 asks (nothing with an empty list), 2 reads △ (cancel) and ✕ (held), 3
    /// tests the platinum bolts (sound 2 when short) and that the mod is not bought, 4 pays: "You can't afford
    /// that" when short, else "THANK YOU", sound 7, the platinum spent, the mod's bought bits (2·slot and 2·slot + 1)
    /// set, the list rebuilt.
    fn buy_mods(&mut self, pad: &PadState, game: &mut GameState, records: &[ItemRecord], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Out>) {
        let e = self.current().unwrap_or_default();
        match self.buy {
            1 => self.buy = if self.entries.is_empty() { 0 } else { 2 },
            2 => {
                if pad.pressed & button::TRIANGLE != 0 {
                    self.confirmed = false;
                    self.buy = 4;
                } else if pad.pressed & button::CROSS != 0 {
                    self.buy = 3;
                }
            }
            3 => {
                self.confirmed = false;
                if game.platinum() < e.price {
                    out.push(Out::Sound(sound::DENIED));
                } else {
                    self.confirmed = game.mods_bought[e.item.max(0) as usize & 0x3f] & (1 << ((e.slot & 0xf) << 1)) == 0;
                }
                self.buy = 4;
            }
            4 => {
                if !self.confirmed {
                    out.push(Out::Sound(sound::SELECT));
                } else if game.platinum() < e.price {
                    self.set_ticker(&texts(text::CANT_AFFORD_THAT));
                    out.push(Out::Sound(sound::SELECT));
                } else {
                    self.set_ticker(&texts(text::THANK_YOU));
                    out.push(Out::Sound(sound::PURCHASE));
                    game.spend_platinum(e.price);
                    if mods::HELP.get(self.sel).copied() == Some(2) { out.push(Out::Help(0xa01, 0x58)); }
                    let priced = record(records, e.item).is_some_and(|r| r.mods.get(e.slot as usize).is_some_and(|&m| m != 0));
                    if priced {
                        let b = &mut game.mods_bought[e.item.max(0) as usize & 0x3f];
                        *b |= 1 << ((e.slot & 0xf) << 1);
                        if *b & (1 << (e.slot * 2)) != 0 { *b |= 1 << (e.slot * 2 + 1); }
                    }
                    let level = self.level;
                    self.entries = mods_list(game, records, level);
                }
                self.buy = 0;
                self.rebuild_preview(game, records, texts);
            }
            _ => self.buy = 0,
        }
    }

    /// `0x3747e8`: the buy (module docs).
    fn buy_frame(&mut self, pad: &PadState, game: &mut GameState, records: &[ItemRecord], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Out>) {
        let Some(e) = self.current() else {
            self.buy = 0;
            return;
        };
        let Some(r) = record(records, e.item).copied() else {
            self.buy = 0;
            return;
        };
        let (unit, max) = (r.ammo_price as i32, r.max_ammo as i32);
        let ammo = Self::ammo_of(game, e.item);
        let bolts = game.bolts as i32;
        match self.buy {
            1 => {
                self.qty = if unit == 0 { 0 } else { bolts / unit };
                if max - ammo < self.qty { self.qty = max - ammo; }
                self.confirmed = false;
                self.qty_mode = false;
                if !e.ammo || ammo < max {
                    if !e.ammo {
                        if self.price(records) <= bolts {
                            self.buy = 2;
                            return;
                        }
                        out.push(Out::Sound(sound::DENIED));
                        self.buy = 2;
                        return;
                    }
                    if unit <= bolts {
                        self.qty_mode = true;
                        self.buy = 2;
                        return;
                    }
                }
                out.push(Out::Sound(sound::DENIED));
                self.buy = 2;
            }
            2 => {
                if self.qty_mode {
                    let (hl, hr) = (self.held_l, self.held_r);
                    if (pad.pressed & button::LEFT != 0 || (0xf < hl && hl & 7 == 0) || 0x2f < hl) && 1 < self.qty {
                        self.qty -= 1;
                        if hl & 3 == 0 || hl < 0x10 { out.push(Out::Sound(sound::CURSOR)); }
                    }
                    if (pad.pressed & button::RIGHT != 0 || (0xf < hr && hr & 7 == 0) || 0x2f < hr) && unit != 0 {
                        let most = (max - ammo).min(bolts / unit);
                        if self.qty < most {
                            self.qty += 1;
                            if hr & 3 == 0 || hr < 0x10 { out.push(Out::Sound(sound::CURSOR)); }
                        }
                    }
                }
                if pad.pressed & button::TRIANGLE != 0 && self.prev_pressed & button::TRIANGLE == 0 {
                    self.confirmed = false;
                    self.buy = 4;
                } else if pad.pressed & button::CROSS != 0 && self.prev_pressed & button::CROSS == 0 {
                    self.confirmed = true;
                    self.buy = 4;
                }
            }
            4 => {
                let mut snd = sound::SELECT;
                if self.confirmed {
                    let cost = if e.ammo { unit * self.qty } else { self.price(records) };
                    if bolts < cost && max <= ammo {
                        // Both too dear and full: nothing.
                    } else if e.ammo {
                        if max <= ammo {
                            // Full.
                        } else if bolts < unit * self.qty {
                            self.set_ticker(&texts(text::CANT_AFFORD_THAT));
                        } else {
                            // `0x2d4358(item, qty)`: the ammo added, the overflow given back.
                            let a = game.ammo.get_mut(e.item.max(0) as usize);
                            let overflow = match a {
                                Some(a) => {
                                    *a = (*a).min(max);
                                    let over = (*a + self.qty - max).max(0);
                                    *a = (*a + self.qty).min(max);
                                    over
                                }
                                None => self.qty,
                            };
                            game.bolts = (bolts - unit * (self.qty - overflow)).max(0) as u32;
                            let gadgetron = self.gadgetron;
                            self.entries = build_list(game, records, gadgetron, false);
                            self.set_ticker(&texts(text::THANK_YOU));
                            snd = sound::PURCHASE;
                        }
                    } else {
                        let price = self.price(records);
                        if bolts < price {
                            self.set_ticker(&texts(text::CANT_AFFORD_THAT));
                        } else {
                            game.get_item(e.item);
                            // `0x2f46e0`'s ammo: the record's `+0x82` when it has an ammo price and none yet (the
                            // port's `get_item` knows only the hand rows' ammo).
                            if r.ammo_price != 0 && Self::ammo_of(game, e.item) == 0 {
                                if let Some(a) = game.ammo.get_mut(e.item.max(0) as usize) { *a = r.start_ammo as i32; }
                            }
                            if e.mega {
                                if let Some(m) = record_at(records, e.item, mega_level(e.item)).filter(|m| m.ammo_price != 0) {
                                    if let Some(a) = game.ammo.get_mut(e.item.max(0) as usize) { *a = m.max_ammo as i32; }
                                }
                            }
                            if e.item != 0x1f && e.item != 0x3d { out.push(Out::Equip(e.item)); }
                            game.bolts = (bolts - price).max(0) as u32;
                            let gadgetron = self.gadgetron;
                            self.entries = build_list(game, records, gadgetron, false);
                            if self.entries.len().saturating_sub(1) < self.sel { self.sel = self.entries.len().saturating_sub(1); }
                            self.set_ticker(&texts(text::THANK_YOU));
                            snd = sound::PURCHASE;
                        }
                    }
                }
                out.push(Out::Sound(snd));
                self.buy = 0;
                self.rebuild_preview(game, records, texts);
            }
            _ => self.buy = 0,
        }
    }

    /// `0x37ba50`'s mode-0 draw (module docs) as GUI quads: `icon` gives an icon frame's texture and texel size.
    pub fn draw(&self, game: &GameState, records: &[ItemRecord], a: &Assets, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let f = self.fade;
        let scale = self.scale[0] + (self.scale[1] - self.scale[0]) * f;
        let alpha = |k: usize| (self.alphas[0][k] as f32 + (self.alphas[1][k] - self.alphas[0][k]) as f32 * f) as i32;
        let rgba = |c: u32| c.to_le_bytes();
        let sprites: &[u32] = match self.kind {
            Kind::Weapons => &FRAME_SPRITES,
            Kind::Armor => &ARMOR_SPRITES,
            Kind::Mods => &MODS_SPRITES,
            Kind::Ship => &[],
        };
        for (k, (&id, &rgb)) in sprites.iter().zip(FRAME_RGB.iter()).enumerate() {
            let al = alpha(k.min(2));
            if al <= 0 { continue; }
            if let Some(s) = a.sprites.get(&id) { gui::sprite(out, s, SCREEN_CENTRE, 0.0, [scale, scale], rgba((al as u32) << 24 | rgb)); }
        }
        if f < 1.0 { return; }
        let g = &a.glyphs_2;
        match self.kind {
            Kind::Weapons => {
                self.draw_title(game, records, g, icon, texts, out);
                self.draw_buttons(game, records, g, texts, out);
                self.draw_question(game, records, g, icon, texts, out);
                self.draw_strip(records, g, icon, texts, out);
            }
            Kind::Armor => {
                self.draw_armor_title(game, g, icon, texts, out);
                self.draw_armor_buttons(game, g, texts, out);
                self.draw_armor_question(game, g, texts, out);
                self.draw_armor_strip(icon, out);
            }
            Kind::Mods => {
                self.draw_mods_title(game, records, g, icon, texts, out);
                self.draw_mods_buttons(game, g, texts, out);
                self.draw_mods_question(game, g, texts, out);
                self.draw_mods_strip(records, g, icon, out);
            }
            Kind::Ship => {}
        }
        self.draw_ticker(g, out);
    }

    /// The strip's pulsing selection box (`0x2f18a8` twice: the pulse `(|((frame & 0xf)·4 − 0x20)| + 0x40) · 0x10202`
    /// over `0x80442d00` two pixels in).
    fn draw_box(&self, x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<Quad>) {
        let pulse = ((self.ticks & 0xf) * 4 - 0x20).abs();
        let k = (pulse + 0x40) as u32;
        gui::rect(out, y0, y1, x0, x1, (k * 0x10202 | 0x8000_0000).to_le_bytes());
        gui::rect(out, y0 + 2, y1 - 2, x0 + 2, x1 - 2, 0x8044_2d00u32.to_le_bytes());
    }

    /// `0x378678`: "Can't buy any armor" centred at (256, 363) without an entry; else "Cost:" ending at 140, the
    /// bolt icon at 156 and the price from 176 (red when the bolts fall short) at y 290, the armour's name centred
    /// at (256, 363).
    fn draw_armor_title(&self, game: &GameState, g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let Some(e) = self.current().filter(|e| 0 < e.level) else {
            centred(out, g, &texts(armor::NONE), 256.0, 363.0, TEXT_WHITE);
            return;
        };
        right(out, g, &texts(text::COST), 0x8c as f32, 290.0, TEXT_WHITE);
        icon_quad(out, icon, BOLT_ICON, [0x9c as f32, 290.0], [16.0, 16.0], BOLT_ICON_RGBA);
        let colour = if (game.bolts as i32) < e.price { TEXT_RED } else { TEXT_WHITE };
        gui::text(out, g, 2, Self::commas(e.price).as_bytes(), 0xb0 as f32, 290.0, 1.0, colour.to_le_bytes());
        centred(out, g, &texts(armor::NAMES[e.level.clamp(0, armor::MAX) as usize]), 256.0, 363.0, TEXT_WHITE);
    }

    /// `0x379560`: "✕ Buy" from 40 (with an entry) and "△ Exit" ending at 280 at y 312; in a buy "△ Back", or "✕
    /// Yes" / "△ No" when the bolts cover it.
    fn draw_armor_buttons(&self, game: &GameState, g: &[Glyph; 256], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let (y, left, rightx) = (312.0, 0x28 as f32, 0x118 as f32);
        let price = self.current().map_or(0, |e| e.price);
        if self.buy == 0 {
            if 0 < price { gui::text(out, g, 2, &texts(text::BUY), left, y, 1.0, TEXT_WHITE.to_le_bytes()); }
            right(out, g, &texts(text::EXIT), rightx, y, TEXT_WHITE);
        } else if (game.bolts as i32) < price {
            right(out, g, &texts(text::BACK), rightx, y, TEXT_WHITE);
        } else {
            gui::text(out, g, 2, &texts(text::YES), left, y, 1.0, TEXT_WHITE.to_le_bytes());
            right(out, g, &texts(text::NO), rightx, y, TEXT_WHITE);
        }
    }

    /// `0x37a050`: the question in a box over x 314..486 centred at (400, 217): "You can't afford it!", else
    /// "Purchase?" at 160.
    fn draw_armor_question(&self, game: &GameState, g: &[Glyph; 256], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        if self.buy == 0 { return; }
        let price = self.current().map_or(0, |e| e.price);
        let mut b = TextBox { top: 120, bottom: 314, left: 0x13a, right: 0x1e6, x: 400, y: 217, width: 0xa4, height: 186, line: 0x10, flags: text_box::CENTRE | text_box::MIDDLE };
        let white = TEXT_WHITE.to_le_bytes();
        if (game.bolts as i32) < price {
            gui::text_in_box(out, g, 2, &mut b, &texts(text::CANT_AFFORD_IT), white);
        } else {
            b.flags = text_box::CENTRE;
            b.y = 120 + 0x28;
            gui::text_in_box(out, g, 2, &mut b, &texts(text::PURCHASE), white);
        }
    }

    /// `0x377948`: the armour icons on the left: the box at (101, 123)-(157, 179) with an entry, icons 40 × 40 at
    /// y 129, 0x38 apart from `scroll − 0x3f + 2`, fading 0.75 a pixel from x 150.
    fn draw_armor_strip(&self, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, out: &mut Vec<Quad>) {
        if self.current().is_some_and(|e| 0 < e.price) { self.draw_box(0x65, 123, 0x9d, 179, out); }
        let fade = |x: f32| (0x80 - ((150.0 - x).abs() * STRIP_FADE) as i32).max(0);
        for k in -3..5i32 {
            let Some(e) = usize::try_from(self.sel as i32 + k).ok().and_then(|i| self.entries.get(i)) else { continue };
            let x = (self.scroll - 0x3f + STRIP_PITCH * (k + 3)) as f32;
            let al = (fade(x + 2.0) + fade(x + STRIP_ICON)) / 2;
            icon_quad(out, icon, e.icon, [x + 2.0, 129.0], [STRIP_ICON, STRIP_ICON], (al as u32) << 24 | 0xf0f0b0);
        }
    }

    /// `0x3788c8` (a mod not yet bought): "Mod for Weapon" centred at (256, 263), "Cost: N Platinum" centred at
    /// (256, 112) with the platinum icon after it, "You've got: N Platinum" centred at (250, 205) with its icon.
    fn draw_mods_title(&self, game: &GameState, records: &[ItemRecord], g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let Some(e) = self.current() else { return };
        if game.mods_bought[e.item.max(0) as usize & 0x3f] & (1 << ((e.slot & 0xf) << 1)) != 0 { return; }
        let slot = e.slot.clamp(0, 2) as usize;
        if e.item != 0 {
            let mut t = texts(mods::NAMES[slot]);
            t.extend_from_slice(b" for ");
            t.extend_from_slice(&texts(record(records, e.item).map_or(0, |r| r.name)));
            centred(out, g, &t, 256.0, 263.0, TEXT_WHITE);
        }
        let platinum = texts(mods::PLATINUM);
        let mut cost = texts(text::COST);
        cost.push(b' ');
        cost.extend_from_slice(Self::commas(e.price).as_bytes());
        cost.push(b' ');
        cost.extend_from_slice(&platinum);
        let w = gui::text_width(g, &cost, 1.0);
        centred(out, g, &cost, 256.0, 112.0, TEXT_WHITE);
        icon_quad(out, icon, mods::PLATINUM_ICON, [256.0 + (w >> 1) as f32 + 4.0, 112.0], [16.0, 16.0], BOLT_ICON_RGBA);
        let mut have = texts(mods::YOU_HAVE);
        have.push(b' ');
        have.extend_from_slice(game.platinum().to_string().as_bytes());
        have.push(b' ');
        have.extend_from_slice(&platinum);
        let w = gui::text_width(g, &have, 1.0);
        centred(out, g, &have, 0xfa as f32, 0xcd as f32, TEXT_WHITE);
        icon_quad(out, icon, mods::PLATINUM_ICON, [0xfa as f32 + (w >> 1) as f32 + 4.0, 0xcd as f32], [16.0, 16.0], BOLT_ICON_RGBA);
    }

    /// `0x379738`: "✕ Buy" centred at (90, 164) and "△ Exit" at (420, 164); in a buy "△ Back" alone when the
    /// *bolts* fall short of the price (the game's test), else "✕ Yes" / "△ No".
    fn draw_mods_buttons(&self, game: &GameState, g: &[Glyph; 256], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let (y, left, rightx) = (0xa4 as f32, 0x5a as f32, (0x5a + 0x14a) as f32);
        let price = self.current().map_or(0, |e| e.price);
        if self.buy == 0 {
            centred(out, g, &texts(text::BUY), left, y, TEXT_WHITE);
            centred(out, g, &texts(text::EXIT), rightx, y, TEXT_WHITE);
        } else if (game.bolts as i32) < price {
            centred(out, g, &texts(text::BACK), rightx, y, TEXT_WHITE);
        } else {
            centred(out, g, &texts(text::YES), left, y, TEXT_WHITE);
            centred(out, g, &texts(text::NO), rightx, y, TEXT_WHITE);
        }
    }

    /// `0x37a218`: "You can't afford it!" (the platinum bolts short) or "Purchase?" centred at (256, 160).
    fn draw_mods_question(&self, game: &GameState, g: &[Glyph; 256], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        if self.buy == 0 { return; }
        let price = self.current().map_or(0, |e| e.price);
        let id = if game.platinum() < price { text::CANT_AFFORD_IT } else { text::PURCHASE };
        centred(out, g, &texts(id), 0x100 as f32, 0xa0 as f32, TEXT_WHITE);
    }

    /// `0x377338`: the strip at y 312..382: the box at (227, 312)-(287, 379); per slot from `scroll − 0x2f` the
    /// weapon's icon 24 × 24 at (+13, 346) in its class colour, the mod's icon 0xeaa3 (frame = the slot) 30 × 30 at
    /// (+10, 315) and its frame 3 at (+10, 345), fading 0.75 a pixel from x 256.
    fn draw_mods_strip(&self, records: &[ItemRecord], g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, out: &mut Vec<Quad>) {
        let _ = g;
        self.draw_box(0xe3, 0x138, 0x11f, 0x17b, out);
        let fade = |x: f32| (0x80 - ((256.0 - x).abs() * STRIP_FADE) as i32).max(0);
        for k in -5..7i32 {
            let Some(e) = usize::try_from(self.sel as i32 + k).ok().and_then(|i| self.entries.get(i)) else { continue };
            let x = (self.scroll - 0x2f + STRIP_PITCH * (k + 5)) as f32;
            let Some(r) = record(records, e.item) else { continue };
            let class = if SPECIALS.contains(&e.item) && r.class == 1 { 2 } else { r.class as usize };
            let rgb = ICON_COLOURS.get(class).copied().unwrap_or(0) & 0xff_ffff;
            let al = (fade(x + 2.0) + fade(x + STRIP_ICON)) / 2;
            icon_quad(out, icon, r.icon, [x + 13.0, 346.0], [24.0, 24.0], (al as u32) << 24 | rgb);
            icon_quad_frame(out, icon, mods::ICON, e.slot.clamp(0, 2) as u16, [x + 10.0, 315.0], [30.0, 30.0], (al as u32) << 24 | rgb);
            icon_quad_frame(out, icon, mods::ICON, 3, [x + 10.0, 345.0], [30.0, 30.0], (al as u32) << 24 | rgb);
        }
    }

    /// `0x376c80`: a bolt count with commas.
    fn commas(n: i32) -> String {
        if n < 1000 {
            format!("{n}")
        } else if n < 1_000_000 {
            format!("{},{:03}", n / 1000, n % 1000)
        } else {
            format!("{},{:03},{:03}", n / 1_000_000, (n / 1000) % 1000, n % 1000)
        }
    }

    /// `0x3781c8`: the selection's name centred at (256, 274) — an ammo entry's "Name-Ammo" — and its cost at 210:
    /// "You're maxed out!" for full ammo, else "Cost:", the bolt icon and the price (red when the bolts fall short).
    fn draw_title(&self, game: &GameState, records: &[ItemRecord], g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let Some(e) = self.current() else { return };
        let bolts = game.bolts as i32;
        let (name, price, room) = if e.ammo {
            let r = record(records, e.item);
            let mut name = texts(r.map_or(0, |r| r.name));
            name.push(b'-');
            name.extend_from_slice(&texts(text::AMMO));
            let unit = r.map_or(0, |r| r.ammo_price as i32);
            (name, unit, r.is_some_and(|r| Self::ammo_of(game, e.item) < r.max_ammo as i32))
        } else if e.mega {
            (texts(record_at(records, e.item, mega_level(e.item)).map_or(0, |r| r.name)), self.price(records), true)
        } else {
            (texts(record(records, e.item).map_or(0, |r| r.name)), self.price(records), true)
        };
        centred(out, g, &name, 256.0, 274.0, TEXT_WHITE);
        let colour = if bolts < price { TEXT_RED } else { TEXT_WHITE };
        if !room {
            centred(out, g, &texts(text::MAXED_OUT), 0x103 as f32, 210.0, TEXT_WHITE);
        } else {
            right(out, g, &texts(text::COST), 0xdc as f32, 210.0, TEXT_WHITE);
            icon_quad(out, icon, BOLT_ICON, [0xec as f32, 210.0], [16.0, 16.0], BOLT_ICON_RGBA);
            gui::text(out, g, 2, Self::commas(price).as_bytes(), 256.0, 210.0, 1.0, colour.to_le_bytes());
        }
    }

    /// The selection can be paid for (`0x379000`'s test): a weapon within the bolts; ammo with room and a unit
    /// within them.
    fn affordable(&self, game: &GameState, records: &[ItemRecord]) -> bool {
        let Some(e) = self.current() else { return false };
        let bolts = game.bolts as i32;
        if !e.ammo { return self.price(records) <= bolts; }
        record(records, e.item).is_some_and(|r| Self::ammo_of(game, e.item) < r.max_ammo as i32 && r.ammo_price as i32 <= bolts)
    }

    /// `0x379000`: the buttons at y 164: "✕ Buy" at 80 (when affordable) and "△ Exit" at 420; in a buy "△ Back",
    /// or "✕ Yes" / "△ No" for an affordable weapon, "✕ Buy" / "△ Back" for affordable ammo.
    fn draw_buttons(&self, game: &GameState, records: &[ItemRecord], g: &[Glyph; 256], texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let (y, left, rightx) = (164.0, 0x50 as f32, 0x1a4 as f32);
        if self.buy == 0 {
            if self.affordable(game, records) { centred(out, g, &texts(text::BUY), left, y, TEXT_WHITE); }
            centred(out, g, &texts(text::EXIT), rightx, y, TEXT_WHITE);
            return;
        }
        let Some(e) = self.current() else { return };
        let bolts = game.bolts as i32;
        let (ammo, max, unit) = record(records, e.item).map_or((0, 0, 0), |r| (Self::ammo_of(game, e.item), r.max_ammo as i32, r.ammo_price as i32));
        if e.ammo {
            if max <= ammo || bolts < unit {
                centred(out, g, &texts(text::BACK), rightx, y, TEXT_WHITE);
            } else {
                centred(out, g, &texts(text::BUY), left, y, TEXT_WHITE);
                centred(out, g, &texts(text::BACK), rightx, y, TEXT_WHITE);
            }
        } else if bolts < self.price(records) {
            centred(out, g, &texts(text::BACK), rightx, y, TEXT_WHITE);
        } else {
            centred(out, g, &texts(text::YES), left, y, TEXT_WHITE);
            centred(out, g, &texts(text::NO), rightx, y, TEXT_WHITE);
        }
    }

    /// `0x379c50`: the question in a box over x 160..352 centred at (256, 160): "You can't afford it!", or "How
    /// many?" at 132 with "< n >" at 160 and the cost (the bolt icon at 260) at 180, or "Purchase?" at 126.
    fn draw_question(&self, game: &GameState, records: &[ItemRecord], g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        if self.buy == 0 { return; }
        let Some(e) = self.current() else { return };
        let bolts = game.bolts as i32;
        let mut b = TextBox { top: 0x78, bottom: 200, left: 0xa0, right: 0x160, x: 0x100, y: 160, width: 0xb8, height: 72, line: 0x10, flags: text_box::CENTRE | text_box::MIDDLE };
        let white = TEXT_WHITE.to_le_bytes();
        if e.ammo {
            let unit = record(records, e.item).map_or(0, |r| r.ammo_price as i32);
            if unit <= bolts {
                b.flags = text_box::CENTRE;
                b.y = 132;
                gui::text_in_box(out, g, 2, &mut b, &texts(text::HOW_MANY), white);
                b.y = 0x78 + 0x28;
                gui::text_in_box(out, g, 2, &mut b, format!("< {} >", self.qty).as_bytes(), white);
                right(out, g, Self::commas(self.qty * unit).as_bytes(), 256.0, 180.0, TEXT_WHITE);
                icon_quad(out, icon, BOLT_ICON, [0x104 as f32, 180.0], [16.0, 16.0], BOLT_ICON_RGBA);
                return;
            }
            gui::text_in_box(out, g, 2, &mut b, &texts(text::CANT_AFFORD_IT), white);
        } else if bolts < self.price(records) {
            gui::text_in_box(out, g, 2, &mut b, &texts(text::CANT_AFFORD_IT), white);
        } else {
            b.flags = text_box::CENTRE;
            b.y = 0x78 + 6;
            gui::text_in_box(out, g, 2, &mut b, &texts(text::PURCHASE), white);
        }
    }

    /// `0x376d40`: the strip (module docs). The offset eases 4 a frame to 0; the selection's box pulses with the
    /// frame count; seven icons either side of the selection's slot.
    fn draw_strip(&self, records: &[ItemRecord], g: &[Glyph; 256], icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, texts: &dyn Fn(u32) -> Vec<u8>, out: &mut Vec<Quad>) {
        let pulse = ((self.ticks & 0xf) * 4 - 0x20).abs();
        let k = (pulse + 0x40) as u32;
        gui::rect(out, 325, 381, 0xe5, 0x11d, (k * 0x10202 | 0x8000_0000).to_le_bytes());
        gui::rect(out, 327, 379, 0xe7, 0x11b, 0x8044_2d00u32.to_le_bytes());
        let fade = |x: f32| (0x80 - ((256.0 - x).abs() * STRIP_FADE) as i32).max(0);
        for k in -5..7i32 {
            let Some(e) = usize::try_from(self.sel as i32 + k).ok().and_then(|i| self.entries.get(i)) else { continue };
            let x = (self.scroll - 0x2f + STRIP_PITCH * (k + 5)) as f32;
            let Some(r) = record(records, e.item) else { continue };
            let class = if e.mega { 2 } else if SPECIALS.contains(&e.item) && r.class == 1 { 2 } else { r.class as usize };
            let rgb = ICON_COLOURS.get(class).copied().unwrap_or(0) & 0xff_ffff;
            let al = (fade(x + 2.0) + fade(x + STRIP_ICON)) / 2;
            icon_quad(out, icon, r.icon, [x + 4.0, STRIP_Y], [STRIP_ICON, STRIP_ICON], (al as u32) << 24 | rgb);
            let label = if e.mega { Some(text::MEGA) } else if e.ammo { Some(text::AMMO) } else { None };
            if let Some(id) = label {
                let t = texts(id);
                let w = gui::text_width(g, &t, 0.75);
                self.glyphs(g, &t, x - ((w >> 1) - 0x19) as f32 - 1.0, LABEL_Y, 0.75, TEXT_WHITE, out);
            }
        }
    }

    /// `0x376638`: text glyph by glyph at `scale`, each with the alpha of its place: 0x80 less 0.75 a pixel from
    /// x 256 (the mean of its sides here).
    fn glyphs(&self, g: &[Glyph; 256], t: &[u8], mut x: f32, y: f32, scale: f32, colour: u32, out: &mut Vec<Quad>) {
        let cell = scale * 16.0;
        for &c in t {
            if c == 0 { break; }
            let adv = g[c as usize].advance as f32 * scale;
            if g[c as usize].advance != 0 {
                let a = |px: f32| (0x80 - ((256.0 - px).abs() * STRIP_FADE) as i32).max(0);
                let al = (a(x) + a(x + cell)) / 2;
                gui::text(out, g, 2, &[c], x, y, scale, ((al as u32) << 24 | colour & 0xff_ffff).to_le_bytes());
            }
            x += adv;
        }
    }

    /// `0x376a58`: the ticker at (−scroll, 60), scale 1.5.
    fn draw_ticker(&self, g: &[Glyph; 256], out: &mut Vec<Quad>) {
        self.glyphs(g, &self.ticker, -(self.ticker_scroll as f32), 60.0, 1.5, TICKER_RGB, out);
    }
}

/// `0x2ea608`: font-2 text centred on `x`.
fn centred(out: &mut Vec<Quad>, g: &[Glyph; 256], t: &[u8], x: f32, y: f32, colour: u32) {
    let w = gui::text_width(g, t, 1.0);
    gui::text(out, g, 2, t, x - (w >> 1) as f32, y, 1.0, colour.to_le_bytes());
}

/// `0x2ea4d8`: font-2 text ending at `x`.
fn right(out: &mut Vec<Quad>, g: &[Glyph; 256], t: &[u8], x: f32, y: f32, colour: u32) {
    let w = gui::text_width(g, t, 1.0);
    gui::text(out, g, 2, t, x - w as f32, y, 1.0, colour.to_le_bytes());
}

/// `0x2faf68`: an icon's frame 0 at `pos`, `size`.
fn icon_quad(out: &mut Vec<Quad>, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, id: u16, pos: [f32; 2], size: [f32; 2], colour: u32) {
    icon_quad_frame(out, icon, id, 0, pos, size, colour);
}

/// `0x2faa98(id, frame)` + `0x2faf68`: an icon's frame at `pos`, `size`.
fn icon_quad_frame(out: &mut Vec<Quad>, icon: &dyn Fn(u16, u16) -> Option<(Tex, [f32; 2])>, id: u16, frame: u16, pos: [f32; 2], size: [f32; 2], colour: u32) {
    if colour >> 24 == 0 { return; }
    let Some((tex, [tw, th])) = icon(id, frame) else { return };
    let [x, y] = pos;
    let [w, h] = size;
    out.push(Quad { pos: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]], uv: [[0.0, 0.0], [tw, 0.0], [0.0, th], [tw, th]], rgba: colour.to_le_bytes(), tex });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weapon(price: i32, ammo_price: u16, max_ammo: u16) -> ItemRecord {
        ItemRecord { mega: true, price, ammo_price, max_ammo, uses_ammo: ammo_price != 0, classes: [0x5d2, 0], ..Default::default() }
    }

    fn records() -> Vec<ItemRecord> {
        let mut v = vec![ItemRecord::default(); 0x38];
        v[0x1e] = weapon(0, 1, 200);
        v[0x2a] = weapon(0, 20, 8);
        v[0x16] = weapon(5000, 2, 100);
        v[0x1a] = weapon(7500, 5, 50);
        v[0x17] = weapon(15000, 3, 60);
        v
    }

    /// A new game on Aranos stocks the Lancer and the Gravity Bomb as owned (`0x2fcf60`); Oozla (level 1) adds the
    /// Chopper and the Blitz Gun for sale; the Seeker Gun (level 3) is not sold there; a planet past 0x14 sells
    /// nothing new.
    #[test]
    fn the_stock_follows_the_planet_table() {
        let r = records();
        let mut g = GameState::default();
        g.get_item(0x1e);
        g.get_item(0x2a);
        g.stock_clean();
        g.stock_for_level(0, &r);
        assert_eq!(&g.stock[..3], &[0x5e, 0x6a, 0xff]);
        g.stock_for_level(1, &r);
        assert_eq!(&g.stock[..5], &[0x5e, 0x6a, 0x16, 0x1a, 0xff]);
        assert!(!g.sold_on(0x17, 1));
        assert!(g.sold_on(0x17, 3));
        assert!(!g.sold_on(0x16, 0x15));
        // A weapon added again keeps its slot; bought, its slot turns owned.
        g.stock_for_level(1, &r);
        assert_eq!(g.stock_len(), 4);
        g.get_item(0x16);
        assert_eq!(g.stock[2], 0x56);
    }

    /// The list: the stock in order (owned items as ammo), then the ammo of owned items off the stock; no duplicates.
    #[test]
    fn the_list_walks_the_stock_then_the_owned_ammo() {
        let r = records();
        let mut g = GameState::default();
        g.get_item(0x1e);
        g.get_item(0x2a);
        g.stock_for_level(1, &r);
        let l = build_list(&mut g, &r, false, false);
        assert_eq!(l.iter().map(|e| (e.item, e.ammo)).collect::<Vec<_>>(), vec![(0x1e, true), (0x2a, true), (0x16, false), (0x1a, false)]);
        // The Seeker Gun picked up elsewhere (not in the stock): its ammo is listed after.
        g.items[0x17] = 1;
        g.stock[4] = 0xff;
        let l = build_list(&mut g, &r, false, true);
        assert_eq!(l.iter().map(|e| (e.item, e.ammo)).collect::<Vec<_>>(), vec![(0x1e, true), (0x2a, true), (0x17, true)]);
    }

    /// The buy: too few bolts refuses with sound 2; enough buys the weapon (owned, the hand asked for, the bolts
    /// taken, the list rebuilt with its ammo).
    #[test]
    fn a_weapon_is_bought_with_enough_bolts() {
        let r = records();
        let mut g = GameState::default();
        g.get_item(0x1e);
        g.get_item(0x2a);
        g.stock_for_level(1, &r);
        let texts = |_: u32| b"x".to_vec();
        let mut out = Vec::new();
        let mut v = Vendor::open(0, &mut g, &r, false, &mut out);
        v.sub = 1;
        v.fade = 1.0;
        v.sel = 2;
        assert_eq!(v.current().map(|e| e.item), Some(0x16));
        let press = |mask: u32| PadState { pressed: mask, held: mask, ..Default::default() };
        g.bolts = 100;
        let o = v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(o, vec![Out::Sound(sound::DENIED)]);
        assert_eq!(v.buy, 0);
        v.frame(&PadState::default(), &mut g, &r, &texts);
        g.bolts = 6000;
        let o = v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(o, vec![Out::Sound(sound::SELECT)]);
        assert_eq!(v.buy, 2);
        v.frame(&PadState::default(), &mut g, &r, &texts);
        let o = v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(v.buy, 4);
        assert!(o.is_empty());
        let o = v.frame(&PadState::default(), &mut g, &r, &texts);
        assert_eq!(o, vec![Out::Equip(0x16), Out::Sound(sound::PURCHASE)]);
        assert_eq!(g.bolts, 1000);
        assert_eq!(g.items[0x16], 1);
        assert_eq!(v.entries.iter().map(|e| (e.item, e.ammo)).collect::<Vec<_>>(), vec![(0x1e, true), (0x2a, true), (0x16, true), (0x1a, false)]);
    }

    /// The armour menu offers the levels past the one worn that the planet allows; a purchase sets the armour and
    /// leaves at once, announcing it.
    #[test]
    fn armour_is_offered_by_the_planet_and_bought() {
        let r = records();
        let mut g = GameState::default();
        g.armor_offer = 0b10;
        let texts = |_: u32| b"x".to_vec();
        let mut out = Vec::new();
        let v = Vendor::open(2, &mut g, &r, false, &mut out);
        assert_eq!(v.entries.iter().map(|e| (e.level, e.price)).collect::<Vec<_>>(), vec![(1, 25_000)]);
        g.armor = 1;
        g.armor_offer = 0b1100;
        let mut v = Vendor::open(2, &mut g, &r, false, &mut out);
        assert_eq!(v.entries.iter().map(|e| e.level).collect::<Vec<_>>(), vec![2, 3]);
        v.sub = 1;
        v.fade = 1.0;
        v.sel = 0;
        let press = |mask: u32| PadState { pressed: mask, held: mask, ..Default::default() };
        g.bolts = 150_000;
        v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(v.buy, 2);
        v.frame(&PadState::default(), &mut g, &r, &texts);
        v.frame(&press(button::CROSS), &mut g, &r, &texts);
        let o = v.frame(&PadState::default(), &mut g, &r, &texts);
        assert!(o.contains(&Out::Sound(sound::PURCHASE)) && o.contains(&Out::Sound(sound::EXIT)));
        assert_eq!((g.armor, g.bolts, v.sub), (2, 50_000, 2));
        assert_eq!(g.prompt.text, armor::GOT[2]);
    }

    /// The mod menu: the planet's mods go on offer; an owned weapon's priced, unbought mods are listed; a purchase
    /// spends platinum bolts and sets the bought bits.
    #[test]
    fn mods_are_offered_by_the_planet_and_bought_with_platinum() {
        let mut r = records();
        r[0x1e].mods = [2, 0, 3];
        let mut g = GameState::default();
        g.get_item(0x1e);
        g.level = 3;
        g.platinum_found = 5;
        let l = mods_list(&mut g, &r, 3);
        assert_eq!(g.mods_available[0x1e], 1);
        assert_eq!(l.iter().map(|e| (e.item, e.slot, e.price)).collect::<Vec<_>>(), vec![(0x1e, 0, 2)]);
        let texts = |_: u32| b"x".to_vec();
        let mut out = Vec::new();
        let mut v = Vendor::open(1, &mut g, &r, false, &mut out);
        v.sub = 1;
        v.fade = 1.0;
        let press = |mask: u32| PadState { pressed: mask, held: mask, ..Default::default() };
        v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(v.buy, 2);
        v.frame(&PadState::default(), &mut g, &r, &texts);
        v.frame(&press(button::CROSS), &mut g, &r, &texts);
        assert_eq!(v.buy, 3);
        v.frame(&PadState::default(), &mut g, &r, &texts);
        assert_eq!((v.buy, v.confirmed), (4, true));
        let o = v.frame(&PadState::default(), &mut g, &r, &texts);
        assert_eq!(o, vec![Out::Sound(sound::PURCHASE)]);
        assert_eq!((g.platinum(), g.mods_bought[0x1e]), (3, 0b11));
        assert!(v.entries.is_empty());
    }

    #[test]
    fn bolt_counts_take_commas() {
        assert_eq!(Vendor::commas(999), "999");
        assert_eq!(Vendor::commas(7500), "7,500");
        assert_eq!(Vendor::commas(1_234_567), "1,234,567");
    }
}
