//! Design-frame geometry and the two palettes.
//!
//! MNT-242 is drawn on a 1320x768 frame with 77 px side gutters, an 86 px top
//! gutter and an 87 px bottom gutter. The values here are the single source of
//! truth for every region; views must not invent their own spacing.

use gpui::Rgba;

pub const FRAME_WIDTH: f32 = 1320.0;
pub const FRAME_HEIGHT: f32 = 768.0;
pub const PAD_TOP: f32 = 86.0;
pub const PAD_X: f32 = 77.0;
pub const PAD_BOTTOM: f32 = 87.0;
pub const GAP: f32 = 55.0;

pub const SIDEBAR_WIDTH: f32 = 350.0;
pub const ART_SIZE: f32 = 350.0;
pub const SPECTRUM_HEIGHT: f32 = 197.0;
pub const ART_GAP: f32 = 48.0;

pub const PANEL_HEIGHT: f32 = 444.0;
pub const ROW_HEIGHT: f32 = 37.0;
pub const VISIBLE_ROWS: usize = 10;
pub const SCROLLBAR_WIDTH: f32 = 6.0;
pub const SCROLLBAR_TOP: f32 = 27.0;
pub const SCROLLBAR_THUMB_HEIGHT: f32 = 25.0;

/// Left inset shared by the header and every row.
pub const LIST_INSET: f32 = 11.0;
/// Cell widths, in the order Index, Artist, Title, Album, Duration. Bitrate
/// takes the rest of the panel so it stays flush with the right edge.
pub const COL_INDEX: f32 = 69.0;
pub const COL_ARTIST: f32 = 174.0;
pub const COL_TITLE: f32 = 196.0;
pub const COL_ALBUM: f32 = 174.0;
pub const COL_DURATION: f32 = 89.0;

pub const EQ_WIDTH: f32 = 666.0;
pub const EQ_HEIGHT: f32 = 443.0;
/// First and last dashed guide lines inside the panel.
pub const EQ_LINE_TOP: f32 = 77.0;
pub const EQ_LINE_HEIGHT: f32 = 288.0;
pub const EQ_LINE_LEFT: f32 = 91.0;
pub const EQ_LINE_RIGHT: f32 = 583.0;
/// Slider track extends eight pixels past the +6/-6 guide lines.
pub const EQ_TRACK_TOP: f32 = 69.0;
pub const EQ_TRACK_HEIGHT: f32 = 304.0;
pub const EQ_TRACK_WIDTH: f32 = 8.0;
pub const EQ_THUMB_WIDTH: f32 = 8.0;
pub const EQ_THUMB_HEIGHT: f32 = 25.0;
pub const EQ_SLIDER_X: [f32; 5] = [130.0, 230.0, 330.0, 430.0, 530.0];
pub const EQ_GAIN_RANGE: f32 = 6.0;

/// Average cover luminance at or above this selects the light palette.
pub const LIGHT_THRESHOLD: f32 = 0.42;

/// Multiplies a colour's alpha, for secondary text and hairlines.
pub fn fade(color: Rgba, factor: f32) -> Rgba {
    Rgba {
        a: color.a * factor.clamp(0.0, 1.0),
        ..color
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub overlay: Rgba,
    pub panel: Rgba,
    pub card: Rgba,
    pub selected: Rgba,
    pub hover: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub scroll_track: Rgba,
    pub scroll_thumb: Rgba,
    pub bar_fill: Rgba,
    pub eq_panel: Rgba,
    pub eq_track: Rgba,
    pub eq_thumb: Rgba,
    pub eq_curve: Rgba,
    pub eq_grid: Rgba,
    pub eq_text: Rgba,
    pub eq_border: Rgba,
    pub close: Rgba,
    pub placeholder_top: Rgba,
    pub placeholder_bottom: Rgba,
    pub error: Rgba,
}

/// Picks the palette from the current cover. No cover means dark.
pub fn palette_for(luminance: Option<f32>) -> Palette {
    if luminance.is_some_and(|luminance| luminance >= LIGHT_THRESHOLD) {
        light()
    } else {
        dark()
    }
}

pub fn dark() -> Palette {
    Palette {
        overlay: gpui::rgba(0x1E1E1EBF),
        panel: gpui::rgba(0x1E1E1E80),
        card: gpui::rgba(0x4C4C4CE6),
        selected: gpui::rgba(0x00000080),
        hover: gpui::rgba(0xFFFFFF14),
        text: gpui::rgb(0xF2F2F2),
        muted: gpui::rgb(0x888888),
        scroll_track: gpui::rgba(0x01010155),
        scroll_thumb: gpui::rgba(0x88888855),
        bar_fill: gpui::rgba(0x88888866),
        eq_panel: gpui::rgba(0x4C4C4CE6),
        eq_track: gpui::rgba(0x01010180),
        eq_thumb: gpui::rgb(0x525252),
        eq_curve: gpui::rgba(0xD9D9D9B3),
        eq_grid: gpui::rgba(0xFFFFFF33),
        eq_text: gpui::rgb(0x888888),
        eq_border: gpui::rgba(0x1E1E1EBF),
        close: gpui::rgb(0xFF5F59),
        placeholder_top: gpui::rgb(0x2A2A2A),
        placeholder_bottom: gpui::rgb(0x101010),
        error: gpui::rgb(0xE07A6A),
    }
}

pub fn light() -> Palette {
    Palette {
        overlay: gpui::rgba(0xF7F7F7B8),
        panel: gpui::rgba(0xFFFFFF8C),
        card: gpui::rgba(0xF4F4F4EE),
        selected: gpui::rgba(0x00000024),
        hover: gpui::rgba(0x0000000D),
        text: gpui::rgb(0x1B1B1B),
        muted: gpui::rgb(0x6E6E6E),
        scroll_track: gpui::rgba(0x0000001F),
        scroll_thumb: gpui::rgba(0x6E6E6E55),
        bar_fill: gpui::rgba(0x00000033),
        eq_panel: gpui::rgba(0x1E1E1EE6),
        eq_track: gpui::rgba(0x01010180),
        eq_thumb: gpui::rgb(0x525252),
        eq_curve: gpui::rgba(0xFFFFFF80),
        eq_grid: gpui::rgba(0xFFFFFF40),
        eq_text: gpui::rgb(0xAAAAAA),
        eq_border: gpui::rgba(0x1E1E1EBF),
        close: gpui::rgb(0xFF5F59),
        placeholder_top: gpui::rgb(0xE8E8E8),
        placeholder_bottom: gpui::rgb(0xCFCFCF),
        error: gpui::rgb(0xB2402F),
    }
}

/// Gain in dB to a y offset inside the EQ graph, in panel coordinates.
pub fn gain_to_y(gain: f32) -> f32 {
    EQ_LINE_TOP
        + (EQ_GAIN_RANGE - gain.clamp(-EQ_GAIN_RANGE, EQ_GAIN_RANGE)) / (EQ_GAIN_RANGE * 2.0)
            * EQ_LINE_HEIGHT
}

/// y offset inside the EQ graph back to a gain in dB.
pub fn y_to_gain(y: f32) -> f32 {
    let position = ((y - EQ_LINE_TOP) / EQ_LINE_HEIGHT).clamp(0.0, 1.0);
    EQ_GAIN_RANGE - position * EQ_GAIN_RANGE * 2.0
}

/// Where the scroll offset lands when `target` must become visible.
pub fn scroll_to_show(scroll: usize, total: usize, target: usize) -> usize {
    let max = total.saturating_sub(VISIBLE_ROWS);
    if target < scroll {
        target
    } else if target >= scroll + VISIBLE_ROWS {
        (target + 1).saturating_sub(VISIBLE_ROWS)
    } else {
        scroll
    }
    .min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_selects_the_palette() {
        assert_eq!(palette_for(None).text, dark().text);
        assert_eq!(palette_for(Some(0.1)).text, dark().text);
        assert_eq!(palette_for(Some(0.9)).text, light().text);
        assert_eq!(palette_for(Some(LIGHT_THRESHOLD)).text, light().text);
    }

    #[test]
    fn gain_mapping_round_trips() {
        for gain in [-6.0, -2.5, 0.0, 3.0, 6.0] {
            let y = gain_to_y(gain);
            assert!((y_to_gain(y) - gain).abs() < 1e-4, "gain {gain}");
        }
        assert_eq!(gain_to_y(6.0), EQ_LINE_TOP);
        assert_eq!(gain_to_y(-6.0), EQ_LINE_TOP + EQ_LINE_HEIGHT);
    }

    #[test]
    fn scrolling_reveals_the_target_row() {
        assert_eq!(scroll_to_show(0, 30, 0), 0);
        assert_eq!(scroll_to_show(0, 30, 12), 3);
        assert_eq!(scroll_to_show(8, 30, 3), 3);
        assert_eq!(scroll_to_show(0, 5, 4), 0);
        assert_eq!(scroll_to_show(100, 30, 29), 20);
    }
}
