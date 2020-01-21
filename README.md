[![Build Status (Travis-CI)](https://travis-ci.com/notviri/rmp3.svg?branch=master)](https://travis-ci.com/notviri/rmp3)
[![Crates.io](https://img.shields.io/crates/v/rmp3)](https://crates.io/crates/rmp3)
[![Documentation](https://docs.rs/rmp3/badge.svg)](https://docs.rs/rmp3)

# rmp3
Idiomatic `no_std` bindings to [minimp3](https://github.com/lieff/minimp3) which don't allocate.

## Usage
```toml
# Cargo.toml
[dependencies]
rmp3 = "0.2"
```
A simple forward streaming iterator is provided for decoding samples.

```rust
use rmp3::{Decoder, Frame};

// It returns a reference to the internal fixed buffer along with the frame info:
let mut decoder = Decoder::new(&your_data_here);
while let Some(Frame { channels, sample_rate, samples, .. }) = decoder.next_frame() {
    // * process frame data here *
}

// Sometimes you just want to iterate the frames without decoding them, as it's much faster.
// Example to calculate song length - 800µs vs. 350ms when decoding a 4:52 track (on a low-end CPU)
let mut decoder = Decoder::new(&your_data_here);
let mut length = 0.0f32; // length in seconds
while let Some(Frame { sample_rate, sample_count, .. }) = decoder.peek_frame() {
    // Not all frames necessarily contain samples (next_frame would skip over these).
    if sample_count != 0 {
        length += sample_count as f32 / sample_rate as f32;
    }
    decoder.skip_frame();
}
```

## Features
- `float` - Output 32-bit float PCM instead of signed 16-bit integers
- `no-simd` - Disable all manual SIMD optimizations
- `only-mp3` (default) - Strip MP1/MP2 decoding logic
