# rmp3
Idiomatic bindings to `minimp3` that don't allocate.

## Usage
```rust
let mut decoder = rmp3::Decoder::new(&your_data);
while let Some(rmp3::Frame { bitrate, channels, sample_rate, samples, .. }) = decoder.next_frame() {
    /* process frame data */
}
```

## Features
- `float` - Output 32-bit float PCM instead of signed 16-bit integers
- `no-simd` - Disable all manual SIMD optimizations
- `only-mp3` (default) - Strip MP1/MP2 decoding logic
