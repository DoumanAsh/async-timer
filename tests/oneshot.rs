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

    assert!(diff.as_millis() >= 1_500 && diff.as_millis() <= 2_500);
}

#[test]
fn test_tons_oneshot() {
    const NUM: usize = 1024;
    let mut jobs = Vec::with_capacity(NUM);

    for idx in 0..NUM {
        if idx % 2 == 0 {
            jobs.push(Timer::new(time::Duration::from_secs(2)));
        } else {
            jobs.push(Timer::new(time::Duration::from_secs(1)));
        }
    }

    let before = time::SystemTime::now();
    block_on(futures::future::join_all(jobs));
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 1_500 && diff.as_millis() <= 2_500);
}
