//! Skald: a focused desktop music player.

mod actions;
mod app;
mod library;
mod player;
mod settings;
mod ui;

use crate::actions::{
    About, AddFiles, AddFolder, PlayPause, PlaySelected, Quit, RemoveSelected, SelectNext,
    SelectPrevious, ToggleEqualizer,
};
use crate::app::SkaldApp;
use crate::settings::Settings;
use crate::ui::theme::{FRAME_HEIGHT, FRAME_WIDTH};
use gpui::{
    App, Application, Bounds, Menu, MenuItem, SystemMenuType, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, prelude::*, px, size,
};

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Regular.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Medium.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-SemiBold.ttf")),
            ])
            .expect("failed to load the bundled Roboto fonts");

        cx.bind_keys(actions::bind_keys());
        cx.set_menus(menus());
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

        let bounds = restored_bounds(cx);
        #[cfg(target_os = "linux")]
        let window_decorations = Some(WindowDecorations::Client);
        #[cfg(not(target_os = "linux"))]
        let window_decorations: Option<WindowDecorations> = None;

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(FRAME_WIDTH), px(FRAME_HEIGHT))),
                    // The OS titlebar is hidden; `titlebar.rs` draws the three
                    // buttons and the drag strip in the top 30px of the frame.
                    titlebar: Some(TitlebarOptions {
                        title: Some("Skald".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(-100.), px(14.))),
                    }),
                    window_decorations,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(SkaldApp::new);
                    window.focus(&view.read(cx).focus);

                    let weak = view.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        let bounds = window.bounds();
                        weak.update(cx, |app, _| {
                            app.remember_window(bounds);
                            app.shutdown();
                        })
                        .ok();
                        true
                    });

                    view.update(cx, |app, cx| app.startup(cx));
                    view
                },
            )
            .expect("failed to open the main window");

        window
            .update(cx, |_, window, _| window.activate_window())
            .ok();
        cx.activate(true);
    });
}

/// Reopens the window where it was last closed, at no less than the design
/// frame so the 1320x768 layout always fits.
fn restored_bounds(cx: &App) -> Bounds<gpui::Pixels> {
    let saved = Settings::load(&settings::config_dir()).window;
    match saved {
        Some(window) => Bounds {
            origin: point(px(window.x), px(window.y)),
            size: size(
                px(window.width.max(FRAME_WIDTH)),
                px(window.height.max(FRAME_HEIGHT)),
            ),
        },
        None => Bounds::centered(None, size(px(FRAME_WIDTH), px(FRAME_HEIGHT)), cx),
    }
}

/// The native menu bar. Every entry is an action, so the shortcuts bound in
/// [`actions::bind_keys`] show up beside the menu titles automatically.
fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Skald".into(),
            items: vec![
                MenuItem::action("About Skald", About),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit Skald", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Add Folder…", AddFolder),
                MenuItem::action("Add Files…", AddFiles),
            ],
        },
        Menu {
            name: "Playback".into(),
            items: vec![
                MenuItem::action("Play / Pause", PlayPause),
                MenuItem::action("Play Selected", PlaySelected),
                MenuItem::action("Remove Selected", RemoveSelected),
                MenuItem::separator(),
                MenuItem::action("Previous Track", SelectPrevious),
                MenuItem::action("Next Track", SelectNext),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![MenuItem::action("Equalizer…", ToggleEqualizer)],
        },
    ]
}
