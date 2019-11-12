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

///Assertion macro, which panics with last OS error
///
///With `no_std` equal to `assert!`
#[cfg(not(feature = "no_std"))]
macro_rules! os_assert {
    ($cond:expr) => ({
        if !($cond) {
            panic!("Assertion '{}' failed. OS error: {:?}", stringify!($cond), std::io::Error::last_os_error());
        }
    })
}

#[cfg(feature = "no_std")]
macro_rules! os_assert {
    ($cond:expr) => ({
        assert!($cond);
    })
}
