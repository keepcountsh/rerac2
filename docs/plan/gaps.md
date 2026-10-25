# R&C2 gap register

One list of every known gap in the Going Commando port, **by missing shared system**. First harvest 2026-10-22, from
the class census (`class_census.md`: the ranking of missing systems by the instances they block), the "not yet"
notes of `bringup.md`, and the module docs and comments of `crates/rc-game`, `crates/rc-engine/src/{front_end,level_wad,mobys,play}.rs` and
`crates/rc-formats/src/disc` ("not ported", "not yet", "not drawn", "later"). The module docs stay the detailed record;
this page is the index. The systems R&C1's port has and this one does not are mapped to their R&C1 code in
[`porting_from_rac1.md`](porting_from_rac1.md).

## How to use it

- **Gaps are systems.** A row names the shared system that is missing (with its addresses, L00 = Aranos's overlay)
  and lists the levels, objects and items that need it as **consumers**. A row of its own only for unique content.
- **Census numbers** are the units / created instances whose verdict is "missing" because of the system (a unit can
  count under several), and in brackets those for which it is the only missing system. Re-run the census after a
  system lands and update the numbers.
- **Keep it current.** Add what is left unfinished (a consumer on an existing row, or a new row with the next free ID of
  its section); strike what closes (`~~…~~`, "closed YYYY-MM-DD, <commit>"). Never renumber or reuse an ID.
- **Don't guess.** What cannot be placed goes to "Needs triage" with its source.

**ID scheme.** `G-<SYS>-NNN`: `REN` rendering and effects, `PRT` particles, `CAM` cameras, `CLS` moby classes and the
moby system, `LVL` levels, `HERO` Ratchet and Clank, `WPN` weapons, gadgets and pickups, `ENM` enemies, `AUD` audio,
`UI` HUD, menus, help and input, `CUT` scenes, `SAV` saves, `TRI` needs triage.

**Columns.** *System*: what it is in the game. *Consumers*. *Census*: units / created (alone). *Sources*: `file:line`
under `crates/` shortened to `<crate>/src/...`, or the doc §. *Seen*: whether a player sees, hears or feels it.

## Rendering and effects (REN)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-REN-001 | **Moby shadow-volume pass** `0x31da58` (`+0x70` / `+0x74`; not the blob shadows, crate::shadows, which landed) | enemies 0x0b0a / 0x0b0b; 80 class-port units on every level (census 2026-10-25) | 80 / 2,512 | rc-game/src/moby/classes/enemies.rs:20, 340; bringup.md §6 | yes |
| G-REN-002 | **Breakable effects**: shards `0x32b580`, burst `0x2ecfb0`, debris `0x3210a8` / `0x327498`, death bursts `0x3f8348`, thrown chunks `0x327648` (mobys 0x43e, callback `0x39c6b8`) | the props that break (rc-game props.rs:155, 175), the enemies' deaths, 67 units | 67 / 2,312 (45 / 1,465) | props.rs:155, 175; enemies.rs:340, 842; census | yes |
| G-REN-003 | **The first draw-callback list** `0x2e7718` (0x40 entries `0x1be040` / `0x1be140`, run by `0x2e7768` before the particles `0x3311e0`). The port has the second list (`0x2e7920`, crate::fx); each user's callback is its own code. On Aranos only the arrival ship's draw (`0x362b40`, done by the engine) and class 0xd0's ribbon (`0x39ac30`, not placed) use it (2026-10-22: Aranos's 0x0ce9 / 0x0340 were a census keying collision; they use the second list) | 152 units on the other levels (the registrar `H:582266980bbdcff0`, L01 `0x2dce00`) | partly: 72 / 485 | census; rc-game/src/fx.rs | yes |
| G-REN-004 | **Effect quads**: the beam strip `0x2edd50`, trails `0x323c90` (the wrench's `+0x70`, the bites' `0x3dbde8` / `0x3dbe88` / `0x3dc100`), 0x0b0a's emerge flash `0x3d8920` → `0x2ee190` (FX 0x3b), 0x0b0b's wire `0x3db790`, the group branch of `0x3dc220` | wrench, 0x0b0a, 0x0b0b, 4 units | 4 / 400 (2 / 21) | fx.rs:13; wrench.rs:15; enemies.rs:340, 842 | yes |
| G-REN-005 | **Moby colour take-over and flash**: `0x31fa50` / `0x3241d8`, the hit flash `0x3201e8` | shards (shards.rs:26), enemies, 11 units | 11 / 336 (5 / 25) | shards.rs:26; enemies.rs:20 | yes |
| G-REN-006 | **Lighting**: point lights — done: the bank `0x1c6ec0` (`0x2fda20` / `0x2fdb60` / `0x2fdc48`, rc-game/src/point_lights.rs), the light moby 0x27f (classes/light.rs: the Gravity Bomb), the Lancer's flash light (lancer.rs, `0x3a8b68`), the blasts' flash ball 0x70 (`0x38e5a8` / `0x38e768`, classes/flash.rs — a mesh, not a point light) and their camera shake (blast.rs); left: the enemies' lighting `0x31a8e4`, the moby lights and colours `0x3132e0` (bolts), tie and moby lighting (milestone D) | enemies, bolts, 3 units | 3 / 56 (3 / 56) | enemies.rs:20; bolts.rs:7; moby.rs:29; bringup.md §2 D | yes |
| G-REN-007 | **Ship and space**: the ship's engine glow and draw callback, the parked ship, the hull mesh (`0x2fef68`, `SPACE.WAD` `+0xab8`) | Aranos's ship 0x0d54 | — | props.rs:526; bringup.md §6 | yes |
| G-REN-008 | **Occlusion offset point** (`occlusion_rad_offset`, set by a camera mode in `0x2f2af0`) | — | — | bringup.md §6 | minor |
| G-REN-009 | **Water surfaces and the sea** (Oozla's swamp water 0x0a17's draw `0x3bc3c0` ported 2026-10-25: R&C1's ocean 1111 `0x30a908` in R&C2, oozla::draw_0a17): the strips' scroll, wobble and bob, the ripple patch (`0x37dc10`, its height through `0x31d7a0`; R&C2's own code: R&C1's ripple simulation scores 0.04–0.13 against it), the water plane (`0x1b25bc..cc`), the sea | the levels with water (to map); the underwater test's water height (rc_game::fog) | — | R&C1's sea draws through `DrawEnvOverlayMesh`, whose R&C2 copy L00 `0x2ec618` the port has (moby/classes/nanotech.rs); R&C1's sea classes have no R&C2 counterpart (2026-10-25) | yes |
| G-REN-010 | ~~**Fog zones and the underwater view**~~ closed 2026-10-25: the light regions' fog `0x2dbc20`, the underwater test `0x2d9a58`, `UpdateFog` `0x2e4af0` and the tint (rc_game::fog; Ratchet's writes of `0x1b9000`). Left: the camera's mode-6 clear, the water heights of the ripple patch and the water plane (`0x31d7a0`, G-REN-009) | levels 00, 05, 08, 12, 13, 19 (fog regions); every level with water | — | rc-game/src/fog.rs | yes |
| G-REN-011 | **Sky stars**: the star sprites of the sky dispatch | the levels with stars (to map) | — | R&C2's code not mapped yet; R&C1's port: `sky_stars.rs` (porting_from_rac1.md) | yes |
| G-REN-012 | **After-images**: translucent copies of a moby where it was a few ticks ago | the users (to map) | — | R&C2's code not mapped yet; R&C1's port: `afterimage.rs`, `afterimage_render.rs` (porting_from_rac1.md) | yes |
| G-REN-013 | **The explosion moby 0x986** (L25 `0x3cbfe8(at, 0, kind)`: at most 5 alive, the oldest recycled) | Wupash: the missiles' hits and the ship's death (the ground blast `0x4184c8` stands in) | — | rc-game/src/ship.rs (`update_mission`, the missile's hit) | yes |
| G-REN-014 | **The Tractor Beam's rope** `0x38dd68` (20 points, springy and waving) | the Tractor Beam (a straight line with a slight wave stands in) | — | rc-game/src/moby/classes/tractor.rs:22 | yes |
| G-REN-015 | **Overlay tables read at Aranos's addresses only**: the nanotech cluster's glow tables (`GlowTables`, gated on level 0) | the nanotech cluster 0x326 on every level with nanotech crates | — (lint) | rc-engine/src/play.rs:330; census `aranos_only.tsv` | yes |

## Particles (PRT)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-PRT-001 | **Particle types beyond 0x17 / 0x2c / 0x30 / 0x55 / 0x59** (0x30, R&C1's type 48, closed 2026-10-22: particles.rs `drift`): type 2 `0x3327e8`, 0x3c `0x33b5e0`, sparks (`0x38ce80`, `0x38cc88`, `0x33a058`, `0x336148`, `0x3dbc20`), dust `0x334528`, the bolt sparkle `0x33a6a0`, bubbles and splashes (`0x3378b0`, `0x2aac58`, `0x37d5a0`), smoke | wrench, grind, chute, zip line, crates, bolts, water, burns, 53 units | 53 / 1,757 (30 / 1,047) | particles.rs:15; wrench.rs:15; grind.rs:22, 398; misc.rs:15; crate_.rs:13; bolt.rs:7; water.rs:19; damage.rs:12; props.rs:329 | yes |
| G-PRT-002 | The gravity boots' bend of particle positions `0x380e08` (`+0x1c` = 2) | — | — | particles.rs:15 | minor |
| G-PRT-003 | **The laser end's sparks** `0x346ee8` | Wupash: the ship's lasers (`0x4134b0`; crate::particles::spark stands in) | — | rc-game/src/ship.rs (`laser_burst`) | yes |

## Cameras (CAM)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-CAM-001 | **Script camera** (class 5): position `0x4287b8`, look `0x428810`, set-up and switch `0x428880`, update `0x4289f0` | 25 units on 22 levels | 25 / 561 (13 / 64) | census; cameras.rs:31 | yes |
| G-CAM-002 | **Other camera classes and blends**: 4 first person (mode 4 / 0x51), 6, 7 (Swingshot), 0x17 the placed view (`0x42b950` / `0x42b428`, the follow camera's setters `0x425e50..0x426110`), 0x1b; the blends `0x2d8ab0` / `0x2d9488`; the fly-by's look at a moby | Aranos's placed view (cuboid 0x1d), the Swingshot, first-person aim | — | cameras.rs:31, 298; bringup.md §6 | yes |
| G-CAM-003 | **Follow camera leftovers**: the collision grid (`0x427758`, `0x4219a0`), the moby nudges (0x8ba, 0xd07, 0x10d0, 0x477, 0xb), the focus moby `0x1b8e84` / `0x18b270`, the stick auto-centre, the tweak requests `0x2d7568`, the body cameras (Clank, Giant Clank), the water settle, the knock-down view (state 0x74) | — | — | camera.rs:31, 539, 587 | yes |

## Moby system and classes (CLS)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-CLS-001 | **The unported classes** (the census): 1,052 class-levels, 662 units; cheap first | Aranos ~~0x0c5c, 0x0cda, 0x0ce9, 0x0340~~ (closed 2026-10-22: props.rs `update_0c5c` / `update_0cda` / `update_0340`, classes/tie_lights.rs; Aranos has no unported class left); every other level | 190 units / 2,324 cheap | class_census.md | yes |
| G-CLS-002 | **Moby groups** (`0x1fa8a0[n]`): the members' byte `+0xbc` `0x31c970`, the group walk `0x31cac0` | 30 units | partly: 15 / 139 | census | yes |
| G-CLS-003 | **The sequence-time test** `0x323bf8` (over `0x311880`, `0x323b78`) | 33 units | partly: 3 / 58 | census | yes |
| G-CLS-004 | **The static spawn's conditions and the moby lifecycle**: missions, the save bits `0x19b2e8` / `0x19b4a8`, the bolt / XP tallies `0x1c5400` / `0x1c54c0`, the ground snap of rooted instances, the deletion and the created mobys' pvar pool | every level | — | moby.rs:29; bringup.md §2 C | yes |
| G-CLS-005 | **The line test** `0x31c618` (through `0x2de878` / `0x2df8d8`); the see-through classes `0x381350` (list `0x2318d0`) | 7 units, aim | partly: 1 / 4 | census; aim.rs:10 | yes |
| G-CLS-006 | **Ratchet / ship queries**: direction `0x326448`, distance `0x326820` (the ship mode `0x1a8ff4`, `0x1a9000`) | 20 units | partly: 3 / 27 | census | yes |
| G-CLS-007 | **Attachments leftovers**: the carrier's joint frames (`0x3245b0`), the offset moby `+0x40`, the shrubs' sphere and scale, the ties' update `0x36c068` | carriers | — | attach.rs:8 | minor |
| G-CLS-008 | **Talk records** `0x139a34 + 0x10·i` through the level's talk list (`0x1c5d20` from `0x28ead0[level]`; marked by `0x325980(0x3259d8(moby))`) | Oozla's Tractor Beam seller 0x0ace; the missions' conditions 4 / 5 (crate::travel::missions) | — | rc-game/src/moby/classes/oozla.rs (`update_0ace`); rc-game/src/travel/missions.rs | no |

## Levels (LVL)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-LVL-001 | **Level variant regions** `0x321a78`: the gameplay header's boxes (`0x1c92e0` / `0x1c9300`, count `0x1c92cc`) against the variant `0x1c92c0` | 14 units | 14 / 1,186 (7 / 841) | census; game.rs (the variant) | yes |
| G-LVL-002 | **Level streaming and change steps** `0x304200` | 16 units | partly | census | yes |
| G-LVL-003 | **Gravity frames** (`$gp − 0x6000`, `0x18c0e3`, `0x1a8ff0`): R&C2's gravity surfaces and walkways | Ratchet (physics, walk, melee, ledge, posture), aim, AI, shards, wrench, particles | — | physics.rs:26, 404; ai.rs:30; aim.rs:10; shards.rs:26; wrench.rs:15 | yes |
| G-LVL-004 | **Sliding floors and ice** (the drag `0x18a890`, `0x2b96a0`) | Ratchet's walk, jump, melee | — | physics.rs:467; walk.rs:17; jump.rs:28; melee.rs:12 | yes |
| G-LVL-005 | **Platform carry** (`0x189f20`; a carried moby's velocity `0x3222a8`) | Ratchet, the nanotech crate, bolts on platforms, 2 units | 2 / 50 | physics.rs:26, 685; crate_.rs:160; bolt.rs:7 | yes |
| G-LVL-006 | **Space travel** and the level end's autosave `0x30d658` | Aranos's last trigger (to Oozla) | — | props.rs:1239; game.rs:146; bringup.md §6 | yes |
| G-LVL-007 | **Level chunks**: the chunk switch `0x2f9bd8` (L01): chunk k's data (header `+0x30 + 8k`, its terrain and tables) and sound bank (`+0x48 + 8k`, bank k + 1, `0x323ca0`). The port loads chunk 0's bank only (level_wad::chunk_sounds), so banks 2 / 3's sounds play nothing | levels 01, 02, 04, 07, 08, 11, 19, 20 (chunked); 33 units on banks 2 / 3 | 33 / 430 (`B:2` / `B:3`) | rc-engine/src/level_wad.rs (chunk_sounds); census 2026-10-25 | yes |

## Ratchet and Clank (HERO)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-HERO-001 | **Water**: the probe `0x2aca80`, the swim, the liquid heights `0x18a150` / `0x18a154` (states 0x7c, 0x65, the pit), the water hurts 0x72 / 0x73, the drowning bubbles (class 0x52a) | Ratchet, the camera's water settle | — | hero.rs:16; surface.rs:10; water.rs:19; damage.rs:12 | yes |
| G-HERO-002 | **Body lean, landing effects and camera jolt** (`0x2b7c88`, `0x2b8dc0`, `0x2a6188`, `0x2d35e8`), the teeter (`0x2ab498`), the arms at a wall `0x2ab250`, the model's tilt probes `0x2b8f50` | Ratchet | — | jump.rs:28; air.rs:11; walk.rs:17; posture.rs:18; wall.rs:14; physics.rs:582 | yes |
| G-HERO-003 | **Bodies and vehicles** (`0x18c0d4` ≠ 0) | Ratchet's states, surfaces | — | states.rs:12; jump.rs:28; surface.rs:10 | yes |
| G-HERO-004 | **Clank**: his own animation `0x323d10` / `0x323e58`, his hurts 0x42 / 0x43, the idle look-around, the punch's sounds | Clank, the packs, 3 units | 3 / 5 | pack.rs:11; clank.rs:10; damage.rs:12 | yes |
| G-HERO-005 | **Stats counters** (`0x18c3d8`, `0x18c750`, `0x18c75c`, `0x18c600`, swim, swing) | melee, swing, water, script | — | melee.rs:12; swing.rs:26; water.rs:19 | no |
| G-HERO-006 | **Rail and ledge leftovers**: the rail-jump targets (`0x18a4ac` / `0x18a4b0` / `0x18a440`), the hop's side, the ledge's carrying moby, the stuck-in-air jump `0x18a022`, the scripted timers `0x18a012` / `0x18a032` | Ratchet | — | grind.rs:22; jump.rs:28, 248; ledge.rs:19; air.rs:11 | yes |
| G-HERO-007 | **Pose leftovers**: the idle fidgets (`0x2ca788`), the back items' poses, the swap face `0x18a01c`, the tuck in two falls `0x2cb650`, the state-change hook `0x2af450` | Ratchet | — | ground.rs:9; layers.rs:25; states.rs:12 | minor |
| G-HERO-008 | **Remote control** (Ratchet's state 0x1c steering a remote moby, class 0xac): the range check of 0x0340 (`0x3a2030`: its regions `{path / −1, low, high, cuboid}`, `0x31cf68`), the screen static `0x3a1e28` (texture 0x1e tiles, fading in over 90 ticks) and the give-up `0x392728` | 0x0340 on 18 levels (Aranos's: its live part only) | — | props.rs `update_0340` | yes |

## Weapons, gadgets and pickups (WPN)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-WPN-001 | **The hand items beyond the wrench, Swingshot and Lancer**: the other slots (`0x18b130`), the quick select and the requested item (`0x18c0e8`, `0x3227d8`), the L2 + R2 pull, the firing arm's layers `0x2af5e0`, the weapon stances (`0x2cb1c8`, `0x18c0da`), the upgraded Lancer, the first-person aim (`0x18b320`, `0x380ef8`), the lock-on `0x2cb2e0` | every weapon and gadget | — | items.rs:11, 303; layers.rs:25; lancer.rs:20; shot.rs:20; bringup.md §6.4 | yes |
| G-WPN-002 | **Reward items**: equipping `0x325270`, the item 0x18 `0x2f51a0` | 0x0f6e's reward, Aranos's ship, 15 units | 15 / 60 (11 / 16) | props.rs:526, 1239; census | yes |
| G-WPN-003 | **Bolt leftovers**: the XP of a drop (`+0xb6`, `0x18d198` / `0x18d208`), the challenge multiplier `0x1a7a10`, the magnet `0x3831a8`, the spin, the settle arc `0x382e20`, the per-level tally | bolts | — | bolts.rs:7; bolt.rs:7, 65 | yes |
| G-WPN-004 | **Crate leftovers**: the stack's fall `0x39dfd8`, the TNT crate 0x1f9, the ammo and nanotech pickups (`0x31aa10` / `0x3d2d30`), the respawn's stacking search `0x31c9d8`, the out-of-sight test `0x2f0080` | crates | — | crate_.rs:13, 86, 179 | yes |
| G-WPN-005 | **The shot's reaction** on a hit moby `0x4126b8` (`+0x98`) and the gravity boots' bend (`0x37f538` / `0x37f618`) | the Lancer's shot | — | shot.rs:20 | yes |
| G-WPN-006 | **Targeting**: the target list, the aim searches, the throw's landing preview, the ground reticle and markers | the thrown and aimed items, the HUD | — | R&C2's code not mapped yet; R&C1's port: `targeting.rs`, `reticle_render.rs`, `marker_render.rs` (porting_from_rac1.md) | yes |
| G-WPN-007 | **Inventory and equipment**: what Ratchet owns, each slot's equipped item, the rules that change them | the hand items, the vendor, the saves | — | R&C2's code not mapped yet; R&C1's port: `inventory.rs` (porting_from_rac1.md) | yes |
| G-WPN-008 | **The Tractor Beam's target** `0x18a3e0` (its writer not found in L01; the port picks the nearest box of the target list ahead) | the Tractor Beam and its boxes | — | rc-game/src/moby/classes/tractor.rs:9 | yes |

## Enemies (ENM)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-ENM-001 | **AI leftovers**: the joint aims (`0x32db38`, `0x324208`, `0x327308`, records `+0x3d0` / `+0x450`), the dangerous floor `0x322058`, helpers and decoys as targets, the scene's hiding (`0x1a8f00` = 2) | 0x0b0a, 0x0b0b, every creature; Oozla's Tractor Beam seller 0x0ace (its head and body turning to Ratchet, L01 `0x31a0e0`) | — | ai.rs:30; enemies.rs:340, 842 | yes |
| G-ENM-002 | **The space fighters' shot pool** `0x2218c0` (0x80 bytes each, `0x375198` / `0x375410`; a 0xffa moby stands in) and the crosshair's draw `0x4383d8` (one sprite stands in) | Wupash | — | rc-game/src/ship.rs | yes |

## Audio (AUD)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-AUD-001 | **The level sound table** `0x360750` (`0x1b196c`, 0x20 each, through `0x3603c8`) | 30 units | partly: 14 / 34 | census | yes |
| G-AUD-002 | **Moby and hero sounds**: most `0x3605f0` calls (enemies, crates' other classes `0x360698`), the hero's (states, packs `0x2b9cb8` / `0x3602f0`, swing, grind, hurt, voice `0x2d6cc8`), the boots' footsteps (`0x18b0b8`), the landing footsteps `0x2a6180`, the env sample points | every class, Ratchet | — | enemies.rs:20; crate_.rs:13, 110; play.rs:345; level_wad.rs:280; bringup.md §6 | yes |
| G-AUD-003 | **Speech**: the scene's speech start `0x32ea38`, the help voice (`0x1a63cc`, `0x14df60`) | scenes, help, 10 units | 10 / 23 (3 / 3) | scene.rs:27; help.rs:18 | yes |
| G-AUD-004 | **Music control**: the scenes' pause and resume, the ship's `0x3607e8(0x17)`, the front end's sounds and movie audio | scenes, ship, front end | — | scene.rs:27; props.rs:526; bringup.md §5 | yes |

## HUD, menus, help and input (UI)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-UI-001 | **The message / prompt slot** `0x327a28` / `0x327ae0` (`0x1a8fb0` / `0x1a8fb4`), with the help log `0x2f6538`, the slot's time and level bits (`0x18c95c` / `0x18c960`) | help, 38 units | 38 / 249 (20 / 78) | help.rs:18; census | yes |
| G-UI-002 | **HUD leftovers**: the weapon's experience bar (sprite 5, `0x139868`), the low-ammo pulse, the quick select ring, the vendor, PAL's sizes | HUD | — | hud.rs:32 | yes |
| G-UI-003 | **Help leftovers**: the multi-page help `0x27d800`, the shake offset `0x27d808`, PAL's height | help | — | help.rs:18 | minor |
| G-UI-004 | **Subtitles** in other languages and the attract-mode list | scenes | — | scene.rs:27; bringup.md §6 | yes |
| G-UI-005 | **Pad rumble** `0x2d6d60` | Lancer, 1 unit | 1 / 15 (1 / 15) | lancer.rs:20 | yes |
| G-UI-006 | **Pause menu and in-level menus**: the mode system, the page menus, Options, the menu text and models | every level | — | R&C2's code not mapped yet; R&C1's port: `menus/`, `menu_render.rs`, `menu_models.rs`, `text_render.rs` (porting_from_rac1.md) | yes |
| G-UI-007 | **The map**: the map files, the fog mask and its writer, the saved mask | the pause menu | — | R&C2's code not mapped yet; R&C1's port: `map.rs`, `map/` (porting_from_rac1.md) | yes |
| G-UI-008 | **Cheats**: the cheat bytes, the in-game entry, the mirror mode | the menus, the camera | — | R&C2's code not mapped yet; R&C1's port: `cheats.rs`, `mirror_render.rs` (porting_from_rac1.md) | minor |

## Scenes (CUT)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-CUT-001 | **Scene leftovers**: the end's teleport (`0x1bca86`) and callback (`0x1a8fd0`), the mirror cheat's camera, the actors' lighting and shadows, the modes 3..9 | Aranos's scenes | — | scene.rs:27 | yes |
| G-CUT-002 | **Movie mode in a level and the credits slideshow** (game modes 1 and 7) | the classes and scenes that start movies, the ending | — | R&C2's code not mapped yet; R&C1's port: `movie_player.rs`, `slideshow.rs`, `movie_render.rs`, `media_render.rs` (porting_from_rac1.md) | yes |

## Saves (SAV)

| ID | System | Consumers | Census | Sources | Seen |
|---|---|---|---|---|---|
| G-SAV-001 | **The save-slot page** between New Game and the level, the autosave `0x30d658`, the level name | the front end, the level end | — | front_end.rs:314; game.rs:146; props.rs:526, 1239 | yes |
| G-SAV-002 | **Memory card and the persistent game state**: the card driver, the save block, the save paths | the save-slot page, the autosave, the Options | — | R&C2's code not mapped yet; R&C1's port: `memcard.rs`, `game_state.rs`, `saves.rs` (porting_from_rac1.md) | yes |

## Needs triage

| ID | Item | Source |
|---|---|---|
| G-TRI-001 | `0x2e6330` → `0x30ef70` → `0x42f608` on `0x1a9054 + 0x36f28` (9 units / 387): a level-code module behind a pointer; also called by the message slot `0x327a28` | census |
| G-TRI-002 | The 417 untagged census keys; the heaviest are listed in `class_census.md` ("Untagged shared functions") | census |
| G-TRI-003 | The level-08 family (H:b475d9038b64efbf, H:7ea40223d9c5f399, H:45a2f8ef05e2e2a9, H:20d7834dfd80b5e8, H:17e83ec9eed89fc4: 3 units / 575) | census |
| G-TRI-004 | The planet page's missions tables (`0x281560`) are read from Aranos's overlay on every level: confirm the table is the same in every overlay, or relocate it | travel's planet page | — (lint) | rc-engine/src/play.rs:1276; census `aranos_only.tsv` | yes |
