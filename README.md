# Lifelogging

![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/henri123lemoine/life-logging?include_prereleases)
![GitHub](https://img.shields.io/github/license/henri123lemoine/life-logging)

Lifelogging is a Rust-based project for [lifelogging](https://en.wikipedia.org/wiki/Lifelog). It runs a low-memory server that makes it easy for other projects on your machine to access audio data, transcriptions, keypresses, and more. This project is meant only for personal use.

## Features

Audio:

- Continuous audio recording.
- Circular buffer to store the most recent audio data.
- API to retrieve audio data as PCM/WAV/FLAC/OPUS files.
- Configurable buffer duration.
- Long-term audio persistence with AWS S3.

## Getting Started

### Prerequisites

- Rust (latest stable version)
- A system with audio input capabilities

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
   There are two options for running the server:
   - To run the server in the foreground (useful for debugging):
      ```bash
      cargo run --release
      ```
      To close the server, press `Ctrl+C`.
   - To run the server in the background:
      Make the script executable (first time only):
      ```bash
      chmod +x run_background.sh
      ```
      Run the script:
      ```bash
      ./run_background.sh
      ```
      This will start the server in the background, show you the process ID after compilation, and start logging output. You can run `tail -f logs/server.log` to follow the logs.
      Closing: Run `kill <PID>` to kill the process. If you forget the PID, you can find it with `ps aux | grep life-logging`.

The server will start on `http://127.0.0.1:61429`, or whichever port is chosen in your configuration.

## Usage

- **Get Audio**: `GET /get_audio` - Returns the most recent audio data. Query parameters:
  - `format`: The audio format to return. Supported formats are `pcm`, `wav`, `flac`, and `opus`. Default is `wav`.
  - `duration`: The duration of audio to return, in seconds. Default is the entire buffer, namely 20 minutes.
- **Visualize Audio**: `GET /visualize_audio` - Returns a PNG image visualizing the recent audio data. Useful for debugging.

Run the app and visit `http://localhost:61429/swagger-ui/` to view the full API documentation.

### Examples

Retrieve the last minute of audio in WAV format:
```bash
curl "http://127.0.0.1:61429/get_audio?format=wav&duration=60" --output recent_audio.wav
```

Visualize recent audio data: Go to `http://127.0.0.1:61429/visualize_audio` in your browser.

### Configuration

The application uses a flexible configuration system that supports both file-based configuration and environment variable overrides.

### Configuration File

The default configuration is set in `config/default.toml`. To override the default configuration, add another `.toml` file to the `config` directory with your preferred settings. The server will automatically load the configuration from this file.

Temporarily, AWS configuration is stored in a `.env` file. You can copy the `.env.example` file to `.env` and fill in your AWS credentials and bucket configurations.

### Logging

Set the `RUST_LOG` environment variable to `info` or `debug` to see more detailed logs. Valid log levels are: `error`, `warn`, `info`, `debug`, `trace`.

## Security Considerations

This project involves continuous audio recording, which has significant privacy implications. Please ensure you:

- Use this software only for personal use on your own devices.
- Inform anyone in the vicinity that audio is being recorded.
- Securely store and manage any saved audio data.
- Do not use this software in jurisdictions where continuous audio recording may be illegal.

## Future Improvements

- [ ] Long-term audio persistence
  - [x] Every `buffer_duration` seconds, store the audio buffer to disk
  - [x] Efficient compression with Opus at 32kbps
  - [x] s3 persistence
  - [ ] Silence removal? Better compression? Switch to cheaper s3 storage?
        *Note: the returns on further compression are very small, and storage costs are absurdly low.*
  - [ ] Remove +1d old local files.
- [ ] Transcription with whisperx
- [ ] Audio analysis (e.g., live note detection)
- [ ] Websocket support for real-time audio streaming
- [ ] Keypress logging integration
- [ ] Occasional screenshots (?)

## Performance investigations

- Fast-write (with unsafe) is 10x faster than write.
- 7ms to copy full buffer, 250ms to return the `.wav` file containing that buffer. How to optimize this?

## Acknowledgments

- [CPAL](https://github.com/RustAudio/cpal) for audio capture
- [Axum](https://github.com/tokio-rs/axum) for the web framework
- [Tokio](https://github.com/tokio-rs/tokio) for async runtime
- [Plotters](https://github.com/plotters-rs/plotters) for audio visualization
- Claude Sonnet 3.5 for help with Rust syntax.
