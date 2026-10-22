//! The particle pool and allocator (R&C1's, which Going Commando keeps: 2048 records of 0x40 bytes; R&C2 `0x330df0`
//! creates, `0x330f40` kills), the record accessors, the def table, and the update steps both games share (type 23,
//! the glow puff = R&C2's 0x17; type 48, the drifting particle = R&C2's 0x30). The update scheduler is
//! crate::particles::update_parts.
//!
//! * **Pool**: allocation bitmap, lowest-free hint, highest-ever-live index ("hw", −1 when empty), live count.
//!   [`PartPool::level_init`] resets the counters and bitmaps but **not** the records.
//! * **[`PartPool::create_part`]**: takes the hint slot, sets its bit, `hw = max(hw, i)` (`pmaxw`), new hint = `i + 1`
//!   when `i > old hw`, else the first clear bit scanning from bit 0 of the byte holding `i + 1` (0x800 when none).
//!   Record: `sq zero, 0x0` then **three** `sq zero, 0x10`, so bytes 0x20..0x3f keep the previous occupant's data;
//!   byte 0 = type.
//! * **[`PartPool::kill_part`]**: count−−; clear the bitmap-2 bit if byte1 & 0x40; clear the bit; if `i == hw`, hw =
//!   the next lower set bit (−1 if none); byte1 = 0x80; hint = `min(hint, i)` (`pminw`).
//!
//! **Floats** follow the EE FPU / VU0 model of [`crate::ps2v`] (round toward zero, no denormals).

use crate::ps2v::{self, F};
use rc_formats::particle_tex::PartDefs;

pub const POOL_RECORDS: usize = 2048;
pub(crate) const RECORD_SIZE: usize = 0x40;
/// One raw particle record.
pub(crate) type Record = [u8; RECORD_SIZE];

/// Byte-level accessors on a record (little-endian, PS2 float bits for `f`).
pub mod rec {
    use super::Record;
    #[inline]
    pub fn u32(r: &Record, o: usize) -> u32 { u32::from_le_bytes(r[o..o + 4].try_into().unwrap()) }
    #[inline]
    pub(crate) fn set_u32(r: &mut Record, o: usize, v: u32) { r[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
    #[inline]
    pub(crate) fn i16(r: &Record, o: usize) -> i16 { i16::from_le_bytes([r[o], r[o + 1]]) }
    #[inline]
    pub(crate) fn set_i16(r: &mut Record, o: usize, v: i16) { r[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
    /// Position (+0x10, xyz) as `f32`.
    pub fn pos(r: &Record) -> [f32; 3] { [0x10, 0x14, 0x18].map(|o| f32::from_bits(u32(r, o))) }
    /// An `f32` field (native arithmetic; the value is stored as its bits).
    #[inline]
    pub fn ff(r: &Record, o: usize) -> f32 { f32::from_bits(u32(r, o)) }
    #[inline]
    pub(crate) fn set_ff(r: &mut Record, o: usize, v: f32) { set_u32(r, o, v.to_bits()); }
    /// Three `f32` at `o` (a vector's xyz).
    pub fn v3(r: &Record, o: usize) -> [f32; 3] { [ff(r, o), ff(r, o + 4), ff(r, o + 8)] }
    pub(crate) fn set_v3(r: &mut Record, o: usize, v: [f32; 3]) { for (k, x) in v.iter().enumerate() { set_ff(r, o + 4 * k, *x); } }
    /// Four `f32` at `o` (a vector's xyzw).
    pub(crate) fn v4(r: &Record, o: usize) -> [f32; 4] { [ff(r, o), ff(r, o + 4), ff(r, o + 8), ff(r, o + 12)] }
    /// Four `f32` at `o`.
    pub(crate) fn set_v4(r: &mut Record, o: usize, v: [f32; 4]) { for (k, x) in v.iter().enumerate() { set_ff(r, o + 4 * k, *x); } }
}

/// Record byte 1 flags.
pub(crate) const FLAG_DEAD: u8 = 0x80;
pub(crate) const FLAG_BITMAP2: u8 = 0x40;
/// Set by `PartProc` on records it drew this frame, cleared when a previously drawn record is culled.
pub const FLAG_DRAWN: u8 = 0x20;

/// The 2048-record pool with the game's allocator state.
#[derive(Clone)]
pub struct PartPool {
    pub recs: Box<[Record]>,
    /// 0x1b1d00: bit i = record i allocated.
    pub(crate) bitmap: [u8; 256],
    /// 0x1b1e00: only touched for records with byte1 & 0x40 (never in retail).
    pub(crate) bitmap2: [u8; 256],
    /// gp−0x6a90: lowest free index (0x800 = full).
    pub(crate) hint: i32,
    /// gp−0x6a8c: highest index ever live since the last kill of the top record, −1 when empty.
    pub hw: i32,
    /// gp−0x6a88: live records.
    pub count: i32,
}

impl Default for PartPool {
    fn default() -> Self { Self::new() }
}

impl PartPool {
    /// A pool in the level-init state. The records start as zero bytes: on the PS2 the pool is placed after
    /// other level data and its previous contents are unknown (level init does not clear it).
    pub(crate) fn new() -> Self {
        PartPool { recs: vec![[0u8; RECORD_SIZE]; POOL_RECORDS].into_boxed_slice(), bitmap: [0; 256], bitmap2: [0; 256], hint: 0, hw: -1, count: 0 }
    }

    /// Level init 0x255958: hint 0, hw −1, count 0, both bitmaps zero; records untouched.
    pub fn level_init(&mut self) {
        self.hint = 0;
        self.hw = -1;
        self.count = 0;
        self.bitmap = [0; 256];
        self.bitmap2 = [0; 256];
    }

    fn bit(&self, i: usize) -> bool { self.bitmap[i >> 3] & (1 << (i & 7)) != 0 }

    /// `CreatePart(type)` 0x27c498/0x27c4a0: the record index, or `None` when the pool is full.
    pub(crate) fn create_part(&mut self, ty: u8) -> Option<usize> {
        let i = self.hint;
        if i - 0x800 >= 0 { return None; }
        let iu = i as usize;
        self.bitmap[iu >> 3] |= 1 << (iu & 7);
        let mut next = i + 1;
        if i - self.hw < 1 {
            // Scan from bit 0 of the byte holding i + 1 for the first clear bit.
            let mut byte = (next as u32 >> 3) as usize;
            next = (byte << 3) as i32;
            'scan: while next - 0x800 < 0 {
                let mut m: u32 = 1;
                loop {
                    let set = self.bitmap[byte] as u32 & m;
                    m <<= 1;
                    if set == 0 { break 'scan; }
                    next += 1;
                    if m & 0xff == 0 { break; }
                }
                byte += 1;
            }
        }
        self.hint = next;
        self.hw = self.hw.max(i); // pmaxw
        self.count += 1;
        let r = &mut self.recs[iu];
        r[..0x10].fill(0); // sq zero, 0x0(v0)
        for _ in 0..3 { r[0x10..0x20].fill(0); } // sq zero, 0x10(v0) three times
        r[0] = ty;
        Some(iu)
    }

    /// `KillPart(rec)` 0x27c5e8.
    pub(crate) fn kill_part(&mut self, i: usize) {
        self.count -= 1;
        let m = 1u8 << (i & 7);
        if self.recs[i][1] & FLAG_BITMAP2 != 0 { self.bitmap2[i >> 3] &= !m; }
        self.bitmap[i >> 3] &= !m;
        if i as i32 == self.hw {
            let mut h = self.hw - 1;
            while h >= 0 && !self.bit(h as usize) { h -= 1; }
            self.hw = h.max(-1);
        }
        self.recs[i][1] = FLAG_DEAD;
        self.hint = self.hint.min(i as i32); // pminw
    }

}

/// Time base (`fun_00214970`): 0x15ed60 speed, 0x15ed68 timer scale and 0x15ed70 dt².
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TimeBase {
    /// 0x15ed60 = gp−0x7ea0: 1.0 NTSC, 1.2 PAL.
    pub(crate) speed: F,
    /// 0x15ed68 = gp−0x7e98: 1.0 NTSC, 0.8333 PAL.
    pub(crate) timer_scale: F,
    /// 0x15ed70 = gp−0x7e90: dt² (1/3600 NTSC = 0x3991a2b4, 1/2500 PAL).
    pub(crate) dt2: F,
}

impl TimeBase {
    pub(crate) const NTSC: TimeBase = TimeBase { speed: 0x3f80_0000, timer_scale: 0x3f80_0000, dt2: 0x3991_a2b4 };

    /// 0x220e30: `cvt.w.s(0.25 + 0.25 + (f32)n · timer_scale)` (`adda.s` then `madd.s`).
    pub(crate) fn ticks(&self, n: i32) -> i32 {
        let acc = ps2v::add(0x3e80_0000, 0x3e80_0000);
        ps2v::ftoi0(ps2v::add(acc, ps2v::mul(ps2v::itof0(n), self.timer_scale)))
    }
}

/// `FastDecTimer__FRs` 0x220ea8 on the s16 at `r[o]`: 1 if it is 0 (not written), else t = max(t, 1) − 1 and
/// 2 when t ≤ 0 (the kill signal), 0 otherwise.
pub(crate) fn fast_dec_timer(r: &mut Record, o: usize) -> i32 {
    let t = rec::i16(r, o) as i32;
    if t == 0 { return 1; }
    let t = t.max(1) - 1;
    rec::set_i16(r, o, t as i16);
    if t > 0 { 0 } else { 2 }
}

/// `FastTweenColor(f, a, b)` 0x2221a8 on VU0: per byte (R low) `ftoi0(itof0(a)·(1 − f) + itof0(b)·f)` (`vsubx.w`
/// 1 − f, `vmulaw` then `vmaddx`), packed back with `ppach`/`ppacb` (the low byte of each lane).
pub(crate) fn tween_color(f: F, a: u32, b: u32) -> u32 {
    let g = ps2v::sub(ps2v::ONE, f);
    (0..4).fold(0u32, |out, k| {
        let (ca, cb) = (ps2v::itof0((a >> (8 * k) & 0xff) as i32), ps2v::itof0((b >> (8 * k) & 0xff) as i32));
        let v = ps2v::ftoi0(ps2v::add(ps2v::mul(ca, g), ps2v::mul(cb, f)));
        out | (v as u32 & 0xff) << (8 * k)
    })
}

/// The particle system state the game keeps in globals.
pub struct Particles {
    pub pool: PartPool,
    pub(crate) time: TimeBase,
    /// `part_defs`: spawners take `*def[n]` as the first frame.
    pub defs: Option<PartDefs>,
}

impl Particles {
    /// Level-init state.
    pub fn new(defs: Option<PartDefs>) -> Self { Particles { pool: PartPool::new(), time: TimeBase::NTSC, defs } }

    pub(crate) fn create_part(&mut self, ty: u8) -> Option<usize> { self.pool.create_part(ty) }

    pub(crate) fn kill_part(&mut self, i: usize) { self.pool.kill_part(i); }

    /// `*def[n]` (0 when the level has no defs).
    pub(crate) fn def_first(&self, n: u8) -> u8 { self.defs.as_ref().and_then(|d| d.first_frame(n as usize)).unwrap_or(0) }

    /// `*(def[n] + k)` (0 when the level has no defs).
    pub(crate) fn def_frame(&self, n: u8, k: usize) -> u8 { self.defs.as_ref().and_then(|d| d.start(n as usize).and_then(|s| d.blob.get(s + k).copied())).unwrap_or(0) }
}

/// The view data `FastBSphereCheck` (level01 0x2221f0) reads from 0x16d140.., written by the camera matrix
/// builder (`fun_001f2260`, 0x218a48) and `UpdateViewContext` (0x219580) during the previous frame's render.
#[derive(Clone, Copy, Debug)]
pub struct BSphereView {
    /// 0x16d140/150/160 = the rotation-only view 0x167100, row j = camera-space image of world axis j
    /// (camera x right = −left, y down = −up, z forward).
    pub(crate) rows: [[F; 3]; 3],
    /// 0x16d170 = camera position 0x167240 × 1024 (`fun_001f9a68(1024, ...)`), w = 1024.
    pub(crate) cam: [F; 3],
    /// 0x16d180.xy = (tan_x, tan_y) = (0.63, 0.63·0.775).
    pub(crate) tan: [F; 2],
    /// 0x16d1a0.xy = 1/cos(atan(tan)) (`FastArcTan` + `fast_cos` on the PS2; √(1 + tan²) here).
    pub(crate) sec: [F; 2],
}

impl BSphereView {
    /// From a camera given as game-space eye and (forward, left, up) rows.
    pub fn from_camera(eye: [f32; 3], forward: [f32; 3], left: [f32; 3], up: [f32; 3], tan_x: f32, tan_y: f32) -> Self {
        let rows = [0, 1, 2].map(|j| [(-left[j]).to_bits(), (-up[j]).to_bits(), forward[j].to_bits()]);
        let cam = eye.map(|v| ps2v::mul(v.to_bits(), ps2v::K1024));
        let sec = [tan_x, tan_y].map(|t| (1.0 + t * t).sqrt().to_bits());
        BSphereView { rows, cam, tan: [tan_x.to_bits(), tan_y.to_bits()], sec }
    }

    /// `FastBSphereCheck(far, sphere) == −1` (outside): beyond `far` in view depth, entirely behind the eye
    /// (`c.z + r ≤ 0`, as the sign bit of `0 − (r + c.z)` is clear), or outside a side plane
    /// (`tan·c.z − (|c| − r·sec) < 0` in x or y). The 0/1 distinction (intersecting / fully inside, using
    /// 0x16d190/0x16d1b0) is not modelled: its callers here only test for −1. Everything is in integer
    /// units (×1024) with the VU0 operation order of 0x2221f0..0x2222c8.
    pub(crate) fn culled(&self, far: f32, sphere: [f32; 4]) -> bool {
        use ps2v::{add, mul, neg, sub};
        let w = ps2v::K1024;
        let v1 = sphere.map(|x| mul(x.to_bits(), w)); // vmulw.xyzw vf1, vf1, vf24
        let d = [0, 1, 2].map(|k| sub(v1[k], self.cam[k])); // vsub.xyz vf2, vf1, vf24
        let (r, nr) = (add(0, v1[3]), sub(0, v1[3])); // vaddw.x / vsubw.y vf3, vf0, vf1
        let c = [0, 1, 2].map(|k| add(add(mul(self.rows[0][k], d[0]), mul(self.rows[1][k], d[1])), mul(self.rows[2][k], d[2])));
        let far_i = mul(far.to_bits(), w); // vmulw.y vf19, vf4, vf24
        let (z_plus, z_minus) = (add(r, c[2]), add(nr, c[2])); // vaddz.xy vf1, vf3, vf2
        let behind = sub(0, z_plus); // vsub.xy vf3, vf19, vf1 (x lane: vf19.x = 0)
        let beyond = sub(far_i, z_minus);
        if neg(beyond) || !neg(behind) { return true; }
        for k in 0..2 {
            let t = mul(self.tan[k], c[2]); // vmulz.xy vf5, vf20, vf2
            let rs = mul(self.sec[k], r); // vmulw.xy vf8, vf22, vf1
            let side = sub(t, sub(c[k] & ps2v::MAX, rs)); // vsub vf7 = vf5 − (|c| − vf8)
            if neg(side) { return true; }
        }
        false
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_takes_lowest_free_and_tracks_hw() {
        let mut p = PartPool::new();
        assert_eq!((p.hint, p.hw, p.count), (0, -1, 0));
        for k in 0..5 { assert_eq!(p.create_part(6), Some(k)); }
        assert_eq!((p.hint, p.hw, p.count), (5, 4, 5));
        p.kill_part(1);
        p.kill_part(3);
        assert_eq!((p.hint, p.hw, p.count), (1, 4, 3));
        assert_eq!(p.recs[3][1], FLAG_DEAD);
        // Refill hole 1: 1 <= hw, so the new hint is the first clear bit from bit 0 of byte (2 >> 3) = 0 → 3.
        assert_eq!(p.create_part(7), Some(1));
        assert_eq!(p.hint, 3);
        assert_eq!(p.create_part(7), Some(3));
        assert_eq!(p.hint, 5);
        // Killing the top record walks hw down past dead slots.
        p.kill_part(4);
        assert_eq!(p.hw, 3);
        for k in [3, 2, 1, 0] { p.kill_part(k); }
        assert_eq!((p.hint, p.hw, p.count), (0, -1, 0));
    }

    #[test]
    fn allocator_scan_crosses_full_bytes_and_reports_full() {
        let mut p = PartPool::new();
        for k in 0..20 { assert_eq!(p.create_part(1), Some(k)); }
        p.kill_part(2);
        assert_eq!(p.create_part(1), Some(2));
        // Bytes 0 and 1 are full, byte 2 has bits 0..3 set: the scan ends at 20.
        assert_eq!(p.hint, 20);
        let mut q = PartPool::new();
        for _ in 0..POOL_RECORDS { assert!(q.create_part(0).is_some()); }
        assert_eq!(q.hint, 0x800);
        assert_eq!(q.create_part(0), None);
        q.kill_part(1000);
        assert_eq!(q.create_part(0), Some(1000));
        assert_eq!(q.hint, 0x800, "no clear bit left: the scan runs to 0x800");
    }

    #[test]
    fn create_clears_only_the_first_half() {
        let mut p = PartPool::new();
        let i = p.create_part(6).unwrap();
        p.recs[i] = [0xab; RECORD_SIZE];
        p.kill_part(i);
        let j = p.create_part(9).unwrap();
        assert_eq!(i, j);
        assert_eq!(p.recs[j][0], 9);
        assert!(p.recs[j][1..0x20].iter().all(|&b| b == 0));
        assert!(p.recs[j][0x20..].iter().all(|&b| b == 0xab), "0x20..0x3f are stale");
    }


    #[test]
    fn dec_timer_and_ticks() {
        let mut r = [0u8; RECORD_SIZE];
        rec::set_i16(&mut r, 10, 2);
        assert_eq!(fast_dec_timer(&mut r, 10), 0);
        assert_eq!(fast_dec_timer(&mut r, 10), 2);
        assert_eq!(rec::i16(&r, 10), 0);
        assert_eq!(fast_dec_timer(&mut r, 10), 1);
        rec::set_i16(&mut r, 10, -5);
        assert_eq!(fast_dec_timer(&mut r, 10), 2);
        assert_eq!(rec::i16(&r, 10), 0);
        assert_eq!(TimeBase::NTSC.ticks(150), 150);
        assert_eq!(TimeBase::NTSC.ticks(0), 0);
    }

    #[test]
    fn bsphere_check_depth_and_sides() {
        // Camera at the origin looking along +X (forward X, left Y, up Z).
        let v = BSphereView::from_camera([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775);
        assert!(!v.culled(240.0, [100.0, 0.0, 0.0, 20.0]));
        assert!(!v.culled(240.0, [259.0, 0.0, 0.0, 20.0]), "near side of the sphere within 240");
        assert!(v.culled(240.0, [261.0, 0.0, 0.0, 20.0]));
        assert!(v.culled(240.0, [-30.0, 0.0, 0.0, 20.0]), "behind");
        assert!(!v.culled(240.0, [-10.0, 0.0, 0.0, 20.0]), "straddles the eye");
        // Left of the frustum: y = 0.63·100 + 20·√(1+0.63²) + 1 → outside; −1 → inside.
        let edge = 63.0 + 20.0 * (1.0f32 + 0.63 * 0.63).sqrt();
        assert!(v.culled(240.0, [100.0, edge + 1.0, 0.0, 20.0]));
        assert!(!v.culled(240.0, [100.0, edge - 1.0, 0.0, 20.0]));
    }
}
