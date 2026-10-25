//! The blasts (L00 `0x3f8348`): what a moby leaves when it blows up — the enemies' deaths, the carriers' wrecks. The
//! game creates a moby of class 0x13a0 for one (its pvars hold the parameters; it lives 60 ticks, `0x3f8688`) and
//! bursts it at once (`0x3f8a70`, [`burst`]), all relative to the size `s`:
//!
//! * two flash sprites (`0x33ef78`, 3·s → 4·s across, alpha 100 → 0 over 30 ticks; colours 0x388ef7 and 0x63f7) and
//!   two pairs more (alpha 0x7f, the sprite list's frames 5 and 6), the frames of type 0x5c's list;
//! * three sets of blast sprites (crate::particles::burst, [`Blast::bursts`]) thrown toward the camera's side within
//!   80°, 9..12·s a second across and 0..12·s up, 30..50 ticks: orange (0x407fff, frame 1, with a wide 0x20ff halo on
//!   type 0x35's frame), yellow (0x8fff, frame 5), blue (0x3064ff, frame 3);
//! * streaks (crate::particles::streak, [`Blast::streaks`]) in a cone of 15°..80° about the line to the camera,
//!   15..20·s a second, 20..35 ticks;
//! * puffs (crate::particles::puff, [`Blast::puffs`]): lilac (0x7f60a0ff) glows drifting out 1/60..2/60·s a tick, 40..70
//!   ticks.
//!
//! * the flash ([`Blast::light`], `+0x34`): the red (0xff, 0, 0) flash ball of class 0x70 (crate::moby::classes::flash,
//!   `0x38e5a8`) at the centre, 3.5·s the class's scale across in full, alpha 0x28, 16 ticks — spawned between the
//!   streaks and the puffs (its three random draws sit there);
//! * the camera shake ([`Blast::shake`]): when the blast is in view (`0x2f0080(128, its sphere of 3·s)`) within 48 of
//!   the camera, the up-and-down shake (crate::camera::Shakes, slot 0) of 0.075 over 40 ticks.
//!
//! Every Aranos blast asks for the flash and none for the shake (0x099c, 0x0b0a, 0x0b0b, 0x09c7). `k` (20 at every
//! caller) only reaches the flash's unread pvar and `0x320538`.
//!
//! Not ported: the 0x13a0 moby's draws — the heat shimmer `0x3f9720` (`+0x18`, a pass that samples the frame) and the
//! screen flash (`+0x38`) — and its danger area (`+0x3c`, `0x322058`: it also gives the velocity `0x3f8a70` adds to the
//! sprites' and the flash's vector; the port adds nothing, as with no danger area). The generic blast `0x320538` it
//! calls does nothing with these arguments (no damage, no debris, no flare while the frame rate is full).

use super::{Ctx, MobyWorld};
use crate::hero::physics::{add_rot, atan2, cross, dist, scale, sub, unit, V3};
use crate::particles::{self, rec};

/// The parameters of `0x3f8348(s, k, pos, shake, a, b, c, puffs, streaks, ring, light, _, flash, danger)` this port uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blast {
    /// `s`: everything scales with it.
    pub size: f32,
    /// The orange, yellow and blue blast sprites (`+0x20`, `+0x24`, `+0x28`).
    pub bursts: [i32; 3],
    /// `+0x2c`.
    pub puffs: i32,
    /// `+0x30`.
    pub streaks: i32,
    /// `+0x34`: the flash ball.
    pub light: bool,
    /// `shake`: the camera shake near the camera.
    pub shake: bool,
}

/// `k` at every caller: the flash's unread `+0x10`.
const K: f32 = 20.0;
/// The flash: `0x38e5a8(3.5·s, k, pos, (0, 0, 0.1333·s) + the danger area's velocity, 16, 0xff, 0, 0, 0x28)`.
const FLASH_LIFE: i32 = 0x10;
const FLASH_ALPHA: u8 = 0x28;

/// `0x31fe98` + `0x321b28`: `v` turned by `a` about the unit axis `n`.
fn turn(v: V3, n: V3, a: f32) -> V3 {
    let (c, s) = (a.cos(), a.sin());
    let d = n[0] * v[0] + n[1] * v[1] + n[2] * v[2];
    let x = cross(n, v);
    std::array::from_fn(|k| v[k] * c + x[k] * s + n[k] * d * (1.0 - c))
}

/// `0x3f8348` → `0x3f8a70`: a blast at `at` (crate docs).
pub fn burst(w: &mut MobyWorld, ctx: &mut Ctx, at: V3, b: &Blast) {
    let cam = ctx.host.camera_pos();
    let s = b.size;
    // `0x3f8348`: in view (`0x2f0080(128, (at, 3·s))`) within 48 of the camera (`0x2ef018`), the shake when asked.
    if b.shake && !ctx.view.is_some_and(|v| v.culled(128.0, [at[0], at[1], at[2], s * 3.0])) && dist(at, cam) < 48.0 {
        let sh = &mut ctx.game.shakes.0[0];
        (sh.amp, sh.timer, sh.max) = (0.075, 0x28, 0);
    }
    let yaw = atan2(cam[0] - at[0], cam[1] - at[1]);
    // The 0x13a0 moby's rows (`0x3f8348`): toward the camera, then across and up.
    let r0 = unit(sub(cam, at));
    let r1 = unit(cross(r0, [0.0, 0.0, 1.0]));
    if let Some(sys) = ctx.parts.as_deref_mut() {
        let g = &mut ctx.game.rng;
        let (big, small) = (4.0 * s * 210_000.0, 3.0 * s * 210_000.0);
        for (rgb, alpha, frame, n) in [(0x38_8ef7, 100, 0, 1), (0x63f7, 100, 0, 1), (0x38_8ef7, 0x7f, 5, 2), (0x38_8ef7, 0x7f, 6, 2)] {
            for _ in 0..n {
                if let Some(i) = particles::sprite(sys, g, small, big, 0.0, at, [0.0; 3], 30, rgb, 0, alpha, 0, 0) {
                    sys.pool.recs[i][2] = sys.def_frame(0x5c, frame);
                }
            }
        }
        let sets = [(b.bursts[0], 0.6, 1.0, 0x40_7fff, 1), (b.bursts[1], 0.75, 1.5, 0x8fff, 5), (b.bursts[2], 0.6, 1.0, 0x30_64ff, 3)];
        for (k, &(n, lo, hi, rgb, frame)) in sets.iter().enumerate() {
            for _ in 0..n {
                let spin = g.randf_signed(2.0, 5.0) as i32;
                let life = g.rand_range(30, 50) as i16;
                let p: V3 = std::array::from_fn(|k| at[k] + g.randf_signed(0.0, 0.1) * s);
                let size = g.randf(lo, hi) * s;
                let speed = g.randf(9.0, 12.0) * s;
                let a = add_rot(yaw, g.randf_signed(0.0, 1.396_263_4));
                let up = g.randf(0.0, 12.0) * s / 60.0;
                let v = [a.cos() / 60.0 * speed, a.sin() / 60.0 * speed, up];
                particles::burst(sys, g, size * 210_000.0, s * 2100.0, 0.004_05, p, v, life, rgb, spin as u8, 0x7f, 0x7f, frame);
                if k == 0 {
                    if let Some(i) = particles::burst(sys, g, size * 1_260_000.0, s * 4200.0, 0.004_05, p, v, life, 0x20ff, (spin * -2) as u8, 0x30, 0x30, 0) {
                        sys.pool.recs[i][2] = sys.def_first(0x35);
                    }
                }
            }
        }
        for _ in 0..b.streaks {
            let v = scale(r0, g.randf(15.0, 20.0) * s / 60.0);
            let (a1, a2) = (g.randf_signed(0.261_799_4, 1.396_263_4), g.randf_signed(0.261_799_4, 1.396_263_4));
            let v = turn(turn(v, r1, a1), r0, a2);
            let life = g.rand_range(20, 35) as i16;
            particles::streak(sys, 0.004_05, at, v, life);
        }
    }
    // The flash, between the streaks and the puffs.
    if b.light {
        super::classes::flash::spawn(w, ctx, s * 3.5, K, at, [0.0, 0.0, s * 0.133_333_34], FLASH_LIFE, [0xff, 0, 0], FLASH_ALPHA);
    }
    let Some(sys) = ctx.parts.as_deref_mut() else { return };
    let g = &mut ctx.game.rng;
    let mut side = 1;
    for _ in 0..b.puffs {
        side = -side;
        let p: V3 = std::array::from_fn(|k| at[k] + g.randf_signed(0.0, 1.0) * s);
        let r = g.randf(0.016_666_668, 0.033_333_335) * s;
        let pitch = g.randf_signed(0.0, 1.396_263_4);
        let a = add_rot(yaw, g.randf_signed(0.0, 1.570_796_4));
        // `0x325100(r, a, pitch)`.
        let v = [a.cos() * r * pitch.cos(), a.sin() * r * pitch.cos(), pitch.sin() * r];
        if let Some(i) = particles::puff(sys, g, 0.0, 1.0, 1.01, s * 315_000.0, p, side, v, 0x7f60_a0ff, 0) {
            let life = g.rand_range(40, 70) as i16;
            let t = g.rand_range(1, 10) as f32;
            let r = &mut sys.pool.recs[i];
            rec::set_i16(r, 0xa, life);
            r[9] = 4 + 0x40;
            rec::set_u32(r, 0x24, 3);
            rec::set_ff(r, 0x3c, t);
            r[0x2a] = 0x40;
            r[0x2b] = r[0xa];
        }
    }
}
