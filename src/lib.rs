#![cfg_attr(not(feature = "std"), no_std)]

#[doc(hidden)]
pub mod ffi;

use core::{marker::PhantomData, mem::MaybeUninit, num::NonZeroUsize, ptr};
use libc::c_int;

// The minimp3 API takes `int` for size, however that won't work if
// your file exceeds 2GB (usually 2^31-1 bytes) in size. Thankfully,
// under pretty much no circumstances will each frame be >2GB.
// Even if it would be, this makes it not UB and just return err/eof.
#[inline(always)]
fn data_len_safe(len: usize) -> c_int {
    len.min(c_int::max_value() as usize) as c_int
}

/// Returns the source slice from a received `mp3dec_frame_info_t`.
#[inline(always)]
unsafe fn source_slice<'src, 'frame>(
    data: &'src [u8],
    frame_recv: &'frame ffi::mp3dec_frame_info_t,
) -> &'src [u8] {
    data.get_unchecked(frame_recv.frame_offset as usize..frame_recv.frame_bytes as usize)
}

// Note: This is redefined because rustdoc is annoying, and will output:
// `pub const ... = ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize // 2304`
//
// There's a cargo test in case this is adjusted in the in the future.
/// Maximum amount of samples that can be yielded per frame.
pub const MAX_SAMPLES_PER_FRAME: usize = 0x900;

/// Describes audio samples in a frame.
pub struct Audio<'src, 'pcm> {
    // entire result from minimp3 as-is
    info: ffi::mp3dec_frame_info_t,

    // pcm data, if any
    pcm: Option<ptr::NonNull<Sample>>, // of lifetime 'pcm
    sample_count: usize,

    // source slice (without garbage)
    source: &'src [u8],

    // 👻
    phantom: PhantomData<&'pcm [Sample]>,
}

// Safety: The lifetimes do it for us.
unsafe impl<'src, 'pcm> Send for Audio<'src, 'pcm> {}
unsafe impl<'src, 'pcm> Sync for Audio<'src, 'pcm> {}

/// Describes a frame, which contains audio samples or other data.
pub enum Frame<'src, 'pcm> {
    /// PCM Audio
    Audio(Audio<'src, 'pcm>),

    /// ID3 or other unknown data
    Other(&'src [u8]),
}

/// High-level streaming iterator for parsing or decoding MPEG Audio data.
pub struct Decoder<'src> {
    cached_peek_len: Option<NonZeroUsize>,
    pcm: MaybeUninit<[Sample; MAX_SAMPLES_PER_FRAME]>,
    raw: RawDecoder,
    source: &'src [u8],
    source_copy: &'src [u8],
}

/// Low-level decoder for parsing or decoding MPEG Audio data.
///
/// If you can load the entire file in advance, [`Decoder`] (which is a wrapper around this struct) will be more convenient.
///
/// The second tuple field on the [`next`](Self::next) and [`peek`](Self::peek)
/// functions indicate how many bytes the decoder consumed.
pub struct RawDecoder(MaybeUninit<ffi::mp3dec_t>);

/// Conditional type used to represent one PCM sample in output data.
///
/// Normally a signed 16-bit integer (i16), but if the *"float"* feature is enabled,
/// it's a 32-bit single precision float (f32).
#[cfg(not(feature = "float"))]
pub type Sample = i16;
#[cfg(feature = "float")]
pub type Sample = f32;

impl<'src> Decoder<'src> {
    /// Constructs a new `Decoder` for processing MPEG Audio.
    pub fn new(source: &'src [u8]) -> Self {
        Self {
            cached_peek_len: None,
            pcm: MaybeUninit::uninit(),
            raw: RawDecoder::new(),
            source,
            source_copy: source,
        }
    }

    /// Reads the next frame, skipping over potential garbage data.
    pub fn next<'pcm>(
        &'pcm mut self,
    ) -> Option<Frame<'src, 'pcm>> {
        self.cached_peek_len = None; // clear cache
        unsafe {
            let (frame, len) = self.raw.next(self.source, &mut *self.pcm.as_mut_ptr())?;
            self.offset_trusted(len);
            Some(frame)
        }
    }

    /// Reads the next frame without decoding it, or advancing the decoder.
    /// Use [`skip`](Self::skip) to advance.
    ///
    /// This means that the samples will always be empty in [`Audio`],
    /// and [`sample_count`](Audio::sample_count) should be used to inspect the length.
    pub fn peek(&mut self) -> Option<Frame<'src, 'static>> {
        let (frame, len) = self.raw.peek(self.source)?;
        self.cached_peek_len = NonZeroUsize::new(len);
        Some(frame)
    }

    /// Gets the current position in the input data, starting from 0.
    #[inline]
    pub fn position(&self) -> usize {
        unsafe { self.source.as_ptr().sub(self.source_copy.as_ptr() as usize) as usize }
    }

    /// Sets the current position in the input data.
    ///
    /// If `position` is out of bounds, it's set to the end of the data instead.
    #[inline]
    pub fn set_position(&mut self, position: usize) {
        let position = self.source_copy.len().min(position);
        self.source = unsafe { self.source_copy.get_unchecked(position..) };
        self.cached_peek_len = None;
    }

    /// Skips the current frame the decoder is over, if any.
    pub fn skip(&mut self) -> Option<()> {
        unsafe {
            let offset = match self.cached_peek_len.take() {
                Some(offset) => offset.get(),
                None => self.raw.peek(self.source)?.1,
            };
            self.offset_trusted(offset);
        }
        Some(())
    }

    #[inline]
    unsafe fn offset_trusted(&mut self, offset: usize) {
        self.source = self.source.get_unchecked(offset..);
    }
}

impl RawDecoder {
    /// Constructs a new `RawDecoder` for processing MPEG Audio.
    pub fn new() -> Self {
        let mut decoder = MaybeUninit::uninit();
        unsafe {
            ffi::mp3dec_init(decoder.as_mut_ptr());
        }
        Self(decoder)
    }

    /// Reads the next frame, skipping over potential garbage data.
    ///
    /// If the frame contains audio data, [`samples`](Audio::samples) should be used
    /// to get the slice, as not all of the `dest` slice may be filled up.
    #[inline]
    pub fn next<'src, 'pcm>(
        &mut self,
        src: &'src [u8],
        dest: &'pcm mut [Sample; MAX_SAMPLES_PER_FRAME],
    ) -> Option<(Frame<'src, 'pcm>, usize)> {
        self.call(src, Some(dest))
    }

    /// Reads the next frame without decoding it.
    ///
    /// This means that the samples will always be empty in [`Audio`],
    /// and [`sample_count`](Audio::sample_count) should be used to inspect the length.
    #[inline]
    pub fn peek<'src>(
        &mut self,
        src: &'src [u8],
    ) -> Option<(Frame<'src, 'static>, usize)> {
        self.call(src, None)
    }

    fn call<'src, 'pcm>(
        &mut self,
        src: &'src [u8],
        dest: Option<&'pcm mut [Sample; MAX_SAMPLES_PER_FRAME]>,
    ) -> Option<(Frame<'src, 'pcm>, usize)> {
        let src_length = data_len_safe(src.len());
        let dest_ptr: *mut Sample = dest.map_or(ptr::null_mut(), |x| x).cast();
        unsafe {
            let mut info = MaybeUninit::uninit().assume_init();
            let result = ffi::mp3dec_decode_frame(
                self.0.as_mut_ptr(),
                src.as_ptr(),
                src_length,
                dest_ptr,
                &mut info,
            );
            let skip = info.frame_bytes as usize;

            if result != 0 {
                Some((
                    Frame::Audio(Audio {
                        info,
                        pcm: ptr::NonNull::new(dest_ptr),
                        sample_count: result as usize,
                        source: source_slice(src, &info),
                        phantom: PhantomData,
                    }),
                    skip,
                ))
            } else if info.frame_bytes != 0 {
                Some((Frame::Other(source_slice(src, &info)), skip))
            } else {
                None
            }
        }
    }
}

impl<'src, 'pcm> Audio<'src, 'pcm> {
    /// Gets the bitrate of this frame in kb/s.
    #[inline]
    pub fn bitrate(&self) -> u32 {
        self.info.bitrate_kbps as u32
    }

    /// Gets the channel count of this frame.
    #[inline]
    pub fn channels(&self) -> u16 {
        // TODO: This has to be safe, right? Pretty sure there's a table.
        self.info.channels as u16
    }

    /// Gets the MPEG layer of this frame.
    #[inline]
    pub fn mpeg_layer(&self) -> u8 {
        // TODO: Same as above!
        self.info.layer as u8
    }

    /// Gets the sample rate of this frame in Hz.
    #[inline]
    pub fn sample_rate(&self) -> u32 {
        self.info.hz as u32
    }

    /// Gets the slice of samples in this frame.
    /// Samples are interleaved, so the length is
    /// [`channels`](Self::channels) \* [`sample_count`](Self::sample_count).
    ///
    /// Do not use this to inspect the number of samples, as
    /// if this frame was `peek`ed, an empty slice will be given.
    #[inline]
    pub fn samples(&self) -> &'pcm [Sample] {
        match self.pcm {
            Some(ptr) => unsafe {
                (&*ptr.cast::<[Sample; MAX_SAMPLES_PER_FRAME]>().as_ptr())
                    .get_unchecked(..self.sample_count * self.info.channels as usize)
            },
            None => &[],
        }
    }

    /// Gets the sample count per [`channel`](Self::channels).
    #[inline]
    pub fn sample_count(&self) -> usize {
        self.sample_count
    }

    /// Gets the source slice with potential garbage stripped.
    #[inline]
    pub fn source(&self) -> &'src [u8] {
        self.source
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanity() {
        // See the comment on `crate::MAX_SAMPLES_PER_FRAME`
        assert_eq!(
            crate::MAX_SAMPLES_PER_FRAME,
            crate::ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize,
        );
    }
}
