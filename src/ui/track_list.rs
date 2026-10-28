//! The one flat track list, its header and the custom scrollbar.

use crate::app::SkaldApp;
use crate::ui::theme::{
    COL_ALBUM, COL_ARTIST, COL_DURATION, COL_INDEX, COL_TITLE, LIST_INSET, PANEL_HEIGHT,
    ROW_HEIGHT, SCROLLBAR_THUMB_HEIGHT, SCROLLBAR_TOP, SCROLLBAR_WIDTH, VISIBLE_ROWS, fade,
};
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, MouseButton, SharedString, div, px};

impl SkaldApp {
    pub(crate) fn render_track_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let mut panel = div()
            .id("track-list")
            .relative()
            .w_full()
            .h(px(PANEL_HEIGHT))
            .flex_shrink_0()
            .rounded(px(4.))
            .bg(palette.panel)
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                this.scroll_by(event.delta, cx);
            }));

        panel = panel.child(
            div()
                .h(px(ROW_HEIGHT))
                .flex_shrink_0()
                .flex()
                .items_center()
                .pl(px(LIST_INSET))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.muted)
                .child(header_cell("Index", COL_INDEX))
                .child(header_cell("Artist", COL_ARTIST))
                .child(header_cell("Title", COL_TITLE))
                .child(header_cell("Album", COL_ALBUM))
                .child(header_cell("Duration", COL_DURATION))
                .child(header_cell("Bitrate", 0.0)),
        );

        if self.tracks.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(fade(palette.muted, 0.8))
                    .child(if self.loading > 0 {
                        "Scanning your library…"
                    } else {
                        "No tracks yet"
                    }),
            );
        } else {
            let end = (self.scroll + VISIBLE_ROWS).min(self.tracks.len());
            for index in self.scroll..end {
                panel = panel.child(self.track_row(index, cx));
            }
        }

        if self.tracks.len() > VISIBLE_ROWS {
            panel = panel.child(self.scrollbar());
        }

        panel.into_any_element()
    }

    fn track_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();
        let track = &self.tracks[index];
        let selected = self.selected.contains(&index);
        let current = self.current == Some(index);

        let mut row = div()
            .id(SharedString::from(format!("row-{index}")))
            .h(px(ROW_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .pl(px(LIST_INSET))
            .text_size(px(13.))
            .text_color(palette.text)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.click_row(index, event.modifiers, event.click_count, cx);
                }),
            );

        if selected || current {
            row = row.bg(palette.selected);
        } else {
            row = row.hover(|style| style.bg(palette.hover));
        }

        row.child(text_cell((index + 1).to_string(), COL_INDEX))
            .child(text_cell(track.display_artist().to_string(), COL_ARTIST))
            .child(text_cell(track.title.clone(), COL_TITLE))
            .child(text_cell(track.display_album().to_string(), COL_ALBUM))
            .child(text_cell(track.duration_label(), COL_DURATION))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(if track.bitrate_kbps > 0 {
                        track.bitrate_kbps.to_string()
                    } else {
                        "—".to_string()
                    }),
            )
            .into_any_element()
    }

    fn scrollbar(&self) -> AnyElement {
        let palette = self.palette();
        let track_height = PANEL_HEIGHT - SCROLLBAR_TOP;
        let max_scroll = self.max_scroll();
        let thumb_height = SCROLLBAR_THUMB_HEIGHT;
        let travel = (track_height - thumb_height).max(0.0);
        let thumb_top = if max_scroll == 0 {
            0.0
        } else {
            travel * self.scroll as f32 / max_scroll as f32
        };

        div()
            .absolute()
            .top(px(SCROLLBAR_TOP))
            .right_0()
            .w(px(SCROLLBAR_WIDTH))
            .h(px(track_height))
            .bg(palette.scroll_track)
            .child(
                div()
                    .absolute()
                    .top(px(thumb_top))
                    .left_0()
                    .w(px(SCROLLBAR_WIDTH))
                    .h(px(thumb_height))
                    .rounded(px(3.))
                    .bg(palette.scroll_thumb),
            )
            .into_any_element()
    }
}

fn header_cell(label: &'static str, width: f32) -> AnyElement {
    let mut cell = div().h_full().flex().items_center().truncate();
    if width > 0.0 {
        cell = cell.w(px(width));
    } else {
        cell = cell.flex_1().min_w_0();
    }
    cell.child(label).into_any_element()
}

fn text_cell(value: String, width: f32) -> AnyElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .truncate()
        .child(value)
        .into_any_element()
}
