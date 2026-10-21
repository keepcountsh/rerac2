//! The glass canopy of Ratchet's ship (L00 `0x367b68`): not a moby but the ship's draw callback, registered for the
//! ship (class 0x0d54) every scene tick (`0x3716f8`: `0x2e7920(0x367b68, actor)`) and by the ship's own update. It
//! draws 38 four-vertex strips over 43 vertices of the level overlay's tables — positions `0x28aac0`, "normals"
//! `0x28afd0` (w = 1), the strips' vertex numbers `0x28ad70` (four `{s16 vertex, s16}` each) — in the frame of the
//! ship's joint list 0 (`0x312728(ship, 0)`), with the effect texture 0x15 (`0x2e71b8`) sphere-mapped by the view's
//! reflection and the colour 0x81a2a5 with alpha 0x3c (0x28 in a scene), blended (ALPHA 0x44).
//!
//! The reflection's texel coordinates follow the camera only while it is near (in play and in a scene: within 16 of
//! the ship in x and in y; always in the other modes): when it comes near again they ease from the ones it left over
//! 60 frames (`gp - 0x2d28` the countdown, `0x221268` the ones left); far, they hold.

use crate::hero::physics::V3;

/// The overlay's canopy tables.
#[derive(Clone, Debug, Default)]
pub struct CanopyMesh {
    pub pos: Vec<[f32; 4]>,
    pub nrm: Vec<[f32; 4]>,
    pub strips: Vec<[u16; 4]>,
}

pub const VERTS: usize = 43;
pub const STRIPS: usize = 38;
const POS: u32 = 0x28aac0;
const NRM: u32 = 0x28afd0;
const STRIP: u32 = 0x28ad70;
/// The effect texture (`0x2e71b8(0x15)`).
pub const TEXTURE: usize = 0x15;

impl CanopyMesh {
    /// The tables through `read(vaddr, len)` (the level overlay's data).
    pub fn read(read: impl Fn(u32, usize) -> Option<Vec<u8>>) -> Option<CanopyMesh> {
        let f = |b: &[u8], i: usize| f32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        let v4 = |a: u32| read(a, VERTS * 16).map(|b| (0..VERTS).map(|k| std::array::from_fn(|c| f(&b, 4 * k + c))).collect::<Vec<[f32; 4]>>());
        let s = read(STRIP, STRIPS * 16)?;
        let strips = (0..STRIPS).map(|k| std::array::from_fn(|c| i16::from_le_bytes([s[16 * k + 4 * c], s[16 * k + 4 * c + 1]]) as u16)).collect::<Vec<_>>();
        if strips.iter().flatten().any(|&i| i as usize >= VERTS) { return None; }
        Some(CanopyMesh { pos: v4(POS)?, nrm: v4(NRM)?, strips })
    }
}

/// This frame's canopy for the engine: world vertices, their texel coordinates, the strips, the colour (0x80 = 1).
#[derive(Clone, Debug, Default)]
pub struct CanopyFrame {
    pub verts: Vec<V3>,
    pub uvs: Vec<[f32; 2]>,
    pub strips: Vec<[u16; 4]>,
    pub rgba: [u8; 4],
}

/// The callback's state (`gp - 0x2d30..0x2d28`, `0x221110` / `0x221268`).
#[derive(Clone, Debug)]
pub struct Canopy {
    pub mesh: CanopyMesh,
    uv: Vec<[f32; 2]>,
    left: Vec<[f32; 2]>,
    /// `gp - 0x2d30`: 1 until the first draw computes the coordinates.
    first: bool,
    /// `gp - 0x2d2c`: the coordinates followed the camera last frame.
    near: bool,
    /// `gp - 0x2d28`.
    countdown: i32,
}

impl Canopy {
    pub fn new(mesh: CanopyMesh) -> Canopy { Canopy { mesh, uv: vec![[0.5; 2]; VERTS], left: vec![[0.5; 2]; VERTS], first: true, near: false, countdown: 0 } }

    /// `0x367b68` for a ship whose joint list 0 is `rows` at `at` (the 4×4 `0x312728` gives: rows and position) and
    /// whose place is `ship`, seen from `camera`, in game mode `mode`.
    pub fn draw(&mut self, rows: [V3; 3], at: V3, ship: V3, camera: V3, mode: i32) -> CanopyFrame {
        let alpha = if mode == crate::scene::SCENE { 0x28 } else { 0x3c };
        let near = !(mode == crate::scene::PLAY || mode == crate::scene::SCENE) || ((camera[0] - ship[0]).abs() < 16.0 && (camera[1] - ship[1]).abs() < 16.0);
        // `0x2ef340`: x·r0 + y·r1 + z·r2 + w·position.
        let apply = |v: [f32; 4]| -> V3 { std::array::from_fn(|k| v[0] * rows[0][k] + v[1] * rows[1][k] + v[2] * rows[2][k] + v[3] * at[k]) };
        let set_len = |v: V3, l: f32| { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n > 0.0 { v.map(|c| c * l / n) } else { v } };
        let verts: Vec<V3> = self.mesh.pos.iter().map(|&p| apply(p)).collect();
        if near || self.first {
            self.near = true;
            // `0x2eeab0`: the countdown down by one (not below 0).
            if self.countdown != 0 { self.countdown = self.countdown.max(1) - 1; }
            let t = self.countdown as f32 / 60.0;
            for (k, p) in verts.iter().enumerate() {
                let a = set_len(std::array::from_fn(|c| p[c] - camera[c]), 1.0);
                let b = set_len(apply(self.mesh.nrm[k]), 0.1);
                let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                let r = set_len(std::array::from_fn(|c| a[c] - b[c] * (d + d)), 1.0);
                let m = (2.0 * (r[2] + 1.0)).sqrt() * 2.0;
                let new = [r[0] / m + 0.5, r[1] / m + 0.5];
                self.uv[k] = if self.first || self.countdown == 0 { new } else { std::array::from_fn(|c| new[c] + (self.left[k][c] - new[c]) * t) };
            }
            self.first = false;
        } else {
            if std::mem::take(&mut self.near) { self.left = self.uv.clone(); }
            self.countdown = 60;
        }
        CanopyFrame { verts, uvs: self.uv.clone(), strips: self.mesh.strips.clone(), rgba: [0x81, 0xa2, 0xa5, alpha] }
    }
}
