//! Tie instance lighting: Going Commando's light pass (L00 `0x36f0a8`) and the VU0 routines it drives (the boot's
//! copy of R&C1's program 436083 at `SCUS_972.68` file offset 0x1a70, entries 0x2c0 / 0x420). Derivation of the
//! shared parts: docs/plan/tie_lighting.md (R&C1's `LightTies`).
//!
//! Ties are instanced, so the game lights the class's colour slots once per instance, not the vertices: slot `j` has
//! a class normal (azimuth and elevation, [`crate::tie::TieColours::normals`]) and an instance colour
//! ([`crate::disc::gameplay::TieInstanceColours`]). The class's colour program then averages the lit slots into each
//! vertex's colour ([`crate::tie::TieVertex::colors`]).
//!
//! Against R&C1's `LightTies`: the slot colour is PEXT5'd, shifted and biased by the instance's header word; the
//! normal is `(cos a·cos e, sin a·cos e, sin e)` from the boot's `(cos, sin)` table (the VU0 entries open with
//! `mulz.xy` and dot the normal's w lane where R&C1 converted an s16 normal with `itof15`); the third light is zero
//! (point lights are a later pass of their own, per slot, VU0 0x580); slot 511 takes the class's self-lit colour,
//! or the colour a level object set (`0x3701d0`). The rest — the light set select and blend, the light directions
//! rotated into class space with the normalised transpose of the instance matrix, `max(d, d·w)`, the clamp to
//! **243** — is R&C1's.
//!
//! All arithmetic runs on PS2 float bit patterns ([`crate::tfrag_light::ps2`]).

use crate::disc::gameplay::TieInstanceColours;
use crate::tfrag_light::{ps2, LightBank, NormalTable};
use crate::tie::{TieClass, TieInstance};

type V4 = [u32; 4];

/// `I` of the VU0 program (`minii.xyz` clamp after the colour sum): 65536 + 243 / 128.
pub const COLOR_CLAMP: u32 = 0x4780_00f3;

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
fn vadd(a: V4, b: V4) -> V4 { [0, 1, 2, 3].map(|k| ps2::add(a[k], b[k])) }
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

/// The unit axis columns the loader leaves in the run-time matrix (`0x1b249c`): column i's w lane is set to
/// `1.0 / length` (EE FPU `div.s`), and the light pass scales the column by it.
fn unit_columns(inst: &TieInstance) -> [V4; 3] {
    let lens = column_lengths(inst);
    std::array::from_fn(|i| {
        let inv = ps2::div(ps2::ONE, lens[i]);
        let c = bits(inst.matrix[i]);
        [ps2::mul(c[0], inv), ps2::mul(c[1], inv), ps2::mul(c[2], inv), inv]
    })
}

/// The lights in VU0 register form: colours (w = 0) and back-face factors, and the class-space "to-light" vectors
/// transposed into vf24..vf26 (lane k = light k; the third light is zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TieLightRegs {
    /// vf27, vf28, vf29.
    pub colors: [V4; 3],
    /// vf30.xyz: `color.w` of each light (`max(d, d * w)`).
    pub back: [u32; 3],
    /// vf24, vf25, vf26.
    pub rows: [V4; 3],
}

/// The EE half for one instance: the light set (run-time record +0x18 = the instance's `directional_lights`) and its
/// blend, and the light directions rotated into class space.
pub fn light_regs(inst: &TieInstance, bank: &LightBank) -> TieLightRegs {
    let sel = inst.directional_lights as u16;
    let set = |i: u16| bank.sets[(i & 0xf) as usize];
    let (mut ca, mut da, mut cb, mut db);
    if sel & 0xff00 == 0 {
        let s = set(sel);
        (ca, da, cb, db) = (bits(s.color_a), bits(s.dir_a), bits(s.color_b), bits(s.dir_b));
    } else {
        // vitof12.x t = ((sel >> 4) & 0xff0); w = 1 - t; set A (low nibble) * w + set B (next nibble) * t.
        let t = ps2::itof12(((sel as u32 >> 4) & 0xff0) as i32);
        let w = ps2::sub(ps2::ONE, t);
        let (a, b) = (set(sel), set(sel >> 4));
        let blend = |x: [f32; 4], y: [f32; 4]| vadd(vmuls(bits(x), w), vmuls(bits(y), t));
        ca = blend(a.color_a, b.color_a);
        cb = blend(a.color_b, b.color_b);
        da = blend(a.dir_a, b.dir_a);
        db = blend(a.dir_b, b.dir_b);
        for d in [&mut da, &mut db] {
            let q = ps2::rsqrt(ps2::ONE, len2(*d));
            *d = [ps2::mul(d[0], q), ps2::mul(d[1], q), ps2::mul(d[2], q), d[3]];
        }
    }
    let (wa, wb) = (ps2::add(0, ca[3]), ps2::add(0, cb[3]));
    ca[3] = 0;
    cb[3] = 0;
    // L = -N^T d: the light vector in class space, pointing towards the light (N = the unit axis columns).
    let n = unit_columns(inst);
    let neg = |c: usize, k: usize| ps2::sub(0, n[c][k]);
    let class_space = |d: V4| -> V4 {
        let mut l = [0u32; 4];
        for (c, lc) in l.iter_mut().enumerate().take(3) {
            let acc = ps2::add(ps2::mul(neg(c, 0), d[0]), ps2::mul(neg(c, 1), d[1]));
            *lc = ps2::add(acc, ps2::mul(neg(c, 2), d[2]));
        }
        l
    };
    let (la, lb) = (class_space(da), class_space(db));
    let rows = std::array::from_fn(|i| [la[i], lb[i], 0, 0]);
    TieLightRegs { colors: [ca, cb, [0; 4]], back: [wa, wb, 0], rows }
}

/// Slot `j`'s colour before lighting: the pair of slots it shares a word with, PEXT5'd into one doubleword
/// (`pextlh`, `pext5`), shifted right by `shift` as a doubleword, then `paddub` with the bias (alpha + 0x80).
pub fn slot_base(c: &TieInstanceColours, j: usize) -> [u8; 4] {
    let lane = |k: usize| -> u64 {
        let h = c.slots.get(k).copied().unwrap_or(0) as u64;
        ((h & 0x1f) << 3) | ((h >> 5 & 0x1f) << 11) | ((h >> 10 & 0x1f) << 19) | ((h >> 15) << 31)
    };
    let pair = (lane(j & !1) | lane(j | 1) << 32) >> c.shift.min(63);
    let w = (pair >> (32 * (j & 1))) as u32;
    let bias = [c.bias[0], c.bias[1], c.bias[2], 0x80];
    std::array::from_fn(|k| ((w >> (8 * k)) as u8).saturating_add(bias[k]))
}

/// The VU0 entries 0x2c0 / 0x420 for one slot: the table normal, the dot products, `max(d, d * w)`,
/// `base + A * dA + B * dB (+ 0)`, `minii.xyz` 243; then the EE's `ppach/ppacb` keeps the low byte of each lane.
pub fn light_slot(regs: &TieLightRegs, table: &NormalTable, normal: [u8; 2], base: [u8; 4]) -> [u8; 4] {
    let [ca, sa] = table.0[normal[0] as usize];
    let [ce, se] = table.0[normal[1] as usize];
    let n = [ps2::mul(ca, ce), ps2::mul(sa, ce), se];
    let [r24, r25, r26] = regs.rows;
    let mut f = [0u32; 3];
    for (k, fk) in f.iter_mut().enumerate() {
        let d = ps2::add(ps2::add(ps2::mul(r24[k], n[0]), ps2::mul(r25[k], n[1])), ps2::mul(r26[k], n[2]));
        *fk = ps2::max(d, ps2::mul(d, regs.back[k]));
    }
    let base = base.map(|b| 0x4780_0000 + b as u32);
    let mut out = [0u8; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let mut acc = ps2::mul(base[c], ps2::ONE);
        for (l, fl) in f.iter().enumerate() { acc = ps2::add(acc, ps2::mul(regs.colors[l][c], *fl)); }
        if c < 3 { acc = ps2::min(acc, COLOR_CLAMP); }
        *o = acc as u8;
    }
    out
}

/// One instance's lit colours: its class's slots, then the self-lit colour (slot 511's; zero without one).
pub fn light_instance(class: &TieClass, inst: &TieInstance, colours: Option<&TieInstanceColours>, bank: &LightBank, table: &NormalTable) -> Vec<[u8; 4]> {
    let regs = light_regs(inst, bank);
    let none = TieInstanceColours::default();
    let c = colours.unwrap_or(&none);
    let mut out: Vec<[u8; 4]> = class.colours.normals.iter().enumerate().map(|(j, &n)| light_slot(&regs, table, n, slot_base(c, j))).collect();
    out.push(class.colours.self_lit.unwrap_or([0; 4]));
    out
}
