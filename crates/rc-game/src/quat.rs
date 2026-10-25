//! The level code's quaternions (x, y, z, w) and how they turn a moby's rows: `0x2efc20` builds the rotation matrix
//! R(q) and `0x2efa00(rows, R, rows)` multiplies each row on the left (`row · R`), which is Rᵀ — the rotation by `q`'s
//! conjugate ([`rotate`], [`turn_rows`]); `0x2efc20` alone sets the rows to R(q) ([`rows`]). Users: the break pieces'
//! spin (crate::moby::shards), the bolts' tumble (crate::moby::classes::bolt), the camera's shake turn.

use crate::hero::physics::{add, cross, dot, scale, set_len, V3};

pub type Q = [f32; 4];

pub const IDENTITY: Q = [0.0, 0.0, 0.0, 1.0];

/// `0x31fe98(a, out, axis)`: the turn by `a` about the unit `axis`: xyz = axis·sin(a/2), w = cos(a/2).
pub fn axis_angle(a: f32, axis: V3) -> Q {
    let (s, c) = (a * 0.5).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

/// `0x321bb8(s, v)`: the quaternion of axis `v` with sin(half angle) `s`.
pub fn axis_quat(s: f32, v: V3) -> Q {
    let a = set_len(v, s);
    [a[0], a[1], a[2], (1.0 - s * s).max(0.0).sqrt()]
}

/// `0x2efa98`: `a · b`.
pub fn mul(a: Q, b: Q) -> Q {
    let (av, bv) = ([a[0], a[1], a[2]], [b[0], b[1], b[2]]);
    let c = cross(av, bv);
    let v: V3 = std::array::from_fn(|k| a[3] * bv[k] + b[3] * av[k] + c[k]);
    [v[0], v[1], v[2], a[3] * b[3] - dot(av, bv)]
}

/// A row turned by `q` as `0x2efc20` / `0x2efa00` turn the rows (module docs).
pub fn rotate(q: Q, v: V3) -> V3 {
    let u = [-q[0], -q[1], -q[2]];
    let t = scale(cross(u, v), 2.0);
    add(add(v, scale(t, q[3])), cross(u, t))
}

/// `0x2efc20` + `0x2efa00(rows, R, rows)`: every row turned by `q`.
pub fn turn_rows(rows: &[V3; 3], q: Q) -> [V3; 3] { rows.map(|r| rotate(q, r)) }

/// `0x2efc20(q, rows)`: the rows R(q) (each axis turned by `q`).
pub fn rows(q: Q) -> [V3; 3] { [rotate(q, [1.0, 0.0, 0.0]), rotate(q, [0.0, 1.0, 0.0]), rotate(q, [0.0, 0.0, 1.0])] }

/// The quaternion whose [`rows`] are `r` (orthonormal): what `0x320000` (the rows' Euler angles) then `0x31d898`
/// (their quaternion) give, up to the sign, which turns the same.
pub fn from_rows(r: &[V3; 3]) -> Q {
    // The rows are the axes turned by the conjugate (row i = column i of the usual active matrix of q̄): m[i][j] =
    // r[j][i] is that matrix, whose quaternion is q̄.
    let m = |i: usize, j: usize| r[j][i];
    let tr = m(0, 0) + m(1, 1) + m(2, 2);
    let q = if 0.0 < tr {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(m(2, 1) - m(1, 2)) / s, (m(0, 2) - m(2, 0)) / s, (m(1, 0) - m(0, 1)) / s, 0.25 * s]
    } else if m(0, 0) > m(1, 1) && m(0, 0) > m(2, 2) {
        let s = (1.0 + m(0, 0) - m(1, 1) - m(2, 2)).sqrt() * 2.0;
        [0.25 * s, (m(0, 1) + m(1, 0)) / s, (m(0, 2) + m(2, 0)) / s, (m(2, 1) - m(1, 2)) / s]
    } else if m(1, 1) > m(2, 2) {
        let s = (1.0 + m(1, 1) - m(0, 0) - m(2, 2)).sqrt() * 2.0;
        [(m(0, 1) + m(1, 0)) / s, 0.25 * s, (m(1, 2) + m(2, 1)) / s, (m(0, 2) - m(2, 0)) / s]
    } else {
        let s = (1.0 + m(2, 2) - m(0, 0) - m(1, 1)).sqrt() * 2.0;
        [(m(0, 2) + m(2, 0)) / s, (m(1, 2) + m(2, 1)) / s, 0.25 * s, (m(1, 0) - m(0, 1)) / s]
    };
    normalize([-q[0], -q[1], -q[2], q[3]])
}

/// `0x2efad8(t, out, a, b)`: `a·(1 − t) + b·t` (b negated when the two are more than a half turn apart), normalized.
pub fn nlerp(a: Q, b: Q, t: f32) -> Q {
    let d: f32 = (0..4).map(|k| a[k] * b[k]).sum();
    let k = if d < 0.0 { -t } else { t };
    normalize(std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * k))
}

fn normalize(q: Q) -> Q {
    let l = q.iter().map(|x| x * x).sum::<f32>().sqrt();
    if 0.0 < l { q.map(|x| x / l) } else { IDENTITY }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[V3; 3], b: &[V3; 3]) -> bool { (0..3).all(|i| (0..3).all(|j| (a[i][j] - b[i][j]).abs() < 1e-5)) }

    #[test]
    fn rows_round_trip() {
        for (a, axis) in [(0.7, [0.0, 0.0, 1.0]), (2.9, [0.6, 0.0, 0.8]), (-1.3, [0.0, 1.0, 0.0]), (3.1, [1.0, 0.0, 0.0])] {
            let q = axis_angle(a, axis);
            assert!(close(&rows(from_rows(&rows(q))), &rows(q)));
        }
    }

    #[test]
    fn turn_rows_is_row_times_r() {
        let q = axis_angle(0.4, [0.0, 0.6, 0.8]);
        let r = rows(q);
        let a = rows(axis_angle(1.1, [1.0, 0.0, 0.0]));
        let by_matrix: [V3; 3] = a.map(|row| std::array::from_fn(|k| row[0] * r[0][k] + row[1] * r[1][k] + row[2] * r[2][k]));
        assert!(close(&turn_rows(&a, q), &by_matrix));
    }

    #[test]
    fn nlerp_ends() {
        let (a, b) = (axis_angle(0.3, [0.0, 0.0, 1.0]), axis_angle(-2.0, [1.0, 0.0, 0.0]));
        assert!(close(&rows(nlerp(a, b, 1.0)), &rows(b)));
        assert!(close(&rows(nlerp(a, b, 0.0)), &rows(a)));
    }
}
