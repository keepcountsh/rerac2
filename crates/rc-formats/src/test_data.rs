//! Where tests and dev tools find the development data folder: `RC_EXTRACTED`, else `<workspace>/extracted` (the
//! disc's files as `rerac2-extract extract` copies them; git-ignored, never shipped). Tests skip when a file they
//! need is absent. The game resolves its own data folder (rc-engine `disc_source`: `--data-dir`, `RC_DATA_DIR`, then
//! this one). Nothing here writes to it.

use std::path::PathBuf;

/// The development data root: `RC_EXTRACTED` (when set and non-empty), else `<workspace>/extracted`.
pub fn root() -> PathBuf {
    std::env::var_os("RC_EXTRACTED")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted"))
}
