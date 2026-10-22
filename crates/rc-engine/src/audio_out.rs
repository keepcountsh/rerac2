//! Audio output: the game's audio (`rc_game::audio`: sound slots, 989snd, music, software SPU2) run once per
//! 60 Hz game tick and played through Bevy's audio as one endless rodio source.
//!
//! * **With the game tick** (crate::play): the tick runs the EE frame (`sound_update`) and its 800 samples
//!   ([`AudioOut::push_frame`]) itself ([`AudioOut::external`]). A rendered frame whose game ticks did not run gets
//!   IOP-only frames here (989snd and the SPU keep playing; no EE update).
//! * **Without it** (`RC_PLAY=0`): each 60 Hz tick ([`GameTicks`]) runs the EE part of the frame on the audio
//!   system's private stream (`srand(1234)`) with the listener = the main camera and the hero position = the
//!   camera, then renders exactly 800 samples (48 kHz stereo).
//! * The mix is a pure function of the tick sequence and those inputs, so frame-exact runs give identical
//!   audio. `RC_AUDIO=0` removes the sound layer's draws from the game stream (the game without sound).
//! * The samples go to a ring buffer read by [`MixerDecoder`], a `rodio::Source` behind the `Decodable` asset
//!   [`MixerStream`] (bevy_audio 0.19, `add_audio_source`). The reader waits for 50 ms of audio before it
//!   starts (and again after an underrun, outputting silence meanwhile); the ring is capped at 0.25 s.
//!
//! Environment:
//! - `RC_AUDIO=0`: no audio (nothing is loaded).
//! - `RC_AUDIO_WAV=path`: also write the first 10 s of the mix as a 16-bit 48 kHz stereo WAV (on reaching
//!   10 s, or whatever was rendered when the app exits earlier); `RC_AUDIO_WAV_SECONDS=n` changes the length.
//! - `RC_AUDIO_TRACE=1`: one line per 60 output frames: music stream voices playing / held, speech, the main
//!   music player's state and track, the cutscene volumes, the pending resume; and one line per class sound played
//!   (tick, class, index, flags, the slot it got or −1), footstep and level sound (`PlayLevelSoundAtMoby`), and one
//!   line per reverb command (`SetReverbEx` / `snd_AutoReverb`: type, depth).
//! - `RC_REVERB=0`: no reverb output (a developer switch for A/B captures; the zones still run).
//! - `RC_AUDIO_MUSIC` / `RC_AUDIO_SFX`: the options menu volumes 0..=1024 (0x15edec / 0x15edf0; defaults 716 and
//!   1024 from the boot data), e.g. `RC_AUDIO_MUSIC=0` to hear or record the effects alone.

use crate::determinism::GameTicks;
use bevy::audio::{AddAudioSource, AudioPlayer, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use rc_game::audio::{self, AudioSystem, FrameInput, LevelAudio};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// `RC_AUDIO` is not "0".
pub fn enabled() -> bool { !std::env::var("RC_AUDIO").is_ok_and(|v| v.trim() == "0") }


/// Frames the reader waits for before (re)starting: 50 ms.
const PREFILL: usize = 2400;
/// Ring cap: 0.25 s (older frames are dropped when the game runs ahead of the device).
const RING_CAP: usize = 12_000;
/// `RC_AUDIO_WAV` length: 10 s unless `RC_AUDIO_WAV_SECONDS`.
fn wav_frames() -> usize {
    let secs = std::env::var("RC_AUDIO_WAV_SECONDS").ok().and_then(|v| v.trim().parse::<usize>().ok()).filter(|&s| s > 0).unwrap_or(10);
    secs * audio::OUTPUT_RATE as usize
}

#[derive(Default)]
struct RingState {
    frames: VecDeque<[i16; 2]>,
    running: bool,
    underruns: u64,
    /// Frames dropped at the cap (the game ran ahead of the device).
    dropped: u64,
}

/// Rendered frames waiting for the audio device.
#[derive(Default)]
pub struct Ring(Mutex<RingState>);

impl Ring {
    fn push(&self, frames: &[[i16; 2]]) {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        r.frames.extend(frames.iter().copied());
        let over = r.frames.len().saturating_sub(RING_CAP);
        r.frames.drain(..over);
        r.dropped += over as u64;
    }

    fn pop(&self) -> [i16; 2] {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !r.running {
            if r.frames.len() < PREFILL { return [0, 0]; }
            r.running = true;
        }
        match r.frames.pop_front() {
            Some(f) => f,
            None => {
                r.running = false;
                r.underruns += 1;
                [0, 0]
            }
        }
    }
}

/// The asset bevy_audio plays: a handle on the ring.
#[derive(Asset, TypePath)]
pub struct MixerStream {
    ring: Arc<Ring>,
}

/// The rodio source: interleaved stereo f32 at 48 kHz, endless.
pub struct MixerDecoder {
    ring: Arc<Ring>,
    right: Option<f32>,
}

impl Iterator for MixerDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.right.take() { return Some(r); }
        let [l, r] = self.ring.pop();
        self.right = Some(r as f32 / 32768.0);
        Some(l as f32 / 32768.0)
    }
}

impl Source for MixerDecoder {
    fn current_span_len(&self) -> Option<usize> { None }
    fn channels(&self) -> ChannelCount { ChannelCount::new(2).unwrap() }
    fn sample_rate(&self) -> SampleRate { SampleRate::new(audio::OUTPUT_RATE).unwrap() }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

impl Decodable for MixerStream {
    type Decoder = MixerDecoder;
    fn decoder(&self) -> MixerDecoder { MixerDecoder { ring: Arc::clone(&self.ring), right: None } }
}

/// `RC_AUDIO_WAV`: the first 10 s (`RC_AUDIO_WAV_SECONDS`).
struct WavCapture {
    path: PathBuf,
    frames: Vec<[i16; 2]>,
    limit: usize,
    written: bool,
}

impl WavCapture {
    fn write(&mut self) {
        if self.written { return; }
        self.written = true;
        match std::fs::write(&self.path, audio::wav_bytes(&self.frames)) {
            Ok(()) => println!("audio: wrote {:.2} s to {}", self.frames.len() as f64 / audio::OUTPUT_RATE as f64, self.path.display()),
            Err(e) => eprintln!("audio: cannot write {}: {e}", self.path.display()),
        }
    }
}

impl Drop for WavCapture {
    fn drop(&mut self) { self.write(); }
}

/// The audio system and its output (ring, WAV capture). crate::gameplay runs the sound step of each game tick
/// on it ([`AudioOut::system`], then [`AudioOut::push_frame`]).
#[derive(Resource)]
pub struct AudioOut {
    system: AudioSystem,
    ring: Arc<Ring>,
    wav: Option<WavCapture>,
    /// 800-sample frames output so far.
    done: u64,
    /// The frame being rendered (the sound step appends to it).
    pub buf: Vec<[i16; 2]>,
    last_report: u64,
    /// `RC_AUDIO_TRACE`.
    trace: bool,
    /// `RC_AUDIO_SFX` / `RC_AUDIO_MUSIC` given: they win over the saved game's options ([`sync_options`]).
    /// A game other than R&C1's tick runs the EE frames and pushes them (Going Commando's, crate::play): the
    /// frames it did not run get IOP-only ones here.
    pub external: bool,
}

impl AudioOut {
    pub fn system(&mut self) -> &mut AudioSystem { &mut self.system }

    /// A runtime level change (crate::level_switch): the next level's sound data (bank, defs, sound instances, music) in a
    /// new audio system, with the options, the reverb switch and the logs of the old one; the output (ring, WAV capture,
    /// frame count) goes on.
    pub fn swap_level(&mut self, data: LevelAudio) {
        let old = &self.system;
        let mut system = AudioSystem::new(data);
        (system.music_option, system.sfx_option) = (old.music_option, old.sfx_option);
        system.spu.reverb.enabled = old.spu.reverb.enabled;
        if old.play_log.is_some() { system.play_log = Some(Vec::new()); }
        if old.reverb_log.is_some() { system.reverb_log = Some(Vec::new()); }
        self.system = system;
    }

    /// The audio system and the frame buffer, borrowed together (the sound step renders into `buf`).
    pub fn parts(&mut self) -> (&mut AudioSystem, &mut Vec<[i16; 2]>) { (&mut self.system, &mut self.buf) }

    /// Output the rendered frame [`AudioOut::buf`] (ring and WAV capture) and clear it.
    pub fn push_frame(&mut self) {
        self.done += 1;
        self.ring.push(&self.buf);
        if let Some(w) = &mut self.wav {
            if !w.written {
                let take = (w.limit - w.frames.len()).min(self.buf.len());
                w.frames.extend_from_slice(&self.buf[..take]);
                if w.frames.len() == w.limit { w.write(); }
            }
        }
        self.buf.clear();
        if self.trace {
            for (tick, class, index, flags, slot) in self.system.play_log.as_mut().map(std::mem::take).unwrap_or_default() {
                if class == audio::class_sounds::FOOTSTEP_LOG_CLASS {
                    println!("audio trace: tick {tick}: footstep, level def {index}, flags {flags:#x} -> slot {slot}");
                    continue;
                }
                if class == audio::class_sounds::LEVEL_SOUND_LOG_CLASS {
                    println!("audio trace: tick {tick}: level sound {index}, flags {flags:#x} -> slot {slot}");
                    continue;
                }
                let who = match class { 0 => "Ratchet", 0x47 => "wrench", 0xd0 => "Swingshot", _ => "moby" };
                println!("audio trace: tick {tick}: class sound {index} of class {class} ({who}), flags {flags:#x} -> slot {slot}");
            }
        }
        if self.trace {
            for (frame, c) in self.system.reverb_log.as_mut().map(std::mem::take).unwrap_or_default() {
                let name = |k: u8| audio::reverb::mode::NAMES.get(k as usize).copied().unwrap_or("?");
                match c {
                    audio::voices::SndCommand::SetReverb { kind, depth, delay, feedback } => {
                        println!("audio trace: frame {frame}: SetReverbEx type {kind} ({}), depth {depth}, delay {delay}, feedback {feedback}", name(kind))
                    }
                    audio::voices::SndCommand::AutoReverb { depth, delta, .. } => println!("audio trace: frame {frame}: AutoReverb depth {depth} over {delta}"),
                    _ => {}
                }
            }
        }
        if self.trace && self.done.is_multiple_of(60) {
            let s = &self.system;
            let (music, held) = s.music_voices();
            let m = s.music.main;
            println!(
                "audio trace: frame {} ({:.2} s): music voices {music} playing, {held} held; speech {}; main player state {:#x} track {} handle {}; cutscene {}; resume in {:?}",
                self.done, self.done as f64 / 60.0, s.speech_playing(), m.state, m.track, m.handle, s.scene.cutscene, s.scene.resume_in
            );
        }
    }
}

/// Adds the audio when the level's data loaded.
pub struct AudioOutPlugin(Mutex<Option<LevelAudio>>);

impl AudioOutPlugin {
    pub fn new(data: Option<LevelAudio>) -> Self { AudioOutPlugin(Mutex::new(data)) }
}

impl Plugin for AudioOutPlugin {
    fn build(&self, app: &mut App) {
        let Some(data) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        let ring = Arc::new(Ring::default());
        let mut system = AudioSystem::new(data);
        let option = |name: &str| std::env::var(name).ok().and_then(|v| v.trim().parse::<i32>().ok()).map(|v| v.clamp(0, 0x400));
        let (fixed_music, fixed_sfx) = (option("RC_AUDIO_MUSIC"), option("RC_AUDIO_SFX"));
        if let Some(v) = fixed_music { system.music_option = v; }
        if let Some(v) = fixed_sfx { system.sfx_option = v; }
        let limit = wav_frames();
        let wav = std::env::var_os("RC_AUDIO_WAV").filter(|v| !v.is_empty()).map(|p| WavCapture { path: p.into(), frames: Vec::with_capacity(limit), limit, written: false });
        let trace = std::env::var("RC_AUDIO_TRACE").is_ok_and(|v| v.trim() == "1");
        if trace {
            system.play_log = Some(Vec::new());
            system.reverb_log = Some(Vec::new());
        }
        if std::env::var("RC_REVERB").is_ok_and(|v| v.trim() == "0") {
            system.spu.reverb.enabled = false;
            println!("audio: RC_REVERB=0: no reverb output");
        }
        app.add_audio_source::<MixerStream>()
            .insert_resource(AudioOut { system, ring, wav, done: 0, buf: Vec::new(), last_report: 0, trace, external: false })
            .add_systems(Startup, start_output)
            .add_systems(PostUpdate, run_audio);
    }
}

fn start_output(mut commands: Commands, out: Res<AudioOut>, mut streams: ResMut<Assets<MixerStream>>) {
    let handle = streams.add(MixerStream { ring: Arc::clone(&out.ring) });
    commands.spawn(AudioPlayer(handle));
}

fn run_audio(
    mut out: ResMut<AudioOut>,
    ticks: Res<GameTicks>,
    cams: Query<&Transform, With<crate::fly_cam::FlyCam>>,
) {
    let Some(cam) = cams.iter().next() else { return };
    let eye = crate::game_camera::game_eye(cam);
    let rows = crate::game_camera::game_rows(cam);
    let listener = audio::voices::Listener { pos: eye.to_array(), rows: rows.map(|r| r.to_array()), underwater: out.system.underwater, water_height: 0.0 };
    let input = FrameInput { listener, hero_pos: listener.pos };
    let out = &mut *out;
    // With the game tick the EE frames ran in its sound step; fill the ticks it did not run with IOP-only frames.
    let driven = out.external;
    while out.done < ticks.0 {
        out.buf.clear();
        if driven {
            out.system.render(audio::SAMPLES_PER_FRAME, &mut out.buf);
        } else {
            out.system.tick(&input, &mut out.buf);
        }
        out.push_frame();
    }
    if out.done >= out.last_report + 600 {
        out.last_report = out.done;
        let s = &out.system;
        let (underruns, dropped, fill) = out.ring.0.lock().map(|r| (r.underruns, r.dropped, r.frames.len())).unwrap_or_default();
        println!(
            "audio: tick {}, {} slots busy, {} SPU voices active, {} sounds playing, {} plays so far ({} of {} class sounds got a slot), {} underruns, {} frames dropped, ring {} frames{}",
            out.done,
            s.slots.slots.iter().filter(|x| x.state != audio::voices::state::FREE).count(),
            s.spu.voices.iter().filter(|v| v.active()).count(),
            s.snd.playing(),
            s.stats.plays,
            s.stats.class_slots,
            s.stats.class_sounds,
            underruns,
            dropped,
            fill,
            if s.snd.unimplemented.is_empty() { String::new() } else { format!(", unported grains {:?}", s.snd.unimplemented) }
        );
    }
}
