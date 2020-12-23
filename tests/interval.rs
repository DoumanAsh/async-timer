use async_timer::{Interval};
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

#[cfg(feature = "stream")]
#[tokio::test]
async fn test_stream_interval() {
    use futures_core::Stream;
    use core::future::Future;
    use core::pin::Pin;
    use core::task;

    pub struct Next<'a, S> {
        stream: &'a mut S,
    }

    impl<'a, S> Next<'a, S> {
        fn new(stream: &'a mut S) -> Self {
            Self {
                stream,
            }
        }
    }

    impl<S: Stream + Unpin> Future for Next<'_, S> {
        type Output = Option<S::Item>;

        fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
            let stream = Pin::new(&mut self.stream);
            stream.poll_next(ctx)
        }
    }

    let mut interval = Interval::platform_new(time::Duration::from_secs(1));

    let before = time::SystemTime::now();
    Next::new(&mut interval).await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);

    let before = time::SystemTime::now();
    Next::new(&mut interval).await;
    let after = time::SystemTime::now();
    let diff = after.duration_since(before).unwrap();

    assert!(diff.as_millis() >= 750 && diff.as_millis() <= 1_250);
}
