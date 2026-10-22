//! Moby (animated character / dynamic object) classes. Spec: docs/formats/moby_rac1.md, with the
//! skinning/lighting corrections from docs/plan/moby_skinning_lighting.md. Ported
//! from the retired C++ reference extractor (git 2230812; `parse_moby_class`, `moby_normal`, the index walk);
//!
//! What the loader resolves at load time, so a renderer never has to replay the PS2 machinery:
//! * the 9-bit vertex-cache ids (stored 7 vertices late, spec 2.9) and duplicate vertices (2.8);
//! * the VU0 matrix-slot machine (skinning doc 4-5) into per-vertex [`MobySkin`]
//!   (up to 3 joint-palette indices, integer weights summing to 256);
//! * the index stream (secret indices, texture switches, flush trailer) into [`MobyTriangle`]s.
//!
//! Not decoded here: animation sequences/keyframes (spec 4; the keyframe decode in the game's
//! `fun_0020e0e0` is not reversed yet), collision (5), sound defs, shadow, joints lists, bangles
//! (unused on the RAC1 disc) and the separately compressed RAC1 gadget classes.

use crate::buf::{invalid, Buf, Result};
use crate::level::{ClassEntry, LevelCore};
use crate::vif::parse_vif;
use bytemuck::{Pod, Zeroable};

/// Moby class header at byte 0 of a class blob (0x48 bytes). Spec 1.2. Pointers are byte offsets
/// from the start of the class blob; 0 means absent.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyClassHeader {
    /// 0x00: packet (submesh) table; 0 = no mesh.
    pub(crate) packet_table_offset: i32,
    /// 0x04: high-LOD packet count (first in the table).
    pub(crate) high_lod_count: u8,
    /// 0x05: low-LOD packet count (after the high-LOD entries).
    pub(crate) low_lod_count: u8,
    /// 0x06: metal ("shine") packet count.
    pub metal_count: u8,
    /// 0x07: table index of the first metal packet (= high + low on the disc).
    pub(crate) metal_begin: u8,
    /// 0x08: joint count used with the high LOD (size of `skeleton` / `common_trans`).
    pub joint_count: u8,
    /// 0x09: joint count used with the low LOD (skinning doc 2).
    pub low_lod_joint_count: u8,
    /// 0x0a: first glow packet of the high-LOD list (0xff / past the end: none); docs/plan/moby_skinning_lighting.md §10.
    pub rac1_byte_a: u8,
    /// 0x0b: first glow packet of the low-LOD list (0xff / past the end: none).
    pub rac1_byte_b: u8,
    /// 0x0c: animation sequence count; the pointer list is at 0x48.
    pub sequence_count: u8,
    /// 0x0d: sound definition count.
    pub(crate) sound_count: u8,
    /// 0x0e: LOD switch distance; the game uses the low LOD beyond `lod_trans * 1024` depth.
    pub lod_trans: u8,
    /// 0x0f: number of 16-byte shadow entries just before `skeleton`.
    pub shadow: u8,
    /// 0x10: collision data: class-relative offset of the collision blob (0 = none), copied to `moby+0x94` by
    /// `InitMobyInstance`; parsed by [`crate::moby_collision::MobyCollision::of_class`].
    pub collision: i32,
    /// 0x14: `joint_count` Mat4 skeleton matrices (the game uses them as inverse bind matrices).
    pub(crate) skeleton: i32,
    /// 0x18: `joint_count` x [`MobyTrans`].
    pub(crate) common_trans: i32,
    /// 0x1c: joints lists (spec 3.3).
    pub joints: i32,
    /// 0x20: GIF usage table (spec 6.2).
    pub(crate) gif_usage: i32,
    /// 0x24: world units = packed position * scale / 1024.
    pub scale: f32,
    /// 0x28: sound definitions.
    pub(crate) sound_defs: i32,
    /// 0x2c: bangles block, in quadwords (0 on every RAC1 class).
    pub(crate) bangles: u8,
    /// 0x2d: mipmap distance.
    pub(crate) mip_dist: u8,
    /// 0x2e: unknown in RAC1.
    pub(crate) rac1_short_2e: i16,
    /// 0x30: bounding sphere centre xyz and radius, world units.
    pub bsphere: [f32; 4],
    /// 0x40: glow colour (RGBA, R low); non-zero → the moby's mode 0x10 and +0x90 (`InitMobyInstance`).
    pub glow_rgba: i32,
    /// 0x44: class mode bits, unknown.
    pub mode_bits: i16,
    /// 0x46: unknown type tag.
    pub ty: u8,
    /// 0x47: more mode bits, unknown.
    pub mode_bits2: u8,
}
const _: () = assert!(std::mem::size_of::<MobyClassHeader>() == 0x48);

/// Packet table entry (0x10 bytes). Spec 2.1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyPacketEntry {
    /// 0x0: class-relative offset of the VIF list.
    pub(crate) vif_list_offset: u32,
    /// 0x4: VIF list size in quadwords.
    pub(crate) vif_list_size: u16,
    /// 0x6: quadword offset of the ad-gif unpack in the list (0 = none).
    pub(crate) vif_list_texture_unpack_offset: u16,
    /// 0x8: class-relative offset of the vertex table header.
    pub(crate) vertex_offset: u32,
    /// 0xc: vertex data size in quadwords, header included.
    pub(crate) vertex_data_size: u8,
    /// 0xd: `(0xf + 6 * transfer_vertex_count) / 16`, the qwc of the positions DMA.
    pub(crate) unknown_d: u8,
    /// 0xe: `(3 + transfer_vertex_count) / 4`, the qwc of the colours DMA.
    pub(crate) unknown_e: u8,
    /// 0xf: vertices sent to VU1 (in-file + duplicates).
    pub(crate) transfer_vertex_count: u8,
}
const _: () = assert!(std::mem::size_of::<MobyPacketEntry>() == 0x10);

/// RAC1 vertex table header (0x20 bytes) of a regular packet. Spec 2.7.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MobyVertexTableHeader {
    /// 0x00: pre-loop matrix transfers following this header.
    pub(crate) matrix_transfer_count: u32,
    /// 0x04: two-way blend vertices (first in the vertex array).
    pub(crate) two_way_blend_vertex_count: u32,
    /// 0x08: three-way blend vertices (next).
    pub(crate) three_way_blend_vertex_count: u32,
    /// 0x0c: single-matrix vertices (last).
    pub(crate) main_vertex_count: u32,
    /// 0x10: duplicate vertex entries.
    pub(crate) duplicate_vertex_count: u32,
    /// 0x14: in-file + duplicate vertices.
    pub transfer_vertex_count: u32,
    /// 0x18: byte offset of the vertex array from this header.
    pub(crate) vertex_table_offset: u32,
    /// 0x1c: byte offset of the RGBA multiplier blob from this header (also ends the epilogue).
    pub(crate) unknown_e: u32,
}
const _: () = assert!(std::mem::size_of::<MobyVertexTableHeader>() == 0x20);

/// Metal packet vertex table header (0x10 bytes). Skinning doc 6.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct MobyMetalVertexTableHeader {
    /// 0x0: vertex count; the 16-byte metal vertices follow the header.
    pub(crate) vertex_count: i32,
    /// 0x4: output offset of the positions (EE driver output buffer).
    pub(crate) unknown_4: i32,
    /// 0x8: output offset of the colours.
    pub(crate) unknown_8: i32,
    /// 0xc: total output bytes (ST at 0).
    pub(crate) unknown_c: i32,
}
const _: () = assert!(std::mem::size_of::<MobyMetalVertexTableHeader>() == 0x10);

/// Pre-loop matrix transfer: `VU0[vu0_dest_addr] = palette[spr_joint_index]`. Spec 2.10.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct MobyMatrixTransfer {
    pub(crate) spr_joint_index: u8,
    /// VU0 quadword address (multiple of 4).
    pub(crate) vu0_dest_addr: u8,
}

/// Per-joint `common_trans` record (0x10 bytes). Spec 3.2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct MobyTrans {
    /// 0x0: per-joint vector, role unknown.
    pub(crate) vector: [f32; 3],
    /// 0xc: parent joint index * 0x40 (byte offset into the skeleton table).
    pub(crate) parent_offset: u16,
    /// 0xe: unknown, usually 0x70.
    pub(crate) seventy: u16,
}
const _: () = assert!(std::mem::size_of::<MobyTrans>() == 0x10);

/// Resolved skin attributes of one vertex: position = sum_k weights[k]/256 * (F[joints[k]] * p),
/// with F the per-moby joint palette (F_j = pose_j * skeleton_j; identity F gives the stored pose).
/// Only the first `count` entries are meaningful; unused entries are 0.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobySkin {
    /// 1..=3.
    pub count: u8,
    /// Joint-palette indices (the game's SPR joint slots; joint 0 is identity for unanimated classes).
    pub joints: [u8; 3],
    /// Integer weights, sum exactly 256.
    pub weights: [u16; 3],
}

impl MobySkin {
    pub(crate) fn joint(j: u8) -> Self { MobySkin { count: 1, joints: [j, 0, 0], weights: [256, 0, 0] } }
}

/// Vertex kind (spec 2.9 types; metal from skinning doc 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MobyVertexKind { TwoWay = 1, ThreeWay = 2, Single = 3, Metal = 4 }

/// One vertex as sent to VU1.
#[derive(Clone, Copy, Debug)]
pub struct MobyVertex {
    /// Regular: bytes 0-7 of the record (skinning bytes, spec 2.9). Metal: bytes 8-15
    /// (joint[3], count, weight[3], pad). Duplicates carry their source vertex's bytes.
    pub raw: [u8; 8],
    pub normal_azimuth: u8,
    pub normal_elevation: u8,
    /// Packed position; world units = value * class scale / 1024.
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) z: i16,
    /// 9-bit vertex-cache id after undoing the 7-vertex shift; metal: the vertex number.
    pub id: u16,
    pub kind: MobyVertexKind,
    /// Copied from the vertex cache (spec 2.8): position, normal and skin of an earlier vertex.
    pub duplicate: bool,
    pub skin: MobySkin,
    /// Texture coordinate, 4.12 fixed point (`/ 4096.0`); 0 for metal (sphere-mapped at runtime).
    pub st: [i16; 2],
}

impl MobyVertex {
    pub fn packed_position(&self) -> [i16; 3] { [self.x, self.y, self.z] }
}

/// One triangle: indices into the packet's `vertices`, in index-stream order (strip positions
/// n-2, n-1, n; winding is not normalised), and the texture in effect when it was kicked.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MobyTriangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    /// TEX0 `data_lo`: index into the class's 16-entry texture list
    /// (`ClassEntry::textures`), or -1 none, -2 chrome, -3 glass.
    pub texture: i32,
}

/// One packet ("submesh"): up to ~0x60 vertices that VU1 processes in one go.
#[derive(Clone, Debug)]
pub struct MobySubmesh {
    pub entry: MobyPacketEntry,
    /// Zeroed for metal packets.
    pub vertex_table: MobyVertexTableHeader,
    /// Zeroed for regular packets.
    pub(crate) metal_header: MobyMetalVertexTableHeader,
    pub(crate) transfers: Vec<MobyMatrixTransfer>,
    /// Raw duplicate entries (`cache id << 7`).
    pub(crate) duplicates: Vec<u16>,
    /// The vertex records as stored: in-file + epilogue (regular) or the metal records.
    pub(crate) raw_vertices: Vec<u8>,
    /// Going Commando: the vertex table header's word 7, the glow vertices: the driver (L00 `0x2dd410` → `0x2de438`)
    /// writes the moby's glow word (job `+0xac`, MobyProc's for mode 0x10) over the packet's last that many output
    /// colours (in-file vertices, then duplicates). 0 for R&C1 (class byte 0xa picks whole glow packets) and for an
    /// old-format R&C2 class (its loader zeroes the word).
    pub glow_vertices: u32,
    /// The RAC1 `unknown_e` blob as stored: RGBA lighting multiplier per transfer vertex
    /// (0x80 = 1.0), zero-padded to 16 bytes. Every multiplier on the disc is 0x80. Empty for metal.
    pub(crate) rgba_multipliers: Vec<u8>,
    /// In-file vertices, then duplicates.
    pub vertices: Vec<MobyVertex>,
    /// Raw ST unpack (s, t pairs), may be longer than `vertices`; empty for metal.
    pub(crate) st: Vec<[i16; 2]>,
    /// Index bytes after the 4-byte index header, including the terminator and padding.
    pub(crate) index_bytes: Vec<u8>,
    /// Index header byte 2, then byte 0xc of the first `ad_gif count` quadwords of the ad-gif unpack.
    pub(crate) secret_indices: Vec<u8>,
    /// TEX0 `data_lo` of each ad-gif block, in switch order.
    pub(crate) texture_indices: Vec<i32>,
    /// Texture in effect at packet start (GS state left by the previous packet of the list; -1 for the first).
    pub(crate) initial_texture: i32,
    /// Duplicates whose cache slot was never written (always 0 on the disc).
    pub(crate) unresolved_duplicates: u32,
    pub triangles: Vec<MobyTriangle>,
    pub(crate) is_metal: bool,
}

impl MobySubmesh {
    /// One RGBA multiplier per vertex of `vertices` (in-file then duplicates; duplicates are not
    /// re-lit by the game, they copy their source's colour). Empty for metal packets.
    pub fn rgba_multiplier_records(&self) -> &[[u8; 4]] {
        let n = if self.is_metal { 0 } else { self.vertex_table.transfer_vertex_count as usize * 4 };
        bytemuck::cast_slice(&self.rgba_multipliers[..n.min(self.rgba_multipliers.len())])
    }
}

/// Skeleton: `joint_count` matrices and hierarchy records.
#[derive(Clone, Debug, Default)]
pub(crate) struct MobySkeleton {
    /// Row-major Mat4 as stored (rows 0-2 rotation, row 3 translation in packed units).
    pub(crate) matrices: Vec<[[f32; 4]; 4]>,
    pub(crate) trans: Vec<MobyTrans>,
}

#[derive(Clone, Debug)]
pub struct MobyClass {
    pub header: MobyClassHeader,
    pub sequence_pointers: Vec<i32>,
    pub high_lod: Vec<MobySubmesh>,
    pub low_lod: Vec<MobySubmesh>,
    pub metal: Vec<MobySubmesh>,
    pub(crate) skeleton: MobySkeleton,
    /// Going Commando's break pieces ([`MobyPiece`]; empty on R&C1), by piece number (None: no such piece).
    pub pieces: Vec<Option<MobyPiece>>,
}

/// One break piece of a Going Commando class: the packets a moby drawn as this piece shows (its `+0x7c` mask, L00
/// `0x318fd0`), and the piece's centre and points for its debris (`0x311cf0`), in 1/1024 of the moby's scale.
///
/// * The packets: class byte 0x2c gives a table at `16·byte`; `+4 + 4k` holds piece k's high-LOD range `{first,
///   count}` and its low-LOD range (the packets follow the class's LOD and metal lists).
/// * The points: class s16 0x2e gives a table at `16·word`; its byte `k + 1` is piece k's record (in quadwords from
///   the table, 0xff none): the centre (f32 x, y, z, w), then the points (s16 x, y, z, w; the first's w is their count).
#[derive(Clone, Debug, Default)]
pub struct MobyPiece {
    pub packets: Vec<MobySubmesh>,
    pub centre: [f32; 3],
    pub points: Vec<[f32; 3]>,
}

/// The most pieces a class has (the mask's bits 0..14).
pub(crate) const MAX_PIECES: usize = 15;

/// [`MobyPiece`]s of a Going Commando class blob.
fn read_pieces(b: Buf, h: &MobyClassHeader) -> Result<Vec<Option<MobyPiece>>> {
    let ranges = b.u8(0x2c)? as usize * 16;
    let points = b.i16(0x2e)?;
    if ranges == 0 || h.packet_table_offset <= 0 { return Ok(Vec::new()); }
    let mut out = Vec::new();
    for k in 0..MAX_PIECES {
        let o = ranges + 4 + 4 * k;
        let (first, count) = (b.u8(o)? as usize, b.u8(o + 1)? as usize);
        if count == 0 { out.push(None); continue; }
        let entries: Vec<MobyPacketEntry> = b.pod_slice(h.packet_table_offset as usize + first * std::mem::size_of::<MobyPacketEntry>(), count, "moby piece packets")?;
        let mut st = ListState::new();
        let packets = entries.iter().map(|e| read_packet(false, b, e, false, &mut st)).collect::<Result<Vec<_>>>()?;
        let mut piece = MobyPiece { packets, ..Default::default() };
        // The points (a record that does not read leaves the piece without: the classes have such entries for pieces
        // their breaks never ask for).
        let read = |k: usize| -> Result<([f32; 3], Vec<[f32; 3]>)> {
            let table = points as usize * 16;
            let q = b.u8(table + 1 + k)?;
            if q == 0xff { return Ok(([0.0; 3], Vec::new())); }
            let r = table + q as usize * 16;
            let n = b.i16(r + 0x16)?;
            if !(0..=64).contains(&n) { return invalid("moby piece point count"); }
            let pts = (0..n as usize).map(|j| -> Result<[f32; 3]> { let p = r + 0x10 + 8 * j; Ok([b.i16(p)? as f32, b.i16(p + 2)? as f32, b.i16(p + 4)? as f32]) }).collect::<Result<_>>()?;
            Ok(([b.f32(r)?, b.f32(r + 4)?, b.f32(r + 8)?], pts))
        };
        if 0 < points { if let Ok((c, pts)) = read(k) { (piece.centre, piece.points) = (c, pts); } }
        out.push(Some(piece));
    }
    while out.last().is_some_and(Option::is_none) { out.pop(); }
    Ok(out)
}

/// 9-bit vertex-cache id of a duplicate entry (`index << 7`). Spec 2.8.
pub(crate) fn duplicate_cache_id(entry: u16) -> u16 { (entry >> 7) & 0x1ff }

/// Undoes the 7-vertex id shift (spec 2.9): record i carries the id of vertex i-7; the ids of the
/// last vertices continue in the epilogue records and then in the six u16 at bytes 4..16 of the
/// last epilogue record. `records` are the in-file + epilogue 16-byte records.
pub(crate) fn vertex_cache_ids(records: &[[u8; 16]], in_file: usize) -> Result<Vec<u16>> {
    let mut ids: Vec<u16> = records.iter().skip(7).map(|r| u16::from_le_bytes([r[0], r[1]]) & 0x1ff).collect();
    if let Some(last) = records.last() {
        for k in 0..6 {
            if ids.len() >= in_file { break; }
            ids.push(u16::from_le_bytes([last[4 + k * 2], last[5 + k * 2]]) & 0x1ff);
        }
    }
    if ids.len() < in_file { return invalid("moby packet: not enough vertex ids"); }
    ids.truncate(in_file);
    Ok(ids)
}

/// VU0 data memory of program 104691 as matrix slots (skinning doc 4-5): each quadword address a
/// matrix was stored at holds one palette matrix (a transfer) or a 2-/3-way blend of them.
/// Carried across the packets of one LOD list.
pub(crate) struct Vu0Slots { slot: [Option<MobySkin>; 256] }

impl Default for Vu0Slots { fn default() -> Self { Vu0Slots { slot: [None; 256] } } }

impl Vu0Slots {
    pub(crate) fn store(&mut self, addr: u8, s: MobySkin) -> Result<()> {
        if !addr.is_multiple_of(4) { return invalid("moby skin: unaligned VU0 store address"); }
        self.slot[addr as usize] = Some(s);
        Ok(())
    }
    pub(crate) fn load(&self, addr: u8) -> Result<MobySkin> {
        if !addr.is_multiple_of(4) { return invalid("moby skin: unaligned VU0 load address"); }
        if addr >= 0xf4 { return invalid("moby skin: load from the VU0 sink/constant area"); }
        match self.slot[addr as usize] {
            Some(s) => Ok(s),
            None => invalid("moby skin: load from a VU0 slot that was never written"),
        }
    }
    fn load_joint(&self, addr: u8) -> Result<u8> {
        let s = self.load(addr)?;
        if s.count != 1 { return invalid("moby skin: blend input is itself a blend"); }
        Ok(s.joints[0])
    }

    /// Runs one regular vertex record (bytes 0-7) of the given kind through the slot machine and
    /// returns its skin. Two-way: store palette[b1>>1] at b6, blend b2/b3 with weights b4/b5, store
    /// at b7. Three-way: blend b2/b3/(b1 & 0xfe) with b4/b5/b6, store at b7. Single: store
    /// palette[b1>>1] at b3, then load b2. Stores happen before loads.
    pub(crate) fn vertex(&mut self, kind: MobyVertexKind, r: &[u8; 8]) -> Result<MobySkin> {
        let joint = r[1] >> 1;
        match kind {
            MobyVertexKind::TwoWay => {
                self.store(r[6], MobySkin::joint(joint))?;
                let (a, b) = (self.load_joint(r[2])?, self.load_joint(r[3])?);
                if r[4] as u16 + r[5] as u16 != 256 { return invalid("moby skin: 2-way weights do not sum to 256"); }
                let m = MobySkin { count: 2, joints: [a, b, 0], weights: [r[4] as u16, r[5] as u16, 0] };
                self.store(r[7], m)?;
                Ok(m)
            }
            MobyVertexKind::ThreeWay => {
                let (a, b, c) = (self.load_joint(r[2])?, self.load_joint(r[3])?, self.load_joint(r[1] & 0xfe)?);
                if r[4] as u16 + r[5] as u16 + r[6] as u16 != 256 { return invalid("moby skin: 3-way weights do not sum to 256"); }
                let m = MobySkin { count: 3, joints: [a, b, c], weights: [r[4] as u16, r[5] as u16, r[6] as u16] };
                self.store(r[7], m)?;
                Ok(m)
            }
            MobyVertexKind::Single => {
                self.store(r[3], MobySkin::joint(joint))?;
                self.load(r[2])
            }
            MobyVertexKind::Metal => invalid("moby skin: metal vertices do not use VU0 slots"),
        }
    }
}

/// Skin of a metal vertex from its bytes 8-15 (joint[3], count, weight[3], pad): count <= 1 is a
/// single joint (byte 8), otherwise `count` joints whose weights sum to 256. Skinning doc 6.
pub(crate) fn metal_skin(r: &[u8; 8]) -> Result<MobySkin> {
    let n = r[3];
    if n <= 1 { return Ok(MobySkin::joint(r[0])); }
    if n > 3 { return invalid("moby metal vertex: joint count > 3"); }
    let mut s = MobySkin { count: n, ..Default::default() };
    for k in 0..n as usize { s.joints[k] = r[k]; s.weights[k] = r[4 + k] as u16; }
    if s.weights.iter().sum::<u16>() != 256 { return invalid("moby metal vertex: weights do not sum to 256"); }
    Ok(s)
}

fn walk_indices(sub: &MobySubmesh) -> Result<(Vec<MobyTriangle>, i32)> {
    struct Push { idx: u32, nokick: bool, texture: i32 }
    let mut texture = sub.initial_texture;
    let mut pushes: Vec<Push> = Vec::new();
    let (mut secret_i, mut adgif_i, mut ended) = (0usize, 0usize, false);
    for &b in &sub.index_bytes {
        let (idx, nokick) = if b == 0 {
            let Some(&s) = sub.secret_indices.get(secret_i) else { return invalid("moby packet: ran out of secret indices") };
            secret_i += 1;
            if s == 0 { ended = true; break; }
            let Some(&t) = sub.texture_indices.get(adgif_i) else { return invalid("moby packet: texture switch without an ad-gif block") };
            texture = t;
            adgif_i += 1;
            ((s & 0x7f) as u32, true)
        } else {
            ((b & 0x7f) as u32, b & 0x80 != 0)
        };
        if idx == 0 || idx as usize > sub.vertices.len() { return invalid("moby packet: vertex index out of range"); }
        pushes.push(Push { idx: idx - 1, nokick, texture });
    }
    if !ended { return invalid("moby packet: index stream not terminated"); }
    if pushes.len() < 3 { return invalid("moby packet: fewer than 3 indices before the terminator"); }
    pushes.truncate(pushes.len() - 3);
    let tris = (2..pushes.len())
        .filter(|&n| !pushes[n].nokick)
        .map(|n| MobyTriangle { a: pushes[n - 2].idx, b: pushes[n - 1].idx, c: pushes[n].idx, texture: pushes[n].texture })
        .collect();
    Ok((tris, texture))
}

/// State carried across the packets of one LOD list: vertex cache, VU0 slots, GS texture.
struct ListState { cache: Vec<Option<MobyVertex>>, slots: Vu0Slots, texture: i32 }

impl ListState { fn new() -> Self { ListState { cache: vec![None; 512], slots: Vu0Slots::default(), texture: -1 } } }

/// `old`: the packet has R&C1's vertex table header (a class "in an old format", [`parse_moby_class`]).
fn read_packet(old: bool, blob: Buf, e: &MobyPacketEntry, metal: bool, state: &mut ListState) -> Result<MobySubmesh> {
    // VIF list: ST (regular only), indices, optional ad-gifs.
    let list = blob.sub(e.vif_list_offset as usize, e.vif_list_size as usize * 0x10, "moby VIF list")?;
    let codes = parse_vif(list.bytes())?;
    let mut unpacks = codes.iter().filter(|c| c.is_unpack());
    let mut sub = MobySubmesh {
        entry: *e,
        vertex_table: Default::default(),
        metal_header: Default::default(),
        transfers: Vec::new(),
        duplicates: Vec::new(),
        raw_vertices: Vec::new(),
        glow_vertices: 0,
        rgba_multipliers: Vec::new(),
        vertices: Vec::new(),
        st: Vec::new(),
        index_bytes: Vec::new(),
        secret_indices: Vec::new(),
        texture_indices: Vec::new(),
        initial_texture: -1,
        unresolved_duplicates: 0,
        triangles: Vec::new(),
        is_metal: metal,
    };
    if !metal {
        let Some(st) = unpacks.next() else { return invalid("moby packet: fewer than 2 unpacks") };
        if st.vn() != 1 || st.vl() != 1 { return invalid("moby packet: first unpack is not V2_16"); }
        sub.st = bytemuck::pod_collect_to_vec(&st.data[..st.count() as usize * 4]);
    }
    let Some(ix) = unpacks.next() else { return invalid(if metal { "moby packet: missing index unpack" } else { "moby packet: fewer than 2 unpacks" }) };
    if ix.vn() != 3 || ix.vl() != 2 { return invalid("moby packet: index unpack is not V4_8"); }
    let ix_bytes = ix.count() as usize * 4;
    sub.index_bytes = ix.data[4..ix_bytes].to_vec();
    sub.secret_indices.push(ix.data[2]);
    if let Some(ad) = unpacks.next() {
        if ad.vn() != 3 || ad.vl() != 0 { return invalid("moby packet: texture unpack is not V4_32"); }
        for b in 0..ad.count() as usize / 4 {
            sub.texture_indices.push(i32::from_le_bytes(ad.data[b * 0x40 + 0x20..b * 0x40 + 0x24].try_into().unwrap()));
            // One extra index per ad-gif block, from successive quadwords (block 0 holds the first four).
            sub.secret_indices.push(ad.data[b * 0x10 + 0x0c]);
        }
    }

    let vo = e.vertex_offset as usize;
    if metal {
        sub.metal_header = blob.pod(vo, "moby metal vertex table header")?;
        let count = sub.metal_header.vertex_count;
        if !(0..=4096).contains(&count) { return invalid("moby metal packet: implausible vertex count"); }
        sub.raw_vertices = blob.sub(vo + 0x10, count as usize * 0x10, "moby metal vertices")?.bytes().to_vec();
        for (i, r) in sub.raw_vertices.as_chunks::<16>().0.iter().enumerate() {
            let raw: [u8; 8] = r[8..16].try_into().unwrap();
            sub.vertices.push(MobyVertex {
                raw,
                normal_azimuth: r[6],
                normal_elevation: r[7],
                x: i16::from_le_bytes([r[0], r[1]]),
                y: i16::from_le_bytes([r[2], r[3]]),
                z: i16::from_le_bytes([r[4], r[5]]),
                id: i as u16,
                kind: MobyVertexKind::Metal,
                duplicate: false,
                skin: metal_skin(&raw)?,
                st: [0, 0],
            });
        }
    } else {
        // Going Commando's vertex table header is eight u16s (0x10 bytes) and has no RGBA multiplier blob: its level
        // code converts an R&C1-layout class ("moby %d is in an old format", L00 0x2ffe70) by halving the eight words
        // to u16s, moving the rest down 0x10 bytes and zeroing the multiplier offset.
        let (h, head) = if old {
            (blob.pod::<MobyVertexTableHeader>(vo, "moby vertex table header")?, 0x20)
        } else {
            let w: [u16; 8] = blob.pod(vo, "moby vertex table header")?;
            let [matrix, two, three, main, dup, transfer, table, mult] = w.map(u32::from);
            let h = MobyVertexTableHeader {
                matrix_transfer_count: matrix, two_way_blend_vertex_count: two, three_way_blend_vertex_count: three,
                main_vertex_count: main, duplicate_vertex_count: dup, transfer_vertex_count: transfer,
                vertex_table_offset: table, unknown_e: mult,
            };
            (h, 0x10)
        };
        sub.vertex_table = h;
        if !old { sub.glow_vertices = h.unknown_e; }
        let (n2, n3) = (h.two_way_blend_vertex_count as usize, h.three_way_blend_vertex_count as usize);
        let in_file = n2 + n3 + h.main_vertex_count as usize;
        if h.transfer_vertex_count as usize != in_file + h.duplicate_vertex_count as usize { return invalid("moby packet: transfer count mismatch"); }
        if h.transfer_vertex_count != e.transfer_vertex_count as u32 { return invalid("moby packet: entry/table transfer count mismatch"); }
        let tvc = e.transfer_vertex_count as u32;
        if e.unknown_d as u32 != (tvc * 6).div_ceil(0x10) || e.unknown_e as u32 != tvc.div_ceil(4) {
            return invalid("moby packet: redundant entry fields do not match");
        }
        sub.transfers = blob.pod_slice(vo + head, h.matrix_transfer_count as usize, "moby matrix transfers")?;
        let mut ofs = vo + head + h.matrix_transfer_count as usize * 2;
        if !ofs.is_multiple_of(4) { ofs += 2; }
        if !ofs.is_multiple_of(8) { ofs += 4; }
        sub.duplicates = blob.pod_slice(ofs, h.duplicate_vertex_count as usize, "moby duplicate vertices")?;
        // The vertex records end with the packet's vertex data, or where an old-format packet's multiplier blob starts.
        let records_end = if old { h.unknown_e } else { e.vertex_data_size as u32 * 0x10 };
        if records_end < h.vertex_table_offset { return invalid("moby packet: unknown_e before vertex table"); }
        let epilogue = ((records_end - h.vertex_table_offset) / 0x10) as usize;
        let epilogue = epilogue.wrapping_sub(in_file);
        if !(1..7).contains(&epilogue) { return invalid(format!("moby packet: epilogue vertex count {}", epilogue as isize)); }
        if old {
            // An old-format packet's RGBA multiplier blob is checked but not kept: the conversion zeroes its offset.
            let mult_size = e.vertex_data_size as usize * 0x10;
            if mult_size < h.unknown_e as usize { return invalid("moby packet: multiplier blob starts past the vertex data"); }
            let mult = blob.sub(vo + h.unknown_e as usize, mult_size - h.unknown_e as usize, "moby RGBA multipliers")?;
            if mult.bytes().len() != (h.transfer_vertex_count as usize * 4).div_ceil(16) * 16 {
                return invalid("moby packet: multiplier blob is not align16(4 * transfer_vertex_count)");
            }
        }
        let vbase = vo + h.vertex_table_offset as usize;
        sub.raw_vertices = blob.sub(vbase, (in_file + epilogue) * 0x10, "moby vertices")?.bytes().to_vec();
        let records: Vec<[u8; 16]> = sub.raw_vertices.as_chunks::<16>().0.to_vec();
        let ids = vertex_cache_ids(&records, in_file)?;

        for t in &sub.transfers { state.slots.store(t.vu0_dest_addr, MobySkin::joint(t.spr_joint_index))?; }
        for (i, r) in records[..in_file].iter().enumerate() {
            let raw: [u8; 8] = r[..8].try_into().unwrap();
            let kind = if i < n2 { MobyVertexKind::TwoWay } else if i < n2 + n3 { MobyVertexKind::ThreeWay } else { MobyVertexKind::Single };
            let skin = state.slots.vertex(kind, &raw)?;
            sub.vertices.push(MobyVertex {
                raw,
                normal_azimuth: r[8],
                normal_elevation: r[9],
                x: i16::from_le_bytes([r[10], r[11]]),
                y: i16::from_le_bytes([r[12], r[13]]),
                z: i16::from_le_bytes([r[14], r[15]]),
                id: ids[i],
                kind,
                duplicate: false,
                skin,
                st: [0, 0],
            });
        }
        // Duplicates copy an earlier vertex (position, normal, skin) from the 512-entry cache.
        for v in &sub.vertices { state.cache[(v.id & 0x1ff) as usize] = Some(*v); }
        for &d in &sub.duplicates {
            let id = duplicate_cache_id(d);
            let v = match state.cache[id as usize] {
                Some(v) => v,
                None => {
                    sub.unresolved_duplicates += 1;
                    MobyVertex { raw: [0; 8], normal_azimuth: 0, normal_elevation: 0, x: 0, y: 0, z: 0, id: 0, kind: MobyVertexKind::Single, duplicate: false, skin: MobySkin::default(), st: [0, 0] }
                }
            };
            sub.vertices.push(MobyVertex { id, duplicate: true, ..v });
        }
        if sub.st.len() < sub.vertices.len() { return invalid("moby packet: ST array shorter than vertex list"); }
        for (v, st) in sub.vertices.iter_mut().zip(&sub.st) { v.st = *st; }
    }

    sub.initial_texture = state.texture;
    let (tris, texture) = walk_indices(&sub)?;
    sub.triangles = tris;
    state.texture = texture;
    Ok(sub)
}

/// Parses a moby class blob. Going Commando's class header is R&C1's (its relocation, L00 0x2ffe70, walks the same
/// fields); its packets differ only in the vertex table header ([`read_packet`]).
pub fn parse_moby_class(blob: &[u8]) -> Result<MobyClass> {
    let b = Buf(blob);
    let h: MobyClassHeader = b.pod(0, "moby class header")?;
    let sequence_pointers = if h.sequence_count > 0 { b.pod_slice(0x48, h.sequence_count as usize, "moby sequence pointers")? } else { Vec::new() };
    let (mut high_lod, mut low_lod, mut metal) = (Vec::new(), Vec::new(), Vec::new());
    if h.packet_table_offset > 0 {
        let total = h.high_lod_count as usize + h.low_lod_count as usize + h.metal_count as usize;
        if h.metal_count > 0 && h.metal_begin as usize + h.metal_count as usize > total { return invalid("moby class: metal packets past the packet table"); }
        let entries: Vec<MobyPacketEntry> = b.pod_slice(h.packet_table_offset as usize, total, "moby packet table")?;
        // Going Commando reads a class with byte 0xb set as one "in an old format" (L00 `0x2ffe70`): its high and low
        // packets have R&C1's vertex table header, converted at load with the multiplier offset zeroed (no RGBA
        // multipliers), and the byte cleared.
        let old = h.rac1_byte_b != 0;
        // Vertex cache, VU0 slots and GS texture state carry across the packets of one list.
        let list = |first: usize, count: usize, is_metal: bool, out: &mut Vec<MobySubmesh>| -> Result<()> {
            let mut st = ListState::new();
            for e in &entries[first..first + count] {
                out.push(read_packet(old && !is_metal, b, e, is_metal, &mut st)?);
            }
            Ok(())
        };
        list(0, h.high_lod_count as usize, false, &mut high_lod)?;
        list(h.high_lod_count as usize, h.low_lod_count as usize, false, &mut low_lod)?;
        list(h.metal_begin as usize, h.metal_count as usize, true, &mut metal)?;
    }
    let jc = h.joint_count as usize;
    let mut skeleton = MobySkeleton::default();
    if h.skeleton > 0 && jc > 0 { skeleton.matrices = b.pod_slice::<[[f32; 4]; 4]>(h.skeleton as usize, jc, "moby skeleton")?; }
    if h.common_trans > 0 && jc > 0 { skeleton.trans = b.pod_slice(h.common_trans as usize, jc, "moby common trans")?; }
    // A class whose pieces do not read keeps none (its debris then draws whole).
    let pieces = read_pieces(b, &h).unwrap_or_default();
    let mut h = h;
    // Going Commando's MobyProc reads neither byte (its glow is per packet, MobySubmesh::glow_vertices): no class-wide
    // glow packets, and byte 0xb's old-format flag is spent.
    (h.rac1_byte_a, h.rac1_byte_b) = (0xff, 0xff);
    Ok(MobyClass { header: h, sequence_pointers, high_lod, low_lod, metal, skeleton, pieces })
}

/// Gives `class` the packets of a separate mesh blob: `u8 high, low, metal, metal_begin`, `+4` the packet table,
/// offsets from the blob's start (Going Commando's armour, `crate::rac2::armor`; its level class 0 has none).
pub fn attach_packets(class: &mut MobyClass, mesh: &[u8]) -> Result<()> {
    let b = Buf(mesh);
    let (high, low, metal, metal_begin) = (b.u8(0)? as usize, b.u8(1)? as usize, b.u8(2)? as usize, b.u8(3)? as usize);
    let table = b.u32(4)? as usize;
    let total = high + low + metal;
    if metal > 0 && metal_begin + metal > total { return invalid("moby mesh: metal packets past the packet table"); }
    let entries: Vec<MobyPacketEntry> = b.pod_slice(table, total, "moby mesh packet table")?;
    let list = |first: usize, count: usize, is_metal: bool| -> Result<Vec<MobySubmesh>> {
        let mut st = ListState::new();
        entries[first..first + count].iter().map(|e| read_packet(false, b, e, is_metal, &mut st)).collect()
    };
    class.high_lod = list(0, high, false)?;
    class.low_lod = list(high, low, false)?;
    class.metal = list(metal_begin, metal, true)?;
    let h = &mut class.header;
    (h.high_lod_count, h.low_lod_count, h.metal_count, h.metal_begin) = (high as u8, low as u8, metal as u8, metal_begin as u8);
    Ok(())
}

/// A level's moby class with its core-index entry (whose `textures[16]` maps a triangle's
/// `texture` >= 0 to an index into `LevelCore::moby_textures`).
#[derive(Clone, Debug)]
pub struct LevelMobyClass {
    pub o_class: i32,
    pub entry: ClassEntry,
    pub class: MobyClass,
}

impl LevelMobyClass {
    /// Moby texture table index (`LevelCore::moby_textures`) for a triangle texture, None for
    /// -1/-2/-3 (none/chrome/glass) or an unused slot (0xff).
    pub fn texture_table_index(&self, texture: i32) -> Option<usize> {
        let slot = *self.entry.textures.get(usize::try_from(texture).ok()?)?;
        (slot != 0xff).then_some(slot as usize)
    }
}

/// Parses every moby class of a level that has a geometry blob (entries with offset 0 are
/// skipped), in core-index order. `core_data` is the decompressed core data.
pub fn parse_level_mobys(core: &LevelCore, core_data: &[u8]) -> Result<Vec<LevelMobyClass>> {
    let mut out = Vec::new();
    for e in core.moby_classes.iter().filter(|e| e.offset_in_asset_wad > 0) {
        let Some(blk) = core.blocks.iter().find(|b| b.offset == e.offset_in_asset_wad as usize && b.name.starts_with("moby_class/")) else {
            return invalid(format!("moby class {}: no core block", e.o_class));
        };
        let blob = Buf(core_data).sub(blk.offset, blk.size, "moby class blob")?;
        let class = parse_moby_class(blob.bytes()).map_err(|err| crate::FormatError::Invalid(format!("moby class {}: {err}", e.o_class)))?;
        out.push(LevelMobyClass { o_class: e.o_class, entry: *e, class });
    }
    Ok(out)
}

/// The two byte lists of joint list `list` of a class (spec 3.3: header `joints` -> `s32 count`,
/// `s32 pointer[count]`; each list `s16 n1, s16 n2, u8 a[n1], u8 b[n2], 0xff`).
///
/// The first list is a root-to-joint chain; its **last** entry is the joint the game attaches to:
/// `fun_0020cca8` (boot 0x20cca8, L01 0x264508) passes the list to `fun_00210850` (boot 0x210850),
/// which evaluates the pose of exactly those joints and returns the pose matrix of the last one.
pub fn joint_list(blob: &[u8], header: &MobyClassHeader, list: usize) -> Result<(Vec<u8>, Vec<u8>)> {
    let b = Buf(blob);
    if header.joints <= 0 { return invalid("class has no joint lists"); }
    let base = header.joints as usize;
    let count = b.i32(base)?;
    if list >= count.max(0) as usize { return invalid(format!("joint list {list} of {count}")); }
    let p = b.i32(base + 4 + 4 * list)?;
    if p <= 0 { return invalid("null joint list pointer"); }
    let p = p as usize;
    let (n1, n2) = (b.i16(p)?, b.i16(p + 2)?);
    if n1 < 0 || n2 < 0 { return invalid("negative joint list length"); }
    let (n1, n2) = (n1 as usize, n2 as usize);
    let a = b.sub(p + 4, n1, "joint list")?.bytes().to_vec();
    let c = b.sub(p + 4 + n1, n2, "joint list")?.bytes().to_vec();
    if b.u8(p + 4 + n1 + n2)? != 0xff { return invalid("joint list without 0xff terminator"); }
    Ok((a, c))
}

