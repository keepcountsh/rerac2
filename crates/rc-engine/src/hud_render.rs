//! The 2D pass: sprites, glyph quads and rectangles as the game sends them to the GS (PATH2 DIRECT packets, no VU1
//! program), drawn after all 3D: the front end (crate::front_end) and the HUD, the markers and the fades of the game
//! (crate::play) each draw a [`Layer`] of [`Hud2d`] primitives.
//!
//! **Mapping.** A corner at game pixel (x, y) is sent as `X = x·16 + OFX − 8` with `OFX = (2048 − 256)·16`, i.e.
//! at window pixel `x − 0.5`. The GS samples pixel *X* at its integer position, the GPU at `X + 0.5`, so the
//! corner lands exactly on GPU coordinate `x` of a 512×416 target and a w-wide primitive covers pixels
//! `x .. x + w − 1`, as on the GS. UVs are texel·16 (FST): the vertex UV here is in texels; at pixel `px` the
//! interpolated U is `u0 + (px + 0.5 − x)·Δu/Δx`, the GS's value.
//!
//! **Pixel pipeline** (`assets/shaders/hud.wgsl`). Texture: TEX1 bilinear + CLAMP inherited from the AA blit's
//! A+D block (the 2D packets set neither), done by hand on an atlas of the layer's textures (raw GS bytes, alpha
//! 0x80 = 1.0): sample position quantised to 1/16 texel (UV is 12.4 fixed point), four texels clamped to the
//! texture's own rectangle, weights `k/16`, result truncated [M]; a primitive drawn under TEX1_1 = 1
//! ([`Prim::nearest`]) takes the single texel `floor(U, V)` (point sampling). MODULATE: `C = Ct·Cf >> 7`,
//! `A = At·Af >> 7` (clamped to 0xff). Blend ALPHA_1 0x44 `(Cs − Cd)·As/128 + Cd` ([`GsPass::Hud`]: one blended
//! draw is exact). Untextured rectangles use the vertex RGBA. Scissor per primitive.
//!
//! **Where it draws.** A layer's primitives render in submission (painter's) order into an offscreen
//! `Rgba16Float` target on a dedicated `Camera2d` (order −10, [`HUD_LAYER`]): 512×416, the game's own resolution
//! (the HUD options Original and Sharp pixels: the GS's bilinear filter works on 512×416 pixels), or a whole
//! multiple of it near the frame's height (High resolution, [`size_layers`]: the primitives cover the same game
//! pixels, sampled finer, as an emulator's upscaled render draws them; the scissor stays in game pixels). Blending there runs in display bytes / 255 without clamping, premultiplied
//! (the target keeps `Σ Cs·As` in RGB and the coverage in A). The result is composited onto the frame by a Bevy UI
//! node ([`HudComposite`], `UiMaterial`) on the frame's 4:3 box, sampled as the HUD option says (crate::graphics:
//! Original bilinear half a pixel to the left, as the game's display copy and the TV showed the 512×416 picture,
//! crate::aa_blit; Sharp pixels nearest). The composite mixes in linear light on the sRGB target (exact where the
//! coverage is 0 or 1; the GS mixes display bytes), the difference the world passes also accept (gs_state.rs).

use crate::gs_state::GsPass;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{ClearColorConfig, Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues, VertexFormat};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension, TextureFormat,
};
use bevy::render::view::Msaa;
use bevy::shader::{ShaderDefVal, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin, MeshMaterial2d};
use bevy::ui::UiTargetCamera;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialKey, UiMaterialPlugin};
use rc_formats::texture::Texture;

const SHADER_PATH: &str = "shaders/hud.wgsl";
/// Render layer of the offscreen HUD camera and its mesh.
pub const HUD_LAYER: usize = 29;
/// The GS draw buffer (NTSC).
pub const W: i32 = 512;
pub const H: i32 = 416;
/// Atlas width in texels.
const ATLAS_W: u32 = 1024;

/// Per vertex: texel UV.
pub const ATTRIBUTE_UV: MeshVertexAttribute = MeshVertexAttribute::new("HudUv", 0x4855_4401, VertexFormat::Float32x2);
/// Per vertex: RGBA bytes (R low).
pub const ATTRIBUTE_RGBA: MeshVertexAttribute = MeshVertexAttribute::new("HudRgba", 0x4855_4402, VertexFormat::Uint32);
/// Per vertex: atlas x | y << 16, texture w | h << 16, flags (1 = textured, 2 = REPEAT, 4 = NEAREST), 0.
pub const ATTRIBUTE_TEX: MeshVertexAttribute = MeshVertexAttribute::new("HudTex", 0x4855_4403, VertexFormat::Uint32x4);
/// Per vertex: scissor x0, x1, y0, y1 (inclusive pixels).
pub const ATTRIBUTE_SCISSOR: MeshVertexAttribute = MeshVertexAttribute::new("HudScissor", 0x4855_4404, VertexFormat::Uint32x4);

/// A texture a primitive samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    None,
    /// The layer's texture n.
    Frame(usize),
}

/// One GS primitive: four corners in strip order (v0 v1 v2 v3: triangles 012, 123), game pixels, with texel UVs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prim {
    pub tex: Tex,
    pub pos: [[i32; 2]; 4],
    pub uv: [[i32; 2]; 4],
    pub rgba: u32,
    /// x0, x1, y0, y1, inclusive.
    pub scissor: [i32; 4],
    /// TEX1_1 MMAG = MMIN = NEAREST (point sampling) instead of the bilinear the 2D pass inherits: the pictures (the
    /// splash screens, the backdrop, a movie).
    pub nearest: bool,
}

const FULL_SCISSOR: [i32; 4] = [0, W - 1, 0, H - 1];


/// The frame's 2D primitive list, in draw order.
#[derive(Clone, Debug)]
pub struct Hud2d {
    pub prims: Vec<Prim>,
    scissor: [i32; 4],
    /// The primitives sent with 1/16-pixel corners: their index in [`Hud2d::prims`] and their corners in 1/16 pixels
    /// (the prim's own `pos` holds them rounded down).
    pub fine: Vec<(usize, [[i32; 2]; 4])>,
    /// The primitives (indices into `prims`, ascending) drawn under ALPHA_1 0x48 (`Cs·As + Cd`) instead of 0x44.
    pub add: Vec<usize>,
}

impl Default for Hud2d {
    fn default() -> Self { Hud2d { prims: Vec::new(), scissor: FULL_SCISSOR, fine: Vec::new(), add: Vec::new() } }
}

impl Hud2d {
    /// A screen primitive in pixels, its corners kept to 1/16 pixel.
    pub fn screen_prim(&mut self, tex: Tex, pos: [[f32; 2]; 4], uv: [[f32; 2]; 4], rgba: u32, add: bool) {
        if add { self.add.push(self.prims.len()); }
        let pos16 = pos.map(|p| [(p[0] * 16.0) as i32, (p[1] * 16.0) as i32]);
        self.fine.push((self.prims.len(), pos16));
        let uv = uv.map(|u| [u[0] as i32, u[1] as i32]);
        self.prims.push(Prim { tex, pos: pos16.map(|p| [p[0] >> 4, p[1] >> 4]), uv, rgba, scissor: self.scissor, nearest: false });
    }
}

/// The layer's textures packed into one image.
struct Atlas {
    rgba: Vec<u8>,
    height: u32,
    frames: Vec<[u32; 4]>,
}

/// Shelf-packs the textures (tallest first) into one raw-byte image.
fn build_atlas(frames: &[Texture]) -> Atlas {
    let mut items: Vec<(usize, &Texture)> = frames.iter().enumerate().collect();
    items.sort_by_key(|(i, t)| (std::cmp::Reverse(t.height), std::cmp::Reverse(t.width), *i));
    let mut rects = vec![[0u32; 4]; frames.len()];
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    for (i, t) in &items {
        if x + t.width > ATLAS_W {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        rects[*i] = [x, y, t.width, t.height];
        x += t.width;
        shelf = shelf.max(t.height);
    }
    let height = (y + shelf).max(1);
    let mut rgba = vec![0u8; (ATLAS_W * height * 4) as usize];
    for (i, t) in &items {
        let [rx, ry, w, _] = rects[*i];
        for row in 0..t.height {
            let src = &t.rgba[(row * w * 4) as usize..((row + 1) * w * 4) as usize];
            let dst = (((ry + row) * ATLAS_W + rx) * 4) as usize;
            rgba[dst..dst + src.len()].copy_from_slice(src);
        }
    }
    Atlas { rgba, height, frames: rects }
}

/// Pipeline key of [`HudMaterial`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HudMaterialKey {
    additive: bool,
}

impl From<&HudMaterial> for HudMaterialKey {
    fn from(m: &HudMaterial) -> Self { HudMaterialKey { additive: m.additive } }
}

/// ALPHA_1 0x44 and 0x48 on a premultiplied fragment (`Cs·As`, coverage As, or 0 under 0x48).
const PREMULT_BLEND: BlendState = BlendState {
    color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add },
    alpha: BlendComponent::OVER,
};

/// ALPHA_1 0x68 with FIX: `Cd + Cs·FIX/128` (the fragment outputs `Cs·FIX/128` and coverage 0, so the target's
/// premultiplied colour grows and its coverage stays).
const ADD_FIX_BLEND: BlendState = BlendState {
    color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
    alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
};

/// The 2D primitives' material: the atlas; `additive` = ALPHA_1 0x68 (FIX = the vertex alpha) instead of 0x44;
/// `target.x` the target's pixels per game pixel ([`size_layers`]).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(HudMaterialKey)]
pub struct HudMaterial {
    #[texture(0)]
    pub atlas: Handle<Image>,
    #[uniform(1)]
    pub target: Vec4,
    pub additive: bool,
}

impl Material2d for HudMaterial {
    fn vertex_shader() -> ShaderRef { SHADER_PATH.into() }
    fn fragment_shader() -> ShaderRef { SHADER_PATH.into() }
    fn alpha_mode(&self) -> AlphaMode2d { AlphaMode2d::Blend }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef, key: Material2dKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_UV.at_shader_location(1),
            ATTRIBUTE_RGBA.at_shader_location(2),
            ATTRIBUTE_TEX.at_shader_location(3),
            ATTRIBUTE_SCISSOR.at_shader_location(4),
        ])?];
        descriptor.vertex.shader_defs.push(ShaderDefVal::Bool("HUD_PRIMS".into(), true));
        if let Some(f) = descriptor.fragment.as_mut() { f.shader_defs.push(ShaderDefVal::Bool("HUD_PRIMS".into(), true)); }
        descriptor.primitive.cull_mode = None;
        GsPass::Hud.specialize(descriptor);
        // The GS blends 0x44 / 0x48 as `Cs·As + Cd·(1 − As)` / `Cs·As + Cd`: the fragment outputs `Cs·As` and its coverage
        // (0 for a 0x48 primitive), blended premultiplied.
        if let Some(f) = descriptor.fragment.as_mut() {
            for t in f.targets.iter_mut().flatten() { t.blend = Some(PREMULT_BLEND); }
        }
        if key.bind_group_data.additive {
            if let Some(f) = descriptor.fragment.as_mut() {
                f.shader_defs.push("HUD_ADD".into());
                for t in f.targets.iter_mut().flatten() { t.blend = Some(ADD_FIX_BLEND); }
            }
        }
        Ok(())
    }
}

/// The UI node that puts the 512×416 HUD image on the main camera (premultiplied → straight alpha), scaled as the
/// HUD option says ([`scaling`]).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct HudComposite {
    #[texture(0)]
    pub image: Handle<Image>,
    #[uniform(1)]
    pub scaling: Vec4,
}

impl UiMaterial for HudComposite {
    fn fragment_shader() -> ShaderRef { SHADER_PATH.into() }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        if let Some(f) = descriptor.fragment.as_mut() {
            f.entry_point = Some("composite".into());
            f.shader_defs.push(ShaderDefVal::Bool("HUD_COMPOSITE".into(), true));
            // Premultiplied: the additive draws (coverage 0) add (hud.wgsl `composite`).
            for t in f.targets.iter_mut().flatten() { t.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING); }
        }
        descriptor.vertex.shader_defs.push(ShaderDefVal::Bool("HUD_COMPOSITE".into(), true));
    }
}

/// The composites' scaling uniform for the HUD option (crate::graphics): x = 1 bilinear (Original: the TV showed the
/// 512×416 image smoothed), 0 nearest (Sharp pixels), 2 plain bilinear (High resolution: the image is near the
/// frame's size already).
pub fn scaling(h: crate::graphics::Hud) -> Vec4 {
    use crate::graphics::Hud;
    Vec4::new(match h { Hud::Original => 1.0, Hud::SharpPixels => 0.0, Hud::High => 2.0 }, 0.0, 0.0, 0.0)
}

/// The HUD option on every composite (a layer spawned after a change too).
fn apply_scaling(gfx: Res<crate::graphics::GraphicsSettings>, mut a: ResMut<Assets<HudComposite>>) {
    let v = scaling(gfx.hud);
    let stale: Vec<_> = a.iter().filter(|(_, m)| m.scaling != v).map(|(id, _)| id).collect();
    for id in stale { if let Some(mut m) = a.get_mut(id) { m.scaling = v; } }
}

/// A layer's target and material, on its camera ([`size_layers`]).
#[derive(Component)]
pub struct LayerTarget {
    image: Handle<Image>,
    material: Handle<HudMaterial>,
}

/// The layers' target size: 512×416 times the HUD option's scale (High resolution: the frame's height over 416,
/// rounded, 1..=8; else 1).
fn size_layers(
    gfx: Res<crate::graphics::GraphicsSettings>,
    frame: Option<Res<crate::display::GameFrame>>,
    layers: Query<&LayerTarget>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<HudMaterial>>,
) {
    let k = match (gfx.hud, frame) {
        (crate::graphics::Hud::High, Some(f)) => (f.size.y as f32 / H as f32).round().clamp(1.0, 8.0) as u32,
        _ => 1,
    };
    let size = Extent3d { width: W as u32 * k, height: H as u32 * k, depth_or_array_layers: 1 };
    for l in &layers {
        if images.get(&l.image).is_some_and(|i| i.texture_descriptor.size != size) {
            if let Some(mut i) = images.get_mut(&l.image) { i.resize(size); }
        }
        if materials.get(&l.material).is_some_and(|m| m.target.x != k as f32) {
            if let Some(mut m) = materials.get_mut(&l.material) { m.target.x = k as f32; }
        }
    }
}

/// A 2D layer with its own textures and primitive list (the front end, the game's HUD): an atlas, material, mesh
/// and composite. Its textures are [`Tex::Frame`] n; the composite sits on the frame's 4:3 box
/// (crate::display::UiBoxed), drawn by `ui_camera`.
pub struct Layer {
    pub atlas: Handle<Image>,
    pub mesh: Handle<Mesh>,
    /// Atlas rectangle of each texture.
    pub frames: Vec<[u32; 4]>,
    /// The layer's camera, primitives and composite, for [`Layer::despawn`].
    pub entities: [Entity; 3],
}

impl Layer {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        commands: &mut Commands,
        images: &mut Assets<Image>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<HudMaterial>,
        composites: &mut Assets<HudComposite>,
        textures: &[Texture],
        scaling: Vec4,
        ui_camera: Entity,
    ) -> Layer {
        let atlas = build_atlas(textures);
        let mut img = Image::new(
            Extent3d { width: ATLAS_W, height: atlas.height, depth_or_array_layers: 1 },
            TextureDimension::D2,
            atlas.rgba,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::all(),
        );
        img.sampler = bevy::image::ImageSampler::nearest();
        let atlas_handle = images.add(img);
        let target = images.add(Image::new_target_texture(W as u32, H as u32, TextureFormat::Rgba16Float, None));
        let material = materials.add(HudMaterial { atlas: atlas_handle.clone(), target: Vec4::new(1.0, 0.0, 0.0, 0.0), additive: false });
        let camera = commands.spawn((
            LayerTarget { image: target.clone(), material: material.clone() },
            Camera2d,
            Camera { order: -10, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            RenderTarget::Image(target.clone().into()),
            Hdr,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(HUD_LAYER),
            Name::new("2d layer camera (offscreen)"),
        )).id();
        let mesh = meshes.add(empty_mesh());
        let prims = commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(HUD_LAYER),
            Name::new("2d layer primitives"),
        )).id();
        let composite = commands.spawn((
            Node { position_type: PositionType::Absolute, ..default() },
            crate::display::UiBoxed,
            MaterialNode(composites.add(HudComposite { image: target, scaling })),
            UiTargetCamera(ui_camera),
            Name::new("2d layer composite"),
        )).id();
        Layer { atlas: atlas_handle, mesh, frames: atlas.frames, entities: [camera, prims, composite] }
    }

    /// Removes the layer's entities (its images and mesh go with their last handles).
    pub fn despawn(&self, commands: &mut Commands) { for e in self.entities { commands.entity(e).despawn(); } }

    /// The mesh of a primitive list.
    pub fn build(&self, d: &Hud2d) -> Mesh { build_mesh(&d.prims, &d.fine, &d.add, &self.frames) }

    /// Rewrites texture `i`'s texels (its own size, raw GS bytes): a movie's picture.
    pub fn write(&self, images: &mut Assets<Image>, i: usize, rgba: &[u8]) {
        let Some(&[x, y, w, h]) = self.frames.get(i) else { return };
        let Some(mut img) = images.get_mut(&self.atlas) else { return };
        let Some(data) = img.data.as_mut() else { return };
        for row in 0..h.min(rgba.len() as u32 / (w * 4).max(1)) {
            let src = &rgba[(row * w * 4) as usize..((row + 1) * w * 4) as usize];
            let dst = (((y + row) * ATLAS_W + x) * 4) as usize;
            data[dst..dst + src.len()].copy_from_slice(src);
        }
    }
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((Material2dPlugin::<HudMaterial>::default(), UiMaterialPlugin::<HudComposite>::default())).add_systems(PostUpdate, (apply_scaling, size_layers));
    }
}

fn empty_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
        .with_inserted_attribute(ATTRIBUTE_UV, vec![[0.0f32; 2]; 3])
        .with_inserted_attribute(ATTRIBUTE_RGBA, VertexAttributeValues::Uint32(vec![0; 3]))
        .with_inserted_attribute(ATTRIBUTE_TEX, VertexAttributeValues::Uint32x4(vec![[0; 4]; 3]))
        .with_inserted_attribute(ATTRIBUTE_SCISSOR, VertexAttributeValues::Uint32x4(vec![[1, 0, 1, 0]; 3]))
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

/// The primitives as one triangle list in submission order (the GPU blends triangles of one draw in order).
fn build_mesh(prims: &[Prim], fine: &[(usize, [[i32; 2]; 4])], add: &[usize], frames: &[[u32; 4]]) -> Mesh {
    if prims.is_empty() { return empty_mesh(); }
    let mut fine = fine.iter().peekable();
    let mut add = add.iter().peekable();
    let n = prims.len() * 4;
    let (mut pos, mut uv, mut rgba, mut tex, mut sc, mut idx) =
        (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n / 4 * 6));
    for (i, p) in prims.iter().enumerate() {
        let pos16 = fine.next_if(|(k, _)| *k == i).map(|(_, q)| *q);
        let additive = add.next_if(|&&k| k == i).is_some();
        let rect = match p.tex {
            Tex::None => None,
            Tex::Frame(i) => frames.get(i).copied(),
        };
        let blend = if additive { 8 } else { 0 };
        let t = match rect {
            Some([x, y, w, hh]) => [x | y << 16, w | hh << 16, 1 | if p.nearest { 4 } else { 0 } | blend, 0],
            None => [0, 0x0001_0001, blend, 0],
        };
        let s = p.scissor.map(|v| v.clamp(-1, 0xffff) as u32);
        let base = pos.len() as u32;
        for k in 0..4 {
            pos.push(match pos16 {
                Some(q) => [q[k][0] as f32 / 16.0, q[k][1] as f32 / 16.0, 0.0],
                None => [p.pos[k][0] as f32, p.pos[k][1] as f32, 0.0],
            });
            uv.push([p.uv[k][0] as f32, p.uv[k][1] as f32]);
            rgba.push(p.rgba);
            tex.push(t);
            sc.push([s[0], s[1], s[2], s[3]]);
        }
        idx.extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(ATTRIBUTE_UV, uv)
        .with_inserted_attribute(ATTRIBUTE_RGBA, VertexAttributeValues::Uint32(rgba))
        .with_inserted_attribute(ATTRIBUTE_TEX, VertexAttributeValues::Uint32x4(tex))
        .with_inserted_attribute(ATTRIBUTE_SCISSOR, VertexAttributeValues::Uint32x4(sc))
        .with_inserted_indices(Indices::U32(idx))
}
