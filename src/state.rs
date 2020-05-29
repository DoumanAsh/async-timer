//!State module

use core::task;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

mod noop {
    use core::{ptr, task};

    static VTABLE: task::RawWakerVTable = task::RawWakerVTable::new(clone, action, action, action);

    fn clone(_: *const()) -> task::RawWaker {
        task::RawWaker::new(ptr::null(), &VTABLE)
    }

    fn action(_: *const ()) {
    }

    #[inline(always)]
    pub fn waker() -> task::Waker {
        unsafe {
            task::Waker::from_raw(task::RawWaker::new(ptr::null(), &VTABLE))
        }
    }
}

/// Idle state
const WAITING: u8 = 0;

/// A new waker value is being registered with the `AtomicWaker` cell.
const REGISTERING: u8 = 0b01;

/// The waker currently registered with the `AtomicWaker` cell is being woken.
const WAKING: u8 = 0b10;

// Based on futures-rs
struct AtomicWaker {
    state: AtomicU8,
    waker: UnsafeCell<task::Waker>,
}

impl AtomicWaker {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(WAITING),
            waker: UnsafeCell::new(noop::waker()),
        }
    }

    fn register(&self, waker: &task::Waker) {
        match self.state.compare_and_swap(WAITING, REGISTERING, Ordering::Acquire) {
            WAITING => {
                unsafe {
                    // Locked acquired, update the waker cell
                    *self.waker.get() = waker.clone();

                    // Release the lock. If the state transitioned to include
                    // the `WAKING` bit, this means that a wake has been
                    // called concurrently, so we have to remove the waker and
                    // wake it.`
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we jut set it to.
                    let res = self.state.compare_exchange(REGISTERING, WAITING, Ordering::AcqRel, Ordering::Acquire);

                    match res {
                        Ok(_) => {}
                        Err(actual) => {
                            // This branch can only be reached if a
                            // concurrent thread called `wake`. In this
                            // case, `actual` **must** be `REGISTERING |
                            // `WAKING`.
                            debug_assert_eq!(actual, REGISTERING | WAKING);

                            // Wake without dropping.
                            (*self.waker.get()).wake_by_ref();

                            // Just swap, because no one could change state while state == `REGISTERING` | `WAKING`.
                            self.state.swap(WAITING, Ordering::AcqRel);
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

    fn wake(&self) {
        // AcqRel ordering is used in order to acquire the value of the `task`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicWaker` is associated with.
        match self.state.fetch_or(WAKING, Ordering::AcqRel) {
            WAITING => {
                // The waking lock has been acquired.
                unsafe { (*self.waker.get()).wake_by_ref() };

                // Release the lock
                self.state.fetch_and(!WAKING, Ordering::Release);
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
            }
        }
    }
}

unsafe impl Send for AtomicWaker {}
unsafe impl Sync for AtomicWaker {}

///Timer's state
pub struct TimerState {
    woken: AtomicBool,
    inner: AtomicWaker,
}

impl TimerState {
    ///Initializes state.
    pub fn new() -> Self {
        Self {
            woken: AtomicBool::new(false),
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
        self.woken.load(Ordering::Acquire)
    }

    #[inline]
    ///Registers `Waker` with state
    pub fn register(&self, waker: &task::Waker) {
        self.inner.register(waker);
        self.woken.store(false, Ordering::Release);
    }

    #[inline]
    ///Notifies underlying `Waker`
    ///
    ///After that `Waker` is no longer registered with `TimerState`
    pub fn wake(&self) {
        if !self.woken.compare_and_swap(false, true, Ordering::SeqCst) {
            self.inner.wake();
        }
    }
}
