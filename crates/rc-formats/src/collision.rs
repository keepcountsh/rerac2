//! Level collision mesh. Spec: docs/formats/collision_rac1.md; runtime queries:
//! docs/plan/collision_queries.md. Ported from the retired C++ reference
//! extractor (git 2230812; `parse_collision`, `collision_triangles`).
//!
//! RAC1 has one collision block per level, at `LevelCoreHeader::collision` in the decompressed
//! core data. It holds the baked world-space mesh of all static geometry (tfrags plus whatever
//! tie/shrub/moby collision the level build merged in), bucketed into a sparse grid of 4x4x4-unit
//! cells indexed Z -> Y -> X, followed by the hero (player-only) collision groups. Every cell has
//! its own vertices and faces: a face overlapping N cells is stored N times, so a point query
//! only ever reads one cell. Per-class moby collision is a separate format (moby class header
//! +0x10) and is not covered here.
//!
//! Tree offsets (slab, row and leaf) are byte offsets from the start of the mesh
//! ([`CollisionHeader::mesh`]), not from the block.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use bytemuck::{Pod, Zeroable};

/// Block header (8 bytes; bytes 0x08..`mesh` are zero padding on every retail level).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct CollisionHeader {
    /// 0x0: offset of the mesh (root node) from the block start; 0x40 on every retail level.
    pub(crate) mesh: i32,
    /// 0x4: offset of the hero-collision section from the block start, or 0 for none. Every
    /// retail level has one (possibly with zero groups), and it directly follows the mesh.
    pub(crate) hero_groups: i32,
}
const _: () = assert!(std::mem::size_of::<CollisionHeader>() == 8);

/// Header shared by the three tree levels (4 bytes), followed by `count` entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct CollisionNodeHeader {
    /// 0x0: cell coordinate of entry 0 on this node's axis (Z for the root, Y for a slab, X for a row).
    pub(crate) base: i16,
    /// 0x2: number of entries.
    pub(crate) count: u16,
}

/// Root node at the mesh start: one `u16` entry per Z slab; slab node offset = entry * 4, 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CollisionRoot {
    pub(crate) header: CollisionNodeHeader,
    pub(crate) slabs: Vec<u16>,
}

/// A non-empty Z slab: one `u32` entry per Y row, a byte offset of the row node, 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CollisionSlab {
    /// Byte offset of this node from the mesh start (= root entry * 4).
    pub(crate) offset: u32,
    /// Cell Z coordinate of the slab.
    pub(crate) z: i16,
    pub(crate) header: CollisionNodeHeader,
    pub(crate) rows: Vec<u32>,
}

/// A non-empty Y row: one `u32` leaf word per X cell ([`CollisionCell::leaf_word`]), 0 = empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CollisionRow {
    /// Byte offset of this node from the mesh start.
    pub(crate) offset: u32,
    /// Cell Z and Y coordinates of the row.
    pub(crate) z: i16,
    pub(crate) y: i16,
    pub(crate) header: CollisionNodeHeader,
    pub(crate) cells: Vec<u32>,
}

/// Leaf header (4 bytes). The leaf continues with `vertex_count` packed vertices, `face_count`
/// face records (quads first), `quad_count` fourth indices, and zero padding to 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionLeafHeader {
    /// 0x0: quads + triangles.
    pub face_count: u16,
    /// 0x2
    pub vertex_count: u8,
    /// 0x3: the first `quad_count` faces are quads.
    pub quad_count: u8,
}

impl CollisionLeafHeader {
    /// Payload bytes before padding: `4 + 4V + 4F + Q`.
    pub(crate) fn payload_size(&self) -> usize { 4 + 4 * self.vertex_count as usize + 4 * self.face_count as usize + self.quad_count as usize }
    /// Leaf size in quadwords, which the tree word's low byte must equal.
    pub(crate) fn size_qwords(&self) -> usize { self.payload_size().div_ceil(16) }
}

/// A packed cell-relative vertex (u32). Bit fields, all signed two's complement:
/// X = bits 0..10 at 1/16, Y = bits 10..20 at 1/16, Z = bits 20..32 at 1/64, each relative to
/// the cell centre. Extraction is by shift pairs (`(w << 22) >> 22`, `(w << 12) >> 22`,
/// `w >> 20`, arithmetic right shifts on the signed word), as in the retired C++ reference
/// and Wrench's `read_collision_mesh`; the scales are Wrench's, confirmed on the disc by the
/// collision map coinciding with the tfrag terrain (spec 6b).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedCollisionVertex(pub(crate) u32);

impl PackedCollisionVertex {
    /// Raw signed fields `[x, y, z]`: x, y in -512..=511, z in -2048..=2047.
    pub fn fields(self) -> [i32; 3] {
        let w = self.0;
        [((w << 22) as i32) >> 22, ((w << 12) as i32) >> 22, (w as i32) >> 20]
    }
    /// Offset from the cell centre in world units (`x/16`, `y/16`, `z/64`; exact in f32).
    pub(crate) fn offset(self) -> [f32; 3] {
        let [x, y, z] = self.fields();
        [x as f32 / 16.0, y as f32 / 16.0, z as f32 / 64.0]
    }
    /// World position given the cell centre, computed as `centre + offset` per component in
    /// f32 like the retired C++ reference (every intermediate is exactly representable).
    pub(crate) fn world(self, centre: [f32; 3]) -> [f32; 3] {
        let o = self.offset();
        [centre[0] + o[0], centre[1] + o[1], centre[2] + o[2]]
    }
}

/// Face record (4 bytes). For a quad the fourth index is in [`CollisionCell::quad_v3`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CollisionFace {
    /// 0x0: indices into the cell's vertex array.
    pub v: [u8; 3],
    /// 0x3: collision type byte. Bits 0-4 surface id (0x1f = none), bits 5-6 footstep sound
    /// class, bit 7 excluded by queries with flag 0x80 (docs/plan/collision_queries.md section 3).
    pub surface: u8,
}

/// One non-empty grid cell (a tree leaf) with its payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionCell {
    /// Cell coordinates (`base + index` on each tree level).
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) z: i16,
    /// Tree entry: `leaf offset from mesh << 8 | leaf size in quadwords`.
    pub(crate) leaf_word: u32,
    pub header: CollisionLeafHeader,
    pub packed: Vec<PackedCollisionVertex>,
    /// World-space positions of `packed` (see [`PackedCollisionVertex::world`]).
    pub(crate) vertices: Vec<[f32; 3]>,
    /// Quads first (`header.quad_count` of them), then triangles.
    pub faces: Vec<CollisionFace>,
    /// Fourth index of each quad, in quad order.
    pub quad_v3: Vec<u8>,
}

impl CollisionCell {
    /// Cell centre `4c + 2` per axis, computed as `c * 4.0 + 2.0` in f32 like the C++.
    pub(crate) fn centre(&self) -> [f32; 3] { [self.x, self.y, self.z].map(|c| c as f32 * 4.0 + 2.0) }
    pub(crate) fn is_quad(&self, face: usize) -> bool { face < self.header.quad_count as usize }
    /// The face's four indices; a triangle repeats `v0` as the fourth, like the C++ `CollisionFace::v`.
    pub(crate) fn face_indices(&self, face: usize) -> [u8; 4] {
        let f = self.faces[face];
        [f.v[0], f.v[1], f.v[2], if self.is_quad(face) { self.quad_v3[face] } else { f.v[0] }]
    }
}

/// Hero-collision group record (0x10 bytes), relative to the hero section.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroGroupHeader {
    /// 0x0: bounding sphere x, y, z, radius, unsigned at 1/64.
    pub sphere: [u16; 4],
    /// 0x8
    pub triangle_count: u16,
    /// 0xa
    pub vertex_count: u16,
    /// 0xc: offset of the group data from the hero-section start.
    pub(crate) data: u32,
}
const _: () = assert!(std::mem::size_of::<HeroGroupHeader>() == 0x10);

/// Hero vertex (8 bytes): unsigned world position at 1/64; `pad` is zero (validated).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroVertex {
    pub xyz: [u16; 3],
    pub(crate) pad: u16,
}

/// Hero triangle (4 bytes): three group-local indices and a zero pad byte (validated). No surface id.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HeroTriangle {
    pub v: [u8; 3],
    pub pad: u8,
}

/// Player-only collision (invisible walls, fences): a flat list of groups with bounding spheres.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeroGroup {
    pub header: HeroGroupHeader,
    pub vertices: Vec<HeroVertex>,
    pub triangles: Vec<HeroTriangle>,
}

/// The parsed collision block. Nodes and cells are in tree-walk order (Z, then Y, then X
/// ascending), which is also the order of the retired C++ reference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collision {
    pub(crate) header: CollisionHeader,
    pub(crate) root: CollisionRoot,
    pub(crate) slabs: Vec<CollisionSlab>,
    pub(crate) rows: Vec<CollisionRow>,
    pub cells: Vec<CollisionCell>,
    /// The hero section's `s32` group count (0 when `header.hero_groups == 0`).
    pub(crate) hero_group_count: i32,
    pub hero_groups: Vec<HeroGroup>,
}

impl Collision {
    /// Index into [`cells`](Self::cells) of the cell at the given cell coordinates, found by
    /// walking the parsed tree Z -> Y -> X like [`lookup_cell_word`].
    pub fn cell_at(&self, x: i16, y: i16, z: i16) -> Option<usize> {
        self.cells.binary_search_by_key(&(z, y, x), |c| (c.z, c.y, c.x)).ok()
    }
}

fn node<'a>(m: Buf<'a>, at: usize, entry_size: usize, what: &'static str) -> Result<(CollisionNodeHeader, Buf<'a>)> {
    let h: CollisionNodeHeader = m.pod(at, what)?;
    if h.count > 4096 { return invalid(format!("collision: implausible {what} count {}", h.count)); }
    let entries = m.sub(at + 4, h.count as usize * entry_size, what)?;
    Ok((h, entries))
}

fn parse_leaf(m: Buf, x: i16, y: i16, z: i16, word: u32) -> Result<CollisionCell> {
    let leaf = (word >> 8) as usize;
    let header: CollisionLeafHeader = m.pod(leaf, "collision leaf header")?;
    let (f, v, q) = (header.face_count as usize, header.vertex_count as usize, header.quad_count as usize);
    if q > f { return invalid("collision: quad count exceeds face count"); }
    if header.size_qwords() != (word & 0xff) as usize { return invalid("collision: leaf size byte does not match its contents"); }
    m.check(leaf, header.size_qwords() * 16, "collision leaf")?;
    let mut cell = CollisionCell { x, y, z, leaf_word: word, header, ..Default::default() };
    cell.packed = m.pod_slice(leaf + 4, v, "collision vertices")?;
    let centre = cell.centre();
    cell.vertices = cell.packed.iter().map(|p| p.world(centre)).collect();
    cell.faces = m.pod_slice(leaf + 4 + 4 * v, f, "collision faces")?;
    cell.quad_v3 = m.sub(leaf + 4 + 4 * v + 4 * f, q, "collision quad indices")?.bytes().to_vec();
    for i in 0..f {
        let n = if i < q { 4 } else { 3 };
        if cell.face_indices(i)[..n].iter().any(|&ix| ix as usize >= v) { return invalid("collision: face index out of range"); }
    }
    Ok(cell)
}

fn parse_hero_groups(hb: Buf, c: &mut Collision) -> Result<()> {
    c.hero_group_count = hb.i32(0)?;
    if !(0..=100_000).contains(&c.hero_group_count) { return invalid("collision: implausible hero group count"); }
    let headers: Vec<HeroGroupHeader> = hb.pod_slice(0x10, c.hero_group_count as usize, "hero group headers")?;
    for header in headers {
        let (data, nv, nt) = (header.data as usize, header.vertex_count as usize, header.triangle_count as usize);
        let vertices: Vec<HeroVertex> = hb.pod_slice(data, nv, "hero vertices")?;
        let triangles: Vec<HeroTriangle> = hb.pod_slice(data + 8 * nv, nt, "hero triangles")?;
        if vertices.iter().any(|v| v.pad != 0) { return invalid("collision: unknown hero vertex variant"); }
        for t in &triangles {
            if t.v.iter().any(|&ix| ix as usize >= nv) { return invalid("collision: hero index out of range"); }
            if t.pad != 0 { return invalid("collision: unknown hero triangle variant"); }
        }
        c.hero_groups.push(HeroGroup { header, vertices, triangles });
    }
    Ok(())
}

/// Parses a collision block (the bytes at `LevelCoreHeader::collision`).
pub(crate) fn parse_collision_block(block: &[u8]) -> Result<Collision> {
    let b = Buf(block);
    let header: CollisionHeader = b.pod(0, "collision header")?;
    if header.mesh <= 0 { return invalid("collision: no mesh pointer"); }
    if header.hero_groups != 0 && header.hero_groups < header.mesh { return invalid("collision: hero section before the mesh"); }
    // The mesh runs to the hero section (spec 2.1), or to the end of the block when there is none.
    let mesh_end = if header.hero_groups > 0 { header.hero_groups as usize } else { b.len() };
    let m = b.sub(header.mesh as usize, mesh_end.saturating_sub(header.mesh as usize), "collision mesh")?;
    let mut c = Collision { header, ..Default::default() };

    let (rh, zs) = node(m, 0, 2, "collision z")?;
    c.root = CollisionRoot { header: rh, slabs: (0..rh.count as usize).map(|i| zs.u16(2 * i)).collect::<Result<_>>()? };
    for (zi, &zoff) in c.root.slabs.iter().enumerate() {
        if zoff == 0 { continue; }
        let z = rh.base.wrapping_add(zi as i16);
        let so = zoff as usize * 4;
        let (sh, ys) = node(m, so, 4, "collision y")?;
        let rows: Vec<u32> = (0..sh.count as usize).map(|i| ys.u32(4 * i)).collect::<Result<_>>()?;
        for (yi, &ro) in rows.iter().enumerate() {
            if ro == 0 { continue; }
            let y = sh.base.wrapping_add(yi as i16);
            let (xh, xs) = node(m, ro as usize, 4, "collision x")?;
            let words: Vec<u32> = (0..xh.count as usize).map(|i| xs.u32(4 * i)).collect::<Result<_>>()?;
            for (xi, &word) in words.iter().enumerate() {
                if word == 0 { continue; }
                c.cells.push(parse_leaf(m, xh.base.wrapping_add(xi as i16), y, z, word)?);
            }
            c.rows.push(CollisionRow { offset: ro, z, y, header: xh, cells: words });
        }
        c.slabs.push(CollisionSlab { offset: so as u32, z, header: sh, rows });
    }
    if header.hero_groups > 0 { parse_hero_groups(b.tail(header.hero_groups as usize, "hero collision")?, &mut c)?; }
    Ok(c)
}

/// The level's collision block: the `"collision"` entry of `core.blocks`, sliced from the
/// decompressed core data (its size comes from the core boundary rule).
pub(crate) fn collision_block<'a>(core: &LevelCore, core_data: &'a [u8]) -> Result<&'a [u8]> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "collision") else { return invalid("level has no collision block") };
    Ok(Buf(core_data).sub(blk.offset, blk.size, "collision block")?.bytes())
}

/// Parses the level's collision from the decompressed core data.
pub fn parse_collision(core: &LevelCore, core_data: &[u8]) -> Result<Collision> {
    parse_collision_block(collision_block(core, core_data)?)
}

/// One world-space triangle of the main mesh (44 bytes; the layout of the C++ `CollisionTriangle`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct CollisionTriangle {
    pub(crate) a: [f32; 3],
    pub(crate) b: [f32; 3],
    pub(crate) c: [f32; 3],
    /// Index into [`Collision::cells`].
    pub(crate) cell: u32,
    /// Index into [`CollisionCell::faces`].
    pub(crate) face: u16,
    /// The face's surface id.
    pub(crate) surface: u8,
    /// 0 = triangle face, 1 = first half of a quad `(v0, v1, v2)`, 2 = second half `(v0, v2, v3)`.
    pub(crate) part: u8,
}
const _: () = assert!(std::mem::size_of::<CollisionTriangle>() == 44);

