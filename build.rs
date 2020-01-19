fn main() {
    println!("cargo:rerun-if-changed=ffi/minimp3.c");
    cc::Build::new()
        .include("ffi/minimp3")
        .define("MINIMP3_IMPLEMENTATION", None)
        .file("ffi/minimp3.c")
        .compile("minimp3");
}
