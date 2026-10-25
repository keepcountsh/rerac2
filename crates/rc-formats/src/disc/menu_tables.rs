//! The pause pages' data in a level's overlay: the item records (`0x262500` on Aranos, `0xe0` a row, an item's row
//! `0x139568[item]`) and the pages' grids ([`Grid`]: cells of 10 bytes, [`GridCell`]). Each level's overlay has them
//! at its own addresses: found from Aranos's through [`crate::level_overlay::Relocation`] (the
//! code that forms the address, matched in the level's copy).

use crate::level_overlay::{LevelOverlay, Relocation};

/// Aranos's (level 0, the reference) address the code forms for the item records (their base − 0x10).
const ITEM_ANCHOR: u32 = 0x2624f0;
const ITEM_BASE_FROM_ANCHOR: u32 = 0x10;
/// The records read (the 0x38 items, then the upgrades' rows).
pub const ITEM_ROWS: usize = 0x60;
pub const ITEM_STRIDE: u32 = 0xe0;

/// A pause page's grid (the grid drawer `0x42e0c8`'s table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grid {
    /// The Weapons page's 4 × 6.
    Weapons,
    /// The Gadgets page's hand items (3 × 2), packs (2) and boots (3).
    Hand,
    Packs,
    Boots,
    /// The Items page's four: 2 × 1, 1 × 1, 4 × 2 and the 2 × 2 counters.
    ItemsTop,
    ItemsSingle,
    ItemsMain,
    ItemsCounters,
}

impl Grid {
    pub const ALL: [Grid; 8] = [Grid::Weapons, Grid::Hand, Grid::Packs, Grid::Boots, Grid::ItemsTop, Grid::ItemsSingle, Grid::ItemsMain, Grid::ItemsCounters];

    /// Aranos's address of the table and its cells.
    fn table(self) -> (u32, usize) {
        match self {
            Grid::Weapons => (0x282978, 24),
            Grid::Hand => (0x282700, 6),
            Grid::Packs => (0x1aab38, 2),
            Grid::Boots => (0x1aabf8, 3),
            Grid::ItemsTop => (0x1aab70, 2),
            Grid::ItemsSingle => (0x1aab90, 1),
            Grid::ItemsMain => (0x286d48, 8),
            Grid::ItemsCounters => (0x1aabd0, 4),
        }
    }
}

/// A grid cell: an icon and its frame (drawn as is when `kind` is 2), the item (0: none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridCell {
    pub icon: u16,
    pub frame: u16,
    pub kind: u16,
    pub item: i16,
}

/// What the pause pages read of an item record (offsets from the row's base `0x262500 + 0xe0·row`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemRecord {
    /// `−0x10` ≠ 0: it has a mega upgrade (its upgraded rows show it in their own colour).
    pub mega: bool,
    /// `−0x0c`: the record's armour / colour class (the upgrade bar's colour table index).
    pub class: u8,
    /// `−0x08`: its name (text id).
    pub name: u32,
    /// `−0x0a`: its picture on the pause pages (the page's [`crate::disc::toc::PictureTable`] row; < 0: none).
    pub picture: i16,
    /// `+0x2c`: its icon (GUI image id; 0xea7e: none).
    pub icon: u16,
    /// `+0x32`: its description on the Items page (text id; 0: none).
    pub description: i16,
    /// `+0x3a`: the next upgrade's row (0: none).
    pub next: i16,
    /// `+0x5c`: the experience the upgrade takes (< 1: none).
    pub xp_max: i32,
    /// `+0x78` ≠ 0: it uses ammo; `+0x7e`: the most.
    pub uses_ammo: bool,
    pub max_ammo: u16,
    /// `+0x7c`: the ammo one pickup gives (the crates' `0x3d2d30`).
    pub pickup_ammo: u16,
    /// `+0x2e`: the class of its ammo pickup.
    pub pickup_class: i16,
    /// `+0x2a` / `+0x28`: the pickup's message for one and for more (text ids).
    pub pickup_text: [i16; 2],
    /// `+0x00`: Ratchet's attachment the item's moby sits at; `+0x04` / `+0x08`: its mobys' classes (the worn feet
    /// items have two, one a foot).
    pub attach: i32,
    pub classes: [i32; 2],
    /// The vendor's fields (`0x3724c8`, `0x3747e8`): `+0x70` its price, `+0x78` its ammo's unit price (0: no ammo:
    /// the same word as `uses_ammo`), `+0x82` the ammo it comes with, `+0x36` its description on the vendor's ticker
    /// (text id), `+0x3c` the row it upgrades (0: a base row).
    pub price: i32,
    pub ammo_price: u16,
    pub start_ammo: u16,
    pub vendor_text: i16,
    pub base: i16,
    /// `+0x88` / `+0x8c` / `+0x90`: its weapon mods' prices in platinum bolts by slot (0: no such mod; the mod
    /// vendor `0x373268`).
    pub mods: [i32; 3],
}

/// A level's pause tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuTables {
    /// By row.
    pub items: Vec<ItemRecord>,
    /// The grids' cells (row-major), by [`Grid::ALL`]'s order.
    pub grids: Vec<Vec<GridCell>>,
    /// The grids' items as the Help pages walk them (`0x439e98`: from the first cell on, while the item is not
    /// negative, at most 25 cells: past the grid's own cells), by [`Grid::ALL`]'s order.
    pub walks: Vec<Vec<i16>>,
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
                    mega: i32_at(b - 0x10).unwrap_or(0) != 0,
                    class: t.read(b - 0x0c, 1).map_or(0, |x| x[0]),
                    name: i32_at(b - 0x08).unwrap_or(0) as u32,
                    picture: i16_at(b - 0x0a).unwrap_or(-1),
                    icon: icon as u16,
                    description: i16_at(b + 0x32).unwrap_or(0),
                    next: i16_at(b + 0x3a).unwrap_or(0),
                    xp_max: i32_at(b + 0x5c).unwrap_or(0),
                    uses_ammo: i16_at(b + 0x78).unwrap_or(0) != 0,
                    max_ammo: i16_at(b + 0x7e).unwrap_or(0) as u16,
                    pickup_ammo: i16_at(b + 0x7c).unwrap_or(0) as u16,
                    pickup_class: i16_at(b + 0x2e).unwrap_or(0),
                    pickup_text: [i16_at(b + 0x2a).unwrap_or(0), i16_at(b + 0x28).unwrap_or(0)],
                    attach: i32_at(b).unwrap_or(0),
                    classes: [i32_at(b + 4).unwrap_or(0), i32_at(b + 8).unwrap_or(0)],
                    price: i32_at(b + 0x70).unwrap_or(0),
                    ammo_price: i16_at(b + 0x78).unwrap_or(0) as u16,
                    start_ammo: i16_at(b + 0x82).unwrap_or(0) as u16,
                    vendor_text: i16_at(b + 0x36).unwrap_or(0),
                    base: i16_at(b + 0x3c).unwrap_or(0),
                    mods: [i32_at(b + 0x88).unwrap_or(0), i32_at(b + 0x8c).unwrap_or(0), i32_at(b + 0x90).unwrap_or(0)],
                });
            }
        }
        out.grids = Grid::ALL
            .iter()
            .map(|g| {
                let (at, n) = g.table();
                let Some(at) = rel.data(at) else { return Vec::new() };
                let u = |a: u32| i16_at(a).unwrap_or(0) as u16;
                (0..n as u32).map(|k| at + 10 * k).map(|c| GridCell { icon: u(c), frame: u(c + 2), kind: u(c + 4), item: u(c + 6) as i16 }).collect()
            })
            .collect();
        out.walks = Grid::ALL
            .iter()
            .map(|g| {
                let Some(at) = rel.data(g.table().0) else { return Vec::new() };
                (0..25u32).map(|k| i16_at(at + 10 * k + 6).unwrap_or(-1)).take_while(|&i| 0 <= i).collect()
            })
            .collect();
        Ok(out)
    }

    /// Grid `g`'s items as the Help pages walk them.
    pub fn walk(&self, g: Grid) -> &[i16] {
        Grid::ALL.iter().position(|&x| x == g).and_then(|i| self.walks.get(i)).map_or(&[], Vec::as_slice)
    }

    /// Grid `g`'s cells (none when not found).
    pub fn grid(&self, g: Grid) -> &[GridCell] {
        Grid::ALL.iter().position(|&x| x == g).and_then(|i| self.grids.get(i)).map_or(&[], Vec::as_slice)
    }
}

/// No tables (the front end's pages).
pub static EMPTY: MenuTables = MenuTables { items: Vec::new(), grids: Vec::new(), walks: Vec::new() };
