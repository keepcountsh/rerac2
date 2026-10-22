//! Shrub instance lighting: a bit-exact port of the game's `LightShrubs` (Going Commando: L00 `0x35d690`; R&C1's
//! level01 `FUN_0029e7e8`, 0x5b4 bytes) and the VU0 micro routines it drives (program 436083,
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
use crate::tfrag_light::{ps2, LightBank};

type V4 = [u32; 4];

/// `I` of VU0 program 436083 (`minii.xyz` after the colour sum): 65536 + 243 / 128.
pub const COLOR_CLAMP: u32 = 0x4780_00f3;
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

/// The ambient word the level loader (`FUN_00255958`, the loop after gameplay pointer 0x3c) stores in
/// the matrix block's col0.w: `b << 16 | g << 8 | 0x80000000 | r` on the raw s32 channels (OR'ed, not
/// masked: a channel above 255 would spill into the next byte; retail channels are 0..255).
pub fn packed_ambient(inst: &ShrubInstance) -> u32 {
    let [r, g, b] = inst.colour;
    (b.wrapping_shl(16) | g.wrapping_shl(8) | r) as u32 | 0x8000_0000
}

/// `lw col0.w; pextlb; pextlh; padduw 0x47800000`: byte i of the packed word -> `65536 + c / 128`.
fn ambient_floats(word: u32) -> V4 { word.to_le_bytes().map(|b| 0x4780_0000 + b as u32) }

/// The unit axis columns: `vmul.xyz; vadday.x; vmaddz.x` (|c|²), `vrsqrt Q, vf0w`, `vmulq.xyz`.
fn unit_columns(inst: &ShrubInstance) -> [V4; 3] {
    std::array::from_fn(|i| {
        let c = bits(inst.matrix[i]);
        let q = ps2::rsqrt(ps2::ONE, len2(c));
        [ps2::mul(c[0], q), ps2::mul(c[1], q), ps2::mul(c[2], q), c[3]]
    })
}

/// The VU0 registers `LightShrubs` sets up before the micro calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShrubLightRegs {
    /// vf27, vf28, vf29: colour of light A, B and the merged point light (w cleared; zero at load).
    pub colors: [V4; 3],
    /// vf30.xyz: the back-face factors (`max(d, d * w)`).
    pub back: [u32; 3],
    /// vf24, vf25, vf26: x, y and z components of the three class-space light vectors (lane k = light k).
    pub rows: [V4; 3],
    /// vf31: the ambient colour as floats (`65536 + c / 128`, alpha lane 0x80).
    pub ambient: V4,
}

/// The EE half of `LightShrubs` for one instance: light-set select and blend, the rotation of the light directions into class space and the ambient.
pub fn light_regs(inst: &ShrubInstance, bank: &LightBank) -> ShrubLightRegs {
    // Run-time record +0x1c = (u16) dir_lights.
    let sel = inst.dir_lights as u16;
    let set = |i: u16| bank.sets[(i & 0xf) as usize];
    let (mut ca, mut da, mut cb, mut db);
    if sel & 0xff00 == 0 {
        // lqc2 vf27, vf24, vf28, vf25 = qw 0..3 of the set: no renormalisation.
        let s = set(sel);
        (ca, da, cb, db) = (bits(s.color_a), bits(s.dir_a), bits(s.color_b), bits(s.dir_b));
    } else {
        // vitof12.x t = ((sel >> 4) & 0xff0); vsubx.w w = 1 - t; set A * w and set B * t on xyz only
        // (vmulw.xyz / vmulx.xyz), then vadd.xyzw: the w lanes (back factors) add unscaled.
        let t = ps2::itof12(((sel as u32 >> 4) & 0xff0) as i32);
        let w = ps2::sub(ps2::ONE, t);
        let (a, b) = (set(sel), set(sel >> 4));
        let blend = |x: [f32; 4], y: [f32; 4]| -> V4 {
            let (x, y) = (bits(x), bits(y));
            [ps2::add(ps2::mul(x[0], w), ps2::mul(y[0], t)), ps2::add(ps2::mul(x[1], w), ps2::mul(y[1], t)),
             ps2::add(ps2::mul(x[2], w), ps2::mul(y[2], t)), ps2::add(x[3], y[3])]
        };
        ca = blend(a.color_a, b.color_a);
        cb = blend(a.color_b, b.color_b);
        da = blend(a.dir_a, b.dir_a);
        db = blend(a.dir_b, b.dir_b);
        // vrsqrt Q, vf0w, |d|²; vmulq.xyz.
        for d in [&mut da, &mut db] {
            let q = ps2::rsqrt(ps2::ONE, len2(*d));
            *d = [ps2::mul(d[0], q), ps2::mul(d[1], q), ps2::mul(d[2], q), d[3]];
        }
    }
    // vaddw.x / vaddw.y vf30, vf0, colour: the back factors; vsubw.w clears the colour w lanes.
    let (wa, wb) = (ps2::add(0, ca[3]), ps2::add(0, cb[3]));
    ca[3] = 0;
    cb[3] = 0;

    // The point lights merge into a third light (the renderer's: crate docs).
    let (cp, dp, wp) = ([0u32; 4], [0u32; 4], 0);

    // vf4..vf6 = rows of -N (N = unit columns): vf4 = (-c0.x, -c1.x, -c2.x), vf5 = (.y), vf6 = (.z);
    // L = vf4 * d.x + vf5 * d.y + vf6 * d.z = -N^T d (vmulax / vmadday / vmaddz .xyz).
    let n = unit_columns(inst);
    let neg = |c: usize, k: usize| ps2::sub(0, n[c][k]);
    let class_space = |d: V4| -> V4 {
        let mut l = [0u32; 4];
        for (c, lc) in l.iter_mut().enumerate().take(3) {
            *lc = ps2::add(ps2::add(ps2::mul(neg(c, 0), d[0]), ps2::mul(neg(c, 1), d[1])), ps2::mul(neg(c, 2), d[2]));
        }
        l
    };
    let (la, lb, lp) = (class_space(da), class_space(db), class_space(dp));
    // vaddx.x vf24, vf0, vf1 ...: vf24 = (La.x, Lb.x, Lp.x, 0) etc. (each lane through vf0 + x).
    let rows = std::array::from_fn(|i| [ps2::add(0, la[i]), ps2::add(0, lb[i]), ps2::add(0, lp[i]), 0]);
    ShrubLightRegs { colors: [ca, cb, cp], back: [wa, wb, wp], rows, ambient: ambient_floats(packed_ambient(inst)) }
}

/// VU0 436083 for one normal (entry 0 / 0x2c, one of the four lanes): `itof15.xyz`, three dot
/// products, back-face `max(d, d * w)`, `ambient * 1.0 + A * dA + B * dB + P * dP` (xyzw),
/// `minii.xyz` 243; then the EE's `ppach/ppacb` keeps the low byte of each lane.
pub fn light_normal(regs: &ShrubLightRegs, normal: [i16; 4]) -> [u8; 4] {
    // itof15: exact (|n| < 2^24, power-of-two scale).
    let n = [0, 1, 2].map(|k| (normal[k] as f32 / 32768.0).to_bits());
    let [r24, r25, r26] = regs.rows;
    let mut f = [0u32; 3];
    for (k, fk) in f.iter_mut().enumerate() {
        let d = ps2::add(ps2::add(ps2::mul(r24[k], n[0]), ps2::mul(r25[k], n[1])), ps2::mul(r26[k], n[2]));
        *fk = ps2::max(d, ps2::mul(d, regs.back[k]));
    }
    let mut out = [0u8; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let mut acc = ps2::mul(regs.ambient[c], ps2::ONE);
        for (l, fl) in f.iter().enumerate() { acc = ps2::add(acc, ps2::mul(regs.colors[l][c], *fl)); }
        if c < 3 { acc = ps2::min(acc, COLOR_CLAMP); }
        *o = acc as u8;
    }
    out
}

/// Runs `LightShrubs` for one instance as at level load (the loader sets every record's point-light list to 0xffff):
/// the 24 palette colours, indexed by normal (0x80 = 1.0 under GS MODULATE; RGB <= 243; alpha 0x80).
pub fn light_shrub_instance(class: &ShrubClass, inst: &ShrubInstance, bank: &LightBank) -> [[u8; 4]; PALETTE] {
    let regs = light_regs(inst, bank);
    std::array::from_fn(|j| light_normal(&regs, class.normals.get(j).copied().unwrap_or([0; 4])))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vu1_quirk_addresses() {
        let base = vu1_palette_base(0xee, 0);
        assert_eq!(base, 0xf3);
        // Normal vertices read their own palette entry.
        assert_eq!(vu1_colour_address(6, 2, 7, base), 0xf3 + 7);
        assert_eq!(vu1_colour_address(8, 3, 7, base), 0xf3 + 7);
        assert_eq!(vu1_shrub_data(0xf3 + 7), Vu1ShrubData::Palette { buffer: 0xee, slot: 0, entry: 7 });
        // Vertex 3 of a 6-vertex packet, slot 0 of buffer 0xee: n + 0x1e6 lands in the other buffer's slot 3
        // (palette entry 18 + n for n < 6), slot 4's matrix (n = 6..9) or slot 4's palette (n - 10).
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 2, base)), Vu1ShrubData::Palette { buffer: 0x17b, slot: 3, entry: 20 });
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 7, base)), Vu1ShrubData::Matrix { buffer: 0x17b, slot: 4, column: 1 });
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 20, base)), Vu1ShrubData::Palette { buffer: 0x17b, slot: 4, entry: 10 });
        // Any later slot, or the 0x17b buffer, lands in a GS-packet output buffer.
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 0, vu1_palette_base(0xee, 1))), Vu1ShrubData::Other(0x21e));
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 23, vu1_palette_base(0x17b, 4))), Vu1ShrubData::Other(0x3f7));
    }













}
