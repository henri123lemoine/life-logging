# Life-Logging

Life-Logging is a Rust-based continuous audio recording service that captures and serves the most recent audio data.

## Features

- Continuous audio recording using CPAL
- Circular buffer to store the most recent audio data
- RESTful API to retrieve audio data as WAV files
- Real-time audio visualization
- Configurable buffer size and sample rate

## Getting Started

### Prerequisites

- Rust 1.54 or later
- An audio input device

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/henri123lemoine/life-logging.git
   cd life-logging
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Run the server:
   ```bash
   cargo run --release
   ```

The server will start on `http://127.0.0.1:3000`.

## Usage

- **Get Audio**: `GET /get_audio` - Returns the most recent audio data as a WAV file.
- **Visualize Audio**: `GET /visualize_audio` - Returns a PNG image visualizing the recent audio data.
- **Health Check**: `GET /health` - Returns the server's health status.

## Configuration

Current configuration is set in `src/main.rs`:

- `SAMPLE_RATE`: 48000 Hz
- `MAX_BUFFER_DURATION`: 60 seconds
- `BUFFER_SIZE`: SAMPLE_RATE * MAX_BUFFER_DURATION

## Future Improvements

- Additional audio format support (MP3, OGG)
- Enhanced visualizations and customization options
- Metrics and monitoring
- Authentication and rate limiting for public deployments
- Long-term storage and retrieval of audio data (e.g. save audio data to disk every minute, and persist in s3)

## Acknowledgments

- [CPAL](https://github.com/RustAudio/cpal) for audio capture
- [Axum](https://github.com/tokio-rs/axum) for the web framework
- [Plotters](https://github.com/plotters-rs/plotters) for audio visualization
