[![Build Status (Travis-CI)](https://travis-ci.com/notviri/rmp3.svg?branch=trunk)](https://travis-ci.com/notviri/rmp3)
[![Crates.io](https://img.shields.io/crates/v/rmp3)](https://crates.io/crates/rmp3)
[![Documentation](https://docs.rs/rmp3/badge.svg)](https://docs.rs/rmp3)

# rmp3
Idiomatic `no_std` bindings to [minimp3](https://github.com/lieff/minimp3) which don't allocate.

## Documentation

The documentation is hosted online over [at docs.rs](https://docs.rs/rmp3/).

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rmp3 = "0.3"
```

... or, if you need `std` specific features:
```toml
[dependencies]
rmp3 = { features = ["std"], version = "0.3" }
```

The most basic example is using the provided streaming iterator to decode a file, like so:

```rust
use rmp3::{Decoder, Frame};

let mp3 = std::fs::read("test.mp3")?;
let mut decoder = Decoder::new(&mp3);
while let Some(frame) = decoder.next() {
    if let Frame::Audio(audio) = frame {
        // process audio frame here!
        imaginary_player.append(
            audio.channels(),
            audio.sample_count(),
            audio.sample_rate(),
            audio.samples(),
        );
    }
}
```

Check out the [documentation](#Documentation) for more examples and info.

## Features
- `float`: Changes the sample type to a single-precision float,
and thus decoders will output float PCM.
    - **This is a non-additive feature and will change API.**
    **Do not do this in a library without notice [(why?)](
https://github.com/rust-lang/cargo/issues/4328#issuecomment-652075026).**
- `mp1-mp2`: Includes MP1 and MP2 decoding code.
- `simd` *(default)*: Enables handwritten SIMD optimizations on eligible targets.
- `std`: Adds things that require `std`,
