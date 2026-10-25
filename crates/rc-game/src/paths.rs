//! The grind paths' spline kernels (L00): the walk along a path `0x342b50`, the next point `0x342ae8`, the closest
//! point `0x3432d8` (with `0x343038` per segment and the distance `0x342db8`), and the search for a path near a
//! point `0x2ad8e0`. A path is points `{x, y, z, the length to the next}`; a position on it is a segment and the
//! distance along that segment.

pub use rc_formats::disc::gameplay::GrindPath;

type V3 = [f32; 3];

fn p3(p: &[f32; 4]) -> V3 { [p[0], p[1], p[2]] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: V3, k: f32) -> V3 { [a[0] * k, a[1] * k, a[2] * k] }
fn len(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn len2(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1]).sqrt() }
fn set_len(a: V3, l: f32) -> V3 {
    let m = len(a);
    if m == 0.0 { a } else { scale(a, l / m) }
}

/// `0x343fb8(t, path, looped, &pos, &angles, flags)` (L01 `0x33a340`): the pose at `t` along the points (between
/// points ⌊t⌋ and ⌊t⌋ + 1; an open path holds its last segment up to its end, a looped one wraps): the place, the
/// direction's pitch (−atan2(|dxy|, dz)) and yaw, the direction blended from this segment's into the next's (an open
/// path's last: its own), and the points' w blended (the roll the callers that use it take).
pub fn pose(points: &[[f32; 4]], t: f32, looped: bool) -> (V3, f32, f32, f32) {
    let n = points.len() as i32;
    let mut i = t as i32;
    let end = !looped && n - 2 <= i;
    if end { i = n - 2; }
    let mut f = t - i as f32;
    if end && 1.0 < f { f = 1.0; }
    let at = |k: i32| points[k.rem_euclid(n.max(1)) as usize];
    let (a, b) = (at(i), at(i + 1));
    let pos = add(p3(&a), scale(sub(p3(&b), p3(&a)), f));
    let d0 = sub(p3(&b), p3(&a));
    let dir = if end { d0 } else { add(d0, scale(sub(sub(p3(&at(i + 2)), p3(&b)), d0), f)) };
    let pitch = -crate::hero::physics::atan2(len2(dir), dir[2]);
    let yaw = crate::hero::physics::atan2(dir[0], dir[1]);
    (pos, pitch, yaw, a[3] * (1.0 - f) + b[3] * f)
}

/// `0x342b50(dist, path, &out, &seg, &t, looped)`: `dist` (signed) along the path from `(seg, t)`; `out` the point
/// reached. Returns whether it ran off an end (an open path's last point; a loop's wrap).
pub fn advance(points: &[[f32; 4]], dist: f32, seg: &mut usize, t: &mut f32, looped: bool) -> (V3, bool) {
    let n = points.len();
    if n == 0 { return ([0.0; 3], true); }
    let dir: i32 = if dist < 0.0 { -1 } else { 1 };
    let mut d = dist.abs();
    let mut end = false;
    loop {
        let i = (*seg).min(n - 1);
        if d == 0.0 || (i == n - 1 && !looped) {
            let mut out = p3(&points[i]);
            if i == n - 1 && !looped {
                end = true;
                if dir == -1 {
                    *seg = 0;
                    out = p3(&points[0]);
                    *t = 0.0;
                }
            } else {
                let j = (i + 1) % n;
                out = add(out, set_len(sub(p3(&points[j]), p3(&points[i])), *t));
            }
            return (out, end);
        }
        let over = if dir == 1 { points[i][3] < *t + d } else { *t < d };
        let mut i2 = i as i32 + dir;
        if over {
            if i2 >= n as i32 || i2 < 0 { i2 = (i2 + n as i32) % n as i32; }
            if dir == 1 {
                let rem = d - (points[i][3] - *t);
                *t = 0.0;
                d = rem.max(0.0);
            } else {
                d -= *t;
                *t = points[i2 as usize][3];
            }
            if (dir == 1 && i == n - 1) || (dir == -1 && i == 0) { end = true; }
            *seg = i2 as usize;
        } else {
            *t += dir as f32 * d;
            d = 0.0;
        }
    }
}

/// `0x342ae8(path, i, k, looped)`: the point `k` after `i` (clamped to the ends, or wrapped).
pub fn next_index(n: usize, i: usize, k: i32, looped: bool) -> usize {
    if n == 0 { return 0; }
    if looped { return ((i as i32 + k + n as i32) % n as i32) as usize; }
    (i as i32 + k).clamp(0, n as i32 - 1) as usize
}

/// `0x342db8(band, a, b)`: across only when the heights are within `band` (band > 0), else the full distance.
fn dist_band(band: f32, a: V3, b: V3) -> f32 {
    if 0.0 < band && (a[2] - b[2]).abs() < band { len2(sub(a, b)) } else { len(sub(a, b)) }
}

/// `0x343038(band, &out, p, a, b)`: the point of the segment a–b closest to p (across, the height interpolated,
/// when p is within `band` of both ends' heights); returns its distance.
fn closest_on(band: f32, p: V3, a: V3, b: V3) -> (V3, f32) {
    let q = if 0.0 < band && (p[2] - a[2]).abs() < band && (p[2] - b[2]).abs() < band {
        let ab = [b[0] - a[0], b[1] - a[1], 0.0];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1];
        let f = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) };
        add(a, scale(sub(b, a), f))
    } else {
        let ab = sub(b, a);
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let f = if l2 == 0.0 { 0.0 } else { ((sub(p, a)[0] * ab[0] + sub(p, a)[1] * ab[1] + sub(p, a)[2] * ab[2]) / l2).clamp(0.0, 1.0) };
        add(a, scale(ab, f))
    };
    (q, dist_band(0.0, p, q))
}

/// `0x3432d8(max, step, band, path, p, &out, &seg, &t, looped)`: the path's point closest to `p` — its segment and
/// the distance along it — when within `max + step`.
pub fn closest(points: &[[f32; 4]], max: f32, step: f32, band: f32, p: V3, looped: bool) -> Option<(V3, usize, f32)> {
    let n = points.len();
    if n == 0 { return None; }
    let segs = if looped { n } else { n.saturating_sub(1) };
    let mut best: Option<(V3, usize, f32)> = None;
    let mut best_d = f32::MAX;
    if segs == 0 {
        let q = p3(&points[0]);
        let d = dist_band(band, p, q);
        return (d <= max + step).then_some((q, 0, 0.0));
    }
    for i in 0..segs {
        let (a, b) = (p3(&points[i]), p3(&points[(i + 1) % n]));
        let (q, _) = closest_on(band, p, a, b);
        let d = dist_band(band, p, q);
        if d < best_d {
            best_d = d;
            best = Some((q, i, len(sub(q, a))));
        }
    }
    if max + step < best_d { return None; }
    best
}

/// A path found near a point (`0x2ad8e0`'s outputs).
#[derive(Clone, Copy, Debug)]
pub struct RailHit {
    pub path: usize,
    pub point: V3,
    pub seg: usize,
    pub t: f32,
    pub looped: bool,
}

/// `0x2ad8e0(p, …, exclude, only)`: a path whose sphere holds `p` with its closest point within `reach` across and
/// `height` up or down (and, unless `no_floor`, a floor under the point: a line from 0.25 above to 1 below hits).
pub fn find_rail(paths: &[GrindPath], p: V3, exclude: Option<usize>, reach: f32, height: f32, floor: &dyn Fn(V3, V3) -> bool) -> Option<RailHit> {
    for (i, g) in paths.iter().enumerate() {
        if exclude == Some(i) || g.points.is_empty() { continue; }
        if g.sphere[3] < len(sub(p3(&g.sphere), p)) { continue; }
        let Some((q, seg, t)) = closest(&g.points, 12.0, 10.0, 0.0, p, g.looped) else { continue };
        let has_floor = floor(add(q, [0.0, 0.0, 0.25]), add(q, [0.0, 0.0, -1.0]));
        if len2(sub(p, q)) < reach && (p[2] - q[2]).abs() < height && has_floor {
            return Some(RailHit { path: i, point: q, seg, t, looped: g.looped });
        }
    }
    None
}
