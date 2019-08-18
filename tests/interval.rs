#![feature(async_await)]

use async_timer::{Interval};

use std::time;

#[tokio::test]
async fn test_interval() {
    let mut interval = Interval::platform_new(time::Duration::from_secs(1));

    let before = time::SystemTime::now();
    interval = interval.await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);

    let before = time::SystemTime::now();
    let _interval = interval.await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);
}