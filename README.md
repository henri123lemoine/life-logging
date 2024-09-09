# life-logging

life-logging: A Rust-based continuous audio recording service

## Plan

One thread continuously saves audio to a ring buffer, another serves the past n seconds of audio data as a .wav file.

Issues:

- Extracting audio data resets the buffer for some reason
- The buffer gets full, instead of being overwritten with new data
- Allow easier customization of buffer duration, sample rate, etc.

## Links

- https://github.com/RustAudio/cpal
