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
    /// Ratchet's joint lists (chains) for his attachment points, and the joint each list's modifiers act on
    /// (`rc_formats::moby_anim::list_target`; 0xff none).
    pub hero_chains: Vec<Vec<u8>>,
    pub hero_targets: Vec<u8>,
    /// The level's grind paths (the gameplay's section 31).
    pub rails: Vec<rc_game2::paths::GrindPath>,
    /// The level's volumes and paths.
    pub level: rc_game2::level::LevelData,
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
        match self.placed_index(m) {
            Some(k) => {
                let a = self.anim.as_deref()?;
                let i = a.instances.get(k)?;
                Some((i.class, i.state, a.snapshots.get(k).cloned().flatten()))
            }
            // A created moby nothing has animated yet plays its class's first sequence (`InitMobyInstance`): its
            // sphere and joints are that sequence's.
            None if m.instance.is_none() => {
                let ci = self.class_of(m.o_class)?;
                Some((ci, AnimState::spawn(self.lv.anim.get(ci)?), None))
            }
            None => None,
        }
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
        if let Some(c) = self.created.get_mut(&m.index) {
            c.state.speed = speed;
            return;
        }
        let Some(k) = self.placed_index(m) else { return };
        if let Some(i) = self.anim.as_deref_mut().and_then(|a| a.instances.get_mut(k)) { i.state.speed = speed; }
    }

    /// `0x311880`: key A's time (key B's without a sequence A) over 16; between two keys of one sequence the lerp of
    /// theirs by `t`; else key A's plus `t`.
    fn frame_time(&self, m: &Moby) -> f32 {
        let Some((class, s, _)) = self.state_of(m) else { return 0.0 };
        let Some(ac) = self.lv.anim.get(class) else { return 0.0 };
        let time = |q: u8, f: u8| ac.sequences.get(q as usize).and_then(Option::as_ref).and_then(|q| q.frames.get(f as usize)).map_or(0.0, |f| f.header.time as f32 / 16.0);
        let a = if s.seq_a == 0xff { time(s.seq_b, s.frame_b) } else { time(s.seq_a, s.frame_a) };
        if s.t == 0.0 { return a; }
        if s.seq_a == s.seq_b && s.frame_a <= s.frame_b { return a * (1.0 - s.t) + time(s.seq_b, s.frame_b) * s.t; }
        a + s.t
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

    /// `0x312728`: joint `j`'s pose (the chain 0..=j) turned into the world by the moby's rows, scale and place.
    fn joint_matrix(&self, m: &Moby, j: usize) -> Option<([V3; 3], V3)> {
        let (class, state, snap) = self.state_of(m)?;
        let ac: &MobyAnimClass = self.lv.anim.get(class)?;
        if ac.joint_count <= j { return None; }
        let chain: Vec<u8> = (0..=j as u8).collect();
        let p = moby_anim::evaluate_chains(ac, &state, snap.as_ref(), &[chain.as_slice()]);
        let p = p.first()?;
        let rows = m.rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
        Some(rows_f(moby_anim::attach_matrix(p, &rows, m.pos, m.scale)))
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
    pub fn load(lv: &crate::level_load::LoadedLevel, blobs: crate::rac2_level::ClassBlobs, host: &Host, game: &mut rc_game2::game::GameState, fresh: bool) -> Rac2Mobys {
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
                reactions: m
                    .anim
                    .get(ci)
                    .map(|a| {
                        let blob = blobs.blobs.get(&c.o_class);
                        a.sequences
                            .iter()
                            .map(|q| {
                                let td = q.as_ref()?.header.trigger_data as usize;
                                if td == 0 { return None; }
                                rc_game2::moby::ai::Reaction::parse(blob?.get(td..td + 0x20)?)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        use rc_formats::rac2::gameplay as g2;
        let records = g2::moby_instances(&lv.gameplay).unwrap_or_default();
        // The classes without a model (the level's logic objects: the arrival, the scene triggers, …) are in the
        // game's class table too, with their updates (`0x32e1f8` fills the slots by o_class, model or not).
        let mut classes: Vec<ClassInfo> = classes;
        for r in &records {
            let o = r.base.o_class as i16;
            if classes.iter().any(|c| c.o_class == o) { continue; }
            classes.push(ClassInfo { o_class: o, update: rc_game2::moby::classes::update_for(o), scale: 1.0, ..Default::default() });
        }
        let mut world = MobyWorld::new(classes);
        let data = rc_game2::moby::SpawnData {
            pvars: g2::pvar_blocks(&lv.gameplay).map_err(|e| eprintln!("rac2: pvars: {e}")).unwrap_or_default(),
            links: g2::pvar_fixups(&lv.gameplay, "pvar_moby_links").unwrap_or_default(),
            fixups: g2::pvar_fixups(&lv.gameplay, "pvar_pointer_fixups").unwrap_or_default(),
            groups: g2::moby_groups(&lv.gameplay).map_err(|e| eprintln!("rac2: moby groups: {e}")).unwrap_or_default(),
            ties: g2::tie_instances(&lv.gameplay).unwrap_or_default().iter().map(|t| t.world_matrix()).collect(),
            tie_classes: g2::tie_instances(&lv.gameplay).unwrap_or_default().iter().map(|t| t.o_class).collect(),
            shrubs: g2::shrub_instances(&lv.gameplay).unwrap_or_default().iter().map(|t| t.world_matrix()).collect(),
            tie_groups: g2::instance_groups(&lv.gameplay, "tie_groups").map_err(|e| eprintln!("rac2: tie groups: {e}")).unwrap_or_default(),
            shrub_groups: g2::instance_groups(&lv.gameplay, "shrub_groups").map_err(|e| eprintln!("rac2: shrub groups: {e}")).unwrap_or_default(),
        };
        let ground = |p: [f32; 3]| -> Option<f32> {
            let coll = lv.collision.as_ref()?;
            rc_game::collision_query::coll_line(coll, [p[0], p[1], p[2] + 0.5], [p[0], p[1], 0.01], rc_game::collision_query::QueryFlags(2)).map(|o| o.point[2])
        };
        let mut rules = rc_game2::moby::SpawnRules { game, fresh, ground: &ground };
        world.spawn_static(&records, &data, host, &mut rules);
        let hero_moby = world.mobys.iter().position(|m| m.o_class == 0);
        let lists: Vec<(Vec<u8>, Vec<u8>)> = m
            .classes
            .iter()
            .find(|c| c.o_class == 0)
            .and_then(|c| blobs.blobs.get(&0).map(|b| (c, b)))
            .map(|(c, b)| (0..31).map(|l| rc_formats::gadget::joint_list(b, &c.class.header, l).unwrap_or_default()).collect())
            .unwrap_or_default();
        let hero_targets = lists.iter().map(|(_, second)| rc_formats::moby_anim::list_target(second).unwrap_or(0xff)).collect();
        let hero_chains = lists.into_iter().map(|(chain, _)| chain).collect();
        let coll = Arc::new(blobs.collision.into_iter().map(|(c, b)| (c as i16, b)).collect());
        let snap = Some(take_snapshot(&world, &coll, host));
        let rails = g2::grind_paths(&lv.gameplay).map_err(|e| eprintln!("rac2: grind paths: {e}")).unwrap_or_default();
        Rac2Mobys { world, coll, cache: Arc::new(Mutex::new(PoseCache::default())), hero_moby, snap, created: HashMap::new(), hero_chains, hero_targets, rails, level: rc_game2::level::LevelData::load(&lv.gameplay) }
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

/// The extra-instance set the created mobys draw with (crate::moby_render's dynamic slots, as R&C1's created mobys:
/// `ExtraMobys::show_slot`): a slot per moby.
#[derive(Resource)]
pub struct CreatedDraw {
    extra: ExtraMobys,
    /// Moby index → draw slot.
    slots: HashMap<usize, u32>,
}

impl CreatedDraw {
    pub fn new(level: &crate::level_load::LoadedLevel, buffers: &mut Assets<ShaderBuffer>) -> CreatedDraw {
        let records = vec![0u8; DRAW_SLOTS * moby_render::EXTRA_RECORD_SIZE];
        let extra = ExtraMobys::new(level, records, crate::moby_anim::identity_palette(DRAW_SLOTS as u32 * SLOT_JOINTS), buffers);
        CreatedDraw { extra, slots: HashMap::new() }
    }

    /// Draws every live created moby with a class of the level (state < 0x80, not hidden): its slot shown with its
    /// class (`show_slot`: the entities, the slot's look), its record from its rows, scale and position, its pose from
    /// its own animation (unlit for now: the moby lighting is milestone D). Slots whose moby went away are hidden.
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
        _transforms: &mut Query<&mut Transform>,
    ) {
        let lv = &level.mobys;
        let mut palette = crate::moby_anim::identity_palette(DRAW_SLOTS as u32 * SLOT_JOINTS);
        let mut records = vec![0u8; DRAW_SLOTS * moby_render::EXTRA_RECORD_SIZE];
        let live = |m: &Moby| m.instance.is_none() && m.state < 0x80 && m.mode & 0x81 == 0 && m.o_class != 0;
        self.slots.retain(|id, _| mobys.world.mobys.get(*id).is_some_and(live));
        let mut shown: Vec<u32> = Vec::new();
        for m in mobys.world.mobys.iter().filter(|m| live(m)) {
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
            let Some(class) = lv.classes.get(c.class) else { continue };
            let used: Vec<u32> = self.slots.values().copied().collect();
            let Some(next) = (0..DRAW_SLOTS as u32).find(|s| !used.contains(s)) else { continue };
            let slot = *self.slots.entry(id).or_insert(next);
            let model = moby_render::extra_model(m.rows, m.scale, m.pos);
            let look = moby_render::SlotLook { model, alpha: 0x80, fading: false, mode: m.mode, glow: 0, shine: 0, e: [[0.0; 3]; 3], late: false };
            self.extra.show_slot(commands, level, slot, Some((class, look)), meshes, images, materials, buffers);
            shown.push(slot);
            if let Some(ac) = lv.anim.get(c.class) {
                let f = moby_anim::evaluate_with_snapshot(ac, &c.state, c.snapshot.as_ref());
                let at = (slot * SLOT_JOINTS) as usize * 64;
                for (k, b) in f.iter().take(SLOT_JOINTS as usize).flat_map(|r| r.iter().flatten().flat_map(|v| v.to_le_bytes())).enumerate() {
                    if let Some(p) = palette.get_mut(at + k) { *p = b; }
                }
            }
            let rec = moby_render::extra_record(&model, None, slot * SLOT_JOINTS);
            let at = slot as usize * moby_render::EXTRA_RECORD_SIZE;
            records[at..at + rec.len()].copy_from_slice(&rec);
        }
        for slot in 0..DRAW_SLOTS as u32 {
            if !shown.contains(&slot) { self.extra.show_slot(commands, level, slot, None, meshes, images, materials, buffers); }
        }
        crate::asset_write::set_buffer(buffers, &self.extra.palette, &palette);
        crate::asset_write::set_buffer(buffers, &self.extra.instances, &records);
    }
}
