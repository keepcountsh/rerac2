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
    /// The trigger areas (`0x1b1a80`).
    pub areas: Vec<rc_formats::rac2::gameplay::Area>,
}

impl LevelData {
    /// From the gameplay file (missing sections: empty).
    pub fn load(gameplay: &[u8]) -> LevelData {
        use rc_formats::rac2::gameplay as g;
        let v = |n: &str| g::volumes(gameplay, n).map_err(|e| eprintln!("rac2: {n}: {e}")).unwrap_or_default();
        LevelData {
            cuboids: v("cuboids"),
            spheres: v("spheres"),
            cylinders: v("cylinders"),
            paths: g::paths(gameplay).unwrap_or_default(),
            areas: g::areas(gameplay).map_err(|e| eprintln!("rac2: areas: {e}")).unwrap_or_default(),
        }
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

    /// The inverse's local point of `p` in a volume.
    fn local(v: &Volume, p: V3) -> V3 {
        let d = [p[0] - v.matrix[3][0], p[1] - v.matrix[3][1], p[2] - v.matrix[3][2]];
        let m = &v.inverse;
        [
            d[0] * m[0][0] + d[1] * m[1][0] + d[2] * m[2][0],
            d[0] * m[0][1] + d[1] * m[1][1] + d[2] * m[2][1],
            d[0] * m[0][2] + d[1] * m[1][2] + d[2] * m[2][2],
        ]
    }

    /// `0x3219e8(p, i)`: inside the sphere `i`.
    pub fn in_sphere(&self, i: i32, p: V3) -> bool {
        let Some(v) = usize::try_from(i).ok().and_then(|i| self.spheres.get(i)) else { return false };
        let l = Self::local(v, p);
        (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() < 1.0
    }

    /// `0x321930(p, i)`: inside the cylinder `i` (across under 1, along ±1).
    pub fn in_cylinder(&self, i: i32, p: V3) -> bool {
        let Some(v) = usize::try_from(i).ok().and_then(|i| self.cylinders.get(i)) else { return false };
        let l = Self::local(v, p);
        (l[0] * l[0] + l[1] * l[1]).sqrt() < 1.0 && (-1.0..=1.0).contains(&l[2])
    }

    /// `0x31cf68(p, path)`: inside the path's polygon across (the crossings of a ray along x).
    pub fn in_polygon(&self, i: i32, p: V3) -> bool {
        let Some(pts) = usize::try_from(i).ok().and_then(|i| self.paths.get(i)) else { return false };
        let n = pts.len();
        let mut inside = false;
        for k in 0..n {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            if ((a[1] < p[1] && p[1] <= b[1]) || (b[1] < p[1] && p[1] <= a[1])) && a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) < p[0] {
                inside = !inside;
            }
        }
        inside
    }

    /// `0x32a848(p, area)`: in the area's sphere, then in any of its cuboids, spheres, cylinders or polygons.
    pub fn in_area(&self, i: i32, p: V3) -> bool {
        let Some(a) = usize::try_from(i).ok().and_then(|i| self.areas.get(i)) else { return false };
        let s = a.sphere;
        if s[3] < ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2) + (p[2] - s[2]).powi(2)).sqrt() { return false; }
        a.lists[1].iter().any(|&c| self.in_cuboid(c, p))
            || a.lists[2].iter().any(|&c| self.in_sphere(c, p))
            || a.lists[3].iter().any(|&c| self.in_cylinder(c, p))
            || a.lists[0].iter().any(|&c| self.in_polygon(c, p))
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

    /// A cuboid's position (`+0x30`) and its `+0x70` vector (the angles of the objects that place Ratchet there).
    pub fn cuboid_place(&self, i: i32) -> Option<(V3, V3)> {
        let c = usize::try_from(i).ok().and_then(|i| self.cuboids.get(i))?;
        Some(([c.matrix[3][0], c.matrix[3][1], c.matrix[3][2]], [c.extra[0], c.extra[1], c.extra[2]]))
    }

    /// A cuboid's `+0x78` word (its yaw, for the objects that face along it).
    pub fn cuboid_yaw(&self, i: i32) -> f32 { usize::try_from(i).ok().and_then(|i| self.cuboids.get(i)).map_or(0.0, |c| c.extra[2]) }
}
