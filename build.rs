fn main() {
    #[cfg(all(feature = "c_wrapper", unix, not(any(target_os = "macos", target_os = "ios"))))]
    {
        cc::Build::new().file("src/c_wrapper/posix.c")
                        .compile("libposix_wrapper.a");
    }
}
