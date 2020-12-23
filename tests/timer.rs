use async_timer::timer::{Timer, Platform, SyncTimer, new_sync_timer};
use tokio_1 as tokio;

use std::time;

#[tokio::test]
async fn test_async_timer() {
    let work = Platform::new(time::Duration::from_secs(2));
    assert!(!work.is_ticking());
    assert!(!work.is_expired());

    let before = time::SystemTime::now();
    work.await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 1_500 && diff.as_millis() <= 2_500);
}

#[test]
fn test_cancel_timer() {
    let mut work = new_sync_timer(time::Duration::from_secs(500000));

    assert!(!work.is_ticking());
    assert!(!work.is_expired());

    assert!(!work.tick());

    assert!(work.is_ticking());
    assert!(!work.is_expired());

    work.cancel();

    assert!(!work.is_ticking());
    assert!(work.is_expired());
}
