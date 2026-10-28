//! The window skeleton: blurred backdrop, tint, and the two-column stage.

use crate::player::spectrum::ROWS;
use crate::ui::theme::{GAP, PAD_BOTTOM, PAD_TOP, PAD_X, SIDEBAR_WIDTH};
use gpui::prelude::*;
use gpui::{
    AnyElement, Div, ObjectFit, RenderImage, div, img, linear_color_stop, linear_gradient, px,
};
use image::{Frame, RgbaImage};
use std::collections::VecDeque;
use std::sync::Arc;

use super::theme::Palette;

pub const SPECTRUM_COLUMNS: usize = 350;

/// Full-window album-art backdrop and the colour wash over it.
pub fn backdrop(background: Option<Arc<RenderImage>>, palette: Palette) -> Div {
    let art = match background {
        Some(image) => img(image)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        None => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(linear_gradient(
                180.,
                linear_color_stop(palette.placeholder_top, 0.0),
                linear_color_stop(palette.placeholder_bottom, 1.0),
            ))
            .into_any_element(),
    };

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(art)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(palette.overlay),
        )
}

/// Places the sidebar and the main column inside the design gutters.
pub fn stage(sidebar: AnyElement, main: AnyElement) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .px(px(PAD_X))
        .pt(px(PAD_TOP))
        .pb(px(PAD_BOTTOM))
        .flex()
        .gap(px(GAP))
        .child(
            div()
                .w(px(SIDEBAR_WIDTH))
                .h_full()
                .flex_shrink_0()
                .child(sidebar),
        )
        .child(div().flex_1().h_full().min_w_0().child(main))
}

/// Converts an RGBA bitmap into a GPUI texture. GPUI stores BGRA.
pub fn render_image(mut image: RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(image)]))
}

/// Builds the 350x197 spectrogram from the waterfall history. Newest columns
/// sit on the right; low frequencies sit on the bottom.
pub fn spectrum_bitmap(history: &VecDeque<Vec<f32>>) -> Arc<RenderImage> {
    let mut image = RgbaImage::new(SPECTRUM_COLUMNS as u32, ROWS as u32);
    let first_x = SPECTRUM_COLUMNS.saturating_sub(history.len());

    for (column_index, column) in history.iter().enumerate() {
        let x = (first_x + column_index) as u32;
        for (row, value) in column.iter().take(ROWS).enumerate() {
            let intensity = value.clamp(0.0, 1.0).powf(0.8);
            let shade = (intensity * 236.0) as u8;
            let y = (ROWS - 1 - row) as u32;
            image.put_pixel(x, y, image::Rgba([shade, shade, shade, 255]));
        }
    }

    render_image(image)
}
