# easy script for regenerating bindings (mainly for myself, NOT part of the build process)
# run from repository root, also install `cargo install bindgen` cli if you haven't
git submodule update --init --recursive
bindgen ffi/bindgen.h --use-core --ctypes-prefix libc --output src/bindings.rs -- -Iffi/minimp3

# fix a typedef error with the float feature which changes the pointer type
sed -i 's/pub type mp3d_sample_t = i16;/#[cfg(not(feature = "float"))]\npub type mp3d_sample_t = i16;\n#[cfg(feature = "float")]\npub type mp3d_sample_t = f32;/' src/bindings.rs
