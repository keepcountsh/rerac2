//! The walls ahead: the wall-jump probe (L00 `0x2ac5f0`, the end of the jump and glide physics) arms the wall jump
//! 0x11 for 6 ticks (7 from a wall jump) when a tall, near-vertical wall faces Ratchet within reach and no ledge
//! will be in reach 20 ticks on; the ahead probe (`0x2c3068`, every third frame after the move) measures the wall
//! straight ahead for the pack jumps and the wall jump.
//!
//! **The wall jump's wall.** In the air at least 0.4 above the ground, not falling faster than 1/3: a line 0.3 up
//! from the capsule's edge to 1 ahead hits; a line 3 up (1.7 from a wall jump or a moby that gives way) from the
//! centre to the radius + 1.4 ahead hits a wall (not surface 10 / 0xc) at least 75° steep whose normal points back
//! at him within 50°; a line 0.2 up hits within 0.72 (0.95) of its start. Then, in or just after a jump (group 4
//! now or in the previous two states), and either the first wall jump or a wall turned at least 170° from the last
//! one, the wall's normal and yaw are kept. A wall of surface 0xf allows wall jumps in a row; any other only one
//! (`0x18c2fb` counts them).
//!
//! Not ported: the moby walls' "gives way" flag (`+0x3c` & 0x20), the strafe camera mode (`0x18c0e3`), the ahead
//! probe's edge test (`0x18a0b0`, for the teeter `0x2ab498`).

use super::physics::*;
use super::Hero;
use rc_game::collision_query::CollOutput;

/// The wall-jump block.
#[derive(Clone, Copy, Debug, Default)]
pub struct WallBlock {
    /// `0x189ff4`: the ticks the wall jump stays armed (counted down by the post-move).
    pub timer: i32,
    /// `0x18a300` / `0x18a30c`: the wall's normal and its yaw.
    pub normal: V3,
    pub yaw: f32,
    /// `0x18c2fa`: the wall is surface 0xf (the high wall jump, any number in a row); `0x18c2fb`: wall jumps in a row
    /// off other walls; `0x18c2f8`: a wall jump is in progress (the next wall must face away from this one).
    pub high: bool,
    pub count: u8,
    pub lock: u8,
    /// `0x18a030`: a ledge will be in reach (the wall jump is not armed).
    pub regrab: i16,
}

/// The ahead probe's results.
#[derive(Clone, Copy, Debug)]
pub struct AheadBlock {
    /// `0x18a098`: the distance to the wall straight ahead 0.7 up (4: none); `0x18a0a0` its slope; `0x18a0a5` it is
    /// a moby, `0x18a0a4` a moby that blocks (`0x3204e8`).
    pub dist: f32,
    pub slope: f32,
    pub moby: bool,
    pub solid: bool,
}

impl Default for AheadBlock {
    fn default() -> Self { AheadBlock { dist: 4.0, slope: 0.0, moby: false, solid: false } }
}

impl Hero {
    /// `0x2d39d8(d, 0, z, out, 1)`: the point `d` ahead along the facing, `z` up the body, from the feet.
    pub fn local_point(&self, x: f32, y: f32, z: f32) -> V3 {
        let r = &self.rows;
        [
            self.pos[0] + r[0][0] * x + r[1][0] * y + r[2][0] * z,
            self.pos[1] + r[0][1] * x + r[1][1] * y + r[2][1] * z,
            self.pos[2] + r[0][2] * x + r[1][2] * y + r[2][2] * z,
        ]
    }

    /// `0x2ab180(z, d, out)`: a line `z` up from the capsule's edge (radius − 0.02) to `d` ahead (flags 2); the hit and
    /// its distance from the start.
    pub fn ahead_line(&self, env: &Env, z: f32, d: f32) -> Option<(CollOutput, f32)> {
        let a = self.local_point(self.cap_radius - 0.02, 0.0, z);
        let b = self.local_point(d, 0.0, z);
        env.line(a, b, 2).map(|o| { let k = dist(a, o.point); (o, k) })
    }

    /// `0x2c3068`'s wall half (every third frame).
    pub fn ahead_probe(&mut self, env: &Env) {
        if self.frame % 3 != 0 { return; }
        self.ahead = AheadBlock::default();
        if let Some((o, d)) = self.ahead_line(env, 0.7, 4.0) {
            self.ahead.dist = d;
            self.ahead.slope = slope_of(o.normal);
            if o.moby.is_some() {
                self.ahead.moby = true;
                self.ahead.solid = true;
            }
        }
    }

    /// `0x2ac5f0`: arms the wall jump (module docs).
    pub fn wall_probe(&mut self, env: &Env) {
        if self.height < 0.4 || self.vel[2] < -1.0 / 3.0 { return; }
        if self.ledge.found {
            self.wall.timer = 0;
            return;
        }
        // Where he will be in 20 ticks: a ledge in reach there, no wall jump.
        let g = self.jump.g;
        let mut at = add(self.pos, [self.disp[0] * 20.0, self.disp[1] * 20.0, 0.0]);
        at[2] += self.vel[2] * 20.0 - g * 20.0 * 20.0 * 0.5;
        if self.ledge_probe(env, at, false).is_some() {
            self.wall.timer = 0;
            self.wall.regrab = 0x16;
            return;
        }
        if self.ahead_line(env, 0.3, 1.0).is_none() { return; }
        let z = if self.state == 0x11 || (self.ahead.moby && !self.ahead.solid) { 1.7 } else { 3.0 };
        let a = self.local_point(0.0, 0.0, z);
        let b = self.local_point(self.cap_radius + 1.4, 0.0, z);
        let Some(o) = env.line(a, b, 2) else { return };
        let surf = o.surface_id();
        if surf == 10 || surf == 0xc { return; }
        if slope_of(o.normal) < 1.308_997 { return; }
        let n = unit(o.normal);
        // The normal in Ratchet's frame: pointing back at him within 50°.
        let lx = dot(n, self.rows[0]);
        let ly = dot(n, self.rows[1]);
        if 0.872_664_7 < rot_dist(atan2(lx, ly), PI) { return; }
        let reach = if self.state == 0x11 { 0.95 } else { 0.72 };
        let a = self.local_point(0.0, 0.0, 0.2);
        let b = self.local_point(self.cap_radius + 1.4, 0.0, 0.2);
        let Some(lo) = env.line(a, b, 2) else { return };
        if reach < dist2(lo.point, a) { return; }
        let yaw = atan2(n[0], n[1]);
        self.wall.normal = n;
        let near_jump = self.group == 4 || self.prev_group == 4 || self.prev2_group == 4;
        let turned = self.wall.lock == 0 || 2.967_06 <= rot_dist(self.jump.wall_yaw, yaw);
        self.wall.yaw = yaw;
        if !(near_jump && turned) { return; }
        self.wall.high = surf == 0xf;
        if !self.wall.high && self.wall.count != 0 { return; }
        self.wall.timer = if self.state == 0x11 { 7 } else { 6 };
    }
}
