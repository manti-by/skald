//! Actions the native menu and the keyboard share.
//!
//! Every menu entry names one of these units, and [`bind_keys`] gives it a
//! shortcut. Keeping both in one place means the menu and the keyboard can
//! never drift apart.

use gpui::{KeyBinding, actions};

actions!(
    skald,
    [
        AddFolder,
        AddFiles,
        ToggleEqualizer,
        About,
        PlayPause,
        PlaySelected,
        RemoveSelected,
        SelectPrevious,
        SelectNext,
        CloseOverlay,
        Quit,
    ]
);

/// `cmd` on macOS and `ctrl` elsewhere.
pub const MOD: &str = if cfg!(target_os = "macos") {
    "cmd"
} else {
    "ctrl"
};

/// The shortcuts documented in `wiki/docs/MORE.md`.
pub fn bind_keys() -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new("space", PlayPause, None),
        KeyBinding::new("enter", PlaySelected, None),
        KeyBinding::new("delete", RemoveSelected, None),
        KeyBinding::new("backspace", RemoveSelected, None),
        KeyBinding::new("up", SelectPrevious, None),
        KeyBinding::new("down", SelectNext, None),
        KeyBinding::new("escape", CloseOverlay, None),
    ];
    bindings.push(KeyBinding::new(&format!("{MOD}-o"), AddFolder, None));
    bindings.push(KeyBinding::new(&format!("{MOD}-shift-o"), AddFiles, None));
    bindings.push(KeyBinding::new(&format!("{MOD}-e"), ToggleEqualizer, None));
    bindings.push(KeyBinding::new(&format!("{MOD}-i"), About, None));
    bindings.push(KeyBinding::new(&format!("{MOD}-q"), Quit, None));
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_every_shortcut_from_the_docs() {
        let bindings = bind_keys();
        assert_eq!(bindings.len(), 12);

        let keys: Vec<_> = bindings
            .iter()
            .map(|binding| binding.keystrokes()[0].key().to_string())
            .collect();
        for key in [
            "space",
            "enter",
            "delete",
            "backspace",
            "up",
            "down",
            "escape",
        ] {
            assert!(keys.iter().any(|bound| bound == key), "missing {key}");
        }
        for key in ["o", "e", "i", "q"] {
            assert!(keys.iter().any(|bound| bound == key), "missing {key}");
        }

        let add_files = bindings
            .iter()
            .find(|binding| {
                let keystroke = &binding.keystrokes()[0];
                keystroke.key() == "o" && keystroke.modifiers().shift
            })
            .expect("shift-o binding");
        let modifiers = add_files.keystrokes()[0].modifiers();
        if cfg!(target_os = "macos") {
            assert!(modifiers.platform);
        } else {
            assert!(modifiers.control);
        }
    }
}
