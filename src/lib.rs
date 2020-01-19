use std::{mem, os::raw::c_int};

/// Raw minimp3 bindings if you need them for whatever reason.
///
/// Although if there's a desired feature make an issue/PR.
pub mod ffi {
    #![allow(clippy::all, non_camel_case_types)]

    include!("bindings.rs");
}

type Sample = i16; // conditionally replace this later if mp3 should produce float

pub struct Decoder<'a> {
    data: &'a [u8],
    ffi_frame: ffi::mp3dec_frame_info_t,
    instance: ffi::mp3dec_t,
    pcm: [i16; ffi::MINIMP3_MAX_SAMPLES_PER_FRAME as usize],
}

pub struct Frame<'a> {
    pub samples: &'a [Sample],
    pub sample_rate: i32,
    pub channels: i32,
    pub mpeg_layer: i32, // probably enumifiable
    pub bitrate: i32,    // kb/s
}

impl<'a> Decoder<'a> {
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

    pub fn next_frame(&mut self) -> Option<Frame> {
        unsafe {
            let mut samples = ffi::mp3dec_decode_frame(
                &mut self.instance,       // mp3dec instance
                self.data.as_ptr(),       // data pointer
                self.data.len() as c_int, // pointer length
                self.pcm.as_mut_ptr(),    // output buffer
                &mut self.ffi_frame,      // frame info
            );
            self.data = self
                .data
                .get_unchecked(self.ffi_frame.frame_bytes as usize..);
            if samples > 0 {
                samples *= self.ffi_frame.channels;
                Some(Frame {
                    samples: self.pcm.get_unchecked(..samples as usize), // todo: feature?
                    sample_rate: self.ffi_frame.hz,
                    channels: self.ffi_frame.channels,
                    mpeg_layer: self.ffi_frame.layer,
                    bitrate: self.ffi_frame.bitrate_kbps,
                })
            } else if self.ffi_frame.frame_bytes != 0 {
                self.next_frame()
            } else {
                None
            }
        }
    }
}
