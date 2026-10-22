//! Per-level HUD graphics: the `hud_header` lump and its five `hud_banks`. Spec: docs/plan/hud_text.md §1.
//! Ported from the retired C++ reference extractor (git 2230812).
//!
//! The header (copied to the HUD heap by `LoadHudBanks__Fv`, level01 0x253e28) holds four tables, all
//! offsets relative to the header:
//!
//! * **icons** ([`IconEntry`], 8 bytes, ends with id 0xffff): `Hud_GetIconIndex__Fi` (0x24a4e0) scans them
//!   linearly; `GetIconFrame__Fii` (0x24fe10) turns (icon id, k) into a frame index;
//! * **frames** ([`FrameEntry`], 4 bytes): palette index + texture index;
//! * **palettes** ([`PaletteEntry`], 8 bytes): 256 × RGBA32 CLUT (CSM1 order) at `offset` in its bank;
//! * **textures** ([`TextureEntry`], 8 bytes): `w·h` 8-bit indices at `offset` in its bank, `w = 1 << log2_w`.
//!
//! Bank *b* owns palettes `palette_cum[b−1]..palette_cum[b]` and textures `texture_cum[b−1]..texture_cum[b]`
//! (`LinkHudBank__FiPc` 0x24a848 clears bit 31 of those entries and adds the bank's base address).
//! Banks are WAD-compressed on disc; every function here takes them decompressed.

use crate::buf::{invalid, Buf, Result};
use crate::texture::{clut_index, Texture};
use bytemuck::{Pod, Zeroable};

/// Number of HUD banks (`hud_banks[5]` in the level data header).
pub(crate) const BANKS: usize = 5;

/// `hud_header` +0x00..+0xb4 (the four table offsets and the per-bank ranges). Every per-bank array is
/// `u32[8]` of which the first [`BANKS`] are used (the rest are 0). +0x74 and +0x94 are runtime fields
/// (bank pointers, stash handles), 0 on disc.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct HudHeader {
    /// Icon entries including the 0xffff terminator.
    pub(crate) icon_count: u16,
    pub(crate) frame_count: u16,
    pub(crate) icon_offset: u32,
    pub(crate) frame_offset: u32,
    pub(crate) palette_offset: u32,
    pub(crate) texture_offset: u32,
    /// +0x14: cumulative palette count per bank.
    pub(crate) palette_cum: [u32; 8],
    /// +0x34: cumulative texture count per bank.
    pub(crate) texture_cum: [u32; 8],
    /// +0x54: decompressed size of each bank (0 = absent).
    pub(crate) bank_size: [u32; 8],
    /// +0x74, +0x94.
    pub(crate) runtime_bank_base: [u32; 8],
    pub(crate) runtime_stash: [u32; 8],
}
const _: () = assert!(std::mem::size_of::<HudHeader>() == HEADER_SIZE);

/// Bytes of [`HudHeader`]; the icon table follows (offset 0xb4 on every level).
pub(crate) const HEADER_SIZE: usize = 0xb4;

/// Icon table entry: `{u16 id, u16 frame_count, u16 first_frame, u8 anim_mode, u8 ticks_per_frame}`.
/// `anim_mode` / `ticks_per_frame` drive the per-slot frame animation (`FUN_0024f6b0`: 0 static,
/// 1 loop, 2 ping-pong, 3 ping-pong with a random pause).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct IconEntry {
    pub(crate) id: u16,
    pub(crate) frame_count: u16,
    pub(crate) first_frame: u16,
    pub(crate) anim_mode: u8,
    pub(crate) ticks_per_frame: u8,
}

/// Frame table entry: palette and texture index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct FrameEntry {
    pub(crate) palette: i16,
    pub(crate) texture: i16,
}

/// Palette table entry: `offset | 0x80000000` (bit 31 = not linked yet), runtime CBP.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct PaletteEntry {
    pub(crate) offset_flags: u32,
    pub(crate) cbp: u16,
    pub(crate) pad: u16,
}

/// Texture table entry: `offset | 0x80000000`, runtime TBP, log2 of the size.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct TextureEntry {
    pub(crate) offset_flags: u32,
    pub(crate) tbp: u16,
    pub(crate) log2_w: u8,
    pub(crate) log2_h: u8,
}

impl PaletteEntry {
    pub(crate) fn offset(&self) -> usize { (self.offset_flags & 0x7fff_ffff) as usize }
}

impl TextureEntry {
    pub(crate) fn offset(&self) -> usize { (self.offset_flags & 0x7fff_ffff) as usize }
    pub(crate) fn width(&self) -> u32 { 1 << (self.log2_w & 0x1f) }
    pub(crate) fn height(&self) -> u32 { 1 << (self.log2_h & 0x1f) }
}

/// A parsed HUD set with its decompressed banks.
#[derive(Clone, Debug)]
pub struct Hud {
    pub(crate) header: HudHeader,
    /// Including the terminator (id 0xffff).
    pub(crate) icons: Vec<IconEntry>,
    pub(crate) frames: Vec<FrameEntry>,
    pub(crate) palettes: Vec<PaletteEntry>,
    pub(crate) textures: Vec<TextureEntry>,
    pub(crate) banks: [Vec<u8>; BANKS],
}

/// Bank owning entry `i` of a table with cumulative per-bank counts `cum`.
fn owning_bank(cum: &[u32; 8], i: usize) -> Option<usize> { cum[..BANKS].iter().position(|&c| i < c as usize) }

/// Just the header record (to learn which banks exist before decompressing them).
pub(crate) fn parse_header(header: &[u8]) -> Result<HudHeader> { Buf(header).pod(0, "hud header") }

/// Parses `hud_header` and keeps the decompressed `banks` (absent banks empty).
pub fn parse_hud(header: &[u8], banks: [&[u8]; BANKS]) -> Result<Hud> {
    let b = Buf(header);
    let h = parse_header(header)?;
    let n_pal = h.palette_cum[BANKS - 1] as usize;
    let n_tex = h.texture_cum[BANKS - 1] as usize;
    if h.icon_count == 0 || h.icon_count > 0x1000 || h.frame_count > 0x4000 || n_pal > 0x1000 || n_tex > 0x1000 {
        return invalid("implausible hud header counts");
    }
    let icons: Vec<IconEntry> = b.pod_slice(h.icon_offset as usize, h.icon_count as usize, "hud icon table")?;
    if icons.last().map(|e| e.id) != Some(0xffff) { return invalid("hud icon table has no 0xffff terminator"); }
    let frames: Vec<FrameEntry> = b.pod_slice(h.frame_offset as usize, h.frame_count as usize, "hud frame table")?;
    let palettes: Vec<PaletteEntry> = b.pod_slice(h.palette_offset as usize, n_pal, "hud palette table")?;
    let textures: Vec<TextureEntry> = b.pod_slice(h.texture_offset as usize, n_tex, "hud texture table")?;
    for (i, &want) in h.bank_size[..BANKS].iter().enumerate() {
        if banks[i].len() < want as usize { return invalid(format!("hud bank {i} shorter than its header size")); }
    }
    Ok(Hud { header: h, icons, frames, palettes, textures, banks: banks.map(|s| s.to_vec()) })
}

/// Decodes 8-bit indices with a CSM1 CLUT keeping the GS alpha as stored (0x80 = 1.0, no scaling):
/// what the GS samples (TCC = 1, CT32 CLUT). [`decode_indexed8`] is the same with `scale_alpha`.
pub(crate) fn decode_indexed8_raw(indices: &[u8], width: u32, height: u32, clut: &[u8]) -> Result<Texture> {
    let n = width as usize * height as usize;
    if indices.len() < n { return invalid("indexed texture data too small"); }
    if clut.len() < 1024 { return invalid("palette too small"); }
    let mut rgba = Vec::with_capacity(n * 4);
    for &ix in &indices[..n] {
        let e = clut_index(ix as u32) as usize * 4;
        rgba.extend_from_slice(&clut[e..e + 4]);
    }
    Ok(Texture { width, height, rgba })
}

impl Hud {
    /// Bank of palette `i` / texture `i`.
    pub(crate) fn palette_bank(&self, i: usize) -> Option<usize> { owning_bank(&self.header.palette_cum, i) }
    pub(crate) fn texture_bank(&self, i: usize) -> Option<usize> { owning_bank(&self.header.texture_cum, i) }

    /// `Hud_GetIconIndex__Fi`: linear scan; not found → the terminator's index.
    pub(crate) fn icon_index(&self, id: u16) -> usize {
        self.icons.iter().position(|e| e.id == 0xffff || e.id == id).unwrap_or(self.icons.len() - 1)
    }

    /// `GetIconFrame__Fii(id, k)` with every bank resident: `first_frame + k` if the icon exists and
    /// `k < frame_count`, else frame 0. (The game also returns 0 while the frame's palette or texture is
    /// not linked, i.e. its bank is not loaded.)
    pub fn icon_frame(&self, id: u16, k: i32) -> usize {
        let e = &self.icons[self.icon_index(id)];
        if e.id == 0xffff || k >= e.frame_count as i32 { return 0; }
        (e.first_frame as i32 + k).max(0) as usize
    }

    fn frame_parts(&self, i: usize) -> Result<(&[u8], &[u8], u32, u32)> {
        let Some(f) = self.frames.get(i) else { return invalid("hud frame index out of range") };
        let (Ok(pi), Ok(ti)) = (usize::try_from(f.palette), usize::try_from(f.texture)) else {
            return invalid("hud frame with negative palette or texture");
        };
        let (Some(p), Some(t)) = (self.palettes.get(pi), self.textures.get(ti)) else { return invalid("hud frame entry out of range") };
        let (Some(pb), Some(tb)) = (self.palette_bank(pi), self.texture_bank(ti)) else { return invalid("hud entry owned by no bank") };
        let (w, h) = (t.width(), t.height());
        let px = Buf(&self.banks[tb]).sub(t.offset(), w as usize * h as usize, "hud texture pixels")?;
        let clut = Buf(&self.banks[pb]).sub(p.offset(), 1024, "hud palette")?;
        Ok((px.bytes(), clut.bytes(), w, h))
    }

    /// Frame `i` with the raw GS alpha (0x80 = 1.0), as the renderer samples it.
    pub fn decode_frame_raw(&self, i: usize) -> Result<Texture> {
        let (px, clut, w, h) = self.frame_parts(i)?;
        decode_indexed8_raw(px, w, h, clut)
    }
}
