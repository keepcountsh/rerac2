//! The memory card in slot 1 and the game's save data (rc_game::memcard, rc_game::save): one [`Card`] for the whole
//! run, its machine and monitor run once per 60 Hz frame before the front end and the game tick, as the main loop
//! runs them.
//!
//! The card is a folder (`rc_game::memcard::save_root`: `RC_SAVE_DIR`, else `<config base>/rerac/memcard`, the card
//! R&C1's port uses); deterministic runs get none unless `RC_SAVE_DIR` names one. Without the disc's save blob or its
//! tables there is no card at all (the menus then never open).

use bevy::prelude::*;
use rc_formats::disc::boot::BootElf;
use rc_formats::disc::save_game::{card_dir_name, ChunkTables, SaveBlob, SaveFile};
use rc_formats::disc::toc::Toc;
use rc_game::memcard::{save_root, CardFs, MemCard};
use rc_game::save::SaveData;
use std::path::Path;

/// `MISC.WAD`'s part with the save blob (icon.sys, static.ico, the blank save).
const MISC_SAVE: usize = 2;

#[derive(Resource)]
pub struct Card {
    pub mc: MemCard,
    /// The save data the card reads into and writes from (the game state's persistent part).
    pub save: SaveData,
    /// The blank save (a new game's).
    pub blank: SaveData,
}

impl Card {
    pub fn load(root: &Path) -> anyhow::Result<Card> {
        let read = |rel: &str| crate::disc_source::read(root, rel);
        let elf = read("SCUS_972.68")?;
        let tables = ChunkTables::from_boot_elf(&BootElf::parse(&elf)?)?;
        let hdr = read("RC2.HDR")?;
        let t = Toc::parse(&hdr)?;
        let misc = read("G/MISC.WAD")?;
        let part = t.misc_part(MISC_SAVE)?;
        let blob = SaveBlob::parse(misc.get(part.bytes()).ok_or_else(|| anyhow::anyhow!("MISC.WAD: the save blob is past the file"))?, tables.file_size())?;
        let blank = SaveData::from_file(tables.clone(), SaveFile::parse(&blob.template, tables.global_size())?);
        let dir = card_dir_name(&read("SYSTEM.CNF")?)?;
        let base = if crate::determinism::deterministic() { None } else { crate::render_settings::config_base() };
        let fs = CardFs::new(save_root(base.as_deref()));
        Ok(Card { mc: MemCard::new(fs, dir, Some(blob)), save: blank.clone(), blank })
    }
}

/// The card's frame, before the front end's and the game's.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CardStep;

pub struct CardPlugin(pub std::path::PathBuf);

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        match Card::load(&self.0) {
            Ok(c) => {
                println!("card: {}", c.mc.fs.root.as_ref().map_or("none".to_string(), |r| r.display().to_string()));
                app.insert_resource(c).add_systems(FixedUpdate, tick.in_set(CardStep));
            }
            Err(e) => eprintln!("card: none ({e:#})"),
        }
    }
}

/// The monitor (`0x298f20`), then the machine (`0x299cf0`).
fn tick(mut card: ResMut<Card>) {
    let c = &mut *card;
    c.mc.monitor();
    c.mc.update(&mut c.save);
}
