use async_timer::timer::{Timer, Platform};

use std::time;

#[tokio::test]
async fn test_timer() {
    let work = Platform::new(time::Duration::from_secs(2));
    assert!(!work.is_expired());

    let before = time::SystemTime::now();
    work.await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 1_500 && diff.as_millis() <= 2_500);
}

