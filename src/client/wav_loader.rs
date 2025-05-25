use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::audio::{Decodable, Source};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use hound::{SampleFormat as HoundSampleFormatEnum, WavReader, WavSpec};
use rubato::{Resampler, ResamplerConstructionError, SincFixedIn};
use std::io::Cursor;
use std::time::Duration;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum MorrowindWavError {
    #[error("IO Error loading WAV: {0}")]
    Io(#[from] std::io::Error),

    #[error("Hound WAV parsing error: {0}")]
    Hound(#[from] hound::Error),

    #[error("Unsupported WAV format: {spec:?}")]
    UnsupportedFormat { spec: WavSpec },

    #[error("Rubato resampling error: {0}")]
    ResamplingError(#[from] rubato::ResampleError),

    #[error("Rubato resampler construction error: {0}")]
    ResamplerConstructionError(#[from] ResamplerConstructionError),
}

#[derive(Asset, Clone, TypePath, Debug)]
pub struct MorrowindDecodedWav {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Default)]
pub struct MorrowindWavLoader;

impl AssetLoader for MorrowindWavLoader {
    type Asset = MorrowindDecodedWav;
    type Settings = ();
    type Error = MorrowindWavError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let cursor = Cursor::new(bytes);
        let mut wav_reader = WavReader::new(cursor)?;

        let spec = wav_reader.spec();
        let asset_path_str = load_context.path().to_string_lossy();

        if spec.channels == 0 || spec.sample_rate == 0 {
            error!(
                "MorrowindWavLoader: Read spec with ZERO channels or sample_rate for {}! Spec: {:?}",
                asset_path_str, spec
            );
            return Err(MorrowindWavError::UnsupportedFormat { spec });
        }

        let mut samples_f32: Vec<f32> = Vec::new();

        match spec.sample_format {
            HoundSampleFormatEnum::Int => match spec.bits_per_sample {
                8 => {
                    for sample_result in wav_reader.samples::<i8>() {
                        let raw_sample = sample_result?;
                        let u_sample = (raw_sample as i16 + 128) as u8;
                        samples_f32.push((u_sample as f32 / 255.0) * 2.0 - 1.0);
                    }
                }
                16 => {
                    for sample_result in wav_reader.samples::<i16>() {
                        samples_f32.push(sample_result? as f32 / (std::i16::MAX as f32));
                    }
                }
                24 => {
                    for sample_result in wav_reader.samples::<i32>() {
                        let val = sample_result? >> 8;
                        samples_f32.push(val as f32 / 8_388_607.0);
                    }
                }
                32 => {
                    for sample_result in wav_reader.samples::<i32>() {
                        samples_f32.push(sample_result? as f32 / (std::i32::MAX as f32));
                    }
                }
                _ => return Err(MorrowindWavError::UnsupportedFormat { spec }),
            },
            HoundSampleFormatEnum::Float => match spec.bits_per_sample {
                32 => {
                    for sample_result in wav_reader.samples::<f32>() {
                        samples_f32.push(sample_result?);
                    }
                }
                _ => return Err(MorrowindWavError::UnsupportedFormat { spec }),
            },
        }

        let mut final_sample_rate = spec.sample_rate;
        let mut final_samples_vec = samples_f32;

        if asset_path_str.ends_with("sounds/bodyfallMED.wav")
            && spec.sample_rate == 22050
            && spec.channels == 1
        {
            let target_sample_rate = 44100;
            let params = rubato::SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: rubato::SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: rubato::WindowFunction::BlackmanHarris2,
            };
            // Create a SincFixedIn resampler for mono (1 channel)
            let mut resampler = SincFixedIn::<f32>::new(
                target_sample_rate as f64 / spec.sample_rate as f64, // ratio
                2.0, // asynchronous, ratio is varying slightly
                params,
                final_samples_vec.len(), // input frames for one channel
                1,                       // num_channels
            )?; // This ? now works due to From<ResamplerConstructionError> for MorrowindWavError

            // Prepare input for rubato (Vec<&[f32]>)
            let input_buffer_slice: Vec<&[f32]> = vec![final_samples_vec.as_slice()];
            let resampled_waves = resampler.process(&input_buffer_slice, None)?; // This ? works due to From<ResampleError>

            if !resampled_waves.is_empty() {
                final_samples_vec = resampled_waves[0].clone(); // Get the resampled data for the first (only) channel
                final_sample_rate = target_sample_rate;
            } else {
                error!(
                    "Resampling resulted in empty buffer for: {}",
                    asset_path_str
                );
                // Keep original samples if resampling fails to produce output
            }
        }

        Ok(MorrowindDecodedWav {
            samples: final_samples_vec,
            sample_rate: final_sample_rate,
            channels: spec.channels,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["wav"]
    }
}

pub struct MorrowindWavDecoder {
    samples_iter: std::vec::IntoIter<f32>,
    sample_rate: u32,
    channels: u16,
    total_duration: Duration,
}

impl Iterator for MorrowindWavDecoder {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.samples_iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.samples_iter.size_hint()
    }
}
impl ExactSizeIterator for MorrowindWavDecoder {}

// Implement bevy::audio::Source as indicated by the compiler error
impl Source for MorrowindWavDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.total_duration)
    }
}

impl Decodable for MorrowindDecodedWav {
    type DecoderItem = f32;
    type Decoder = MorrowindWavDecoder; // Must implement bevy::audio::Source

    fn decoder(&self) -> Self::Decoder {
        let num_total_samples_f32 = self.samples.len();
        let calculated_duration_secs_f64 = if self.sample_rate > 0 && self.channels > 0 {
            (num_total_samples_f32 as f64 / self.channels as f64) / self.sample_rate as f64
        } else {
            0.0
        };

        if self.channels == 0 || self.sample_rate == 0 {
            error!(
                "FATAL: MorrowindDecodedWav::decoder() called with channels OR sample_rate as ZERO. Asset sample_rate: {}, channels: {}.",
                self.sample_rate, self.channels
            );
        }

        MorrowindWavDecoder {
            samples_iter: self.samples.clone().into_iter(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            total_duration: Duration::from_secs_f64(calculated_duration_secs_f64),
        }
    }
}
