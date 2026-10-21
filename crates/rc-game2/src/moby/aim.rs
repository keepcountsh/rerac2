//! The weapons' aim search `0x3257a0` (through `0x325f90`): the moby of the target list (`0x1fac60`) a shot from
//! `from` along `dir` would pick. A candidate is drawn, run by the enemy framework (mode 0x20) with health left and of
//! class type 5 (or a crate, which the searches here allow), not of the excluded classes (`0x1aa300`: 0xc87, 0xecc).
//! Its aim point (the position raised by its health block's `+0x10`) is within `range` (and 0.1 away); its yaw and
//! pitch off `dir` are within limits that narrow with the distance (from `near_yaw` / `near_pitch` at Ratchet to
//! `yaw` / `pitch` at the range, `0x31bc68`'s ease 1 − (1 − t)³); the score `3·yaw off + 0.2·pitch off·distance +
//! 0.1·distance` (a crate's × 0.8·range) is the lowest, and the line to it (flags 0x12) is clear. A target held by
//! `0x18b270` wins outright (not ported: nothing sets it on Aranos).
//!
//! Not ported: the walls-and-ceilings branch (`$gp − 0x6000`, the gravity boots' planes) and the see-through classes a
//! blocked line may still pass (`0x381350`, the list at `0x2318d0`).

use super::{ai, mode, Ctx, MobyWorld};
use crate::hero::physics::{atan2, len, len2, rot_dist, sub, V3};

/// `0x1aa300`: the classes the search skips.
const EXCLUDED: [i16; 2] = [0xc87, 0xecc];

/// The limits of a search (`0x3257a0`'s floats 1..5).
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    pub yaw: f32,
    pub pitch: f32,
    pub range: f32,
    pub near_pitch: f32,
    pub near_yaw: f32,
}

/// `0x31fa20(m)`: moby `m`'s health and aim height (its health block's `+0x00` / `+0x10`) when the enemy framework
/// runs it.
pub fn health(w: &MobyWorld, m: usize) -> Option<(f32, f32)> {
    let x = w.mobys.get(m).filter(|x| x.mode & ai::AI != 0)?;
    let h = ai::health(x);
    Some((x.pv_f(h), x.pv_f(h + 0x10)))
}

/// `0x3204e8`: a crate (o_class 500..540).
pub fn is_crate(w: &MobyWorld, m: usize) -> bool { w.mobys.get(m).is_some_and(|x| (500..=540).contains(&x.o_class)) }

/// The aim point of moby `m`: its position raised by its aim height.
pub fn aim_point(w: &MobyWorld, m: usize) -> V3 {
    let p = w.mobys[m].pos;
    let up = health(w, m).map_or(0.0, |(_, a)| a);
    [p[0], p[1], p[2] + up]
}

/// `0x3257a0(cone, from, dir, ..)`: the moby picked, if any.
pub fn search(w: &MobyWorld, ctx: &Ctx, cone: Cone, from: V3, dir: V3, crates: bool) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    let dir_yaw = atan2(dir[0], dir[1]);
    let dir_pitch = atan2(len2(dir), dir[2]);
    for t in w.targets(ctx.host.camera_pos()) {
        let id = t.id;
        let m = &w.mobys[id];
        if m.drawn == 0 || m.mode & mode::TARGET == 0 { continue; }
        let crate_ = is_crate(w, id);
        if crate_ && !crates { continue; }
        let Some((hp, _)) = health(w, id) else { continue };
        if !(0.0 < hp || crate_) { continue; }
        if !(t.kind == 5 || crate_) { continue; }
        if EXCLUDED.contains(&m.o_class) { continue; }
        let p = aim_point(w, id);
        let d = sub(p, from);
        let dist = len(d);
        if !(dist <= cone.range && 0.1 <= dist) { continue; }
        let k = dist / cone.range;
        let ease = 1.0 - (1.0 - k) * (1.0 - k) * (1.0 - k);
        let yaw_off = rot_dist(atan2(d[0], d[1]), dir_yaw);
        if cone.near_yaw + (cone.yaw - cone.near_yaw) * ease <= yaw_off { continue; }
        let pitch_off = rot_dist(atan2(len2(d), d[2]), dir_pitch);
        if cone.near_pitch + (cone.pitch - cone.near_pitch) * ease <= pitch_off { continue; }
        let mut score = yaw_off * 3.0 + pitch_off * dist * 0.2 + dist * 0.1;
        if crate_ { score *= 0.8 * cone.range; }
        if best.is_some_and(|(b, _)| b <= score) { continue; }
        // `0x2de878(from, point, 0x12, m)`: a wall or another moby between.
        let blocked = ctx.coll.is_some_and(|c| rc_game::collision_query::coll_line_m(c, ctx.scene, from, p, rc_game::collision_query::QueryFlags(0x12), Some(id)).is_some());
        if blocked { continue; }
        best = Some((score, id));
    }
    best.map(|(_, id)| id)
}
