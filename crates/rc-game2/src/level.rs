//! The level's data the objects read besides their own: the volumes (L00 `0x1b1f0c` cuboids, `0x1b1f14` spheres,
//! `0x1b1f1c` cylinders; the gameplay's sections 26..28) and the paths (`0x1fda00`, section 30).

pub use rc_formats::rac2::gameplay::Volume;

type V3 = [f32; 3];

/// The level data (built once from the gameplay file).
#[derive(Clone, Debug, Default)]
pub struct LevelData {
    pub cuboids: Vec<Volume>,
    pub spheres: Vec<Volume>,
    pub cylinders: Vec<Volume>,
    /// The paths' points `{x, y, z, the length to the next}`.
    pub paths: Vec<Vec<[f32; 4]>>,
}

impl LevelData {
    /// From the gameplay file (missing sections: empty).
    pub fn load(gameplay: &[u8]) -> LevelData {
        use rc_formats::rac2::gameplay as g;
        let v = |n: &str| g::volumes(gameplay, n).map_err(|e| eprintln!("rac2: {n}: {e}")).unwrap_or_default();
        LevelData { cuboids: v("cuboids"), spheres: v("spheres"), cylinders: v("cylinders"), paths: g::paths(gameplay).unwrap_or_default() }
    }

    /// `0x321858(p, i)`: `p` inside the cuboid `i` (−1, or out of range: never).
    pub fn in_cuboid(&self, i: i32, p: V3) -> bool {
        let Some(c) = usize::try_from(i).ok().and_then(|i| self.cuboids.get(i)) else { return false };
        let d = [p[0] - c.matrix[3][0], p[1] - c.matrix[3][1], p[2] - c.matrix[3][2]];
        // `0x2ef318(out, d, inverse)`: the rows of the 3×4 as columns (the VU0 matrix · vector).
        let m = &c.inverse;
        let l = [
            d[0] * m[0][0] + d[1] * m[1][0] + d[2] * m[2][0],
            d[0] * m[0][1] + d[1] * m[1][1] + d[2] * m[2][1],
            d[0] * m[0][2] + d[1] * m[1][2] + d[2] * m[2][2],
        ];
        l.iter().all(|x| (-1.0..=1.0).contains(x))
    }

    /// A path with its segment lengths filled in (the objects that walk one fill `w` so, the last to the first).
    pub fn walked_path(&self, i: i32) -> Vec<[f32; 4]> {
        let Some(p) = usize::try_from(i).ok().and_then(|i| self.paths.get(i)) else { return Vec::new() };
        let n = p.len();
        (0..n)
            .map(|k| {
                let (a, b) = (p[k], p[(k + 1) % n]);
                let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                [a[0], a[1], a[2], d]
            })
            .collect()
    }

    /// A cuboid's `+0x78` word (its yaw, for the objects that face along it).
    pub fn cuboid_yaw(&self, i: i32) -> f32 { usize::try_from(i).ok().and_then(|i| self.cuboids.get(i)).map_or(0.0, |c| c.extra[2]) }
}
