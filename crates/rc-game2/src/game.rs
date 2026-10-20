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
    /// `0x30d910(n)`: the HUD's bolt counter shows for n more ticks.
    pub bolt_counter_show: i32,
    /// The random stream (`0x1163b0`).
    pub rng: crate::rng::Rng,
    /// Recent crate breaks (`$gp+0x3fc`, decays): fewer bolt mobys a drop the more.
    pub recent_breaks: i32,
    /// `$gp+0x3f8` / the frame counter `0x1b1948` of the last break.
    pub last_break_frame: i32,
    pub frame: i32,
}

impl Default for GameState {
    fn default() -> Self {
        GameState {
            bolts: 0, raritanium: 0, max_health: 4, level: 0, bolt_bank: 0, xp_bank: 0, xp: 0, uid_bits: vec![0; 0x400],
            bolt_counter_show: 0, rng: Default::default(), recent_breaks: 0, last_break_frame: -1000, frame: 0,
        }
    }
}

impl GameState {
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
