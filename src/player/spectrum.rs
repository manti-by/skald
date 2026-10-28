//! FFT waterfall for the 350x197 spectrogram.
//!
//! The analyzer keeps a 2048-sample window with a 1024-sample hop and maps the
//! linear FFT result onto 197 logarithmically spaced frequency rows. Values are
//! normalized to 0..=1 and sent to the UI as one column per hop.

use rustfft::Fft;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use std::sync::Arc;

pub const FFT_SIZE: usize = 2_048;
pub const HOP: usize = 1_024;
pub const ROWS: usize = 197;
const MIN_FREQUENCY: f32 = 35.0;
const MAX_FREQUENCY: f32 = 18_000.0;
const DB_FLOOR: f32 = -72.0;

pub struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    samples: Vec<f32>,
    buffer: Vec<Complex32>,
    bin_ranges: Vec<(usize, usize)>,
}

impl SpectrumAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|index| {
                let phase = 2.0 * std::f32::consts::PI * index as f32 / (FFT_SIZE - 1) as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();

        let sample_rate = sample_rate.max(1) as f32;
        let bin_width = sample_rate / FFT_SIZE as f32;
        let ratio = MAX_FREQUENCY / MIN_FREQUENCY;
        let bin_ranges = (0..ROWS)
            .map(|row| {
                let position = row as f32 / (ROWS - 1) as f32;
                let frequency = MIN_FREQUENCY * ratio.powf(position);
                let center = (frequency / bin_width).round() as usize;
                let low = center.saturating_sub(1).clamp(1, FFT_SIZE / 2);
                let high = (center + 1).clamp(low, FFT_SIZE / 2);
                (low, high)
            })
            .collect();

        Self {
            fft,
            window,
            samples: Vec::with_capacity(FFT_SIZE * 2),
            buffer: vec![Complex32::default(); FFT_SIZE],
            bin_ranges,
        }
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Adds interleaved stereo samples and returns every complete column.
    pub fn push(&mut self, stereo: &[f32]) -> Vec<Vec<f32>> {
        for frame in stereo.as_chunks::<2>().0 {
            self.samples.push((frame[0] + frame[1]) * 0.5);
        }

        let mut columns = Vec::new();
        while self.samples.len() >= FFT_SIZE {
            columns.push(self.analyze());
            self.samples.drain(..HOP);
        }
        columns
    }

    fn analyze(&mut self) -> Vec<f32> {
        for (index, sample) in self.samples[..FFT_SIZE].iter().enumerate() {
            self.buffer[index] = Complex32::new(sample * self.window[index], 0.0);
        }
        self.fft.process(&mut self.buffer);

        let scale = 2.0 / FFT_SIZE as f32;
        let mut column = Vec::with_capacity(ROWS);
        for &(low, high) in &self.bin_ranges {
            let mut peak = 0.0f32;
            for bin in low..=high {
                let value = self.buffer[bin].norm() * scale;
                if value > peak {
                    peak = value;
                }
            }
            let db = 20.0 * peak.max(1e-9).log10();
            let normalized = ((db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0);
            column.push(normalized.powf(1.15));
        }
        column
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, sample_rate: f32, frames: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(frames * 2);
        for index in 0..frames {
            let value =
                (2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate).sin() * 0.8;
            samples.push(value);
            samples.push(value);
        }
        samples
    }

    #[test]
    fn columns_are_197_rows() {
        let mut analyzer = SpectrumAnalyzer::new(44_100);
        let columns = analyzer.push(&tone(1_000.0, 44_100.0, FFT_SIZE * 2));
        assert!(!columns.is_empty());
        assert!(columns.iter().all(|column| column.len() == ROWS));
    }

    #[test]
    fn silence_is_dark() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let columns = analyzer.push(&vec![0.0; FFT_SIZE * 2]);
        let peak = columns
            .iter()
            .flat_map(|column| column.iter())
            .fold(0.0f32, |peak, value| peak.max(*value));
        assert!(peak < 0.01, "peak {peak}");
    }

    #[test]
    fn a_tone_lights_the_matching_row() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let columns = analyzer.push(&tone(1_000.0, 48_000.0, FFT_SIZE * 3));
        let column = columns.last().expect("column");
        let (row, value) = column
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap();
        let expected = ((1_000f32 / MIN_FREQUENCY).ln() / (MAX_FREQUENCY / MIN_FREQUENCY).ln()
            * (ROWS - 1) as f32)
            .round() as usize;
        assert!(
            (row as isize - expected as isize).abs() <= 4,
            "row {row} expected {expected}"
        );
        assert!(*value > 0.7, "value {value}");
    }

    #[test]
    fn reset_drops_partial_windows() {
        let mut analyzer = SpectrumAnalyzer::new(44_100);
        analyzer.push(&vec![0.1; 100]);
        analyzer.reset();
        let columns = analyzer.push(&vec![0.0; HOP]);
        assert!(columns.is_empty());
    }
}
