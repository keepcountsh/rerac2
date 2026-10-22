//! The pause pages' data in a level's overlay: the item records (`0x262500` on Aranos, `0xe0` a row, an item's row
//! `0x139568[item]`) and the Weapons page's grid (`0x282978`: 24 cells of 10 bytes, the item at `+6`). Each level's
//! overlay has them at its own addresses: found from Aranos's through [`crate::level_overlay::Relocation`] (the
//! code that forms the address, matched in the level's copy).

use crate::level_overlay::{LevelOverlay, Relocation};

/// Aranos's (level 0, the reference) addresses the code forms: the item records' anchor (the records' base
/// − 0x10) and the Weapons grid.
const ITEM_ANCHOR: u32 = 0x2624f0;
const ITEM_BASE_FROM_ANCHOR: u32 = 0x10;
const WEAPON_GRID: u32 = 0x282978;
/// The records read (the 0x38 items, then the upgrades' rows).
pub const ITEM_ROWS: usize = 0x60;
pub const ITEM_STRIDE: u32 = 0xe0;
pub const WEAPON_CELLS: usize = 24;

/// What the pause pages read of an item record (offsets from the row's base `0x262500 + 0xe0·row`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemRecord {
    /// `−0x0c`: the record's armour / colour class (the upgrade bar's colour table index).
    pub class: u8,
    /// `−0x08`: its name (text id).
    pub name: u32,
    /// `+0x2c`: its icon (GUI image id; 0xea7e: none).
    pub icon: u16,
    /// `+0x3a`: the next upgrade's row (0: none).
    pub next: i16,
    /// `+0x5c`: the experience the upgrade takes (< 1: none).
    pub xp_max: i32,
    /// `+0x78` ≠ 0: it uses ammo; `+0x7e`: the most.
    pub uses_ammo: bool,
    pub max_ammo: u16,
}

/// A level's pause tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuTables {
    /// By row.
    pub items: Vec<ItemRecord>,
    /// The Weapons page's cells (row-major, 4 a row): the item, 0 empty.
    pub weapon_grid: Vec<i16>,
}

impl MenuTables {
    /// From the level's overlay and Aranos's (the same bytes on Aranos itself). Tables not found stay empty.
    pub fn read(level: &[u8], aranos: &[u8]) -> crate::buf::Result<MenuTables> {
        let (t, r) = (LevelOverlay::parse(level)?, LevelOverlay::parse(aranos)?);
        let rel = Relocation::new(&r, &t);
        let i16_at = |a: u32| t.read(a, 2).map(|b| i16::from_le_bytes([b[0], b[1]]));
        let i32_at = |a: u32| t.read(a, 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
        let mut out = MenuTables::default();
        if let Some(base) = rel.data(ITEM_ANCHOR).map(|a| a + ITEM_BASE_FROM_ANCHOR) {
            for row in 0..ITEM_ROWS as u32 {
                let b = base + row * ITEM_STRIDE;
                let Some(icon) = i16_at(b + 0x2c) else { break };
                out.items.push(ItemRecord {
                    class: t.read(b - 0x0c, 1).map_or(0, |x| x[0]),
                    name: i32_at(b - 0x08).unwrap_or(0) as u32,
                    icon: icon as u16,
                    next: i16_at(b + 0x3a).unwrap_or(0),
                    xp_max: i32_at(b + 0x5c).unwrap_or(0),
                    uses_ammo: i16_at(b + 0x78).unwrap_or(0) != 0,
                    max_ammo: i16_at(b + 0x7e).unwrap_or(0) as u16,
                });
            }
        }
        if let Some(g) = rel.data(WEAPON_GRID) {
            out.weapon_grid = (0..WEAPON_CELLS as u32).map(|k| i16_at(g + 10 * k + 6).unwrap_or(0)).collect();
        }
        Ok(out)
    }
}
