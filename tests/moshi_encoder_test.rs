use candle_core::{Device, Tensor};
use hound::{WavReader, WavSpec, WavWriter};
use moshi::encodec::{Config, Encodec};
use std::path::Path;

#[test]
fn test_moshi_encoder() -> Result<(), Box<dyn std::error::Error>> {
    // Load the WAV file
    let input_path = Path::new("/Users/henrilemoine/Downloads/samples_gb0.wav");
    let mut reader = WavReader::open(input_path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    // Resample to 24kHz if necessary
    let resampled_samples = if spec.sample_rate != 24000 {
        println!("Resampling from {}Hz to 24000Hz", spec.sample_rate);
        resample(&samples, spec.sample_rate, 24000)
    } else {
        samples
    };

    // Create the Moshi Encodec
    let device = Device::Cpu;
    let config = Config::v0_1(None);
    let vb = candle_nn::VarBuilder::zeros(candle_core::DType::F32, &device);
    let mut encodec = Encodec::new(config, vb)?;

    // Prepare input tensor
    let input_tensor =
        Tensor::from_slice(&resampled_samples, (1, 1, resampled_samples.len()), &device)?;

    // Encode the audio
    let encoded_tensor = encodec.encode(&input_tensor)?;

    // Print the size of the encoded data
    println!(
        "Original audio duration: {} seconds",
        resampled_samples.len() as f32 / 24000.0
    );
    println!("Original audio size: {} bytes", resampled_samples.len() * 4);
    println!("Encoded tensor shape: {:?}", encoded_tensor.shape());

    // Decode the audio
    let decoded_tensor = encodec.decode(&encoded_tensor)?;

    // Convert decoded tensor to samples
    let decoded_samples: Vec<f32> = decoded_tensor.flatten_all()?.to_vec1()?;

    // Save the decoded audio as a new WAV file
    let output_path = Path::new("output.wav");
    let mut writer = WavWriter::create(
        output_path,
        WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;

    for &sample in &decoded_samples {
        writer.write_sample((sample * 32767.0) as i16)?;
    }
    writer.finalize()?;

    println!("Decoding complete. Output saved to 'output.wav'");

    Ok(())
}

fn resample(data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return data.to_vec();
    }

    if data.is_empty() {
        return Vec::new();
    }

    let ratio = from_rate as f32 / to_rate as f32;
    let new_len = (data.len() as f32 / ratio).ceil() as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let pos = i as f32 * ratio;
        let index = (pos.floor() as usize).min(data.len() - 1);
        let next_index = (index + 1).min(data.len() - 1);
        let frac = pos - pos.floor();

        let sample = data[index] * (1.0 - frac) + data[next_index] * frac;
        resampled.push(sample);
    }

    resampled
}
