
fn main() {
    #[cfg(feature = "c_wrapper")]
    {
        use std::env;

        let host = env::var("HOST").unwrap();
        let target = env::var("TARGET").unwrap();

        if host != target {
            println!("cargo:warning=async-timer is cross-compiled, C wrapper cannot be used. Sorry but I'm too lazy to bother with it, use cross or docker with proper image");
            return;
        }

        #[cfg(all(target_family = "unix", not(any(target_os = "macos", target_os = "ios"))))]
        {
            cc::Build::new().file("src/c_wrapper/posix.c")
                            .compile("libposix_wrapper.a");
        }
    }
}
