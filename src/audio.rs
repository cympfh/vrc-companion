use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    last_sound_time: Arc<Mutex<Instant>>,
    silence_threshold: f32,
    recording_start_time: Arc<Mutex<Option<Instant>>>,
    current_max_amplitude: Arc<Mutex<f32>>,
}

impl AudioRecorder {
    pub fn new(silence_threshold: f32) -> Self {
        Self {
            stream: None,
            sample_rate: 0,
            last_sound_time: Arc::new(Mutex::new(Instant::now())),
            silence_threshold,
            recording_start_time: Arc::new(Mutex::new(None)),
            current_max_amplitude: Arc::new(Mutex::new(0.0)),
        }
    }

    pub fn get_max_amplitude(&self) -> f32 {
        *self.current_max_amplitude.lock().unwrap()
    }

    pub fn is_silent(&self, silence_duration_secs: f32) -> bool {
        let start_time = self.recording_start_time.lock().unwrap();
        if let Some(start) = *start_time {
            if start.elapsed() < Duration::from_secs(3) {
                return false;
            }
        }

        let last_sound = self.last_sound_time.lock().unwrap();
        last_sound.elapsed() >= Duration::from_secs_f32(silence_duration_secs)
    }

    pub fn get_silence_duration(&self) -> Duration {
        self.last_sound_time.lock().unwrap().elapsed()
    }

    pub fn get_recording_duration(&self) -> f32 {
        let start_time = self.recording_start_time.lock().unwrap();
        start_time.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0)
    }

    /// 指定した入力デバイス(未指定ならデフォルト)で録音を開始する
    pub fn start_recording(
        &mut self,
        device_name: Option<&str>,
        chunk_sender: UnboundedSender<Vec<f32>>,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| format!("Failed to get input devices: {}", e))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or(format!("Input device '{}' not found", name))?
        } else {
            host.default_input_device()
                .ok_or("No input device available")?
        };

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        *self.last_sound_time.lock().unwrap() = Instant::now();
        *self.current_max_amplitude.lock().unwrap() = 0.0;
        *self.recording_start_time.lock().unwrap() = Some(Instant::now());

        self.sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let chunk_size = (self.sample_rate as f32 * 0.1) as usize; // 100ms

        let last_sound_time = Arc::clone(&self.last_sound_time);
        let current_max_amplitude = Arc::clone(&self.current_max_amplitude);
        let threshold = self.silence_threshold;

        let stream = build_input_stream(
            &device,
            &config,
            last_sound_time,
            current_max_amplitude,
            threshold,
            channels,
            chunk_sender,
            chunk_size,
        )?;

        stream
            .play()
            .map_err(|e| format!("Failed to play stream: {}", e))?;
        self.stream = Some(stream);

        Ok(())
    }

    /// stream を drop すると chunk_sender も drop され、
    /// 受信側は disconnect を検知して audio.done を送信できる
    pub fn stop_recording(&mut self) {
        self.stream = None;
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// 利用可能な入力デバイス名の一覧を取得する
pub fn get_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to get input devices: {}", e))?;

    Ok(devices.filter_map(|d| d.name().ok()).collect())
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    last_sound_time: Arc<Mutex<Instant>>,
    current_max_amplitude: Arc<Mutex<f32>>,
    threshold: f32,
    channels: u16,
    chunk_sender: UnboundedSender<Vec<f32>>,
    chunk_size: usize,
) -> Result<cpal::Stream, String> {
    let stream_config: cpal::StreamConfig = config.clone().into();
    let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);
    let mut local_chunk: Vec<f32> = Vec::with_capacity(chunk_size);

    macro_rules! build {
        ($sample_type:ty) => {
            device.build_input_stream(
                &stream_config,
                move |data: &[$sample_type], _: &cpal::InputCallbackInfo| {
                    let mono_samples: Vec<f32> = if channels == 1 {
                        data.iter()
                            .map(|&s| <f32 as cpal::Sample>::from_sample(s))
                            .collect()
                    } else {
                        data.chunks_exact(channels as usize)
                            .map(|chunk| {
                                let sum: f32 = chunk
                                    .iter()
                                    .map(|&s| <f32 as cpal::Sample>::from_sample(s))
                                    .sum();
                                sum / channels as f32
                            })
                            .collect()
                    };

                    let mut has_sound = false;
                    let mut max_amplitude = 0.0f32;
                    for sample in mono_samples {
                        let abs_sample = sample.abs();
                        max_amplitude = max_amplitude.max(abs_sample);
                        if abs_sample > threshold {
                            has_sound = true;
                        }
                        local_chunk.push(sample);
                        if local_chunk.len() >= chunk_size {
                            let _ = chunk_sender.send(local_chunk.drain(..).collect());
                        }
                    }

                    {
                        let mut current_max = current_max_amplitude.lock().unwrap();
                        *current_max = current_max.max(max_amplitude) * 0.95;
                    }
                    if has_sound {
                        *last_sound_time.lock().unwrap() = Instant::now();
                    }
                },
                err_fn,
                None,
            )
        };
    }

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::U16 => build!(u16),
        other => return Err(format!("Unsupported sample format: {:?}", other)),
    }
    .map_err(|e| format!("Failed to build input stream: {}", e))?;

    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_silent_within_grace_period() {
        let recorder = AudioRecorder::new(0.01);
        *recorder.recording_start_time.lock().unwrap() = Some(Instant::now());
        assert!(!recorder.is_silent(0.1));
    }

    #[test]
    fn test_get_max_amplitude_default() {
        let recorder = AudioRecorder::new(0.01);
        assert_eq!(recorder.get_max_amplitude(), 0.0);
    }

    #[test]
    fn test_get_recording_duration_zero_when_not_started() {
        let recorder = AudioRecorder::new(0.01);
        assert_eq!(recorder.get_recording_duration(), 0.0);
    }
}
