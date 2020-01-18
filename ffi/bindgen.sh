# easy script for regenerating bindings
# run from repository root btw, also install `cargo install bindgen` cli if you haven't
git submodule update --init --recursive
bindgen ffi/bindgen.h --output src/bindings.rs -- -Iffi/minimp3
