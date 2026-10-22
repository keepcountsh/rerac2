//! The game's three bitmap fonts: glyph tables in the level overlay and the FX textures they index.
//! Spec: docs/plan/hud_text.md §3.1.
//!
//! A font is an FX texture (`GetEffectTex__Fii(n)`, level01 0x21ae98: n = 1 regular, 2 small, 3 large; 256×128
//! PSMT8, decoded by [`crate::particle_tex`]) plus a 232-entry [`Glyph`] table indexed by the byte value.
//! The tables live in the level overlay's `.data`, **at a different address in every overlay** (level 01:
//! 0x1c35d0 / 0x1c3970 / 0x1c3d10; level 00: 0x1c3150 / …); their bytes are identical in all 19 overlays.
//! The game reaches them only through the font wrappers (`font_print_*` 0x21cf70 / 0x21cff0 / 0x21d070 and
//! their right / centre / window variants), which load the FX texture and the table address and call
//! `FontPrint` (0x21ccf0):
//!
//! ```text
//! jal  GetEffectTex        ; li a0, n  (delay slot)
//! lui  t2, hi(table)
//! ...                      ; argument moves
//! jal  FontPrint           ; addiu t2, t2, lo(table)  (delay slot)
//! ```
//!
//! [`find_glyph_tables`] finds the addresses by that call pattern (the same references the game uses), so no
//! per-level address list is needed.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

/// `{u8 u, u8 v, s8 y_off, s8 advance}`: atlas cell (16×16 texels; 24×16 for the pad icons 0x10..0x1f) at
/// texel (u, v) of the font's FX texture, drawn at `y + y_off`; the pen then moves by `advance`.
/// `advance == 0` means "no glyph": `FontPrint` skips the byte.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct Glyph {
    pub u: u8,
    pub v: u8,
    pub(crate) y_off: i8,
    pub advance: i8,
}

/// One section of a level overlay (`ratchet-executable` layout, docs/formats/wad_layouts_rac1.md §4):
/// `{u32 dest, u32 size, u32 type, u32 entry}` then `size` bytes; the list ends at the first header whose
/// entry point differs from the first one (the game's own termination rule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlaySection {
    pub dest: u32,
    /// ELF `sh_type`: 1 PROGBITS, 8 NOBITS.
    pub(crate) kind: u32,
    pub(crate) entry: u32,
    pub data: Vec<u8>,
}

/// Splits the raw `overlay` lump (`LevelFiles::overlay`, `extracted/levels/NN/overlay.bin`) into its sections.
pub(crate) fn parse_overlay_sections(overlay: &[u8]) -> Result<Vec<OverlaySection>> {
    let b = Buf(overlay);
    let mut out: Vec<OverlaySection> = Vec::new();
    let mut pos = 0usize;
    while pos + 16 <= overlay.len() {
        let (dest, size, kind, entry) = (b.u32(pos)?, b.u32(pos + 4)? as usize, b.u32(pos + 8)?, b.u32(pos + 12)?);
        if out.first().is_some_and(|f| f.entry != entry) { break; }
        let data = b.sub(pos + 16, size, "overlay section")?.bytes().to_vec();
        out.push(OverlaySection { dest, kind, entry, data });
        pos += 16 + size;
    }
    if out.is_empty() { return invalid("overlay has no sections"); }
    Ok(out)
}

/// `n` bytes at EE address `addr` of the loaded overlay.
pub(crate) fn read_overlay(sections: &[OverlaySection], addr: u32, n: usize) -> Option<&[u8]> {
    sections.iter().filter(|s| s.kind != 8).find_map(|s| {
        let off = addr.checked_sub(s.dest)? as usize;
        s.data.get(off..off.checked_add(n)?)
    })
}
