use core::pin::Pin;
use tokio_1 as tokio;

use std::time;

#[tokio::test]
async fn test_timed() {
    let mut future = async_timer::new_timer(time::Duration::from_secs(4));
    let future = Pin::new(&mut future);
    let work = async_timer::timed(future, time::Duration::from_secs(3));

    let before = time::SystemTime::now();

    let expired = work.await.unwrap_err();
    let work = expired.await;

    assert!(work.await.is_ok());
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 3_500 && diff.as_millis() <= 4_500);
}
