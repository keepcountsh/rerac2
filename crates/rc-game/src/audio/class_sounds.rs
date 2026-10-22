//! The audio layer on the game's tick (docs/plan/audio.md §3.1, §3.2): the class sounds the gameplay code plays
//! (`PlayClassSound` level01 0x2a1618 / 0x2a16c0) and the per-tick sound step, all on the game's one `rand`
//! stream ([`crate::base::tick::Game::rng`]) at the game's points:
//!
//! * **Moby class sounds** (crates, bolts, platforms, …): the moby loop calls `PlayClassSound` through
//!   [`crate::base::moby_update::services::SoundSink`]; [`ClassSoundSink`] allocates the slot at once
//!   ([`AudioSystem::play_class_sound`], `SoundSlotAlloc` 0x2a13a0), so its pitch-bend `randi` lands in the moby
//!   order. The listener is the camera 0x167240 as the previous tick's camera update left it.
//! * **Ratchet's animation triggers**: `RatchetAnimAdvance` 0x247d48 plays the class sound of the first trigger
//!   whose time its key time passed this tick ([`ratchet_trigger`]), inside the hero update: [`HeroClassSounds`]
//!   is the hero's [`HeroSounds`] (`Game::tick_with_hero_sounds` → `hero_update_with_sounds` calls it right
//!   after the advance), so the pitch-bend draw lands where the game makes it (before the back items' draw
//!   0x247800 and the hero's physics). The owner is Ratchet's moby as the last write-back left it; the listener
//!   is the camera of the previous tick (the camera updates after the hero), as for the moby sounds.
//! * **Ratchet's voices** `0x236738(index, flags)` (`PlayClassSound` on his moby: the hurt / death voices of
//!   `hero::damage`): [`HeroClassSounds`] too ([`HeroSounds::voice`]).
//! * **The sound step** [`sound_step`]: after the camera, before the counter increment (level01 `FUN_002aba68`,
//!   docs/plan/trace_results_novalis.md "Second savestate"): the EE frame (`sound_update` 0x2a0638: occlusion
//!   origin = 3 draws every frame, the 6-origin batch per new occluded sound, the sound instances' plays) and its
//!   800 samples.
//!
//! * **The hand item's sounds** (the Swingshot's fire / hit / pull, the wrench's hit: `PlayClassSound(i, 0, item)`):
//!   [`HeroSounds::item_sound`], played by the tick right after the item's update (`hero::gadgets::flush_item_sounds`),
//!   with the gadget class's defs (the gadget blobs' defs with their parked remap: [`crate::audio::LevelAudio`]).
//!
//! Clank (601) and the back packs (607–609) have no class sound defs on any level (class header +0x0d = 0), so
//! the triggers of their `MobyAnimAdvance` never play or draw; the hand items' animation triggers (wrench 71, …) are
//! not routed (the hero's item code advances them).

use super::voices::{Listener, Owner};
use super::AudioSystem;
use crate::hero::anim::AnimView;
use crate::audio::SoundEvent;
use crate::rng::Rng;
use rc_formats::moby_anim::MobyAnimClass;
use rc_formats::sound_bank::SoundOwner;

/// Class 0x472 (1138): its sounds may use slots 26..29 like the hero's.
pub(crate) const PRIVILEGED_CLASS: i16 = 0x472;

impl AudioSystem {
    /// `PlayClassSound(index, flags, moby)` (0x2a1618; 0x2a16c0 with another class's table): class sound
    /// `index` of `ev.sound_class` (−1 when the class has no such def), `SoundSlotAlloc` with the moby as the
    /// owner (privileged: the hero `hero` or class 0x472), volume 0x400; the slot remembers the class-sound
    /// index (+0xe). Draws the def's pitch bend from `rng` when it gets a slot.
    pub fn play_class_sound(&mut self, ev: &SoundEvent, hero: Option<usize>, listener: &Listener, rng: &mut Rng) -> i32 {
        self.stats.class_sounds += 1;
        let Ok(local) = usize::try_from(ev.index) else { return -1 };
        let Some(def) = self.data.sounds.def(SoundOwner::Class(ev.sound_class as i32), local).copied() else { return -1 };
        let owner = Owner { id: ev.moby as u32, privileged: Some(ev.moby) == hero || ev.o_class == PRIVILEGED_CLASS };
        let k = self.slots.play(&def, ev.flags as u8, Some(owner), Some(ev.pos), None, 0x400, listener, rng);
        if k >= 0 {
            self.slots.slots[k as usize].class_index = ev.index as u16;
            self.stats.class_slots += 1;
        }
        if let Some(log) = self.play_log.as_mut() { log.push((ev.tick, ev.sound_class, ev.index, ev.flags, k)); }
        k
    }
}

/// The play log's "class" for a footstep (its index is the level def).
pub const FOOTSTEP_LOG_CLASS: i16 = -1;
/// The play log's "class" for a level sound (`PlayLevelSoundAtMoby`; its index is the level def).
pub const LEVEL_SOUND_LOG_CLASS: i16 = -2;

/// The trigger check at the end of `RatchetAnimAdvance` (0x247d48): when key A and key B were the same
/// sequence before the advance and it has triggers (`+0x7e`), the first trigger word (lo16 class sound, hi16
/// time in 1/16 frames, signed) with `16·frame_a₀ + trunc(16·t₀) < time ≤ 16·frame_a₁ + trunc(16·t₁)` fires
/// (`before` / `after` = the views around the advance). Returns its class sound.
pub fn ratchet_trigger(class: &MobyAnimClass, before: &AnimView, after: &AnimView) -> Option<u16> {
    if before.seq_a != before.seq_b { return None; }
    let seq = class.sequence(after.seq_a)?;
    if seq.header.trigger_count == 0 { return None; }
    let lo = before.frame_a as i32 * 16 + (before.t * 16.0) as i32;
    let hi = after.frame_a as i32 * 16 + (after.t * 16.0) as i32;
    seq.triggers.iter().take(seq.header.trigger_count as usize).find(|&&w| {
        let time = (w as i32) >> 16;
        lo < time && time <= hi
    }).map(|&w| (w & 0xffff) as u16)
}

