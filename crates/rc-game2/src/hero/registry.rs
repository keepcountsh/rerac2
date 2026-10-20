//! Going Commando's hero states: every id SetState (L00 `0x2c4560`) has a case for, the movement group its entry
//! writes to `0x18c0bc` (−1: the entry sets none, or sets it conditionally), the port module that owns it and
//! whether the port has it. A state without `ported` freezes the hero ([`super::HeroTick::Unported`]).
//!
//! The ids R&C1 also has keep R&C1's meaning where the code shows it (idle 0, walk 2, stop 3, crouch 4, fall 6, jump 7,
//! run jump 9, double jump 0xe; 0xb, R&C1's long jump, is R&C2's crouch flip); the rest are named by their group or left unnamed until ported.

/// The module that owns a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// `ground.rs`: idle 0, stop 3, crouch 4.
    Ground,
    /// `walk.rs`: walk / run 2.
    Walk,
    /// `air.rs`: fall 6.
    Air,
    /// `damage.rs`: the hurts and deaths (0x16, 0x8c, 0x39, 0x65, 0x67, 0x74, 0x78, 0x79, 0x7a, 0x7c, 0x7d, 0x7f).
    Damage,
    /// `ledge.rs`: the ledges (group 3: 0x17..0x1a).
    Ledge,
    /// `melee.rs`: the wrench (group 6: 0x13, 0x14, 0x15, 0x22, 0x83).
    Melee,
    /// `jump.rs`: the jump group (group 4, and 0x66 in 0x19).
    Jump,
    /// `pack.rs`: the glide 8 (group 5).
    Pack,
    /// `swing.rs`: the Swingshot (group 0xd: 0x23, 0x24, 0x25; the hook swing 0x2b; the pull 0x8a / 0x8b).
    Swing,
    /// `script.rs`: the states objects and scenes set (group 9 / 0x18, the scripted walk 0x62..0x64, the fidgets).
    Script,
    /// `clank.rs`: Clank's own body (`0x18c0d4` = 1: 0x3f..0x4e).
    Clank,
    /// `water.rs`: swimming (groups 0x11 / 0x12: 0x32..0x36) and the water hurts 0x72 / 0x73.
    Water,
    /// `misc.rs`: the stagger 0x26 / 0x2f, the slide 0x30, the mud 0x76, the bonk 0x77.
    Misc,
    /// Not ported yet.
    Unported,
}

/// One state of the machine.
#[derive(Clone, Copy, Debug)]
pub struct StateInfo {
    pub id: i32,
    /// The group SetState writes (−1: none or conditional).
    pub group: i32,
    pub module: Module,
    pub ported: bool,
}

const fn s(id: i32, group: i32, module: Module, ported: bool) -> StateInfo { StateInfo { id, group, module, ported } }
use Module::*;

/// Every state id with a SetState case on L00, in id order.
pub const STATES: &[StateInfo] = &[
    s(0x00, 0, Ground, true), s(0x01, 0, Ground, true), s(0x02, 1, Walk, true), s(0x03, 1, Ground, true),
    s(0x04, 0xc, Ground, true), s(0x06, 2, Air, true), s(0x07, 4, Jump, true), s(0x08, 5, Pack, true),
    s(0x09, 4, Jump, true), s(0x0a, 4, Jump, true), s(0x0b, 4, Jump, true), s(0x0c, 4, Jump, true),
    s(0x0d, 4, Jump, true), s(0x0e, 4, Jump, true), s(0x0f, 4, Jump, true), s(0x10, 4, Jump, true),
    s(0x11, 4, Jump, true), s(0x12, 4, Jump, true), s(0x13, 6, Melee, true), s(0x14, 6, Melee, true),
    s(0x15, 6, Melee, true), s(0x16, 7, Damage, true), s(0x17, 3, Ledge, true), s(0x18, 3, Ledge, true),
    s(0x19, 3, Ledge, true), s(0x1a, 3, Ledge, true), s(0x1b, 4, Jump, true), s(0x1c, 9, Script, true),
    s(0x1d, 0, Ground, true), s(0x1e, 9, Script, true), s(0x1f, -1, Melee, true), s(0x22, 6, Melee, true),
    s(0x23, 0xd, Swing, true), s(0x24, 0xd, Swing, true), s(0x25, 0xd, Swing, true), s(0x26, 8, Misc, true),
    s(0x27, 0xf, Unported, false), s(0x28, 0xf, Unported, false), s(0x29, 0xf, Unported, false), s(0x2a, 0xf, Unported, false),
    s(0x2b, 0xe, Swing, true), s(0x2c, 2, Air, true), s(0x2e, 1, Walk, true), s(0x2f, 8, Misc, true),
    s(0x30, 0x10, Misc, true), s(0x31, 9, Script, true), s(0x32, 0x11, Water, true), s(0x33, 0x11, Water, true),
    s(0x34, 0x11, Water, true), s(0x35, 0x12, Water, true), s(0x36, 0x12, Water, true), s(0x37, 9, Script, true),
    s(0x38, 4, Jump, true), s(0x39, 0x14, Damage, true), s(0x3c, -1, Script, true), s(0x3d, -1, Script, true), s(0x3e, 0xf, Unported, false),
    s(0x3f, 0, Clank, true), s(0x40, 1, Clank, true), s(0x41, 2, Clank, true), s(0x42, 7, Clank, true),
    s(0x43, 0x14, Clank, true), s(0x44, 2, Clank, true), s(0x45, 4, Jump, true), s(0x46, 3, Clank, true),
    s(0x47, 3, Clank, true), s(0x48, 4, Jump, true), s(0x49, 3, Clank, true), s(0x4a, 3, Clank, true),
    s(0x4b, 5, Clank, true), s(0x4d, -1, Clank, true), s(0x4e, 2, Clank, true), s(0x5b, 4, Jump, true),
    s(0x60, -1, Script, true), s(0x61, 0x18, Script, true), s(0x62, 1, Script, true), s(0x63, 1, Script, true),
    s(0x64, 0, Script, true), s(0x65, 0x19, Damage, true), s(0x66, 0x19, Jump, true), s(0x67, 0x14, Damage, true),
    s(0x6d, 6, Melee, true), s(0x6e, 4, Jump, true), s(0x6f, 9, Script, true), s(0x70, 1, Walk, true),
    s(0x71, 0x1a, Unported, false), s(0x72, -1, Water, true), s(0x73, 7, Water, true), s(0x74, 0x14, Damage, true),
    s(0x75, 9, Script, true), s(0x76, 2, Misc, true), s(0x77, 10, Misc, true), s(0x78, 0x19, Damage, true),
    s(0x79, 0x14, Damage, true), s(0x7a, 0x14, Damage, true), s(0x7b, 0x1c, Unported, false), s(0x7c, 0x14, Damage, true),
    s(0x7d, 0x14, Damage, true), s(0x7f, 0x14, Damage, true), s(0x81, 4, Jump, true), s(0x82, 0x1e, Unported, false),
    s(0x83, 6, Melee, true), s(0x85, 0x20, Unported, false), s(0x86, 0x20, Unported, false), s(0x89, 4, Jump, true),
    s(0x8a, 0x21, Swing, true), s(0x8b, 0x21, Swing, true), s(0x8c, 7, Damage, true), s(0x92, 4, Jump, true),
    s(0x93, 6, Melee, true),
];

pub fn info(id: i32) -> Option<&'static StateInfo> { STATES.iter().find(|s| s.id == id) }
pub fn module_of(id: i32) -> Module { info(id).map_or(Unported, |s| s.module) }
pub fn implemented(id: i32) -> bool { info(id).is_some_and(|s| s.ported) }
