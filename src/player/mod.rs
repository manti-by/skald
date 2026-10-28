//! Playback: symphonia decoding, a resampler, the biquad equalizer, the FFT
//! waterfall and cpal output.

pub mod decoder;
pub mod dsp;
pub mod engine;
pub mod spectrum;

pub use dsp::{EQ_FREQS, EQ_LABELS};
pub use engine::{Event, Player};
