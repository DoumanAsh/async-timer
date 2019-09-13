use async_timer::{Interval};

use std::time;

#[test]
fn test_interval() {
    cute_async::runtime::tokio(async {
        let mut interval = Interval::platform_new(time::Duration::from_secs(1));

        let before = time::SystemTime::now();
        interval.as_mut().await;
        let after = time::SystemTime::now();
        let diff = after.duration_since(before).unwrap();

        assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);

        let before = time::SystemTime::now();
        interval.as_mut().await;
        let after = time::SystemTime::now();
        let diff = after.duration_since(before).unwrap();

        assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);
    });
}
