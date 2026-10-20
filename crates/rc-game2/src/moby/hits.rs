//! Hits: the template a hit is described by (`0x31d158`), the 64-record hit log (L00 `0x1c3400`, 0x40 bytes a
//! record, next slot `0x1bf194`; a moby's record index at `+0xa8`, −1 none) and the sphere query that hits every moby
//! it touches (`0x2e14b0`, the moby pass of rc_game's `coll_sphere_mobys`, which is R&C2's). R&C1's design
//! (rc_game::moby_update::services::HitLog), with R&C2's merge rule: a moby whose current record is its own and does
//! at least the new damage keeps it and takes the new flags into it.

use super::{mode, MobyWorld};
use crate::hero::physics::V3;
use rc_game::collision_query::mobys::{coll_sphere_mobys, MobyScene};
use rc_game::collision_query::QueryFlags;

/// Hit records in the log.
pub const HIT_RECORDS: usize = 0x40;

/// A hit's description (`0x31d158(damage, &t, source, flags, dir)`: `t[0..4]` direction, `t[4]` source moby, `t[5]`
/// flags, `t[7]` damage, `t[8]` = 1; callers then set `t[6]` (the class / kind word) and `t[3]` (5627.98, "no
/// knock-back limit" on the wrench's)).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitTemplate {
    pub dir: [f32; 4],
    pub source: Option<usize>,
    pub flags: u32,
    /// `t[6]`: the caller's word (the wrench: `o_class << 16 | 0x100`, the comet's `(combo + 1) << 16 | 0x100`).
    pub kind: u32,
    pub damage: f32,
    pub count: u32,
}

impl HitTemplate {
    /// `0x31d158(damage, &t, source, flags, dir)`.
    pub fn new(damage: f32, source: Option<usize>, flags: u32, dir: [f32; 4]) -> HitTemplate {
        HitTemplate { dir, source, flags, kind: 0, damage, count: 1 }
    }
}

/// One record of the log: the point (sphere hits: 0, 0, 0, 1), the template and the target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitRecord {
    pub point: [f32; 4],
    pub tmpl: HitTemplate,
    pub target: usize,
}

/// The log (`0x1c3400`) and its next slot (`0x1bf194`).
#[derive(Clone, Debug)]
pub struct HitLog {
    pub records: Vec<HitRecord>,
    pub next: usize,
}

impl Default for HitLog {
    fn default() -> Self { HitLog { records: vec![HitRecord::default(); HIT_RECORDS], next: 0 } }
}

impl MobyWorld {
    /// Moby `id`'s current hit record (its `+0xa8` slot, when that record is for it): what its update reads.
    pub fn hit_message(&self, id: usize) -> Option<HitRecord> {
        let slot = usize::try_from(self.mobys.get(id)?.hit_slot).ok()?;
        self.hits.records.get(slot).copied().filter(|r| r.target == id)
    }

    /// Clears moby `id`'s record (the class updates do this once they have read it: `+0xa8 = −1`).
    pub fn clear_hit(&mut self, id: usize) {
        if let Some(m) = self.mobys.get_mut(id) { m.hit_slot = -1; }
    }

    /// The delivery `0x2e14b0` does per moby: only mobys with mode 0x4000; a current record of its own that does at
    /// least `t.damage` takes `t.flags` and stays; else the next slot gets the record.
    pub fn deliver_hit(&mut self, id: usize, t: &HitTemplate, point: [f32; 4]) {
        let Some(m) = self.mobys.get(id) else { return };
        if m.mode & mode::LINE == 0 { return; }
        if let Ok(slot) = usize::try_from(m.hit_slot) {
            if let Some(r) = self.hits.records.get_mut(slot).filter(|r| r.target == id) {
                if r.tmpl.damage >= t.damage {
                    r.tmpl.flags |= t.flags;
                    return;
                }
            }
        }
        let slot = self.hits.next;
        self.hits.records[slot] = HitRecord { point, tmpl: *t, target: id };
        self.mobys[id].hit_slot = slot as i8;
        self.hits.next = (slot + 1) & (HIT_RECORDS - 1);
    }

    /// A line query's delivery (`0x2de878` with a template): the moby the line hit gets the record whatever its mode.
    pub fn deliver_hit_any(&mut self, id: usize, t: &HitTemplate, point: [f32; 4]) {
        let Some(m) = self.mobys.get(id) else { return };
        if let Ok(slot) = usize::try_from(m.hit_slot) {
            if let Some(r) = self.hits.records.get(slot).filter(|r| r.target == id) {
                if t.damage < r.tmpl.damage { return; }
            }
        }
        let slot = self.hits.next;
        self.hits.records[slot] = HitRecord { point, tmpl: *t, target: id };
        self.mobys[id].hit_slot = slot as i8;
        self.hits.next = (slot + 1) & (HIT_RECORDS - 1);
    }

    /// `0x2e14b0(r, centre, flags, ignore, tmpl)`: the mobys a sphere touches (grid order), each delivered the hit;
    /// returns how many it touched.
    pub fn sphere_hit(&mut self, scene: Option<&MobyScene>, centre: V3, r: f32, flags: u32, ignore: Option<usize>, t: Option<&HitTemplate>) -> usize {
        let Some(sc) = scene else { return 0 };
        let ids = coll_sphere_mobys(sc, centre, r, QueryFlags(flags), ignore);
        if let Some(t) = t {
            for &id in &ids { self.deliver_hit(id, t, [0.0, 0.0, 0.0, 1.0]); }
        }
        ids.len()
    }
}
