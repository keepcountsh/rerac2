//! A Going Commando level for the shared renderers: `G/LEVELn.WAD`'s lumps (rc_formats::rac2::level) read into the
//! same [`LoadedLevel`] R&C1's levels fill (crate::level_load), so tfrags, ties, shrubs, the sky, the occlusion grid,
//! the collision and the textures draw through R&C1's renderers (docs/plan/rac2_bringup.md §6).
//!
//! What a Going Commando level has so far (§6.3): A, the world; C, the moby classes at their instances (R&C1's moby
//! reader with R&C2's vertex table header, `rc_formats::moby::parse_moby_class_for`). Left empty here, each for its
//! milestone: the tie / shrub / tfrag / moby lighting (D: neutral colours), the HUD, the level code's water tables,
//! the sound (E).

use crate::level_load::{LoadTimings, LoadedLevel};
use anyhow::{Context, Result};
use rc_formats::rac2::level::{LevelWad, Lump, Part};
use rc_formats::{level, texture, tfrag, Game};
use std::path::Path;
use std::time::Instant;

/// Level `index`'s file in the data folder.
pub fn file_name(index: u32) -> String { format!("G/LEVEL{index}.WAD") }

/// Ratchet's body (rc_formats::rac2::armor): level class 0's header and skeleton with armour `armor`'s packets, its
/// textures appended to the level's moby texture table (slot `k` of the armour = entry `moby_textures.len() + k`).
/// The wrench (class 0x47) is supplied the same way from its row ([`rc_formats::rac2::armor::WRENCH_ROW`]).
fn attach_ratchet(root: &Path, core: &level::LevelCore, mobys: &mut crate::moby_render::LevelMobys, textures: &mut Vec<texture::LevelTexture>, armor: usize) -> Result<()> {
    attach_armour_row(root, core, mobys, textures, 0, armor)
}

/// Armour row `row`'s mesh and textures into class `o_class` ([`attach_ratchet`]).
fn attach_armour_row(root: &Path, core: &level::LevelCore, mobys: &mut crate::moby_render::LevelMobys, textures: &mut Vec<texture::LevelTexture>, o_class: i32, row: usize) -> Result<()> {
    use rc_formats::rac2::{armor as ar, toc::Toc};
    let hdr = crate::disc_source::read(root, "RC2.HDR")?;
    let parts = ar::armors(&Toc::parse(&hdr)?)?;
    let part = *parts.get(row).with_context(|| format!("no armour row {row}"))?;
    let a = ar::read(&crate::disc_source::read(root, "G/ARMOR.WAD")?, part)?;
    let class = mobys.classes.iter_mut().find(|c| c.o_class == o_class).with_context(|| format!("no class {o_class} in the level"))?;
    rc_formats::moby::attach_packets(Game::Rac2, &mut class.class, &a.mesh)?;
    let base = core.moby_textures.len().max(textures.iter().filter(|t| t.table == texture::TextureTable::Moby).map(|t| t.index + 1).max().unwrap_or(0));
    for (k, t) in a.textures.into_iter().enumerate() {
        let index = base + k;
        let slot = u8::try_from(index).context("moby texture index past 255")?;
        if let Some(e) = class.entry.textures.get_mut(k) { *e = slot; }
        textures.push(texture::LevelTexture { table: texture::TextureTable::Moby, index, source: texture::TextureSource::Entry(bytemuck_zeroed()), texture: t });
    }
    Ok(())
}

/// The ship's paint (rc_formats::rac2::space; `0x1a7af8 >> 16 & 0x1f`, 0 on a new game) appended to the level's moby
/// texture table and given to every slot of the classes whose core entry has `+8 ≠ 0` (`0x304660`: the ship and its
/// parts).
fn attach_ship_paint(root: &Path, core: &level::LevelCore, mobys: &mut crate::moby_render::LevelMobys, textures: &mut Vec<texture::LevelTexture>, paint: usize) -> Result<()> {
    use rc_formats::rac2::{space, toc::Toc};
    let hdr = crate::disc_source::read(root, "RC2.HDR")?;
    let t = space::paint(&Toc::parse(&hdr)?, &crate::disc_source::read(root, "G/SPACE.WAD")?, paint)?;
    let index = core.moby_textures.len().max(textures.iter().filter(|t| t.table == texture::TextureTable::Moby).map(|t| t.index + 1).max().unwrap_or(0));
    let slot = u8::try_from(index).context("moby texture index past 255")?;
    for class in mobys.classes.iter_mut().filter(|c| c.entry.unknown_8 != 0) { class.entry.textures = [slot; 16]; }
    textures.push(texture::LevelTexture { table: texture::TextureTable::Moby, index, source: texture::TextureSource::Entry(bytemuck_zeroed()), texture: t });
    Ok(())
}

/// An empty texture-table entry (the armour's textures have no entry in the level's table).
fn bytemuck_zeroed() -> rc_formats::level::TextureEntry { rc_formats::level::TextureEntry { data_offset: 0, width: 0, height: 0, ty: 0, palette: 0, mipmap: 0, pad: 0 } }

/// What the moby system reads of the level's class blobs, re-read from the level file (the loaded level keeps no
/// class blobs): the collision blobs (class header +0x10; the same blob and kernels as R&C1's, rc_game2::moby) and
/// the joint lists of the classes that carry attachments (class header +0x1c; Ratchet's, for his attachment points).
pub struct ClassBlobs {
    pub collision: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    pub blobs: std::collections::HashMap<i32, Vec<u8>>,
}

pub fn class_blobs(root: &Path, index: u32) -> Result<ClassBlobs> {
    let name = file_name(index);
    let file = crate::disc_source::read(root, &name)?;
    let wad = LevelWad::parse(&file).with_context(|| name.clone())?;
    let core_index = wad.lump(Lump::CoreIndex)?.context("no core index")?;
    let core_data = rc_formats::wad::decompress(wad.lump(Lump::CoreData)?.context("no core data")?)?;
    let core = level::parse_level_core_for(Game::Rac2, core_index, core_data.len())?;
    let collision = rc_formats::moby_collision::parse_level(&core, &core_data)?;
    let blobs = core
        .moby_classes
        .iter()
        .filter_map(|e| core.block(&core_data, &format!("moby_class/{:04}", e.o_class)).map(|b| (e.o_class, b.to_vec())))
        .collect();
    Ok(ClassBlobs { collision, blobs })
}

/// The level's HUD textures (`0x2ff200`): the header is lump 3, its five banks lumps 4..8 (WAD-compressed; the
/// absent ones 16-byte stubs); R&C1's HUD format (rc_formats::hud).
pub fn hud(root: &Path, index: u32) -> Result<rc_formats::hud::Hud> {
    let name = file_name(index);
    let file = crate::disc_source::read(root, &name)?;
    let wad = LevelWad::parse(&file).with_context(|| name.clone())?;
    let header = wad.lump(Lump::Lump3)?.context("no HUD header")?;
    let bank = |l: Lump| -> Vec<u8> {
        wad.lump(l).ok().flatten().filter(|b| 16 < b.len()).map(|b| if rc_formats::wad::is_wad(b) { rc_formats::wad::decompress(b).unwrap_or_default() } else { b.to_vec() }).unwrap_or_default()
    };
    let banks = [bank(Lump::HudHeader), bank(Lump::HudBank0), bank(Lump::HudBank1), bank(Lump::HudBank2), bank(Lump::HudBank3)];
    Ok(rc_formats::hud::parse_hud(header, [&banks[0], &banks[1], &banks[2], &banks[3], &banks[4]])?)
}

pub fn load(root: &Path, index: u32) -> Result<LoadedLevel> {
    let mut t = LoadTimings::default();
    let t0 = Instant::now();
    let name = file_name(index);
    let file = crate::disc_source::read(root, &name)?;
    let wad = LevelWad::parse(&file).with_context(|| name.clone())?;
    let lump = |l: Lump| -> Result<&[u8]> { wad.lump(l)?.with_context(|| format!("{name}: no {l:?} lump")) };
    let core_index = lump(Lump::CoreIndex)?;
    let gs_ram = lump(Lump::GsRam)?;
    t.read = t0.elapsed();

    let t0 = Instant::now();
    let core_data = rc_formats::wad::decompress(lump(Lump::CoreData)?).context("decompressing the core data")?;
    let gameplay = rc_formats::wad::decompress(wad.part_bytes(Part::Gameplay)?.context("no gameplay part")?).context("decompressing the gameplay file")?;
    t.decompress = t0.elapsed();

    let t0 = Instant::now();
    let core = level::parse_level_core_for(Game::Rac2, core_index, core_data.len()).context("parsing the core index")?;
    t.parse_core = t0.elapsed();

    let t0 = Instant::now();
    // The stored colours, unlit (milestone D).
    let tfrags = tfrag::parse_level_tfrags(&core, &core_data).context("parsing tfrags")?;
    t.parse_tfrags = t0.elapsed();
    let t0 = Instant::now();
    let textures = texture::parse_textures(&core, &core_data, gs_ram).context("decoding textures")?;
    t.parse_textures = t0.elapsed();
    let tfrag_lod = crate::tfrag_lod::load(&core, &core_data, gs_ram)?;

    // Level settings: the background colour (+0x00) and the fog (+0x0c..+0x20) are where R&C1 keeps them (Aranos:
    // background (0x96, 0xbe, 0xd2), fog colour (0x66, 0x66, 0x81), near 61440, far 358400).
    let fog = crate::game_camera::LevelFog::parse(&gameplay).context("parsing level settings")?;
    let background = crate::game_camera::level_background(&gameplay).context("parsing level settings")?;
    let collision = rc_formats::collision::parse_collision(&core, &core_data)
        .map_err(|e| eprintln!("rac2 level {index}: collision not parsed ({e})"))
        .ok();
    let ties = crate::tie_render::load_ties(&core, &core_data, gs_ram, &gameplay, &textures).context("loading ties")?;
    let shrubs = crate::shrub_render::load_shrubs(&core, &core_data, gs_ram, &gameplay, &textures).context("loading shrubs")?;
    let sky = crate::sky_render::load(&core, &core_data, index).context("loading the sky")?;
    let mut textures = textures;
    let mut mobys = crate::moby_render::load_mobys_with_gs(root, &core, &core_data, &gameplay, gs_ram, collision.as_ref()).context("loading mobys")?;
    if let Err(e) = attach_ratchet(root, &core, &mut mobys, &mut textures, 0) { eprintln!("rac2: Ratchet's armour not attached ({e:#}); he has no mesh"); }
    if let Err(e) = attach_armour_row(root, &core, &mut mobys, &mut textures, 0x47, rc_formats::rac2::armor::WRENCH_ROW) { eprintln!("rac2: the wrench's mesh not attached ({e:#})"); }
    if let Err(e) = attach_ship_paint(root, &core, &mut mobys, &mut textures, 0) { eprintln!("rac2: the ship's paint not attached ({e:#})"); }
    let occlusion = crate::occlusion::load(&core, &core_data, &gameplay, &tfrags, &ties.instances, &mobys.instances).context("loading occlusion")?;
    let particles = crate::particle_render::LevelParticles {
        textures: rc_formats::particle_tex::parse_particle_textures(&core, core_index, &core_data).map_err(|e| eprintln!("rac2: no particle textures ({e})")).ok(),
        owners: Vec::new(),
        level: index,
        grid: rc_formats::level::HeightGrid::parse(&core.header, &core_data),
    };
    let death_z = rc_formats::rac2::gameplay::level_settings(&gameplay)?
        .and_then(|s| s.get(0x28..0x2c))
        .map_or(f32::NEG_INFINITY, |b| f32::from_le_bytes(b.try_into().unwrap()));
    let moby_class_order = core.moby_classes.iter().map(|e| e.o_class).collect();
    println!(
        "rac2: level {index} ({name}): {} tfrags, {} textures, {} tie classes / {} instances, {} shrub classes / {} instances, {} moby classes / {} instances, sky {}, collision {}",
        tfrags.len(), textures.len(), ties.classes.len(), ties.instances.len(), shrubs.classes.len(), shrubs.instances.len(),
        mobys.classes.len(), mobys.instances.len(),
        sky.is_some(), collision.is_some()
    );
    Ok(LoadedLevel {
        tfrags, textures, fog, background, mobys, ties, shrubs, sky, occlusion, tfrag_lod, particles, hud: None,
        water: Default::default(), sea: Default::default(), audio: audio(&wad, core_index, &core, &core_data, collision.clone()), collision, death_z, gameplay, moby_class_order, timings: t,
    })
}

/// The level's sound data for crate::audio_out (rc_game::audio, the 989snd bank R&C1's reader parses as it is): the
/// level's core sound bank (the file's part 1) and the sound definitions (level and class, `0x304660`'s remap).
/// Not yet: the sound instances, the env sample points and the music (`AUDIOn.WAD`). None with `RC_AUDIO=0` or when
/// the bank does not read (with a warning).
fn audio(wad: &LevelWad, core_index: &[u8], core: &level::LevelCore, core_data: &[u8], collision: Option<rc_formats::collision::Collision>) -> Option<rc_game::audio::LevelAudio> {
    if !crate::audio_out::enabled() { return None; }
    let run = || -> Result<rc_game::audio::LevelAudio> {
        let part = wad.part_bytes(Part::CoreBank)?.context("no core sound bank part")?;
        let bank = if rc_formats::wad::is_wad(part) { rc_formats::wad::decompress(part)? } else { part.to_vec() };
        let bank = std::sync::Arc::new(rc_formats::sound_bank::parse_bank(&bank)?);
        let sounds = rc_formats::sound_bank::parse_level_sounds_for(Game::Rac2, core_index, core, core_data)?;
        Ok(rc_game::audio::LevelAudio {
            bank, sounds, instances: Vec::new(), instance_pvars: Vec::new(), music: Default::default(), env_points: Vec::new(), collision: collision.map(std::sync::Arc::new),
        })
    };
    match run() {
        Ok(a) => {
            println!("audio: {} sounds, {} samples, {} level defs, {} classes with sounds", a.bank.sounds.len(), a.bank.vags.len(), a.sounds.level_defs.len(), a.sounds.classes.iter().filter(|c| !c.defs.is_empty()).count());
            Some(a)
        }
        Err(e) => {
            eprintln!("audio: the level's sound data not read ({e:#}); no audio");
            None
        }
    }
}
