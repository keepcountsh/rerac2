# R&C2 (Going Commando) bring-up

Status: **started 2026-10-16; the front end runs (milestones 1–3).** Goal for 0.3.5: Going Commando can be selected, starts, and shows its own main menu.
New Game says "not yet". Nothing of R&C1 is rewritten for it.

## 1. Approach

- **One engine, two games.** The engine (`rc-engine`), the renderer, the format readers (`rc-formats`) and the data
  layer (`rc-data`) are shared. Where a format differs between the games, the reader takes the game as a parameter;
  it is not copied.
- **R&C2's game logic lives in its own crate, `rc-game`,** the way R&C1's lives in `rc-game`. Systems that turn out
  to be the same code in both games (by `tools/ghidra/scripts/crossgame_diff.py`) move to shared modules when a
  second consumer exists, not before (no speculative ports).
- **Evidence first.** Every R&C2 behaviour comes from the R&C2 binaries in the `ratchet2` Ghidra project
  (`docs/workflows/ghidra.md`, R&C2 section), never from R&C1 by analogy.

## 2. Milestones

1. **Extractor (done).** Build `SCUS_972.68` v2.00 NTSC-U is supported. R&C2 keeps its assets in the ISO filesystem,
   so its Tier 0 archive is the filesystem's files under their disc paths (`SYSTEM.CNF`, `SCUS_972.68`,
   `IOPRP255.IMG`, `RC2.HDR`, `G/*.WAD`; `G/DUMMY.` is padding and skipped): 96 files, 3,807,638,923 bytes, table
   `crates/rc-extract/data/scus_972_68.tsv`. `extract-info.json` gains `"game"`. There is no Tier 1 cache for R&C2
   yet. The development data folder is `<repo>/extracted`.
2. **Front-end map (Ghidra).** The boot flow from the boot ELF's entry (0x131ae8) to the main menu: logos and
   movies, the title, the main menu's pages, the game-mode values, and the assets each needs (fonts, menu textures,
   text, background, music).
3. **Engine boot path (done).** A data folder holding `RC2.HDR` starts Going Commando (`rc-engine` `disc_source::game`,
   `front_end.rs`); the front end's logic is `rc-game` (`front_end.rs`, `gui.rs`), its data readers `rc-formats` `disc/`.
   The splashes, the Insomniac movie, the title (couch movies, "Press START") and the main menu run and draw through
   one `hud_render::Layer`. Not yet: any sound (movie audio, menu sounds, music), what the rows open (New Game, Load,
   Options log "not in this version yet"), the subtitles over movies, PAL / other languages.
   Run: `RC_DATA_DIR=<repo>/extracted cargo dev` (front end, then New Game → Aranos);
   `RC_LEVEL=n` starts in level n without the front end (`RC_FRONTEND=1` keeps it).
4. **Ship.** The launcher lists Going Commando, installs it to `games/rac2`, and starts it on the main menu.

## 3. Disc facts (SCUS_972.68 v2.00)

- Boot ELF 2,621,256 bytes, SHA-1 `4124a79c1e5a1ceb56f514df0dbbc11f072af56f`; image 1,867,488 sectors.
- `RC2.HDR` (sector 1001) is the table of contents: the global WAD headers, then the level table with three
  sector ranges per level (level, audio, scene), sector numbers relative to each file
  (`docs/formats/disc_layout.md` §2.10, §4).
- Global WADs: `MPEG`, `MPEG1`, `MPEG2`, `MISC`, `HUD`, `BONUS`, `AUDIO`, `SPACE`, `SCENE`, `GADGET`, `ARMOR`.
  Per level `n` = 0..26: `LEVELn`, `AUDIOn`, `SCENEn`.

## 4. Code facts so far

- The level overlays' entry is the main loop (level 0: 0x2a3b88). The game mode is the word at 0x1a8f00, cases
  −1..9. The memory-card check is the same as R&C1's (icon 0x755d).
- The boot ELF holds the front end's strings (`\DEMOLOGO.NTS`, `\INFOBOT2.PSS`); R&C1's front end also lived in its
  boot ELF.
- `crossgame_diff` over level 1: 16% of functions identical or constants-only, 8% the same shape, 75% changed. The
  front end (`entry`, the page menus, the movie player) changed.

## 5. The front end (boot ELF `SCUS_972.68`)

Addresses are the boot ELF's. Names are ours.

**Program flow.** `entry` (0x131ae8) → `main` (0x131d98): runs a code pointer in a loop, starting with the front
end (0x26ede8); after each run `0x131cb8` copies the code overlay of what was loaded next (segments
`{dest, size, ?, id}` from `*(0x1a7288)`) and returns its entry.

**Disc reads.** `RC2.HDR` is read into memory at 0x14b540: the `MPEG` header there, `MISC` at 0x14b868
(`+4` the absolute sector, then `{sector offset, sector count}` pairs from `+8`). MISC pairs (v2.00):
(1, 9), (10, 367), (377, 62), (439, 844), **(1283, 1109) the front end**, (2392, 2081), (4473, 2772).

**The front-end blob** (MISC pair 4, WAD-compressed, 2,985,592 bytes unpacked) starts with 13 `{offset, size}` pairs:

| Pair | Offset | Size | Use |
| --- | --- | --- | --- |
| 0 | 0x80 | 0x11400 | texture set (`0x292650`): `{data offset, count, table offset, …}`, rows `{palette, pixels, w, h}` (16-byte units from the data, 8-bit, 256-colour CSM1 palettes); slot 1 is the 16×16-cell font |
| 1 | 0x11480 | 0xe1088 | `0x2b7700` (textures) |
| 2 | 0xf2540 | 0xd0010 | the menu backdrop: 16-byte header `{512, 416}`, then 512×416 RGBA32 (the station interior) |
| 3–8 | 0x1c2580.. | | not mapped yet |
| 9 | 0x1f8980 | 0xd17b0 | text: six language tables (index 1 empty); each `{count, size}` then `count` rows `{string offset from the table, id, -1, 0}` |
| 10 | 0x2ca140 | 0x4ab7 | splash "Sony Computer Entertainment America presents" (WAD: `{w, h}` 16-byte header, RGBA32) |
| 11 | 0x2cec00 | 0x4533 | splash "A game created and developed by" |
| 12 | 0x2d3140 | 0x5d38 | the copyright page (shown by game mode 1) |

**Boot sequence** (0x26ede8): wait for the memory card and pad checks; unpack the blob; splash 10 fades in
(`0x26ea00`: 0x80 → 0 in steps of 2), 45 frames; fade 30; splash 11; sound and text set up; then movie 0xb2 (the
Insomniac logo) plays (`0x2894c8(0x1bacc0, 0xb2)`) and the game-mode loop starts with the idle counter at 0x708. So
the first mode-0 frame starts the **title**: the attract playlist (`0x26ec58`: 24 entries from three shuffles of 0xb3..0xbb, each
the couch scene with a different clip on the TV) with "Press START" over every frame (`0x27db38`: text 0x2db6 at
(60, 328), colour pulsing 0x5ff0c070 ↔ 0x7fe0e0e0 over 100 frames, drawn by the PSS player's frame callback
`0x2b73a0` while `0x1bacc0` is set; otherwise that callback draws subtitles, `0x27dc40`). Start ends the playlist,
mode 0 comes back with the counter reset, draws the station image and opens the main menu (kind 6) over it. A minute
on the menu (0x708 frames) goes back to the title.

**Movies.** Row `i` of the MPEG header (`RC2.HDR` + 0x10 + 16·i: `{sector from MPEG.WAD's start, bytes, 0, 0}`)
spans `MPEG.WAD`, `MPEG1.WAD` and `MPEG2.WAD`, which lie back to back on the disc (sectors 2611, 527644, 1160568).
Ids map to rows by `0x2efe68` (ids above 0xb0: id − 0x91; 0xb2 = row 33).

**Game mode** (current 0x1a8bb0, pending 0x1a8bb8 = −2 when none; push `0x2b58f8(mode, 1, arg, …)`, pop
`0x2b5a88`; applied by `0x2b5b38`, which runs the old mode's exit and the new one's enter):

| Mode | Front-end loop | Meaning |
| --- | --- | --- |
| 0, −1 | draw the backdrop, push mode 3 with kind 6; after 0x708 idle frames: shuffled attract movies (0xb3..0xbb) | title |
| 1 | `0x2f6e10`: the PSS player (`0x34fca0`), movies from MPEG.WAD (header 0x14b544, 16-byte rows) | movie |
| 3 | `0x2cb860` update, `0x2ccc18` draw; 0x708 idle frames → pop | menu |
| 4, 6 | other screens | not mapped yet |

**The menu** (mode 3, enter `0x2cac28(kind)`): sub-state 0x1f27c0 runs 1 → 2 → 3 → 4 (page shown). `0x2cb038`
picks the page from the kind (0x1f27c8): kind 6 = the main menu, page 0x259438. A page is 14 panel rectangles
`{x centre, y centre, w, h}` (0xe0 bytes), then `+0xe8` the focused panel and `+0xec` 14 panel pointers. A panel is
0x30 bytes: `{update, draw, open, close, flags, ?, x, y, w, h, …}`; flag 1 = draws itself. The main menu page has one
panel (0x1aa618) that drives a GUI object at `*(0x1a8d04) + 0x3e9c8`, opened with the menu definition 0x261520
(`0x342dc0`) and drawn by `0x29d958`.

**Menu definitions** are rows of 0x14 bytes `{scale (f32), text id, kind, target, 0}`, ending in a zero row. The main
menu (0x261520): "New Game" (0x2c35, kind 4, 0x260ec8), "Load" (0x2c2b, kind 5, 0x260d98), "Options" (0x2bec,
kind 3, page 0x259568).

**The main menu's look.** Origin (250, 120) (`0x342bf0`). Per item `i`: two GUI sprites at (150, 120 + 60·i) —
0x73 (colour 0x60442d00) and 0x74 (0x55f0c070; the selected item's pulses 0x20fffeed ↔ 0x70fffeed over 20 frames,
`0x2aa3f0`), both at keyframe 0.25 — then the label centred at (250, 131 + 60·i) (`0x348e70`; list step 60, offset
11), colour 0x70fffeed selected, 0x80f0f0f0 normal, 0x80808080 disabled. Behind: the station image, darkened by a
full-screen black quad at alpha 0x38 (`0x27e440`). Colours are GS RGBA bytes (alpha 0x80 = opaque). The screen is
512×416 (`0x284c18(0x200, 0x1a0, …)`).

**GUI sprites** (the `BE_H` block, HUD.WAD; sector and count in the HUD header at `RC2.HDR` + 0x368 + 0x1350; v2.00:
sector 16415, 64 sectors): `+0x18` count, then `{id, offset}` rows. A sprite: `u16 vertices, prims, bones, textures,
frames, frame stride; f32 scale (× 0.0032116016); offsets of vertices, prims, textures, ?, frames; +0x24 packed;
+0x28 colour`. Packed vertices are one word: bone = bits 28..31, x = sext(bits 0..13)·4, z = bits 12..27 (signed);
else 16 bytes `{s16 x, y, z, bone; s16 u, v (/4096); RGBA}`. Prims are four vertex indices (a quad drawn a, b, d, c);
a word whose top 24 bits are all ones selects a texture. A bone key is 0x18 bytes: quaternion (s16 / 32768), scale
(u16 / 4096), translation (s16) and parent (s16, −1 = none); two keys blend linearly (`0x3015d8`). On screen
(`0x3018a0`): x = pos.x + (256 / 0.62)·s·v.x / 16, y = pos.y − (208 / (0.62·0.775))·s·v.z / 16; vertex colour × draw
colour / 128.

**Font** (`0x27fcb0`): glyph table at 0x263b10, 4 bytes per character `{u, v, y offset, advance}` in the 16×16 cells
of texture slot 1; a glyph is drawn 16·scale wide at (x, y + y offset·scale); characters 8..15 change the colour,
those below 0x20 are pad-button icons (24×16, drawn grey), 0x80..0xa7 carry an accent from character + 0x40.

## 6. Aranos (level 0): what is shared, what is new

Goal (2026-10-16): the first level, Aranos (level 0; the planet table in the help text starts Aranos, Oozla, Maktar
Nebula, …), playable with its cinematics, mobys, effects, sound and saves. R&C1 does not change behaviour: every R&C2
branch is taken on `rc_formats::Game::Rac2` only, and R&C1's default paths stay byte for byte what they were.

### 6.1 Evidence

- **Data** (`crates/rc-formats/examples/rac2_probe.rs` over `G/LEVEL0.WAD`). The level file is a 0x60-byte header
  `{0x60, 0, id, reverb}` then `{sector, sectors}` parts: data, core sound bank, gameplay (WAD), occlusion, chunks.
  The data part opens with a 0x80-byte `{offset, size}` table: overlay (0x80), core index, gs_ram, a 0xfdc-byte
  lump (not mapped), HUD header, four HUD banks, core data (WAD), transition (`rc_formats::disc::level`).
  - The **core index** has R&C1's header and tables, unchanged: 833 tfrags, 351 textures, shrubs, sky (6 shells),
    collision and occlusion parse with R&C1's readers as they are.
  - **Ties**: the class header holds R&C1's fields in another order (`tie::read_tie_header`); the packets, strips and
    GS packets are R&C1's (73 of 74 classes walk). There are no per-vertex colour indices (count 0): each instance
    has its own colour slots (gameplay `tie_colors`), lit per instance by `0x36f0a8` (R&C1's `LightTies` maths, normals
    from the angle table) and averaged into vertex colours by per-LOD colour programs at +0x34 (`rc_formats::tie`,
    `rc_formats::tie_light`); slot 511 is the class's self-lit colour. Not ported: the per-slot point lights (VU0 0x580).
    Tfrags and shrubs are lit as in R&C1 (L00 `0x36b4f0` = `LightTfrags`, `0x35d690` = `LightShrubs`, the same VU0
    entries), at load.
  - **Moby classes**: R&C1's class header and packets; only the vertex table header differs, eight u16s (0x10
    bytes) with no RGBA multiplier blob. R&C2's relocation (L00 `0x2ffe70`) converts an "old format" class — R&C1's
    eight u32s — to that form. `moby::parse_moby_class_for` reads every class of every level checked (127–208 per
    level). Class 0 (Ratchet) has no mesh in the level (`{0, 0x5502163f, …}`). The core's words 0x88 / 0x8c are not a
    gadget table on R&C2 (Aranos: 0xa7, 8); gadgets are in `GADGET.WAD`.
  - **Gameplay**: another section table (pointer order differs; 0x1c89c0 bytes on Aranos).
- **Code** (`tools/ghidra/scripts/crossgame_diff.py`, `work/crossgame/level01.tsv`, R&C1 level 1 against R&C2):
  identical or nearly so: `TfragProc`, `PatchTfragGifs`, the sky (`LoadSky`, `SkyDrawShell*`, `SkyTransformVertices`),
  `MobyAnimEval`, `MobyAnimProc`, `ProcessMobyAnimData`, `PatchMobyGifs`, `PatchShrubGifs`, collision
  (`CollCellLookup`, `coll_sphere_hero_groups`, `CollType`), `UpdateFog`, `ParseOcclGrid`, GS register resets,
  `SoundInstanceUpdate`, `memcard_GetDataSize`, many particle types (08, 21, 26, 27, 45, 47, 52, 60, 66, 72).
  Rewritten (0.1–0.4): the hero (`HeroStatePhysics`, `HeroStateTransitions`, `HeroInit`), `GameStateUpdate`,
  `LoadLevelCoreData`, `MobyProc`, `TieProc`, `ShrubProc`, `LightTies`, the path enemies.

### 6.2 Decisions

1. **One engine app, the game picks its logic.** `rc-engine` keeps one Bevy app: the renderers (tfrag, tie, shrub,
   sky, mobys, particles, water, display, GS state, AA, graphics settings), the frame pace, the capture, audio out.
   The game-specific layer is a plugin: R&C1's `gameplay`, menus, travel and front end; R&C2's `rac2` (logic in
   `rc-game`). No second copy of a renderer.
2. **Formats branch, not fork.** A reader whose layout differs takes the game from `LevelCore::game` (or a `Game`
   argument) and maps R&C2's bytes into the same structure (`tie::parse_tie_class_for`). A reader gets an R&C2
   twin only when the structure itself differs (moby classes, the gameplay table) — in `rc_formats::disc`.
3. **Truly shared code moves once.** What the diff shows identical (the list above) is one implementation; when
   R&C2 is its second consumer it moves out of `rc-game` into a shared place with a re-export left in `rc-game`, so
   R&C1's paths and behaviour do not change. Nothing is moved before R&C2 calls it.
4. **Rewritten code is reimplemented, with R&C1 as the map.** The hero, the game state, the moby classes, the
   menus and the cameras are written from R&C2's code in `rc-game`, in the same organisation as R&C1's: the hero as
   a state registry (state → group module, the game's dispatchers for SetState, physics and transitions), one module
   per state group, a `physics` pipeline and an `anim` interface, field names with the R&C2 addresses. R&C1's
   modules are the hint for where to look, never the source of a behaviour.
5. **Port options are the same options.** The settings (graphics preset, anti-aliasing, detail, textures, HUD
   resolution, sound volumes where the port adds them) are engine settings in the one settings file and apply to
   both games. Each game shows them on a "Port Options" page in its own menu style: R&C2's page is built from its
   own GUI sprites, font and sounds, as R&C1's is from its page machinery.
6. **Saves: same model, R&C2's tables.** R&C1's save code reads its chunk tables from the boot ELF, checks a
   CRC-16 and keeps one folder per card (`rc_formats::save_game`, `rc_game::memcard`). `memcard_GetDataSize` is
   identical in R&C2 and `memcard_TestChecksum` nearly so: R&C2's saves are read with the same reader given R&C2's
   table addresses and file names (475 KB per save, R&C2's icon), on the same native card folder under a per-game
   directory; R&C2's progression (what a save holds) is `rc-game`'s.

### 6.3 Milestones (each one runs and is committed on its own)

| # | Milestone | Shared | New |
| --- | --- | --- | --- |
| A | Aranos world: the level file's lumps, tfrags, shrubs, sky, collision, textures, ties (unlit), fly camera (done, `RC_LEVEL=0`); New Game loads Aranos (done: the front end runs over the loaded level in the one app, crate::rac2; the save-slot page between is §I) | the renderers, `level_load` | `disc::level`, the R&C2 load path |
| B | The gameplay file: section table, tie / shrub / moby instances, level settings (fog, background, death height), cameras, volumes, splines, lights | the instance readers where equal | the R&C2 section map |
| C | Moby classes: mesh, skeleton, animation, textures; drawn and animated at their instances (done: 384 of 472 Aranos instances placed, 156 animated; not yet the loader's ground snap, the spawn rules, Ratchet's armour mesh) | moby renderer, anim eval | the R&C2 vertex table header |
| D | Tie and moby lighting as R&C2 does it | light passes where equal | R&C2's tie colours |
| E | Sound: the level's bank, sound instances, music, movie audio, the front end's sounds | `audio_out`, 989snd, VAG | R&C2's bank tables |
| F | Ratchet: hero states for Aranos (walk, jump, swing, wrench, the tutorial's gadgets), follow camera, collision | collision queries | `rc-game::hero` |
| G | Aranos's mobys: the tutorial's objects, enemies, crates, bolts, doors, the ship; HUD and pause menu | moby scheduler services | `rc-game` classes |
| H | Cinematics: Aranos's scenes (SCENE0) and movies, subtitles | `scene_render`, movies | R&C2's scene format |
| I | Saves and the level end (to Oozla, or the "not yet" screen) | `save_game`, card folder | R&C2's tables |

### 6.4 Ratchet (milestone F, started 2026-10-16)

**What the code shows.** R&C2's hero is R&C1's hero evolved, not rewritten: the same driver (`0x2b37e8` = R&C1
`0x231d18`: stick with the d-pad below 0.25, per-state physics, yaw / position rings, rows, capsule, move, post-move),
the same two dispatchers (physics `0x2ba8e0`, transitions `0x2cb650`), SetState `0x2c4560` with R&C1's bookkeeping,
the same helpers (stick target `0x2b3af8`, turn `0x2b4000`, speed `0x2b42c0`, drag `0x2b5600`, edge brake `0x2ba008`,
wall check `0x2b4560`, move `0x2b5c68`, probe `0x2b4a08`, snap `0x2b5358`) and the same core state ids (idle 0, walk
2, stop 3, crouch 4, fall 6, jump 7, run jump 9), with R&C2's constants (run speed 5.95, jump 1.28–2.3 / 1.0–2.7,
gravity 0.00825) and additions (gravity frames, strafe, 0x93 states). From the velocity on, the hero block's fields
sit at R&C1's address + 0x4aae0. The crossgame diff's 0.1–0.4 similarity came from those additions inside long
functions. So the port follows R&C1's organisation (`rc_game::hero`: registry, states, physics, ground, walk, air,
jump), each function read from R&C2's code, in native f32. Details that differ were found that way (the stick
magnitude `(max(|x|, |y|) + |stick|) / 2`, the spring's pre-clamp, the snap constants).

**Shared, used as they are:** the pad record (`0x3306b0` = R&C1's `ProcessPadInput`), the collision kernels, and
Ratchet's animation player (SetAnim `0x2d2508` = R&C1's `0x247a90`). New in R&C2 and now read by `rc_formats`:
Ratchet's mesh is the armour's (`disc::armor`), and his sequences are compressed (frame pointers with 0xf in the top
nibble, decoded by `0x313fa8`: `moby_anim::decode_compressed_frame`).

**Done (2026-10-21): every state SetState has a case for.** `rc_game::hero::registry` lists them by module:
ground (0, 1, 3, 4, 0x1d), walk (2, 0x2e ice, 0x70 wading), air (6, 0x2c tumble), jump (7, 9, 10, 0xb..0x12, 0x1b, 0x38,
0x45, 0x48, 0x5b, 0x66, 0x6e, 0x81, 0x89, 0x92), pack (8 glide), melee (0x13..0x15, 0x1f, 0x22, 0x6d, 0x83, 0x93),
ledge (0x17..0x1a), damage (0x16, 0x39, 0x65, 0x67, 0x74, 0x78..0x7d, 0x7f, 0x8c), water (0x32..0x36, 0x72, 0x73), swing
(0x23..0x25, 0x2b, 0x8a, 0x8b), grind (0x27..0x2a, 0x3e, 0x71), Clank's body (0x3f..0x4e), scripts (0x1c, 0x1e, 0x31,
0x37, 0x3c, 0x3d, 0x60..0x64, 0x6f, 0x75), gear (0x7b, 0x82, 0x85, 0x86), misc (0x26, 0x2f, 0x30, 0x76, 0x77). With
them the probes (ledge `0x2ad000`, wall jump `0x2ac5f0`, rail `0x2adbb8`, pole `0x2adda8`, ahead `0x2c3068`, the
ground probe's liquids `0x2b4a08`), the surface check `0x2aca80`, the prologue's water checks, strafing and the
strafe flip, the hit intake and the health limit from the XP (`0x2d6258`). Objects ask for states through
`Hero::request_state` / `script_walk_to`. Clank's packs read `GameState::clank` / `thruster` (off on Aranos's
opening).

**Not yet:** the weapons and gadgets as hand items (the trigger cases other than the wrench and the Swingshot), the
lock-on `0x2cb2e0`, the quick select, the vehicles' bodies (`0x18c0d4` 2 / 3), the sounds, the effects (sparks,
splashes, bubbles) and the body lean.

### 6.5 The follow camera (milestone F, 2026-10-16)

**What the code shows.** The camera update `0x2d9f38` calls the current camera's type update from the table at
`0x2a3a0c` (0x14 bytes a type). Type 0, the follow camera (update `0x4278c8`, init `0x424908`), is R&C1's type 0
evolved: the same pre-steps (collision flags `0x2d9ba8`, motion `0x2d9760`) and the same steps in the same order
(platform carry, state tweaks, targets with the target modes and the vertical target, leash, stick, avoidance with
the sphere chain and end sphere, placement, rows, spring back), each a function of its own. R&C2's differences:

* the vertical spring is 0.0125 (R&C1 0.015), and in the air the state tweaks ramp it from the smoothed target's
  vertical gap;
* the leash blends at 0.85;
* the stick's yaw rates are 1° / 1.3° / 2.2° a tick (the option at `0x1a7bb4`);
* target mode 1 also starts in group 0x1e;
* the knock-down state is 0x74 (R&C1 0x77);
* the placement has no 15° clamp: a 0.25 sphere (flags 0x36) pushes the camera out up to three times instead;
* the rows' pitch bias applies only on the ground;
* the snap behind Ratchet at the init only pulls in on the line from the look point, 0.5 short of the wall.

The hero tells the camera about jumps through `0x18c2c0` (0x50 in the jump states; the smoothed hero height holds
while it is set). The port is `rc_game::camera` (one function per step, the L00 address on each), in native f32.
**Not ported** (none is in use on Aranos's opening): the camera-tweak requests `0x2d7568`, the camera-collision grid,
the moby collision of the camera's queries, the avoidance's moby nudges, the focus mobys, the stick auto-centre, the
Clank and Giant Clank cameras, water, the knock-down view, the other camera types, the blends and the shakes.

### 6.6 Aranos's objects (milestone G, 2026-10-21)

The level's update table (`lvl.vtbl` at `0x2a2e80`, 233 records `{o_class, update, 0x2a3978}`) covers the level's
own classes and every weapon and gadget; a class not in it (or with a null update) has none. Of the 76 classes with
instances on Aranos, 50 have an update. Ported (`rc_game::moby::classes`): the wrench 0x47, the crates 500 / 501 /
511, the bolts 13..19, the spawn points 0x0325, the chute 0x02a7, the gate 0x0ac7; in `props`: 0x0919 (a camera
volume), 0x0ac0 (a mover), 0x0ac1 / 0x0ac2 / 0x0aca (the lifts, carrying Ratchet), 0x0ac3, 0x0ac4 (a gate), 0x0ac5 /
0x0ac6, 0x0ac8, 0x0adb (a hovering craft), the wrecks 0x0ac9 / 0x0c0a..0x0c0d / 0x0e01 / 0x0e02, 0x0c22 (the
tutorial's help), 0x0c4c (the sky ships), 0x0c63 (a mission area), 0x0c8b (the death height), 0x0c8c (the interior
swap), 0x0d1e (searchlights), 0x0ddf (a view volume), 0x0f13 (a scene's props), 0x1143 / 0x12fa (breakables),
0x12f5, 0x12f9 (a barrier held by two objects), 0x132d (the sun flare's object), 0x1330 (the arrival); in
`carriers`: 0x0a38, 0x0c66, 0x09c7 (a floating rock with a mount). Shared services: the volumes, areas and paths
(`rc_game::level`), moby states `0x31b8d0`, the sound hooks of the moby host, the camera tweaks, the spawn rules
and the checkpoint, the help requests, the hero's carry on movers (`0x322668`, `moby::carry_hero`), the eased move
`0x31ed78` (`physics::ease`), the group attach (`moby::attach`: mobys, ties and shrubs).

**Effects only** (nothing else in their updates; they wait for the effects pass): 0x0cda (steam), 0x0c5c (puffs),
0x0ce9 (a tie group's colours pulsing, `0x3701d0`). 0x0340 acts only with Ratchet in the ship (`0x18c0b4` = 0x1c,
the class 0xac), never on Aranos.

**The help panel** (`rc_game::help`, drawn with the HUD): the requests the objects make (`0x2f5270`), the
panel's states `0x2f5680` (open, grow, fade in, show for 7 ticks a character, △ dismisses and counts in the slot,
fade out, shrink), the word wrap `0x2ea908` and the rounded box `0x2e9548`, over the level's English help texts.
The border is the GUI sprites 0x57 / 0x58 (`0x42f3b0`). Not yet: the help voice, the prompt slot it holds
(`0x327ae0`).

**Engine gaps the objects left** (the game side is done): the ties and shrubs the carriers move
(`MobyWorld::moved_ties` / `moved_shrubs` with their new matrices in `ties` / `shrubs`: the tie and shrub instance
buffers, the tie LOD spheres and the shrub culls follow nothing yet) and the ties hidden by 0x0ddf / 0x09c7
(`MobyWorld::hidden_ties`); both need the shared tie / shrub renderers to take updates, which R&C1 never needed. The
arrival scene of 0x1330 and the scene parts of 0x0f13 wait for the scene system (`0x42a4e0`, `0x1bca70`).

**Also ported**: the stubs 0x0b41 / 0x0b42 / 0x0b45 / 0x0b4d / 0x0b51 / 0x0b52 / 0x0b53 / 0x0c02 / 0x0c3f (`0x41b058`
.. `0x41b170`, `0x3dead0`: state 1, mode |= 0x800) and the scene trigger 0x0f6e (`0x40ffa0`: its scenes, level 0's
lists 0x21 / 0x23 / 0x22 / 0x16 at `0x1b1000`, end at once; then the item, the mission and the checkpoint).

**The enemies** (`rc_game::moby::classes::enemies`, on R&C2's enemy framework `rc_game::moby::ai`): 0x099c (a
flyer, `0x3bb120` / `0x3bbab0` / `0x3bbf38`), 0x0b0a (a burrower with its mound 0x0ab7, `0x3d6900` / `0x3d7a48` /
`0x3d81e0`, its bite `0x3d89f0`) and 0x0b0b (`0x3d9020` / `0x3dac08` / `0x3db4b8`: its two spinning parts 0x0aa2 at
its joints 0 and 2, `0x3dbed0` / `0x3dbfb0` / `0x3dc090`; its wander; its path walk). Each module's docs list its
states. The framework (`moby::ai`'s docs): the blocks, the senses `0x32dcf0`, the target `0x32ad38` with the trigger
areas' occupants (`GameState::area_tick`), the movement `0x32cc30` / `0x32cd20` / `0x32ce98` with the floor, walk,
push-out, fence and ground steps, the path walk `0x32dd60`, the wander `0x323958` with its step walker `0x31c278`, the
damage intake `0x327e38` with the reactions (sequence `+0x14` records), the knock-back flight `0x328b80`, the attack
spheres `0x31d180` / `0x31d2c8`, the ground height `0x31ce20`, Ratchet's hitter of the tick (`0x18c2bc`).

Not yet (the effects and sound passes): their deaths' bursts and debris (`0x3f8348`, `0x3210a8`, `0x327498`), the
sparks and trails, the shadows `0x31da58`, the colour flash `0x3201e8`, the lighting `0x31a8e4`, the head and tail
aims (`0x324208`, `0x327308`, `0x32db38`), most of their sounds; the gravity frames (`$gp − 0x6000`).


### 6.7 Scenes and movies (milestone H, 2026-10-21)

R&C2 numbers its scenes and movies together (`0x369fe8`): ids 0x20..=0xb0 are in-engine scenes, numbered from each
level's base (`$gp − 0x1c00`: Aranos 0x21..0x25 are its records 0..4); the others are movies (row = the id, or the
id less 0x91, in `RC2.HDR`'s movie table). `G/SCENEn.WAD` (`rc_formats::disc::scene`) holds 15 records of 0x14c bytes
(`{speech sectors[12], chunk sectors[71]}`), the chunks in R&C1's format (`rc_formats::scene` parses them as they
are). The game side is `rc_game::scene`: the queue `0x1bef40` (5 entries, a chain plays back to back, a skip drops
the rest), the modes `0x1a8f00` with their stack (`0x32c5f0` / `0x32c780`, the switch `0x32c858` at the top of each
frame with the blackout into a scene), the scene start `0x370e08`, tick `0x3716f8` (the world still runs: the moby
loop; Ratchet's update does nothing in mode 2) and end `0x3711e8`. The engine (`crate::play`) loads the scene,
makes the actors (each in the next sequence slot of its class, as the game appends the streamed sequence to the
class's table; class 0 is Ratchet's own moby), poses them, drives the scene camera (`rc_game::scene_player::
scene_camera`: the same SDK rotations), the fade, and plays movies (mode 1) with the front end's decoder in the HUD
layer's 512×416 slot.

Users: the scene trigger 0x0f6e (queues its list: Aranos's triggers play 0x22, 0x23 and the movie 0x16), 0x0f13
(scene props: spinning parts 0x0aa2 on the actors 2 and 3 of scenes 1 and 2), the arrival 0x1330 (scene 0x21 on a
first visit, then Ratchet's ship 0x0d54 with its nine parts flown along its path under the fly-by camera 2, then
Ratchet walked out).

**The level cameras** (`rc_game::cameras`, R&C1's level camera system evolved): the records of the gameplay's
cameras section with their pvar blocks (in the moby pvar table) as slots, the choice `0x2d8618` / `0x2d8408` (hooks,
activation kinds, priorities), the switch `0x2d8168` (cuts only: Aranos's cameras have blend kind 0), and the fly-by
(class 0x13: `0x42b0d8` / `0x42a660` / `0x42aa28`, armed by the gate 0x0ac7 and the arrival; the fade, the letterbox
`0x2e82c0`, the FOV keys, Ratchet held in state 0x6f). Aranos's nine cameras: 0x17 (the placed view at the start,
cuboid 0x1d), two fly-bys, the follow camera (class 0), 4 / 5 / 6 / 7 (first person, script, …, the Swingshot's) and
0x1b.

Not yet: the speech and music (milestone E), the subtitles (they show only with the subtitle option, off by default;
`0x2e7f80` draws them in a rounded box, `*` lines as captions), the placed view 0x17 (`0x42b950` / `0x42b428`: it
frames Ratchet from its record's place through the follow camera's setters `0x425e50..0x426110` while he is still
in its cuboid), first person (class 4), the blends `0x2d8ab0` / `0x2d9488`.

**The placed view** (class 0x17, analysed, not ported): its row `{0x17, 0x42b950, stub, stub, stub}`; the hook
always says "go on" but in play, while its pvar `+0x26` is 0, runs `0x42b428`: after the level's 4th frame, with
Ratchet in its region (Aranos: cuboid 0x1d; `0x42b1a8`: still — `|eff.xy|` ≤ 1/60 unless in the scripted walk
0x62 / 0x63 —, with `+0x34` no stick input, the follow camera not taken by another, `0x425e88`) it takes the follow
camera: on its first two ticks it snaps the follow record to its place (the record's position as the camera; the
horizontal distance to Ratchet `+0x38` as D+0x18c, the height `+0x3c` as D+0x190, the look height `+0x40` = `+0x3c −
+0x38 · tan(pitch +0x2c)` as D+0x110; the pivot, target, look point and offsets from them; the yaw toward `+0x30` when
it differs by over 2° as `+0x44`), then each tick sets the distance, the pivot and look heights (`0x425f18` with the
lock `0x426110`, `0x425f60`, `0x425f98`), the pitch `+0x28`° (`0x426030`), the leash off unless `+0x24`, and the yaw
target `+0x44` (`0x425fe8`: D+0x4c with the rates D+0x54 / +0x58, which the port's follow camera does not have yet).
Leaving the region or moving sets `+0x26` (off) until `0x42b9f0` resets it (the arrival 0x1330 does after the
fly-by). Aranos's record: at (254.3, 196.4, 54.2), pitch 0.24, yaw −2.5.

**The level's end** (ported 2026-10-21): Aranos's three scene triggers 0x0f6e play 0x22 (cuboid 0x27), the movie 0x16
(cuboid 0x2e) and 0x23 (cuboid 0x29, at the far end); the last gives item 1 and has its word [0xc] = 1, so its reward
(`0x40ffa0` states 3 / 4) calls `0x2f50f8(1, 1)`: `0x1b1940` (`$gp + 0x2950`) = Oozla and the level's main loop ends
(`$gp − 0x611c` = `0x1a8ed4`), then `0x367400` loads it. The port: `GameState::go_to_level`, the engine loads the
level through the shared `crate::level_switch` and carries the game-wide state (`GameState::enter_level`). Not
ported: the autosave `0x30d658` it asks for. The other ways to leave a level: the ship (below) and the menu.

**The ship** (`0x3012a0`): the level load places Ratchet's parked ship 0x0d54 at the level settings' `+0x3c`
(yaw `+0x48`) when x > 0, its update `0x361628`: Ratchet near it (or on it) gets the prompt 0x12ea and △ starts
the space travel (`0x1b1a60` = 1; play mode → mode 6 `0x364260`, which writes `0x1b1940`). Aranos's is at the
placeholder (20, 20, 20): the arrival's ship (0x1330) is its own and is hidden after the fly-by. The ship class's
own update `0x3fd5a0` is empty. Its texture is the paint (`rc_formats::disc::space`): one PIF of `SPACE.WAD` at the
header's `+0xad0` row `paint` (`0x2ff030`), given by `0x304660` to every class whose core entry has `+8 ≠ 0` (the
ship and its parts; their texture slots are 0xff). Not ported: the parked ship, its prompt, the space travel, the
hull mesh (`0x2fef68`, `SPACE.WAD` row `+0xab8`, a further 0x0d54 the load appends).

**Old-format classes** (`0x2ffe70`): a class with header byte 0xb set has R&C1's vertex table headers in its high
and low packets; the load converts them (u32s to u16s, the multiplier offset zeroed) and clears the byte. Oozla's
class 2700 is one; `rc_formats::moby` reads them as R&C1 packets without multipliers.

### 6.8 Sound (milestone E, 2026-10-21)

Going Commando's sound is R&C1's design, so it runs on R&C1's audio system (`rc_game::audio`: 989snd, the software
SPU2, the slots, the emitters, the music player) fed with R&C2's data (`crate::level_wad::audio`):

- **The bank**: the level file's part 1 is the 989snd `SBlk` bank R&C1's reader parses as it is (Aranos: 190
  sounds, 424 grains, 212 samples).
- **The defs** (`0x304660`, core header `+0x70`): the level defs and the remap as R&C1's; the per-class lists open
  with one more word (the loader writes it + 1 into the defs' bank handle), so def j takes entry j + 1
  (`sound_bank::parse_level_sounds_for`). The level defs: the moby-attached ones, then from the **footstep base**
  (`$gp − 0x60e4` = `0x1a8f0c`, the level settings' word after their variant blocks: `[0x17 + 8·n]`, n = `[0x1a]`,
  `[0x1f]` without blocks; 20 on Aranos) the footsteps, then the sound instances' and the objects' level sounds
  (`0x3607e8(i)` plays def i + the base; `0x360920` the footstep def `class·4 + foot·2 + variant` + the base).
- **The class sounds**: `0x3605f0(index, mode, moby)` is R&C1's `PlayClassSound` (the class's defs `+0x28`, count
  `+0x0d`); the moby host plays them into the audio system (`mobys::HostSound`), the tick's sound step runs
  with the mobys' positions as the slot owners. Ratchet's animation triggers (`0x313420`: R&C1's rule) play on his
  moby; his footsteps are `0x2a6420` (walk 3: 49.5 / 17.0, run 4: 12.5 / 1.0, back-pedal 0x14: 8.0 / 21.0; the
  walk's tick 22) with the ground's footstep class `0x18b2bd` (the probe `0x2b4a08`: the hit's sound bits, 0 on a
  moby).
- **The sound instances**: the gameplay's `sound_instances` section, R&C1's records and pvars.
- **The music** (`G/AUDIOn.WAD`): mono VAG streams in left / right pairs (`rc2_l00_main_al` / `_ar`, `_bl` /
  `_br`), pair k = stereo track k; the stream player gives a stereo track a second voice. R&C1's level start (track
  0 once, track 1 looping).
- **Scenes and movies**: a scene pauses the music and streams its record's English speech (`SCENEn.WAD`
  `speech[0]`), its end resumes the music 30 ticks later; an in-level movie stops the sounds, mixes its own audio
  and restarts the music at its end.

Not yet: the env sample points, the boots' footstep variant (`0x18b0b8`), the landing footsteps (`0x2a6180`), the
hero's other sounds (the states' own `0x3605f0` calls: hurt, the packs, the swing, the grind), the front end's
sounds and its movies' audio, the help voice.

### 6.9 The intro pass (2026-10-21, checked against the original's frames)

* **Ratchet's items beyond the hand.** `0x2afc68` makes, besides the hand item: slot 2 (`0x18b0e0`) the head item
  (item `0x1a73a0`, else 0x2f: the pilot's helmet, class 0x50a; not on armour 4 or with the cheat bodies), slot 1
  (`0x18b090` / `0x18b094`, a pair, item `0x1a739c` else 0x34), slot 3 (`0x18b130`, Clank or a pack, items 2 / 3),
  `0x18b134` (item 1) and three optional ones (`0x18b180..`). `0x2b09e0` places slot k's mobys at attachment
  `0x18aa70 + 0x40·row word 0`; the head item, the gloves (0xc / 0x11) and the 0x13 / 0x34 / 0x36 / 0x14 items are
  instead posed from Ratchet's joints by `0x2aa278(table, buffer, class, 0, ratchet)`: a keyframe of their own,
  joint 0 at rest, joint i the quaternion of Ratchet's `table[i]` (from `0x3181c0`, his decoded joint data), its
  scale where it is not 1 and its translation where it differs from the item's rest. Helmet: table `0x1a7d08`
  (+4: joints 8, 10), attachment 4 (joints 0..7). Ported: the helmet (`rc_game::hero::items::head_pass`,
  `Mobys::skin_head`); his joint modifiers' scale (the head's 0.875, `0x2d51f8` / `0x2ab498`) reaches it, the
  head look's rotation not yet. Not ported: slots 1 and 3, `0x18b134`, the optional slots, the gloves.
* **The GS stash.** The classes on the core's `moby_gs_stash_list` (s16 list, negative end: Clank 601, the packs
  607..609, the helmet 1290, 0xa73, 0xb08) keep their textures resident; their moby texture entries hold offsets
  from the stash address (the gs_ram table's entry past `gs_ram.count`; word 0x84 is the stash's entry count) into
  the gs_ram lump. `LevelCore::moby_stash` / `moby_stash_addr`, `texture::parse_textures`.
* **The ship's screen** (`0x2fd3d8` + `0x2e6350`): crate::world_screen (an offscreen camera onto a world quad).
* **The canopy** (`0x367b68`): a draw callback, not a moby: `rc_game::canopy`, drawn through crate::fx_draw like
  R&C1's ship glass (display-byte blend). The ship's parts ride the scene's ship actor at itself.
* **Occlusion mode** `0x1b19d0` (`0x2e54a0`): 0 all visible, 2 the grid (`0x2e4d88`), else kept; `0x3012a0` sets 0
  at every level load, the arrival 2 when its fly-by ends (`GameState::occlusion`). Nothing in level 0 sets 2 when
  the arrival scene was already seen (state 0 → 4), so then the grid stays off: as found, unverified in the
  original. R&C2's grid fallback has a fourth mode, 3: a mask by the camera's angle around the core's
  `occlusion_rad_offset` point (64 sectors), set by a camera mode in `0x2f2af0`; not ported.
* **Created mobys' metal pass**: through `moby_lod`'s shine gate (`0x305428` gives a class with metal packets +0x63
  = 0x18).
* Open from the user's list: the moby / tie lighting (the hull's sheen; milestone D), the background mountains
  in the arrival (not looked at yet).

## 7. Saves, the menus' pages (milestone I, started 2026-10-22)

**The save format** (`rc_formats::disc::save_game`, R&C1's with Going Commando's tables): descriptor tables at boot
`0x2560b8` (global, 34 chunks, section 0x1cf8 bytes) and `0x2562e8` (per level, 10 chunks, 0xcd0); a `save%d.bin` is
the 8-byte header, the global section and 28 level sections (0x183c0 bytes); R&C1's CRC-16 with the global section's
size as its limit (`0x29bca0`). The save blob is `MISC.WAD` part 2 (`[0]` icon.sys 0x3c4 bytes, `[2]`/`[3]`
static.ico, `[4]` the blank save). Chunk addresses: crate::save (rc-game) lists the typed ones.

**The card** (`rc_game::memcard`): machine `0x299cf0` (state `0x13953c`, substate `0x139540`, request
`0x139544`/`0x139548`, error `0x13954c`/`0x139550`, busy `0x139534`, checked `0x13955c`, save card `0x139528`;
the card record `0x1393e0`, 0xa0 per port, 2 ports polled); monitor `0x298f20` (status `0x1a7420`, flags
`0x1a7424`, handlers `0x256050`, defined in Ghidra as `mc_status_NN`; status 22 = the check after a create, 25 = an
auto-save `0x2997c8`); `memcard_Save` `0x29c1d0` (clock `0x1a7360`, map pack `0x297fa0`, previews, PrepData
`0x29bd88`, request 0xf). The card folder `BASCUS-97268RATCHET` (`0x299998`): icon.sys, static.ico, save0..3 and the
reservation (0xd804 bytes from RAM 0x1a79f8); the check wants exactly these 9 entries and 0x1da..0x1db KB. States
0x18..0x1b scan R&C1's saves (`BASCUS-97199RATCHET` etc., 5 slots) and set the bonus bits `0x1a7a28` from R&C1's
chunk 10. Front-end flags: `0x1a8c88` (the front end is up), `0x1a8c8c` (its card check page is running).

**The menus' pages** (mode 3, `0x2cac28(kind)`, the page from the kind in `0x2cb038`): kind 1 `0x259308`, 2
`0x25a170`, 4 `0x25e468`, 5 `0x261218` (sets `0x1a8c8c`), 6 the main menu `0x259438`, 8 `0x258ff8`, 10
`0x25a718`, 0xb = 5 with `0x2d6b00(0x1aa4d0)`. The current page `0x1f27d4`, the next `0x1f27d8`; `0x2d67a0(4, page)`
pushes a page. The New Game slot page `0x260ec8` (in the front end; `0x25da18` in a level) has the panel update
`0x2dd888`: slots 0..3 (`0x1a7c10`, Up / Down, sounds 3 / 4), an empty slot (preview level −1) is taken at once
(`0x2df668`), a used one asks first (page `0x25e850`, the slot in `0x25e9c0`); the card status picks the dialog
(`0x2b58f8(4, …)`). Pad bits: `0x138344` pressed, `0x138334` repeat.

**The page loop** (mode 3 sub-state `0x1f27c0`: 1 set up `0x2cb038`, 3 fade (darken 0x38) into the next page, 4 run
`0x2cbab8`, 5 leave `0x2cbd68`). A page: 14 panel rectangles `{x, y, w, h}` (0xe0 bytes), `+0xe0` the parent page
(△), `+0xe8` the focused panel, `+0xec` 14 panel pointers, `+0x128` a pending focus. A panel (0x50 bytes):
`{update, draw, open, close, flags (+0x30), …, +0x40 the selection, +0x48 its buffer, +0x4c its state}`. Running a
page: on entry every panel's open(p, 0); each frame every panel's update (non-zero ends the menu: exit code
`0x1f27dc` = 1, or 2 for kind 4); a pending focus closes the old focused panel (p, 1) and opens the new; a next page
(`0x1f27d8`) plays sound 0x11 (the same page) or 0x12, closes every panel and fades. Leaving picks the next mode by
the exit code (`0x2b58f8(mode, …)`: 4 the card dialog, 6 a level load).

**Pages of the front end**: New Game `0x260ec8` (panels: title `0x1ab098`, slots `0x260ff8`, `0x25dc78`, `0x261048`,
footer `0x25dd70`), Load `0x260d98` (title `0x1ab068`, slots `0x25dd20`, `0x25dc78`, footer). The slot panels share
draw `0x2de768`, open `0x2dd7e8` (`0x2df368(0)` its buffer, state 0), close `0x2dd858`; the in-level save page's
slot panel is `0x2dd888` (page `0x25e468`, kind 4). Pad bits (`0x138344` pressed, `0x138334` repeat when the panel's
flag 1): 0x10 △, 0x20 ○, 0x40 ✕, 0x80 □, 0x100 select, 0x800 start, 0x1000 up, 0x4000 down.
* **New Game** `0x2de168`: the selected slot `0x1a7c10` (0..3, shared by the slot pages); when the card is idle
  (state < 3, no request, 11 idle frames `0x1f2924`, a PS2 card): Up / Down (sound `0x2dffa0(1, 0x11)`); ✕ on an empty
  slot (preview level −1) → state 1; on a used one → the confirm page `0x2610e8` (`0x1f28a4` = 1, the slot in
  `0x25e9c0`), whose yes brings the panel back with state 1; ○ → continue without saving (`0x299bf8`, flags ~6 | 0x20,
  `0x2f7328`); △ → the parent page; Start / Select → leave (not in the front end's card check). State 1: the new game's
  whole save into the slot (`0x2df710(buffer, slot)`), waiting (`0x1f28f8`), checked, the message 0x2c92; done: no
  error → the slot's preview, `0x2f7328` (start the game), `0x1a8c88` = 0; an error → flags 0x80 and the card dialog
  (`0x2b58f8(4, 1, 1 or 5, page)`). The card status 9 (present, not checked) pushes the card check page (`0x2d67a0(4,
  0x261218 / 0x260ec8 / 0x25da18)`); a status other than 1 / 0x10 opens the card dialog or goes back. Flag 0x2000 =
  the challenge-mode variant (`0x2df7a0`, the confirm page `0x261398`).
* **Load** `0x2ddd30`: ✕ on a used slot: sound 4, save card 0, the slot, `0x29bb90(0x1f29f0)`, request 0xd, waiting,
  the message 0x2c93; done: no error → checked, `0x2e5698`, then the level in the save (`0x2b58f8(6, 2, 5, level)`), or
  `0x2f7328` (a new game) when the level is below 1 and level 0 not visited; an error → flags 0x100, the dialog. ✕ on an
  empty slot: sound 5. The card status 9 pushes `0x2d67a0(5, 0x260d98 / 0x25db48)`.
