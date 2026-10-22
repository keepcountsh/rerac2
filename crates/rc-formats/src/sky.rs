//! The level sky: concentric shells of small indexed-triangle clusters plus the sky's own
//! 8-bit textures. Spec: docs/formats/shrub_sky_rac1.md part 2; renderer facts from the
//! decomp in docs/plan/sky_render_notes.md. Ported from the retired C++
//! reference extractor (git 2230812; `parse_sky`, `decode_sky_texture`, `sky_gs_vertices`);
//!
//! How the game draws it (not Wrench's model): the sky is **not** a VU1 program. The EE builds
//! GS packets with VU0 macro code and sends them over PATH2 (VIF1 DIRECT):
//! * `sky_draw_shell` (boot 0x22b690) draws a shell *textured* when its `flags` word is 0 and
//!   *gouraud* otherwise;
//! * per cluster the header's `data_size` bytes are DMAed to the scratchpad, every vertex is
//!   transformed (boot 0x22bf94), then each face emits three GS vertices in stored index order
//!   as a plain triangle list (PRIM 3, no strips): textured (boot 0x22c208) ST = u16 / 4096,
//!   RGBAQ = (0x80, 0x80, 0x80, vertex alpha), TEX0 re-sent whenever the face texture differs
//!   from the previous drawn face of the same cluster; gouraud (boot 0x22c0e0) RGBAQ = the
//!   vertex's attribute word verbatim. That walk is [`sky_gs_vertices`].
//! * for gouraud shells the "ST" array is the per-vertex **colour**; the header colour is not
//!   used by the shell code.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use crate::texture::{IndexedImage, Texture};
use bytemuck::{Pod, Zeroable};

/// Sky block header (0x40 bytes) at byte 0 of the block. Offsets in it are relative to the block
/// start (the loader, boot 0x2028e0, relocates them in place).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct SkyHeader {
    /// 0x00: RGBA, 0x80 = 1.0. Not read by the shell drawing code.
    pub(crate) colour: [u8; 4],
    /// 0x04: the frame-clear gate. The loader overwrites it with 1, so the disc value is dead; the
    /// frame render skips its clear packet while it is 0 (see sky_render_notes.md).
    pub(crate) clear_screen: i16,
    /// 0x06: shells drawn, 0..=8, in index order.
    pub(crate) shell_count: i16,
    /// 0x08: 0 on the disc; run-time star-sprite count (set to 0x100 on first use by the boot's
    /// `update_sky_effects`).
    pub(crate) sprite_count: i16,
    /// 0x0a: sprite slots reserved at `sprites` (0x20 bytes each).
    pub(crate) maximum_sprite_count: i16,
    /// 0x0c: entries at `texture_defs`.
    pub(crate) texture_count: i16,
    /// 0x0e: bytes at `fx_list` (the first `fx_count` textures are sprite/FX textures).
    pub(crate) fx_count: i16,
    /// 0x10: [`SkyTextureDef`] array.
    pub(crate) texture_defs: i32,
    /// 0x14: base of the palette/pixel region; texture-def offsets are relative to it.
    pub(crate) texture_data: i32,
    /// 0x18: `fx_count` u8 texture indices.
    pub(crate) fx_list: i32,
    /// 0x1c: sprite scratch (zeroed on the disc) or 0.
    pub(crate) sprites: i32,
    /// 0x20: shell header offsets; the first `shell_count` are used.
    pub(crate) shells: [i32; 8],
}

/// Texture definition (0x10 bytes) as stored on the disc. At load the game rewrites the record in
/// place to `{u64 TEX0 cache = 0, s16 texture_offset >> 4, s16 palette_offset >> 4, s16 log2 width,
/// s16 log2 height}` (boot 0x2028e0); this is the disc layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SkyTextureDef {
    /// 0x0: 256 x RGBA32 CLUT (CSM1 order, PSMCT32), relative to `texture_data`.
    pub(crate) palette_offset: i32,
    /// 0x4: `width * height` PSMT8 indices, relative to `texture_data`.
    pub(crate) texture_offset: i32,
    /// 0x8: pixels, a power of two.
    pub(crate) width: i32,
    /// 0xc: pixels, a power of two.
    pub(crate) height: i32,
}

/// Cluster header (0x20 bytes); `cluster_count` of them start at shell offset + 0x10.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct SkyClusterHeader {
    /// 0x00: bounding sphere (x, y, z, radius) in vertex units, for the per-cluster cull (boot 0x22c4c8).
    pub(crate) bsphere: [f32; 4],
    /// 0x10: cluster data, relative to the block start.
    pub(crate) data: i32,
    /// 0x14
    pub(crate) vertex_count: i16,
    /// 0x16
    pub(crate) tri_count: i16,
    /// 0x18: vertex array within `data` (0 on every retail cluster).
    pub(crate) vertex_offset: i16,
    /// 0x1a: attribute (ST or colour) array within `data`.
    pub(crate) st_offset: i16,
    /// 0x1c: face array within `data`.
    pub(crate) tri_offset: i16,
    /// 0x1e: bytes DMAed to the scratchpad (`>> 4` quadwords); all three arrays lie inside it.
    pub(crate) data_size: i16,
}

/// Vertex (8 bytes). The game feeds x, y, z to the transform as plain integers (`vitof0`); only the
/// direction matters because the sky is drawn with GS Z = 0 (see the notes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct SkyVertex {
    /// 0x0
    pub(crate) x: i16,
    /// 0x2
    pub(crate) y: i16,
    /// 0x4: up.
    pub(crate) z: i16,
    /// 0x6: textured shells: the low byte is the vertex alpha (0x80 = 1.0). Unused by gouraud shells.
    pub(crate) alpha: i16,
}

/// Per-vertex attribute word (4 bytes) at `st_offset`, index-parallel with the vertices. Its meaning
/// depends on the shell: see [`SkyVertexAttr::st`] and [`SkyVertexAttr::rgba`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct SkyVertexAttr(pub(crate) [u8; 4]);

impl SkyVertexAttr {
    /// Textured shells: (s, t) as the game reads them: zero-extended u16 (`pextlh` with zero), 4.12 fixed.
    pub(crate) fn st_raw(self) -> [u16; 2] { [u16::from_le_bytes([self.0[0], self.0[1]]), u16::from_le_bytes([self.0[2], self.0[3]])] }
    /// Textured shells: (s, t) as sent to the GS ST register (`vitof12`), with Q = 1.
    pub(crate) fn st(self) -> [f32; 2] { let [s, t] = self.st_raw(); [s as f32 / 4096.0, t as f32 / 4096.0] }
    /// Gouraud shells: the vertex colour RGBA as sent to RGBAQ (0x80 = 1.0).
    pub(crate) fn rgba(self) -> [u8; 4] { self.0 }
}

/// Face (4 bytes): cluster-local vertex indices and a texture-definition index (0xff in gouraud shells).
/// The GS draws it as-is with no back-face culling, so the winding carries no meaning for the game.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct SkyFace {
    /// 0x0
    pub(crate) indices: [u8; 3],
    /// 0x3: index into [`Sky::texture_defs`] (and [`parse_sky_textures`]); 0xff = untextured.
    pub(crate) texture: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkyCluster {
    pub(crate) header: SkyClusterHeader,
    pub(crate) vertices: Vec<SkyVertex>,
    pub(crate) attrs: Vec<SkyVertexAttr>,
    pub(crate) faces: Vec<SkyFace>,
}

/// Shell header: `{s32 cluster_count, s32 flags}`, then 8 zero bytes (all retail shells), then the
/// cluster headers at +0x10. RAC1 has no per-shell rotation, angular velocity or bloom in the data.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyShell {
    pub(crate) cluster_count: i32,
    /// 0 = textured; any other value = gouraud (retail: 0 or 1).
    pub(crate) flags: i32,
    pub clusters: Vec<SkyCluster>,
}

impl SkyShell {
    /// What `sky_draw_shell` tests (the whole word, not bit 0).
    pub fn textured(&self) -> bool { self.flags == 0 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sky {
    pub(crate) header: SkyHeader,
    pub(crate) fx_list: Vec<u8>,
    pub(crate) texture_defs: Vec<SkyTextureDef>,
    pub shells: Vec<SkyShell>,
}

/// One vertex of the GS triangle list the game builds for a cluster (three per face, face order),
/// the byte layout of the C++ `rc::SkyGsVertex` (20 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SkyGsVertex {
    /// 0x0: raw vertex position (x, y, z-up).
    pub position: [i16; 3],
    /// 0x6: texture-definition index; 0xff for gouraud shells.
    pub texture: u8,
    /// 0x7
    pub(crate) pad: u8,
    /// 0x8: (s, t), Q = 1; 0 for gouraud shells.
    pub st: [f32; 2],
    /// 0x10: GS RGBA, 0x80 = 1.0. Textured: (0x80, 0x80, 0x80, alpha) modulating the texel; gouraud: the vertex colour.
    pub rgba: [u8; 4],
}

/// The per-face vertex stream of `SkyDrawShellTextured` / `SkyDrawShellGouraud` for one cluster,
/// before the view-dependent parts (transform, the face rejection when all three vertices share an
/// outside clip flag). Triangle `k` is elements `3k..3k+3`.
pub fn sky_gs_vertices(shell: &SkyShell, cluster: &SkyCluster) -> Vec<SkyGsVertex> {
    let mut out = Vec::with_capacity(cluster.faces.len() * 3);
    for f in &cluster.faces {
        for &ix in &f.indices {
            let v = cluster.vertices[ix as usize];
            let a = cluster.attrs[ix as usize];
            let position = [v.x, v.y, v.z];
            out.push(if shell.textured() {
                SkyGsVertex { position, texture: f.texture, pad: 0, st: a.st(), rgba: [0x80, 0x80, 0x80, v.alpha as u8] }
            } else {
                SkyGsVertex { position, texture: 0xff, pad: 0, st: [0.0; 2], rgba: a.rgba() }
            });
        }
    }
    out
}

fn off(v: i32, what: &str) -> Result<usize> {
    if v < 0 { return invalid(format!("sky: negative {what} offset")); }
    Ok(v as usize)
}

/// Parses a whole sky block (the `sky` entry of [`LevelCore::blocks`]).
pub fn parse_sky_block(block: &[u8]) -> Result<Sky> {
    let b = Buf(block);
    let header: SkyHeader = b.pod(0, "sky header")?;
    let h = &header;
    if !(0..=8).contains(&h.shell_count) { return invalid("sky: shell count out of range"); }
    let fx_list = if h.fx_count > 0 && h.fx_list > 0 {
        b.sub(h.fx_list as usize, h.fx_count as usize, "sky fx list")?.bytes().to_vec()
    } else { Vec::new() };
    let texture_defs = if h.texture_count > 0 && h.texture_defs > 0 {
        b.pod_slice(h.texture_defs as usize, h.texture_count as usize, "sky texture defs")?
    } else { Vec::new() };
    let mut shells = Vec::with_capacity(h.shell_count as usize);
    for &so in &h.shells[..h.shell_count as usize] {
        let so = off(so, "shell")?;
        let cluster_count = b.i32(so)?;
        let flags = b.i32(so + 4)?;
        if !(0..=10000).contains(&cluster_count) { return invalid("sky: implausible cluster count"); }
        let headers: Vec<SkyClusterHeader> = b.pod_slice(so + 0x10, cluster_count as usize, "sky cluster headers")?;
        let mut clusters = Vec::with_capacity(headers.len());
        for ch in headers {
            // The game DMAs only data_size bytes of the cluster (SkyDrawShellTextured), so every array must lie inside it.
            let within = |o: i16, len: usize| o >= 0 && ch.data_size >= 0 && o as usize + len <= ch.data_size as usize;
            let (vc, tc) = (ch.vertex_count, ch.tri_count);
            if ch.data < 0 || vc < 0 || tc < 0
                || !within(ch.vertex_offset, vc as usize * 8) || !within(ch.st_offset, vc as usize * 4) || !within(ch.tri_offset, tc as usize * 4) {
                return invalid("sky cluster arrays outside data_size");
            }
            let d = ch.data as usize;
            let vertices: Vec<SkyVertex> = b.pod_slice(d + ch.vertex_offset as usize, vc as usize, "sky vertices")?;
            let attrs: Vec<SkyVertexAttr> = b.pod_slice(d + ch.st_offset as usize, vc as usize, "sky st")?;
            let faces: Vec<SkyFace> = b.pod_slice(d + ch.tri_offset as usize, tc as usize, "sky faces")?;
            for f in &faces {
                if f.indices.iter().any(|&i| i as i16 >= vc) { return invalid("sky face index out of range"); }
                if f.texture != 0xff && f.texture as i16 >= h.texture_count { return invalid("sky face texture out of range"); }
            }
            clusters.push(SkyCluster { header: ch, vertices, attrs, faces });
        }
        shells.push(SkyShell { cluster_count, flags, clusters });
    }
    Ok(Sky { header, fx_list, texture_defs, shells })
}

/// The level's sky block bytes, or `None` when the core index has no sky.
pub fn sky_block<'a>(core: &LevelCore, core_data: &'a [u8]) -> Result<Option<&'a [u8]>> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "sky") else { return Ok(None) };
    Ok(Some(Buf(core_data).sub(blk.offset, blk.size, "sky block")?.bytes()))
}

/// A decoded sky texture; `index` is the texture-definition index that [`SkyFace::texture`] and
/// [`SkyGsVertex::texture`] refer to.
#[derive(Clone, Debug)]
pub struct SkyTexture {
    pub index: usize,
    pub def: SkyTextureDef,
    pub texture: Texture,
}

/// Decodes one texture definition: `width * height` PSMT8 indices with the 256-entry CLUT, CSM1 order
/// and 0x80-alpha scaling exactly like level textures (the game's TEX0 is PSMT8 / CPSM CT32 / CSM1 / TCC 1).
pub(crate) fn decode_sky_texture(block: &[u8], sky: &Sky, index: usize) -> Result<Texture> { sky_texture_image(block, sky, index)?.decode() }

/// The stored indices and CLUT of one texture definition, the parts [`decode_sky_texture`] decodes.
pub(crate) fn sky_texture_image<'a>(block: &'a [u8], sky: &Sky, index: usize) -> Result<IndexedImage<'a>> {
    let Some(t) = sky.texture_defs.get(index) else { return invalid("sky texture index out of range") };
    if t.width <= 0 || t.height <= 0 { return invalid("sky texture with no pixels"); }
    let b = Buf(block);
    let base = off(sky.header.texture_data, "texture data")?;
    let px = b.sub(base + off(t.texture_offset, "texture")?, t.width as usize * t.height as usize, "sky texture pixels")?;
    let clut = b.sub(base + off(t.palette_offset, "palette")?, 1024, "sky palette")?;
    Ok(IndexedImage { width: t.width as u32, height: t.height as u32, indices: px.bytes(), clut: clut.bytes() })
}

/// Decodes every texture definition in table order (FX textures first).
pub fn parse_sky_textures(block: &[u8], sky: &Sky) -> Result<Vec<SkyTexture>> {
    (0..sky.texture_defs.len())
        .map(|index| Ok(SkyTexture { index, def: sky.texture_defs[index], texture: decode_sky_texture(block, sky, index)? }))
        .collect()
}

