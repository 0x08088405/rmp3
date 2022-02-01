#![warn(missing_docs)]

#![no_std]

// TODO: should the members here be pub(crate)? hope that won't need sed
mod ffi;

use core::{fmt, marker::PhantomData, mem::MaybeUninit, num::NonZeroU16, ptr, slice};
use chlorine::c_int;

/// Maximum number of samples per frame.
pub const MAX_SAMPLES: usize = 1152 * 2;

/// Describes a frame, which may contain audio samples.
pub enum Frame<'src, 'pcm> {
    /// A frame containing PCM data.
    Audio(Audio<'src, 'pcm>),

    /// A frame containing miscellaneous data.
    Other(&'src [u8]),
}

#[derive(Clone)]
pub struct Audio<'src, 'pcm> {
    bitrate: u16,
    channels: u8,
    mpeg_layer: u8,
    sample_count: NonZeroU16,
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
    /// Interval: x ∈ [8, 448]
    #[inline]
    pub fn bitrate(&self) -> u16 {
        // TODO check what happens with the reserved bitrates
        self.bitrate
    }

    /// Gets how many channels are in this frame.
    #[inline]
    pub fn channels(&self) -> NonZeroU16 {
        unsafe { NonZeroU16::new_unchecked(self.channels as u16) }
    }

    /// Gets the MPEG layer of this frame.
    #[inline]
    pub fn mpeg_layer(&self) -> u8 {
        // TODO check what happens when the illegal 0b00 layer is passed
        self.mpeg_layer as u8
    }

    /// Gets the number of samples in this frame per [channel](Self::channels).
    #[inline]
    pub fn sample_count(&self) -> NonZeroU16 {
        self.sample_count
    }

    /// Gets the sample rate of this frame in Hz.
    #[inline]
    pub fn sample_rate(&self) -> NonZeroU16 {
        // TODO what happens with the DIY ones?
        unsafe { NonZeroU16::new_unchecked(self.sample_rate as u16) }
    }

    /// Gets the slice of decoded samples.
    ///
    /// Samples are interleaved, so this has the length of
    /// [`channels`](Self::channels) * [`sample_count`](Self::sample_count),
    /// to a maximum of [`MAX_SAMPLES`](crate::MAX_SAMPLES).
    ///
    /// If the samples weren't decoded, this is an empty slice.
    #[inline]
    pub fn samples(&self) -> &'pcm [f32] {
        if let Some(buf) = self.pcm {
            unsafe { slice::from_raw_parts(buf.as_ptr(), usize::from(self.sample_count.get() * self.channels as u16)) }
        } else {
            &[]
        }
    }

    /// Gets the slice of the source which contains the entire frame.
    ///
    /// Leading garbage is omitted from the slice.
    #[inline]
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

/// Primitive stateless decoder for parsing and/or decoding MPEG Audio.
///
/// # Examples
///
/// Simple example decoding frames into a big `Vec` of all the samples.
///
/// ```no_run
/// # fn main() {
/// use empy::{Decoder, Frame};
///
/// let mut data: &[u8] = &[/* your file here */];
///
/// let mut decoder = Decoder::new();
/// let mut buffer = [0.0; empy::MAX_SAMPLES];
/// let mut pcm: Vec<f32> = Vec::with_capacity(1024 * 1024 * 32);
///
/// while let Some((frame, bytes_consumed)) = decoder.decode(data, Some(&mut buffer)) {
///     match frame {
///         Frame::Audio(audio) => {
///             // note that you'd want to keep track of bitrate, channels, sample_rate
///             // they can differ between adjacent frames (especially bitrate for VBR)
///             pcm.extend_from_slice(audio.samples());
///         },
///         Frame::Other(_) => (/* don't care */),
///     }
///     data = &data[bytes_consumed..];
/// }
/// # }
/// ```
///
/// You don't need to decode the samples if it's not necessary.
/// Example computing length in minutes and seconds:
///
/// ```no_run
/// # fn main() {
/// use empy::{Decoder, Frame};
///
/// let mut data: &[u8] = &[/* your file here */];
/// let mut decoder = Decoder::new();
/// let mut length = 0.0f64;
///
/// while let Some((frame, bytes_consumed)) = decoder.decode(data, None) {
///     if let Frame::Audio(audio) = frame {
///         // note here that sample_count is *per channel* so it works out
///         length += f64::from(audio.sample_count().get()) / f64::from(audio.sample_rate().get());
///     }
///     data = &data[bytes_consumed..];
/// }
///
/// println!("Length: {:.0}m{:.0}s", length / 60.0, length % 60.0);
/// # }
/// ```
pub struct Decoder(MaybeUninit<ffi::mp3dec_t>);

impl Decoder {
    /// Initialises a new [`Decoder`].
    #[inline]
    pub fn new() -> Self {
        let mut state = MaybeUninit::uninit();
        unsafe { ffi::mp3dec_init(state.as_mut_ptr()) };
        Self(state)
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
            let mut info_recv = MaybeUninit::uninit();
            let sample_count = ffi::mp3dec_decode_frame(
                state.as_mut_ptr(),
                src.as_ptr(),
                src_c_len,
                dest_ptr,
                info_recv.as_mut_ptr(),
            );
            let info = &*info_recv.as_ptr();

            if sample_count != 0 {
                let nz_u16 = NonZeroU16::new_unchecked;
                let audio = Audio {
                    bitrate: info.bitrate_kbps as u16,         // x ∈ [8, 448]
                    channels: info.channels as u8,             // x ∈ {1, 2}
                    mpeg_layer: info.layer as u8,              // x ∈ {1, 2, 3}
                    sample_count: nz_u16(sample_count as u16), // x ∈ (0, MAX_SAMPLES]
                    sample_rate: info.hz as u16,               // x ∈ [8000, 44100]

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
