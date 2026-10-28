//! Playlist model, directory scanning and tag/cover extraction.
//!
//! Tags, duration and stream information are read with symphonia when a file
//! is added. Cover art is decoded lazily when a track becomes current, so a
//! large library does not pay for thousands of 1400x1400 JPEGs at once.

use anyhow::{Context as _, Result};
use image::{RgbaImage, imageops};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTag};
use walkdir::WalkDir;

/// Extensions Skald advertises. The list is also used when scanning folders.
pub const SUPPORTED_EXTENSIONS: [&str; 3] = ["flac", "wav", "mp3"];

/// One row in the playlist.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub artist: String,
    pub title: String,
    pub album: String,
    /// Duration in seconds. Zero when the container did not provide one.
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
    pub bitrate_kbps: u32,
}

impl Track {
    pub fn display_artist(&self) -> &str {
        if self.artist.trim().is_empty() {
            "Unknown Artist"
        } else {
            &self.artist
        }
    }

    pub fn display_album(&self) -> &str {
        if self.album.trim().is_empty() {
            "Unknown Album"
        } else {
            &self.album
        }
    }

    pub fn duration_label(&self) -> String {
        format_clock(self.duration)
    }

    /// "FLAC 2117kbps 44100Hz Stereo", as printed under the now-playing title.
    pub fn format_label(&self) -> String {
        let mut out = if self.codec.is_empty() {
            "Audio".to_string()
        } else {
            self.codec.clone()
        };
        if self.bitrate_kbps > 0 {
            out.push_str(&format!(" {}kbps", self.bitrate_kbps));
        }
        if self.sample_rate > 0 {
            out.push_str(&format!(" {}Hz", self.sample_rate));
        }
        out.push(' ');
        out.push_str(channel_label(self.channels));
        out
    }
}

pub fn format_clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0:00".to_string();
    }
    let total = seconds.round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

pub fn channel_label(channels: u16) -> &'static str {
    match channels {
        0 | 1 => "Mono",
        2 => "Stereo",
        6 => "5.1",
        8 => "7.1",
        _ => "Multi-channel",
    }
}

pub fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| supported.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

/// Expands files and directories into a sorted, de-duplicated list of audio
/// paths. Directories are walked recursively.
pub fn collect_audio_paths(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for input in inputs {
        if input.is_dir() {
            for entry in WalkDir::new(input)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
            {
                let path = entry.path();
                if entry.file_type().is_file() && supported_extension(path) {
                    push_unique(&mut paths, &mut seen, path.to_path_buf());
                }
            }
        } else if input.is_file() && supported_extension(input) {
            push_unique(&mut paths, &mut seen, input.clone());
        }
    }

    paths.sort_by(|left, right| {
        left.to_string_lossy()
            .to_lowercase()
            .cmp(&right.to_string_lossy().to_lowercase())
    });
    paths
}

fn push_unique(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if seen.insert(key) {
        paths.push(path);
    }
}

/// Reads every tag and stream property needed by a playlist row.
pub fn read_track(path: &Path) -> Result<Track> {
    let file_size = fs::metadata(path)
        .with_context(|| format!("reading metadata of {}", path.display()))?
        .len();

    let (mut format, codec_params) = open_format(path)?;
    let track = format
        .default_track(TrackType::Audio)
        .context("the file has no audio track")?;
    let track_id = track.id;
    let sample_rate = codec_params
        .as_ref()
        .and_then(|params| match params {
            CodecParameters::Audio(audio) => audio.sample_rate,
            _ => None,
        })
        .unwrap_or(0);
    let channels = codec_params
        .as_ref()
        .and_then(|params| match params {
            CodecParameters::Audio(audio) => audio.channels.as_ref().map(|c| c.count() as u16),
            _ => None,
        })
        .unwrap_or(2);

    let duration = track_duration(track);
    let duration = if duration > 0.0 {
        duration
    } else if sample_rate > 0 {
        track
            .num_frames
            .map(|frames| frames as f64 / sample_rate as f64)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    {
        let mut metadata = format.metadata();
        if let Some(revision) = metadata.skip_to_latest() {
            for tag in &revision.media.tags {
                let Some(standard) = &tag.std else { continue };
                match standard {
                    StandardTag::TrackTitle(value) if title.is_empty() => {
                        title = value.to_string();
                    }
                    StandardTag::Artist(value) if artist.is_empty() => {
                        artist = value.to_string();
                    }
                    StandardTag::Album(value) if album.is_empty() => {
                        album = value.to_string();
                    }
                    StandardTag::Performer(value) if artist.is_empty() => {
                        artist = value.to_string();
                    }
                    _ => {}
                }
            }
        }
    }

    if title.trim().is_empty() {
        title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
    }

    let codec = codec_name(path);
    let bitrate_kbps = if duration > 0.5 {
        ((file_size as f64 * 8.0 / duration) / 1000.0).round() as u32
    } else {
        0
    };

    let _ = track_id;

    Ok(Track {
        path: path.to_path_buf(),
        artist,
        title,
        album,
        duration,
        sample_rate,
        channels,
        codec,
        bitrate_kbps,
    })
}

/// Scans every input and reads its tags. Files that fail to parse are skipped
/// with a warning; one broken file must not abort a folder import.
pub fn scan(inputs: &[PathBuf]) -> Vec<Track> {
    let mut tracks = Vec::new();
    for path in collect_audio_paths(inputs) {
        match read_track(&path) {
            Ok(track) => tracks.push(track),
            Err(error) => log::warn!("skipping {}: {error:#}", path.display()),
        }
    }
    tracks
}

fn open_format(path: &Path) -> Result<(Box<dyn FormatReader>, Option<CodecParameters>)> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }

    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .with_context(|| format!("probing {}", path.display()))?;

    let params = format
        .default_track(TrackType::Audio)
        .and_then(|track| track.codec_params.clone());
    Ok((format, params))
}

fn track_duration(track: &symphonia::core::formats::Track) -> f64 {
    let Some(duration) = track.duration else {
        return 0.0;
    };
    let Some(time_base) = track.time_base else {
        return 0.0;
    };
    time_base
        .calc_duration(duration)
        .map(|time| time.as_secs_f64())
        .unwrap_or(0.0)
}

fn codec_name(path: &Path) -> String {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("flac") => "FLAC".to_string(),
        Some("wav") => "WAV".to_string(),
        Some("mp3") => "MP3".to_string(),
        Some(other) => other.to_ascii_uppercase(),
        None => "Audio".to_string(),
    }
}

/// A decoded cover image plus the brightness used to pick a theme.
pub struct Cover {
    pub image: RgbaImage,
    pub luminance: f32,
}

/// Loads the cover for a track: embedded picture first, then the conventional
/// `cover`/`folder` files beside the audio file. JPEG and PNG both work.
pub fn load_cover(track: &Path) -> Option<Cover> {
    if let Some(bytes) = embedded_cover(track)
        && let Some(image) = decode_image(&bytes)
    {
        let luminance = average_luminance(&image);
        return Some(Cover { image, luminance });
    }

    let directory = track.parent()?;
    for name in [
        "cover.jpg",
        "cover.jpeg",
        "cover.png",
        "folder.jpg",
        "folder.jpeg",
        "folder.png",
    ] {
        let candidate = directory.join(name);
        if candidate.is_file()
            && let Some(image) = decode_image_file(&candidate)
        {
            let luminance = average_luminance(&image);
            return Some(Cover { image, luminance });
        }
    }

    None
}

fn embedded_cover(path: &Path) -> Option<Vec<u8>> {
    let (mut format, _) = open_format(path).ok()?;
    let mut metadata = format.metadata();
    let revision = metadata.skip_to_latest()?;
    revision
        .media
        .visuals
        .iter()
        .find(|visual| !visual.data.is_empty())
        .map(|visual| visual.data.to_vec())
}

fn decode_image(bytes: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(bytes)
        .ok()
        .map(|image| image.into_rgba8())
}

fn decode_image_file(path: &Path) -> Option<RgbaImage> {
    image::open(path).ok().map(|image| image.into_rgba8())
}

/// Perceived brightness in 0..=1. Fully transparent pixels are ignored and the
/// average is done in linearized sRGB so a cream cover reads brighter than a
/// saturated one at the same byte value.
pub fn average_luminance(image: &RgbaImage) -> f32 {
    let mut sum = 0.0f64;
    let mut count = 0u64;
    let step = ((image.width() as usize * image.height() as usize) / 65_536).max(1);
    for (index, pixel) in image.pixels().enumerate() {
        if index % step != 0 || pixel[3] < 8 {
            continue;
        }
        let r = srgb_to_linear(pixel[0]);
        let g = srgb_to_linear(pixel[1]);
        let b = srgb_to_linear(pixel[2]);
        sum += 0.2126 * r + 0.7152 * g + 0.0722 * b;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        (sum / count as f64) as f32
    }
}

fn srgb_to_linear(value: u8) -> f64 {
    let value = value as f64 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Produces the small, deliberately soft background bitmap used behind the
/// window. It is scaled to the window by the UI, which turns the bilinear
/// upscale into the rest of the gaussian look.
pub fn background_bitmap(image: &RgbaImage) -> RgbaImage {
    let width = 330u32;
    let height = 192u32;
    let resized = imageops::resize(image, width, height, imageops::FilterType::Triangle);
    imageops::blur(&resized, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("skald-library-{name}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A tiny mono 8-bit PCM WAV: 44.1 kHz, 0.25 seconds of silence.
    fn write_wav(path: &Path, seconds: u32) {
        let sample_rate = 44_100u32;
        let frames = sample_rate * seconds;
        let data_len = frames * 2;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(44 + data_len as usize, 0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn clock_formatting() {
        assert_eq!(format_clock(0.0), "0:00");
        assert_eq!(format_clock(59.4), "0:59");
        assert_eq!(format_clock(137.0), "2:17");
        assert_eq!(format_clock(3_661.0), "1:01:01");
    }

    #[test]
    fn channel_labels() {
        assert_eq!(channel_label(1), "Mono");
        assert_eq!(channel_label(2), "Stereo");
        assert_eq!(channel_label(6), "5.1");
    }

    #[test]
    fn extensions_are_case_insensitive() {
        assert!(supported_extension(Path::new("a.flac")));
        assert!(supported_extension(Path::new("a.MP3")));
        assert!(!supported_extension(Path::new("a.ogg")));
        assert!(!supported_extension(Path::new("a")));
    }

    #[test]
    fn scan_is_recursive_and_ignores_other_files() {
        let dir = temp_dir("scan");
        let album = dir.join("Album");
        fs::create_dir_all(&album).unwrap();
        write_wav(&album.join("one.wav"), 1);
        write_wav(&dir.join("two.WAV"), 1);
        fs::write(dir.join("notes.txt"), b"not audio").unwrap();
        fs::write(album.join("broken.mp3"), b"nope").unwrap();

        let tracks = scan(std::slice::from_ref(&dir));
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|track| track.codec == "WAV"));
        assert!(tracks.iter().all(|track| track.duration > 0.9));
        assert!(tracks.iter().any(|track| track.title == "one"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_wav_stream_information() {
        let dir = temp_dir("wav");
        let path = dir.join("tone.wav");
        write_wav(&path, 1);

        let track = read_track(&path).unwrap();
        assert_eq!(track.sample_rate, 44_100);
        assert_eq!(track.channels, 1);
        assert!((track.duration - 1.0).abs() < 0.01);
        assert!(track.format_label().starts_with("WAV"));
        assert!(track.format_label().contains("44100Hz"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn brightness_orders_black_and_white() {
        let black = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        let white = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        assert!(average_luminance(&black) < 0.01);
        assert!(average_luminance(&white) > 0.99);
    }

    #[test]
    fn cover_falls_back_to_a_folder_image() {
        let dir = temp_dir("cover");
        let path = dir.join("song.wav");
        write_wav(&path, 1);
        let image: RgbaImage = ImageBuffer::from_pixel(16, 16, Rgba([240, 230, 210, 255]));
        image.save(dir.join("cover.png")).unwrap();

        let cover = load_cover(&path).expect("folder cover");
        assert_eq!(cover.image.dimensions(), (16, 16));
        assert!(cover.luminance > 0.7);

        let background = background_bitmap(&cover.image);
        assert_eq!(background.dimensions(), (330, 192));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_cover_is_not_an_error() {
        let dir = temp_dir("no-cover");
        let path = dir.join("song.wav");
        write_wav(&path, 1);
        assert!(load_cover(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
