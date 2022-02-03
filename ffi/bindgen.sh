#!/bin/sh

# script for reproducing src/ffi.rs
# you can get `bindgen` via cargo install
# you're meant to run this from the repository root

# generate src/ffi.rs based on ffi/bindgen.h
bindgen ffi/bindgen.h \
        --raw-line '#![allow(clippy::all, non_camel_case_types)]' \
        --allowlist-function '^mp3dec.+' \
        --allowlist-type '^mp3dec.+' \
        --ctypes-prefix 'chlorine' \
        --no-layout-tests \
        --rust-target '1.47' \
        --size_t-is-usize \
        --use-core \
        --output src/ffi.rs \
        -- -I 'ffi/minimp3'

# bindgen filter misses the unused stdint.h definitions
sed -ri '/type __u?int[0-9]/d' src/ffi.rs
