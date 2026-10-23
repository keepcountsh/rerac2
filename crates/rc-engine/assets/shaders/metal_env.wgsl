// The metal pass's environment map copy (Rust side: metal_env.rs): the game frame's 4:3 box squashed into the chrome
// (256×256) or glass (64×64) map as the GS sprites of the boot's 0x138ad0 / 0x138dc0 packets do. The frame (display
// bytes in an sRGB image) is read as 512×416 PS2 texels, each the hardware average at its centre; a map pixel takes
// the GS's bilinear filter at U = x·step.x, V = y·step.y (texel centres at +0.5, clamped), then DTHE's 4×4 dither and
// the PSMCT16 frame's 5 bits a channel (read back << 3). Alpha: the frame's is 0 under the world (crate::gs_state
// `specialize_world`, as the game's shadow pass clears it to 0x7f before the mobys) and full where a moby drew; the
// GS filters 0x7f / 0x80 and keeps bit 7, so a map pixel gets TEXA's TA1 (0x80) only when every texel it filters is
// a moby's, else TA0 (0). Output: GS bytes / 255.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var frame: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var frame_sampler: sampler;
// xy: the box's origin, zw: its size (uv of the frame).
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> area: vec4<f32>;
// xy: the PS2 texels per map pixel; zw: the map's size.
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> step: vec4<f32>;

const PS2: vec2<f32> = vec2<f32>(512.0, 416.0);

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// PS2 texel `t` (clamped): display bytes, and 1 where most of it is a moby's (alpha bit 7).
fn texel(t: vec2<i32>) -> vec4<f32> {
    let c = clamp(t, vec2<i32>(0), vec2<i32>(PS2) - 1);
    let uv = area.xy + (vec2<f32>(c) + 0.5) / PS2 * area.zw;
    let s = textureSampleLevel(frame, frame_sampler, uv, 0.0);
    return vec4<f32>(linear_to_srgb(s.rgb) * 255.0, select(0.0, 1.0, s.a >= 0.5));
}

// The GS's dither matrix (DIMX), row by y & 3.
const DITHER: array<vec4<f32>, 4> = array<vec4<f32>, 4>(
    vec4<f32>(-4.0, 2.0, -3.0, 3.0),
    vec4<f32>(0.0, -2.0, 1.0, -1.0),
    vec4<f32>(-3.0, 3.0, -4.0, 2.0),
    vec4<f32>(1.0, -1.0, 0.0, -2.0),
);

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let px = vec2<u32>(floor(in.position.xy));
    let p = vec2<f32>(px) * step.xy - 0.5;
    let i = vec2<i32>(floor(p));
    let f = p - floor(p);
    let t00 = texel(i);
    let t10 = texel(i + vec2<i32>(1, 0));
    let t01 = texel(i + vec2<i32>(0, 1));
    let t11 = texel(i + vec2<i32>(1, 1));
    let c = floor(mix(mix(t00.rgb, t10.rgb, f.x), mix(t01.rgb, t11.rgb, f.x), f.y));
    // Bit 7 of the filtered alpha: no weight on a texel without it.
    let w = vec4<f32>((1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y);
    let lit = all((w <= vec4<f32>(0.0)) | (vec4<f32>(t00.a, t10.a, t01.a, t11.a) > vec4<f32>(0.5)));
    let d = DITHER[px.y & 3u][px.x & 3u];
    let q = floor(clamp(c + d, vec3<f32>(0.0), vec3<f32>(255.0)) / 8.0) * 8.0;
    return vec4<f32>(q / 255.0, select(0.0, 128.0 / 255.0, lit));
}
