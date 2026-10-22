# Workflow: PCSX2 (savestates, PINE, memory reads)

Tool: `tools/trace` (package `rc-trace`; README there). All commands run from the repo root with
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`. R&C1's compare, record and replay commands stayed in R&C1's
repository (docs/plan/porting_from_rac1.md); its `docs/plan/trace_harness.md` is the design they follow.

## Where things go

| What | Where |
|---|---|
| Kept savestates (`.p2s`) and their screenshots | `~/PS2/ratchet2/savestates/` (personal; `RC_PERSONAL` overrides `~/PS2/ratchet2`) |
| EE dumps and reports | `work/trace/` (generated; `RC_WORK` overrides `work/`) |
| Probe scripts of a session | `work/scratch/` (git-ignored) |
| Test inputs | only distilled, numbers-only values or fixtures |

`rc-trace` never writes into `extracted/`.

## One-time setup

| Task | Command / action |
|---|---|
| Enable PINE (live reads) | PCSX2 → Tools → Show Advanced Settings; then Settings → Advanced → PINE: tick **Enable**, slot **28011** (the default) |
| Check PINE sees the game | `cargo run -q -p rc-trace -- pine-info` |

## Savestates and memory

| Task | Command / action |
|---|---|
| Take a savestate | In PCSX2 press **F1** (Fn+F1) or System → Save State (lands in PCSX2's `sstates` folder), or `cargo run -p rc-trace -- pine-savestate <slot>` |
| Dump a state's EE RAM | `cargo run -p rc-trace -- dump-ee --state NAME` (writes `work/trace/NAME_ee.bin`) |
| Find bytes / hex-dump | `cargo run -p rc-trace -- find --state NAME --bytes HEX`, `read --state NAME --addr HEX --len N` |
| Poke bytes into the running game | `cargo run -p rc-trace -- pine-write --addr HEX --bytes HEX` |

Anywhere a command takes `--state`, `NAME` means the kept `~/PS2/ratchet2/savestates/NAME.p2s`, `latest` the newest
state in PCSX2's folder; `--ee FILE` reads a dump and `--pine` the running game. The level overlay's tables move
between levels: read addresses of the level loaded in the state.

## Findings reach tests only as distilled values

A test never reads a savestate, an EE dump or a recording: those are personal or generated, and absent on any other
machine. When a comparison becomes a regression, distil the facts it needs into a few numbers or a small,
human-readable fixture (no raw memory), commit it, and test the port against it with only `extracted/`.
