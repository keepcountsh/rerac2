//! **The runtime level change**: the old level ends and the next one is loaded in this process, exactly as the boot
//! loads the first one. The game asks for it when a level's main loop ends (`0x2f50f8`, crate::rac2_play: Aranos's
//! last trigger sends Ratchet to Oozla).
//!
//! **API**: [`LevelChange::start_load`] / [`LevelChange::loaded`] / [`LevelChange::swap`] (the async loader, its
//! completion, the new overlay's entry).
//!
//! **What a change does** ([`apply_switch`], an exclusive system at the end of the frame the game asks for it):
//! 1. The new level was loaded on a worker thread ([`load_bundle`]).
//! 2. The old level's entities are despawned: every mesh, camera and UI root that is not [`KeepAcrossLevels`] (the main
//!    camera, the frame's presentation). The old level's state is dropped by each module's [`LevelUnload`] systems.
//! 3. The level-wide resources are replaced: `Level`, `GameFog`, `ClearColor`, the audio system's level data
//!    (`crate::audio_out::AudioOut::swap_level`); the current level ([`crate::level_load::level_index`]) and the lumps
//!    the store keeps for the old one ([`rc_data::Lumps::forget_level`]).
//! 4. [`LevelGeneration`] + 1, then the level start runs again: [`LevelStartup`] / [`LevelPostStartup`] (the systems that
//!    ran at the boot's `Startup` / `PostStartup` for the level), and on the next frame the lazy set-ups whose guards
//!    follow the generation.

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::JoinHandle;

/// The level-start systems (run at the boot after `Startup`, and after each level change).
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LevelStartup;

/// After [`LevelStartup`]'s commands (the boot's `PostStartup` systems of the level).
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LevelPostStartup;

/// The old level's state dropped before the change (each module registers its own: [`remove`] / [`reset`]).
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LevelUnload;

/// An entity a level change keeps (the main camera, the movie UI).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KeepAcrossLevels;

/// The level generation: 0 at the boot, +1 per change. The lazy per-level set-ups re-run when it changes
/// (`Res<LevelGeneration>::is_changed`).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelGeneration(pub u64);

/// A level's data as the loader leaves it (crate::main at the boot, [`LevelChange`] at a change).
pub struct LevelBundle {
    pub index: u32,
    pub level: crate::level_load::LoadedLevel,
}

/// Loads level `index`: the one load path of the boot and of every change.
pub fn load_bundle(root: &std::path::Path, index: u32) -> anyhow::Result<LevelBundle> {
    Ok(LevelBundle { index, level: crate::level_load::load_level(root, index)? })
}

/// The loader of the next level.
#[derive(Resource, Default)]
pub struct LevelChange {
    load: Option<Load>,
    /// The swap was asked for: done at the end of the frame once the load is ready.
    swap: bool,
}

struct Load {
    index: u32,
    thread: Mutex<Option<JoinHandle<anyhow::Result<LevelBundle>>>>,
    result: Option<anyhow::Result<LevelBundle>>,
}

impl LevelChange {
    /// `read_file_entry_with_retry(level)` and the async loader: level `index` loads on a worker thread.
    pub fn start_load(&mut self, index: u32) {
        if self.load.as_ref().is_some_and(|l| l.index == index) { return; }
        let root: PathBuf = crate::level_load::extracted_root();
        let t = std::thread::Builder::new().name(format!("level {index:02} load")).spawn(move || load_bundle(&root, index));
        let (thread, result) = match t {
            Ok(h) => (Some(h), None),
            Err(e) => (None, Some(Err(anyhow::anyhow!("cannot start the level loader thread: {e}")))),
        };
        println!("level switch: loading level {index:02} in the background");
        self.load = Some(Load { index, thread: Mutex::new(thread), result });
        self.swap = false;
    }

    /// `FUN_00257dc8`: the load is done (or failed: the failure is reported at the swap).
    pub fn loaded(&mut self) -> bool {
        let Some(l) = self.load.as_mut() else { return false };
        if l.result.is_some() { return true; }
        let mut g = l.thread.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().is_some_and(|h| h.is_finished()) {
            let h = g.take().unwrap();
            drop(g);
            l.result = Some(h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the level loader thread panicked"))));
            return true;
        }
        false
    }

    /// The new overlay's `entry`: swap at the end of this frame (the load must be done).
    pub fn swap(&mut self) { self.swap = true; }
}

/// [`LevelUnload`]: remove resource `T` (the lazy set-up re-creates it).
pub fn remove<T: Resource>(mut commands: Commands) { commands.remove_resource::<T>(); }

/// [`LevelUnload`]: `T` back to its default (a per-frame or per-level cache built at the plugin's `build`).
pub fn reset<T: Resource + Default>(mut commands: Commands) { commands.insert_resource(T::default()); }

pub struct LevelSwitchPlugin;

impl Plugin for LevelSwitchPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(LevelStartup)
            .init_schedule(LevelPostStartup)
            .init_schedule(LevelUnload)
            .init_resource::<LevelGeneration>()
            .init_resource::<LevelChange>()
            .add_systems(PostStartup, run_level_startup)
            .add_systems(Last, apply_switch);
    }
}

/// The boot's level start: [`LevelStartup`] then [`LevelPostStartup`] (after the app's `Startup`: the main camera).
fn run_level_startup(world: &mut World) {
    world.run_schedule(LevelStartup);
    world.run_schedule(LevelPostStartup);
}

/// The swap (module docs).
fn apply_switch(world: &mut World) {
    let due = world.get_resource::<LevelChange>().is_some_and(|c| c.swap && c.load.as_ref().is_some_and(|l| l.result.is_some()));
    if !due { return; }
    let load = world.resource_mut::<LevelChange>().load.take().unwrap();
    world.resource_mut::<LevelChange>().swap = false;
    let bundle = match load.result.unwrap() {
        Ok(b) => b,
        Err(e) => {
            // The game has no failure path (the disc read retries forever): the port stays on the old level.
            eprintln!("level switch: level {:02} not loaded ({e:#}); staying on level {:02}", load.index, crate::level_load::level_index());
            return;
        }
    };
    let t0 = std::time::Instant::now();
    let old = crate::level_load::level_index();
    world.run_schedule(LevelUnload);
    despawn_level_entities(world);
    let LevelBundle { index, mut level } = bundle;
    crate::level_load::set_level_index(index);
    rc_data::global().forget_level(&crate::level_load::extracted_root(), old);
    if let Some(data) = level.audio.take() {
        if let Some(mut out) = world.get_resource_mut::<crate::audio_out::AudioOut>() { out.swap_level(data); }
    }
    let bg = level.background;
    world.insert_resource(crate::game_camera::GameFog::new(level.fog, crate::game_camera::PARTICLE_FAR12));
    world.insert_resource(ClearColor(Color::srgb_u8(bg[0], bg[1], bg[2])));
    world.insert_resource(crate::Level(level));
    world.resource_mut::<LevelGeneration>().0 += 1;
    world.run_schedule(LevelStartup);
    world.run_schedule(LevelPostStartup);
    println!("level switch: level {old:02} → {index:02} in {:.1} ms (generation {})", t0.elapsed().as_secs_f64() * 1e3, world.resource::<LevelGeneration>().0);
}

/// Despawns every mesh, camera and UI root of the old level (module docs): entities with `Mesh3d`, `Mesh2d`, a
/// `Camera` or a UI `Node` that are not [`KeepAcrossLevels`] and have no kept ancestor.
fn despawn_level_entities(world: &mut World) {
    let mut q = world.query_filtered::<Entity, (Or<(With<Mesh3d>, With<Mesh2d>, With<Camera>, With<Node>)>, Without<KeepAcrossLevels>)>();
    let candidates: Vec<Entity> = q.iter(world).collect();
    let mut gone = 0usize;
    for e in candidates {
        // A child of a kept entity stays with it.
        let mut cur = e;
        let mut kept = false;
        while let Some(p) = world.get::<ChildOf>(cur).map(|c| c.parent()) {
            if world.get::<KeepAcrossLevels>(p).is_some() {
                kept = true;
                break;
            }
            cur = p;
        }
        if kept { continue; }
        if world.get_entity(e).is_ok() && world.despawn(e) { gone += 1; }
    }
    println!("level switch: {gone} entities of the old level despawned");
}
