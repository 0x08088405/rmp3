# rmp3
Idiomatic `no_std` bindings to [minimp3](https://github.com/lieff/minimp3) which don't allocate.

## Usage
A simple streaming iterator is provided for decoding samples.
```rust
let mut decoder = rmp3::Decoder::new(&your_data_here); // anything that can AsRef<[u8]>
```
It returns a reference to the internal fixed buffer along with the frame info:
```rust
while let Some(rmp3::Frame { channels, sample_rate, samples, .. }) = decoder.next_frame() {
    // process frame data
}
```

Sometimes you just want to iterate the frames without decoding them (as it's much faster):
```rust
// calculate song length - 800µs vs. 350ms when decoding a 4:52 track (on a low end CPU)
let mut length = 0.0f32;
while let Some(rmp3::Frame { sample_rate, sample_count, source_len, .. }) = decoder.peek_frame() {
    if sample_count != 0 {
        length += sample_count as f32 / sample_rate as f32;
    }
    decoder.skip_frame(source_len);
}
println!("Length: {}:{}", (length / 60.0) as u32, (length % 60.0) as u32);
```
## Features
- `float` - Output 32-bit float PCM instead of signed 16-bit integers
- `no-simd` - Disable all manual SIMD optimizations
- `only-mp3` (default) - Strip MP1/MP2 decoding logic
