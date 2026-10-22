//! Tfrag vertex lighting: a bit-exact port of the game's `LightTfrags` (boot 0x234f98, level
//! overlays e.g. level01 0x2a8e40). Spec and derivation: docs/plan/tfrag_lighting.md.
//!
//! The EE runs this pass in VU0 macro mode on PS2 floats. Colours are carried as the float
//! `65536.0 + c / 128` (bits `0x4780_0000 + c`), so the final colour byte is simply the low byte
//! of the float's bit pattern after all accumulations; every add truncates to that 1/128 grid.
//! To reproduce the bytes we therefore emulate the PS2 FMAC arithmetic on raw bit patterns
//! ([`ps2`]), not IEEE `f32`.
//!
//! Inputs:
//! * [`LightBank`]: the level's directional light sets, gameplay-file section pointer 0x04
//!   (copied to EE 0x180340 by the level-load routine, `FUN_00255958` in level01).
//! * [`NormalTable`]: 256 `(cos, sin)` f32 pairs the pass DMAs into scratchpad 0x70003800. The values
//!   are not IEEE-rounded `cos`/`sin` (296 of 512 differ in the last bits), so they must be read
//!   from the executable: boot ELF vaddr [`BOOT_NORMAL_TABLE_VADDR`] (every level overlay carries an
//!   identical copy, at 0x166500 in level01).
//! * The tfrag's per-vertex light records ([`crate::tfrag::TfragLight`], re-read here as
//!   `u16 pos_ofs, u8 azimuth, u8 elevation, u16 rgb555a1, u16 light_select`).

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

/// Directional light sets in the EE bank (`FastMemZero16(0x180340, 0x400)`).
pub(crate) const BANK_SETS: usize = 16;
/// The level loader caps the gameplay light count at 12 (`if (0xb < n) { printf; n = 0xc; }`).
pub(crate) const MAX_LEVEL_LIGHTS: usize = 12;

/// One directional light set (0x40 bytes): two lights, each a colour and a direction.
/// `color*.w` is the back-face factor: the dot product `d` becomes `max(d, d * w)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct DirLightSet {
    /// 0x00: light A colour (1.0 adds 128 to the colour byte), w = back-face factor.
    pub(crate) color_a: [f32; 4],
    /// 0x10: light A direction (unit vector the light travels along), w unused.
    pub(crate) dir_a: [f32; 4],
    /// 0x20: light B colour, w = back-face factor.
    pub(crate) color_b: [f32; 4],
    /// 0x30: light B direction.
    pub(crate) dir_b: [f32; 4],
}
const _: () = assert!(std::mem::size_of::<DirLightSet>() == 0x40);

/// The EE directional light bank (16 sets; sets past the level's count are zero).
#[derive(Clone, Debug, PartialEq)]
pub struct LightBank {
    pub(crate) sets: [DirLightSet; BANK_SETS],
    /// Number of sets the gameplay file provided (after the loader's cap of 12).
    pub(crate) count: usize,
}

impl Default for LightBank {
    fn default() -> Self { LightBank { sets: [DirLightSet::default(); BANK_SETS], count: 0 } }
}

/// Parses the directional lights from a decompressed gameplay file: `u32` at 0x04 is the
/// section offset; the section is `s32 count`, 12 bytes padding, then `count` x [`DirLightSet`].
pub fn parse_light_bank(gameplay: &[u8]) -> Result<LightBank> {
    let g = Buf(gameplay);
    let ofs = g.u32(0x04)? as usize;
    let n = g.i32(ofs)?;
    if n < 0 { return invalid("negative directional light count"); }
    let n = (n as usize).min(MAX_LEVEL_LIGHTS);
    let mut bank = LightBank { count: n, ..Default::default() };
    let sets: Vec<DirLightSet> = g.pod_slice(ofs + 0x10, n, "directional lights")?;
    bank.sets[..n].copy_from_slice(&sets);
    Ok(bank)
}

/// One point light as stored at EE 0x180740 + 0x20 * slot (built at run time by gameplay code:
/// `FUN_002525f8`/`FUN_00252750` write colour = rgb / 128 and position/radius).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct PointLight {
    /// 0x00: colour (1.0 adds 128), w = back-face factor (also added into alpha, see spec).
    pub(crate) color: [f32; 4],
    /// 0x10: world position, w = radius.
    pub(crate) pos: [f32; 4],
}

/// 256 `(cos, sin)` pairs for angle `i * 2pi / 256`, as raw f32 bits.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalTable(pub [[u32; 2]; 256]);

impl NormalTable {
    /// Reads the table from an ELF32 image (the boot ELF) at `vaddr`.
    pub fn from_elf(elf: &[u8], vaddr: u32) -> Result<NormalTable> {
        let bytes = elf_read(elf, vaddr, 256 * 8)?;
        let words: Vec<u32> = Buf(bytes).pod_slice(0, 512, "normal table")?;
        let mut t = [[0u32; 2]; 256];
        for (i, e) in t.iter_mut().enumerate() { *e = [words[2 * i], words[2 * i + 1]]; }
        Ok(NormalTable(t))
    }
}

/// Reads `len` bytes at virtual address `vaddr` from the PT_LOAD segments of a little-endian ELF32.
pub(crate) fn elf_read(elf: &[u8], vaddr: u32, len: usize) -> Result<&[u8]> {
    let e = Buf(elf);
    if e.sub(0, 4, "ELF magic")?.bytes() != b"\x7fELF" { return invalid("not an ELF file"); }
    let (phoff, phentsize, phnum) = (e.u32(0x1c)? as usize, e.u16(0x2a)? as usize, e.u16(0x2c)? as usize);
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        let (ty, off, va, filesz) = (e.u32(ph)?, e.u32(ph + 4)? as usize, e.u32(ph + 8)?, e.u32(ph + 16)? as usize);
        if ty == 1 && vaddr >= va && (vaddr - va) as usize + len <= filesz {
            return Ok(e.sub(off + (vaddr - va) as usize, len, "ELF segment data")?.bytes());
        }
    }
    invalid(format!("vaddr {vaddr:#x}+{len:#x} is not in any ELF load segment"))
}

/// The per-vertex light record, decoded with the field meanings `LightTfrags` uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexLight {
    /// +0: byte offset of this vertex's position (3 x s16) from the tfrag data start (point lights only).
    pub(crate) pos_ofs: u16,
    /// +2: normal azimuth index into [`NormalTable`] (unsigned; 2pi / 256 per step).
    pub azimuth: u8,
    /// +3: normal elevation index.
    pub elevation: u8,
    /// +4: base (ambient) colour, 5:5:5:1 (r low bits, alpha bit 15), expanded by PEXT5.
    pub(crate) color: u16,
    /// +6: light set select: if bits 8..15 are 0, set `bits 0..3`; else blend set `bits 0..3`
    /// with set `bits 4..7` by `t = bits 8..15 / 256`.
    pub(crate) select: u16,
}

impl VertexLight {
    pub fn from_record(l: &crate::tfrag::TfragLight) -> VertexLight {
        let b = bytemuck::bytes_of(l);
        VertexLight {
            pos_ofs: u16::from_le_bytes([b[0], b[1]]),
            azimuth: b[2],
            elevation: b[3],
            color: u16::from_le_bytes([b[4], b[5]]),
            select: u16::from_le_bytes([b[6], b[7]]),
        }
    }
}

/// PS2 FPU/VU float arithmetic on raw bit patterns. Behaviour modelled (see the spec for sources):
/// no NaN/Inf/denormals (exponent 0 = zero, exponent 255 is an ordinary value, overflow clamps to
/// ±0x7fffffff, underflow flushes to ±0); every result rounds toward zero; the adder first
/// discards the bits of the smaller operand that lie more than one bit below the larger operand's
/// LSB, and returns the larger operand unchanged when the exponents differ by 25 or more.
/// Not modelled: the multiplier's rare last-bit deviations from a truncated exact product.
pub mod ps2 {
    pub const SIGN: u32 = 0x8000_0000;
    pub const MAX: u32 = 0x7fff_ffff;
    pub const ONE: u32 = 0x3f80_0000;

    fn exp(x: u32) -> i32 { ((x >> 23) & 0xff) as i32 }
    fn man(x: u32) -> u64 { ((x & 0x7f_ffff) | 0x80_0000) as u64 }

    /// Packs `sign * mag * 2^(lsb_exp)` (mag > 0, lsb_exp = biased exponent of mag's bit 23),
    /// truncating to 24 significant bits.
    fn pack(sign: u32, mag: u64, lsb_exp: i32) -> u32 {
        let k = 63 - mag.leading_zeros() as i32;
        let e = lsb_exp + (k - 23);
        let m = if k >= 23 { mag >> (k - 23) } else { mag << (23 - k) };
        if e > 255 { sign | MAX } else if e < 1 { sign } else { sign | (e as u32) << 23 | (m as u32 & 0x7f_ffff) }
    }

    pub fn mul(a: u32, b: u32) -> u32 {
        let s = (a ^ b) & SIGN;
        if exp(a) == 0 || exp(b) == 0 { return s; }
        // man(a) * man(b) has its binary point at bit 46; bit 23 of (p >> 23) is worth 2^(ea+eb-254).
        pack(s, (man(a) * man(b)) >> 23, exp(a) + exp(b) - 127)
    }

    pub fn add(a: u32, b: u32) -> u32 {
        let (ea, eb) = (exp(a), exp(b));
        if ea == 0 || eb == 0 {
            if ea != 0 { return a; }
            if eb != 0 { return b; }
            return if a & b & SIGN != 0 { SIGN } else { 0 };
        }
        let (hi, lo) = if ea >= eb { (a, b) } else { (b, a) };
        let d = exp(hi) - exp(lo);
        if d >= 25 { return hi; }
        let lo = if d >= 1 { lo & (u32::MAX << (d - 1)) } else { lo };
        let sv = |x: u32, m: u64| if x & SIGN != 0 { -(m as i64) } else { m as i64 };
        let sum = sv(hi, man(hi) << d) + sv(lo, man(lo));
        if sum == 0 { return 0; }
        pack(if sum < 0 { SIGN } else { 0 }, sum.unsigned_abs(), exp(lo))
    }

    pub fn sub(a: u32, b: u32) -> u32 { add(a, b ^ SIGN) }

    /// DIV: truncated quotient; division by zero gives ±MAX.
    pub fn div(a: u32, b: u32) -> u32 {
        let s = (a ^ b) & SIGN;
        if exp(b) == 0 { return s | MAX; }
        if exp(a) == 0 { return s; }
        let q = (man(a) << 24) / man(b); // in [2^23, 2^25)
        pack(s, q, exp(a) - exp(b) + 127 - 1)
    }

    /// SQRT of |x|, truncated.
    pub fn sqrt(x: u32) -> u32 {
        if exp(x) == 0 { return 0; }
        let e = exp(x) - 127;
        let odd = e.rem_euclid(2);
        let r = isqrt(man(x) << (23 + odd)); // in [2^23, 2^24)
        pack(0, r, (e - odd) / 2 + 127)
    }

    /// RSQRT a / sqrt(b): the square root is truncated first, then divided.
    pub(crate) fn rsqrt(a: u32, b: u32) -> u32 { div(a, sqrt(b)) }

    /// ITOF12: signed integer / 4096 (truncating integers wider than 24 bits).
    pub(crate) fn itof12(i: i32) -> u32 {
        if i == 0 { return 0; }
        pack(if i < 0 { SIGN } else { 0 }, i.unsigned_abs() as u64, 127 + 23 - 12)
    }

    fn key(x: u32) -> i64 { if x & SIGN != 0 { -((x & MAX) as i64) } else { x as i64 } }
    pub fn max(a: u32, b: u32) -> u32 { if key(b) > key(a) { b } else { a } }
    pub fn min(a: u32, b: u32) -> u32 { if key(b) < key(a) { b } else { a } }

    fn isqrt(n: u64) -> u64 {
        let mut r = (n as f64).sqrt() as u64;
        while r * r > n { r -= 1; }
        while (r + 1) * (r + 1) <= n { r += 1; }
        r
    }
}

