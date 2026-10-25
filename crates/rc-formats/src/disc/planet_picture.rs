//! The planets' pictures of the pause's map and the ship's pages (L00 `0x351410` / `0x355508`): per level a
//! WAD-compressed lump of `HUD.WAD` (the `HUD` header's rows at +0x1188, `0x14ca30`, and the alternate set at +0x126c,
//! `0x14cb14`, used with the byte `0x1a7b05`; `{sector, size}` from the WAD's start), whose words `+0x10`, `+0x14`
//! and `+0x18` are the offsets of three PIFs: the planet's surface, a second strip and the globe's shading. The game
//! builds three 128 × 128 textures (`0x3064b8(7, 7, …)`) of the first PIF's palette with each PIF's pixels, and
//! draws the surface scrolling under the shading (`0x355508`: the kind per level `0x28a3b8`).

use crate::buf::{invalid, Buf, Result};
use crate::texture::{decode_indexed8, Texture};

/// `0x28a3b8`: how the picture is drawn per level: 0 one still quad, else the surface inset by [`INSET`] under the
/// shading, scrolling.
pub const KINDS: [u8; 21] = [0, 1, 0, 2, 3, 0, 2, 2, 2, 2, 0, 2, 2, 1, 1, 3, 2, 1, 2, 2, 3];
/// `0x355508`: the inset per kind on the ship's pages and on the pause's map page.
pub const INSET_SHIP: [i32; 4] = [0, 0x32, 0x26, 0x19];
pub const INSET_MAP: [i32; 4] = [0, 0x26, 0x1c, 0x14];
/// The picture's box on the ship's pages (29, 77, 191 × 178) and on the map page (30, 76, 153 × 140).
pub const BOX_SHIP: [i32; 4] = [0x1d, 0x4d, 0xbf, 0xb2];
pub const BOX_MAP: [i32; 4] = [0x1e, 0x4c, 0x99, 0x8c];
/// The surface scrolls `vsync + level · 0x2ab` sixteenths of a texel, modulo 0x800 (the 128 texels).
pub const SCROLL_PER_LEVEL: i32 = 0x2ab;

/// The three textures.
#[derive(Clone, Debug)]
pub struct PlanetPicture {
    pub surface: Texture,
    pub second: Texture,
    pub shading: Texture,
}

/// From the lump's bytes (WAD-compressed).
pub fn parse(raw: &[u8]) -> Result<PlanetPicture> {
    let block = if crate::wad::is_wad(raw) { crate::wad::decompress(raw)? } else { raw.to_vec() };
    let b = Buf(&block);
    let at = |k: usize| -> Result<usize> {
        let o = b.i32(0x10 + 4 * k)?;
        if o <= 0 || block.len() <= o as usize { return invalid(format!("planet picture: PIF {k} at {o}")); }
        Ok(o as usize)
    };
    let p4 = crate::pif::Pif::parse_at(&block, at(0)?)?;
    let p5 = crate::pif::Pif::parse_at(&block, at(1)?)?;
    let p6 = crate::pif::Pif::parse_at(&block, at(2)?)?;
    Ok(PlanetPicture {
        surface: decode_indexed8(p4.pixels, p4.width, p4.height, p4.palette)?,
        second: decode_indexed8(p5.pixels, p5.width, p5.height, p4.palette)?,
        shading: decode_indexed8(p6.pixels, p6.width, p6.height, p4.palette)?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_planet_picture_decodes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted");
        let (Ok(hdr), Ok(hud)) = (std::fs::read(root.join("RC2.HDR")), std::fs::read(root.join("G/HUD.WAD"))) else { return };
        let toc = crate::disc::toc::Toc::parse(&hdr).unwrap();
        let part = toc.planet_picture(1, false).unwrap().expect("Oozla's picture");
        let p = super::parse(&hud[part.bytes()]).unwrap();
        let dir = std::path::Path::new("/private/tmp/claude-501/-Users-aslanhud-Repos-randcre/780d5ae9-5d6d-4c3e-9999-b946bde12c64/scratchpad");
        for (n, t) in [("surface", &p.surface), ("second", &p.second), ("shading", &p.shading)] {
            let _ = std::fs::write(dir.join(format!("oozla_{n}_{}x{}.rgba", t.width, t.height)), &t.rgba);
        }
        assert_eq!((p.surface.width, p.surface.height), (128, 128));
        let g = toc.item_picture(crate::disc::toc::PictureTable::Galaxy, 0).unwrap().expect("the galaxy backdrop");
        let pic = crate::disc::item_picture::parse(&hud[g.bytes()]).unwrap();
        let _ = std::fs::write(dir.join(format!("galaxy_{}x{}.rgba", pic.width, pic.height)), &pic.rgba);
    }
}
