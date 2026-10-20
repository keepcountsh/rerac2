//! The game-wide state the level code reads and writes (L00 / the boot's .bss; names from the addresses' readers):
//! currency, health limits and the per-level tallies. What a save holds is milestone I (docs/plan/rac2_bringup.md).

/// The player's progress fields the gameplay code touches.
#[derive(Clone, Debug)]
pub struct GameState {
    /// `0x1a7a00`: bolts (u32; `CollectBolts`' add path).
    pub bolts: u32,
    /// `0x1a7a04`: raritanium.
    pub raritanium: u32,
    /// `0x1a7a08`: the health limit (the heal clamp and the HUD's bar).
    pub max_health: i32,
    /// `0x1a79f8`: the level (planet) index.
    pub level: i32,
    /// `0x1a7a1c`: the bolt bank: value a drop could not put into bolt mobys (the limit per drop, bolts that fell
    /// far from the camera); each later drop takes (bank + 49) / 50 of it.
    pub bolt_bank: i32,
    /// `0x1a7a18` / `0x1a7a14`: the XP bank and the XP total.
    pub xp_bank: i32,
    pub xp: i32,
    /// The per-level save bytes of the uids (`0x19b4a8 + level·0x400`, 3 bits per uid: low nibble even uids, high
    /// odd): how often a crate was broken (the bolt percentage table's row) and bit 3 (the second table).
    pub uid_bits: Vec<u8>,
    /// This tick's HUD requests: `0x30d910(n)` (the bolt counter shows for n ticks) and `0x30d9f8(n)` (the health).
    pub show_bolts: Option<i32>,
    pub show_health: Option<i32>,
    /// The random stream (`0x1163b0`).
    pub rng: crate::rng::Rng,
    /// Recent crate breaks (`$gp+0x3fc`, decays): fewer bolt mobys a drop the more.
    pub recent_breaks: i32,
    /// `$gp+0x3f8` / the frame counter `0x1b1948` of the last break.
    pub last_break_frame: i32,
    pub frame: i32,
    /// `0x1a7b02`: Clank rides on Ratchet's back (not yet on Aranos's opening); `0x1a7a11`: the Thruster-Pack (else the
    /// Heli-Pack); `0x1a7b13`: the Gravity Boots.
    pub clank: bool,
    pub thruster: bool,
    pub boots: bool,
    /// `0x1a7b36` with the item `0x1a739c` = 0x36: the Charge Boots on.
    pub charge_boots: bool,
    /// `0x1a7b14`: the Grind Boots.
    pub grind_boots: bool,
    /// `0x1a7a30..0x1a7a3a`: the platinum bolts' nanotech upgrades (+1 health each); `0x18c350` / `0x18c34c`: the
    /// XP of the health level reached and of the next.
    pub nanotech: [bool; 10],
    pub level_xp: [i32; 2],
}

/// `0x2621e0`: the health levels, the XP / 32 to the next (each gives one health), −1 ends.
const HP_LEVELS: [i16; 66] = [
    0, 180, 210, 240, 270, 300, 330, 360, 390, 420, 450, 480, 500, 520, 520, 520, 520, 520, 520, 520, 520, 520, 530, 530, 560,
    580, 600, 620, 640, 660, 680, 700, 720, 740, 760, 780, 800, 840, 810, 945, 1080, 1215, 1350, 1485, 1620, 1755, 1890, 2025,
    2160, 2250, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2385, 2475, 2520, 2610, 2700, 2790, 2880,
];

impl Default for GameState {
    fn default() -> Self {
        GameState {
            bolts: 0, raritanium: 0, max_health: 4, level: 0, bolt_bank: 0, xp_bank: 0, xp: 0, uid_bits: vec![0; 0x400],
            show_bolts: None, show_health: None, rng: Default::default(), recent_breaks: 0, last_break_frame: -1000, frame: 0,
            clank: false, thruster: false, boots: false, charge_boots: false, grind_boots: false, nanotech: [false; 10], level_xp: [0; 2],
        }
    }
}

impl GameState {
    /// `0x2d61d0`: the health the XP gives (4 + one a level of `HP_LEVELS`), and the XP of the level reached and of
    /// the next.
    pub fn xp_health(&self) -> (i32, [i32; 2]) {
        let x = self.xp >> 5;
        let (mut hp, mut cum, mut last) = (4, 0i32, 0i32);
        let mut i = 0;
        while cum <= x {
            if 0 < i { hp += 1; }
            last = cum;
            i += 1;
            if 0x4c < i || HP_LEVELS.len() <= i { break; }
            cum += HP_LEVELS[i] as i32;
        }
        (hp, [last, cum])
    }

    /// `0x2d6258`'s health part (every 16th frame and the first): the health limit = the XP's + the nanotech
    /// upgrades; a change fills the health (a rise past frame 10 shows the bar: `show_health`).
    pub fn health_tick(&mut self, hp: &mut i32) {
        if self.frame & 0xf == 1 && self.frame != 1 { return; }
        let (h, lv) = self.xp_health();
        self.level_xp = lv;
        let max = h + self.nanotech.iter().filter(|&&n| n).count() as i32;
        if max != self.max_health {
            if 10 < self.frame && self.max_health < max { self.show_health = Some(0x168); }
            self.max_health = max;
            *hp = max;
        }
    }

    /// `0x2adf18(3)`: the back slot's item (`0x2afc68`: with Clank the Heli-Pack 2, or 3 with the Thruster-Pack).
    pub fn pack(&self) -> i32 { if self.clank { if self.thruster { 3 } else { 2 } } else { -1 } }

    /// The 3-bit count of uid `uid` (`0x2e4670`'s read).
    pub fn uid_count(&self, uid: i16) -> u8 {
        let b = self.uid_bits.get((uid as i32 >> 1).max(0) as usize).copied().unwrap_or(0);
        if uid & 1 != 0 { (b >> 4) & 7 } else { b & 7 }
    }

    /// Bit 3 of uid `uid`'s nibble.
    pub fn uid_flag(&self, uid: i16) -> bool {
        let b = self.uid_bits.get((uid as i32 >> 1).max(0) as usize).copied().unwrap_or(0);
        (if uid & 1 != 0 { b >> 4 } else { b }) & 8 != 0
    }

    /// The count + 1 (saturating at 7).
    pub fn bump_uid(&mut self, uid: i16) {
        let Some(b) = self.uid_bits.get_mut((uid as i32 >> 1).max(0) as usize) else { return };
        if uid & 1 != 0 { if (*b >> 4) & 7 != 7 { *b += 0x10; } } else if *b & 7 != 7 { *b += 1; }
    }
}
