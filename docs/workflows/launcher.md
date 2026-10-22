# Workflow: the launcher (dev version)

The launcher is a separate repo, [re-rac/rerac-launcher](https://github.com/re-rac/rerac-launcher) (your local
checkout of it, wherever it lives; Tauri; its README covers building and running it). The
contract between the two repos is `docs/plan/launcher_contract.md`; the mod system is `docs/plan/mods.md`.

## Run a dev build of the game from the launcher

| Task | Command / action |
|---|---|
| Make a version folder and zip of this repo's game | `tools/package/package.sh` (docs/workflows/release.md) |
| Install it in the launcher | Launcher → Settings → Version Management → Development → **Install from zip…** → `dist/rerac2-<version>-<os>-<arch>.zip`, then **Set active** |

Instead of the zip, **Add build folder…** with the unpacked `dist/rerac2-<version>-<os>-<arch>/` folder uses it in
place (re-run `tools/package/package.sh --no-build` after a `cargo build --release` to refresh it).

The launcher then extracts the game data with the version's own `rerac2-extract` into its data folder
(`<data root>/games/rac2/data/`) and starts `rerac2 --data-dir <that folder>`. That folder is separate from the
repo's development `extracted/` (docs/workflows/game-data.md).

**Not yet:** the launcher installs only R&C1 builds today (`INSTALLABLE` in its `commands.rs`; Going Commando is
listed as not supported) and reads official releases from `re-rac/rerac` only. Taking this game's builds and releases
is launcher work.
