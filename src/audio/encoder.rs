use tracing::info;
use std::process::Command;
use std::io::Write;
use tempfile::NamedTempFile;

pub trait AudioEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8>;
}

pub struct WavEncoder;

impl AudioEncoder for WavEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = (data.len() * 2) as u32; // 2 bytes per sample for 16-bit
        let file_size = 36 + data_size;

        let mut buffer = Vec::with_capacity(file_size as usize + 8);

        // RIFF header
        buffer.extend_from_slice(b"RIFF");
        buffer.extend_from_slice(&(file_size as u32).to_le_bytes());
        buffer.extend_from_slice(b"WAVE");

        // Format chunk
        buffer.extend_from_slice(b"fmt ");
        buffer.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
        buffer.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
        buffer.extend_from_slice(&channels.to_le_bytes());
        buffer.extend_from_slice(&sample_rate.to_le_bytes());
        buffer.extend_from_slice(&byte_rate.to_le_bytes());
        buffer.extend_from_slice(&block_align.to_le_bytes());
        buffer.extend_from_slice(&bits_per_sample.to_le_bytes());

        // Data chunk
        buffer.extend_from_slice(b"data");
        buffer.extend_from_slice(&data_size.to_le_bytes());

        // Audio data
        for &sample in data {
            let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            buffer.write_all(&value.to_le_bytes()).unwrap();
        }

        info!("Encoded {} samples into {} bytes of WAV data", data.len(), buffer.len());
        buffer
    }
}

pub struct FlacEncoder;

impl AudioEncoder for FlacEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
        // Create a temporary WAV file
        let mut temp_wav = NamedTempFile::new().unwrap();
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate);
        temp_wav.write_all(&wav_data).unwrap();
        
        // Use external FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg(temp_wav.path())
            .output()
            .expect("Failed to execute FLAC encoder");

        if !output.status.success() {
            panic!("FLAC encoding failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        info!("Encoded {} samples into {} bytes of FLAC data", data.len(), output.stdout.len());
        output.stdout
    }
}
