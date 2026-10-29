//! Custom window chrome: three traffic-light buttons and the drag strip that
//! replaces the native titlebar.
//!
//! The strip occupies the top 30px of the window, inside the design frame's
//! 86px gutter. Everything except the buttons starts a window drag, exactly
//! like the OS titlebar used to.

use crate::app::SkaldApp;
use gpui::prelude::*;
use gpui::{AnyElement, Context, CursorStyle, MouseButton, Window, div, px, rgb, rgba};

pub const TITLEBAR_HEIGHT: f32 = 30.0;
const BUTTON_SIZE: f32 = 12.0;
const BUTTON_LEFT: f32 = 20.0;
const BUTTON_GAP: f32 = 8.0;

const CLOSE: u32 = 0xFF5F57;
const MINIMIZE: u32 = 0xFEBC2E;
const MAXIMIZE: u32 = 0x28C840;
const GLYPH: u32 = 0x00000066;

impl SkaldApp {
    pub(crate) fn render_titlebar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(TITLEBAR_HEIGHT))
            .flex()
            .items_center()
            .pl(px(BUTTON_LEFT))
            .gap(px(BUTTON_GAP))
            .cursor(CursorStyle::OpenHand)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    begin_window_move(window);
                }),
            )
            .child(Self::chrome_button(
                "chrome-close",
                CLOSE,
                "×",
                cx,
                |this, window, cx| this.close_window(window, cx),
            ))
            .child(Self::chrome_button(
                "chrome-minimize",
                MINIMIZE,
                "−",
                cx,
                |_, window, _| window.minimize_window(),
            ))
            .child(Self::chrome_button(
                "chrome-maximize",
                MAXIMIZE,
                "+",
                cx,
                |_, window, _| window.zoom_window(),
            ))
            .into_any_element()
    }

    fn chrome_button(
        id: &'static str,
        color: u32,
        glyph: &'static str,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .relative()
            .w(px(BUTTON_SIZE))
            .h(px(BUTTON_SIZE))
            .flex_shrink_0()
            .rounded_full()
            .bg(rgb(color))
            .cursor(CursorStyle::PointingHand)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.))
            .line_height(px(BUTTON_SIZE))
            .text_color(rgba(GLYPH))
            .hover(|style| style.opacity(0.72))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                action(this, window, cx);
            }))
            .child(glyph)
            .into_any_element()
    }
}

/// Hands the drag to the platform. macOS needs the original mouse-down event;
/// Linux compositors take over from the current pointer press.
fn begin_window_move(window: &Window) {
    #[cfg(target_os = "macos")]
    {
        macos::begin_drag();
        let _ = window;
    }

    #[cfg(not(target_os = "macos"))]
    window.start_window_move();
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    pub fn begin_drag() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(window) = app.keyWindow() else {
            return;
        };
        let Some(event) = app.currentEvent() else {
            return;
        };
        window.performWindowDragWithEvent(&event);
    }
}
