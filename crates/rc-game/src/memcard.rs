//! The memory card, natively: the card machine `0x299cf0` (state `0x13953c`, substate `0x139540`), the card monitor
//! `0x298f20` (status `0x1a7420`, flags `0x1a7424`, its 26 handlers through the table `0x256050`) and `memcard_Save`
//! `0x29c1d0`. R&C1's port is the map (rerac:rc_game::memcard): the same machine, here with Going Commando's card
//! folder, 4 slots, 28 level sections, its size check and the scan of R&C1's saves.
//!
//! **Native card.** A card is a folder ([`CardFs`]): `<save root>/BASCUS-97268RATCHET/` holding exactly the files the
//! game writes on a card (`icon.sys`, `static.ico`, `save0.bin`..`save3.bin`, the `BASCUS-97268RATCHET` reservation),
//! byte-compatible with a real card's. The save root is the one R&C1's port uses (`<config>/rerac/memcard`, a
//! directory per game, as on a PS2 card), so the scan of R&C1's saves (states 0x18..0x1b) finds the R&C1 port's. The
//! libmc calls run synchronously on the folder and their results reach the machine one frame later through the
//! `sceMcSync` step the game polls (`0x139534`), so every state runs in the game's order, one call per frame. A written
//! file is replaced atomically on close. The machine polls two card ports, as the game does; port 1 has no card.
//!
//! **What reaches the save.** Loads restore the global section and the 28 level sections into the
//! [`SaveData`] through `RestoreData` (crate::save); the error count lands in [`Card::errors`]. Saves write the bytes
//! `PrepData` made when the save was asked for ([`MemCard::memcard_save`]), or the whole-save buffer (state 0x13).
//!
//! ## Coverage (`0x299cf0`; `r` = the last libmc result `0x139524`)
//! | state | what | port |
//! |---|---|---|
//! | entry | busy `0x139534`: `sceMcSync` → r, busy = not done; else busy = 1 | [`MemCard::update`] |
//! | 0 / 1 / 2 | poll ports 0..1 (`sceMcGetInfo`); r ≠ 0: slot −4, info r, errors −1, used 0, previews empty; after the requested card: the request (card info ≠ 0 and not 0x18: error 0x271a); idle 10 frames | ported |
//! | 3 / 4 | format (unformatted cards) → error 1 | ported |
//! | 5 / 6 | unformat → error 2 | ported (no caller) |
//! | 7 / 8.0 | `sceMcGetDir("/…RATCHET/*")`: the folder holds the 9 entries and 0x1da..0x1db KB → slot −1; else slot −2, used = its KB; r −2 → error 3, < 0 (not −4) → 4 | ported |
//! | 8.1–8.5 | open save0, read the header, compare with the tables' sizes (errors), close → 0x15; errors 10000, 0x2711..0x2716, 3, 4 | ported |
//! | 9 / 10 | create: free + used < 0x1db KB → error 7; mkdir; the save blob from the disc; icon.sys, static.ico, save0..3 (the blank save), the reservation (0xd804 bytes of RAM from 0x1a79f8: zero-filled here [L]); errors 6, 7, 8, 0xb, 0xc, 0xd, 10 | ported |
//! | 0xb / 0xc | delete the folder | not ported (no caller) |
//! | 0xd / 0xe | load: save%d (slot): header, global (≤ 0x2800, `RestoreData`), 28 levels (≤ 0x1000 each); errors 0x13..0x1c; `0x29c418` after | ported |
//! | 0xf / 0x10 | incremental save: errors → 0x2717, no slot → 0x1d; write the global section and the save's level section; errors 0x1e..0x26 | ported |
//! | 0x11 / 0x12 | erase a slot to the blank save | not ported (no caller) |
//! | 0x13 / 0x14 | whole save of the prepared buffer; errors 0x2718, 0x27, 0x2b, 0xb, 0x28..0x2d | ported |
//! | 0x15..0x17 | the 4 previews: open, header, global section → `0x29bea0` | ported |
//! | 0x18..0x1b | R&C1's saves (`BASCUS-97199RATCHET` / `BESCES-50916` / `BISCPS-15037`, 5 slots each; the first missing slot ends a region): checksum, chunk 10's five bytes → the bonus bits `0x1a7a28` and the card's count | ported |

use crate::save::{set_clock, set_visited, visited, SaveData, Scope};
use rc_formats::disc::save_game::{save_file_name, Section, SaveBlob, LEVEL_SLOTS, SAVE_SLOTS};
use std::path::{Path, PathBuf};

/// Card ports the machine polls.
pub const PORTS: usize = 2;
/// The KB a save folder takes (the check's 0x1da..0x1db and the create's "free + used" floor).
pub const FOLDER_KB: i32 = 0x1db;
/// The free KB the native card reports (an empty 8 MB card).
pub const NATIVE_FREE: i32 = 8000;
/// The reservation file's size (0xd804 bytes of RAM from 0x1a79f8).
pub const RESERVE: usize = 0xd804;
/// The directory listing's capacity (`sceMcGetDir`'s maxent 0xb) and the entries a whole folder has.
pub const DIR_MAX: usize = 0xb;
pub const DIR_ENTRIES: i32 = 9;
/// `sceMcGetInfo`'s card type of a PS2 memory card.
pub const TYPE_PS2: i32 = 2;
/// R&C1's save folders (`0x1a9a00` / `0x1a9a20` / `0x1a9a40`) and their slots.
pub const RAC1_DIRS: [&str; 3] = ["BASCUS-97199RATCHET", "BESCES-50916RATCHET", "BISCPS-15037RATCHET"];
pub const RAC1_SLOTS: i32 = 5;
/// The size the scan reads of an R&C1 save (`0x2800`).
pub const RAC1_READ: usize = 0x2800;

/// libmc results.
pub mod mc {
    pub const OK: i32 = 0;
    pub const NO_FORMAT: i32 = -2;
    pub const FULL: i32 = -3;
    pub const NOT_FOUND: i32 = -4;
    pub const DENIED: i32 = -5;
    pub const NO_CARD: i32 = -10;
}

/// The monitor's status (`0x1a7420`), named after the dialog each one shows.
pub mod status {
    pub const INIT: i32 = 0;
    /// Checked (`0x13955c`): auto-save on.
    pub const READY: i32 = 1;
    pub const CHANGED: i32 = 2;
    pub const RECHECK: i32 = 3;
    pub const POLL: i32 = 4;
    pub const UNFORMATTED: i32 = 5;
    pub const FORMAT_ASK: i32 = 6;
    pub const FORMAT: i32 = 7;
    pub const FORMATTING: i32 = 8;
    pub const PRESENT: i32 = 9;
    pub const CHECK: i32 = 10;
    pub const CHECKING: i32 = 11;
    pub const NO_DATA: i32 = 12;
    pub const CREATE_ASK: i32 = 13;
    pub const CREATE: i32 = 14;
    pub const CREATING: i32 = 15;
    /// Data found (auto-save still off until a save or load).
    pub const FOUND: i32 = 16;
    pub const FORMAT_FAILED: i32 = 17;
    pub const CREATE_FAILED: i32 = 18;
    pub const NO_SPACE: i32 = 19;
    pub const LOAD_FAILED: i32 = 20;
    pub const SAVE_FAILED: i32 = 21;
    /// After a create: the folder checked again; auto-save on.
    pub const CREATED: i32 = 22;
    pub const UNFORMATTED_FE: i32 = 23;
    pub const NO_DATA_FE: i32 = 24;
    /// An auto-save running (`0x2997c8`).
    pub const AUTOSAVE: i32 = 25;
}

/// One slot's preview (`0x139410 + card·0xa0 + slot·0x1c`, `0x29bea0`): the global section's bytes at +0x10 (the
/// level), +0x1c (the bolts), +0x2e (the byte `0x1a7a12`), +0x64 (`0x1a7bc8`) and +0x70 (the save clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preview {
    /// −1: an empty slot (the blank save's level).
    pub level: i32,
    pub bolts: i32,
    pub byte_12: i32,
    pub word_bc8: i32,
    /// `sceCdCLOCK` (BCD): [0] stat, [1] second, [2] minute, [3] hour, [4] pad, [5] day, [6] month, [7] year.
    pub clock: [u8; 8],
    /// +0x18: the global section's checksum failed.
    pub bad: bool,
}

impl Default for Preview {
    fn default() -> Self { Preview { level: -1, bolts: 0, byte_12: 0, word_bc8: 0, clock: [0; 8], bad: false } }
}

impl Preview {
    /// `0x29bea0` on a global section's bytes (header included).
    pub fn from_section(b: &[u8], max: usize) -> Preview {
        let w = |o: usize| b.get(o..o + 4).map_or(0, |x| i32::from_le_bytes(x.try_into().unwrap()));
        let ok = Section::decode(b, max).is_ok_and(|s| s.crc_ok);
        let mut clock = [0u8; 8];
        if let Some(c) = b.get(0x70..0x78) { clock.copy_from_slice(c); }
        Preview { level: w(0x10), bolts: w(0x1c), byte_12: b.get(0x2e).copied().unwrap_or(0) as i32, word_bc8: w(0x64), clock, bad: !ok }
    }
}

/// The card record (`0x1393e0`, 0xa0 bytes per port).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// +0x08 `sceMcGetInfo` type (2 = PS2 card), +0x0c free KB, +0x14 formatted.
    pub ty: i32,
    pub free: i32,
    pub formatted: i32,
    /// +0x10: the last `sceMcGetInfo` result (0 same card, −1 new formatted card, −2 unformatted, ≤ −10 none).
    pub info: i32,
    /// +0x18: the active save slot 0..3; −1 the folder is whole with no slot chosen; −2 no (whole) folder; −4 no card.
    pub slot: i32,
    /// +0x1a: the preview loop's slot.
    pub preview_slot: i32,
    /// +0x1c: the R&C1 saves' bonus bits counted.
    pub bonus: i32,
    /// +0x20: the KB a broken folder takes (the create's floor counts them back).
    pub used: i32,
    /// +0x24: `RestoreData` errors of the last load (−1 no card).
    pub errors: i32,
    /// +0x28: the last header read (global, level section sizes).
    pub header: [u32; 2],
    /// +0x30: the four previews.
    pub previews: [Preview; SAVE_SLOTS],
}

impl Default for Card {
    fn default() -> Self {
        Card { ty: 0, free: 0, formatted: 0, info: 0, slot: -4, preview_slot: 0, bonus: 0, used: 0, errors: 0, header: [0; 2], previews: [Preview::default(); SAVE_SLOTS] }
    }
}

/// `sceMcOpen` modes.
pub mod open_mode {
    pub const READ: i32 = 1;
    pub const WRITE: i32 = 2;
    pub const CREATE_RDWR: i32 = 0x203;
}

/// The folder standing for a memory card (module docs).
#[derive(Clone, Debug)]
pub struct CardFs {
    /// The folder holding the card's directories (the save root); None = no card inserted.
    pub root: Option<PathBuf>,
    open: Vec<Option<OpenFile>>,
}

#[derive(Clone, Debug)]
struct OpenFile {
    path: PathBuf,
    data: Vec<u8>,
    pos: usize,
    write: bool,
}

impl CardFs {
    pub fn new(root: Option<PathBuf>) -> CardFs { CardFs { root, open: Vec::new() } }

    /// The host path of a card path (`/DIR/file`, or relative to the card's root).
    fn host(&self, p: &str) -> Option<PathBuf> {
        let mut h = self.root.clone()?;
        for part in p.split('/').filter(|s| !s.is_empty()) {
            if part == ".." || part.contains('\\') { return None; }
            h.push(part);
        }
        Some(h)
    }

    /// `sceMcGetInfo`: (result, type, free KB, formatted). The root is created on first use (an empty card).
    pub fn get_info(&mut self) -> (i32, i32, i32, i32) {
        match &self.root {
            Some(r) if std::fs::create_dir_all(r).is_ok() => (mc::OK, TYPE_PS2, NATIVE_FREE, 1),
            _ => (mc::NO_CARD, 0, 0, 0),
        }
    }

    pub fn format(&mut self) -> i32 {
        match &self.root {
            Some(r) if std::fs::create_dir_all(r).is_ok() => mc::OK,
            _ => mc::NO_CARD,
        }
    }

    pub fn mkdir(&mut self, dir: &str) -> i32 {
        let Some(h) = self.host(dir) else { return mc::NO_CARD };
        if h.is_dir() { return mc::NOT_FOUND; }
        match std::fs::create_dir_all(&h) {
            Ok(()) => mc::OK,
            Err(_) => mc::DENIED,
        }
    }

    /// `sceMcGetDir("/DIR/*")`: the number of entries (`.` and `..` first, as on a card), with each name and size; −4
    /// when the folder does not exist.
    pub fn get_dir(&mut self, pattern: &str) -> (i32, Vec<(String, u64)>) {
        let dir = pattern.trim_end_matches('*').trim_end_matches('/');
        let Some(h) = self.host(dir) else { return (mc::NO_CARD, Vec::new()) };
        let Ok(rd) = std::fs::read_dir(&h) else { return (mc::NOT_FOUND, Vec::new()) };
        let mut out = vec![(".".to_string(), 0), ("..".to_string(), 0)];
        let mut files: Vec<(String, u64)> = rd
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| Some((e.file_name().into_string().ok()?, e.metadata().ok()?.len())))
            .filter(|(n, _)| !n.ends_with(".tmp-rerac"))
            .collect();
        files.sort();
        out.extend(files);
        out.truncate(DIR_MAX);
        (out.len() as i32, out)
    }

    /// `sceMcOpen(path, mode)`: a descriptor ≥ 0, or −4 (not found).
    pub fn open(&mut self, path: &str, mode: i32) -> i32 {
        let Some(h) = self.host(path) else { return mc::NO_CARD };
        let create = mode & 0x200 != 0;
        let data = match std::fs::read(&h) {
            Ok(d) if !create => d,
            Ok(_) => Vec::new(),
            Err(_) if create && h.parent().is_some_and(Path::is_dir) => Vec::new(),
            Err(_) => return mc::NOT_FOUND,
        };
        let f = OpenFile { path: h, data, pos: 0, write: mode & 2 != 0 };
        let fd = match self.open.iter().position(Option::is_none) {
            Some(i) => {
                self.open[i] = Some(f);
                i
            }
            None => {
                self.open.push(Some(f));
                self.open.len() - 1
            }
        };
        fd as i32
    }

    fn file_mut(&mut self, fd: i32) -> Option<&mut OpenFile> { usize::try_from(fd).ok().and_then(|i| self.open.get_mut(i)).and_then(Option::as_mut) }

    pub fn read(&mut self, fd: i32, n: usize) -> (i32, Vec<u8>) {
        let Some(f) = self.file_mut(fd) else { return (mc::DENIED, Vec::new()) };
        let end = (f.pos + n).min(f.data.len()).max(f.pos);
        let out = f.data.get(f.pos..end).map(<[u8]>::to_vec).unwrap_or_default();
        f.pos = end;
        (out.len() as i32, out)
    }

    pub fn write(&mut self, fd: i32, b: &[u8]) -> i32 {
        let Some(f) = self.file_mut(fd) else { return mc::DENIED };
        if !f.write { return mc::DENIED; }
        if f.data.len() < f.pos + b.len() { f.data.resize(f.pos + b.len(), 0); }
        f.data[f.pos..f.pos + b.len()].copy_from_slice(b);
        f.pos += b.len();
        b.len() as i32
    }

    /// `sceMcSeek(fd, offset, whence)`: 0 from the start, 1 from the current position; the new position.
    pub fn seek(&mut self, fd: i32, off: i64, whence: i32) -> i32 {
        let Some(f) = self.file_mut(fd) else { return mc::DENIED };
        let base = if whence == 1 { f.pos as i64 } else { 0 };
        f.pos = (base + off).max(0) as usize;
        f.pos as i32
    }

    /// `sceMcClose`: a written file is replaced atomically.
    pub fn close(&mut self, fd: i32) -> i32 {
        let Some(f) = usize::try_from(fd).ok().and_then(|i| self.open.get_mut(i)).and_then(Option::take) else { return mc::DENIED };
        if !f.write { return mc::OK; }
        let tmp = f.path.with_extension("tmp-rerac");
        if std::fs::write(&tmp, &f.data).and_then(|_| std::fs::rename(&tmp, &f.path)).is_ok() { mc::OK } else { mc::DENIED }
    }
}

/// What a create step writes (`0x139554` / `0x139558`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Src {
    IconSys,
    StaticIco,
    Template,
    Reserve,
}

/// The card globals (`0x139520..0x139564`) and the monitor's (`0x1a7420`, `0x1a7424`, `0x1b19b8`, `0x1b19bc`).
#[derive(Clone, Debug)]
pub struct MemCard {
    pub fs: CardFs,
    /// The card folder (`0x299998`, e.g. `BASCUS-97268RATCHET`).
    pub dir: String,
    /// The disc's save blob (icon.sys, static.ico, the blank save).
    pub blob: Option<SaveBlob>,
    pub cards: [Card; PORTS],
    /// `0x139524` the last result, `0x13952c` the card being worked on, `0x139530` the level loop, `0x139534` busy, `0x139538`
    /// the descriptor, `0x139528` the save card (−1 none).
    pub result: i32,
    pub cur: usize,
    pub level: usize,
    pub busy: bool,
    pub fd: i32,
    pub save_card: i32,
    /// `0x13953c` state, `0x139540` substate, `0x139544` / `0x139548` the requested state and card (−1 none).
    pub state: i32,
    pub sub: i32,
    pub req_state: i32,
    pub req_card: i32,
    /// `0x13954c` the error code (the dialog's message), `0x139550` its card.
    pub error: i32,
    pub error_card: usize,
    /// `0x13955c`: checked (a save or load was made: auto-save on).
    pub checked: bool,
    /// `0x139560` the polled port, `0x139564` the idle frames.
    poll: usize,
    idle_frames: i32,
    /// `0x1a7420` / `0x1a7424` / `0x1b19b8` (frames in the status) / `0x1b19bc` (an error was seen).
    pub status: i32,
    pub flags: u32,
    pub status_frames: i32,
    pub error_seen: bool,
    /// `0x1a8c88`: the front end is up; `0x1a8c8c`: the front end's own card check is running.
    pub front_end: bool,
    pub front_end_check: bool,
    /// `0x1a7a28`: the R&C1 saves' bonus bits (0x80 any found; 1, 2, 4, 8, 0x10 its five flags).
    pub rac1_bonus: u32,
    /// Set by `0x29c418` when a load finished (the engine starts the loaded game).
    pub loaded: bool,
    /// Set by status 25 when an auto-save failed (the freeze dialog `0x2b58f8(4, …)`).
    pub freeze_request: bool,
    len: usize,
    src: Option<Src>,
    /// The last `sceMcGetDir` listing (`0x1c5200`).
    dir_entries: Vec<(String, u64)>,
    pending: Option<i32>,
    read_buf: Vec<u8>,
    /// `PrepData` of the global section and of one level section (`0x1a2ba8` / `0x1a53a8`), made by `memcard_save`.
    pub buf_global: Vec<u8>,
    pub buf_level: Vec<u8>,
    /// The whole-save buffer state 0x13 writes (`0x139554`).
    pub whole: Vec<u8>,
}

impl MemCard {
    /// A card in `fs`, folder `dir`; the machine at rest.
    pub fn new(fs: CardFs, dir: String, blob: Option<SaveBlob>) -> MemCard {
        MemCard {
            fs,
            dir,
            blob,
            cards: [Card::default(), Card::default()],
            result: 0,
            cur: 0,
            level: 0,
            busy: false,
            fd: -1,
            save_card: -1,
            state: 0,
            sub: 0,
            req_state: -1,
            req_card: -1,
            error: 0,
            error_card: 0,
            checked: false,
            poll: 0,
            idle_frames: 0,
            status: status::INIT,
            flags: 0,
            status_frames: 0,
            error_seen: false,
            front_end: false,
            front_end_check: false,
            rac1_bonus: 0,
            loaded: false,
            freeze_request: false,
            len: 0,
            src: None,
            dir_entries: Vec::new(),
            pending: None,
            read_buf: Vec::new(),
            buf_global: Vec::new(),
            buf_level: Vec::new(),
            whole: Vec::new(),
        }
    }

    /// No card at all (deterministic runs, tests).
    pub fn absent() -> MemCard { MemCard::new(CardFs::new(None), "BASCUS-97268RATCHET".into(), None) }

    /// The machine is idle and nothing is queued (`state < 3 && req < 0`): the menus' gate, `memcard_Save`'s.
    pub fn idle(&self) -> bool { self.state < 3 && self.req_state < 0 }

    /// The card is busy (the busy icon: state > 2 or a request queued).
    pub fn in_use(&self) -> bool { 2 < self.state || 0 <= self.req_state }

    /// Requests `state` on `card` when nothing is queued.
    pub fn request(&mut self, state: i32, card: i32) {
        if self.req_state < 0 {
            self.req_state = state;
            self.req_card = card;
        }
    }

    fn call(&mut self, r: i32) { self.pending = Some(r); }
    fn path(&self, name: &str) -> String { format!("/{}/{name}", self.dir) }
    fn slot_file(&self, slot: i32) -> String { self.path(&save_file_name(slot.max(0) as usize)) }
    fn card(&mut self) -> &mut Card { &mut self.cards[self.cur] }

    /// Machine to rest: busy off, state 0, card 0 (`LAB_0029b8e0`).
    fn rest(&mut self) {
        self.busy = false;
        self.state = 0;
        self.cur = 0;
    }

    /// Error `code` on the current card, machine to rest.
    fn fail(&mut self, code: i32) {
        self.error_card = self.cur;
        self.error = code;
        self.rest();
    }

    /// A short read or write: close, then rest (`LAB_0029b4f0`; the close's result is never synced).
    fn fail_close(&mut self) {
        self.fs.close(self.fd);
        self.rest();
    }

    /// The libmc error of `r` (≤ −2) as the machine maps it, `codes` = (−5, −4, −3, −2, other).
    fn mc_error(r: i32, codes: (i32, i32, i32, i32, i32)) -> i32 {
        match r {
            -5 => codes.0,
            -4 => codes.1,
            -3 => codes.2,
            -2 => codes.3,
            _ => codes.4,
        }
    }

    fn sizes(save: &SaveData) -> (usize, usize) { (save.tables.global_size(), save.tables.level_size()) }

    /// One frame of `0x299cf0`. `save` receives the loads; its tables give the sizes.
    pub fn update(&mut self, save: &mut SaveData) {
        if self.busy {
            // sceMcSync: the native call finished when it was made.
            if let Some(r) = self.pending.take() { self.result = r; }
            self.busy = false;
            return;
        }
        self.busy = true;
        let r = self.result;
        match self.state {
            0 => {
                if self.poll > 1 { self.poll = 0; }
                let (res, ty, free, formatted) = if self.poll == 0 { self.fs.get_info() } else { (mc::NO_CARD, 0, 0, 0) };
                let c = &mut self.cards[self.poll];
                (c.ty, c.free, c.formatted) = (ty, free, formatted);
                self.call(res);
                self.state = 1;
            }
            1 => {
                if r != 0 {
                    let c = &mut self.cards[self.poll];
                    c.slot = -4;
                    c.info = r;
                    c.errors = -1;
                    c.used = 0;
                    c.previews = [Preview::default(); SAVE_SLOTS];
                    c.bonus = 0;
                }
                self.poll += 1;
                if self.req_card < self.poll as i32 && self.req_state >= 0 {
                    self.state = 2;
                    let rc = self.req_card;
                    if rc < 0 || (self.cards[rc as usize].info != 0 && self.req_state != 0x18) {
                        self.error = 0x271a;
                    } else {
                        self.cur = rc as usize;
                        self.state = self.req_state;
                        self.error = 0;
                    }
                    self.req_state = -1;
                    self.req_card = -1;
                    return;
                }
                self.state = if self.poll < 2 { 0 } else { 2 };
                self.busy = false;
            }
            2 => {
                self.idle_frames += 1;
                if self.req_state >= 0 || self.idle_frames > 10 {
                    self.state = 0;
                    self.idle_frames = 0;
                    self.poll = 0;
                }
                self.busy = false;
            }
            3 | 5 => {
                if self.card().formatted == 0 {
                    let res = self.fs.format();
                    self.call(res);
                    self.state += 1;
                    return;
                }
                self.busy = false;
                self.state = 2;
            }
            4 | 6 => {
                if r != 0 {
                    self.error_card = self.cur;
                    self.error = if self.state == 4 { 1 } else { 2 };
                }
                self.rest();
            }
            7 => {
                let pattern = self.path("*");
                let (res, entries) = self.fs.get_dir(&pattern);
                self.dir_entries = entries;
                self.call(res);
                self.sub = 0;
                self.state = 8;
            }
            8 => self.check_step(save),
            9 => {
                self.state = 10;
                self.sub = 0;
                self.card().slot = 0;
                self.create_step(save);
            }
            10 => self.create_step(save),
            0xd => {
                if self.card().slot < 0 {
                    self.busy = false;
                    self.state = 0;
                    self.cur = 0;
                    self.error = 0x13;
                    return;
                }
                self.level = 0;
                self.state = 0xe;
                self.sub = 0;
                self.read_step(save);
            }
            0xe | 0x17 => self.read_step(save),
            0xf => {
                let code = if self.card().errors != 0 {
                    0x2717
                } else if self.card().slot >= 0 {
                    self.sub = 0;
                    self.state = 0x10;
                    return self.save_step(save);
                } else {
                    0x1d
                };
                self.busy = false;
                self.state = 0;
                self.cur = 0;
                self.error = code;
            }
            0x10 => self.save_step(save),
            0x13 => {
                let code = if self.card().errors != 0 {
                    0x2718
                } else if self.card().slot >= 0 {
                    self.level = 0;
                    self.state = 0x14;
                    self.sub = 0;
                    return self.whole_step(save);
                } else {
                    0x27
                };
                self.busy = false;
                self.state = 0;
                self.cur = 0;
                self.error = code;
            }
            0x14 => self.whole_step(save),
            0x15 => {
                self.state = 0x16;
                self.card().preview_slot = -1;
                self.level = 0;
            }
            0x16 => {
                self.card().preview_slot += 1;
                if self.card().preview_slot < SAVE_SLOTS as i32 {
                    self.sub = 0;
                    self.state = 0x17;
                } else {
                    self.rest();
                }
            }
            0x18 => {
                self.state = 0x19;
                self.sub = 0;
                self.card().bonus = 1;
                self.rac1_step(save);
            }
            0x19..=0x1b => self.rac1_step(save),
            _ => {}
        }
    }

    /// State 8: the folder check (`sceMcGetDir` result `r` = the entries), then save0's header.
    fn check_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        let (gsz, lsz) = Self::sizes(save);
        match self.sub {
            0 => {
                if r < 1 {
                    self.card().slot = -2;
                    if r == -2 {
                        self.error_card = self.cur;
                        self.error = 3;
                    } else if r < 0 && r != mc::NOT_FOUND {
                        self.error_card = self.cur;
                        self.error = 4;
                    }
                    return self.rest();
                }
                let entries = std::mem::take(&mut self.dir_entries);
                let reserve_name = self.dir.clone();
                let (mut kb, mut icon, mut ico, mut reserve, mut slots) = (0i32, false, false, false, [false; SAVE_SLOTS]);
                for (name, size) in entries.iter().take(r as usize) {
                    kb += ((size + 0x3ff) >> 10) as i32;
                    icon |= name == "icon.sys";
                    ico |= name == "static.ico";
                    reserve |= *name == reserve_name;
                    if let Some(d) = name.strip_prefix("save").filter(|n| n.len() == 5 && n.ends_with(".bin")).and_then(|n| n[..1].parse::<usize>().ok()) {
                        if d < SAVE_SLOTS { slots[d] = true; }
                    }
                }
                kb += (r + 2) / 2;
                if ((kb - 0x1da) as u32) < 2 && r == DIR_ENTRIES && icon && ico && reserve && slots.iter().all(|&s| s) {
                    self.card().used = 0;
                    self.card().slot = -1;
                    self.busy = false;
                    self.sub = 1;
                    return;
                }
                self.card().used = kb;
                self.card().slot = -2;
                self.rest();
            }
            1 => {
                let p = self.slot_file(0);
                let res = self.fs.open(&p, open_mode::READ);
                self.call(res);
                self.sub = 2;
            }
            2 => {
                if r >= 0 {
                    self.fd = r;
                    self.sub = 3;
                    self.busy = false;
                    return;
                }
                self.card().slot = -2;
                let code = if r == -7 { 10000 } else { Self::mc_error(r, (0x2711, 0x2712, 0x2713, 3, 4)) };
                self.fail(code);
            }
            3 => {
                self.len = 8;
                let (res, b) = self.fs.read(self.fd, 8);
                self.read_buf = b;
                self.call(res);
                self.sub = 4;
            }
            4 => {
                if r == self.len as i32 {
                    let h = header(&self.read_buf);
                    let c = self.card();
                    c.header = h;
                    c.errors = (h[0] as usize != gsz) as i32 + (h[1] as usize != lsz) as i32;
                    self.busy = false;
                    self.sub = 5;
                    return;
                }
                self.card().slot = -2;
                self.error_card = self.cur;
                if r >= 0 {
                    self.error = 0x2716;
                    return self.fail_close();
                }
                self.error = Self::mc_error(r, (0x2711, 0x2714, 0x2715, 3, 4));
                self.rest();
            }
            5 => {
                let res = self.fs.close(self.fd);
                self.call(res);
                self.busy = false;
                self.state = 0x15;
            }
            _ => self.rest(),
        }
    }

    /// State 10: create the folder and its files.
    fn create_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        let (gsz, lsz) = Self::sizes(save);
        match self.sub {
            0 => {
                let c = self.card();
                if c.free + c.used < FOLDER_KB {
                    self.error_card = self.cur;
                    self.error = 7;
                    return self.rest();
                }
                self.sub = 1;
                self.busy = false;
            }
            1 => {
                let d = format!("/{}", self.dir);
                let res = self.fs.mkdir(&d);
                self.call(res);
                self.sub = 2;
            }
            2 => {
                if r == 0 || r == mc::NOT_FOUND {
                    self.sub = 3;
                    return;
                }
                let code = match r {
                    -3 => 7,
                    -2 => 6,
                    _ => 0xd,
                };
                self.fail(code);
            }
            // 3 / 4: the save blob read from the disc (here: already in memory, the next frame).
            3 => {
                self.sub = 4;
                self.busy = false;
            }
            4 => {
                self.sub = 5;
                self.busy = false;
            }
            5 | 9 | 0xd | 0x12 => {
                let name = match self.sub {
                    5 => "icon.sys".to_string(),
                    9 => "static.ico".to_string(),
                    0xd => save_file_name(self.card().slot.max(0) as usize),
                    _ => self.dir.clone(),
                };
                let p = self.path(&name);
                let res = self.fs.open(&p, open_mode::CREATE_RDWR);
                self.call(res);
                self.fd = -1;
                self.sub += 1;
            }
            6 | 10 | 0xe | 0x13 => {
                if r < 0 {
                    self.error_card = self.cur;
                    self.error = 10;
                    return self.rest();
                }
                if self.fd < 0 { self.fd = r; }
                let src = match self.sub {
                    6 => Src::IconSys,
                    10 => Src::StaticIco,
                    0xe => Src::Template,
                    _ => Src::Reserve,
                };
                let bytes = self.src_bytes(src, gsz + LEVEL_SLOTS * lsz + 8);
                self.len = bytes.len();
                self.src = Some(src);
                let res = self.fs.write(self.fd, &bytes);
                self.call(res);
                self.sub += 1;
            }
            7 | 0xb | 0xf | 0x14 => {
                if r == self.len as i32 {
                    let res = self.fs.close(self.fd);
                    self.call(res);
                    self.sub += 1;
                    return;
                }
                self.error_card = self.cur;
                self.error = 0xb;
                if r >= 0 { return self.fail_close(); }
                self.error = match r {
                    -4 => 8,
                    -3 => 7,
                    -2 => 6,
                    _ => 0xd,
                };
                self.rest();
            }
            8 | 0xc | 0x10 => {
                if r == 0 {
                    self.sub += 1;
                } else {
                    self.error = 0xc;
                    self.error_card = self.cur;
                    self.cur = 0;
                    self.state = 0;
                }
                self.busy = false;
            }
            0x11 => {
                self.card().slot += 1;
                self.sub = if self.card().slot > 3 { 0x12 } else { 0xd };
                self.busy = false;
            }
            0x15 => {
                let c = self.card();
                c.header = [gsz as u32, lsz as u32];
                c.slot = -1;
                c.errors = 0;
                self.cur = 0;
                self.state = 0;
            }
            _ => self.rest(),
        }
    }

    /// The bytes a create step writes (the blank save's size is the file's).
    fn src_bytes(&self, src: Src, file: usize) -> Vec<u8> {
        let b = self.blob.as_ref();
        match src {
            Src::IconSys => b.map(|b| b.icon_sys.clone()).unwrap_or_default(),
            Src::StaticIco => b.map(|b| b.static_ico.clone()).unwrap_or_default(),
            Src::Template => b.map(|b| b.template.clone()).unwrap_or_else(|| vec![0; file]),
            // RAM from 0x1a79f8: not reproducible natively, and nothing reads it back [L].
            Src::Reserve => vec![0; RESERVE],
        }
    }

    /// States 0xe (load) and 0x17 (a slot's preview).
    fn read_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        let preview = self.state == 0x17;
        match self.sub {
            0 => {
                let slot = if preview { self.card().preview_slot } else { self.card().slot };
                let p = self.slot_file(slot);
                let res = self.fs.open(&p, open_mode::READ);
                self.call(res);
                self.sub = 1;
            }
            1 => {
                if r >= 0 {
                    self.fd = r;
                    self.sub = 2;
                    self.busy = false;
                    return;
                }
                let code = if r == -7 { 0x14 } else { Self::mc_error(r, (0x15, 0x16, 0x17, 0x18, 0x1c)) };
                self.fail(code);
            }
            2 | 4 | 6 => {
                self.len = match self.sub {
                    2 => 8,
                    4 => self.card().header[0] as usize,
                    _ => self.card().header[1] as usize,
                };
                let limit = match self.sub {
                    4 => rc_formats::disc::save_game::GLOBAL_MAX,
                    6 => rc_formats::disc::save_game::LEVEL_MAX,
                    _ => 8,
                };
                // The game traps above its buffers; here the read fails as a short read.
                let n = if self.len > limit { 0 } else { self.len };
                let (res, b) = self.fs.read(self.fd, n);
                self.read_buf = b;
                self.call(if self.len > limit { -1 } else { res });
                self.sub += 1;
            }
            3 | 5 | 7 => {
                if r != self.len as i32 {
                    self.error_card = self.cur;
                    self.error = 0x1b;
                    if r >= 0 { return self.fail_close(); }
                    self.error = Self::mc_error(r, (0x15, 0x19, 0x1a, 0x18, 0x1c));
                    return self.rest();
                }
                let max = save.max();
                match self.sub {
                    3 => {
                        let h = header(&self.read_buf);
                        self.card().header = h;
                        self.busy = false;
                        self.sub = 4;
                    }
                    5 if preview => {
                        let i = self.card().preview_slot.max(0) as usize;
                        let p = Preview::from_section(&self.read_buf, max);
                        if let Some(slot) = self.card().previews.get_mut(i) { *slot = p; }
                        self.sub = 8;
                    }
                    5 => {
                        self.card().errors = restore(save, Scope::Global, &self.read_buf);
                        self.sub = 6;
                    }
                    _ => {
                        let e = restore(save, Scope::Level(self.level), &self.read_buf);
                        self.card().errors += e as i32;
                        self.level += 1;
                        self.sub = if self.level < LEVEL_SLOTS { 6 } else { 8 };
                    }
                }
                if self.sub != 4 { self.busy = false; }
            }
            8 => {
                let res = self.fs.close(self.fd);
                self.call(res);
                if preview {
                    self.busy = false;
                    self.state = 0x16;
                    return;
                }
                // 0x29c418: the loaded game's fresh-start values.
                self.loaded = true;
                self.cur = 0;
                self.state = 0;
            }
            _ => self.rest(),
        }
    }

    /// State 0x10: the incremental save (the global section and the save's level section).
    fn save_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        let (gsz, lsz) = Self::sizes(save);
        match self.sub {
            0 => {
                let slot = self.card().slot;
                let p = self.slot_file(slot);
                let res = self.fs.open(&p, open_mode::WRITE);
                self.call(res);
                self.sub = 1;
            }
            1 => {
                if r >= 0 {
                    self.fd = r;
                    self.sub = 2;
                    self.busy = false;
                    return;
                }
                let code = if r == -7 { 0x1e } else { Self::mc_error(r, (0x1f, 0x20, 0x21, 0x22, 0x26)) };
                self.fail(code);
            }
            2 => {
                let res = self.fs.seek(self.fd, 8, 0);
                self.call(res);
                self.sub = 3;
            }
            3 => {
                if r >= 0 {
                    self.sub = 4;
                    return;
                }
                let res = self.fs.close(self.fd);
                self.call(res);
                self.cur = 0;
                self.busy = false;
                self.state = 0;
                self.error = 0x26;
            }
            4 => {
                let b = self.buf_global.clone();
                self.len = gsz;
                let res = self.fs.write(self.fd, &b);
                self.call(res);
                self.sub = 5;
            }
            5 | 9 => {
                if r == self.len as i32 {
                    self.sub += 1;
                    return;
                }
                self.error_card = self.cur;
                if r >= 0 {
                    self.error = 0x25;
                    return self.fail_close();
                }
                let other = if self.sub == 5 { (0x22, 0x26) } else { (0x18, 0x1c) };
                self.error = Self::mc_error(r, (0x1f, 0x23, 0x24, other.0, other.1));
                self.rest();
            }
            6 => {
                let h = self.cards[0].header;
                let at = h[0] as i64 + self.level as i64 * h[1] as i64 + 8;
                self.len = at as usize;
                let res = self.fs.seek(self.fd, at, 0);
                self.call(res);
                self.sub = 7;
            }
            7 => {
                if r == self.len as i32 {
                    self.sub = 8;
                    return;
                }
                self.fail_close();
            }
            8 => {
                let b = self.buf_level.clone();
                self.len = lsz;
                let res = self.fs.write(self.fd, &b);
                self.call(res);
                self.sub = 9;
            }
            10 => {
                let res = self.fs.close(self.fd);
                self.call(res);
                self.cur = 0;
                self.state = 0;
            }
            _ => self.rest(),
        }
    }

    /// State 0x14: the whole save of the prepared buffer.
    fn whole_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        let (gsz, lsz) = Self::sizes(save);
        match self.sub {
            0 => {
                let slot = self.card().slot;
                let p = self.slot_file(slot);
                let res = self.fs.open(&p, open_mode::WRITE);
                self.call(res);
                self.sub = 1;
            }
            1 => {
                if r < 0 {
                    self.error_card = self.cur;
                    self.error = 0x2b;
                    return self.rest();
                }
                self.fd = r;
                self.len = gsz + LEVEL_SLOTS * lsz + 8;
                let mut b = self.whole.clone();
                b.resize(self.len, 0);
                let res = self.fs.write(self.fd, &b);
                self.call(res);
                self.sub = 2;
            }
            2 => {
                if r == self.len as i32 {
                    let res = self.fs.close(self.fd);
                    self.call(res);
                    self.sub = 3;
                    return;
                }
                self.error_card = self.cur;
                self.error = 0xb;
                if r >= 0 { return self.fail_close(); }
                self.error = match r {
                    -4 => 0x28,
                    -3 => 0x29,
                    -2 => 0x2a,
                    _ => 0x2d,
                };
                self.rest();
            }
            3 => {
                if r != 0 {
                    self.error_card = self.cur;
                    self.error = 0x2c;
                }
                self.rest();
            }
            _ => self.rest(),
        }
    }

    /// States 0x19..0x1b: R&C1's saves (substate = region · 5 + slot).
    fn rac1_step(&mut self, save: &mut SaveData) {
        let r = self.result;
        match self.state {
            0x19 => {
                let region = (self.sub / 5) as usize;
                let Some(dir) = RAC1_DIRS.get(region) else { return self.rest() };
                let p = format!("{dir}/save{}.bin", self.sub % 5);
                let res = self.fs.open(&p, open_mode::READ);
                self.call(res);
                self.state = 0x1a;
            }
            0x1a => {
                if r < 0 {
                    self.state = 0x19;
                    self.busy = false;
                    self.sub = (self.sub / 5 + 1) * 5;
                    return;
                }
                self.fd = r;
                let (res, b) = self.fs.read(self.fd, RAC1_READ);
                self.read_buf = b;
                self.call(res);
                self.state = 0x1b;
            }
            _ => {
                if r < RAC1_READ as i32 {
                    self.sub = (self.sub / 5 + 1) * 5;
                } else {
                    let res = self.fs.close(self.fd);
                    self.call(res);
                    self.rac1_bonus_of(save.max());
                    self.sub += 1;
                }
                self.state = 0x19;
                self.busy = false;
            }
        }
    }

    /// `0x299cf0` state 0x1b: an R&C1 save's global section (at +8) passes its checksum → chunk 10's bytes 0xa, 0x12, 0x19,
    /// 0xd and 0x13 set bits 1, 2, 4, 8 and 0x10 of `0x1a7a28` (with 0x80), each counted on the card.
    fn rac1_bonus_of(&mut self, max: usize) {
        let Some(sec) = self.read_buf.get(8..).and_then(|b| Section::decode(b, max).ok()) else { return };
        if !sec.crc_ok { return; }
        let Some(c) = sec.chunk(10) else { return };
        let mut bits = self.rac1_bonus | 0x80;
        for (at, bit) in [(0xa, 1), (0x12, 2), (0x19, 4), (0xd, 8), (0x13, 0x10)] {
            if c.data.get(at).is_some_and(|&b| b != 0) {
                bits |= bit;
                self.card().bonus += 1;
            }
        }
        self.rac1_bonus = bits;
    }

    // ---------------------------------------------------------------------------------------------------------------
    // The monitor.

    /// One frame of `0x298f20`: the error mark, the failed save / load flags, the status' handler, its frame count.
    pub fn monitor(&mut self) {
        let before = self.status;
        let c0 = &self.cards[0];
        if self.error != 0 || c0.info != 0 || c0.errors > 0 { self.error_seen = true; }
        if self.flags & 0x80 != 0 {
            self.status = status::SAVE_FAILED;
            self.flags = (self.flags & !0x80) | 0x40;
        }
        if self.flags & 0x100 != 0 {
            self.status = status::LOAD_FAILED;
            self.flags = (self.flags & !0x100) | 0x40;
        }
        self.monitor_step();
        self.status_frames += 1;
        if self.status != before { self.status_frames = 0; }
    }

    fn monitor_step(&mut self) {
        use status::*;
        let info = self.cards[0].info;
        let at_rest = self.state == 2 && self.req_state < 0;
        match self.status {
            INIT => {
                self.status = RECHECK;
                self.cards[0].info = 0;
                self.checked = false;
            }
            READY => {
                let f = self.flags & !6;
                self.flags = f;
                if !self.checked {
                    self.status = RECHECK;
                } else if f & 0x80 != 0 || self.error != 0 {
                    self.status = SAVE_FAILED;
                    self.flags = (f & !0x86) | 0x40;
                } else if f & 0x100 != 0 {
                    self.status = LOAD_FAILED;
                    self.flags = (f ^ 0x100) | 0x40;
                } else if info == 0 {
                    if f & 0x200 != 0 {
                        self.status = AUTOSAVE;
                        self.flags = f ^ 0x200;
                    }
                } else {
                    self.checked = false;
                    if self.front_end || self.front_end_check {
                        self.status = RECHECK;
                    } else {
                        self.status = CHANGED;
                        self.flags = f | 1;
                    }
                }
            }
            CHANGED => {
                if self.flags & 1 == 0 { self.status = RECHECK; }
            }
            RECHECK => {
                self.req_card = -1;
                self.req_state = -1;
                self.status = POLL;
            }
            POLL => {
                self.flags &= !0x20;
                if self.cards[0].ty == TYPE_PS2 {
                    match info {
                        0 => self.status = PRESENT,
                        -1 => {
                            self.cards[0].info = 0;
                            self.status = PRESENT;
                        }
                        mc::NO_FORMAT => self.status = UNFORMATTED,
                        _ => {}
                    }
                }
            }
            UNFORMATTED => {
                if info != mc::NO_FORMAT {
                    self.status = RECHECK;
                } else if self.front_end_check || self.flags & 2 != 0 {
                    self.status = FORMAT_ASK;
                }
            }
            FORMAT_ASK => {
                if info != mc::NO_FORMAT {
                    self.status = RECHECK;
                } else if self.flags & 0x20 != 0 {
                    self.flags ^= 0x20;
                    self.status = if self.front_end { UNFORMATTED_FE } else { UNFORMATTED };
                } else if self.flags & 8 != 0 {
                    self.status = FORMAT;
                    self.flags ^= 8;
                }
            }
            FORMAT => {
                self.cards[0].info = 0;
                self.request(3, 0);
                self.status = FORMATTING;
            }
            FORMATTING => {
                if at_rest {
                    if self.error != 0 {
                        self.status = FORMAT_FAILED;
                        self.flags |= 0x40;
                    } else {
                        self.status = CREATE;
                    }
                }
            }
            PRESENT => {
                if info != 0 {
                    self.status = RECHECK;
                } else if self.flags & 6 != 0 {
                    self.status = CHECK;
                } else if self.flags & 0x200 != 0 {
                    self.status = AUTOSAVE;
                }
            }
            CHECK => {
                if at_rest {
                    self.req_state = 7;
                    self.req_card = 0;
                    self.status = CHECKING;
                }
            }
            CHECKING => {
                if !at_rest { return; }
                if self.error == 0 {
                    if info < -1 {
                        self.status = RECHECK;
                        return;
                    }
                    let c = &self.cards[0];
                    if c.slot != -2 {
                        if c.slot >= -1 { self.status = FOUND; }
                        return;
                    }
                    if c.free + c.used < FOLDER_KB {
                        self.status = NO_SPACE;
                        return;
                    }
                }
                self.status = NO_DATA;
            }
            NO_DATA => {
                if info != 0 {
                    self.status = RECHECK;
                } else if self.flags & 2 != 0 {
                    self.status = CREATE_ASK;
                }
            }
            CREATE_ASK => {
                if info != 0 {
                    self.status = RECHECK;
                } else if self.flags & 0x20 != 0 {
                    self.flags ^= 0x20;
                    self.status = if self.front_end { NO_DATA_FE } else { NO_DATA };
                } else if self.flags & 0x10 != 0 {
                    self.status = CREATE;
                    self.flags ^= 0x10;
                }
            }
            CREATE => {
                if at_rest {
                    self.req_state = 9;
                    self.req_card = 0;
                    self.status = CREATING;
                }
            }
            CREATING => {
                if at_rest {
                    if self.error != 0 {
                        self.status = CREATE_FAILED;
                        self.flags |= 0x40;
                        return;
                    }
                    self.req_state = 7;
                    self.req_card = 0;
                    self.status = CREATED;
                    self.save_card = 0;
                    self.cards[0].slot = 0;
                }
            }
            FOUND => {
                self.flags &= !6;
                if self.flags & 0x80 != 0 {
                    self.status = SAVE_FAILED;
                    self.flags = (self.flags ^ 0x80) | 0x40;
                } else if self.flags & 0x100 != 0 {
                    self.status = LOAD_FAILED;
                    self.flags = (self.flags ^ 0x100) | 0x40;
                } else if info != 0 {
                    self.status = RECHECK;
                } else if self.checked {
                    self.status = READY;
                }
            }
            FORMAT_FAILED | CREATE_FAILED | LOAD_FAILED | SAVE_FAILED => {
                if self.flags & 0x40 == 0 { self.status = RECHECK; }
            }
            NO_SPACE => {
                if info != 0 { self.status = RECHECK; }
            }
            CREATED => {
                if at_rest {
                    if self.error != 0 || self.cards[0].errors != 0 {
                        self.checked = false;
                        self.status = SAVE_FAILED;
                        self.flags |= 0x440;
                    } else {
                        self.checked = true;
                        self.status = READY;
                    }
                }
            }
            UNFORMATTED_FE => {
                if info != mc::NO_FORMAT {
                    self.status = RECHECK;
                } else if self.flags & 0x20 != 0 {
                    self.status = UNFORMATTED;
                    self.flags ^= 0x20;
                }
            }
            NO_DATA_FE => {
                if info != 0 {
                    self.status = RECHECK;
                } else if self.flags & 0x20 != 0 {
                    self.status = NO_DATA;
                    self.flags ^= 0x20;
                }
            }
            AUTOSAVE => {
                if at_rest {
                    if self.error == 0 && info == 0 {
                        self.checked = true;
                        self.status = READY;
                    } else {
                        self.checked = false;
                        if self.error == 0 {
                            self.flags = (self.flags & !0x40) | 1;
                        } else {
                            // The freeze dialog 0x2b58f8(4, …): the caller sees `freeze_request`.
                            self.freeze_request = true;
                            self.flags = 0x40;
                        }
                        self.status = CHANGED;
                    }
                }
            }
            _ => {}
        }
    }

    /// The status allows the save and load pages without a dialog (`0x1a7420 ∈ {1, 0x10}`).
    pub fn ready(&self) -> bool { matches!(self.status, status::READY | status::FOUND) }

    // ---------------------------------------------------------------------------------------------------------------
    // The saves.

    /// `memcard_Save(force, level)` `0x29c1d0`, after the clock stamp (`clock`) and the game state stored into `save`.
    /// No card or slot: returns `!force`. Else: checked |= force; an auto-save (force false) raises flag 0x200; when
    /// idle, the active slot's preview, `PrepData` of the global section and of the current level's section (with
    /// `level` ≥ 0 standing in as the level, its visited byte set, for the preparation only), and state 0xf requested.
    /// Returns whether the incremental save is queued.
    pub fn memcard_save(&mut self, save: &mut SaveData, force: bool, level: i32, clock: [u8; 8]) -> bool {
        set_clock(save, clock);
        let card_ok = usize::try_from(self.save_card).ok().is_some_and(|c| c < PORTS && self.cards[c].slot >= 0);
        if !card_ok { return !force; }
        self.checked |= force;
        if self.checked {
            if !force { self.flags |= 0x200; }
            if self.idle() {
                let cur = save.i32(Scope::Global, 0x1a79f8);
                self.level = cur.max(0) as usize;
                let mut put_back = None;
                if level >= 0 {
                    let was = visited(save, level as usize);
                    save.set_i32(Scope::Global, 0x1a79f8, level);
                    if was == 0 { set_visited(save, level as usize, 1); }
                    put_back = Some(was);
                }
                let slot = self.cards[0].slot.max(0) as usize;
                let p = Preview {
                    level: save.i32(Scope::Global, 0x1a79f8),
                    bolts: save.i32(Scope::Global, 0x1a7a00),
                    byte_12: save.u8(Scope::Global, 0x1a7a12) as i32,
                    word_bc8: save.i32(Scope::Global, 0x1a7bc8),
                    clock,
                    bad: false,
                };
                if let Some(s) = self.cards[0].previews.get_mut(slot) { *s = p; }
                self.buf_global = save.prep(Scope::Global);
                self.buf_level = save.prep(Scope::Level(self.level));
                if let Some(was) = put_back {
                    set_visited(save, level as usize, was);
                    save.set_i32(Scope::Global, 0x1a79f8, cur);
                }
                if self.req_state < 0 {
                    self.req_state = 0xf;
                    self.req_card = self.save_card;
                }
            }
        }
        self.req_state == 0xf
    }

    /// A whole save of `save` into `slot` (state 0x13: the new game's and the save page's).
    pub fn save_whole(&mut self, save: &SaveData, slot: i32) {
        self.whole = save.to_file().to_bytes(save.max());
        self.checked = true;
        self.cards[0].slot = slot;
        self.save_card = 0;
        self.request(0x13, 0);
    }

    /// The load of `slot` (state 0xd).
    pub fn load(&mut self, slot: i32) {
        self.save_card = 0;
        self.cards[0].slot = slot;
        self.request(0xd, 0);
    }
}

/// `RestoreData` of one section read from the card (a section that does not decode: one error, nothing copied).
fn restore(save: &mut SaveData, scope: Scope, bytes: &[u8]) -> i32 {
    match Section::decode(bytes, save.max()) {
        Ok(s) => save.restore(scope, &s) as i32,
        Err(_) => 1,
    }
}

fn header(b: &[u8]) -> [u32; 2] {
    let w = |o: usize| b.get(o..o + 4).map_or(0, |x| u32::from_le_bytes(x.try_into().unwrap()));
    [w(0), w(4)]
}

/// `sceCdCLOCK` (BCD) of a civil date and time: [0] stat 0, second, minute, hour, pad 0, day, month, year % 100.
pub fn bcd_clock_civil(year: u64, month: u64, day: u64, hour: u64, minute: u64, second: u64) -> [u8; 8] {
    let bcd = |v: u64| (((v / 10) % 10) << 4 | (v % 10)) as u8;
    [0, bcd(second), bcd(minute), bcd(hour), 0, bcd(day), bcd(month), bcd(year % 100)]
}

/// The card's save root: `RC_SAVE_DIR=<path>` (`0` or empty: no card), else `<config base>/rerac/memcard`, the card
/// R&C1's port uses (module docs); None = no card.
pub fn save_root(base: Option<&Path>) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("RC_SAVE_DIR") {
        return (!v.is_empty() && v != "0").then(|| PathBuf::from(v));
    }
    base.map(|b| b.join("rerac").join("memcard"))
}
