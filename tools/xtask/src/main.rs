//! `cargo xtask <command>`: one short command per dev chore (alias in `.cargo/config.toml`). Dev only, never ships;
//! std only. Doc: `tools/xtask/README.md`.

mod classify;
mod regen;
mod sweep;
mod test;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

const HELP: &str = "\
cargo xtask <command> [options]    dev chores, one command each (tools/xtask/README.md)

commands:
  regen-data [--iso <image>] [--data-dir <dir>] [--force] [--check]
      Rebuild the dev game data folder (extracted/, or RC_EXTRACTED) from your disc image with the dev build of
      rerac2-extract: extract into a staging folder, verify it, then swap it in and delete the old folder.
      Known leftovers (an interrupted extract, .DS_Store) are listed and removed; any unknown file stops it unless
      --force.
        --iso <image>     the disc image (default: RC_ISO)
        --data-dir <dir>  the folder to rebuild (default: RC_EXTRACTED, else <repo>/extracted)
        --force           also delete unknown files
        --check           only list what the folder holds; change nothing (no disc needed)
  package [--no-build]
      Release packaging: runs tools/package/package.sh (dist/rerac2-<version>-<os>-<arch>/ and its .zip).
  sweep [--limit <size>] [--dry-run]
      Keep target/ under a size limit (default 30GB; sizes like 30GB, 20G, 500MB, 2GiB) with cargo-sweep: the
      least recently used build units go first (never incremental/; `cargo clean` is the full reset). Refuses while
      another cargo, rustc or cargo-nextest runs. Prints target/ before and after, and what went. Without cargo-sweep
      it prints `cargo install cargo-sweep --locked` and exits 0. test-full runs it at the end (a failure only warns).
        --dry-run         only show what it would remove

test tiers (docs/workflows/testing.md §1-2): the only way to run tests; never `cargo test` / `cargo nextest` by hand.
RC_AUDIO=0; cargo-nextest when installed, else `cargo test --workspace --features rc-engine/dev`.
  test-quick [crate]
      Unit tests only: every crate's lib and bin tests (`--lib --bins`, about 5 s). With nextest a crate name runs
      just that crate's (without nextest every crate runs: a single one would rebuild).
  test-job <area>... [--test <binary>]...
      The per-job tier: the unit tests plus the integration binaries of each area. Areas (aliases):
        game (hero, classes, mobys, camera, audio, particles: unit tests only), formats, engine (render, input:
        unit tests only), repo (guards, layout).
      --test <binary>     also run this integration binary whole (repeatable); with no area, only the binaries
                          (no unit tests). Binaries: formats guards.
  test-full
      The full suite (on merge to main, or when the coordinator asks), then `sweep` (a failure only warns).
    Targeting options of test-quick and test-job (the same meaning under both runners):
      --filter <name>     only tests whose path contains <name> (repeatable: any), e.g. disc:: or a fn name
      --exact             the filters are full paths, <module>::<test_fn>
      --ignored           only the ignored tests (surveys, printers)
      --nocapture         show the tests' output
    Options of the test commands:
      --cargo-test        use `cargo test --workspace --features rc-engine/dev` even when nextest is installed
      -- <args>           passed on as is: to nextest, or to the test binaries under cargo test
  help
      This list.";

/// The repository root (this crate lives at `tools/xtask`).
pub fn repo_root() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    p.canonicalize().unwrap_or(p)
}

/// The `cargo` that ran us (`CARGO` is set by `cargo run`), else `cargo` from `PATH`.
pub fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let (cmd, rest) = match argv.split_first() {
        Some((c, r)) => (c.to_string_lossy().into_owned(), r),
        None => ("help".to_string(), &argv[..]),
    };
    match cmd.as_str() {
        "regen-data" => regen::run(rest),
        "package" => package(rest),
        "test-quick" => test::quick(rest),
        "test-job" => test::job(rest),
        "test-full" => test::full(rest),
        "sweep" => sweep::main(rest),
        "help" | "--help" | "-h" => { println!("{HELP}"); ExitCode::SUCCESS }
        other => {
            eprintln!("error: unknown command {other:?}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

/// `cargo xtask package [args]`: `tools/package/package.sh [args]`, from the repo root.
fn package(args: &[OsString]) -> ExitCode {
    let root = repo_root();
    let status = Command::new("bash").arg(root.join("tools/package/package.sh")).args(args).current_dir(&root).status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(1)),
        Err(e) => { eprintln!("error: cannot run bash tools/package/package.sh: {e}"); ExitCode::FAILURE }
    }
}
