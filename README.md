# Life-Logging

Life-Logging is a Rust-based project for [life logging](https://en.wikipedia.org/wiki/Lifelog). The objective is to be a low-memory project that makes it easy for other projects on your machine to access audio data, transcription, keypresses, and more. This project is meant only for personal use.

## Features

Audio:

- Continuous audio recording.
- Circular buffer to store the most recent audio data.
- API to retrieve audio data as PCM/WAV/FLAC files.
- Configurable buffer size and sample rate.

Transcription:

- TODO

Keypress:

- TODO

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

2. Run the server:
   ```bash
   cargo run
   ```

The server will start on `http://127.0.0.1:61429`, or whatever port you chose in your configuration.

## Usage

- **Get Audio**: `GET /get_audio` - Returns the most recent audio data as a WAV file. `/get_audio` takes the following query parameters:
  - `format`: The audio format to return. Supported formats are `pcm`, `wav`, and `flac`. Default is `wav`.
  <!-- - `duration`: The duration of audio to return in seconds. Default is 10 seconds. -->
- **Visualize Audio**: `GET /visualize_audio` - Returns a PNG image visualizing the recent audio data.
- **Health Check**: `GET /health` - Returns the server's health status.

## Configuration

Current configuration is set in `config/default.toml`:

- `sample_rate`: 48000 Hz
- `buffer_duration`: 60 seconds
- `[server] host`: "127.0.0.1"
- `[server] port`: 61429

## Future Improvements

Since this is just a server that provides information to other projects, many features can be added. The following come to mind:

- Streaming audio data
- Audio analysis
- Transcription
- Keypress logging

More detailedly,

High priotity:

- Select between available audio devices, no need for configuring sampling rate if this can be determined from device. (Handle multiple simulatneous devices?)
- Websocket support for real-time audio streaming
- Long-term storage and retrieval of audio data (e.g. save audio data to disk every minute, and persist in s3 once a day)
- Transcription from audio data. E.g.: Every 30(?) seconds, get audio from last minute, transcribe it, and combine to previous previous transcriptions with diff(?) algorithm to make a full-time transcription. In fact, estimate the memory/RAM cost of having continuous live transcription, just to verify that this is feasible.
- If someone says something insightful, simple keybind saves the audio+transcribed quote in a quotes folder.

Low priority:

- Audio analysis. E.g.: Detect live note (for whistling/singing practice), clean up audio, etc.
- Additional audio format support (MP3, OGG)

## Acknowledgments

- [CPAL](https://github.com/RustAudio/cpal) for audio capture
- [Axum](https://github.com/tokio-rs/axum) for the web framework
- [Tokio](https://github.com/tokio-rs/tokio) for async runtime
- [Plotters](https://github.com/plotters-rs/plotters) for audio visualization
