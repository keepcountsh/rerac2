//! Going Commando's game tick in the engine (rc_game: [`rc_game::hero`], [`rc_game::camera`]): Ratchet runs,
//! jumps and falls on the level's collision, his moby and pose follow him, the camera follows him
//! (docs/plan/bringup.md §6.3 F). The pieces that are the same in both games come from the R&C1 side: the pad
//! record (`rc_game::pad`), Ratchet's animation player (`rc_game::hero::anim::RatchetAnim`), the moby renderer's
//! drive of a static instance (crate::moby_render) and the play camera (crate::play_camera).
//!
//! * **Start** (the first tick after the level loaded, and after the front end's New Game): Ratchet is the level's
//!   class-0 instance (its placement: Aranos (247.7, 194, 49.9), facing +y); the hero spawns there, idle; the camera
//!   snaps behind him.
//! * **Tick** (`FixedUpdate`, 60 Hz): the pad (keyboard / gamepad, or `RC_PLAY_SCRIPT`) into the PAD record, the hero
//!   update, the camera; Ratchet's instance gets his position and rows (`MobyOcclusion::drive`) and his animation
//!   state (`MobyAnim`); the camera goes to `PlayView`.
//!
//! Environment: `RC_PLAY=0` (no tick), `RC_PLAY_FLY=1` (start on the fly camera; Tab switches), `RC_PLAY_TRACE=1`
//! (one line per tick: state, position, velocity, air ticks, sequence), `RC_SCENES=0` (the scenes count as seen),
//! `RC_HERO_AT=x,y,z,yaw` (Ratchet's start).

use crate::game_camera::CameraSource;
use crate::input_map::{self, PadFrame, Script};
use crate::moby_anim::MobyAnim;
use crate::moby_render::MobyOcclusion;
use crate::play_camera::{self, PlayView};
use bevy::prelude::*;
use rc_game::camera::CameraView;
use rc_game::hero::anim::RatchetAnim;
use rc_game::pad::PadState;
use rc_game::ps2v::Pf;
use rc_game::camera::{CamInput, FollowCamera};
use rc_game::hero::physics::Env;
use rc_game::hero::{hero_update, Hero};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("RC_PLAY").is_ok_and(|v| v.trim() == "0") { return; }
        let fly = std::env::var("RC_PLAY_FLY").is_ok_and(|v| v.trim() == "1");
        let script = std::env::var("RC_PLAY_SCRIPT").ok().filter(|s| !s.trim().is_empty()).map(|s| Script::parse(&s).unwrap_or_else(|e| panic!("RC_PLAY_SCRIPT: {e}")));
        println!("{}", input_map::CONTROLS);
        app.insert_resource(if fly { CameraSource::Fly } else { CameraSource::Play })
            .insert_resource(PadScript(script))
            .init_resource::<PadFrame>()
            .init_resource::<CanopyDraw>()
            .init_resource::<ShotDraw>()
            .add_systems(PreUpdate, (input_map::sample_system, switch_camera).chain().after(bevy::input::InputSystems))
            .add_systems(FixedUpdate, (tick, pause_tick).chain().after(crate::card::CardStep))
            .add_systems(Update, (draw_created, draw_hud, move_instances, ship_screen.after(draw_created), crate::overlay_mobys::assign_layers.after(draw_created)))
            .add_systems(PostUpdate, draw_canopy.before(bevy::asset::AssetEventSystems))
            .add_systems(PostUpdate, draw_shots.before(bevy::asset::AssetEventSystems))
            .add_systems(PostUpdate, occlusion_mode.before(crate::occlusion::OcclusionSet))
            .add_systems(RunFixedMainLoop, play_camera::apply.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop))
            .add_systems(crate::level_switch::LevelUnload, (unload, crate::level_switch::remove::<GameHud>, crate::level_switch::remove::<crate::mobys::CreatedDraw>, crate::level_switch::reset::<CanopyDraw>, crate::level_switch::reset::<ShotDraw>));
    }
}

/// Quit Game: the front end comes back (crate::front_end), the running game ends.
#[derive(Resource)]
pub(crate) struct ReturnToFrontEnd;

/// The pause's pages, after the game's tick read the pad (rc_game::menus: the pause list, Options, the save and load
/// pages, Port Options, Quit Game).
#[allow(clippy::too_many_arguments)]
fn pause_tick(
    play: Option<ResMut<Play>>,
    card: Option<ResMut<crate::card::Card>>,
    mut gfx: ResMut<crate::graphics::GraphicsSettings>,
    mut display: ResMut<crate::display::DisplaySettings>,
    hud: Option<Res<GameHud>>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    if p.pause.is_none() { return; }
    let Some(mut card) = card else {
        p.pause = None;
        return;
    };
    let c = &mut *card;
    if std::mem::take(&mut p.pause_opened) { p.game.store(&mut c.save); }
    let mut port = crate::port_options::values(&gfx, &display);
    let clock = if crate::determinism::deterministic() { [0; 8] } else { crate::clock::clock_now() };
    let held = (p.pad.held & 0xffff) as u16;
    let mut cx = rc_game::menus::Ctx { card: &mut c.mc, save: &mut c.save, blank: &c.blank, clock, port: &mut port, held, option_rows: &[], tables: hud.as_deref().map_or(&rc_formats::disc::menu_tables::EMPTY, |h| &h.tables) };
    let Some(m) = p.pause.as_mut() else { return };
    let outs = m.tick((p.pad.pressed & 0xffff) as u16, &mut cx);

    use rc_game::menus::Out;
    for o in outs {
        match o {
            Out::Resume | Out::MainMenu | Out::Close => p.pause = None,
            Out::Port(s, v) => crate::port_options::apply(s, v, &mut gfx, &mut display),
            // A save of level 0 not visited yet starts a new game from it (`0x2f7328`): level 0, as a load.
            Out::Loaded { .. } | Out::NewGame => {
                let level = if let Out::Loaded { level } = o { level } else { 0 };
                // The loaded game, on its level (the level change carries it: [`unload`]).
                let mut g = rc_game::game::GameState::default();
                g.load(&c.save);
                p.game = g;
                p.game.next_level = Some(level.max(0));
                p.pause = None;
            }
            Out::Quit => {
                // The Quit page's `0x2f50f8(−1, 0)`.
                println!("play: Quit Game");
                p.game.go_to_level(-1, false);
                p.pause = None;
            }
            // `0x344dd0`: leaving the menu asks for the hand item the pages took (`0x18c0e8`) when it is not the one held
            // (0: none, 0x77). Only the items the hero can hold (rc_game::hero::items::ROWS); the feet and back slots
            // (the boots, the packs) are not ported.
            Out::Equip { slot: 0, item } => equip(p.game.held, &mut p.hero, item),
            Out::Equip { .. } => {}
            Out::Sound(_) => {}
        }
    }
}

/// The quick select's screen and ring for a frame (`0x2f84e8`, then the ring's part of `0x443c08`): the item it asks
/// for is taken as the pause's Weapons page's ([`equip`]).
#[allow(clippy::too_many_arguments)]
fn quick_select_frame(qs: &mut rc_game::quick_select::QuickSelect, paused: &mut bool, pad: &PadState, game: &mut rc_game::game::GameState, hero: &mut Hero, records: &[rc_formats::disc::menu_tables::ItemRecord], pause: bool) {
    let slots = rc_game::quick_select::slots(game, records);
    for o in qs.update(pad, &slots, pause, *paused) {
        match o {
            rc_game::quick_select::Out::Equip(item) => {
                // Its ammo cut to the most.
                if let Some(r) = rc_game::hero::items::row(item).filter(|r| r.ammo) {
                    if let Some(a) = game.ammo.get_mut(item as usize) { *a = (*a).min(r.max_ammo); }
                }
                equip(game.held, hero, item);
            }
            rc_game::quick_select::Out::Resume => *paused = false,
            rc_game::quick_select::Out::Sound(_) => {}
        }
    }
    qs.ring_tick(&slots);
}

/// `0x3227d8`: the hand item asked for (rc_game::hero::items::ItemBlock::ask).
pub(crate) fn equip(held: i32, hero: &mut Hero, item: i32) {
    if !hero.items.ask(held, item) { eprintln!("play: item {item:#x} cannot be held yet"); }
}

/// The game state a level change keeps (`GameState::enter_level` keeps its game-wide part), for the next level's
/// [`start`].
#[derive(Resource)]
pub(crate) struct Carry(pub(crate) rc_game::game::GameState);

/// [`crate::level_switch::LevelUnload`]: the running game ends with its level; its game state is carried over.
fn unload(mut commands: Commands, play: Option<Res<Play>>) {
    let Some(p) = play else { return };
    commands.insert_resource(Carry(p.game.clone()));
    commands.remove_resource::<Play>();
}

#[derive(Resource)]
struct PadScript(Option<Script>);

/// The running game.
#[derive(Resource)]
pub(crate) struct Play {
    pub(crate) hero: Hero,
    anim: RatchetAnim,
    /// Ratchet's class (index into `LevelMobys::classes` / `anim`) and static instance.
    class: usize,
    instance: usize,
    scale: f32,
    pub(crate) pad: PadState,
    pub(crate) camera: FollowCamera,
    /// The space fights' flight while the ship's camera is current (rc_game::ship).
    ship: Option<rc_game::ship::Flight>,
    /// A story chain plays in the flight (`0x3671b8`).
    chain: bool,
    /// The level's mobys (crate::mobys).
    pub(crate) mobys: Option<crate::mobys::Mobys>,
    /// The game-wide state (bolts, health limit).
    pub(crate) game: rc_game::game::GameState,
    /// The HUD's counters (rc_game::hud).
    pub(crate) hud: rc_game::hud::Hud,
    /// The help panel (rc_game::help) and the level's English help texts.
    pub(crate) help: rc_game::help::Panel,
    /// The HUD screen's message boxes, and the font they measure with (the HUD's, kept from its first draw).
    boxes: rc_game::msgbox::Boxes,
    glyphs: Option<Box<[rc_formats::disc::boot::Glyph; 256]>>,
    pub(crate) texts: Vec<rc_formats::strings::Message>,
    pub(crate) ticks: u64,
    trace: bool,
    /// The level's objects spawned once (later spawns are respawns).
    loaded: bool,
    /// The level's scene file (`G/SCENEn.WAD`) and its table, read at the first scene.
    /// This tick's draw-callback quads (rc_game::fx), drawn by [`draw_shots`].
    fx_quads: Vec<rc_game::fx::FxQuad>,
    scene_wad: Option<(Vec<u8>, rc_formats::disc::scene::SceneWad)>,
    /// `G/SPACE.WAD` and the story scenes' table between the levels (rc_formats::disc::scene::is_shared).
    shared_scenes: Option<(Vec<u8>, rc_formats::disc::scene::SceneWad)>,
    /// The class sequence slots a scene's actors took (class, slot) and what the slots held before.
    scene_slots: Vec<(usize, usize, Option<rc_formats::moby_anim::MobySequence>)>,
    /// The movie playing (mode 1) and its picture for the next draw.
    movie: Option<crate::front_end::Playing>,
    movie_picture: Option<Vec<u8>>,
    /// The level being left (`0x2f50f8`): the game waits while the next one loads (crate::level_switch).
    leaving: Option<i32>,
    /// The scene camera as the sound code's listener (its last frame).
    scene_listener: Option<rc_game::audio::voices::Listener>,
    /// A movie started this tick (its sound stops the others').
    movie_started: bool,
    /// His looping sounds' slots per channel (`0x18c24c`).
    hero_loops: [i32; 9],
    /// The pause's pages (rc_game::menus) while the game is stopped, and its first frame (the game state is stored
    /// into the card's save data then, for the save page).
    pause: Option<rc_game::menus::Menus>,
    pause_opened: bool,
    /// The death's fade to black, its frame (the game stopped; [`DEATH_FADE`] frames, then [`death_reset`]).
    death_fade: Option<u32>,
    /// The quick select (rc_game::quick_select), and the game stopped in its mode (Quick Select Pause on).
    qs: rc_game::quick_select::QuickSelect,
    qs_paused: bool,
    /// The vendor's menu (game mode 5, rc_game::vendor through crate::vendor_play).
    pub(crate) vendor: crate::vendor_play::VendorPlay,
    /// A ship scene (rc_game::travel: the take-off or the landing from `SPACE.WAD`) for the next scene start, in
    /// place of the level's scene of the id asked for.
    space_scene: Option<std::sync::Arc<rc_formats::scene::Scene>>,
    /// The flight between the planets while the next level loads ([`FlightRun`]), the scene camera of its last frame
    /// (for the planet picture's projection) and its 2D draws (the planet picture, the caption).
    flight: Option<FlightRun>,
    flight_cam: Option<([f32; 3], [[f32; 3]; 3], f32)>,
    flight_quads: Vec<rc_game::gui::Quad>,
}

/// The flight between the planets (`0x367400` → `0x364260` sub 2, `0x363360`; the rules: rc_game::travel::Flight):
/// its lump (rc_formats::disc::space::Transition), the destination's arrival pictures (the level file's lump 10) and
/// whether they were written into the HUD layer yet.
struct FlightRun {
    lump: rc_formats::disc::space::Transition,
    pictures: Option<rc_formats::disc::level::PlanetPictures>,
    pictures_written: bool,
}

impl Play {
    /// The game's point lights (rc_game::point_lights, owned by the explosion lights), for crate::world_lights.
    pub(crate) fn point_lights(&self) -> Option<&rc_game::point_lights::PointLights> { self.mobys.as_ref().map(|m| &m.world.point_lights) }
}

fn switch_camera(keys: Res<ButtonInput<KeyCode>>, mut source: ResMut<CameraSource>) {
    if keys.just_pressed(KeyCode::Tab) {
        *source = if *source == CameraSource::Play { CameraSource::Fly } else { CameraSource::Play };
        println!("play: camera: {:?}", *source);
    }
}

#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    mut level: ResMut<crate::Level>,
    play: Option<ResMut<Play>>,
    front: Option<Res<crate::front_end::FrontEndState>>,
    frame: Res<PadFrame>,
    script: Res<PadScript>,
    source: Res<CameraSource>,
    mut occl: Option<ResMut<MobyOcclusion>>,
    mut anim: Option<ResMut<MobyAnim>>,
    carry: Option<Res<Carry>>,
    mut change: ResMut<crate::level_switch::LevelChange>,
    mut audio: Option<ResMut<crate::audio_out::AudioOut>>,
    mut parts: Option<ResMut<crate::particle_render::ParticleSim>>,
    mut card: Option<ResMut<crate::card::Card>>,
    mut lights: Option<ResMut<crate::moby_render::PointLightFrame>>,
) {
    // The game drives the particles (crate::particles of rc_game on the shared pool; R&C1's own tick stays off).

    // The front end's layer is up: the game waits (as R&C1's tick does under its front end).
    if front.is_some_and(|f| !f.done()) { return; }
    // The Sound page's volumes (rc_game::menus::opt, the card's save data; `RC_AUDIO_MUSIC` / `RC_AUDIO_SFX` win).
    if let (Some(out), Some(c)) = (audio.as_deref_mut(), card.as_deref()) {
        use rc_game::menus::opt;
        let w = |a: u32| c.save.i32(rc_game::save::Scope::Global, a).clamp(0, 0x400);
        let sys = out.system();
        if std::env::var_os("RC_AUDIO_MUSIC").is_none() && sys.music_option != w(opt::MUSIC) { sys.music_option = w(opt::MUSIC); }
        if std::env::var_os("RC_AUDIO_SFX").is_none() && sys.sfx_option != w(opt::SFX) { sys.sfx_option = w(opt::SFX); }
    }
    // The scenes add sequence slots to the level's class animations (as the game's actors do), unseen by the
    // systems that watch the level for a new one.
    let level = level.bypass_change_detection();
    let Some(mut p) = play else {
        if let Some(mut p) = start(&level.0) {
            let mut game = carry.map(|c| c.0.clone()).unwrap_or_default();
            game.enter_level(crate::level_load::level_index() as i32);
            p.hero.hp = game.max_health;
            p.game = game;
            commands.remove_resource::<Carry>();
            commands.insert_resource(PlayView { view: camera_view(&p.camera, p.camera.pos, p.camera.rows), tan_half_fov: play_camera::GAME_TAN_HALF_FOV, far: p.game.view_far * 1024.0 });
            commands.insert_resource(p);
        }
        return;
    };
    let p = &mut *p;
    if level.0.collision.is_none() { return; }
    if p.mobys.is_none() {
        let lv = &level.0;
        let blobs = crate::level_wad::class_blobs(&crate::level_load::extracted_root(), crate::level_load::level_index())
            .map_err(|e| eprintln!("play: no class blobs ({e:#}): no moby collision"))
            .unwrap_or(crate::level_wad::ClassBlobs { collision: Vec::new(), blobs: Default::default() });
        let mut created = Default::default();
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut created, hero: None, hero_chains: &[], chains: None, camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        let fresh = !p.loaded;
        let m = crate::mobys::Mobys::load(lv, blobs, &host, &mut p.game, fresh);
        p.loaded = true;
        // The dev start switches, on the first level only.
        static DEV_START: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let dev = fresh && !DEV_START.swap(true, std::sync::atomic::Ordering::Relaxed);
        // `RC_SCENES=0`: every scene counts as seen (no opening, no scene triggers' scenes).
        if dev && std::env::var("RC_SCENES").is_ok_and(|v| v.trim() == "0") { p.game.scenes_seen.iter_mut().for_each(|s| *s = true); }
        // `RC_HERO_AT=x,y,z,yaw`: Ratchet starts there (as the checkpoint, so the arrival leaves him).
        if let Some(v) = std::env::var("RC_HERO_AT").ok().filter(|_| dev).map(|v| v.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 4) {
            let (at, rot) = ([v[0], v[1], v[2]], [0.0, 0.0, v[3]]);
            p.hero.teleport(at, rot);
            p.game.checkpoint = Some((at, rot));
        }
        let mut m = m;
        m.canopy = canopy_mesh(&crate::level_load::extracted_root(), crate::level_load::level_index()).map(rc_game::canopy::Canopy::new);
        // The nanotech glow's tables (Aranos's overlay's addresses, rc_game::moby::classes::nanotech).
        m.world.nanotech.tables = (crate::level_load::level_index() == 0)
            .then(|| crate::level_wad::overlay(&crate::level_load::extracted_root(), 0).ok())
            .flatten()
            .and_then(|ov| rc_game::moby::classes::nanotech::GlowTables::read(|a, n| crate::level_wad::overlay_read(&ov, a, n)))
            .map(std::sync::Arc::new);
        // The Gravity Bomb's shockwave mesh (rc_game::moby::classes::gravity_bomb): the blast's tables at this level's
        // copy of Aranos's addresses (crate::level_code), on the levels that have the bomb's class (the blast's own
        // class has no assets of its own: it is not in the level's class list).
        m.world.gravity_bomb.mesh = m.world.class(rc_game::moby::classes::gravity_bomb::BOMB_CLASS).is_some().then(|| {
            let code = crate::level_code::LevelCode::load(&crate::level_load::extracted_root(), crate::level_load::level_index())
                .map_err(|e| eprintln!("play: the level's code not read ({e:#}): no shockwave mesh"))
                .ok()?;
            let mesh = code.with_reader(|read| rc_game::moby::classes::gravity_bomb::DomeMesh::read(read));
            match &mesh {
                Some(d) => println!("play: the Gravity Bomb's shockwave mesh: {} quads, {} vertices", d.quads.len(), d.verts.len()),
                None => eprintln!("play: the Gravity Bomb's shockwave tables not found in the level's overlay: no shockwave mesh"),
            }
            mesh.map(std::sync::Arc::new)
        }).flatten();
        // `0x3012a0`: Ratchet's parked ship from the level settings (rc_game::travel).
        {
            let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut m.created, hero: None, hero_chains: &[], chains: Some(&m.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
            if let Some(s) = rc_game::travel::park_ship(&mut m.world, &host, &mut p.game, &lv.gameplay) {
                let q = m.world.mobys[s].pos;
                println!("play: the ship parked at ({:.1}, {:.1}, {:.1}), the fly-away path {}, its cameras {} / {}", q[0], q[1], q[2], p.game.travel.path, p.game.travel.cam_a, p.game.travel.cam_b);
            }
        }
        println!("play: {} mobys, {} with collision, Ratchet's joint lists {}", m.world.mobys.len(), m.world.mobys.iter().filter(|m| m.has_collision).count(), m.hero_chains.len());
        // `0x2fd028(level)` (the level start `0x2fcf60`): the planet's weapons into the vendor's stock (rc_game::vendor).
        p.game.stock_for_level(crate::level_load::level_index() as i32, &m.level.items);
        p.mobys = Some(m);
        // `0x2da248`: the level cameras' slots (their pvar blocks are in the moby pvar table).
        use rc_formats::disc::gameplay as g2;
        let pvars = g2::pvar_blocks(&lv.gameplay).unwrap_or_default();
        let records = g2::cameras(&lv.gameplay).map_err(|e| eprintln!("play: cameras: {e}")).unwrap_or_default();
        p.game.cameras = rc_game::cameras::Cameras::load(
            records.iter().map(|r| (r.class as i16, r.pos, r.rot, usize::try_from(r.pvar_index).ok().and_then(|i| pvars.get(i)).cloned().unwrap_or_default())).collect(),
        );
        // `entry`'s landing on arrival (rc_game::travel): after a trip by the ship, the landing scene at the start.
        if fresh { travel_arrival(p, lv); }
    }
    // `0x2f50f8`: the level's main loop ends; the next level loads (crate::level_switch) and replaces this one.
    if let Some(to) = p.game.next_level.take() {
        // `0x2f50f8(level, 1)`: the autosave `0x30d658(0, level)` (the card's active slot, auto-save on), the game
        // stored with the next level standing in as its level.
        if std::mem::take(&mut p.game.next_level_save) && 0 <= to {
            if let Some(c) = card.as_deref_mut() {
                p.game.store(&mut c.save);
                let clock = if crate::determinism::deterministic() { [0; 8] } else { crate::clock::clock_now() };
                if c.mc.memcard_save(&mut c.save, false, to, clock) { println!("play: autosave for level {to}"); }
            }
        }
        if to < 0 {
            // The front end (Quit Game's request): the card's slot dropped, the game over.
            println!("play: back to the front end");
            if let Some(c) = card.as_deref_mut() {
                c.mc.cards[0].slot = -1;
                c.mc.checked = false;
                c.mc.flags &= !0x200;
            }
            commands.insert_resource(ReturnToFrontEnd);
            commands.remove_resource::<Play>();
            commands.remove_resource::<Carry>();
            return;
        } else if p.leaving.is_none() {
            println!("play: leaving level {} for level {to}", p.game.level);
            change.start_load(to as u32);
            p.leaving = Some(to);
            // `0x367400`, every level change: the flight's flag cleared, the story chain picked (`gp−0x2d80`), the
            // level left marked, and the flight over the load (rc_game::travel).
            let g = &mut p.game;
            g.travel.flight_ran = false;
            g.travel.dest = to;
            g.travel.story = rc_game::travel::story_pick(g, to, g.travel.story);
            rc_game::travel::level_left(g);
            if g.travel.story != 0 { println!("play: the story chain {:#x} on the way to level {to}", g.travel.story); }
            // `0x360b90` / `0x32ee80`: every sound and stream stopped (the level's loops with them).
            if let Some(out) = audio.as_deref_mut() { out.system().movie_stop(); }
            flight_start(p, &mut level.0, &mut commands);
        }
    }
    if p.leaving.is_some() {
        // The flight between the planets (rc_game::travel) over the load, with its sound step (a movie of the story
        // chain mixes its own, movie_tick).
        if p.game.scenes.mode != rc_game::scene::MOVIE { scene_audio_frame(p, audio.as_deref_mut()); }
        if p.flight.is_some() {
            if !flight_tick(p, &mut level.0, &mut commands, occl.as_deref_mut(), anim.as_deref_mut(), parts.as_deref_mut().map(|s| &mut s.sys), audio.as_deref_mut(), change.loaded()) {
                p.ticks += 1;
                return;
            }
            p.flight = None;
            p.flight_quads.clear();
            commands.remove_resource::<crate::sky_render::FlightSky>();
            commands.remove_resource::<crate::sky_render::FlightSkyPose>();
        }
        if change.loaded() { change.swap(); }
        return;
    }
    // `0x32c858`: a scene or a movie starting or ending.
    mode_changes(p, &mut level.0);
    // A movie stops every sound and the music (R&C1's `StartPssMovie`: `movie_stop`).
    if std::mem::take(&mut p.movie_started) {
        if let Some(out) = audio.as_deref_mut() { out.system().movie_stop(); }
    }
    let input = match &script.0 {
        Some(s) => s.at(p.ticks),
        None if *source == CameraSource::Play => frame.0,
        None => rc_game::pad::PadInput::neutral(),
    };
    p.pad.update(Some(&input.bytes()), false);
    // The pause: the game stops while its pages run ([`pause_tick`]); the frame count goes on (the pad script reads it).
    if p.pause.is_some() {
        p.ticks += 1;
        return;
    }
    // The death (`0x2b31f0`): `0x2e7c90(16)` fades to black over 16 frames with the game stopped, then the level
    // restarts from the checkpoint ([`death_reset`]).
    if let Some(k) = p.death_fade {
        p.ticks += 1;
        if k + 1 < DEATH_FADE {
            p.death_fade = Some(k + 1);
        } else {
            p.death_fade = None;
            death_reset(p, &level.0);
        }
        return;
    }
    p.game.frame += 1;
    // `0x3596d0`: the play time (NTSC: one a frame).
    p.game.play_time += 1;
    match p.game.scenes.mode {
        rc_game::scene::SCENE => {
            scene_tick(p, &mut level.0, &mut commands, occl.as_deref_mut(), anim.as_deref_mut(), parts.as_deref_mut().map(|s| &mut s.sys));
            scene_audio_frame(p, audio.as_deref_mut());
            p.ticks += 1;
            return;
        }
        rc_game::scene::MOVIE => {
            movie_tick(p, audio.as_deref_mut());
            p.ticks += 1;
            return;
        }
        _ => {}
    }
    // The ship mode (rc_game::travel): △ at the hatch of the last tick starts the take-off (`0x3706f0`'s `0x1b1a60`
    // → `0x32c5f0(6, 1, 0, 0)`); its pages (mode 3 kind 4) and the fly-away (mode 6 sub 1) replace the main loop's
    // frame while they run.
    if std::mem::take(&mut p.game.travel.enter) {
        travel_take_off(p, &mut level.0);
        p.ticks += 1;
        return;
    }
    if p.game.travel.blocks_play() {
        travel_frame(p, &mut level.0, &mut commands, occl.as_deref_mut());
        p.ticks += 1;
        return;
    }
    // The vendor's menu (game mode 5, crate::vendor_play): a vendor moby's request of the last tick switches the mode
    // at this frame's start (`0x32c858` → `0x374038`); while it runs its frame replaces the main loop's (`0x375f40`:
    // the world stands), its camera the view, and Ratchet is hidden (`0x2d3208`).
    if p.game.vendor_open.is_some() || p.vendor.menu.is_some() {
        // `0x374038`'s light: the bank's set 14 lit from the vendor's rows (the preview's light word 0x0e0e).
        if let Some(id) = p.game.vendor_open {
            if let (Some(v), Some(l)) = (p.mobys.as_ref().and_then(|m| m.world.mobys.get(id)), level.0.mobys.lighting.as_mut()) {
                l.bank.set_dir_light(rc_game::vendor::LIGHT_SET, rc_game::vendor::light_set(v.rows));
            }
        }
        let lv = &level.0;
        let f = {
            let Some(mobys) = p.mobys.as_mut() else { return };
            let widescreen = card.as_deref().is_some_and(|c| c.save.u8(rc_game::save::Scope::Global, rc_game::menus::opt::WIDESCREEN) != 0);
            let sound = audio.as_deref_mut().map(|o| crate::mobys::HostSound { system: o.system(), listener: listener(&p.camera), tick: p.ticks, loops: &mut mobys.loop_slots });
            let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound };
            let mut parts = crate::vendor_play::Parts { world: &mut mobys.world, level: &mobys.level, game: &mut p.game, hero: &mut p.hero, hud: &mut p.hud, help: &mut p.help, pad: &p.pad, texts: &p.texts, host: &mut host };
            if let Some(id) = parts.game.vendor_open.take() {
                // `0x327c68`: the platinum bolts found (the save's per-level bytes), for the mod vendor.
                if let Some(c) = card.as_deref() { parts.game.platinum_found = rc_game::menus::platinum_bolts(&c.save); }
                p.vendor.open(&mut parts, id, widescreen);
            }
            p.vendor.frame(&mut parts)
        };
        if f.closed {
            p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
            commands.insert_resource(crate::overlay_mobys::OverlayWindow::default());
        }
        if let Some((eye, rows)) = f.view {
            if let Some(o) = occl.as_mut() { o.drive(p.instance, p.hero.pos, p.hero.rows, p.scale, None, true); }
            // `0x37ba50`: the preview drawn over the panel inside `0x37ca08(158, 360, 115, 206)` (crate::overlay_mobys).
            commands.insert_resource(crate::overlay_mobys::OverlayWindow { rect: Some(rc_game::vendor::PREVIEW_WINDOW), mobys: p.vendor.preview.into_iter().collect() });
            let far = p.game.view_far * 1024.0;
            commands.insert_resource(PlayView { view: scene_view(&rc_game::scene_camera::SceneCamera { eye, rows, tan_half_fov: play_camera::GAME_TAN_HALF_FOV, cut: false }), tan_half_fov: play_camera::GAME_TAN_HALF_FOV, far });
            p.ticks += 1;
            return;
        }
    }
    // The quick select's part of the main loop (`0x3706f0`, rc_game::quick_select): with Quick Select Pause on, the
    // game stops in the ring's mode until △ lets go; only the ring and its screen run then.
    let qs_pause = card.as_deref().is_some_and(|c| c.save.u8(rc_game::save::Scope::Global, rc_game::menus::opt::QS_PAUSE) != 0);
    // `0x330240(0x10, 0x1a8fb8, 0)`: △ counts only within the frames since the prompt lease was freed (rc_game::prompt).
    let qs_ok = !p.qs_paused && 7 < p.game.frame && !matches!(p.hero.state, 0x6f | 0x31) && p.hero.items.flying != 2 && p.hero.mode < 2 && 0 < p.game.prompt.free_frames;
    if p.qs.main_loop(&p.pad, qs_pause, qs_ok) { p.qs_paused = true; }
    if p.qs_paused {
        if let Some(m) = p.mobys.as_ref() { quick_select_frame(&mut p.qs, &mut p.qs_paused, &p.pad, &mut p.game, &mut p.hero, &m.level.items, qs_pause); }
        p.ticks += 1;
        return;
    }
    // Start (level 00 `0x3706f0`: mode 3 kind 1, after the level's first 8 frames, not while dying or in a vehicle's
    // states 0x6f / 0x31).
    if p.pad.pressed & rc_game::pad::button::START != 0 && 7 < p.game.frame && !matches!(p.hero.state, 0x6f | 0x31) {
        p.pause = Some(rc_game::menus::Menus::pause(0).0);
        p.pause_opened = true;
        return;
    }
    // `0x327950`: the action prompt's lease (rc_game::prompt), not in state 0x1c.
    if p.hero.state != 0x1c { p.game.prompt.tick(); }
    let lv = &level.0;
    let Some(coll) = lv.collision.as_ref() else { return };
    // `0x2d6258`: the health limit from the XP and the nanotech.
    p.game.health_tick(&mut p.hero.hp);
    let class = &lv.mobys.anim[p.class];
    let mobys = p.mobys.as_mut().expect("mobys");
    let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
    // `+0x31` (MobyProc writes it each frame): a placed moby drawn last frame (shown, not hidden), as R&C1's
    // `write_visible`; the moby loop's activity and the weapons' aim read it. Created mobys keep their own (their
    // creators set it; the engine draws every live one), Ratchet is always drawn.
    if let Some(o) = occl.as_deref() {
        for m in mobys.world.mobys.iter_mut() {
            let Some(ii) = m.instance else { continue };
            let hidden = o.anim_index(ii).zip(anim.as_deref()).is_some_and(|(k, a)| a.hidden.get(k).copied().unwrap_or(false));
            m.drawn = (o.anim_index(ii).is_some() && o.drawn(ii) && !hidden) as u8;
        }
        if let Some(h) = mobys.hero_moby.and_then(|i| mobys.world.mobys.get_mut(i)) { h.drawn = 1; }
    }
    // The frame's order (`0x364260`): the moby loop `0x32e398`, then the hero `0x2a6dd0` (with its item pass), then
    // the camera `0x2d9f38`.
    {
        let mods = p.hero.posture.modifiers(&mobys.hero_targets);
        let hl = mobys.hero_layers(&p.hero.layers);
        let pose = crate::mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale, mods: &mods, layers: &hl };
        // The class sounds play during the loop (`0x3605f0`), heard from the camera as the last tick left it.
        let sound = audio.as_deref_mut().map(|o| crate::mobys::HostSound { system: o.system(), listener: listener(&p.camera), tick: p.ticks, loops: &mut mobys.loop_slots });
        let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound };
        let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        // The movers' carry of this tick starts from nothing (`0x189f70`).
        (p.hero.carry, p.hero.carry_yaw) = ([0.0; 3], 0.0);
        // The view the updates test their spheres against: the camera as the last tick left it (`0x2f0080`).
        let (tx, ty) = crate::game_camera::projection_tans(None);
        let [fwd, left, up] = p.camera.rows;
        let view = Some(rc_game::particles::BSphereView::from_camera(p.camera.pos, fwd, left, up, tx, ty));
        let mut ctx = rc_game::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level, parts: parts.as_deref_mut().map(|s| &mut s.sys), view };
        mobys.world.tick(p.camera.pos, &mut ctx);
    }
    // The collision kernels read the mobys as they are: the hero's queries this tick see where the loop left them
    // (a lift's floor moved this tick), not where the last tick did.
    if mobys.world.changed {
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        mobys.snap = Some(crate::mobys::take_snapshot(&mobys.world, &mobys.coll, &host));
        mobys.world.changed = false;
    }
    // `RC_MOBY_TRACE=0b0b,099c`: those classes' mobys every 15 ticks (`RC_MOBY_TRACE_EVERY`; a dev trace).
    let every = std::env::var("RC_MOBY_TRACE_EVERY").ok().and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(15).max(1);
    if p.ticks % every == 0 {
        if let Ok(list) = std::env::var("RC_MOBY_TRACE") {
            let classes: Vec<i16> = list.split(',').filter_map(|x| i16::from_str_radix(x.trim().trim_start_matches("0x"), 16).ok()).collect();
            for m in mobys.world.mobys.iter().filter(|m| m.state < 0x80 && classes.contains(&m.o_class)) {
                let d = rc_game::hero::physics::dist2(m.pos, p.hero.pos);
                // `RC_MOBY_PV=240,258`: those pvar words too (hex offsets).
                let a = m.instance.and_then(|ii| occl.as_ref()?.anim_index(ii)).and_then(|k| anim.as_ref()?.instances.get(k)).map(|i| i.state);
                let a = a.map_or(String::new(), |s| format!(" seq {}/{} f {}/{} t {:.2} speed {:.2} rate {:.3} flags {}", s.seq_a, s.seq_b, s.frame_a, s.frame_b, s.t, s.speed, s.rate, s.flags));
                let pv: Vec<String> = std::env::var("RC_MOBY_PV").unwrap_or_default().split(',').filter_map(|x| usize::from_str_radix(x.trim(), 16).ok()).map(|o| format!("+{o:#x}={}", m.pv_i(o))).collect();
                println!("moby trace tick {}: #{} class {:#06x} state {} sub {} mode {:#x} upd {} drawn {} group {} pvars #{} pos ({:.1}, {:.1}, {:.1}) hero at {:.1}{a} {}", p.ticks, m.index, m.o_class, m.state, m.sub_state, m.mode, m.update_dist, m.drawn, m.group, m.pvar_index, m.pos[0], m.pos[1], m.pos[2], d, pv.join(" "));
            }
        }
    }
    let targets = mobys.world.targets(p.camera.pos);
    // Ratchet's hit record (his moby's `+0xa8`) with its source.
    let hit = mobys.hero_moby.and_then(|i| mobys.world.hit_message(i)).map(|rec| {
        let src = rec.tmpl.source.and_then(|s| mobys.world.mobys.get(s));
        rc_game::hero::damage::HitIn { rec, source_pos: src.map(|m| m.pos), source_class: src.map_or(-1, |m| m.o_class) }
    });
    {
        let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        if p.game.death_z.is_none() && lv.death_z.is_finite() { p.game.death_z = Some(lv.death_z); }
        let death_z = p.game.death_z;
        let classes: Vec<i16> = mobys.world.mobys.iter().map(|m| m.o_class).collect();
        let nodes = rc_game::moby::classes::swing_nodes(&mobys.world, 0x2f6);
        let hooks = rc_game::moby::classes::swing_nodes(&mobys.world, 0x323);
        let pulls = rc_game::moby::classes::swing_nodes(&mobys.world, 0x99d);
        let env = Env {
            coll, pad: &p.pad, cam_yaw: p.camera.yaw(), cam_pos: p.camera.pos, cam_rows: p.camera.rows, mirror: false, scene: scene.as_ref(), hero_moby: mobys.hero_moby, targets: &targets, hit, death_z,
            clank: p.game.clank, pack: p.game.pack(), boots: p.game.boots, charge_boots: p.game.charge_boots, grind_boots: p.game.grind_boots, rails: &mobys.rails, level: p.game.level, classes: &classes, nodes: &nodes, hooks: &hooks, pulls: &pulls,
        };
        let before = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let mut ctl = p.anim.ctl(class);
        hero_update(&mut p.hero, &env, &mut ctl);
        // His animation's advance (`0x313420`): the first trigger whose time it passed plays its class sound on his
        // moby (`0x3605f0`), as R&C1's `RatchetAnimAdvance` does.
        let after = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        if let (Some(out), Some(h)) = (audio.as_deref_mut(), mobys.hero_moby) {
            let sys = out.system();
            let mut rng = sys.rng;
            if let Some(idx) = rc_game::audio::class_sounds::ratchet_trigger(class, &before, &after) {
                let ev = rc_game::audio::SoundEvent { index: idx as i32, flags: 0, moby: h, o_class: 0, sound_class: 0, pos: p.hero.pos, tick: p.ticks };
                sys.play_class_sound(&ev, Some(h), &listener(&p.camera), &mut rng);
            }
            // His states' own sounds (`0x2b9c90`).
            for (index, flags) in std::mem::take(&mut p.hero.sounds) {
                let ev = rc_game::audio::SoundEvent { index, flags: flags as u32, moby: h, o_class: 0, sound_class: 0, pos: p.hero.pos, tick: p.ticks };
                sys.play_class_sound(&ev, Some(h), &listener(&p.camera), &mut rng);
            }
            // His looping sounds: a channel whose slot still plays his sound keeps it; stopped channels release it.
            let ours = |sys: &rc_game::audio::AudioSystem, k: i32| usize::try_from(k).ok().and_then(|i| sys.slots.slots.get(i)).is_some_and(|s| s.state != rc_game::audio::voices::state::FREE && s.owner.is_some_and(|o| o.id == h as u32));
            let mut stops = std::mem::take(&mut p.hero.loop_stops);
            if std::mem::take(&mut p.hero.loops_off) { stops.extend(0..p.hero_loops.len()); }
            for k in stops {
                let Some(slot) = p.hero_loops.get_mut(k) else { continue };
                if ours(sys, *slot) { sys.slots.release(*slot); }
                *slot = -1;
            }
            for (k, index) in std::mem::take(&mut p.hero.loops) {
                if k >= p.hero_loops.len() || ours(sys, p.hero_loops[k]) { continue; }
                let ev = rc_game::audio::SoundEvent { index, flags: 4, moby: h, o_class: 0, sound_class: 0, pos: p.hero.pos, tick: p.ticks };
                p.hero_loops[k] = sys.play_class_sound(&ev, Some(h), &listener(&p.camera), &mut rng);
            }
            // `0x2a6420`: the footsteps, `0x360920`: level def `class·4 + foot·2 + variant` past the level's
            // footstep base, on his moby. Not ported: the boots' variant (0x18b0b8).
            for foot in p.hero.footsteps(&after) {
                let i = mobys.footstep_base + p.hero.footstep as usize * 4 + foot as usize * 2;
                let Some(def) = sys.data.sounds.level_defs.get(i).copied() else { continue };
                let owner = rc_game::audio::voices::Owner { id: h as u32, privileged: true };
                let k = sys.slots.play(&def, 0, Some(owner), Some(p.hero.pos), None, 0x400, &listener(&p.camera), &mut rng);
                if let Some(log) = sys.play_log.as_mut() { log.push((p.ticks, rc_game::audio::class_sounds::FOOTSTEP_LOG_CLASS, i as i32, 0, k)); }
            }
            sys.rng = rng;
        }
    }
    // `0x2ae140` (in the hero's update): the trigger areas asked about list Ratchet when he is in them.
    p.game.area_tick(&mobys.level, p.hero.pos);
    if let Some(i) = mobys.hero_moby {
        if p.hero.damage.took || hit.is_some() { mobys.world.clear_hit(i); }
    }
    p.hero.damage.took = false;
    if let Some(t) = p.hero.show_health.take() { p.game.show_health = Some(t); }
    if let Some(t) = p.hero.show_bolts.take() { p.game.show_bolts = Some(t); }
    // `0x2a6dd0`'s write-back: Ratchet's moby takes his position, angles and rows.
    if let Some(h) = mobys.hero_moby.and_then(|i| mobys.world.mobys.get_mut(i)) {
        (h.pos, h.rot, h.rows) = (p.hero.pos, p.hero.rot, p.hero.rows);
        h.mode |= rc_game::moby::mode::KEEP_ROWS;
        // `0x2a94d8`: his shine distance.
        h.shine = 0x10;
        // `0x2b2b48` → `0x31a8e4`: his light from the level's light regions where he is.
        if let Some((light, ambient)) = rc_game::light::region_light(&mobys.level.light_regions, h.pos) { (h.light, h.ambient) = (light, ambient); }
    }
    // The item pass (`0x2b21f0`): the hand item placed at Ratchet's new pose and updated.
    {
        let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let mods = p.hero.posture.modifiers(&mobys.hero_targets);
        let hl = mobys.hero_layers(&p.hero.layers);
        let pose = crate::mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale, mods: &mods, layers: &hl };
        // The items' sounds (the wrench's hits and whirr) play like the loop's.
        let sound = audio.as_deref_mut().map(|o| crate::mobys::HostSound { system: o.system(), listener: listener(&p.camera), tick: p.ticks, loops: &mut mobys.loop_slots });
        let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound };
        // The write-back's `0x313dc0`: his moby's sphere and grid cells follow him (the hits find him there).
        if let Some(i) = mobys.hero_moby { mobys.world.rebuild(i, &host); }
        let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut ctx = rc_game::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level, parts: parts.as_deref_mut().map(|s| &mut s.sys), view: None };
        rc_game::hero::items::item_pass(&mut mobys.world, &mut ctx);
        // The swim's effects (crate::hero::water's requests of this tick), into the particle pool.
        rc_game::hero::water::fx_pass(&mobys.world, &mut ctx);
    }
    // The use stance's sequence for Ratchet (`0x2af5e0` / `0x2af848` call `0x2d2508` from the item pass).
    if let Some((blend, seq)) = p.hero.items.hero_anim.take() {
        let mut ctl = p.anim.ctl(class);
        p.hero.set_anim(&mut ctl, blend, seq, 0);
    }
    // `0x2afc38`: his pose layers (the holding layers, the face), after the item pass.
    rc_game::hero::layers::upkeep(&mut p.hero, &p.anim.ctl(class));
    // `UpdateParts` (`0x331150`), after the hero.
    if let Some(s) = parts.as_deref_mut() {
        let aux = rc_game::particles::PartAux { cam_yaw: p.camera.yaw(), water_z: p.hero.surf.water_z, coll: Some(coll) };
        rc_game::particles::update_parts(&mut s.sys, &mobys.world, p.camera.pos, p.hero.pos, &mut p.game.rng, &aux);
    }
    // The draw callbacks the updates registered, called by the frame's draw (rc_game::fx).
    {
        // The callbacks read joints (0x0b0b's glows) through the loop's host, as the draw reads the mobys' poses.
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        let (tx, ty) = crate::game_camera::projection_tans(None);
        let [fwd, left, up] = p.camera.rows;
        let view = rc_game::particles::BSphereView::from_camera(p.camera.pos, fwd, left, up, tx, ty);
        p.fx_quads = rc_game::fx::callbacks(&mut mobys.world, &host, &mut p.game, p.hero.state, &view, p.camera.pos);
    }
    // The point lights the mobys merge (crate::moby_render).
    if let Some(l) = lights.as_deref_mut() {
        let now: Vec<_> = mobys.world.point_lights.active().copied().collect();
        if l.0 != now { l.0 = now; }
    }
    // `0x2b09e0`: the items take Ratchet's light word and ambient (`+0x38..0x3f`) every tick.
    if let Some((light, ambient)) = mobys.hero_moby.and_then(|i| mobys.world.mobys.get(i)).map(|h| (h.light, h.ambient)) {
        for id in [p.hero.items.hand, p.hero.items.head, p.hero.items.feet[0], p.hero.items.feet[1]].into_iter().flatten() {
            if let Some(m) = mobys.world.mobys.get_mut(id) { (m.light, m.ambient) = (light, ambient); }
        }
    }
    // The head item's pose (slot 2, placed by the item pass): from his head.
    {
        let mods = p.hero.posture.modifiers(&mobys.hero_targets);
        let hl = mobys.hero_layers(&p.hero.layers);
        let pose = crate::mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale, mods: &mods, layers: &hl };
        mobys.skin_worn(lv, &crate::mobys::Mobys::worn(&p.hero.items), Some(&pose));
    }
    if mobys.world.changed {
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        mobys.snap = Some(crate::mobys::take_snapshot(&mobys.world, &mobys.coll, &host));
        mobys.world.changed = false;
    }
    if let Some(o) = occl.as_mut() {
        for &ii in &mobys.left_out { o.set_hidden(ii, true); }
        for m in &mobys.world.mobys {
            let Some(ii) = m.instance else { continue };
            let hidden = m.state >= 0x80 || m.mode & 1 != 0;
            if m.update.is_some() || m.drive {
                o.drive(ii, m.pos, m.rows, m.scale, crate::mobys::moby_lights(&lv.mobys, m.rows, m.light, m.ambient).as_ref(), hidden);
                o.game_look(ii, m.alpha, m.mode, m.glow, m.draw_dist, m.shine);
            } else {
                o.set_hidden(ii, hidden);
                o.set_shine(ii, m.shine);
            }
        }
    }
    canopy_tick(mobys, lv, occl.as_deref(), anim.as_deref_mut(), None, p.camera.pos, p.game.scenes.mode);
    {
        let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut inp = cam_input(&p.hero, Some(coll), [p.pad.rx.to_f32(), p.pad.ry.to_f32()]);
        (inp.scene, inp.hero_moby) = (scene.as_ref(), mobys.hero_moby);
        inp.level = p.game.level;
        inp.strafe_held = p.pad.held & (rc_game::pad::button::L2 | rc_game::pad::button::R2) != 0;
        inp.ground = p.hero.ground_moby.and_then(|i| mobys.world.mobys.get(i).map(|m| (i, m.o_class, m.pos[2])));
        // The Camera page's options (rc_game::menus::opt, in the card's save data).
        if let Some(c) = card.as_deref() {
            use rc_game::menus::opt;
            let w = |a: u32| c.save.i32(rc_game::save::Scope::Global, a);
            (inp.pitch_normal, inp.yaw_normal, inp.speed) = (w(opt::PITCH_NORMAL) != 0, w(opt::YAW_NORMAL) != 0, w(opt::CAM_SPEED).clamp(0, 2) as usize);
        }
        // The placed views' hooks (`0x2d8618`'s choice, before the current camera's update).
        let l1 = p.pad.held & rc_game::pad::button::L1 != 0;
        let (frame, mode) = (p.game.frame, p.game.scenes.mode);
        p.game.cameras.placed_views(&mut p.camera, &p.hero, &mobys.level, frame, mode, l1);
        p.camera.update(&inp, &mut p.game.cam_tweaks);
    }
    // The rest of the camera update `0x2d9f38`: the fade back in (`0x2d9d98`), the level cameras' choice and the
    // current one's update (`0x2d8618`: the fly-bys).
    p.game.scenes.fade_step();
    let g = &mut p.game;
    let mut cam = g.cameras.tick(&p.hero, &mobys.level, (p.camera.pos, p.camera.rows), &mut g.scenes.fade, &mut g.scenes.fade_in);
    // The ship's camera (class 0x19, rc_game::ship): its update flies the ship and is the view.
    if p.game.cameras.current_class() == rc_game::ship::CLASS {
        if p.ship.is_none() {
            if let Some((pos, rot, idx)) = p.game.cameras.current_record(0x20) {
                let ms = &mobys.world.mobys;
                let ship = ms.iter().position(|m| m.index as i32 == idx && m.o_class == rc_game::ship::SHIP_CLASS).or_else(|| ms.iter().position(|m| m.o_class == rc_game::ship::SHIP_CLASS && m.state < 0x80));
                let mut f = rc_game::ship::Flight::new(pos, rot, ship);
                // The fight's sphere (the mission object 0x0eee's `+0x68`[0]).
                let sphere = ms.iter().find(|m| m.o_class == rc_game::ship::MISSION_CLASS).map(|m| m.pv_i(0));
                f.arena = sphere.and_then(|i| usize::try_from(i).ok()).and_then(|i| mobys.level.spheres.get(i)).map(|v| {
                    let m = &v.matrix;
                    ([m[3][0], m[3][1], m[3][2]], (m[0][0] * m[0][0] + m[0][1] * m[0][1] + m[0][2] * m[0][2]).sqrt())
                });
                p.ship = Some(f);
            }
        }
        let heard = listener(&p.camera);
        if let Some(f) = p.ship.as_mut() {
            let sound = audio.as_deref_mut().map(|o| crate::mobys::HostSound { system: o.system(), listener: heard, tick: p.ticks, loops: &mut mobys.loop_slots });
            let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: f.pos, camera_rows: f.rows, sound };
            let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
            f.tick(&p.pad, &mut mobys.world, &mut host, Some(coll), scene.as_ref());
            // `0x307d80`: the mission's help texts.
            if let Some((text, slot)) = f.help.take() {
                let shown = p.game.help(text, slot);
                if text == 0xa16 && (shown || p.game.help_state != 0) { f.opening_shown = true; }
            }
            // The shakes asked (`0x1b91a0`) shake this view with the others' (`0x2d9f38`, below).
            cam.view = Some((f.pos, f.rows));
            // The ship's flame (its draw callback `0x411c78`) on the ship as this tick left it, its shield's bubble and the
            // dust streaming past; the bolts' panel's ticks.
            if let Some(s) = f.ship { rc_game::ship::draw_flame(&mobys.world, s, f.pos, &mut p.game.rng, &mut p.fx_quads); }
            f.draw_shield(&mobys.world, f.pos, &mut p.fx_quads);
            f.draw_dust(&mobys.world, f.pos, &mut p.game.rng, &mut p.fx_quads);
            f.draw_scrape(f.pos, &mut p.game.rng, &mut p.fx_quads);
            if 0 < p.game.space_bolts_shown { p.game.space_bolts_shown -= 1; }
            // `0x2c63f0`: the ship's death starts the fight again; `0x307bf0(2, 1)`: the fight won, Maktar Nebula.
            if f.restart && p.game.next_level.is_none() { p.game.go_to_level(p.game.level, false); }
            if f.done && p.game.next_level.is_none() {
                // `0x1a7c09` = 1 (Wupash's byte of `0x1a7bf0`): the galactic map's way to Maktar (`0x33c0c0`) no longer
                // goes through the fight.
                if let Some(v) = p.game.visited.get_mut(25) { *v = 1; }
                p.game.go_to_level(2, true);
            }
            p.game.scenes.fade = f.fade;
            // The mobys' distances, the sounds and the draw callbacks go by this camera next tick.
            (p.camera.pos, p.camera.rows) = (f.pos, f.rows);
        }
    } else {
        p.ship = None;
    }
    if let Some(st) = cam.hero_state { p.hero.request_state(st, true); }
    if let Some(on) = cam.letterbox { p.game.scenes.letterbox = on; }
    if cam.follow_reset { p.camera = FollowCamera::new(&cam_input(&p.hero, Some(coll), [0.0; 2])); }
    // The script camera's hand-over with its pose kept (`0x2d8168` with the release flag 3 / 5, the init's `0x4247a0`).
    if let Some((pos, rows)) = cam.follow_pose { p.camera = FollowCamera::from_pose(&cam_input(&p.hero, Some(coll), [0.0; 2]), pos, rows); }
    p.game.scenes.bars_step();
    // △ opens the quick select's screen with Quick Select Pause off (the hero's transitions `0x2cb650`), then its frame
    // (the GUI screens' pass) before the HUD's.
    if !qs_pause && p.pad.pressed & rc_game::pad::button::TRIANGLE != 0 && 7 < p.game.frame && p.hero.no_menu == 0 && p.hero.items.no_use == 0 && p.hero.items.flying != 2 && p.game.prompt.owner == 0 {
        p.qs.open();
    }
    quick_select_frame(&mut p.qs, &mut p.qs_paused, &p.pad, &mut p.game, &mut p.hero, &mobys.level.items, qs_pause);
    // The HUD (`0x443c08`) with this tick's requests.
    let (sb, sh) = (p.game.show_bolts.take(), p.game.show_health.take());
    // `0x2fa770`: the held item when it has ammo (not with the items off, `0x18c0d4`).
    let it = &p.hero.items;
    let weapon = rc_game::hero::items::row(it.item).filter(|r| r.ammo && it.ready == 2 && p.hero.mode == 0).map(|r| (r.icon, p.game.ammo.get(it.item.max(0) as usize).copied().unwrap_or(0), r.max_ammo));
    p.hud.tick(p.hero.hp, p.game.max_health, sb, sh, weapon);
    // The boss bar's own update (`0x2f2458`): its ticks left counted down, gone at 0.
    p.game.boss_bar = p.game.boss_bar.and_then(|(f, t)| (1 < t).then_some((f, t - 1)));
    // The message boxes (`0x4451e8`'s part, after the panel).
    if let Some(g) = p.glyphs.as_deref() { p.boxes.tick(&mut p.game.messages, &p.texts, g); }
    // The help panel (`0x2f5680`).
    p.help.tick(&mut p.game, p.pad.pressed & rc_game::pad::button::TRIANGLE != 0, &p.texts);
    // The sound step (`sound_update` after the camera): the slots follow their owners (the mobys' positions; a
    // deleted one stops its sound), heard from this tick's camera; then the tick's 800 samples.
    if let Some(out) = audio.as_deref_mut() {
        let input = rc_game::audio::FrameInput { listener: listener(&p.camera), hero_pos: p.hero.pos };
        let world = &mobys.world;
        let owner = |id: u32| world.mobys.get(id as usize).filter(|m| m.state < 0x80).map(|m| m.pos);
        out.external = true;
        let (sys, buf) = out.parts();
        let mut rng = sys.rng;
        sys.game_frame_with(&input, p.ticks as u32 + 1, &mut rng, &owner);
        sys.rng = rng;
        sys.render(rc_game::audio::SAMPLES_PER_FRAME, buf);
        out.push_frame();
    }
    // A pose with nothing to show (a dev check): the snapshot blend without its snapshot, or a sequence the class
    // has not got.
    {
        let st = p.anim.state;
        let missing = |q: u8| q != rc_formats::moby_anim::SNAPSHOT_SEQ && class.sequence(q).is_none();
        if (st.seq_a == rc_formats::moby_anim::SNAPSHOT_SEQ && p.anim.snapshot.is_none()) || missing(st.seq_a) || missing(st.seq_b) {
            println!("tick {}: Ratchet's pose has nothing to show: state {:#x} seq {}/{} snapshot {}", p.ticks, p.hero.state, st.seq_a, st.seq_b, p.anim.snapshot.is_some());
        }
    }
    if p.trace {
        let v = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let h = &p.hero;
        println!(
            "tick {}: state {:#x}/{} pos ({:.2}, {:.2}, {:.2}) vel ({:.3}, {:.3}, {:.3}) air {} h {:.2} slope {:.2} seq {}/{} f {:.1} spd {:.2} cap {} at ({:.2}, {:.2}, {:.2}) n ({:.2}, {:.2}, {:.2}) capsule {:.2}/{:.2}/{:.2} cam ({:.2}, {:.2}, {:.2}) yaw {:.2} pitch {:.2}",
            p.ticks, h.state, h.substate, h.pos[0], h.pos[1], h.pos[2], h.vel[0], h.vel[1], h.vel[2], h.air_ticks, h.height, h.slope, v.seq_a, v.seq_b, v.frame, h.anim_speed,
            h.cap_hit, h.contact_point[0], h.contact_point[1], h.contact_point[2], h.contact_normal[0], h.contact_normal[1], h.contact_normal[2], h.cap_bottom, h.cap_top, h.cap_radius,
            p.camera.pos[0], p.camera.pos[1], p.camera.pos[2], p.camera.yaw(), p.camera.pitch()
        );
    }
    if let Some(o) = occl.as_mut() {
        let lights = mobys.hero_moby.and_then(|i| mobys.world.mobys.get(i)).and_then(|h| crate::mobys::moby_lights(&lv.mobys, p.hero.rows, h.light, h.ambient));
        o.drive(p.instance, p.hero.pos, p.hero.rows, p.scale, lights.as_ref(), false);
        if let (Some(a), Some(k)) = (anim.as_mut(), o.anim_index(p.instance)) {
            let mut s = p.anim.state;
            s.skip_advance = true;
            if let Some(i) = a.instances.get_mut(k) { i.state = s; }
            if let Some(slot) = a.snapshots.get_mut(k) { *slot = p.anim.snapshot.clone(); }
            // His posture's joint modifiers (rc_game::hero::posture).
            if let (Some(mods), Some(m)) = (a.mods.get_mut(k), p.mobys.as_ref()) { *mods = p.hero.posture.modifiers(&m.hero_targets); }
            // His pose layers (rc_game::hero::layers).
            if let (Some(l), Some(m)) = (a.layers.get_mut(k), p.mobys.as_ref()) { *l = m.hero_layers(&p.hero.layers); }
        }
    }
    // `0x2e5df8`'s view distance: the far plane and the mobys' cap.
    let far = p.game.view_far * 1024.0;
    crate::moby_lod::set_draw_cap(p.game.moby_draw_cap());
    // `0x2d9f38`'s shakes on the frame's eye and rows (rc_game::camera::Shakes; the cameras keep their own).
    let (view, fwd) = match cam.view {
        Some((mut pos, mut rows)) => {
            p.game.shakes.apply(&mut pos, &mut rows);
            // The fly-by's FOV is its own (`0x1bd2f0`); the script camera keeps the play's.
            let tan = if p.game.cameras.current_class() == rc_game::cameras::CLASS_FLYBY { p.game.cameras.tan } else { play_camera::GAME_TAN_HALF_FOV };
            (PlayView { view: scene_view(&rc_game::scene_camera::SceneCamera { eye: pos, rows, tan_half_fov: tan, cut: false }), tan_half_fov: tan, far }, rows[0])
        }
        None => {
            let (mut pos, mut rows) = (p.camera.pos, p.camera.rows);
            p.game.shakes.apply(&mut pos, &mut rows);
            (PlayView { view: camera_view(&p.camera, pos, rows), tan_half_fov: play_camera::GAME_TAN_HALF_FOV, far }, rows[0])
        }
    };
    commands.insert_resource(view);
    // The frame's sky draw `0x2fd348`: the shells turn by the view's forward (rc_game::sky).
    p.game.sky.frame(fwd);
    commands.insert_resource(crate::sky_render::GameSkyRotations(p.game.sky.angles.iter().map(|&a| crate::sky_render::euler_rotation(a)).collect()));
    p.ticks += 1;
    // `0x2b31f0`'s respawn request: Ratchet back at his start with full health (the checkpoints and the fade are not
    // ported); the world keeps its state.
    // `0x2b31f0`'s respawn request: the level restarts from the checkpoint (the spawn points', else Ratchet's start):
    // the objects spawn again by their rules (the ones gone for good stay out), Ratchet there with full health. The
    // fade is not ported.
    if p.hero.damage.dead && p.death_fade.is_none() { p.death_fade = Some(0); }
}

/// `0x2e7c90(0x10)`'s frames.
const DEATH_FADE: u32 = 16;

/// The death's restart (mode 0 with `0x18c0e1` set: `0x304660(0, 1)`, `0x32eb88`): the objects spawn again by their
/// rules (the ones gone for good stay out), Ratchet at the checkpoint (the spawn points', else his start) with full
/// health; the game keeps the rest (bolts, items, ammo).
fn death_reset(p: &mut Play, lv: &crate::level_load::LoadedLevel) {
    let inst = &lv.mobys.instances[p.instance];
    let armour = p.hero.damage.armour;
    let (at, rot) = p.game.checkpoint.unwrap_or((inst.position, inst.rotation));
    p.game.restart();
    p.mobys = None;
    p.hero = Hero::spawn(at, rot[2]);
    p.hero.hp = p.game.max_health;
    p.hero.damage.armour = armour;
    p.anim = RatchetAnim { curves: &rc_game::hero::CURVES, ..RatchetAnim::new(&lv.mobys.anim[p.class]) };
    p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
    p.game.show_health = Some(0x168);
}

/// The scene's audio frame (the world runs in a scene: `0x3716f8`; the level change's flight and story chain, mode 6):
/// the EE sound step heard from the scene camera, then the frame's samples.
fn scene_audio_frame(p: &Play, audio: Option<&mut crate::audio_out::AudioOut>) {
    let Some(out) = audio else { return };
    let l = p.scene_listener.unwrap_or_else(|| listener(&p.camera));
    let input = rc_game::audio::FrameInput { listener: l, hero_pos: p.hero.pos };
    let owner = |id: u32| p.mobys.as_ref().and_then(|m| m.world.mobys.get(id as usize)).filter(|m| m.state < 0x80).map(|m| m.pos);
    out.external = true;
    let (sys, buf) = out.parts();
    let mut rng = sys.rng;
    sys.scene_frame_with(true, &input, p.ticks as u32 + 1, &mut rng, &owner, buf);
    sys.rng = rng;
    out.push_frame();
}

/// `0x32c858`'s changes (rc_game::scene): a scene's end and start, a movie's start.
fn mode_changes(p: &mut Play, lv: &mut crate::level_load::LoadedLevel) {
    for c in p.game.scenes.switch(p.hero.hp) {
        match c {
            rc_game::scene::Change::EndedScene => {
                end_scene(p, lv);
                travel_scene_ended(p, lv);
            }
            rc_game::scene::Change::StartScene(id) => start_scene(p, lv, id),
            rc_game::scene::Change::StartMovie(id) => {
                println!("play: movie {id:#x}");
                p.movie_started = true;
                p.movie = Some(crate::front_end::Playing::level(id as u32, rc_formats::disc::scene::record_of(id) as usize));
            }
        }
    }
}

/// `0x367400` / `0x365b10` / `0x365960`: the flight starts: its lump (rc_formats::disc::space, the kind of the trip),
/// the destination's arrival pictures (its level file's lump 10), the flight's sky in place of the level's
/// (crate::sky_render::FlightSky), the first variant's scene far above the level (`rc_game::travel::FLIGHT_FRAME`).
fn flight_start(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, commands: &mut Commands) {
    use rc_formats::disc::{space, toc::Toc};
    let dest = p.game.travel.dest;
    let root = crate::level_load::extracted_root();
    let read = || -> anyhow::Result<(space::Transition, Option<rc_formats::disc::level::PlanetPictures>)> {
        let hdr = crate::disc_source::read(&root, "RC2.HDR")?;
        let kind = rc_game::travel::flight_kind(p.game.level, dest);
        let lump = space::transition(&Toc::parse(&hdr)?, &crate::disc_source::read(&root, "G/SPACE.WAD")?, kind)?;
        let file = crate::disc_source::read(&root, &crate::level_wad::file_name(dest.max(0) as u32))?;
        let pictures = rc_formats::disc::level::LevelWad::parse(&file)?.planet_pictures()?;
        Ok((lump, pictures))
    };
    let (lump, pictures) = match read() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("travel: the flight's lump not read ({e:#}): no flight");
            return;
        }
    };
    match lump.sky() {
        Ok((sky, textures)) => commands.insert_resource(crate::sky_render::FlightSky(crate::sky_render::LevelSky { level: 0, sky, textures })),
        Err(e) => eprintln!("travel: the flight's sky not read ({e:#}): the level's stays"),
    }
    let flight = rc_game::travel::Flight::start(&mut p.game, dest);
    println!("travel: tick {}: the flight to level {dest} (lump kind {}, variant {})", p.ticks, lump.kind, flight.variant);
    let v = flight.variant;
    p.game.travel.flight = Some(flight);
    p.flight = Some(FlightRun { lump, pictures, pictures_written: false });
    flight_variant(p, lv, v);
}

/// `0x365960` / `0x363360`'s variant start: its scene from the lump, played in the flight's frame (mode 6 sub 2).
fn flight_variant(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, v: usize) {
    let Some(f) = p.flight.as_ref() else { return };
    let scene = match f.lump.variant(v) {
        Ok(s) => std::sync::Arc::new(s),
        Err(e) => {
            eprintln!("travel: the flight's variant {v} not read ({e:#})");
            if let Some(fl) = p.game.travel.flight.as_mut() { fl.done = true; }
            return;
        }
    };
    println!("travel: tick {}: the flight's variant {v}: {} ticks, actors {:?}", p.ticks, scene.end_tick(), scene.actor_classes());
    p.space_scene = Some(scene);
    p.game.scenes.space = Some(rc_game::scene::SpaceScene { sub: 2, skip_at: None });
    p.game.travel.frame = Some(rc_game::travel::FLIGHT_FRAME);
    start_scene(p, lv, rc_game::travel::SCENE_ID_BASE + 0x10 + v as i32);
}

/// `0x364260` sub 2 → `0x363360`: a tick of the flight: the variant's scene, the sky's drift and turn, the ship
/// actor's scale in the arrival's last 40 ticks, the planet picture and the caption, and at a variant's end the next
/// (or none: true, the flight is over). The old level's world runs under it as the original's does.
#[allow(clippy::too_many_arguments)]
fn flight_tick(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, commands: &mut Commands, occl: Option<&mut MobyOcclusion>, anim: Option<&mut MobyAnim>, parts: Option<&mut rc_game::particles::Particles>, audio: Option<&mut crate::audio_out::AudioOut>, loaded: bool) -> bool {
    use rc_game::travel::FLIGHT_FRAME;
    if !p.game.travel.flight.as_ref().is_some_and(|f| !f.done) { return true; }
    // The story chain (`0x3671b8`) plays through the scenes' modes (rc_game::scene: the queue, its skip), then the
    // flight goes on.
    if p.chain {
        mode_changes(p, lv);
        match p.game.scenes.mode {
            rc_game::scene::SCENE => scene_tick(p, lv, commands, occl, anim, parts),
            rc_game::scene::MOVIE => movie_tick(p, audio),
            _ => {}
        }
        let s = &p.game.scenes;
        if s.mode != rc_game::scene::PLAY || s.pending != rc_game::scene::NONE || s.player.is_some() || p.movie.is_some() { return false; }
        p.chain = false;
        p.game.travel.frame = Some(FLIGHT_FRAME);
        return flight_variant_next(p, lv, loaded);
    }
    if p.game.scenes.player.is_none() {
        // The variant's scene did not start (no actors to make, say): on to the next.
        return flight_next(p, lv, loaded);
    }
    scene_tick(p, lv, commands, occl, anim, parts);
    let (tick, end, done) = p.game.scenes.player.as_ref().map_or((0, 1, true), |pl| (pl.tick, pl.end_tick(), pl.done));
    let Some(mut fl) = p.game.travel.flight.take() else { return true };
    let scale = fl.tick(tick, end);
    // The sky's shells: their angles with the variant's turn, the drift (crate::sky_render).
    let pose: Vec<(Quat, [f32; 3])> = (0..8).map(|k| (crate::sky_render::euler_rotation(fl.shell_angles(k)), fl.sky_translation)).collect();
    commands.insert_resource(crate::sky_render::FlightSkyPose(pose));
    // The actors (and the ship's parts) shrink into the planet in the arrival's last 40 ticks; then, the ship there
    // without the trip kind's own moby (kind 0), the flight ran (`0x152c2c` = 1): the landing on arrival.
    if scale < 1.0 && fl.kind == 0 { p.game.travel.flight_ran = true; }
    if scale < 1.0 {
        if let (Some(mobys), Some(pl)) = (p.mobys.as_mut(), p.game.scenes.player.as_ref()) {
            let ids: Vec<usize> = pl.actors.iter().map(|a| a.0).filter(|&a| a != usize::MAX).chain(p.game.ship_parts.iter().copied()).collect();
            for id in ids {
                let Some(class) = mobys.world.mobys.get(id).map(|m| m.o_class) else { continue };
                let base = mobys.world.class(class).map_or(1.0, |c| c.scale);
                if let Some(m) = mobys.world.mobys.get_mut(id) { m.scale = base * scale; }
            }
        }
    }
    // The planet picture (variant 4: a world quad the ship flies toward) and the caption (`0x363188`, `0x3632c0`).
    p.flight_quads.clear();
    if let (Some(corners), Some((eye, rows, tan))) = (fl.picture_corners(), p.flight_cam) {
        let (cx, cy) = (rc_game::gui::SCREEN_W * 0.5, rc_game::gui::SCREEN_H * 0.5);
        let project = |q: [f32; 3]| -> Option<[f32; 2]> {
            let d = [q[0] - eye[0], q[1] - eye[1], q[2] - eye[2]];
            let dot = |r: [f32; 3]| r[0] * d[0] + r[1] * d[1] + r[2] * d[2];
            let z = dot(rows[0]);
            if z <= 0.0 { return None; }
            Some([cx - dot(rows[1]) / z * (cx / tan), cy - dot(rows[2]) / z * (cy / (tan * 0.775))])
        };
        let pts: Vec<[f32; 2]> = corners.iter().filter_map(|&c| project(c)).collect();
        if pts.len() == 4 {
            let (pw, ph) = p.flight.as_ref().and_then(|f| f.pictures.as_ref()).and_then(|pp| pp.picture.as_ref()).map_or((256.0, 256.0), |t| (t.width.min(256) as f32, t.height.min(256) as f32));
            p.flight_quads.push(rc_game::gui::Quad { pos: [pts[0], pts[1], pts[2], pts[3]], uv: [[0.0, 0.0], [pw, 0.0], [0.0, ph], [pw, ph]], tex: rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(2)), rgba: [0x80, 0x80, 0x80, 0x80] });
        }
    }
    let alpha = fl.caption_alpha(tick);
    if 0 < alpha {
        let [x, y] = rc_game::travel::CAPTION_AT;
        let [w, h] = rc_game::travel::CAPTION_SIZE;
        p.flight_quads.push(rc_game::gui::Quad { pos: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]], uv: [[0.0, 0.0], [w, 0.0], [0.0, h], [w, h]], tex: rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(3)), rgba: [0x80, 0x80, 0x80, alpha] });
    }
    let _ = FLIGHT_FRAME;
    p.game.travel.flight = Some(fl);
    if done { return flight_next(p, lv, loaded); }
    false
}

/// A variant's end (`0x363360`): its actors go; a story chain picked (`gp−0x2d80`) plays (`0x3671b8`: the ids it marks
/// seen, then its scenes and movies queued and started, `0x2f4ee8` / `0x2f5030`) and is cleared; then the next variant
/// or the end of the flight.
fn flight_next(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, loaded: bool) -> bool {
    if p.game.scenes.player.is_some() { end_scene(p, lv); }
    p.game.scenes.mode = rc_game::scene::PLAY;
    p.game.scenes.space = None;
    let story = std::mem::take(&mut p.game.travel.story);
    if story != 0 {
        let (marks, plays) = rc_game::travel::story_chain(story);
        for &id in marks { if let Some(s) = p.game.scenes_seen.get_mut(id as usize) { *s = true; } }
        for &id in plays { p.game.scenes.queue.push(id); }
        let hp = p.hero.hp.max(1);
        if p.game.scenes.start_queue(hp) {
            println!("travel: tick {}: the story chain {story:#x}: {plays:x?}", p.ticks);
            p.chain = true;
            return false;
        }
    }
    flight_variant_next(p, lv, loaded)
}

/// The next variant, or the end of the flight.
fn flight_variant_next(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, loaded: bool) -> bool {
    let Some(mut fl) = p.game.travel.flight.take() else { return true };
    let next = fl.next_variant(&mut p.game, loaded);
    let over = fl.done;
    p.game.travel.flight = Some(fl);
    match next {
        Some(v) => {
            flight_variant(p, lv, v);
            false
        }
        None => {
            println!("travel: tick {}: the flight ended", p.ticks);
            p.game.travel.flight = None;
            p.game.travel.frame = None;
            over
        }
    }
}

/// A ship scene's lump of `SPACE.WAD` (rc_formats::disc::space: the landing 0 / 2, the take-off 1 / 3).
fn space_scene(lump: usize) -> Option<std::sync::Arc<rc_formats::scene::Scene>> {
    let root = crate::level_load::extracted_root();
    let read = || -> anyhow::Result<rc_formats::scene::Scene> {
        use rc_formats::disc::{space, toc::Toc};
        let hdr = crate::disc_source::read(&root, "RC2.HDR")?;
        Ok(space::scene(&Toc::parse(&hdr)?, &crate::disc_source::read(&root, "G/SPACE.WAD")?, lump)?)
    };
    match read() {
        Ok(s) => Some(std::sync::Arc::new(s)),
        Err(e) => {
            eprintln!("travel: the ship's scene lump {lump} not read ({e:#})");
            None
        }
    }
}

/// The ground under `at` (`0x31ce20(0.5, at)`).
fn ground_under(lv: &crate::level_load::LoadedLevel, at: [f32; 3]) -> Option<f32> {
    let c = lv.collision.as_ref()?;
    rc_game::collision::coll_line(c, [at[0], at[1], at[2] + 0.5], [at[0], at[1], 0.01], rc_game::collision::QueryFlags(2)).map(|o| o.point[2])
}

/// `0x361d08(sub)`: a ship scene set up: the lump (the landing or the take-off, with Clank's variant), the parked ship
/// hidden, the scene asked for in the ship's frame behind a fade of `fade` frames (`0x305690`'s `0x2e7c90`: as the
/// scene switch's blackout, 0.1 a frame to 1.2). False when the level has no parked ship or the lump does not read.
fn travel_scene(p: &mut Play, lv: &crate::level_load::LoadedLevel, take_off: bool, fade: i32) -> bool {
    let Some(mobys) = p.mobys.as_mut() else { return false };
    let ground = |at: [f32; 3]| ground_under(lv, at);
    let Some(frame) = rc_game::travel::ship_frame(&mobys.world, &p.game, &ground) else {
        eprintln!("travel: no parked ship on this level: no ship scene");
        return false;
    };
    let lump = rc_game::travel::Travel::lump(take_off, p.game.clank);
    let Some(scene) = space_scene(lump) else { return false };
    if let Some(s) = p.game.travel.ship.and_then(|s| mobys.world.mobys.get_mut(s)) { s.mode |= 1; }
    p.space_scene = Some(scene);
    p.game.scenes.space = Some(rc_game::scene::SpaceScene { sub: if take_off { 0 } else { 3 }, skip_at: None });
    p.game.travel.frame = Some(frame);
    p.game.travel.phase = if take_off { rc_game::travel::Phase::TakeOff } else { rc_game::travel::Phase::Landing };
    // `0x361f98`: Ratchet's health at least 1 (the scene's request needs him alive).
    if p.hero.hp < 1 { p.hero.hp = 1; }
    p.game.scenes.zoom = 1.2 - 0.1 * fade as f32;
    let hp = p.hero.hp;
    let r = p.game.scenes.request(rc_game::scene::SCENE, true, rc_game::travel::SCENE_ID_BASE + lump as i32, 0, hp);
    println!("travel: tick {}: the ship's {} (lump {lump}, request {r}) at ({:.1}, {:.1}, {:.1}) yaw {:.2}", p.ticks, if take_off { "take-off" } else { "landing" }, frame.at[0], frame.at[1], frame.at[2], frame.yaw);
    r == 0
}

/// `0x361f98`: the take-off of △ at the hatch (the help panel closed `0x2f5310`, the level-start flag off).
fn travel_take_off(p: &mut Play, lv: &mut crate::level_load::LoadedLevel) {
    p.game.help_state = 0;
    p.game.help_req = None;
    p.game.travel.level_start = false;
    if !travel_scene(p, lv, true, 6) { p.game.travel.phase = rc_game::travel::Phase::Idle; }
}

/// `0x362010(dest)` for the current planet (a 2-frame fade) and `0x362138` at the level's start (no fade): the landing.
fn travel_landing(p: &mut Play, lv: &crate::level_load::LoadedLevel, level_start: bool) {
    p.game.travel.level_start = level_start;
    if !travel_scene(p, lv, false, if level_start { 0 } else { 2 }) {
        // No scene: mode 6 is left at once (`0x3621c8`).
        if !level_start { rc_game::travel::hatch_exit(&mut p.game, &mut p.hero); }
        p.game.travel.phase = rc_game::travel::Phase::Idle;
    }
}

/// `entry`'s `0x32c5f0(6, 1, 6, 0, 0)`: the landing on arrival after a trip by the ship (rc_game::travel).
fn travel_arrival(p: &mut Play, lv: &crate::level_load::LoadedLevel) {
    if p.game.travel.lands_on_arrival(&p.game) {
        println!("travel: arrived by the ship on level {}: the landing", p.game.level);
        travel_landing(p, lv, true);
    }
    // `0x367400` / `0x304420`: the flight's flag is for one arrival.
    p.game.travel.flight_ran = false;
}

/// `0x364260`'s end of a ship scene: after the take-off the pages (mode 3 kind 4); after the landing mode 6 is left
/// (`0x3621c8`: Ratchet shown, at the hatch spot after a landing from the menu, where the level put him on arrival,
/// the screen in over 8 ticks). The parked ship shows again.
fn travel_scene_ended(p: &mut Play, lv: &mut crate::level_load::LoadedLevel) {
    use rc_game::travel::Phase;
    let phase = p.game.travel.phase;
    if !matches!(phase, Phase::TakeOff | Phase::Landing) { return; }
    p.game.travel.frame = None;
    if let Some(m) = p.mobys.as_mut() {
        if let Some(s) = p.game.travel.ship.and_then(|s| m.world.mobys.get_mut(s)) { s.mode &= !1; }
    }
    match phase {
        Phase::TakeOff => {
            println!("travel: tick {}: the take-off ended: the galaxy map", p.ticks);
            // The missions tables (`0x281560`, Aranos's overlay) for the planet page.
            if p.game.travel.menu.missions.is_empty() {
                match crate::level_wad::overlay(&crate::level_load::extracted_root(), 0) {
                    Ok(ov) => p.game.travel.menu.missions = rc_game::travel::missions::read_tables(&|a, n| crate::level_wad::overlay_read(&ov, a, n)),
                    Err(e) => eprintln!("travel: no missions tables ({e:#})"),
                }
            }
            rc_game::travel::open_menu(&mut p.game);
        }
        _ => {
            println!("travel: tick {}: the landing ended", p.ticks);
            if p.game.travel.level_start {
                p.hero.request_state(0, true);
                p.game.scenes.fade = 1.0;
                p.game.scenes.fade_in = 1.0 / 8.0;
            } else {
                rc_game::travel::hatch_exit(&mut p.game, &mut p.hero);
            }
            p.game.travel.level_start = false;
            p.game.travel.phase = Phase::Idle;
            p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
        }
    }
}

/// The pages' frame (mode 3 kind 4) and the fly-away's (mode 6 sub 1), in place of the main loop's (rc_game::travel).
fn travel_frame(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, commands: &mut Commands, occl: Option<&mut MobyOcclusion>) {
    use rc_game::travel::{MenuOut, Phase};
    match p.game.travel.phase {
        Phase::Menu => {
            rc_game::travel::menu_frame(&mut p.game);
            for o in rc_game::travel::menu_tick(&mut p.game, &p.pad) {
                match o {
                    MenuOut::Sound(_) => {}
                    MenuOut::Movie(id) => {
                        // `0x2f4ee8` + `0x2f5030`: the movie plays over the pages (the tick and the draw return to
                        // them after it).
                        p.game.scenes.queue.push(id);
                        p.game.scenes.start_queue(p.hero.hp);
                    }
                    MenuOut::Travel(dest) => {
                        println!("travel: tick {}: the pages close for planet {dest}", p.ticks);
                        if dest == p.game.level {
                            travel_landing(p, lv, false);
                        } else if let Some(m) = p.mobys.as_mut() {
                            rc_game::travel::fly_away_start(&mut p.game, &mut m.world, &m.level, dest);
                        } else {
                            p.game.go_to_level(dest, true);
                            p.game.travel.phase = Phase::Idle;
                        }
                        break;
                    }
                    MenuOut::Leave(dest) => {
                        println!("travel: tick {}: the level change to {dest} at once", p.ticks);
                        p.game.go_to_level(dest, true);
                        p.game.travel.phase = Phase::Idle;
                        break;
                    }
                }
            }
        }
        Phase::FlyAway => {
            let Some(mobys) = p.mobys.as_mut() else { return };
            let mut riders = Vec::new();
            let out = rc_game::travel::fly_away_tick(&mut p.game, &mut mobys.world, &mobys.level, &p.pad, &p.hero, &mut riders);
            // Ratchet on his seat (the rider `0x311320(0)` of the original: the port moves him).
            for (m, pos, rot) in riders {
                if m.is_none() {
                    (p.hero.pos, p.hero.rot) = (pos, rot);
                    p.hero.rows = rc_game::hero::physics::euler_rows(rot);
                }
            }
            if let Some(h) = mobys.hero_moby.and_then(|i| mobys.world.mobys.get_mut(i)) { (h.pos, h.rot, h.rows) = (p.hero.pos, p.hero.rot, p.hero.rows); }
            if let Some(o) = occl { o.drive(p.instance, p.hero.pos, p.hero.rows, p.scale, None, false); }
            // The camera: the cameras' choice (the script camera at its targets) into the view.
            let g = &mut p.game;
            let cam = g.cameras.tick(&p.hero, &mobys.level, (p.camera.pos, p.camera.rows), &mut g.scenes.fade, &mut g.scenes.fade_in);
            let far = g.view_far * 1024.0;
            if let Some((pos, rows)) = cam.view {
                commands.insert_resource(PlayView { view: scene_view(&rc_game::scene_camera::SceneCamera { eye: pos, rows, tan_half_fov: g.cameras.tan, cut: false }), tan_half_fov: g.cameras.tan, far });
            }
            if out.leave {
                // `0x1a8ed4 = 1`: the level's main loop ends and `0x367400` loads the destination behind the flight; the
                // autosave `0x30d658(0, dest)` of `0x362010` goes with it.
                let dest = p.game.travel.dest;
                println!("travel: tick {}: the fly-away ended: level {dest}", p.ticks);
                p.game.travel.phase = Phase::Idle;
                p.game.travel.fly = None;
                p.game.go_to_level(dest, true);
            }
        }
        _ => {}
    }
}

/// A movie's frame (mode 1, `0x371e50`): its picture at the movie's rate; at its end, or skipped (Start alone on
/// Aranos, else Start with L1 L2 R1 R2 held), the chain's next or back. No sound yet.
fn movie_tick(p: &mut Play, mut audio: Option<&mut crate::audio_out::AudioOut>) {
    let start = p.pad.pressed & rc_game::pad::button::START != 0;
    // The level the game is in (`0x1a79f8`): during a level change already the destination (`0x367400`).
    let level = if p.leaving.is_some() { p.game.travel.dest } else { p.game.level };
    let skip = start && (level <= 0 || p.pad.held & 0xf == 0xf);
    let over = match p.movie.as_mut() {
        Some(m) => {
            if let Some(pic) = m.advance() { p.movie_picture = Some(pic); }
            m.ended
        }
        None => true,
    };
    // The movie's audio, one vsync's 800 samples a frame (R&C1's `mix_movie`).
    if let (Some(out), Some(m)) = (audio.as_deref_mut(), p.movie.as_ref()) {
        out.external = true;
        let chunk = m.audio_frame();
        let mut buf = std::mem::take(&mut out.buf);
        buf.clear();
        out.system().mix_movie(&chunk, &mut buf);
        out.buf = buf;
        out.push_frame();
    }
    if over || skip {
        p.movie = None;
        // Its end: the level's music again (`movie_exit`).
        if let Some(out) = audio { out.system().movie_exit(); }
        let hp = p.hero.hp;
        let g = &mut p.game;
        g.scenes.movie_done(skip, hp, &mut g.scenes_seen);
    }
}

/// The scene file of scene `id` and its table: the level's `G/SCENEn.WAD`, or for the story scenes between the
/// levels `G/SPACE.WAD` with the `SPACE` header's records (rc_formats::disc::space::story_scenes); loaded once each.
fn scene_file(p: &mut Play, id: i32) -> anyhow::Result<&(Vec<u8>, rc_formats::disc::scene::SceneWad)> {
    use anyhow::Context;
    let shared = rc_formats::disc::scene::is_shared(id);
    let slot = if shared { &mut p.shared_scenes } else { &mut p.scene_wad };
    if slot.is_none() {
        let root = crate::level_load::extracted_root();
        *slot = Some(if shared {
            let hdr = crate::disc_source::read(&root, "RC2.HDR")?;
            let table = rc_formats::disc::space::story_scenes(&rc_formats::disc::toc::Toc::parse(&hdr)?)?;
            (crate::disc_source::read(&root, "G/SPACE.WAD")?, table)
        } else {
            let path = root.join(format!("G/SCENE{}.WAD", crate::level_load::level_index()));
            let wad = std::fs::read(&path).with_context(|| format!("{}", path.display()))?;
            let table = rc_formats::disc::scene::SceneWad::parse(&wad)?;
            (wad, table)
        });
    }
    Ok(slot.as_ref().expect("scene file"))
}

/// Scene `id` (its record in its scene file, [`scene_file`]).
fn load_scene(p: &mut Play, id: i32) -> anyhow::Result<Option<rc_formats::scene::Scene>> {
    let (wad, table) = scene_file(p, id)?;
    Ok(table.scene(wad, rc_formats::disc::scene::record_of(id) as usize)?)
}

/// A scene's speech: its record's English stream (`speech[0]`, sectors from `SCENEn.WAD`'s start), a `VAGp` file
/// (its size from the header's big-endian data size).
fn scene_speech(p: &Play, id: i32) -> Option<std::sync::Arc<[u8]>> {
    let (wad, table) = if rc_formats::disc::scene::is_shared(id) { p.shared_scenes.as_ref()? } else { p.scene_wad.as_ref()? };
    let r = table.records.get(rc_formats::disc::scene::record_of(id) as usize)?;
    let at = *r.speech.first().filter(|&&s| s != 0)? as usize * 2048;
    let head = wad.get(at..at + 0x30)?;
    if &head[..4] != b"VAGp" { return None; }
    let size = u32::from_be_bytes(head[0xc..0x10].try_into().ok()?) as usize;
    Some(std::sync::Arc::from(wad.get(at..(at + 0x30 + size).min(wad.len()))?))
}

/// `0x370e08` / `0x305428`: the scene's actors made (class 0: Ratchet's moby; each in the next sequence slot of its
/// class, which plays the chunk's streamed sequence), the 0xcb mobys and the hand item hidden, the scene started.
fn start_scene(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, id: i32) {
    // A ship scene (rc_game::travel) comes loaded from `SPACE.WAD`.
    let loaded = match p.space_scene.take() {
        Some(s) => Ok(Some(s)),
        None => load_scene(p, id).map(|o| o.map(std::sync::Arc::new)),
    };
    let scene = match loaded {
        Ok(Some(s)) => s,
        r => {
            match r {
                Err(e) => eprintln!("play: scene {id:#x}: {e:#}: skipped"),
                _ => eprintln!("play: scene {id:#x}: no such scene: skipped"),
            }
            let hp = p.hero.hp;
            p.game.scenes.player = None;
            if !p.game.scenes.start_queue(hp) { p.game.scenes.pop(hp); }
            return;
        }
    };
    let Some(mobys) = p.mobys.as_mut() else { return };
    use rc_game::moby::mode;
    let mut actors = Vec::new();
    for a in &scene.chunks[0].actors {
        let o_class = match a.class { 0x215 | 0x10d1 => 0x0d54, c => c as i16 };
        let Some(ci) = lv.mobys.classes.iter().position(|c| c.o_class == o_class as i32) else {
            eprintln!("play: scene {id:#x}: actor class {o_class:#x} is not in the level");
            actors.push((usize::MAX, 0xff));
            continue;
        };
        let m = if o_class == 0 {
            match mobys.hero_moby {
                // `0x305428` makes his moby anew (`0x311418`): a class with metal packets gets 0x18.
                Some(h) => {
                    if lv.mobys.classes[ci].class.header.metal_count != 0 { mobys.world.mobys[h].shine = 0x18; }
                    h
                }
                None => {
                    actors.push((usize::MAX, 0xff));
                    continue;
                }
            }
        } else {
            let m = mobys.world.create(o_class);
            // `0x305428`: Ratchet's light word and ambient (none: ambient 0x38).
            let lit = mobys.hero_moby.and_then(|i| mobys.world.mobys.get(i)).map_or((0, [0x38; 3]), |h| (h.light, h.ambient));
            let a = &mut mobys.world.mobys[m];
            (a.light, a.ambient) = lit;
            (a.draw_dist, a.drawn, a.update_dist, a.has_collision) = (0x1ff, 1, 0xff, false);
            a.mode |= mode::NO_UPDATE | mode::NO_MATRIX | mode::KEEP_ROWS;
            m
        };
        // `0x305428`: the slot is the class's sequence count (`class+0xc`), which goes up by one per actor.
        let count = lv.mobys.classes[ci].class.header.sequence_count as usize;
        let taken = p.scene_slots.iter().filter(|s| s.0 == ci).count();
        let slot = count + taken;
        let anim = &mut lv.mobys.anim[ci];
        if anim.sequences.len() <= slot { anim.sequences.resize(slot + 1, None); }
        let before = anim.sequences[slot].replace(a.sequence.clone());
        p.scene_slots.push((ci, slot, before));
        let slot = slot as u8;
        if o_class != 0 {
            // Its first pose (the scene's first tick poses it again).
            let state = rc_formats::moby_anim::AnimState { seq_a: slot, frame_a: 0, seq_b: slot, frame_b: 1, t: 0.0, speed: 0.0, rate: 0.0, flags: 0, trigger_count: 0, skip_advance: true };
            mobys.created.insert(m, crate::mobys::Created { class: ci, state, snapshot: None, serial: mobys.world.mobys[m].serial });
            if let Some(q) = a.positions.first() { mobys.world.mobys[m].pos = [q[0], q[1], q[2]]; }
        }
        actors.push((m, slot));
    }
    for m in mobys.world.mobys.iter_mut().filter(|m| m.o_class == 0xcb && m.state < 0x80) { m.mode |= 0x80; }
    if let Some(h) = p.hero.items.hand.and_then(|h| mobys.world.mobys.get_mut(h)) { h.mode |= 1; }
    println!("play: scene {id:#x}: {} ticks, actors {:?}", scene.end_tick(), scene.actor_classes());
    // `0x370e08`: the music paused, the scene's speech streamed (its record's English VAG; crate::audio_out).
    use rc_game::audio::scene::{post, SceneAudioCmd};
    post(SceneAudioCmd::PauseMusic);
    if let Some(vag) = scene_speech(p, id) { post(SceneAudioCmd::Speech { vag }); }
    let g = &mut p.game;
    g.scenes.start_scene(id, scene, actors, &mut g.scenes_seen);
}

/// `0x3711e8`: the actors deleted and their class slots given back, the hidden mobys shown, Ratchet idle on the
/// ground under him, the follow camera behind him.
fn end_scene(p: &mut Play, lv: &mut crate::level_load::LoadedLevel) {
    let actors = p.game.scenes.player.as_ref().map(|pl| pl.actors.clone()).unwrap_or_default();
    if let Some(mobys) = p.mobys.as_mut() {
        for (m, _) in actors {
            if m == usize::MAX || Some(m) == mobys.hero_moby { continue; }
            mobys.world.delete(m);
            mobys.created.remove(&m);
        }
        for m in mobys.world.mobys.iter_mut().filter(|m| m.o_class == 0xcb && m.state < 0x80) { m.mode &= !0x80; }
        if let Some(h) = p.hero.items.hand.and_then(|h| mobys.world.mobys.get_mut(h)) { h.mode &= !1; }
    }
    for (ci, slot, before) in p.scene_slots.drain(..).rev() {
        if let Some(q) = lv.mobys.anim.get_mut(ci).and_then(|a| a.sequences.get_mut(slot)) { *q = before; }
    }
    let at = p.hero.pos;
    let ground = lv.collision.as_ref().and_then(|c| rc_game::collision::coll_line(c, [at[0], at[1], at[2] + 0.5], [at[0], at[1], 0.01], rc_game::collision::QueryFlags(2))).map(|o| o.point[2]);
    p.game.scenes.end_scene(&mut p.hero, ground);
    // `0x3711e8`: the speech stopped, the music back 30 ticks later.
    rc_game::audio::scene::post(rc_game::audio::scene::SceneAudioCmd::StopSpeech);
    rc_game::audio::scene::post(rc_game::audio::scene::SceneAudioCmd::ResumeMusic { after: 30 });
    p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
}

/// A scene's tick (`0x3716f8`): the moby loop (Ratchet's update does nothing in a scene), then the scene's: the
/// actors posed in their class slots at their tracks, Ratchet's instance at his, the scene camera.
fn scene_tick(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, commands: &mut Commands, mut occl: Option<&mut MobyOcclusion>, mut anim: Option<&mut MobyAnim>, mut parts: Option<&mut rc_game::particles::Particles>) {
    let Some(coll) = lv.collision.as_ref() else { return };
    let Some(mobys) = p.mobys.as_mut() else { return };
    {
        let class = &lv.mobys.anim[p.class];
        let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let mods = p.hero.posture.modifiers(&mobys.hero_targets);
        let hl = mobys.hero_layers(&p.hero.layers);
        let pose = crate::mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale, mods: &mods, layers: &hl };
        let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut ctx = rc_game::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level, parts: parts.as_deref_mut(), view: None };
        mobys.world.tick(p.camera.pos, &mut ctx);
    }
    let hp = p.hero.hp;
    let level = p.game.level;
    p.game.scenes.bars_step();
    let g = &mut p.game;
    let Some(mut frame) = g.scenes.tick(&p.pad, level, false, hp, &mut g.scenes_seen) else { return };
    // A ship scene plays in the ship's frame (`0x364260`: the camera's and the actors' places turned by its yaw and
    // moved to it, rc_game::travel).
    if let Some(f) = g.travel.frame {
        frame.camera.eye = f.to_world(frame.camera.eye);
        frame.camera.rows = frame.camera.rows.map(|r| f.turn(r));
        for pose in frame.poses.iter_mut() { pose.pos = f.to_world(pose.pos); }
    }
    // The flight (`0x363360`): the view's tangent at least the variant's (`0x1bd2f0`).
    if let Some(fl) = g.travel.flight.as_ref() { frame.camera.tan_half_fov = frame.camera.tan_half_fov.max(fl.min_tan); }
    let flight_cam = g.travel.flight.is_some().then(|| (frame.camera.eye, frame.camera.rows, frame.camera.tan_half_fov));
    let player = g.scenes.player.as_ref().expect("a scene");
    let classes = &lv.mobys.classes;
    let ci_of = |o: i16| classes.iter().position(|c| c.o_class == o as i32);
    if frame.new_chunk {
        let chunk = &player.scene.chunks[player.chunk];
        for (k, &(m, slot)) in player.actors.iter().enumerate() {
            if m == usize::MAX { continue; }
            let Some(ci) = mobys.world.mobys.get(m).and_then(|x| ci_of(x.o_class)) else { continue };
            if let (Some(q), Some(a)) = (lv.mobys.anim[ci].sequences.get_mut(slot as usize), chunk.actors.get(k)) { *q = Some(a.sequence.clone()); }
        }
    }
    // The actors' rows: the identity, or the ship's yaw in a ship scene (`0x364260`: `+0xf8` = the ship's).
    let identity = g.travel.scene_rows();
    let mut hero_pose = None;
    for pose in &frame.poses {
        let state = rc_formats::moby_anim::AnimState { seq_a: pose.slot, frame_a: pose.frame_a, seq_b: pose.slot, frame_b: pose.frame_b, t: pose.t, speed: 0.0, rate: 0.0, flags: 0, trigger_count: 0, skip_advance: true };
        if Some(pose.moby) == mobys.hero_moby {
            hero_pose = Some((pose.pos, state));
            continue;
        }
        let Some(m) = mobys.world.mobys.get_mut(pose.moby) else { continue };
        (m.pos, m.rows) = (pose.pos, identity);
        let Some(ci) = ci_of(pose.o_class) else { continue };
        let serial = m.serial;
        mobys.created.insert(pose.moby, crate::mobys::Created { class: ci, state, snapshot: None, serial });
    }
    // `0x2d51f8` → `0x2d4fc8`: the head item on the scene's Ratchet (at his track with the scene's sequence and
    // his head's scale), else on Ratchet as he stands.
    {
        let (pos, rows, state, snapshot, mods) = match hero_pose {
            Some((pos, state)) => (pos, identity, state, None, scene_hero_mods(&mobys.hero_targets, p.hero.items.feet[0].is_some())),
            None => (p.hero.pos, p.hero.rows, p.anim.state, p.anim.snapshot.clone(), p.hero.posture.modifiers(&mobys.hero_targets)),
        };
        // The scene's Ratchet takes none of play's pose layers (below).
        let scene_hero = hero_pose.is_some();
        let hl = if scene_hero { Vec::new() } else { mobys.hero_layers(&p.hero.layers) };
        let pose = crate::mobys::HeroPose { class: p.class, state: &state, snapshot: snapshot.as_ref(), pos, rows, scale: p.scale, mods: &mods, layers: &hl };
        {
            let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(&lv.mobys.anim[p.class]));
            let mut host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
            let scene = crate::mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
            let mut ctx = rc_game::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level, parts: parts.as_deref_mut(), view: None };
            rc_game::hero::items::head_pass(&mut mobys.world, &mut ctx);
            rc_game::hero::items::feet_pass(&mut mobys.world, &mut ctx);
            // The ship's parts on the scene's ship actor (`0x2fd818` / `0x364260` → `0x361050`).
            let ship = ctx.game.scenes.player.as_ref().and_then(|pl| pl.actors.iter().map(|a| a.0).find(|&a| mobys.world.mobys.get(a).is_some_and(|m| m.o_class == 0xd54)));
            if let Some(s) = ship { rc_game::moby::classes::props::ride_ship_parts(&mut mobys.world, s, &mut ctx); }
        }
        let hl = if scene_hero { Vec::new() } else { mobys.hero_layers(&p.hero.layers) };
        let pose = crate::mobys::HeroPose { class: p.class, state: &state, snapshot: snapshot.as_ref(), pos, rows, scale: p.scale, mods: &mods, layers: &hl };
        mobys.skin_worn(lv, &crate::mobys::Mobys::worn(&p.hero.items), Some(&pose));
    }
    if let Some(o) = occl.as_mut() {
        for &ii in &mobys.left_out { o.set_hidden(ii, true); }
        for m in &mobys.world.mobys {
            let Some(ii) = m.instance else { continue };
            let hidden = m.state >= 0x80 || m.mode & 1 != 0;
            if m.update.is_some() || m.drive {
                o.drive(ii, m.pos, m.rows, m.scale, crate::mobys::moby_lights(&lv.mobys, m.rows, m.light, m.ambient).as_ref(), hidden);
                o.game_look(ii, m.alpha, m.mode, m.glow, m.draw_dist, m.shine);
            } else {
                o.set_hidden(ii, hidden);
                o.set_shine(ii, m.shine);
            }
        }
        // Ratchet: at his track with the scene's sequence and his head's scale (`0x2d51f8`), else as he stands.
        let (pos, rows, state) = match hero_pose {
            Some((pos, state)) => (pos, identity, state),
            None => (p.hero.pos, p.hero.rows, p.anim.state),
        };
        let lights = mobys.hero_moby.and_then(|i| mobys.world.mobys.get(i)).and_then(|h| crate::mobys::moby_lights(&lv.mobys, rows, h.light, h.ambient));
        o.drive(p.instance, pos, rows, p.scale, lights.as_ref(), false);
        if let (Some(a), Some(k)) = (anim.as_mut(), o.anim_index(p.instance)) {
            let mut s = state;
            s.skip_advance = true;
            if let Some(i) = a.instances.get_mut(k) { i.state = s; }
            if hero_pose.is_some() {
                if let Some(slot) = a.snapshots.get_mut(k) { *slot = None; }
                if let Some(mods) = a.mods.get_mut(k) { *mods = scene_hero_mods(&mobys.hero_targets, p.hero.items.feet[0].is_some()); }
                // The scene's Ratchet is his moby made anew (`0x305428`: `0x311418(moby, 0)`), posed by the scene alone:
                // none of play's pose layers (the weapon's hold, the face's look at an enemy) go with him.
                if let Some(l) = a.layers.get_mut(k) { l.clear(); }
            }
        }
    }
    if mobys.world.changed {
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: p.camera.pos, camera_rows: p.camera.rows, sound: None };
        mobys.snap = Some(crate::mobys::take_snapshot(&mobys.world, &mobys.coll, &host));
        mobys.world.changed = false;
    }
    let ship = p.game.scenes.player.as_ref().and_then(|pl| pl.actors.iter().map(|a| a.0).find(|&a| mobys.world.mobys.get(a).is_some_and(|m| m.o_class == 0xd54)));
    canopy_tick(mobys, lv, occl.as_deref(), anim.as_deref_mut(), ship, frame.camera.eye, p.game.scenes.mode);
    if let Some(sys) = parts.as_deref_mut() {
        let aux = rc_game::particles::PartAux { cam_yaw: p.camera.yaw(), water_z: p.hero.surf.water_z, coll: Some(coll) };
        rc_game::particles::update_parts(sys, &mobys.world, frame.camera.eye, p.hero.pos, &mut p.game.rng, &aux);
    }
    // The frame's draw calls the callbacks the scene tick's updates registered too (rc_game::fx): the enemies hidden
    // for the scene registered none, so play's last glows go.
    {
        let host = crate::mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera: frame.camera.eye, camera_rows: frame.camera.rows, sound: None };
        let (tx, ty) = crate::game_camera::projection_tans(None);
        let [fwd, left, up] = frame.camera.rows;
        let view = rc_game::particles::BSphereView::from_camera(frame.camera.eye, fwd, left, up, tx, ty);
        p.fx_quads = rc_game::fx::callbacks(&mut mobys.world, &host, &mut p.game, p.hero.state, &view, frame.camera.eye);
    }
    commands.insert_resource(PlayView { view: scene_view(&frame.camera), tan_half_fov: frame.camera.tan_half_fov, far: p.game.view_far * 1024.0 });
    p.flight_cam = flight_cam;
    p.scene_listener = Some(rc_game::audio::voices::Listener { pos: frame.camera.eye, rows: frame.camera.rows, underwater: false, water_height: 0.0 });
}

/// The scene's Ratchet's joint modifiers: his head's scale (list 24: 0.849 × 0.849 × 0.875) and, under the worn feet,
/// his feet hidden (`0x2ac218`'s 0.01 on the feet records, lists 22 / 23).
fn scene_hero_mods(targets: &[u8], feet: bool) -> Vec<rc_formats::moby_anim::JointModifier> {
    use rc_formats::moby_anim::JointModifier;
    let t = |l: usize| targets.get(l).copied().filter(|&t| t != 0xff);
    let mut mods: Vec<JointModifier> = t(24).map(|j| JointModifier { scale: [0.848_75, 0.848_75, 0.875], ..JointModifier::compose(j) }).into_iter().collect();
    if feet { mods.extend([22, 23].into_iter().filter_map(t).map(|j| JointModifier { scale: [0.01; 3], ..JointModifier::compose(j) })); }
    mods
}

fn scene_view(c: &rc_game::scene_camera::SceneCamera) -> CameraView {
    let v = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let f = c.rows[0];
    let euler = [Pf::ZERO, Pf::f(-f[2].clamp(-1.0, 1.0).asin()), Pf::f(f[1].atan2(f[0])), Pf::ZERO];
    CameraView { pos: v(c.eye), euler, rows: c.rows.map(v), class: 0, flyby_phase: 0 }
}

/// The in-game HUD's layer: the front end's texture set (its fonts, slot 1) and the level's HUD icons after it.
#[derive(Resource)]
struct GameHud {
    layer: crate::hud_render::Layer,
    data: crate::front_end::Data,
    /// `(icon, frame)` → layer texture and texel size.
    icons: std::collections::HashMap<(u16, u16), (usize, [f32; 2])>,
    /// FX texture → layer texture and texel size (the markers', rc_game::hud::marker).
    fx: std::collections::HashMap<u16, (usize, [f32; 2])>,
    /// The level's pause tables (its item records, the Weapons grid).
    tables: rc_formats::disc::menu_tables::MenuTables,
    pictures: ItemPictures,
    /// The flight's planet picture (256 × 256) and caption (256 × 32) textures (rc_game::travel's flight, written at
    /// its start from the destination's lump 10).
    flight_slots: [usize; 2],
    /// The ship's planet page's picture (rc_formats::disc::planet_picture: the surface and the shading, 128 × 128,
    /// `0x355508`), the level written, and the levels' lumps in `HUD.WAD`.
    planet_slots: [usize; 2],
    planet_shown: Option<i32>,
    planet_parts: Vec<Option<rc_formats::disc::toc::Part>>,
}

/// The pause pages' item pictures: two 256 × 256 textures of the layer a page's pictures are read into when they
/// change (the picture panel `0x3548f0` streams them from `HUD.WAD` the same way).
struct ItemPictures {
    slots: [usize; 2],
    shown: [Option<(PictureTable, u16)>; 2],
    /// What each slot was last asked for (a failed read is not retried).
    asked: [Option<(PictureTable, u16)>; 2],
    parts: std::collections::HashMap<(PictureTable, u16), rc_formats::disc::toc::Part>,
}

use rc_formats::disc::toc::PictureTable;

impl ItemPictures {
    fn new(root: &std::path::Path, slots: [usize; 2]) -> ItemPictures {
        let mut parts = std::collections::HashMap::new();
        match crate::disc_source::read(root, "RC2.HDR").map_err(anyhow::Error::from).and_then(|h| {
            let t = rc_formats::disc::toc::Toc::parse(&h)?;
            for p in PictureTable::ALL {
                for i in 0..p.rows() {
                    if let Some(part) = t.item_picture(p, i)? { parts.insert((p, i as u16), part); }
                }
            }
            Ok(())
        }) {
            Ok(()) => {}
            Err(e) => eprintln!("play: no item pictures ({e:#})"),
        }
        ItemPictures { slots, shown: [None; 2], asked: [None; 2], parts }
    }

    /// Picture `want` into slot `k` when it is not there yet.
    fn show(&mut self, k: usize, want: Option<(PictureTable, u16)>, layer: &crate::hud_render::Layer, images: &mut Assets<Image>) {
        if want.is_none() || want == self.asked[k] { return; }
        self.asked[k] = want;
        let Some(&part) = want.and_then(|w| self.parts.get(&w)) else { return };
        let read = || -> anyhow::Result<_> {
            let raw = crate::disc_source::read_range(&crate::level_load::extracted_root(), "G/HUD.WAD", part.bytes())?;
            Ok(rc_formats::disc::item_picture::parse(&raw)?)
        };
        match read() {
            // A smaller picture (row 0's 128 × 128) fills the slot's corner.
            Ok(p) if p.width <= 256 && p.height <= 256 => {
                let mut rgba = vec![0u8; 256 * 256 * 4];
                for y in 0..p.height as usize {
                    let w = p.width as usize * 4;
                    rgba[y * 1024..y * 1024 + w].copy_from_slice(&p.rgba[y * w..(y + 1) * w]);
                }
                layer.write(images, self.slots[k], &rgba);
                self.shown[k] = want;
            }
            Ok(p) => eprintln!("play: item picture {want:?} is {}×{}", p.width, p.height),
            Err(e) => eprintln!("play: item picture {want:?} ({e:#})"),
        }
    }
}

/// Level `index`'s pause tables, found from Aranos's (rc_formats::disc::menu_tables); none on a failure.
/// The ship's canopy tables in the level's overlay (rc_game::canopy: Aranos's addresses, relocated to the level's).
fn canopy_mesh(root: &std::path::Path, index: u32) -> Option<rc_game::canopy::CanopyMesh> {
    use rc_formats::level_overlay::{LevelOverlay, Relocation};
    let level = crate::level_wad::overlay(root, index).ok()?;
    let aranos = if index == 0 { level.clone() } else { crate::level_wad::overlay(root, 0).ok()? };
    let (t, r) = (LevelOverlay::parse(&level).ok()?, LevelOverlay::parse(&aranos).ok()?);
    let rel = Relocation::new(&r, &t);
    rc_game::canopy::CanopyMesh::read(|a| rel.data(a), |a, n| t.read(a, n).map(<[u8]>::to_vec))
}

pub(crate) fn menu_tables(root: &std::path::Path, index: u32) -> rc_formats::disc::menu_tables::MenuTables {
    let read = || -> anyhow::Result<_> {
        let level = crate::level_wad::overlay(root, index)?;
        let aranos = if index == 0 { level.clone() } else { crate::level_wad::overlay(root, 0)? };
        Ok(rc_formats::disc::menu_tables::MenuTables::read(&level, &aranos)?)
    };
    read().unwrap_or_else(|e| {
        eprintln!("play: no pause tables ({e:#})");
        Default::default()
    })
}

/// The HUD layer, made on the first frame with a game (composited on the main camera's UI, as crate::hud_render's
/// HUD is), then drawn each frame.
#[allow(clippy::too_many_arguments)]
fn draw_hud(
    mut commands: Commands,
    mut play: Option<ResMut<Play>>,
    hud: Option<ResMut<GameHud>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::hud_render::HudMaterial>>,
    mut composites: ResMut<Assets<crate::hud_render::HudComposite>>,
    flycams: Query<Entity, With<crate::fly_cam::FlyCam>>,
    level: Res<crate::Level>,
    card: Option<Res<crate::card::Card>>,
    gfx: Res<crate::graphics::GraphicsSettings>,
    display: Res<crate::display::DisplaySettings>,
) {
    let Some(p) = play.as_mut() else { return };
    let Some(mut hud) = hud else {
        let root = crate::level_load::extracted_root();
        let data = match crate::front_end::Data::load(&root) {
            Ok(d) => d,
            Err(e) => { eprintln!("play: no HUD ({e:#})"); return; }
        };
        let mut textures: Vec<rc_formats::texture::Texture> = data.textures().to_vec();
        let mut icons = std::collections::HashMap::new();
        let tables = menu_tables(&root, crate::level_load::level_index());
        match crate::level_wad::hud(&root, crate::level_load::level_index()) {
            Ok(h) => {
                // The bolt icon and the items' (the weapon panel, `ItemRow::icon`).
                for id in std::iter::once(0x7567u16).chain(rc_game::hero::items::ROWS.iter().map(|r| r.icon).filter(|&i| i != 0)) {
                    let f = h.icon_frame(id, 0);
                    if let Ok(t) = h.decode_frame_raw(f) {
                        icons.insert((id, 0), (textures.len(), [t.width as f32, t.height as f32]));
                        textures.push(t);
                    }
                }
                // The pause's pages (rc_game::menus): the Options and Help icons, the save slots' icons and picture.
                let menu = (0..8).flat_map(|f| [(0xea9eu16, f), (0xea9d, f)]).chain([(rc_game::menus::ICON_SLOT, 2), (rc_game::menus::ICON_SLOT, 3), (rc_game::menus::ICON_SLOT, 4), (rc_game::menus::ICON_PICTURE, 0)]);
                // The vendors' (rc_game::vendor): the armours' icons, the mods' frames 0..3, the platinum bolt.
                let menu = menu.chain((1..=4).map(|l| (rc_game::vendor::armor::ICON + l, 0))).chain((0..4).map(|f| (rc_game::vendor::mods::ICON, f))).chain([(rc_game::vendor::mods::PLATINUM_ICON, 0)]);
                // The item records' icons (the Weapons page's cells, every upgrade's).
                let menu = menu.chain(tables.items.iter().map(|r| (r.icon, 0)).filter(|&(i, _)| i != 0));
                // The grids' own icons (the plain cells, `0xea85`, item 7's `0xea9a`).
                // The ship's pages (rc_game::travel): the galaxy map's picture and its planet dots, the planet page's icon.
                let menu = menu.chain(tables.grids.iter().flatten().filter(|c| c.icon != 0).map(|c| (c.icon, c.frame))).chain([(0xea9a, 0), (0xe99a, 10), (0xe99e, 0), (0xe99e, 1), (0xe99e, 2), (0xeaa2, 0), (0xe99d, 1)]);
                // The space fights' HUD icons (rc_game::ship::HUD_ICONS).
                let menu = menu.chain((0..16).map(|f| (rc_game::ship::HUD_ICONS, f)));
                for (id, f) in menu {
                    if !h.has_icon(id) || icons.contains_key(&(id, f)) { continue; }
                    if let Ok(t) = h.decode_frame_raw(h.icon_frame(id, f as i32)) {
                        icons.insert((id, f), (textures.len(), [t.width as f32, t.height as f32]));
                        textures.push(t);
                    }
                }
            }
            Err(e) => eprintln!("play: no HUD icons ({e:#})"),
        }
        // The markers' FX textures (the Lancer's reticle) and the space fights' HUD's (rc_game::ship).
        let mut fx = std::collections::HashMap::new();
        for id in std::iter::once(0x26u16).chain(rc_game::ship::HUD_FX) {
            if let Some(t) = level.0.particles.textures.as_ref().and_then(|t| t.fx_textures.get(id as usize)).and_then(Option::as_ref) {
                fx.insert(id, (textures.len(), [t.width as f32, t.height as f32]));
                // The HUD's atlas holds the GS's bytes (0x80 = 1.0); the FX textures' alpha is decoded to 0..0xff (their
                // colours stay as the GS has them, up to 2.0).
                let mut t = t.clone();
                for px in t.rgba.as_chunks_mut::<4>().0 { px[3] = ((px[3] as u16 + 1) >> 1) as u8; }
                textures.push(t);
            }
        }
        // The pause pages' item pictures.
        let pictures = ItemPictures::new(&root, [textures.len(), textures.len() + 1]);
        for _ in 0..2 { textures.push(rc_formats::texture::Texture { width: 256, height: 256, rgba: vec![0; 256 * 256 * 4] }); }
        // The flight's planet picture and caption (`0x363188` / `0x3632c0`).
        let flight_slots = [textures.len(), textures.len() + 1];
        textures.push(rc_formats::texture::Texture { width: 256, height: 256, rgba: vec![0; 256 * 256 * 4] });
        textures.push(rc_formats::texture::Texture { width: 256, height: 32, rgba: vec![0; 256 * 32 * 4] });
        // The ship's planet page's picture (`0x351410` builds its textures from the level's lump).
        let planet_slots = [textures.len(), textures.len() + 1];
        for _ in 0..2 { textures.push(rc_formats::texture::Texture { width: 128, height: 128, rgba: vec![0; 128 * 128 * 4] }); }
        let planet_parts = crate::disc_source::read(&root, "RC2.HDR").map_err(anyhow::Error::from).and_then(|h| {
            let t = rc_formats::disc::toc::Toc::parse(&h)?;
            (0..0x1c).map(|l| Ok(t.planet_picture(l, false)?)).collect::<anyhow::Result<Vec<_>>>()
        }).unwrap_or_else(|e| { eprintln!("play: no planet pictures ({e:#})"); Vec::new() });
        // On the main camera's UI, as crate::hud_render's HUD is.
        let Some(cam) = flycams.iter().next() else { return };
        let scaling = crate::hud_render::scaling(crate::graphics::Hud::High);
        let layer = crate::hud_render::Layer::spawn(&mut commands, &mut images, &mut meshes, &mut materials, &mut composites, &textures, scaling, cam);
        commands.insert_resource(GameHud { layer, data, icons, fx, tables, pictures, flight_slots, planet_slots, planet_shown: None, planet_parts });
        return;
    };
    let p: &mut Play = p;
    let mut quads = Vec::new();
    if let Some(pic) = p.movie_picture.take() { hud.layer.write(&mut images, crate::front_end::MOVIE, &pic); }
    if let (Some(m), Some(c)) = (p.pause.as_ref(), card.as_deref()) {
        // The page loop's picture (`0x2ccc18`): the stopped game darkened, then the pages.
        rc_game::gui::fill(&mut quads, [0, 0, 0, 0x38]);
        let port = crate::port_options::values(&gfx, &display);
        let want = m.pictures(&rc_game::menus::View { card: &c.mc, save: &c.save, port: &port, option_rows: &[], tables: &hud.tables, pictures: [None; 2] });
        let h = &mut *hud;
        for (k, w) in want.into_iter().enumerate() { h.pictures.show(k, w, &h.layer, &mut images); }
        let view = rc_game::menus::View { card: &c.mc, save: &c.save, port: &port, option_rows: &[], tables: &hud.tables, pictures: hud.pictures.shown };
        let icon = |id: u16, f: u16| hud.icons.get(&(id, f)).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
        quads.extend(m.draw(hud.data.assets(), &view, &icon));
    } else if p.game.travel.phase == rc_game::travel::Phase::Menu && p.game.scenes.mode != rc_game::scene::MOVIE {
        // The ship's pages (mode 3 kind 4, rc_game::travel) over the stopped game darkened (`0x2ccc18`).
        rc_game::gui::fill(&mut quads, [0, 0, 0, 0x38]);
        let hud = &mut *hud;
        // The pages' streamed pictures (rc_game::travel::page_pictures): the map page's backdrop over the whole
        // screen (`0x3546e0`), the planet page's planet (`0x355508`: the surface inset and scrolling, wrapping
        // horizontally, under the shading).
        let pp = rc_game::travel::page_pictures(&p.game);
        let picture = |pos: [f32; 4], uv: [f32; 4], k: u8, alpha: u8| rc_game::gui::Quad {
            pos: [[pos[0], pos[1]], [pos[2], pos[1]], [pos[0], pos[3]], [pos[2], pos[3]]],
            uv: [[uv[0], uv[1]], [uv[2], uv[1]], [uv[0], uv[3]], [uv[2], uv[3]]],
            tex: rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(k)),
            rgba: [0x80, 0x80, 0x80, alpha],
        };
        if let Some((lvl, alpha)) = pp.galaxy {
            let want = Some((PictureTable::Galaxy, lvl as u16));
            hud.pictures.show(0, want, &hud.layer, &mut images);
            if hud.pictures.shown[0] == want && 0 < alpha {
                quads.push(picture([0.0, 0.0, rc_game::gui::SCREEN_W, rc_game::gui::SCREEN_H], [0.0, 0.0, 256.0, 128.0], 0, alpha));
            }
        }
        let a = hud.data.assets();
        let text = |id: u32| a.strings.get(id).to_vec();
        let mut lines = Vec::new();
        let mut texts = Vec::new();
        let width = |font: u8, t: &[u8]| rc_game::gui::text_width(if font == 2 { &a.glyphs_2 } else { &a.glyphs }, t, 1.0);
        let mut scroll = 0;
        let draws = rc_game::travel::menu_draws(&p.game, &text, &width, &mut lines, &mut texts, &mut scroll);
        p.game.travel.menu.mission_scroll += scroll;
        let icon = |id: u16, f: u16| hud.icons.get(&(id, f)).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
        quads.extend(rc_game::hud::quads(&draws, &a.sprites, &a.glyphs, &icon));
        // The planet widget draws its picture after its window (`0x435f10`: `0x431fa8`, then `0x355508`).
        if let Some(d) = pp.planet {
            if hud.planet_shown != Some(d.level) {
                hud.planet_shown = Some(d.level);
                let part = hud.planet_parts.get(d.level as usize).copied().flatten();
                let read = || -> anyhow::Result<_> {
                    let part = part.ok_or_else(|| anyhow::anyhow!("no lump"))?;
                    let raw = crate::disc_source::read_range(&crate::level_load::extracted_root(), "G/HUD.WAD", part.bytes())?;
                    Ok(rc_formats::disc::planet_picture::parse(&raw)?)
                };
                match read() {
                    Ok(pic) => {
                        // The layer takes raw GS bytes: `texture::scale_alpha` undone.
                        let gs = |t: &rc_formats::texture::Texture| -> Vec<u8> {
                            let mut out = t.rgba.clone();
                            out.resize(128 * 128 * 4, 0);
                            for a in out.iter_mut().skip(3).step_by(4) { *a = if *a == 0xff { 0x80 } else { *a / 2 }; }
                            out
                        };
                        hud.layer.write(&mut images, hud.planet_slots[0], &gs(&pic.surface));
                        hud.layer.write(&mut images, hud.planet_slots[1], &gs(&pic.shading));
                    }
                    Err(e) => eprintln!("play: planet picture {} ({e:#})", d.level),
                }
            }
            let [x, y, w, h] = d.rect;
            if d.kind == 0 {
                quads.push(picture([x, y, x + w, y + h], [0.0, 0.0, 128.0, 128.0], 4, 0x80));
            } else {
                let (ix, iy, iw, ih) = (x + d.inset, y + d.inset, w - 2.0 * d.inset, h - 2.0 * d.inset);
                let split = ix + iw * (128.0 - d.scroll) / 128.0;
                quads.push(picture([ix, iy, split, iy + ih], [d.scroll, 0.0, 128.0, 128.0], 4, 0x80));
                if 0.0 < d.scroll { quads.push(picture([split, iy, ix + iw, iy + ih], [0.0, 0.0, d.scroll, 128.0], 4, 0x80)); }
                quads.push(picture([x, y, x + w, y + h], [0.0, 0.0, 128.0, 128.0], 5, 0x80));
            }
        }
        // The pages' texts: the panel widgets' font 1 (the menus', slot 1), the list's font 2 (slot 2).
        for t in texts {
            if t.colour >> 24 == 0 { continue; }
            let (glyphs, slot) = if t.font == 2 { (&a.glyphs_2, 2) } else { (&a.glyphs, 1) };
            let w = rc_game::gui::text_width(glyphs, t.text.as_bytes(), t.scale);
            let x = match t.align { rc_game::hud::Align::Left => t.pos[0], rc_game::hud::Align::Centre => t.pos[0] - (w >> 1) as f32, rc_game::hud::Align::Right => t.pos[0] - w as f32 };
            rc_game::gui::text(&mut quads, glyphs, slot, t.text.as_bytes(), x, t.pos[1], t.scale, t.colour.to_le_bytes());
        }
        // `0x2fbfc8`: the lines, a pixel wide.
        for ([x0, y0, x1, y1], c) in lines {
            let (x1, y1) = (x1.max(x0 + 1.0), y1.max(y0 + 1.0));
            quads.push(rc_game::gui::Quad { pos: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]], uv: [[0.0; 2]; 4], tex: rc_game::gui::Tex::None, rgba: c.to_le_bytes() });
        }
    } else if p.game.scenes.mode == rc_game::scene::MOVIE {
        rc_game::gui::picture(&mut quads, rc_game::gui::Picture::Movie);
    } else if p.game.scenes.mode != rc_game::scene::SCENE && p.game.travel.phase != rc_game::travel::Phase::FlyAway {
        // In a scene and in the ship mode's fly-away the HUD and the help are not drawn (`0x2e6d88`, `0x3658e0`).
        // In a scene the HUD and the help are not drawn (`0x2e6d88`).
        // `0x2db598`: the tick's markers, then cleared (not while Ratchet is in state 0x6f).
        let markers = std::mem::take(&mut p.game.markers);
        if p.hero.state != 0x6f {
            let tex = |fx: u16| hud.fx.get(&fx).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
            let alive = |id: usize| p.mobys.as_ref().and_then(|m| m.world.mobys.get(id)).is_some_and(|m| m.state != 0xfe && m.state != 0xfd);
            quads.extend(rc_game::hud::marker_quads(&markers, p.camera.pos, p.camera.rows, crate::play_camera::GAME_TAN_HALF_FOV, &tex, &alive));
        }
        // The space fight's crosshair and radar.
        if let (Some(f), Some(m)) = (p.ship.as_ref(), p.mobys.as_ref()) {
            let tex = |fx: u16| hud.fx.get(&fx).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
            quads.extend(f.hud_quads(&m.world, crate::play_camera::GAME_TAN_HALF_FOV, &tex));
            // `0x40e7a8`: the ship's HUD, in its order.
            let strings = &hud.data.assets().strings;
            let label = |id: u32| String::from_utf8_lossy(strings.get(id)).trim_end_matches('\0').to_string();
            let icon = |id: u16, fr: u16| hud.icons.get(&(id, fr)).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
            for item in f.hud_items(&m.world, (p.game.space_bolts, p.game.space_bolts_shown), &label) {
                match item {
                    rc_game::ship::HudItem::Quad(q) => quads.push(q),
                    rc_game::ship::HudItem::Draw(d) => quads.extend(rc_game::hud::quads(std::slice::from_ref(&d), &hud.data.assets().sprites, &hud.data.assets().glyphs, &icon)),
                }
            }
            let tex = |fx: u16| hud.fx.get(&fx).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
            quads.extend(f.radar_quads(&m.world, &tex));
        }
        // In the space fights the ship's HUD stands for Ratchet's (rc_game::ship::Flight::status).
        let mut draws = if p.ship.is_some() { Vec::new() } else { p.hud.draw(p.game.bolts) };
        // The quick select's ring and texts (rc_game::quick_select).
        if let Some(m) = p.mobys.as_ref() {
            let slots = rc_game::quick_select::slots(&p.game, &m.level.items);
            let strings = &hud.data.assets().strings;
            p.qs.draw(&slots, &p.pad, &|id| strings.get(id).to_vec(), &mut draws);
        }
        let icon = |id: u16, f: u16| hud.icons.get(&(id, f)).map(|&(i, s)| (rc_game::gui::Tex::Slot(i), s));
        quads.extend(rc_game::hud::quads(&draws, &hud.data.assets().sprites, &hud.data.assets().glyphs, &icon));
        let a = hud.data.assets();
        // The action prompt's text (rc_game::prompt) and the vendor's menu (rc_game::vendor): the level's texts, else
        // the front end's strings.
        {
            let texts = |id: u32| rc_formats::strings::find_index(&p.texts, id as i32).map(|i| p.texts[i].text.clone()).unwrap_or_else(|| a.strings.get(id).to_vec());
            p.game.prompt.draw(&texts, &a.glyphs_2, &mut quads);
            if let (Some(v), Some(m)) = (p.vendor.menu.as_ref(), p.mobys.as_ref()) { v.draw(&p.game, &m.level.items, a, &icon, &texts, &mut quads); }
        }
        if p.glyphs.is_none() { p.glyphs = Some(Box::new(a.glyphs)); }
        p.boxes.draw(&mut quads, &a.glyphs, &a.sprites);
        quads.extend(p.help.draw(&p.game, &p.texts, &a.glyphs, &a.sprites));
    }
    // The flight between the planets (rc_game::travel, `0x363ba0`): the destination's pictures into their textures
    // once, then the planet picture quad and the caption over the flight's world.
    if let Some(f) = p.flight.as_mut() {
        if !f.pictures_written {
            f.pictures_written = true;
            if let Some(pp) = f.pictures.as_ref() {
                // The PIF's texels into the slot's corner, their alpha back to the GS's (`texture::scale_alpha`
                // undone: 0xff → 0x80, else halved; the layer takes raw GS bytes).
                let fit = |t: &rc_formats::texture::Texture, w: usize, h: usize| -> Vec<u8> {
                    let mut out = vec![0u8; w * h * 4];
                    for row in 0..(t.height as usize).min(h) {
                        let n = (t.width as usize).min(w) * 4;
                        let src = (row * t.width as usize * 4).min(t.rgba.len());
                        let len = n.min(t.rgba.len() - src);
                        out[row * w * 4..row * w * 4 + len].copy_from_slice(&t.rgba[src..src + len]);
                    }
                    for a in out.iter_mut().skip(3).step_by(4) { *a = if *a == 0xff { 0x80 } else { *a / 2 }; }
                    out
                };
                if let Some(t) = pp.picture.as_ref() { hud.layer.write(&mut images, hud.flight_slots[0], &fit(t, 256, 256)); }
                if let Some(t) = pp.captions.first() { hud.layer.write(&mut images, hud.flight_slots[1], &fit(t, 256, 32)); }
            }
        }
        quads.extend(p.flight_quads.iter().cloned());
    }
    // The letterbox and the scene fade are the frame draw's (`0x2e82c0`, `0x1b1860`): a movie's player (`0x366de8`)
    // draws its pictures alone, without them.
    let movie = p.game.scenes.mode == rc_game::scene::MOVIE;
    // The letterbox (`0x2e82c0`: black bars of `0x1b186c` pixels at the top and the bottom).
    let bars = if movie { 0.0 } else { p.game.scenes.bars as f32 };
    if 0.0 < bars {
        let (w, h) = (rc_game::gui::SCREEN_W, rc_game::gui::SCREEN_H);
        for (y0, y1) in [(0.0, bars), (h - bars, h)] {
            quads.push(rc_game::gui::Quad { pos: [[0.0, y0], [w, y0], [0.0, y1], [w, y1]], uv: [[0.0; 2]; 4], tex: rc_game::gui::Tex::None, rgba: [0, 0, 0, 0x80] });
        }
    }
    // The death's fade (`0x2e7c90(16)`: frame i of 16 drawn black at alpha `0x80 − (15 − i)·0x80/16`).
    if let Some(k) = p.death_fade {
        rc_game::gui::fill(&mut quads, [0, 0, 0, (0x80 - (DEATH_FADE - 1 - k) * 0x80 / DEATH_FADE) as u8]);
    }
    // The fade into and out of a scene (`0x1b1860`, 1 black).
    let fade = if movie { 0.0 } else { p.game.scenes.fade.clamp(0.0, 1.0) };
    if 0.0 < fade { rc_game::gui::fill(&mut quads, [0, 0, 0, (fade * 128.0) as u8]); }
    let mut d = crate::hud_render::Hud2d::default();
    for q in quads {
        let (tex, nearest) = match q.tex {
            rc_game::gui::Tex::Slot(i) => (crate::hud_render::Tex::Frame(i), false),
            rc_game::gui::Tex::Picture(rc_game::gui::Picture::Movie) => (crate::hud_render::Tex::Frame(crate::front_end::MOVIE), true),
            rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(k)) if k < 2 => (crate::hud_render::Tex::Frame(hud.pictures.slots[k as usize & 1]), false),
            // The flight's planet picture (2) and caption (3); the ship's planet page's surface (4) and shading (5).
            rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(k)) if k < 4 => (crate::hud_render::Tex::Frame(hud.flight_slots[(k as usize - 2) & 1]), false),
            rc_game::gui::Tex::Picture(rc_game::gui::Picture::Item(k)) => (crate::hud_render::Tex::Frame(hud.planet_slots[(k as usize - 4) & 1]), false),
            _ => (crate::hud_render::Tex::None, false),
        };
        d.screen_prim(tex, q.pos, q.uv, u32::from_le_bytes(q.rgba), false);
        if nearest { if let Some(p) = d.prims.last_mut() { p.nearest = true; } }
    }
    // The mesh lives only in the render world: replaced, not edited (as crate::hud_render does).
    let _ = meshes.insert(&hud.layer.mesh, hud.layer.build(&d));
}

/// The created mobys' draw (crate::mobys::CreatedDraw), made on the first frame with mobys.
#[allow(clippy::too_many_arguments)]
fn draw_created(
    mut commands: Commands,
    level: Res<crate::Level>,
    play: Option<Res<Play>>,
    draw: Option<ResMut<crate::mobys::CreatedDraw>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::moby_render::MobyMaterial>>,
    mut buffers: ResMut<Assets<bevy::render::storage::ShaderBuffer>>,
    mut transforms: Query<&mut Transform, Without<crate::fly_cam::FlyCam>>,
    cams: Query<(&Transform, &Projection), With<crate::fly_cam::FlyCam>>,
) {
    let Some(mobys) = play.as_ref().and_then(|p| p.mobys.as_ref()) else { return };
    let Some(mut draw) = draw else {
        commands.insert_resource(crate::mobys::CreatedDraw::new(&level.0, &mut buffers));
        return;
    };
    // The frame's camera for MobyProc's shine gate (crate::moby_lod).
    let view = cams.iter().next().map(|(t, p)| {
        let [fwd, left, up] = crate::game_camera::game_rows(t);
        (crate::game_camera::game_eye(t), crate::moby_lod::camera_rows(fwd, left, up), crate::game_camera::projection_tans(Some(p)))
    });
    draw.update(mobys, &level.0, &mut commands, &mut meshes, &mut images, &mut materials, &mut buffers, &mut transforms, view);
}

/// The ties and shrubs the level's objects moved, hid or recoloured (`MobyWorld::moved_ties` / `moved_shrubs` /
/// `hidden_ties` / `tie_colours`) to the tie and shrub renderers: the new matrix with the class sphere (scaled as the loaders' instance centre).
#[allow(clippy::too_many_arguments)]
fn move_instances(
    mut commands: Commands,
    level: Res<crate::Level>,
    play: Option<ResMut<Play>>,
    ties: Option<ResMut<crate::tie_lod::TieLodState>>,
    shrubs: Option<ResMut<crate::shrub_render::ShrubSway>>,
    mut buffers: ResMut<Assets<bevy::render::storage::ShaderBuffer>>,
    mut hidden: Local<std::collections::HashSet<usize>>,
) {
    let Some(mut p) = play else { return };
    let Some(mobys) = p.mobys.as_mut() else { return };
    let world = &mut mobys.world;
    let lv = &level.0;
    let scaled = |b: [f32; 4], s: f32| b.map(|x| x * s);
    if let Some(mut ties) = ties {
        // A new level's ties start shown.
        if ties.is_added() { hidden.clear(); }
        for ii in std::mem::take(&mut world.moved_ties) {
            let (Some(m), Some(Some(ci))) = (world.ties.get(ii), lv.ties.class_of.get(ii)) else { continue };
            let h = &lv.ties.classes[*ci].class.header;
            if let Some((model, es)) = ties.move_instance(ii, *m, scaled(h.bsphere, h.scale)) {
                let t = Transform::from_matrix(model);
                for e in es.to_vec() { commands.entity(e).insert(t); }
            }
        }
        for (ii, rgb) in std::mem::take(&mut world.tie_colours) { ties.set_colour(ii, rgb); }
        if world.hidden_ties != *hidden {
            for &ii in hidden.difference(&world.hidden_ties) { ties.set_hidden(ii, false); }
            for &ii in &world.hidden_ties { ties.set_hidden(ii, true); }
            *hidden = world.hidden_ties.clone();
        }
    }
    if let Some(mut sway) = shrubs {
        for ii in std::mem::take(&mut world.moved_shrubs) {
            let (Some(m), Some(Some(ci))) = (world.shrubs.get(ii), lv.shrubs.class_of.get(ii)) else { continue };
            let h = &lv.shrubs.classes[*ci].class.header;
            sway.move_instance(&mut commands, ii, *m, scaled(h.bsphere, h.scale));
        }
        sway.flush(&mut buffers);
    }
}

/// Aranos's arrival: the ship's screen (`rc_game::scene::Scenes::ship_screen`) through crate::world_screen. The
/// actor's entities go to the screen's layer while it shows (off the frame's) and back after.
#[allow(clippy::too_many_arguments)]
fn ship_screen(
    mut commands: Commands,
    play: Option<Res<Play>>,
    view: Option<Res<PlayView>>,
    draw: Option<Res<crate::mobys::CreatedDraw>>,
    mut screens: ResMut<crate::world_screen::WorldScreens>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut id: Local<Option<crate::world_screen::WorldScreenId>>,
    mut layered: Local<Vec<Entity>>,
    generation: Res<crate::level_switch::LevelGeneration>,
) {
    // A level change resets the screens (crate::level_switch::reset, a change of the resource, not an addition) and
    // the actor's entities: the id and the moved entities go with them.
    if screens.is_added() || generation.is_changed() { (*id, *layered) = (None, Vec::new()); }
    let screen = play.as_ref().zip(view.as_ref()).and_then(|(p, v)| {
        let mobys = p.mobys.as_ref()?;
        p.game.scenes.ship_screen(p.game.level, &mobys.world, v.view.pos_f32()).map(|s| (s, v.tan_half_fov))
    });
    let Some((s, tan)) = screen else {
        for e in layered.drain(..) { if let Ok(mut c) = commands.get_entity(e) { c.remove::<bevy::camera::visibility::RenderLayers>(); } }
        if let Some(i) = *id { screens.show(i, None); }
        return;
    };
    let i = *id.get_or_insert_with(|| screens.create(&mut commands, &mut images, &mut meshes, &mut materials, UVec2::splat(256), "ship"));
    let layer = screens.layer(i);
    for e in draw.map(|d| d.entities_of(s.actor)).unwrap_or_default() {
        if !layered.contains(&e) {
            commands.entity(e).insert(layer.clone());
            layered.push(e);
        }
    }
    // The frame's projection centred in the 256×256 target: its 512×416 screen's focal lengths over 128 texels.
    let ty = tan * crate::game_camera::NTSC_Y_RATIO;
    let v = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let f = s.rows[0];
    let euler = [Pf::ZERO, Pf::f(-f[2].clamp(-1.0, 1.0).asin()), Pf::f(f[1].atan2(f[0])), Pf::ZERO];
    let eye = play_camera::view_transform(&CameraView { pos: v(s.eye), euler, rows: s.rows.map(v), class: 0, flyby_phase: 0 });
    screens.show(i, Some(crate::world_screen::WorldScreenView {
        eye,
        tan: (tan * 128.0 / (crate::game_camera::SCREEN_W * 0.5), ty * 128.0 / (crate::game_camera::SCREEN_H * 0.5)),
        corners: s.corners.map(crate::tfrag_render::game_to_bevy),
        uvs: s.uvs,
        clear: Color::BLACK,
    }));
}

/// `0x367b68` on the ship being drawn — `ship` (the scene's actor), else the first drawn one of class 0x0d54 — seen
/// from `camera`: this tick's canopy for [`draw_canopy`].
fn canopy_tick(mobys: &mut crate::mobys::Mobys, lv: &crate::level_load::LoadedLevel, occl: Option<&MobyOcclusion>, anim: Option<&mut MobyAnim>, ship: Option<usize>, camera: [f32; 3], mode: i32) {
    use rc_game::moby::MobyHost;
    mobys.canopy_frame = None;
    let Some(canopy) = mobys.canopy.as_mut() else { return };
    let drawn = |m: &rc_game::moby::Moby| m.o_class == 0xd54 && m.state < 0x80 && m.mode & 0x81 == 0;
    let Some(s) = ship.filter(|&s| mobys.world.mobys.get(s).is_some_and(drawn)).or_else(|| mobys.world.mobys.iter().position(drawn)) else { return };
    let host = crate::mobys::Host { lv: &lv.mobys, occl, anim, created: &mut mobys.created, hero: None, hero_chains: &[], chains: Some(&mobys.chains), camera, camera_rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], sound: None };
    let m = &mobys.world.mobys[s];
    let Some((rows, at)) = host.joint_matrix(m, 0) else { return };
    mobys.canopy_frame = Some(canopy.draw(rows, at, m.pos, camera, mode));
}

/// The canopy's draw: a list-1 draw callback (crate::fx_draw: FX 0x15, MODULATE, ALPHA 0x44 on the display bytes, as
/// R&C1's ship glass `0x2a70a8` draws).
#[derive(Resource, Default)]
struct CanopyDraw {
    slots: crate::fx_draw::FxSlots,
}

#[allow(clippy::too_many_arguments)]
fn draw_canopy(
    mut commands: Commands,
    level: Res<crate::Level>,
    play: Option<Res<Play>>,
    mut draw: ResMut<CanopyDraw>,
    fog: Option<Res<crate::game_camera::GameFog>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::fx_draw::FxPrimMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    let frame = play.as_ref().and_then(|p| p.mobys.as_ref()).and_then(|m| m.canopy_frame.as_ref());
    let mut groups = Vec::new();
    if let Some(f) = frame {
        let [r, g, b, a] = f.rgba.map(u32::from);
        let rgba = r | g << 8 | b << 16 | a << 24;
        let mut prims = crate::fx_draw::PrimBuf::default();
        for q in &f.strips {
            let k = q.map(|i| i as usize);
            prims.quad(k.map(|i| f.verts[i]), k.map(|i| f.uvs[i]), [rgba; 4]);
        }
        groups.push(crate::fx_draw::FxGroup { fx: rc_game::canopy::TEXTURE, additive: false, subtract: false, prims });
    }
    let fog = fog.map(|f| f.uniform).unwrap_or_else(|| crate::game_camera::TfragFog::new(&level.0.fog));
    let tex = level.0.particles.textures.as_ref().map(|t| t.fx_textures.as_slice());
    let mut a = crate::fx_draw::FxAssets { meshes: &mut meshes, images: &mut images, materials: &mut materials, fx: tex, fog };
    draw.slots.show(&mut commands, &mut vis, &mut a, groups, crate::fx_draw::LIST1_BIAS, "ship canopy");
}

/// The mobys' draw callbacks through crate::fx_draw: the quads the tick's callbacks made (rc_game::fx: the shots'
/// trails, 0x0b0a's glows), one group per texture and blend.
#[derive(Resource, Default)]
struct ShotDraw {
    slots: crate::fx_draw::FxSlots,
}

#[allow(clippy::too_many_arguments)]
fn draw_shots(
    mut commands: Commands,
    level: Res<crate::Level>,
    play: Option<Res<Play>>,
    mut draw: ResMut<ShotDraw>,
    fog: Option<Res<crate::game_camera::GameFog>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::fx_draw::FxPrimMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    let mut groups = Vec::new();
    // The callbacks' quads, one group per texture and blend in the order they came.
    if let Some(p) = play.as_ref() {
        let mut keyed: Vec<((usize, bool), crate::fx_draw::PrimBuf)> = Vec::new();
        for q in &p.fx_quads {
            let k = (q.fx, q.additive);
            let i = keyed.iter().position(|(kk, _)| *kk == k).unwrap_or_else(|| { keyed.push((k, crate::fx_draw::PrimBuf::default())); keyed.len() - 1 });
            keyed[i].1.quad(q.corners, q.st, q.rgba);
        }
        for ((fx, additive), prims) in keyed { if !prims.is_empty() { groups.push(crate::fx_draw::FxGroup { fx, additive, subtract: false, prims }); } }
    }
    let fog = fog.map(|f| f.uniform).unwrap_or_else(|| crate::game_camera::TfragFog::new(&level.0.fog));
    let tex = level.0.particles.textures.as_ref().map(|t| t.fx_textures.as_slice());
    let mut a = crate::fx_draw::FxAssets { meshes: &mut meshes, images: &mut images, materials: &mut materials, fx: tex, fog };
    draw.slots.show(&mut commands, &mut vis, &mut a, groups, crate::fx_draw::LIST1_BIAS, "shot trails");
}

/// `0x2e54a0`'s mode (`GameState::occlusion`, `0x1b19d0`) onto the occlusion frame: 0 everything visible, 2 the
/// loader's mode (the grid), else the mask kept. Left alone under `RC_OCCL`.
fn occlusion_mode(play: Option<Res<Play>>, level: Res<crate::Level>, frame: Option<ResMut<crate::occlusion::OcclusionFrame>>) {
    use rc_formats::occlusion::OcclusionMode;
    let (Some(p), Some(mut f)) = (play, frame) else { return };
    if std::env::var_os("RC_OCCL").is_some() { return; }
    let mode = match p.game.occlusion {
        0 => OcclusionMode::Off,
        2 => level.0.occlusion.loader_mode,
        _ => OcclusionMode::Freeze,
    };
    if f.mode != mode { f.mode = mode; }
}

/// The hero at Ratchet's instance (class 0).
fn start(lv: &crate::level_load::LoadedLevel) -> Option<Play> {
    let m = &lv.mobys;
    let Some(instance) = m.instances.iter().position(|i| i.o_class == 0) else {
        eprintln!("play: no class-0 instance on this level: no Ratchet, no game tick");
        return None;
    };
    let class = m.classes.iter().position(|c| c.o_class == 0)?;
    let inst = &m.instances[instance];
    let scale = m.placed.get(instance).copied().flatten().map_or(1.0, |pl| pl.scale);
    let mut hero = Hero::spawn(inst.position, inst.rotation[2]);
    hero.hp = rc_game::game::GameState::default().max_health;
    let camera = FollowCamera::new(&cam_input(&hero, lv.collision.as_ref(), [0.0; 2]));
    println!("play: Ratchet at ({:.1}, {:.1}, {:.1}), yaw {:.2}: game tick on", inst.position[0], inst.position[1], inst.position[2], inst.rotation[2]);
    Some(Play {
        anim: RatchetAnim { curves: &rc_game::hero::CURVES, ..RatchetAnim::new(&m.anim[class]) },
        hero, class, instance, scale, pad: PadState::default(), camera, ship: None, chain: false, mobys: None, game: Default::default(), hud: rc_game::hud::Hud::new(rc_game::game::GameState::default().max_health), ticks: 0, loaded: false,
        fx_quads: Vec::new(), scene_wad: None, shared_scenes: None, scene_slots: Vec::new(), movie: None, movie_picture: None, leaving: None, scene_listener: None, movie_started: false, hero_loops: [-1; 9], pause: None, pause_opened: false, death_fade: None, qs: Default::default(), qs_paused: false, vendor: Default::default(), space_scene: None, flight: None, flight_cam: None, flight_quads: Vec::new(),
        help: Default::default(), boxes: Default::default(), glyphs: None, texts: rc_formats::strings::parse_strings(&lv.gameplay, rc_formats::strings::lang::ENGLISH).map_err(|e| eprintln!("play: no help texts ({e:#})")).unwrap_or_default(),
        trace: std::env::var("RC_PLAY_TRACE").is_ok_and(|v| v.trim() == "1"),
    })
}

/// The sound code's listener: the follow camera (`0x1b8d40`, its rows).
fn listener(c: &FollowCamera) -> rc_game::audio::voices::Listener {
    rc_game::audio::voices::Listener { pos: c.pos, rows: c.rows, underwater: false, water_height: 0.0 }
}

/// The camera's inputs: the options' yaw and pitch normal (`0x1a7bb0` / `0x1a7bac`, the defaults).
pub(crate) fn cam_input<'a>(hero: &'a Hero, coll: Option<&'a rc_formats::collision::Collision>, rstick: [f32; 2]) -> CamInput<'a> {
    CamInput { hero, coll, rstick, yaw_normal: true, pitch_normal: true, speed: 0, scene: None, hero_moby: None, level: 0, strafe_held: false, ground: None }
}

/// The view from `c` at the eye `pos` with the rows `rows` (shaken; the angles are the camera's own, as `0x2d9f38`
/// takes them before the shakes).
fn camera_view(c: &FollowCamera, pos: [f32; 3], rows: [[f32; 3]; 3]) -> CameraView {
    let v = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    CameraView { pos: v(pos), euler: [Pf::ZERO, Pf::f(c.pitch()), Pf::f(c.yaw()), Pf::ZERO], rows: rows.map(v), class: 0, flyby_phase: 0 }
}
