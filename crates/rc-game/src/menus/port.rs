//! **Port-only** (not in the game): the Port Options page, the settings the PS2 never had (the graphics options and
//! their preset, the aspect, the render resolution and fullscreen), as R&C1's port has them. The page is built from
//! the game's own Options list (its frame, selection and font, crate::menus); the engine owns each setting's meaning
//! and persistence ([`PortValues`]).

/// A port setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    /// The graphics preset: 0 Original, 1 Enhanced, 2 Custom (shown when the rows match no preset, never chosen).
    Preset,
    /// 0 Original (the game's own softening passes), 1 off, 2 / 3 / 4: 2x, 4x, 8x multisampling.
    AntiAliasing,
    /// 0 Original (the game's distances), 1 Far, 2 Farther, 3 Maximum.
    Detail,
    /// 0 Original (the PS2's rule), 1 Smooth, 2 Sharp.
    Textures,
    /// 0 Original (512×416 scaled smoothly), 1 Sharp pixels, 2 High resolution.
    Hud,
    /// 0 the TV's 4:3, 1 16:10, 2 16:9.
    Aspect,
    /// 0 the window's, then a height of 416, 720, 1080, 1440, 2160.
    Resolution,
    /// 0 windowed, 1 fullscreen.
    Fullscreen,
}

/// One row: its setting, label and values (✕ steps through them).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub setting: Setting,
    pub label: &'static str,
    pub values: &'static [&'static str],
}

pub const ENTRIES: &[Entry] = &[
    Entry { setting: Setting::Preset, label: "Preset", values: &["Original", "Enhanced", "Custom"] },
    Entry { setting: Setting::AntiAliasing, label: "Anti-aliasing", values: &["Original", "Off", "2x", "4x", "8x"] },
    Entry { setting: Setting::Detail, label: "Detail distance", values: &["Original", "Far", "Farther", "Maximum"] },
    Entry { setting: Setting::Textures, label: "Texture filtering", values: &["Original", "Smooth", "Sharp"] },
    Entry { setting: Setting::Hud, label: "HUD", values: &["Original", "Sharp pixels", "High resolution"] },
    Entry { setting: Setting::Aspect, label: "Aspect ratio", values: &["4:3", "16:10", "16:9"] },
    Entry { setting: Setting::Resolution, label: "Resolution", values: &["Window", "416p", "720p", "1080p", "1440p", "2160p"] },
    Entry { setting: Setting::Fullscreen, label: "Fullscreen", values: &["Off", "On"] },
];

/// The row of a setting.
pub fn row_of(s: Setting) -> usize { ENTRIES.iter().position(|e| e.setting == s).unwrap_or(0) }

/// The settings' current values (row order) and each row's selectable values (bit k = value k), filled by the engine
/// every frame; a page's change comes back as [`super::Out::Port`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortValues {
    pub values: Vec<u8>,
    pub choices: Vec<u32>,
}

impl Default for PortValues {
    fn default() -> Self { PortValues { values: vec![0; ENTRIES.len()], choices: vec![u32::MAX; ENTRIES.len()] } }
}

impl PortValues {
    pub fn get(&self, s: Setting) -> u8 { self.values.get(row_of(s)).copied().unwrap_or(0) }

    /// The value after the row's current one among those it allows (wrapping).
    pub fn next(&self, row: usize) -> u8 {
        let n = ENTRIES[row].values.len();
        let (v, choices) = (self.values[row] as usize, self.choices.get(row).copied().unwrap_or(u32::MAX));
        (1..=n).map(|k| (v + k) % n).find(|&i| choices & (1 << i) != 0).unwrap_or(v) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_skips_values_a_row_does_not_allow() {
        let mut p = PortValues::default();
        let r = row_of(Setting::Preset);
        p.choices[r] = 0b011;
        p.values[r] = 1;
        assert_eq!(p.next(r), 0);
        p.values[r] = 2;
        assert_eq!(p.next(r), 0);
    }
}
