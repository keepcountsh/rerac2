// Tie pass: instanced class meshes, colour = the instance's lit colour slots, GS TFX = MODULATE,
// GS fog (one F per instance), GS mip rule. Rust side: tie_render.rs; lighting: rc_formats::tie_light.
//
// Instance data (`insts[mesh tag]`): the class -> Bevy-world matrix (the game's column-major instance
// matrix with game_to_bevy applied after it; it may scale, shear and mirror, so the entity transform is
// not used for positions), the bounding-sphere centre (Bevy world) with the draw distance in w, the radius, and the
// instance's first colour in `colors` (packed RGBA8, 0x80 = 1.0: its class's lit slots, then the self-lit colour).
//
// Per-frame TieProc decision (`lods[2 * mesh tag]`, tie_lod.rs): x = LOD (3 = culled) | F << 8, y = k,
// z = VU qw4.w = 256k, w = qw4.z = 256 - 256k (f32 bits). The next word is the wind shear
// (`lods[2 * mesh tag + 1].xy` = sx, sy, f32 bits; 0 without sway): TieProc shears the instance columns
// `x += sx·z, y += sy·z` (world Z up, about the origin), applied here to the morphed position. Vertices of the
// other LODs are dropped; VU1 program 13507 writes the same F for every vertex of an instance (qw 5.w).
// Fat vertices: position + k * delta. Colour: the average of the vertex's four colour slots, per byte
// (`(a + b + c + d) >> 2`, Going Commando's light pass `0x36f0a8` writing the colour cache), or 0 (black flag).
// Texture: same GS rules as tfrag.wgsl (MODULATE, raw display-encoded bytes, As = At * Af >> 7, mip level
// round(log2(z / 32) + K) clamped to 0..MXL).
#import bevy_pbr::{
    mesh_functions,
    view_transformations::{position_world_to_clip, position_world_to_view},
}

struct TieFog {
    color: vec4<f32>,
    params: vec4<f32>,
    // x: the texture option (crate::graphics; sample_world).
    tex_mode: vec4<f32>,
}

struct TieParams {
    // x = near (32), y = MXL, z = 1: tint by LOD.
    misc: vec4<f32>,
}

struct TieInst {
    model: mat4x4<f32>,
    // xyz = bounding-sphere centre (Bevy world), w = draw distance (world units).
    centre: vec4<f32>,
    // x = bounding radius (world units), y = the instance's first colour (u32 bits).
    misc: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<storage, read> fog: TieFog;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> params: TieParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage, read> insts: array<TieInst>;

@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage, read> lods: array<vec4<u32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<storage, read> colors: array<u32>;

// The texture option (crate::graphics, `fog.tex_mode.x`): 0 Original = the GS level round(L) clamped to 0..MXL (one
// level, bilinear inside it); 1 Smooth = L unrounded, the two nearest levels blended by the sampler (no switch line);
// 2 Sharp = the GPU's own level from the UV footprint `duv` (dx, dy), anisotropic. The sampler's mip filter is linear:
// an integer level reads that level alone, as the GS does.
fn sample_world(uv: vec2<f32>, l: f32, mxl: f32, duv: vec4<f32>) -> vec4<f32> {
    let mode = fog.tex_mode.x;
    if mode > 1.5 {
        return textureSampleGrad(tex, tex_sampler, uv, duv.xy, duv.zw);
    }
    let level = select(clamp(floor(l + 0.5), 0.0, mxl), clamp(l, 0.0, mxl), mode > 0.5);
    return textureSampleLevel(tex, tex_sampler, uv, level);
}

struct TieVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // colour 0 | colour 1 << 10 | TEX1 K (s12, 1/16) << 20
    @location(2) info: u32,
    // colour 2 | black << 10 | fat << 12 | LOD << 13 | colour 3 << 16
    @location(3) morph: u32,
    @location(4) delta: vec3<f32>,
}

struct TieVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(linear) color: vec4<f32>,
    @location(2) @interpolate(flat) fog: f32,
    // Camera depth in raw units, perspective-correct (= n / Q per pixel).
    @location(3) depth: f32,
    @location(4) @interpolate(flat) k: f32,
    // Debug tint: 0 none, 1 LOD 1, 2 LOD 2.
    @location(5) @interpolate(flat) tint: u32,
}

@vertex
fn vertex(v: TieVertex) -> TieVertexOutput {
    var out: TieVertexOutput;
    let tag = mesh_functions::get_tag(v.instance_index);
    let inst = &insts[tag];
    let lod = lods[2u * tag];
    let vlod = (v.morph >> 13u) & 3u;
    if ((lod.x & 3u) != vlod) {
        // Culled this frame (3), or a vertex of another LOD: the triangle is clipped away.
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let k = bitcast<f32>(lod.y);
    let pos = v.position + k * v.delta;
    let sw = bitcast<vec2<f32>>(lods[2u * tag + 1u].xy);
    var world: vec3<f32>;
    if (sw.x != 0.0 || sw.y != 0.0) {
        // ShrubProc's shear (shrub.wgsl): about the origin, x += sx·y, z -= sy·y in Bevy axes.
        var off = ((*inst).model * vec4<f32>(pos, 0.0)).xyz;
        off = vec3<f32>(off.x + sw.x * off.y, off.y, off.z - sw.y * off.y);
        world = off + (*inst).model[3].xyz;
    } else {
        world = ((*inst).model * vec4<f32>(pos, 1.0)).xyz;
    }
    out.position = position_world_to_clip(world);
    out.uv = v.uv;
    var rgba = 0u;
    if (((v.morph >> 10u) & 1u) == 0u) {
        let first = bitcast<u32>((*inst).misc.y);
        let c = array<u32, 4>(colors[first + (v.info & 0x3ffu)], colors[first + ((v.info >> 10u) & 0x3ffu)],
            colors[first + (v.morph & 0x3ffu)], colors[first + ((v.morph >> 16u) & 0x3ffu)]);
        for (var i = 0u; i < 32u; i += 8u) {
            let sum = ((c[0] >> i) & 0xffu) + ((c[1] >> i) & 0xffu) + ((c[2] >> i) & 0xffu) + ((c[3] >> i) & 0xffu);
            rgba |= (sum >> 2u) << i;
        }
    }
    out.color = unpack4x8unorm(rgba) * (255.0 / 128.0);
    out.depth = -position_world_to_view(world).z * 1024.0;
    out.fog = f32(lod.x >> 8u) / 255.0;
    out.k = f32(bitcast<i32>(v.info) >> 20u) / 16.0;
    out.tint = select(0u, vlod, params.misc.z > 0.5);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: TieVertexOutput) -> @location(0) vec4<f32> {
    let duv = vec4<f32>(dpdx(in.uv), dpdy(in.uv));
    let t = sample_world(in.uv, log2(in.depth / params.misc.x) + in.k, params.misc.y, duv);
    var rgb = min(t.rgb * in.color.rgb, vec3<f32>(1.0));
    if (fog.color.w > 0.5) {
        rgb = mix(fog.color.rgb, rgb, in.fog);
    }
    if (in.tint == 1u) { rgb = mix(rgb, vec3<f32>(1.0, 0.0, 0.0), 0.5); }
    if (in.tint == 2u) { rgb = mix(rgb, vec3<f32>(0.0, 0.2, 1.0), 0.5); }
    let a_s = min(floor(round(t.a * 255.0) * round(in.color.a * 128.0) / 128.0), 255.0);
    // GS TEST_1 alpha test (ATST GEQUAL AREF, AFAIL RGB_ONLY): which half of the split this draw is (gs_state.rs).
#ifdef GS_ATEST_PASS
    if (a_s < f32(#{GS_AREF})) { discard; }
#endif
#ifdef GS_ATEST_FAIL
    if (a_s >= f32(#{GS_AREF})) { discard; }
#endif
    return vec4<f32>(srgb_to_linear(rgb), a_s / 128.0);
}
