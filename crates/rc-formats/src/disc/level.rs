//! A Going Commando level file, `G/LEVELn.WAD`: the 0x60-byte level header, then its parts by sector. The data part
//! starts with its own table of `{offset, size}` byte ranges (0x80 bytes), the lumps the level overlay and
//! `LoadLevelCoreData` read. The lumps that R&C1 kept as separate files (`levels/NN/core_index.bin`, `gs_ram.bin`,
//! `core_data.bin`, `gameplay_ntsc.bin`, …) are slices of this one file, and where their layout matches R&C1's the
//! shared readers take them as they are (docs/plan/bringup.md §6).

use crate::buf::{invalid, Buf, Result};
use std::ops::Range;

/// The level header's parts (`{s32 sector, s32 sectors}` from +0x10, sectors from the file's start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part { Data = 0, CoreBank = 1, Gameplay = 2, Occlusion = 3 }

/// The flight's arrival pictures of a planet ([`LevelWad::planet_pictures`]): the planet seen from space (the quad
/// the ship flies toward) and its name as a picture per language (English first).
#[derive(Clone, Debug)]
pub struct PlanetPictures {
    pub picture: Option<crate::texture::Texture>,
    pub captions: Vec<crate::texture::Texture>,
}

/// The data part's lumps, in its table's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lump {
    Overlay = 0,
    CoreIndex = 1,
    GsRam = 2,
    /// 0xfdc bytes on level 0; not mapped yet.
    Lump3 = 3,
    HudHeader = 4,
    HudBank0 = 5,
    HudBank1 = 6,
    HudBank2 = 7,
    HudBank3 = 8,
    /// WAD-compressed (the core index's `assets_compressed_size`).
    CoreData = 9,
    Transition = 10,
}

pub struct LevelWad<'a> {
    file: &'a [u8],
    /// The level number the header carries (+8).
    pub id: i32,
    data: Range<usize>,
}

impl<'a> LevelWad<'a> {
    pub fn parse(file: &'a [u8]) -> Result<LevelWad<'a>> {
        let b = Buf(file);
        if b.u32(0)? != 0x60 { return invalid(format!("level WAD header size {:#x}, 0x60 expected", b.u32(0)?)); }
        let mut w = LevelWad { file, id: b.i32(8)?, data: 0..0 };
        w.data = w.part(Part::Data)?.ok_or_else(|| crate::buf::FormatError::Invalid("level WAD without a data part".into()))?;
        Ok(w)
    }

    /// A part's byte range in the file (sector sized), `None` when empty.
    pub(crate) fn part(&self, p: Part) -> Result<Option<Range<usize>>> {
        let b = Buf(self.file);
        let o = 0x10 + 8 * p as usize;
        let (sector, sectors) = (b.u32(o)? as usize, b.u32(o + 4)? as usize);
        if sectors == 0 { return Ok(None); }
        let r = sector * 2048..((sector + sectors) * 2048).min(self.file.len());
        if r.start >= self.file.len() { return invalid(format!("level WAD part {p:?} starts past the file")); }
        Ok(Some(r))
    }

    pub fn part_bytes(&self, p: Part) -> Result<Option<&'a [u8]>> { Ok(self.part(p)?.map(|r| &self.file[r])) }

    /// The planet's arrival pictures (the flight's variant 4, `0x363360` / `0x3630a8`): the loader `0x304420` reads
    /// the destination's data part (the boot's `0x294310`: the level header's `+0x10` / `+0x14` plus the file's disc
    /// sector, into `0x1a7288`), and `0x3630a8` takes the lump at its table's `+0x50` — lump 10, [`Lump::Transition`],
    /// WAD-compressed — whose words `+4` and `+8 + 4·(language − 1)` are the offsets of the planet's picture and of
    /// its caption (PIFs, `0x2fe120`'s TEX0). `None` without the lump.
    pub fn planet_pictures(&self) -> Result<Option<PlanetPictures>> {
        let Some(lump) = self.lump(Lump::Transition)? else { return Ok(None) };
        let block = crate::wad::decompress(lump)?;
        let w = |o: usize| Buf(&block).i32(o);
        let pif = |o: i32| -> Result<Option<crate::texture::Texture>> {
            if o <= 0 || block.len() <= o as usize { return Ok(None); }
            Ok(Some(crate::pif::Pif::parse_at(&block, o as usize)?.decode()?))
        };
        let picture = pif(w(4)?)?;
        let mut captions = Vec::new();
        for k in 0..6 {
            match pif(w(8 + 4 * k)?)? {
                Some(t) => captions.push(t),
                None => break,
            }
        }
        Ok(Some(PlanetPictures { picture, captions }))
    }

    /// A lump of the data part (exact size), `None` when empty.
    pub fn lump(&self, l: Lump) -> Result<Option<&'a [u8]>> {
        let data = &self.file[self.data.clone()];
        let b = Buf(data);
        let o = 8 * l as usize;
        let (off, size) = (b.i32(o)?, b.i32(o + 4)?);
        if off < 0 || size <= 0 { return Ok(None); }
        let (off, size) = (off as usize, size as usize);
        data.get(off..off + size).map(Some).ok_or_else(|| crate::buf::FormatError::Invalid(format!("level lump {l:?} runs past the data part")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planet_pictures_of_a_level_decode() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted");
        let Ok(file) = std::fs::read(root.join("G/LEVEL3.WAD")) else { return };
        let w = LevelWad::parse(&file).unwrap();
        let p = w.planet_pictures().unwrap().expect("part 1");
        let pic = p.picture.expect("a planet picture");
        println!("planet picture {}x{}, {} captions: {:?}", pic.width, pic.height, p.captions.len(), p.captions.iter().map(|c| (c.width, c.height)).collect::<Vec<_>>());
        // A look at the data: the decoded pictures as raw RGBA (passing tests' output is captured).
        let dir = std::path::Path::new("/private/tmp/claude-501/-Users-aslanhud-Repos-randcre/780d5ae9-5d6d-4c3e-9999-b946bde12c64/scratchpad");
        let _ = std::fs::write(dir.join(format!("planet3_{}x{}.rgba", pic.width, pic.height)), &pic.rgba);
        if let Some(c) = p.captions.first() { let _ = std::fs::write(dir.join(format!("caption3_{}x{}.rgba", c.width, c.height)), &c.rgba); }
        assert!(!p.captions.is_empty());
    }
}
