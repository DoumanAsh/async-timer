//!State module

use core::{ptr, task, hint, mem};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cold]
fn should_not_clone(_: *const()) -> task::RawWaker {
    panic!("Impossible Waker Clone");
}

mod plain_fn {
    use core::{task, mem};

    static VTABLE: task::RawWakerVTable = task::RawWakerVTable::new(super::should_not_clone, action, action, super::noop::action);

    unsafe fn action(callback: *const ()) {
        let func: fn() = mem::transmute(callback);
        func()
    }

    pub fn waker(data: fn()) -> task::Waker {
        unsafe {
            task::Waker::from_raw(task::RawWaker::new(data as *const (), &VTABLE))
        }
    }
}

mod noop {
    use core::{ptr, task};

    static VTABLE: task::RawWakerVTable = task::RawWakerVTable::new(super::should_not_clone, action, action, action);

    pub fn action(_: *const ()) {
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

#[doc(hidden)]
/// Atomic waker used by `TimerState`
pub struct AtomicWaker {
    state: AtomicU8,
    waker: UnsafeCell<task::Waker>,
}

struct StateRestore<F: Fn()>(F);
impl<F: Fn()> Drop for StateRestore<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

macro_rules! impl_register {
    ($this:ident($waker:ident) { $($impl:tt)+ }) => {
        match $this.state.compare_exchange(WAITING, REGISTERING, Ordering::Acquire, Ordering::Acquire).unwrap_or_else(|err| err) {
            WAITING => {
                //Make sure we do not stuck in REGISTERING state
                let state_guard = StateRestore(|| {
                    $this.state.store(WAITING, Ordering::Release);
                });

                unsafe {
                    $(
                        $impl
                    )+

                    // Release the lock. If the state transitioned to include
                    // the `WAKING` bit, this means that a wake has been
                    // called concurrently, so we have to remove the waker and
                    // wake it.`
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we jut set it to.
                    match $this.state.compare_exchange(REGISTERING, WAITING, Ordering::AcqRel, Ordering::Acquire) {
                        Ok(_) => {
                            mem::forget(state_guard);
                        }
                        Err(actual) => {
                            // This branch can only be reached if a
                            // concurrent thread called `wake`. In this
                            // case, `actual` **must** be `REGISTERING |
                            // `WAKING`.
                            debug_assert_eq!(actual, REGISTERING | WAKING);

                            let mut waker = noop::waker();
                            ptr::swap($this.waker.get(), &mut waker);

                            // Just restore state,
                            // because no one could change state while state == `REGISTERING` | `WAKING`.
                            drop(state_guard);
                            waker.wake();
                        }
                    }
                }
            }
            WAKING => {
                // Currently in the process of waking the task, i.e.,
                // `wake` is currently being called on the old task handle.
                // So, we call wake on the new waker
                $waker.wake_by_ref();
                hint::spin_loop();
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
                    state == REGISTERING | WAKING
                );
            }
        }
    };
}

impl AtomicWaker {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(WAITING),
            waker: UnsafeCell::new(noop::waker()),
        }
    }

    ///This is the same function as `register` but working with owned version.
    fn register(&self, waker: task::Waker) {
        impl_register!(self(waker) {
            //unconditionally store since we already have ownership
            *self.waker.get() = waker;
        });
    }

    fn register_ref(&self, waker: &task::Waker) {
        impl_register!(self(waker) {
            // Lock acquired, update the waker cell
            if !(*self.waker.get()).will_wake(waker) {
                //Clone new waker if it is definitely not the same as old one
                *self.waker.get() = waker.clone();
            }
        });
    }

    fn wake(&self) {
        // AcqRel ordering is used in order to acquire the value of the `task`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicWaker` is associated with.
        match self.state.fetch_or(WAKING, Ordering::AcqRel) {
            WAITING => {
                // The waking lock has been acquired.
                let mut waker = noop::waker();
                unsafe {
                    ptr::swap(self.waker.get(), &mut waker);
                }

                // Release the lock
                self.state.fetch_and(!WAKING, Ordering::Release);
                waker.wake();
            }
            state => {
                // There is a concurrent thread currently updating the
                // associated task.
                //
                // Nothing more to do as the `WAKING` bit has been set. It
                // doesn't matter if there are concurrent registering threads or
                // not.
                debug_assert!(
                    state == REGISTERING ||
                    state == REGISTERING | WAKING ||
                    state == WAKING
                );
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
    ///Resets state, allowing to wake once again.
    pub fn reset(&self) {
        self.woken.store(false, Ordering::Release);
    }

    #[inline]
    ///Informs that timer is cancel, therefore no further callbacks to be passed
    pub fn cancel(&self) {
        self.woken.store(true, Ordering::Release);
    }

    #[inline]
    ///Registers `Callback` with the state.
    ///
    ///This callback is used replaces previous one, if any.
    pub fn register<C: Callback>(&self, cb: C) {
        cb.register(&self.inner);
    }

    #[inline]
    ///Notifies underlying `Waker`
    ///
    ///After that `Waker` is no longer registered with `TimerState`
    pub(crate) fn wake(&self) {
        if !self.woken.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).unwrap_or_else(|err| err) {
            self.inner.wake();
        }
    }
}

///Interface to timer's callback
///
///It is guaranteed that callback is invoked only once, unless `Timer` is restarted or
///`TimerState::reset` is called(happens when timer is restarted)
pub trait Callback {
    #[doc(hidden)]
    fn register(self, waker: &AtomicWaker);
}

impl<'a> Callback for &'a task::Waker {
    #[inline(always)]
    fn register(self, waker: &AtomicWaker) {
        waker.register_ref(self)
    }
}

impl Callback for task::Waker {
    #[inline(always)]
    fn register(self, waker: &AtomicWaker) {
        waker.register(self)
    }
}

impl Callback for fn() {
    fn register(self, waker: &AtomicWaker) {
        waker.register(plain_fn::waker(self));
    }
}
