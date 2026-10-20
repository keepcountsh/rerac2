//! Going Commando's mobys in the engine (rc_game2::moby): the moby world made from the loaded level (classes, the
//! static spawn), the host the moby loop and the class updates call (the animation states: crate::moby_anim's for
//! the placed instances, a store of their own for the mobys the game creates; joint positions; Ratchet's attachment
//! points), the moby scene the collision queries test (the moby pass of rc_game's kernels, which is R&C2's), and the
//! draw of the created mobys (crate::moby_render's extra instances). Built and ticked by crate::rac2_play
//! (docs/plan/rac2_bringup.md §6.6).

use crate::moby_anim::MobyAnim;
use crate::moby_render::{self, ExtraMobys, LevelMobys, MobyMaterial, MobyOcclusion};
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use rc_formats::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};
use rc_formats::moby_collision::MobyCollision;
use rc_game::collision_query::mobys::{pose_joints, CollMoby, MobyGrid, MobyScene, MobySource, PoseCache};
use rc_game2::hero::physics::V3;
use rc_game2::moby::{ClassInfo, Moby, MobyHost, MobyWorld};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Ratchet's joint lists `0x2aa1f0` evaluates (the nine words at `0x1a7f68`); attachment `k` is list `HERO_LISTS[k]`.
pub const HERO_LISTS: [usize; 9] = [0, 1, 2, 3, 4, 5, 6, 29, 30];
/// Palette slots per created moby (joints; the largest gadget class has fewer).
const SLOT_JOINTS: u32 = 96;
/// Created mobys drawn at once.
const DRAW_SLOTS: usize = 48;

/// A created moby's animation (the placed instances' are crate::moby_anim's).
pub struct Created {
    pub class: usize,
    pub state: AnimState,
    pub snapshot: Option<MobyFrame>,
}

/// The moby world of the level and what its collision and draw need.
pub struct Rac2Mobys {
    pub world: MobyWorld,
    pub coll: Arc<HashMap<i16, MobyCollision>>,
    pub cache: Arc<Mutex<PoseCache>>,
    /// Ratchet's moby (the class-0 instance): the queries ignore it.
    pub hero_moby: Option<usize>,
    /// The collision snapshot of the last rebuild (when a moby moved) and the grid it was taken with.
    pub snap: Option<(Snapshot, Arc<MobyGrid>)>,
    /// Created mobys by moby index.
    pub created: HashMap<usize, Created>,
    /// Ratchet's joint lists (chains) for his attachment points.
    pub hero_chains: Vec<Vec<u8>>,
}

/// Ratchet's animation as the hero update left it, for his attachment points.
#[derive(Clone, Copy)]
pub struct HeroPose<'a> {
    pub class: usize,
    pub state: &'a AnimState,
    pub snapshot: Option<&'a MobyFrame>,
    pub pos: V3,
    pub rows: [V3; 3],
    pub scale: f32,
}

/// The loop's view of the engine.
pub struct Host<'a> {
    pub lv: &'a LevelMobys,
    pub occl: Option<&'a MobyOcclusion>,
    pub anim: Option<&'a mut MobyAnim>,
    pub created: &'a mut HashMap<usize, Created>,
    pub hero: Option<HeroPose<'a>>,
    pub hero_chains: &'a [Vec<u8>],
    /// The camera's position (`0x1b8d40`).
    pub camera: V3,
}

impl Host<'_> {
    fn placed_index(&self, m: &Moby) -> Option<usize> { self.occl?.anim_index(m.instance?) }

    /// The class index (into `LevelMobys`) and the animation state of moby `m`.
    fn state_of(&self, m: &Moby) -> Option<(usize, AnimState, Option<MobyFrame>)> {
        if let Some(c) = self.created.get(&m.index) { return Some((c.class, c.state, c.snapshot.clone())); }
        let k = self.placed_index(m)?;
        let a = self.anim.as_deref()?;
        let i = a.instances.get(k)?;
        Some((i.class, i.state, a.snapshots.get(k).cloned().flatten()))
    }

    fn class_of(&self, o_class: i16) -> Option<usize> { self.lv.classes.iter().position(|c| c.o_class == o_class as i32) }
}

fn rows_f(r: [[f32; 4]; 4]) -> ([V3; 3], V3) {
    let f = |v: [f32; 4]| [v[0], v[1], v[2]];
    ([f(r[0]), f(r[1]), f(r[2])], f(r[3]))
}

impl MobyHost for Host<'_> {
    fn camera_pos(&self) -> V3 { self.camera }

    /// `0x313420` for a created moby (the placed ones advance in crate::moby_anim).
    fn advance(&mut self, m: &Moby) {
        let lv = self.lv;
        if let Some(c) = self.created.get_mut(&m.index) {
            if let Some(ac) = lv.anim.get(c.class) { moby_anim::advance(&mut c.state, ac); }
        }
    }

    fn anim(&self, m: &Moby) -> Option<AnimState> { self.state_of(m).map(|s| s.1) }

    fn set_anim(&mut self, m: &Moby, seq: u8, frame: u8, blend: u8) {
        let lv = self.lv;
        if !self.created.contains_key(&m.index) && self.placed_index(m).is_none() {
            if let Some(ci) = self.class_of(m.o_class) {
                if let Some(st) = lv.anim.get(ci).map(AnimState::spawn) {
                    self.created.insert(m.index, Created { class: ci, state: st, snapshot: None });
                }
            }
        }
        if let Some(c) = self.created.get_mut(&m.index) {
            if let Some(ac) = lv.anim.get(c.class) { moby_anim::set_sequence(&mut c.state, ac, seq, frame as i32, blend as i32, &mut c.snapshot); }
            return;
        }
        let Some(k) = self.placed_index(m) else { return };
        let Some(a) = self.anim.as_deref_mut() else { return };
        let Some(i) = a.instances.get_mut(k) else { return };
        let Some(ac) = lv.anim.get(i.class) else { return };
        let mut snap = a.snapshots.get(k).cloned().flatten();
        moby_anim::set_sequence(&mut i.state, ac, seq, frame as i32, blend as i32, &mut snap);
        if let Some(s) = a.snapshots.get_mut(k) { *s = snap; }
    }

    fn set_anim_speed(&mut self, m: &Moby, speed: f32) {
        if let Some(c) = self.created.get_mut(&m.index) { c.state.speed = speed; }
    }

    /// `0x3127c8`: joint `j`'s posed translation · scale / 1024 through the rows, plus the position.
    fn joint(&self, m: &Moby, j: usize) -> Option<V3> {
        let (class, state, snap) = match (self.hero, m.o_class) {
            (Some(h), 0) => (h.class, *h.state, h.snapshot.cloned()),
            _ => self.state_of(m)?,
        };
        let ac = self.lv.anim.get(class)?;
        if ac.joint_count <= j { return None; }
        let chain: Vec<u8> = (0..=j as u8).collect();
        let t = moby_anim::joint_translations(ac, &state, snap.as_ref(), &[chain.as_slice()]);
        let t = t.first()?;
        let p = [f32::from_bits(t[0]), f32::from_bits(t[1]), f32::from_bits(t[2])];
        let s = m.scale / 1024.0;
        let r = m.rows;
        Some(std::array::from_fn(|k| m.pos[k] + (r[0][k] * p[0] + r[1][k] * p[1] + r[2][k] * p[2]) * s))
    }

    /// `0x2aa1f0` / `0x3128f0`: the pose of Ratchet's joint list `HERO_LISTS[k]` turned into the world.
    fn hero_attach(&self, k: usize) -> Option<([V3; 3], V3)> {
        let h = self.hero?;
        let chain = self.hero_chains.get(*HERO_LISTS.get(k)?)?;
        let ac: &MobyAnimClass = self.lv.anim.get(h.class)?;
        let p = moby_anim::evaluate_chains(ac, h.state, h.snapshot, &[chain.as_slice()]);
        let p = p.first()?;
        let rows = h.rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
        Some(rows_f(moby_anim::attach_matrix(p, &rows, h.pos, h.scale)))
    }

    /// The sphere of sequence B, blended from A by t when they differ (`0x313dc0`: `B·t + A·(1 − t)`; A = the
    /// snapshot slot: B's).
    fn sequence_sphere(&self, m: &Moby) -> Option<[f32; 4]> {
        let (class, s, _) = self.state_of(m)?;
        let seqs = &self.lv.anim.get(class)?.sequences;
        let sphere = |q: u8| seqs.get(q as usize).and_then(Option::as_ref).map(|q| q.header.sphere);
        let b = sphere(s.seq_b)?;
        if s.seq_a == s.seq_b { return Some(b); }
        let Some(a) = sphere(s.seq_a) else { return Some(b) };
        Some(std::array::from_fn(|i| b[i] * s.t + a[i] * (1.0 - s.t)))
    }
}

impl Rac2Mobys {
    /// The class table (core order) and the static spawn of the level's moby instances.
    pub fn load(lv: &crate::level_load::LoadedLevel, blobs: crate::rac2_level::ClassBlobs, host: &Host) -> Rac2Mobys {
        let m = &lv.mobys;
        let classes = m
            .classes
            .iter()
            .enumerate()
            .map(|(ci, c)| ClassInfo {
                o_class: c.o_class as i16,
                slot: 0,
                update: rc_game2::moby::classes::update_for(c.o_class as i16),
                scale: c.class.header.scale,
                has_collision: c.class.header.collision != 0,
                mode: c.class.header.mode_bits as u16,
                animated: m.anim.get(ci).and_then(|a| a.sequences.first()).and_then(Option::as_ref).is_some_and(|q| q.header.frame_count >= 2),
                kind: (c.class.header.ty as u16 | (c.class.header.mode_bits2 as u16) << 8) as i16,
            })
            .collect();
        let mut world = MobyWorld::new(classes);
        use rc_formats::rac2::gameplay as g2;
        let records = g2::moby_instances(&lv.gameplay).unwrap_or_default();
        let data = rc_game2::moby::SpawnData {
            pvars: g2::pvar_blocks(&lv.gameplay).map_err(|e| eprintln!("rac2: pvars: {e}")).unwrap_or_default(),
            links: g2::pvar_fixups(&lv.gameplay, "pvar_moby_links").unwrap_or_default(),
            fixups: g2::pvar_fixups(&lv.gameplay, "pvar_pointer_fixups").unwrap_or_default(),
            groups: g2::moby_groups(&lv.gameplay).map_err(|e| eprintln!("rac2: moby groups: {e}")).unwrap_or_default(),
        };
        world.spawn_static(&records, &data, host);
        let hero_moby = world.mobys.iter().position(|m| m.o_class == 0);
        let hero_chains = m
            .classes
            .iter()
            .find(|c| c.o_class == 0)
            .and_then(|c| blobs.blobs.get(&0).map(|b| (c, b)))
            .map(|(c, b)| (0..31).map(|l| rc_formats::gadget::joint_list(b, &c.class.header, l).map(|x| x.0).unwrap_or_default()).collect())
            .unwrap_or_default();
        let coll = Arc::new(blobs.collision.into_iter().map(|(c, b)| (c as i16, b)).collect());
        let snap = Some(take_snapshot(&world, &coll, host));
        Rac2Mobys { world, coll, cache: Arc::new(Mutex::new(PoseCache::default())), hero_moby, snap, created: HashMap::new(), hero_chains }
    }

}

/// The scene the queries test: every moby's collision view and, for classes whose blob poses joints, the joint
/// positions of its current animation state; with the grid as it is now.
pub fn take_snapshot(world: &MobyWorld, coll: &HashMap<i16, MobyCollision>, host: &Host) -> (Snapshot, Arc<MobyGrid>) {
    let mobys: Vec<CollMoby> = world.mobys.iter().map(Moby::coll).collect();
    let joints = world
        .mobys
        .iter()
        .map(|m| {
            let Some(b) = coll.get(&m.o_class) else { return Vec::new() };
            let n = b.joint_counts[0].max(b.joint_counts[1]) as usize;
            if n == 0 { return Vec::new(); }
            match host.state_of(m) {
                Some((class, s, snap)) => pose_joints(host.lv.anim.get(class), &s, snap.as_ref(), n),
                None => vec![[0; 4]; n],
            }
        })
        .collect();
    (Snapshot { mobys, joints }, Arc::new(world.grid.clone()))
}

/// The scene from the snapshot's parts (a free function, so the world can be borrowed mutably beside it).
pub fn scene_of<'a>(snap: &'a Option<(Snapshot, Arc<MobyGrid>)>, coll: &'a Arc<HashMap<i16, MobyCollision>>, cache: &'a Mutex<PoseCache>) -> Option<MobyScene<'a>> {
    snap.as_ref().map(|(s, g)| MobyScene { mobys: s, grid: g, classes: coll, cache })
}

/// The mobys as the collision kernels see them, taken after the moby loop.
pub struct Snapshot {
    mobys: Vec<CollMoby>,
    joints: Vec<Vec<[u32; 4]>>,
}

impl MobySource for Snapshot {
    fn moby(&self, id: usize) -> Option<CollMoby> { self.mobys.get(id).copied() }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> {
        let mut v = self.joints.get(id).cloned().unwrap_or_default();
        v.resize(count, [0; 4]);
        v
    }
}

// ---------------------------------------------------------------------------------------------------
// The created mobys' draw

/// The extra-instance set the created mobys draw with: slot per moby, entities per (slot, class).
#[derive(Resource)]
pub struct CreatedDraw {
    extra: ExtraMobys,
    /// Moby index → draw slot.
    slots: HashMap<usize, u32>,
    /// Entities per (slot, o_class).
    ents: HashMap<(u32, i32), Vec<Entity>>,
    shown: HashMap<u32, i32>,
}

impl CreatedDraw {
    pub fn new(level: &crate::level_load::LoadedLevel, buffers: &mut Assets<ShaderBuffer>) -> CreatedDraw {
        let records = vec![0u8; DRAW_SLOTS * moby_render::EXTRA_RECORD_SIZE];
        let extra = ExtraMobys::new(level, records, crate::moby_anim::identity_palette(DRAW_SLOTS as u32 * SLOT_JOINTS), buffers);
        CreatedDraw { extra, slots: HashMap::new(), ents: HashMap::new(), shown: HashMap::new() }
    }

    /// Draws every live created moby with a class of the level (state < 0x80, not hidden): its pose from its own
    /// animation, its record from its rows, scale and position (unlit for now: the moby lighting is milestone D).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        mobys: &Rac2Mobys,
        level: &crate::level_load::LoadedLevel,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<MobyMaterial>,
        buffers: &mut Assets<ShaderBuffer>,
        transforms: &mut Query<&mut Transform>,
    ) {
        let lv = &level.mobys;
        let mut palette = crate::moby_anim::identity_palette(DRAW_SLOTS as u32 * SLOT_JOINTS);
        let mut records = vec![0u8; DRAW_SLOTS * moby_render::EXTRA_RECORD_SIZE];
        let mut live: Vec<u32> = Vec::new();
        for m in mobys.world.mobys.iter().filter(|m| m.instance.is_none() && m.state < 0x80 && m.mode & 1 == 0 && m.o_class != 0) {
            let id = m.index;
            let fallback;
            let c = match mobys.created.get(&id) {
                Some(c) => c,
                None => {
                    let Some(ci) = lv.classes.iter().position(|c| c.o_class == m.o_class as i32) else { continue };
                    let Some(ac) = lv.anim.get(ci) else { continue };
                    fallback = Created { class: ci, state: AnimState::spawn(ac), snapshot: None };
                    &fallback
                }
            };
            let used: Vec<u32> = self.slots.values().copied().collect();
            let next = (0..DRAW_SLOTS as u32).find(|s| !used.contains(s)).unwrap_or(DRAW_SLOTS as u32);
            let slot = *self.slots.entry(id).or_insert(next);
            if slot as usize >= DRAW_SLOTS { continue; }
            let Some(class) = lv.classes.get(c.class) else { continue };
            let key = (slot, class.o_class);
            if !self.ents.contains_key(&key) {
                let e = self.extra.spawn(commands, level, class, slot, Transform::IDENTITY, "rac2 created moby", meshes, images, materials);
                self.ents.insert(key, e);
            }
            if self.shown.get(&slot) != Some(&class.o_class) {
                for (k, ents) in &self.ents {
                    if k.0 != slot { continue; }
                    let v = if k.1 == class.o_class { Visibility::Inherited } else { Visibility::Hidden };
                    for &e in ents { commands.entity(e).insert(v); }
                }
                self.shown.insert(slot, class.o_class);
            }
            live.push(slot);
            if let Some(ac) = lv.anim.get(c.class) {
                let f = moby_anim::evaluate_with_snapshot(ac, &c.state, c.snapshot.as_ref());
                let at = (slot * SLOT_JOINTS) as usize * 64;
                for (k, b) in f.iter().take(SLOT_JOINTS as usize).flat_map(|r| r.iter().flatten().flat_map(|v| v.to_le_bytes())).enumerate() {
                    if let Some(p) = palette.get_mut(at + k) { *p = b; }
                }
            }
            let model = moby_render::extra_model(m.rows, m.scale, m.pos);
            let rec = moby_render::extra_record(&model, None, slot * SLOT_JOINTS);
            let at = slot as usize * moby_render::EXTRA_RECORD_SIZE;
            records[at..at + rec.len()].copy_from_slice(&rec);
            let t = Transform::from_matrix(model);
            if let Some(ents) = self.ents.get(&key) {
                for &e in ents {
                    if let Ok(mut tr) = transforms.get_mut(e) { *tr = t; }
                }
            }
        }
        // Slots whose moby went away hide.
        let gone: Vec<u32> = self.shown.keys().copied().filter(|s| !live.contains(s)).collect();
        for s in gone {
            for (k, ents) in &self.ents {
                if k.0 == s { for &e in ents { commands.entity(e).insert(Visibility::Hidden); } }
            }
            self.shown.remove(&s);
        }
        self.slots.retain(|id, _| mobys.world.mobys.get(*id).is_some_and(|m| m.instance.is_none() && m.state < 0x80 && m.mode & 1 == 0));
        crate::asset_write::set_buffer(buffers, &self.extra.palette, &palette);
        crate::asset_write::set_buffer(buffers, &self.extra.instances, &records);
    }
}
