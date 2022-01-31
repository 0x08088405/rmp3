//! This is a cool library!

// TODO TODO TODO TODO TODO TODO TODO TODO TODO TODO
//#![deny(missing_docs)]

#![no_std]

// TODO: should the members here be pub(crate)? hope that won't need sed
mod ffi;

use core::{fmt, marker::PhantomData, mem::MaybeUninit, num::NonZeroU16, ptr, slice};
use chlorine::c_int;

/// Maximum number of samples per frame.
pub const MAX_SAMPLES: usize = 1152 * 2;

pub enum Frame<'src, 'pcm> {
    Audio(Audio<'src, 'pcm>),
    Other(&'src [u8]),
}

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
    #[inline]
    pub fn bitrate(&self) -> u16 {
        // 8 <= x <= 448
        // TODO check what happens with the reserved bitrates
        self.bitrate
    }

    /// Gets how many channels are in this frame.
    #[inline]
    pub fn channels(&self) -> NonZeroU16 {
        // x ∈ {1, 2}
        unsafe { NonZeroU16::new_unchecked(self.channels as u16) }
    }

    /// Gets the MPEG layer of this frame.
    #[inline]
    pub fn mpeg_layer(&self) -> u8 {
        // 1 <= x <= 3
        // TODO check what happens when the illegal 0b00 layer is passed
        self.mpeg_layer as u8
    }

    /// Gets the number of samples in this frame.
    #[inline]
    pub fn sample_count(&self) -> NonZeroU16 {
        // 0 < x <= MAX_SAMPLES < 2^16-1
        unsafe { NonZeroU16::new_unchecked(self.sample_count as u16) }
    }

    /// Gets the sample rate of this frame in Hz.
    #[inline]
    pub fn sample_rate(&self) -> NonZeroU16 {
        // 8000 <= x <= 44100
        unsafe { NonZeroU16::new_unchecked(self.sample_rate as u16) }
    }

    /// Gets the slice of decoded samples.
    ///
    /// If [`inspect`](Decoder::inspect) was used,
    /// this slice will be empty, as nothing was decoded.
    #[inline]
    pub fn samples(&self) -> &'pcm [f32] {
        if let Some(buf) = self.pcm {
            unsafe { slice::from_raw_parts(buf.as_ptr(), usize::from(self.sample_count)) }
        } else {
            &[]
        }
    }

    #[inline]
    pub fn source(&self) -> &'src [u8] {
        self.src
    }
}

impl fmt::Debug for Frame<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio(audio) => f.debug_tuple("Audio").field(audio).finish(),
            Self::Other(_) => f.debug_tuple("Other").field(&"&[...]").finish(),
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
                if self.pcm.is_some() {
                    &"&[...]"
                } else {
                    &"&[not decoded]"
                }
            })
            .finish()
    }
}

pub struct Decoder(MaybeUninit<ffi::mp3dec_t>);

impl Decoder {
    #[inline]
    pub fn new() -> Self {
        let mut state = MaybeUninit::uninit();
        unsafe { ffi::mp3dec_init(state.as_mut_ptr()) };
        Self(state)
    }

    #[inline]
    pub fn decode<'src, 'pcm>(
        &mut self,
        src: &'src [u8],
        dest: &'pcm mut [f32; MAX_SAMPLES],
    ) -> Option<(Frame<'src, 'pcm>, usize)> {
        self.process(src, Some(dest))
    }

    #[inline]
    pub fn inspect<'src>(
        &mut self,
        src: &'src [u8],
    ) -> Option<(Frame<'src, 'static>, usize)> {
        self.process(src, None)
    }

    fn process<'src, 'pcm>(
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
                let audio = Audio {
                    bitrate: info.bitrate_kbps as u16, // 8 <= x <= 448
                    channels: info.channels as u8,     // x ∈ {1, 2}
                    mpeg_layer: info.layer as u8,      // 1 <= x <= 3
                    sample_count: sample_count as u16, // 0 < x <= MAX_SAMPLES
                    sample_rate: info.hz as u16,       // 8000 <= x <= 44100

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
