# Workflow: testing (tiers, areas)

What runs when and which tests belong to which area. All commands run from the repo root with
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`. Tests that need game data skip themselves when `extracted/` is
absent.

**No test relies on the user's personal files** (§5): not the disc image, savestates, PCSX2 traces, screen
recordings, the settings file or session output in `work/`, not even "skip if missing". Tests read `extracted/`
through the data root and committed fixtures; findings from personal material reach them only as a few distilled
numbers. `guards::no_test_or_product_file_references_personal_paths` enforces it.

R&C1's test suite (its integration binaries per area, the hero digest, the all-levels smoke, the loader golden
tests) stayed in R&C1's repository (docs/plan/porting_from_rac1.md).

## 1. Commands that share the dev build

**Tests run only through `cargo xtask test-*`.** Never run `cargo test` or `cargo nextest` directly: there is no
Cargo test alias, and the xtask commands pass the one flag set that shares the dev Bevy build
(`--workspace --features rc-engine/dev`). Compile checks use the `.cargo/config.toml` aliases:

| What | Command |
|---|---|
| All unit tests (every crate's `src/` tests; rc-engine's live in its bin) | `cargo xtask test-quick` |
| One crate's unit tests | `cargo xtask test-quick <crate>` (e.g. `rc-formats`) |
| A job's tier (§2) | `cargo xtask test-job <area…>` |
| One or more integration binaries alone (the binaries of §3) | `cargo xtask test-job --test <binary> [--test <binary> …]` |
| One module of a binary | `cargo xtask test-job --test <binary> --filter <module>::` |
| One test | `cargo xtask test-job --test <binary> --filter <module>::<test_fn> --exact`, or a substring |
| Ignored surveys, with their output | add `--ignored --nocapture` |
| Type check of everything, with tests | `cargo check-all` |
| Lints | `cargo clippy-all` |
| Full suite | `cargo xtask test-full` |

The `cargo xtask test-*` commands set `RC_AUDIO=0` (other variables pass through) and run through cargo-nextest when
it is installed, else through `cargo test --workspace --features rc-engine/dev` (§4). `--filter`, `--exact`,
`--ignored` and `--nocapture` mean the same under both runners. A test's path is `<module>::<test_fn>`, e.g.
`disc::front_end_and_main_menu` in the binary `formats`.

Why no by-hand runs: `cargo test -p <crate> --lib` resolves other features and recompiles the crates from scratch next
to the shared copy, and plain `cargo test --workspace` rebuilds Bevy statically.

## 2. Tiers

- **Per job:** the unit tests (`cargo xtask test-quick`, every crate: seconds, and the only cheap way to cover a
  dependency change), the integration binaries of the areas the job touched (`cargo xtask test-job <area…>` runs
  both), then `cargo check-all` once at the end and `cargo clippy-all` for the job's files. The report lists every
  command run and its result.
- **Full suite:** `cargo xtask test-full`, on merge to main or for a big shared-code commit; it ends with
  `cargo xtask sweep` (§4).

Gameplay is checked by running the game (the user's tests, and the headless screenshot runs of
docs/workflows/dev-switches.md), not by the suite.

## 3. Area map

The unit tests of every area run in every `test-job`. The table is `AREAS` in `tools/xtask/src/test.rs` (area names
and aliases are the same).

| Area (aliases) | Integration binary | Unit tests |
|---|---|---|
| game (hero, classes, mobys, camera, audio, particles) | none | rc-game lib |
| formats | `formats` (rc-formats: `disc`, the disc's own files against the values read in Ghidra) | rc-formats lib |
| engine (render, input) | none | rc-engine bin |
| repo (guards, layout) | `guards` (tools/repo-checks) | — |

## 4. cargo-nextest, the xtask commands, target/

**Install** (once): `cargo install cargo-nextest --locked`. It lands in `~/.cargo/bin` (`CARGO_HOME`); `cargo nextest …`
works without it on `PATH` because cargo looks there for subcommands.

**Settings:** `.config/nextest.toml`, profile `default`: `fail-fast = false`; a test is flagged SLOW after 30 s;
`test-threads = 8`, the build's `jobs` cap; the summary lists the slow and failed tests again.

**Doctests:** none, so the nextest runs miss nothing. When one appears, add a
`cargo test --workspace --features rc-engine/dev --doc` step to `test-full` (`full_steps` in `tools/xtask/src/test.rs`).

| Command | Runs |
|---|---|
| `cargo xtask test-quick [crate]` | the unit tests (`--lib --bins`); with nextest a crate name narrows it (`-E package(<crate>)`) |
| `cargo xtask test-job <area…>` | the unit tests and the areas' binaries in one nextest run |
| `cargo xtask test-job --test <binary>…` | those integration binaries alone; next to areas, added to them |
| `cargo xtask test-full` | the whole suite with `--no-fail-fast`, then `cargo xtask sweep` (only warns) |

**One Bevy build.** The `cargo xtask test-*` commands pass `--workspace --features rc-engine/dev`, which enables
Bevy's `dynamic_linking` like `cargo dev`, so the tests reuse the one Bevy build `cargo dev` made. `cargo check-all`
and `cargo clippy-all` add `--all-targets` with the same features. Release builds and `tools/package` stay static
(never `--features dev`).

**target/.** `cargo xtask sweep` keeps it under a limit (30 GB by default; `--limit 20G`, `--dry-run`) with
cargo-sweep (`cargo install cargo-sweep --locked`): while `target/` holds more than the limit, the least recently used
build units go, oldest first; `incremental/` and unhashed outputs are never touched, so `cargo dev` does not rebuild
Bevy. It refuses while another `cargo`, `rustc` or `cargo-nextest` runs. `cargo clean` is the full reset.

## 5. Personal files: none in tests

**The rule.** No test relies on the user's personal files, not even optionally or "skip if missing". Personal means:
anything in the personal folder (the disc image, savestates, PCSX2 traces), screen recordings, the user's settings
file, anything in `work/` that came from the user's own sessions, and any other file that exists only on the user's
machine. Tests may read `extracted/` through the data root (`rc_formats::test_data::root`, `RC_EXTRACTED`) and
committed fixtures. Findings from personal material reach tests only as small distilled values; provenance comments
name the source in words, never with a path.

**The guard.** `tools/repo-checks` (`guards::no_test_or_product_file_references_personal_paths`) scans every file under
`crates/` and every tool's `tests/` folder for the personal patterns (`.p2s`, `~/PS2`, `PS2/ratchet2`, `work/`,
`/traces/`, `RC_PERSONAL`), and test files additionally for the disc-image and home-folder hooks (`RC_ISO`, `HOME`,
`home_dir`). It has no allow-list. Tool sources (`tools/*/src`) are not scanned: tools read personal material on
purpose (`rc-trace` savestates, `xtask regen-data` the disc image).
