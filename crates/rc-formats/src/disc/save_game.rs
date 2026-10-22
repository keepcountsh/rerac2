//! The persistent game state's on-card format: the chunk descriptor tables, the `save%d.bin` file (header, global
//! section, 28 level sections), its CRC-16 and the disc's save blob (`MISC.WAD` part 2: the icon files and the blank
//! save). R&C1's format with Going Commando's tables (rerac:rc_formats::save_game, docs/plan/game_state.md §1–§4 there);
//! the boot's card machine is `0x299cf0` (crate-level docs of rc_game::memcard).
//!
//! Nothing here is copied from the disc: the descriptor tables are read from the boot ELF
//! ([`ChunkTables::from_boot_elf`]), the template and icons from the disc blob ([`SaveBlob::parse`]).
//!
//! * **Descriptor** (boot `0x2560b8` global, `0x2562e8` per level; 16 bytes): `{u32 addr, u32 size, s32 id,
//!   s32 restore_status}`, ended by `addr == 0`. A per-level chunk of level slot `L` lives at `addr + L·size`.
//! * **Size** (`0x29bc68`): `8 + Σ align4(size + 8) + 8`.
//! * **Section**: `{u32 data_size, u32 crc16(data)}` then `data` = chunks `{s32 id, u32 size, bytes[size], pad to 4}`
//!   and the terminator `{s32 −1, u32 0}`.
//! * **Checksum** (`0x29bca0`): R&C1's CRC-16 (seed `0xedb88320`, polynomial `0x1f45`, MSB first); 0 for data longer
//!   than the global section's size. `0x29bd48`: a section passes when its stored CRC is non-zero and equal.
//! * **File**: `{u32 global_size, u32 level_size}` (compared with the tables' sizes: an out-of-date save), the global
//!   section at 8, level section `L` at `8 + global_size + L·level_size`.

use crate::buf::{invalid, Buf, Result};
use crate::disc::boot::BootElf;

/// The global chunk descriptor table (`0x2560b8`: the card machine's `GetDataSize` / `RestoreData` argument).
pub const GLOBAL_CHUNKS: u32 = 0x0025_60b8;
/// The per-level chunk descriptor table (`0x2562e8`).
pub const LEVEL_CHUNKS: u32 = 0x0025_62e8;
/// Level sections in a save (the load reads 0x1c).
pub const LEVEL_SLOTS: usize = 28;
/// Save slots on a card (`save0.bin` .. `save3.bin`: the create loop and the directory check stop at 4).
pub const SAVE_SLOTS: usize = 4;
/// Chunk id of the section terminator.
pub const END_ID: i32 = -1;
/// Descriptor stride.
pub const DESC_SIZE: usize = 16;
/// The load's buffers (`0x1a2ba8` global, 0x2800 bytes; `0x1a53a8` level, 0x1000): a larger section traps.
pub const GLOBAL_MAX: usize = 0x2800;
pub const LEVEL_MAX: usize = 0x1000;
/// `icon.sys`'s size (the card machine writes 0x3c4 bytes from the blob).
pub const ICON_SYS_SIZE: usize = 0x3c4;

/// One `{addr, size, id}` descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkDesc {
    /// EE address of the data (level slot 0 for per-level chunks).
    pub addr: u32,
    pub size: u32,
    pub id: i32,
}

impl ChunkDesc {
    /// EE address of this chunk for level slot `slot` (global chunks: slot 0).
    pub fn addr_for(&self, slot: usize) -> u32 { self.addr + slot as u32 * self.size }
}

/// Reads a descriptor table at `vaddr` of the boot ELF, up to its `addr == 0` terminator.
pub fn read_chunk_table(elf: &BootElf, vaddr: u32) -> Result<Vec<ChunkDesc>> {
    let mut out = Vec::new();
    for i in 0..256u32 {
        let e = Buf(elf.read(vaddr + i * DESC_SIZE as u32, DESC_SIZE)?);
        let (addr, size, id) = (e.u32(0)?, e.u32(4)?, e.i32(8)?);
        if addr == 0 { return Ok(out); }
        if id == END_ID { return invalid(format!("chunk table {vaddr:#x}: entry {i} uses the terminator id")); }
        out.push(ChunkDesc { addr, size, id });
    }
    invalid(format!("chunk table {vaddr:#x}: no terminator in 256 entries"))
}

/// `0x29bc68`: the section's byte size including its header.
pub fn section_size(descs: &[ChunkDesc]) -> usize { 8 + descs.iter().map(|d| align4(d.size as usize + 8)).sum::<usize>() + 8 }

fn align4(n: usize) -> usize { (n + 3) & !3 }

/// Both descriptor tables, in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkTables {
    pub global: Vec<ChunkDesc>,
    pub level: Vec<ChunkDesc>,
}

impl ChunkTables {
    pub fn from_boot_elf(elf: &BootElf) -> Result<ChunkTables> {
        Ok(ChunkTables { global: read_chunk_table(elf, GLOBAL_CHUNKS)?, level: read_chunk_table(elf, LEVEL_CHUNKS)? })
    }
    pub fn global_size(&self) -> usize { section_size(&self.global) }
    pub fn level_size(&self) -> usize { section_size(&self.level) }
    /// Size of a `save%d.bin` (the create step's `global + 0x1c·level + 8`).
    pub fn file_size(&self) -> usize { 8 + self.global_size() + LEVEL_SLOTS * self.level_size() }
}

/// `0x29bca0`: CRC-16, MSB first, register seeded with 0xedb88320 (only the low 16 bits reach the result), polynomial
/// 0x1f45, no final xor. 0 for data longer than `max` (the global section's size).
pub fn crc16(data: &[u8], max: usize) -> u16 {
    if data.len() > max { return 0; }
    let mut r: u32 = 0xedb8_8320;
    for &b in data {
        r ^= (b as u32) << 8;
        for _ in 0..8 { r = if r & 0x8000 != 0 { (r << 1) ^ 0x1f45 } else { r << 1 }; }
    }
    (r & 0xffff) as u16
}

/// One tagged chunk of a section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: i32,
    pub data: Vec<u8>,
    /// The `align4(len) − len` bytes after `data` (whatever the buffer held; zero for fresh chunks).
    pub pad: Vec<u8>,
}

impl Chunk {
    pub fn new(id: i32, data: Vec<u8>) -> Chunk {
        let pad = vec![0; align4(data.len()) - data.len()];
        Chunk { id, data, pad }
    }
}

/// A decoded or to-be-encoded section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub chunks: Vec<Chunk>,
    /// Decode: the checksum passed (`0x29bd48`). `RestoreData` copies nothing from a section that fails.
    pub crc_ok: bool,
}

impl Section {
    pub fn new(chunks: Vec<Chunk>) -> Section { Section { chunks, crc_ok: true } }

    pub fn chunk(&self, id: i32) -> Option<&Chunk> { self.chunks.iter().find(|c| c.id == id) }

    /// `{data_size, crc16}` + chunks + `{−1, 0}`; `max` = the checksum's limit (the global section's size).
    pub fn encode(&self, max: usize) -> Vec<u8> {
        let mut v = vec![0; 8];
        for c in &self.chunks {
            v.extend_from_slice(&c.id.to_le_bytes());
            v.extend_from_slice(&(c.data.len() as u32).to_le_bytes());
            v.extend_from_slice(&c.data);
            let n = align4(c.data.len()) - c.data.len();
            v.extend((0..n).map(|k| c.pad.get(k).copied().unwrap_or(0)));
        }
        v.extend_from_slice(&END_ID.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        let size = (v.len() - 8) as u32;
        let crc = crc16(&v[8..], max) as u32;
        v[0..4].copy_from_slice(&size.to_le_bytes());
        v[4..8].copy_from_slice(&crc.to_le_bytes());
        v
    }

    /// Decodes a section at the start of `bytes` (walks the chunks to the terminator, bounds-checked).
    pub fn decode(bytes: &[u8], max: usize) -> Result<Section> {
        let b = Buf(bytes);
        let data_size = b.u32(0)? as usize;
        let stored = b.u32(4)?;
        let data = b.sub(8, data_size, "section data")?;
        let crc_ok = stored != 0 && crc16(data.bytes(), max) as u32 == stored;
        let mut chunks = Vec::new();
        let mut p = 0usize;
        loop {
            let id = data.i32(p)?;
            let size = data.u32(p + 4)? as usize;
            if id == END_ID { break; }
            let body = data.sub(p + 8, align4(size), "chunk data")?.bytes();
            chunks.push(Chunk { id, data: body[..size].to_vec(), pad: body[size..].to_vec() });
            p += 8 + align4(size);
        }
        Ok(Section { chunks, crc_ok })
    }
}

/// A whole `save%d.bin`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveFile {
    pub global: Section,
    pub levels: Vec<Section>,
}

impl SaveFile {
    /// Parses a card file (or the disc template); `max` = the checksum's limit ([`ChunkTables::global_size`]).
    pub fn parse(bytes: &[u8], max: usize) -> Result<SaveFile> {
        let b = Buf(bytes);
        let (gs, ls) = (b.u32(0)? as usize, b.u32(4)? as usize);
        if !(16..=GLOBAL_MAX).contains(&gs) || !(16..=LEVEL_MAX).contains(&ls) {
            return invalid(format!("save file: implausible section sizes {gs:#x}/{ls:#x}"));
        }
        b.check(0, 8 + gs + LEVEL_SLOTS * ls, "save file")?;
        let global = Section::decode(&bytes[8..8 + gs], max)?;
        let levels = (0..LEVEL_SLOTS).map(|l| {
            let at = 8 + gs + l * ls;
            Section::decode(&bytes[at..at + ls], max)
        }).collect::<Result<Vec<_>>>()?;
        Ok(SaveFile { global, levels })
    }

    /// The header then every section.
    pub fn to_bytes(&self, max: usize) -> Vec<u8> {
        let g = self.global.encode(max);
        let lv: Vec<Vec<u8>> = self.levels.iter().map(|s| s.encode(max)).collect();
        let mut v = Vec::with_capacity(8 + g.len() + lv.iter().map(Vec::len).sum::<usize>());
        v.extend_from_slice(&(g.len() as u32).to_le_bytes());
        v.extend_from_slice(&(lv.first().map_or(0, Vec::len) as u32).to_le_bytes());
        v.extend_from_slice(&g);
        for l in &lv { v.extend_from_slice(l); }
        v
    }

    pub fn all_crcs_ok(&self) -> bool { self.global.crc_ok && self.levels.iter().all(|s| s.crc_ok) }

    /// The incremental save (card state 0x10): into the existing `card_file` (no create, no truncate), the global
    /// section at 8 and level section `level` at `8 + global + level·level_size`; the header and the other level
    /// sections keep what the card held.
    pub fn write_incremental(&self, level: usize, card_file: &mut [u8], max: usize) -> Result<()> {
        if level >= self.levels.len() { return invalid(format!("level slot {level} out of range")); }
        let g = self.global.encode(max);
        let l = self.levels[level].encode(max);
        let at = 8 + g.len() + level * l.len();
        if card_file.len() < at + l.len() { return invalid("incremental save: card file shorter than a whole save"); }
        card_file[8..8 + g.len()].copy_from_slice(&g);
        card_file[at..at + l.len()].copy_from_slice(&l);
        Ok(())
    }
}

/// Card file name of save slot `slot` (`"/…RATCHET/save%d.bin"` at boot 0x1a79d8).
pub fn save_file_name(slot: usize) -> String { format!("save{slot}.bin") }
pub const ICON_SYS_NAME: &str = "icon.sys";
pub const STATIC_ICO_NAME: &str = "static.ico";

/// `0x299998`: the card directory from `SYSTEM.CNF` (`BOOT2 = cdrom0:\SCUS_972.68;1` → `BASCUS-97268RATCHET`). The
/// template `/BA****-*****RATCHET` takes chars 16..20, 21..24 and 25..27 of the file; char 18 picks the region
/// letter (`E` → E, `P` → I, `K` → K, else A).
pub fn card_dir_name(system_cnf: &[u8]) -> Result<String> {
    Buf(system_cnf).check(0, 27, "SYSTEM.CNF")?;
    let s = system_cnf;
    let region = match s[0x12] { b'E' => b'E', b'P' => b'I', b'K' => b'K', _ => b'A' };
    let mut n = b"BA****-*****RATCHET".to_vec();
    n[1] = region;
    n[2..6].copy_from_slice(&s[16..20]);
    n[7..10].copy_from_slice(&s[21..24]);
    n[10..12].copy_from_slice(&s[25..27]);
    String::from_utf8(n).or_else(|_| invalid("SYSTEM.CNF: non-ASCII boot name"))
}

/// The save blob (`MISC.WAD` part 2, read by card state 10.3 / 0x11 from sectors `0x14b880`): words `[0]` icon.sys
/// offset (0x3c4 bytes), `[2]`/`[3]` static.ico offset / size, `[4]` the blank save's offset (its size is the file's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveBlob {
    pub icon_sys: Vec<u8>,
    pub static_ico: Vec<u8>,
    /// The blank `save%d.bin` (new-game defaults; every created slot starts as it).
    pub template: Vec<u8>,
}

impl SaveBlob {
    pub fn parse(blob: &[u8], file_size: usize) -> Result<SaveBlob> {
        let b = Buf(blob);
        let at = |i: usize| -> Result<usize> { Ok(b.u32(i * 4)? as usize) };
        Ok(SaveBlob {
            icon_sys: b.sub(at(0)?, ICON_SYS_SIZE, "icon.sys")?.bytes().to_vec(),
            static_ico: b.sub(at(2)?, at(3)?, "static.ico")?.bytes().to_vec(),
            template: b.sub(at(4)?, file_size, "save template")?.bytes().to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_tables() {
        let d = |size| ChunkDesc { addr: 0x1000, size, id: 1 };
        // 8 + align4(4 + 8) + align4(5 + 8) + 8
        assert_eq!(section_size(&[d(4), d(5)]), 8 + 12 + 16 + 8);
    }

    #[test]
    fn section_round_trip_and_checksum() {
        let s = Section::new(vec![Chunk::new(3, vec![1, 2, 3]), Chunk::new(7, vec![9; 8])]);
        let bytes = s.encode(0x1000);
        let back = Section::decode(&bytes, 0x1000).unwrap();
        assert!(back.crc_ok);
        assert_eq!(back.chunks, s.chunks);
        let mut bad = bytes.clone();
        bad[12] ^= 1;
        assert!(!Section::decode(&bad, 0x1000).unwrap().crc_ok);
        // Longer than the limit: the checksum is 0, which never passes.
        assert!(!Section::decode(&s.encode(4), 4).unwrap().crc_ok);
    }

    #[test]
    fn card_dir_from_system_cnf() {
        assert_eq!(card_dir_name(b"BOOT2 = cdrom0:\\SCUS_972.68;1\r\n").unwrap(), "BASCUS-97268RATCHET");
    }
}
