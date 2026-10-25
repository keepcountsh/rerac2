//! The fog the view draws with: the level fog globals, the regions' fog, the underwater test and the fog selection
//! (L00; R&C1's `fog_zones` is the same code, its zone lookup a 2-D circle where Going Commando's is a sphere).
//!
//! **The level fog globals** (`0x1b1900..0x1b1913`, [`FogGlobals`]): the level settings' fog at the level start
//! (`0x3012a0`), overwritten by the camera update's region fog (`0x2d9f38` → `0x2dbc20(camera)`, [`region_fog`]): in
//! a light region ([`crate::light::lookup`]) whose flags have bit 1, the fog is its two faces' blended across the box
//! (t: the colour bytes `(c2·i + c1·(255 − i)) >> 8` with `i = (int)(t·255)`; with `u = 1 − t` the distances
//! `(d2·t + d1·u)·1024` and the intensities `255 − (k2·t + k1·u)·255`). Nothing restores them on leaving a region.
//!
//! **Under water** (`0x1b9000`, [`underwater_test`]: the camera update's `0x2d9a58`, before the region fog): off in the
//! camera's mode 6 and outside play (`0x1a8f00 ≠ 0`); else a line from 0.75 above the camera to 0.75 below it (flags
//! 0x12): a hit on another surface than water (`0x2e23e0` ≠ 0) goes on from 0.01 under it (at most 6); the water's
//! (surface 0) height (`0x31d7a0`: the ripple patch's, else the water plane's, else the hit's own) + 0.04 above the
//! camera sets the flag, else clears it; no hit leaves it. Ratchet's code writes it too (crate::hero: his teleport
//! `0x2b9e68` to whether he swims under water, his way out of the water `0x2c8f38` and SetState `0x2c4560` off when he
//! is out of the water and the camera above its surface), and the level's camera start `0x2da248` clears it.
//!
//! **The fog selection** (`0x2e4af0`, [`update_fog`], run at the end of every frame's draw and at the level start):
//! the view's fog is the level globals and the particles' far 500 units, or under water the alternate set
//! (`0x1ad8a4..`, [`UnderwaterLook`]) and 64 units. Under water the frame's HUD pass also tints the screen
//! (`0x2e6658`: a full-screen fill in `0x1ad8a0..a3`, ALPHA 0x44, before the scene fade).

use crate::hero::physics::V3;
use rc_formats::disc::gameplay::LightRegion;

/// The level fog globals (`0x1b1900..`): FOGCOL, the distances in units × 1024 along the view axis, the GS F at them
/// (255 = no fog).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogGlobals {
    /// `0x1b1900..0x1b1902`.
    pub colour: [u8; 3],
    /// `0x1b1904` / `0x1b1908`.
    pub near_dist: f32,
    pub far_dist: f32,
    /// `0x1b190c` / `0x1b1910`.
    pub near_intensity: f32,
    pub far_intensity: f32,
}

/// `gp + 0x3050`: the particles' far in 20.12 fixed point, 500 units ...
pub const PARTICLE_FAR12: i32 = 0x1f_4000;
/// ... and 64 under water.
pub const UNDERWATER_PARTICLE_FAR12: i32 = 0x4_0000;

/// The underwater tint and fog (`0x1ad8a0..0x1ad8b7`, the same bytes in every level's overlay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnderwaterLook {
    /// `0x1ad8a0..a3`: the full-screen fill's RGBA (ALPHA 0x44: `Cd + (Cs − Cd)·A / 128`).
    pub tint: [u8; 4],
    /// `0x1ad8a4..`: the fog under water.
    pub fog: FogGlobals,
}

impl Default for UnderwaterLook {
    fn default() -> Self {
        UnderwaterLook { tint: [0x18, 0x30, 0x90, 0x40], fog: FogGlobals { colour: [0x18, 0x30, 0x90], near_dist: 0.0, far_dist: 32768.0, near_intensity: 255.0, far_intensity: 48.0 } }
    }
}

/// `0x2dbc20(pos)`: the region fog (module docs) into `g`; the region and t hit (whatever its flags).
pub fn region_fog(g: &mut FogGlobals, regions: &[LightRegion], pos: V3) -> Option<(usize, f32)> {
    let (k, t) = crate::light::lookup(regions, pos)?;
    let r = &regions[k];
    if r.flags & 2 == 0 { return Some((k, t)); }
    let i = (t * 255.0) as i32;
    let u = 1.0 - t;
    let [s1, s2] = r.fog;
    g.far_intensity = 255.0 - (s2[3] * t + s1[3] * u) * 255.0;
    g.far_dist = (s2[2] * t + s1[2] * u) * 1024.0;
    g.near_dist = (s2[0] * t + s1[0] * u) * 1024.0;
    g.near_intensity = 255.0 - (s2[1] * t + s1[1] * u) * 255.0;
    let (c1, c2) = (r.fog_colour[0] as i32, r.fog_colour[1] as i32);
    for (k, out) in g.colour.iter_mut().enumerate() {
        let (a, b) = ((c1 >> (8 * k)) & 0xff, (c2 >> (8 * k)) & 0xff);
        *out = ((b * i + a * (0xff - i)) >> 8) as u8;
    }
    Some((k, t))
}

/// `0x2e4af0`: the view's fog and the particles' far (module docs).
pub fn update_fog(level: &FogGlobals, underwater: bool, look: &UnderwaterLook) -> (FogGlobals, i32) {
    if underwater { (look.fog, UNDERWATER_PARTICLE_FAR12) } else { (*level, PARTICLE_FAR12) }
}

/// `0x2d9a58` (module docs): `off` = the camera's mode 6 or outside play; `cast(a, b)` = the line (flags 0x12) → its
/// point and surface id; `water_height(point)` = `0x31d7a0`'s patch or plane height (None: the point's own).
pub fn underwater_test(flag: &mut bool, cam: V3, off: bool, mut cast: impl FnMut(V3, V3) -> Option<(V3, i32)>, mut water_height: impl FnMut(V3) -> Option<f32>) {
    if off {
        *flag = false;
        return;
    }
    let mut a = [cam[0], cam[1], cam[2] + 0.75];
    let b = [cam[0], cam[1], cam[2] - 0.75];
    for _ in 0..6 {
        let Some((hit, surface)) = cast(a, b) else { return };
        if surface == 0 {
            let h = water_height(hit).unwrap_or(hit[2]);
            *flag = cam[2] < h + 0.04;
            return;
        }
        a = [hit[0], hit[1], hit[2] - 0.01];
    }
}

/// [`underwater_test`] against the world and the mobys' meshes (`0x2de878(a, b, 0x12)`), the water's height its hit's
/// (the ripple patch and the water plane are not ported: G-REN-009).
pub fn underwater_test_in(flag: &mut bool, cam: V3, off: bool, coll: &rc_formats::collision::Collision, scene: Option<&crate::collision::mobys::MobyScene>) {
    let cast = |a: V3, b: V3| crate::collision::coll_line_m(coll, scene, a, b, crate::collision::QueryFlags(0x12), None).map(|o| (o.point, o.surface_id()));
    underwater_test(flag, cam, off, cast, |_| None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(flags: u32) -> LightRegion {
        // A box centred at (10, 20, 5), half sizes (4, 2, 3), local x along world +y.
        let to_box = [[0.0, -0.5, 0.0, 0.0], [0.25, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0 / 3.0, 0.0], [-5.0, 5.0, -5.0 / 3.0, 1.0]];
        LightRegion {
            sphere: [10.0, 20.0, 5.0, 25.0],
            to_box,
            ambient: [[0; 4]; 2],
            lights: [0xb; 2],
            flags,
            fog_colour: [0x0f0505, 0xb37e69],
            fog: [[0.0, 0.0, 50.0, 0.8], [0.0, 0.0, 240.0, 0.6]],
        }
    }

    #[test]
    fn region_fog_blends_and_keeps() {
        let r = [region(2)];
        let mut g = FogGlobals { colour: [1, 2, 3], near_dist: 0.0, far_dist: 9.0, near_intensity: 255.0, far_intensity: 100.0 };
        assert_eq!(region_fog(&mut g, &r, [10.0, 16.0, 5.0]), Some((0, 0.0)));
        assert_eq!(g.colour, [((5i32 * 255) >> 8) as u8, ((5i32 * 255) >> 8) as u8, ((15i32 * 255) >> 8) as u8]);
        assert_eq!(g.far_dist, 51200.0);
        assert!((g.far_intensity - 51.0).abs() < 1e-3);
        assert_eq!(region_fog(&mut g, &r, [10.0, 24.0, 5.0]), Some((0, 1.0)));
        assert_eq!(g.far_dist, 245760.0);
        // Out of every region: nothing restored.
        let before = g;
        assert_eq!(region_fog(&mut g, &r, [100.0, 100.0, 0.0]), None);
        assert_eq!(g, before);
        // Bit 1 clear: hit, fog untouched.
        assert_eq!(region_fog(&mut g, &[region(1)], [10.0, 20.0, 5.0]), Some((0, 0.5)));
        assert_eq!(g, before);
    }

    #[test]
    fn underwater_test_hysteresis() {
        let water = |a: V3, b: V3| (a[2] >= 10.0 && b[2] <= 10.0).then_some(([a[0], a[1], 10.0], 0));
        let mut f = false;
        underwater_test(&mut f, [0.0, 0.0, 9.5], false, water, |_| None);
        assert!(f);
        underwater_test(&mut f, [0.0, 0.0, 5.0], false, water, |_| None);
        assert!(f, "no hit: unchanged");
        underwater_test(&mut f, [0.0, 0.0, 10.5], false, water, |_| None);
        assert!(!f);
        underwater_test(&mut f, [0.0, 0.0, 10.03], false, water, |_| None);
        assert!(f, "under h + 0.04");
        underwater_test(&mut f, [0.0, 0.0, 9.5], true, water, |_| None);
        assert!(!f, "off: cleared");
    }

    #[test]
    fn update_fog_selects() {
        let look = UnderwaterLook::default();
        let level = FogGlobals { colour: [1, 2, 3], ..Default::default() };
        assert_eq!(update_fog(&level, false, &look), (level, PARTICLE_FAR12));
        assert_eq!(update_fog(&level, true, &look), (look.fog, UNDERWATER_PARTICLE_FAR12));
    }
}
