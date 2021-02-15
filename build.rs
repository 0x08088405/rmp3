fn main() {
    println!("cargo:rerun-if-changed=ffi/minimp3.c");
    let mut build = cc::Build::new();

    build.include("ffi/minimp3");

    if cfg!(feature = "float") {
        build.define("MINIMP3_FLOAT_OUTPUT", None);
    }
    if cfg!(not(feature = "simd")) {
        build.define("MINIMP3_NO_SIMD", None);
    }
    if cfg!(not(feature = "mp1-mp2")) {
        build.define("MINIMP3_ONLY_MP3", None);
    }

    build
        .define("MINIMP3_IMPLEMENTATION", None)
        .file("ffi/minimp3.c")
        .compile("minimp3");
}
