//! Level message text (help boxes, banners, subtitles) from the `gameplay_ntsc` / `gameplay_pal` file.
//! Spec: docs/plan/hud_text.md §5.
//!
//! The gameplay header word at `+0x10 + 4·lang` points to a text block (the level loader 0x255958 copies the
//! block of the current language to the level heap and relocates it):
//!
//! ```text
//! u32 count, u32 size
//! count × { s32 text_offset (from the block start), s32 id, s32 help_audio (-1 none; voice = audio + 30000), s32 0 }
//! NUL-terminated strings
//! ```
//!
//! `msg_string__Fi` (0x2259e8) looks an id up with `Help_FindIndex` (0x225978): a linear scan, first match,
//! "Paradox! This message does not exist" when absent.

use crate::buf::{invalid, Buf, Result};

/// Languages as the game numbers them (`0x15ed88`); 1 is unused (its block is empty on the disc).
pub mod lang {
    pub const ENGLISH: u32 = 0;
}

/// Language slots in the gameplay header (+0x10 .. +0x2c).
pub(crate) const LANGUAGE_SLOTS: u32 = 8;

/// One text entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub(crate) id: i32,
    /// Bytes up to (not including) the NUL, control codes kept (§5 "Encoding").
    pub text: Vec<u8>,
    /// Help voice clip index, −1 = none.
    pub(crate) help_audio: i32,
}

/// Parses one text block (`count`, `size`, entries, strings) starting at byte 0 of `block`.
pub(crate) fn parse_text_block(block: &[u8]) -> Result<Vec<Message>> {
    let b = Buf(block);
    let count = b.u32(0)? as usize;
    let size = b.u32(4)? as usize;
    if count > 0x10000 { return invalid("implausible text block count"); }
    let body = b.sub(0, size.max(8), "text block")?;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let e = 8 + 16 * k;
        let (off, id, audio) = (body.i32(e)?, body.i32(e + 4)?, body.i32(e + 8)?);
        let Ok(off) = usize::try_from(off) else { return invalid("negative text offset") };
        let tail = body.tail(off, "message text")?.bytes();
        let Some(end) = tail.iter().position(|&c| c == 0) else { return invalid("message text without NUL") };
        out.push(Message { id, text: tail[..end].to_vec(), help_audio: audio });
    }
    Ok(out)
}

/// The messages of language `lang` from the decompressed gameplay file (empty for a language slot with no
/// entries).
pub fn parse_strings(gameplay: &[u8], lang: u32) -> Result<Vec<Message>> {
    if lang >= LANGUAGE_SLOTS { return invalid("language out of range"); }
    let off = Buf(gameplay).u32(0x10 + 4 * lang as usize)? as usize;
    parse_text_block(Buf(gameplay).tail(off, "text block")?.bytes())
}

/// `Help_FindIndex`: index of the first entry with `id`.
pub fn find_index(messages: &[Message], id: i32) -> Option<usize> { messages.iter().position(|m| m.id == id) }

