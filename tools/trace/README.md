# tools/trace: PCSX2 memory reads and the class census (`rc-trace`)

Dev only, never ships. Package name `rc-trace` (workspace member at `tools/trace`), so every command is
`cargo run -p rc-trace -- <command>`; `--help` lists them all. Workflow: `docs/workflows/pcsx2.md`. R&C1's comparison
and replay commands (lighting, spawn state, hero recordings, the overlay diff) stayed in R&C1's repository
(docs/plan/porting_from_rac1.md).

## Purpose

Reads the PS2's EE memory (a PCSX2 savestate, a raw dump, or the live game over PINE) to check the port against the
original, and runs the static census of the unported moby classes.

## Commands

| Task | Command |
|---|---|
| Census of the unported moby classes and the shared systems they call (no PCSX2; reads `extracted/` and `work/decomp/`, tags `tools/ghidra/names/census_systems.tsv`; docs/plan/class_census.md) | `cargo run --release -p rc-trace -- class-census` → `work/census/` |
| List a savestate's entries | `cargo run -p rc-trace -- info --state NAME` |
| Dump EE RAM | `cargo run -p rc-trace -- dump-ee --state NAME` |
| Find bytes in EE RAM | `cargo run -p rc-trace -- find --state NAME --bytes HEX` |
| Hex-dump EE RAM | `cargo run -p rc-trace -- read --state NAME --addr HEX --len N` |
| PCSX2 over PINE: status, save a state, poke bytes | `pine-info`, `pine-savestate <slot>`, `pine-write --addr HEX --bytes HEX` |

## Inputs and outputs

* **Game data:** `extracted/` (`RC_EXTRACTED` or `--extracted` overrides). Read only: the tool refuses to write there.
* **Personal material:** `~/PS2/ratchet2/` (`RC_PERSONAL` overrides): kept savestates in `savestates/` (`--state NAME`
  resolves `savestates/NAME.p2s`). `--state latest` reads PCSX2's own folder
  `~/Library/Application Support/PCSX2/sstates` (the newest `SCUS-97268*.p2s`).
* **Generated output:** `work/` (`RC_WORK` overrides it): EE dumps in `work/trace/`, the census in `work/census/`.

## Tests

`cargo xtask test-quick rc-trace` (unit tests only). No test reads a savestate, dump or disc image (no test relies on
personal files; `tools/repo-checks` scans `tests/`).
