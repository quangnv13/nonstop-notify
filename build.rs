use std::env;
use std::fs;
use std::path::PathBuf;

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("notification.wav");
    fs::write(output, notification_wav()).expect("write generated notification WAV");
    tauri_build::build();
}

fn notification_wav() -> Vec<u8> {
    let samples = chime_samples();
    let data_len = (samples.len() * size_of::<i16>()) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&CHANNELS.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(
        &(SAMPLE_RATE * u32::from(CHANNELS) * u32::from(BITS_PER_SAMPLE) / 8).to_le_bytes(),
    );
    wav.extend_from_slice(&(CHANNELS * BITS_PER_SAMPLE / 8).to_le_bytes());
    wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

fn chime_samples() -> Vec<i16> {
    let tones = [(440u32, 100u32), (523u32, 150u32)];
    let fade_samples = SAMPLE_RATE * 20 / 1_000;
    let amplitude = f32::from(i16::MAX) * 0.12;
    let mut samples = Vec::new();
    for (frequency, duration_ms) in tones {
        let sample_count = SAMPLE_RATE * duration_ms / 1_000;
        for index in 0..sample_count {
            let phase =
                std::f32::consts::TAU * frequency as f32 * index as f32 / SAMPLE_RATE as f32;
            let fade =
                index.min(sample_count - index - 1).min(fade_samples) as f32 / fade_samples as f32;
            samples.push((phase.sin() * amplitude * fade) as i16);
        }
    }
    samples
}
