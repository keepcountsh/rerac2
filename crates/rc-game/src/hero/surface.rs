//! The surface under the feet (L00 `0x2aca80`, the update's step after the move): the flags of the surface id the
//! ground probe left (`0x18b2b0`), and the states they force — the lava hop 0x38 (surface 1; with no health the
//! deaths), the slide 0x30 (4), the sinking 0x7c (0xd, below the liquid's surface `0x18a150`), the quicksand 0x65 /
//! 0x78 (3, below `0x18a154`), the pit 0x78 / 0x74 (0xb).
//!
//! Surface ids: 0 / 0xe water (0xe: a puddle 0.2 deep), 1 lava, 2 a magnetic walkway (with the Gravity Boots
//! `0x1a7b13`: `0x18a168` = 1), 3 quicksand, 4 a slide, 7 ice, 8 / 0xc mud (`0x18b2ba`), 0xb a pit, 0xd a deep
//! liquid.
//!
//! Not ported: the liquid heights `0x18a150` / `0x18a154` come from the water probe (with the swim states), so 0x7c,
//! 0x65 and the pit stay off until it is; the vehicle body's lava 0x7a (`0x18c0d4` = 1).

use super::physics::*;
use super::Hero;
use crate::hero::anim::AnimCtl;

/// The flags (`0x18b2b0..0x18b2bd`, cleared every tick) and the liquid heights.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceBlock {
    /// `0x18b2b4` water, `0x18b2b2` ice, `0x18b2b5` lava, `0x18b2b6` quicksand, `0x18b2bb` a pit, `0x18b2b3` a slide,
    /// `0x18b2bc` a deep liquid, `0x18b2ba` mud, `0x18b2b7` magnetic.
    pub water: bool,
    pub ice: bool,
    pub hot: bool,
    pub sand: bool,
    pub pit: bool,
    pub slide: bool,
    pub deep: bool,
    pub mud: bool,
    pub magnet: bool,
    /// `0x18a168`: on a magnetic walkway with the Gravity Boots.
    pub boots: u8,
    /// `0x18c0d9`: wading (water 0.25..0.85 deep).
    pub wading: bool,
    /// `0x18a150` / `0x18a154`: the liquid's and the quicksand's surface (−∞: none); `0x18c2e4`: the water's depth.
    pub water_z: f32,
    pub sand_z: f32,
    pub depth: f32,
}

impl Default for SurfaceBlock {
    fn default() -> Self {
        SurfaceBlock {
            water: false, ice: false, hot: false, sand: false, pit: false, slide: false, deep: false, mud: false, magnet: false,
            boots: 0, wading: false, water_z: f32::NEG_INFINITY, sand_z: f32::NEG_INFINITY, depth: 0.0,
        }
    }
}

impl Hero {
    /// `0x2aca80`. Returns whether it set a state.
    pub fn surface_check(&mut self, env: &Env, anim: &mut dyn AnimCtl) -> bool {
        let (water_z, sand_z) = (self.surf.water_z, self.surf.sand_z);
        self.surf = SurfaceBlock { water_z, sand_z, ..Default::default() };
        let id = self.surface;
        let on = self.grounded_ticks != 0 || (self.state == 0x7b && self.height < 0.4);
        if id == -1 { return false; }
        let s = &mut self.surf;
        match id {
            2 => {
                s.magnet = true;
                if self.land_lag == 0 && (self.f16c == 0 || self.height < 2.5) && self.mode == 0 && env.boots { s.boots = 1; }
            }
            0xe => {
                s.water = true;
                s.water_z = self.ground_z + 0.2;
                s.depth = 0.2;
            }
            0 => {
                s.depth = s.water_z - self.ground_z;
                if s.depth < 0.85 && 0.25 < s.depth { s.wading = true; }
                s.water = true;
            }
            3 => s.sand = true,
            0xb => s.pit = true,
            4 => s.slide = true,
            7 => s.ice = true,
            0xd => s.deep = true,
            1 => s.hot = true,
            8 | 0xc => s.mud = true,
            _ => {}
        }
        let s = self.surf;
        let mut next = None;
        if s.slide && on && !matches!(self.group, 0x10 | 0x14 | 7) {
            next = Some(0x30);
        } else if s.hot && on && (self.state != 0x38 || self.jump.descending != 0) {
            next = Some(if self.hp == 0 { 0x39 } else { 0x38 });
        } else if s.deep && self.state != 0x7c && (s.water_z - (self.pos[2] + 0.25)).abs() < 1.0 && 0.0 < s.water_z - self.pos[2] && (self.disp[2] < 0.0 || self.state == 0x7b) {
            next = Some(0x7c);
        } else if s.sand && !matches!(self.state, 0x65 | 0x78) && (s.sand_z - (self.pos[2] + 0.25)).abs() < 1.0 && 0.0 < s.sand_z - self.pos[2]
            && !(self.state == 0x66 && self.jump.descending == 0) && (self.disp[2] < 0.0 || self.state == 0x7b)
        {
            next = Some(if self.hp != 0 { 0x65 } else { 0x78 });
        } else if s.pit && !matches!(self.state, 0x78 | 0x74) && (s.sand_z - (self.pos[2] + 0.25)).abs() < 1.0 && 0.0 < s.sand_z - self.pos[2] && (self.disp[2] < 0.0 || self.state == 0x7b) {
            next = Some(0x78);
        }
        match next {
            Some(n) => self.set_state(anim, env, n, true),
            None => false,
        }
    }
}
