//! The game-wide state the level code reads and writes (L00 / the boot's .bss; names from the addresses' readers):
//! currency, health limits and the per-level tallies. What a save holds is milestone I (docs/plan/rac2_bringup.md).

/// The player's progress fields the gameplay code touches.
#[derive(Clone, Debug)]
pub struct GameState {
    /// `0x1a7a00`: bolts (u32; `CollectBolts`' add path).
    pub bolts: u32,
    /// `0x1a7a04`: raritanium.
    pub raritanium: u32,
    /// `0x1a7a08`: the health limit (the heal clamp and the HUD's bar).
    pub max_health: i32,
    /// `0x1a79f8`: the level (planet) index.
    pub level: i32,
    /// `0x1a7a1c`: the bolt bank: value a drop could not put into bolt mobys (the limit per drop, bolts that fell
    /// far from the camera); each later drop takes (bank + 49) / 50 of it.
    pub bolt_bank: i32,
    /// `0x1a7a18` / `0x1a7a14`: the XP bank and the XP total.
    pub xp_bank: i32,
    pub xp: i32,
    /// The per-level save bytes of the uids (`0x19b4a8 + level·0x400`, 3 bits per uid: low nibble even uids, high
    /// odd): how often a crate was broken (the bolt percentage table's row) and bit 3 (the second table).
    pub uid_bits: Vec<u8>,
    /// This tick's HUD requests: `0x30d910(n)` (the bolt counter shows for n ticks) and `0x30d9f8(n)` (the health).
    pub show_bolts: Option<i32>,
    pub show_health: Option<i32>,
    /// The random stream (`0x1163b0`).
    pub rng: crate::rng::Rng,
    /// Recent crate breaks (`$gp+0x3fc`, decays): fewer bolt mobys a drop the more.
    pub recent_breaks: i32,
    /// `$gp+0x3f8` / the frame counter `0x1b1948` of the last break.
    pub last_break_frame: i32,
    pub frame: i32,
    /// `0x1a7b02`: Clank rides on Ratchet's back (not yet on Aranos's opening); `0x1a7a11`: the Thruster-Pack (else the
    /// Heli-Pack); `0x1a7b13`: the Gravity Boots.
    pub clank: bool,
    pub thruster: bool,
    pub boots: bool,
    /// `0x1a7b36` with the item `0x1a739c` = 0x36: the Charge Boots on.
    pub charge_boots: bool,
    /// `0x1a7b14`: the Grind Boots.
    pub grind_boots: bool,
    /// `0x1a7a30..0x1a7a3a`: the platinum bolts' nanotech upgrades (+1 health each); `0x18c350` / `0x18c34c`: the
    /// XP of the health level reached and of the next.
    pub nanotech: [bool; 10],
    pub level_xp: [i32; 2],
    /// `0x1b1a68`: the death height (the level's setting; objects change it).
    pub death_z: Option<f32>,
    /// `0x19b2e8 + level·0x10`: the missions' progress, two bits per sub-mission (`mission + level·0x10`, bits
    /// `2·(sub & 3)`); `0x18c0e6`: the mission whose area Ratchet is in.
    pub missions: Vec<u8>,
    pub mission_area: u8,
    /// The camera-tweak requests (`0x1bb970`).
    pub cam_tweaks: crate::camera::CamTweaks,
    /// Per uid: `0x21d554` what happened since the checkpoint (the mission + 2), `0x21e1c4` gone for good (a respawn
    /// leaves it out); `0x1b1c60` the missions' table (bit 7: running); `0x1c5400` / `0x1c54c0` the missions' banked XP
    /// and bolts (mission·3 + sub).
    pub uid_pending: Vec<u8>,
    pub uid_dead: Vec<u8>,
    pub mission_table: Vec<u8>,
    pub mission_xp: Vec<i32>,
    pub mission_bolts: Vec<i32>,
    /// The checkpoint (`0x21dd70` position, `0x21dd80` angles), from the spawn points.
    pub checkpoint: Option<([f32; 3], [f32; 3])>,
    /// `0x21ddc0`: the level's uid bits as the checkpoint kept them (a respawn puts them back).
    pub checkpoint_bits: Vec<u8>,
    /// The checkpoint message's ticks to show (`0x327448(0x2dcc, 0x5a)`) and the spawn point that showed it last
    /// (`$gp + 0x400`).
    pub checkpoint_message: Option<i32>,
    pub last_checkpoint: Option<usize>,
    /// The help panel (`0x27d7c8` its state, 0 none; `0x27d7ec` / `0x27d7f0` the text asked for and its slot) and
    /// the slots' counts (`0x18c958 + 0xc·slot`: the times △ dismissed it; −1 off).
    pub help_state: i32,
    pub help_req: Option<(i32, i32)>,
    pub help_counts: Vec<i16>,
    /// `0x18c310`: Ratchet in one of the tutorial's no-item areas.
    pub no_item_area: bool,
    /// `0x1c92c0`: the level's variant (the scenes and the level code set it; 0 on a load).
    pub variant: i32,
}

/// `0x2621e0`: the health levels, the XP / 32 to the next (each gives one health), −1 ends.
const HP_LEVELS: [i16; 66] = [
    0, 180, 210, 240, 270, 300, 330, 360, 390, 420, 450, 480, 500, 520, 520, 520, 520, 520, 520, 520, 520, 520, 530, 530, 560,
    580, 600, 620, 640, 660, 680, 700, 720, 740, 760, 780, 800, 840, 810, 945, 1080, 1215, 1350, 1485, 1620, 1755, 1890, 2025,
    2160, 2250, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2340, 2385, 2475, 2520, 2610, 2700, 2790, 2880,
];

impl Default for GameState {
    fn default() -> Self {
        GameState {
            bolts: 0, raritanium: 0, max_health: 4, level: 0, bolt_bank: 0, xp_bank: 0, xp: 0, uid_bits: vec![0; 0x400],
            show_bolts: None, show_health: None, rng: Default::default(), recent_breaks: 0, last_break_frame: -1000, frame: 0,
            clank: false, thruster: false, boots: false, charge_boots: false, grind_boots: false, nanotech: [false; 10], level_xp: [0; 2], death_z: None, missions: vec![0; 0x200], mission_area: 0, cam_tweaks: Default::default(), uid_pending: vec![0; 0x800], uid_dead: vec![0; 0x800], mission_table: vec![0; 0x100],
            mission_xp: vec![0; 0x30], mission_bolts: vec![0; 0x30], checkpoint: None, checkpoint_bits: Vec::new(), checkpoint_message: None, last_checkpoint: None, help_state: 0, help_req: None, help_counts: vec![0; 0x40], no_item_area: false, variant: 0,
        }
    }
}

impl GameState {
    /// `0x2f5270(text, slot)`: ask for a help text — only with no panel up or asked for, the slot not off, past the
    /// sixth frame.
    pub fn help(&mut self, text: i32, slot: i32) -> bool {
        if self.help_state != 0 || self.help_req.is_some() || self.frame <= 5 { return false; }
        if self.help_counts.get(slot.max(0) as usize).copied().unwrap_or(-1) == -1 { return false; }
        self.help_req = Some((text, slot));
        true
    }

    /// A help slot's count.
    pub fn help_count(&self, slot: usize) -> i16 { self.help_counts.get(slot).copied().unwrap_or(0) }

    /// The two progress bits of a mission's sub-mission on this level.
    pub fn mission(&self, mission: u8, sub: u8) -> u8 {
        let b = self.missions.get(self.level.max(0) as usize * 0x10 + mission as usize).copied().unwrap_or(0);
        (b >> ((sub & 0xf) * 2)) & 3
    }

    pub fn set_mission(&mut self, mission: u8, sub: u8, v: u8) {
        let i = self.level.max(0) as usize * 0x10 + (mission & 0xf) as usize;
        if let Some(b) = self.missions.get_mut(i) {
            let k = (sub & 3) * 2;
            *b = (*b & !(3 << k)) | ((v & 3) << k);
        }
    }

    /// `0x2d61d0`: the health the XP gives (4 + one a level of `HP_LEVELS`), and the XP of the level reached and of
    /// the next.
    pub fn xp_health(&self) -> (i32, [i32; 2]) {
        let x = self.xp >> 5;
        let (mut hp, mut cum, mut last) = (4, 0i32, 0i32);
        let mut i = 0;
        while cum <= x {
            if 0 < i { hp += 1; }
            last = cum;
            i += 1;
            if 0x4c < i || HP_LEVELS.len() <= i { break; }
            cum += HP_LEVELS[i] as i32;
        }
        (hp, [last, cum])
    }

    /// `0x2d6258`'s health part (every 16th frame and the first): the health limit = the XP's + the nanotech
    /// upgrades; a change fills the health (a rise past frame 10 shows the bar: `show_health`).
    pub fn health_tick(&mut self, hp: &mut i32) {
        if self.frame & 0xf == 1 && self.frame != 1 { return; }
        let (h, lv) = self.xp_health();
        self.level_xp = lv;
        let max = h + self.nanotech.iter().filter(|&&n| n).count() as i32;
        if max != self.max_health {
            if 10 < self.frame && self.max_health < max { self.show_health = Some(0x168); }
            self.max_health = max;
            *hp = max;
        }
    }

    /// `0x2adf18(3)`: the back slot's item (`0x2afc68`: with Clank the Heli-Pack 2, or 3 with the Thruster-Pack).
    pub fn pack(&self) -> i32 { if self.clank { if self.thruster { 3 } else { 2 } } else { -1 } }

    /// The 3-bit count of uid `uid` (`0x2e4670`'s read).
    pub fn uid_count(&self, uid: i16) -> u8 {
        let b = self.uid_bits.get((uid as i32 >> 1).max(0) as usize).copied().unwrap_or(0);
        if uid & 1 != 0 { (b >> 4) & 7 } else { b & 7 }
    }

    /// Bit 3 of uid `uid`'s nibble.
    pub fn uid_flag(&self, uid: i16) -> bool {
        let b = self.uid_bits.get((uid as i32 >> 1).max(0) as usize).copied().unwrap_or(0);
        (if uid & 1 != 0 { b >> 4 } else { b }) & 8 != 0
    }

    /// The count cleared (bit 3 kept).
    pub fn clear_uid(&mut self, uid: i16) {
        let Some(b) = self.uid_bits.get_mut((uid as i32 >> 1).max(0) as usize) else { return };
        if uid & 1 != 0 { *b &= 0x8f } else { *b &= 0xf8 }
    }

    /// Bit 3 set, the count cleared.
    pub fn set_uid_flag(&mut self, uid: i16) {
        let Some(b) = self.uid_bits.get_mut((uid as i32 >> 1).max(0) as usize) else { return };
        if uid & 1 != 0 { *b = (*b | 0x80) & 0x8f } else { *b = (*b | 8) & 0xf8 }
    }

    /// An object gone (`0x31adc0`, `0x381e60`): marked since the checkpoint (the mission + 2), and for good when
    /// its mission is none or running and not the table's.
    pub fn uid_gone(&mut self, uid: i16, mission: u8) {
        let u = uid.max(0) as usize;
        if let Some(p) = self.uid_pending.get_mut(u) { *p = mission.wrapping_add(2); }
        let lv = self.missions.get(self.level.max(0) as usize * 0x10 + mission as usize).copied().unwrap_or(0);
        if mission == 0xff || (self.mission_table.get(mission as usize).copied().unwrap_or(0) & 0x80 == 0 && lv & 0x80 != 0) {
            if let Some(d) = self.uid_dead.get_mut(u) { *d = mission.wrapping_add(2); }
        }
    }

    /// `0x359b20`: the checkpoint — the position and angles kept, and what happened since the last one made
    /// permanent (a pending uid of no mission or a running one: gone for good, its save count + 1).
    pub fn save_checkpoint(&mut self, pos: [f32; 3], rot: [f32; 3]) {
        for u in 0..self.uid_pending.len().min(0x7ff) {
            let v = self.uid_pending[u];
            if v == 0 { continue; }
            let lv = self.missions.get(self.level.max(0) as usize * 0x10 + (v as usize).wrapping_sub(2) % 0x10).copied().unwrap_or(0);
            if v == 1 || lv & 0x80 != 0 {
                self.uid_dead[u] = v;
                self.bump_uid(u as i16);
            }
        }
        self.checkpoint = Some((pos, rot));
        self.checkpoint_bits = self.uid_bits.clone();
    }

    /// The level restarted from the checkpoint (`0x2b31f0`'s request): the uid bits as it kept them, nothing pending.
    pub fn restart(&mut self) {
        if !self.checkpoint_bits.is_empty() { self.uid_bits = self.checkpoint_bits.clone(); }
        self.uid_pending.iter_mut().for_each(|p| *p = 0);
    }

    /// The count + 1 (saturating at 7).
    pub fn bump_uid(&mut self, uid: i16) {
        let Some(b) = self.uid_bits.get_mut((uid as i32 >> 1).max(0) as usize) else { return };
        if uid & 1 != 0 { if (*b >> 4) & 7 != 7 { *b += 0x10; } } else if *b & 7 != 7 { *b += 1; }
    }
}
