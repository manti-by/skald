//! The five-band equalizer overlay.
//!
//! The graph is drawn with the same geometry the design frame uses: dashed
//! guides at +6, +3, 0, -3 and -6 dB, one slider per band, and a response curve
//! that passes exactly through the five gain points.

use crate::app::SkaldApp;
use crate::player::{EQ_FREQS, EQ_LABELS};
use crate::ui::theme::{
    EQ_GAIN_RANGE, EQ_HEIGHT, EQ_LINE_HEIGHT, EQ_LINE_LEFT, EQ_LINE_RIGHT, EQ_LINE_TOP,
    EQ_SLIDER_X, EQ_THUMB_HEIGHT, EQ_THUMB_WIDTH, EQ_TRACK_HEIGHT, EQ_TRACK_TOP, EQ_TRACK_WIDTH,
    EQ_WIDTH, gain_to_y, y_to_gain,
};
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, CursorStyle, MouseButton, PathBuilder, Window, div, point, px,
    quad, size,
};

/// How many line segments the response curve is sampled into.
pub const CURVE_SAMPLES: usize = 160;
const GUIDE_STEPS: usize = 4;
const CLOSE_SIZE: f32 = 12.0;

/// The five (x, gain) control points of the response curve.
pub fn band_points(gains: &[f32; EQ_FREQS.len()]) -> [(f32, f32); EQ_FREQS.len()] {
    std::array::from_fn(|index| {
        (
            EQ_SLIDER_X[index],
            gains[index].clamp(-EQ_GAIN_RANGE, EQ_GAIN_RANGE),
        )
    })
}

const CONTROL_POINTS: usize = EQ_FREQS.len() + 2;

/// Spline controls for the response curve: x positions, gains and tangents.
fn controls(
    gains: &[f32; EQ_FREQS.len()],
    left: f32,
    right: f32,
) -> (
    [f32; CONTROL_POINTS],
    [f32; CONTROL_POINTS],
    [f32; CONTROL_POINTS],
) {
    let bands = band_points(gains);
    let mut xs = [0.0; CONTROL_POINTS];
    let mut gains_at = [0.0; CONTROL_POINTS];
    xs[0] = left.min(bands[0].0 - 1.0);
    gains_at[0] = bands[0].1;
    xs[CONTROL_POINTS - 1] = right.max(bands[EQ_FREQS.len() - 1].0 + 1.0);
    gains_at[CONTROL_POINTS - 1] = bands[EQ_FREQS.len() - 1].1;
    for (index, (x, gain)) in bands.iter().enumerate() {
        xs[index + 1] = *x;
        gains_at[index + 1] = *gain;
    }

    let mut tangents = [0.0f32; CONTROL_POINTS];
    for index in 1..CONTROL_POINTS - 1 {
        tangents[index] =
            (gains_at[index + 1] - gains_at[index - 1]) / (xs[index + 1] - xs[index - 1]).max(1.0);
    }
    (xs, gains_at, tangents)
}

/// The gain of the smooth curve at an arbitrary x, in dB.
fn curve_gain(
    xs: &[f32; CONTROL_POINTS],
    gains_at: &[f32; CONTROL_POINTS],
    tangents: &[f32; CONTROL_POINTS],
    x: f32,
) -> f32 {
    let mut segment = 0;
    while segment + 2 < CONTROL_POINTS && x > xs[segment + 1] {
        segment += 1;
    }
    let span = (xs[segment + 1] - xs[segment]).max(1.0);
    let t = ((x - xs[segment]) / span).clamp(0.0, 1.0);
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * gains_at[segment]
        + h10 * span * tangents[segment]
        + h01 * gains_at[segment + 1]
        + h11 * span * tangents[segment + 1]
}

/// Samples the gain curve through the band points, in panel coordinates.
///
/// The curve starts and ends flat, just outside the outermost sliders, which is
/// how the design draws the response running edge to edge.
pub fn response_points(gains: &[f32; EQ_FREQS.len()], left: f32, right: f32) -> Vec<(f32, f32)> {
    let (xs, gains_at, tangents) = controls(gains, left, right);
    (0..=CURVE_SAMPLES)
        .map(|sample| {
            let x = left + (right - left) * sample as f32 / CURVE_SAMPLES as f32;
            (x, gain_to_y(curve_gain(&xs, &gains_at, &tangents, x)))
        })
        .collect()
}

impl SkaldApp {
    pub(crate) fn render_eq_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let gains = self.settings.eq;
        let curve = response_points(&gains, EQ_LINE_LEFT, EQ_LINE_RIGHT);

        let mut panel = div()
            .id("eq-panel")
            .relative()
            .w(px(EQ_WIDTH))
            .h(px(EQ_HEIGHT))
            .flex_shrink_0()
            .rounded(px(4.))
            .border_1()
            .border_color(palette.eq_border)
            .bg(palette.eq_panel)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if let Some(index) = this.eq_drag {
                        this.set_eq_gain_at(index, event.position.y, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.eq_drag.take().is_some() {
                        this.settings_dirty = true;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.eq_drag.take().is_some() {
                        this.settings_dirty = true;
                        cx.notify();
                    }
                }),
            );

        panel = panel.child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    paint_guides(bounds, palette, window);
                    paint_curve(bounds, &curve, palette, window);
                },
            )
            .size_full(),
        );

        for (index, gain) in gains.iter().enumerate() {
            panel = panel.child(self.eq_slider(index, *gain, cx));
        }

        panel = panel.child(
            div()
                .absolute()
                .left(px(26.))
                .top(px(EQ_LINE_TOP - 9.))
                .h(px(16.))
                .flex()
                .items_center()
                .text_size(px(11.))
                .text_color(palette.eq_text)
                .child("+6 dB"),
        );

        panel = panel.child(
            div()
                .absolute()
                .left(px(26.))
                .top(px(EQ_LINE_TOP + EQ_LINE_HEIGHT - 7.))
                .h(px(16.))
                .flex()
                .items_center()
                .text_size(px(11.))
                .text_color(palette.eq_text)
                .child("-6 dB"),
        );

        for (index, label) in EQ_LABELS.iter().enumerate() {
            panel = panel.child(
                div()
                    .absolute()
                    .left(px(EQ_SLIDER_X[index] - 50.))
                    .top(px(EQ_TRACK_TOP + EQ_TRACK_HEIGHT + 8.))
                    .w(px(100.))
                    .flex()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(palette.eq_text)
                    .child(*label),
            );
        }

        panel = panel.child(
            div()
                .id("eq-close")
                .absolute()
                .left(px(637.))
                .top(px(16.))
                .w(px(CLOSE_SIZE))
                .h(px(CLOSE_SIZE))
                .rounded_full()
                .bg(palette.close)
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_overlay(cx);
                })),
        );

        div()
            .id("eq-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.close_overlay(cx);
                }),
            )
            .child(panel)
            .into_any_element()
    }

    fn eq_slider(&self, index: usize, gain: f32, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let x = EQ_SLIDER_X[index];
        let thumb_top = gain_to_y(gain) - EQ_TRACK_TOP;

        div()
            .id(("eq-slider", index))
            .absolute()
            .left(px(x - 24.))
            .top(px(EQ_TRACK_TOP - EQ_THUMB_HEIGHT / 2.0))
            .w(px(48.))
            .h(px(EQ_TRACK_HEIGHT + EQ_THUMB_HEIGHT))
            .cursor(CursorStyle::ResizeUpDown)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.eq_drag = Some(index);
                    this.set_eq_gain_at(index, event.position.y, window, cx);
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(24. - EQ_TRACK_WIDTH / 2.0))
                    .top(px(EQ_THUMB_HEIGHT / 2.0))
                    .w(px(EQ_TRACK_WIDTH))
                    .h(px(EQ_TRACK_HEIGHT))
                    .rounded(px(EQ_TRACK_WIDTH / 2.0))
                    .bg(palette.eq_track),
            )
            .child(
                div()
                    .absolute()
                    .left(px(24. - EQ_THUMB_WIDTH / 2.0))
                    .top(px(thumb_top))
                    .w(px(EQ_THUMB_WIDTH))
                    .h(px(EQ_THUMB_HEIGHT))
                    .rounded(px(4.))
                    .bg(palette.eq_thumb),
            )
            .into_any_element()
    }
}

fn paint_guides(
    bounds: Bounds<gpui::Pixels>,
    palette: crate::ui::theme::Palette,
    window: &mut Window,
) {
    let origin = bounds.origin;
    for step in 0..=GUIDE_STEPS {
        let y = EQ_LINE_TOP + EQ_LINE_HEIGHT * step as f32 / GUIDE_STEPS as f32;
        let mut x = EQ_LINE_LEFT;
        while x < EQ_LINE_RIGHT {
            let width = (EQ_LINE_RIGHT - x).min(3.0);
            window.paint_quad(quad(
                Bounds {
                    origin: origin + point(px(x), px(y)),
                    size: size(px(width), px(1.)),
                },
                px(0.),
                palette.eq_grid,
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
            x += 7.0;
        }
    }
}

fn paint_curve(
    bounds: Bounds<gpui::Pixels>,
    curve: &[(f32, f32)],
    palette: crate::ui::theme::Palette,
    window: &mut Window,
) {
    if curve.len() < 2 {
        return;
    }
    let mut builder = PathBuilder::stroke(px(1.5));
    builder.move_to(bounds.origin + point(px(curve[0].0), px(curve[0].1)));
    for (x, y) in &curve[1..] {
        builder.line_to(bounds.origin + point(px(*x), px(*y)));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, palette.eq_curve);
    }
}

/// Reads a window-space y back into a gain for the centered panel.
pub fn gain_at_window_y(window_height: f32, y: f32) -> f32 {
    let top = (window_height - EQ_HEIGHT) / 2.0;
    y_to_gain(y - top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_passes_through_every_band() {
        let gains = [6.0, -3.0, 0.0, 2.5, -6.0];
        let points = response_points(&gains, EQ_LINE_LEFT, EQ_LINE_RIGHT);
        assert_eq!(points.len(), CURVE_SAMPLES + 1);
        let (xs, gains_at, tangents) = controls(&gains, EQ_LINE_LEFT, EQ_LINE_RIGHT);
        for (index, gain) in gains.iter().enumerate() {
            let y = gain_to_y(curve_gain(&xs, &gains_at, &tangents, EQ_SLIDER_X[index]));
            assert!(
                (y - gain_to_y(*gain)).abs() < 0.05,
                "band {index} at {y} instead of {}",
                gain_to_y(*gain)
            );
        }
        assert!((points[0].0 - EQ_LINE_LEFT).abs() < 0.001);
        assert!((points[CURVE_SAMPLES].0 - EQ_LINE_RIGHT).abs() < 0.001);
    }

    #[test]
    fn a_flat_curve_is_a_straight_line() {
        let points = response_points(&[0.0; 5], EQ_LINE_LEFT, EQ_LINE_RIGHT);
        let middle = gain_to_y(0.0);
        assert!(points.iter().all(|(_, y)| (y - middle).abs() < 0.01));
    }

    #[test]
    fn the_window_gain_round_trips() {
        let gain = gain_at_window_y(768.0, (768.0 - EQ_HEIGHT) / 2.0 + gain_to_y(-4.0));
        assert!((gain + 4.0).abs() < 1e-3);
    }

    #[test]
    fn labels_cover_the_five_bands() {
        assert_eq!(EQ_LABELS, ["63 Hz", "250 Hz", "1 KHz", "4 KHz", "16 KHz"]);
        assert_eq!(EQ_FREQS, [63.0, 250.0, 1_000.0, 4_000.0, 16_000.0]);
    }

    #[test]
    fn secondary_labels_stay_readable() {
        let palette = crate::ui::theme::dark();
        assert!(crate::ui::theme::fade(palette.muted, 0.5).a < palette.muted.a);
    }
}
