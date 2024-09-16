# Lifelogging

![GitHub release (latest by date)](https://img.shields.io/github/v/release/henri123lemoine/life-logging)
![GitHub](https://img.shields.io/github/license/henri123lemoine/life-logging)

Lifelogging is a Rust-based project for [lifelogging](https://en.wikipedia.org/wiki/Lifelog). It runs a low-memory server that makes it easy for other projects on your machine to access audio data, transcriptions, keypresses, and more. This project is meant only for personal use.

## Features

Audio:

- Continuous audio recording.
- Circular buffer to store the most recent audio data.
- API to retrieve audio data as PCM/WAV/FLAC files.
- Configurable buffer size and sample rate.

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
      ```bash
      chmod +x run_background.sh  # Make the script executable (first time only)
      ./run_background.sh
      ```
      This will start the server in the background and log output to `logs/output.log`. You can run `tail -f logs/output.log` to view the logs in real time.
      Closing: Run `ps aux | grep life-logging` to find the process ID, then run `kill <PID>` to kill the process.

The server will start on `http://127.0.0.1:61429`, or whichever port is chosen in your configuration.

## Usage

- **Get Audio**: `GET /get_audio` - Returns the most recent audio data. Query parameters:
  - `format`: The audio format to return. Supported formats are `pcm`, `wav`, and `flac`. Default is `wav`.
- **Visualize Audio**: `GET /visualize_audio` - Returns a PNG image visualizing the recent audio data. Useful for debugging.
- **Health Check**: `GET /health` - Returns the server's health status.

### Examples

Retrieve the last minute of audio in WAV format:
```bash
curl "http://127.0.0.1:61429/get_audio?format=wav" --output recent_audio.wav
```

Visualize recent audio data: Go to `http://127.0.0.1:61429/visualize_audio` in your browser.

### Configuration

The application uses a flexible configuration system that supports both file-based configuration and environment variable overrides.

### Configuration File

The default configuration is set in config/default.toml. This file should contain your base configuration:

```toml
buffer_duration = 60

[server]
host = "127.0.0.1"
port = 61429
# Optional: Set a specific audio device
# selected_device = "Default"
```

To override the default configuration, add a `.toml` file to the `config` directory with your preferred settings. The server will automatically load the configuration from this file.

### Environment Variables

You can override any file-configured value using environment variables. The format is:
```bash
LIFELOGGING__<SECTION>__<KEY>=<VALUE>
```
For example:

- To change the buffer duration: LIFELOGGING__BUFFER_DURATION=120
- To change the server port: LIFELOGGING__SERVER__PORT=8080

### Logging

Set the `RUST_LOG` environment variable to `info` or `debug` to see more detailed logs. Valid log levels are: error, warn, info, debug, trace.

### Configuration Reloading

The application supports hot-reloading of the configuration. You can trigger a configuration reload by sending a POST request to the /reload_config endpoint:
```bash
curl -X POST http://127.0.0.1:61429/reload_config
```
This will re-read the configuration file and apply any changes made to it or to the environment variables.

## Security Considerations

This project involves continuous audio recording, which has significant privacy implications. Please ensure you:

- Use this software only for personal use on your own devices.
- Inform anyone in the vicinity that audio is being recorded.
- Securely store and manage any saved audio data.
- Do not use this software in jurisdictions where continuous audio recording may be illegal.

## Future Improvements

- [ ] Select between available audio devices
  - [x] When some audio device is disconnected, automatically switch to another available device
  - [ ] Handle multiple simultaneous devices (?)
- [ ] Websocket support for real-time audio streaming
- [ ] Long-term audio persistence with s3
  - [ ] Efficient compression with Opus at 32kbps and silence removal
- [ ] Transcription with whisperx
- [ ] Audio analysis (e.g., live note detection)
- [ ] Keypress logging integration
- [ ] Occasional screenshots (?)

## Acknowledgments

- [CPAL](https://github.com/RustAudio/cpal) for audio capture
- [Axum](https://github.com/tokio-rs/axum) for the web framework
- [Tokio](https://github.com/tokio-rs/tokio) for async runtime
- [Plotters](https://github.com/plotters-rs/plotters) for audio visualization
- Claude Sonnet 3.5 for help with Rust syntax.
