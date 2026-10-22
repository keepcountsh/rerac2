//! VIF1 command-list parsing. Spec: docs/formats/tfrag_rac1.md 2.1. Ported from the retired C++ reference extractor (git 2230812).

use crate::buf::{invalid, Result};

/// VIF command numbers (bits 30..24 of the code word) used by this crate.
pub(crate) const NOP: u8 = 0x00;
pub(crate) const STCYCL: u8 = 0x01;
pub(crate) const STMOD: u8 = 0x05;
pub(crate) const STMASK: u8 = 0x20;
pub(crate) const STROW: u8 = 0x30;
pub(crate) const STCOL: u8 = 0x31;
pub(crate) const MPG: u8 = 0x4a;
pub(crate) const DIRECT: u8 = 0x50;
pub(crate) const DIRECTHL: u8 = 0x51;

/// One VIF1 code with its inline payload.
#[derive(Clone, Copy, Debug)]
pub struct VifPacket<'a> {
    /// Bits 30..24 of the code word (the interrupt bit 31 is dropped).
    pub(crate) cmd: u8,
    /// Bits 23..16 (0 means 256 for unpacks).
    pub(crate) num: u8,
    /// Bits 15..0.
    pub(crate) imm: u16,
    /// Byte offset of the code word in the list.
    pub offset: usize,
    /// Inline payload (unpack data, STROW rows, ...).
    pub(crate) data: &'a [u8],
}

impl VifPacket<'_> {
    pub(crate) fn is_unpack(&self) -> bool { self.cmd & 0x60 == 0x60 }
    /// 0..3 = 1..4 components.
    pub(crate) fn vn(&self) -> u8 { (self.cmd >> 2) & 3 }
    /// 0 = 32-bit, 1 = 16-bit, 2 = 8-bit, 3 = 5-bit.
    pub(crate) fn vl(&self) -> u8 { self.cmd & 3 }
    /// Unpack USN bit (1 = unsigned).
    pub(crate) fn usn(&self) -> bool { (self.imm >> 14) & 1 != 0 }
    /// Unpack destination quadword address.
    pub(crate) fn addr(&self) -> u16 { self.imm & 0x3ff }
    /// Unpack element count (NUM, with 0 meaning 256).
    pub(crate) fn count(&self) -> u32 { if self.num == 0 { 256 } else { self.num as u32 } }
    /// Bytes per unpacked element on disk.
    pub(crate) fn element_size(&self) -> u32 { ((32u32 >> self.vl()) * (self.vn() as u32 + 1)) / 8 }
}

/// Parses a VIF command list up to the end of the buffer.
pub(crate) fn parse_vif(list: &[u8]) -> Result<Vec<VifPacket<'_>>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= list.len() {
        let code = u32::from_le_bytes(list[pos..pos + 4].try_into().unwrap());
        let mut p = VifPacket { cmd: ((code >> 24) & 0x7f) as u8, num: ((code >> 16) & 0xff) as u8, imm: (code & 0xffff) as u16, offset: pos, data: &[] };
        let payload = if p.is_unpack() {
            (p.count() as usize * p.element_size() as usize + 3) & !3
        } else {
            match p.cmd {
                STMASK => 4,
                STROW | STCOL => 16,
                MPG => p.count() as usize * 8,
                DIRECT | DIRECTHL => (if p.imm == 0 { 65536 } else { p.imm as usize }) * 16,
                _ => 0,
            }
        };
        if pos + 4 + payload > list.len() { return invalid("VIF packet runs past end of list"); }
        p.data = &list[pos + 4..pos + 4 + payload];
        out.push(p);
        pos += 4 + payload;
    }
    Ok(out)
}

