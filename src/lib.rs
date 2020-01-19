#![no_std]

use core::{mem, ptr};
use libc::c_int;

/// Raw minimp3 bindings if you need them for whatever reason.
///
/// Although if there's a desired feature make an issue/PR.
pub mod ffi {
    #![allow(clippy::all, non_camel_case_types)]

    include!("bindings.rs");
}

#[cfg(not(feature = "float"))]
pub type Sample = i16;
#[cfg(feature = "float")]
pub type Sample = f32;

pub struct Decoder<'a> {
    data: &'a [u8],
    ffi_frame: ffi::mp3dec_frame_info_t,
    instance: ffi::mp3dec_t,
    pcm: [Sample; ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize],
}

pub struct Frame<'a> {
    /// Bitrate of this frame in kb/s.
    pub bitrate: u32,

    /// Number of channels in this frame.
    pub channels: u32,

    /// MPEG layer of this frame.
    pub mpeg_layer: i32, // TODO: Enumify

    /// Reference to the samples in this frame, copy if needed to allocate.
    pub samples: &'a [Sample],

    /// Sample count per channel.
    /// Should be identical to `samples.len() / channels`
    /// unless you used [peek_frame](struct.Decoder.html#method.peek_frame).
    pub sample_count: u32,

    /// Sample rate of this frame in Hz.
    pub sample_rate: u32,

    /// Size of the source frame in bytes.
    pub source_len: usize,
}

impl<'a> Decoder<'a> {
    /// Creates a decoder over `data` (mp3 bytes).
    pub fn new(data: &'a (impl AsRef<[u8]> + ?Sized)) -> Self {
        Self {
            data: data.as_ref(),
            ffi_frame: unsafe { mem::zeroed() },
            instance: unsafe {
                let mut decoder: ffi::mp3dec_t = mem::zeroed();
                ffi::mp3dec_init(&mut decoder);
                decoder
            },
            pcm: [Default::default(); ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize],
        }
    }

    /// Reads the next frame, if available.
    pub fn next_frame(&mut self) -> Option<Frame> {
        unsafe {
            let mut samples =
                self.ffi_decode_frame(self.data.as_ptr(), self.data.len() as c_int) as u32;
            self.data = self
                .data
                .get_unchecked(self.ffi_frame.frame_bytes as usize..);
            if samples > 0 {
                samples *= self.ffi_frame.channels as u32;
                Some(Frame {
                    bitrate: self.ffi_frame.bitrate_kbps as u32,
                    channels: self.ffi_frame.channels as u32,
                    samples: self.pcm.get_unchecked(..samples as usize), // todo: feature?
                    sample_rate: self.ffi_frame.hz as u32,
                    mpeg_layer: self.ffi_frame.layer,
                    sample_count: samples,
                    source_len: self.ffi_frame.frame_bytes as usize,
                })
            } else if self.ffi_frame.frame_bytes != 0 {
                self.next_frame()
            } else {
                None
            }
        }
    }

    /// Reads a frame without actually decoding it or advancing.
    /// Useful when you want to, for example, calculate the audio length.
    ///
    /// It should be noted that the [samples](struct.Frame.html#structfield.sample_count)
    /// in [Frame](struct.Frame.html) are an empty slice,
    /// but you can still read its [sample_count](struct.Frame.html#structfield.sample_count).
    pub fn peek_frame(&mut self) -> Option<Frame> {
        let samples = unsafe { self.ffi_decode_frame(ptr::null(), 0) as u32 };
        if self.ffi_frame.frame_bytes != 0 {
            Some(Frame {
                bitrate: self.ffi_frame.bitrate_kbps as u32,
                channels: self.ffi_frame.channels as u32,
                mpeg_layer: self.ffi_frame.layer,
                samples: &[],
                sample_rate: self.ffi_frame.hz as u32,
                sample_count: samples,
                source_len: self.ffi_frame.frame_bytes as usize,
            })
        } else {
            None
        }
    }

    /// Skips ahead by `frame_length` bytes.
    /// Should be used in combination with [peek_frame](struct.Decoder.html#method.peek_frame)
    /// so you know how long the frame is.
    pub fn skip_frame(&mut self, frame_length: usize) {
        self.data = self.data.get(..frame_length).unwrap_or(&[]);
    }

    unsafe fn ffi_decode_frame(&mut self, data: *const u8, len: c_int) -> c_int {
        ffi::mp3dec_decode_frame(
            &mut self.instance,    // mp3dec instance
            data,                  // data pointer
            len,                   // pointer length
            self.pcm.as_mut_ptr(), // output buffer
            &mut self.ffi_frame,   // frame info
        )
    }
}
