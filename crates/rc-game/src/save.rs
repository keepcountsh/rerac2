//! The persistent game state as the card holds it: every chunk of the two descriptor tables, kept as bytes
//! ([`SaveData`]), with the game's restore (`RestoreData` `0x29bf58`) and prepare (`PrepData` `0x29bd88`), and the
//! sync of the [`GameState`] fields the port types (by their EE addresses: [`GameState::store`],
//! [`GameState::load`]). A chunk the port does not type keeps the card's bytes, so a save made from a loaded one
//! carries them unchanged. The on-card format is `rc_formats::disc::save_game`; the card is crate::memcard.
//!
//! **Global chunks** (boot `0x2560b8`, 34): the level `0x1a79f8`; `0x1a7a00` (0x40: bolts, raritanium, the health
//! limit, the wrench-out and Thruster-Pack bytes `0x1a7a10` / `0x1a7a11`, the XP `0x1a7a14`, its bank `0x1a7a18`, the
//! bolt bank `0x1a7a1c`, the nanotech bytes `0x1a7a30..`); the items had `0x1a7b00` (0x38, one byte per item: Clank
//! 2, the Gravity Boots 0x13, the Grind Boots 0x14, the Charge Boots 0x36) and their "ever had" copy `0x1a7b38`; the
//! ammo `0x139688` (0xe0); the quick-select list `0x139948` (0x70); the hand item `0x1a7398`; the quick select's slots `0x1a73b8` (8 items); the scenes seen
//! `0x139768` (0x100: scene `n` is bit `n & 31` of the word at byte `n & !3`, as the scene code indexes it); the save
//! clock `0x1a7360` (8 bytes, BCD); and others not typed yet.
//! **Level chunks** (boot `0x2562e8`, 10 per level, at `addr + level·size`): visited `0x1a7bf0` (1), the map mask
//! `0x18d278` (0x800), the missions `0x19b2e8` (0x10), the uid bits `0x19b4a8` (0x400), and six not typed yet.

use crate::game::GameState;
use rc_formats::disc::save_game::{ChunkDesc, ChunkTables, SaveFile, Section, LEVEL_SLOTS};

/// The save's two section kinds (`RestoreData`'s table argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Level(usize),
}

/// Every chunk of a save, in table order (the decoded sections of a `save%d.bin`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveData {
    pub tables: ChunkTables,
    pub global: Section,
    pub levels: Vec<Section>,
}

/// Chunk id 6000: `RestoreData` checks it but never copies it.
const NOT_RESTORED: i32 = 6000;

impl SaveData {
    /// The blank save (the disc's template: a new game's state, every level empty).
    pub fn from_file(tables: ChunkTables, file: SaveFile) -> SaveData { SaveData { tables, global: file.global, levels: file.levels } }

    /// The whole file (`MakeWholeSave`): the header and every section.
    pub fn to_file(&self) -> SaveFile { SaveFile { global: self.global.clone(), levels: self.levels.clone() } }

    /// The checksum's limit (the global section's size, `0x29bca0`).
    pub fn max(&self) -> usize { self.tables.global_size() }

    fn descs(&self, scope: Scope) -> &[ChunkDesc] {
        match scope {
            Scope::Global => &self.tables.global,
            Scope::Level(_) => &self.tables.level,
        }
    }

    fn section_mut(&mut self, scope: Scope) -> Option<&mut Section> {
        match scope {
            Scope::Global => Some(&mut self.global),
            Scope::Level(l) => self.levels.get_mut(l),
        }
    }

    fn section(&self, scope: Scope) -> Option<&Section> {
        match scope {
            Scope::Global => Some(&self.global),
            Scope::Level(l) => self.levels.get(l),
        }
    }

    /// The `n` bytes at EE address `addr` (a level chunk's slot-0 address for [`Scope::Level`]), when one chunk holds
    /// them all.
    pub fn bytes(&self, scope: Scope, addr: u32, n: usize) -> Option<&[u8]> {
        let d = *self.descs(scope).iter().find(|d| addr >= d.addr && (addr - d.addr) as usize + n <= d.size as usize)?;
        let c = self.section(scope)?.chunks.iter().find(|c| c.id == d.id)?;
        let o = (addr - d.addr) as usize;
        c.data.get(o..o + n)
    }

    pub fn bytes_mut(&mut self, scope: Scope, addr: u32, n: usize) -> Option<&mut [u8]> {
        let d = *self.descs(scope).iter().find(|d| addr >= d.addr && (addr - d.addr) as usize + n <= d.size as usize)?;
        let c = self.section_mut(scope)?.chunks.iter_mut().find(|c| c.id == d.id)?;
        let o = (addr - d.addr) as usize;
        c.data.get_mut(o..o + n)
    }

    pub fn u8(&self, scope: Scope, addr: u32) -> u8 { self.bytes(scope, addr, 1).map_or(0, |b| b[0]) }
    pub fn i32(&self, scope: Scope, addr: u32) -> i32 { self.bytes(scope, addr, 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap())) }

    pub fn set_u8(&mut self, scope: Scope, addr: u32, v: u8) {
        if let Some(b) = self.bytes_mut(scope, addr, 1) { b[0] = v; }
    }

    pub fn set_i32(&mut self, scope: Scope, addr: u32, v: i32) {
        if let Some(b) = self.bytes_mut(scope, addr, 4) { b.copy_from_slice(&v.to_le_bytes()); }
    }

    /// `RestoreData(buffer, level, table)` `0x29bf58` on a section read from the card: nothing when its checksum fails
    /// (one error); else each chunk whose id the table has is copied (the shorter of the two sizes; id 6000 never),
    /// an unknown id is an error, and so is a table chunk the section lacks or holds at another size, and a section
    /// whose walked size is not the table's. Returns the error count (the card record's `+0x24`).
    pub fn restore(&mut self, scope: Scope, card: &Section) -> u32 {
        if !card.crc_ok { return 1; }
        let descs = self.descs(scope).to_vec();
        let mut errors = 0u32;
        let mut seen = vec![false; descs.len()];
        let mut walked = 8usize;
        for c in &card.chunks {
            let Some(i) = descs.iter().position(|d| d.id == c.id) else {
                errors += 1;
                continue;
            };
            let size = descs[i].size as usize;
            seen[i] = c.data.len() == size;
            let n = size.min(c.data.len());
            if c.id != NOT_RESTORED {
                if let Some(dst) = self.section_mut(scope).and_then(|s| s.chunks.iter_mut().find(|x| x.id == c.id)) {
                    dst.data[..n].copy_from_slice(&c.data[..n]);
                }
            }
            walked += 8 + ((n + 3) & !3);
        }
        if walked + 8 != rc_formats::disc::save_game::section_size(&descs) { errors += 1; }
        errors + seen.iter().filter(|s| !**s).count() as u32
    }

    /// `PrepData(buffer, level, table)` `0x29bd88`: the section's bytes as the card gets them.
    pub fn prep(&self, scope: Scope) -> Vec<u8> {
        self.section(scope).map(|s| s.encode(self.max())).unwrap_or_default()
    }

    /// The level sections a save has.
    pub fn level_count(&self) -> usize { self.levels.len().min(LEVEL_SLOTS) }
}

/// Field addresses (module docs).
mod at {
    pub const LEVEL: u32 = 0x1a79f8;
    pub const BOLTS: u32 = 0x1a7a00;
    pub const RARITANIUM: u32 = 0x1a7a04;
    pub const MAX_HEALTH: u32 = 0x1a7a08;
    pub const WRENCH_OUT: u32 = 0x1a7a10;
    pub const THRUSTER: u32 = 0x1a7a11;
    pub const XP: u32 = 0x1a7a14;
    pub const XP_BANK: u32 = 0x1a7a18;
    pub const BOLT_BANK: u32 = 0x1a7a1c;
    pub const NANOTECH: u32 = 0x1a7a30;
    pub const ITEMS: u32 = 0x1a7b00;
    pub const ITEMS_LEN: usize = 0x38;
    pub const AMMO: u32 = 0x139688;
    pub const AMMO_LEN: usize = 0x38;
    pub const QUICK_SELECT: u32 = 0x139948;
    pub const QUICK_SELECT_LEN: usize = 28;
    pub const HELD: u32 = 0x1a7398;
    pub const RING: u32 = 0x1a73b8;
    pub const SCENES: u32 = 0x139768;
    pub const SAVE_CLOCK: u32 = 0x1a7360;
    pub const PLAY_TIME: u32 = 0x1a7bc8;
    pub const MISSIONS: u32 = 0x19b2e8;
    pub const UID_BITS: u32 = 0x19b4a8;
    pub const VISITED: u32 = 0x1a7bf0;
    /// The vendor's stock (crate::vendor) and the challenge mode.
    pub const STOCK: u32 = 0x1a7b70;
    pub const CHALLENGE: u32 = 0x1a7a12;
    /// The armour worn, the weapon mods on offer and bought (0x38 bytes each), the platinum bolts spent.
    pub const ARMOR: u32 = 0x1a7a20;
    pub const MODS_AVAILABLE: u32 = 0x1a7ac0;
    pub const MODS_BOUGHT: u32 = 0x1a7a88;
    pub const PLATINUM_SPENT: u32 = 0x139648;
    /// The platinum bolts found, 4 bytes a level.
    pub const PLATINUM: u32 = 0x19b278;
    /// The planets unlocked (`0x322730`), one byte per level (crate::travel).
    pub const PLANETS: u32 = 0x1a7bd0;
    pub const PLANETS_LEN: usize = 0x1c;
    /// Item ids of the bytes `GameState` also keeps as flags.
    pub const CLANK: usize = 2;
    pub const GRAVITY_BOOTS: usize = 0x13;
    pub const GRIND_BOOTS: usize = 0x14;
    pub const CHARGE_BOOTS: usize = 0x36;
}

/// Scene `n`'s bit: the word at byte `n & !3` of `0x139768`, bit `n & 31` (the scene code's indexing).
fn scene_bit(n: usize) -> (u32, u32) { (at::SCENES + (n as u32 & !3), 1 << (n & 31)) }

impl GameState {
    /// Writes the typed fields into the save's chunks (what `PrepData` reads from RAM). Per level, the missions of
    /// every level and the uid bits of the current one.
    pub fn store(&self, s: &mut SaveData) {
        use Scope::Global as G;
        s.set_i32(G, at::LEVEL, self.level);
        s.set_i32(G, at::BOLTS, self.bolts as i32);
        s.set_i32(G, at::RARITANIUM, self.raritanium as i32);
        s.set_i32(G, at::MAX_HEALTH, self.max_health);
        s.set_u8(G, at::WRENCH_OUT, self.wrench_out as u8);
        s.set_u8(G, at::THRUSTER, self.thruster as u8);
        s.set_i32(G, at::XP, self.xp);
        s.set_i32(G, at::XP_BANK, self.xp_bank);
        s.set_i32(G, at::BOLT_BANK, self.bolt_bank);
        for (i, &n) in self.nanotech.iter().enumerate() { s.set_u8(G, at::NANOTECH + i as u32, n as u8); }
        for i in 0..at::ITEMS_LEN {
            let had = self.items.get(i).copied().unwrap_or(0);
            s.set_u8(G, at::ITEMS + i as u32, had);
        }
        for (item, on) in [(at::CLANK, self.clank), (at::GRAVITY_BOOTS, self.boots), (at::GRIND_BOOTS, self.grind_boots), (at::CHARGE_BOOTS, self.charge_boots)] {
            s.set_u8(G, at::ITEMS + item as u32, on as u8);
        }
        for i in 0..at::AMMO_LEN { s.set_i32(G, at::AMMO + 4 * i as u32, self.ammo.get(i).copied().unwrap_or(0)); }
        for i in 0..at::QUICK_SELECT_LEN { s.set_i32(G, at::QUICK_SELECT + 4 * i as u32, self.quick_select.get(i).copied().unwrap_or(0)); }
        for i in 0..at::PLANETS_LEN { s.set_u8(G, at::PLANETS + i as u32, self.planets.get(i).copied().unwrap_or(0)); }
        s.set_i32(G, at::HELD, self.held);
        for (i, &item) in self.ring.iter().enumerate() { s.set_i32(G, at::RING + 4 * i as u32, item); }
        s.set_i32(G, at::PLAY_TIME, self.play_time);
        if let Some(b) = s.bytes_mut(G, at::STOCK, self.stock.len()) { b.copy_from_slice(&self.stock); }
        s.set_u8(G, at::CHALLENGE, self.challenge as u8);
        if let Some(b) = s.bytes_mut(G, at::ARMOR, 2) { b.copy_from_slice(&self.armor.to_le_bytes()); }
        if let Some(b) = s.bytes_mut(G, at::MODS_AVAILABLE, 0x38) { b.copy_from_slice(&self.mods_available); }
        if let Some(b) = s.bytes_mut(G, at::MODS_BOUGHT, 0x38) { b.copy_from_slice(&self.mods_bought); }
        s.set_u8(G, at::PLATINUM_SPENT, self.platinum_spent);
        if let Some(b) = s.bytes_mut(G, at::PLATINUM, self.platinum.len()) { b.copy_from_slice(&self.platinum); }
        for (n, &seen) in self.scenes_seen.iter().enumerate() {
            let (w, bit) = scene_bit(n);
            let v = s.i32(G, w) as u32;
            s.set_i32(G, w, (if seen { v | bit } else { v & !bit }) as i32);
        }
        for l in 0..s.level_count() {
            for k in 0..0x10u32 {
                let v = self.missions.get(l * 0x10 + k as usize).copied().unwrap_or(0);
                s.set_u8(Scope::Level(l), at::MISSIONS + k, v);
            }
            set_visited(s, l, self.visited.get(l).copied().unwrap_or(0));
        }
        if let Ok(l) = usize::try_from(self.level) {
            if let Some(b) = s.bytes_mut(Scope::Level(l), at::UID_BITS, 0x400) {
                let n = self.uid_bits.len().min(0x400);
                b[..n].copy_from_slice(&self.uid_bits[..n]);
            }
        }
    }

    /// Reads the typed fields from the save's chunks (after `RestoreData`); the uid bits of `self.level`.
    pub fn load(&mut self, s: &SaveData) {
        use Scope::Global as G;
        self.level = s.i32(G, at::LEVEL);
        self.bolts = s.i32(G, at::BOLTS) as u32;
        self.raritanium = s.i32(G, at::RARITANIUM) as u32;
        self.max_health = s.i32(G, at::MAX_HEALTH);
        self.wrench_out = s.u8(G, at::WRENCH_OUT) != 0;
        self.thruster = s.u8(G, at::THRUSTER) != 0;
        self.xp = s.i32(G, at::XP);
        self.xp_bank = s.i32(G, at::XP_BANK);
        self.bolt_bank = s.i32(G, at::BOLT_BANK);
        for (i, n) in self.nanotech.iter_mut().enumerate() { *n = s.u8(G, at::NANOTECH + i as u32) != 0; }
        for i in 0..at::ITEMS_LEN {
            if let Some(b) = self.items.get_mut(i) { *b = s.u8(G, at::ITEMS + i as u32); }
        }
        let had = |item: usize| s.u8(G, at::ITEMS + item as u32) != 0;
        (self.clank, self.boots, self.grind_boots, self.charge_boots) = (had(at::CLANK), had(at::GRAVITY_BOOTS), had(at::GRIND_BOOTS), had(at::CHARGE_BOOTS));
        for i in 0..at::AMMO_LEN {
            if let Some(a) = self.ammo.get_mut(i) { *a = s.i32(G, at::AMMO + 4 * i as u32); }
        }
        for i in 0..at::QUICK_SELECT_LEN {
            if let Some(q) = self.quick_select.get_mut(i) { *q = s.i32(G, at::QUICK_SELECT + 4 * i as u32); }
        }
        for i in 0..at::PLANETS_LEN {
            if let Some(p) = self.planets.get_mut(i) { *p = s.u8(G, at::PLANETS + i as u32); }
        }
        self.held = s.i32(G, at::HELD);
        for (i, item) in self.ring.iter_mut().enumerate() { *item = s.i32(G, at::RING + 4 * i as u32); }
        // Port-only: the port's saves before the quick select left its slots empty; the items had get theirs.
        if self.ring == [0; 8] {
            for &item in &crate::game::RING_ITEMS {
                if self.items.get(item as usize).is_some_and(|&b| b != 0) { self.ring_add(item); }
            }
        }
        self.play_time = s.i32(G, at::PLAY_TIME);
        if let Some(b) = s.bytes(G, at::STOCK, self.stock.len()) { self.stock.copy_from_slice(b); }
        self.challenge = s.u8(G, at::CHALLENGE) != 0;
        if let Some(b) = s.bytes(G, at::ARMOR, 2) { self.armor = i16::from_le_bytes([b[0], b[1]]); }
        if let Some(b) = s.bytes(G, at::MODS_AVAILABLE, 0x38) { self.mods_available.copy_from_slice(b); }
        if let Some(b) = s.bytes(G, at::MODS_BOUGHT, 0x38) { self.mods_bought.copy_from_slice(b); }
        self.platinum_spent = s.u8(G, at::PLATINUM_SPENT);
        if let Some(b) = s.bytes(G, at::PLATINUM, self.platinum.len()) { self.platinum.copy_from_slice(b); }
        for n in 0..self.scenes_seen.len() {
            let (w, bit) = scene_bit(n);
            self.scenes_seen[n] = s.i32(G, w) as u32 & bit != 0;
        }
        for l in 0..s.level_count() {
            for k in 0..0x10u32 {
                if let Some(m) = self.missions.get_mut(l * 0x10 + k as usize) { *m = s.u8(Scope::Level(l), at::MISSIONS + k); }
            }
            if let Some(v) = self.visited.get_mut(l) { *v = visited(s, l); }
        }
        if let Some(b) = usize::try_from(self.level).ok().and_then(|l| s.bytes(Scope::Level(l), at::UID_BITS, 0x400)) {
            let n = self.uid_bits.len().min(0x400);
            self.uid_bits[..n].copy_from_slice(&b[..n]);
        }
    }
}

/// The save clock (`0x1a7360`, `sceCdCLOCK` BCD) in a save.
pub fn set_clock(s: &mut SaveData, clock: [u8; 8]) {
    if let Some(b) = s.bytes_mut(Scope::Global, at::SAVE_CLOCK, 8) { b.copy_from_slice(&clock); }
}

/// `memcard_Save`'s visited byte of `level` (`0x1a7bf0[level]`, level chunk 3001).
pub fn visited(s: &SaveData, level: usize) -> u8 { s.u8(Scope::Level(level), at::VISITED) }
pub fn set_visited(s: &mut SaveData, level: usize, v: u8) { s.set_u8(Scope::Level(level), at::VISITED, v) }

#[cfg(test)]
mod tests {
    use super::*;
    use rc_formats::disc::save_game::{Chunk, ChunkDesc};

    /// A two-chunk global table and a one-chunk level table, with a blank save of them.
    fn blank() -> SaveData {
        let tables = ChunkTables {
            global: vec![ChunkDesc { addr: at::LEVEL, size: 4, id: 0 }, ChunkDesc { addr: at::BOLTS, size: 0x40, id: 39 }],
            level: vec![ChunkDesc { addr: at::MISSIONS, size: 0x10, id: 3004 }],
        };
        let global = Section::new(vec![Chunk::new(0, vec![0; 4]), Chunk::new(39, vec![0; 0x40])]);
        SaveData { tables, global, levels: vec![Section::new(vec![Chunk::new(3004, vec![0; 0x10])]); 2] }
    }

    #[test]
    fn fields_round_trip_through_the_chunks() {
        let mut s = blank();
        let mut g = GameState { level: 1, bolts: 1234, xp: 77, max_health: 9, thruster: true, ..Default::default() };
        g.missions[0x10 + 3] = 0x80;
        g.store(&mut s);
        assert_eq!(s.i32(Scope::Global, at::BOLTS), 1234);
        assert_eq!(s.u8(Scope::Level(1), at::MISSIONS + 3), 0x80);
        let mut back = GameState::default();
        back.load(&s);
        assert_eq!((back.level, back.bolts, back.xp, back.max_health, back.thruster, back.missions[0x13]), (1, 1234, 77, 9, true, 0x80));
    }

    #[test]
    fn restore_copies_and_counts_errors() {
        let mut s = blank();
        let mut card = s.global.clone();
        card.chunks[1].data[0] = 5;
        assert_eq!(s.restore(Scope::Global, &card), 0);
        assert_eq!(s.i32(Scope::Global, at::BOLTS), 5);
        // A chunk missing and an unknown one: two errors (and the size walk differs).
        let odd = Section::new(vec![Chunk::new(0, vec![1, 0, 0, 0]), Chunk::new(77, vec![0; 4])]);
        assert!(s.restore(Scope::Global, &odd) >= 2);
        assert_eq!(s.i32(Scope::Global, at::LEVEL), 1, "the known chunk is still copied");
        // A failed checksum copies nothing.
        let mut bad = card.clone();
        bad.crc_ok = false;
        bad.chunks[1].data[0] = 9;
        assert_eq!(s.restore(Scope::Global, &bad), 1);
        assert_eq!(s.i32(Scope::Global, at::BOLTS), 5);
    }

    #[test]
    fn scene_bits_use_the_games_indexing() {
        assert_eq!(scene_bit(0x21), (at::SCENES + 0x20, 1 << 1));
        assert_eq!(scene_bit(5), (at::SCENES + 4, 1 << 5));
    }
}
