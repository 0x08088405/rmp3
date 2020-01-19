fn main() {
    println!("cargo:rerun-if-changed=ffi/minimp3.c");
    let mut build = cc::Build::new();

    build
        .include("ffi/minimp3")
        .define("MINIMP3_IMPLEMENTATION", None);

    if cfg!(feature = "only-mp3") {
        build.define("MINIMP3_ONLY_MP3", None);
    }

    build.file("ffi/minimp3.c").compile("minimp3");
}
