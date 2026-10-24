//! The game-wide state the level code reads and writes (L00 / the boot's .bss; names from the addresses' readers):
//! currency, health limits and the per-level tallies. What a save holds is milestone I (docs/plan/bringup.md).

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
    /// `0x1b19d0`: the occlusion's mode (`0x2e54a0`): 0 everything visible, 2 the grid (`0x2e4d88`), else the mask
    /// kept. The level's object loader `0x3012a0` sets 0 (the default here, every level entry); Aranos's arrival
    /// sets 2 once its fly-by is over (`0x3846d0`).
    pub occlusion: i32,
    /// `0x1b2794`: the frame a crate last played its break sound.
    pub crate_sound_frame: i32,
    /// `$gp+0x3a6c`: the frame an ammo pickup last played its sound (crate::moby::classes::pickup).
    pub pickup_sound_frame: i32,
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
    /// `0x1a7bc8`: the play time in frames (`0x3596d0`: one a game frame; the save slots show it as hh:mm).
    pub play_time: i32,
    /// `0x1a7bf0`: per level, visited (the level start sets 1; the save page's load starts a new game from a save of
    /// level 0 not visited yet).
    pub visited: Vec<u8>,
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
    /// The boss health bar (`0x2f20b8(fraction, …)`: `$gp−0x5f2c` the fraction, `$gp−0x5f28` 5 ticks left, refreshed each
    /// tick of a fight; the bar's draw `0x2f2158` is not ported).
    pub boss_bar: Option<(f32, i32)>,
    pub death_z: Option<f32>,
    /// Oozla's water (L01 `$gp+0xdec` / `$gp+0xde8`, kept by its class 0x0a17): the level of the water and the
    /// height the floating objects read (`0x3bc3b8`: the level and a 0.1 wave; 0 until the class runs).
    pub water_base: f32,
    pub water_z: f32,
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
    /// The spawn point that showed the checkpoint message last (`$gp + 0x400`).
    pub last_checkpoint: Option<usize>,
    /// The message boxes' requests this tick (crate::msgbox).
    pub messages: Vec<crate::msgbox::Request>,
    /// The help panel (`0x27d7c8` its state, 0 none; `0x27d7ec` / `0x27d7f0` the text asked for and its slot) and
    /// the slots' counts (`0x18c958 + 0xc·slot`: the times △ dismissed it; −1 off).
    pub help_state: i32,
    pub help_req: Option<(i32, i32)>,
    pub help_counts: Vec<i16>,
    /// `0x18c310`: Ratchet in one of the tutorial's no-item areas.
    pub no_item_area: bool,
    /// `0x1c92c0`: the level's variant (the scenes and the level code set it; 0 on a load).
    pub variant: i32,
    /// `0x1a7b00`: the items had (one byte each); `0x139948`: the list `0x322730` keeps (28 slots, 0 free).
    pub items: Vec<u8>,
    pub quick_select: Vec<i32>,
    /// `0x1a73b8`: the quick select's eight slots (their items, 0 empty; crate::quick_select).
    pub ring: [i32; 8],
    /// `0x139688 + 4·item`: the ammo (items whose row has ammo, crate::hero::items::ItemRow).
    pub ammo: Vec<i32>,
    /// `0x1a7398`: the last weapon held (the hand item ○ brings back), `0x1a7a10`: the wrench was out.
    pub held: i32,
    pub wrench_out: bool,
    /// `$gp+0x1574`: the shot trails' texture phase (crate::moby::classes::shot::draw_trail).
    pub trail_phase: f32,
    /// `0x2e5df8`'s view distance (units; 1024 from the level load, 1024..4096): the projection's far plane (`0x1bd2e4`
    /// = it × 1024) and the mobys' draw cap (`0x1b1e34` = it − 64). The mountains 0x0c4c ask for 3000.
    pub view_far: f32,
    /// The sky's turning shells (crate::sky).
    pub sky: crate::sky::SkySpin,
    /// `0x1bb000` (count `0x1bb120`): the screen markers of the tick (crate::hud::marker), drawn and cleared by the HUD.
    pub markers: Vec<crate::hud::Marker>,
    /// `0x139768`: the scenes seen (one bit each).
    pub scenes_seen: Vec<bool>,
    /// The scene queue, the modes and the scene playing (crate::scene).
    pub scenes: crate::scene::Scenes,
    /// The level camera system (crate::cameras).
    pub cameras: crate::cameras::Cameras,
    /// `0x220d50`: the ship's parts (`0x360d28`).
    pub ship_parts: Vec<usize>,
    /// The trigger areas' occupants (`0x1b1a7c`, filled by `0x2ae140` for the areas asked about in the last 10
    /// frames, the area record's `+0x3a`): only Ratchet is kept (the helpers 0xcb / 0xeb0 and the decoys are not on
    /// Aranos).
    pub area_ask: Vec<i16>,
    pub area_hero: Vec<bool>,
    /// `0x2f50f8`'s request: the level to go to (`0x1b1940`; −1 the front end), the level's main loop ending
    /// (`$gp − 0x611c`).
    pub next_level: Option<i32>,
    /// The request's `save`: the autosave `0x30d658(0, level)` before the level ends.
    pub next_level_save: bool,
    /// `0x1a7a04`: the space fights' bolt pickups (the first one's help asked).
    pub bolt_help_done: bool,
    /// `0x1a7a04` itself and its panel's ticks left (`0x40f3e0`, the ship's `+0x30`).
    pub space_bolts: i32,
    pub space_bolts_shown: i32,
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
            bolts: 0, raritanium: 0, max_health: 4, level: 0, occlusion: 0, crate_sound_frame: -100, pickup_sound_frame: -100, bolt_bank: 0, xp_bank: 0, xp: 0, uid_bits: vec![0; 0x400],
            show_bolts: None, show_health: None, rng: Default::default(), recent_breaks: 0, last_break_frame: -1000, frame: 0,
            clank: false, thruster: false, boots: false, charge_boots: false, grind_boots: false, nanotech: [false; 10], level_xp: [0; 2], death_z: None, boss_bar: None, water_base: 0.0, water_z: 0.0, missions: vec![0; 0x200], mission_area: 0, cam_tweaks: Default::default(), uid_pending: vec![0; 0x800], uid_dead: vec![0; 0x800], mission_table: vec![0; 0x100],
            mission_xp: vec![0; 0x30], mission_bolts: vec![0; 0x30], checkpoint: None, checkpoint_bits: Vec::new(), last_checkpoint: None, messages: Vec::new(), help_state: 0, help_req: None, help_counts: vec![0; 0x40], no_item_area: false, variant: 0, items: vec![0; 0x40], quick_select: vec![0; 28], ring: [0; 8], ammo: vec![0; 0x50], held: 0, wrench_out: false, trail_phase: 0.0, view_far: 1024.0, sky: Default::default(), markers: Vec::new(), scenes_seen: vec![false; 0x100], scenes: Default::default(), cameras: Default::default(), ship_parts: Vec::new(), area_ask: Vec::new(), area_hero: Vec::new(), next_level: None, next_level_save: false, bolt_help_done: false, space_bolts: 0, space_bolts_shown: 0, play_time: 0, visited: vec![0; 0x1c],
        }
    }
}

/// The items `0x2f45e0` puts in the quick select: those of the pause's Weapons and Gadgets lists (`0x28297e`,
/// `0x282706`) that the hand holds (record `−0x04` = 0; item 9's is 3) and that neither `0x286d4e` nor `0x1a8e60`
/// keeps off.
pub const RING_ITEMS: [i32; 28] = [
    0x1e, 0x2a, 0x16, 0x18, 0x17, 0x29, 0x1a, 0x2d, 0x1f, 0x1d, 0x25, 0x1b, 0x1c, 0x19, 0x20, 0x10, 0x12, 0x0c, 0x35, 0x0e, 0x11,
    0x2b, 0x2c, 0x0d, 0x24, 0x27, 0x2e, 0x37,
];

impl GameState {
    /// `0x2e5df8(far)`: the view distance, clamped to 1024..4096.
    pub fn set_view_far(&mut self, far: f32) { self.view_far = far.clamp(1024.0, 4096.0); }

    /// `0x1b1e34`: the mobys' draw-distance cap (the view distance − 64).
    pub fn moby_draw_cap(&self) -> i32 { self.view_far as i32 - 0x40 }

    /// `0x2f50f8(level, save)`: leave for `level` (−1: the front end, no save: the card's slot dropped); `save` asks
    /// for the autosave `0x30d658(0, level)` (the engine's, with the card).
    pub fn go_to_level(&mut self, level: i32, save: bool) {
        self.next_level = Some(level);
        self.next_level_save = save;
    }

    /// The level start (`0x2a3b88`, `0x304660`): level `level` with this level's state fresh and the game's kept (the
    /// bolts, the health and the XP, the items, the missions, the scenes seen, the help slots' counts). The per-level
    /// save bytes (`0x19b4a8 + level·0x400`) are one level's here: a level visited again starts as new.
    pub fn enter_level(&mut self, level: i32) {
        let old = std::mem::take(self);
        *self = GameState {
            level,
            bolts: old.bolts,
            raritanium: old.raritanium,
            max_health: old.max_health,
            bolt_bank: old.bolt_bank,
            xp_bank: old.xp_bank,
            xp: old.xp,
            rng: old.rng,
            clank: old.clank,
            thruster: old.thruster,
            boots: old.boots,
            charge_boots: old.charge_boots,
            grind_boots: old.grind_boots,
            nanotech: old.nanotech,
            level_xp: old.level_xp,
            missions: old.missions,
            help_counts: old.help_counts,
            items: old.items,
            quick_select: old.quick_select,
            ring: old.ring,
            ammo: old.ammo,
            held: old.held,
            wrench_out: old.wrench_out,
            scenes_seen: old.scenes_seen,
            play_time: old.play_time,
            visited: old.visited,
            ..Default::default()
        };
        // The level start (`0x2a3b88`): visited.
        if let Some(v) = usize::try_from(level).ok().and_then(|l| self.visited.get_mut(l)) {
            if *v == 0 { *v = 1; }
        }
        self.start_items();
    }

    /// `0x2fcf60` (every level start, `0x2fcd40`): the items a game starts with, when not had yet — the Lancer (the
    /// weapon held, `0x1a7398`), the Gravity Bomb and the pilot's helmet (`0x1a73a0`; the head item is its default
    /// already). Not ported: the Weapons page's list `0x1a7b70`.
    pub fn start_items(&mut self) {
        use crate::hero::items::{GRAVITY_BOMB, LANCER};
        if self.items.get(LANCER as usize).is_some_and(|&b| b == 0) {
            self.get_item(LANCER);
            self.held = LANCER;
        }
        if self.items.get(GRAVITY_BOMB as usize).is_some_and(|&b| b == 0) { self.get_item(GRAVITY_BOMB); }
        if let Some(b) = self.items.get_mut(0x2f) { *b = 1; }
    }

    /// `0x2f46e0(item)`: the item had, with its row's starting ammo when it has none, and a slot of the quick select
    /// for it (`0x2f45e0`).
    pub fn get_item(&mut self, item: i32) {
        let Some(have) = self.items.get_mut(item.max(0) as usize) else { return };
        if *have != 0 { return; }
        *have = 1;
        if let (Some(r), Some(a)) = (crate::hero::items::row(item), self.ammo.get_mut(item.max(0) as usize)) {
            if r.ammo && *a == 0 { *a = r.start_ammo; }
        }
        self.ring_add(item);
    }

    /// `0x2f45e0(item)`: an item of the pause's Weapons or Gadgets lists (`0x28297e`, `0x282706`) that the hand holds
    /// (its record's `−0x04` is 0) and that is not kept off the ring (`0x286d4e`, `0x1a8e60`) takes the first free
    /// slot, unless it has one: [`RING_ITEMS`].
    pub fn ring_add(&mut self, item: i32) {
        if !RING_ITEMS.contains(&item) || self.ring.contains(&item) { return; }
        if let Some(s) = self.ring.iter_mut().find(|s| **s == 0) { *s = item; }
    }

    /// `0x322730(item)`: the item had; a weapon or gadget (below 0x15, or 0x18) goes in the first free
    /// quick-select slot.
    pub fn give_item(&mut self, item: i32) {
        let Some(have) = self.items.get_mut(item.max(0) as usize) else { return };
        if *have != 0 { return; }
        *have = 1;
        if item < 0x15 || item == 0x18 {
            let n = self.quick_select.iter().filter(|&&q| q != 0).count();
            if let Some(q) = self.quick_select.get_mut(n) { *q = item; }
        }
    }

    /// `0x2ae140` (from the hero's update): each area asked about within the last 10 frames lists Ratchet when he is
    /// in it.
    pub fn area_tick(&mut self, level: &crate::level::LevelData, hero: [f32; 3]) {
        let n = level.areas.len();
        self.area_ask.resize(n, 0);
        self.area_hero.resize(n, false);
        for i in 0..n {
            self.area_hero[i] = false;
            let mut t = self.area_ask[i] as i32;
            let done = crate::moby::countdown(&mut t);
            self.area_ask[i] = t as i16;
            if !done && level.in_area(i as i32, hero) { self.area_hero[i] = true; }
        }
    }

    /// `0x32ab30`'s ask: area `i` is watched for the next 10 frames; whether Ratchet was in it at the last look.
    pub fn area_has_hero(&mut self, i: i32) -> bool {
        let Ok(i) = usize::try_from(i) else { return false };
        if self.frame < 2 { return false; }
        if i < self.area_ask.len() { self.area_ask[i] = 10; }
        self.area_hero.get(i).copied().unwrap_or(false)
    }

    /// `0x3132a8(mission)`: the level's mission done (bit 7).
    pub fn mission_done(&mut self, mission: u8) {
        if mission == 0xff { return; }
        if let Some(b) = self.missions.get_mut(self.level.max(0) as usize * 0x10 + mission as usize) { *b |= 0x80; }
    }

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

    /// `0x2e4450(mission)`: the mission's rank, how many of its enemies come (the placed records' word 9 against
    /// it, crate::moby::SpawnRules): 3 in the challenge mode (`0x1a7a12`), else `3 − stat / 100`, at least 1. The stat
    /// (`0x2e43a8`) weighs ten of the player's counters (`0x1a24a8`) by the level's table (`0x25ed60`: per mission
    /// 0x28 bytes of `{counter row, counter, weight}`); the port keeps no counters yet, so it is 0 and the rank 3, as
    /// on a fresh game.
    pub fn mission_rank(&self, _mission: u8) -> u8 { 3 }

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

    /// The count up by one (at most 7).
    pub fn count_uid(&mut self, uid: i16) {
        let Some(b) = self.uid_bits.get_mut((uid as i32 >> 1).max(0) as usize) else { return };
        if uid & 1 != 0 {
            if (*b >> 4) & 7 != 7 { *b += 0x10; }
        } else if *b & 7 != 7 {
            *b += 1;
        }
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
