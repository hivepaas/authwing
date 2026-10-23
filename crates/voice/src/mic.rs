//! Hold-to-talk recording from the default microphone.
//!
//! The UI starts a [`Recording`] when the key goes down and stops it when
//! the key comes up. Each frame is averaged to mono as it arrives, and the
//! whole is resampled to 16 kHz at the end, which takes no time for a few
//! seconds of speech. What comes after [`LONGEST`] is dropped: naming an
//! account takes a second or two, and a key held down by mistake should
//! not fill memory or keep Whisper busy.
//!
//! On macOS the app's bundle must say why it records
//! (`NSMicrophoneUsageDescription`), or the system will not let it; until
//! the user allows it, the microphone gives silence.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::resample;

/// The longest recording kept.
pub const LONGEST: Duration = Duration::from_secs(6);

/// Why the microphone could not record.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("there is no microphone")]
    NoMicrophone,
    #[error("the microphone gives {0} samples, which are not read")]
    Format(SampleFormat),
    #[error("microphone: {0}")]
    Audio(#[from] cpal::Error),
}

/// A recording from the default microphone, running until stopped.
pub struct Recording {
    stream: Stream,
    heard: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

impl Recording {
    /// Starts recording from the system's default input device.
    pub fn start() -> Result<Self, Error> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or(Error::NoMicrophone)?;
        let supported = device.default_input_config()?;
        let config = supported.config();
        let rate = config.sample_rate;
        let longest = (LONGEST.as_secs_f64() * f64::from(rate)) as usize;
        let heard = Arc::new(Mutex::new(Vec::with_capacity(longest)));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => listen::<f32>(&device, config, &heard, longest),
            SampleFormat::I16 => listen::<i16>(&device, config, &heard, longest),
            SampleFormat::I32 => listen::<i32>(&device, config, &heard, longest),
            SampleFormat::U16 => listen::<u16>(&device, config, &heard, longest),
            other => return Err(Error::Format(other)),
        }?;
        stream.play()?;
        Ok(Self {
            stream,
            heard,
            rate,
        })
    }

    /// Stops recording; what was heard, mono at [`crate::SAMPLE_RATE`].
    /// Dropping a recording stops it too, keeping nothing.
    pub fn stop(self) -> Vec<f32> {
        drop(self.stream);
        let heard = self.heard.lock().unwrap_or_else(PoisonError::into_inner);
        resample(&heard, self.rate)
    }
}

/// A stream from `device` that adds each frame's mean to `heard`, up to
/// `longest` samples. Stream errors are let go: the recording ends short,
/// and what it has is still worth hearing.
fn listen<T>(
    device: &Device,
    config: StreamConfig,
    heard: &Arc<Mutex<Vec<f32>>>,
    longest: usize,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels.max(1));
    let heard = Arc::clone(heard);
    device.build_input_stream(
        config,
        move |data: &[T], _: &_| {
            let mut heard = heard.lock().unwrap_or_else(PoisonError::into_inner);
            let room = longest.saturating_sub(heard.len());
            heard.extend(data.chunks(channels).take(room).map(|frame| {
                frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / frame.len() as f32
            }));
        },
        |_| {},
        None,
    )
}
