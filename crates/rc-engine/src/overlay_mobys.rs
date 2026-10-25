//! Mobys drawn over the 2D layer inside a GUI window: the vendor's preview (`0x37ba50` draws its preview list after
//! the frame sprites and the HUD, clipped by `0x37ca08(158, 360, 115, 206)`); the game's other screens that draw
//! mobys over their panels can use the same window.
//!
//! A second `Camera3d` on the game frame ([`OverlayCamera`], order 10: after the main camera and its UI, which
//! composites the 2D layer) draws only [`OVERLAY_LAYER`] with the main camera's transform and projection, its
//! viewport and sub-camera view the window's rectangle (game pixels of the 512×416 picture on the frame's 4:3 box,
//! as the HUD composite is placed: crate::display::ui_box), its depth cleared so the mobys draw over everything.
//! It is active only while a window is asked for ([`OverlayWindow`]). The window's mobys are created mobys
//! (crate::mobys::CreatedDraw): their draw entities move to the overlay layer while listed, back to the default
//! layer after.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera3dDepthLoadOp, CameraOutputMode, ClearColorConfig, RenderTarget, SubCameraView, Viewport};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use std::collections::HashSet;

/// The overlay camera's render layer.
pub const OVERLAY_LAYER: usize = 28;
/// The game's picture (the GS draw buffer).
const GAME_W: f32 = 512.0;
const GAME_H: f32 = 416.0;

/// What to draw over the 2D layer this frame: the window (`[x0, x1, y0, y1]`, game pixels, inclusive as the GS
/// scissor takes them) and the created mobys in it.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayWindow {
    pub rect: Option<[i32; 4]>,
    pub mobys: Vec<usize>,
}

#[derive(Component)]
pub struct OverlayCamera;

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        // [`assign_layers`] runs from crate::play after its created-moby draw.
        app.init_resource::<OverlayWindow>().add_systems(Update, spawn_camera).add_systems(PostUpdate, place_camera.after(crate::display::ResizeTargets));
    }
}

/// The camera, once the game frame exists.
fn spawn_camera(mut commands: Commands, frame: Option<Res<crate::display::GameFrame>>, cams: Query<(), With<OverlayCamera>>) {
    let Some(frame) = frame else { return };
    if !cams.is_empty() { return; }
    commands.spawn((
        Camera3d { depth_load_op: Camera3dDepthLoadOp::Clear(0.0), ..default() },
        Camera { order: 10, is_active: false, clear_color: ClearColorConfig::None, output_mode: CameraOutputMode::Write { blend_state: Some(BlendState::REPLACE), clear_color: ClearColorConfig::None }, ..default() },
        RenderTarget::Image(frame.image.clone().into()),
        crate::game_camera::game_projection(),
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(OVERLAY_LAYER),
        OverlayCamera,
        crate::render_settings::WorldCamera,
        crate::level_switch::KeepAcrossLevels,
        Name::new("overlay moby camera"),
    ));
}

type MainView<'w, 's> = Query<'w, 's, (&'static Transform, &'static Projection), (With<crate::fly_cam::FlyCam>, Without<OverlayCamera>)>;
type OverlayView<'w, 's> = Query<'w, 's, (&'static mut Camera, &'static mut Transform, &'static mut Projection), (With<OverlayCamera>, Without<crate::fly_cam::FlyCam>)>;

/// The camera follows the main camera and takes the window: active with one, its viewport and sub-camera view the
/// window's pixels on the frame.
fn place_camera(window: Res<OverlayWindow>, frame: Option<Res<crate::display::GameFrame>>, main: MainView, mut cams: OverlayView) {
    let Some(frame) = frame else { return };
    let Some((src, proj)) = main.iter().next() else { return };
    let (o, s) = crate::display::ui_box(frame.size);
    for (mut cam, mut t, mut p) in &mut cams {
        if *t != *src { *t = *src; }
        *p = proj.clone();
        let Some([x0, x1, y0, y1]) = window.rect.filter(|_| !window.mobys.is_empty()) else {
            if cam.is_active { cam.is_active = false; }
            continue;
        };
        let px = |x: i32| (o.x + x as f32 * s.x / GAME_W).round().max(0.0) as u32;
        let py = |y: i32| (o.y + y as f32 * s.y / GAME_H).round().max(0.0) as u32;
        let (l, r, tp, bm) = (px(x0).min(frame.size.x), px(x1 + 1).min(frame.size.x), py(y0).min(frame.size.y), py(y1 + 1).min(frame.size.y));
        if r <= l || bm <= tp {
            if cam.is_active { cam.is_active = false; }
            continue;
        }
        let (pos, size) = (UVec2::new(l, tp), UVec2::new(r - l, bm - tp));
        let same = cam.viewport.as_ref().is_some_and(|v| v.physical_position == pos && v.physical_size == size) && cam.sub_camera_view.as_ref().is_some_and(|v| v.full_size == frame.size);
        if !same {
            cam.viewport = Some(Viewport { physical_position: pos, physical_size: size, ..default() });
            cam.sub_camera_view = Some(SubCameraView { full_size: frame.size, offset: pos.as_vec2(), size });
        }
        if !cam.is_active { cam.is_active = true; }
    }
}

/// The window's mobys' draw entities on the overlay layer; the others back on the default one.
pub(crate) fn assign_layers(mut commands: Commands, window: Res<OverlayWindow>, draw: Option<Res<crate::mobys::CreatedDraw>>, mut on_layer: Local<HashSet<Entity>>) {
    let mut want: HashSet<Entity> = HashSet::new();
    if let (Some(draw), Some(_)) = (draw.as_deref(), window.rect) {
        for &m in &window.mobys { want.extend(draw.entities_of(m)); }
    }
    for &e in on_layer.iter() {
        if !want.contains(&e) {
            if let Ok(mut ec) = commands.get_entity(e) { ec.insert(RenderLayers::layer(0)); }
        }
    }
    for &e in &want {
        if !on_layer.contains(&e) {
            if let Ok(mut ec) = commands.get_entity(e) { ec.insert(RenderLayers::layer(OVERLAY_LAYER)); }
        }
    }
    *on_layer = want;
}
