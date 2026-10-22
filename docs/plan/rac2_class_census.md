# R&C2 class census: the unported moby classes and the shared systems they need

A static census (2026-10-22) of every moby class on Going Commando's 27 levels (`G/LEVEL0..26.WAD`) whose update the
port does not run. It is R&C1's census (`class_census.md`, same method, same tool) pointed at R&C2: for each class it
finds the update on every level, groups the level copies that are the same code into one **class-port unit**, walks
the unit's static call graph down to the **shared functions** it calls (engine code in the overlay, level code other
classes also call, the boot ELF `SCUS_972.68`), and tags each shared function with the system it belongs to and whether
the port has it. Nothing is run; nothing in `crates/` changed for it. The gaps it finds are filed in `rac2_gaps.md`.

**Numbers at this run.** **1,052 unported class-levels**: **11,266 placed, 10,865 created** instances, in **662
class-port units**. They call **879 shared functions**; **462 are tagged**, 417 are not (most are level copies used by
one or two units). Verdicts:

| verdict (per unit) | units | created instances |
|---|---|---|
| **cheap**: every shared call is one the port has, trivial, or the unit's own code | 190 | 2,324 |
| **missing**: calls at least one function of a system the port lacks | 242 | 6,529 |
| **partly**: calls a function the port has only in part | 106 | 780 |
| **unknown**: nothing missing, but at least one untagged function | 124 | 1,232 |

"Cheap" means the class needs only its own code ported: the shared systems are there. It is not free: the private code
size (functions / words) is in the cheap list below and in `units.tsv`. As in R&C1, the census counts a function as the
unit's own when only that unit calls it, so a unit's private code can still hold a whole system. A second blind spot:
a **particle spawner's type** is a constant passed to `0x330df0`, and its update is reached only through the engine's
type table `0x1fe6c0` (set by `0x331ea0`), so a unit that spawns a type the port lacks still reads as cheap (0x0c5c's
type 0x30 was one). Check the types a cheap unit's spawners make against `rc-game/src/particles.rs`.

**Aranos (level 0)** had 4 unported classes at this run, all ported since (2026-10-22): 0x0c5c and 0x0cda (cheap;
`props::update_0c5c` with the particle type 0x30, `props::update_0cda`), 0x0ce9 (the tie lights,
`classes::tie_lights`) and 0x0340 (the remote's range, `props::update_0340`: its live part; the remote branch is
G-HERO-008). The first run read the last two as users of the first draw list `0x2e7718`: a keying collision, below.

**Re-run 2026-10-22 (the keying fix and the Aranos ports).** The draw-list registrars `0x2e7718` (the first list) and
`0x2e7920` (the second) are the same code with different list addresses, so their masked hashes are equal, and the
first run keyed every level's copy of either to the first L00 match, `0x2e7718`. Now a copy at the address of an L00
function with the same masked code is that function, and a hash shared by several L00 functions names none of them
(the copy keeps its `H:` key). The first list's registrar on the other levels is `H:582266980bbdcff0` (L01 `0x2dce00`,
152 units on 26 levels: their callbacks are their own code); the second's resolves to `L00:002e7920`. Aranos has
**no unported class left**. **1,020 unported class-levels, 11,160 placed / 10,759 created** instances in **653 units**,
876 shared functions (461 tagged). Verdicts: cheap 189 units / 2,353, missing 242 / 6,529, partly 99 / 685, unknown
123 / 1,192. The missing-system ranking below is unchanged; the partly line now reads the first draw list 72 units /
485 (G-REN-003).

## Method (to regenerate)

```
cargo run --release -p rc-trace -- class-census --game rac2 \
    --extracted <extracted-rac2> --out work/census-rac2
python3 tools/census/rac2_tags.py                           # regenerates the tag table from shared.tsv
```

Run the census, then the tag script (it reads `work/census-rac2/shared.tsv` and the port's citations), then the census
again so the verdicts use the new tags. Defaults with `--game rac2`: out `work/census-rac2`, tags
`tools/ghidra/names/census_systems_rac2.tsv`, extracted `rc_formats::test_data::rac2_root()`.

`tools/trace/src/class_census.rs` takes the game (`Rac`: `RAC1` / `RAC2`): R&C2's overlay base is `0x1a7c80` (the
`.lit` section; the same seven sections as R&C1: `.lit`, `.bss`, `.data`, `lvl.vtbl`, `camvtbl`, `sndvtbl`, `.text`),
its `$gp` is `0x1aeff0`, and each level's overlay sits at 0x80 of `LEVELn.WAD`'s data part. The reference level for the
keys is level 0 (Aranos: the only level with a decompiler export, `work/decomp/rc2/level00.elf`).

1. **Class list and instances.** Per level the class table (`lvl.vtbl`, 12-byte rows `{o_class, update, w8}`), the
   placed instances (the gameplay's moby list) and the created ones (`rc_game::moby::SpawnRules` with a fresh game:
   the instances the static spawn makes). A class is unported when `rc_game::moby::classes::update_for` gives no
   port and its update is not 0.
2. **Call graph.** Function starts as `LevelOverlay` finds them, with R&C2's `$gp` (`address_refs_gp`): `jal`
   targets, class-table updates, the start after `jr ra` + delay slot, code addresses formed with `lui`/`%lo`. Edges:
   `jal`, a `j` out of the function (tail call), stored callbacks (a formed code address with start evidence of its
   own), the fall-through into a start without evidence. `jalr` sites are counted as unresolved.
3. **Units.** From the update, a callee joins the unit when every caller of it is already in it; the rest are the
   unit's shared functions. Level copies of an update are one unit when their masked code hashes the same.
4. **Keys.** A shared function is `boot:<addr>` (boot ELF), `L00:<addr>` (its level-0 copy: same masked code at a level
   0 function start), else `H:<hash>` (the masked-code hash: level code Aranos does not link). `shared.tsv`'s `near`
   column gives an `H:` key's closest L00 function by masked similarity (≥ 0.85, length within 4 words).
5. **Tags** (`tools/ghidra/names/census_systems_rac2.tsv`, generated by `tools/census/rac2_tags.py`):
   * `cite`: the address is cited in R&C2's port (`crates/rc-game`, `crates/rc-engine/src/rac2*`,
     `crates/rc-formats/src/rac2`) or docs (`docs/plan/rac2*`): a code line that names it = **has**; only lines saying
     it is not ported ("not ported", "not yet", "not drawn", "later", …) = **missing**; the citing file names the
     system;
   * `L`: read in the decomp (`tools/census/rac2_tags_manual.tsv`, with a note per row; these win);
   * `range`: the vector maths `0x2ee000..0x2f0200` = trivial;
   * `leaf`: a leaf of at most 12 words = trivial (accessors, `jr ra` stubs);
   * `near`: an `H:` key takes its near L00 twin's tag;
   * `wrap`: at most 12 words with one tagged callee takes the callee's tag;
   * `calls`: at most 40 words whose callees are all tagged, with at most one system past the maths, takes that one.
   Statuses: has / partly / missing / trivial (R&C1's `family` is not used yet).
6. **Outputs** in `work/census-rac2/`: `classes.tsv` (level, class, placed, created, update, unit, port), `units.tsv`,
   `shared.tsv` (per key: units, instances, words, citations, callees, near), `globals.tsv` (engine globals the units'
   own code forms), `unit_systems.tsv` (verdict, systems, blocking keys, untagged keys) and `systems.tsv` (the ranking).
7. **Direct reads** (`G:` tags per global, `X:` overrides) work as in R&C1; no R&C2 `G:` / `X:` rows exist yet.

## Main table: missing systems, ranked by the instances they block

Units whose verdict is **missing**, by each missing system they call (a unit can count under several); "alone" = the
units for which it is the only missing system, so porting it makes them cheap or partly.

| system | units | created | alone: units / created | key functions (L00) | gap |
|---|---|---|---|---|---|
| breakable effects (shards, bursts, debris) | 67 | 2,312 | 45 / 1,465 | `0x3210a8`, `0x32b580`, `0x2ecfb0`, `0x327648`, H:55c6aaae… (`0x32b580`'s copy) | G-REN-002 |
| blob shadows | 86 | 2,260 | 38 / 727 | `0x31da58` | G-REN-001 |
| particle spawners (types beyond 0x17 / 0x2c / 0x55 / 0x59) | 53 | 1,757 | 30 / 1,047 | `0x3327e8` (type 2), `0x33b5e0` (0x3c), `0x37d5a0` | G-PRT-001 |
| level variant regions | 14 | 1,186 | 7 / 841 | `0x321a78` | G-LVL-001 |
| script camera (class 5) | 25 | 561 | 13 / 64 | `0x4287b8`, `0x428810`, `0x428880`, `0x4289f0` | G-CAM-001 |
| effect quads (beam, trails) | 4 | 400 | 2 / 21 | `0x2edd50`, `0x323c90` | G-REN-004 |
| colour take-over | 11 | 336 | 5 / 25 | `0x3241d8`, `0x31fa50` | G-REN-005 |
| message / prompt slot | 38 | 249 | 20 / 78 | `0x327a28` | G-UI-001 |
| reward items | 15 | 60 | 11 / 16 | `0x2f51a0`, `0x325270` | G-WPN-002 |
| point lights | 3 | 56 | 3 / 56 | `0x2fda20` | G-REN-006 |
| platform carry | 2 | 50 | 0 / 0 | `0x3222a8` | G-LVL-005 |
| speech | 10 | 23 | 3 / 3 | `0x32ea38` | G-AUD-003 |
| pad rumble | 1 | 15 | 1 / 15 | `0x2d6d60` | G-UI-005 |
| Clank's animation | 3 | 5 | 1 / 3 | `0x323d10`, `0x323e58` | G-HERO-004 |

**Partly** units, by the system they have in part: the first draw list (79 units / 580 at the first run, 72 / 485 after the keying fix: its registrar; G-REN-003),
moby groups (15 / 139: `0x31c970`, `0x31cac0`; G-CLS-002), the level sound table (14 / 34: `0x360750`; G-AUD-001),
the sequence-time test (3 / 58: `0x323bf8`; G-CLS-003), Ratchet / ship queries (3 / 27: `0x326448`, `0x326820`;
G-CLS-006), the line test (1 / 4: `0x31c618`; G-CLS-005).

`systems.tsv` ranks by every unit that calls a system (not only the blocked ones): the first draw list leads there (153
units / 2,397 after the fix), since most other levels' enemies register a callback on it.

## Cheap wins: classes whose shared calls the port already has

190 units / 2,324 created instances. The largest (class ids hex; private code = functions / words):

| unit | classes | levels | created | private code |
|---|---|---|---|---|
| U316 | 0x12a8 | 07 | 344 | 1 / 198 |
| U123 | 0x094e 0x0969 0x0afc 0x0b18 0x0ccb 0x0d40 0x0d54 0x0e1b 0x0e1c 0x0f78 0x1101 0x11ed 0x1354 | 02 03 04 09 11 12 13 14 16 17 20 26 | 140 | 1 / 2 |
| U99 | 0x0873 | 02 03 04 09 12 17 20 21 | 129 | 1 / 46 |
| U109 | 0x0a24 | 02 03 04 09 12 17 20 21 | 129 | 1 / 38 |
| U220 | 0x0cfd | 04 06 07 12 13 | 100 | 2 / 238 |
| U427 | 0x126e | 11 | 60 | 3 / 278 |
| U144 | 0x0e96 | 02 | 60 | 3 / 278 |
| U112 | 0x0a55 | 02 03 04 09 12 17 20 21 | 59 | 1 / 30 |
| U432 | 0x1351 | 12 | 45 | 2 / 138 |
| U586 | 0x0fbd | 17 | 43 | 1 / 170 |
| U275 | 0x0dda | 06 11 12 13 14 16 17 18 19 21 | 43 | 3 / 266 |
| U423 | 0x1105 | 11 | 38 | 4 / 840 |
| U48 | 0x0102 | 01 03 04 12 13 17 | 33 | 3 / 298 |
| U350 | 0x0bf4 0x0bf7 | 09 | 33 | 1 / 40 |
| U485 | 0x0d97 | 13 | 32 | 1 / 154 |
| U484 | 0x0d95 | 13 | 32 | 1 / 154 |
| U119 | 0x0aaf | 02 | 32 | 1 / 22 |
| U351 | 0x0c21 | 09 | 30 | 1 / 50 |
| U680 | 0x134e | 21 | 29 | 1 / 64 |
| U59 | 0x0a26 | 01 | 29 | 3 / 654 |
| U67 | 0x0a92 0x0a93 | 01 | 28 | 2 / 202 |
| U168 | 0x09a3 0x09a4 0x09a6 0x1345 | 03 21 | 26 | 2 / 182 |
| U580 | 0x0937 | 17 | 25 | 2 / 398 |
| U516 | 0x0b19 | 14 | 25 | 7 / 508 |
| U341 | 0x12aa | 08 | 25 | 1 / 136 |

On Aranos: 0x0c5c (U34, 7 instances) and 0x0cda (U39, 4), both ported since.

The unit ids change between runs (they are numbered by instances).

## Untagged shared functions (for a later pass)

417 keys, weighted by the created instances of the units they leave "unknown". The heaviest:

| key | units | created | at (rep. site) | calls |
|---|---|---|---|---|
| H:c0a367fb31d59c29 | 24 | 1,471 | L01 `0x3eb1d0`, 82 words | `0x311320`, `0x311688` (delete), `0x313330` (colour) |
| H:ec151f0b3dbeb437 | 9 | 634 | L09 `0x33fb10` | H:918c135569fd2f36 |
| H:f70f118271ca8beb | 5 | 592 | L01 `0x330218`, 76 words | rng, `0x330df0` (a particle spawner) |
| H:b475d9038b64efbf … (5 keys) | 3 | 575 | L08 | a level-08 family |
| H:a054c90325f108e8 | 19 | 456 | L02 `0x2f0170`, 142 words | maths, `0x311320`, `0x3237c8` |
| H:cc2970ae97f0def5 | 8 | 426 | L18 `0x2d0050`, 126 words | hero code (`0x2a4db0`, `0x2b2cd8`, …) |
| H:4104046b48beee6d | 22 | 389 | L01 `0x3245e8`, 28 words | (leaf) |
| L00 `0x2e6330` | 9 | 387 | 8 words | `0x30ef70` (`0x42f608` on `0x1a9054 + 0x36f28`) |
