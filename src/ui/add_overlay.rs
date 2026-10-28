//! The empty-library card: two ways to fill the playlist.

use crate::app::SkaldApp;
use crate::ui::theme::fade;
use gpui::prelude::*;
use gpui::{AnyElement, Context, CursorStyle, FontWeight, div, px};

const CARD_WIDTH: f32 = 440.0;
const CARD_HEIGHT: f32 = 208.0;
const BUTTON_WIDTH: f32 = 160.0;
const BUTTON_HEIGHT: f32 = 38.0;

impl SkaldApp {
    pub(crate) fn render_add_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(CARD_WIDTH))
                    .h(px(CARD_HEIGHT))
                    .rounded(px(6.))
                    .bg(palette.card)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(18.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text)
                            .child("Your library is empty"),
                    )
                    .child(
                        div()
                            .mt(px(6.))
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(palette.muted)
                            .child("Add a folder, or pick individual files."),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .gap(px(14.))
                            .child(
                                div()
                                    .id("add-folder")
                                    .w(px(BUTTON_WIDTH))
                                    .h(px(BUTTON_HEIGHT))
                                    .rounded(px(3.))
                                    .bg(palette.bar_fill)
                                    .cursor(CursorStyle::PointingHand)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .text_color(palette.text)
                                    .hover(|style| style.bg(palette.selected))
                                    .on_click(cx.listener(|this, _, _, cx| this.add_folder(cx)))
                                    .child("Add Folder…"),
                            )
                            .child(
                                div()
                                    .id("add-files")
                                    .w(px(BUTTON_WIDTH))
                                    .h(px(BUTTON_HEIGHT))
                                    .rounded(px(3.))
                                    .bg(palette.bar_fill)
                                    .cursor(CursorStyle::PointingHand)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .text_color(palette.text)
                                    .hover(|style| style.bg(palette.selected))
                                    .on_click(cx.listener(|this, _, _, cx| this.add_files(cx)))
                                    .child("Add Files…"),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(16.))
                            .text_size(px(10.))
                            .text_color(fade(palette.muted, 0.8))
                            .child("FLAC · WAV · MP3"),
                    ),
            )
            .into_any_element()
    }
}
