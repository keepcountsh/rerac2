//! SPU ADPCM ("VAG") sample data: the frame decoder, sample extents and loop points, the VAG file header,
//! and the SPU pitch helpers (`sceSdNote2Pitch`). Spec: docs/plan/audio.md §2.3–2.4.
//!
//! A frame is 16 bytes: `b0 = shift | filter << 4`, `b1 = flags` (bit0 end, bit1 repeat, bit2 loop start),
//! then 14 bytes = 28 four-bit samples, low nibble first. Bank samples (the `sound_bank` lump's second chunk)
//! and the music / speech VAG bodies use the same frames.
//!
//! [`decode_frame`] takes a [`Variant`]: `Rounded` is the PCSX2 / DuckStation form with the prediction rounded
//! (`+32` before `>> 6`), the form the port plays; `OpenGoal` shifts the two prediction terms separately without
//! rounding (OpenGOAL `game/sound/common/voice.cpp`, `Voice::DecodeSamples`); they differ by up to 2 LSB.

use crate::buf::{invalid, Result};

/// Bytes per ADPCM frame.
pub const FRAME_BYTES: usize = 16;
/// Samples per ADPCM frame.
pub const FRAME_SAMPLES: usize = 28;
/// Positive prediction coefficients (/64), indexed by the filter nibble.
pub(crate) const K0: [i32; 5] = [0, 60, 115, 98, 122];
/// Negative prediction coefficients (/64).
pub(crate) const K1: [i32; 5] = [0, 0, -52, -55, -60];

/// Frame flag bits (byte 1).
pub const FLAG_END: u8 = 1;
pub const FLAG_REPEAT: u8 = 2;
pub const FLAG_LOOP_START: u8 = 4;

/// Which rounding the prediction uses (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// `s += (K0·h1 + K1·h2 + 32) >> 6` (PCSX2 / DuckStation).
    #[default]
    Rounded,
    /// `s += (K0·h1) >> 6; s += (K1·h2) >> 6` (OpenGOAL).
    OpenGoal,
}

/// Decoder history: the previous two output samples (`h1` newest). Zero at key-on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct History {
    pub(crate) h1: i32,
    pub(crate) h2: i32,
}

/// Decodes one 16-byte frame (`frame.len() >= 16`), updating `hist`. The disc only uses shift 0–12 and
/// filter 0–4; a larger filter is clamped to 4 here, a larger shift is applied as given.
pub fn decode_frame(frame: &[u8], hist: &mut History, variant: Variant) -> [i16; FRAME_SAMPLES] {
    let shift = (frame[0] & 0x0f) as u32;
    let filter = ((frame[0] >> 4) as usize).min(4);
    let (k0, k1) = (K0[filter], K1[filter]);
    let mut out = [0i16; FRAME_SAMPLES];
    for (i, o) in out.iter_mut().enumerate() {
        let nibble = (frame[2 + i / 2] >> ((i & 1) * 4)) & 0x0f;
        // (i16)(n << 12) >> shift: the nibble is the top four bits of a signed 16-bit value.
        let mut s = (((nibble as u16) << 12) as i16 as i32) >> shift;
        match variant {
            Variant::Rounded => s += (k0 * hist.h1 + k1 * hist.h2 + 32) >> 6,
            Variant::OpenGoal => {
                s += (k0 * hist.h1) >> 6;
                s += (k1 * hist.h2) >> 6;
            }
        }
        let s = s.clamp(i16::MIN as i32, i16::MAX as i32);
        hist.h2 = hist.h1;
        hist.h1 = s;
        *o = s as i16;
    }
    out
}

/// Where one sample lives in an ADPCM blob and how a voice plays it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleExtent {
    /// Byte offset of the first frame.
    pub(crate) offset: usize,
    /// Frames up to and including the first frame with the end flag (what a voice plays before it jumps
    /// to the loop start or stops).
    pub(crate) frames: usize,
    /// Frame index (relative to `offset`) of the last loop-start flag seen before the end frame; the SPU
    /// latches LSA at every flagged frame, so the last one wins.
    pub(crate) loop_start: Option<usize>,
    /// The end frame has the repeat flag: the voice jumps to LSA instead of stopping.
    pub(crate) looped: bool,
    /// Flags byte of the frame after the end frame, if it is inside the blob (one-shots are padded with a
    /// flags-7 frame on the disc).
    pub(crate) next_flags: Option<u8>,
}

impl SampleExtent {
    /// Bytes played (`frames · 16`).
    pub fn bytes(&self) -> usize { self.frames * FRAME_BYTES }
}

/// Walks the frames of the sample starting at `offset` until the first end flag.
pub fn sample_extent(adpcm: &[u8], offset: usize) -> Result<SampleExtent> {
    if !offset.is_multiple_of(FRAME_BYTES) { return invalid(format!("sample offset {offset:#x} is not frame aligned")); }
    let mut loop_start = None;
    let mut f = 0usize;
    loop {
        let at = offset + f * FRAME_BYTES;
        if at + FRAME_BYTES > adpcm.len() { return invalid(format!("sample at {offset:#x} runs past the data without an end flag")); }
        let flags = adpcm[at + 1];
        if flags & FLAG_LOOP_START != 0 { loop_start = Some(f); }
        if flags & FLAG_END != 0 {
            let next = offset + (f + 1) * FRAME_BYTES;
            let next_flags = (next + FRAME_BYTES <= adpcm.len()).then(|| adpcm[next + 1]);
            return Ok(SampleExtent { offset, frames: f + 1, loop_start, looped: flags & FLAG_REPEAT != 0, next_flags });
        }
        f += 1;
    }
}

// ---------------------------------------------------------------------------------------------------
// VAG files (music, scene speech): a 0x30-byte big-endian header, then one mono ADPCM body.

/// `VAGp` header (0x30 bytes, big-endian).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VagHeader {
    pub(crate) version: u32,
    /// Body bytes after the header.
    pub(crate) data_size: u32,
    /// 44100 for music, 44056 for scene speech.
    pub sample_rate: u32,
    /// Up to 16 characters, e.g. `L01_Enemy_Loop`.
    pub name: String,
}

pub(crate) const VAG_HEADER_SIZE: usize = 0x30;

/// Parses a VAG file into its header and body (`data_size` bytes, clipped to the file).
pub fn parse_vag(bytes: &[u8]) -> Result<(VagHeader, &[u8])> {
    if bytes.len() < VAG_HEADER_SIZE || &bytes[..4] != b"VAGp" { return invalid("not a VAGp file"); }
    let be = |o: usize| u32::from_be_bytes(bytes[o..o + 4].try_into().unwrap());
    let name_bytes = &bytes[0x20..0x30];
    let name = String::from_utf8_lossy(&name_bytes[..name_bytes.iter().position(|&b| b == 0).unwrap_or(16)]).into_owned();
    let h = VagHeader { version: be(4), data_size: be(0x0c), sample_rate: be(0x10), name };
    let end = (VAG_HEADER_SIZE + h.data_size as usize).min(bytes.len());
    Ok((h, &bytes[VAG_HEADER_SIZE..end]))
}

// ---------------------------------------------------------------------------------------------------
// Pitch. The SPU plays a voice at `pitch / 0x1000 × 48000` Hz (pitch 0x1000 = 48 kHz, capped at 0x3fff).

/// libsd's note table (`sceSdNote2Pitch`): 12 semitone ratios then 128 steps of 1/128 semitone, as
/// 0x8000·2^(k/12) and 0x8000·2^(k/1536), truncated. Verified equal to the 140 u16 the RAC1 libsd IRX
/// (global `irx` entry 21) carries, and to OpenGOAL's `NotePitchTable` (`989snd/util.cpp`).
pub(crate) fn note_pitch_table() -> [u16; 140] {
    let mut t = [0u16; 140];
    for (k, v) in t.iter_mut().enumerate() {
        let e = if k < 12 { k as f64 / 12.0 } else { (k - 12) as f64 / 1536.0 };
        *v = (32768.0 * 2f64.powf(e)) as u16;
    }
    t
}

/// `sceSdNote2Pitch(center_note, center_fine, note, fine)`: the SPU pitch word that plays a sample recorded
/// at `center_note + center_fine/128` (at 48 kHz) as `note + fine/128`. Integer arithmetic as libsd does it
/// (C division truncating toward zero); mirrors OpenGOAL `sceSdNote2Pitch` step for step.
pub(crate) fn note_to_pitch(center_note: u16, center_fine: u16, note: u16, fine: i16) -> u16 {
    let table = note_pitch_table();
    let fine_total = fine as i32 + center_fine as i32;
    let fine_carry = if fine_total < 0 { fine_total + 127 } else { fine_total } / 128;
    let n = note as i32 + fine_carry - center_note as i32;
    let mut oct = n / 6;
    if n < 0 { oct -= 1; }
    let mut fine_idx = fine_total - fine_carry * 128;
    let neg = if n < 0 { -1 } else { 0 };
    if oct < 0 { oct -= 1; }
    let octave = oct / 2 - neg;
    let mut shift = octave - 2;
    let mut semi = n - octave * 12;
    if semi < 0 || (semi == 0 && fine_idx < 0) {
        semi += 12;
        shift = octave - 3;
    }
    if fine_idx < 0 {
        semi = semi - 1 + fine_carry;
        fine_idx += (fine_carry + 1) * 128;
    }
    let mut ret = (table[semi as usize] as i32 * table[(fine_idx + 12) as usize] as i32) / 0x10000;
    if shift < 0 { ret = (ret + (1 << (-shift - 1))) >> -shift; }
    ret as u16
}

/// 989snd's `PS1Note2Pitch`: a negative `center_note` is a PS2-rate sample (use `-center_note`); a
/// non-negative one is a PS1-style 44.1 kHz sample, scaled by 44100/48000 after the table lookup.
pub fn ps1_note_to_pitch(center_note: i8, center_fine: i8, note: i16, fine: i16) -> u16 {
    let ps1 = center_note >= 0;
    let cn = if ps1 { center_note as i32 } else { -(center_note as i32) };
    let p = note_to_pitch(cn as u16, center_fine as u8 as u16, note as u16, fine);
    if ps1 { (44100 * p as u32 / 48000) as u16 } else { p }
}

/// SPU pitch word for a stream recorded at `rate` Hz (`rate · 0x1000 / 48000`, truncated). Inferred: the
/// 989snd stream player's own computation is not reversed (docs/plan/audio.md §4).
pub fn rate_to_pitch(rate: u32) -> u16 { ((rate as u64 * 0x1000) / 48000).min(0x3fff) as u16 }

