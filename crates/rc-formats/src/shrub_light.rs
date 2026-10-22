//! Shrub instance lighting: a bit-exact port of the game's `LightShrubs` (level01 `FUN_0029e7e8`,
//! 0x5b4 bytes; boot copy not located) and the VU0 micro routines it drives (program 436083,
//! `vcallms 0` for normals 0-3 into vf09-vf12 and `vcallms 0x160` = entry 0x2c for normals 4-7 into
//! vf13-vf16; the same code as the tie entries 0x58 / 0x84 on other registers). Spec:
//! docs/plan/shrub_lighting.md.
//!
//! Per instance the pass lights the class's 24 normals ([`ShrubClass::normals`], s16 / 32768) once and
//! writes a 24-entry RGBA palette (the instance's 0x60-byte slot at `gp-0x6760`); VU1 program 56467
//! colours each vertex with `palette[vertex.normal]`. Like `LightTies` it rotates the (up to three) light
//! directions into class space with the normalised transpose of the instance matrix; the differences
//! from the tie pass, all read in the disassembly:
//! * the column normalisation is `c * rsqrt(|c|²)` on VU0 (`vrsqrt Q, vf0w, len²; vmulq.xyz`), not the
//!   loader's `1.0 / FastVecLength` (FPU `div.s`) that ties use;
//! * a blended light set scales only the xyz lanes (`vmulw.xyz` / `vmulx.xyz`), so the blended back
//!   factor (colour `.w`) is the plain **sum** of both sets' factors (ties scale xyzw);
//! * the ambient is one colour per instance (`colour`, packed by the loader into the matrix block's
//!   col0.w as `r | g << 8 | b << 16 | 0x80000000`), alpha 0x80;
//! * the point-light origin is the instance's world bounding-sphere centre (run-time record +0x00).
//!
//! All arithmetic runs on PS2 float bit patterns ([`crate::tfrag_light::ps2`]).

use crate::shrub::{ShrubClass, ShrubInstance};
use crate::tfrag_light::ps2;

type V4 = [u32; 4];

/// Normals per shrub class = palette entries per instance.
pub const PALETTE: usize = 24;

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
/// `vmul t, a, a; vadday.x ACC, t, t; vmaddz.x r, vf21 (1.0), t`: `(x*x + y*y) + 1.0*(z*z)`.
fn len2(a: V4) -> u32 {
    let (x, y, z) = (ps2::mul(a[0], a[0]), ps2::mul(a[1], a[1]), ps2::mul(a[2], a[2]));
    ps2::add(ps2::add(x, y), ps2::mul(ps2::ONE, z))
}

/// `FastVecLength` (boot 0x1f9af0) of the instance matrix's three axis columns, as the loader
/// computes them for the bounding radius: `sqrt((x*x + y*y) + 1.0*(z*z))`.
pub(crate) fn column_lengths(inst: &ShrubInstance) -> [u32; 3] { std::array::from_fn(|i| ps2::sqrt(len2(bits(inst.matrix[i])))) }

/// The instance's world bounding sphere as the loader stores it in the run-time record (+0x00; used by
/// `LightShrubs` for point lights and by `ShrubProc` for the draw-distance / frustum tests):
/// `fun_001f9cf8` (M3x3 · class bsphere.xyz), radius = bsphere.w × max column length (FPU),
/// `fun_001f9a80` (× class `scale`, all lanes), `fun_001f9a10` (+ translation, xyz). So the class
/// bounding sphere is in units of `scale` world units (1024 × the vertex quantum), as Wrench's builder
/// assumes (docs/formats/shrub_sky_rac1.md §1.5).
pub fn instance_centre(class: &ShrubClass, inst: &ShrubInstance) -> [f32; 4] {
    let m = inst.matrix.map(bits);
    let b = bits(class.header.bsphere);
    let mut c = [0u32; 4];
    for (k, ck) in c.iter_mut().enumerate().take(3) {
        *ck = ps2::add(ps2::add(ps2::mul(m[0][k], b[0]), ps2::mul(m[1][k], b[1])), ps2::mul(m[2][k], b[2]));
    }
    let lens = column_lengths(inst);
    let maxlen = f32::from_bits(lens[0]).max(f32::from_bits(lens[1])).max(f32::from_bits(lens[2]));
    c[3] = ps2::mul(b[3], maxlen.to_bits());
    let c = vmuls(c, class.header.scale.to_bits());
    [ps2::add(c[0], m[3][0]), ps2::add(c[1], m[3][1]), ps2::add(c[2], m[3][2]), c[3]].map(f32::from_bits)
}

/// VU1 data-memory quadwords (addresses wrap).
pub(crate) const VU1_QWC: u16 = 0x400;
/// The two instance buffers of VU1 program 56467 (batch count at the base; `ShrubProc` alternates them
/// per `MSCALF` batch, `0x269 - base`).
pub(crate) const VU1_INSTANCE_BUFFERS: [u16; 2] = [0xee, 0x17b];
/// Quadwords per instance slot (4 matrix columns + 24 palette entries).
pub(crate) const VU1_SLOT_QWC: u16 = 0x1c;
/// Instances per `MSCALF` batch.
pub(crate) const VU1_BATCH: usize = 5;

/// `vi13` while VU1 56467 draws instance `k` of a batch in the buffer at `buffer`: the instance's palette
/// base = `buffer + 1 + 0x1c k` (slot) `+ 4` (matrix) (`iaddiu vi13, vi13, 5` after the matrix loads, then
/// `+ 0x1c` per instance).
pub fn vu1_palette_base(buffer: u16, k: usize) -> u16 { buffer + 5 + VU1_SLOT_QWC * k as u16 }

/// The VU1 address vertex `vertex` of a packet reads its colour from (`lq.xyz vfXX, 0(vi07)`,
/// `vi07 = (n & 0x7fff) + vi13`). Quirk (docs/formats/shrub_sky_rac1.md §1.3b): when the stop flag is on
/// vertex 2 (a packet of 6 written vertices), the loop leaves at instruction 182 (`ibltz vi01, L12`) with
/// `vi07` already = n + vi13 for vertex 3, and L12 adds `vi13` again (instruction 308): vertex 3 reads
/// `n + 2·vi13`. Only the colour (RGB) is affected; alpha stays the instance's.
pub fn vu1_colour_address(written_vertices: usize, vertex: usize, normal: u8, palette_base: u16) -> u16 {
    let extra = if written_vertices == 6 && vertex == 3 { palette_base } else { 0 };
    (normal as u16 + palette_base + extra) % VU1_QWC
}

/// What an address of VU1 data memory holds while the shrub program runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vu1ShrubData {
    /// Palette entry `entry` of instance slot `slot` of an instance buffer.
    Palette { buffer: u16, slot: usize, entry: u8 },
    /// Matrix column of instance slot `slot` (floats: the colour bytes would be the low bytes of the bits).
    Matrix { buffer: u16, slot: usize, column: u8 },
    /// Anything else: constants, the input buffers, a batch count, or a GS-packet output buffer
    /// (0x208 / 0x2b0 / 0x358, 0xa8 qw each: tags, ad-gifs and the ST / RGBAQ / XYZF2 of earlier kicks).
    Other(u16),
}

/// Decodes a VU1 data address against the 56467 memory map.
pub fn vu1_shrub_data(addr: u16) -> Vu1ShrubData {
    let addr = addr % VU1_QWC;
    for buffer in VU1_INSTANCE_BUFFERS {
        let rel = addr.wrapping_sub(buffer);
        if (1..=VU1_SLOT_QWC * VU1_BATCH as u16).contains(&rel) {
            let (slot, qw) = (((rel - 1) / VU1_SLOT_QWC) as usize, ((rel - 1) % VU1_SLOT_QWC) as u8);
            return if qw < 4 { Vu1ShrubData::Matrix { buffer, slot, column: qw } } else { Vu1ShrubData::Palette { buffer, slot, entry: qw - 4 } };
        }
    }
    Vu1ShrubData::Other(addr)
}

