//! Game logic of Ratchet & Clank: Going Commando, ported from the game's code (docs/plan/bringup.md): the boot's
//! front end ([`front_end`], [`gui`]), Ratchet ([`hero`], [`camera`]) and the mobys ([`moby`]). The engine (`rc-engine`)
//! loads the data, plays the movies, reads the pad and draws what this crate lays out. The systems that are the same
//! code as R&C1's (the pad record, the collision queries, Ratchet's animation player) came from R&C1's port as they
//! were (docs/plan/porting_from_rac1.md).

pub mod audio;
pub mod camera;
pub mod cameras;
pub mod canopy;
pub mod collision;
pub mod front_end;
pub mod fx;
pub mod game;
pub mod gui;
pub mod help;
pub mod hero;
pub mod hud;
pub mod level;
pub mod light;
pub mod memcard;
pub mod moby;
pub mod pad;
pub mod particles;
pub mod paths;
pub mod point_lights;
pub mod ps2v;
pub mod rng;
pub mod save;
pub mod scene;
pub mod scene_camera;
pub mod sky;
pub mod spline;
