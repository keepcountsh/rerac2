//! The planet page's missions list (L00): the level's table (`0x281560[level]`, one per level in Aranos's overlay),
//! its states (`0x30faf8`), the list it makes (`0x30fdf0(…, 1)`) and the widget (`0x351b90` keys, `0x353fb0` draw).
//!
//! **A mission entry** (0x28 bytes): `+0` its text (0 ends the table), `+2` / `+4` the condition that shows it,
//! `+8` / `+0xc` the one that completes it, `+0x10` flags (1: a completed one is left out, 2: never listed, 4: only
//! once the planet is visited), `+0x14..` the list's texts (`0x30fdf0` with its last argument 1 takes `+0x14 +
//! 2·result`, the result of the entry's own function `+0x1c` on `+0x20`, 0 here), `+0x24` the state, `+0x26` the
//! result. **A condition** (`0x30fc98(kind, arg)`): 0 always; 1 planet `arg` unlocked (`0x1a7bd0`); 2 item `arg` had
//! (`0x1a7b00`); 3 item `arg` ever had (`0x1a7b38`); 4 / 5 the item record `0x139a34 + 0x10·arg`'s word ≥ 1 / ≥ 2;
//! 6 the flag byte `0x1395b8[arg]`; 7 the function `arg`; 8 the byte `0x19b278 + 4·(arg >> 16) + (arg & 0xffff)`;
//! 9 scene `arg` seen (`0x139768`); 10 `0x37c3f8(level)`. **A state** (`0x30faf8`): 0 hidden, 1 open (the first
//! condition met), 2 done (both). **The list**: the entries not flagged 2 with a state, a done one left out when
//! flagged 1, its text 0x31b9 ("Completed") when done; the widget's row is per level (`+0x30 + 4·level`).
//!
//! Not ported: the kinds 4..8 and 10 (the port keeps none of those records and flags: never met), the entries' own
//! functions (`+0x1c`: the result 0), the `+0x12 + result` list (`0x70000100`: nothing reads it on the ship's pages),
//! the rows' text wrapping (`0x2eaf70`: one line each), the selection's glow (`0x2e9ae0`).

use crate::game::GameState;

/// What the conditions read of the game (disjoint fields, so the menu can be borrowed beside them).
#[derive(Clone, Copy, Debug)]
pub struct Flags<'a> {
    pub planets: &'a [u8],
    pub items: &'a [u8],
    pub visited: &'a [u8],
    pub scenes_seen: &'a [bool],
}

impl<'a> Flags<'a> {
    pub fn of(g: &'a GameState) -> Flags<'a> { Flags { planets: &g.planets, items: &g.items, visited: &g.visited, scenes_seen: &g.scenes_seen } }
}

/// `0x281560`: the levels' tables (0x1c pointers).
pub const TABLES: u32 = 0x0028_1560;
pub const ENTRY: usize = 0x28;
/// 0x31b9: "Completed".
pub const DONE_TEXT: u32 = 0x31b9;
/// The widget (`0x353fb0`): the title 0x2c5b at its rect, the rows from 0x14 below at 16 a line, the check box at
/// x + 9 (`0x3542c0`: 10 × 10, the mark icon 0xe99d frame 1 at (−13, −18), 30 × 30), the text at x + 16.
pub const TITLE_TEXT: u32 = 0x2c5b;
pub const MARK_ICON: u16 = 0xe99d;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mission {
    pub text: i32,
    pub show: (i16, i32),
    pub done: (i16, i32),
    pub flags: u16,
    pub texts: [i32; 4],
}

/// The levels' missions from Aranos's overlay through `read(address, length)`.
pub fn read_tables(read: &dyn Fn(u32, usize) -> Option<Vec<u8>>) -> Vec<Vec<Mission>> {
    let Some(ptrs) = read(TABLES, 0x1c * 4) else { return Vec::new() };
    (0..0x1c)
        .map(|l| {
            let at = u32::from_le_bytes(ptrs[4 * l..4 * l + 4].try_into().unwrap());
            let mut out = Vec::new();
            for k in 0..64 {
                let Some(e) = read(at + (k * ENTRY) as u32, ENTRY) else { break };
                let i16 = |o: usize| i16::from_le_bytes([e[o], e[o + 1]]);
                let i32 = |o: usize| i32::from_le_bytes(e[o..o + 4].try_into().unwrap());
                let text = i16(0) as i32;
                if text == 0 { break; }
                out.push(Mission { text, show: (i16(2), i32(4)), done: (i16(8), i32(0xc)), flags: i16(0x10) as u16, texts: [i16(0x14) as i32, i16(0x16) as i32, i16(0x18) as i32, i16(0x1a) as i32] });
            }
            out
        })
        .collect()
}

/// `0x30fc98(kind, arg)` on the game's state (module docs).
pub fn condition(g: &Flags, kind: i16, arg: i32) -> bool {
    let byte = |v: &[u8], i: i32| usize::try_from(i).ok().and_then(|i| v.get(i)).is_some_and(|&b| b != 0);
    match kind {
        0 => true,
        1 => byte(g.planets, arg),
        2 | 3 => byte(g.items, arg),
        9 => usize::try_from(arg).ok().and_then(|i| g.scenes_seen.get(i)).copied().unwrap_or(false),
        _ => false,
    }
}

/// `0x30faf8`'s state of a mission on `level`.
pub fn state(g: &Flags, level: i32, m: &Mission) -> u8 {
    if m.flags & 4 != 0 && !usize::try_from(level).ok().and_then(|l| g.visited.get(l)).is_some_and(|&v| v != 0) { return 0; }
    if !condition(g, m.show.0, m.show.1) { return 0; }
    if condition(g, m.done.0, m.done.1) { 2 } else { 1 }
}

/// `0x30fdf0(…, 1)`: the rows shown: (text, done).
pub fn rows(g: &Flags, level: i32, table: &[Mission]) -> Vec<(u32, bool)> {
    table
        .iter()
        .filter_map(|m| {
            if m.flags & 2 != 0 { return None; }
            let s = state(g, level, m);
            if s == 0 || (m.flags & 1 != 0 && s == 2) { return None; }
            Some(if s == 2 { (DONE_TEXT, true) } else { (m.texts[0].max(0) as u32, false) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oozla_rows_follow_the_items() {
        // Oozla's table (`0x27ef40`): the first mission always shown, done by an item record (never here); the third
        // once visited, done by item 0x24; the fourth after item 0x24, done by planet 2; the fifth never listed.
        let t = vec![
            Mission { text: 0x115a, show: (0, 0), done: (4, 3), flags: 0, texts: [0x115b, 0, 0, 0] },
            Mission { text: 0x115c, show: (4, 3), done: (2, 0x2e), flags: 0, texts: [0x115d, 0, 0, 0] },
            Mission { text: 0x1156, show: (0, 0), done: (2, 0x24), flags: 4, texts: [0x1157, 0, 0, 0] },
            Mission { text: 0x1158, show: (2, 0x24), done: (1, 2), flags: 4, texts: [0x1159, 0, 0, 0] },
            Mission { text: -1, show: (10, 2), done: (0xb, 0), flags: 2, texts: [0; 4] },
        ];
        let mut g = GameState::default();
        assert_eq!(rows(&Flags::of(&g), 1, &t), vec![(0x115b, false)]);
        g.visited[1] = 1;
        assert_eq!(rows(&Flags::of(&g), 1, &t), vec![(0x115b, false), (0x1157, false)]);
        g.items[0x24] = 1;
        assert_eq!(rows(&Flags::of(&g), 1, &t), vec![(0x115b, false), (DONE_TEXT, true), (0x1159, false)]);
    }
}
