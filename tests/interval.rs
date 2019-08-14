#![feature(async_await)]

use async_timer::{Interval};

use std::time;

#[tokio::test]
async fn test_interval() {
    #[cfg(feature = "tokio")]
    let _reactor = tokio_reactor::Reactor::new().unwrap();

    let mut interval = Interval::platform_new(time::Duration::from_secs(1));

    let before = time::SystemTime::now();
    interval = interval.next().await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);

    let before = time::SystemTime::now();
    let _interval = interval.next().await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);
}
