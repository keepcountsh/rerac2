//! The class updates the port has, by o_class: the level's update table (L00 `0x2a2e80`, records `{o_class,
//! update, 0x2a3978}` up to o_class −1, written to the slot table `0x1da240` by `0x32e1f8`) as Rust ports.

pub mod area_switch;
pub mod beast;
pub mod bolt;
pub mod carriers;
pub mod chute;
pub mod crate_;
pub mod dynamo;
pub mod flower;
pub mod flyer;
pub mod enemies;
pub mod gate;
pub mod gravity_bomb;
pub mod lancer;
pub mod leaper;
pub mod oozla;
pub mod nanotech;
pub mod pickup;
pub mod props;
pub mod spitter;
pub mod shot;
pub mod spawn_point;
pub mod tie_lights;
pub mod tractor;
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
        0x0102 => Some(area_switch::update),
        0x08fb => Some(oozla::update_08fb),
        0x0f14 => Some(oozla::update_0f14),
        0x0a6a | 0x0a6b | 0x0aa0 | 0x0aa1 => Some(oozla::update_mover),
        0x0c58 => Some(oozla::update_0c58),
        0x0ca0 => Some(oozla::update_0ca0),
        0x046f => Some(oozla::update_046f),
        0x0a17 => Some(oozla::update_0a17),
        0x0a92 | 0x0a93 => Some(oozla::update_float),
        0x09a2 => Some(leaper::update),
        0x0b2b => Some(spitter::update),
        0x0a49 => Some(flyer::update),
        0x09fc => Some(beast::update),
        0x0a53 => Some(oozla::update_0a53),
        0x0a56 => Some(oozla::update_0a56),
        0x0a8d => Some(flower::update_arm),
        0x0a8e => Some(flower::update_core),
        0x0b1a => Some(flower::update_0b1a),
        0x0cb5 => Some(oozla::update_0cb5),
        0x0936 | 0x08fa | 0x0a00 => Some(oozla::update_dynamo_platform),
        0x0aae => Some(oozla::update_0aae),
        0x0a1e => Some(oozla::update_0a1e),
        0x02e1 => Some(oozla::update_02e1),
        0x0a9d | 0x0aa7 => Some(oozla::update_gate),
        0x0a9e | 0x0a9f => Some(oozla::update_sent),
        0x0a95 => Some(oozla::update_0a95),
        0x0a7b => Some(oozla::update_0a7b),
        0x0825 => Some(dynamo::update),
        0x00bc => Some(tractor::update),
        0x0acb | 0x0acc => Some(tractor::update_box),
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
