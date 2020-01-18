# easy script for regenerating bindings
git submodule update --init --recursive
bindgen src/include.h --output src/bindings.rs -- -Iminimp3
