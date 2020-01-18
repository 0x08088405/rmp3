fn main() {
    cc::Build::new()
        .include("ffi/minimp3")
        .define("MINIMP3_IMPLEMENTATION", None)
        .file("ffi/minimp3.c")
        .compile("minimp3");
}
