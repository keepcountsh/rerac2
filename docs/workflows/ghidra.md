# Workflow: the Ghidra project

Tools: `tools/ghidra/` (README there). Conventions for names: `docs/plan/decomp_workflow.md`. Run every command
from the repo root with the Ghidra GUI open on the project and the GhidraMCP plugin serving `127.0.0.1:8089`. The plugin is [bethington/ghidra-mcp](https://github.com/bethington/ghidra-mcp)
(not LaurieWired's GhidraMCP, which has no project or import endpoints); if its Server Port option is set to another
port, set `GHIDRA_MCP_URL` (e.g. `http://127.0.0.1:18489`) for the scripts, as for the plugin's MCP bridge.

## What the project holds, and why it is disposable

* **Programs.** `~/Ghidra/ratchet2.gpr` holds `/SCUS_972.68` (the NTSC v2.00 boot ELF: the always-resident core,
  loader, memory, IOP RPC, the renderer's fixed part) and `/levels/levelNN.elf` for the 27 levels (00–26). Each
  level has its own **overlay**: everything else, the hero included, compiled into every level and loaded over a
  fixed address window when the level starts. Overlays overlap each other, so each is its own program; boot
  addresses are not valid inside a level and vice versa.
* **The source of truth is not the project.** It is the disc (through `extracted/`) and the scripts; everything in
  the project can be rebuilt from those. The analysis and the names given in Ghidra are the project's own, so back
  it up (below).
* **Overlays.** Uncompressed, the same seven sections as R&C1's in the same order (`.lit` at 0x1a7c80, `.bss`,
  `.data`, `lvl.vtbl`, `lvl.camvtbl`, `lvl.sndvtbl`, `.text` at ~0x2a4000, 1.6–2.0 MB of code), at offset 0x80 of
  each level's data WAD (`G/LEVELn.WAD` header 0x60 bytes, `data` range at 0x10, in sectors relative to the file;
  the level data header's first ByteRange is the overlay). `$gp` is 0x1aeff0.

## Set up from scratch

Needs `extracted/` (docs/workflows/game-data.md), the PS2 processor module (`~/Globals/ghidra-emotionengine-reloaded`)
and the GhidraMCP plugin (`~/Globals/ghidra-mcp`) installed in Ghidra. One step per row, in order:

| Step | Command / action |
|---|---|
| 1. Level ELFs and the boot ELF from the disc's files | `python3 tools/ghidra/scripts/overlay_to_elf.py` (reads `extracted/`, writes `work/ghidra-import/`) |
| 2. Create or open `ratchet2` in the Ghidra GUI | the plugin only sees the open project |
| 3. Import and analyse the boot ELF and the 27 levels | `python3 tools/ghidra/scripts/import_levels.py` (background; refuses unless `ratchet2` is open) |
| 4. Names from R&C1 | in R&C1's project export `SCUS_971.99` and `levels/level01.elf` as *Ghidra Zip File*; import both into `ratchet2` under `/rc1`; run a Version Tracking session rc1 → rc2 (exact bytes, then function hash, then reference correlators) and accept the matches; the names travel with them |
| 5. Export the decompiled C | `python3 tools/ghidra/scripts/export_decomp.py level00.elf` (and `SCUS_972.68`) |

The export lands in `work/decomp/<program>/` (one `.c` per function plus `index.tsv`; git-ignored, it is the game's
code). The class census reads `work/decomp/level00.elf` and `work/decomp/SCUS_972.68` (docs/plan/class_census.md).

## Cross-game compare (`crossgame_diff.py`)

`python3 tools/ghidra/scripts/crossgame_diff.py --rc1 <R&C1 level 01 overlay.bin> --rc1-index <R&C1
work/decomp/level01.elf/index.tsv> --cite crates/rc-game/src/hero` compares R&C1's level01 overlay with this game's
function by function (the level-to-level relocation mask, plus a stack-frame normalisation because Going Commando is
built with 64-bit `sd`/`ld` callee saves and tighter frames where R&C1 uses `sq`/`lq`, plus nop/branch-offset
normalisation because its compiler pads branches) and writes `work/crossgame/level01.tsv` (`=` identical, `k` other
constants, `s` same shape, `?` changed or gone). The R&C1 inputs are R&C1's own (its data folder and decompiler
export). `--similar NAME...` scores a changed function against its closest counterpart here. Result 2026-10-16: 16%
of R&C1's level functions are identical or differ only in constants, 8% keep their shape, 75% changed. The engine
services survive (collision, moby/shrub/tie processing, skin lighting, vector maths, GS packets at 0.8–0.99
similarity); the hero, game state, level loading and the path enemies were rewritten (0.1–0.4).

So "find the movement code here" is: find it in R&C1's named `level01.elf`, version-track that program against
`ratchet2`'s `level01.elf`, and read the functions that did *not* match exactly: those are the ones Insomniac
changed. R&C1's per-level overlay diff (`rc-trace overlay-diff`) stays in R&C1's repository until its section indices
and `$gp` are parameterised for this game (docs/plan/porting_from_rac1.md).

## Backups

| Task | Command |
|---|---|
| Back up the project (GUI closed) | `tar -C ~/Ghidra -czf ~/PS2/ratchet2/ghidra-backup-$(date +%Y%m%d).tgz ratchet2.gpr ratchet2.rep` |

The scripts are in git and the import ELFs are rebuilt by step 1, so they need no backup of their own.

## Reference material (not vendored)

Checked out next to the repo on the author's machine, never copied into it:

* `~/Globals/wrench`: Wrench (GPL-3), used as format documentation only; all reader code here is written from the
  specs in `docs/formats/`.
* `~/Globals/jak-project`: OpenGOAL (ISC), architectural reference.
* `~/Globals/ghidra-emotionengine-reloaded`: PS2 processor module for Ghidra.
* `~/Globals/ghidra-mcp`: GhidraMCP bridge to the Ghidra project.
