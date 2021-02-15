use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::RingBuffer;

pub struct Audio {
    config: cpal::StreamConfig,
    _stream: cpal::Stream,
    prod: ringbuf::Producer<[f32; 2]>,
}

impl Audio {
    const BUFFER_LEN: usize = 2048;

    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No audio output device available!");
        let config = device
            .default_output_config()
            .expect("No audio output config available!");

        match config.sample_format() {
            cpal::SampleFormat::F32 => Audio::init::<f32>(device, config.into()),
            cpal::SampleFormat::I16 => Audio::init::<i16>(device, config.into()),
            cpal::SampleFormat::U16 => Audio::init::<u16>(device, config.into()),
        }
    }

    fn init<T: cpal::Sample>(device: cpal::Device, config: cpal::StreamConfig) -> Self {
        let buffer = RingBuffer::<[f32; 2]>::new(Audio::BUFFER_LEN);
        let (prod, mut cons) = buffer.split();

        let output_config = OutputConfig::from(config.channels);
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(output_config as usize) {
                        let samples = cons.pop().unwrap_or_else(|| [0.0, 0.0]);
                        match output_config {
                            OutputConfig::Mono => {
                                let sample = samples.iter().sum::<f32>() / 2.0;
                                frame[0] = cpal::Sample::from::<f32>(&sample);
                            }
                            OutputConfig::Stereo => {
                                frame[0] = cpal::Sample::from::<f32>(&(samples[0]));
                                frame[1] = cpal::Sample::from::<f32>(&(samples[1]));
                            }
                        }
                    }
                },
                |err| error!("Audio Stream Error: {}", err),
            )
            .unwrap();
        stream.play().unwrap();

        Audio {
            config,
            _stream: stream,
            prod,
        }
    }

    pub fn push_sample(&mut self, left_sample: f32, right_sample: f32) {
        while self.prod.is_full() {} // TODO: Block thread instead of using CPU
        self.prod.push([left_sample, right_sample]).unwrap();
    }

    pub fn sample_rate(&self) -> usize {
        self.config.sample_rate.0 as usize
    }
}

#[derive(Clone, Copy)]
enum OutputConfig {
    Mono = 1,
    Stereo = 2,
}

impl From<u16> for OutputConfig {
    fn from(value: u16) -> Self {
        use OutputConfig::*;
        match value {
            1 => Mono,
            2 => Stereo,
            _ => panic!("Only Mono and Stereo audio devices supported!"),
        }
    }
}
