//! Saves against the disc: the blank save (`MISC.WAD` part 2) loaded into the game state and stored back comes out
//! byte for byte, so every typed field sits where the game keeps it. Skipped without the data folder.

use rc_formats::disc::boot::BootElf;
use rc_formats::disc::save_game::{ChunkTables, SaveBlob, SaveFile};
use rc_formats::disc::toc::Toc;
use rc_game::game::GameState;
use rc_game::save::SaveData;

fn read(rel: &str) -> Option<Vec<u8>> { std::fs::read(rc_formats::test_data::root().join(rel)).ok() }

/// The blank save and its tables.
pub fn blank() -> Option<(SaveData, Vec<u8>)> {
    let hdr = read("RC2.HDR")?;
    let toc = Toc::parse(&hdr).unwrap();
    let elf = read("SCUS_972.68")?;
    let tables = ChunkTables::from_boot_elf(&BootElf::parse(&elf).unwrap()).unwrap();
    let misc = read("G/MISC.WAD")?;
    let blob = SaveBlob::parse(&misc[toc.misc_part(2).unwrap().bytes()], tables.file_size()).unwrap();
    let file = SaveFile::parse(&blob.template, tables.global_size()).unwrap();
    Some((SaveData::from_file(tables, file), blob.template))
}

#[test]
fn blank_save_round_trips_through_the_game_state() {
    let Some((mut s, template)) = blank() else { eprintln!("skipped: no disc data"); return };
    let mut g = GameState::default();
    g.load(&s);
    println!("blank save: level {}, bolts {}, health limit {}, items had {:?}", g.level, g.bolts, g.max_health,
        g.items.iter().enumerate().filter(|(_, &b)| b != 0).map(|(i, _)| i).collect::<Vec<_>>());
    g.store(&mut s);
    assert_eq!(s.to_file().to_bytes(s.max()), template);
}

mod card {
    use super::*;
    use rc_formats::disc::save_game::{Chunk, Section};
    use rc_game::memcard::{status, CardFs, MemCard};
    use std::path::PathBuf;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rerac2-card-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn blob() -> Option<SaveBlob> {
        let hdr = read("RC2.HDR")?;
        let toc = Toc::parse(&hdr).unwrap();
        let elf = read("SCUS_972.68")?;
        let tables = ChunkTables::from_boot_elf(&BootElf::parse(&elf).unwrap()).unwrap();
        let misc = read("G/MISC.WAD")?;
        Some(SaveBlob::parse(&misc[toc.misc_part(2).unwrap().bytes()], tables.file_size()).unwrap())
    }

    /// Frames of the main loop: the monitor, then the machine, until `done` (at most `n` frames).
    fn run(c: &mut MemCard, s: &mut SaveData, n: usize, done: impl Fn(&MemCard) -> bool) -> usize {
        for f in 0..n {
            c.monitor();
            c.update(s);
            if done(c) { return f; }
        }
        panic!("not done in {n} frames: status {}, state {:#x}/{:#x}, error {:#x}", c.status, c.state, c.sub, c.error);
    }

    #[test]
    fn create_save_load_and_previews_on_a_native_card() {
        let (Some((mut s, _)), Some(b)) = (blank(), blob()) else { eprintln!("skipped: no disc data"); return };
        let root = temp("cycle");
        let mut c = MemCard::new(CardFs::new(Some(root.clone())), "BASCUS-97268RATCHET".into(), Some(b.clone()));

        // A save is asked for: the check finds no folder, the create makes it, the check after it turns auto-save on.
        c.flags |= 2;
        run(&mut c, &mut s, 400, |c| c.status == status::NO_DATA);
        c.flags |= 2;
        run(&mut c, &mut s, 50, |c| c.status == status::CREATE_ASK);
        c.flags |= 0x10;
        run(&mut c, &mut s, 400, |c| c.status == status::READY);
        assert!(c.checked && c.error == 0);
        let dir = root.join("BASCUS-97268RATCHET");
        let size = |n: &str| std::fs::metadata(dir.join(n)).map(|m| m.len()).unwrap_or(0);
        assert_eq!((size("icon.sys"), size("static.ico"), size("BASCUS-97268RATCHET")), (0x3c4, b.static_ico.len() as u64, 0xd804));
        for k in 0..4 { assert_eq!(std::fs::read(dir.join(format!("save{k}.bin"))).unwrap(), b.template, "save{k}"); }

        // A whole save of a game into slot 2, then a load of it into a blank save.
        let mut g = GameState::default();
        g.load(&s);
        (g.level, g.bolts, g.max_health) = (0, 4321, 8);
        g.missions[3] = 0x80;
        g.store(&mut s);
        c.save_whole(&s, 2);
        run(&mut c, &mut s, 200, |c| c.idle() && c.state == 2);
        assert_eq!(c.error, 0);
        let (mut fresh, _) = blank().unwrap();
        c.load(2);
        run(&mut c, &mut fresh, 400, |c| c.loaded);
        assert_eq!(c.cards[0].errors, 0);
        let mut back = GameState::default();
        back.load(&fresh);
        assert_eq!((back.level, back.bolts, back.max_health, back.missions[3]), (0, 4321, 8, 0x80));

        // An incremental save (memcard_Save) after more bolts: the global section and level 0's.
        back.bolts = 5000;
        back.store(&mut fresh);
        c.save_card = 0;
        assert!(c.memcard_save(&mut fresh, true, -1, [0, 0x30, 0x15, 0x12, 0, 0x22, 0x10, 0x26]));
        run(&mut c, &mut fresh, 200, |c| c.idle() && c.state == 2);
        assert_eq!(c.error, 0);
        assert_eq!((c.cards[0].previews[2].bolts, c.cards[0].previews[2].level), (5000, 0));

        // A new card machine on the same folder: the check finds it whole, the previews read the slots.
        let mut c2 = MemCard::new(CardFs::new(Some(root.clone())), "BASCUS-97268RATCHET".into(), Some(b));
        c2.flags |= 4;
        run(&mut c2, &mut s, 600, |c| c.status == status::FOUND && c.state < 3 && c.req_state < 0);
        let p = c2.cards[0].previews;
        assert_eq!((p[0].level, p[2].level, p[2].bolts, p[2].clock[2]), (-1, 0, 5000, 0x15));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The scan of R&C1's saves: chunk 10's five bytes become the bonus bits.
    #[test]
    fn rac1_save_scan_sets_the_bonus_bits() {
        let Some((mut s, _)) = blank() else { eprintln!("skipped: no disc data"); return };
        let root = temp("rac1");
        let mut data = vec![0u8; 0x20];
        data[0xa] = 1;
        data[0x19] = 1;
        let sec = Section::new(vec![Chunk::new(0, vec![0; 4]), Chunk::new(10, data)]).encode(s.max());
        let mut file = vec![0u8; 8];
        file.extend_from_slice(&sec);
        file.resize(0x2800, 0);
        std::fs::create_dir_all(root.join("BASCUS-97199RATCHET")).unwrap();
        std::fs::write(root.join("BASCUS-97199RATCHET/save0.bin"), &file).unwrap();
        let mut c = MemCard::new(CardFs::new(Some(root.clone())), "BASCUS-97268RATCHET".into(), None);
        // The machine alone (the monitor's re-check would clear the request; the game asks from a settled status).
        c.request(0x18, 0);
        for _ in 0..200 {
            c.update(&mut s);
            if c.state == 2 && c.req_state < 0 && c.rac1_bonus != 0 { break; }
        }
        assert_eq!(c.rac1_bonus, 0x80 | 1 | 4);
        assert_eq!(c.cards[0].bonus, 3);
        let _ = std::fs::remove_dir_all(&root);
    }
}
