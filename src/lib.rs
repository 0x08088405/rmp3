//! Idiomatic `no_std` bindings to lieff's [minimp3](https://github.com/lieff/minimp3).
//!
//! # Features
//!
//! - `mp1-mp2`: Includes MP1 and MP2 decoding code.
//! - `simd` *(default)*: Enables handwritten SIMD optimizations on eligible targets.
//!
//! # Example
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn ::std::error::Error>> {
//! use empy::{DecoderStream, Frame};
//!
//! let mp3 = std::fs::read("test.mp3")?;
//! let mut decoder = DecoderStream::new(&mp3);
//!
//! while let Some(frame) = decoder.next() {
//!     // *process frame here*
//! }
//! # Ok(())
//! # }
//! ```
//!
//! See documentation for [`DecoderStream`] and its lower-level cousin [`Decoder`] for more info.

#![deny(missing_docs)]
#![no_std]

// TODO: should the members here be pub(crate)? hope that won't need sed
mod ffi;

use core::{fmt, marker::PhantomData, mem, num, ptr, slice};
use chlorine::c_int;

/// Maximum number of samples per frame.
pub const MAX_SAMPLES: usize = 1152 * 2;

/// Describes a frame that contains audio or other (unknown) data.
pub enum Frame<'src, 'pcm> {
    /// A frame containing PCM data.
    Audio(Audio<'src, 'pcm>),

    /// A frame containing miscellaneous data.
    Other(&'src [u8]),
}

/// Describes audio samples in a frame.
#[derive(Clone)]
pub struct Audio<'src, 'pcm> {
    bitrate: u16,
    channels: u8,
    mpeg_layer: u8,
    sample_count: u16,
    sample_rate: u16,

    src: &'src [u8],
    pcm: Option<ptr::NonNull<f32>>,

    // 👻
    phantom: PhantomData<&'pcm [f32]>,
}
unsafe impl<'src, 'pcm> Send for Audio<'src, 'pcm> {}
unsafe impl<'src, 'pcm> Sync for Audio<'src, 'pcm> {}

impl<'src, 'pcm> Audio<'src, 'pcm> {
    /// Gets the bitrate of this frame in kb/s.
    ///
    /// Possible values are in the interval [8, 448].
    pub fn bitrate(&self) -> u16 {
        // TODO check what happens with the reserved bitrates
        self.bitrate
    }

    /// Gets how many channels are in this frame.
    ///
    /// Possible values are one of {1, 2}.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Gets the MPEG layer of this frame.
    ///
    /// Possible values are one of {1, 2, 3}.
    pub fn mpeg_layer(&self) -> u8 {
        // TODO check what happens when the illegal 0b00 layer is passed
        self.mpeg_layer
    }

    /// Gets the number of samples in this frame per [channel](Self::channels).
    ///
    /// Possible values are in the interval (0, [`MAX_SAMPLES`]].
    pub fn sample_count(&self) -> u16 {
        self.sample_count
    }

    /// Gets the sample rate of this frame in Hz.
    ///
    /// Possible values are in the interval [8000, 44100].
    pub fn sample_rate(&self) -> u16 {
        // TODO what happens with the DIY ones?
        self.sample_rate
    }

    /// Gets the slice of decoded samples.
    ///
    /// If the samples weren't decoded, this is an empty slice.
    ///
    /// Channels are interleaved, so this has the length of
    /// [`channels`](Self::channels) * [`sample_count`](Self::sample_count),
    /// to a maximum of [`MAX_SAMPLES`](crate::MAX_SAMPLES).
    #[inline]
    pub fn samples(&self) -> &'pcm [f32] {
        if let Some(buf) = self.pcm {
            unsafe { slice::from_raw_parts(buf.as_ptr(), usize::from(self.sample_count * self.channels as u16)) }
        } else {
            &[]
        }
    }

    /// Gets the slice of the source which contains the entire frame.
    ///
    /// Leading garbage is omitted from the slice.
    pub fn source(&self) -> &'src [u8] {
        self.src
    }
}

impl fmt::Debug for Frame<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio(audio) => f.debug_tuple("Audio").field(audio).finish(),
            Self::Other(_) => f.debug_tuple("Other").field(&format_args!("&[...]")).finish(),
        }
    }
}

impl fmt::Debug for Audio<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Audio")
            .field("bitrate", &self.bitrate)
            .field("channels", &self.channels)
            .field("mpeg_layer", &self.mpeg_layer)
            .field("sample_count", &self.sample_count)
            .field("sample_rate", &self.sample_rate)
            .field("samples", {
                &if self.pcm.is_some() {
                    format_args!("&[...]")
                } else {
                    format_args!("&[not decoded]")
                }
            })
            .finish()
    }
}

/// Low-level stateless decoder for parsing and/or decoding MPEG Audio.
///
/// The struct itself holds the memory (6.5KiB) for the decoding process.
///
/// # Examples
///
/// Simple example decoding frames into a big `Vec` of all the samples.
///
/// ```
/// # fn main() {
/// use empy::{Decoder, Frame};
///
/// let mut data: &[u8] = &[/* your file here */];
///
/// let mut decoder = Decoder::new();
/// let mut buffer = [0.0; empy::MAX_SAMPLES];
/// let mut pcm: Vec<f32> = Vec::with_capacity(1024 * 1024 * 32);
///
/// while let Some((frame, bytes_read)) = decoder.decode(data, Some(&mut buffer)) {
///     match frame {
///         Frame::Audio(audio) => {
///             // note that you'd want to keep track of bitrate, channels, sample_rate
///             // they can differ between adjacent frames (especially bitrate for VBR)
///             pcm.extend_from_slice(audio.samples());
///         },
///         Frame::Other(_) => (/* don't care */),
///     }
///     data = &data[bytes_read..];
/// }
/// # }
/// ```
///
/// You don't need to decode the samples if it's not necessary.
/// Example computing length in minutes and seconds:
///
/// ```
/// # fn main() {
/// use empy::{Decoder, Frame};
///
/// let mut data: &[u8] = &[/* your file here */];
/// let mut decoder = Decoder::new();
/// let mut length = 0.0f64;
///
/// while let Some((frame, bytes_read)) = decoder.decode(data, None) {
///     if let Frame::Audio(audio) = frame {
///         // note here that sample_count is *per channel* so it works out
///         length += f64::from(audio.sample_count()) / f64::from(audio.sample_rate());
///     }
///     data = &data[bytes_read..];
/// }
///
/// println!("Length: {:.0}m{:.0}s", length / 60.0, length % 60.0);
/// # }
/// ```
pub struct Decoder(mem::MaybeUninit<ffi::mp3dec_t>);

impl Decoder {
    /// Initialises a new [`Decoder`].
    pub const fn new() -> Self {
        Self(mem::MaybeUninit::uninit())
    }

    /// Tries to find and decode a frame in `src`.
    ///
    /// Decoding the samples will be skipped if `dest` is [`None`](Option::None).
    ///
    /// If there's some garbage present before the framesync, it will be skipped.
    ///
    /// On success, returns information about the [`Frame`],
    /// and how many bytes it read total (including garbage, if any).
    pub fn decode<'src, 'pcm>(
        &mut self,
        src: &'src [u8],
        dest: Option<&'pcm mut [f32; MAX_SAMPLES]>,
    ) -> Option<(Frame<'src, 'pcm>, usize)> {
        let Self(state) = self;

        let src_c_len = src.len().min(c_int::max_value() as usize) as c_int;
        let dest_ptr: *mut f32 = dest.map_or(ptr::null_mut(), |x| x).cast();
        unsafe {
            // this is really cheap, it literally sets one integer
            // moving this here allows new() to be const fn
            ffi::mp3dec_init(state.as_mut_ptr());

            let mut info_recv = mem::MaybeUninit::uninit();
            let sample_count = ffi::mp3dec_decode_frame(
                state.as_mut_ptr(),
                src.as_ptr(),
                src_c_len,
                dest_ptr,
                info_recv.as_mut_ptr(),
            );
            let info = &*info_recv.as_ptr();

            if sample_count != 0 {
                let audio = Audio {
                    bitrate: info.bitrate_kbps as u16, // x ∈ [8, 448]
                    channels: info.channels as u8,     // x ∈ {1, 2}
                    mpeg_layer: info.layer as u8,      // x ∈ {1, 2, 3}
                    sample_count: sample_count as u16, // x ∈ (0, MAX_SAMPLES]
                    sample_rate: info.hz as u16,       // x ∈ [8000, 44100]

                    src: frame_src(src, info),
                    pcm: ptr::NonNull::new(dest_ptr),

                    phantom: PhantomData,
                };
                Some((Frame::Audio(audio), info.frame_bytes as usize))
            } else if info.frame_bytes != 0 {
                Some((Frame::Other(frame_src(src, info)), info.frame_bytes as usize))
            } else {
                None
            }
        }
    }
}

#[inline]
unsafe fn frame_src<'src>(
    data: &'src [u8],
    info: &ffi::mp3dec_frame_info_t,
) -> &'src [u8] {
    data.get_unchecked(info.frame_offset as usize..info.frame_bytes as usize)
}

/// High-level streaming iterator for parsing and/or decoding MPEG Audio.
///
/// Convenience wrapper over [`Decoder`] to simplify general use
/// where the entire file is already loaded in memory.
///
/// # Examples
///
/// These examples are adapted from [`Decoder`] to show the conveniences of [`DecoderStream`].
///
/// Simple example decoding frames into a big `Vec` of all the samples:
///
/// ```
/// # fn main() {
/// use empy::{DecoderStream, Frame};
///
/// let mut decoder = DecoderStream::new(&[/* your file here */]);
/// let mut pcm: Vec<f32> = Vec::with_capacity(1024 * 1024 * 32);
///
/// while let Some(frame) = decoder.next() {
///     match frame {
///         Frame::Audio(audio) => {
///             // note that you'd want to keep track of bitrate, channels, sample_rate
///             // they can differ between adjacent frames (especially bitrate for VBR)
///             pcm.extend_from_slice(audio.samples());
///         },
///         Frame::Other(_) => (/* don't care */),
///     }
/// }
/// # }
/// ```
///
/// You don't need to decode the samples if it's not necessary.
/// Example computing length in minutes and seconds:
///
/// ```
/// # fn main() {
/// use empy::{DecoderStream, Frame};
///
/// let mut decoder = DecoderStream::new(&[/* your file here */]);
/// let mut length = 0.0f64;
///
/// while let Some(frame) = decoder.peek() {
///     if let Frame::Audio(audio) = frame {
///         // note here that sample_count is *per channel* so it works out
///         length += f64::from(audio.sample_count()) / f64::from(audio.sample_rate());
///     }
///     decoder.skip();
/// }
///
/// println!("Length: {:.0}m{:.0}s", length / 60.0, length % 60.0);
/// # }
/// ```
pub struct DecoderStream<'src> {
    decoder: Decoder,
    buffer: mem::MaybeUninit<[f32; MAX_SAMPLES]>,

    base: &'src [u8], // entire file
    view: &'src [u8], // offset to end

    cache: Option<num::NonZeroUsize>, // bytes until next frame
}

impl<'src> DecoderStream<'src> {
    /// Initialises a new [`DecoderStream`] over `src`.
    pub const fn new(src: &'src [u8]) -> Self {
        Self {
            decoder: Decoder::new(),
            buffer: mem::MaybeUninit::uninit(),
            base: src,
            view: src,
            cache: None,
        }
    }

    /// Decodes the next frame, skipping over potential garbage data.
    pub fn next<'pcm>(&'pcm mut self) -> Option<Frame<'src, 'pcm>> {
        self.cache = None;
        unsafe {
            let (frame, bytes_read) = self.decoder.decode(self.view, Some(&mut *self.buffer.as_mut_ptr()))?;
            self.view = self.view.get_unchecked(bytes_read..);
            Some(frame)
        }
    }

    /// Parses the next frame without decoding any samples or moving forward.
    ///
    /// To advance, use the [`skip`](Self::skip) function.
    pub fn peek(&mut self) -> Option<Frame<'src, 'static>> {
        let (frame, bytes_read) = self.decoder.decode(self.view, None)?;
        self.cache = num::NonZeroUsize::new(bytes_read);
        Some(frame)
    }

    /// Skips the current frame, moving on to the next.
    /// Avoids re-parsing after a previous call to [`peek`](Self::peek).
    ///
    /// If there was a frame to skip, returns how many bytes forward the [`DecoderStream`] advanced.
    pub fn skip(&mut self) -> Option<usize> {
        let bytes_to_skip = match self.cache.take() {
            Some(amount) => amount.get(),
            None => self.decoder.decode(self.view, None)?.1,
        };
        unsafe { self.view = self.view.get_unchecked(bytes_to_skip..) };
        Some(bytes_to_skip)
    }

    /// Returns the offset in the input data from the start (0).
    pub fn offset(&self) -> usize {
        let base = self.base.as_ptr() as usize;
        let view = self.view.as_ptr() as usize;
        view - base
    }

    /// Sets the offset in the input data from the beginning.
    ///
    /// If `offset` is out of bounds, returns the maximum valid offset.
    pub fn set_offset(&mut self, offset: usize) -> Result<(), usize> {
        self.view = self.base.get(offset..).ok_or(self.base.len())?;
        self.cache = None;
        Ok(())
    }
}

/// Highly optimised function for converting `f32` samples to `i16` samples.
///
/// # Panics
/// Panics if `f32pcm` and `i16pcm` have a different length.
pub fn f32_to_i16_pcm(f32pcm: &[f32], i16pcm: &mut [i16]) {
    assert_eq!(f32pcm.len(), i16pcm.len());

    // annoyingly, this API takes `c_int` like everything else so we have to get a bit creative
    assert!(c_int::max_value() as u128 <= usize::max_value() as u128);
    let mut remaining = f32pcm.len();
    loop {
        let batch_len = remaining.min(c_int::max_value() as usize);
        unsafe { ffi::mp3dec_f32_to_s16(f32pcm.as_ptr(), i16pcm.as_mut_ptr(), batch_len as c_int) };
        remaining -= batch_len;

        if remaining == 0 {
            break
        }
    }
}
