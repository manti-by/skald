//! Audio output, the decode thread and the lock-free ring between them.
//!
//! The decode thread owns symphonia, the resampler, the equalizer and the FFT.
//! It writes finished stereo frames into a single-producer/single-consumer ring;
//! the cpal callback only pops frames, applies volume and converts to the
//! device's sample format. No allocation, locking or decoding happens in the
//! callback.

use crate::player::decoder::{AudioFile, DecodedPacket};
use crate::player::dsp::{EQ_FREQS, EqChain, Resampler, downmix_to_stereo};
use crate::player::spectrum::SpectrumAnalyzer;
use anyhow::{Context as _, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::cell::UnsafeCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

const RING_FRAMES: usize = 16_384;
const FREE_THRESHOLD: usize = 8_192;
const SCRATCH_FRAMES: usize = 8_192;

#[derive(Debug)]
pub enum Event {
    Ended,
    Error(String),
}

enum Command {
    Play { path: PathBuf, start: f64 },
    Seek(f64),
    Stop,
}

struct Shared {
    output_rate: u32,
    position_frames: AtomicU64,
    paused: AtomicBool,
    active: AtomicBool,
    volume: AtomicU32,
    eq: [AtomicU32; EQ_FREQS.len()],
    flush_requested: AtomicBool,
    flush_ack: AtomicBool,
    stopping: AtomicBool,
}

impl Shared {
    fn new(output_rate: u32, volume: f32, eq: [f32; EQ_FREQS.len()]) -> Self {
        let eq = std::array::from_fn(|index| AtomicU32::new(eq[index].to_bits()));
        Self {
            output_rate,
            position_frames: AtomicU64::new(0),
            paused: AtomicBool::new(true),
            active: AtomicBool::new(false),
            volume: AtomicU32::new(volume.clamp(0.0, 1.0).to_bits()),
            eq,
            flush_requested: AtomicBool::new(false),
            flush_ack: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        }
    }

    fn eq_gains(&self) -> [f32; EQ_FREQS.len()] {
        std::array::from_fn(|index| f32::from_bits(self.eq[index].load(Ordering::Relaxed)))
    }

    fn set_eq(&self, gains: [f32; EQ_FREQS.len()]) {
        for (index, gain) in gains.iter().enumerate() {
            self.eq[index].store(gain.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Single-producer/single-consumer interleaved stereo ring buffer.
struct AudioRing {
    capacity_frames: usize,
    data: Box<[UnsafeCell<f32>]>,
    read: AtomicUsize,
    write: AtomicUsize,
}

// The read index is only written by the audio callback and the write index only
// by the decode thread. `clear` is called by the callback while the producer is
// paused by the flush handshake.
unsafe impl Send for AudioRing {}
unsafe impl Sync for AudioRing {}

impl AudioRing {
    fn new(capacity_frames: usize) -> Self {
        assert!(capacity_frames.is_power_of_two());
        let data = (0..capacity_frames * 2)
            .map(|_| UnsafeCell::new(0.0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            capacity_frames,
            data,
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    fn available_frames(&self) -> usize {
        self.write
            .load(Ordering::Acquire)
            .wrapping_sub(self.read.load(Ordering::Acquire))
    }

    fn free_frames(&self) -> usize {
        self.capacity_frames.saturating_sub(self.available_frames())
    }

    /// Producer side. Returns the number of frames written.
    fn push_stereo(&self, samples: &[f32]) -> usize {
        let frames = samples.len() / 2;
        let write = self.write.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let free = self
            .capacity_frames
            .saturating_sub(write.wrapping_sub(read));
        let count = frames.min(free);
        let mask = self.capacity_frames - 1;
        for frame in 0..count {
            let slot = (write.wrapping_add(frame) & mask) * 2;
            // SAFETY: this frame lies between the write and read indices, so no
            // consumer can observe it until the release store below.
            unsafe {
                *self.data[slot].get() = samples[frame * 2];
                *self.data[slot + 1].get() = samples[frame * 2 + 1];
            }
        }
        self.write
            .store(write.wrapping_add(count), Ordering::Release);
        count
    }

    /// Consumer side. Returns the number of frames read.
    fn pop_stereo(&self, output: &mut [f32]) -> usize {
        let frames = output.len() / 2;
        let read = self.read.load(Ordering::Relaxed);
        let write = self.write.load(Ordering::Acquire);
        let available = write.wrapping_sub(read);
        let count = frames.min(available);
        let mask = self.capacity_frames - 1;
        for frame in 0..count {
            let slot = (read.wrapping_add(frame) & mask) * 2;
            // SAFETY: this frame was published by the producer's release store
            // and is strictly before the write index.
            unsafe {
                output[frame * 2] = *self.data[slot].get();
                output[frame * 2 + 1] = *self.data[slot + 1].get();
            }
        }
        self.read.store(read.wrapping_add(count), Ordering::Release);
        count
    }

    /// Consumer side only.
    fn clear(&self) {
        let write = self.write.load(Ordering::Acquire);
        self.read.store(write, Ordering::Release);
    }
}

pub struct Player {
    commands: Sender<Command>,
    events: Receiver<Event>,
    spectrum: Receiver<Vec<f32>>,
    shared: Arc<Shared>,
    stream: Option<cpal::Stream>,
    thread: Option<JoinHandle<()>>,
    output_rate: u32,
}

impl Player {
    pub fn new(volume: f32, eq: [f32; EQ_FREQS.len()]) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no audio output device is available")?;
        let supported = device
            .default_output_config()
            .context("the output device has no default configuration")?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let output_rate = config.sample_rate;

        let shared = Arc::new(Shared::new(output_rate, volume, eq));
        let ring = Arc::new(AudioRing::new(RING_FRAMES));

        let stream = match sample_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::F64 => {
                build_stream::<f64>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::U32 => {
                build_stream::<u32>(&device, &config, shared.clone(), ring.clone())
            }
            SampleFormat::I8 => build_stream::<i8>(&device, &config, shared.clone(), ring.clone()),
            SampleFormat::U8 => build_stream::<u8>(&device, &config, shared.clone(), ring.clone()),
            other => Err(anyhow!("unsupported output sample format {other}")),
        }?;
        stream.play().context("starting audio output")?;

        let (command_tx, command_rx) = channel();
        let (event_tx, event_rx) = channel();
        let (spectrum_tx, spectrum_rx) = sync_channel(96);
        let thread_shared = shared.clone();
        let thread_ring = ring.clone();
        let thread = std::thread::Builder::new()
            .name("skald-decode".to_string())
            .spawn(move || {
                decode_loop(
                    thread_shared,
                    thread_ring,
                    command_rx,
                    event_tx,
                    spectrum_tx,
                )
            })
            .context("starting the decode thread")?;

        Ok(Self {
            commands: command_tx,
            events: event_rx,
            spectrum: spectrum_rx,
            shared,
            stream: Some(stream),
            thread: Some(thread),
            output_rate,
        })
    }

    pub fn play(&self, path: PathBuf, start_seconds: f64) {
        let start_seconds = start_seconds.max(0.0);
        self.shared.position_frames.store(
            (start_seconds * self.output_rate as f64) as u64,
            Ordering::Relaxed,
        );
        self.shared.active.store(true, Ordering::Relaxed);
        self.shared.paused.store(false, Ordering::Release);
        let _ = self.commands.send(Command::Play {
            path,
            start: start_seconds,
        });
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Release);
    }

    pub fn seek(&self, seconds: f64) {
        let seconds = seconds.max(0.0);
        self.shared.position_frames.store(
            (seconds * self.output_rate as f64) as u64,
            Ordering::Relaxed,
        );
        let _ = self.commands.send(Command::Seek(seconds));
    }

    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
        self.shared.active.store(false, Ordering::Relaxed);
        self.shared.paused.store(true, Ordering::Release);
    }

    pub fn set_volume(&self, volume: f32) {
        self.shared
            .volume
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn set_eq(&self, gains: [f32; EQ_FREQS.len()]) {
        self.shared.set_eq(gains);
    }

    pub fn position_seconds(&self) -> f64 {
        let frames = self.shared.position_frames.load(Ordering::Relaxed);
        frames as f64 / self.output_rate.max(1) as f64
    }

    pub fn is_playing(&self) -> bool {
        self.shared.active.load(Ordering::Relaxed) && !self.shared.paused.load(Ordering::Relaxed)
    }

    pub fn try_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    pub fn try_spectrum(&self) -> Option<Vec<f32>> {
        self.spectrum.try_recv().ok()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
        self.stream.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Shared>,
    ring: Arc<AudioRing>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32> + 'static,
{
    let channels = config.channels as usize;
    let mut scratch = vec![0.0f32; SCRATCH_FRAMES * 2];
    let mut gain = f32::from_bits(shared.volume.load(Ordering::Relaxed)).clamp(0.0, 1.0);
    let error_shared = shared.clone();

    let stream = device
        .build_output_stream(
            *config,
            move |output: &mut [T], _| {
                if shared.flush_requested.swap(false, Ordering::AcqRel) {
                    ring.clear();
                    shared.flush_ack.store(true, Ordering::Release);
                }

                for sample in output.iter_mut() {
                    *sample = T::from_sample(0.0);
                }

                if shared.paused.load(Ordering::Relaxed) {
                    return;
                }

                let frames = output.len() / channels;
                let target_gain = f32::from_bits(shared.volume.load(Ordering::Relaxed));
                let target_gain = target_gain.clamp(0.0, 1.0);
                let mut consumed = 0usize;
                let mut written = 0usize;

                while written < frames {
                    let chunk = (frames - written).min(SCRATCH_FRAMES);
                    let got = ring.pop_stereo(&mut scratch[..chunk * 2]);
                    for frame in 0..chunk {
                        let index = written + frame;
                        let base = index * channels;
                        if frame < got {
                            // Short ramp avoids zipper noise when the volume
                            // slider is dragged.
                            gain += (target_gain - gain) * 0.002;
                            let left = (scratch[frame * 2] * gain).clamp(-1.0, 1.0);
                            let right = (scratch[frame * 2 + 1] * gain).clamp(-1.0, 1.0);
                            if channels == 1 {
                                output[base] = T::from_sample((left + right) * 0.5);
                            } else {
                                output[base] = T::from_sample(left);
                                output[base + 1] = T::from_sample(right);
                            }
                        } else {
                            // Underrun: leave silence. The next callback will
                            // pick up the remaining frames.
                            written = frames;
                            break;
                        }
                    }
                    consumed += got;
                    written += chunk;
                    if got < chunk {
                        break;
                    }
                }

                if consumed > 0 {
                    shared
                        .position_frames
                        .fetch_add(consumed as u64, Ordering::Relaxed);
                }
            },
            move |error| {
                log::error!("audio output error: {error}");
                if error_shared.paused.load(Ordering::Relaxed) {
                    log::debug!("audio output stopped");
                }
            },
            None,
        )
        .context("creating the output stream")?;
    Ok(stream)
}

fn flush_ring(shared: &Shared, ring: &AudioRing) {
    shared.flush_requested.store(true, Ordering::Release);
    for _ in 0..250 {
        if shared.flush_ack.swap(false, Ordering::AcqRel) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    shared.flush_requested.store(false, Ordering::Release);
    ring.clear();
    // Let the callback finish the block it had already started before the
    // decoder starts publishing the new position.
    std::thread::sleep(Duration::from_millis(12));
}

struct DecodeState {
    ring: Arc<AudioRing>,
    shared: Arc<Shared>,
    output_rate: u32,
    file: Option<AudioFile>,
    resampler: Resampler,
    stereo: Vec<f32>,
    eq: EqChain,
    spectrum: SpectrumAnalyzer,
    eq_gains: [f32; EQ_FREQS.len()],
}

impl DecodeState {
    fn new(shared: Arc<Shared>, ring: Arc<AudioRing>) -> Self {
        let output_rate = shared.output_rate;
        Self {
            ring,
            shared,
            output_rate,
            file: None,
            resampler: Resampler::new(output_rate, output_rate),
            stereo: Vec::new(),
            eq: EqChain::new(output_rate),
            spectrum: SpectrumAnalyzer::new(output_rate),
            eq_gains: [0.0; EQ_FREQS.len()],
        }
    }

    fn start(&mut self, path: PathBuf, start_seconds: f64, events: &Sender<Event>) {
        flush_ring(&self.shared, &self.ring);
        self.file = None;
        self.spectrum.reset();
        self.eq.reset();

        match AudioFile::open(&path) {
            Ok(mut file) => {
                if start_seconds > 0.001
                    && let Err(error) = file.seek(start_seconds)
                {
                    log::warn!("seek to {start_seconds:.2}s failed: {error:#}");
                }
                self.resampler = Resampler::new(file.source_rate(), self.output_rate);
                self.resampler.reset();
                self.shared.position_frames.store(
                    (start_seconds * self.output_rate as f64) as u64,
                    Ordering::Relaxed,
                );
                self.shared.active.store(true, Ordering::Relaxed);
                self.shared.paused.store(false, Ordering::Release);
                self.file = Some(file);
            }
            Err(error) => {
                self.shared.active.store(false, Ordering::Relaxed);
                self.shared.paused.store(true, Ordering::Release);
                let _ = events.send(Event::Error(format!("{error:#}")));
            }
        }
    }

    fn seek(&mut self, seconds: f64, events: &Sender<Event>) {
        flush_ring(&self.shared, &self.ring);
        self.spectrum.reset();
        self.eq.reset();
        if let Some(file) = &mut self.file
            && let Err(error) = file.seek(seconds)
        {
            log::warn!("seek to {seconds:.2}s failed: {error:#}");
            let _ = events.send(Event::Error(format!("{error:#}")));
            return;
        }
        self.shared.position_frames.store(
            (seconds * self.output_rate as f64) as u64,
            Ordering::Relaxed,
        );
    }

    fn process_packet(&mut self, packet: DecodedPacket, spectrum: &SyncSender<Vec<f32>>) {
        downmix_to_stereo(&packet.samples, packet.channels, &mut self.stereo);

        if packet.sample_rate != self.resampler_source_rate() {
            self.resampler = Resampler::new(packet.sample_rate, self.output_rate);
        }

        let gains = self.shared.eq_gains();
        if gains != self.eq_gains {
            self.eq.set_gains(gains);
            self.eq_gains = gains;
        }

        let samples = self.resampler.process(&self.stereo);
        self.eq.process_stereo(samples);

        for column in self.spectrum.push(samples) {
            match spectrum.try_send(column) {
                Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
            }
        }

        let mut remaining: &mut [f32] = samples;
        while !remaining.is_empty() {
            let written = self.ring.push_stereo(remaining);
            if written == 0 {
                if self.shared.stopping.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            } else {
                remaining = &mut remaining[written * 2..];
            }
        }
    }

    fn resampler_source_rate(&self) -> u32 {
        // The resampler is rebuilt whenever a packet's rate differs; this
        // helper keeps the comparison readable without exposing internals.
        self.resampler.source_rate()
    }
}

fn decode_loop(
    shared: Arc<Shared>,
    ring: Arc<AudioRing>,
    commands: Receiver<Command>,
    events: Sender<Event>,
    spectrum: SyncSender<Vec<f32>>,
) {
    let mut state = DecodeState::new(shared, ring);
    let mut at_end = false;

    loop {
        if state.shared.stopping.load(Ordering::Relaxed) {
            return;
        }
        let mut restarted = false;
        loop {
            match commands.try_recv() {
                Ok(Command::Play { path, start }) => {
                    state.start(path, start, &events);
                    restarted = true;
                }
                Ok(Command::Seek(seconds)) => {
                    state.seek(seconds, &events);
                    restarted = true;
                }
                Ok(Command::Stop) => return,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        }
        if restarted {
            at_end = false;
        }

        if state.shared.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(8));
            continue;
        }

        if state.file.is_none() {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }

        if at_end {
            if state.ring.available_frames() == 0 {
                state.shared.active.store(false, Ordering::Relaxed);
                state.shared.paused.store(true, Ordering::Release);
                state.file = None;
                at_end = false;
                let _ = events.send(Event::Ended);
            } else {
                std::thread::sleep(Duration::from_millis(8));
            }
            continue;
        }

        if state.ring.free_frames() < FREE_THRESHOLD {
            std::thread::sleep(Duration::from_millis(3));
            continue;
        }

        let next = match state.file.as_mut() {
            Some(file) => file.next_packet(),
            None => break,
        };

        match next {
            Ok(Some(packet)) => state.process_packet(packet, &spectrum),
            Ok(None) => at_end = true,
            Err(error) => {
                log::warn!("playback stopped: {error:#}");
                state.file = None;
                state.shared.active.store(false, Ordering::Relaxed);
                state.shared.paused.store(true, Ordering::Release);
                let _ = events.send(Event::Error(format!("{error:#}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_round_trips_stereo_frames() {
        let ring = AudioRing::new(8);
        let input = [0.1, -0.1, 0.2, -0.2, 0.3, -0.3];
        assert_eq!(ring.push_stereo(&input), 3);
        assert_eq!(ring.available_frames(), 3);

        let mut output = [0.0f32; 6];
        assert_eq!(ring.pop_stereo(&mut output), 3);
        assert_eq!(output, input);
        assert_eq!(ring.available_frames(), 0);
    }

    #[test]
    fn ring_refuses_to_overwrite_unread_frames() {
        let ring = AudioRing::new(4);
        let input = [1.0f32; 12];
        assert_eq!(ring.push_stereo(&input), 4);
        assert_eq!(ring.push_stereo(&input), 0);

        let mut output = [0.0f32; 8];
        assert_eq!(ring.pop_stereo(&mut output), 4);
        assert_eq!(ring.push_stereo(&[2.0; 4]), 2);
    }

    #[test]
    fn ring_wraps_around() {
        let ring = AudioRing::new(4);
        for round in 0..5 {
            let input = [round as f32; 8];
            assert_eq!(ring.push_stereo(&input), 4);
            let mut output = [0.0f32; 8];
            assert_eq!(ring.pop_stereo(&mut output), 4);
            assert!(output.iter().all(|sample| *sample == round as f32));
        }
    }

    #[test]
    fn clear_drops_queued_frames() {
        let ring = AudioRing::new(4);
        assert_eq!(ring.push_stereo(&[1.0; 8]), 4);
        ring.clear();
        assert_eq!(ring.available_frames(), 0);
        let mut output = [0.0f32; 8];
        assert_eq!(ring.pop_stereo(&mut output), 0);
    }

    #[test]
    fn shared_position_tracks_consumed_frames() {
        let shared = Shared::new(48_000, 0.5, [0.0; 5]);
        shared.position_frames.store(48_000, Ordering::Relaxed);
        assert!(
            (shared.position_frames.load(Ordering::Relaxed) as f64 / 48_000.0 - 1.0).abs() < 1e-9
        );
    }
}
