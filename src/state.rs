//!State module

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize};
use core::sync::atomic::Ordering::{Acquire, Release, AcqRel};
use core::task::{self, Waker};

// Based on futures-rs
struct AtomicWaker {
    state: AtomicUsize,
    waker: UnsafeCell<Option<Waker>>,
}

/// Idle state
const WAITING: usize = 0;

/// A new waker value is being registered with the `AtomicWaker` cell.
const REGISTERING: usize = 0b01;

/// The waker currently registered with the `AtomicWaker` cell is being woken.
const WAKING: usize = 0b10;

impl AtomicWaker {
    const fn new() -> Self {
        Self {
            state: AtomicUsize::new(WAITING),
            waker: UnsafeCell::new(None),
        }
    }

    fn register(&self, waker: &Waker) {
        match self.state.compare_and_swap(WAITING, REGISTERING, Acquire) {
            WAITING => {
                unsafe {
                    // Locked acquired, update the waker cell
                    *self.waker.get() = Some(waker.clone());

                    // Release the lock. If the state transitioned to include
                    // the `WAKING` bit, this means that a wake has been
                    // called concurrently, so we have to remove the waker and
                    // wake it.`
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we jut set it to.
                    let res = self.state.compare_exchange(REGISTERING, WAITING, AcqRel, Acquire);

                    match res {
                        Ok(_) => {}
                        Err(actual) => {
                            // This branch can only be reached if a
                            // concurrent thread called `wake`. In this
                            // case, `actual` **must** be `REGISTERING |
                            // `WAKING`.
                            debug_assert_eq!(actual, REGISTERING | WAKING);

                            // Take the waker to wake once the atomic operation has
                            // completed.
                            let waker = (*self.waker.get()).take().unwrap();

                            // Just swap, because no one could change state while state == `REGISTERING` | `WAKING`.
                            self.state.swap(WAITING, AcqRel);

                            // The atomic swap was complete, now
                            // wake the task and return.
                            waker.wake();
                        }
                    }
                }
            }
            WAKING => {
                // Currently in the process of waking the task, i.e.,
                // `wake` is currently being called on the old task handle.
                // So, we call wake on the new waker
                waker.wake_by_ref();
            }
            state => {
                // In this case, a concurrent thread is holding the
                // "registering" lock. This probably indicates a bug in the
                // caller's code as racing to call `register` doesn't make much
                // sense.
                //
                // We just want to maintain memory safety. It is ok to drop the
                // call to `register`.
                debug_assert!(
                    state == REGISTERING ||
                    state == REGISTERING | WAKING);
            }
        }
    }

    fn is_registered(&self) -> bool {
        unsafe { (*self.waker.get()).is_some() }
    }

    fn wake(&self) {
        if let Some(waker) = self.take() {
            waker.wake();
        }
    }

    fn take(&self) -> Option<Waker> {
        // AcqRel ordering is used in order to acquire the value of the `task`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicWaker` is associated with.
        match self.state.fetch_or(WAKING, AcqRel) {
            WAITING => {
                // The waking lock has been acquired.
                let waker = unsafe { (*self.waker.get()).take() };

                // Release the lock
                self.state.fetch_and(!WAKING, Release);

                waker
            }
            state => {
                // There is a concurrent thread currently updating the
                // associated task.
                //
                // Nothing more to do as the `WAKING` bit has been set. It
                // doesn't matter if there are concurrent registering threads or
                // not.
                //
                debug_assert!(
                    state == REGISTERING ||
                    state == REGISTERING | WAKING ||
                    state == WAKING);
                None
            }
        }
    }
}

unsafe impl Send for AtomicWaker {}
unsafe impl Sync for AtomicWaker {}

///Timer's state
pub struct TimerState {
    ///Underlying waker
    inner: AtomicWaker,
}

impl TimerState {
    ///Initializes state.
    pub const fn new() -> Self {
        Self {
            inner: AtomicWaker::new(),
        }
    }

    #[inline]
    ///Returns whether notification has been fired.
    ///
    ///Namely it checks whether `Waker` is registered
    ///with `TimerState` or not. It is not intended for user
    ///to call `is_done` before  `register`
    pub fn is_done(&self) -> bool {
        !self.inner.is_registered()
    }

    #[inline]
    ///Registers `Waker` with state
    pub fn register(&self, waker: &task::Waker) {
        self.inner.register(waker)
    }

    #[inline]
    ///Notifies underlying `Waker`
    ///
    ///After that `Waker` is no longer registered with `TimerState`
    pub fn wake(&self) {
        self.inner.wake();
    }
}
