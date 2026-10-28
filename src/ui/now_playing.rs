//! Now-playing line, play/pause, volume and the seek bar.

use crate::app::{Drag, SkaldApp};
use crate::library::format_clock;
use crate::ui::theme::{GAP, PAD_X, SIDEBAR_WIDTH, fade};
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, FontWeight, MouseButton, PathBuilder, div, point, px, quad,
    relative, size,
};

pub const VOLUME_WIDTH: f32 = 224.0;
const PROGRESS_HEIGHT: f32 = 6.0;
const THUMB_WIDTH: f32 = 14.0;

impl SkaldApp {
    pub(crate) fn render_main(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let track_list = self.render_track_list(cx);

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(track_list)
            .child(
                div()
                    .mt(px(48.))
                    .h(px(45.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(self.now_playing_text())
                    .child(self.play_button(cx))
                    .child(div().flex_1())
                    .child(self.volume_bar(cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .h(px(12.))
                    .flex_shrink_0()
                    .flex()
                    .items_end()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(palette.muted)
                    .child(div().child(format_clock(self.position)))
                    .child(div().child(format_clock(self.duration()))),
            )
            .child(div().h(px(6.)).flex_shrink_0())
            .child(self.progress_bar(cx))
            .into_any_element()
    }

    fn now_playing_text(&self) -> AnyElement {
        let palette = self.palette();
        let (title, detail, error) = match self.current.and_then(|index| self.tracks.get(index)) {
            Some(track) => (
                format!("{} - {}", track.display_artist(), track.title),
                track.format_label(),
                false,
            ),
            None => (
                "Nothing playing".to_string(),
                if self.loading > 0 {
                    "Scanning your library…".to_string()
                } else {
                    "Add a folder or some files to begin".to_string()
                },
                false,
            ),
        };

        let detail = match (&self.audio_error, error) {
            (Some(message), _) => (message.clone(), true),
            (None, _) => (detail, false),
        };

        div()
            .w(px(380.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .text_size(px(18.))
                    .line_height(px(22.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(palette.text)
                    .truncate()
                    .child(title),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(13.))
                    .line_height(px(17.))
                    .text_color(if detail.1 {
                        palette.error
                    } else {
                        palette.muted
                    })
                    .truncate()
                    .child(detail.0),
            )
            .into_any_element()
    }

    fn play_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let playing = self.is_playing();
        let enabled = self.current.is_some() || !self.selected.is_empty();
        let color = if enabled {
            fade(palette.text, 0.82)
        } else {
            fade(palette.text, 0.25)
        };

        div()
            .id("play-pause")
            .w(px(36.))
            .h(px(34.))
            .flex_shrink_0()
            .cursor(gpui::CursorStyle::PointingHand)
            .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx)))
            .child(
                gpui::canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        let origin = bounds.origin;
                        if playing {
                            let bar = Bounds {
                                origin: origin + point(px(9.), px(4.)),
                                size: size(px(7.), px(26.)),
                            };
                            window.paint_quad(quad(
                                bar,
                                px(1.),
                                color,
                                px(0.),
                                gpui::transparent_black(),
                                Default::default(),
                            ));
                            let bar = Bounds {
                                origin: origin + point(px(21.), px(4.)),
                                size: size(px(7.), px(26.)),
                            };
                            window.paint_quad(quad(
                                bar,
                                px(1.),
                                color,
                                px(0.),
                                gpui::transparent_black(),
                                Default::default(),
                            ));
                        } else {
                            let mut builder = PathBuilder::fill();
                            builder.move_to(origin + point(px(10.), px(4.)));
                            builder.line_to(origin + point(px(30.), px(17.)));
                            builder.line_to(origin + point(px(10.), px(30.)));
                            builder.close();
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, color);
                            }
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }

    fn volume_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let fraction = self.settings.volume.clamp(0.0, 1.0);

        div()
            .id("volume")
            .relative()
            .w(px(VOLUME_WIDTH))
            .h(px(PROGRESS_HEIGHT))
            .flex_shrink_0()
            .rounded(px(3.))
            .bg(palette.scroll_track)
            .cursor(gpui::CursorStyle::PointingHand)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.drag = Some(Drag::Volume);
                    this.set_volume_at(event.position.x, window, cx);
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if this.drag == Some(Drag::Volume) {
                        this.set_volume_at(event.position.x, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    this.settings_dirty = true;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    this.settings_dirty = true;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(fraction))
                    .rounded(px(3.))
                    .bg(palette.bar_fill)
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(px(THUMB_WIDTH * fraction.max(0.06)))
                            .h_full()
                            .rounded(px(3.))
                            .bg(palette.scroll_thumb),
                    ),
            )
            .into_any_element()
    }

    fn progress_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let duration = self.duration();
        let fraction = if duration > 0.0 {
            (self.position / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        div()
            .id("progress")
            .relative()
            .mx(px(1.))
            .h(px(PROGRESS_HEIGHT))
            .flex_shrink_0()
            .rounded(px(3.))
            .bg(palette.scroll_track)
            .cursor(gpui::CursorStyle::PointingHand)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.drag = Some(Drag::Progress);
                    this.seek_at(event.position.x, window, cx);
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if this.drag == Some(Drag::Progress) {
                        this.seek_at(event.position.x, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    this.save_position();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    this.save_position();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(fraction))
                    .rounded(px(3.))
                    .bg(palette.bar_fill)
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(px(THUMB_WIDTH))
                            .h_full()
                            .rounded(px(3.))
                            .bg(palette.scroll_thumb),
                    ),
            )
            .into_any_element()
    }
}

/// Main-column left edge and width in window coordinates.
pub fn main_x(viewport_width: f32) -> (f32, f32) {
    let left = PAD_X + SIDEBAR_WIDTH + GAP;
    let width = (viewport_width - PAD_X * 2.0 - SIDEBAR_WIDTH - GAP).max(0.0);
    (left, width)
}

/// Volume slider left edge in window coordinates.
pub fn volume_x(viewport_width: f32) -> f32 {
    viewport_width - PAD_X - VOLUME_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::FRAME_WIDTH;

    #[test]
    fn main_column_matches_the_design_frame() {
        let (left, width) = main_x(FRAME_WIDTH);
        assert_eq!(left, 482.0);
        assert_eq!(width, 761.0);
        assert_eq!(volume_x(FRAME_WIDTH), 1019.0);
    }
}
