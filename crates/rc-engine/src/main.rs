//! Runtime entry point: loads one level from the game data folder and runs it (the front end over it unless a
//! level is asked for).
//!
//! Arguments (launcher contract, docs/plan/launcher_contract.md; crate::disc_source):
//! - `--data-dir <dir>` the game data folder (`rerac2-extract` output); `--version-json` print the version and exit
//!
//! Environment:
//! - `RC_DATA_DIR`   the game data folder when `--data-dir` is absent; else `RC_EXTRACTED`, else `<workspace>/extracted`
//! - `RC_LEVEL`      start straight in this level (skips the front end unless `RC_FRONTEND=1`), crate::front_end
//! - `RC_SCREENSHOT` if set, save a PNG of the window there after the scene has rendered, then exit
//! - `RC_SCREENSHOT_DELAY` seconds to wait before that capture (default 3); with `RC_SCREENSHOT_FRAME`
//!   the capture is frame-exact instead (crate::determinism) and this wall-clock one is not scheduled
//! - `RC_CAM`        optional camera override in game coordinates: `eye_x,eye_y,eye_z,target_x,target_y,target_z`
//! - `RC_GS_ALPHA=0` the previous alpha mapping (one blended draw without Z for any As != 0x80), crate::gs_state
//! - `RC_PLAY=0`, `RC_PLAY_FLY=1`, `RC_PLAY_SCRIPT`, `RC_PLAY_TRACE=1`, `RC_SCENES=0`, `RC_HERO_AT`, `RC_MOBY_TRACE` the
//!   game tick and its dev switches, crate::play
//! - `RC_MSAA=0|2|4|8` the Anti-aliasing option at start: Off or that multisampling (default: Original, the game's own
//!   softening passes), crate::graphics
//! - `RC_SETTINGS_FILE=<path>` the port-settings file (`0` or empty: none), crate::render_settings
//! - `RC_AUDIO=0` no sound, crate::audio_out

mod crash_log;
mod asset_write;
mod audio_out;
mod determinism;
mod display;
mod display_blend;
mod disc_source;
mod fly_cam;
mod fx_draw;
mod game_camera;
mod gs_state;
mod hud_render;
mod input_map;
mod level_load;
mod level_switch;
mod draw_stats;
mod frame_pace;
mod graphics;
mod moby_anim;
mod moby_light;
mod moby_lod;
mod moby_render;
mod no_clusters;
mod occlusion;
mod particle_render;
mod play_camera;
mod render_settings;
mod gs_post;
mod front_end;
mod card;
mod port_options;
mod level_wad;
mod mobys;
mod play;
mod clock;

mod world_screen;
mod frame_log;
mod pre_shadow;
mod shrub_billboard;
mod shrub_render;
mod sky_render;
mod tfrag_lod;
mod tfrag_render;
mod tie_lod;
mod tie_render;
mod aa_blit;
mod world_lights;

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::path::PathBuf;
use std::time::Duration;

use fly_cam::{FlyCam, FlyCamPlugin};
use level_load::LoadedLevel;
use tfrag_render::{game_to_bevy, TfragMaterial};

#[derive(Resource)]
pub(crate) struct Level(pub(crate) LoadedLevel);

#[derive(Resource)]
struct ScreenshotRequest {
    path: PathBuf,
    /// Minimum wall time and frame count before capturing, so the shader has compiled and the meshes have been uploaded.
    delay: Duration,
    min_frames: u32,
}

fn main() -> anyhow::Result<()> {
    crash_log::install();
    // `--version-json` exits here; a missing or wrong data folder exits with a clear error (crate::disc_source).
    let root = disc_source::startup();
    let index = level_load::level_index();
    // The one level load (the boot's and every runtime level change's, crate::level_switch).
    let level_switch::LevelBundle { mut level, .. } = level_switch::load_bundle(&root, index)?;
    let t = &level.timings;
    let bg = level.background;
    println!(
        "level {index:02} from {}: {} tfrags, {} textures; load {:.1} ms (read {:.1}, WAD {:.1}, core {:.1}, tfrags {:.1}, textures {:.1})",
        root.display(), level.tfrags.len(), level.textures.len(), ms(t.total()),
        ms(t.read), ms(t.decompress), ms(t.parse_core), ms(t.parse_tfrags), ms(t.parse_textures)
    );
    println!("background (GS clear colour): {bg:?}");
    println!("{}", fly_cam::CONTROLS);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                // 4:3, as the TV showed the 512x416 GS draw buffer (crate::display; docs/plan/render_pipeline.md).
                primary_window: Some(Window { title: "ReRAC2".into(), resolution: (1024u32, 768u32).into(), ..default() }),
                ..default()
            })
            .set(AssetPlugin { file_path: asset_dir(), ..default() })
            // Bevy's task pools by core count (its default). `RC_THREADS=n` makes them share n threads: fewer idle workers
            // waking for the frame's ~100 small systems (Novalis, 1,500 frames: ~28 s of CPU by default, ~21 s at 3). Not
            // the default: at 3 a Windows PC on Vulkan (16 cores, NVIDIA, two 4K screens) showed a black window while the
            // game ran, with sound (v0.3.1; the default threads and DX12 both showed the picture).
            .set(TaskPoolPlugin {
                task_pool_options: match std::env::var("RC_THREADS").ok().and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0) {
                    0 => TaskPoolOptions::default(),
                    n => TaskPoolOptions::with_num_threads(n),
                },
            }),
    )
    // The game frame every camera renders into and its presentation in the window (before anything spawns a camera,
    // and before the capture, which captures the frame).
    .add_plugins(display::DisplayPlugin)
    // Frame-exact ticks / capture and the deterministic phase order, before anything spawns a camera.
    .add_plugins(determinism::DeterminismPlugin)
    // One game tick per drawn frame, at most 60 frames a second (crate::frame_pace).
    .add_plugins(frame_pace::FramePacePlugin)
    .add_plugins(no_clusters::NoClustersPlugin)
    .add_plugins(graphics::GraphicsPlugin)
    .add_plugins(draw_stats::DrawStatsPlugin)
    // MSAA of the world cameras (default off; `RenderSettings` can change it at run time).
    .add_plugins(render_settings::RenderSettingsPlugin)
    .add_plugins((MaterialPlugin::<TfragMaterial>::default(), FlyCamPlugin, game_camera::GameCameraPlugin { fog: level.fog }))
    .add_plugins(occlusion::OcclusionPlugin)
    .add_plugins(tfrag_lod::TfragLodPlugin)
    .add_plugins(moby_render::MobyRenderPlugin)
    .add_plugins(pre_shadow::PreShadowPlugin)
    .add_plugins(frame_log::FrameLogPlugin)
    .add_plugins(tie_render::TieRenderPlugin)
    .add_plugins(shrub_render::ShrubRenderPlugin)
    // Point lights on tfrags, ties and shrubs (the game's Light* point passes; crate::world_lights).
    .add_plugins(world_lights::WorldLightsPlugin)
    .add_plugins(sky_render::SkyRenderPlugin)
    .add_plugins(display_blend::DisplayBlendPlugin)
    .add_plugins(particle_render::ParticlePlugin)
    .add_plugins(fx_draw::FxDrawPlugin)
    .add_plugins(hud_render::HudPlugin)
    .add_plugins(world_screen::WorldScreenPlugin)
    .add_plugins(aa_blit::AaBlitPlugin)
    // Sound slots, 989snd, music and the software SPU2 mixed per game tick (crate::audio_out; RC_AUDIO=0 off).
    .add_plugins(audio_out::AudioOutPlugin::new(level.audio.take()))
    // The GS frame clear: level settings +0x00..+0x08 via `set_background_color` (game_camera::level_background).
    // srgb_u8: the bytes are display-encoded like every GS colour; the sRGB view target stores them back exactly.
    .insert_resource(ClearColor(Color::srgb_u8(bg[0], bg[1], bg[2])))
    .insert_resource(Level(level))
    // The runtime level change and the level start's schedules (crate::level_switch).
    .add_plugins(level_switch::LevelSwitchPlugin)
    .add_systems(Startup, setup_camera)
    .add_systems(level_switch::LevelStartup, setup)
    .add_systems(Update, report_fps);
    // The front end over the loaded level (crate::front_end), then the game tick (crate::play).
    // The memory card (crate::card) the front end's pages and the game's saves use.
    app.add_plugins(card::CardPlugin(root.to_path_buf()));
    if front_end::requested() { app.add_plugins(front_end::FrontEndPlugin::new(&root)?); }
    app.add_plugins(play::PlayPlugin);
    // RC_RENDER_DIAG=1: Bevy's per-pass render timings (CPU only on Metal) and the views rendered, logged every 5 s.
    if std::env::var("RC_RENDER_DIAG").is_ok_and(|v| v.trim() == "1") {
        app.add_plugins((
            bevy::render::diagnostic::RenderDiagnosticsPlugin,
            bevy::diagnostic::LogDiagnosticsPlugin { wait_duration: std::time::Duration::from_secs(5), ..default() },
        ));
        if let Some(r) = app.get_sub_app_mut(bevy::render::RenderApp) {
            r.add_systems(bevy::render::Render, count_views.in_set(bevy::render::RenderSystems::Cleanup));
        }
    }

    if let Some(path) = std::env::var_os("RC_SCREENSHOT").filter(|_| determinism::screenshot_frame().is_none()) {
        let secs = std::env::var("RC_SCREENSHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0);
        app.insert_resource(ScreenshotRequest { path: path.into(), delay: Duration::from_secs_f32(secs), min_frames: 30 })
            .add_systems(Update, take_screenshot);
    }
    app.run();
    Ok(())
}

fn ms(d: Duration) -> f64 { d.as_secs_f64() * 1e3 }

/// The shader folder: `assets/` next to the executable (a packaged version, `tools/package`), or
/// `../Resources/assets` inside a macOS `.app`; otherwise the repo's `crates/rc-engine/assets` (`cargo dev`,
/// `target/*/rerac2`). The executable path is resolved through symlinks first, so a symlinked binary finds the
/// folder beside its real file.
pub(crate) fn asset_dir() -> String {
    let exe = std::env::current_exe().and_then(std::fs::canonicalize).ok();
    let dir = exe.as_deref().and_then(std::path::Path::parent);
    [dir.map(|d| d.join("assets")), dir.map(|d| d.join("../Resources/assets"))]
        .into_iter()
        .flatten()
        .find(|p| p.join("shaders").is_dir())
        .map_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(), |p| p.to_string_lossy().into_owned())
}

fn setup(
    mut commands: Commands,
    level: Res<Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TfragMaterial>>,
    mut buffers: ResMut<Assets<bevy::render::storage::ShaderBuffer>>,
) {
    let level = &level.0;
    let s = tfrag_render::spawn_tfrags(&mut commands, level, &mut meshes, &mut images, &mut materials, &mut buffers);
    println!(
        "tfrags: {} tfrags, {} LOD-0 triangles, {} vertices, {} meshes ({} with the GS alpha-test split), {} images; mesh build {:.1} ms, asset upload {:.1} ms; vertex AABB {:.1?} .. {:.1?}",
        s.tfrags, s.triangles, s.vertices, s.meshes, s.blended_meshes, s.images, ms(s.mesh_build), ms(s.upload), s.min, s.max
    );
}

/// The main world camera, once per process (crate::level_switch keeps it across level changes); framed on the boot
/// level's tfrags (the play camera takes over from the first tick).
fn setup_camera(mut commands: Commands, level: Res<Level>) {
    let level = &level.0;

    // Camera framing from the tfrag bounding spheres. They are stored in integer position units
    // (x1024 like origin + local), not world units: on Novalis the raw centroid is ~(166138, 189596, 59301)
    // while the LOD-0 vertices span (22..322, 45..388, 30..112) after /1024.
    let spheres: Vec<[f32; 4]> = level.tfrags.iter().map(|t| t.header.bsphere.map(|v| v / 1024.0)).collect();
    let n = spheres.len().max(1) as f32;
    let c = spheres.iter().fold(Vec3::ZERO, |a, s| a + Vec3::new(s[0], s[1], s[2])) / n;
    let extent = spheres.iter().map(|s| (Vec3::new(s[0], s[1], s[2]) - c).length() + s[3]).fold(0.0f32, f32::max);
    println!("bounds (game units): sphere centroid {:.1?}, extent {:.1}", c.to_array(), extent);
    let (eye, target) = match std::env::var("RC_CAM").ok().and_then(|v| parse_cam(&v)) {
        Some((e, t)) => (game_to_bevy(e), game_to_bevy(t)),
        // Up (game +Z) and back (game -Y) from the centroid, looking at it.
        None => (game_to_bevy((c + Vec3::new(0.0, -0.35 * extent, 0.25 * extent)).to_array()), game_to_bevy(c.to_array())),
    };
    let transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    commands.spawn((
        Camera3d::default(),
        game_camera::game_projection(),
        // No tonemapping or dithering: the shader already produces the final display value.
        Tonemapping::None,
        DebandDither::Disabled,
        render_settings::WorldCamera,
        // The UI nodes without a target camera draw on the game frame (not on the present camera).
        bevy::ui::IsDefaultUiCamera,
        FlyCam::from_transform(&transform, (extent / 20.0).clamp(5.0, 200.0)),
        transform,
        level_switch::KeepAcrossLevels,
    ));
}

/// `ex,ey,ez,tx,ty,tz` in game coordinates.
fn parse_cam(s: &str) -> Option<([f32; 3], [f32; 3])> {
    let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
    (v.len() == 6).then(|| ([v[0], v[1], v[2]], [v[3], v[4], v[5]]))
}

/// RC_RENDER_DIAG: the views (cameras) the render world drew this frame, every 300 frames.
fn count_views(views: Query<&bevy::render::view::ExtractedView>, mut n: Local<u32>) {
    *n += 1;
    if (*n).is_multiple_of(300) { println!("render: {} views", views.iter().count()); }
}

/// Drawn frames per second over 5 s of wall-clock time (the game clock advances by whole ticks: crate::frame_pace).
fn report_fps(mut acc: Local<Option<(std::time::Instant, u32)>>) {
    let now = std::time::Instant::now();
    let (start, n) = acc.get_or_insert((now, 0));
    *n += 1;
    let t = (now - *start).as_secs_f32();
    if t >= 5.0 {
        println!("fps: {:.1}", *n as f32 / t);
        *acc = Some((now, 0));
    }
}

fn take_screenshot(
    mut commands: Commands,
    req: Res<ScreenshotRequest>,
    frame: Res<display::GameFrame>,
    time: Res<Time<Real>>,
    mut frames: Local<u32>,
    mut requested: Local<bool>,
) {
    *frames += 1;
    if *requested || *frames < req.min_frames || time.elapsed() < req.delay { return; }
    *requested = true;
    let path = req.path.clone();
    // The game frame (crate::display), not the window: no bars, and no blank capture of an occluded window.
    commands.spawn(Screenshot::image(frame.image.clone())).observe(move |shot: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
        match shot.image.clone().try_into_dynamic() {
            Ok(img) => match img.to_rgb8().save(&path) {
                Ok(()) => println!("screenshot saved to {}", path.display()),
                Err(e) => eprintln!("cannot save screenshot to {}: {e}", path.display()),
            },
            Err(e) => eprintln!("cannot convert screenshot: {e:?}"),
        }
        exit.write(AppExit::Success);
    });
}
