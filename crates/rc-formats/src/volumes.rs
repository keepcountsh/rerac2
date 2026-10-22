//! Gameplay volumes: the shape sections the trigger tests read (cuboids 0x60, spheres 0x64, cylinders 0x68,
//! pills 0x6c), plus the two spline sections the area tests and the grind code read (paths 0x70, grind paths
//! 0x74). Spec and the tests that use them: docs/plan/triggers.md; section layout: docs/formats/wad_layouts_rac1.md
//! §3.2, docs/formats/collision_rac1.md §6.2.
//!
//! **Loader** (`InitLevelRenderGlobals` 0x255958, level01, after the grind paths):
//!
//! | gameplay ptr | runtime table (count / base) | stride | test |
//! |---|---|---|---|
//! | 0x60 cuboids | 0x1600f0 / 0x1600ec | 0x80 | `PointInCuboid` 0x274820 |
//! | 0x64 spheres | 0x1600f8 / 0x1600f4 | 0x80 | `PointInSphere` 0x2749b0 |
//! | 0x68 cylinders | 0x160100 / 0x1600fc | 0x80 | `PointInCylinder` 0x2748f8 |
//! | 0x6c pills | 0x1600e8 / 0x1600e4 | **0x90** | camera-grid only (`FUN_0020ff18` type 7) |
//! | 0x70 paths | 0x160104 / 0x1b0930[i] | — | `PointInPathPolygon` 0x26e6c0, spline sample / project |
//! | 0x74 grind paths | 0x15f710 / 0x15f70c | 0x20 | grind rails (0x314e98 / 0x315358) |
//!
//! Each shape section is `s32 count, pad[3]` then `count` × 0x80-byte [`Shape`] records; the loader zeroes the
//! table and `MemCopy`s every record. Pills are the exception: the runtime stride is 0x90 and the loader copies
//! **0x90 bytes from each 0x80-byte source record**, so the runtime +0x80 word (the pill cap radius the
//! camera-grid test reads) is the first word of whatever follows the record in the file. No RAC1 level has a
//! pill (every count is 0), so this never matters on disc; [`Volumes::pill_cap_radius`] reproduces it.

use bytemuck::{Pod, Zeroable};

/// Size of one shape record in the file (and the runtime stride of cuboids, spheres and cylinders).
pub(crate) const SHAPE_SIZE: usize = 0x80;

/// One shape record (0x80 bytes), identical for all four kinds. The shape is a canonical primitive in local
/// space mapped to the world by `matrix`: cuboid |l.x|, |l.y|, |l.z| ≤ 1; sphere |l| < 1; cylinder |l.xy| < 1,
/// |l.z| ≤ 1 (axis = local z); pill = the cylinder plus two cap spheres at local z = ±1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Shape {
    /// 0x00: local → world, VU row-vector form: `world = l.x·row0 + l.y·row1 + l.z·row2 + row3`. Row 3 is the
    /// centre (the tests subtract it: `+0x30`). Wrench writes 0.01 into `[3][3]`; the tests never read it.
    pub(crate) matrix: [[f32; 4]; 4],
    /// 0x40: world → local rotation/scale rows applied to `p − centre` (VU0 0x2215e0:
    /// `l = d.x·inv0 + d.y·inv1 + d.z·inv2`). The inverse translation is not stored (the tests subtract the
    /// centre first).
    pub(crate) inverse: [[f32; 4]; 3],
    /// 0x70: Euler angles of `matrix`. No test reads them; classes that use a cuboid as a placement marker do
    /// (camera trigger 737: `HeroTeleport(centre, euler)` and `CameraScript(centre, euler)`).
    pub(crate) euler: [f32; 3],
    /// 0x7c: unused (0).
    pub(crate) unused_7c: f32,
}
const _: () = assert!(std::mem::size_of::<Shape>() == SHAPE_SIZE);

