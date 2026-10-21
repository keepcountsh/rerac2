//! Going Commando's game tick in the engine (rc_game2: [`rc_game2::hero`], [`rc_game2::camera`]): Ratchet runs,
//! jumps and falls on the level's collision, his moby and pose follow him, the camera follows him
//! (docs/plan/rac2_bringup.md §6.3 F). The pieces that are the same in both games come from the R&C1 side: the pad
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
//! (one line per tick: state, position, velocity, air ticks, sequence).

use crate::game_camera::CameraSource;
use crate::input_map::{self, PadFrame, Script};
use crate::moby_anim::MobyAnim;
use crate::moby_render::MobyOcclusion;
use crate::play_camera::{self, PlayView};
use bevy::prelude::*;
use rc_game::follow_camera::CameraView;
use rc_game::hero::anim::RatchetAnim;
use rc_game::pad::PadState;
use rc_game::ps2v::Pf;
use rc_game2::camera::{CamInput, FollowCamera};
use rc_game2::hero::physics::Env;
use rc_game2::hero::{hero_update, Hero};

pub struct Rac2PlayPlugin;

impl Plugin for Rac2PlayPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("RC_PLAY").is_ok_and(|v| v.trim() == "0") { return; }
        let fly = std::env::var("RC_PLAY_FLY").is_ok_and(|v| v.trim() == "1");
        let script = std::env::var("RC_PLAY_SCRIPT").ok().filter(|s| !s.trim().is_empty()).map(|s| Script::parse(&s).unwrap_or_else(|e| panic!("RC_PLAY_SCRIPT: {e}")));
        println!("{}", input_map::CONTROLS);
        app.insert_resource(if fly { CameraSource::Fly } else { CameraSource::Play })
            .insert_resource(Rac2Script(script))
            .init_resource::<PadFrame>()
            .add_systems(PreUpdate, (input_map::sample_system, switch_camera).chain().after(bevy::input::InputSystems))
            .add_systems(FixedUpdate, tick)
            .add_systems(Update, (draw_created, draw_hud))
            .add_systems(RunFixedMainLoop, play_camera::apply.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop));
    }
}

#[derive(Resource)]
struct Rac2Script(Option<Script>);

/// The running game.
#[derive(Resource)]
struct Play {
    hero: Hero,
    anim: RatchetAnim,
    /// Ratchet's class (index into `LevelMobys::classes` / `anim`) and static instance.
    class: usize,
    instance: usize,
    scale: f32,
    pad: PadState,
    camera: FollowCamera,
    /// The level's mobys (crate::rac2_mobys).
    mobys: Option<crate::rac2_mobys::Rac2Mobys>,
    /// The game-wide state (bolts, health limit).
    game: rc_game2::game::GameState,
    /// The HUD's counters (rc_game2::hud).
    hud: rc_game2::hud::Hud,
    /// The help panel (rc_game2::help) and the level's English help texts.
    help: rc_game2::help::Panel,
    texts: Vec<rc_formats::strings::Message>,
    ticks: u64,
    trace: bool,
    /// The level's objects spawned once (later spawns are respawns).
    loaded: bool,
    /// The level's scene file (`G/SCENEn.WAD`) and its table, read at the first scene.
    scene_wad: Option<(Vec<u8>, rc_formats::rac2::scene::SceneWad)>,
    /// The class animations a scene's actors added sequence slots to, with their sequence counts before.
    scene_slots: Vec<(usize, usize)>,
}

fn switch_camera(keys: Res<ButtonInput<KeyCode>>, mut source: ResMut<CameraSource>) {
    if keys.just_pressed(KeyCode::Tab) {
        *source = if *source == CameraSource::Play { CameraSource::Fly } else { CameraSource::Play };
        println!("rac2: camera: {:?}", *source);
    }
}

#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    mut level: ResMut<crate::Level>,
    play: Option<ResMut<Play>>,
    front: Option<Res<crate::rac2::Rac2>>,
    frame: Res<PadFrame>,
    script: Res<Rac2Script>,
    source: Res<CameraSource>,
    mut occl: Option<ResMut<MobyOcclusion>>,
    mut anim: Option<ResMut<MobyAnim>>,
) {
    // The front end's layer is up: the game waits (as R&C1's tick does under its front end).
    if front.is_some_and(|f| !f.done()) { return; }
    // The scenes add sequence slots to the level's class animations (as the game's actors do), unseen by the
    // systems that watch the level for a new one.
    let level = level.bypass_change_detection();
    let Some(mut p) = play else {
        if let Some(p) = start(&level.0) {
            commands.insert_resource(PlayView { view: camera_view(&p.camera), tan_half_fov: play_camera::GAME_TAN_HALF_FOV, far: crate::game_camera::FAR });
            commands.insert_resource(p);
        }
        return;
    };
    let p = &mut *p;
    if level.0.collision.is_none() { return; }
    if p.mobys.is_none() {
        let lv = &level.0;
        let blobs = crate::rac2_level::class_blobs(&crate::level_load::extracted_root(), crate::level_load::level_index())
            .map_err(|e| eprintln!("rac2: no class blobs ({e:#}): no moby collision"))
            .unwrap_or(crate::rac2_level::ClassBlobs { collision: Vec::new(), blobs: Default::default() });
        let mut created = Default::default();
        let host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut created, hero: None, hero_chains: &[], camera: p.camera.pos };
        let fresh = !p.loaded;
        let m = crate::rac2_mobys::Rac2Mobys::load(lv, blobs, &host, &mut p.game, fresh);
        p.loaded = true;
        println!("rac2: {} mobys, {} with collision, Ratchet's joint lists {}", m.world.mobys.len(), m.world.mobys.iter().filter(|m| m.has_collision).count(), m.hero_chains.len());
        p.mobys = Some(m);
    }
    // `0x32c858`: a scene or a movie starting or ending.
    mode_changes(p, &mut level.0);
    let input = match &script.0 {
        Some(s) => s.at(p.ticks),
        None if *source == CameraSource::Play => frame.0,
        None => rc_game::pad::PadInput::neutral(),
    };
    p.pad.update(Some(&input.bytes()), false);
    p.game.frame += 1;
    match p.game.scenes.mode {
        rc_game2::scene::SCENE => {
            scene_tick(p, &mut level.0, &mut commands, occl.as_deref_mut(), anim.as_deref_mut());
            p.ticks += 1;
            return;
        }
        rc_game2::scene::MOVIE => {
            p.ticks += 1;
            return;
        }
        _ => {}
    }
    let lv = &level.0;
    let Some(coll) = lv.collision.as_ref() else { return };
    // `0x2d6258`: the health limit from the XP and the nanotech.
    p.game.health_tick(&mut p.hero.hp);
    let class = &lv.mobys.anim[p.class];
    let mobys = p.mobys.as_mut().expect("mobys");
    let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
    // The frame's order (`0x364260`): the moby loop `0x32e398`, then the hero `0x2a6dd0` (with its item pass), then
    // the camera `0x2d9f38`.
    {
        let pose = crate::rac2_mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale };
        let mut host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, camera: p.camera.pos };
        let scene = crate::rac2_mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut ctx = rc_game2::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level };
        mobys.world.tick(p.camera.pos, &mut ctx);
    }
    let targets = mobys.world.targets(p.camera.pos);
    // Ratchet's hit record (his moby's `+0xa8`) with its source.
    let hit = mobys.hero_moby.and_then(|i| mobys.world.hit_message(i)).map(|rec| {
        let src = rec.tmpl.source.and_then(|s| mobys.world.mobys.get(s));
        rc_game2::hero::damage::HitIn { rec, source_pos: src.map(|m| m.pos), source_class: src.map_or(-1, |m| m.o_class) }
    });
    {
        let scene = crate::rac2_mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        if p.game.death_z.is_none() && lv.death_z.is_finite() { p.game.death_z = Some(lv.death_z); }
        let death_z = p.game.death_z;
        let classes: Vec<i16> = mobys.world.mobys.iter().map(|m| m.o_class).collect();
        let nodes = rc_game2::moby::classes::swing_nodes(&mobys.world, 0x2f6);
        let hooks = rc_game2::moby::classes::swing_nodes(&mobys.world, 0x323);
        let pulls = rc_game2::moby::classes::swing_nodes(&mobys.world, 0x99d);
        let env = Env {
            coll, pad: &p.pad, cam_yaw: p.camera.yaw(), cam_pos: p.camera.pos, cam_rows: p.camera.rows, mirror: false, scene: scene.as_ref(), hero_moby: mobys.hero_moby, targets: &targets, hit, death_z,
            clank: p.game.clank, pack: p.game.pack(), boots: p.game.boots, charge_boots: p.game.charge_boots, grind_boots: p.game.grind_boots, rails: &mobys.rails, level: p.game.level, classes: &classes, nodes: &nodes, hooks: &hooks, pulls: &pulls,
        };
        let mut ctl = p.anim.ctl(class);
        hero_update(&mut p.hero, &env, &mut ctl);
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
        h.mode |= rc_game2::moby::mode::KEEP_ROWS;
    }
    // The item pass (`0x2b21f0`): the hand item placed at Ratchet's new pose and updated.
    {
        let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let pose = crate::rac2_mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale };
        let mut host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, camera: p.camera.pos };
        let scene = crate::rac2_mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut ctx = rc_game2::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level };
        rc_game2::hero::items::item_pass(&mut mobys.world, &mut ctx);
    }
    if mobys.world.changed {
        let host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], camera: p.camera.pos };
        mobys.snap = Some(crate::rac2_mobys::take_snapshot(&mobys.world, &mobys.coll, &host));
        mobys.world.changed = false;
    }
    if let Some(o) = occl.as_mut() {
        for m in mobys.world.mobys.iter().filter(|m| m.update.is_some() || m.drive) {
            if let Some(ii) = m.instance { o.drive(ii, m.pos, m.rows, m.scale, None, m.state >= 0x80 || m.mode & 1 != 0); }
        }
    }
    {
        let scene = crate::rac2_mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut inp = cam_input(&p.hero, Some(coll), [p.pad.rx.to_f32(), p.pad.ry.to_f32()]);
        (inp.scene, inp.hero_moby) = (scene.as_ref(), mobys.hero_moby);
        inp.level = p.game.level;
        inp.strafe_held = p.pad.held & (rc_game::pad::button::L2 | rc_game::pad::button::R2) != 0;
        p.camera.update(&inp, &mut p.game.cam_tweaks);
    }
    // The HUD (`0x443c08`) with this tick's requests.
    let (sb, sh) = (p.game.show_bolts.take(), p.game.show_health.take());
    p.hud.tick(p.hero.hp, p.game.max_health, sb, sh);
    // The help panel (`0x2f5680`).
    p.help.tick(&mut p.game, p.pad.pressed & rc_game::pad::button::TRIANGLE != 0, &p.texts);
    if p.trace {
        let v = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let h = &p.hero;
        println!(
            "rac2 tick {}: state {:#x}/{} pos ({:.2}, {:.2}, {:.2}) vel ({:.3}, {:.3}, {:.3}) air {} h {:.2} slope {:.2} seq {}/{} f {:.1} spd {:.2} cap {} at ({:.2}, {:.2}, {:.2}) n ({:.2}, {:.2}, {:.2}) capsule {:.2}/{:.2}/{:.2} cam ({:.2}, {:.2}, {:.2}) yaw {:.2} pitch {:.2}",
            p.ticks, h.state, h.substate, h.pos[0], h.pos[1], h.pos[2], h.vel[0], h.vel[1], h.vel[2], h.air_ticks, h.height, h.slope, v.seq_a, v.seq_b, v.frame, h.anim_speed,
            h.cap_hit, h.contact_point[0], h.contact_point[1], h.contact_point[2], h.contact_normal[0], h.contact_normal[1], h.contact_normal[2], h.cap_bottom, h.cap_top, h.cap_radius,
            p.camera.pos[0], p.camera.pos[1], p.camera.pos[2], p.camera.yaw(), p.camera.pitch()
        );
    }
    if let Some(o) = occl.as_mut() {
        o.drive(p.instance, p.hero.pos, p.hero.rows, p.scale, None, false);
        if let (Some(a), Some(k)) = (anim.as_mut(), o.anim_index(p.instance)) {
            let mut s = p.anim.state;
            s.skip_advance = true;
            if let Some(i) = a.instances.get_mut(k) { i.state = s; }
            if let Some(slot) = a.snapshots.get_mut(k) { *slot = p.anim.snapshot.clone(); }
            // His posture's joint modifiers (rc_game2::hero::posture).
            if let (Some(mods), Some(m)) = (a.mods.get_mut(k), p.mobys.as_ref()) { *mods = p.hero.posture.modifiers(&m.hero_targets); }
        }
    }
    commands.insert_resource(PlayView { view: camera_view(&p.camera), tan_half_fov: play_camera::GAME_TAN_HALF_FOV, far: crate::game_camera::FAR });
    p.ticks += 1;
    // `0x2b31f0`'s respawn request: Ratchet back at his start with full health (the checkpoints and the fade are not
    // ported); the world keeps its state.
    // `0x2b31f0`'s respawn request: the level restarts from the checkpoint (the spawn points', else Ratchet's start):
    // the objects spawn again by their rules (the ones gone for good stay out), Ratchet there with full health. The
    // fade is not ported.
    if p.hero.damage.dead {
        let inst = &lv.mobys.instances[p.instance];
        let armour = p.hero.damage.armour;
        let (at, rot) = p.game.checkpoint.unwrap_or((inst.position, inst.rotation));
        p.game.restart();
        p.mobys = None;
        p.hero = Hero::spawn(at, rot[2]);
        p.hero.hp = p.game.max_health;
        p.hero.damage.armour = armour;
        p.anim = RatchetAnim::new(&lv.mobys.anim[p.class]);
        p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
        p.game.show_health = Some(0x168);
    }
}

/// `0x32c858`'s changes (rc_game2::scene): a scene's end and start; a movie is not played yet (it ends at once).
fn mode_changes(p: &mut Play, lv: &mut crate::level_load::LoadedLevel) {
    for c in p.game.scenes.switch(p.hero.hp) {
        match c {
            rc_game2::scene::Change::EndedScene => end_scene(p, lv),
            rc_game2::scene::Change::StartScene(id) => start_scene(p, lv, id),
            rc_game2::scene::Change::StartMovie(id) => {
                eprintln!("rac2: movie {id:#x} is not played yet: it ends at once");
                let hp = p.hero.hp;
                let g = &mut p.game;
                g.scenes.movie_done(false, hp, &mut g.scenes_seen);
            }
        }
    }
}

/// Scene `id` of the level (its record in `G/SCENEn.WAD`).
fn load_scene(p: &mut Play, id: i32) -> anyhow::Result<Option<rc_formats::scene::Scene>> {
    use anyhow::Context;
    if p.scene_wad.is_none() {
        let path = crate::level_load::extracted_root().join(format!("G/SCENE{}.WAD", crate::level_load::level_index()));
        let wad = std::fs::read(&path).with_context(|| format!("{}", path.display()))?;
        let table = rc_formats::rac2::scene::SceneWad::parse(&wad)?;
        p.scene_wad = Some((wad, table));
    }
    let (wad, table) = p.scene_wad.as_ref().expect("scene file");
    Ok(table.scene(wad, rc_formats::rac2::scene::record_of(id) as usize)?)
}

/// `0x370e08` / `0x305428`: the scene's actors made (class 0: Ratchet's moby; each in the next sequence slot of its
/// class, which plays the chunk's streamed sequence), the 0xcb mobys and the hand item hidden, the scene started.
fn start_scene(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, id: i32) {
    let scene = match load_scene(p, id) {
        Ok(Some(s)) => s,
        r => {
            match r {
                Err(e) => eprintln!("rac2: scene {id:#x}: {e:#}: skipped"),
                _ => eprintln!("rac2: scene {id:#x}: no such scene: skipped"),
            }
            let hp = p.hero.hp;
            p.game.scenes.player = None;
            if !p.game.scenes.start_queue(hp) { p.game.scenes.pop(hp); }
            return;
        }
    };
    let Some(mobys) = p.mobys.as_mut() else { return };
    use rc_game2::moby::mode;
    let mut actors = Vec::new();
    for a in &scene.chunks[0].actors {
        let o_class = match a.class { 0x215 | 0x10d1 => 0x0d54, c => c as i16 };
        let Some(ci) = lv.mobys.classes.iter().position(|c| c.o_class == o_class as i32) else {
            eprintln!("rac2: scene {id:#x}: actor class {o_class:#x} is not in the level");
            actors.push((usize::MAX, 0xff));
            continue;
        };
        let m = if o_class == 0 {
            match mobys.hero_moby {
                Some(h) => h,
                None => {
                    actors.push((usize::MAX, 0xff));
                    continue;
                }
            }
        } else {
            let m = mobys.world.create(o_class);
            let a = &mut mobys.world.mobys[m];
            (a.draw_dist, a.drawn, a.update_dist, a.has_collision) = (0x1ff, 1, 0xff, false);
            a.mode |= mode::NO_UPDATE | mode::NO_MATRIX | mode::KEEP_ROWS;
            m
        };
        let anim = &mut lv.mobys.anim[ci];
        if !p.scene_slots.iter().any(|&(c, _)| c == ci) { p.scene_slots.push((ci, anim.sequences.len())); }
        let slot = anim.sequences.len() as u8;
        anim.sequences.push(Some(a.sequence.clone()));
        actors.push((m, slot));
    }
    for m in mobys.world.mobys.iter_mut().filter(|m| m.o_class == 0xcb && m.state < 0x80) { m.mode |= 0x80; }
    if let Some(h) = p.hero.items.hand.and_then(|h| mobys.world.mobys.get_mut(h)) { h.mode |= 1; }
    println!("rac2: scene {id:#x}: {} ticks, actors {:?}", scene.end_tick(), scene.actor_classes());
    let g = &mut p.game;
    g.scenes.start_scene(id, std::sync::Arc::new(scene), actors, &mut g.scenes_seen);
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
    for (ci, n) in p.scene_slots.drain(..) {
        if let Some(a) = lv.mobys.anim.get_mut(ci) { a.sequences.truncate(n); }
    }
    let at = p.hero.pos;
    let ground = lv.collision.as_ref().and_then(|c| rc_game::collision_query::coll_line(c, [at[0], at[1], at[2] + 0.5], [at[0], at[1], 0.01], rc_game::collision_query::QueryFlags(2))).map(|o| o.point[2]);
    p.game.scenes.end_scene(&mut p.hero, ground);
    p.camera = FollowCamera::new(&cam_input(&p.hero, lv.collision.as_ref(), [0.0; 2]));
}

/// A scene's tick (`0x3716f8`): the moby loop (Ratchet's update does nothing in a scene), then the scene's: the
/// actors posed in their class slots at their tracks, Ratchet's instance at his, the scene camera.
fn scene_tick(p: &mut Play, lv: &mut crate::level_load::LoadedLevel, commands: &mut Commands, mut occl: Option<&mut MobyOcclusion>, mut anim: Option<&mut MobyAnim>) {
    let Some(coll) = lv.collision.as_ref() else { return };
    let Some(mobys) = p.mobys.as_mut() else { return };
    {
        let class = &lv.mobys.anim[p.class];
        let hero_view = rc_game::hero::anim::AnimCtl::view(&p.anim.ctl(class));
        let pose = crate::rac2_mobys::HeroPose { class: p.class, state: &p.anim.state, snapshot: p.anim.snapshot.as_ref(), pos: p.hero.pos, rows: p.hero.rows, scale: p.scale };
        let mut host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: Some(pose), hero_chains: &mobys.hero_chains, camera: p.camera.pos };
        let scene = crate::rac2_mobys::scene_of(&mobys.snap, &mobys.coll, &mobys.cache);
        let mut ctx = rc_game2::moby::Ctx { host: &mut host, hero: &mut p.hero, game: &mut p.game, coll: Some(coll), scene: scene.as_ref(), hero_moby: mobys.hero_moby, hero_view, level: &mobys.level };
        mobys.world.tick(p.camera.pos, &mut ctx);
    }
    let hp = p.hero.hp;
    let level = p.game.level;
    let g = &mut p.game;
    let Some(frame) = g.scenes.tick(&p.pad, level, false, hp, &mut g.scenes_seen) else { return };
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
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
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
        mobys.created.insert(pose.moby, crate::rac2_mobys::Created { class: ci, state, snapshot: None });
    }
    if let Some(o) = occl.as_mut() {
        for m in mobys.world.mobys.iter().filter(|m| m.update.is_some() || m.drive) {
            if let Some(ii) = m.instance { o.drive(ii, m.pos, m.rows, m.scale, None, m.state >= 0x80 || m.mode & 1 != 0); }
        }
        // Ratchet: at his track with the scene's sequence and his head's scale (`0x2d51f8`), else as he stands.
        let (pos, rows, state) = match hero_pose {
            Some((pos, state)) => (pos, identity, state),
            None => (p.hero.pos, p.hero.rows, p.anim.state),
        };
        o.drive(p.instance, pos, rows, p.scale, None, false);
        if let (Some(a), Some(k)) = (anim.as_mut(), o.anim_index(p.instance)) {
            let mut s = state;
            s.skip_advance = true;
            if let Some(i) = a.instances.get_mut(k) { i.state = s; }
            if hero_pose.is_some() {
                if let Some(slot) = a.snapshots.get_mut(k) { *slot = None; }
                let head = mobys.hero_targets.get(24).copied().filter(|&t| t != 0xff);
                if let Some(mods) = a.mods.get_mut(k) {
                    *mods = head.map(|t| vec![rc_formats::moby_anim::JointModifier { scale: [0.848_75, 0.848_75, 0.875], ..rc_formats::moby_anim::JointModifier::compose(t) }]).unwrap_or_default();
                }
            }
        }
    }
    if mobys.world.changed {
        let host = crate::rac2_mobys::Host { lv: &lv.mobys, occl: occl.as_deref(), anim: anim.as_deref_mut(), created: &mut mobys.created, hero: None, hero_chains: &[], camera: p.camera.pos };
        mobys.snap = Some(crate::rac2_mobys::take_snapshot(&mobys.world, &mobys.coll, &host));
        mobys.world.changed = false;
    }
    commands.insert_resource(PlayView { view: scene_view(&frame.camera), tan_half_fov: frame.camera.tan_half_fov, far: crate::game_camera::FAR });
}

fn scene_view(c: &rc_game::scene_player::SceneCamera) -> CameraView {
    let v = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let f = c.rows[0];
    let euler = [Pf::ZERO, Pf::f(-f[2].clamp(-1.0, 1.0).asin()), Pf::f(f[1].atan2(f[0])), Pf::ZERO];
    CameraView { pos: v(c.eye), euler, rows: c.rows.map(v), class: 0, flyby_phase: 0 }
}

/// The in-game HUD's layer: the front end's texture set (its fonts, slot 1) and the level's HUD icons after it.
#[derive(Resource)]
struct GameHud {
    layer: crate::hud_render::Layer,
    data: crate::rac2::Data,
    /// `(icon, frame)` → layer texture and texel size.
    icons: std::collections::HashMap<(u16, u16), (usize, [f32; 2])>,
}

/// The HUD layer, made on the first frame with a game (its own 2D camera above the world's), then drawn each frame.
#[allow(clippy::too_many_arguments)]
fn draw_hud(
    mut commands: Commands,
    mut play: Option<ResMut<Play>>,
    hud: Option<Res<GameHud>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::hud_render::HudMaterial>>,
    mut composites: ResMut<Assets<crate::hud_render::HudComposite>>,
) {
    let Some(p) = play.as_mut() else { return };
    let Some(hud) = hud else {
        let root = crate::level_load::extracted_root();
        let data = match crate::rac2::Data::load(&root) {
            Ok(d) => d,
            Err(e) => { eprintln!("rac2: no HUD ({e:#})"); return; }
        };
        let mut textures: Vec<rc_formats::texture::Texture> = data.textures().to_vec();
        let mut icons = std::collections::HashMap::new();
        match crate::rac2_level::hud(&root, crate::level_load::level_index()) {
            Ok(h) => {
                for id in [0x7567u16] {
                    let f = h.icon_frame(id, 0);
                    if let Ok(t) = h.decode_frame_raw(f) {
                        icons.insert((id, 0), (textures.len(), [t.width as f32, t.height as f32]));
                        textures.push(t);
                    }
                }
            }
            Err(e) => eprintln!("rac2: no HUD icons ({e:#})"),
        }
        let cam = commands.spawn((Camera2d, Camera { order: 900, clear_color: ClearColorConfig::None, ..default() }, Name::new("rac2 HUD camera"))).id();
        let scaling = crate::hud_render::scaling(crate::graphics::Hud::Original);
        let layer = crate::hud_render::Layer::spawn(&mut commands, &mut images, &mut meshes, &mut materials, &mut composites, &textures, scaling, cam);
        commands.insert_resource(GameHud { layer, data, icons });
        return;
    };
    let p: &mut Play = p;
    let mut quads = Vec::new();
    // In a scene the HUD and the help are not drawn (`0x2e6d88`).
    if p.game.scenes.mode != rc_game2::scene::SCENE {
        let draws = p.hud.draw(p.game.bolts);
        let icon = |id: u16, f: u16| hud.icons.get(&(id, f)).map(|&(i, s)| (rc_game2::gui::Tex::Slot(i), s));
        quads = rc_game2::hud::quads(&draws, &hud.data.assets().sprites, &hud.data.assets().glyphs, &icon);
        quads.extend(p.help.draw(&p.game, &p.texts, &hud.data.assets().glyphs, &hud.data.assets().sprites));
    }
    // The fade into and out of a scene (`0x1b1860`, 1 black).
    let fade = p.game.scenes.fade.clamp(0.0, 1.0);
    if 0.0 < fade { rc_game2::gui::fill(&mut quads, [0, 0, 0, (fade * 128.0) as u8]); }
    let mut d = crate::hud_render::Hud2d::default();
    for q in quads {
        let tex = match q.tex {
            rc_game2::gui::Tex::Slot(i) => crate::hud_render::Tex::Frame(i),
            _ => crate::hud_render::Tex::None,
        };
        d.screen_prim(tex, q.pos, q.uv, u32::from_le_bytes(q.rgba), false);
    }
    if let Some(mut m) = meshes.get_mut(&hud.layer.mesh) { *m = hud.layer.build(&d); }
}

/// The created mobys' draw (crate::rac2_mobys::CreatedDraw), made on the first frame with mobys.
#[allow(clippy::too_many_arguments)]
fn draw_created(
    mut commands: Commands,
    level: Res<crate::Level>,
    play: Option<Res<Play>>,
    draw: Option<ResMut<crate::rac2_mobys::CreatedDraw>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::moby_render::MobyMaterial>>,
    mut buffers: ResMut<Assets<bevy::render::storage::ShaderBuffer>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(mobys) = play.as_ref().and_then(|p| p.mobys.as_ref()) else { return };
    let Some(mut draw) = draw else {
        commands.insert_resource(crate::rac2_mobys::CreatedDraw::new(&level.0, &mut buffers));
        return;
    };
    draw.update(mobys, &level.0, &mut commands, &mut meshes, &mut images, &mut materials, &mut buffers, &mut transforms);
}

/// The hero at Ratchet's instance (class 0).
fn start(lv: &crate::level_load::LoadedLevel) -> Option<Play> {
    let m = &lv.mobys;
    let Some(instance) = m.instances.iter().position(|i| i.o_class == 0) else {
        eprintln!("rac2: no class-0 instance on this level: no Ratchet, no game tick");
        return None;
    };
    let class = m.classes.iter().position(|c| c.o_class == 0)?;
    let inst = &m.instances[instance];
    let scale = m.placed.get(instance).copied().flatten().map_or(1.0, |pl| pl.scale);
    let mut hero = Hero::spawn(inst.position, inst.rotation[2]);
    hero.hp = rc_game2::game::GameState::default().max_health;
    let camera = FollowCamera::new(&cam_input(&hero, lv.collision.as_ref(), [0.0; 2]));
    println!("rac2: Ratchet at ({:.1}, {:.1}, {:.1}), yaw {:.2}: game tick on", inst.position[0], inst.position[1], inst.position[2], inst.rotation[2]);
    Some(Play {
        anim: RatchetAnim::new(&m.anim[class]),
        hero, class, instance, scale, pad: PadState::default(), camera, mobys: None, game: Default::default(), hud: rc_game2::hud::Hud::new(rc_game2::game::GameState::default().max_health), ticks: 0, loaded: false,
        scene_wad: None, scene_slots: Vec::new(),
        help: Default::default(), texts: rc_formats::strings::parse_strings(&lv.gameplay, rc_formats::strings::lang::ENGLISH).map_err(|e| eprintln!("rac2: no help texts ({e:#})")).unwrap_or_default(),
        trace: std::env::var("RC_PLAY_TRACE").is_ok_and(|v| v.trim() == "1"),
    })
}

/// The camera's inputs: the options' yaw and pitch normal (`0x1a7bb0` / `0x1a7bac`, the defaults).
fn cam_input<'a>(hero: &'a Hero, coll: Option<&'a rc_formats::collision::Collision>, rstick: [f32; 2]) -> CamInput<'a> {
    CamInput { hero, coll, rstick, yaw_normal: true, pitch_normal: true, scene: None, hero_moby: None, level: 0, strafe_held: false }
}

fn camera_view(c: &FollowCamera) -> CameraView {
    let v = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    CameraView { pos: v(c.pos), euler: [Pf::ZERO, Pf::f(c.pitch()), Pf::f(c.yaw()), Pf::ZERO], rows: c.rows.map(v), class: 0, flyby_phase: 0 }
}
