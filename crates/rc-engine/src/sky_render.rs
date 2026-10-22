//! The level sky, drawn as the game draws it (docs/plan/sky_render_notes.md).
//!
//! The game builds the sky's GS packets on the EE (no VU1 program): every shell in index order, every
//! cluster and face in stored order, as a plain triangle list (`rc_formats::sky::sky_gs_vertices`).
//! Per shell kind (A+D blocks 0x13d0f0 / 0x13d160, sky_render_notes.md §3):
//! * gouraud (`flags != 0`): RGBAQ = vertex colour, no texture, ZBUF ZMSK 0 (writes Z), ZTST ALWAYS;
//! * textured (`flags == 0`): MODULATE with RGBAQ = (0x80, 0x80, 0x80, vertex alpha), Q = 1, ZMSK 1, ZTST ALWAYS;
//! * both: ALPHA_1 = (Cs − Cd)·As + Cd, FGE 0 (no fog), GS Z = 0 on every vertex (the builders 0x22c0e0 /
//!   0x22c208 store the XYZ2 high word as zero), no back-face culling.
//!
//! Projection: `p = v·SkyM·C` with C = boot 0x187040 = the rotation-only view 0x186f40 (translation row
//! zero) times the world projection with rows 0/1 ×4, which the screen scale 0x18ce90·¼ cancels. So the
//! sky uses the world camera's exact projection and a view without translation; SkyM is the per-shell
//! rotation the game steps each frame (`0x35dfa0`: Euler angles, [`GameSkyRotations`]).
//!
//! Bevy mapping: a second `Camera3d` on [`SKY_LAYER`], ordered before the main camera, with the same
//! projection and transform; the shader drops the view translation (`w = 0`), SkyM is the entity's
//! rotation. Every draw is `AlphaMode::Blend` (SrcAlpha / OneMinusSrcAlpha = the GS equation for
//! As ≤ 0x80) in the Transparent3d phase, whose back-to-front sort is overridden by a per-draw
//! `depth_bias` so the draws run exactly in the game's order; `specialize` sets depth compare Always and
//! depth writes for gouraud shells only, and the vertex shader puts every vertex at depth 0 (= GS Z 0, the
//! far end under the game's reverse-Z mapping, game_camera.rs). The main camera then loads colour and depth
//! instead of clearing, so it draws over the sky like the rest of the GS chain.
//!
//! Output: both cameras share the view's main texture, so after the world pass it holds the whole frame. The main
//! camera writes it to the target as is (`BlendState::REPLACE`): Bevy would otherwise composite a second camera on
//! the same target with alpha blending over the first one's earlier output (the sky pass alone), and every world
//! pixel whose frame alpha is below 1 (e.g. a tie fat vertex whose VU-blended alpha lane truncates 0x80 to 0x7f,
//! or a translucent draw's `OVER` alpha) would let that stale image through: the see-through, flickering surfaces
//! of 2026-09-27. The GS frame buffer's alpha is never used for display either.
//! The sky camera writes nothing (`CameraOutputMode::Skip`): the main camera's write replaces the whole target, so
//! the sky's own copy of the main texture to it was a full-frame blit thrown away every frame.
//!
//! Frame clear: the game clears (colour = level background, Z = 0) before the sky only while sky header
//! +4 is non-zero. The loader sets it to 1; [`level_zeroes_clear_flag`] says on which levels the frame code zeroes
//! it (the frame then cleared only on the first frame, after which the gouraud dome is the clear).
//!
//! Not ported: the star sprites, the per-cluster bounding-sphere and per-face clip-flag rejection (GPU clipping
//! replaces them).

use crate::game_camera;
use crate::gs_state::GsPass;
use crate::level_load::LoadedLevel;
use crate::tfrag_render::game_to_bevy;
use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{Camera3dDepthLoadOp, CameraOutputMode, ClearColorConfig};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension,
    TextureFormat,
};
use bevy::shader::ShaderRef;
use bevy::transform::TransformSystems;
use rc_formats::level::LevelCore;
use rc_formats::sky::{self, Sky, SkyTexture};

const SHADER_PATH: &str = "shaders/sky.wgsl";

/// Render layer of the sky meshes and the sky camera.
pub const SKY_LAYER: usize = 1;

/// Transparent3d sort spacing between consecutive sky draws. The sort key is the view depth of the mesh
/// centre plus this bias; mesh centres lie within ±32768 raw units of the camera, far below the spacing.
pub const ORDER_SPACING: f32 = 1.0e6;

/// The parsed sky and its decoded textures.
pub struct LevelSky {
    pub level: u32,
    pub sky: Sky,
    pub textures: Vec<SkyTexture>,
}

/// Parses the level's sky block (None when the core index has none; every retail level has one).
pub fn load(core: &LevelCore, core_data: &[u8], level: u32) -> Result<Option<LevelSky>> {
    let Some(block) = sky::sky_block(core, core_data).context("locating sky block")? else { return Ok(None) };
    let sky = sky::parse_sky_block(block).context("parsing sky")?;
    let textures = sky::parse_sky_textures(block, &sky).context("decoding sky textures")?;
    Ok(Some(LevelSky { level, sky, textures }))
}

/// Whether the level's frame code zeroes sky header +4 (`clear_screen`) every frame, i.e. the frame is cleared only on
/// the first frame after load. Not read from Going Commando's code yet: R&C1's rule (every level but 05, 07, 10, 13,
/// 14, 15 zeroes it), which on Aranos clears the first frame only (G-REN-009).
pub fn level_zeroes_clear_flag(level: u32) -> bool { !matches!(level, 5 | 7 | 10 | 13 | 14 | 15) }

/// Pipeline key: gouraud shells write depth (ZMSK 0), textured ones do not (ZMSK 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SkyMaterialKey {
    textured: bool,
}

impl From<&SkyMaterial> for SkyMaterialKey {
    fn from(m: &SkyMaterial) -> Self { SkyMaterialKey { textured: m.textured } }
}

/// One sky draw: a run of consecutive faces of one shell with one texture state.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(SkyMaterialKey)]
pub struct SkyMaterial {
    /// Raw GS texel alpha (0..0x80) in the alpha channel; a 1×1 white for gouraud shells (unused there).
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    pub textured: bool,
    /// Position in the game's draw order.
    pub order: u32,
}

impl Material for SkyMaterial {
    fn vertex_shader() -> ShaderRef { SHADER_PATH.into() }
    fn fragment_shader() -> ShaderRef { SHADER_PATH.into() }
    fn alpha_mode(&self) -> AlphaMode { AlphaMode::Blend }
    /// Sort key bias: Transparent3d draws ascending, so a larger bias draws later.
    fn depth_bias(&self) -> f32 { self.order as f32 * ORDER_SPACING }
    fn enable_prepass() -> bool { false }
    fn enable_shadows() -> bool { false }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let textured = key.bind_group_data.textured;
        descriptor.primitive.cull_mode = None;
        // Depth compare Always, depth write for gouraud shells only, the GS blend (crate::gs_state).
        let pass = if textured { GsPass::SkyTextured } else { GsPass::SkyDome };
        pass.specialize_world(descriptor);
        if textured {
            descriptor.vertex.shader_defs.push("SKY_TEXTURED".into());
            if let Some(f) = descriptor.fragment.as_mut() { f.shader_defs.push("SKY_TEXTURED".into()); }
        }
        Ok(())
    }
}

/// Marks the sky camera.
#[derive(Component)]
pub struct SkyCamera;

/// A sky mesh entity of shell `shell`.
#[derive(Component)]
struct SkyShellEntity {
    shell: usize,
}

/// The shells' rotations a game computes itself (Going Commando's `0x35dfa0`: Euler angles stepped each frame), by
/// shell index; shells past the list are unturned. Inserted every frame by its play.
#[derive(Resource, Default)]
pub struct GameSkyRotations(pub Vec<Quat>);

/// SkyM of Euler angles `a` (x, y, z; `0x2ef6f8` = R&C1's builder 0x1fa070: v' = v·Rz·Ry·Rx) as a Bevy rotation.
pub fn euler_rotation(a: [f32; 3]) -> Quat {
    let axis = |v: [f32; 3]| game_to_bevy(v);
    Quat::from_axis_angle(axis([1.0, 0.0, 0.0]), a[0]) * Quat::from_axis_angle(axis([0.0, 1.0, 0.0]), a[1]) * Quat::from_axis_angle(axis([0.0, 0.0, 1.0]), a[2])
}

/// The level's sky state.
#[derive(Resource)]
struct SkyAnim {
    /// The frame code zeroes the clear flag: clear on the first frame only.
    clear_first_frame_only: bool,
}

pub struct SkyRenderPlugin;

impl Plugin for SkyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .add_systems(crate::level_switch::LevelStartup, spawn_sky)
            .add_systems(crate::level_switch::LevelPostStartup, main_camera_loads)
            .add_systems(crate::level_switch::LevelUnload, crate::level_switch::remove::<SkyAnim>)
            .add_systems(Update, (animate_shells, stop_clearing))
            .add_systems(PostUpdate, follow_main_camera.before(TransformSystems::Propagate));
    }
}

fn spawn_sky(
    mut commands: Commands,
    level: Res<crate::Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let level: &LoadedLevel = &level.0;
    let Some(ls) = &level.sky else {
        println!("sky: none in this level; the frame is cleared to the level background");
        return;
    };
    let s = spawn_shells(&mut commands, ls, RenderLayers::layer(SKY_LAYER), &mut meshes, &mut images, &mut materials, |e, si| {
        e.insert((Transform::IDENTITY, SkyShellEntity { shell: si }));
    });
    let (order, n_tris) = (s.order, s.triangles);
    let has_dome = ls.sky.shells.iter().any(|s| !s.textured());
    let clear_first_frame_only = level_zeroes_clear_flag(ls.level);
    commands.spawn((
        Camera3d { depth_load_op: Camera3dDepthLoadOp::Clear(0.0), ..default() },
        // First frame: the loader's clear_screen = 1 clears to the level background (the ClearColor resource).
        Camera { order: -1, clear_color: ClearColorConfig::Default, output_mode: CameraOutputMode::Skip, ..default() },
        game_camera::game_projection(),
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(SKY_LAYER),
        SkyCamera,
        // Same MSAA as the main camera (crate::render_settings): the main pass loads this pass's depth.
        crate::render_settings::WorldCamera,
    ));
    println!(
        "sky: {} shells ({} gouraud), {} draws, {} triangles, {} textures; frame clear: {}",
        ls.sky.shells.len(), ls.sky.shells.iter().filter(|s| !s.textured()).count(), order, n_tris, ls.textures.len(),
        match (clear_first_frame_only, has_dome) {
            (true, true) => "first frame only (the gouraud dome is the clear)",
            (true, false) => "first frame only (no gouraud shell: previous frames persist where the sky is transparent)",
            (false, _) => "every frame",
        }
    );
    commands.insert_resource(SkyAnim { clear_first_frame_only });
}

/// What [`spawn_shells`] made.
struct SpawnedShells {
    /// The next draw-order slot after the shells.
    order: u32,
    triangles: usize,
}

/// The shells of a sky block as draws (one per run of consecutive faces with the same texture, in shell, cluster and
/// face order) on render layer `layer`, each given its components by `extra(entity, shell)` (the shell's SkyM
/// `Transform`, a marker).
fn spawn_shells(
    commands: &mut Commands,
    ls: &LevelSky,
    layer: RenderLayers,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<SkyMaterial>,
    mut extra: impl FnMut(&mut bevy::ecs::system::EntityCommands, usize),
) -> SpawnedShells {
    let white = images.add(Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[255, 255, 255, 0x80],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let tex_images: Vec<Handle<Image>> = ls.textures.iter().map(|t| images.add(sky_image(t))).collect();
    let (mut order, mut n_tris) = (0u32, 0usize);
    for (si, shell) in ls.sky.shells.iter().enumerate() {
        // Runs of consecutive faces with the same texture, in cluster and face order: one draw each.
        let mut runs: Vec<(u8, Vec<sky::SkyGsVertex>)> = Vec::new();
        for c in &shell.clusters {
            let verts = sky::sky_gs_vertices(shell, c);
            for tri in verts.as_chunks::<3>().0 {
                match runs.last_mut() {
                    Some((t, v)) if *t == tri[0].texture => v.extend_from_slice(tri),
                    _ => runs.push((tri[0].texture, tri.to_vec())),
                }
            }
        }
        for (tex, verts) in runs {
            n_tris += verts.len() / 3;
            let textured = shell.textured();
            let image = if textured {
                match ls.textures.iter().position(|t| t.index == tex as usize) {
                    Some(i) => tex_images[i].clone(),
                    None => { warn!("sky shell {si}: texture {tex} not decoded; drawing white"); white.clone() }
                }
            } else { white.clone() };
            let positions: Vec<[f32; 3]> =
                verts.iter().map(|v| game_to_bevy(v.position.map(|x| x as f32)).to_array()).collect();
            let uvs: Vec<[f32; 2]> = verts.iter().map(|v| v.st).collect();
            // Raw GS RGBA bytes; the shader applies the GS rules per shell kind.
            let colors: Vec<[f32; 4]> = verts.iter().map(|v| v.rgba.map(|c| c as f32)).collect();
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            let mut e = commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(SkyMaterial { texture: image, textured, order })),
                layer.clone(),
                NoFrustumCulling,
                Name::new(format!("sky shell {si} tex {tex}")),
            ));
            extra(&mut e, si);
            order += 1;
        }
    }
    SpawnedShells { order, triangles: n_tris }
}

/// Decoded sky texture as the GS sees it: display-encoded RGB, raw texel alpha 0..0x80.
pub(crate) fn sky_image(t: &SkyTexture) -> Image {
    let mut rgba = t.texture.rgba.clone();
    // Undo `texture::scale_alpha` (a < 0x80 → 2a, a ≥ 0x80 → 0xff): exact for a ≤ 0x80, which holds for
    // every texel the Novalis sky uses.
    for a in rgba.iter_mut().skip(3).step_by(4) { *a = if *a == 0xff { 0x80 } else { *a / 2 }; }
    let mut img = Image::new(
        Extent3d { width: t.texture.width, height: t.texture.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    // TEX1: bilinear mag/min, no mips; CLAMP_1: clamp on both axes.
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Nearest,
        ..default()
    });
    img
}

/// The world pass continues the GS chain after the sky: no colour clear, depth loaded from the sky pass, and the
/// finished frame (sky + world, one shared main texture) replaces the target instead of being alpha-blended over the
/// sky camera's output (module doc, "Output").
fn main_camera_loads(
    sky_cam: Query<(), With<SkyCamera>>,
    mut cams: Query<(&mut Camera, &mut Camera3d), Without<SkyCamera>>,
) {
    if sky_cam.is_empty() { return; }
    for (mut cam, mut c3d) in &mut cams {
        cam.clear_color = ClearColorConfig::None;
        cam.output_mode = CameraOutputMode::Write { blend_state: Some(BlendState::REPLACE), clear_color: ClearColorConfig::None };
        c3d.depth_load_op = Camera3dDepthLoadOp::Load;
    }
}

/// After the first frame, levels whose dispatch zeroes the clear flag stop clearing.
fn stop_clearing(anim: Option<Res<SkyAnim>>, mut cams: Query<&mut Camera, With<SkyCamera>>, mut frames: Local<u32>, generation: Res<crate::level_switch::LevelGeneration>) {
    // A runtime level change (crate::level_switch): the new level's first frame clears again.
    if generation.is_changed() { *frames = 0; }
    let Some(anim) = anim else { return };
    *frames += 1;
    if *frames != 2 || !anim.clear_first_frame_only { return; }
    for mut cam in &mut cams { cam.clear_color = ClearColorConfig::None; }
}

/// SkyM per shell from the game's rotations.
fn animate_shells(game: Option<Res<GameSkyRotations>>, mut shells: Query<(&SkyShellEntity, &mut Transform)>) {
    let Some(g) = game else { return };
    for (s, mut t) in &mut shells {
        let r = g.0.get(s.shell).copied().unwrap_or(Quat::IDENTITY);
        if t.rotation != r { t.rotation = r; }
    }
}

type MainView<'w, 's> = Query<'w, 's, (&'static Transform, &'static Projection), (With<crate::fly_cam::FlyCam>, Without<SkyCamera>)>;
type SkyView<'w, 's> = Query<'w, 's, (&'static mut Transform, &'static mut Projection), (With<SkyCamera>, Without<crate::fly_cam::FlyCam>)>;

/// The sky camera shares the main camera's transform and **projection** (the game draws the sky with the world
/// camera's own projection, 0x187040 = the rotation-only view times the world projection; the shader removes the
/// translation). The projection follows every change of tan(hfov/2) 0x16cf70: a scene camera record's (0.414 or 0.554
/// on Novalis, gameplay 0.63), so the sky turns exactly with the world instead of sliding against it.
pub(crate) fn follow_main_camera(main: MainView, mut sky: SkyView) {
    let Some((src, proj)) = main.iter().next() else { return };
    let tan = |p: &Projection| match p {
        Projection::Custom(c) => c.get::<game_camera::GameProjection>().map(|g| (g.tan_x, g.tan_y)),
        _ => None,
    };
    let want = tan(proj);
    for (mut t, mut p) in &mut sky {
        if *t != *src { *t = *src; }
        if want.is_some() && tan(&p) != want {
            if let (Some((x, y)), Projection::Custom(c)) = (want, &mut *p) {
                if let Some(g) = c.get_mut::<game_camera::GameProjection>() {
                    g.tan_x = x;
                    g.tan_y = y;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Rz(θ) row-vector form: rows (c, s, 0), (−s, c, 0): game +X turns toward +Y (counter-clockwise about up).
    #[test]
    fn rotation_is_ccw_about_game_up() {
        let q = euler_rotation([0.0, 0.0, PI / 2.0]);
        let x = q * game_to_bevy([1.0, 0.0, 0.0]);
        assert!((x - game_to_bevy([0.0, 1.0, 0.0])).length() < 1e-6, "{x}");
    }
}
