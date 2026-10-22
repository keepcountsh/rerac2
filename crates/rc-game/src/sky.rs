//! The sky's turning shells (L00 `0x35dfa0` → `0x35ded0`, from the frame's sky draw `0x2fd348`): each of the first ten
//! shells has Euler angles (`0x28a7f0`, 12 bytes a shell) that gain its speeds (the table `0x1b1c50` points at) every
//! frame, its matrix built from them (`0x2ef6f8`); shells from the tenth on are drawn unturned.
//!
//! The speeds are the level's. With the planet index `0x1a79f8` zero (Aranos) `0x2fd238` takes the table `0x27e508`
//! and sets each shell's z speed to its base (`0x27e490`'s z: 0.0005, 0.001 and 0.002 for shells 2, 3 and 5) × −(the
//! view's forward · (1, 0, 0)): the cloud shells turn by how far the view looks down the ship's x axis, which reads as
//! the ship flying on. Not ported: the other planets' tables (`0x27e490` as is), the mode-2 axis (0, −1, 0).

use crate::hero::physics::{add_rot, V3};

/// The shells that turn.
pub const SHELLS: usize = 10;
/// `0x27e490`'s z speeds (radians a frame): Aranos's bases.
const ARANOS_BASE_Z: [f32; SHELLS] = [0.0, 0.0, 0.0005, 0.001, 0.0, 0.002, 0.0, 0.0, 0.0, 0.0];

/// `0x28a7f0`: the shells' angles (x, y, z).
#[derive(Clone, Copy, Debug, Default)]
pub struct SkySpin {
    pub angles: [V3; SHELLS],
}

impl SkySpin {
    /// `0x2fd348` on Aranos (`0x2fd238`, then `0x35dfa0`'s step): the speeds from the view's forward `fwd`, each angle
    /// stepped by its speed.
    pub fn frame(&mut self, fwd: V3) {
        let k = -fwd[0];
        for (a, base) in self.angles.iter_mut().zip(ARANOS_BASE_Z) {
            a[2] = add_rot(a[2], base * k);
        }
    }
}
