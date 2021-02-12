#!/bin/sh

# easy script for regenerating bindings (mainly for myself, NOT part of the build process)
# run from repository root, also install `cargo install bindgen` cli if you haven't
# sed fixes a typedef with the float feature which changes the type based on a #define (-> rust feature)
# !! make sure to remove platform specifics after running and keep bare minimum !!

ss='s/'
ss+='pub type mp3d_sample_t = i16;'
ss+='/'
ss+='\/\/\/ Note: Type depends on `float` feature (see [`Sample`](crate::Sample))\n'
ss+='#[cfg(not(feature = "float"))]\n'
ss+='pub type mp3d_sample_t = i16;\n'
ss+='#[cfg(feature = "float")]\n'
ss+='pub type mp3d_sample_t = f32;'
ss+='/'

git submodule update --init --recursive && \
    bindgen ffi/bindgen.h \
        --use-core --ctypes-prefix libc \
        --output src/bindings.rs -- -Iffi/minimp3 && \
    sed -i "${ss}" src/bindings.rs
