//! Symphonia wrapper that turns an audio file into interleaved f32 packets.
//!
//! Opening and decoding happen on the player thread; nothing here touches the
//! audio callback. Seeking is sample-accurate: symphonia seeks to the packet
//! before the requested timestamp, and the wrapper drops the leftover frames.

use anyhow::{Context as _, Result, anyhow};
use std::fs::File;
use std::path::Path;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

/// One decoded packet, still at the file's native rate and channel count.
pub struct DecodedPacket {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

pub struct AudioFile {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    source_rate: u32,
    skip_frames: usize,
}

impl AudioFile {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
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

        let (track_id, time_base, source_rate, decoder) = {
            let track = format
                .default_track(TrackType::Audio)
                .context("the file has no audio track")?;
            let params = track
                .codec_params
                .as_ref()
                .and_then(|params| params.audio())
                .context("the audio track has no codec parameters")?;
            let decoder = symphonia::default::get_codecs()
                .make_audio_decoder(params, &AudioDecoderOptions::default())
                .with_context(|| format!("creating a decoder for {}", path.display()))?;
            (
                track.id,
                track.time_base,
                params.sample_rate.unwrap_or(44_100),
                decoder,
            )
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            source_rate,
            skip_frames: 0,
        })
    }

    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    /// Seeks to `seconds` and drops the frames before the exact position.
    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        let seconds = seconds.max(0.0);
        let Some(time) = Time::try_from_secs_f64(seconds) else {
            return Ok(());
        };

        let seeked = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;

        let actual = self
            .time_base
            .and_then(|time_base| time_base.calc_time(seeked.actual_ts))
            .map(|time| time.as_secs_f64())
            .unwrap_or(seconds);
        let skip = ((seconds - actual).max(0.0) * self.source_rate as f64).round() as usize;
        self.skip_frames = skip;
        self.decoder.reset();
        Ok(())
    }

    /// Decodes the next packet. `Ok(None)` means end of stream.
    pub fn next_packet(&mut self) -> Result<Option<DecodedPacket>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(SymphoniaError::ResetRequired) => {
                    return Err(anyhow!("the stream changed while decoding"));
                }
                Err(error) => return Err(error.into()),
            };

            if packet.track_id != self.track_id {
                continue;
            }

            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
                Err(error) => return Err(error.into()),
            };

            let spec = buffer.spec();
            let sample_rate = spec.rate();
            let channels = spec.channels().count().max(1);
            let total = buffer.samples_interleaved();
            let mut samples = vec![0.0f32; total];
            buffer.copy_to_slice_interleaved(&mut samples);

            if self.skip_frames > 0 {
                let frames = samples.len() / channels;
                if self.skip_frames >= frames {
                    self.skip_frames -= frames;
                    continue;
                }
                let drop = self.skip_frames * channels;
                samples.drain(..drop);
                self.skip_frames = 0;
            }

            return Ok(Some(DecodedPacket {
                samples,
                sample_rate,
                channels,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("skald-decoder-{name}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 0.5 s stereo 16-bit PCM WAV whose left channel is a 440 Hz sine.
    fn write_tone(path: &Path) {
        let sample_rate = 44_100u32;
        let frames = sample_rate / 2;
        let data_len = frames * 4;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for index in 0..frames {
            let value =
                (2.0 * std::f32::consts::PI * 440.0 * index as f32 / sample_rate as f32).sin();
            let sample = (value * 0.5 * i16::MAX as f32) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn decodes_a_wav_to_stereo_f32() {
        let dir = temp_dir("wav");
        let path = dir.join("tone.wav");
        write_tone(&path);

        let mut file = AudioFile::open(&path).unwrap();
        assert_eq!(file.source_rate(), 44_100);

        let packet = file.next_packet().unwrap().expect("packet");
        assert_eq!(packet.sample_rate, 44_100);
        assert_eq!(packet.channels, 2);
        assert!(!packet.samples.is_empty());
        assert!(packet.samples.iter().any(|sample| sample.abs() > 0.1));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reaches_end_of_stream() {
        let dir = temp_dir("eof");
        let path = dir.join("tone.wav");
        write_tone(&path);
        let mut file = AudioFile::open(&path).unwrap();
        let mut packets = 0;
        while file.next_packet().unwrap().is_some() {
            packets += 1;
        }
        assert!(packets > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seek_lands_on_the_requested_packet() {
        let dir = temp_dir("seek");
        let path = dir.join("tone.wav");
        write_tone(&path);
        let mut file = AudioFile::open(&path).unwrap();
        file.seek(0.2).unwrap();
        let packet = file.next_packet().unwrap().expect("packet after seek");
        assert!(!packet.samples.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
