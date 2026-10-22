//! Ratchet's idle behaviour (docs/plan/player_controller.md §13 "Idle behaviour"): the fidget chance
//! `0x241e00` and its caller in the idle transitions, the head look `0x22b928`, the idle secondaries
//! `0x22bdd0`, Ratchet's blink (`0x227590` → `0x2274e8`), Clank's glow and blink `0x2278c0`, the joint
//! springs of the records those write (`0x2273d0` → `0x227050`), and the back items (the pack and Clank)
//! as far as the idle code drives them: the sequence table the hero's animation calls draw from
//! (`0x2476d0`, called from `SetAnim`'s `0x247550` and the advance's `0x247800`), Clank's fidget
//! `0x2473e0`, and their creation / advance in `HeroItemsUpdate` (`0x22f3c0` / `0x22fec0`).
//!
//! Every routine here draws from the game's one `rand` stream in the game's order within the hero update
//! `0x228870`:
//! 1. advance `0x247d48` → `0x247800`: one `rand_range(0, n − 1)` per tick while Ratchet's sequence has `n > 0`
//!    rows in the back table (sequence 0 has 3 on Novalis: one draw per idle tick);
//! 2. the idle transitions: `SetAnim` → `0x247550` (one draw when the new sequence has rows, e.g. every blend
//!    back to sequence 0), the fidget chance (one `randf` per tick in sequence 0 once the 120-tick cooldown is
//!    over and a record is available), Clank's fidget (`rand_range(110, 270)` + one table draw);
//! 3. the head look: in sequence 1/2 one `rand_range(40, 70)` per tick, in sequence 0 four draws per expiry;
//! 4. the secondaries (sequence 0 only): 2, 2, 2, 3 draws per expiry of their four timers;
//! 5. Ratchet's blink: two `randi(period)` per blink; Clank's blink: one `rand_range(50, 200)` per blink.
//!
//! Standard floats (`f32`) except where an existing PS2-float helper of the hero code is reused (the joint
//! springs `turn_spring`, `fast_sin`); the random helpers are [`crate::rng`]'s.
//!
//! The edge look-down branch of `0x22b928` reads the edge probe `Hero::edge_probe`. The other producers the hero update
//! runs around these (the feet `0x22c5c0`, `HeroScanTargets` 0x22c080, the Magneboots lean `0x2352e0`) are
//! `super::pose`. The cheats (`rerac:rc_game::cheats`, [`Hero::cheats`]): 0x15edb1 (record 17's scale 1.57), 0x15edb3 (0x15ee18 → 1.8,
//! record 18 = 0x15ee18). Not ported: the cheat 0x15edb5's mirror (below), Clank hidden (0x141628, 0 on Novalis), the
//! hit flash 0x13f53e in the glow, and the sound triggers of the advances (`PlayClassSound` pitch draws, the sound layer).
//!
//! **The joint modifiers** (Ratchet's moby +0x64, docs/plan/hero_gameplay.md §7): the records the springs update are
//! linked into [`Idle::manips`] while active (`AttachManipulator` / `DetachManipulator`), with the eyelid nodes of the
//! blink ([`BLINK_NODES`]); [`Idle::modifiers`] is the list as the evaluator reads it (written to his moby by the
//! write-back). The walk / air lean `HeroLean` 0x235638 is [`Hero::lean`].
#![allow(clippy::neg_cmp_op_on_partial_ord)] // compare semantics spelled out as in the game.

/// `FUN_00221e38(a, out, axis)`: the rotation quaternion about axis 0 / 1 / 2 by `−a` as the game builds it from the
/// full angle: `c = 2·cos a`, `s = −sin a`, `k = sqrt(|c| + 2)`; `c ≥ 0`: `w = k/2`, `v = s/k`; `c < 0`: `v = k/2`,
/// `w = s/k` (so `v ≥ 0` there); `v` on the axis.
pub(crate) fn axis_quat(a: f32, axis: usize) -> [f32; 4] {
    let (c, s) = (2.0 * a.cos(), -a.sin());
    let k = (c.abs() + 2.0).sqrt();
    let (v, w) = if c < 0.0 { (0.5 * k, s / k) } else { (s / k, 0.5 * k) };
    let mut q = [0.0, 0.0, 0.0, w];
    q[axis] = v;
    q
}

/// `FUN_0026ee30(out, e)`: Euler angles → quaternion `(q_x(e.x) ⊗ q_y(e.y)) ⊗ q_z(e.z)` ([`axis_quat`]).
pub(crate) fn euler_quat(e: [f32; 3]) -> [f32; 4] {
    use rc_formats::moby_anim::quat_product;
    quat_product(quat_product(axis_quat(e[0], 0), axis_quat(e[1], 1)), axis_quat(e[2], 2))
}

// ------------------------------------------------------------------------------------------------
// The back items.

// ------------------------------------------------------------------------------------------------

