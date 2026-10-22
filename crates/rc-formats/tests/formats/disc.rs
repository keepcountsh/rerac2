//! `disc` on the disc files (`SCUS_972.68` v2.00): the table of contents, the front-end blob, the GUI
//! sprites and the boot ELF's tables, against the values read in Ghidra (docs/plan/bringup.md §5). Skipped
//! without the data folder (`extracted/`, or `RC_EXTRACTED`).

use rc_formats::disc::boot::{BootElf, GLYPHS, MAIN_MENU};
use rc_formats::disc::front_end::FrontEnd;
use rc_formats::disc::gui_sprite::{self, Prim};
use rc_formats::disc::toc::{movie_row, Toc, MISC_FRONT_END};

fn read(rel: &str) -> Option<Vec<u8>> { std::fs::read(rc_formats::test_data::root().join(rel)).ok() }

#[test]
fn front_end_and_main_menu() {
    let Some(hdr) = read("RC2.HDR") else { eprintln!("skipped: no RC2.HDR in the data folder"); return };
    let toc = Toc::parse(&hdr).unwrap();
    let part = toc.misc_part(MISC_FRONT_END).unwrap();
    assert_eq!((part.sector, part.sectors), (1283, 1109));
    let misc = read("G/MISC.WAD").unwrap();
    let fe = FrontEnd::parse(rc_formats::wad::decompress(&misc[part.bytes()]).unwrap()).unwrap();
    for i in 0..3 {
        let s = fe.splash(i).unwrap();
        assert_eq!((s.width, s.height, s.rgba.len()), (512, 416, 512 * 416 * 4), "splash {i}");
    }
    assert_eq!(fe.backdrop().unwrap().width, 512);
    let tex = fe.textures().unwrap();
    assert_eq!(tex.iter().map(|t| (t.width, t.height)).collect::<Vec<_>>(), [(32, 32), (256, 128), (256, 128)]);
    let en = fe.strings(0).unwrap();
    assert_eq!(en.get(0x2c35), b"New Game");
    assert_eq!(en.get(0x2c2b), b"Load");
    assert_eq!(en.get(0x2bec), b"Options");
    assert_eq!(en.get(0x2db6), b"Press START");

    let elf = read("SCUS_972.68").unwrap();
    let boot = BootElf::parse(&elf).unwrap();
    let rows = boot.menu(MAIN_MENU).unwrap();
    assert_eq!(rows.iter().map(|r| (r.text, r.kind)).collect::<Vec<_>>(), [(0x2c35, 4), (0x2c2b, 5), (0x2bec, 3)]);
    let g = boot.glyphs(GLYPHS).unwrap();
    assert_eq!((g[b'A' as usize].u, g[b'A' as usize].v, g[b'A' as usize].advance), (0, 0, 14));
    assert_eq!((g[b'0' as usize].v, g[b'0' as usize].advance), (0x40, 13));
}

#[test]
fn gui_sprites_and_movies() {
    let Some(hdr) = read("RC2.HDR") else { eprintln!("skipped: no RC2.HDR in the data folder"); return };
    let toc = Toc::parse(&hdr).unwrap();
    let block = toc.gui_block().unwrap();
    assert_eq!((block.sector, block.sectors), (16415, 64));
    let hud = read("G/HUD.WAD").unwrap();
    let sprites = gui_sprite::parse(&hud[block.bytes()]).unwrap();
    assert_eq!(sprites.len(), 230);
    let (fill, frame) = (&sprites[&0x73], &sprites[&0x74]);
    assert_eq!((fill.vertices.len(), fill.frames.len(), fill.frames[0].len()), (6, 2, 2));
    assert_eq!(frame.vertices.len(), 28);
    assert_eq!(fill.prims, [Prim::Texture(0xff), Prim::Quad([0, 1, 2, 3])]);
    // Bone 1 slides along x between the two keys (the button's width).
    assert_eq!((fill.frames[0][1].translation[0], fill.frames[1][1].translation[0]), (3654.0, 23762.0));
    assert_eq!(fill.frames[0][1].parent, Some(0));

    let m = toc.movie(movie_row(0xb2).unwrap()).unwrap().unwrap();
    assert_eq!(m.bytes, 22_675_460);
    // The title's couch movies: rows 34..=40, all present.
    for id in 0xb3..=0xb9 { assert!(toc.movie(movie_row(id).unwrap()).unwrap().is_some(), "movie {id:#x}"); }
}

/// The save format against the disc: the boot ELF's chunk tables, the save blob (`MISC.WAD` part 2) and its blank
/// save, whose header sizes equal the tables' (else every save would load as "out of date").
#[test]
fn save_blob_and_template() {
    use rc_formats::disc::save_game::{card_dir_name, ChunkTables, SaveBlob, SaveFile, LEVEL_SLOTS};
    let Some(hdr) = read("RC2.HDR") else { eprintln!("skipped: no RC2.HDR in the data folder"); return };
    let toc = Toc::parse(&hdr).unwrap();
    let elf = read("SCUS_972.68").unwrap();
    let tables = ChunkTables::from_boot_elf(&BootElf::parse(&elf).unwrap()).unwrap();
    let part = toc.misc_part(2).unwrap();
    let misc = read("G/MISC.WAD").unwrap();
    let blob = SaveBlob::parse(&misc[part.bytes()], tables.file_size()).unwrap();
    let save = SaveFile::parse(&blob.template, tables.global_size()).unwrap();
    println!(
        "tables: {} global chunks ({:#x} bytes), {} level chunks ({:#x}); file {:#x}; static.ico {:#x}; template crcs ok {}",
        tables.global.len(), tables.global_size(), tables.level.len(), tables.level_size(), tables.file_size(),
        blob.static_ico.len(), save.all_crcs_ok()
    );
    assert_eq!(u32::from_le_bytes(blob.template[0..4].try_into().unwrap()) as usize, tables.global_size());
    assert_eq!(u32::from_le_bytes(blob.template[4..8].try_into().unwrap()) as usize, tables.level_size());
    assert_eq!(save.levels.len(), LEVEL_SLOTS);
    assert_eq!(save.global.chunks.iter().map(|c| c.id).collect::<Vec<_>>(), tables.global.iter().map(|d| d.id).collect::<Vec<_>>());
    assert_eq!(save.to_bytes(tables.global_size()), blob.template, "the template re-encodes byte for byte");
    let cnf = read("SYSTEM.CNF").unwrap();
    assert_eq!(card_dir_name(&cnf).unwrap(), "BASCUS-97268RATCHET");
}

/// The pause tables of a level found from Aranos's addresses: the same Weapons grid on Oozla, the Lancer's record.
#[test]
fn menu_tables_relocate_across_levels() {
    use rc_formats::disc::level::{LevelWad, Lump};
    use rc_formats::disc::menu_tables::MenuTables;
    let overlay = |i: u32| -> Option<Vec<u8>> {
        let f = read(&format!("G/LEVEL{i}.WAD"))?;
        Some(LevelWad::parse(&f).ok()?.lump(Lump::Overlay).ok()??.to_vec())
    };
    let (Some(a), Some(o)) = (overlay(0), overlay(1)) else { eprintln!("skipped: no disc data"); return };
    let (m0, m1) = (MenuTables::read(&a, &a).unwrap(), MenuTables::read(&o, &a).unwrap());
    use rc_formats::disc::menu_tables::Grid;
    let items = |m: &MenuTables, g| m.grid(g).iter().map(|c| c.item).collect::<Vec<_>>();
    assert_eq!(items(&m0, Grid::Weapons).len(), 24);
    assert_eq!(items(&m0, Grid::Hand), [0xd, 0x24, 0x27, 0, 0x2e, 0x37]);
    assert_eq!(items(&m0, Grid::Packs), [2, 3]);
    assert_eq!(items(&m0, Grid::Boots), [0x14, 0x13, 0x36]);
    assert_eq!(items(&m0, Grid::ItemsMain), [0x15, 7, 0x32, 5, 0x26, 0x33, 4, 8]);
    assert_eq!(m0.grid(Grid::ItemsCounters)[3], rc_formats::disc::menu_tables::GridCell { icon: 0xeaa2, frame: 3, kind: 2, item: 0 });
    for g in Grid::ALL {
        assert_eq!(m0.grid(g), m1.grid(g), "{g:?}");
    }
    let lancer = m0.items[0x1e];
    assert_eq!((lancer.icon, lancer.max_ammo, lancer.uses_ammo), (0xea64, 200, true));
    assert_eq!(m1.items[0x1e], lancer);
}

#[test]
fn item_pictures_decode() {
    let (Some(hdr), Some(hud)) = (read("RC2.HDR"), read("G/HUD.WAD")) else { eprintln!("skipped: no disc data"); return };
    let toc = Toc::parse(&hdr).unwrap();
    let mut n = 0;
    use rc_formats::disc::toc::PictureTable;
    for (t, i) in PictureTable::ALL.into_iter().flat_map(|t| (0..t.rows()).map(move |i| (t, i))) {
        let Some(part) = toc.item_picture(t, i).unwrap() else { continue };
        let r = part.bytes();
        let p = rc_formats::disc::item_picture::parse(&hud[r.start..r.end.min(hud.len())]).unwrap();
        if let Ok(dir) = std::env::var("RC_DUMP_DIR") {
            std::fs::write(format!("{dir}/item_picture_{t:?}_{i}_{}x{}.rgba", p.width, p.height), &p.rgba).unwrap();
        }
        n += 1;
    }
    assert_eq!(n, 74);
}
