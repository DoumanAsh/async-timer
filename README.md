# async-timer

![Rust](https://github.com/DoumanAsh/async-timer/workflows/Rust/badge.svg?branch=master)
[![Crates.io](https://img.shields.io/crates/v/async-timer.svg)](https://crates.io/crates/async-timer)
[![Documentation](https://docs.rs/async-timer/badge.svg)](https://docs.rs/crate/async-timer/)
[![dependency status](https://deps.rs/crate/async-timer/1.0.0-beta.4/status.svg)](https://deps.rs/crate/async-timer)

Timer facilities for Rust's async story

Minimal Rust version: 1.36

## Timed

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

## Interval

```rust
async fn job() {
}

async fn do_a_while() {
    let mut times: u8 = 0;
    let mut interval = async_timer::Interval::platform_new(core::time::Duration::from_secs(1));

    while times < 5 {
        job().await;
        interval.as_mut().await;
        times += 1;
    }
}
```
