use async_timer::oneshot::{Oneshot, Timer};
use futures::executor::block_on;

use std::time;

#[test]
fn test_oneshot() {
    let work = Timer::new(time::Duration::from_secs(2));

    let before = time::SystemTime::now();
    block_on(work);
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    #[cfg(not(windows))]
    assert_eq!(diff.as_secs(), 2);
    //Windows note: Since we're using thread pool timer, it might cause some inaccuracy
    #[cfg(windows)]
    assert!(diff.as_millis() >= 1_500 && diff.as_millis() <= 2_500);
}
