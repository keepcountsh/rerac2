//! The class updates the port has, by o_class: the level's update table (L00 `0x2a2e80`, records `{o_class,
//! update, 0x2a3978}` up to o_class −1, written to the slot table `0x1da240` by `0x32e1f8`) as Rust ports.

pub mod bolt;
pub mod carriers;
pub mod chute;
pub mod crate_;
pub mod debris;
pub mod enemies;
pub mod gate;
pub mod gravity_bomb;
pub mod lancer;
pub mod light;
pub mod nanotech;
pub mod pickup;
pub mod props;
pub mod shot;
pub mod spawn_point;
pub mod tie_lights;
pub mod wrench;

use super::UpdateFn;

/// The port of `o_class`'s update (`None`: not ported, the moby gets mode 2 like a class without one).
pub fn update_for(o_class: i16) -> Option<UpdateFn> {
    match o_class {
        0x47 => Some(wrench::update),
        0x5d2 => Some(lancer::update),
        gravity_bomb::HAND_CLASS => Some(gravity_bomb::update),
        gravity_bomb::BOMB_CLASS => Some(gravity_bomb::update_bomb),
        gravity_bomb::BLAST_CLASS => Some(gravity_bomb::update_blast),
        0xbfe => Some(shot::update),
        500 | 501 | 511 => Some(crate_::update),
        13..=19 => Some(bolt::update),
        0x12f5 => Some(props::update_12f5),
        0x0c8b => Some(props::update_0c8b),
        0x0ac0 => Some(props::update_0ac0),
        0x099c => Some(enemies::update_099c),
        0x0b0a => Some(enemies::update_0b0a),
        0x0b0b => Some(enemies::update_0b0b),
        0x0b41 | 0x0b42 | 0x0b45 | 0x0b4d | 0x0b51 | 0x0b52 | 0x0b53 | 0x0c02 | 0x0c3f => Some(props::update_stub),
        0x0f6e => Some(props::update_0f6e),
        0x0ddf => Some(props::update_0ddf),
        0x0a38 => Some(carriers::update_0a38),
        0x09c7 => Some(carriers::update_09c7),
        0x0c66 => Some(carriers::update_0c66),
        0x0ac1 => Some(props::update_0ac1),
        0x0ac2 => Some(props::update_0ac2),
        0x0aca => Some(props::update_0aca),
        0x0adb => Some(props::update_0adb),
        0x0ac4 => Some(props::update_0ac4),
        0x1330 => Some(props::update_1330),
        0x0f13 => Some(props::update_0f13),
        0x0ac9 | 0x0c0a..=0x0c0d | 0x0e01 | 0x0e02 => Some(props::update_wreck),
        0x0ac3 => Some(props::update_0ac3),
        0x0ac5 => Some(props::update_0ac5),
        0x0ac6 => Some(props::update_0ac6),
        0x0c4c => Some(props::update_0c4c),
        0x0c63 => Some(props::update_0c63),
        0x0c5c => Some(props::update_0c5c),
        0x0cda => Some(props::update_0cda),
        0x0ce9 => Some(tie_lights::update),
        0x0340 => Some(props::update_0340),
        0x0ac8 => Some(props::update_0ac8),
        0x0d1e => Some(props::update_0d1e),
        0x12f9 => Some(props::update_12f9),
        0x12fa | 0x1143 => Some(props::update_breakable),
        0x132d => Some(props::update_132d),
        0x0919 => Some(props::update_0919),
        0x0c8c => Some(props::update_0c8c),
        0x0325 => Some(spawn_point::update),
        0x02a7 => Some(chute::update),
        0x0ac7 => Some(gate::update),
        0x0c22 => Some(props::update_0c22),
        pickup::CLASS => Some(pickup::update),
        nanotech::CLASS => Some(nanotech::update),
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
