# Workflow: game data (identify, extract, verify)

Tool: `rerac2-extract` (product crate `crates/rc-extract`; the launcher runs the same binary). Contract:
`docs/plan/launcher_contract.md`. All commands run from the repo root with
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`; `RC_ISO` can stand in for `--iso`.

## The commands

| Task | Command |
|---|---|
| Identify a disc image | `cargo run --release -p rc-extract -- identify --iso "<disc>.iso"` |
| Extract (copy the disc's files into a data folder) | `cargo run --release -p rc-extract -- extract --iso "<disc>.iso" --out <data folder>` |
| Verify a data folder against the built-in size/SHA-1 table | `cargo run --release -p rc-extract -- verify --out <data folder>` |
| Regenerate the size/SHA-1 table (developer) | `cargo run --release -p rc-extract -- table --iso "<disc>.iso" --output crates/rc-extract/data/scus_972_68.tsv` |

The supported disc is the NTSC-U `SCUS_972.68` v2.00. The disc keeps its assets as ISO files (`RC2.HDR`,
`G/*.WAD`, the boot ELF), and the extractor copies them as they are: 96 files, about 3.8 GB.

## The development data folder: `extracted/`

Tests, `rc-trace` and `cargo dev` read `<repo>/extracted` (`RC_EXTRACTED` overrides it). **The one command to
(re)build it:**

```
cargo xtask regen-data
```

It needs the disc image: set `RC_ISO` once (fish: `set -Ux RC_ISO "<disc>.iso"`; zsh / bash: an
`export RC_ISO="<disc>.iso"` line in `~/.zshrc` / `~/.bashrc`; without it the command prints these lines with your
disc's path), or pass `--iso <disc>.iso`. It lists what `extracted/` holds, builds the dev `rerac2-extract`, extracts
into `work/data-staging`, verifies it, and only then swaps it in and deletes the old folder, so a failure never
leaves you without data. `cargo xtask regen-data --check` only lists what the folder holds. An interrupted extract's
`*.partial` files and Finder's `.DS_Store` are deleted with the old folder; any other unknown file stops it (move it to
`work/` or `~/PS2/ratchet2/`, or pass `--force` to delete it). Details and exit codes: `tools/xtask/README.md`.

**What may live in `extracted/`:** only what `rerac2-extract` writes (the disc's files and `extract-info.json`).
**What may not:** trace dumps, savestates, reports, captures, Ghidra material, anything a dev tool writes. Those go to
`work/` (generated) or `~/PS2/ratchet2/` (personal).

## How the engine finds its data

The engine never reads the disc image; it reads only a data folder that `rerac2-extract` wrote.

```
cargo dev -- --data-dir <data folder>     # or RC_DATA_DIR=<data folder> cargo dev
cargo dev -- --version-json               # {"name":"rerac2","version":"0.1.0","game":"rac2","data_format":1}
```

The data folder is chosen in this order:

1. `--data-dir <folder>` (also `--data-dir=<folder>`), how the launcher starts the game;
2. `RC_DATA_DIR`;
3. the development default: `RC_EXTRACTED`, else `<repo>/extracted`.

The folder must exist and hold `RC2.HDR`, and for 1 and 2 also the extractor's `extract-info.json` with a matching
`data_format` (the development tree may lack it: one warning). Otherwise the engine prints one `error:` line saying
what to do (re-extract via the launcher) and exits before opening a window: code 3 (folder missing, not a data folder,
or extraction incomplete), code 4 (data format mismatch or unreadable `extract-info.json`), code 2 (`--data-dir`
without a value). `--version-json` prints the contract line and exits without touching any data.

## What tests read

Tests and `rc-trace` read the development `extracted/` tree (`rc_formats::test_data::root()`, `RC_EXTRACTED`
overrides it) and skip when a file they need is absent.

## Players

Players never run these by hand: the launcher (docs/workflows/launcher.md) runs `extract` into its own data folder
and starts the game with `--data-dir`.
