//! The eight dynamic point lights (level01 bank 0x180740, 8 × 0x20 bytes; per slot `{r, g, b, intensity}` then
//! `{position, radius}`, the active flag at 0x180950 + 0x30·i): `WritePointLight_B` 0x252750 takes the first free
//! slot (only while the frame-load measure 0x15f5d4 is at most 0.8), the owner rewrites its slot every tick, and
//! `FreePointLight` 0x252850 clears it. The explosion light moby (class 0x27f, `crate::base::moby_update::creature::fx`)
//! is the owner on every level.
//!
//! Who reads the bank in the game: `MobyProc` merges the lights in range of a moby into its third directional light
//! (docs/plan/moby_skinning_lighting.md "Point lights"; the port: `rc-engine` moby_render `point_light_merge`), and
//! `LightTfrags` / `LightTies` / `LightShrubs` relight the world instances whose point-light lists name a slot
//! (docs/plan/tfrag_lighting.md "Point lights on world geometry"). Which instances a slot's list names is decided
//! here ([`WorldLightLists`], the game's `UpdateAllPointLights` 0x2528a8 / `CreatePointLight` 0x252a28 /
//! `DetachPointLight` 0x252e08); the lighting itself runs in the engine's world shaders.

/// One slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    /// +0x00..+0x08: colour (the light moby's channels / 100; 1.0 adds 128 to a colour byte).
    pub color: [f32; 3],
    /// +0x0c: intensity (the moby's +0x24 byte / 100).
    pub intensity: f32,
    /// +0x10..+0x18: position (game units).
    pub pos: [f32; 3],
    /// +0x1c: radius (game units).
    pub radius: f32,
}

/// The bank.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointLights {
    pub slots: [Option<PointLight>; 8],
    /// Bumped by every [`PointLights::alloc`] of the slot: `WritePointLight_*` zeroes the slot's attachment record
    /// (0x180940 + 0x30·i) and sets its state to 1, so [`WorldLightLists`] can tell a new light in a reused slot.
    pub(crate) generation: [u32; 8],
}

/// The world instances' bounding spheres the attachment tests, in the game's run-time records: ties and shrubs
/// `(centre, radius)` in game units (record +0x00, `tie_light::instance_centre` / `shrub_light::instance_centre`),
/// tfrags the header's sphere in raw units (1024 per game unit, header +0x00). Index = instance / tfrag index.
#[derive(Clone, Debug, Default)]
pub struct WorldSpheres {
    pub ties: Vec<[f32; 4]>,
    pub tfrags: Vec<[f32; 4]>,
    pub shrubs: Vec<[f32; 4]>,
}

/// A point-light nibble list with no light (the level loader's value; low nibble first, 0xf ends the list).
pub(crate) const NO_LIGHTS: u16 = 0xffff;
/// Entries of one slot's instance list (`0x180ac0 + 0x400·i`, 0x200 u16), shared by ties, then tfrags, then shrubs.
pub(crate) const LIST_CAPACITY: usize = 0x200;
/// `UpdateAllPointLights` re-attaches a light once it is more than this far (game units, `vec_distance` 0x221360:
/// 3D) from where it was attached; `CreatePointLight` tests the spheres with `radius + 8`, so the lists stay a
/// superset of the instances the light can reach while it moves less than that.
pub(crate) const MOVE_THRESHOLD: f32 = 8.0;

/// One slot's attachment record (0x180940 + 0x30·i): state (+0x10: 0 free, 1 written, 2 attached), the position it
/// was attached at (+0x20) and the instances whose lists name it (the u16 list at +0x0c; counts +0x02/+0x0a/+0x06).
#[derive(Clone, Debug, Default)]
struct Attachment {
    state: u8,
    generation: u32,
    pos: [f32; 3],
    ties: Vec<u16>,
    tfrags: Vec<u16>,
    shrubs: Vec<u16>,
}

/// The per-instance point-light lists of the world geometry (tie and shrub run-time record +0x1e, tfrag header
/// +0x36) and the slots' attachment records, maintained exactly like the game's `UpdateAllPointLights` (0x2528a8,
/// once per tick after the mobys) with `CreatePointLight` (0x252a28) and `DetachPointLight` (0x252e08):
/// * a slot written by `WritePointLight_*` (state 1, attachment position zeroed) or an attached one (state 2) whose
///   light is more than [`MOVE_THRESHOLD`] from its attachment position is (re)attached at its current position;
/// * attaching tests the sphere `(pos, radius + 8)` (the radius *at that moment*; later radius changes do not
///   re-attach) against every tie record, then every tfrag (the sphere scaled by 1024), then every shrub
///   (`fun_001f9bb0`: `(dx² + dy²) + dz² − (r₁ + r₂)² < 0`); a hit takes the first free nibble of the instance's
///   list (none free: the instance is skipped), and the slot's list holds at most [`LIST_CAPACITY`] instances over
///   all three kinds (full: the rest of the world is not attached);
/// * freeing a slot (`FreePointLight` 0x252850) or re-attaching removes its nibble from the instances it named
///   (the higher nibbles move down); an instance whose list empties is marked dirty and relit once, so its colours
///   go back to the baked ones.
///
/// The lists only decide *which* instances a light may relight; the lighting passes test the light's current
/// radius per vertex (tfrag) or at the instance centre (tie, shrub).
#[derive(Clone, Debug, Default)]
pub struct WorldLightLists {
    pub ties: Vec<u16>,
    pub tfrags: Vec<u16>,
    pub shrubs: Vec<u16>,
    slots: [Attachment; 8],
}

/// `fun_001f9bb0`: true when the spheres overlap (`vsub.xyz`, `vadd.w`, `(dx² + dy²) + 1·dz² − (r₁ + r₂)²` negative).
pub(crate) fn spheres_overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let r = a[3] + b[3];
    (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2] - r * r < 0.0
}

/// `CreatePointLight`'s nibble insert: the first free nibble of `list` (low end first) gets `slot`; false when all
/// four are taken.
pub(crate) fn add_to_list(list: &mut u16, slot: usize) -> bool {
    let s = slot as u16 & 0xf;
    let l = *list;
    *list = if l == NO_LIGHTS {
        s | 0xfff0
    } else if l & 0xfff0 == 0xfff0 {
        l & 0xff0f | s << 4
    } else if l & 0xff00 == 0xff00 {
        l & 0xf0ff | s << 8
    } else if l & 0xf000 == 0xf000 {
        l & 0x0fff | s << 12
    } else {
        return false;
    };
    true
}

/// `DetachPointLight`'s nibble removal: the nibble naming `slot` goes, the higher ones move down, 0xf fills the top.
/// (No match: the game prints an error and leaves the list; unreachable here.)
pub(crate) fn remove_from_list(list: &mut u16, slot: usize) {
    let (l, s) = (*list as u32, slot as u32 & 0xf);
    let r = if l & 0xf == s {
        l >> 4 | 0xf000
    } else if l & 0xf0 == s << 4 {
        l & 0xf | (l >> 4) & 0xff0 | 0xf000
    } else if l & 0xf00 == s << 8 {
        l & 0xff | (l >> 4) & 0xf00 | 0xf000
    } else if l & 0xf000 == s << 12 {
        l | 0xf000
    } else {
        l
    };
    *list = r as u16;
}

impl WorldLightLists {
    /// Every list empty (`NO_LIGHTS`), as the level loader leaves them.
    pub fn new(spheres: &WorldSpheres) -> WorldLightLists {
        WorldLightLists {
            ties: vec![NO_LIGHTS; spheres.ties.len()],
            tfrags: vec![NO_LIGHTS; spheres.tfrags.len()],
            shrubs: vec![NO_LIGHTS; spheres.shrubs.len()],
            slots: Default::default(),
        }
    }

    /// `UpdateAllPointLights` for the bank as the tick left it (frees and writes since the last call applied first).
    /// True when any list changed.
    pub fn update(&mut self, bank: &PointLights, spheres: &WorldSpheres) -> bool {
        let mut changed = false;
        for i in 0..8 {
            let light = bank.slots[i];
            // FreePointLight (or a free + a new WritePointLight in the same slot): detach and clear the record.
            if self.slots[i].state != 0 && (light.is_none() || bank.generation[i] != self.slots[i].generation) {
                changed |= self.detach(i);
                self.slots[i] = Attachment::default();
            }
            let Some(l) = light else { continue };
            if self.slots[i].state == 0 {
                self.slots[i] = Attachment { state: 1, generation: bank.generation[i], ..Default::default() };
            }
            let a = self.slots[i].pos;
            let d = [l.pos[0] - a[0], l.pos[1] - a[1], l.pos[2] - a[2]];
            if (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2] > MOVE_THRESHOLD * MOVE_THRESHOLD {
                self.slots[i].pos = l.pos;
                if self.slots[i].state == 2 { changed |= self.detach(i); }
                changed |= self.create(i, &l, spheres);
                self.slots[i].state = 2;
            }
        }
        changed
    }

    /// `CreatePointLight(i)`: ties, then tfrags, then shrubs, within the shared list capacity.
    fn create(&mut self, i: usize, l: &PointLight, spheres: &WorldSpheres) -> bool {
        let sphere = [l.pos[0], l.pos[1], l.pos[2], l.radius + 8.0];
        let scaled = sphere.map(|x| x * 1024.0);
        let mut n = 0usize;
        let mut att = std::mem::take(&mut self.slots[i]);
        // (instance lists, their spheres, the instances this slot names, the test sphere)
        type Kind<'a> = (&'a mut Vec<u16>, &'a [[f32; 4]], &'a mut Vec<u16>, [f32; 4]);
        let kinds: [Kind; 3] = [
            (&mut self.ties, &spheres.ties, &mut att.ties, sphere),
            (&mut self.tfrags, &spheres.tfrags, &mut att.tfrags, scaled),
            (&mut self.shrubs, &spheres.shrubs, &mut att.shrubs, sphere),
        ];
        'kinds: for (lists, spheres, named, s) in kinds {
            for (k, b) in spheres.iter().enumerate() {
                if n >= LIST_CAPACITY { break 'kinds; }
                if spheres_overlap(s, *b) && add_to_list(&mut lists[k], i) {
                    named.push(k as u16);
                    n += 1;
                }
            }
        }
        self.slots[i] = att;
        n > 0
    }

    /// `DetachPointLight(i)`: its nibble leaves every instance it named. True when it named any.
    fn detach(&mut self, i: usize) -> bool {
        let a = &mut self.slots[i];
        let any = !(a.ties.is_empty() && a.tfrags.is_empty() && a.shrubs.is_empty());
        for (lists, named) in [(&mut self.ties, &mut a.ties), (&mut self.tfrags, &mut a.tfrags), (&mut self.shrubs, &mut a.shrubs)] {
            for k in named.drain(..) {
                if let Some(l) = lists.get_mut(k as usize) { remove_from_list(l, i); }
            }
        }
        any
    }
}

