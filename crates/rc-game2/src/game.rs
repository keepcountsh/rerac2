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
    /// Bolts collected since the counter last showed them (the HUD's bolt counter animates the sum in).
    pub bolts_pending: u32,
}

impl Default for GameState {
    fn default() -> Self { GameState { bolts: 0, raritanium: 0, max_health: 4, level: 0, bolts_pending: 0 } }
}
