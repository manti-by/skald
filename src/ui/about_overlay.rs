//! The About card: name, version, what it plays and what it is built from.

use crate::app::SkaldApp;
use crate::ui::theme::fade;
use gpui::prelude::*;
use gpui::{AnyElement, Context, CursorStyle, FontWeight, MouseButton, div, px};

const CARD_WIDTH: f32 = 440.0;
const CARD_HEIGHT: f32 = 268.0;
const CLOSE_SIZE: f32 = 12.0;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

impl SkaldApp {
    pub(crate) fn render_about_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();

        let panel = div()
            .id("about-panel")
            .relative()
            .w(px(CARD_WIDTH))
            .h(px(CARD_HEIGHT))
            .flex_shrink_0()
            .rounded(px(6.))
            .bg(palette.card)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id("about-close")
                    .absolute()
                    .left(px(CARD_WIDTH - 36.))
                    .top(px(14.))
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
                    .on_click(cx.listener(|this, _, _, cx| this.close_overlay(cx))),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(48.))
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text)
                            .child("SKALD"),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child(format!("Version {VERSION}")),
                    )
                    .child(
                        div()
                            .mt(px(26.))
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(fade(palette.text, 0.9))
                            .child("A focused desktop music player"),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .text_size(px(12.))
                            .line_height(px(19.))
                            .text_color(palette.muted)
                            .child("FLAC, WAV and MP3 with a five-band equalizer"),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .text_size(px(11.))
                            .line_height(px(18.))
                            .text_color(fade(palette.muted, 0.9))
                            .child("Built with Rust and GPUI"),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(11.))
                            .line_height(px(18.))
                            .text_color(fade(palette.muted, 0.75))
                            .child("MIT License · © 2026 manti-by"),
                    ),
            );

        div()
            .id("about-scrim")
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
}
