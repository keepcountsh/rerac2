//! Render-to-texture onto a quad in the world: a 3D view of some entities rendered into an offscreen image and drawn
//! on four world corners (a screen inside the level that moves with what carries it). Going Commando's ship screen in
//! Aranos's arrival uses it (crate::rac2_play, `rc_game::scene::Scenes::ship_screen`).
//!
//! **What it reproduces.** The game hides the moby from the frame, draws it alone into a VRAM render target with a
//! view of its own and the frame's projection (centred in the target, so the target shows the middle of what the
//! frame would), then draws the target on a textured quad among the frame's 3D draws.
//!
//! **API** ([`WorldScreens`], a resource):
//! * [`WorldScreens::create`]`(commands, images, meshes, materials, size, name) → WorldScreenId` once (an image, a
//!   hidden quad, a render layer of its own);
//! * [`WorldScreens::layer`]`(id)`: the `RenderLayers` the caller puts on the entities the screen shows (off layer 0,
//!   so the frame does not draw them);
//! * [`WorldScreens::show`]`(id, Some(WorldScreenView { .. }))` every frame it is visible, `None` to hide it (the camera
//!   is despawned: other systems take "the" `Camera3d`).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::Msaa;

use crate::game_camera::GameProjection;

/// The first render layer the screens take (one each).
const FIRST_LAYER: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldScreenId(usize);

/// Where and how a screen shows this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldScreenView {
    /// The offscreen view's camera transform (crate::play_camera::view_transform of a game view).
    pub eye: Transform,
    /// Its projection's half-angle tangents (x, y) over the image.
    pub tan: (f32, f32),
    /// The quad's corners (Bevy space) and their texel coordinates; triangles (0, 1, 2) and (2, 1, 3).
    pub corners: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub clear: Color,
}

struct Slot {
    image: Handle<Image>,
    cam: Option<Entity>,
    quad: Entity,
    mesh: Handle<Mesh>,
    layer: usize,
    view: Option<WorldScreenView>,
    shown: bool,
}

#[derive(Resource, Default)]
pub struct WorldScreens {
    slots: Vec<Slot>,
}

impl WorldScreens {
    /// A new screen (hidden) with a `size` image.
    pub fn create(&mut self, commands: &mut Commands, images: &mut Assets<Image>, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, size: UVec2, name: &str) -> WorldScreenId {
        let image = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None));
        let mesh = meshes.add(quad_mesh(&[Vec3::ZERO; 4], &[[0.0; 2]; 4]));
        let mat = materials.add(StandardMaterial { base_color_texture: Some(image.clone()), unlit: true, cull_mode: None, double_sided: true, fog_enabled: false, ..default() });
        let quad = commands
            .spawn((Name::new(format!("world screen {name}")), Mesh3d(mesh.clone()), MeshMaterial3d(mat), Transform::IDENTITY, Visibility::Hidden, bevy::camera::visibility::NoFrustumCulling))
            .id();
        let layer = FIRST_LAYER + self.slots.len();
        self.slots.push(Slot { image, cam: None, quad, mesh, layer, view: None, shown: false });
        WorldScreenId(self.slots.len() - 1)
    }

    pub fn layer(&self, id: WorldScreenId) -> RenderLayers { RenderLayers::layer(self.slots[id.0].layer) }

    /// This frame's view (None: hidden).
    pub fn show(&mut self, id: WorldScreenId, view: Option<WorldScreenView>) { self.slots[id.0].view = view; }
}

fn quad_mesh(corners: &[Vec3; 4], uvs: &[[f32; 2]; 4]) -> Mesh {
    let n = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners.map(|c| c.to_array()).to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![n.to_array(); 4])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs.to_vec())
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 2, 1, 3]))
}

pub struct WorldScreenPlugin;

impl Plugin for WorldScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldScreens>()
            .add_systems(crate::level_switch::LevelUnload, crate::level_switch::reset::<WorldScreens>)
            .add_systems(PostUpdate, apply.before(bevy::transform::TransformSystems::Propagate));
    }
}

fn apply(mut commands: Commands, mut screens: ResMut<WorldScreens>, mut meshes: ResMut<Assets<Mesh>>, mut cams: Query<(&mut Transform, &mut Projection, &mut Camera)>) {
    for s in &mut screens.slots {
        let Some(v) = s.view else {
            if let Some(e) = s.cam.take() { commands.entity(e).despawn(); }
            if std::mem::take(&mut s.shown) { commands.entity(s.quad).insert(Visibility::Hidden); }
            continue;
        };
        let proj = GameProjection { tan_x: v.tan.0, tan_y: v.tan.1, ..GameProjection::default() };
        match s.cam {
            None => {
                let cam = commands.spawn((
                    Camera3d::default(),
                    Camera { order: -5, clear_color: ClearColorConfig::Custom(v.clear), ..default() },
                    RenderTarget::Image(s.image.clone().into()),
                    Projection::custom(proj),
                    Msaa::Off,
                    Tonemapping::None,
                    DebandDither::Disabled,
                    RenderLayers::layer(s.layer),
                    v.eye,
                    Name::new(format!("world screen camera (layer {})", s.layer)),
                ));
                s.cam = Some(cam.id());
            }
            Some(e) => {
                if let Ok((mut t, mut p, mut c)) = cams.get_mut(e) {
                    if *t != v.eye { *t = v.eye; }
                    let cur = match &*p {
                        Projection::Custom(c) => c.get::<GameProjection>().map(|g| (g.tan_x, g.tan_y)),
                        _ => None,
                    };
                    if cur != Some(v.tan) { *p = Projection::custom(proj); }
                    if !matches!(c.clear_color, ClearColorConfig::Custom(k) if k == v.clear) { c.clear_color = ClearColorConfig::Custom(v.clear); }
                }
            }
        }
        if let Some(mut m) = meshes.get_mut(&s.mesh) { *m = quad_mesh(&v.corners, &v.uvs); }
        if !std::mem::replace(&mut s.shown, true) { commands.entity(s.quad).insert(Visibility::Inherited); }
    }
}
