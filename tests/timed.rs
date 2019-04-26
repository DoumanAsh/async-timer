use async_timer::{Timed};
use async_timer::oneshot::{Oneshot, Timer};
use futures::executor::block_on;

use std::time;

#[test]
fn test_timed() {
    let future = Timer::new(time::Duration::from_secs(4));
    let work = Timed::platform_new(future, time::Duration::from_secs(3));

    let before = time::SystemTime::now();

    let expired = block_on(work).unwrap_err();
    let work = block_on(expired);

    assert!(block_on(work).is_ok());
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    #[cfg(not(windows))]
    assert_eq!(diff.as_secs(), 4);
    //Windows note: Since we're using thread pool timer, it might cause some inaccuracy
    #[cfg(windows)]
    assert!(diff.as_millis() >= 3_500 && diff.as_millis() <= 4_500);
}
