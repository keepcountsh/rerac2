//! Draw-callback effects: the GS primitives the game sends straight from a moby's draw callback (`0x2e7920`, the
//! frame's second list, after the particles; the callbacks are rc_game2::fx). One material for all of them
//! ([`FxPrimMaterial`], `fx_prim.wgsl`: `0x2eb998` quads and strips with an FX texture, MODULATE, fog, ALPHA 0x44 /
//! 0x48 on the frame's display bytes through crate::display_blend) and one vertex builder ([`PrimBuf`]); crate::rac2_play
//! draws the tick's quads through [`FxSlots`].

use crate::game_camera::TfragFog;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;

const SHADER_PATH: &str = "shaders/fx_prim.wgsl";
/// Transparent3d sort bias of the list-1 callback draws: after the mobys, before the particles (1e6).
pub const LIST1_BIAS: f32 = 9.0e5;

/// Static uniform of one draw group.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct FxPrimParams {
    /// x = 1: ALPHA 0x48 (additive `Cs·As + Cd`), 0: ALPHA 0x44 (`(Cs − Cd)·As + Cd`). y = 1: the subtractive
    /// `Cd − Cs·As` (ALPHA 0x62 with As = FIX: [`FxPrimParams::subtract`]; a pipeline key). w = 1: an opaque world
    /// surface ([`FxPrimParams::opaque`]; the pipeline key).
    pub misc: Vec4,
}

impl FxPrimParams {
    pub fn blend(additive: bool) -> Self { FxPrimParams { misc: Vec4::new(additive as u32 as f32, 0.0, 0.0, 0.0) } }

    /// ALPHA 0x62 (`(0 − Cs)·FIX + Cd`) with As = FIX through the vertex alpha: the colour taken off the frame.
    pub fn subtract() -> Self { FxPrimParams { misc: Vec4::new(0.0, 1.0, 0.0, 0.0) } }

    fn is_opaque(&self) -> bool { self.misc.w > 0.5 }
    fn is_subtract(&self) -> bool { self.misc.y > 0.5 }
}

/// The pipeline key: an effect (display blend, no Z) or an opaque world surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FxPrimKey {
    opaque: bool,
    subtract: bool,
}

impl From<&FxPrimMaterial> for FxPrimKey {
    fn from(m: &FxPrimMaterial) -> Self { FxPrimKey { opaque: m.params.is_opaque(), subtract: m.params.is_subtract() } }
}

/// One draw group of a callback: an FX texture, the fog, one of the two ALPHA equations, its Transparent3d order.
/// Drawn by crate::display_blend's effect pass (the entity carries `DisplayEffect`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(FxPrimKey)]
pub struct FxPrimMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    #[uniform(2)]
    pub fog: TfragFog,
    #[uniform(3)]
    pub params: FxPrimParams,
    /// Transparent3d sort bias (the callback list's band + the draw's position in it).
    pub order: f32,
}

impl Material for FxPrimMaterial {
    fn vertex_shader() -> ShaderRef { SHADER_PATH.into() }
    fn fragment_shader() -> ShaderRef { SHADER_PATH.into() }
    fn alpha_mode(&self) -> AlphaMode { if self.params.is_opaque() { AlphaMode::Opaque } else { AlphaMode::Blend } }
    fn depth_bias(&self) -> f32 { self.order }
    fn enable_prepass() -> bool { false }
    fn enable_shadows() -> bool { false }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if key.bind_group_data.opaque {
            crate::gs_state::GsPass::Opaque.specialize(descriptor);
            if let Some(f) = descriptor.fragment.as_mut() { f.shader_defs.push("FX_OPAQUE".into()); }
        } else {
            // TEST_1 0x53001 / 0x51001 (ATST NEVER, AFAIL FB_ONLY: colour, never Z; ZTST GEQUAL).
            crate::gs_state::GsPass::BlendNoZ.specialize(descriptor);
            crate::display_blend::specialize(descriptor);
            if key.bind_group_data.subtract { crate::display_blend::specialize_subtract(descriptor); }
        }
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?];
        Ok(())
    }
}

/// An FX texture (`GetEffectTex`) as an image of raw GS bytes; TEX1 0xff9000000260 (bilinear, no mips), CLAMP 0
/// (repeat). The decoded texture's alpha is scaled to 0..0xff (`rc_formats::texture::scale_alpha`); it goes back to
/// GS units (0x80 = 1.0) here, as for the particle textures (`particle_render`), since `fx_prim.wgsl` multiplies the
/// texel alpha by the vertex alpha like the GS (`At·Av >> 7`).
pub fn fx_image(images: &mut Assets<Image>, t: &rc_formats::texture::Texture) -> Handle<Image> {
    let mut img = Image::new_uninit(
        Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut rgba = t.rgba.clone();
    for a in rgba.iter_mut().skip(3).step_by(4) { *a = if *a == 0xff { 0x80 } else { *a / 2 }; }
    img.data = Some(rgba);
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    images.add(img)
}

/// Mesh data of one draw group being built: game-space points, (s, t), GS RGBA bytes.
#[derive(Default)]
pub struct PrimBuf {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    color: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl PrimBuf {
    fn vertex(&mut self, p: [f32; 3], st: [f32; 2], c: u32) -> u32 {
        let b = self.pos.len() as u32;
        self.pos.push(crate::tfrag_render::game_to_bevy(p).to_array());
        self.uv.push(st);
        self.color.push([(c & 0xff) as f32, (c >> 8 & 0xff) as f32, (c >> 16 & 0xff) as f32, (c >> 24) as f32].map(|x| x / 128.0));
        b
    }

    /// One `FastDrawQuadReal` quad: corners in GS strip order (triangles 0 1 2, 1 2 3). A quad whose four alphas are
    /// 0 adds nothing under either equation and is skipped.
    pub fn quad(&mut self, p: [[f32; 3]; 4], st: [[f32; 2]; 4], rgba: [u32; 4]) {
        if rgba.iter().all(|c| c >> 24 == 0) { return; }
        let b = self.pos.len() as u32;
        for k in 0..4 { self.vertex(p[k], st[k], rgba[k]); }
        self.idx.extend([b, b + 1, b + 2, b + 1, b + 2, b + 3]);
    }

    pub fn is_empty(&self) -> bool { self.idx.is_empty() }

    /// Whether `mesh` already holds these primitives (crate::asset_write: a rewrite is an upload).
    fn same_as(&self, mesh: &Mesh) -> bool {
        use bevy::mesh::VertexAttributeValues as V;
        matches!(mesh.try_attribute_option(Mesh::ATTRIBUTE_POSITION), Ok(Some(V::Float32x3(v))) if *v == self.pos)
            && matches!(mesh.try_attribute_option(Mesh::ATTRIBUTE_UV_0), Ok(Some(V::Float32x2(v))) if *v == self.uv)
            && matches!(mesh.try_attribute_option(Mesh::ATTRIBUTE_COLOR), Ok(Some(V::Float32x4(v))) if *v == self.color)
            && matches!(mesh.try_indices_option(), Ok(Some(Indices::U32(v))) if *v == self.idx)
    }

    /// Writes into mesh `h` unless it already holds these primitives.
    pub fn update(self, meshes: &mut Assets<Mesh>, h: &Handle<Mesh>) {
        if meshes.get(h).is_some_and(|m| self.same_as(m)) { return; }
        if let Some(mut m) = meshes.get_mut(h) { self.write(&mut m); }
    }

    pub fn write(self, mesh: &mut Mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.color);
        mesh.insert_indices(Indices::U32(self.idx));
    }
}

/// One draw of a callback this tick: FX texture, equation and primitives.
pub struct FxGroup {
    pub fx: usize,
    pub additive: bool,
    /// [`FxPrimParams::subtract`] (over `additive`).
    pub subtract: bool,
    pub prims: PrimBuf,
}

/// One slot's entity, mesh, material and whether it is shown.
type FxSlot = (Entity, Handle<Mesh>, Handle<FxPrimMaterial>, bool);

/// The persistent entities of the callback draws: slot k = the k-th draw of the list this tick (Bevy's mesh
/// allocator keeps no slab for an empty mesh, so an entity is made when its slot first has primitives).
#[derive(Default)]
pub struct FxSlots {
    slots: Vec<Option<FxSlot>>,
    fx: Vec<Option<Handle<Image>>>,
    /// The render layer of the slots' entities (None: the default layer; the flight's draws are on the transition's
    /// layer, crate::flight_render).
    pub layer: Option<bevy::camera::visibility::RenderLayers>,
}

/// The assets the slots need.
pub struct FxAssets<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub images: &'a mut Assets<Image>,
    pub materials: &'a mut Assets<FxPrimMaterial>,
    pub fx: Option<&'a [Option<rc_formats::texture::Texture>]>,
    pub fog: TfragFog,
}

impl FxSlots {
    /// FX image `i`, made on first use.
    fn image(&mut self, a: &mut FxAssets, i: usize) -> Option<Handle<Image>> {
        let t = a.fx?.get(i)?.as_ref()?;
        if self.fx.len() <= i { self.fx.resize(i + 1, None); }
        if self.fx[i].is_none() { self.fx[i] = Some(fx_image(a.images, t)); }
        self.fx[i].clone()
    }

    /// Shows `groups` in slots 0.. (bias `band + slot`) and hides the rest.
    pub fn show(&mut self, commands: &mut Commands, vis: &mut Query<&mut Visibility>, a: &mut FxAssets, groups: Vec<FxGroup>, band: f32, name: &str) {
        let n = groups.len();
        for (k, g) in groups.into_iter().enumerate() {
            let img = self.image(a, g.fx);
            if self.slots.len() <= k { self.slots.push(None); }
            let show = !g.prims.is_empty() && img.is_some();
            if show {
                let img = img.expect("checked");
                let params = if g.subtract { FxPrimParams::subtract() } else { FxPrimParams::blend(g.additive) };
                match &mut self.slots[k] {
                    Some((_, mesh, mat, _)) => {
                        g.prims.update(a.meshes, mesh);
                        let need = a.materials.get(&*mat).is_none_or(|m| m.texture != img || m.params.misc != params.misc || m.fog != a.fog);
                        if need {
                            if let Some(mut m) = a.materials.get_mut(&*mat) {
                                m.texture = img;
                                m.params = params;
                                m.fog = a.fog;
                            }
                        }
                    }
                    None => {
                        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                        g.prims.write(&mut m);
                        let mesh = a.meshes.add(m);
                        let mat = a.materials.add(FxPrimMaterial { texture: img, fog: a.fog, params, order: band + k as f32 });
                        let e = commands
                            .spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(mat.clone()),
                                Transform::IDENTITY,
                                NoFrustumCulling,
                                Visibility::Inherited,
                                crate::display_blend::DisplayEffect,
                                Name::new(format!("{name} draw {k}")),
                            ))
                            .id();
                        if let Some(l) = &self.layer { commands.entity(e).insert(l.clone()); }
                        self.slots[k] = Some((e, mesh, mat, true));
                        continue;
                    }
                }
            }
            set_visible(vis, &mut self.slots[k], show);
        }
        for s in self.slots.iter_mut().skip(n) { set_visible(vis, s, false); }
    }
}

fn set_visible(vis: &mut Query<&mut Visibility>, s: &mut Option<FxSlot>, show: bool) {
    let Some((e, _, _, shown)) = s else { return };
    if *shown == show { return; }
    *shown = show;
    if let Ok(mut v) = vis.get_mut(*e) { *v = if show { Visibility::Inherited } else { Visibility::Hidden }; }
}

pub struct FxDrawPlugin;

impl Plugin for FxDrawPlugin {
    fn build(&self, app: &mut App) { app.add_plugins(MaterialPlugin::<FxPrimMaterial>::default()); }
}
