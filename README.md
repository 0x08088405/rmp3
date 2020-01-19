# rmp3
Idiomatic bindings to `minimp3` that don't allocate.

## Usage
```rust
fn main() {
    blah();
}
```

## Features
- `float` - Output 32-bit float PCM instead of signed 16-bit integers
- `no-simd` - Disable all manual SIMD optimizations
- `only-mp3` (default) - Strip MP1/MP2 decoding logic
