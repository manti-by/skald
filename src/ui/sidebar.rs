//! Album art and the live spectrogram in the left column.

use crate::app::SkaldApp;
use crate::ui::theme::{ART_GAP, ART_SIZE, SPECTRUM_HEIGHT, fade};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, ObjectFit, div, img, linear_color_stop, linear_gradient, px,
};

impl SkaldApp {
    pub(crate) fn render_sidebar(&self, _cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette();

        let art = match &self.cover {
            Some(image) => img(image.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .bg(linear_gradient(
                    160.,
                    linear_color_stop(palette.placeholder_top, 0.0),
                    linear_color_stop(palette.placeholder_bottom, 1.0),
                ))
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fade(palette.muted, 0.55))
                        .child("SKALD"),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(fade(palette.muted, 0.5))
                        .child("NO COVER"),
                )
                .into_any_element(),
        };

        let spectrum: AnyElement = match &self.spectrum_image {
            Some(image) => img(image.clone())
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .object_fit(ObjectFit::Fill)
                .into_any_element(),
            None => div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(gpui::black())
                .into_any_element(),
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(ART_GAP))
            .child(
                div()
                    .w(px(ART_SIZE))
                    .h(px(ART_SIZE))
                    .flex_shrink_0()
                    .rounded(px(4.))
                    .overflow_hidden()
                    .child(art),
            )
            .child(
                div()
                    .w(px(ART_SIZE))
                    .flex_1()
                    .min_h(px(SPECTRUM_HEIGHT))
                    .relative()
                    .overflow_hidden()
                    .bg(gpui::black())
                    .child(spectrum),
            )
            .into_any_element()
    }
}
