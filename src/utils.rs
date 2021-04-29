#[doc(hidden)]
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! unreach {
    () => ({
        unsafe {
            core::hint::unreachable_unchecked();
        }
    })
}

#[doc(hidden)]
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! unreach {
    () => ({
        unreachable!()
    })
}

#[allow(unused_macros)]
///Assertion macro, which panics with last OS error
macro_rules! os_assert {
    ($cond:expr) => ({
        if !($cond) {
            panic!("Assertion '{}' failed. {}", stringify!($cond), error_code::SystemError::last());
        }
    })
}

#[allow(unused_macros)]
#[doc(hidden)]
macro_rules! assert_time {
    ($time:expr) => ({
        debug_assert!(!($time.as_secs() == 0 && $time.subsec_nanos() == 0), "Zero timeout makes no sense");
    })
}
