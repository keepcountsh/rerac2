//! Audio of the in-engine scenes (mode 2, docs/plan/cutscenes_transitions.md §3): the speech VAG stream,
//! the music pause at the start and its resume after the end, and the cutscene master volumes.
//!
//! * Start (`DialogStreamStart` 0x2ac330): `music_Pause(0)`, then the dialogue player 0x151720 streams
//!   `scenes[id].speech[language]` (group 2) — 3 vsyncs before the first scene tick
//!   ([`crate::scene_camera::AUDIO_LEAD`]).
//! * While the scene runs `sound_update` uses the cutscene master volumes (`voices::master_volumes`:
//!   music 0, groups 0 and 3 halved).
//! * End (0x2ac608): the speech stops (0x15172a → 5); `FUN_0027a460(ticks(30))` arms the paused music
//!   players' resume delay, so the music resumes 30 ticks later.
//!
//! The 989snd dialogue / pause internals are not reversed (inferred): the speech plays on one stream voice
//! at `rate_to_pitch(44056)` with the stream player's volume law at full volume; the pause freezes every
//! music stream voice (group 1, pitch and volume 0, position kept) and the resume restores them.
//!
//! **Hook.** The scene player runs in the engine's frame, the audio in its own driver, so requests go through
//! a process-wide inbox: [`post`] queues a [`SceneAudioCmd`]; the audio drains it at the start of its next
//! frame (the same 60 Hz frame when the scene system runs first).
//!
//! **Frames of a scene.** The engine suspends the gameplay tick while a scene runs, so the sound step of the
//! tick does not run either; [`AudioSystem::scene_frame_with`] is the audio frame of each scene frame: the
//! requests apply at once (the game issues them as calls from the blocking start / end code, not from
//! `sound_update`), then the EE frame (`sound_update`, `music_Update`: `CutsceneModeUpdate` runs them) when the
//! world runs this frame, else only the IOP / SPU (the blocking fades and vsync waits), then the 800 samples.
//!
//! **One music instance.** The pause holds every music stream voice, also one keyed while it lasts (a stream
//! command the render runs after the pause), until the resume; a second resume finds nothing to restore.

use super::music::{STREAM_ADSR1, STREAM_ADSR2};
use super::voices::{VoiceSlot, VoiceUse};
use super::{AudioSystem, StreamPart, VoiceData};
use rc_formats::vag;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Dialogue voice group (the dialogue player streams in group 2).
pub(crate) const SPEECH_GROUP: u8 = 2;
/// Music stream group.
pub(crate) const MUSIC_GROUP: u8 = 1;

/// A request from the scene player (`crate::scene_camera::AudioRequest`, with the speech bytes resolved).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneAudioCmd {
    /// Scene start (`music_Pause(0)`): freeze the music voices, switch to the cutscene volumes.
    PauseMusic,
    /// Start the speech VAG (the whole file, header included).
    Speech { vag: Arc<[u8]> },
    /// Speech stop (0x15172a → 5) and the normal volumes.
    StopSpeech,
    /// `FUN_0027a460(after)`: the music resumes after `after` ticks.
    ResumeMusic { after: i32 },
}

static INBOX: Mutex<VecDeque<SceneAudioCmd>> = Mutex::new(VecDeque::new());

/// Tests that post to or drain the process-wide inbox hold this (the test harness runs them in parallel).
#[cfg(test)]
pub(crate) static TEST_INBOX: Mutex<()> = Mutex::new(());

/// Queues a request for the audio driver.
pub fn post(cmd: SceneAudioCmd) { INBOX.lock().unwrap_or_else(|e| e.into_inner()).push_back(cmd); }

/// Takes every queued request.
pub(crate) fn take() -> Vec<SceneAudioCmd> { INBOX.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect() }

/// The scene audio state kept by [`AudioSystem`].
#[derive(Clone, Debug, Default)]
pub struct SceneAudio {
    /// Mode 2: `sound_update` uses the cutscene master volumes.
    pub cutscene: bool,
    /// The speech voice and its key-on generation.
    pub(crate) speech: Option<(usize, u32)>,
    /// Between `PauseMusic` and the resume: every music stream voice is held.
    pub(crate) music_paused: bool,
    /// Frozen music voices: (voice, generation, pitch, volume registers).
    paused: Vec<(usize, u32, u16, [u16; 2])>,
    /// Ticks until the music resumes (`FUN_0027a460`).
    pub resume_in: Option<i32>,
    /// Requests applied so far (reports / tests).
    pub(crate) applied: u32,
}

impl AudioSystem {
    /// Applies one scene request (called by `game_frame` for the inbox; callable directly).
    pub(crate) fn scene_command(&mut self, cmd: SceneAudioCmd) {
        self.scene.applied += 1;
        match cmd {
            SceneAudioCmd::PauseMusic => {
                self.scene.cutscene = true;
                self.scene.resume_in = None;
                self.scene.music_paused = true;
                self.pause_music();
            }
            SceneAudioCmd::Speech { vag } => {
                self.stop_speech();
                self.start_speech(&vag);
            }
            SceneAudioCmd::StopSpeech => {
                self.stop_speech();
                self.scene.cutscene = false;
            }
            SceneAudioCmd::ResumeMusic { after } => {
                self.scene.cutscene = false;
                self.scene.resume_in = Some(after.max(0));
            }
        }
    }

    /// Applies the queued requests ([`post`]) now.
    pub(crate) fn scene_inbox(&mut self) {
        for c in take() { self.scene_command(c); }
    }

    /// One audio frame of a mode-2 scene frame while the gameplay tick is suspended (module docs): the
    /// requests, then the EE frame on the game's stream ([`AudioSystem::game_frame_with`]) when `world_runs`,
    /// then the 800 samples appended to `out`.
    pub fn scene_frame_with(
        &mut self,
        world_runs: bool,
        input: &super::FrameInput,
        frame: u32,
        rng: &mut crate::rng::Rng,
        owner_pos: &dyn Fn(u32) -> Option<[f32; 3]>,
        out: &mut Vec<[i16; 2]>,
    ) {
        self.scene_inbox();
        if world_runs { self.game_frame_with(input, frame, rng, owner_pos); }
        self.render(super::SAMPLES_PER_FRAME, out);
    }

    /// Per EE frame, before `sound_update`: the inbox.
    pub(super) fn scene_frame(&mut self) {
        self.scene_inbox();
    }

    /// The resume countdown: `music_UpdateStream` 0x27a4a0's `FastDecTimer(+0xe)` on a paused player (state & 0x8000)
    /// inside `music_Update`, so every frame that runs `sound_update` counts it: gameplay, the page menus and the
    /// credits slideshow (mode 7), which the end of the game enters within the 30 ticks after its last scene.
    pub(super) fn music_resume_countdown(&mut self) {
        if let Some(t) = self.scene.resume_in {
            if t <= 0 {
                self.scene.resume_in = None;
                self.resume_music();
            } else {
                self.scene.resume_in = Some(t - 1);
            }
        }
    }

    /// Freezes the music stream voices. The render calls it again after its commands while the pause lasts:
    /// a stream keyed meanwhile is held too, and a held voice whose registers were rewritten (the group volume
    /// update when the cutscene volumes end, a stream volume change) keeps the new values for the resume and
    /// stays silent.
    pub(super) fn pause_music(&mut self) {
        let vm = &self.snd.vm;
        for (i, slot) in vm.voices.iter().enumerate() {
            let v = &mut self.spu.voices[i];
            if slot.group != MUSIC_GROUP || !matches!(slot.owner, VoiceUse::Stream { .. }) || !v.active() || slot.generation != v.generation { continue; }
            match self.scene.paused.iter_mut().find(|p| p.0 == i && p.1 == v.generation) {
                Some(p) => {
                    if v.pitch != 0 { p.2 = v.pitch; }
                    if v.vol != [0, 0] { p.3 = v.vol; }
                }
                None => {
                    self.scene.paused.retain(|p| p.0 != i);
                    self.scene.paused.push((i, v.generation, v.pitch, v.vol));
                }
            }
            v.pitch = 0;
            v.vol = [0, 0];
        }
    }

    fn resume_music(&mut self) {
        self.scene.music_paused = false;
        for (i, generation, pitch, vol) in std::mem::take(&mut self.scene.paused) {
            let v = &mut self.spu.voices[i];
            if v.generation == generation {
                v.pitch = pitch;
                v.vol = vol;
            }
        }
    }

    fn start_speech(&mut self, bytes: &[u8]) {
        let Ok((h, body)) = vag::parse_vag(bytes) else { return };
        let Ok(ext) = vag::sample_extent(body, 0) else { return };
        let part = StreamPart { data: Arc::from(&body[..ext.bytes()]), looped: false };
        let vm = &mut self.snd.vm;
        let Some(i) = vm.allocate(&mut self.spu, SPEECH_GROUP as usize, 127, 0) else { return };
        let base = vm.make_volume(127, 0, 127, 0, 127, 0);
        let regs = vm.voice_registers(base, SPEECH_GROUP as usize);
        let v = &mut self.spu.voices[i];
        v.key_on(VoiceData::Stream { cur: part, queue: Default::default() }, 0, vag::rate_to_pitch(h.sample_rate), STREAM_ADSR1, STREAM_ADSR2);
        v.vol = regs;
        let generation = v.generation;
        vm.voices[i] = VoiceSlot { owner: VoiceUse::Stream { handle: u32::MAX }, priority: 127, group: SPEECH_GROUP, basevol: base, start_tick: 0, generation };
        self.scene.speech = Some((i, generation));
    }

    fn stop_speech(&mut self) {
        if let Some((i, generation)) = self.scene.speech.take() {
            if self.spu.voices[i].generation == generation { self.spu.voices[i].key_off(); }
            self.snd.vm.voices[i].owner = VoiceUse::Free;
        }
    }

    /// Music stream voices (group 1) sounding and held by the pause: (playing, paused).
    pub fn music_voices(&self) -> (usize, usize) {
        let (mut playing, mut held) = (0, 0);
        for (i, slot) in self.snd.vm.voices.iter().enumerate() {
            let v = &self.spu.voices[i];
            if slot.group != MUSIC_GROUP || !matches!(slot.owner, VoiceUse::Stream { .. }) || !v.active() || slot.generation != v.generation { continue; }
            if self.scene.paused.iter().any(|p| p.0 == i && p.1 == v.generation) { held += 1 } else { playing += 1 }
        }
        (playing, held)
    }

    /// Whether the speech voice is still sounding.
    pub fn speech_playing(&self) -> bool {
        self.scene.speech.is_some_and(|(i, g)| self.spu.voices[i].generation == g && self.spu.voices[i].active())
    }
}

