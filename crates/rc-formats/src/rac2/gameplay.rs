//! A Going Commando gameplay file (the level file's gameplay part, WAD-compressed; `rac2::level::Part::Gameplay`):
//! 40 header words of section offsets (0 = absent), then the sections. The order is R&C1's
//! (`crate::gameplay::SECTION_NAMES`) with sections added; the records that the shared readers take are R&C1's or
//! map to R&C1's structures here (docs/plan/rac2_bringup.md §6.1).
//!
//! **How each name is known** (Aranos, level 0; [`SECTIONS`]): *content* = the payload has R&C1's layout or carries
//! counts that match the level (e.g. the occlusion mappings hold 833 tfrags and 2000 ties, the core's counts);
//! *order* = only its place in R&C1's order says what it is (to confirm in R&C2's level loader).

use crate::buf::{invalid, Buf, Result};
use crate::gameplay::MobyInstance;
use crate::shrub::{self, ShrubInstance};
use crate::tie::TieInstance;

/// Header words (0x28 × 4 = 0xa0 bytes; the last is 0 on every level seen).
pub const HEADER_WORDS: usize = 40;

/// `(name, known by)` per header word.
pub const SECTIONS: [(&str, &str); HEADER_WORDS] = [
    ("level_settings", "content"),
    ("directional_lights", "content"),
    ("cameras", "content"),
    ("sound_instances", "content"),
    ("help_us_english", "content"),
    ("help_uk_english", "content"),
    ("help_french", "content"),
    ("help_german", "content"),
    ("help_spanish", "content"),
    ("help_italian", "content"),
    ("help_japanese", "content"),
    ("help_korean", "content"),
    ("tie_classes", "content"),
    ("tie_instances", "content"),
    // New in R&C2: a count (0x1b on Aranos) and 0x800-byte records.
    ("tie_groups", "order"),
    ("shrub_classes", "content"),
    ("shrub_instances", "content"),
    // New in R&C2: a count (1 on Aranos).
    ("shrub_groups", "order"),
    ("moby_classes", "content"),
    ("moby_instances", "content"),
    ("moby_groups", "order"),
    ("shared_data", "order"),
    ("pvar_moby_links", "order"),
    ("pvar_table", "content"),
    ("pvar_data", "order"),
    ("pvar_pointer_fixups", "order"),
    ("cuboids", "content"),
    ("spheres", "order"),
    ("cylinders", "order"),
    ("pills", "order"),
    ("paths", "content"),
    ("grind_paths", "order"),
    ("unknown_32", "order"),
    ("unknown_33", "order"),
    ("unknown_34", "order"),
    ("unknown_35", "order"),
    ("occlusion_mappings", "content"),
    // New in R&C2: RGBA5551 colours for the ties' vertices (the colour indices of R&C1's tie packets are gone).
    ("tie_colors", "content"),
    // The trigger areas (L00 `0x3012a0` → `0x1b1a80`, tested by `0x32a848`).
    ("areas", "content"),
    ("unused_39", "order"),
];

/// The index of a named section in [`SECTIONS`].
pub fn index(name: &str) -> Option<usize> { SECTIONS.iter().position(|s| s.0 == name) }

/// The present sections as (name, byte range) in file order: each runs to the next section's start.
pub fn sections(gameplay: &[u8]) -> Result<Vec<(&'static str, std::ops::Range<usize>)>> {
    let g = Buf(gameplay);
    let mut starts = Vec::new();
    for (i, (name, _)) in SECTIONS.iter().enumerate() {
        let o = g.i32(4 * i)?;
        if o > 0 { starts.push((o as usize, *name)); }
    }
    starts.sort();
    let mut out = Vec::with_capacity(starts.len());
    for (k, &(start, name)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(gameplay.len(), |s| s.0);
        if end > gameplay.len() { return invalid(format!("gameplay section {name} runs past the end")); }
        out.push((name, start..end));
    }
    Ok(out)
}

/// One named section's bytes, `None` when absent.
pub fn section<'a>(gameplay: &'a [u8], name: &str) -> Result<Option<&'a [u8]>> {
    Ok(sections(gameplay)?.into_iter().find(|s| s.0 == name).map(|(_, r)| &gameplay[r]))
}

/// The level-settings section (R&C1's fields where they are the same; header word 0).
pub fn level_settings(gameplay: &[u8]) -> Result<Option<&[u8]>> { section(gameplay, "level_settings") }

/// R&C2's tie instance (0x60 bytes): R&C1's record without the 64 per-light-slot colours (0x50..0xd0), so
/// `{o_class, draw distance, 0, occlusion index, matrix, directional lights, uid, 0, 0}`.
const TIE_INSTANCE: usize = 0x60;

/// The tie instances (`s32 count`, 12 bytes pad, `count` × 0x60), as R&C1's structure with no slot colours.
pub fn tie_instances(gameplay: &[u8]) -> Result<Vec<TieInstance>> {
    let Some(s) = section(gameplay, "tie_instances")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let count = b.i32(0)?;
    if !(0..=100_000).contains(&count) { return invalid("implausible tie instance count"); }
    (0..count as usize)
        .map(|i| {
            let o = 0x10 + i * TIE_INSTANCE;
            let r = b.sub(o, TIE_INSTANCE, "tie instance")?;
            Ok(TieInstance {
                o_class: r.i32(0)?,
                draw_distance: r.i32(4)?,
                pad_8: r.i32(8)?,
                occlusion_index: r.i32(0xc)?,
                matrix: r.pod(0x10, "tie matrix")?,
                ambient_rgbas: [0; 64],
                directional_lights: r.i32(0x50)?,
                uid: r.i32(0x54)?,
                pad_d8: r.i32(0x58)?,
                pad_dc: r.i32(0x5c)?,
            })
        })
        .collect()
}

/// The shrub instances: R&C1's 0x70-byte record, unchanged.
pub fn shrub_instances(gameplay: &[u8]) -> Result<Vec<ShrubInstance>> {
    match section(gameplay, "shrub_instances")? { Some(s) => shrub::parse_shrub_instance_section(s), None => Ok(Vec::new()) }
}

/// R&C2's moby instance (0x88 bytes): R&C1's 0x78-byte record with four words inserted at 0x10 (after the spawn
/// id; their meaning is not mapped yet), so the class is at 0x28 and the position at 0x40.
pub const MOBY_INSTANCE: usize = 0x88;

/// A moby instance: R&C1's fields, and the four words R&C2 adds.
#[derive(Clone, Copy, Debug)]
pub struct Rac2MobyInstance {
    pub base: MobyInstance,
    pub extra: [i32; 4],
}

/// The static moby instances (`s32 count, s32 spawnable, pad[2]`, then records stepped by their size word).
pub fn moby_instances(gameplay: &[u8]) -> Result<Vec<Rac2MobyInstance>> {
    let Some(s) = section(gameplay, "moby_instances")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let count = b.i32(0)?;
    if !(0..=10_000).contains(&count) { return invalid("implausible moby instance count"); }
    let mut out = Vec::with_capacity(count as usize);
    let mut p = 0x10;
    for i in 0..count as usize {
        let size = b.i32(p)?;
        if size as usize != MOBY_INSTANCE { return invalid(format!("moby instance {i}: size {size:#x}, expected 0x88")); }
        // R&C1's record is this one without bytes 0x10..0x20.
        let mut rec = Vec::with_capacity(crate::gameplay::MOBY_INSTANCE_SIZE);
        rec.extend_from_slice(b.sub(p, 0x10, "moby instance")?.bytes());
        rec.extend_from_slice(b.sub(p + 0x20, MOBY_INSTANCE - 0x20, "moby instance")?.bytes());
        let mut base: MobyInstance = Buf(&rec).pod(0, "moby instance")?;
        base.size = crate::gameplay::MOBY_INSTANCE_SIZE as i32;
        out.push(Rac2MobyInstance { base, extra: [b.i32(p + 0x10)?, b.i32(p + 0x14)?, b.i32(p + 0x18)?, b.i32(p + 0x1c)?] });
        p += MOBY_INSTANCE;
    }
    Ok(out)
}

/// The instance pvars (sections 23 `pvar_table` and 24 `pvar_data`): `{s32 offset, s32 size}` per pvar index into the
/// data. The loader (L00 `0x3012a0`) copies each moby's block (its record's pvar index, `+0x68`) out of the data.
pub fn pvar_blocks(gameplay: &[u8]) -> Result<Vec<Vec<u8>>> {
    let (Some(table), Some(data)) = (section(gameplay, "pvar_table")?, section(gameplay, "pvar_data")?) else { return Ok(Vec::new()) };
    let t = Buf(table);
    let n = table.len() / 8;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (off, size) = (t.i32(8 * i)?, t.i32(8 * i + 4)?);
        if off < 0 || size < 0 || off as usize + size as usize > data.len() { return invalid(format!("pvar {i}: {off:#x} + {size:#x} past the data")); }
        out.push(data[off as usize..off as usize + size as usize].to_vec());
    }
    Ok(out)
}

/// `{pvar index, byte offset}` pairs up to a negative index: section 22 `pvar_moby_links` (the word there is a moby
/// record index, which the loader turns into the runtime moby's) and section 25 `pvar_pointer_fixups` (a pointer
/// relative to its own block, which the loader makes absolute).
pub fn pvar_fixups(gameplay: &[u8], name: &str) -> Result<Vec<(usize, usize)>> {
    let Some(s) = section(gameplay, name)? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let mut out = Vec::new();
    let mut p = 0;
    while p + 8 <= s.len() {
        let (i, off) = (b.i32(p)?, b.i32(p + 4)?);
        if i < 0 { break; }
        out.push((i as usize, off.max(0) as usize));
        p += 8;
    }
    Ok(out)
}

/// The moby groups (section 20): `s32 count, s32 bytes, pad[2]`, then `count` offsets (−1 none) into the lists after
/// them; a list is u16 moby record indices, the last with bit 15 (L00 `0x3012a0`: the members become runtime moby
/// indices, records that did not spawn are dropped; the group table `0x1fa8a0`).
pub fn moby_groups(gameplay: &[u8]) -> Result<Vec<Vec<u16>>> {
    let Some(s) = section(gameplay, "moby_groups")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let count = b.i32(0)?;
    if !(0..=0xf0).contains(&count) { return invalid(format!("moby groups: count {count}")); }
    let lists = 0x10 + 4 * count as usize;
    let mut out = Vec::with_capacity(count as usize);
    for g in 0..count as usize {
        let off = b.i32(0x10 + 4 * g)?;
        let mut members = Vec::new();
        if off >= 0 {
            let mut p = lists + off as usize;
            loop {
                let v = b.u16(p)?;
                members.push(v & 0x7fff);
                if v & 0x8000 != 0 { break; }
                p += 2;
            }
        }
        out.push(members);
    }
    Ok(out)
}

/// A grind path (section 31 `grind_paths`): its bounding sphere, the word at record `+0x10` (the loader moves it to
/// `+0x14`: the path loops when nonzero) and the spline, points `{x, y, z, the length to the next}`.
#[derive(Clone, Debug, PartialEq)]
pub struct GrindPath {
    pub sphere: [f32; 4],
    pub looped: bool,
    pub points: Vec<[f32; 4]>,
}

/// The grind paths: `s32 count, s32 data offset, s32 data bytes, pad`, then `count` 0x20-byte records (the sphere,
/// the loop word), then `count` offsets into the data, each at a spline `{s32 n, pad[3], n × vec4}` (L00 `0x3012a0`;
/// the table `0x1b1640`).
pub fn grind_paths(gameplay: &[u8]) -> Result<Vec<GrindPath>> {
    let Some(s) = section(gameplay, "grind_paths")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let count = b.i32(0)?;
    if !(0..=0x400).contains(&count) { return invalid(format!("grind paths: count {count}")); }
    let data = b.i32(4)? as usize;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let r = 0x10 + 0x20 * i;
        let sphere = [b.f32(r)?, b.f32(r + 4)?, b.f32(r + 8)?, b.f32(r + 12)?];
        let looped = b.i32(r + 0x10)? != 0;
        let off = b.i32(0x10 + 0x20 * count as usize + 4 * i)? as usize;
        let p = data + off;
        let n = b.i32(p)?;
        if !(0..=0x10000).contains(&n) { return invalid(format!("grind path {i}: {n} points")); }
        let mut points = Vec::with_capacity(n as usize);
        for k in 0..n as usize {
            let q = p + 0x10 + 0x10 * k;
            points.push([b.f32(q)?, b.f32(q + 4)?, b.f32(q + 8)?, b.f32(q + 12)?]);
        }
        out.push(GrindPath { sphere, looped, points });
    }
    Ok(out)
}

/// A volume record (sections 26..29 `cuboids` / `spheres` / `cylinders` / `pills`, 0x80 bytes): the matrix (its
/// translation row at `+0x30`), the 3×4 inverse at `+0x40` (a point is inside when `inverse · (p − position)` is
/// within ±1, L00 `0x321858`) and the words at `+0x70`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub matrix: [[f32; 4]; 4],
    pub inverse: [[f32; 4]; 3],
    pub extra: [f32; 4],
}

/// One volume section: `s32 count`, pad to 0x10, `count` × 0x80 (L00 `0x3012a0`: the tables `0x1b1f0c` cuboids,
/// `0x1b1f14` spheres, `0x1b1f1c` cylinders).
pub fn volumes(gameplay: &[u8], name: &str) -> Result<Vec<Volume>> {
    let Some(s) = section(gameplay, name)? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let n = b.i32(0)?;
    if !(0..=0x4000).contains(&n) { return invalid(format!("{name}: count {n}")); }
    (0..n as usize)
        .map(|i| {
            let o = 0x10 + 0x80 * i;
            let row = |k: usize| -> Result<[f32; 4]> { Ok([b.f32(o + k)?, b.f32(o + k + 4)?, b.f32(o + k + 8)?, b.f32(o + k + 12)?]) };
            Ok(Volume { matrix: [row(0)?, row(0x10)?, row(0x20)?, row(0x30)?], inverse: [row(0x40)?, row(0x50)?, row(0x60)?], extra: row(0x70)? })
        })
        .collect()
}

/// The paths (section 30 `paths`): `s32 count, s32 data offset, s32 data bytes, pad`, `count` offsets into the data,
/// each at a spline `{s32 n, pad[3], n × vec4}` (L00 `0x3012a0`: the table `0x1fda00`). The `w` of the points is
/// what the file holds (objects that walk a path fill in the segment lengths).
pub fn paths(gameplay: &[u8]) -> Result<Vec<Vec<[f32; 4]>>> {
    let Some(s) = section(gameplay, "paths")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let count = b.i32(0)?;
    if !(0..=0x4000).contains(&count) { return invalid(format!("paths: count {count}")); }
    let data = b.i32(4)? as usize;
    (0..count as usize)
        .map(|i| {
            let p = data + b.i32(0x10 + 4 * i)? as usize;
            let n = b.i32(p)?;
            if !(0..=0x10000).contains(&n) { return invalid(format!("path {i}: {n} points")); }
            (0..n as usize).map(|k| { let q = p + 0x10 + 0x10 * k; Ok([b.f32(q)?, b.f32(q + 4)?, b.f32(q + 8)?, b.f32(q + 12)?]) }).collect()
        })
        .collect()
}

/// A trigger area (section 38 `areas`, 0x30 bytes from `+0x20`): its bounding sphere and the volumes it is made of —
/// paths (xy polygons), cuboids, spheres, cylinders and a fifth kind (unused here) — as indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Area {
    pub sphere: [f32; 4],
    pub lists: [Vec<i32>; 5],
}

/// The areas: `s32 count`, the five lists' offsets (`+0x04..+0x14`, from the section's start), then the records
/// (`+0x10` five s16 counts, `+0x1c` five offsets into their lists).
pub fn areas(gameplay: &[u8]) -> Result<Vec<Area>> {
    let Some(s) = section(gameplay, "areas")? else { return Ok(Vec::new()) };
    let b = Buf(s);
    let n = b.i32(0)?;
    if !(0..=0x1000).contains(&n) { return invalid(format!("areas: count {n}")); }
    let bases: Vec<usize> = (0..5).map(|k| b.i32(4 + 4 * k).map(|v| v as usize)).collect::<Result<_>>()?;
    (0..n as usize)
        .map(|i| {
            let r = 0x20 + 0x30 * i;
            let sphere = [b.f32(r)?, b.f32(r + 4)?, b.f32(r + 8)?, b.f32(r + 12)?];
            let mut lists: [Vec<i32>; 5] = Default::default();
            for k in 0..5 {
                let c = b.u16(r + 0x10 + 2 * k)? as i16;
                if c <= 0 || bases[k] == 0 { continue; }
                let o = bases[k] + b.i32(r + 0x1c + 4 * k)? as usize;
                lists[k] = (0..c as usize).map(|j| b.i32(o + 4 * j)).collect::<Result<_>>()?;
            }
            Ok(Area { sphere, lists })
        })
        .collect()
}
