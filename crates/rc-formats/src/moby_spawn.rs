//! Moby state at the end of level load: what the per-class init code has done to each moby before the
//! first frame is drawn, without porting the update functions. Spec: docs/plan/moby_update_catalogue.md
//! (§1 mechanism, §4 top classes, §6 "Sequence at spawn"; "In the port" at the end).
//!
//! Mechanism (level01 addresses; the class → update table is the level overlay's at 0x20bb00):
//! * `InitMobyInstance` 0x263488 zeroes the moby (sequence 0, frame 0/0, t 0, speed = rate = 1; a class
//!   with one sequence of ≤ 1 frame gets speed 0, plus mode 0x40 when its loop-sound byte has bit 7 set:
//!   [`AnimState::spawn`]).
//! * The loader `FUN_00255958` then creates the player's ship (`SHIP_CLASSES[ship]`, see
//!   [`first_visit_ship`]; its class comes from the `spaceships` file, [`parse_spaceship`], unless the
//!   level core has it) with `CreateMoby` and hard-cuts it to sequence 1 (`fun_00212ed8(ship, 1, 0)`),
//!   see [`ship_state`].
//! * `FUN_002792d0` (called once from `FUN_00258128`) runs every moby with state ≥ 0 and no mode bit 2,
//!   with no distance gate: `MobyAnimAdvance` (unless mode 0x40), then the class update, then the matrix.
//!   Every update's state-0 branch (its "init") therefore runs before the first frame.
//!
//! [`initial_state`] reproduces those init branches for the classes whose init changes something visible
//! (hidden, sequence, position). The rules are those of the Novalis (level 01) table; the same class id is
//! assumed to run the same code in other levels (true for the shared clusters in the catalogue, not checked
//! per level). Not modelled: the random mirror bit of 577 (`rand() & 1` → mode 0x8000) and every later frame's
//! behaviour. The instance loop's spawn test is [`spawn_test`] (which records the loader creates, and the
//! instance → moby map [`instance_to_moby`]); the ship's arrival hide is [`ship_hidden_on_arrival`].

use crate::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};

/// When the sequence change of a [`SpawnState`] happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeAt {
    /// At creation, before the load pass (the ship: the loader's own `fun_00212ed8`).
    Create,
    /// In the load pass, after its `MobyAnimAdvance` (the class init branch).
    LoadPass,
    /// In the first update that runs after load (the moby's second update: the load pass only moved it to
    /// the state that makes the change). In the game that is the first frame the moby is active
    /// (`fun_0020d868`: drawn last frame, within its update distance of the camera, or group-activated).
    FirstUpdate,
}

/// The moby's state after the load pass, as far as drawing is concerned.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnState {
    /// Mode bit 1 is set after the load pass: `MobyProc` skips the moby (`mode & 0x81`). Also true when
    /// the init deleted the moby.
    pub(crate) hidden: bool,
    /// The init called `DeleteMoby` (state 0xfd/0xfe: never drawn or updated again).
    pub(crate) deleted: bool,
    /// Sequence change: target sequence, frame, and `Some(ticks)` for a blend `fun_00212f90(seq, frame,
    /// ticks)` or `None` for a hard cut `fun_00212ed8(seq, frame)`; `change_at` = None means no change (the
    /// moby keeps playing sequence 0 from its spawn state).
    pub(crate) sequence: u8,
    pub(crate) frame: u8,
    pub blend_ticks: Option<u8>,
    pub(crate) change_at: Option<ChangeAt>,
    /// Speed (moby+0x58) written by the init after the change, if any (the hidden amoeboids: 0).
    pub(crate) speed: Option<f32>,
    /// Mode bits the init sets (moby+0x34 |=): 1 hidden, 0x40 no advance (0x41 = the "hide and freeze"
    /// idiom), 0x1000 targetable. Includes 0x40 for the static classes of [`AnimState::spawn`].
    pub(crate) mode_bits: u16,
    /// Mode bits the init clears (the hidden amoeboids clear 0x1000; the ship clears 2).
    pub(crate) mode_clear: u16,
    /// New position (moby+0x10) when the init moves the moby.
    pub(crate) position: Option<[f32; 3]>,
    /// The load pass runs this moby (advance + update): false only for Ratchet (class 0), whose hero init
    /// sets mode 2 and whose class has no sequence 0 on the disc.
    pub(crate) runs_load_pass: bool,
}

impl SpawnState {

    /// Applies this state's sequence change to `s` (with `snap` the moby's snapshot frame, see
    /// [`moby_anim::set_sequence`]). Used for [`ChangeAt::FirstUpdate`] by the caller's first active tick,
    /// and by [`SpawnState::anim_after_load`] for the other two. The guards of the game code
    /// (`if (m+0x53 != seq)`) are applied: no change when seq B is already the target.
    pub fn apply_change(&self, s: &mut AnimState, class: &MobyAnimClass, snap: &mut Option<MobyFrame>) -> bool {
        if self.change_at.is_none() || (s.seq_b == self.sequence && self.blend_ticks.is_some()) { return false; }
        match self.blend_ticks {
            None => moby_anim::hard_cut(s, class, self.sequence, self.frame as i32),
            Some(n) => moby_anim::set_sequence(s, class, self.sequence, self.frame as i32, n as i32, snap),
        }
    }

}

// ---------------------------------------------------------------------------------------------------
// The loader's spawn test (`FUN_00255958`, the instance loop at 0x256890..0x256a58, level01)
