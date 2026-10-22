//! Readers for the on-disc formats of Ratchet & Clank: Going Commando (2003, PS2).
//!
//! Most formats are R&C1's with Going Commando's changes: the shared readers follow the specifications in
//! `docs/formats/` (written for R&C1, first verified byte for byte against a C++ reference extractor, git 2230812),
//! and [`disc`] holds what is Going Commando's own (the table of contents, the level WADs, the front end).

pub mod buf;
pub mod wad;
pub mod toc;
pub mod iso9660;
pub mod level;
pub mod texture;
pub mod vif;
pub mod tfrag;
pub mod moby;
pub mod tie;
pub mod shrub;
pub mod shrub_light;
pub mod tfrag_light;
pub mod tie_light;
pub mod gameplay;
pub mod moby_light;
pub mod moby_anim;
pub mod moby_collision;
pub mod moby_spawn;
pub mod sky;
pub mod collision;
pub mod occlusion;
pub mod particle_tex;
pub mod hud;
pub mod pif;
pub mod font;
pub mod level_overlay;
pub mod strings;
pub mod sound_bank;
pub mod vag;
pub mod scene;
pub mod volumes;
pub mod cameras;
pub mod pss;
pub mod disc;
pub mod sha1;
pub mod test_data;

pub use buf::{Buf, FormatError};
