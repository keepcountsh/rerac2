//! Tie instance lighting: a bit-exact port of the game's `LightTies` (boot 0x237370, level01
//! overlay 0x2ab218) and the VU0 micro routines it drives (program 436083, entries 0x58 / 0x84).
//! Spec and derivation: docs/plan/tie_lighting.md.
//!
//! Ties are instanced, so the game lights the class's 64 **light slots** once per instance, not the
//! vertices: slot `j` has a class normal (`TieClass::normals[j]`, s16 / 32768) and an instance
//! ambient colour (`TieInstance::ambient_rgbas[j]`, RGBA5551). The result is a 64-entry RGBA table
//! per instance (the 0x100 bytes at +0x40 of the instance's 0x1c0-byte run-time record), which
//! `TieProc` uploads to VU1 (qw 0x346 / 0x386) and VU1 program 13507 indexes with each vertex's
//! colour-index byte ([`crate::tie::TieVertex::color`]).
//!
//! Instead of rotating the 64 normals into world space, the pass rotates the (up to three) light
//! directions into class space with the normalised transpose of the instance matrix; the colour
//! arithmetic then matches `LightTfrags` (docs/plan/tfrag_lighting.md §4), with two differences:
//! a third light (the merged point lights) and a clamp to **243** (`I = 0x478000f3`), not 255.
//!
//! All arithmetic runs on PS2 float bit patterns ([`crate::tfrag_light::ps2`]).

use crate::tfrag_light::ps2;
use crate::tie::{TieClass, TieInstance};

type V4 = [u32; 4];

/// Number of light slots per tie class / instance.
pub const SLOTS: usize = 64;

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
/// `vmul t, a, a; vadday.x ACC, t, t; vmaddz.x r, one, t`: `(x*x + y*y) + 1.0*(z*z)`.
fn len2(a: V4) -> u32 {
    let (x, y, z) = (ps2::mul(a[0], a[0]), ps2::mul(a[1], a[1]), ps2::mul(a[2], a[2]));
    ps2::add(ps2::add(x, y), ps2::mul(ps2::ONE, z))
}

/// The instance's world-space bounding-sphere centre as the level loader stores it in the run-time
/// record (+0x00, read by `LightTies` for point lights and by `TieProc` for culling):
/// `fun_001f9cf8` (M3x3 · bsphere.xyz, w = bsphere.w), `fun_001f9a80` (× class scale, all lanes),
/// `fun_001f9a10` (+ translation, xyz). The radius lane is overwritten by the loader with
/// `bsphere.w × max column length`, then scaled; it is returned in `.w`.
pub fn instance_centre(class: &TieClass, inst: &TieInstance) -> [f32; 4] {
    let m = inst.matrix.map(bits);
    let b = bits(class.header.bsphere);
    // vmulax / vmadday / vmaddaz with the three columns, vmaddw with vf0 = (0, 0, 0, 1).
    let mut c = [0u32; 4];
    for k in 0..4 {
        let acc = ps2::add(ps2::add(ps2::mul(m[0][k], b[0]), ps2::mul(m[1][k], b[1])), ps2::mul(m[2][k], b[2]));
        c[k] = ps2::add(acc, ps2::mul(if k == 3 { ps2::ONE } else { 0 }, b[3]));
    }
    // piVar37[3] = bsphere.w * max(|c0|, |c1|, |c2|) (FPU mul.s; the max is a float compare).
    let lens = column_lengths(inst);
    let maxlen = f32::from_bits(lens[0]).max(f32::from_bits(lens[1])).max(f32::from_bits(lens[2]));
    c[3] = ps2::mul(b[3], maxlen.to_bits());
    let c = vmuls(c, class.header.scale.to_bits());
    [ps2::add(c[0], m[3][0]), ps2::add(c[1], m[3][1]), ps2::add(c[2], m[3][2]), c[3]].map(f32::from_bits)
}

/// `FastVecLength` (boot 0x1f9af0) of the instance matrix's three axis columns:
/// `sqrt((x*x + y*y) + 1.0*(z*z))` on VU0.
pub(crate) fn column_lengths(inst: &TieInstance) -> [u32; 3] {
    std::array::from_fn(|i| ps2::sqrt(len2(bits(inst.matrix[i]))))
}
