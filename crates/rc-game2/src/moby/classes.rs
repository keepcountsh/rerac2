//! The class updates the port has, by o_class: the level's update table (L00 `0x2a2e80`, records `{o_class,
//! update, 0x2a3978}` up to o_class −1, written to the slot table `0x1da240` by `0x32e1f8`) as Rust ports.

pub mod bolt;
pub mod crate_;
pub mod wrench;

use super::UpdateFn;

/// The port of `o_class`'s update (`None`: not ported, the moby gets mode 2 like a class without one).
pub fn update_for(o_class: i16) -> Option<UpdateFn> {
    match o_class {
        0x47 => Some(wrench::update),
        500 | 501 | 511 => Some(crate_::update),
        13..=19 => Some(bolt::update),
        _ => None,
    }
}

/// The Swingshot nodes (class 0x2f6) for the hero's search `0x2c4098`: shown ones (mode 0x1000) with their pvars
/// `+0x68`[0] in use, [4] radius, [8] kind, [0x20] range.
pub fn swing_nodes(w: &super::MobyWorld) -> Vec<crate::hero::swing::Node> {
    w.mobys
        .iter()
        .enumerate()
        .filter(|(_, m)| m.o_class == 0x2f6 && m.state < 0x80 && m.mode & 0x1000 != 0)
        .map(|(id, m)| crate::hero::swing::Node {
            id,
            pos: m.pos,
            busy: m.pv_i(0) != 0,
            radius: m.pv_f(4),
            kind: m.pv_f(8),
            range: m.pv_f(0x20),
            drawn: m.drawn != 0,
            hidden: m.mode & 1 != 0,
        })
        .collect()
}
