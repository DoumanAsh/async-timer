use async_timer::Interval;
use tokio_1 as tokio;

use std::time;

#[tokio::test]
async fn test_interval() {
    let mut interval = Interval::platform_new(time::Duration::from_secs(1));

    let before = time::SystemTime::now();
    interval.wait().await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);

    let before = time::SystemTime::now();
    interval.wait().await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);
}

async fn test_interval_average(num_runs: usize, interval: time::Duration) {
    const ACCURACY: time::Duration = time::Duration::from_nanos(133333);

    let mut times = Vec::with_capacity(num_runs);

    println!("interval={:?}", interval);
    let mut timer = async_timer::interval(interval);

    for _ in 0..num_runs {
        let start = std::time::Instant::now();

        timer.wait().await;

        // simulate some work, this doesn't have to be consistent
        // the timer is the only thing that needs to be consistent
        std::thread::sleep(time::Duration::from_millis(1));

        // log time, the timer should be waiting less
        // to account for the time it took to do the work
        times.push(start.elapsed());
    }

    let total: time::Duration = times.iter().sum();
    let average = total / num_runs as u32;

    let min = interval - ACCURACY;
    let max = interval + ACCURACY;
    println!("Check {:?} <= average={:?} <= {:?}", min, average, max);
    //Margin of error should be within interval-0.1ms..=interval+0.1ms
    assert!(min <= average);
    assert!(average <= max);
}

//Windows timers are shite for small duration
//kevent() also behaves badly for some reason
//only linux's timerfd is reliable
#[tokio::test]
#[cfg(feature = "tokio1")]
#[cfg(target_os = "linux")]
async fn test_average_of_small_interval() {
    test_interval_average(6000, time::Duration::from_secs_f32(1. / 120.)).await;
}

#[tokio::test]
#[cfg(feature = "tokio1")]
#[cfg(target_os = "linux")]
async fn test_average_of_mid_interval() {
    test_interval_average(60, time::Duration::from_secs_f32(135. / 120.)).await;
}
