//! The root view: playlist state, playback, covers, persistence and the
//! overlays.
//!
//! `SkaldApp` is the only place that owns mutable state. The modules in
//! [`crate::ui`] are views over it and never touch the player directly.

use crate::actions::{
    About, AddFiles, AddFolder, CloseOverlay, PlayPause, PlaySelected, RemoveSelected, SelectNext,
    SelectPrevious, ToggleEqualizer,
};
use crate::library::{self, Track};
use crate::player::{Event, Player};
use crate::settings::{self, PlayerState, Settings, WindowState};
use crate::ui::eq_overlay::gain_at_window_y;
use crate::ui::now_playing::{VOLUME_WIDTH, main_x, volume_x};
use crate::ui::shell::{self, SPECTRUM_COLUMNS};
use crate::ui::theme::{
    FONT_FAMILY, Palette, ROW_HEIGHT, VISIBLE_ROWS, palette_for, scroll_to_show,
};
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, FocusHandle, Pixels, Render, RenderImage, ScrollDelta, Window, div,
};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Which modal card is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Equalizer,
    About,
}

/// Which horizontal bar is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    Volume,
    Progress,
}

pub struct SkaldApp {
    pub(crate) tracks: Vec<Track>,
    pub(crate) selected: HashSet<usize>,
    pub(crate) current: Option<usize>,
    pub(crate) scroll: usize,
    pub(crate) position: f64,
    pub(crate) settings: Settings,
    pub(crate) player: Option<Player>,
    pub(crate) paused: bool,
    pub(crate) loaded: bool,
    pub(crate) audio_error: Option<String>,
    pub(crate) cover: Option<Arc<RenderImage>>,
    pub(crate) background: Option<Arc<RenderImage>>,
    pub(crate) spectrum_image: Option<Arc<RenderImage>>,
    pub(crate) luminance: Option<f32>,
    pub(crate) overlay: Option<Overlay>,
    pub(crate) drag: Option<Drag>,
    pub(crate) eq_drag: Option<usize>,
    pub(crate) loading: usize,
    pub(crate) settings_dirty: bool,
    pub(crate) state_dirty: bool,
    pub(crate) focus: FocusHandle,
    config: PathBuf,
    spectrum_history: VecDeque<Vec<f32>>,
    spectrum_pending: VecDeque<Vec<f32>>,
    spectrum_accum: f64,
    last_tick: Instant,
    scroll_fraction: f32,
    cover_generation: u64,
    ticks: u64,
    ticking: bool,
    pending_paths: Vec<PathBuf>,
    pending_last: Option<PathBuf>,
    pending_position: f64,
}

impl SkaldApp {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let config = settings::config_dir();
        if let Err(error) = settings::ensure_first_run(&config) {
            log::warn!("could not create {}: {error:#}", config.display());
        }
        let settings = Settings::load(&config);
        let state = PlayerState::load(&config);
        let pending_paths: Vec<PathBuf> =
            state.tracks.into_iter().filter(|p| p.is_file()).collect();

        Self {
            tracks: Vec::new(),
            selected: HashSet::new(),
            current: None,
            scroll: 0,
            position: 0.0,
            settings,
            player: None,
            paused: true,
            loaded: false,
            audio_error: None,
            cover: None,
            background: None,
            spectrum_image: None,
            spectrum_pending: VecDeque::with_capacity(SPECTRUM_COLUMNS),
            spectrum_accum: 0.0,
            last_tick: Instant::now(),
            luminance: None,
            overlay: None,
            drag: None,
            eq_drag: None,
            loading: 0,
            settings_dirty: false,
            state_dirty: false,
            focus: cx.focus_handle(),
            config,
            spectrum_history: VecDeque::with_capacity(SPECTRUM_COLUMNS),
            scroll_fraction: 0.0,
            cover_generation: 0,
            ticks: 0,
            ticking: false,
            pending_paths,
            pending_last: state.last_track,
            pending_position: state.last_position_ms as f64 / 1000.0,
        }
    }

    /// Restores the saved playlist and the last track. Called once the view is
    /// attached to a window, so background tasks can report back.
    pub(crate) fn startup(&mut self, cx: &mut Context<Self>) {
        let paths = std::mem::take(&mut self.pending_paths);
        if paths.is_empty() {
            return;
        }
        let restore = Some((self.pending_last.take(), self.pending_position));
        self.import_paths(paths, restore, cx);
    }

    pub(crate) fn palette(&self) -> Palette {
        palette_for(self.luminance)
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.player
            .as_ref()
            .is_some_and(|player| player.is_playing())
    }

    pub(crate) fn duration(&self) -> f64 {
        self.current
            .and_then(|index| self.tracks.get(index))
            .map(|track| track.duration)
            .unwrap_or(0.0)
    }

    pub(crate) fn max_scroll(&self) -> usize {
        self.tracks.len().saturating_sub(VISIBLE_ROWS)
    }

    // ---------------------------------------------------------------- overlays

    pub(crate) fn toggle_overlay(&mut self, overlay: Overlay, cx: &mut Context<Self>) {
        self.overlay = if self.overlay == Some(overlay) {
            None
        } else {
            Some(overlay)
        };
        cx.notify();
    }

    pub(crate) fn close_overlay(&mut self, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- playlist

    pub(crate) fn click_row(
        &mut self,
        index: usize,
        modifiers: gpui::Modifiers,
        click_count: usize,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tracks.len() {
            return;
        }
        let extend = modifiers.shift || modifiers.platform || modifiers.control;
        if extend {
            if !self.selected.insert(index) {
                self.selected.remove(&index);
            }
        } else {
            self.selected.clear();
            self.selected.insert(index);
        }
        if click_count >= 2 {
            self.play_index(index, 0.0, cx);
        }
        cx.notify();
    }

    pub(crate) fn scroll_by(&mut self, delta: ScrollDelta, cx: &mut Context<Self>) {
        // Positive wheel deltas mean "scroll up" in GPUI, matching the
        // platform's natural direction; the list counts rows downwards.
        let rows = match delta {
            ScrollDelta::Lines(delta) => -delta.y,
            ScrollDelta::Pixels(delta) => -f32::from(delta.y) / ROW_HEIGHT,
        };
        self.scroll_fraction += rows;
        let steps = self.scroll_fraction.trunc() as i32;
        if steps == 0 {
            return;
        }
        self.scroll_fraction -= steps as f32;
        let next = if steps < 0 {
            self.scroll.saturating_sub(steps.unsigned_abs() as usize)
        } else {
            (self.scroll + steps as usize).min(self.max_scroll())
        };
        if next != self.scroll {
            self.scroll = next;
            cx.notify();
        }
    }

    pub(crate) fn select_relative(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.tracks.is_empty() {
            return;
        }
        let anchor = self
            .selected
            .iter()
            .min()
            .copied()
            .or(self.current)
            .unwrap_or(0);
        let target = (anchor as i64 + delta as i64).clamp(0, self.tracks.len() as i64 - 1) as usize;
        self.selected.clear();
        self.selected.insert(target);
        self.scroll = scroll_to_show(self.scroll, self.tracks.len(), target);
        cx.notify();
    }

    pub(crate) fn play_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.selected.iter().min().copied() {
            self.play_index(index, 0.0, cx);
        } else {
            self.toggle_playback(cx);
        }
    }

    pub(crate) fn remove_selected(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() || self.tracks.is_empty() {
            return;
        }
        let removed: HashSet<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|index| *index < self.tracks.len())
            .collect();
        if removed.is_empty() {
            return;
        }

        let old_current = self.current;
        let mut kept = Vec::with_capacity(self.tracks.len() - removed.len());
        let mut new_current = None;
        for (index, track) in std::mem::take(&mut self.tracks).into_iter().enumerate() {
            if removed.contains(&index) {
                continue;
            }
            if old_current == Some(index) {
                new_current = Some(kept.len());
            }
            kept.push(track);
        }
        self.tracks = kept;
        self.selected.clear();

        if old_current.is_some() && new_current.is_none() {
            self.current = None;
            self.position = 0.0;
            self.loaded = false;
            self.paused = true;
            if let Some(player) = &self.player {
                player.stop();
            }
            self.clear_cover(cx);
        } else {
            self.current = new_current;
        }

        self.scroll = self.scroll.min(self.max_scroll());
        self.state_dirty = true;
        cx.notify();
    }

    // ---------------------------------------------------------------- playback

    pub(crate) fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        let index = match self.current {
            Some(index) if index < self.tracks.len() => Some(index),
            _ => self
                .selected
                .iter()
                .min()
                .copied()
                .or(if self.tracks.is_empty() {
                    None
                } else {
                    Some(0)
                }),
        };
        let Some(index) = index else {
            return;
        };

        if self.loaded
            && self.current == Some(index)
            && let Some(player) = &self.player
        {
            if self.paused {
                player.resume();
                self.paused = false;
            } else {
                player.pause();
                self.paused = true;
                self.state_dirty = true;
            }
            cx.notify();
            return;
        }

        let start = if self.current == Some(index) {
            let duration = self.duration();
            if duration > 0.0 && self.position >= duration - 0.25 {
                0.0
            } else {
                self.position
            }
        } else {
            0.0
        };
        self.play_index(index, start, cx);
    }

    fn play_index(&mut self, index: usize, start: f64, cx: &mut Context<Self>) {
        if index >= self.tracks.len() {
            return;
        }
        if self.current != Some(index) {
            self.set_current(index, cx);
        }
        self.position = start.max(0.0);
        let path = self.tracks[index].path.clone();
        if self.ensure_player() {
            if let Some(player) = &self.player {
                player.play(path, self.position);
            }
            self.paused = false;
            self.loaded = true;
            self.audio_error = None;
        }
        self.state_dirty = true;
        cx.notify();
    }

    fn ensure_player(&mut self) -> bool {
        if self.player.is_none() {
            match Player::new(self.settings.volume, self.settings.eq) {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    log::error!("audio unavailable: {error:#}");
                    self.audio_error = Some(format!("Audio unavailable: {error}"));
                    return false;
                }
            }
        }
        true
    }

    fn set_current(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tracks.len() {
            return;
        }
        self.current = Some(index);
        self.position = 0.0;
        self.scroll = scroll_to_show(self.scroll, self.tracks.len(), index);
        self.clear_spectrum(cx);
        self.load_cover(cx);
        self.state_dirty = true;
        cx.notify();
    }

    pub(crate) fn set_volume_at(&mut self, x: Pixels, window: &Window, cx: &mut Context<Self>) {
        let left = volume_x(f32::from(window.viewport_size().width));
        let fraction = ((f32::from(x) - left) / VOLUME_WIDTH).clamp(0.0, 1.0);
        self.settings.volume = fraction;
        if let Some(player) = &self.player {
            player.set_volume(fraction);
        }
        self.settings_dirty = true;
        cx.notify();
    }

    pub(crate) fn seek_at(&mut self, x: Pixels, window: &Window, cx: &mut Context<Self>) {
        let duration = self.duration();
        if duration <= 0.0 {
            return;
        }
        let (left, width) = main_x(f32::from(window.viewport_size().width));
        let fraction = ((f32::from(x) - left) / (width - 2.0)).clamp(0.0, 1.0) as f64;
        self.position = fraction * duration;
        self.clear_spectrum(cx);
        if self.loaded
            && let Some(player) = &self.player
        {
            player.seek(self.position);
        }
        self.state_dirty = true;
        cx.notify();
    }

    pub(crate) fn save_position(&mut self) {
        self.state_dirty = true;
    }

    pub(crate) fn set_eq_gain_at(
        &mut self,
        index: usize,
        y: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.settings.eq.len() {
            return;
        }
        let gain = gain_at_window_y(f32::from(window.viewport_size().height), f32::from(y));
        self.settings.eq[index] = gain;
        if let Some(player) = &self.player {
            player.set_eq(self.settings.eq);
        }
        self.settings_dirty = true;
        cx.notify();
    }

    // ------------------------------------------------------------------ import

    pub(crate) fn add_folder(&mut self, cx: &mut Context<Self>) {
        let future = rfd::AsyncFileDialog::new()
            .set_title("Add Folder")
            .pick_folders();
        cx.spawn(async move |this, cx| {
            let Some(handles) = future.await else {
                return;
            };
            let paths: Vec<PathBuf> = handles
                .into_iter()
                .map(|handle| handle.path().to_path_buf())
                .collect();
            let _ = this.update(cx, |this, cx| {
                let restore = None;
                this.import_paths(paths, restore, cx);
            });
        })
        .detach();
    }

    pub(crate) fn add_files(&mut self, cx: &mut Context<Self>) {
        let future = rfd::AsyncFileDialog::new()
            .set_title("Add Files")
            .add_filter("Audio", &["flac", "wav", "mp3"])
            .pick_files();
        cx.spawn(async move |this, cx| {
            let Some(handles) = future.await else {
                return;
            };
            let paths: Vec<PathBuf> = handles
                .into_iter()
                .map(|handle| handle.path().to_path_buf())
                .collect();
            let _ = this.update(cx, |this, cx| {
                let restore = None;
                this.import_paths(paths, restore, cx);
            });
        })
        .detach();
    }

    fn import_paths(
        &mut self,
        inputs: Vec<PathBuf>,
        restore: Option<(Option<PathBuf>, f64)>,
        cx: &mut Context<Self>,
    ) {
        if inputs.is_empty() {
            return;
        }
        self.loading += 1;
        let restoring = restore.is_some();
        let mut seen: HashSet<PathBuf> = self
            .tracks
            .iter()
            .map(|track| path_key(&track.path))
            .collect();

        cx.spawn(async move |this, cx| {
            let scanned = cx
                .background_executor()
                .spawn(async move { library::scan(&inputs) })
                .await;
            let _ = this.update(cx, |this, cx| {
                for track in scanned {
                    if !seen.insert(path_key(&track.path)) {
                        continue;
                    }
                    this.tracks.push(track);
                }
                this.loading = this.loading.saturating_sub(1);

                if let Some((last, seconds)) = restore
                    && let Some(index) = last
                        .and_then(|path| this.tracks.iter().position(|track| track.path == path))
                {
                    this.set_current(index, cx);
                    this.position = seconds.max(0.0);
                }
                if this.current.is_none() && !this.tracks.is_empty() {
                    this.set_current(0, cx);
                }
                if restoring || !this.tracks.is_empty() {
                    this.state_dirty = true;
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------------- cover

    fn load_cover(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .current
            .and_then(|index| self.tracks.get(index))
            .map(|track| track.path.clone())
        else {
            self.clear_cover(cx);
            return;
        };

        let generation = self.cover_generation.wrapping_add(1);
        self.cover_generation = generation;
        cx.spawn(async move |this, cx| {
            let cover = cx
                .background_executor()
                .spawn(async move { library::load_cover(&path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.cover_generation != generation {
                    return;
                }
                match cover {
                    Some(cover) => {
                        if let Some(old) = this.cover.take() {
                            cx.drop_image(old, None);
                        }
                        if let Some(old) = this.background.take() {
                            cx.drop_image(old, None);
                        }
                        this.luminance = Some(cover.luminance);
                        this.background = Some(shell::render_image(library::background_bitmap(
                            &cover.image,
                        )));
                        this.cover = Some(shell::render_image(cover.image));
                    }
                    None => this.clear_cover(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clear_cover(&mut self, cx: &mut Context<Self>) {
        self.cover_generation = self.cover_generation.wrapping_add(1);
        if let Some(old) = self.cover.take() {
            cx.drop_image(old, None);
        }
        if let Some(old) = self.background.take() {
            cx.drop_image(old, None);
        }
        self.luminance = None;
        cx.notify();
    }

    // -------------------------------------------------------------- persistence

    fn persist_settings(&self) {
        if let Err(error) = self.settings.save(&self.config) {
            log::warn!("could not save settings: {error:#}");
        }
    }

    fn persist_state(&self) {
        let state = PlayerState {
            tracks: self.tracks.iter().map(|track| track.path.clone()).collect(),
            last_track: self
                .current
                .and_then(|index| self.tracks.get(index))
                .map(|track| track.path.clone()),
            last_position_ms: (self.position.max(0.0) * 1000.0) as u64,
        };
        if let Err(error) = state.save(&self.config) {
            log::warn!("could not save state: {error:#}");
        }
    }

    /// Called from the window's close callback: keep the frame the user left.
    pub(crate) fn remember_window(&mut self, bounds: Bounds<Pixels>) {
        self.settings.window = Some(WindowState::new(
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        ));
        self.settings_dirty = true;
    }

    /// Flush both files and stop the audio thread before the window goes away.
    pub(crate) fn shutdown(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
        }
        self.persist_settings();
        self.persist_state();
    }

    /// The custom close button: flush exactly like the native close did, then
    /// remove the window.
    pub(crate) fn close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_window(window.bounds());
        self.shutdown();
        window.remove_window();
        cx.quit();
    }

    /// Drops queued and displayed spectrum data, for example after a seek.
    fn clear_spectrum(&mut self, cx: &mut Context<Self>) {
        self.spectrum_pending.clear();
        self.spectrum_accum = 0.0;
        if !self.spectrum_history.is_empty() {
            self.spectrum_history.clear();
            if let Some(old) = self.spectrum_image.take() {
                cx.drop_image(old, None);
            }
        }
    }

    // -------------------------------------------------------------------- tick

    fn start_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                // Match the display refresh so spectrum columns appear as they
                // are decoded instead of arriving in 50 ms batches.
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    this.tick(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let mut dirty = false;
        let mut events = Vec::new();
        let mut interval = 1.0 / 43.0;
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        if let Some(player) = &self.player {
            if self.drag != Some(Drag::Progress) && self.loaded {
                let position = player.position_seconds();
                if (position - self.position).abs() > 0.0005 {
                    self.position = position;
                    dirty = true;
                }
            }
            interval = player.spectrum_interval_seconds();
            while let Some(event) = player.try_event() {
                events.push(event);
            }
            // The decoder emits a packet's columns in a burst. Queue them and
            // release one column per hop so the waterfall scrolls continuously
            // instead of jumping several pixels at a time.
            while let Some(column) = player.try_spectrum() {
                if self.spectrum_pending.len() >= SPECTRUM_COLUMNS {
                    self.spectrum_pending.pop_front();
                }
                self.spectrum_pending.push_back(column);
            }
        }

        let mut columns = 0usize;
        if self.is_playing() {
            self.spectrum_accum += elapsed.as_secs_f64().min(0.25);
            while self.spectrum_accum >= interval {
                let Some(column) = self.spectrum_pending.pop_front() else {
                    self.spectrum_accum = 0.0;
                    break;
                };
                if self.spectrum_history.len() >= SPECTRUM_COLUMNS {
                    self.spectrum_history.pop_front();
                }
                self.spectrum_history.push_back(column);
                self.spectrum_accum -= interval;
                columns += 1;
            }
        } else {
            self.spectrum_pending.clear();
            self.spectrum_accum = 0.0;
        }

        if columns > 0 {
            if let Some(old) = self.spectrum_image.take() {
                cx.drop_image(old, None);
            }
            self.spectrum_image = Some(shell::spectrum_bitmap(&self.spectrum_history));
            dirty = true;
        }

        for event in events {
            match event {
                Event::Ended => {
                    let next = self
                        .current
                        .map(|index| index + 1)
                        .filter(|index| *index < self.tracks.len());
                    match next {
                        Some(index) => self.play_index(index, 0.0, cx),
                        None => {
                            self.position = self.duration();
                            self.paused = true;
                            self.loaded = false;
                            self.state_dirty = true;
                        }
                    }
                    dirty = true;
                }
                Event::Error(message) => {
                    log::warn!("playback error: {message}");
                    self.audio_error = Some(message);
                    self.paused = true;
                    self.loaded = false;
                    dirty = true;
                }
            }
        }

        self.ticks = self.ticks.wrapping_add(1);
        if self.settings_dirty {
            self.settings_dirty = false;
            self.persist_settings();
        }
        if self.state_dirty && self.ticks.is_multiple_of(120) {
            self.state_dirty = false;
            self.persist_state();
        }
        if dirty {
            cx.notify();
        }
    }
}

impl Drop for SkaldApp {
    fn drop(&mut self) {
        self.persist_settings();
        self.persist_state();
    }
}

impl Render for SkaldApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_ticker(cx);
        let palette = self.palette();

        let sidebar = self.render_sidebar(cx);
        let main = div().relative().size_full().child(self.render_main(cx));

        let main: AnyElement = if self.tracks.is_empty() && self.loading == 0 {
            main.child(self.render_add_overlay(cx)).into_any_element()
        } else {
            main.into_any_element()
        };

        let mut root = div()
            .relative()
            .size_full()
            .font_family(FONT_FAMILY)
            .track_focus(&self.focus)
            .child(shell::backdrop(self.background.clone(), palette))
            .child(shell::stage(sidebar, main))
            .on_action(cx.listener(|this, _: &AddFolder, _, cx| this.add_folder(cx)))
            .on_action(cx.listener(|this, _: &AddFiles, _, cx| this.add_files(cx)))
            .on_action(cx.listener(|this, _: &ToggleEqualizer, _, cx| {
                this.toggle_overlay(Overlay::Equalizer, cx)
            }))
            .on_action(
                cx.listener(|this, _: &About, _, cx| this.toggle_overlay(Overlay::About, cx)),
            )
            .on_action(cx.listener(|this, _: &CloseOverlay, _, cx| this.close_overlay(cx)))
            .on_action(cx.listener(|this, _: &PlayPause, _, cx| this.toggle_playback(cx)))
            .on_action(cx.listener(|this, _: &PlaySelected, _, cx| this.play_selected(cx)))
            .on_action(cx.listener(|this, _: &RemoveSelected, _, cx| this.remove_selected(cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_relative(-1, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_relative(1, cx)))
            .on_action(|_: &crate::actions::Quit, _, cx| cx.quit());

        if let Some(overlay) = self.overlay {
            root = root.child(match overlay {
                Overlay::Equalizer => self.render_eq_overlay(cx),
                Overlay::About => self.render_about_overlay(cx),
            });
        }

        root.child(self.render_titlebar(cx))
    }
}

fn path_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
