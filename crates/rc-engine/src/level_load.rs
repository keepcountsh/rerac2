//! The loaded level: what the renderers and the game read from one level of the game data folder (`G/LEVELn.WAD`,
//! read by crate::rac2_level).

use anyhow::Result;
use rc_formats::{texture, tfrag};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Aranos (level 0), where a new game starts.
pub const DEFAULT_LEVEL: u32 = 0;

pub struct LoadedLevel {
    pub tfrags: Vec<tfrag::Tfrag>,
    pub textures: Vec<texture::LevelTexture>,
    /// Level-settings fog (gameplay file, NTSC variant), docs/plan/game_camera_fog.md.
    pub fog: crate::game_camera::LevelFog,
    /// Level-settings background colour = the GS frame clear colour (`game_camera::level_background`).
    pub background: [u8; 3],
    /// Moby classes, instances and their lit vertex colours (crate::moby_render).
    pub mobys: crate::moby_render::LevelMobys,
    /// Tie classes, instances and their LightTies colour tables (crate::tie_render).
    pub ties: crate::tie_render::LevelTies,
    /// Shrub classes, instances and their LightShrubs palettes (crate::shrub_render).
    pub shrubs: crate::shrub_render::LevelShrubs,
    /// The sky block and its textures (crate::sky_render).
    pub sky: Option<crate::sky_render::LevelSky>,
    /// Occlusion grid and per-object occlusion words (crate::occlusion).
    pub occlusion: crate::occlusion::LevelOcclusionData,
    /// Tfrag LOD distances and mip chains (crate::tfrag_lod).
    pub tfrag_lod: crate::tfrag_lod::TfragLodData,
    /// Particle textures and the class-27 emitters (crate::particle_render).
    pub particles: crate::particle_render::LevelParticles,
    /// The level collision mesh (the hero and the follow camera query it, crate::gameplay); None when it
    /// does not parse (warned; the game tick is then off).
    pub collision: Option<rc_formats::collision::Collision>,
    /// Level settings +0x28: the death height the game copies to `0x15f638` (player_controller.md §10).
    pub death_z: f32,
    /// Sound bank, defs, sound instances and music (crate::audio_out); None with `RC_AUDIO=0`.
    pub audio: Option<rc_game::audio::LevelAudio>,
    /// The decompressed gameplay file (NTSC), for the moby scheduler's loader (pvars, groups, splines, the
    /// spawnable count; crate::gameplay).
    pub gameplay: Vec<u8>,
    /// The level core's moby class list in order (o_class per entry): the class slot of `MobyClassesRelocate`
    /// (0x2549d0) is the index here (crate::gameplay).

    pub timings: LoadTimings,
}

#[derive(Debug, Default)]
pub struct LoadTimings {
    pub read: Duration,
    pub decompress: Duration,
    pub parse_core: Duration,
    pub parse_tfrags: Duration,
    pub parse_textures: Duration,
}

impl LoadTimings {
    pub fn total(&self) -> Duration { self.read + self.decompress + self.parse_core + self.parse_tfrags + self.parse_textures }
}

/// The game data folder (`--data-dir`, `RC_DATA_DIR`, else the development `extracted/`; crate::disc_source).
pub fn extracted_root() -> PathBuf { crate::disc_source::data_root() }

/// `RC_LEVEL` (decimal level index), else Novalis: the level the boot loads.
pub fn boot_level_index() -> u32 {
    match std::env::var("RC_LEVEL") {
        Ok(s) => s.trim().parse().unwrap_or_else(|_| panic!("RC_LEVEL must be a level index, got {s:?}")),
        Err(_) => DEFAULT_LEVEL,
    }
}

/// The loaded level (0x15ed84 as the engine's loader has it): the boot's [`boot_level_index`] until a runtime level
/// change ([`set_level_index`], crate::level_switch).
pub fn level_index() -> u32 {
    let v = CURRENT_LEVEL.load(std::sync::atomic::Ordering::Relaxed);
    if v == u32::MAX { boot_level_index() } else { v }
}

/// The level loaded by a runtime level change (crate::level_switch).
pub fn set_level_index(index: u32) { CURRENT_LEVEL.store(index, std::sync::atomic::Ordering::Relaxed); }

static CURRENT_LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

pub fn load_level(root: &Path, index: u32) -> Result<LoadedLevel> { crate::rac2_level::load(root, index) }
