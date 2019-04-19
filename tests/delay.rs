#![feature(async_await, await_macro, futures_api)]

use async_timer::{PlatformTimer, Delay};
use futures::executor::block_on;

use std::time;

#[test]
fn test_delay() {
    let work = Delay::<PlatformTimer>::new(time::Duration::from_secs(2));

    let before = time::SystemTime::now();
    block_on(work);
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert_eq!(diff.as_secs(), 2);
}
