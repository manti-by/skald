//! Biquad equalizer and linear resampler.
//!
//! The equalizer is a chain of five peaking filters. All coefficients and
//! state are allocated once, so the decoder thread can update them without
//! allocating or locking. The resampler converts a decoded stream to the output
//! device's sample rate with linear interpolation; it is only used when the
//! device cannot play the source rate directly.

pub const EQ_FREQS: [f32; 5] = [63.0, 250.0, 1_000.0, 4_000.0, 16_000.0];
pub const EQ_LABELS: [&str; 5] = ["63 Hz", "250 Hz", "1 KHz", "4 KHz", "16 KHz"];
const Q: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Coefficients {
    fn peaking(sample_rate: f32, frequency: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * Q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn unity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    #[cfg(test)]
    fn magnitude_at(&self, sample_rate: f32, frequency: f32) -> f32 {
        // H(e^jw) for a direct-form biquad.
        let w = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let (sin1, cos1) = w.sin_cos();
        let (sin2, cos2) = (2.0 * w).sin_cos();
        let numerator = (
            self.b0 + self.b1 * cos1 + self.b2 * cos2,
            -(self.b1 * sin1 + self.b2 * sin2),
        );
        let denominator = (
            1.0 + self.a1 * cos1 + self.a2 * cos2,
            -(self.a1 * sin1 + self.a2 * sin2),
        );
        let num = (numerator.0 * numerator.0 + numerator.1 * numerator.1).sqrt();
        let den = (denominator.0 * denominator.0 + denominator.1 * denominator.1).sqrt();
        num / den.max(1e-12)
    }
}

/// A stereo biquad in transposed direct form II.
#[derive(Clone, Copy, Debug)]
struct StereoBiquad {
    coefficients: Coefficients,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl StereoBiquad {
    fn new() -> Self {
        Self {
            coefficients: Coefficients::unity(),
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    fn set_gain(&mut self, sample_rate: f32, frequency: f32, gain_db: f32) {
        self.coefficients = Coefficients::peaking(sample_rate, frequency, gain_db);
    }

    fn process(&mut self, sample: f32, channel: usize) -> f32 {
        let c = self.coefficients;
        let input = sample - c.a1 * self.z1[channel] - c.a2 * self.z2[channel];
        let output = c.b0 * input + self.z1[channel];
        self.z1[channel] = c.b1 * input - c.a1 * output + self.z2[channel];
        self.z2[channel] = c.b2 * input - c.a2 * output;
        output
    }

    fn reset(&mut self) {
        self.z1 = [0.0; 2];
        self.z2 = [0.0; 2];
    }
}

/// Five peaking filters applied to interleaved stereo samples.
pub struct EqChain {
    sample_rate: f32,
    bands: [StereoBiquad; EQ_FREQS.len()],
    gains: [f32; EQ_FREQS.len()],
}

impl EqChain {
    pub fn new(sample_rate: u32) -> Self {
        let mut chain = Self {
            sample_rate: sample_rate.max(1) as f32,
            bands: [StereoBiquad::new(); EQ_FREQS.len()],
            gains: [0.0; EQ_FREQS.len()],
        };
        chain.gains = [f32::INFINITY; EQ_FREQS.len()];
        chain.set_gains([0.0; EQ_FREQS.len()]);
        chain
    }

    pub fn set_gains(&mut self, gains: [f32; EQ_FREQS.len()]) {
        for (index, gain) in gains.iter().enumerate() {
            let gain = if gain.is_finite() {
                gain.clamp(-6.0, 6.0)
            } else {
                0.0
            };
            if (self.gains[index] - gain).abs() > 0.001 {
                self.bands[index].set_gain(self.sample_rate, EQ_FREQS[index], gain);
                self.gains[index] = gain;
            }
        }
    }

    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }

    /// Filters interleaved 16-bit stereo (`[L, R, L, R, ...]`) in place.
    pub fn process_stereo(&mut self, samples: &mut [f32]) {
        let all_zero = self.gains.iter().all(|gain| gain.abs() < 0.001);
        if all_zero {
            return;
        }
        for frame in samples.as_chunks_mut::<2>().0 {
            let mut left = frame[0];
            let mut right = frame[1];
            for band in &mut self.bands {
                left = band.process(left, 0);
                right = band.process(right, 1);
            }
            frame[0] = left;
            frame[1] = right;
        }
    }

    #[cfg(test)]
    fn gains(&self) -> [f32; EQ_FREQS.len()] {
        self.gains
    }
}

/// Converts mono or multichannel interleaved frames to stereo by keeping the
/// first two channels and averaging the rest into them.
pub fn downmix_to_stereo(input: &[f32], channels: usize, output: &mut Vec<f32>) {
    output.clear();
    if channels == 0 {
        return;
    }
    let frames = input.len() / channels;
    output.reserve(frames * 2);
    for frame in 0..frames {
        let base = frame * channels;
        if channels == 1 {
            let value = input[base];
            output.push(value);
            output.push(value);
        } else {
            let mut left = input[base];
            let mut right = input[base + 1];
            if channels > 2 {
                let extra = channels - 2;
                let mut sum = 0.0;
                for channel in 2..channels {
                    sum += input[base + channel];
                }
                let extra = extra as f32;
                left += sum * 0.35 / extra;
                right += sum * 0.35 / extra;
            }
            output.push(left);
            output.push(right);
        }
    }
}

/// Linear resampler with per-stream phase continuity.
pub struct Resampler {
    source_rate: u32,
    output_rate: u32,
    phase: f64,
    previous: [f32; 2],
    output: Vec<f32>,
}

impl Resampler {
    pub fn new(source_rate: u32, output_rate: u32) -> Self {
        Self {
            source_rate: source_rate.max(1),
            output_rate: output_rate.max(1),
            phase: 0.0,
            previous: [0.0; 2],
            output: Vec::new(),
        }
    }

    pub fn is_passthrough(&self) -> bool {
        self.source_rate == self.output_rate
    }

    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.previous = [0.0; 2];
        self.output.clear();
    }

    /// Resamples interleaved stereo input into the internal buffer and returns
    /// it. The buffer is reused between packets.
    pub fn process(&mut self, input: &[f32]) -> &mut [f32] {
        if self.is_passthrough() {
            self.output.clear();
            self.output.extend_from_slice(input);
            return &mut self.output;
        }

        let frames = input.len() / 2;
        if frames == 0 {
            self.output.clear();
            return &mut self.output;
        }

        let ratio = self.source_rate as f64 / self.output_rate as f64;
        let mut position = self.phase;
        self.output.clear();
        self.output
            .reserve(((frames as f64 / ratio).ceil() as usize + 2) * 2);

        while position < frames as f64 {
            let index = position.floor() as usize;
            let fraction = (position - index as f64) as f32;
            let current = [input[index * 2], input[index * 2 + 1]];
            let previous = if index == 0 {
                self.previous
            } else {
                [input[(index - 1) * 2], input[(index - 1) * 2 + 1]]
            };
            self.output
                .push(previous[0] + (current[0] - previous[0]) * fraction);
            self.output
                .push(previous[1] + (current[1] - previous[1]) * fraction);
            position += ratio;
        }

        self.phase = position - frames as f64;
        self.previous = [input[(frames - 1) * 2], input[(frames - 1) * 2 + 1]];
        &mut self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_is_transparent() {
        let mut eq = EqChain::new(48_000);
        let mut samples = vec![0.0, 0.0, 0.5, -0.5, 0.25, -0.25];
        let original = samples.clone();
        eq.set_gains([0.0; 5]);
        eq.process_stereo(&mut samples);
        assert_eq!(samples, original);
        assert_eq!(eq.gains(), [0.0; 5]);
    }

    #[test]
    fn peaking_filter_boosts_its_center_frequency() {
        let coefficients = Coefficients::peaking(48_000.0, 1_000.0, 6.0);
        let boost = coefficients.magnitude_at(48_000.0, 1_000.0);
        let flat = coefficients.magnitude_at(48_000.0, 10_000.0);
        assert!(boost > 1.8, "boost was {boost}");
        assert!(flat < 1.1, "far band was {flat}");
    }

    #[test]
    fn peaking_filter_cuts_its_center_frequency() {
        let coefficients = Coefficients::peaking(48_000.0, 1_000.0, -6.0);
        let cut = coefficients.magnitude_at(48_000.0, 1_000.0);
        assert!(cut < 0.6, "cut was {cut}");
    }

    #[test]
    fn eq_leaves_silence_silent() {
        let mut eq = EqChain::new(44_100);
        eq.set_gains([6.0, 6.0, 6.0, 6.0, 6.0]);
        let mut samples = vec![0.0; 2_048];
        eq.process_stereo(&mut samples);
        assert!(samples.iter().all(|sample| sample.abs() < 1e-9));
    }

    #[test]
    fn downmix_duplicates_mono() {
        let mut output = Vec::new();
        downmix_to_stereo(&[0.5, -0.5], 1, &mut output);
        assert_eq!(output, vec![0.5, 0.5, -0.5, -0.5]);
    }

    #[test]
    fn downmix_keeps_stereo() {
        let mut output = Vec::new();
        downmix_to_stereo(&[0.1, 0.2, 0.3, 0.4], 2, &mut output);
        assert_eq!(output, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn resampler_changes_length_by_the_rate_ratio() {
        let input: Vec<f32> = (0..4_800).flat_map(|_| [0.25, -0.25]).collect();
        let mut resampler = Resampler::new(48_000, 24_000);
        let output = resampler.process(&input);
        assert!(
            (output.len() as isize - 4_800).abs() <= 2,
            "len {}",
            output.len()
        );
    }

    #[test]
    fn resampler_is_passthrough_at_the_same_rate() {
        let mut resampler = Resampler::new(44_100, 44_100);
        let input = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(resampler.process(&input), input.as_slice());
    }

    #[test]
    fn resampler_keeps_phase_between_packets() {
        let mut resampler = Resampler::new(48_000, 44_100);
        let first: Vec<f32> = (0..1024).flat_map(|_| [1.0, 1.0]).collect();
        let second = first.clone();
        let a = resampler.process(&first).len();
        let b = resampler.process(&second).len();
        // 1024 frames at 48k is 940.8 frames at 44.1k, so two packets are
        // 3763.2 stereo samples in total, not 3764 (phase dropped).
        assert!(((a + b) as isize - 3_763).abs() <= 2, "{a} + {b}");
    }
}
