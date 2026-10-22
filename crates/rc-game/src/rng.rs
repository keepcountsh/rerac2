//! The game's random stream: the boot's newlib `rand` (`0x1163b0`: `s = s·0x41c64e6d + 0x3039`, `s & 0x7fffffff`),
//! seeded with 1234 by the level load (`0x3012a0`'s `srand(0x4d2)`, `0x1163a0`), and the helpers the level code
//! calls on it (native `f32`). Every consumer shares the stream, so a sequence matches the game's only when the
//! consumers run in the game's order.

/// The level load's seed.
pub const LEVEL_SEED: u32 = 1234;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    pub state: u32,
}

impl Default for Rng {
    fn default() -> Self { Rng { state: LEVEL_SEED } }
}

impl Rng {
    /// `0x1163a0`.
    pub fn srand(&mut self, seed: u32) { self.state = seed; }

    /// `0x1163b0`.
    pub fn rand(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        self.state & 0x7fff_ffff
    }

    fn r15(&mut self) -> u32 { (self.rand() >> 16) & 0x7fff }

    /// `0x31b9a0(n)`: `r % n`.
    pub fn randi(&mut self, n: i32) -> i32 { if n == 0 { 0 } else { self.r15() as i32 % n } }

    /// `0x31ba38(a, b)`: `a + r·(b − a)·2⁻¹⁵`.
    pub fn randf(&mut self, a: f32, b: f32) -> f32 { a + self.r15() as f32 * (b - a) * 3.051_757_8e-5 }

    /// `0x31ba98(a, b)`: `a + r·(b − a)·2⁻¹²` with r the low 12 bits, negated when r is odd.
    pub fn randf_signed(&mut self, a: f32, b: f32) -> f32 {
        let r = (self.rand() >> 16) & 0x7fff;
        let v = a + (r & 0xfff) as f32 * (b - a) * 0.000_244_140_62;
        if r & 1 != 0 { -v } else { v }
    }

    /// `0x31bb00`: an angle in [−π, π): `((rand >> 16) & 0xfff) − 0x800` steps of π / 2048.
    pub fn rand_angle(&mut self) -> f32 { (((self.rand() >> 16) & 0xfff) as i32 - 0x800) as f32 * 0.001_533_980_8 }

    /// `0x31b9e0(a, b)`: an integer in a..=b (`r % (b − a + 1) + a`).
    pub fn rand_range(&mut self, a: i32, b: i32) -> i32 { self.randi(b - a + 1) + a }

    /// `randf(a, b)` on PS2 float bits, in the FPU's own rounding (the sound layer's random pitches and volumes).
    pub fn randf_bits(&mut self, a: u32, b: u32) -> u32 {
        use crate::ps2v;
        let r = ps2v::itof0(self.r15() as i32);
        ps2v::add(a, ps2v::mul(ps2v::mul(r, ps2v::sub(b, a)), 0x3800_0000))
    }

    /// [`Rng::randf_bits`] on `f32` values.
    pub fn randf_exact(&mut self, a: f32, b: f32) -> f32 { f32::from_bits(self.randf_bits(a.to_bits(), b.to_bits())) }

    /// `rand_angle()` on PS2 float bits: radians in [−π, π).
    pub fn rand_angle_bits(&mut self) -> u32 {
        use crate::ps2v;
        let k = ((self.rand() >> 16) & 0xfff) as i32 - 0x800;
        ps2v::mul(ps2v::mul(ps2v::itof0(k), 0x4049_0fdb), 0x3a00_0000)
    }
}
