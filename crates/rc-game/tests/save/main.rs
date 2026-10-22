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
