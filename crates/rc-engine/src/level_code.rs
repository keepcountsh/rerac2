//! The loaded level's copy of Aranos's overlay code and data. The weapon classes' code (the Gravity Bomb's hand
//! 0xce3, bomb 0xce4 and blast 0xce5 among them) is compiled into every level's overlay, so its tables sit at
//! other addresses on every level; the port names them by their Aranos (level 0) addresses and asks
//! [`LevelCode`] for the loaded level's (rc_formats::level_overlay: the level's copy of the function that forms
//! the address with `lui`/`addiu`, the same instruction pair read there).

use anyhow::{Context, Result};
use rc_formats::level_overlay::{LevelOverlay, Relocation};
use std::path::Path;

/// Aranos's overlay (the reference) and the loaded level's.
pub struct LevelCode {
    reference: LevelOverlay,
    target: LevelOverlay,
}

impl LevelCode {
    /// Level `index`'s overlay with level 0's as the reference.
    pub fn load(root: &Path, index: u32) -> Result<LevelCode> {
        let target = LevelOverlay::parse(&crate::level_wad::overlay(root, index)?).with_context(|| format!("level {index}'s overlay"))?;
        let reference = if index == 0 { target.clone() } else { LevelOverlay::parse(&crate::level_wad::overlay(root, 0)?).context("level 0's overlay")? };
        Ok(LevelCode { reference, target })
    }

    /// `f` with a reader of Aranos addresses: `(address, length)` gives the bytes at the loaded level's own copy
    /// of that address (None: no copy found, or past the overlay).
    pub fn with_reader<R>(&self, f: impl FnOnce(&dyn Fn(u32, usize) -> Option<Vec<u8>>) -> R) -> R {
        let r = Relocation::new(&self.reference, &self.target);
        let read = |addr: u32, n: usize| -> Option<Vec<u8>> { self.target.read(r.data(addr)?, n).map(<[u8]>::to_vec) };
        f(&read)
    }
}
