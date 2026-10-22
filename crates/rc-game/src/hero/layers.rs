//! Ratchet's pose layers (nodes of his moby's `+0x60` list, which the evaluator blends over his own key), kept by
//! `0x2afc38` right after the item pass: the **holding layers** `0x2aede0` and the **face layer** `0x2aeb90`. They are
//! R&C1's `0x22e660` / `0x22e3a8` (rerac:rc_game::hero::weapons `hold_update` / `face_update`, docs/plan/hero_gameplay.md
//! §11) with R&C2's values; the nodes are R&C1's (crate::hero::anim::AnimLayer: `0x3122c8` / `0x311f78` are
//! `FUN_002641c0` / `FUN_00263f70`).
//!
//! **Holding** (`0x18acb0` / `0x18acb4`, Ratchet's joint lists 12 / 13 at `$gp−0x72f0`, his right and left arm): while
//! the held item's stance type (`0x18c0db`, its row's `+0x0c`) is set — list 13 only with 2 — and it is not tucked away
//! (`0x18c0df`), each plays **his current key from the holding class** 1 / 2 (`0x1d9e80[0x1da7e0[i + 1]]`: his first
//! 0x17 sequences with the arm holding the item; a key below 0x17 is read from it, `+6` / `+7`), so the arm and the item
//! stay level while he runs. Made at weight 0 and grown 0.25 a tick; given up 0.15 a tick when not wanted, 0.1 once
//! tucked away (for good, `+0x34`); 0.15 a tick to 0 while the firing arm's layer (`0x18acb8`) is out and not leaving
//! (`0x18acc4`). Its keys follow his (key A only when he is not between two sequences, `+0x42` = `+0x43`); when he
//! blends into another sequence while the layer is itself between two, it first finishes its own blend at 0.15 a tick
//! (`+0x30`). In the special poses of [`special`] it blends over 15 ticks to the holding class's sequence 0 and plays
//! it (speed 1), and on the way out back to his sequence at 0.1 a tick; a pose with an arm angle pitches both arms to it
//! (posture records 8 / 9, springs 0.05 / 0.3).
//!
//! **Face** (`0x18acc0`, list 0x18): with a reason, a node playing his own sequence (`0x3122c8(.., 8, 1)` on a change,
//! else its own advance) at a weight eased 0.05 a tick to 1 (0.7 / 0.8 for some); without, eased out and freed. The
//! reasons, the last winning: the enemy his head looks at (its health block's `+0x2c`: 1 → 0x72, 2 → 0x70, 3 → 0x6e,
//! else 0x72; 0.7 in the look stance), one health left (0x74 at 0.8, not in the sequences 0x51 / 0x7b / 0x57 when not
//! blending), the Lancer (items 0x1e / 0x1c / 0x1d) in use (0x6f at 0.8).
//!
//! Not ported: the firing arm's layers (`0x2af5e0` with a row's `+0x1c`: the Lancer has none), the swap face
//! (`0x18a01c`, the items 0xe / 0x1a / 0x1b / 0x2b / 0x2c), the tuck in two of the transitions' falls (`0x2cb650`).

use super::physics::approach;
use super::Hero;
use crate::hero::anim::{AnimCtl, AnimLayer};

/// `$gp−0x72f0`: the holding layers' joint lists; the face layer's.
pub const HOLD_LISTS: [u8; 2] = [12, 13];
pub const FACE_LIST: u8 = 0x18;
/// The holding classes' first sequences (`0x2aede0`: a key below it is read from the holding class).
pub const HOLD_SEQS: u8 = 0x17;

/// The nodes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Layers {
    /// `0x18acb0` / `0x18acb4`: the holding layers (`alt` 1 / 2: classes 1 / 2).
    pub hold: [Option<AnimLayer>; 2],
    /// `0x18acc0`: the face layer.
    pub face: Option<AnimLayer>,
    /// Nodes made (the list's order: a later node is linked in front).
    pub made: u32,
    /// The look-kind byte (`+0x2c` of its health block) of the enemy the head looks at (`0x18c2b0`), set by the item
    /// pass; 0 none or no kind.
    pub look: Option<u8>,
}

/// `0x2ae060(seq)`: his key-B sequences in which the holding layers leave his arms to a fixed pose (the holding class's
/// sequence 0), with the arm angle some give: 0x1c / 0x1d / 0x1f (−75° past key time 25), 0x31 / 0x32, 0x37 (−55°),
/// 0x47 / 0x48, 0x4d, 0x59, 0x66 / 0x67.
pub fn special(seq: u8, frame: f32) -> Option<f32> {
    match seq {
        0x1c | 0x1d | 0x1f => Some(if 25.0 < frame { -1.309 } else { 0.0 }),
        0x31 | 0x32 | 0x47 | 0x48 | 0x4d | 0x59 | 0x66 | 0x67 => Some(0.0),
        0x37 => Some(-0.959_931),
        _ => None,
    }
}

fn node(h: &mut Hero, list: u8) -> AnimLayer {
    h.layers.made += 1;
    AnimLayer { born: h.layers.made, ..AnimLayer::new(list) }
}

/// `0x2afc38`'s layers: the holding layers, then the face.
pub fn upkeep(h: &mut Hero, anim: &dyn AnimCtl) {
    hold_update(h, anim);
    face_update(h, anim);
}

/// `0x2aede0`.
pub fn hold_update(h: &mut Hero, anim: &dyn AnimCtl) {
    let v = anim.view();
    for i in 0..2 {
        let mut n = h.layers.hold[i];
        if n.is_none_or(|l| !l.kill) {
            let mut want = h.items.stance != 0 && (i == 0 || h.items.stance == 2);
            if h.items.tucked {
                want = false;
                if let Some(l) = n.as_mut() { l.kill = true; }
            }
            if want {
                if n.is_none() {
                    let mut l = node(h, HOLD_LISTS[i]);
                    l.weight = 0.0;
                    l.alt = i as u8 + 1;
                    n = Some(l);
                }
            } else if let Some(l) = n.as_mut() {
                approach(0.0, 0.15, &mut l.weight);
                if l.weight == 0.0 {
                    h.layers.hold[i] = None;
                    continue;
                }
            }
        } else if let Some(l) = n.as_mut() {
            approach(0.0, 0.1, &mut l.weight);
            if l.weight == 0.0 {
                h.layers.hold[i] = None;
                continue;
            }
        }
        let Some(mut l) = n else { continue };
        let steady = v.seq_a == v.seq_b;
        // The firing arm's layer (`0x18acb8`, not ported: none) would take the arm over.
        if !l.kill { approach(1.0, 0.25, &mut l.weight); }
        l.own_blend = !steady && v.blending() && l.seq_b != v.seq_b && l.seq_a != l.seq_b;
        let angle = special(v.seq_b, v.frame);
        if angle.is_none() && !l.special {
            if !l.own_blend {
                if steady { l.seq_a = v.seq_a; }
                l.seq_b = v.seq_b;
                if steady { l.frame_a = v.frame_a; }
                l.frame_b = v.frame_b;
                l.t = v.t;
            } else {
                l.t += 0.15;
                if 1.0 <= l.t {
                    l.seq_a = l.seq_b;
                    l.frame_a = l.frame_b;
                    l.own_blend = false;
                }
            }
        } else {
            if let Some(a) = angle.filter(|&a| a != 0.0) {
                for r in [8, 9] {
                    let rec = &mut h.posture.recs[r];
                    rec.set_spring(0.05, 0.3);
                    rec.target[1] = a;
                }
            }
            if angle.is_none() {
                // Out of the special poses: back to his key B from frame 0 over 10 ticks.
                if !l.back {
                    l.seq_b = v.seq_b;
                    l.frame_b = 0;
                    l.back = true;
                    l.t = 0.0;
                    l.speed = 0.0;
                } else {
                    l.t = (l.t + 0.1).min(1.0);
                    if l.t == 1.0 { l.special = false; }
                }
            } else {
                if !l.special || l.seq_b != 0 {
                    if l.seq_a == l.seq_b || 0.5 < l.t {
                        l.seq_a = l.seq_b;
                        l.frame_a = l.frame_b;
                    }
                    l.t = 0.0;
                    l.rate = 1.0 / 15.0;
                    l.seq_b = 0;
                    l.frame_b = 0;
                    l.speed = 1.0;
                    l.special = true;
                    l.back = false;
                }
                l.speed = 1.0;
            }
        }
        l.advance(anim);
        h.layers.hold[i] = Some(l);
    }
}

/// `0x2aeb38(kind)`: the glare's sequence.
fn glare(kind: u8) -> u8 {
    match kind { 2 => 0x70, 3 => 0x6e, _ => 0x72 }
}

/// `0x2aeb90`.
pub fn face_update(h: &mut Hero, anim: &dyn AnimCtl) {
    let v = anim.view();
    let mut want = None;
    if let Some(kind) = h.layers.look {
        let mut w = 1.0;
        let seq = if kind != 0 {
            if h.group == 1 { w = 0.7; }
            glare(kind)
        } else {
            0x72
        };
        want = Some((seq, w));
    }
    if h.hp == 1 {
        let keep_off = matches!(v.seq_b, 0x51 | 0x7b | 0x57) && !v.blending();
        want = if keep_off { None } else { Some((0x74, 0.8)) };
    }
    if matches!(h.items.item, 0x1e | 0x1c | 0x1d) && h.items.ready == 2 && h.items.using { want = Some((0x6f, 0.8)); }
    match want {
        Some((seq, w)) => {
            let mut l = match h.layers.face {
                Some(l) => l,
                None => {
                    let mut l = node(h, FACE_LIST);
                    l.weight = 0.0;
                    l
                }
            };
            approach(w, 0.05, &mut l.weight);
            if l.seq_b == seq {
                l.advance(anim);
            } else {
                l.start(anim, seq, 0, 8, true);
            }
            h.layers.face = Some(l);
        }
        None => {
            if let Some(mut l) = h.layers.face {
                approach(0.0, 0.05, &mut l.weight);
                h.layers.face = if l.weight == 0.0 { None } else { Some(l) };
            }
        }
    }
}
