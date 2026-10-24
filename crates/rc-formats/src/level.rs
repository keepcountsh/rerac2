//! Level data container and core index. Spec: docs/formats/wad_layouts_rac1.md sections 2.3–2.5.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};
use std::collections::BTreeSet;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct ByteRange { pub(crate) offset: i32, pub(crate) size: i32 }

/// Header at the start of a level's uncompressed data container (0x58 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct LevelDataHeader {
    pub(crate) overlay: ByteRange,
    pub(crate) sound_bank: ByteRange,
    pub(crate) core_index: ByteRange,
    pub(crate) gs_ram: ByteRange,
    pub(crate) hud_header: ByteRange,
    pub(crate) hud_banks: [ByteRange; 5],
    pub(crate) core_data: ByteRange,
}
const _: () = assert!(std::mem::size_of::<LevelDataHeader>() == 0x58);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct ArrayRange { pub(crate) count: i32, pub(crate) offset: i32 }

/// LevelCoreHeader (0xbc bytes) at byte 0 of the core index. "index" offsets are
/// relative to the core index, "data" offsets to the decompressed core data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LevelCoreHeader {
    pub(crate) gs_ram: ArrayRange,
    pub(crate) tfrags: i32, pub(crate) occlusion: i32, pub(crate) sky: i32, pub(crate) collision: i32,
    pub(crate) moby_classes: ArrayRange, pub(crate) tie_classes: ArrayRange, pub(crate) shrub_classes: ArrayRange,
    pub(crate) tfrag_textures: ArrayRange, pub(crate) moby_textures: ArrayRange, pub(crate) tie_textures: ArrayRange, pub(crate) shrub_textures: ArrayRange,
    pub(crate) part_textures: ArrayRange, pub(crate) fx_textures: ArrayRange,
    pub(crate) textures_base_offset: i32, pub(crate) part_bank_offset: i32, pub(crate) fx_bank_offset: i32,
    pub(crate) part_defs_offset: i32, pub(crate) sound_remap_offset: i32, pub(crate) unknown_74: i32,
    pub(crate) ratchet_seqs: i32, pub(crate) scene_view_size: i32,
    pub(crate) unknown_80: i32, pub(crate) moby_stash_count: i32,
    pub(crate) assets_compressed_size: i32, pub(crate) assets_decompressed_size: i32,
    pub chrome_map_texture: i32, pub chrome_map_palette: i32, pub glass_map_texture: i32, pub glass_map_palette: i32,
    pub(crate) unknown_a0: i32, pub(crate) heightmap_offset: i32, pub(crate) occlusion_oct_offset: i32, pub(crate) moby_gs_stash_list: i32,
    pub(crate) occlusion_rad_offset: i32, pub(crate) moby_sound_remap_offset: i32, pub(crate) occlusion_rad2_offset: i32,
}
const _: () = assert!(std::mem::size_of::<LevelCoreHeader>() == 0xbc);

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsRamEntry { pub(crate) psm: i32, pub(crate) width: i16, pub(crate) height: i16, pub(crate) address: i32, pub(crate) offset: i32 }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClassEntry { pub offset_in_asset_wad: i32, pub o_class: i32, pub unknown_8: i32, pub unknown_c: i32, pub textures: [u8; 16] }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShrubBillboardInfo { pub(crate) width: i16, pub(crate) height: i16, pub(crate) max_mip: i16, pub(crate) palette_offset: i16, pub(crate) texture_offset: i16, pub(crate) mip1: i16, pub(crate) mip2: i16, pub(crate) mip3: i16 }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ShrubClassEntry { pub(crate) base: ClassEntry, pub(crate) billboard: ShrubBillboardInfo }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextureEntry { pub data_offset: i32, pub width: i16, pub height: i16, pub ty: i16, pub palette: i16, pub mipmap: i16, pub pad: i16 }
const _: () = assert!(std::mem::size_of::<ShrubClassEntry>() == 0x30);

#[derive(Clone, Debug)]
pub struct CoreBlock { pub name: String, pub offset: usize, pub size: usize }

#[derive(Debug)]
pub struct LevelCore {
    pub header: LevelCoreHeader,
    pub gs_ram: Vec<GsRamEntry>,
    pub moby_classes: Vec<ClassEntry>,
    pub(crate) tie_classes: Vec<ClassEntry>,
    pub(crate) shrub_classes: Vec<ShrubClassEntry>,
    pub tfrag_textures: Vec<TextureEntry>,
    pub moby_textures: Vec<TextureEntry>,
    pub tie_textures: Vec<TextureEntry>,
    pub shrub_textures: Vec<TextureEntry>,
    pub ratchet_seqs: Vec<i32>,
    /// Every block of the decompressed core data with its size from the boundary rule.
    pub blocks: Vec<CoreBlock>,
    /// Going Commando's GS stash: the moby classes whose textures stay resident in GS memory (the s16 list at
    /// `moby_gs_stash_list`, ended by a negative entry: Clank, the packs, the helmet), and where they are — the address
    /// of the first gs_ram table entry past `gs_ram.count` (the table has word 0x84's count more). A stashed
    /// class's moby texture entries hold offsets from that address into the gs_ram lump. Empty / None on R&C1.
    pub(crate) moby_stash: Vec<i32>,
    pub moby_stash_addr: Option<usize>,
}

impl LevelCore {
    /// The bytes of the first block named `name` (`tfrags`, `sky`, `moby_class/0042`, `ratchet_seq/007`, …) in the
    /// decompressed core data, or None when the level has no such block.
    pub fn block<'a>(&self, data: &'a [u8], name: &str) -> Option<&'a [u8]> {
        let b = self.blocks.iter().find(|b| b.name == name)?;
        data.get(b.offset..b.offset + b.size)
    }
}

fn table<T: Pod>(idx: Buf, r: ArrayRange, what: &'static str) -> Result<Vec<T>> {
    if r.count <= 0 || r.offset <= 0 { return Ok(Vec::new()); }
    if r.count > 100_000 { return invalid(format!("implausible count for {what}")); }
    idx.pod_slice(r.offset as usize, r.count as usize, what)
}

/// Parses a level's core index (`index`) for core data of `data_size` bytes. Going Commando's core index has R&C1's
/// header and tables (checked on every level of SCUS_972.68: the same counts, offsets and block boundaries).
pub fn parse_level_core(index: &[u8], data_size: usize) -> Result<LevelCore> {
    let idx = Buf(index);
    let h: LevelCoreHeader = idx.pod(0, "core header")?;
    let ratchet_seqs = if h.ratchet_seqs > 0 { idx.pod_slice::<i32>(h.ratchet_seqs as usize, 256, "ratchet seq table")? } else { Vec::new() };
    let mut core = LevelCore {
        header: h,
        gs_ram: table(idx, h.gs_ram, "gs_ram table")?,
        moby_classes: table(idx, h.moby_classes, "moby class table")?,
        tie_classes: table(idx, h.tie_classes, "tie class table")?,
        shrub_classes: table(idx, h.shrub_classes, "shrub class table")?,
        tfrag_textures: table(idx, h.tfrag_textures, "tfrag textures")?,
        moby_textures: table(idx, h.moby_textures, "moby textures")?,
        tie_textures: table(idx, h.tie_textures, "tie textures")?,
        shrub_textures: table(idx, h.shrub_textures, "shrub textures")?,
        ratchet_seqs,
        blocks: Vec::new(),
        moby_stash: Vec::new(),
        moby_stash_addr: None,
    };
    let stash_count = h.moby_stash_count;
    if h.moby_gs_stash_list > 0 {
        let mut at = h.moby_gs_stash_list as usize;
        while let Ok(c) = idx.i16(at) {
            if c < 0 { break; }
            core.moby_stash.push(c as i32);
            at += 2;
        }
    }
    if stash_count > 0 && h.gs_ram.offset > 0 {
        let e: GsRamEntry = idx.pod(h.gs_ram.offset as usize + h.gs_ram.count as usize * std::mem::size_of::<GsRamEntry>(), "gs stash entry")?;
        core.moby_stash_addr = Some(e.address as usize);
    }

    // Boundary rule (spec 2.5): a block runs to the next known start offset.
    // moby_sound_remap_offset is not a data offset here.
    let mut bounds = BTreeSet::new();
    let mut add = |v: i32| { if v > 0 { bounds.insert(v as usize); } };
    for v in [h.tfrags, h.occlusion, h.sky, h.collision, h.textures_base_offset, h.part_bank_offset, h.fx_bank_offset] { add(v); }
    for e in &core.moby_classes { add(e.offset_in_asset_wad); }
    for e in &core.tie_classes { add(e.offset_in_asset_wad); }
    for e in &core.shrub_classes { add(e.base.offset_in_asset_wad); }
    for &v in &core.ratchet_seqs { add(v); }
    let end = if h.assets_decompressed_size > 0 { h.assets_decompressed_size as usize } else { data_size };
    bounds.insert(end);

    let block = |blocks: &mut Vec<CoreBlock>, name: String, start: i32| {
        if start <= 0 { return; }
        let start = start as usize;
        let next = bounds.range(start + 1..).next().copied().unwrap_or(end);
        blocks.push(CoreBlock { name, offset: start, size: next - start });
    };
    let mut blocks = Vec::new();
    // tfrags open the data region and run to the first of occlusion/sky/collision.
    let tf_end = if h.occlusion > 0 { h.occlusion } else if h.sky > 0 { h.sky } else { h.collision };
    if tf_end > h.tfrags { blocks.push(CoreBlock { name: "tfrags".into(), offset: h.tfrags as usize, size: (tf_end - h.tfrags) as usize }); }
    block(&mut blocks, "occlusion".into(), h.occlusion);
    block(&mut blocks, "sky".into(), h.sky);
    block(&mut blocks, "collision".into(), h.collision);
    block(&mut blocks, "textures".into(), h.textures_base_offset);
    block(&mut blocks, "part_bank".into(), h.part_bank_offset);
    block(&mut blocks, "fx_bank".into(), h.fx_bank_offset);
    for e in &core.moby_classes { block(&mut blocks, format!("moby_class/{:04}", e.o_class), e.offset_in_asset_wad); }
    for e in &core.tie_classes { block(&mut blocks, format!("tie_class/{:04}", e.o_class), e.offset_in_asset_wad); }
    for e in &core.shrub_classes { block(&mut blocks, format!("shrub_class/{:04}", e.base.o_class), e.base.offset_in_asset_wad); }
    for (i, &v) in core.ratchet_seqs.iter().enumerate() { block(&mut blocks, format!("ratchet_seq/{i:03}"), v); }
    core.blocks = blocks;
    Ok(core)
}

