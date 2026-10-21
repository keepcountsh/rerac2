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
    /// The moby's `serial` it belongs to: a record of a deleted moby is not the next one's in that slot (`CreateMoby`
    /// starts its animation afresh, `0x311418`).
    pub serial: u32,
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
    /// Created mobys posed from another's joints (Ratchet's head item: [`Rac2Mobys::skin_head`]): their palettes.
    pub skinned: HashMap<usize, Vec<moby_anim::Rows>>,
    /// The ship's canopy (`rc_game2::canopy`, from the level overlay) and this tick's for the draw.
    pub canopy: Option<rc_game2::canopy::Canopy>,
    pub canopy_frame: Option<rc_game2::canopy::CanopyFrame>,
    /// The mobys' loop-sound voice slots (`+0x6d`, [`HostSound::loops`]).
    pub loop_slots: HashMap<usize, u8>,
    /// Ratchet's joint lists (chains) for his attachment points, and the joint each list's modifiers act on
    /// (`rc_formats::moby_anim::list_target`; 0xff none).
    pub hero_chains: Vec<Vec<u8>>,
    pub hero_targets: Vec<u8>,
    /// The second byte lists of his joint lists (a pose layer's joints, `0x311e10(moby, list)`).
    pub hero_seconds: Vec<Vec<u8>>,
    /// The holding classes 1 / 2 (rc_game2::hero::layers: his first 0x17 sequences with the arm holding the item).
    pub hold: [Option<Arc<rc_game::hero::anim::HoldClass>>; 2],
    /// Every class's joint lists (class header `+0x1c`, [`rc_formats::gadget::joint_list`]'s chains): what the
    /// joint queries take as their joint (`0x3175f8(m, 1, [list], out)`: the last joint of list `list`).
    pub chains: Chains,
    /// The level's grind paths (the gameplay's section 31).
    pub rails: Vec<rc_game2::paths::GrindPath>,
    /// The level's volumes and paths.
    pub level: rc_game2::level::LevelData,
    /// The level defs' footstep base (`$gp − 0x60e4`, `0x3012a0`): the level settings' word after their variant
    /// blocks (`[0x17 + 8·n]`, n = `[0x1a]`; `[0x1f]` without blocks).
    pub footstep_base: usize,
}

/// Joint lists per class: each list the root-to-joint chain whose last joint the query returns.
pub type Chains = HashMap<i16, Vec<Vec<u8>>>;

/// Ratchet's animation as the hero update left it, for his attachment points.
#[derive(Clone, Copy)]
pub struct HeroPose<'a> {
    pub class: usize,
    pub state: &'a AnimState,
    pub snapshot: Option<&'a MobyFrame>,
    pub pos: V3,
    pub rows: [V3; 3],
    pub scale: f32,
    /// His posture's joint modifiers (rc_game2::hero::posture), as his drawn pose has them.
    pub mods: &'a [rc_formats::moby_anim::JointModifier],
    /// His pose layers (rc_game2::hero::layers, [`Rac2Mobys::hero_layers`]), as his drawn pose has them.
    pub layers: &'a [crate::moby_anim::InstanceLayer],
}

/// MobyProc's light block for a moby with rows `rows`, light word `light` (`+0x38`) and ambient (`+0x3c`), on the
/// level's bank (crate::moby_light); None unlit.
pub fn moby_lights(lv: &LevelMobys, rows: [V3; 3], light: u32, ambient: [u8; 3]) -> Option<rc_formats::moby_light::MobyLights> {
    let l = lv.lighting.as_ref()?;
    let bits = rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
    Some(rc_formats::moby_light::moby_lights(&bits, &l.bank, light, ambient, 0x80))
}

/// The loop's view of the engine.
pub struct Host<'a> {
    pub lv: &'a LevelMobys,
    pub occl: Option<&'a MobyOcclusion>,
    pub anim: Option<&'a mut MobyAnim>,
    pub created: &'a mut HashMap<usize, Created>,
    pub hero: Option<HeroPose<'a>>,
    pub hero_chains: &'a [Vec<u8>],
    /// The classes' joint lists ([`Rac2Mobys::chains`]; None: the queries find no joints).
    pub chains: Option<&'a Chains>,
    /// The camera's position (`0x1b8d40`).
    pub camera: V3,
    /// The sound system the class sounds play into (crate::audio_out); None: no sound.
    pub sound: Option<HostSound<'a>>,
}

/// The moby loop's sound side: the audio system, the listener (the camera as the last tick left it) and the tick.
pub struct HostSound<'a> {
    pub system: &'a mut rc_game::audio::AudioSystem,
    pub listener: rc_game::audio::voices::Listener,
    pub tick: u64,
    /// `+0x6d` per moby: the voice slot of its sequence's loop sound ([`Rac2Mobys::loop_slots`]).
    pub loops: &'a mut HashMap<usize, u8>,
}

impl Host<'_> {
    fn placed_index(&self, m: &Moby) -> Option<usize> { self.occl?.anim_index(m.instance?) }

    /// A record left by a deleted moby in `m`'s slot goes: `m` starts from the spawn pose.
    fn drop_stale(&mut self, m: &Moby) {
        if self.created.get(&m.index).is_some_and(|c| c.serial != m.serial) { self.created.remove(&m.index); }
    }

    /// The class index (into `LevelMobys`) and the animation state of moby `m`.
    fn state_of(&self, m: &Moby) -> Option<(usize, AnimState, Option<MobyFrame>)> {
        if let Some(c) = self.created.get(&m.index).filter(|c| c.serial == m.serial) { return Some((c.class, c.state, c.snapshot.clone())); }
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

    /// `0x3117a0`: the loop sound `lp` (`+0x6c`, 0xff none) on its voice slot (`+0x6d`): without a slot it plays (flags
    /// 4) on the moby's phase tick (`(moby >> 8 & 3) == (frame & 3)`: the index here); with one, the slot is dropped
    /// when another owner took it and released when it plays another sound of this moby.
    fn refresh_loop(&mut self, m: &Moby, lp: u8) {
        let Some(s) = self.sound.as_mut() else { return };
        let slot = s.loops.get(&m.index).copied().unwrap_or(0xff);
        if slot == 0xff {
            if lp == 0xff || (m.index as u64 & 3) != (s.tick & 3) || m.mode & 0x40 != 0 { return; }
            let k = self.play_sound(m, lp as i8 as i32, 4);
            if let Some(s) = self.sound.as_mut() { if 0 <= k { s.loops.insert(m.index, k as u8); } }
            return;
        }
        let owned = s.system.slots.slots.get(slot as usize).map(|x| (x.owner.map(|o| o.id as usize), x.class_index));
        match owned {
            Some((Some(o), index)) if o == m.index => {
                if index != lp as u16 || m.mode & 0x40 != 0 {
                    s.system.slots.release(slot as i32);
                    s.loops.remove(&m.index);
                }
            }
            _ => { s.loops.remove(&m.index); }
        }
    }

    /// Joint list `list` of class `o_class` (a non-empty chain).
    fn chain(&self, o_class: i16, list: usize) -> Option<&[u8]> {
        self.chains?.get(&o_class)?.get(list).map(Vec::as_slice).filter(|c| !c.is_empty())
    }
}

fn rows_f(r: [[f32; 4]; 4]) -> ([V3; 3], V3) {
    let f = |v: [f32; 4]| [v[0], v[1], v[2]];
    ([f(r[0]), f(r[1]), f(r[2])], f(r[3]))
}

impl MobyHost for Host<'_> {
    fn camera_pos(&self) -> V3 { self.camera }

    /// `0x313420` for a created moby (the placed ones advance in crate::moby_anim).
    /// `0x313420` (`MobyAnimAdvance`): the keys step; a tick that did not land a blend then plays the first trigger
    /// word whose time it passed (`0x3605f0(sound, 0, moby)`), else refreshes the sequence's loop sound (`0x3117a0`).
    fn advance(&mut self, m: &Moby) {
        let lv = self.lv;
        self.drop_stale(m);
        let (class, before, after) = if let Some(c) = self.created.get_mut(&m.index) {
            let Some(ac) = lv.anim.get(c.class) else { return };
            let before = c.state;
            moby_anim::advance(&mut c.state, ac);
            (c.class, before, c.state)
        } else {
            let Some(k) = self.placed_index(m) else { return };
            let Some(a) = self.anim.as_deref_mut() else { return };
            let Some(i) = a.instances.get_mut(k) else { return };
            // The moby loop steps it from now on, not the draw's tick (crate::moby_anim).
            if let Some(d) = a.game_advanced.get_mut(k) { *d = true; }
            let Some(ac) = lv.anim.get(i.class) else { return };
            let before = i.state;
            if !i.state.skip_advance { moby_anim::advance(&mut i.state, ac); }
            (i.class, before, i.state)
        };
        let Some(ac) = lv.anim.get(class) else { return };
        if moby_anim::advance_landed(&before, &after) { return; }
        if let Some(sound) = moby_anim::advance_trigger(&before, &after, ac) {
            self.play_sound(m, sound as i16 as i32, 0);
            return;
        }
        self.refresh_loop(m, moby_anim::loop_sound_of(ac, after.seq_b));
    }

    fn anim(&self, m: &Moby) -> Option<AnimState> { self.state_of(m).map(|s| s.1) }

    fn set_anim(&mut self, m: &Moby, seq: u8, frame: u8, blend: u8) {
        let lv = self.lv;
        self.drop_stale(m);
        if !self.created.contains_key(&m.index) && self.placed_index(m).is_none() {
            if let Some(ci) = self.class_of(m.o_class) {
                if let Some(st) = lv.anim.get(ci).map(AnimState::spawn) {
                    self.created.insert(m.index, Created { class: ci, state: st, snapshot: None, serial: m.serial });
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
        self.drop_stale(m);
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
    /// `0x3127c8(m, list, out)`: the last joint of the class's joint list `list` (`0x3175f8`) in the world.
    fn joint(&self, m: &Moby, j: usize) -> Option<V3> {
        let (class, state, snap) = match (self.hero, m.o_class) {
            (Some(h), 0) => (h.class, *h.state, h.snapshot.cloned()),
            _ => self.state_of(m)?,
        };
        let ac = self.lv.anim.get(class)?;
        let chain = self.chain(m.o_class, j)?;
        let p = match (self.hero, m.o_class) {
            // Ratchet with his posture's modifiers, as he is drawn.
            (Some(h), 0) => {
                let pl: Vec<moby_anim::PoseLayer> = h.layers.iter().map(crate::moby_anim::InstanceLayer::pose).collect();
                let r = moby_anim::evaluate_chains_posed(ac, &state, snap.as_ref(), &[chain], &pl, h.mods);
                let r = r.first()?;
                [r[3][0], r[3][1], r[3][2]]
            }
            _ => {
                let t = moby_anim::joint_translations(ac, &state, snap.as_ref(), &[chain]);
                let t = t.first()?;
                [f32::from_bits(t[0]), f32::from_bits(t[1]), f32::from_bits(t[2])]
            }
        };
        let s = m.scale / 1024.0;
        let r = m.rows;
        Some(std::array::from_fn(|k| m.pos[k] + (r[0][k] * p[0] + r[1][k] * p[1] + r[2][k] * p[2]) * s))
    }

    /// `0x312728`: joint `j`'s pose (the chain 0..=j) turned into the world by the moby's rows, scale and place.
    fn joint_matrix(&self, m: &Moby, j: usize) -> Option<([V3; 3], V3)> {
        let (class, state, snap) = self.state_of(m)?;
        let ac: &MobyAnimClass = self.lv.anim.get(class)?;
        let chain = self.chain(m.o_class, j)?;
        let p = moby_anim::evaluate_chains(ac, &state, snap.as_ref(), &[chain]);
        let p = p.first()?;
        let rows = m.rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
        Some(rows_f(moby_anim::attach_matrix(p, &rows, m.pos, m.scale)))
    }

    /// `0x2aa1f0` / `0x3128f0`: the pose of Ratchet's joint list `HERO_LISTS[k]` turned into the world.
    fn hero_attach(&self, k: usize) -> Option<([V3; 3], V3)> {
        let h = self.hero?;
        let chain = self.hero_chains.get(*HERO_LISTS.get(k)?)?;
        let ac: &MobyAnimClass = self.lv.anim.get(h.class)?;
        let pl: Vec<moby_anim::PoseLayer> = h.layers.iter().map(crate::moby_anim::InstanceLayer::pose).collect();
        let p = moby_anim::evaluate_chains_posed(ac, h.state, h.snapshot, &[chain.as_slice()], &pl, h.mods);
        let p = p.first()?;
        let rows = h.rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
        Some(rows_f(moby_anim::attach_matrix(p, &rows, h.pos, h.scale)))
    }

    /// The sphere of sequence B, blended from A by t when they differ (`0x313dc0`: `B·t + A·(1 − t)`; A = the
    /// snapshot slot: B's).
    /// `0x3605f0`: R&C1's `PlayClassSound` (the class's defs at `+0x28`, count `+0x0d`; the slot remembers the index),
    /// on the audio system's stream.
    fn play_sound(&mut self, m: &Moby, index: i32, mode: i32) -> i32 {
        let Some(s) = self.sound.as_mut() else { return -1 };
        let ev = rc_game::moby_update::services::SoundEvent { index, flags: mode as u32, moby: m.index, o_class: m.o_class, sound_class: m.o_class, pos: m.pos, tick: s.tick };
        let mut rng = s.system.rng;
        let k = s.system.play_class_sound(&ev, None, &s.listener, &mut rng);
        s.system.rng = rng;
        k
    }

    /// `0x3602f0(handle)`: the slot released.
    fn stop_sound(&mut self, handle: i32) {
        if let Some(s) = self.sound.as_mut() { if 0 <= handle { s.system.slots.release(handle); } }
    }

    fn sound_playing(&self, handle: i32) -> bool {
        let Some(s) = self.sound.as_ref() else { return false };
        usize::try_from(handle).ok().and_then(|i| s.system.slots.slots.get(i)).is_some_and(|x| x.state != rc_game::audio::voices::state::FREE)
    }

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

fn footstep_base(gameplay: &[u8]) -> usize {
    let Ok(Some(s)) = rc_formats::rac2::gameplay::level_settings(gameplay) else { return 0 };
    let w = |i: usize| s.get(4 * i..4 * i + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()));
    let n = w(0x1a);
    let at = if n == 0 { 0x1f } else if n < 1 { 0x17 } else { 0x17 + 8 * n as usize };
    usize::try_from(w(at)).unwrap_or(0)
}

impl Rac2Mobys {
    /// Ratchet's pose layers for the evaluator (rc_game2::hero::layers), in his `+0x60` list's order (the newest node
    /// first): each over its joint list's second byte list; a holding layer's keys below 0x17 from its class.
    pub fn hero_layers(&self, l: &rc_game2::hero::layers::Layers) -> Vec<crate::moby_anim::InstanceLayer> {
        let mut v: Vec<(u32, crate::moby_anim::InstanceLayer)> = l
            .hold
            .iter()
            .chain(std::iter::once(&l.face))
            .filter_map(|n| {
                let n = n.as_ref()?;
                let joints = self.hero_seconds.get(n.list as usize).filter(|j| !j.is_empty())?.clone();
                let alt = match n.alt {
                    0 => None,
                    k => {
                        let c = self.hold.get(k as usize - 1)?.clone()?;
                        let s = rc_game2::hero::layers::HOLD_SEQS;
                        Some((c, n.seq_a < s, n.seq_b < s))
                    }
                };
                Some((n.born, crate::moby_anim::InstanceLayer { joints, seq_a: n.seq_a, frame_a: n.frame_a, seq_b: n.seq_b, frame_b: n.frame_b, t: n.t, weight: n.weight, alt }))
            })
            .collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.0));
        v.into_iter().map(|x| x.1).collect()
    }

    /// `0x2aa278` for the head item (`rc_game2::hero::items::head_pass`): a keyframe of its own built from Ratchet's
    /// current pose (`0x3181c0`: his blended key as a frame, [`moby_anim::snapshot`]) — joint 0 at rest, joints 1 and
    /// 2 his [`rc_game2::hero::items::HEAD_JOINTS`]' quaternions, their scale where it is not 1 (inherited; with his
    /// joint modifiers' scale, not their rotations: the head look is not carried yet) and their
    /// translation where it differs from the head item's rest — evaluated with the head item's class as key A. It is
    /// drawn in his frame, so it follows his head.
    pub fn skin_head(&mut self, lv: &crate::level_load::LoadedLevel, head: Option<usize>, pose: Option<&HeroPose>) {
        self.skinned.clear();
        let (Some(id), Some(h)) = (head, pose) else { return };
        let Some(m) = self.world.mobys.get(id) else { return };
        let (Some(ac), Some(hc)) = (lv.mobys.anim.get(h.class), lv.mobys.classes.iter().position(|c| c.o_class == m.o_class as i32).and_then(|ci| lv.mobys.anim.get(ci))) else { return };
        let Some(f) = moby_anim::snapshot(ac, h.state, h.snapshot) else { return };
        let joints = rc_game2::hero::items::HEAD_JOINTS;
        let mut quats = vec![[0, 0, 0, 0x7fff]];
        let (mut scales, mut trans) = (Vec::new(), Vec::new());
        for (k, &j) in joints.iter().enumerate() {
            let (j, i) = (j as usize, k + 1);
            // His local rotation as his drawn pose has it: the key's, then his pose layers (the face layer covers
            // his head's joints 8..51).
            let pl: Vec<moby_anim::PoseLayer> = h.layers.iter().map(crate::moby_anim::InstanceLayer::pose).collect();
            quats.push(moby_anim::layered_quat(ac, &pl, j, f.quat_at(j)));
            // The scale his joint has as the draw poses it: its key's, times his joint modifiers' (his head's 0.875,
            // which shrinks his head into the helmet, reaches the helmet too: the game's joint data carries it).
            let key = f.scales.iter().find(|r| r.joint as usize == j).map_or([1.0; 3], |r| r.value());
            let s: [f32; 3] = h.mods.iter().filter(|m| m.joint as usize == j).fold(key, |a, m| std::array::from_fn(|c| a[c] * m.scale[c]));
            let s = s.map(|c| (c * 4096.0).round().clamp(0.0, 65535.0) as u16);
            if s != [0x1000; 3] { scales.push(moby_anim::ScaleRec { scale: s, joint: i as u8, flags: 0x80 }); }
            let t = f.trans.iter().find(|r| r.joint as usize == j).map(|r| r.trans).or_else(|| ac.rest.get(j).map(|v| v.map(|c| c as i16)));
            let rest = hc.rest.get(i).map(|v| v.map(|c| c as i16));
            if let Some(t) = t.filter(|&t| Some(t) != rest) { trans.push(moby_anim::TransRec { trans: t, joint: i as u8, pad: 0 }); }
        }
        let quat_bytes = 8 * quats.len();
        let trans_offset = quat_bytes + 8 * scales.len();
        let mut payload: Vec<u8> = quats.iter().flatten().flat_map(|c| c.to_le_bytes()).collect();
        payload.extend(scales.iter().flat_map(|r| r.scale.iter().flat_map(|c| c.to_le_bytes()).chain([r.joint, r.flags])));
        payload.extend(trans.iter().flat_map(|r| r.trans.iter().flat_map(|c| c.to_le_bytes()).chain([r.joint, r.pad])));
        payload.resize(payload.len().next_multiple_of(16), 0);
        let header = moby_anim::MobyFrameHeader { rate: 0.0, time: 0, qwc: (payload.len() / 16) as u16, quat_bytes: quat_bytes as u16, scale_count: scales.len() as u16, trans_offset: trans_offset as u16, trans_count: trans.len() as u16 };
        let frame = moby_anim::MobyFrame { header, quats, scales, trans, payload };
        let state = AnimState { seq_a: moby_anim::SNAPSHOT_SEQ, frame_a: 0, seq_b: moby_anim::SNAPSHOT_SEQ, frame_b: 0, t: 0.0, speed: 0.0, rate: 0.0, flags: 0, trigger_count: 0, skip_advance: true };
        self.skinned.insert(id, moby_anim::evaluate_with_snapshot(hc, &state, Some(&frame)));
    }

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
                pieces: c.class.pieces.iter().map(|p| p.as_ref().map(|p| (p.centre, p.points.clone()))).collect(),
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
        let hero_seconds = lists.iter().map(|(_, second)| second.clone()).collect();
        let hero_chains = lists.into_iter().map(|(chain, _)| chain).collect();
        let hold = [1i32, 2].map(|o| {
            let b = blobs.blobs.get(&o)?;
            let c = rc_formats::moby::parse_moby_class_for(rc_formats::Game::Rac2, b).ok()?;
            let map = moby_anim::retarget_map(b, &c.header)?;
            let seqs = moby_anim::parse_sequences(b, &c).ok()?;
            Some(Arc::new(rc_game::hero::anim::HoldClass { anim: MobyAnimClass::new(&c, seqs), map }))
        });
        let chains: Chains = m
            .classes
            .iter()
            .filter_map(|c| {
                let b = blobs.blobs.get(&c.o_class)?;
                let h = &c.class.header;
                if h.joints <= 0 { return None; }
                let n = i32::from_le_bytes(b.get(h.joints as usize..h.joints as usize + 4)?.try_into().ok()?).clamp(0, 0x100) as usize;
                Some((c.o_class as i16, (0..n).map(|l| rc_formats::gadget::joint_list(b, h, l).map(|x| x.0).unwrap_or_default()).collect()))
            })
            .collect();
        let coll = Arc::new(blobs.collision.into_iter().map(|(c, b)| (c as i16, b)).collect());
        let snap = Some(take_snapshot(&world, &coll, host));
        let rails = g2::grind_paths(&lv.gameplay).map_err(|e| eprintln!("rac2: grind paths: {e}")).unwrap_or_default();
        Rac2Mobys { world, coll, cache: Arc::new(Mutex::new(PoseCache::default())), hero_moby, snap, created: HashMap::new(), skinned: HashMap::new(), canopy: None, canopy_frame: None, loop_slots: HashMap::new(), hero_chains, hero_targets, hero_seconds, hold, chains, rails, level: rc_game2::level::LevelData::load(&lv.gameplay), footstep_base: footstep_base(&lv.gameplay) }
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
    /// The entities created moby `moby` is drawn with (none before its first draw).
    pub fn entities_of(&self, moby: usize) -> Vec<Entity> { self.slots.get(&moby).map(|&s| self.extra.slot_entities(s)).unwrap_or_default() }

    pub fn new(level: &crate::level_load::LoadedLevel, buffers: &mut Assets<ShaderBuffer>) -> CreatedDraw {
        let records = vec![0u8; DRAW_SLOTS * moby_render::EXTRA_RECORD_SIZE];
        let extra = ExtraMobys::new(level, records, crate::moby_anim::identity_palette(DRAW_SLOTS as u32 * SLOT_JOINTS), buffers);
        CreatedDraw { extra, slots: HashMap::new() }
    }

    /// Draws every live created moby with a class of the level (state < 0x80, not hidden): its slot shown with its
    /// class (`show_slot`: the entities, the slot's look), its record from its rows, scale and position, its pose from
    /// its own animation, lit by its light word and ambient. Slots whose moby went away are hidden.
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
        _transforms: &mut Query<&mut Transform, Without<crate::fly_cam::FlyCam>>,
        view: Option<(Vec3, [Vec3; 3], (f32, f32))>,
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
            let c = match mobys.created.get(&id).filter(|c| c.serial == m.serial) {
                Some(c) => c,
                None => {
                    let Some(ci) = lv.classes.iter().position(|c| c.o_class == m.o_class as i32) else { continue };
                    let Some(ac) = lv.anim.get(ci) else { continue };
                    fallback = Created { class: ci, state: AnimState::spawn(ac), snapshot: None, serial: m.serial };
                    &fallback
                }
            };
            let Some(class) = lv.classes.get(c.class) else { continue };
            let used: Vec<u32> = self.slots.values().copied().collect();
            let Some(next) = (0..DRAW_SLOTS as u32).find(|s| !used.contains(s)) else { continue };
            let slot = *self.slots.entry(id).or_insert(next);
            let model = moby_render::extra_model(m.rows, m.scale, m.pos);
            // MobyProc's shine gate (crate::moby_lod: `+0x73` is 0x18 for a class with metal packets): the metal pass
            // (chrome, glass: the ship's canopy) within its distance, with its sphere-map basis.
            let (shine, e) = view.map_or((0, [[0.0; 3]; 3]), |(eye, cam, tans)| {
                let h = &class.class.header;
                let shine_distance = if h.metal_count > 0 { crate::moby_lod::SHINE_DISTANCE } else { 0 };
                let inp = crate::moby_lod::ProcInput { position: m.pos, rows: m.rows, scale: m.scale, draw_distance: i32::MAX, lod_trans: h.lod_trans, shine_distance, alpha: 0x80 };
                let Some(ac) = lv.anim.get(c.class) else { return (0, [[0.0; 3]; 3]) };
                let sphere = crate::moby_lod::world_sphere(&inp, crate::moby_lod::seq_sphere(ac, &c.state, h.bsphere));
                let v = crate::moby_lod::view_centre(sphere, eye, &cam);
                match crate::moby_lod::moby_proc_view(v, sphere[3], &inp, tans) {
                    Ok(p) if p.shine > 0 => (p.shine, crate::moby_lod::shine_basis(sphere, eye, &cam, &inp.rows)),
                    _ => (0, [[0.0; 3]; 3]),
                }
            });
            let look = moby_render::SlotLook { model, alpha: m.alpha, fading: false, mode: m.mode, glow: 0, shine, e, late: false, piece: m.piece };
            self.extra.show_slot(commands, level, slot, Some((class, look)), meshes, images, materials, buffers);
            shown.push(slot);
            if let Some(f) = mobys.skinned.get(&id).cloned().or_else(|| lv.anim.get(c.class).map(|ac| moby_anim::evaluate_with_snapshot(ac, &c.state, c.snapshot.as_ref()))) {
                let at = (slot * SLOT_JOINTS) as usize * 64;
                for (k, b) in f.iter().take(SLOT_JOINTS as usize).flat_map(|r| r.iter().flatten().flat_map(|v| v.to_le_bytes())).enumerate() {
                    if let Some(p) = palette.get_mut(at + k) { *p = b; }
                }
            }
            let lights = moby_lights(lv, m.rows, m.light, m.ambient);
            let rec = moby_render::extra_record(&model, lights.as_ref(), slot * SLOT_JOINTS);
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
