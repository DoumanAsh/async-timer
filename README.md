# async-timer

[![Build Status](https://dev.azure.com/DoumanAsh/async-timer/_apis/build/status/DoumanAsh.async-timer?branchName=master)](https://dev.azure.com/DoumanAsh/async-timer/_build/latest?definitionId=1&branchName=master)
[![Crates.io](https://img.shields.io/crates/v/async-timer.svg)](https://crates.io/crates/async-timer)
[![Documentation](https://docs.rs/async-timer/badge.svg)](https://docs.rs/crate/async-timer/)
[![dependency status](https://deps.rs/crate/async-timer/0.5.0/status.svg)](https://deps.rs/crate/async-timer)

Timer facilities for Rust's async story

Minimal Rust version: 1.36

## Timed

```rust
#![feature(async_await)]

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
#![feature(async_await)]

async fn job() {
}

async fn do_a_while() {
    let mut times: u8 = 0;
    let mut interval = async_timer::Interval::platform_new(core::time::Duration::from_secs(1));

    while times < 5 {
        job().await;
        interval = interval.await;
        times += 1;
    }
}
```

## Q&A

Q: When it is going to be async/await?

A: When async/await will become `no_std`
