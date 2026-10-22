//! Tfrag terrain block. Spec: docs/formats/tfrag_rac1.md. Ported from the retired
//! C++ reference extractor (git 2230812; `parse_tfrags`, `tfrag_triangles`).
//!
//! Beyond the C++ struct, [`Tfrag`] also carries the mini-sphere array and the
//! 8-corner cube, which the C++ reference did not parse.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use crate::vif::{self, parse_vif};
use bytemuck::{Pod, Zeroable};

/// Tfrags block header, at byte 0 of the block (0x10 bytes). Spec 1.2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct TfragBlockHeader {
    /// 0x00: byte offset of the `TfragHeader` table from the start of the block.
    pub table_offset: i32,
    /// 0x04: number of `TfragHeader` records.
    pub tfrag_count: i32,
    /// 0x08: LOD base distance L in world units (retail 12..32). The level-load tfrag init (boot 0x2040e0 /
    /// level01 0x255470) sets the LOD switch distances to 6L, 4L, 2L; see [`TfragBlockHeader::lod_distances`].
    pub unknown_8: f32,
    /// 0x0c: unknown word, preserved.
    pub unknown_c: u32,
}
const _: () = assert!(std::mem::size_of::<TfragBlockHeader>() == 0x10);

/// Per-tfrag header record (0x40 bytes). Spec 1.3. All `*_ofs` fields are relative to
/// the tfrag's data block, which starts at `table_offset + data` in the tfrags block.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct TfragHeader {
    /// 0x00: bounding sphere centre x, y, z and radius, f32 in raw position units (1024 = 1 world unit),
    /// absolute (not origin-relative). `TfragProc` compares it against `camera * 1024`.
    pub bsphere: [f32; 4],
    /// 0x10: offset of this tfrag's data block, relative to `table_offset`.
    pub(crate) data: i32,
    /// 0x14: offset of the LOD-2-only VIF list (0 in RAC1).
    pub(crate) lod_2_ofs: u16,
    /// 0x16: offset of the common ("shared") VIF list.
    pub(crate) shared_ofs: u16,
    /// 0x18: offset of the LOD-1-only VIF list.
    pub(crate) lod_1_ofs: u16,
    /// 0x1a: offset of the LOD-0+1 ("lod_01") VIF list.
    pub(crate) lod_0_ofs: u16,
    /// 0x1c: offset of the ad-gif payload inside the common list.
    pub(crate) tex_ofs: u16,
    /// 0x1e: offset of the RGBA array; also the end of the LOD-0-only list.
    pub(crate) rgba_ofs: u16,
    /// 0x20: common list size in quadwords.
    pub(crate) common_size: u8,
    /// 0x21: LOD-2 + common list size in quadwords (from the data start).
    pub(crate) lod_2_size: u8,
    /// 0x22: common + LOD-1 + LOD-01 size in quadwords (from `shared_ofs`); the LOD-0-only list starts at `shared_ofs + lod_1_size * 16`.
    pub(crate) lod_1_size: u8,
    /// 0x23: LOD-01 + LOD-0 size in quadwords (from `lod_0_ofs`).
    pub(crate) lod_0_size: u8,
    /// 0x24: RGBA entries used by LOD 2 (unit unverified).
    pub(crate) lod_2_rgba_count: u8,
    /// 0x25: RGBA entries used by LOD 1.
    pub(crate) lod_1_rgba_count: u8,
    /// 0x26: RGBA entries used by LOD 0.
    pub(crate) lod_0_rgba_count: u8,
    /// 0x27: non-zero = always draw LOD 2 (`TfragProc` skips the distance test).
    pub(crate) base_only: u8,
    /// 0x28: number of ad-gif (texture) primitives.
    pub(crate) texture_count: u8,
    /// 0x29: RGBA array size in quadwords (entries = `rgba_size * 4`).
    pub(crate) rgba_size: u8,
    /// 0x2a: unknown (VU location for vertex colours?).
    pub(crate) rgba_verts_loc: u8,
    /// 0x2b: unknown occlusion scratch.
    pub(crate) occl_index_stash: u8,
    /// 0x2c: number of texture spheres at `msphere_ofs` (see [`Tfrag::texture_spheres`]).
    pub(crate) msphere_count: u8,
    /// 0x2d: unknown flags.
    pub(crate) flags: u8,
    /// 0x2e: offset of the texture-sphere array (`msphere_count` x 0x10 bytes).
    pub(crate) msphere_ofs: u16,
    /// 0x30: offset of the origin quadword, followed by the light array.
    pub(crate) light_ofs: u16,
    /// 0x32: end of the light array (== `msphere_ofs` in RAC1).
    pub(crate) light_end_ofs: u16,
    /// 0x34: directional light reference (0xff = none).
    pub(crate) dir_lights_one: u8,
    /// 0x35: directional light update reference.
    pub(crate) dir_lights_upd: u8,
    /// 0x36: point light reference (0xffff = none).
    pub(crate) point_lights: u16,
    /// 0x38: offset of the 8-corner clip box (8 x 4 x s16; corner = s16 * 64 raw units), used by `TfragProc`'s guard-band test.
    pub(crate) cube_ofs: u16,
    /// 0x3a: occlusion index (maps through the gameplay occlusion table).
    pub(crate) occl_index: u16,
    /// 0x3c: total position count (common + lod01 + lod0); also the light count.
    pub vert_count: u8,
    /// 0x3d: LOD-0 triangle count.
    pub(crate) tri_count: u8,
    /// 0x3e: texture paging distance in raw units: the tfrag's texture spheres and ad-gifs are processed
    /// only while its near distance (view depth - radius) is <= `mip_dist` (`TfragProc`). Beyond it GS
    /// uses only the resident mip levels 2/3. Equals the spheres' `mip_dist_128 * 128` rounded up.
    pub(crate) mip_dist: u16,
}
const _: () = assert!(std::mem::size_of::<TfragHeader>() == 0x40);

/// VU header (20 x u16 = 0x28 bytes), unpacked V4_16 to VU address 0. Spec 2.4;
/// VU1 usage from docs/plan/vu1_tfrag_analysis.md 1 and 5. `q.l` = VU quadword.lane.
///
/// Each vertex-info tier (common, lod01, lod0) is `[one entry per position] ++ [extra entries]`;
/// the `unk_*` lanes are the extra ranges' counts and VU addresses.
/// (addr = tier addr + position count; tier size = positions + extra).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct TfragVuHeader {
    /// 0x00 (0.x): common position count.
    pub(crate) positions_common_count: u16,
    /// 0x02 (0.y): extra common vertex-info count (never read by VU1).
    pub(crate) unk_02: u16,
    /// 0x04 (0.z): LOD-01 position count.
    pub(crate) positions_lod_01_count: u16,
    /// 0x06 (0.w): extra LOD-01 vertex-info count = length of `unk_indices_2_lod01`; the VU1 L48 collapse pass count.
    pub(crate) unk_06: u16,
    /// 0x08 (1.x): LOD-0 position count.
    pub(crate) positions_lod_0_count: u16,
    /// 0x0a (1.y): extra LOD-0 vertex-info count = length of `unk_indices_2_lod0`; the VU1 L47 collapse pass count.
    pub(crate) unk_0a: u16,
    /// 0x0c (1.z): VU address of the interleaved position/colour region.
    pub(crate) positions_common_addr: u16,
    /// 0x0e (1.w): VU address of the vertex-info region (also the index STROW value).
    pub(crate) vertex_info_common_addr: u16,
    /// 0x10 (2.x): VU address of the extra common vertex info (`vertex_info_common_addr + positions_common_count`); never read by VU1.
    pub(crate) unk_10: u16,
    /// 0x12 (2.y): VU address of the LOD-01 vertex info.
    pub(crate) vertex_info_lod_01_addr: u16,
    /// 0x14 (2.z): VU address of the extra LOD-01 vertex info (`vertex_info_lod_01_addr + positions_lod_01_count`), walked by the L48 pass.
    pub(crate) unk_14: u16,
    /// 0x16 (2.w): VU address of the LOD-0 vertex info.
    pub(crate) vertex_info_lod_0_addr: u16,
    /// 0x18 (3.x): VU address of the extra LOD-0 vertex info (`vertex_info_lod_0_addr + positions_lod_0_count`), walked by the L47 pass.
    pub(crate) unk_18: u16,
    /// 0x1a (3.y): VU address of the (shared) strip index array.
    pub(crate) indices_addr: u16,
    /// 0x1c (3.z): VU address of the LOD-01 parent indices.
    pub(crate) parent_indices_lod_01_addr: u16,
    /// 0x1e (3.w): VU address of the LOD-01 "unknown indices 2".
    pub(crate) unk_indices_2_lod_01_addr: u16,
    /// 0x20 (4.x): VU address of the LOD-0 parent indices.
    pub(crate) parent_indices_lod_0_addr: u16,
    /// 0x22 (4.y): VU address of the LOD-0 "unknown indices 2".
    pub(crate) unk_indices_2_lod_0_addr: u16,
    /// 0x24 (4.z): VU address of the (shared) strip array.
    pub(crate) strips_addr: u16,
    /// 0x26 (4.w): VU address of the ad-gif array.
    pub(crate) texture_ad_gifs_addr: u16,
}
const _: () = assert!(std::mem::size_of::<TfragVuHeader>() == 0x28);

/// Vertex position (V3_16 signed, 6 bytes), local to [`Tfrag::origin`]. 1024 units = 1 world unit.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TfragPosition {
    /// 0x00: x offset from the origin.
    pub x: i16,
    /// 0x02: y offset.
    pub y: i16,
    /// 0x04: z offset (Z is up).
    pub z: i16,
}

/// Vertex info (V4_16 signed, 8 bytes). Strip indices index this array.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TfragVertexInfo {
    /// 0x00: texture S, signed 1/4096 fixed point.
    pub s: i16,
    /// 0x02: texture T, signed 1/4096 fixed point.
    pub t: i16,
    /// 0x04: VU qword offset (from `positions_common_addr`) of the second LOD parent's position; position index = `parent / 2`. Meaningless for common entries.
    pub(crate) parent: i16,
    /// 0x06: VU qword offset of this vertex's own position; position index = `vertex / 2`.
    pub(crate) vertex: i16,
}

/// Strip command record (V4_8 signed, 4 bytes). Spec 2.6.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct TfragStrip {
    /// 0x00: vertex count; biased by -128 when the record also carries an action; 0 ends the list.
    pub(crate) vertex_count_and_flag: i8,
    /// 0x01: >= 0 = load ad-gif, negative = XGKICK / end of GS packet (see [`tfrag_triangles`]).
    pub(crate) end_of_packet_flag: i8,
    /// 0x02: quadword offset into the ad-gif array (ad-gif index = `z / 5`); on a kick record, -1 = keep
    /// the current texture, >= 0 = load this ad-gif into the new packet.
    pub(crate) ad_gif_offset: i8,
    /// 0x03: padding.
    pub(crate) pad: i8,
}

/// Per-vertex colour (4 bytes), indexed by position index. 0x80 = 1.0.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TfragRgba {
    /// 0x00: red.
    pub r: u8,
    /// 0x01: green.
    pub g: u8,
    /// 0x02: blue.
    pub b: u8,
    /// 0x03: alpha (0x80 = opaque).
    pub a: u8,
}

/// Per-vertex light/normal record (8 bytes), indexed by position index. Spec 4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TfragLight {
    /// 0x00: unknown.
    pub(crate) unknown_0: i8,
    /// 0x01: unknown ("intensity").
    pub(crate) intensity: i8,
    /// 0x02: normal azimuth, pi/128 rad per unit.
    pub(crate) azimuth: i8,
    /// 0x03: normal elevation, pi/128 rad per unit.
    pub(crate) elevation: i8,
    /// 0x04: unknown ("color").
    pub(crate) color: i16,
    /// 0x06: padding.
    pub(crate) pad: i16,
}

/// One GIF A+D quadword (16 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct AdGif {
    /// 0x00: low word of the 64-bit register value.
    pub data_lo: i32,
    /// 0x04: high word of the register value.
    pub data_hi: i32,
    /// 0x08: GS register address (0x06, 0x14, 0x08, 0x34, 0x36 for the five qwords in every retail ad-gif).
    pub(crate) address: u8,
    /// 0x09: padding; bytes 0x0c..0x10 are the qword's w lane (for TEX1 it is 0x45000000 = 2048.0f in every retail ad-gif, read by VU1 as the UV bias).
    pub(crate) pad: [u8; 7],
}

/// Texture primitive: 5 A+D quadwords (0x50 bytes) copied verbatim into the GS packet. The on-disc register
/// values are packed fields that the level-load tfrag init (boot 0x2040e0, level01 0x255470; spec 2.5.1)
/// rewrites into real GS values in place; see [`TfragAdGifs::gs_registers`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TfragAdGifs {
    /// 0x00: TEX0_1; on disc `data_lo` = index into `LevelCore::tfrag_textures`, `data_hi` = 0.
    pub(crate) tex0: AdGif,
    /// 0x10: TEX1_1; on disc `data_lo` = LOD K (s16, 1/16 units), `data_hi` = MMIN (4).
    pub(crate) tex1: AdGif,
    /// 0x20: CLAMP_1; on disc `data_lo` = WMS, `data_hi` = WMT (0 = repeat, 1 = clamp).
    pub clamp: AdGif,
    /// 0x30: MIPTBP1_1; 0 on disc, built from the texture entry at load.
    pub(crate) miptbp1: AdGif,
    /// 0x40: MIPTBP2_1; 0 on disc and at run time.
    pub(crate) miptbp2: AdGif,
}
const _: () = assert!(std::mem::size_of::<TfragAdGifs>() == 0x50);

/// Strips and index bytes of one LOD.
#[derive(Clone, Debug, Default)]
pub struct TfragLod {
    /// Strip command list (terminated by a zero-count record).
    pub(crate) strips: Vec<TfragStrip>,
    /// Indices into [`Tfrag::vertex_info`], zero-padded to a multiple of 4.
    pub(crate) indices: Vec<u8>,
}

/// One parsed tfrag.
#[derive(Clone, Debug, Default)]
pub struct Tfrag {
    pub header: TfragHeader,
    pub vu: TfragVuHeader,
    /// Integer origin (x, y, z, w) added to every position: the last origin STROW row, else the quadword at `light_ofs`.
    pub origin: [i32; 4],
    /// Concatenated common ‖ lod01 ‖ lod0. Index space of `rgba`, `lights` and `vertex_info.vertex / 2`.
    pub positions: Vec<TfragPosition>,
    /// Concatenated common ‖ lod01 ‖ lod0. Index space of the strip indices and parent indices.
    pub vertex_info: Vec<TfragVertexInfo>,
    /// Number of positions in each tier (as classified by the C++ walker).
    pub positions_common: u32,
    pub positions_lod01: u32,
    pub positions_lod0: u32,
    /// Number of vertex-info entries in each tier.
    pub vinfo_common: u32,
    pub vinfo_lod01: u32,
    pub vinfo_lod0: u32,
    /// For LOD-01 vertex-info entry `i` (`i < positions_lod_01_count`, zero-padded to 4): vertex-info index (into the whole
    /// `vertex_info`) of its first LOD parent, always a common entry; the second parent is the entry's `parent / 2` position. See [`Tfrag::lod_link`].
    pub parent_indices_lod01: Vec<u8>,
    /// Per extra LOD-01 vertex-info entry (`vu.unk_06` entries, padded to 4): its first-parent vertex-info index, used only by the
    /// VU1 collapse pass (L48), which replaces the entry with this one when both parents are beyond the LOD switch.
    pub unk_indices_2_lod01: Vec<u8>,
    /// For LOD-0 vertex-info entry `i` (`i < positions_lod_0_count`, padded to 4): vertex-info index of its first LOD parent (common or LOD-01).
    pub parent_indices_lod0: Vec<u8>,
    /// Per extra LOD-0 vertex-info entry (`vu.unk_0a` entries, padded to 4): first-parent / collapse replacement vertex-info index (VU1 L47).
    pub unk_indices_2_lod0: Vec<u8>,
    /// `[0]` = highest detail.
    pub lod: [TfragLod; 3],
    /// Texture primitives; strips select one by `ad_gif_offset / 5`.
    pub ad_gifs: Vec<TfragAdGifs>,
    /// `rgba_size * 4` colours, indexed by position index.
    pub rgba: Vec<TfragRgba>,
    /// `vert_count` light/normal records, indexed by position index.
    pub lights: Vec<TfragLight>,
    /// `msphere_count` texture spheres, raw; decode with [`Tfrag::texture_spheres`] (lane w is not a float). Not in the C++ struct.
    pub mspheres: Vec<[f32; 4]>,
    /// 8 x (4 x s16) at `cube_ofs`: clip-box corners, x/y/z * 64 = absolute raw units, w = 0. Not in the C++ struct.
    pub cube: [[i16; 4]; 8],
}

/// A triangle of vertex-info indices plus the active ad-gif index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TfragTriangle {
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub ad_gif: u16,
}

impl Tfrag {
    /// Position index of a vertex-info entry (`vertex / 2`), as the C++ export resolves it: out of range falls back to 0.
    pub fn position_index(&self, vinfo: usize) -> usize {
        let pi = self.vertex_info[vinfo].vertex as isize as usize / 2;
        if pi >= self.positions.len() { 0 } else { pi }
    }
    /// World-space position of a vertex-info entry, `(origin + local) / 1024`, bit-identical to the C++ OBJ export.
    pub fn world_position(&self, vinfo: usize) -> [f32; 3] {
        let p = self.positions[self.position_index(vinfo)];
        [
            (self.origin[0].wrapping_add(p.x as i32)) as f32 / 1024.0,
            (self.origin[1].wrapping_add(p.y as i32)) as f32 / 1024.0,
            (self.origin[2].wrapping_add(p.z as i32)) as f32 / 1024.0,
        ]
    }
    /// Level tfrag texture index (into `LevelCore::tfrag_textures`) of an ad-gif.
    pub fn texture_index(&self, ad_gif: usize) -> i32 { self.ad_gifs[ad_gif].tex0.data_lo }
}

fn records<T: Pod>(data: &[u8], count: u32) -> Result<Vec<T>> {
    let len = count as usize * std::mem::size_of::<T>();
    if data.len() < len { return invalid("unpack payload smaller than element count"); }
    Ok(bytemuck::pod_collect_to_vec(&data[..len]))
}

/// Mirrors the C++ `ListWalk`: classifies each unpack of a VIF list by format and VU address.
struct ListWalk {
    t: Tfrag,
    have_origin: bool,
    current_strips: Vec<TfragStrip>,
    current_indices: Vec<u8>,
}

impl ListWalk {
    fn new(t: Tfrag) -> Self { ListWalk { t, have_origin: false, current_strips: Vec::new(), current_indices: Vec::new() } }

    fn run(&mut self, list: &[u8]) -> Result<()> {
        if list.is_empty() { return Ok(()); }
        for p in parse_vif(list)? {
            if p.cmd == vif::STROW {
                let row: [i32; 4] = Buf(p.data).pod(0, "STROW row")?;
                // The tfrag origin row precedes every V3_16 position unpack; remember the last non-index row.
                if row[0] != 0x4500_0000 && row[0] as u32 != self.t.vu.vertex_info_common_addr as u32 {
                    self.t.origin = row;
                    self.have_origin = true;
                }
                continue;
            }
            if !p.is_unpack() { continue; }
            let a = p.addr();
            let n = p.count();
            let t = &mut self.t;
            let vu = t.vu;
            match (p.vn(), p.vl()) {
                (3, 1) if p.usn() && a == 0 => t.vu = Buf(p.data).pod(0, "tfrag VU header")?,
                (3, 0) => t.ad_gifs = records(p.data, n / 5)?,
                (2, 1) => {
                    let pos: Vec<TfragPosition> = records(p.data, n)?;
                    if a == vu.positions_common_addr {
                        t.positions_common = n;
                    } else if n != 0 && t.positions_lod01 == 0 && a != vu.positions_common_addr && t.positions_lod0 == 0
                        && a as u32 == (vu.positions_common_addr as u32).wrapping_add(2u32.wrapping_mul(t.positions_common)) {
                        t.positions_lod01 = n;
                    } else {
                        t.positions_lod0 = n;
                    }
                    t.positions.extend(pos);
                }
                (3, 1) => {
                    let vi: Vec<TfragVertexInfo> = records(p.data, n)?;
                    if a == vu.vertex_info_common_addr { t.vinfo_common = n; }
                    else if a == vu.vertex_info_lod_01_addr { t.vinfo_lod01 = n; }
                    else { t.vinfo_lod0 = n; }
                    t.vertex_info.extend(vi);
                }
                (3, 2) => {
                    let bytes = || p.data[..n as usize * 4].to_vec();
                    if !p.usn() && a == vu.strips_addr { self.current_strips = records(p.data, n)?; }
                    else if a == vu.indices_addr { self.current_indices = bytes(); }
                    // An empty region shares its VU address with the next one (e.g. with no LOD-01
                    // "unknown indices 2", unk_indices_2_lod_01_addr == parent_indices_lod_0_addr in
                    // ~90% of retail tfrags), so only match regions whose VU-header count is non-zero.
                    else if a == vu.parent_indices_lod_01_addr && vu.positions_lod_01_count != 0 { t.parent_indices_lod01 = bytes(); }
                    else if a == vu.unk_indices_2_lod_01_addr && vu.unk_06 != 0 { t.unk_indices_2_lod01 = bytes(); }
                    else if a == vu.parent_indices_lod_0_addr && vu.positions_lod_0_count != 0 { t.parent_indices_lod0 = bytes(); }
                    else if a == vu.unk_indices_2_lod_0_addr && vu.unk_0a != 0 { t.unk_indices_2_lod0 = bytes(); }
                    else { return invalid("unexpected V4_8 unpack address in tfrag list"); }
                }
                _ => return invalid("unexpected unpack format in tfrag list"),
            }
        }
        Ok(())
    }

    fn take_lod(&mut self, lod: usize) {
        self.t.lod[lod].strips = std::mem::take(&mut self.current_strips);
        self.t.lod[lod].indices = std::mem::take(&mut self.current_indices);
    }
}

/// Parses every tfrag of a tfrags block (the `"tfrags"` entry of `LevelCore::blocks`).
pub(crate) fn parse_tfrags(block: &[u8]) -> Result<Vec<Tfrag>> {
    let blk = Buf(block);
    let bh: TfragBlockHeader = blk.pod(0, "tfrag block header")?;
    if bh.tfrag_count < 0 || bh.tfrag_count > 100_000 || bh.table_offset < 0x10 { return invalid("implausible tfrag block header"); }
    let mut out = Vec::with_capacity(bh.tfrag_count as usize);
    for i in 0..bh.tfrag_count as usize {
        let header: TfragHeader = blk.pod(bh.table_offset as usize + i * std::mem::size_of::<TfragHeader>(), "tfrag header")?;
        let h = header;
        // C++ computes this in size_t; a negative `data` wraps and then fails the bounds check.
        let base = (bh.table_offset as usize).wrapping_add(h.data as isize as usize);
        let slice = |from: usize, to: usize| -> Result<&[u8]> {
            Ok(blk.sub(base.wrapping_add(from), to.wrapping_sub(from), "tfrag VIF list")?.bytes())
        };
        let lod0_start = h.shared_ofs as usize + h.lod_1_size as usize * 0x10;

        // The common list carries the VU header; parse it first so addresses are known.
        let mut w = ListWalk::new(Tfrag { header, ..Default::default() });
        w.run(slice(h.shared_ofs as usize, h.lod_1_ofs as usize)?)?;
        // Then the LOD lists in transfer order, on a fresh tfrag that already knows the VU header.
        let mut w2 = ListWalk::new(Tfrag { header, vu: w.t.vu, ..Default::default() });
        w2.run(slice(h.shared_ofs as usize, h.lod_1_ofs as usize)?)?; // common: header, ad-gifs, common vinfo, origin, common positions
        w2.run(slice(h.lod_2_ofs as usize, h.shared_ofs as usize)?)?; // LOD 2 strips/indices
        w2.take_lod(2);
        w2.run(slice(h.lod_1_ofs as usize, h.lod_0_ofs as usize)?)?; // LOD 1 strips/indices
        w2.take_lod(1);
        w2.run(slice(h.lod_0_ofs as usize, lod0_start)?)?; // LOD 0+1 parents, vinfo, positions
        w2.run(slice(lod0_start, h.rgba_ofs as usize)?)?; // LOD 0 positions, strips, indices, parents, vinfo
        w2.take_lod(0);
        let have_origin = w2.have_origin;
        let mut t = w2.t;

        t.rgba = blk.pod_slice(base.wrapping_add(h.rgba_ofs as usize), h.rgba_size as usize * 4, "tfrag rgba")?;
        let origin2: [i32; 4] = blk.pod(base.wrapping_add(h.light_ofs as usize), "tfrag origin")?;
        if !have_origin { t.origin = origin2; }
        t.lights = blk.pod_slice(base.wrapping_add(h.light_ofs as usize + 0x10), h.vert_count as usize, "tfrag lights")?;
        t.mspheres = blk.pod_slice(base.wrapping_add(h.msphere_ofs as usize), h.msphere_count as usize, "tfrag mspheres")?;
        t.cube = blk.pod(base.wrapping_add(h.cube_ofs as usize), "tfrag cube")?;
        out.push(t);
    }
    Ok(out)
}

/// The level's tfrags block: the `"tfrags"` entry of `core.blocks`, sliced from the decompressed core data.
pub fn tfrag_block<'a>(core: &LevelCore, core_data: &'a [u8]) -> Result<&'a [u8]> {
    let Some(b) = core.blocks.iter().find(|b| b.name == "tfrags") else { return invalid("level core has no tfrags block"); };
    Ok(Buf(core_data).sub(b.offset, b.size, "tfrags block")?.bytes())
}

/// Parses the tfrags of a level: [`tfrag_block`] then [`parse_tfrags`].
pub fn parse_level_tfrags(core: &LevelCore, core_data: &[u8]) -> Result<Vec<Tfrag>> { parse_tfrags(tfrag_block(core, core_data)?) }

/// Walks a LOD's strip list into triangles (GS triangle-strip semantics, alternating winding). Spec 2.6.
/// Indices are vertex-info indices. `lod` must be 0..=2 (0 = highest detail).
///
/// Texture assignment follows the VU1 strip processor (program 55907, EE 0x103578; L84/L93 for the
/// first record, L79-L82 / L122-L125 for the rest; program 903379 L4-L7 is identical):
/// * the first record always loads the ad-gif at `z` and has its count unbiased (`x + 128`);
/// * a later record with `x > 0` is a plain run of `x` vertices;
/// * `x == 0` ends the list;
/// * `x < 0` is a run of `x + 128` vertices; when `y >= 0` it loads the ad-gif at `z`, otherwise it
///   XGKICKs the packet and then loads the ad-gif at `z` if `z >= 0` (L81 `ibgez vi13, L82`), or
///   keeps the current texture if `z < 0` (-1 on disc).
///
/// `z` is a quadword offset into the ad-gif array; the triangle's `ad_gif` is `z / 5`. A load whose
/// offset is negative, not a multiple of 5 or past the array is an error.
pub fn tfrag_triangles(t: &Tfrag, lod: usize) -> Result<Vec<TfragTriangle>> {
    let mut tris = Vec::new();
    let l = &t.lod[lod];
    let mut cursor = 0usize;
    let mut ad_gif = 0u16;
    for (k, s) in l.strips.iter().enumerate() {
        let mut n = s.vertex_count_and_flag as i32;
        if k == 0 || n <= 0 {
            if k != 0 && n == 0 { break; }
            if k == 0 || s.end_of_packet_flag >= 0 || s.ad_gif_offset >= 0 {
                let z = s.ad_gif_offset as i32;
                if z < 0 || z % 5 != 0 || (z / 5) as usize >= t.ad_gifs.len() { return invalid("strip ad-gif offset outside the ad-gif array"); }
                ad_gif = (z / 5) as u16;
            }
            n += 128;
        }
        let n = n as usize;
        if cursor + n > l.indices.len() { return invalid("strip runs past index array"); }
        for i in 2..n {
            let (a, b, c) = (l.indices[cursor + i - 2] as u16, l.indices[cursor + i - 1] as u16, l.indices[cursor + i] as u16);
            tris.push(if i & 1 != 0 { TfragTriangle { a: b, b: a, c, ad_gif } } else { TfragTriangle { a, b, c, ad_gif } });
        }
        cursor += n;
    }
    Ok(tris)
}

// ---------------------------------------------------------------------------------------------
// Ad-gif decoding (spec 2.5.1). Source: the level-load tfrag init, boot 0x2040e0 (Lombyte
// `fun_002040e0`, called from `transition_load_wad`), level01 overlay copy 0x255470 (called from
// the level-core setup 0x258128), plus the per-frame `PatchTfragGifs` (boot 0x233308, level01 0x2a7c90).
// ---------------------------------------------------------------------------------------------

impl TfragAdGifs {

    /// TEX1_1 K field (bits 32..43) as the signed 12-bit raw value, 1/16 units (retail -137..-89).
    pub fn lod_k_raw(&self) -> i16 { (((self.tex1.data_lo as u32 & 0xfff) << 4) as u16 as i16) >> 4 }

}

/// Per-tfrag draw path chosen by `TfragProc` (boot 0x233fb0, level01 0x2a7e58); the value is the MSCAL
/// address it writes into the VIF packet for the tfrag VU1 program (0x103578). See spec "LOD selection and morphing".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TfragDrawMode {
    /// MSCAL 6: LOD 2 list, no morph.
    Lod2,
    /// MSCAL 8: LOD 1 lists; LOD-01 vertices morph, and collapse onto parent 1 when both parents are beyond D0.
    Lod1Collapse,
    /// MSCAL 0xa: LOD 1 lists; LOD-01 vertices morph.
    Lod1Morph,
    /// MSCAL 0xe: LOD 0 lists; LOD-01 morph, LOD-0 morph with collapse at D1.
    Lod0Collapse,
    /// MSCAL 0x10: LOD 0 lists; LOD-01 at rest, LOD-0 morph.
    Lod0Morph,
    /// MSCAL 0x14: LOD 0 lists, no morph.
    Lod0,
}

impl TfragDrawMode {
    /// MSCAL address written by `TfragProc`.
    pub fn mscal(self) -> u8 {
        match self { Self::Lod2 => 6, Self::Lod1Collapse => 8, Self::Lod1Morph => 0xa, Self::Lod0Collapse => 0xe, Self::Lod0Morph => 0x10, Self::Lod0 => 0x14 }
    }
}

/// Which morphing tier a vertex-info entry belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TfragMorphTier {
    /// LOD-01 tier: weights from VU qw 666/668, fades between D1 and D0.
    Lod01,
    /// LOD-0 tier: weights from VU qw 667/669, fades between D2 and D1.
    Lod0,
}

/// LOD linkage of one LOD-01 or LOD-0 vertex-info entry (spec "LOD selection and morphing").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TfragLodLink {
    pub tier: TfragMorphTier,
    /// True for the first `positions_lod_XX_count` entries of the tier, which the VU morphs (writing the result
    /// into `own_position`'s slot, so every entry sharing that position sees it). Extra entries only collapse.
    pub morphs: bool,
    /// Position index of this entry (`vertex / 2`).
    pub own_position: usize,
    /// Vertex-info index of parent 1 (`parent_indices_*` for primary entries, `unk_indices_2_*` for extras).
    /// Also the collapse replacement: the whole entry (UV and position) is overwritten with this one.
    pub parent1_vinfo: usize,
    /// Position index of parent 1 (`vertex_info[parent1_vinfo].vertex / 2`).
    pub parent1_position: usize,
    /// Position index of parent 2 (this entry's `parent / 2`).
    pub parent2_position: usize,
}

impl TfragBlockHeader {
    /// LOD switch distances `[D0, D1, D2] = [6L, 4L, 2L]` in world units, L = `unknown_8`
    /// (boot 0x2040e0 writes them to 0x160ea0; `set_tfrag_dists` 0x233068 derives everything else).
    pub fn lod_distances(&self) -> [f32; 3] {
        let l = self.unknown_8;
        [l * 6.0, l * 4.0, l + l]
    }
    /// The integer thresholds `TfragProc` compares against: `trunc(D * 1024)` (raw units), stored at 0x160eb0.
    pub fn lod_thresholds_raw(&self) -> [i32; 3] { self.lod_distances().map(|d| (d * 1024.0) as i32) }
}

/// Reads the 0x10-byte header of a tfrags block.
pub fn parse_tfrag_block_header(block: &[u8]) -> Result<TfragBlockHeader> { Buf(block).pod(0, "tfrag block header") }

impl Tfrag {

    /// The draw path `TfragProc` picks for this tfrag (ignoring frustum culling and the guard-band clip test,
    /// which sends tfrags crossing the guard band to the clipping program at full LOD 0 without morphing).
    /// `centre_depth_raw` = view-space depth of `header.bsphere`'s centre in raw units; `thresholds_raw` from
    /// [`TfragBlockHeader::lod_thresholds_raw`]. near/far = depth -/+ radius, truncated to integers (`vftoi0`).
    pub fn draw_mode(&self, centre_depth_raw: f32, thresholds_raw: [i32; 3]) -> TfragDrawMode {
        let r = self.header.bsphere[3];
        let far = (centre_depth_raw + r) as i32;
        let near = (centre_depth_raw - r) as i32;
        let [d0, d1, d2] = thresholds_raw;
        if self.header.base_only != 0 || near >= d0 { TfragDrawMode::Lod2 }
        else if far >= d0 { TfragDrawMode::Lod1Collapse }
        else if near >= d1 { TfragDrawMode::Lod1Morph }
        else if far >= d1 { TfragDrawMode::Lod0Collapse }
        else if near >= d2 || far >= d2 { TfragDrawMode::Lod0Morph }
        else { TfragDrawMode::Lod0 }
    }

    /// LOD linkage of vertex-info entry `vinfo` (index into the concatenated `vertex_info`), or `None` for common
    /// entries, out-of-range indices, or padding. Mirrors VU1 L17-L36 (morph/collapse) and L47/L48 (extras).
    pub fn lod_link(&self, vinfo: usize) -> Option<TfragLodLink> {
        let vc = self.vinfo_common as usize;
        let l01 = vc..vc + self.vinfo_lod01 as usize;
        let l0 = l01.end..l01.end + self.vinfo_lod0 as usize;
        let (tier, k, primaries, parents, extras) = if l01.contains(&vinfo) {
            (TfragMorphTier::Lod01, vinfo - l01.start, self.vu.positions_lod_01_count as usize, &self.parent_indices_lod01, &self.unk_indices_2_lod01)
        } else if l0.contains(&vinfo) {
            (TfragMorphTier::Lod0, vinfo - l0.start, self.vu.positions_lod_0_count as usize, &self.parent_indices_lod0, &self.unk_indices_2_lod0)
        } else {
            return None;
        };
        let (morphs, p1) = if k < primaries { (true, *parents.get(k)?) } else { (false, *extras.get(k - primaries)?) };
        let p1 = p1 as usize;
        let e = self.vertex_info.get(vinfo)?;
        let pos = |v: i16| -> Option<usize> { let p = v as isize as usize / 2; (p < self.positions.len()).then_some(p) };
        Some(TfragLodLink {
            tier, morphs,
            own_position: pos(e.vertex)?,
            parent1_vinfo: p1,
            parent1_position: pos(self.vertex_info.get(p1)?.vertex)?,
            parent2_position: pos(e.parent)?,
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;




    #[test]
    fn kick_records_with_z_switch_texture() {
        let st = |x: i8, y: i8, z: i8| TfragStrip { vertex_count_and_flag: x, end_of_packet_flag: y, ad_gif_offset: z, pad: 0 };
        let mut t = Tfrag { ad_gifs: vec![TfragAdGifs::default(); 3], ..Default::default() };
        // load ad-gif 1 + 3 verts; kick without texture (z < 0) + 3 verts; kick with ad-gif 2 + 3 verts;
        // plain run of 3; load ad-gif 0 + 3 verts; end.
        t.lod[0].strips = vec![st(-125, 0, 5), st(-125, -128, -1), st(-125, -128, 10), st(3, 0, 0), st(-125, 0, 0), st(0, 0, 0)];
        t.lod[0].indices = (0..15).collect();
        let g: Vec<u16> = tfrag_triangles(&t, 0).unwrap().iter().map(|t| t.ad_gif).collect();
        assert_eq!(g, vec![1, 1, 2, 2, 0]);
        // The first record always loads its ad-gif and is always biased (VU1 L84), whatever `y` says.
        t.lod[0].strips = vec![st(-125, -128, 5), st(0, 0, 0)];
        assert_eq!(tfrag_triangles(&t, 0).unwrap().iter().map(|t| t.ad_gif).collect::<Vec<_>>(), vec![1]);
        // A load past the ad-gif array, or off a record boundary, is rejected.
        t.lod[0].strips = vec![st(-125, 0, 15), st(0, 0, 0)];
        assert!(tfrag_triangles(&t, 0).is_err());
        t.lod[0].strips = vec![st(-125, 0, 5), st(-125, -128, 7), st(0, 0, 0)];
        assert!(tfrag_triangles(&t, 0).is_err());
    }



}
