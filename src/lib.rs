#![no_std]

use core::{mem::MaybeUninit, ptr};
use libc::c_int;

/// Raw minimp3 bindings if you need them,
/// although if there's a desired feature please make an issue/PR.
#[allow(clippy::all, non_camel_case_types)]
#[path = "bindings.rs"]
pub mod ffi;

/// Conditional type used to represent one PCM sample in output data.
///
/// Normally a signed 16-bit integer (i16), but if the *"float"* feature is enabled,
/// it's a 32-bit single precision float (f32).
#[cfg(not(feature = "float"))]
pub type Sample = i16;
#[cfg(feature = "float")]
pub type Sample = f32;

/// Maximum amount of samples that can be yielded per frame.
pub const MAX_SAMPLES_PER_FRAME: usize = ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize;

pub enum Chunk<'src, 'pcm> {
    Samples(Frame<'src, 'pcm>),
    UnknownData(usize, &'src [u8]),
}

/// Primitive decoder for parsing or decoding MPEG Audio data.
pub struct Decoder {
    decoder: MaybeUninit<ffi::mp3dec_t>,
    frame_recv: MaybeUninit<ffi::mp3dec_frame_info_t>,
}

/// Accompanying buffer type for a [Decoder](struct.Decoder.html).
///
/// The inner data may be stale, and thus the only way to access it is
/// from the result slice given by [next](struct.Decoder.html#method.next).
#[repr(transparent)]
pub struct DecoderBuffer([Sample; MAX_SAMPLES_PER_FRAME]);

/// Info about the current frame yielded by a [Decoder](struct.Decoder.html).
pub struct Frame<'src, 'pcm> {
    /// Bitrate of the source frame in kb/s.
    pub bitrate: u32,
    /// Number of channels in this frame.
    pub channels: u32,
    /// MPEG layer of this frame.
    pub mpeg_layer: u32,
    /// Sample rate of this frame in Hz.
    pub sample_rate: u32,

    /// Total bytes consumed from the start of the input data.
    pub bytes_read: usize,
    /// Source bytes of the frame, including the header, excluding skipped (potential) garbage data.
    pub source: &'src [u8],
    /// Reference to the samples in this frame,
    /// contained in the output [DecoderBuffer](struct.DecoderBuffer.html).
    /// Empty if using [peek](struct.Decoder.html#method.peek).
    pub samples: &'pcm [Sample],
    /// Total sample count if using [peek](struct.Decoder.html#method.peek),
    /// since [samples](struct.Frame.html#structfield.samples) would be empty.
    pub sample_count: usize,
}

pub struct InsufficientData;

impl Decoder {
    pub fn new() -> Self {
        let mut decoder = MaybeUninit::<ffi::mp3dec_t>::uninit();
        unsafe { &mut *decoder.as_mut_ptr() }.header[0] = 0;
        Self {
            decoder,
            frame_recv: MaybeUninit::uninit(),
        }
    }

    #[inline(always)]
    pub fn peek<'a, 'src>(
        &'a mut self,
        data: &'src [u8],
    ) -> Result<Chunk<'src, 'static>, InsufficientData> {
        self.dec(data, None)
    }

    #[inline(always)]
    pub fn next<'a, 'src, 'pcm>(
        &'a mut self,
        data: &'src [u8],
        buf: &'pcm mut DecoderBuffer,
    ) -> Result<Chunk<'src, 'pcm>, InsufficientData> {
        self.dec(data, Some(buf))
    }

    fn dec<'a, 'src, 'pcm>(
        &'a mut self,
        data: &'src [u8],
        buf: Option<&'pcm mut DecoderBuffer>,
    ) -> Result<Chunk<'src, 'pcm>, InsufficientData> {
        unsafe {
            // The minimp3 API takes `int` for size, however that won't work if
            // your file exceeds 2GB (2147483647b) in size. Thankfully,
            // under pretty much no circumstances will each frame be >2GB.
            // Even if it would be, this makes it not UB and just return err/eof.
            let data_len = data.len().min(c_int::max_value() as usize) as c_int;
            let pcm_ptr = buf
                .map(|r| r as *mut DecoderBuffer)
                .unwrap_or(ptr::null_mut());
            let samples = ffi::mp3dec_decode_frame(
                self.decoder.as_mut_ptr(),    // mp3dec instance
                data.as_ptr(),                // data pointer
                data_len,                     // pointer length
                pcm_ptr as *mut Sample,       // output buffer
                self.frame_recv.as_mut_ptr(), // frame info
            );
            let frame_recv = &*self.frame_recv.as_ptr();
            if samples != 0 {
                // we got samples!
                Ok(Chunk::Samples(Frame {
                    bitrate: frame_recv.bitrate_kbps as u32,
                    channels: frame_recv.channels as u32,
                    mpeg_layer: frame_recv.layer as u32,
                    sample_rate: frame_recv.hz as u32,

                    bytes_read: frame_recv.frame_bytes as usize,
                    source: frame_slice(data, frame_recv),
                    samples: if !pcm_ptr.is_null() {
                        let pcm_points = samples as usize * frame_recv.channels as usize;
                        (&*pcm_ptr).0.get_unchecked(..pcm_points)
                    } else {
                        &[]
                    },
                    sample_count: samples as usize,
                }))
            } else if frame_recv.frame_bytes != 0 {
                Ok(Chunk::UnknownData(
                    frame_recv.frame_bytes as usize,
                    frame_slice(data, frame_recv),
                ))
            } else {
                // nope.
                return Err(InsufficientData);
            }
        }
    }
}

impl DecoderBuffer {
    pub fn new() -> Self {
        Self(unsafe { MaybeUninit::uninit().assume_init() })
    }
}

#[inline(always)]
unsafe fn frame_slice<'src, 'frame>(
    data: &'src [u8],
    frame_recv: &'frame ffi::mp3dec_frame_info_t,
) -> &'src [u8] {
    data.get_unchecked(frame_recv.frame_offset as usize..frame_recv.frame_bytes as usize)
}
