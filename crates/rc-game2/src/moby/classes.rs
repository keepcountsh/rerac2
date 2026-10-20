//! The class updates the port has, by o_class: the level's update table (L00 `0x2a2e80`, records `{o_class,
//! update, 0x2a3978}` up to o_class −1, written to the slot table `0x1da240` by `0x32e1f8`) as Rust ports.

pub mod bolt;
pub mod crate_;
pub mod props;
pub mod wrench;

use super::UpdateFn;

/// The port of `o_class`'s update (`None`: not ported, the moby gets mode 2 like a class without one).
pub fn update_for(o_class: i16) -> Option<UpdateFn> {
    match o_class {
        0x47 => Some(wrench::update),
        500 | 501 | 511 => Some(crate_::update),
        13..=19 => Some(bolt::update),
        0x12f5 => Some(props::update_12f5),
        0x0c8b => Some(props::update_0c8b),
        0x0ac3 => Some(props::update_0ac3),
        0x0ac5 => Some(props::update_0ac5),
        0x0ac6 => Some(props::update_0ac6),
        0x0c4c => Some(props::update_0c4c),
        0x0c63 => Some(props::update_0c63),
        0x0ac8 => Some(props::update_0ac8),
        0x0d1e => Some(props::update_0d1e),
        _ => None,
    }
}

/// The Swingshot's targets for the hero's searches: the nodes (class 0x2f6, `0x2c4098`: shown ones, mode 0x1000), the
/// hook nodes (0x323, `0x2c3e30`) and the pull targets (0x99d, `0x2c4340`), with their pvars `+0x68`.
pub fn swing_nodes(w: &super::MobyWorld, class: i16) -> Vec<crate::hero::swing::Node> {
    w.mobys
        .iter()
        .enumerate()
        .filter(|(_, m)| m.o_class == class && m.state < 0x80 && (class != 0x2f6 || m.mode & 0x1000 != 0))
        .map(|(id, m)| crate::hero::swing::Node {
            id,
            pos: m.pos,
            busy: m.pv_i(0) != 0,
            radius: m.pv_f(4),
            kind: m.pv_f(8),
            range: m.pv_f(0x20),
            drawn: m.drawn != 0,
            hidden: m.mode & 1 != 0,
            cap: m.pv_f(0x10),
            spring: [m.pv_f(0x24), m.pv_f(0x28), m.pv_f(0x2c)],
            keep_yaw: m.pv_i(0x34) != 0,
            yaw: m.rot[2],
        })
        .collect()
}
