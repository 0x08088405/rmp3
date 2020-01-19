# easy script for regenerating bindings (mainly for myself, NOT part of the build process)
# run from repository root, also install `cargo install bindgen` cli if you haven't
git submodule update --init --recursive
bindgen ffi/bindgen.h --use-core --ctypes-prefix libc --output src/bindings.rs -- -Iffi/minimp3
