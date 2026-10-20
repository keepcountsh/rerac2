//! The carriers: a moby that moves a moby group, a tie group and a shrub group with it (L00 `0x3246c0` takes them,
//! `0x324bd0` moves them; the block of 0x50-byte records in the carrier's pvars). Each member keeps its place and
//! its rows in the carrier's frame (the carrier's rows from its angles); each tick they are put back in the world by
//! the carrier's new place and rows. A moby member keeps its rows (mode |= 0x104) and is rebuilt; a tie or shrub
//! member's matrix changes and its index goes to [`MobyWorld::moved_ties`] / [`MobyWorld::moved_shrubs`] for the
//! engine.
//!
//! Not ported: the carrier's joint frames (`param_8`: a member on the moby's nearest joint `0x3245b0`), the offset
//! moby `+0x40`, the shrubs' sphere and scale words and the ties' update `0x36c068` (the engine's).

use super::{mode, MobyHost, MobyWorld};
use crate::hero::physics::V3;

/// What a member is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Moby,
    Tie,
    Shrub,
}

/// A carrier's block: its members, and whether its frame is its rows (`+0x4d`) rather than its angles.
#[derive(Clone, Debug, Default)]
pub struct Carrier {
    pub members: Vec<Member>,
    pub use_rows: bool,
}

/// One member (`+0x48` its index, `+0x4c` the kind; `+0x30` its place and `+0x00` its rows in the carrier's frame).
#[derive(Clone, Copy, Debug)]
pub struct Member {
    pub kind: Kind,
    pub index: usize,
    pub offset: V3,
    pub rows: [V3; 3],
}

fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// `0x2ef6b8`: the rows from Euler angles.
pub fn rows_of(rot: V3) -> [V3; 3] {
    rc_formats::moby_light::rotation_rows(rot).map(|r| [f32::from_bits(r[0]), f32::from_bits(r[1]), f32::from_bits(r[2])])
}

/// `0x320000(rows, &angles)`: the Euler angles of rows built as `X(a)·Y(b)·Z(g)` ([`rows_of`]).
pub fn angles_of(r: &[V3; 3]) -> V3 {
    let b = (-r[0][2]).atan2((r[0][0] * r[0][0] + r[0][1] * r[0][1]).sqrt());
    [r[1][2].atan2(r[2][2]), b, r[0][1].atan2(r[0][0])]
}

/// The rows `a` turned by `r` (`0x2efa00(out, a, r)`: each row times `r`).
pub fn turn(a: &[V3; 3], r: &[V3; 3]) -> [V3; 3] { a.map(|row| to_world(row, r)) }

/// World rows / point into the frame `c` (rows, orthonormal) and back.
fn to_local(v: V3, c: &[V3; 3]) -> V3 { [dot(v, c[0]), dot(v, c[1]), dot(v, c[2])] }
fn to_world(l: V3, c: &[V3; 3]) -> V3 { std::array::from_fn(|k| l[0] * c[0][k] + l[1] * c[1][k] + l[2] * c[2][k]) }

fn rows3(m: &[[f32; 4]; 4]) -> [V3; 3] { std::array::from_fn(|r| [m[r][0], m[r][1], m[r][2]]) }

impl MobyWorld {
    /// `0x3246c0(carrier, block, max, moby group, tie group, shrub group, …)`: the carrier takes up to `max` members
    /// (the mobys first, then the ties, then the shrubs); none (or `max` < 1) clears it.
    pub fn attach(&mut self, carrier: usize, max: usize, moby_group: i32, tie_group: i32, shrub_group: i32) {
        let c = &self.mobys[carrier];
        let (cp, cr) = (c.pos, rows_of(c.rot));
        let mut out = Vec::new();
        let local = |pos: V3, rows: &[V3; 3]| {
            let d = [pos[0] - cp[0], pos[1] - cp[1], pos[2] - cp[2]];
            (to_local(d, &cr), rows.map(|r| to_local(r, &cr)))
        };
        if let Some(g) = usize::try_from(moby_group).ok().and_then(|g| self.groups.get(g)).cloned() {
            for i in g {
                if out.len() >= max { break; }
                let m = &mut self.mobys[i];
                m.mode |= mode::KEEP_ROWS | mode::NO_MATRIX;
                let (offset, rows) = local(m.pos, &m.rows);
                out.push(Member { kind: Kind::Moby, index: i, offset, rows });
            }
        }
        for (kind, group, list, mats) in [(Kind::Tie, tie_group, &self.tie_groups, &self.ties), (Kind::Shrub, shrub_group, &self.shrub_groups, &self.shrubs)] {
            let Some(g) = usize::try_from(group).ok().and_then(|g| list.get(g)) else { continue };
            for &i in g {
                if out.len() >= max { break; }
                let Some(m) = mats.get(i as usize) else { continue };
                let (offset, rows) = local([m[3][0], m[3][1], m[3][2]], &rows3(m));
                out.push(Member { kind, index: i as usize, offset, rows });
            }
        }
        if out.is_empty() { self.attached.remove(&carrier); } else { self.attached.insert(carrier, Carrier { members: out, use_rows: false }); }
    }

    /// `+0x4d`: the carrier's frame from its rows (it turns its rows itself) instead of its angles.
    pub fn attach_use_rows(&mut self, carrier: usize) {
        if let Some(c) = self.attached.get_mut(&carrier) { c.use_rows = true; }
    }

    /// The members' rows in the carrier's frame turned by `r` (`0x2efa00(rows, rows, r)`).
    pub fn attach_turn(&mut self, carrier: usize, r: &[V3; 3]) {
        let Some(c) = self.attached.get_mut(&carrier) else { return };
        for m in &mut c.members { m.rows = m.rows.map(|row| to_world(row, r)); }
    }

    /// Keeps only the first `n` members (a carrier that ends its block early).
    pub fn attach_limit(&mut self, carrier: usize, n: usize) {
        if let Some(c) = self.attached.get_mut(&carrier) { c.members.truncate(n); }
    }

    /// `0x324bd0(carrier, block)`: the members put back by the carrier's place and angles.
    pub fn carry(&mut self, carrier: usize, host: &dyn MobyHost) {
        let Some(Carrier { members, use_rows }) = self.attached.get(&carrier).cloned() else { return };
        let c = &self.mobys[carrier];
        let (cp, cr) = (c.pos, if use_rows { c.rows } else { rows_of(c.rot) });
        let place = |m: &Member| {
            let o = to_world(m.offset, &cr);
            ([cp[0] + o[0], cp[1] + o[1], cp[2] + o[2]], m.rows.map(|r| to_world(r, &cr)))
        };
        for m in &members {
            let (pos, rows) = place(m);
            match m.kind {
                Kind::Moby => {
                    let Some(o) = self.mobys.get_mut(m.index) else { continue };
                    if o.state >= 0x80 { continue; }
                    o.pos = pos;
                    o.rows = rows;
                    self.rebuild(m.index, host);
                }
                Kind::Tie | Kind::Shrub => {
                    let (mats, moved) = if m.kind == Kind::Tie { (&mut self.ties, &mut self.moved_ties) } else { (&mut self.shrubs, &mut self.moved_shrubs) };
                    let Some(t) = mats.get_mut(m.index) else { continue };
                    for r in 0..3 { t[r][..3].copy_from_slice(&rows[r]); }
                    t[3][..3].copy_from_slice(&pos);
                    if !moved.contains(&m.index) { moved.push(m.index); }
                }
            }
        }
        self.changed = true;
    }
}
