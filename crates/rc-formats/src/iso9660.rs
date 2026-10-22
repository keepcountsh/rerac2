//! Minimal ISO 9660 reader for PS2 disc images. Spec: docs/formats/disc_layout.md section 1.
//! Ported from the retired C++ reference extractor's reader (git 2230812).
//!
//! RAC1 only puts `SYSTEM.CNF`, the boot ELF and `IOPRP243.IMG` in the filesystem; everything
//! else is addressed by absolute sector from the TOC, so the reader exposes both the directory
//! tree and raw sector reads. Reads seek to exactly the bytes asked for: the image (~4 GB for the
//! NTSC-U DVD) is never read whole.
//!
//! Plain 2048-byte-sector images (`.iso`) and raw 2352-byte CD dumps (`.bin`, mode 1 or mode 2
//! form 1, detected by the 12-byte sync pattern) are both accepted; only the 2048 user bytes of
//! each sector are returned.

use crate::buf::{Buf, FormatError};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use thiserror::Error;

pub const SECTOR_SIZE: usize = 2048;
const RAW_SECTOR_SIZE: usize = 2352;
const PVD_SECTOR: u32 = 16;
const MAX_DEPTH: u32 = 8;
const MAX_DIR_BYTES: u32 = 16 << 20;

#[derive(Debug, Error)]
pub enum DiscError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Format(#[from] FormatError),
}
pub(crate) type Result<T> = std::result::Result<T, DiscError>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> { Err(DiscError::Format(FormatError::Invalid(msg.into()))) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsoEntry {
    /// Uppercase, without the `;1` version suffix.
    pub(crate) name: String,
    /// `/DIR/FILE.EXT`, uppercase.
    pub path: String,
    pub lba: u32,
    pub size: u32,
    pub is_directory: bool,
}

pub struct IsoImage<R = File> {
    reader: Mutex<R>,
    sector_count: u32,
    raw_sector_size: usize,
    raw_user_offset: usize,
    volume_id: String,
    volume_sectors: u32,
    entries: Vec<IsoEntry>,
}

impl<R: Read + Seek> IsoImage<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let file_size = reader.seek(SeekFrom::End(0))?;
        let mut head = [0u8; 16];
        reader.seek(SeekFrom::Start(0))?;
        let got = read_up_to(&mut reader, &mut head)?;
        const SYNC: [u8; 12] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        let (raw_sector_size, raw_user_offset) = if got == 16 && head[..12] == SYNC {
            (RAW_SECTOR_SIZE, if head[15] == 2 { 24 } else { 16 })   // mode 2 form 1 vs mode 1
        } else {
            (SECTOR_SIZE, 0)
        };
        let sector_count = u32::try_from(file_size / raw_sector_size as u64).map_err(|_| FormatError::Invalid("image too large".into()))?;
        let mut iso = IsoImage { reader: Mutex::new(reader), sector_count, raw_sector_size, raw_user_offset, volume_id: String::new(), volume_sectors: 0, entries: Vec::new() };

        let pvd = iso.read_sectors(PVD_SECTOR, 1)?;
        let b = Buf(&pvd);
        if b.u8(0)? != 1 || &pvd[1..6] != b"CD001" { return bad("no ISO 9660 primary volume descriptor at sector 16"); }
        iso.volume_id = String::from_utf8_lossy(&pvd[40..72]).trim_end_matches(' ').to_string();
        iso.volume_sectors = b.u32(80)?;
        if b.u16(128)? as usize != SECTOR_SIZE { return bad("unexpected ISO 9660 logical block size"); }
        // The root directory record is embedded at PVD+156.
        let (root_lba, root_size) = (b.u32(156 + 2)?, b.u32(156 + 10)?);
        let mut entries = Vec::new();
        iso.walk_directory(root_lba, root_size, "", 0, &mut entries)?;
        iso.entries = entries;
        Ok(iso)
    }

    fn walk_directory(&self, lba: u32, size: u32, prefix: &str, depth: u32, out: &mut Vec<IsoEntry>) -> Result<()> {
        if depth > MAX_DEPTH { return bad("ISO 9660: directory nesting too deep"); }
        if size > MAX_DIR_BYTES { return bad(format!("ISO 9660: implausible directory size {size:#x}")); }
        let dir = self.read_sectors(lba, (size as usize).div_ceil(SECTOR_SIZE) as u32)?;
        let b = Buf(&dir);
        let mut pos = 0usize;
        while pos < size as usize {
            let len = b.u8(pos)? as usize;
            if len == 0 {
                // Records never straddle sectors; a zero length means "continue at the next sector".
                pos = (pos / SECTOR_SIZE + 1) * SECTOR_SIZE;
                continue;
            }
            let (e_lba, e_size, flags, name_len) = (b.u32(pos + 2)?, b.u32(pos + 10)?, b.u8(pos + 25)?, b.u8(pos + 32)? as usize);
            let raw_name = b.sub(pos + 33, name_len, "directory record name")?.0;
            pos += len;
            // "." and ".." are the single bytes 0x00 and 0x01.
            if raw_name.is_empty() || raw_name[0] == 0 || raw_name[0] == 1 { continue; }
            let mut name = String::from_utf8_lossy(raw_name).to_ascii_uppercase();
            if let Some(semi) = name.find(';') { name.truncate(semi); }
            let path = format!("{prefix}/{name}");
            let is_directory = flags & 2 != 0;
            out.push(IsoEntry { name, path: path.clone(), lba: e_lba, size: e_size, is_directory });
            if is_directory { self.walk_directory(e_lba, e_size, &path, depth + 1, out)?; }
        }
        Ok(())
    }

    pub fn sector_count(&self) -> u32 { self.sector_count }
    /// 2048 for a plain `.iso`, 2352 for a raw CD dump.
    pub fn raw_sector_size(&self) -> usize { self.raw_sector_size }
    pub fn volume_id(&self) -> &str { &self.volume_id }
    /// The PVD's volume space size (PVD+80, logical blocks). An image with fewer sectors than this is truncated.
    pub fn volume_sectors(&self) -> u32 { self.volume_sectors }
    pub fn entries(&self) -> &[IsoEntry] { &self.entries }

    /// Case-insensitive lookup by path; the leading `/` and a `;1` suffix are optional.
    pub fn find(&self, path: &str) -> Option<&IsoEntry> {
        let mut want = path.to_ascii_uppercase();
        if let Some(semi) = want.rfind(';') { want.truncate(semi); }
        if !want.starts_with('/') { want.insert(0, '/'); }
        self.entries.iter().find(|e| e.path == want)
    }

    pub fn read_file(&self, e: &IsoEntry) -> Result<Vec<u8>> { self.read_bytes(e.lba as u64 * SECTOR_SIZE as u64, e.size as u64) }

    pub(crate) fn read_sectors(&self, lba: u32, count: u32) -> Result<Vec<u8>> {
        self.read_bytes(lba as u64 * SECTOR_SIZE as u64, count as u64 * SECTOR_SIZE as u64)
    }

    /// Reads `size` user-data bytes starting at logical byte `offset` (= sector * 2048 + in-sector offset).
    pub(crate) fn read_bytes(&self, offset: u64, size: u64) -> Result<Vec<u8>> {
        let ss = SECTOR_SIZE as u64;
        let end = offset.checked_add(size).ok_or_else(|| FormatError::Invalid("byte range overflows".into()))?;
        if end > self.sector_count as u64 * ss {
            return bad(format!("byte range {offset:#x}+{size:#x} beyond end of image ({} sectors)", self.sector_count));
        }
        let mut out = vec![0u8; size as usize];
        let mut r = self.reader.lock().unwrap_or_else(|e| e.into_inner());
        if self.raw_sector_size == SECTOR_SIZE {
            r.seek(SeekFrom::Start(offset))?;
            r.read_exact(&mut out)?;
        } else {
            let mut done = 0usize;
            while done < out.len() {
                let pos = offset + done as u64;
                let (sector, within) = (pos / ss, (pos % ss) as usize);
                let n = (SECTOR_SIZE - within).min(out.len() - done);
                r.seek(SeekFrom::Start(sector * self.raw_sector_size as u64 + (self.raw_user_offset + within) as u64))?;
                r.read_exact(&mut out[done..done + n])?;
                done += n;
            }
        }
        Ok(out)
    }
}

fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? { 0 => break, k => n += k }
    }
    Ok(n)
}

