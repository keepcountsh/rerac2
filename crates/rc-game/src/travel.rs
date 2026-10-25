//! Planet travel (L00): Ratchet's parked ship, the take-off, the galaxy map and the planet page, the fly-away, the
//! landing. Addresses are Aranos's overlay; the ship mode's code is the boot's and the same on every level.
//!
//! **The parked ship** (`0x3012a0`, the level load): class 0x0d54 at the level settings' `+0x3c` (x, y, z) with the
//! yaw `+0x48` when x > 0, draw distance 0xff, update distance 0x10, its update `0x361628` (the class's own `0x3fd5a0`
//! is empty); the fly-away's path `+0x4c` and its camera cuboids `+0x50` / `+0x54` (`0x152c40..`). Its parts are made
//! once (`0x361050` → `0x360d28`) and ride it every tick (crate::moby::classes::props). [`ship_update`]: in play it
//! sits on the ground (`0x31ce20(0.5)` + 0.4, eased by `0x31ed78`); drawn, its engine part glows (`+0xe` += 2 a tick,
//! `cos((v − 0x80)·2π/256)·50 + 0x8c`) and it registers the canopy (`0x367b68`, crate::canopy) and its shadow
//! (`0x361af8`, [`ShipShadow`]). Ratchet within 6 units (8 before the first hit; 6.1 to leave; riding it counts) sets the
//! hatch flag (pvar `+0xc`): the prompt `0x327a28(2, 0x12ea, 1)` ("△ Enter ship", drawn at (255, 80) for 360
//! ticks) and △ (`0x138324 & 0x10`) store the hatch spot `0x152c70` — the ship's frame point (1.48, 2.64, 0) on
//! the ground (`0x31ce20(0.5)`) with the angles (−0.0211, 0.0588, the ship's yaw + 1.2252) — and ask for the take-off
//! (`0x1b1a60 = 1`; the main loop `0x3706f0` then requests mode 6, `0x32c5f0(6, 1, 0, 0)`).
//!
//! **Mode 6** (`0x32c858` case 6 → `0x361f98` / `0x362010(dest)` / `0x362138`; the frame `0x364260`; leaving it
//! `0x3621c8`):
//! * the take-off (`0x361f98` → `0x361d08(0)`): Ratchet in the scene state (0x61) and hidden, the parked ship hidden
//!   (mode bit 1), the 0x4a / 0xcb mobys hidden (mode bit 7); the scene lump 1 (3 with Clank on his back) of
//!   `SPACE.WAD` (rc_formats::disc::space) behind a 6-frame fade, its speech stream 5002 (5005). The scene plays
//!   **in the ship's frame** ([`ShipFrame`]): each actor's track and the camera's record are turned by the ship's yaw
//!   and moved to its place with z on the ground; the actors face the ship's yaw; a class-0 actor is Ratchet, 10 Clank,
//!   0x215 / 0x10d1 the ship (crate::scene plays it: the fade 0.125 a tick, any button past tick 12 skips to the
//!   last 30 ticks). At its end the actors go, the parked ship shows, and the galaxy map opens (mode 3 kind 4).
//! * the galaxy map (page `0x286ea8`; [`Menu`], [`menu_tick`], [`menu_draws`]): the planets in the order they were
//!   unlocked (`0x139948`) as a list (widget `0x287048`, `0x350398` / `0x3529e8`), the map (`0x4325a0` / `0x432970`:
//!   the icons `0x28ec20` over the picture), the title, "✕ Select" and "△ Exit"; Up / Down (L1 / R1) move the row
//!   and select its planet (`0x307388`, sound 3), ✕ opens the planet page (`0x350030` case 3, sound 0x12), △ /
//!   Start / Select select the current planet and close (sound 4).
//! * the planet page (page `0x2870a0`: the HUD panel's widget `+0x3ba58` `0x435ac8` / `0x435f10`, the missions
//!   widget `0x351b90` / `0x353fb0`): "Planet X" (`0x3598d0`), its place, the platinum bolts found of its total
//!   (`0x327cd8` / `0x281730`); L1 / R1 step through the unlocked planets of the map (sound 3), △ goes back to the
//!   map, ✕ travels (`0x34fe48`: to the current planet, the landing; to another, mode 6 (1, dest); to Maktar before
//!   the Wupash fight, level 0x19; from a space level, the level change at once `0x2f50f8`), Start / Select close on
//!   the current planet.
//! * closing the pages (`0x3455d8`: the post-action 2, `0x32c780(0, selected)`) goes back to mode 6 with the planet:
//!   `0x362010(dest)`: the current planet → the landing scene (lump 0 / 2, stream 5001 / 5004, a 2-frame fade);
//!   another → the autosave `0x30d658(0, dest)`, `0x1b1940 = dest` and the fly-away ([`FlyAway`], sub 1).
//! * the fly-away (`0x364260` sub 1): the stream 0x138e; Ratchet (and Clank) made as riders on the seats
//!   (−0.2, 0.5, 1) / (−0.2, −0.46, 1.46) of the ship's frame; the script camera `0x428880(view, 1, 0, 0)` eased
//!   between the two cuboids (position lerp, angles by `0x2efea8`, the blend approached at 1/1800 up to 1/120 a tick,
//!   `0x31ed78`); the path's turn rates as its points' w (13° at the sharpest, the first 20 ramped), its first
//!   segment's length; 120 ticks of easing from the ship's pose to the path's start (`(1 − cos(πt/90))/2`, 1 from
//!   tick 90), the fade in by 0.125, the exhaust `0x363f78` (not ported); then the speed +0.8 a tick to 100, t +=
//!   speed/60/segment, ✕ / △ or the last 12 segments' worth start the fade out (0.1 a tick), the pose on the path
//!   (`0x343fb8`: the lerp of its points, the pitch and yaw of the lerped direction, the roll of the w's), the
//!   trail `0x364108` (not ported); past the end or the fade at 1 the level ends (`0x1a8ed4 = 1`).
//! * the arrival (`entry` → `0x32c5f0(6, 1, 6, 0, 0)` → `0x362138`): when the flight ran (`0x152c2c`), on a planet
//!   that has one (not 0, 5, 10, 15, nor above 0x14 but 0x18; not Barlow with item 0x27 before its flag, Dobbo after
//!   its, Todano before its mission byte), the landing scene from tick 1 without a fade.
//! * leaving mode 6 (`0x3621c8`): Ratchet shown; after a landing from the menu he is put at the hatch spot
//!   (`0x2b9e68`, the yaw turned −2.3 first) and the screen fades in over 8 ticks (`0x2d9d68(8)`); after the arrival
//!   he stays where the level put him (`0x2c8e30(1)`, `0x2d5438`).
//!
//! **The flight between the planets** (`0x367400` → `0x364260` sub 2, `0x363360`; [`Flight`], [`flight_kind`]): the
//! transition lump's random variants with the ship actor, the sky shells and the planet picture with its caption
//! over the load (the engine's crate::sky_render and its HUD layer draw them).
//!
//! **The pages' pictures** ([`page_pictures`]): the map page's backdrop ([`Backdrop`]: the selected planet's photo,
//! `HUD.WAD` header +0x10a8, over the whole screen, `0x354368` / `0x3546e0`), the planet page's planet
//! (rc_formats::disc::planet_picture, `0x351410` / `0x355508`). **The missions list** ([`missions`], `0x30faf8` over
//! `0x281560[level]`, drawn by `0x353fb0`: the rows wrapped into the box, a check box each, the done ones marked;
//! Up / Down walk them `0x351b90`). **○ on the planet page** replays the planet's movie when seen ([`MenuOut::Movie`]).
//! **The "new planet" banner** ([`new_planet_banner`], `0x325270`). **The ship's shadow** ([`ShipShadow`],
//! `0x361af8`).
//!
//! Not ported: the fly-away's exhaust and trail particles (`0x363f78` / `0x364108`: the particle spawner `0x3327e8`
//! has no port yet) and Ratchet's seated sequence, the speech streams, the music pause, the selection's glow
//! (`0x2e9ae0`), the planet page's window sprites' keyframes, the missions list's scroll to the selected row
//! (`+0xa4`), the fly-away's running world (the moby loop goes on under it in the original; here the game stands
//! still), the HUD prompt's fade. The ship's shadow's alpha from the view's sphere test (`0x2f0178`'s distance fade)
//! is 0x80 in view; in the fly-away its ground is the pad's (the parked ship's last probe).

pub mod missions;

use crate::game::GameState;
use crate::hero::physics::{add, add_rot, atan2, dist2, euler_rows, scale, sub, sub_rot};
use crate::hud::{Align, Draw};
use crate::moby::{Ctx, MobyHost, MobyWorld};
use crate::pad::{button, PadState};

type V3 = [f32; 3];

/// Ratchet's ship.
pub const SHIP: i16 = 0x0d54;
/// `0x12ea`: "△ Enter ship" (the prompt's text; crate::prompt, the ship's lease is owner 2).
pub const PROMPT_TEXT: u32 = 0x12ea;
pub const PROMPT_OWNER: i32 = 2;
/// The levels (`0x1a7bd0`, `0x1a7bf0`: 0x1c bytes).
pub const LEVELS: usize = 0x1c;
/// `0x281790`: per level the planet's name and its place (text ids; −1 none).
pub const PLANETS: [(i32, i32); 27] = [
    (0xb45, 0xb2b), (0xb46, 0xb2c), (0xb47, 0xb2d), (0xb48, 0xb2e), (0xb49, 0xb2f), (0xb4a, 0xb30), (0xb4b, 0xb31),
    (0xb4c, 0xb32), (0xb4d, 0xb33), (0xb4e, 0xb34), (0xb4f, 0xb35), (0xb50, 0xb36), (0xb51, 0xb37), (0xb52, 0xb38),
    (0xb53, 0xb39), (0xb54, 0xb3a), (0xb55, 0xb3b), (0xb56, 0xb3c), (0xb57, 0xb3d), (0xb58, 0xb3e), (0xb59, 0xb3f),
    (0xb5a, 0xb40), (0xb5b, 0xb41), (0xb5c, 0xb42), (0xb5d, 0xb43), (0x1b23, -1), (0x1b24, 0x1b22),
];
/// `0xb44`: "Planet" (`0x3598d0`'s "%s %s" with the name; the plain name for 2, 5, 10, 0x17..0x1a).
pub const PLANET_WORD: u32 = 0xb44;
/// `0x28ec20`: the map's icons for the first 0x15 levels: the offset from the map's anchor, the icon 0xe99e's
/// frame, the size.
pub const MAP_ICONS: [(i32, i32, u16, f32); 21] = [
    (-99, -73, 0, 16.0), (15, -109, 2, 12.0), (58, -118, 1, 21.2), (104, -118, 0, 10.4), (137, -122, 0, 21.7), (170, -79, 2, 12.0), (163, -43, 0, 14.0),
    (185, -36, 1, 16.7), (148, -1, 2, 16.0), (114, 24, 0, 12.0), (62, 8, 1, 14.0), (6, -20, 2, 18.0), (-10, -52, 2, 16.0), (10, -89, 1, 12.0),
    (35, -98, 2, 14.0), (61, -82, 0, 18.0), (103, -80, 1, 16.0), (92, -17, 2, 12.0), (92, -39, 0, 14.0), (45, -19, 1, 18.0), (22, -58, 0, 27.7),
];
/// `0x281730`: the platinum bolts of the first 0x15 levels (`0x327d60`).
pub const PLATINUM: [i32; 21] = [0, 2, 2, 2, 2, 1, 3, 2, 3, 2, 1, 2, 3, 3, 1, 1, 1, 2, 2, 3, 2];
/// The space scenes' ids in crate::scene (not a level scene: past the seen bits).
pub const SCENE_ID_BASE: i32 = 0x1000;
/// Wupash Nebula (`0x34fe48`: the way to Maktar until its fight is done).
pub const WUPASH: i32 = 0x19;
pub const MAKTAR: i32 = 2;

/// `0x361628`: the hatch spot in the ship's frame and its angles (the yaw added to the ship's); `0x3621c8`: the yaw
/// turned by −2.3 for the first placement.
const HATCH_OFFSET: V3 = [1.48, 2.64, 0.0];
const HATCH_ROT_XY: [f32; 2] = [-0.021_119_2, 0.058_817_4];
const HATCH_YAW_ADD: f32 = 1.225_219_7;
const EXIT_YAW_ADD: f32 = -2.3;
/// `0x1ac344`: the ship sits this far above the ground (`0x361050`); +0.75 while something stands on it.
const GROUND_LIFT: f32 = 0.4;
/// `0x361050`'s ease of the ship's z (`0x31ed78(target, 1/1800, 1/1800, 1/60)`).
const Z_ACCEL: f32 = 1.0 / 1800.0;
const Z_MAX: f32 = 1.0 / 60.0;
/// `0x1a8f80`: the engine part that glows, by the hull's bit (`0x1a7af8 >> 3 & 2`: shorts 0x10d7 / 0x10d9).
const GLOW_PARTS: [i16; 2] = [0x10d7, 0x10d9];
/// `0x1ac570` / `0x1ac580`: Ratchet's and Clank's seats in the fly-away.
const SEAT_RATCHET: V3 = [-0.2, 0.5, 1.0];
const SEAT_CLANK: V3 = [-0.2, -0.46, 1.46];
/// The fly-away's camera blend (`0x31ed78(1, 1.85e-4, 1.85e-4, 1/120)`).
const CAM_ACCEL: f32 = 1.85e-4;
const CAM_MAX: f32 = 1.0 / 120.0;
/// 13° (`0x364260`: the sharpest turn's roll).
const MAX_ROLL: f32 = 0.226_892_83;

/// The galaxy map's anchor (`0x4321d0`: (255, 207)) and the planet page's (`0x435738`: (255, 200)).
const MAP_AT: [f32; 2] = [255.0, 207.0];
const PAGE_AT: [f32; 2] = [255.0, 200.0];
/// The map page's list box (page `0x286ea8` rect 1: centre (125, 166), 186 × 186).
const LIST_BOX: [f32; 4] = [32.0, 73.0, 186.0, 186.0];
/// The map widget's sub-widgets (`0x4321d0`, `0x4325a0`): the window sprites, the title, the hints, the picture.
const MAP_WINDOW: [(u32, u32); 3] = [(0x87, 0x6044_2d00), (0x88, 0x6024_1700), (0x89, 0x55f0_c070)];
const MAP_TITLE: (u32, [f32; 2]) = (0x2bf6, [-70.0, -182.0]);
const MAP_EXIT: (u32, [f32; 2]) = (0x2be5, [-202.0, 124.0]);
const MAP_SELECT: (u32, [f32; 2]) = (0x2c0b, [-202.0, 101.0]);
const MAP_PICTURE: (u16, u16, [f32; 2], [f32; 2], u32) = (0xe99a, 10, [-20.0, -140.0], [240.0, 194.0], 0x70a0_c0c0);
const MAP_ICON: u16 = 0xe99e;
const ICON_VISITED: u32 = 0x7029_a1ff;
const ICON_UNVISITED: u32 = 0x70a0_c0c0;
/// The reticle: sprite 0x8a (`0x80ffde8d`) at the eased x (anchor y + 55 and − 141, the second upside down) and at
/// the eased y (anchor x − 25); its lines in `0x180000ff`.
const RETICLE_SPRITE: (u32, u32) = (0x8a, 0x80ff_de8d);
const RETICLE_LINE: u32 = 0x1800_00ff;
/// The planet page's sub-widgets (`0x435738`, `0x435d00`, `0x435df0`): the window sprites `0x1ae218` with the
/// colours `0x1ae058`, the hints, the place, the title, the counts.
const PAGE_WINDOW: [(u32, u32); 4] = [(0xa2, 0x6044_2d00), (0xa3, 0x6024_1700), (0xa4, 0x55f0_c070), (0xa5, 0x55f0_c070)];
const PAGE_HINTS: [(u32, [f32; 2]); 3] = [(0x2be5, [-190.0, 80.0]), (0x3129, [-190.0, 104.0]), (0x2c67, [-190.0, 128.0])];
/// `0x435ac8`'s places from the anchor (`gp − 0xdb0..`: `0x1ae240..`, x then y): the title "Planet X" (widget
/// `+0x5d0`) in the top-left bar (−119, −162), the place (`+0x628`) in the top-right bar (118, −165), the icon 0xeaa2
/// (`+0x2dc`) in the bottom-right box (−5, 100), the sprite 0x5d (`+0x420`) beside it (10, 117); `0x435d00` /
/// `0x435df0`: "Platinum Bolt" (`+0x578`, (45, 109)) at 0.9, the "Found:" label (`+0x470`, (45, 129)) with its count
/// right-aligned 150 further (`gp − 0xd48` = 0x1ae2a8: 150).
const PAGE_PLACE_AT: [f32; 2] = [118.0, -165.0];
const PAGE_TITLE_AT: [f32; 2] = [-119.0, -162.0];
const PAGE_SMALL: (u32, [f32; 2]) = (0x333e, [45.0, 109.0]);
const PAGE_FOUND: (u32, [f32; 2], f32) = (0x2c51, [45.0, 129.0], 150.0);
const PAGE_ICON: (u16, [f32; 2], u32) = (0xeaa2, [-5.0, 100.0], 0x60f0_f0b0);
const PAGE_SPRITE: (u32, [f32; 2], u32) = (0x5d, [10.0, 117.0], 0x55f0_c070);
/// The missions widget's title (`0x353fb0`: text 0x2c5b in `0x80ffa888` at its rect; page `0x2870a0` rect 2).
const MISSIONS_TITLE: (u32, [f32; 2]) = (0x2c5b, [248.0, 78.0]);
/// The missions widget's text box (`0x2eaf70`'s box: the widget's `+0x20` = 455 is its right edge, `+0x24` = 265
/// its bottom; the text starts at x + 16 = 264): 191 pixels a line.
const MISSIONS_WRAP: i32 = 455 - (248 + 16);
const MISSIONS_BOTTOM: f32 = 265.0;

/// The text's words laid into `budget` pixels a line (`0x2eaf70`'s wrap, as the port measures it): a word past the
/// budget starts the next line; a line is never empty.
fn wrap_words(text: &[u8], budget: i32, width: &dyn Fn(&[u8]) -> i32) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    for word in text.split(|&b| b == b' ') {
        match out.last_mut() {
            Some(l) if width(&[l.as_slice(), b" ", word].concat()) <= budget => { l.push(b' '); l.extend_from_slice(word); }
            _ => out.push(word.to_vec()),
        }
    }
    out
}
const TEXT_WHITE: u32 = 0x80f0_f0f0;
const TEXT_SELECTED: u32 = 0x8020_ffff;
/// `0x1aa79c` (`gp − 0x4854`): the check box's inner colour (`0x3542c0`).
const CHECK_INNER: u32 = 0x8024_1700;

/// `0x2d6ac8`: the levels where the ship is in flight (no take-off: the level change at once).
pub fn space_level(level: i32) -> bool { matches!(level, 0x19 | 5 | 10 | 0xf) }
/// `0x3598d0`'s plain-name levels.
fn plain_name(level: i32) -> bool { matches!(level, 2 | 5 | 10 | 0x17..=0x1a) }
/// A planet of the galaxy map (`0x322730`: below 0x15, or 0x18).
fn on_map(level: i32) -> bool { level < 0x15 || level == 0x18 }

/// The ship mode's phase (`0x152c20` and the mode: 0 the take-off scene, the pages in mode 3, 1 the fly-away, 3 the
/// landing scene).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    TakeOff,
    Menu,
    FlyAway,
    Landing,
}

/// The frame a ship scene plays in (`0x364260`: the ship's place with z on the ground, its yaw).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipFrame {
    pub at: V3,
    pub yaw: f32,
}

impl ShipFrame {
    /// The frame's rows (the yaw about z).
    pub fn rows(&self) -> [V3; 3] { euler_rows([0.0, 0.0, self.yaw]) }

    /// `0x2ef340` + `0x2eee70`: a scene point in the world.
    pub fn to_world(&self, p: V3) -> V3 {
        let r = self.rows();
        add(add(add(scale(r[0], p[0]), scale(r[1], p[1])), scale(r[2], p[2])), self.at)
    }

    /// A direction turned by the yaw.
    pub fn turn(&self, v: V3) -> V3 {
        let r = self.rows();
        add(add(scale(r[0], v[0]), scale(r[1], v[1])), scale(r[2], v[2]))
    }
}

/// The ship and the trip (`0x152c00..`, `0x1b1a60`, `0x1b1940`).
#[derive(Clone, Debug, Default)]
pub struct Travel {
    /// `0x152c00`: the parked ship; `0x152c40` / `0x152c44` / `0x152c48`: the fly-away's path and camera cuboids.
    pub ship: Option<usize>,
    pub path: i32,
    pub cam_a: i32,
    pub cam_b: i32,
    /// `0x1b1a60`: the take-off asked for; `0x152c70` / `0x152c80`: the hatch spot and its angles.
    pub enter: bool,
    pub hatch: Option<(V3, V3)>,
    pub phase: Phase,
    /// The scene's frame while a ship scene plays.
    pub frame: Option<ShipFrame>,
    /// `0x152c2c`: the flight ran to its arrival (cleared by every level change `0x367400`, set by the arrival
    /// variant's end, [`Flight::tick`]): the next level starts with the landing; `0x152c2a`: the level's start (the
    /// landing there leaves Ratchet where the level put him).
    pub flight_ran: bool,
    pub level_start: bool,
    /// `0x1b1940`: the destination.
    pub dest: i32,
    /// `gp−0x2d80`: the story chain the level change picked ([`story_pick`]), played at the flight's first variant's
    /// end ([`story_chain`]) and cleared.
    pub story: i32,
    pub menu: Menu,
    pub fly: Option<FlyAway>,
    /// The flight between the planets while the next level loads ([`Flight`]).
    pub flight: Option<Flight>,
    /// `0x361050`'s z ease: the velocity (`gp − 0x2ca4`).
    z_vel: f32,
    /// The ship's shadow quad this tick ([`ShipShadow`], `0x361af8`), and the ground under the parked ship.
    pub shadow: Option<ShipShadow>,
    shadow_ground: Option<f32>,
}

/// `0x361af8`, the ship's draw callback: a quad of FX texture 0 (`0x2e71b8(0)`, ALPHA 0x44) under the ship: the
/// corners (±4, ±1.6, 0) (`gp − 0x2c80`) turned by the ship's rows at its position, at the ground under it + 0.1
/// (`0x31ce20(0.5, pos)` in the ship mode and in play, else the ship's z + 0.1), colour `alpha / 2 · 0x1000000 |
/// 0x808080` with `alpha` from the view's sphere test (`0x2f0178(32, pos)`: 0x80 in view), less `4 · (frame −
/// 0x96)` past frame 0x96 of the fly-away (mode 6 sub 1); nothing at 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipShadow {
    pub centre: V3,
    pub corners: [V3; 4],
    pub alpha: i32,
}

const SHADOW_CORNERS: [[f32; 2]; 4] = [[4.0, 1.6], [4.0, -1.6], [-4.0, 1.6], [-4.0, -1.6]];
const SHADOW_ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

impl ShipShadow {
    fn new(pos: V3, rows: [V3; 3], ground: Option<f32>, alpha: i32) -> ShipShadow {
        let z = ground.unwrap_or(pos[2]) + 0.1;
        let corners = SHADOW_CORNERS.map(|[x, y]| { let c = add(add(scale(rows[0], x), scale(rows[1], y)), pos); [c[0], c[1], z] });
        ShipShadow { centre: pos, corners, alpha }
    }
}

/// The callback's draw (crate::fx::callbacks): the quad while the ship is parked or flying away and in view.
pub fn draw_ship_shadow(g: &GameState, view: &crate::particles::BSphereView, out: &mut Vec<crate::fx::FxQuad>) {
    if !matches!(g.travel.phase, Phase::Idle | Phase::FlyAway) { return; }
    let Some(sh) = g.travel.shadow else { return };
    if sh.alpha <= 0 || view.culled(32.0, [sh.centre[0], sh.centre[1], sh.centre[2], 1.0]) { return; }
    let rgba = ((sh.alpha >> 1) as u32) << 24 | 0x0080_8080;
    out.push(crate::fx::FxQuad { fx: 0, additive: false, corners: sh.corners, st: SHADOW_ST, rgba: [rgba; 4] });
}

impl Travel {
    /// What the level change carries (`entry`: `0x152c2a = 1`, the flight's flag kept for the arrival).
    pub fn carried(old: Travel) -> Travel { Travel { flight_ran: old.flight_ran, level_start: true, ..Default::default() } }

    /// The game stops for the ship mode's own phases (the pages, the fly-away).
    pub fn blocks_play(&self) -> bool { matches!(self.phase, Phase::Menu | Phase::FlyAway) }

    /// The scene's rows for its actors (identity outside a ship scene).
    pub fn scene_rows(&self) -> [V3; 3] { self.frame.map_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], |f| f.rows()) }

    /// `0x361d08`'s lump: the landing (0, 2 with Clank) or the take-off (1, 3).
    pub fn lump(take_off: bool, clank: bool) -> usize { (take_off as usize) + 2 * (clank as usize) }

    /// `entry`'s landing test (module docs) for the level just entered.
    pub fn lands_on_arrival(&self, g: &GameState) -> bool {
        let level = g.level;
        if !self.flight_ran || space_level(level) || level == 0 || !on_map(level) { return false; }
        // Barlow with item 0x27 needs the flag `0x1395ca`, Dobbo (9) is out with `0x1395e5`, Todano (0xe) needs
        // its mission byte `0x19b3d2`: the port keeps none of the two flags (0 here).
        if level == 4 && g.items.get(0x27).is_some_and(|&b| b != 0) { return false; }
        if level == 0xe && !g.missions.get(0xe * 0x10 + 0xa).is_some_and(|&b| b != 0) { return false; }
        true
    }
}

/// `0x3012a0`: the parked ship from the level settings (module docs); none when the settings put it at x ≤ 0 or the
/// level has no class 0x0d54.
pub fn park_ship(w: &mut MobyWorld, host: &dyn MobyHost, g: &mut GameState, gameplay: &[u8]) -> Option<usize> {
    let Some(s) = rc_formats::disc::gameplay::level_settings(gameplay).ok().flatten() else {
        println!("travel: no level settings: no parked ship");
        return None;
    };
    let f = |o: usize| s.get(o..o + 4).map(|b| f32::from_le_bytes(b.try_into().unwrap()));
    let i = |o: usize| s.get(o..o + 4).map_or(-1, |b| i32::from_le_bytes(b.try_into().unwrap()));
    let t = &mut g.travel;
    (t.path, t.cam_a, t.cam_b) = (i(0x4c), i(0x50), i(0x54));
    let (x, y, z, yaw) = (f(0x3c)?, f(0x40)?, f(0x44)?, f(0x48)?);
    if x <= 0.0 {
        println!("travel: the level parks no ship (settings +0x3c: ({x}, {y}, {z}), yaw {yaw})");
        return None;
    }
    if w.class(SHIP).is_none() {
        println!("travel: no class 0x0d54 in the level: no parked ship (settings +0x3c: ({x}, {y}, {z}))");
        return None;
    }
    let id = w.create(SHIP);
    let m = &mut w.mobys[id];
    m.pos = [x, y, z];
    m.rot = [0.0, 0.0, yaw];
    m.draw_dist = 0xff;
    m.update_dist = 0x10;
    m.drawn = 1;
    m.update = Some(ship_update);
    m.mode &= !crate::moby::mode::NO_UPDATE;
    if m.shine != 0 { m.shine = 8; }
    w.rebuild(id, host);
    t.ship = Some(id);
    Some(id)
}

/// `0x31ed78(target, accel, accel, max, &v, &vel)`: `v` toward `target` with a velocity that builds by `accel` a
/// tick to `max`, braking so as to land on it; the step taken.
fn approach(target: f32, accel: f32, max: f32, v: &mut f32, vel: &mut f32) -> f32 {
    let d = target - *v;
    if *vel == 0.0 && d == 0.0 { return 0.0; }
    if *vel * d < 0.0 {
        *vel += if 0.0 <= *vel { -accel } else { accel };
        *v += *vel;
        return *vel;
    }
    let brake = (*vel * *vel / accel) * 0.5;
    if d.abs() < brake {
        if brake < d.abs() + vel.abs() {
            // `0x31eb18(0, accel·1.1)`: the velocity toward 0.
            let a = accel * 1.1;
            *vel = if vel.abs() <= a { 0.0 } else { *vel - a * vel.signum() };
        }
    } else {
        let want = (accel * 2.0 * d.abs()).sqrt().min(max) * d.signum();
        // `0x31eb18(want, accel)`: the velocity toward `want` by `accel`.
        let step = want - *vel;
        *vel += step.clamp(-accel, accel);
    }
    if d.abs() <= vel.abs() {
        *v = target;
        return d;
    }
    *v += *vel;
    *vel
}

/// `0x31ce20(0.5, p)`: the ground under `p` (a line from 0.5 above it straight down).
fn ground(ctx: &Ctx, p: V3) -> Option<f32> {
    let c = ctx.coll?;
    crate::collision::coll_line(c, [p[0], p[1], p[2] + 0.5], [p[0], p[1], 0.01], crate::collision::QueryFlags(2)).map(|o| o.point[2])
}

/// `0x361050`: the parked ship's tick in play: its parts made once, its z eased onto the ground + 0.4, the parts
/// placed (crate::moby::classes::props); the "stood on" lift (`+0xbc`, the probe `0x2de878` from Ratchet up 2) is
/// not ported.
fn ship_ground(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if ctx.game.scenes.mode != crate::scene::PLAY {
        if ctx.game.ship_parts.is_empty() { crate::moby::classes::props::ship_parts(w, id, ctx); }
        return;
    }
    let pos = w.mobys[id].pos;
    let target = ground(ctx, pos).map_or(pos[2], |g| g + GROUND_LIFT);
    let (mut z, mut vel) = (pos[2], ctx.game.travel.z_vel);
    approach(target, Z_ACCEL, Z_MAX, &mut z, &mut vel);
    ctx.game.travel.z_vel = vel;
    w.mobys[id].pos[2] = z;
    // The arrival's own ship (0x1330) places the parts on itself while it flies by; the parked ship (at Aranos's
    // placeholder (20, 20, 20)) leaves them to it then [port].
    let flyby = w.mobys.iter().any(|m| m.o_class == 0x1330 && m.state < 0x80 && (1..=3).contains(&m.state));
    if !flyby { crate::moby::classes::props::ride_ship_parts(w, id, ctx); }
}

/// `0x361628`: the parked ship's update (module docs).
pub fn ship_update(w: &mut MobyWorld, id: usize, ctx: &mut Ctx) {
    if ctx.game.travel.ship != Some(id) { return; }
    ship_ground(w, id, ctx);
    {
        let m = &w.mobys[id];
        let g = ground(ctx, m.pos);
        let t = &mut ctx.game.travel;
        t.shadow_ground = g;
        t.shadow = Some(ShipShadow::new(m.pos, m.rows, g, 0x80));
    }
    if w.mobys[id].drawn != 0 {
        let cmd = w.mobys[id].pv_b(0xe).wrapping_add(2);
        w.mobys[id].pvars[0xe] = cmd;
        let v = (((cmd as i32 - 0x80) as f32 * 0.024_543_693).cos() * 50.0) as i32 + 0x8c;
        let glow = (v | v << 8 | v << 16) as u32;
        let part = GLOW_PARTS[0];
        for &p in &ctx.game.ship_parts {
            if let Some(m) = w.mobys.get_mut(p).filter(|m| m.o_class == part) {
                m.glow = glow;
                m.mode |= crate::moby::mode::GLOW;
            }
        }
    }
    let hero = ctx.hero.pos;
    let pos = w.mobys[id].pos;
    let riding = ctx.hero.ground_moby.is_some_and(|r| w.mobys.get(r).is_some_and(|m| m.o_class == SHIP && m.state != 0xfe && m.state != 0xfd));
    let other = ctx.hero.ground_moby.is_some_and(|r| w.mobys.get(r).is_some_and(|m| m.o_class != SHIP));
    let d3 = ((pos[0] - hero[0]).powi(2) + (pos[1] - hero[1]).powi(2) + (pos[2] - hero[2]).powi(2)).sqrt();
    let d2 = dist2(pos, hero);
    let mut near = w.mobys[id].pv_i(0xc) as i16 != 0;
    if !near {
        if (d3 < 8.0 || riding) && !other && (d2 < 6.0 || riding) { near = true; }
    } else if 6.1 < d2 && !riding {
        near = false;
    }
    if w.mobys[id].mode & 1 != 0 { near = false; }
    w.mobys[id].set_pv_i(0xc, near as i32);
    if !near { return; }
    // `0x327a28(2, 0x12ea, 1)`: the prompt's lease; △ (`0x138324 & 0x10`) with it: the hatch spot, the lease
    // released (`0x327b60(2)`), the take-off asked for.
    let lease = ctx.game.prompt.request(PROMPT_OWNER, PROMPT_TEXT, 1);
    if lease != 0 && ctx.hero.items.pressed & button::TRIANGLE != 0 {
        let m = &w.mobys[id];
        let r = m.rows;
        let mut at = add(add(add(scale(r[0], HATCH_OFFSET[0]), scale(r[1], HATCH_OFFSET[1])), scale(r[2], HATCH_OFFSET[2])), m.pos);
        if let Some(g) = ground(ctx, at) { at[2] = g; }
        let yaw = add_rot(HATCH_YAW_ADD, w.mobys[id].rot[2]);
        let p = &mut ctx.game.prompt;
        if p.owner == PROMPT_OWNER { (p.owner, p.hold, p.text, p.show) = (0, 0, 0, 0); }
        let t = &mut ctx.game.travel;
        t.hatch = Some((at, [HATCH_ROT_XY[0], HATCH_ROT_XY[1], yaw]));
        t.enter = true;
    }
}

/// The ship scene's frame for the parked ship (`0x364260`: its place with z on the ground, its yaw).
pub fn ship_frame(w: &MobyWorld, g: &GameState, ground: &dyn Fn(V3) -> Option<f32>) -> Option<ShipFrame> {
    let m = w.mobys.get(g.travel.ship?)?;
    let mut at = m.pos;
    if let Some(z) = ground(at) { at[2] = z; }
    Some(ShipFrame { at, yaw: m.rot[2] })
}

/// `0x3621c8`'s placement after a landing from the menu: Ratchet at the hatch spot (the yaw turned −2.3 first, then
/// as stored), idle, the screen in over 8 ticks.
pub fn hatch_exit(g: &mut GameState, hero: &mut crate::hero::Hero) {
    if let Some((at, rot)) = g.travel.hatch {
        hero.teleport(at, [rot[0], rot[1], add_rot(rot[2], EXIT_YAW_ADD)]);
        hero.teleport(at, rot);
        hero.request_state(0, true);
    }
    g.scenes.fade = 1.0;
    g.scenes.fade_in = 1.0 / 8.0;
}

// ---------------------------------------------------------------------------------------------------------------
// The pages

/// What a frame of the pages asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOut {
    /// `0x351b90`: ○ on the planet page with the level's movie seen (`0x139768`): the movie queued (`0x2f4ee8`) and
    /// started (`0x2f5030`); the pages come back after it.
    Movie(i32),
    /// Sound n of the GUI's set (`0x360750(n, 0, 0)`).
    Sound(u32),
    /// The pages close for mode 6 with the planet (`0x362010(dest)`): the landing on the current one, else the
    /// fly-away.
    Travel(i32),
    /// `0x2f50f8(dest, 1)`: the level change at once (from a space level).
    Leave(i32),
}

/// The page shown (`0x1febd4`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Map,
    Planet,
}

/// The pages' state: the map list's row (`+0x40` of `0x287048`), the planet (`0x1c9550`), the reticle (`0x4325a0`:
/// the eased x and y from (56, −50)), the page fade (`0x344400(n)`: n frames) and the fader's frames.
#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub page: Page,
    pub row: usize,
    pub selected: i32,
    pub reticle: [f32; 2],
    pub fade_frames: i32,
    pub fade: i32,
    /// `0x1fec9b`-like: the list's scroll (`+0x44` of the list widget), in pixels.
    pub scroll: i32,
    pub frame: u32,
    /// The levels' missions tables ([`missions`], read from Aranos's overlay by the engine) and the missions widget's
    /// row per level (`+0x30 + 4·level`).
    pub missions: Vec<Vec<missions::Mission>>,
    pub mission_row: Vec<usize>,
    /// The missions widget's scroll (`+0xa4`: the draw moves it 4 a frame to keep the selected row in the box).
    pub mission_scroll: i32,
    pub backdrop: Backdrop,
}

/// The map page's picture widget `0x286fd8` (`0x354368`, flags 0x40022): the selected planet's photo (`HUD.WAD`
/// header +0x10a8, row = level; 256 × 128) streamed when the selection changes and drawn over the whole screen
/// (`0x3546e0`: `0x2e89d8(0, 0, frame w, frame h, …)`). `+0x44` is the state (1 first load, 2 shown, 3 the next one
/// loading), `+0x50` the level shown, `+0x5c` the frames in the state: with flag 0x40000 a load takes 5 frames and
/// the picture fades in (state 2: alpha `frames · 32`, to 0x80) and out (state 3: `(4 − frames) · 32`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Backdrop {
    pub state: u8,
    pub shown: i32,
    pub next: i32,
    pub frames: i32,
}

impl Backdrop {
    pub fn new() -> Backdrop { Backdrop { state: 0, shown: -1, next: -1, frames: 0 } }

    /// `0x354368` for the selected level.
    fn tick(&mut self, selected: i32) {
        let selected = selected.max(0);
        match self.state {
            1 | 3 => {
                if 4 < self.frames {
                    self.shown = self.next;
                    self.state = 2;
                    self.frames = 0;
                    return;
                }
            }
            2 if selected != self.shown => {
                self.next = selected;
                self.state = 3;
                self.frames = 0;
                return;
            }
            0 => {
                self.next = selected;
                self.state = 1;
                self.frames = 0;
                return;
            }
            _ => {}
        }
        self.frames = (self.frames + 1).min(0x100);
    }

    /// The level drawn and its alpha (`0x3546e0`), once something is loaded.
    pub fn draw(&self) -> Option<(i32, u8)> {
        if self.state < 2 || self.shown < 0 { return None; }
        let a = if self.state == 2 { (self.frames * 32).min(0x80) } else { ((4 - self.frames) * 32).max(0) };
        Some((self.shown, a as u8))
    }
}

/// The map's rows: the planets in the order they were unlocked (`0x139948`, the first 20 slots).
fn map_rows(g: &GameState) -> Vec<i32> { g.quick_select.iter().take(20).copied().take_while(|&l| l != 0).collect() }

/// `0x307148(level)`: the level's row in the list; none for a level not in it, nor for level 0 away from it.
fn row_of(g: &GameState, level: i32) -> Option<usize> {
    let rows = map_rows(g);
    let k = rows.iter().position(|&l| l == level)?;
    if level == 0 && g.level != 0 { return None; }
    Some(k)
}

/// `0x3448a8` kind 4 / `0x344498`: the map opens on the current planet, fading in over 6 frames.
pub fn open_menu(g: &mut GameState) {
    let level = g.level;
    let row = row_of(g, level).unwrap_or(0);
    let missions = std::mem::take(&mut g.travel.menu.missions);
    g.travel.menu = Menu { page: Page::Map, row, selected: level, reticle: [56.0, -50.0], fade_frames: 6, fade: 0, scroll: 0, frame: 0, missions, mission_row: vec![0; LEVELS], mission_scroll: 0, backdrop: Backdrop::new() };
    g.travel.phase = Phase::Menu;
}

/// The pages' frame (`0x3450d0`: the map list `0x350398`, the planet page's `0x351b90` + `0x34fe48`; module docs).
pub fn menu_tick(g: &mut GameState, pad: &PadState) -> Vec<MenuOut> {
    let mut out = Vec::new();
    let pressed = pad.pressed;
    let level = g.level;
    // `0x1a7c09`: the Wupash Nebula's visited byte (`0x1a7bf0[0x19]`, set by its win): until then a trip to Maktar
    // goes to the fight first (`0x34fe48`).
    let wupash = g.visited.get(WUPASH as usize).is_some_and(|&v| v != 0);
    let rows = map_rows(g);
    let unlocked: Vec<bool> = (0..LEVELS as i32).map(|l| g.planets.get(l as usize).is_some_and(|&b| b != 0)).collect();
    let in_list = |l: i32| rows.contains(&l) && !(l == 0 && level != 0);
    let flags = missions::Flags { planets: &g.planets, items: &g.items, visited: &g.visited, scenes_seen: &g.scenes_seen };
    let m = &mut g.travel.menu;
    m.frame = m.frame.wrapping_add(1);
    m.backdrop.tick(m.selected);
    if m.fade < m.fade_frames {
        m.fade += 1;
        return out;
    }
    match m.page {
        Page::Map => {
            if pressed & (button::TRIANGLE | button::START | button::SELECT) != 0 {
                // `0x350398`: △, Start, Select close on the current planet (the flag 0x20: `0x307388(level)`).
                m.selected = level;
                out.push(MenuOut::Sound(4));
                out.push(MenuOut::Travel(level));
                return out;
            }
            let before = m.row;
            if pressed & (button::UP | button::L1) != 0 && 0 < m.row { m.row -= 1; }
            if pressed & (button::DOWN | button::R1) != 0 && m.row + 1 < rows.len() { m.row += 1; }
            if m.row != before {
                out.push(MenuOut::Sound(3));
                m.selected = rows[m.row];
            }
            if pressed & button::CROSS != 0 && !rows.is_empty() {
                // `0x350030` case 3: the planet page (the new page's sound 0x12).
                m.page = Page::Planet;
                m.fade_frames = 0;
                out.push(MenuOut::Sound(0x12));
            }
        }
        Page::Planet => {
            if pressed & (button::START | button::SELECT) != 0 {
                // `0x34fe48`: the pages close on the current planet.
                m.selected = level;
                out.push(MenuOut::Travel(level));
                return out;
            }
            // `0x351b90`: R1 / L1 to the next / previous unlocked planet (the current one stops the walk), when it is
            // on the map's list.
            let before = m.selected;
            for (bit, step) in [(button::R1, 1i32), (button::L1, -1)] {
                if pressed & bit == 0 { continue; }
                let mut l = m.selected;
                loop {
                    l += step;
                    if !(0..0x1c).contains(&l) { break; }
                    if unlocked[l as usize] || l == level {
                        if in_list(l) { m.selected = l; }
                        break;
                    }
                }
            }
            if m.selected != before { out.push(MenuOut::Sound(3)); }
            // `0x351b90`: Up / Down walk the missions list's rows (wrapping), sound 3.
            let n = usize::try_from(m.selected).ok().and_then(|l| m.missions.get(l)).map_or(0, |t| missions::rows(&flags, m.selected, t).len());
            if 0 < n {
                let l = m.selected.max(0) as usize;
                if m.mission_row.len() <= l { m.mission_row.resize(l + 1, 0); }
                let r = m.mission_row[l] % n;
                let next = if pressed & button::UP != 0 { (r + n - 1) % n } else if pressed & button::DOWN != 0 { (r + 1) % n } else { r };
                if next != r { out.push(MenuOut::Sound(3)); }
                m.mission_row[l] = next;
            }
            // ○ (`0x138344 & 0x20`): the planet's movie again: its id is the level (0 → 0x15, 0x18 → 0), when seen.
            if pressed & button::CIRCLE != 0 && 0 <= m.selected {
                let id = match m.selected { 0 => 0x15, 0x18 => 0, l => l };
                if flags.scenes_seen.get(id as usize).copied().unwrap_or(false) {
                    out.push(MenuOut::Movie(id));
                    return out;
                }
            }
            if pressed & button::TRIANGLE != 0 {
                // `0x34fe48`: △ back to the map behind a 4-frame fade (`0x2e7c90(4)`, `0x344400(4)`).
                m.page = Page::Map;
                m.fade_frames = 4;
                m.fade = 0;
                out.push(MenuOut::Sound(0x12));
            } else if pressed & button::CROSS != 0 {
                out.push(MenuOut::Sound(4));
                let dest = m.selected;
                if dest == level {
                    out.push(MenuOut::Travel(level));
                } else if space_level(level) {
                    // `0x2d6ac8` with `0x2d6b08` (the ship's fight done): the level change at once.
                    out.push(MenuOut::Leave(dest));
                } else {
                    let dest = if dest == MAKTAR && !wupash { WUPASH } else { dest };
                    out.push(MenuOut::Travel(dest));
                }
            }
        }
    }
    out
}

/// The text of the planet page's title (`0x3598d0`): "Planet X", or the name alone.
fn planet_title(level: i32, text: &dyn Fn(u32) -> Vec<u8>) -> String {
    let name = PLANETS.get(level.max(0) as usize).map_or(Vec::new(), |p| text(p.0 as u32));
    let name = String::from_utf8_lossy(name.split(|&c| c == 0).next().unwrap_or(&[])).into_owned();
    if plain_name(level) { name } else { format!("{} {}", String::from_utf8_lossy(&text(PLANET_WORD)), name) }
}

fn line(t: Vec<u8>) -> String { String::from_utf8_lossy(t.split(|&c| c == 0).next().unwrap_or(&[])).into_owned() }

/// A page text: the HUD panel's text widgets (`0x42d8b0`) draw in font 1 (`0x42d7c0`: `+0x38` = 1, the menus'
/// font, the engine's `glyphs`), the map's list (`0x3529e8`, flag 8) in font 2 (`0x2e71b8(2)`, `glyphs_2`).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuText {
    pub text: String,
    pub pos: [f32; 2],
    pub scale: f32,
    pub colour: u32,
    pub align: Align,
    /// 1 or 2 (the engine's texture slot and glyph set).
    pub font: u8,
}

/// The pages' draws (the map `0x432970` + the list `0x3529e8`; the planet page `0x435f10` + `0x353fb0`'s title):
/// the sprites and icons as draws, the texts ([`MenuText`], font 2) in `texts`. `text` gives a text id's bytes;
/// `lines` collects the reticle's lines (`0x2fbfc8`: x0, y0, x1, y1, colour).
pub fn menu_draws(g: &GameState, text: &dyn Fn(u32) -> Vec<u8>, width: &dyn Fn(u8, &[u8]) -> i32, lines: &mut Vec<([f32; 4], u32)>, texts: &mut Vec<MenuText>, scroll: &mut i32) -> Vec<Draw> {
    let mut out = Vec::new();
    let m = &g.travel.menu;
    let level = g.level;
    let visited = |l: usize| g.visited.get(l).is_some_and(|&v| v != 0);
    let unlocked = |l: usize| g.planets.get(l).is_some_and(|&v| v != 0);
    match m.page {
        Page::Map => {
            for (id, colour) in MAP_WINDOW {
                out.push(Draw::Sprite { id, pos: MAP_AT, scale: [1.0, 1.0], frame: 0.0, colour });
            }
            let at = |o: [f32; 2]| [MAP_AT[0] + o[0], MAP_AT[1] + o[1]];
            texts.push(MenuText { font: 1, text: line(text(MAP_TITLE.0)), pos: at(MAP_TITLE.1), scale: 1.0, colour: TEXT_WHITE, align: Align::Centre });
            texts.push(MenuText { font: 1, text: line(text(MAP_EXIT.0)), pos: at(MAP_EXIT.1), scale: 1.0, colour: TEXT_WHITE, align: Align::Left });
            texts.push(MenuText { font: 1, text: line(text(MAP_SELECT.0)), pos: at(MAP_SELECT.1), scale: 1.0, colour: TEXT_WHITE, align: Align::Left });
            let (id, frame, pos, size, colour) = MAP_PICTURE;
            out.push(Draw::Icon { id, frame, pos: at(pos), size, colour });
            // The planets' dots: the unlocked ones, visited in blue.
            for (l, &(x, y, frame, size)) in MAP_ICONS.iter().enumerate() {
                if !unlocked(l) { continue; }
                let colour = if visited(l) { ICON_VISITED } else { ICON_UNVISITED };
                out.push(Draw::Icon { id: MAP_ICON, frame, pos: at([x as f32, y as f32]), size: [size, size], colour });
            }
            // The reticle on the selected planet.
            if (0..0x15).contains(&m.selected) {
                let [rx, ry] = m.reticle;
                let (id, colour) = RETICLE_SPRITE;
                out.push(Draw::Sprite { id, pos: at([rx, 55.0]), scale: [1.0, 1.0], frame: 0.0, colour });
                out.push(Draw::Sprite { id, pos: at([rx, -141.0]), scale: [1.0, -1.0], frame: 0.0, colour });
                out.push(Draw::Sprite { id, pos: at([-25.0, ry]), scale: [1.0, 1.0], frame: 0.0, colour });
                let (x0, y0) = (MAP_AT[0] + rx, MAP_AT[1] - 141.0);
                let (x1, y1) = (MAP_AT[0] - 25.0, MAP_AT[1] + ry);
                lines.push(([x0 + 1.0, y0 + 6.0, x0 + 1.0, MAP_AT[1] + 55.0 - 4.0], RETICLE_LINE));
                lines.push(([x1 + 8.0, y1 + 2.0, x1 + 304.0, y1 + 2.0], RETICLE_LINE));
            }
            // The list (`0x3529e8`, flags 0x538: the rows centred, 13 a line, a sub-label under a 16-pixel line).
            let [bx, by, bw, _bh] = LIST_BOX;
            let cx = bx + (bw / 2.0).floor();
            let mut y = by + 4.0 - m.scroll as f32;
            for (k, &l) in map_rows(g).iter().enumerate() {
                let sel = k == m.row;
                let colour = if sel { TEXT_SELECTED } else { TEXT_WHITE };
                let (name, place) = PLANETS.get(l.max(0) as usize).copied().unwrap_or((-1, -1));
                if name != -1 { texts.push(MenuText { font: 2, text: line(text(name as u32)), pos: [cx, y], scale: 1.0, colour, align: Align::Centre }); }
                y += 13.0;
                if place != -1 {
                    lines.push(([cx - 8.0, y + 2.0, cx + 8.0, y + 2.0], colour & 0xff_ffff | 0x5000_0000));
                    y += 6.0;
                    texts.push(MenuText { font: 2, text: line(text(place as u32)), pos: [cx, y], scale: 1.0, colour, align: Align::Centre });
                    y += 14.0;
                }
                y += 13.0;
            }
        }
        Page::Planet => {
            for (id, colour) in PAGE_WINDOW {
                out.push(Draw::Sprite { id, pos: PAGE_AT, scale: [1.0, 1.0], frame: 0.0, colour });
            }
            let at = |o: [f32; 2]| [PAGE_AT[0] + o[0], PAGE_AT[1] + o[1]];
            for (id, o) in PAGE_HINTS {
                texts.push(MenuText { font: 1, text: line(text(id)), pos: at(o), scale: 1.0, colour: TEXT_WHITE, align: Align::Left });
            }
            let (icon, o, colour) = PAGE_ICON;
            out.push(Draw::Icon { id: icon, frame: 0, pos: at(o), size: [32.0, 32.0], colour });
            let (id, o, colour) = PAGE_SPRITE;
            out.push(Draw::Sprite { id, pos: at(o), scale: [1.0, 1.0], frame: 0.0, colour });
            let sel = m.selected;
            let place = PLANETS.get(sel.max(0) as usize).map_or(-1, |p| p.1);
            // The text widgets' default alignment is centred (`0x42d7c0`: `+0x44` = 1); the hints and the label are
            // set left (`0x42d868(w, 0)`), the count right (2).
            if place != -1 { texts.push(MenuText { font: 1, text: line(text(place as u32)), pos: at(PAGE_PLACE_AT), scale: 1.0, colour: TEXT_WHITE, align: Align::Centre }); }
            texts.push(MenuText { font: 1, text: planet_title(sel, text), pos: at(PAGE_TITLE_AT), scale: 1.0, colour: TEXT_WHITE, align: Align::Centre });
            if (1..=0x14).contains(&sel) {
                // `0x435df0`: the platinum bolts found (`0x327cd8`: the port keeps no count, 0) of the level's.
                let (id, o, dx) = PAGE_FOUND;
                texts.push(MenuText { font: 1, text: line(text(id)), pos: at(o), scale: 1.0, colour: TEXT_WHITE, align: Align::Left });
                let total = PLATINUM.get(sel as usize).copied().unwrap_or(0);
                texts.push(MenuText { font: 1, text: format!("{}/{}", 0, total), pos: at([o[0] + dx, o[1]]), scale: 1.0, colour: TEXT_WHITE, align: Align::Right });
                let (id, o) = PAGE_SMALL;
                texts.push(MenuText { font: 1, text: line(text(id)), pos: at(o), scale: 0.9, colour: TEXT_WHITE, align: Align::Left });
            }
            // The missions widget (`0x353fb0`): the title, then the rows 20 below at 16 a line: the check box at x + 9
            // (done: the mark 0xe99d frame 1), the text at x + 16 (font 2), the selected row in yellow.
            let (id, [mx, my]) = MISSIONS_TITLE;
            texts.push(MenuText { font: 1, text: line(text(id)), pos: [mx, my], scale: 1.0, colour: 0x80ff_a888, align: Align::Left });
            let flags = missions::Flags::of(g);
            let table = usize::try_from(sel).ok().and_then(|l| m.missions.get(l)).map(Vec::as_slice).unwrap_or(&[]);
            let rows = missions::rows(&flags, sel, table);
            let row = m.mission_row.get(sel.max(0) as usize).copied().unwrap_or(0);
            // The rows' text is laid into the widget's box (`0x2eaf70`: x + 16 to the box's right edge 455, 16 a
            // line, the lines outside the box's top and bottom not drawn), the rows `scroll` up: a two-line row's
            // check box sits at y + 16 (drawn within 4 of the box), and the row takes `lines · 16`. The selected row
            // moves the scroll by 4 a frame to sit within 4 of the box's top and bottom.
            let top = my + 20.0;
            let mut y = top - m.mission_scroll as f32;
            for (k, (t, done)) in rows.iter().enumerate() {
                let selected = k == row % rows.len().max(1);
                let colour = if selected { TEXT_SELECTED } else { 0x80ff_a888 };
                let wrapped = wrap_words(&text(*t), MISSIONS_WRAP, &|s| width(2, s));
                let n = wrapped.len().max(1) as f32;
                if selected {
                    if y < top + 4.0 { *scroll -= 4; }
                    if MISSIONS_BOTTOM - 16.0 * n - 4.0 < y { *scroll += 4; }
                }
                let (cx, cy) = (mx + 9.0, y + if n == 2.0 { 16.0 } else { 10.0 });
                if top + 4.0 <= cy && cy <= MISSIONS_BOTTOM - 4.0 {
                    lines.push(([cx - 5.0, cy - 5.0, cx + 5.0, cy + 5.0], 0x80ff_a888));
                    lines.push(([cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0], CHECK_INNER));
                    if *done { out.push(Draw::Icon { id: missions::MARK_ICON, frame: 1, pos: [cx - 13.0, cy - 18.0], size: [30.0, 30.0], colour: 0x80ff_ffff }); }
                }
                for (i, l) in wrapped.into_iter().enumerate() {
                    let ly = y + 16.0 * i as f32;
                    if ly < top || MISSIONS_BOTTOM < ly + 16.0 { continue; }
                    texts.push(MenuText { font: 2, text: String::from_utf8_lossy(&l).into_owned(), pos: [mx + 16.0, ly], scale: 1.0, colour, align: Align::Left });
                }
                y += 16.0 * n;
            }
            let _ = level;
        }
    }
    out
}

/// `0x325270(planet)` (from 0x0f6e's reward `0x40ffa0`): the "new planet" banner: the planet's text (`0x2a1130`:
/// 0x11f0 + planet − 1, none for 0; the planet clamped to 0..=0x14) held by the prompt's owner 8 for 180 ticks
/// (`0x327ae0(8, text, 0xb4)`); planet 3's goes to the message box for 240 instead (`0x327448(text, 0xf0)`).
pub fn new_planet_banner(g: &mut GameState, planet: i32) {
    let planet = planet.clamp(0, 0x14);
    let text = if planet == 0 { 0 } else { 0x11f0 + planet - 1 };
    if planet == 3 {
        g.messages.push(crate::msgbox::Request::Text { text, ticks: Some(0xf0) });
    } else {
        g.prompt.force(8, text as u32, 0xb4);
    }
}

/// The pages' streamed pictures (the engine streams them from `HUD.WAD` and draws them under the widgets): the galaxy
/// backdrop on the map page (`0x354368` / `0x3546e0`: over the whole screen), and the selected planet's picture on
/// both pages (`0x355508`, rc_formats::disc::planet_picture: its box, the surface's inset and scroll in texels, the
/// kind).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagePictures {
    /// The backdrop's level and alpha ([`Backdrop::draw`]).
    pub galaxy: Option<(i32, u8)>,
    pub planet: Option<PlanetDraw>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetDraw {
    pub level: i32,
    pub kind: u8,
    /// x, y, w, h.
    pub rect: [f32; 4],
    pub inset: f32,
    /// The surface's scroll in texels (of 128): `(vsync + level · 0x2ab) mod 0x800` sixteenths.
    pub scroll: f32,
}

pub fn page_pictures(g: &GameState) -> PagePictures {
    use rc_formats::disc::planet_picture as pp;
    let m = &g.travel.menu;
    let sel = m.selected;
    // `0x355508` (from the planet widget's draw `0x435f10` only): none for the space levels, nor past the map's
    // levels (but 0x18).
    let planet = (m.page == Page::Planet && !matches!(sel, 5 | 10 | 0xf) && (sel < 0x15 || sel == 0x18) && 0 <= sel).then(|| {
        let kind = pp::KINDS.get(sel as usize).copied().unwrap_or(0);
        let (rect, inset) = (pp::BOX_SHIP, pp::INSET_SHIP[kind as usize & 3]);
        let scroll = ((m.frame as i32 + sel * pp::SCROLL_PER_LEVEL) & 0x7ff) as f32 / 16.0;
        PlanetDraw { level: sel, kind, rect: rect.map(|v| v as f32), inset: inset as f32, scroll }
    });
    PagePictures { galaxy: m.backdrop.draw(), planet }
}

/// `0x4325a0`'s part of the frame: the reticle eased toward the selected planet's dot (its centre: 5 and 3 pixels a
/// frame at most), the list's scroll keeping the selected row in its window (`0x3529e8`: 4 pixels a frame).
pub fn menu_frame(g: &mut GameState) {
    let rows = map_rows(g);
    let m = &mut g.travel.menu;
    if let Some(&(x, y, _, size)) = usize::try_from(m.selected).ok().and_then(|l| MAP_ICONS.get(l)) {
        let tx = x as f32 + size * 0.5;
        let ty = y as f32 + size * 0.5;
        m.reticle[0] += (tx - m.reticle[0]).clamp(-5.0, 5.0);
        m.reticle[1] += (ty - m.reticle[1]).clamp(-3.0, 3.0);
    }
    // The selected row's top within 4 of the window's top, its bottom above 188 (the row pitch 46).
    let pitch = |l: i32| if PLANETS.get(l.max(0) as usize).is_some_and(|p| p.1 != -1) { 46 } else { 26 };
    let top: i32 = rows.iter().take(m.row).map(|&l| pitch(l)).sum::<i32>() + 4 - m.scroll;
    let bottom = top + rows.get(m.row).map_or(0, |&l| pitch(l));
    if top < 4 { m.scroll -= (4 - top).min(4); }
    if 188 < bottom { m.scroll += (bottom - 188).min(4); }
}

// ---------------------------------------------------------------------------------------------------------------
// The fly-away

/// `0x364260` sub 1's state (`0x152c4c..`): the path's points with their roll, the first segment's length, t and
/// the speed, the approach from the ship's pose, the camera blend.
#[derive(Clone, Debug)]
pub struct FlyAway {
    pub tick: i32,
    pub points: Vec<[f32; 4]>,
    pub seg_len: f32,
    pub t: f32,
    pub speed: f32,
    pub from: (V3, V3),
    pub to: (V3, V3),
    pub cam: (f32, f32),
    /// Clank's rider (class 10), when the level has the class.
    pub clank: Option<usize>,
}

/// `0x362010(dest)` for another planet: the trip set up (`0x30d658`'s autosave is the engine's at the level's end).
pub fn fly_away_start(g: &mut GameState, w: &mut MobyWorld, level: &crate::level::LevelData, dest: i32) {
    g.travel.dest = dest;
    g.scenes.fade = 1.0;
    let Some(ship) = g.travel.ship.filter(|&s| w.mobys.get(s).is_some_and(|m| m.state < 0x80)) else {
        g.travel.fly = None;
        g.travel.phase = Phase::FlyAway;
        return;
    };
    // The path: its turns as w (the sharpest 13°), the first 20 ramped.
    let mut points: Vec<[f32; 4]> = usize::try_from(g.travel.path).ok().and_then(|p| level.paths.get(p)).cloned().unwrap_or_default();
    let n = points.len();
    if 2 <= n {
        let yaw = |a: [f32; 4], b: [f32; 4]| atan2(b[0] - a[0], b[1] - a[1]);
        let mut max = 0.0f32;
        for i in 1..n - 1 {
            let turn = sub_rot(yaw(points[i], points[i + 1]), yaw(points[i - 1], points[i]));
            points[i][3] = turn;
            max = max.max(turn.abs());
        }
        points[0][3] = points[1][3];
        points[n - 1][3] = sub_rot(yaw(points[0], points[1]), yaw(points[n - 2], points[n - 1]));
        if 0.0 < max { for p in points.iter_mut() { p[3] *= MAX_ROLL / max; } }
        for (i, p) in points.iter_mut().enumerate().take(20) { p[3] *= 0.05 * i as f32; }
    }
    let (mpos, mrot) = (w.mobys[ship].pos, w.mobys[ship].rot);
    let p = |k: usize| points.get(k).map_or(mpos, |q| [q[0], q[1], q[2]]);
    let seg_len = if 2 <= n { dist2(p(0), p(1)) } else { 1.0 };
    let (p0, p1) = (p(0), p(1));
    let pitch = atan2(((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt(), p0[2] - p1[2]);
    let to = (p0, [points.first().map_or(0.0, |q| q[3]), pitch, atan2(p1[0] - p0[0], p1[1] - p0[1])]);
    let clank = (g.clank && w.class(10).is_some()).then(|| {
        let c = w.create(10);
        let cm = &mut w.mobys[c];
        (cm.draw_dist, cm.update_dist, cm.drawn) = (0x1ff, 0xff, 1);
        cm.mode |= crate::moby::mode::NO_UPDATE | crate::moby::mode::NO_MATRIX;
        c
    });
    g.travel.fly = Some(FlyAway { tick: -1, points, seg_len, t: 0.0, speed: 0.0, from: (mpos, mrot), to, cam: (0.0, 0.0), clank });
    g.travel.phase = Phase::FlyAway;
    w.changed = true;
}

/// `0x343fb8(path, 1, …)`: the pose at `t` along the points: the position, (roll, pitch, yaw).
fn path_pose(points: &[[f32; 4]], t: f32) -> (V3, V3) {
    let n = points.len();
    let i = (t.max(0.0) as usize) % n;
    let f = t - t.floor();
    let p = |k: usize| { let q = points[k % n]; [q[0], q[1], q[2]] };
    let (a, b, c) = (p(i), p(i + 1), p(i + 2));
    let pos = add(a, scale(sub(b, a), f));
    let d0 = sub(b, a);
    let d1 = sub(c, b);
    let dir = add(d0, scale(sub(d1, d0), f));
    let flat = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    let roll = points[i][3] * (1.0 - f) + points[(i + 1) % n][3] * f;
    (pos, [roll, -atan2(flat, dir[2]), atan2(dir[0], dir[1])])
}

/// What a fly-away tick asks of the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlyOut {
    /// The script camera's targets (position, Euler), when the level has the two cuboids.
    pub camera: Option<(V3, V3)>,
    /// `0x1a8ed4 = 1`: the level ends.
    pub leave: bool,
}

/// `0x364260` sub 1's tick (module docs). `riders`: where Ratchet (and Clank) sit, written by the engine.
pub fn fly_away_tick(g: &mut GameState, w: &mut MobyWorld, level: &crate::level::LevelData, pad: &PadState, hero: &crate::hero::Hero, riders: &mut Vec<(Option<usize>, V3, V3)>) -> FlyOut {
    let mut out = FlyOut::default();
    let Some(ship) = g.travel.ship.filter(|&s| w.mobys.get(s).is_some_and(|m| m.state < 0x80)) else {
        out.leave = true;
        return out;
    };
    let Some(mut fly) = g.travel.fly.take() else {
        out.leave = true;
        return out;
    };
    fly.tick += 1;
    if fly.tick == 0 {
        // `0x428880(0x1b8d40, 0x1b8d50, 1, 0, 0)`: the script camera from the view as it is; then its targets.
        let (pos, euler) = g.cameras.view_euler();
        g.cameras.camera_script(pos, euler, crate::script_camera::MODE_SNAP, 0, false, hero);
    }
    // The camera between the cuboids.
    if let (Some((pa, ra)), Some((pb, rb))) = (level.cuboid_place(g.travel.cam_a), level.cuboid_place(g.travel.cam_b)) {
        let (mut f, mut vel) = fly.cam;
        approach(1.0, CAM_ACCEL, CAM_MAX, &mut f, &mut vel);
        fly.cam = (f, vel);
        let pos = add(pa, scale(sub(pb, pa), f));
        let pitch = add_rot(ra[1], sub_rot(rb[1], ra[1]) * f);
        let yaw = add_rot(ra[2], sub_rot(rb[2], ra[2]) * f);
        // `0x4287b8` / `0x428810`: the script camera's targets (mode 1 snaps to them).
        g.cameras.camera_script_targets(Some(pos), Some([0.0, pitch, yaw]));
        out.camera = Some((pos, [0.0, pitch, yaw]));
    }
    let n = fly.points.len();
    if n < 2 {
        // No path: the level ends at once behind black.
        g.scenes.fade = 1.0;
        out.leave = true;
        g.travel.fly = Some(fly);
        return out;
    }
    if fly.tick < 0x78 {
        let mut k = (1.0 - (fly.tick as f32 * (std::f32::consts::PI / 90.0)).cos()) * 0.5;
        if 0x59 < fly.tick { k = 1.0; }
        let (p0, r0) = fly.from;
        let (p1, r1) = fly.to;
        let m = &mut w.mobys[ship];
        m.pos = add(p0, scale(sub(p1, p0), k));
        m.rot = [r1[0] * k, add_rot(r0[1], sub_rot(r1[1], r0[1]) * k), add_rot(r0[2], sub_rot(r1[2], r0[2]) * k)];
        if 0.0 < g.scenes.fade { g.scenes.fade = (g.scenes.fade - 0.125).max(0.0); }
    } else {
        fly.speed = (fly.speed + 0.8).min(100.0);
        if pad.pressed & (button::CROSS | button::TRIANGLE) != 0 && g.scenes.fade < 0.1 { g.scenes.fade = 0.1; }
        let step = fly.speed * (1.0 / 60.0) / fly.seg_len;
        if n as f32 - step * 12.0 < fly.t && g.scenes.fade < 0.1 { g.scenes.fade = 0.1; }
        if 0.0 < g.scenes.fade { g.scenes.fade = (g.scenes.fade + 0.1).min(1.0); }
        fly.t += step;
        if (n - 1) as f32 <= fly.t || 1.0 <= g.scenes.fade {
            out.leave = true;
        } else {
            let (pos, rot) = path_pose(&fly.points, fly.t);
            let m = &mut w.mobys[ship];
            (m.pos, m.rot) = (pos, rot);
        }
    }
    // The riders on their seats, facing as the ship does.
    let rows = euler_rows(w.mobys[ship].rot);
    w.mobys[ship].rows = rows;
    let (pos, srot) = (w.mobys[ship].pos, w.mobys[ship].rot);
    g.travel.shadow = Some(ShipShadow::new(pos, rows, g.travel.shadow_ground, 0x80 - 4 * (fly.tick - 0x96).max(0)));
    let seat = |s: V3| add(add(add(scale(rows[0], s[0]), scale(rows[1], s[1])), scale(rows[2], s[2])), pos);
    riders.push((None, seat(SEAT_RATCHET), srot));
    if let Some(c) = fly.clank {
        if let Some(cm) = w.mobys.get_mut(c) {
            (cm.pos, cm.rot, cm.rows) = (seat(SEAT_CLANK), srot, rows);
            riders.push((Some(c), seat(SEAT_CLANK), srot));
        }
    }
    w.changed = true;
    g.travel.fly = Some(fly);
    out
}

// ---------------------------------------------------------------------------------------------------------------
// The flight between the planets

/// `0x28a950`: the sky shells' drift direction per variant (the translation `0x1b23b0` = it · (tick − 120) · 20 · f).
pub const FLIGHT_DIRS: [V3; FLIGHT_VARIANTS] = [[0.6, 0.15, 0.0], [1.0, -0.7, 0.15], [0.9, -0.4, 0.0], [-0.3, -0.9, 0.0], [0.4, 0.6, -0.2]];
/// `0x1ac510` (`gp − 0x2ae0`): the sky's turn about z per variant (`0x35e038`'s shell angles add to it).
pub const FLIGHT_TURNS: [f32; FLIGHT_VARIANTS] = [-0.12, 0.0, 0.85, -1.8, 0.0];
/// `0x1ac528` (`gp − 0x2ac8`): the least tangent of the view per variant.
pub const FLIGHT_MIN_TAN: [f32; FLIGHT_VARIANTS] = [0.3, 0.3, 0.3, 0.3, 0.63];
/// `0x35e038`: the shells' own angles (x, y, z added to the turn) by shell index.
pub const FLIGHT_SHELL_ANGLES: [V3; 4] = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, -0.075, -0.15], [-0.06, 0.05, 0.125]];
/// `0x28a8e0`: the planet picture's scale per level (1 past 0x12).
pub const PICTURE_SCALE: [f32; 19] = [0.8, 1.0, 0.9, 1.2, 0.9, 1.1, 1.2, 0.8, 1.2, 1.3, 1.1, 0.7, 1.2, 1.2, 1.1, 1.0, 1.2, 1.1, 0.8];
/// `0x1ac480` (`gp − 0x2b70`): where the planet picture starts; `0x1ac4e0` (`gp − 0x2b00`): its drift a tick (× f).
pub const PICTURE_START: V3 = [-340.0, -280.0, 280.0];
pub const PICTURE_DRIFT: V3 = [0.4, 0.8, 0.0];
/// `0x1ac4a0` (`gp − 0x2b50`): the picture quad's corners (strip order), the XZ plane.
pub const PICTURE_CORNERS: [V3; 4] = [[-80.0, 0.0, 80.0], [80.0, 0.0, 80.0], [-80.0, 0.0, -80.0], [80.0, 0.0, -80.0]];
/// `0x3632c0`: the caption at (32, 416 − 88), 256 × 32, from tick 60 at 2 alpha a tick to 0x80.
pub const CAPTION_AT: [f32; 2] = [32.0, 328.0];
pub const CAPTION_SIZE: [f32; 2] = [256.0, 32.0];
/// The flight scenes play far above the level (a port frame: nothing of the level is in view).
pub const FLIGHT_FRAME: ShipFrame = ShipFrame { at: [0.0, 0.0, 6000.0], yaw: 0.0 };
pub const FLIGHT_VARIANTS: usize = 5;
pub const ARRIVAL_VARIANT: usize = 4;

/// `0x367400`: the flight lump's kind (`0x152c2e`) for a trip from `from` to `dest`: 1 to or from the orbit levels
/// 0x16 / 0x17, 2 to or from the Jamming Array 0x1a, 3 to Todano (0xe) before its flag `0x1395e3` (the port keeps
/// no such flag: 3), else 0.
pub fn flight_kind(from: i32, dest: i32) -> usize {
    if dest == 0x16 || from == 0x16 || dest == 0x17 || from == 0x17 { return 1; }
    if dest == 0x1a || from == 0x1a { return 2; }
    if dest == 0xe { return 3; }
    0
}

/// `0x367400`'s story pick for a level change to `dest` (`gp−0x2d80`; `pending` its value so far): the first of
/// Aranos (0), Oozla (1), Notak (5) and Smolg (8) visited (`0x1a7bf0`) and not the destination whose chain is not seen
/// (0xa9, 0x17, 0xab, 0xad: the seen bit `0x139768 + (id & !3)`, `1 << (id & 31)`, crate::game::GameState::scenes_seen);
/// else with the planets 10 or 11 unlocked (`0x1a7bd0`) and 0x1a not seen, 0x1a; else to Joba (0xd) with 0x1b not
/// seen, 0x1b; else with level 0x10 visited, not the destination, and 0xaf not seen, 0xaf; else `pending`.
pub fn story_pick(g: &GameState, dest: i32, pending: i32) -> i32 {
    let visited = |l: usize| g.visited.get(l).is_some_and(|&b| b != 0);
    let unlocked = |l: usize| g.planets.get(l).is_some_and(|&b| b != 0);
    let seen = |id: usize| g.scenes_seen.get(id).copied().unwrap_or(false);
    for (level, id) in [(0, 0xa9), (1, 0x17), (5, 0xab), (8, 0xad)] {
        if visited(level) && dest != level as i32 && !seen(id) { return id as i32; }
    }
    if !(!unlocked(10) && !unlocked(11) || seen(0x1a)) { return 0x1a; }
    if dest == 0xd && !seen(0x1b) { return 0x1b; }
    if visited(0x10) && dest != 0x10 && !seen(0xaf) { return 0xaf; }
    pending
}

/// `0x3671b8`: a story chain: the ids it marks seen first, then what it plays in order (scenes `0x366c68`, movies
/// `0x366de8`; a skip ends it): after Aranos the scene 0xa9, the movie 1 and the scene 0xaa; after Notak 0xab, 0x18,
/// 0xac; after Smolg 0xad, 0x19, 0xae; after level 0x10 0xaf, 0x1c, 0xb0; 0x17, 0x1a and 0x1b are a movie alone.
pub fn story_chain(id: i32) -> (&'static [i32], &'static [i32]) {
    match id {
        0xa9 => (&[1, 0xa9, 0xaa], &[0xa9, 1, 0xaa]),
        0xab => (&[0x18, 0xab, 0xac], &[0xab, 0x18, 0xac]),
        0xad => (&[0x19, 0xad, 0xae], &[0xad, 0x19, 0xae]),
        0xaf => (&[0x1c, 0xb0, 0xaf], &[0xaf, 0x1c, 0xb0]),
        0x17 => (&[], &[0x17]),
        0x1a => (&[], &[0x1a]),
        0x1b => (&[], &[0x1b]),
        _ => (&[], &[]),
    }
}

/// `0x367400`'s mark of the level left (`0x1a79f8`): below 0x13, and not Aranos before the movie 1 is seen, its
/// visited byte 2.
pub fn level_left(g: &mut GameState) {
    let l = g.level;
    if (0..0x13).contains(&l) && (l != 0 || g.scenes_seen.get(1).copied().unwrap_or(false)) {
        if let Some(v) = g.visited.get_mut(l as usize) { *v = 2; }
    }
}

/// The flight (`0x365960` / `0x363360`: `0x152c64` the variant, `0x152c68` the count, the sky's drift and turn, the
/// planet picture's place).
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub kind: usize,
    pub dest: i32,
    pub variant: usize,
    pub count: i32,
    pub picture_pos: V3,
    pub sky_translation: V3,
    pub sky_turn: f32,
    pub min_tan: f32,
    pub done: bool,
}

impl Flight {
    /// `0x365960`: the first variant at random (`rand >> 16 & 3`), the count 0, the picture at its start.
    pub fn start(g: &mut GameState, dest: i32) -> Flight {
        let variant = ((g.rng.rand() >> 16) & 3) as usize;
        let mut f = Flight { kind: flight_kind(g.level, dest), dest, variant, count: 0, picture_pos: PICTURE_START, sky_translation: [0.0; 3], sky_turn: 0.0, min_tan: 0.0, done: false };
        f.begin_variant();
        f
    }

    fn begin_variant(&mut self) {
        self.picture_pos = PICTURE_START;
        self.sky_turn = FLIGHT_TURNS[self.variant];
        self.min_tan = FLIGHT_MIN_TAN[self.variant];
        self.sky_translation = [0.0; 3];
    }

    /// The end of a variant (`0x363360`): with the level not loaded far enough the count starts over; past two
    /// variants it is over; else the next: another at random (`v + rand >> 16 % 3 + 1 & 3`) on the count 0, the
    /// arrival (4) after. Returns the next variant to play, or none when the flight is over.
    pub fn next_variant(&mut self, g: &mut GameState, loaded: bool) -> Option<usize> {
        if !loaded { self.count = 0; }
        if 1 < self.count {
            self.done = true;
            return None;
        }
        self.variant = if self.count == 0 { (self.variant + ((g.rng.rand() >> 16) % 3) as usize + 1) & 3 } else { ARRIVAL_VARIANT };
        self.count += 1;
        self.begin_variant();
        Some(self.variant)
    }

    /// `0x363360`'s per-tick part for scene tick `tick` of `end`: the sky's drift (variant 4 shrinking with f = the
    /// ticks left over the end), the picture's drift (variant 4); returns the actors' scale factor (variant 4's last 40
    /// ticks: `(end − tick − 4) / 36`, else 1) and the ship's glow (`+0x95`: 0xe8 · it).
    pub fn tick(&mut self, tick: i32, end: i32) -> f32 {
        let arrival = self.variant == ARRIVAL_VARIANT;
        let f = if arrival && 0 < end { (end - tick) as f32 / end as f32 } else { 1.0 };
        if arrival { self.picture_pos = add(self.picture_pos, scale(PICTURE_DRIFT, f)); }
        self.sky_translation = scale(FLIGHT_DIRS[self.variant], (tick - 0x78) as f32 * f * 20.0);
        if arrival && end - 40 < tick { ((end - tick - 4) as f32 * (1.0 / 36.0)).max(0.0) } else { 1.0 }
    }

    /// The sky shells' Euler angles (`0x35e038`) by shell index.
    pub fn shell_angles(&self, shell: usize) -> V3 {
        let a = FLIGHT_SHELL_ANGLES.get(shell).copied().unwrap_or([0.0; 3]);
        [a[0], a[1], add_rot(self.sky_turn, a[2])]
    }

    /// The planet picture's corners in the world (variant 4; `0x363188`: the corners × the level's scale + the
    /// place, in the flight's frame).
    pub fn picture_corners(&self) -> Option<[V3; 4]> {
        if self.variant != ARRIVAL_VARIANT { return None; }
        let s = PICTURE_SCALE.get(self.dest.max(0) as usize).copied().unwrap_or(1.0);
        Some(PICTURE_CORNERS.map(|c| FLIGHT_FRAME.to_world(add(scale(c, s), self.picture_pos))))
    }

    /// The caption's alpha at scene tick `tick` (variant 4 past tick 60: 2 a tick to 0x80).
    pub fn caption_alpha(&self, tick: i32) -> u8 {
        if self.variant != ARRIVAL_VARIANT || tick <= 0x3c { return 0; }
        ((tick - 0x3c) * 2).min(0x80) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flight_plays_two_variants_then_the_arrival() {
        let mut g = GameState::default();
        let mut f = Flight::start(&mut g, 3);
        assert!(f.variant < 4);
        let v1 = f.next_variant(&mut g, true).unwrap();
        assert!(v1 < 4 && f.count == 1);
        assert_eq!(f.next_variant(&mut g, true), Some(ARRIVAL_VARIANT));
        assert_eq!(f.next_variant(&mut g, true), None);
        assert!(f.done);
        let mut f = Flight::start(&mut g, 3);
        assert!(f.next_variant(&mut g, false).unwrap() < 4);
        assert_eq!(f.count, 1);
    }

    #[test]
    fn approach_lands_on_its_target() {
        let (mut v, mut vel) = (0.0f32, 0.0f32);
        for _ in 0..400 { approach(1.0, CAM_ACCEL, CAM_MAX, &mut v, &mut vel); }
        assert!((v - 1.0).abs() < 1e-3, "{v}");
        let (mut z, mut zv) = (10.0f32, 0.0f32);
        for _ in 0..600 { approach(10.4, Z_ACCEL, Z_MAX, &mut z, &mut zv); }
        assert!((z - 10.4).abs() < 1e-3, "{z}");
    }

    #[test]
    fn lumps_by_clank() {
        assert_eq!(Travel::lump(false, false), 0);
        assert_eq!(Travel::lump(true, false), 1);
        assert_eq!(Travel::lump(false, true), 2);
        assert_eq!(Travel::lump(true, true), 3);
    }

    #[test]
    fn path_pose_runs_along_the_points() {
        let pts = vec![[0.0, 0.0, 0.0, 0.0], [10.0, 0.0, 0.0, 0.1], [10.0, 10.0, 0.0, 0.2], [0.0, 10.0, 0.0, 0.0]];
        let (p, r) = path_pose(&pts, 0.5);
        assert_eq!(p, [5.0, 0.0, 0.0]);
        assert!((r[0] - 0.05).abs() < 1e-6);
    }
}
