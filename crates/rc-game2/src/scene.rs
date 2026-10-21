//! The scenes and movies (L00): the queue they are asked through (`0x1bef40`), the game's modes with their stack
//! (`0x1a8f00` the mode — 0 play, 1 a movie, 2 a scene — and the request `0x1a8f08`, `0x32c5f0` / `0x32c780` /
//! `0x32c858`), and the in-engine scene player (`0x370e08` the start, `0x3716f8` the tick, `0x3711e8` the end) over a
//! level's scenes ([`rc_formats::rac2::scene`], R&C1's chunk format). The engine loads the scene data, runs the world
//! in a scene's ticks, poses the actors and plays the movies.
//!
//! **The queue**: five `{id, is a scene}` entries in a ring. While a chain plays nothing more is taken; the chain
//! starts with its first entry (`0x2f5030`) and each scene's end starts the next (a movie's end too), the last one
//! going back to the mode before. A skip drops the rest of the chain, marking it seen (`0x2f5000`).
//!
//! **Into a scene from play** (`0x32c858`): the screen goes to black over 12 frames (`0x1a8fcc` up by 0.1 to 1.2, the
//! fade `0x1b1860` full once past 1); from a movie or a scene at once. **The start** (`0x370e08`): the id marked seen,
//! the fade full, the mode 2, Ratchet to state 0x61, the actors made (`0x305428`: class 0 is Ratchet's own moby; 0x215
//! and 0x10d1 play as Ratchet's ship 0x0d54; each takes the next sequence slot of its class, which every chunk
//! re-points at the chunk's streamed sequence), the 0xcb mobys hidden (mode 0x80).
//!
//! **Each tick** (`0x3716f8`, after the world's update — the moby loop and Ratchet's update, which in a scene does
//! nothing but his camera): the tick and the chunk tick +1; the fade down by 1/6 over the first 18 ticks; the skip
//! (from tick 18 with the fade out: Start alone on Aranos or once the game is beaten, else Start with L1 L2 R1 R2
//! held); at the end tick the next entry of the chain, else back; the chunk's rollover at 96 ticks; the camera from
//! the chunk tick's record; each actor at key frames `ct/2` and `ct/2 + 1`, blended by `(ct & 1)/2` (all the way on
//! an odd tick with a cut), at its track's lerp.
//!
//! **The end** (`0x3711e8`): the actors deleted (their class slots given back), the 0xcb mobys shown, Ratchet idle
//! (`0x2c8e30`) on the ground under him (within 4.5), the play camera's fov.
//!
//! Not ported: the speech (`0x32ea38`, the scene's audio start), the music pause and resume, the subtitles' other
//! languages, the attract-mode list (`0x1bef40` word 0), the end's teleport (`0x1bca86`) and callback (`0x1a8fd0`),
//! the mirror cheat's camera, the scene lighting and shadows of the actors, the modes 3..9.

use crate::hero::physics::*;
use rc_formats::scene::{Scene, Subtitle};
use rc_game::pad::{button, PadState};
use rc_game::scene_player::{scene_camera, SceneCamera};
use std::sync::Arc;

pub const PLAY: i32 = 0;
pub const MOVIE: i32 = 1;
pub const SCENE: i32 = 2;
/// `0x1a8f08` with no request.
pub const NONE: i32 = -2;

/// The queue `0x1bef40`.
#[derive(Clone, Debug, Default)]
pub struct Queue {
    ring: [(i32, bool); 5],
    write: usize,
    read: usize,
    count: usize,
    /// `+0x44`: a chain is playing.
    pub busy: bool,
    /// `+0x3c`: the chain was skipped.
    pub skipped: bool,
}

impl Queue {
    /// `0x2f4ee8`: `id` queued, unless the ring is full or a chain plays.
    pub fn push(&mut self, id: i32) -> bool {
        if self.count == 5 || self.busy { return false; }
        self.ring[self.write] = (id, rc_formats::rac2::scene::is_scene(id));
        self.write = (self.write + 1) % 5;
        self.count += 1;
        true
    }

    /// `0x2f4e48`.
    fn pop(&mut self) -> Option<(i32, bool)> {
        if self.count == 0 { return None; }
        self.count -= 1;
        if self.count == 0 { self.busy = false; }
        let e = self.ring[self.read];
        self.ring[self.read] = (-1, false);
        self.read = (self.read + 1) % 5;
        Some(e)
    }

    /// `0x2f4f80`: the entries still queued are marked seen.
    fn mark_seen(&self, seen: &mut [bool]) {
        let mut i = self.read;
        for _ in 0..self.count {
            let id = self.ring[i].0;
            if (0..0xb1).contains(&id) { if let Some(s) = seen.get_mut(id as usize) { *s = true; } }
            i = (i + 1) % 5;
        }
    }

    /// `0x2f5000`: the rest marked seen and dropped.
    pub fn clear(&mut self, seen: &mut [bool]) {
        self.mark_seen(seen);
        *self = Queue::default();
    }
}

/// One actor's pose this tick (`0x3716f8`): its moby, the sequence slot its class plays it in, the chunk's actor
/// record, the key frames and blend, the place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub moby: usize,
    pub o_class: i16,
    pub slot: u8,
    pub chunk: usize,
    pub actor: usize,
    pub frame_a: u8,
    pub frame_b: u8,
    pub t: f32,
    pub pos: V3,
}

/// The scene being played (`0x1bca40..`).
#[derive(Clone, Debug)]
pub struct Player {
    pub id: i32,
    pub scene: Arc<Scene>,
    /// `0x1bca74` / `0x1bca78` / `0x1bca7c`: the tick, the chunk's tick, the chunk.
    pub tick: i32,
    pub chunk_tick: i32,
    pub chunk: usize,
    /// `0x1bca98`.
    pub done: bool,
    /// `0x1bcbcc`: the actors' mobys and their class slots.
    pub actors: Vec<(usize, u8)>,
}

impl Player {
    pub fn end_tick(&self) -> i32 { self.scene.end_tick() }
}

/// What the engine does for a mode change.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Load scene `id` (its level record) and call [`Scenes::start_scene`].
    StartScene(i32),
    /// Play movie `id` (its row: [`rc_formats::rac2::scene::record_of`]); then [`Scenes::movie_done`].
    StartMovie(i32),
    /// The scene ended: [`Scenes::end_scene`] has run; the actors' class slots go back.
    EndedScene,
}

/// This tick's view of the scene for the engine.
#[derive(Clone, Debug)]
pub struct Frame {
    pub camera: SceneCamera,
    pub poses: Vec<Pose>,
    /// The chunk changed this tick: the class slots point at its sequences.
    pub new_chunk: bool,
    pub subtitle: Option<Subtitle>,
}

/// The modes and the scenes.
#[derive(Clone, Debug)]
pub struct Scenes {
    pub queue: Queue,
    /// `0x1a8f00` / `0x1a8f08` and the request's arguments `$gp − 0x4bb8` / `− 0x4bb4`.
    pub mode: i32,
    pub pending: i32,
    pub arg: (i32, i32),
    /// `0x1b2010`: the modes to go back to.
    stack: Vec<i32>,
    /// `0x1a8fcc`: the blackout into a scene; `0x1b1860`: the fade (1 black).
    pub zoom: f32,
    pub fade: f32,
    pub player: Option<Player>,
    /// The movie playing (mode 1).
    pub movie: Option<i32>,
}

impl Default for Scenes {
    fn default() -> Self {
        Scenes { queue: Queue::default(), mode: PLAY, pending: NONE, arg: (0, 0), stack: Vec::new(), zoom: 0.0, fade: 0.0, player: None, movie: None }
    }
}

impl Scenes {
    /// `0x371ba8`: a scene asked for or playing.
    pub fn in_scene(&self) -> bool { self.pending == SCENE || self.mode == SCENE }

    /// `0x32c5f0(mode, push, a, b)`: a mode asked for (none pending; with `push` at most 8 deep; Ratchet alive).
    /// Returns 0, or why not (< 0).
    pub fn request(&mut self, mode: i32, push: bool, a: i32, b: i32, hp: i32) -> i32 {
        let mut r = if self.pending == NONE { 0 } else { -1 };
        if push && 7 < self.stack.len() { r = -2; }
        if hp == 0 { r = -1; }
        if r == 0 {
            self.pending = mode;
            self.arg = (a, b);
            if push { self.stack.push(self.mode); }
        }
        r
    }

    /// `0x32c780`: back to the mode before.
    pub fn pop(&mut self, hp: i32) -> bool {
        if hp == 0 || self.pending != NONE { return false; }
        let Some(m) = self.stack.pop() else { return false };
        self.pending = m;
        self.arg = (0, 0);
        true
    }

    /// `0x2f5030`: the chain's next entry asked for (a scene with its id first, a movie its id second; replacing the
    /// mode while one plays, else pushed).
    pub fn start_queue(&mut self, hp: i32) -> bool {
        if self.queue.count == 0 { return false; }
        self.queue.skipped = false;
        self.queue.busy = true;
        let Some((id, scene)) = self.queue.pop() else { return false };
        let push = !(self.mode == MOVIE || self.mode == SCENE);
        let r = if scene { self.request(SCENE, push, id, 0, hp) } else { self.request(MOVIE, push, 0, id, hp) };
        if r < 0 { self.queue = Queue::default(); }
        r == 0
    }

    /// `0x32c858`, at the top of each frame: a pending mode taken (the old one left first). Returns what the engine
    /// is to do.
    pub fn switch(&mut self, hp: i32) -> Vec<Change> {
        let mut out = Vec::new();
        if self.pending == NONE || hp == 0 { return out; }
        let mut wait = false;
        if self.mode == SCENE { out.push(Change::EndedScene); }
        if self.mode == MOVIE { self.movie = None; }
        match self.pending {
            MOVIE => {
                self.movie = Some(self.arg.1);
                out.push(Change::StartMovie(self.arg.1));
            }
            SCENE => {
                if self.mode == MOVIE || self.mode == SCENE {
                    out.push(Change::StartScene(self.arg.0));
                } else {
                    approach(1.2, 0.1, &mut self.zoom);
                    if 1.0 < self.zoom { self.fade = 1.0; }
                    if 1.2 <= self.zoom {
                        out.push(Change::StartScene(self.arg.0));
                    } else {
                        wait = true;
                    }
                }
            }
            _ => {}
        }
        if !wait {
            self.mode = self.pending;
            self.pending = NONE;
        }
        out
    }

    /// `0x370e08` with the scene's data: the scene starts (module docs). `actors` are the mobys the engine made for
    /// the chunk's actors (class 0: Ratchet's) with their class slots.
    pub fn start_scene(&mut self, id: i32, scene: Arc<Scene>, actors: Vec<(usize, u8)>, seen: &mut [bool]) {
        if (0..0xb1).contains(&id) { if let Some(s) = seen.get_mut(id as usize) { *s = true; } }
        self.queue.mark_seen(seen);
        self.fade = 1.0;
        self.zoom = 0.0;
        self.mode = SCENE;
        self.player = Some(Player { id, scene, tick: 0, chunk_tick: 0, chunk: 0, done: false, actors });
    }

    /// `0x371bd8`'s end: the movie over (or skipped), the chain's next, else back.
    pub fn movie_done(&mut self, skipped: bool, hp: i32, seen: &mut [bool]) {
        if skipped {
            self.queue.skipped = true;
            self.queue.clear(seen);
        } else if self.start_queue(hp) {
            return;
        }
        self.pop(hp);
    }

    /// `0x3716f8`'s scene part, after the world's update: the tick (module docs). `None` once done.
    pub fn tick(&mut self, pad: &PadState, level: i32, beaten: bool, hp: i32, seen: &mut [bool]) -> Option<Frame> {
        let p = self.player.as_mut()?;
        if p.done { return None; }
        p.tick += 1;
        p.chunk_tick += 1;
        if p.tick < 0x12 { approach(0.0, 0.166_666_67, &mut self.fade); }
        let mut skip = false;
        if 0x11 < p.tick && self.fade == 0.0 {
            let start = pad.pressed & button::START != 0;
            skip = if (!beaten && 0 < level) || !start {
                start && pad.held & 0xf == 0xf
            } else {
                true
            };
        }
        let end = p.end_tick();
        if skip {
            self.queue.skipped = true;
            self.queue.clear(seen);
            p.done = true;
        } else if p.tick < end {
            let tpc = p.scene.ticks_per_chunk() as i32;
            let mut new_chunk = false;
            if tpc <= p.chunk_tick && p.chunk + 1 < p.scene.chunks.len() {
                p.chunk += 1;
                p.chunk_tick = 0;
                new_chunk = true;
            }
            let chunk = &p.scene.chunks[p.chunk];
            let ct = p.chunk_tick;
            let rec = chunk.camera.get(ct as usize).or(chunk.camera.last())?;
            let camera = scene_camera(rec);
            let mut poses = Vec::with_capacity(p.actors.len());
            for (k, &(moby, slot)) in p.actors.iter().enumerate() {
                let Some(a) = chunk.actors.get(k) else { continue };
                let fa = (ct >> 1) as u8;
                let fb = fa.wrapping_add(1);
                let t = if camera.cut && ct & 1 != 0 { 1.0 } else { (ct & 1) as f32 * 0.5 };
                let at = |f: u8| a.positions.get(f as usize).or(a.positions.last()).map_or([0.0; 3], |q| [q[0], q[1], q[2]]);
                let (pa, pb) = (at(fa), at(fb));
                let pos = [0, 1, 2].map(|i| pa[i] * (1.0 - t) + pb[i] * t);
                let o_class = match a.class { 0x215 | 0x10d1 => 0x0d54, c => c as i16 };
                poses.push(Pose { moby, o_class, slot, chunk: p.chunk, actor: k, frame_a: fa, frame_b: fb, t, pos });
            }
            let subtitle = p.scene.subtitle_at(p.tick).cloned();
            return Some(Frame { camera, poses, new_chunk, subtitle });
        } else {
            p.done = true;
            if self.start_queue(hp) { return None; }
        }
        self.pop(hp);
        None
    }

    /// `0x3711e8`: the scene's end on the game's side (module docs): Ratchet idle, on the ground under him. The
    /// engine deletes the actors it made and gives their class slots back.
    pub fn end_scene(&mut self, hero: &mut crate::hero::Hero, ground: Option<f32>) {
        self.player = None;
        self.fade = 0.0;
        self.zoom = 0.0;
        hero.request_state(0, true);
        if let Some(g) = ground {
            if 2.0 < g && (hero.pos[2] - g).abs() < 4.5 { hero.pos[2] = g; }
        }
    }
}
