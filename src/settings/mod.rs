//! On-disk settings and playlist state.
//!
//! Skald keeps two JSON files in [`config_dir`]: `settings.json` (window,
//! volume, equalizer) and `state.json` (playlist and last playback position).
//! Both are created on first run and written through a temporary file followed
//! by a rename so an interrupted write can never truncate the previous copy.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const EQ_BANDS: usize = 5;
pub const MIN_VOLUME: f32 = 0.0;
pub const MAX_VOLUME: f32 = 1.0;

/// Directory that owns both JSON files.
///
/// Linux honours `$XDG_CONFIG_HOME`; macOS always uses `~/.config/skald`, as
/// the ticket requires, instead of `~/Library/Application Support`.
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(dir).join("skald");
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("skald")
}

/// The geometry of the main window, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl WindowState {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// `settings.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub window: Option<WindowState>,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default, rename = "eq", alias = "eq_gains")]
    pub eq: [f32; EQ_BANDS],
}

fn default_volume() -> f32 {
    0.8
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window: None,
            volume: default_volume(),
            eq: [0.0; EQ_BANDS],
        }
    }
}

impl Settings {
    /// Reads `settings.json`, returning defaults when the file is absent or
    /// unreadable. Invalid values are clamped rather than rejected.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        let mut settings = match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Self>(&contents) {
                Ok(settings) => settings,
                Err(error) => {
                    log::warn!("ignoring invalid {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("could not read {}: {error}", path.display());
                Self::default()
            }
        };
        settings.sanitize();
        settings
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let mut settings = self.clone();
        settings.sanitize();
        write_json_atomic(&dir.join("settings.json"), &settings)
    }

    pub fn sanitize(&mut self) {
        if !self.volume.is_finite() {
            self.volume = default_volume();
        }
        self.volume = self.volume.clamp(MIN_VOLUME, MAX_VOLUME);
        for gain in &mut self.eq {
            if !gain.is_finite() {
                *gain = 0.0;
            }
            *gain = gain.clamp(-6.0, 6.0);
        }
    }
}

/// `state.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    #[serde(default)]
    pub tracks: Vec<PathBuf>,
    #[serde(default)]
    pub last_track: Option<PathBuf>,
    #[serde(default)]
    pub last_position_ms: u64,
}

impl PlayerState {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("state.json");
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Self>(&contents) {
                Ok(state) => state,
                Err(error) => {
                    log::warn!("ignoring invalid {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("could not read {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        write_json_atomic(&dir.join("state.json"), self)
    }
}

/// Creates the config directory and both files with defaults when this is the
/// first launch. Existing files are left untouched.
pub fn ensure_first_run(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let settings = dir.join("settings.json");
    if !settings.exists() {
        Settings::default().save(dir)?;
    }

    let state = dir.join("state.json");
    if !state.exists() {
        PlayerState::default().save(dir)?;
    }

    Ok(())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        let json = serde_json::to_vec_pretty(value)?;
        file.write_all(&json)
            .with_context(|| format!("writing {}", tmp.display()))?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }

    fs::rename(&tmp, path)
        .with_context(|| format!("replacing {} with {}", path.display(), tmp.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("skald-settings-{name}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn defaults_are_written_on_first_run() {
        let dir = temp_dir("first-run");
        ensure_first_run(&dir).unwrap();
        assert!(dir.join("settings.json").is_file());
        assert!(dir.join("state.json").is_file());

        let settings: Settings =
            serde_json::from_str(&fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
        assert_eq!(settings, Settings::default());
        let state: PlayerState =
            serde_json::from_str(&fs::read_to_string(dir.join("state.json")).unwrap()).unwrap();
        assert_eq!(state, PlayerState::default());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_round_trip_and_clamp() {
        let dir = temp_dir("round-trip");
        let settings = Settings {
            window: Some(WindowState::new(10.0, 20.0, 1320.0, 768.0)),
            volume: 3.0,
            eq: [-9.0, -1.0, 0.0, 2.5, 8.0],
        };
        settings.save(&dir).unwrap();

        let loaded = Settings::load(&dir);
        assert_eq!(loaded.volume, 1.0);
        assert_eq!(loaded.eq, [-6.0, -1.0, 0.0, 2.5, 6.0]);
        assert_eq!(
            loaded.window,
            Some(WindowState::new(10.0, 20.0, 1320.0, 768.0))
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_accept_legacy_eq_key() {
        let dir = temp_dir("legacy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("settings.json"),
            br#"{"volume": 0.5, "eq_gains": [1.0, 2.0, 3.0, 4.0, 5.0]}"#,
        )
        .unwrap();

        let settings = Settings::load(&dir);
        assert_eq!(settings.volume, 0.5);
        assert_eq!(settings.eq, [1.0, 2.0, 3.0, 4.0, 5.0]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_json_falls_back_to_defaults() {
        let dir = temp_dir("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), b"{not json").unwrap();
        assert_eq!(Settings::load(&dir), Settings::default());

        fs::write(dir.join("state.json"), b"[1, 2, 3]").unwrap();
        assert_eq!(PlayerState::load(&dir), PlayerState::default());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_round_trip() {
        let dir = temp_dir("state");
        let state = PlayerState {
            tracks: vec![
                PathBuf::from("/music/a.flac"),
                PathBuf::from("/music/b.mp3"),
            ],
            last_track: Some(PathBuf::from("/music/a.flac")),
            last_position_ms: 48_250,
        };
        state.save(&dir).unwrap();
        assert_eq!(PlayerState::load(&dir), state);

        let json = fs::read_to_string(dir.join("state.json")).unwrap();
        assert!(json.contains("\"last_position_ms\": 48250"));
        assert!(json.contains("\"tracks\""));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let dir = temp_dir("atomic");
        PlayerState::default().save(&dir).unwrap();
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["state.json".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
