//! The spline follower: the game's polyline splines (gameplay sections 0x70 "paths", `0x1b0930[i]`, and 0x74
//! "grind paths", `0x15f70c[i]`) and the three library functions every rider uses — the grind rails and the cable
//! of the hero (`hero::boots`), and the moby classes that ride a path. Level00 addresses (the grind code only
//! exists in the overlays with rails; the same functions are level01 0x272660 / 0x2726c8 / 0x272e28):
//!
//! | game | here | what |
//! |---|---|---|
//! | `0x25d7a0` | [`step_index`] | the next / previous point index (clamped on an open spline, wrapped on a closed one) |
//! | `0x25d808` (doc name `SplineProject`) | [`advance`] | move a cursor `(segment, distance into it)` along the spline by a signed distance and return the point |
//! | `0x25df68` (doc name `SplineSample`) | [`nearest`] | the point of the spline nearest to a position: a coarse walk in steps, then the exact point on the two segments around the nearest vertex |
//! | `0x25da70` | [`band_distance`] | the distance the search uses: horizontal inside a vertical band, 3-D outside it |
//! | `0x25dcd8` / `0x25db00` | [`closest_on_segment`] | the nearest point of one segment (2-D inside the band, the height interpolated; else 3-D) |
//!
//! (The two doc names were coined from the call sites before the functions were read; what they do is the other
//! way round. The port names them by what they do.)
//!
//! **Data.** A spline is `count` points `(x, y, z, w)`; **w is the length of the segment to the next point** (the
//! last point's w is the distance back to the first, used when the spline is closed). The grind-path header's
//! word +0x14 (`rc_formats::volumes::GrindPath::flag`) is the closed flag every call passes (1 = closed loop).
//! Checked on every level's grind paths: w equals the chord to the next point (`tests`, and
//! `tests/hero/hero_boots_grind.rs`).
//!
//! **Quirks kept** (they decide where a rider ends up):
//! * [`advance`] backwards past the first point of an open spline wraps the index to the last point and then
//!   returns the first point with the cursor at `(0, 0)`, reporting the end; forwards it stops at the last point
//!   (cursor on the last point, which has no segment of its own). A closed spline reports the wrap of the last
//!   segment as an end too, but keeps going.
//! * [`nearest`] fails (the caller keeps its old cursor and point) when no coarse sample lies within
//!   `range + step`; the coarse walk keeps the *first* of equally near samples.
//!
//! Standard `f32` arithmetic and `f32::sqrt` (the game's VU0 macro code; no result depends on its last bit).
#![allow(clippy::neg_cmp_op_on_partial_ord)] // The game's compares, NaN included, are spelled out on purpose.

/// One spline point: position and the length of the segment to the next point.
pub(crate) type Point = [f32; 4];

/// A position on a spline: segment `seg` (from point `seg` to the next), `t` units along it (0x13f8b4 / 0x13f8b8
/// for the hero's rail).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    pub(crate) seg: i32,
    pub(crate) t: f32,
}

fn xyz(p: Point) -> [f32; 3] { [p[0], p[1], p[2]] }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: [f32; 3], k: f32) -> [f32; 3] { [a[0] * k, a[1] * k, a[2] * k] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
/// 3-D length.
pub(crate) fn len3(a: [f32; 3]) -> f32 { dot(a, a).sqrt() }
/// `FastVecNormalize(len, v)`: `v` scaled to length `len`, zero for a zero vector.
fn set_len(v: [f32; 3], len: f32) -> [f32; 3] {
    let l = len3(v);
    if l == 0.0 { [0.0; 3] } else { scale(v, len / l) }
}

/// `0x25d808(dist, spline, &out, &seg, &t, closed)`: move `c` by `dist` along the spline (negative: backwards)
/// and return the point there and whether an end was reached (module doc for the ends).
pub(crate) fn advance(pts: &[Point], closed: bool, dist: f32, c: &mut Cursor) -> ([f32; 3], bool) {
    let n = pts.len() as i32;
    if n == 0 { return ([0.0; 3], true); }
    let (mut d, dir) = if dist < 0.0 { (-dist, -1) } else { (dist, 1) };
    let mut ended = false;
    // A spline of zero-length segments would never end on a closed loop (the game would hang): give up after
    // enough crossings.
    let mut guard = 4 * n as usize + 64;
    loop {
        let i = c.seg.clamp(0, n - 1);
        let at_end = i == n - 1 && !closed;
        if d == 0.0 || at_end || guard == 0 {
            if at_end {
                if dir == -1 {
                    c.seg = 0;
                    c.t = 0.0;
                    return (xyz(pts[0]), true);
                }
                return (xyz(pts[i as usize]), true);
            }
            let j = (i + 1).rem_euclid(n);
            let p = xyz(pts[i as usize]);
            let q = xyz(pts[j as usize]);
            return (add(p, set_len(sub(q, p), c.t)), ended);
        }
        guard -= 1;
        let t = c.t;
        let cross = if dir == 1 { pts[i as usize][3] < t + d } else { t < d };
        if !cross {
            c.t = if dir == 1 { t + d } else { t - d };
            d = 0.0;
            continue;
        }
        let mut j = i + dir;
        if j >= n || j < 0 { j = (j + n).rem_euclid(n); }
        let nd = if dir == 1 {
            let rem = d - (pts[i as usize][3] - t);
            c.t = 0.0;
            if 0.0 <= rem { rem } else { 0.0 }
        } else {
            c.t = pts[j as usize][3];
            d - t
        };
        if (dir == 1 && i == n - 1) || (dir == -1 && i == 0) { ended = true; }
        c.seg = j;
        d = nd;
    }
}

