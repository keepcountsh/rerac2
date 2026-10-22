//! The in-engine scene player (game mode 2): docs/plan/cutscenes_transitions.md §3. Ported from level01
//! `DialogStreamStart` 0x2ac330 (start), `CutsceneModeUpdate` 0x2aca80 (tick), `FUN_002ac8d8` (camera
//! record), `FUN_00259288` (chunk parse → actors), `FUN_002ac608` (end) and the subtitle draw
//! `fun_001f4be0` (0x21b620); data from [`rc_formats::scene`].
//!
//! One [`ScenePlayer::tick`] = one 60 Hz frame (vsync). The game's blocking steps are spelled out as frames:
//!
//! | frames | what | world |
//! |---|---|---|
//! | 6 | `FadeToBlack(ticks(6))`: black quads of alpha `0x80 − i·0x80/6` (i = 5..0) drawn over the frozen last image | frozen |
//! | 1 | the fade's closing vsync (black) | frozen |
//! | 3 | the speech stream starts ([`AudioRequest::Speech`]), `0x16cd28 = −3`, 3 vsync waits (black) | frozen |
//! | 1 | the rest of the triggering gameplay frame; its render is skipped (0x15f5d8 = 1): black | runs |
//! | end − 1 | scene ticks 1 .. end − 1 (`CutsceneModeUpdate`) | runs |
//! | 1 | tick = end → `FUN_002ac608`: the first vsync of `FadeToBlack(12)` shows the last image | frozen |
//! | 12 + 1 | `FadeToBlack(12)` and its closing vsync; then mode 0 ([`SceneEnd`]) | frozen |
//!
//! The frame accounting of the blocking loops (one vsync per `func_0x00122298(0)`) is read from the code,
//! not traced [M]. Novalis scene 5: 11 frames before tick 1, 1507 scene frames, 14 after: 1532 frames.
//!
//! **Per tick** (0x2aca80, in order): fade 0x15f3fc −= 0.34 (clamped at 0 after the store); scene tick and
//! chunk tick +1; end when tick ≥ end tick; skip (only when tick ≥ `ticks(18)` and the fade is 0): Start
//! (pressed & 0x800) alone when the game is beaten (0x15eea0), completed (0x15ee20), replayed from the menu
//! (0x15eed8) or on level 0, else Start plus L1 L2 R1 R2 held (0x13cae0 & 0xf); chunk rollover when the
//! chunk tick reaches 96 (80 PAL): parse the next chunk (chunk tick = 0, every actor's sequence slot
//! re-pointed); camera from record `chunk tick` (no interpolation); actors: key frames `f = chunk tick >> 1`
//! and `f + 1`, blend `t = (chunk tick & 1)·0.5`, or 1.0 on an odd tick whose camera record has the cut
//! flag; position = `(1 − t)·track[f] + t·track[f + 1]` (`VecScale`, `VecScale`, `VecAdd`).
//!
//! **Camera** (`FUN_002ac8d8`): `M = I`, then `sceVu0RotMatrixX(M, M, a)` (0x125360, angle +0x10),
//! `sceVu0RotMatrixY` (0x125408, +0x14), `sceVu0RotMatrixZ` (0x1252b8, +0x18; unnamed in Lombyte, same SDK
//! code). Each routine left-multiplies every stored row q_i of M by the rotation (`q_i ← R·q_i`, one VU
//! `vmula/vmadda/vmadd` chain), so starting from I the stored rows are the **columns of
//! Rz(c)·Ry(b)·Rx(a)** (X applied first). The camera rows are forward 0x167450 = −q2, left 0x167460 = −q0,
//! up 0x167470 = q1; the eye is copied to 0x167240 and tan(hfov/2) to 0x16cf70. Sine and cosine come from
//! `sceVu0ECosSin` (0x125240): cos = the 9th-order sine polynomial (coefficients at boot 0x132e00) of
//! π/2 − |θ|, sin = ±√(1 − cos²) with the sign of θ. All of it runs on the PS2 float model
//! ([`crate::ps2v::Pf`]), so the rows are the game's bit patterns.
//!
//! **End** (0x2ac608): speech stop, `FadeToBlack(12)`, mode 0, tan(hfov/2) = 0.63 (0x3f2147ae), fade 0,
//! actors deleted (their class slots freed), hero `SetState(0, 1)` and the ground snap (`GroundHeight(0.5,
//! hero)`: accepted when > 2 and within 4.5 of the hero z), hero items shown again (`FUN_002487a8`), the
//! optional teleport (0x16cd26, written only by `TalkingNpcUpdate` via 0x2783a8: in front of the NPC, facing
//! it), the NPC dialog continues, and `FUN_0027a460(ticks(30))`: music resumes 30 ticks later.
//!
//! Not modelled here (engine / other ports): the mirror cheat's left-row cross product and the 0x15edb0
//! FOV cheat scale, mode 6 space scenes (same chunks, ship-local transform). The world update of the scene frame
//! (the moby loop, the hero, the particles), the actor mobys, classes 74 / 203 (mode bit 0x80, [`SCENE_HIDDEN_CLASSES`])
//! and the state the classes read ([`SceneState`], e.g. the FX driver 1546) are the engine's (`rc-engine`
//! `scene_render`).

use crate::ps2v::Pf;
use rc_formats::scene::CamRecord;

/// `sceVu0ECosSin` polynomial coefficients (boot 0x132e00, x y z w = x⁹ x⁷ x⁵ x³ terms).
const SIN_COEF: [Pf; 4] = [Pf::b(0x362e_9c14), Pf::b(0xb94f_b21f), Pf::b(0x3c08_873e), Pf::b(0xbe2a_aaa4)];
/// π/2 as the rotation routines load it (`lui 0x3fc9; ori 0x0fdb`).
const HALF_PI: Pf = Pf::b(0x3fc9_0fdb);

/// The camera a scene frame uses (the play camera consumes it while a scene runs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCamera {
    /// 0x167240.
    pub eye: [f32; 3],
    /// 0x167450 / 0x167460 / 0x167470: forward, left, up (game space).
    pub rows: [[f32; 3]; 3],
    /// 0x16cf70.
    pub tan_half_fov: f32,
    /// The record's cut flag.
    pub cut: bool,
}

/// `sceVu0ECosSin` as the rotation routines call it: (sin θ, cos θ).
pub(crate) fn ecossin(theta: Pf) -> (Pf, Pf) {
    let neg = theta < Pf::ZERO;
    let x = if neg { HALF_PI + theta } else { HALF_PI - theta };
    let x2 = x * x;
    // vf8 = coef·x, then ·x² per power; the terms are added highest power last.
    let c = SIN_COEF.map(|k| k * x);
    let c = c.map(|v| v * x2);
    let w3 = c[3];
    let c = [c[0] * x2, c[1] * x2, c[2] * x2];
    let mut s = (Pf::ZERO + x) + w3;
    let z5 = c[2];
    let c = [c[0] * x2, c[1] * x2];
    s += z5;
    let y7 = c[1];
    let x9 = c[0] * x2;
    s += y7;
    s += x9;
    let cos = Pf::ZERO + s;
    let q = (Pf::ONE - cos * cos).sqrt();
    let sin_mag = Pf::ZERO + q;
    let sin = if neg { Pf::ZERO - sin_mag } else { Pf::ZERO + sin_mag };
    (sin, cos)
}

type Q = [Pf; 4];

/// One `R · q` (vmulax / vmadday / vmaddaz / vmaddw).
fn apply(r: &[Q; 4], q: Q) -> Q {
    std::array::from_fn(|k| {
        let acc = r[0][k] * q[0];
        let acc = acc + r[1][k] * q[1];
        let acc = acc + r[2][k] * q[2];
        acc + r[3][k] * q[3]
    })
}

/// The rotation "columns" (vf6, vf7, vf8, vf9) of `sceVu0RotMatrixX/Y/Z` for (sin, cos).
fn rot(axis: usize, s: Pf, c: Pf) -> [Q; 4] {
    let (z, o) = (Pf::ZERO, Pf::ONE);
    let (ps, ms, pc) = (z + s, z - s, z + c);
    match axis {
        0 => [[o, z, z, z], [z, pc, ps, z], [z, ms, pc, z], [z, z, z, o]],
        1 => [[pc, z, ms, z], [z, o, z, z], [ps, z, pc, z], [z, z, z, o]],
        _ => [[pc, ps, z, z], [ms, pc, z, z], [z, z, o, z], [z, z, z, o]],
    }
}

/// `FUN_002ac8d8`'s camera from a record (module docs).
pub(crate) fn scene_camera(r: &CamRecord) -> SceneCamera {
    let (z, o) = (Pf::ZERO, Pf::ONE);
    let mut m: [Q; 4] = [[o, z, z, z], [z, o, z, z], [z, z, o, z], [z, z, z, o]];
    for (axis, &a) in r.angles.iter().enumerate() {
        let (s, c) = ecossin(Pf::f(a));
        let rm = rot(axis, s, c);
        m = m.map(|q| apply(&rm, q));
    }
    let v = |q: Q, neg: bool| [0, 1, 2].map(|k| if neg { (-q[k]).to_f32() } else { q[k].to_f32() });
    SceneCamera { eye: r.eye, rows: [v(m[2], true), v(m[0], true), v(m[1], false)], tan_half_fov: r.tan_half_fov, cut: r.is_cut() }
}

// ---------------------------------------------------------------------------------------------------
// What the rest of the game sees of a running scene

