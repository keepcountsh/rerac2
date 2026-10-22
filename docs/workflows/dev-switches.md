# Workflow: engine switches (command line, settings file, environment)

How to steer a `rerac2` run: its two command-line options, the settings file, and the `RC_*` environment switches.
All of them work the same under `cargo dev` (`cargo dev -- <options>`) and on a built `rerac2`.

## Command line

| Option | Effect |
|---|---|
| `--data-dir <folder>` (also `--data-dir=<folder>`) | The game data folder; how the launcher starts the game. Selection order and exit codes: `docs/workflows/game-data.md` "How the engine finds its data" |
| `--version-json` | Prints the launcher contract line (`{"name":"rerac","version":"0.1.0","game":"rac2","data_format":1}`) and exits without touching any data |

## Settings file

Port settings (MSAA, aspect, resolution, the graphics options) live in
`~/Library/Application Support/rerac2/settings.toml` (macOS), `$XDG_CONFIG_HOME/rerac2/settings.toml` (else
`~/.config/rerac2/`, Linux) or `%APPDATA%\rerac2\settings.toml` (Windows), apart from R&C1's port's. `RC_SETTINGS_FILE=<path>`
picks another file; `RC_SETTINGS_FILE=0` (or empty) disables it. Frame-exact runs (`RC_DETERMINISTIC=1`,
`RC_SCREENSHOT_FRAME`, `RC_DUMP_FRAMES`) neither read nor write the default file; an explicit `RC_SETTINGS_FILE` still
applies to them (`crates/rc-engine/src/render_settings.rs`).

## Environment switches

Boolean switches are on with `1` (or off with `0` where the default is on).

This table lists the switches for everyday development. The engine reads more (trace, dump and survey switches added
by individual ports, e.g. `RC_AUDIO_TRACE`, `RC_MOVIE_TRACE`, `RC_SCREENSHOT_FRAME`); each is documented in the
module that reads it: `grep -rn '"RC_' crates/` lists them all.

| Variable | Effect |
|---|---|
| `RC_DATA_DIR` | Game data folder when `--data-dir` is not given (see "Command line") |
| `RC_EXTRACTED` | Development data tree (default `<repo>/extracted`); the engine's fallback when neither `--data-dir` nor `RC_DATA_DIR` is set, and the tests' root |
| `RC_AUDIO` | `0`: no audio (the level's sound bank and music are not loaded); the `cargo xtask test-*` commands set it |
| `RC_LEVEL` | Start straight in this level (skips the front end unless `RC_FRONTEND=1`). Without it the game boots into the front end; the level loaded behind it is Aranos (0) |
| `RC_FRONTEND` | The front end (the movies, the main menu) is the default start. `0`: skip it and start in the level; `1`: keep it even with `RC_LEVEL` |
| `RC_CAM` | Starting camera `ex,ey,ez,tx,ty,tz` (eye and target, game units) |
| `RC_SCREENSHOT` | Save a screenshot to this path, then exit |
| `RC_SCREENSHOT_DELAY` | Seconds before the screenshot (default 3) |
| `RC_DUMP_FRAMES` | `start..end`: save every frame from update `start` to `end` (inclusive) as `frame_NNNNN.png` into `RC_DUMP_DIR` (default `frames`), then exit (dev only; frame-exact, offscreen like `RC_SCREENSHOT_FRAME`) |
| `RC_DUMP_DIR` | Folder for `RC_DUMP_FRAMES` |
| `RC_DUMP_REALTIME` | `1`: `RC_DUMP_FRAMES` in real-time mode (wall-clock ticks as in play, not frame-exact; dev only) |
| `RC_DUMP_TICKS` | `n,m,…`: in frame-exact mode, update k runs the k-th entry's game ticks, cycled (`1,0` = a 120 Hz display, `2` = 30 Hz; dev only, for real-time pacing repros) |
| `RC_NOVSYNC` | `1`: present without vsync, so `fps:` measures headroom |
| `RC_FRAME_CAP` | `0`: no 60 fps limiter (`crate::frame_pace`; also off with `RC_NOVSYNC=1`); ticks then follow the wall clock. `1`: the limiter in frame-exact runs too (`RC_DUMP_FRAMES` / `RC_SCREENSHOT_FRAME` otherwise render as fast as they can) |
| `RC_THREADS` | `n`: Bevy's task pools share n threads (fewer idle workers waking: ~25% less CPU at 3 on a Mac); default and `0`: Bevy's default by core count (3 was the default in v0.3.1 and showed a black window on a Windows PC on Vulkan) |
| `RC_RENDER_DIAG` | `1`: Bevy's per-pass render timings (CPU only on Metal) every 5 s, and the number of views rendered |
| `RC_DRAW_STATS` | `1`: every 300 frames, per material type the mesh entities, the visible ones and their distinct materials / meshes, and the active cameras (`crate::draw_stats`) |
| `RC_ASPECT` | `4:3` (default: the original TV picture), `16:10` or `16:9` (Hor+: the view widens, the HUD's side elements move to the frame's edges, the menus stay in the centred 4:3 box); overrides the Port Options value at start (crate::display) |
| `RC_RESOLUTION` | `window` (default: the largest frame of the Aspect ratio that fits the window) or `416`, `720`, `1080`, `1440`, `2160` (a frame that many lines high at the Aspect ratio, scaled to the window); overrides the Port Options value at start (crate::display) |
| `RC_FULLSCREEN` | `1`: borderless fullscreen on the current monitor; `0`: windowed; overrides the Port Options value at start (F11 toggles in game) |
| `RC_FOG` | `0`: disable fog |
| `RC_NO_LIGHT` | `1`: skip the load-time lighting passes (tfrag, tie, shrub, moby); stored colours used |
| `RC_WORLD_LIGHTS` | `0`: point lights (explosions) do not relight tfrags, ties and shrubs |
| `RC_WORLD_LIGHTS_TRACE` | `1`: print, every frame, the frame time, the point-light bank and the listed tfrag / tie / shrub counts |
| `RC_LOD` | `0`: force tfrag LOD 0 |
| `RC_LOD_TINT` | `1`: tint tfrag LOD 1 red, LOD 2 blue, clipping path green |
| `RC_TIE_LOD` | `0`: force tie LOD 0 with morph k = 0 (culling unchanged) |
| `RC_TIE_LOD_TINT` | `1`: tint tie LOD 1 red, LOD 2 blue |
| `RC_NO_TIES` | `1`: do not draw ties |
| `RC_NO_SHRUBS` | `1`: do not draw shrubs |
| `RC_SWAY` | `0`: no wind sway on shrubs or ties (their per-frame shear stays zero) |
| `RC_ANIM` | `0`: disable moby animation (bind pose) |
| `RC_MOBY_CPU_LIGHT` | `1`: use the bit-exact CPU moby lighting (bind pose) instead of the GPU path |
| `RC_MOBY_GLOW` | `0`: no moby glow list (glow packets drawn lit instead of in the moby's glow colour +0x90) |
| `RC_MOBY_LIGHT_CHECK` | `1`: compare GPU vs bit-exact CPU moby lighting at load and report |
| `RC_OCCL` | `0`: occlusion off; `1`: freeze the mask built from the starting camera |
| `RC_OCCL_STATS` | `1`: print occlusion cell and cull counts, at most once a second |
| `RC_HERO_AT` | `x,y,z[,yaw]`: place Ratchet there at the level load, before the hero init's ground snap (debug) |
| `RC_SCENES` | `0`: the level's scenes count as seen (no arrival scene; headless runs) |
| `RC_PLAY_TRACE` | `1`: one line per gameplay tick with Ratchet's state, position, velocity and the camera (`tick …`, crate::play) |
| `RC_PLAY_SCRIPT` | Scripted pad input by tick, e.g. `0-120:stick 0 -1, 60:press X+R1, 200-201:rstick 1 0` (crate::input_map) |
| `RC_MOBY_TRACE` | `0ce9,0340`: trace those classes' mobys (hex class numbers); `RC_MOBY_TRACE_EVERY=n` every n ticks, `RC_MOBY_PV=0,10` the pvar words (hex offsets) |
| `RC_FRONT_END_TRACE` | `1`: log the front end's movies, rows and sounds (crate::front_end) |

Examples:

```
RC_LEVEL=0 cargo dev                                   # start on Aranos
RC_AUDIO=0 RC_LEVEL=0 RC_SCENES=0 RC_HERO_AT=230,245,112,1.57 RC_SCREENSHOT_FRAME=100 RC_SCREENSHOT=work/captures/a.png cargo dev
RC_DUMP_FRAMES=100..160 RC_DUMP_DIR=work/captures/run1 cargo dev
```
