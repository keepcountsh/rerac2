//! The metal pass's environment maps: Going Commando's moby metal packets take the effect textures −2 (chrome) and −3
//! (glass), which `GetEffectTex` (L00 `0x2e71b8`) does not load but paints: the first call of a frame queues a GS copy
//! of the frame buffer drawn so far (`0x1a72dc`, 512 wide, PSMCT32) into its own VRAM, the packets the boot keeps at
//! `0x138ad0` (chrome) / `0x138dc0` (glass): SCISSOR_2 to the map, ZBUF_2 masked, TEX1_2 bilinear, CLAMP_2 clamped,
//! RGBAQ 0x80808080, DTHE 1, then 16 textured sprites (PRIM 0x316, FST, context 2) that squash the 512×416 picture into
//! a 256×256 (chrome) or 64×64 (glass) PSMCT16 frame. The metal (crate::moby_render, moby_metal.wgsl) sphere-maps
//! it: the shine is the screen itself, softened. A CT16 texel's alpha is TEXA's (`0x8000000000` at boot: TA1 0x80 for
//! the alpha bit set, TA0 0 for it clear), and the bit is the frame's: the shadow pass clears the frame alpha to 0x7f
//! after the world (R&C1's `ShadowAlphaClear`, docs/plan/shadows.md §3.4) and every moby drawn after it writes 0x80,
//! so only the mobys reflect (the ship's red hull in its wings, the face behind the visor stays seen); the port's
//! frame alpha is 0 under the world for the same reason (crate::gs_state `specialize_world`).
//!
//! Here two 2D cameras ([`CHROME`], [`GLASS`]) draw the copy each frame before every game camera, from the game frame
//! (crate::display::GameFrame) as the last frame left it, over its 4:3 box (the PS2's 512×416 view): one frame late,
//! and with the whole frame (the mobys after the first shiny one and the HUD too), where the game copies the frame of
//! the moment. The shader (metal_env.wgsl) samples the frame as 512×416 texels (each the average of its pixels),
//! filters them bilinearly at the sprites' texel coordinates, dithers (the GS's 4×4 matrix) and keeps 5 bits a channel
//! as a CT16 frame would, in display bytes.

use bevy::asset::{uuid_handle, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{ClearColorConfig, Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};

/// The chrome map (effect texture −2, 256×256).
pub const CHROME: Handle<Image> = uuid_handle!("6b7f3c52-2f0e-4e7a-9c1d-3a5e8f0b2c41");
/// The glass map (effect texture −3, 64×64).
pub const GLASS: Handle<Image> = uuid_handle!("0d4e9a17-8b26-4c3f-a5e0-7f1b2d9c6e83");

const SHADER_PATH: &str = "shaders/metal_env.wgsl";
/// The copy cameras' layers (nothing else draws in them).
const CHROME_LAYER: usize = 61;
const GLASS_LAYER: usize = 62;
/// Before every other camera (the HUD layers −10, the screens −5, the sky −1, the world 0).
const ORDER: isize = -20;

/// The copy: the frame, its 4:3 box (uv origin, uv size) and the PS2 texels a map pixel steps (`box.zw`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct MetalEnvCopy {
    #[texture(0)]
    #[sampler(1)]
    pub frame: Handle<Image>,
    /// xy: the box's origin, zw: its size (uv of the frame).
    #[uniform(2)]
    pub area: Vec4,
    /// xy: the PS2 texels per map pixel (the sprites' UV step over their XY step); zw: the map's size.
    #[uniform(3)]
    pub step: Vec4,
}

impl Material2d for MetalEnvCopy {
    fn fragment_shader() -> ShaderRef { SHADER_PATH.into() }
    fn alpha_mode(&self) -> AlphaMode2d { AlphaMode2d::Opaque }
}

#[derive(Component)]
struct EnvCopy(Handle<MetalEnvCopy>);

pub struct MetalEnvPlugin;

impl Plugin for MetalEnvPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<MetalEnvCopy>::default())
            .add_systems(Startup, spawn)
            .add_systems(PostUpdate, follow_frame);
    }
}

/// A map: a render target the metal samples (bilinear, clamped: TEX1 MMAG linear, CLAMP_1 5).
fn map_image(side: u32) -> Image {
    let mut img = Image::new_fill(
        Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST;
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// The frame's 4:3 box in uv (crate::display::ui_box).
fn area(frame: &crate::display::GameFrame) -> Vec4 {
    let (o, s) = crate::display::ui_box(frame.size);
    let f = frame.size.max(UVec2::ONE).as_vec2();
    Vec4::new(o.x / f.x, o.y / f.y, s.x / f.x, s.y / f.y)
}

fn spawn(
    mut commands: Commands,
    frame: Res<crate::display::GameFrame>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MetalEnvCopy>>,
) {
    // The sprites: chrome U 32 / X 16 and V 416 / Y 256; glass U 32 / X 4 and V 416 / Y 64.
    for (handle, side, step, layer, name) in [(CHROME, 256u32, Vec2::new(2.0, 1.625), CHROME_LAYER, "chrome"), (GLASS, 64, Vec2::new(8.0, 6.5), GLASS_LAYER, "glass")] {
        let _ = images.insert(&handle, map_image(side));
        let material = materials.add(MetalEnvCopy { frame: frame.image.clone(), area: area(&frame), step: Vec4::new(step.x, step.y, side as f32, side as f32) });
        commands.spawn((
            Camera2d,
            Camera { order: ORDER + layer as isize - CHROME_LAYER as isize, clear_color: ClearColorConfig::None, ..default() },
            RenderTarget::Image(handle.clone().into()),
            Hdr,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(layer),
            crate::level_switch::KeepAcrossLevels,
            Name::new(format!("metal {name} map copy camera")),
        ));
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(side as f32, side as f32))),
            MeshMaterial2d(material.clone()),
            EnvCopy(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(layer),
            crate::level_switch::KeepAcrossLevels,
            Name::new(format!("metal {name} map copy")),
        ));
    }
}

/// The frame's image and box as they are now (a resize replaces the image).
fn follow_frame(frame: Res<crate::display::GameFrame>, copies: Query<&EnvCopy>, mut materials: ResMut<Assets<MetalEnvCopy>>) {
    let a = area(&frame);
    for c in &copies {
        let Some(m) = materials.get(&c.0) else { continue };
        if m.frame == frame.image && m.area == a { continue; }
        if let Some(mut m) = materials.get_mut(&c.0) {
            m.frame = frame.image.clone();
            m.area = a;
        }
    }
}
