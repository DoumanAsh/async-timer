# async-timer

![Rust](https://github.com/DoumanAsh/async-timer/workflows/Rust/badge.svg?branch=master)
[![Crates.io](https://img.shields.io/crates/v/async-timer.svg)](https://crates.io/crates/async-timer)
[![Documentation](https://docs.rs/async-timer/badge.svg)](https://docs.rs/crate/async-timer/)
[![dependency status](https://deps.rs/crate/async-timer/1.0.0-beta.8/status.svg)](https://deps.rs/crate/async-timer)

Timer facilities for Rust's async story

## Accuracy

Regular timers that do not rely on async event loop tend to be on par with user space timers
like in `tokio`.
If that's not suitable for you you should enable event loop based timers which in most cases
give you the most accurate timers possible on unix platforms (See features.)

## Features

- `tokio1` - Enables event loop based timers using tokio, providing higher resolution timers on unix platforms.
- `c_wrapper` - Uses C shim to create bindings to platform API, which may be more reliable than `libc`.
- `std` - Enables usage of std types (e.g. Error)
- `stream` - Enables `Stream` implementation for `Interval`

## Examples

### Timed

```rust
async fn job() {
}

async fn do_job() {
    let work = unsafe {
        async_timer::Timed::platform_new_unchecked(job(), core::time::Duration::from_secs(1))
    };

    match work.await {
        Ok(_) => println!("I'm done!"),
        //You can retry by polling `expired`
        Err(expired) => println!("Job expired: {}", expired),
    }
}
```

### Interval

```rust
async fn job() {
}

async fn do_a_while() {
    let mut times: u8 = 0;
    let mut interval = async_timer::Interval::platform_new(core::time::Duration::from_secs(1));

    while times < 5 {
        job().await;
        interval.wait().await;
        times += 1;
    }
}
```
