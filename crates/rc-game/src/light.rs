//! The light regions (L00 `0x31a7a8` / `0x31a8e4`): where Ratchet is (and the one class that asks), the level's
//! regions pick his ambient and directional light sets, cross-faded across a region's box. The other mobys keep the
//! light word and ambient of their record (`0x3012a0`); his items and a scene's actors copy his.

use crate::hero::physics::V3;
use rc_formats::disc::gameplay::LightRegion;

/// `0x31a7a8(pos)`: the region `pos` is in and the blend across its box. The first region whose sphere holds `pos`
/// (`|pos − c|² − w` negative) decides: `pos` through its inverse matrix is clipped against the unit cube (any face
/// out: none, later regions not tried); else `t = (x + 1) / 2`.
pub fn lookup(regions: &[LightRegion], pos: V3) -> Option<(usize, f32)> {
    let (i, r) = regions.iter().enumerate().find(|(_, r)| {
        let d: [f32; 3] = std::array::from_fn(|k| pos[k] - r.sphere[k]);
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - r.sphere[3] < 0.0
    })?;
    let m = &r.to_box;
    let l: [f32; 3] = std::array::from_fn(|k| pos[0] * m[0][k] + pos[1] * m[1][k] + pos[2] * m[2][k] + m[3][k]);
    if l.iter().any(|c| c.abs() > 1.0) { return None; }
    Some((i, (l[0] + 1.0) * 0.5))
}

/// `0x31a8e4(moby)` at `pos`: the region ([`lookup`]) with its flag bit 0 set: the ambient and the light word follow
/// (t as a byte: the ambients blended `(a·(255 − t) + b·t) >> 8`, the word `set a | set b << 8 | t << 16`); otherwise
/// they stay as they were (None).
pub fn region_light(regions: &[LightRegion], pos: V3) -> Option<(u32, [u8; 3])> {
    let (i, t) = lookup(regions, pos)?;
    let r = &regions[i];
    if r.flags & 1 == 0 { return None; }
    let t = ((t * 255.0) as i32).clamp(0, 255) as u32;
    let ambient = std::array::from_fn(|k| ((r.ambient[0][k] as u32 * (255 - t) + r.ambient[1][k] as u32 * t) >> 8) as u8);
    Some(((r.lights[0] as u32 & 0xff) | (r.lights[1] as u32 & 0xff) << 8 | t << 16, ambient))
}
