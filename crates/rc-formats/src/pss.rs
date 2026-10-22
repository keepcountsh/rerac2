//! PSS movies (`global/mpegs/NNN.bin`): the MPEG-2 program stream the game's `sceMpeg` player reads, split into
//! its video elementary stream and its SPU-ADPCM audio. Spec: docs/formats/pss.md.
//!
//! * **Packs** `00 00 01 BA` (MPEG-2 form, 14 bytes + stuffing), a system header `BB` in the first pack, PES
//!   packets `E0` (video), `BD` (private stream 1: audio) and `BE` (padding), and the end code `00 00 01 B9`.
//! * **Video** PES packets carry an MPEG-2 video elementary stream (PTS / DTS in their headers); the payloads
//!   concatenated in file order are the stream `rc-video` decodes.
//! * **Audio** PES packets: the payload starts with a 4-byte sub-stream header `FF A1 00 cc` (`cc` = the audio
//!   channel, one per language: 0 English, 2 French, 3 German, 4 Spanish, 5 Italian — the game passes its
//!   language 0x15ed88 to `sceMpegAddStrCallback(…, sceMpegStrADPCM, language, pcmCallback)`, and the callback
//!   skips those 4 bytes). The concatenated channel bytes start with a 0x28-byte header (`SShd`: type 0x10 = SPU
//!   ADPCM, rate, channels, interleave; `SSbd`: body size) followed by the body: blocks of `interleave` bytes per
//!   channel in turn (L, R, L, R, …), each channel one continuous SPU-ADPCM stream (`audioDecBeginPut` 0x31ac00
//!   takes the header, `sendADPCM` 0x31ae68 de-interleaves by `interleave`).

use crate::buf::{invalid, Result};
use crate::vag::{decode_frame, History, Variant, FRAME_BYTES, FRAME_SAMPLES};
use std::collections::BTreeMap;

/// Stream ids.
pub(crate) const PACK: u8 = 0xba;
pub(crate) const SYSTEM_HEADER: u8 = 0xbb;
pub(crate) const PRIVATE_1: u8 = 0xbd;
pub(crate) const PADDING: u8 = 0xbe;
pub(crate) const END: u8 = 0xb9;
/// Audio sub-stream header bytes 0..3 (byte 3 = the channel).
pub(crate) const AUDIO_SUBSTREAM: [u8; 3] = [0xff, 0xa1, 0x00];
/// `SShd` + `SSbd` header bytes at the start of each audio channel.
pub(crate) const AUDIO_HEADER_BYTES: usize = 0x28;
/// `SShd` type of SPU ADPCM.
pub(crate) const AUDIO_TYPE_ADPCM: u32 = 0x10;

/// What a PES packet carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Stream {
    /// MPEG video stream `0xE0 + n`.
    Video(u8),
    /// Private stream 1 with the `FF A1 00 cc` sub-stream header: audio channel `cc`.
    Audio(u8),
    /// Padding, the system header, or any other stream id.
    Other(u8),
}

/// One PES packet.
#[derive(Clone, Copy, Debug)]
pub struct Packet<'a> {
    /// Byte offset of its start code in the file.
    pub offset: usize,
    pub(crate) stream: Stream,
    /// 90 kHz time stamps from the PES header (33 bits).
    pub(crate) pts: Option<u64>,
    pub dts: Option<u64>,
    /// The payload after the PES header (and after the 4-byte sub-stream header for audio).
    pub(crate) payload: &'a [u8],
}

/// The packets of a program stream in file order; stops after the end code (or at the end of the data).
pub(crate) struct Demux<'a> {
    data: &'a [u8],
    pos: usize,
    done: bool,
    /// Packs seen so far.
    pub(crate) packs: usize,
    /// Whether the end code was reached.
    pub(crate) ended: bool,
}

impl<'a> Demux<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self { Demux { data, pos: 0, done: false, packs: 0, ended: false } }

    /// Bytes consumed so far.
    pub(crate) fn position(&self) -> usize { self.pos }

    fn next_packet(&mut self) -> Result<Option<Packet<'a>>> {
        let d = self.data;
        loop {
            let i = self.pos;
            if i + 4 > d.len() { return Ok(None); }
            if d[i..i + 3] != [0, 0, 1] { return invalid(format!("pss: no start code at {i:#x} ({:02x?})", &d[i..(i + 8).min(d.len())])); }
            let id = d[i + 3];
            match id {
                PACK => {
                    if i + 14 > d.len() { return invalid(format!("pss: truncated pack header at {i:#x}")); }
                    if d[i + 4] >> 6 != 1 { return invalid(format!("pss: pack at {i:#x} is not MPEG-2")); }
                    self.pos = i + 14 + (d[i + 13] & 7) as usize;
                    self.packs += 1;
                }
                END => {
                    self.pos = i + 4;
                    self.ended = true;
                    return Ok(None);
                }
                SYSTEM_HEADER..=0xff => {
                    if i + 6 > d.len() { return invalid(format!("pss: truncated packet header at {i:#x}")); }
                    let len = u16::from_be_bytes([d[i + 4], d[i + 5]]) as usize;
                    let end = i + 6 + len;
                    if end > d.len() { return invalid(format!("pss: packet {id:#x} at {i:#x} runs past the end ({end:#x} > {:#x})", d.len())); }
                    self.pos = end;
                    if id == SYSTEM_HEADER || id == PADDING || !(id == PRIVATE_1 || (0xe0..=0xef).contains(&id) || (0xc0..=0xdf).contains(&id)) {
                        return Ok(Some(Packet { offset: i, stream: Stream::Other(id), pts: None, dts: None, payload: &d[i + 6..end] }));
                    }
                    // MPEG-2 PES header: '10' marker, flags, header data length.
                    if len < 3 || d[i + 6] >> 6 != 2 { return invalid(format!("pss: packet {id:#x} at {i:#x} has no MPEG-2 PES header")); }
                    let flags = d[i + 7];
                    let hl = d[i + 8] as usize;
                    let body = i + 9 + hl;
                    if body > end { return invalid(format!("pss: PES header of {id:#x} at {i:#x} longer than the packet")); }
                    let ts = |o: usize| -> Option<u64> {
                        let b = d.get(o..o + 5)?;
                        Some(((b[0] as u64 >> 1) & 7) << 30 | (b[1] as u64) << 22 | (b[2] as u64 >> 1) << 15 | (b[3] as u64) << 7 | b[4] as u64 >> 1)
                    };
                    let pts = (flags & 0x80 != 0 && hl >= 5).then(|| ts(i + 9)).flatten();
                    let dts = (flags & 0xc0 == 0xc0 && hl >= 10).then(|| ts(i + 14)).flatten();
                    let payload = &d[body..end];
                    let stream = if id == PRIVATE_1 {
                        if payload.len() < 4 || payload[..3] != AUDIO_SUBSTREAM { return invalid(format!("pss: private stream 1 at {i:#x} has no audio sub-stream header ({:02x?})", &payload[..payload.len().min(4)])); }
                        return Ok(Some(Packet { offset: i, stream: Stream::Audio(payload[3]), pts, dts, payload: &payload[4..] }));
                    } else if (0xe0..=0xef).contains(&id) {
                        Stream::Video(id)
                    } else {
                        Stream::Other(id)
                    };
                    return Ok(Some(Packet { offset: i, stream, pts, dts, payload }));
                }
                _ => return invalid(format!("pss: unexpected start code {id:#x} at {i:#x}")),
            }
        }
    }
}

impl<'a> Iterator for Demux<'a> {
    type Item = Result<Packet<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        match self.next_packet() {
            Ok(Some(p)) => Some(Ok(p)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

/// A whole movie split by stream.
#[derive(Clone, Debug, Default)]
pub struct Demuxed {
    /// The video elementary stream (0xE0 payloads in file order).
    pub video: Vec<u8>,
    /// Audio channel → its bytes (header + interleaved ADPCM body).
    pub audio: BTreeMap<u8, Vec<u8>>,
    /// PTS of the first video packet that has one (90 kHz).
    pub(crate) first_video_pts: Option<u64>,
    pub packs: usize,
    pub video_packets: usize,
    pub audio_packets: usize,
    pub(crate) other_packets: usize,
    /// The end code was found.
    pub(crate) ended: bool,
    /// Bytes after the end code (the file is padded to whole sectors).
    pub(crate) trailing: usize,
}

/// Splits a PSS file into its streams.
pub fn demux(data: &[u8]) -> Result<Demuxed> {
    let mut out = Demuxed::default();
    let mut dm = Demux::new(data);
    for p in &mut dm {
        let p = p?;
        match p.stream {
            Stream::Video(_) => {
                if out.first_video_pts.is_none() { out.first_video_pts = p.pts; }
                out.video.extend_from_slice(p.payload);
                out.video_packets += 1;
            }
            Stream::Audio(c) => {
                out.audio.entry(c).or_default().extend_from_slice(p.payload);
                out.audio_packets += 1;
            }
            Stream::Other(_) => out.other_packets += 1,
        }
    }
    out.packs = dm.packs;
    out.ended = dm.ended;
    out.trailing = data.len() - dm.position();
    Ok(out)
}

/// The `SShd` / `SSbd` header at the start of an audio channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHeader {
    /// 0x10 = SPU ADPCM.
    pub(crate) kind: u32,
    pub rate: u32,
    pub(crate) channels: u32,
    /// Bytes per channel block.
    pub(crate) interleave: u32,
    pub(crate) loop_start: i32,
    pub(crate) loop_end: i32,
    /// `SSbd` body size in bytes.
    pub(crate) body_bytes: u32,
}

impl AudioHeader {
    pub(crate) fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < AUDIO_HEADER_BYTES { return invalid(format!("pss audio: {} bytes, no SShd header", b.len())); }
        let w = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        if &b[0..4] != b"SShd" || w(4) != 0x18 { return invalid(format!("pss audio: bad SShd header {:02x?}", &b[..8])); }
        if &b[0x20..0x24] != b"SSbd" { return invalid(format!("pss audio: no SSbd at 0x20 ({:02x?})", &b[0x20..0x24])); }
        let h = AudioHeader {
            kind: w(8),
            rate: w(0xc),
            channels: w(0x10),
            interleave: w(0x14),
            loop_start: w(0x18) as i32,
            loop_end: w(0x1c) as i32,
            body_bytes: w(0x24),
        };
        if h.kind != AUDIO_TYPE_ADPCM { return invalid(format!("pss audio: type {:#x} is not SPU ADPCM", h.kind)); }
        if !(1..=2).contains(&h.channels) || h.interleave == 0 || !(h.interleave as usize).is_multiple_of(FRAME_BYTES) || h.rate == 0 {
            return invalid(format!("pss audio: unsupported layout {h:?}"));
        }
        Ok(h)
    }
}

/// One audio channel of a movie (`Demuxed::audio[c]`): header and interleaved body.
#[derive(Clone, Debug)]
pub struct Audio<'a> {
    pub header: AudioHeader,
    pub(crate) body: &'a [u8],
}

impl<'a> Audio<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let header = AudioHeader::parse(bytes)?;
        let body = &bytes[AUDIO_HEADER_BYTES..];
        let body = &body[..body.len().min(header.body_bytes as usize)];
        Ok(Audio { header, body })
    }

    /// Whole blocks per channel.
    pub(crate) fn blocks(&self) -> usize { self.body.len() / (self.header.interleave as usize * self.header.channels as usize) }

    /// Samples per channel.
    pub(crate) fn samples(&self) -> usize { self.blocks() * self.header.interleave as usize / FRAME_BYTES * FRAME_SAMPLES }

    /// Decodes both channels (a mono stream is duplicated) with the port's SPU ADPCM decoder
    /// ([`crate::vag::decode_frame`], rounded form), each channel one continuous stream from zero history, at the
    /// header rate. A trailing partial block is dropped.
    pub fn decode(&self) -> Vec<[i16; 2]> {
        let il = self.header.interleave as usize;
        let ch = self.header.channels as usize;
        let mut out = vec![[0i16; 2]; self.samples()];
        let mut hist = [History::default(); 2];
        let per_block = il / FRAME_BYTES * FRAME_SAMPLES;
        for b in 0..self.blocks() {
            for c in 0..ch {
                let block = &self.body[(b * ch + c) * il..(b * ch + c + 1) * il];
                for (f, frame) in block.as_chunks::<FRAME_BYTES>().0.iter().enumerate() {
                    let s = decode_frame(frame, &mut hist[c], Variant::Rounded);
                    let at = b * per_block + f * FRAME_SAMPLES;
                    for (k, v) in s.into_iter().enumerate() {
                        out[at + k][c] = v;
                        if ch == 1 { out[at + k][1] = v; }
                    }
                }
            }
        }
        out
    }
}

