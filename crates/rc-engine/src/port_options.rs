//! The Port Options rows (rc_game::menus::port) against the engine's settings: the graphics options
//! (crate::graphics) and the display's aspect, resolution and fullscreen (crate::display). A change is applied to
//! the resource (its systems pick it up) and saved to the port settings file.

use crate::display::{Aspect, DisplaySettings, Resolution};
use crate::graphics::{AntiAliasing, Detail, GfxOption, GraphicsSettings, Hud, Preset, Textures};
use rc_game::menus::port::{row_of, PortValues, Setting};

/// The rows' values now; the preset row cannot be set to Custom.
pub fn values(g: &GraphicsSettings, d: &DisplaySettings) -> PortValues {
    let mut p = PortValues::default();
    let mut set = |s: Setting, v: u8| p.values[row_of(s)] = v;
    set(Setting::Preset, match g.preset() { Preset::Original => 0, Preset::Enhanced => 1, Preset::Custom => 2 });
    set(Setting::AntiAliasing, g.aa.index());
    set(Setting::Detail, g.detail.index());
    set(Setting::Textures, g.textures.index());
    set(Setting::Hud, g.hud.index());
    set(Setting::Aspect, Aspect::ALL.iter().position(|&a| a == d.aspect).unwrap_or(0) as u8);
    set(Setting::Resolution, Resolution::ALL.iter().position(|&r| r == d.resolution).unwrap_or(0) as u8);
    set(Setting::Fullscreen, d.fullscreen as u8);
    p.choices[row_of(Setting::Preset)] = 0b011;
    p
}

fn pick<T: GfxOption>(v: u8) -> T { T::ALL.get(v as usize).copied().unwrap_or(T::ALL[0]) }

/// Setting `s` to value `v` (a row's index), saved.
pub fn apply(s: Setting, v: u8, g: &mut GraphicsSettings, d: &mut DisplaySettings) {
    match s {
        Setting::Preset => *g = g.with_preset(if v == 1 { Preset::Enhanced } else { Preset::Original }),
        Setting::AntiAliasing => g.aa = pick::<AntiAliasing>(v),
        Setting::Detail => g.detail = pick::<Detail>(v),
        Setting::Textures => g.textures = pick::<Textures>(v),
        Setting::Hud => g.hud = pick::<Hud>(v),
        Setting::Aspect => d.aspect = Aspect::ALL.get(v as usize).copied().unwrap_or(Aspect::Original),
        Setting::Resolution => d.resolution = Resolution::ALL.get(v as usize).copied().unwrap_or(Resolution::Window),
        Setting::Fullscreen => d.fullscreen = v != 0,
    }
    match s {
        Setting::Aspect | Setting::Resolution | Setting::Fullscreen => d.save(),
        _ => g.save(),
    }
}
