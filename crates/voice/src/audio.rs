//! Audio as Whisper takes it: mono `f32` samples at 16 kHz.
//!
//! Microphones run at 44.1 or 48 kHz. Speech needs nothing above 8 kHz,
//! so a plain resampler does: going down, each sample is the mean of the
//! input it spans, which also keeps out most of what would alias; going
//! up, it lies on the straight line between its neighbours.

/// The sample rate Whisper takes.
pub const SAMPLE_RATE: u32 = 16_000;

/// `samples`, recorded at `rate`, at [`SAMPLE_RATE`]. A rate of 0 has no
/// samples to give.
pub fn resample(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE {
        return samples.to_vec();
    }
    if rate == 0 || samples.is_empty() {
        return Vec::new();
    }
    let step = f64::from(rate) / f64::from(SAMPLE_RATE);
    let len = (samples.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let start = i as f64 * step;
            let at = start as usize;
            if step > 1.0 {
                let end = (((i + 1) as f64 * step) as usize).clamp(at + 1, samples.len());
                samples[at..end].iter().sum::<f32>() / (end - at) as f32
            } else {
                let next = samples.get(at + 1).copied().unwrap_or(samples[at]);
                samples[at] + (next - samples[at]) * (start - at as f64) as f32
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whispers_rate_is_kept() {
        assert_eq!(resample(&[0.1, -0.2, 0.3], SAMPLE_RATE), [0.1, -0.2, 0.3]);
    }

    #[test]
    fn down_takes_the_mean() {
        let second = vec![0.25; 48_000];
        let down = resample(&second, 48_000);
        assert_eq!(down.len(), 16_000);
        assert!(down.iter().all(|s| (s - 0.25).abs() < 1e-6));
        assert_eq!(
            resample(&[0.0, 0.25, 0.5, 1.0, 1.0, 1.0], 48_000),
            [0.25, 1.0]
        );
        assert_eq!(resample(&vec![0.0; 44_100], 44_100).len(), 16_000);
    }

    #[test]
    fn up_draws_lines() {
        let up = resample(&[0.0, 1.0, 0.0], 8_000);
        assert_eq!(up, [0.0, 0.5, 1.0, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn nothing_gives_nothing() {
        assert!(resample(&[], 48_000).is_empty());
        assert!(resample(&[0.5; 10], 0).is_empty());
    }
}
