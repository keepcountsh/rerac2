# Porting from R&C1

ReRAC2 started inside the R&C1 port (ReRAC, GitHub `re-rac/rerac`) and split from it on 2026-10-22. On the split,
only the code Going Commando runs today stayed: R&C1's own game (its moby runtime, hero, tick and game state), its
per-class behaviours, and the systems built on them left the build. R&C1's systems are bound to R&C1's runtime, so a
system comes back by porting it onto R&C2's (crates/rc-game), with R&C2's own code as the source and R&C1's port as
the map, the way the hero, the camera and the particles were done. This page says where each of those systems is.

**Where to read it.** Every path below is the same in two places:

- the R&C1 repository (`re-rac/rerac`, branch `main`): the current version, with R&C1's specs in `docs/plan/`;
- this repository's history at `10b58e0` (the last commit with both games): `git show 10b58e0:<path>`, or
  `git ls-tree -r --name-only 10b58e0 -- <dir>`. The full format readers (`crates/rc-formats/src`), extractor
  (`crates/rc-extract`) and trace tool (`tools/trace`) of that commit are there too.

Doc comments in this repository cite R&C1 code as `rerac:rc_game::<path>` (`crates/rc-game/src/<path>.rs` in R&C1).

**When one comes back**: port it under R&C2's crates (no R&C1 runtime types), strike or update its gap row in
[the gap register](rac2_gaps.md), and drop its line here.

## Systems not in this repository

Game code is under `crates/rc-game/src/`, engine code under `crates/rc-engine/src/`. *Gap* is the gap-register row
the system belongs to.

| System | R&C1 game code | R&C1 engine code | R&C1 spec | Gap |
|---|---|---|---|---|
| Water: strips, wobble, bob, the ripple patch; the sea | `water.rs`, `water/` | `water_render.rs`, `sea_render.rs` | `world_animation.md` | G-REN-009 |
| Fog zones, the underwater test, the fog selection of `UpdateFog` | `fog_zones.rs` | `fog_state.rs` | `world_animation.md` §5–§6, `game_camera_fog.md` | G-REN-010 |
| Moby shadows (game side: directions, probes, ground slabs) | `shadows.rs`, `shadows/` | `shadow_render.rs` | `shadows.md` | G-REN-001 |
| Static lighting of tfrags, ties and shrubs | — | `tfrag_light.rs`, `tie_light.rs`, `shrub_light.rs` (readers: rc-formats `tfrag_light.rs`, `tie_light.rs`, `shrub_light.rs`) | `tfrag_lighting.md`, `shrub_lighting.md` | G-REN-006 |
| Sky stars | `sky_stars.rs` | `sky_stars.rs` | `sky_render_notes.md`, `particles.md` §8 | G-REN-011 |
| After-images ("speed blur") | `afterimage.rs` | `afterimage_render.rs` | — | G-REN-012 |
| Full-screen tints from draw callbacks | (class draw callbacks) | `screen_tint.rs` | — | G-REN-003 |
| Weapon and gadget effects: Tesla arcs, Walloper, Visibomb view, Suck Cannon / Taunter / Morph-o-Ray, thrusters | `hero/` (weapons), class files | `tesla_render.rs`, `walloper_render.rs`, `visibomb_view.rs`, `reactive_render.rs`, `thruster_render.rs` | `gadgets.md` | G-WPN-001 |
| Targeting: target list, aim searches, throw preview, ground reticle, markers | `targeting.rs` | `reticle_render.rs`, `marker_render.rs` | `hero_gameplay.md` §8 | G-WPN-006 |
| Inventory and equipment (owned items, equipped slots, the rules) | `inventory.rs` | — | `gadgets.md` | G-WPN-007 |
| Hand-item attachment, spawned mobys | — | `moby_attach.rs`, `moby_spawn.rs` | `moby_animation.md` | G-CLS-007 |
| Particle emitter types beyond R&C2's | `particles.rs`, `particles/` | `particle_render.rs` (emitters) | `particles.md` | G-PRT-001 |
| Creature AI framework (legs, reactions, effects) | `moby_update/creature.rs`, `moby_update/creature/` | — | `creatures.md` | G-ENM-001 |
| Moby services: hits, sounds, triggers and platforms, story stores, class swap, scheduler | `moby_update/{services,triggers,story,class_swap,scheduler,visit,manip,anim_sound}.rs` | — | `moby_update_catalogue.md` §7, `triggers.md`, `level_scripting.md` | G-CLS-004 |
| The "use" system: context prompt, NPC talk tables, hand-offs; the Gadgetron vendor | `moby_update/interact.rs`, `menus/vendor` | `interact_render.rs`, `vendor_render.rs` | `interaction.md` | G-UI-001, G-UI-002 |
| Cinematics: the classes' calls into camera, hero and players; the scene player and actors | `cinematic.rs`, `scene_player.rs`, `scene_actors.rs` | `scene_render.rs` | `cutscenes.md`, `cutscenes_transitions.md` | G-CUT-001 |
| Movie mode in a level, the credits slideshow, front-end media | `movie_player.rs`, `slideshow.rs` | `movie_render.rs`, `media_render.rs` | `progression.md` (media) | G-CUT-002 |
| Pause menu and in-level menus (mode system, quick select, page menus, Options), menu text and models | `menus/`, `help.rs` | `menu_render.rs`, `menu_models.rs`, `hud_images.rs`, `text_render.rs`, `screen_canvas.rs` | `menus.md`, `hud_text.md` | G-UI-006 |
| The map (Map-o-Matic, fog mask) | `map.rs`, `map/` | (menu_render) | `menus.md` §13 | G-UI-007 |
| Cheats, the mirror mode | `cheats.rs` | `mirror_render.rs` | `progression.md` (media) | G-UI-008 |
| Memory card driver, the persistent game state, save paths | `memcard.rs`, `game_state.rs` | `saves.rs` | `game_state.md` | G-SAV-002 |
| Vehicles (the ridden-vehicle record) | `vehicle.rs` | `flight_render.rs` | — | G-HERO-003 |
| Planet travel, the ship, space transitions | `travel/` | `travel_render.rs` | `progression.md` | G-LVL-006 |

## R&C1-only code (not coming back)

- **R&C1's class behaviours** (`moby_update/classes/`): R&C2's classes are ported from R&C2's own code
  ([`rac2_class_census.md`](rac2_class_census.md), G-CLS-001).
- **R&C1's game loop and boot**: `tick.rs`, `moby_runtime.rs`, `frontend.rs`, engine `gameplay.rs`, `title_world.rs`.
  R&C2's are `rc-game` (`game`, `moby`, `front_end`) and the engine's `rac2*.rs`.
- **R&C1's versions of systems R&C2 has its own port of**: hero, follow camera, HUD, help, audio, particles' pool
  (R&C2's ports took what applied; R&C1's stay the map for the rest, cited as `rerac:rc_game::…`).
- **R&C1's data plumbing**: its disc tables (`SCUS_971.99`), the Tier 1 export, the `rc-data` crate, the golden
  snapshot tests, and the extractor and trace commands for R&C1's files.
- **R&C1's Ghidra tooling**: its name tables (`tools/ghidra/names/`: Lombyte's boot names, the overlay clusters, the
  names from the docs) and the scripts that apply them to R&C1's project. They are the source for naming Going
  Commando's functions by Version Tracking; `tools/ghidra/scripts/crossgame_diff.py` reads R&C1's overlay and
  decompiler export from R&C1's own folder.
