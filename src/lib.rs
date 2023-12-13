/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

pub mod arrayvec;
pub mod base64;
pub mod blob;
pub mod buf;
pub mod cast;
pub mod dictionary;
pub mod error;
pub mod exitcode;
pub mod gate;
pub mod hex;
pub mod immortal;
pub mod inetaddress;
pub mod io;
pub mod memory;
pub mod ringbuffer;
pub mod str;
pub mod sync;
pub mod tofrombytes;
pub mod varint;

/// Initial value that should be used for monotonic tick time variables.
pub const NEVER_HAPPENED_TICKS: i64 = i64::MIN / 2;

/// Get milliseconds since unix epoch.
#[inline]
pub fn ms_since_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Get an estimate of the number of CPU cores in the system.
/// This defaults to 1 if information is not available.
pub fn parallelism() -> usize {
    static mut PARALLELISM: usize = 0;
    let mut p = unsafe { PARALLELISM };
    // It's perfectly fine if this runs more than once due to concurrent calls as it should always yield the same value.
    if p == 0 {
        p = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(1);
        unsafe {
            PARALLELISM = p;
        }
    }
    p
}

/// Get milliseconds since an arbitrary time in the past, guaranteed to monotonically increase within a given process.
#[inline]
pub fn ms_monotonic() -> i64 {
    static STARTUP_INSTANT: std::sync::RwLock<Option<std::time::Instant>> = std::sync::RwLock::new(None);
    let si = *STARTUP_INSTANT.read().unwrap();
    if let Some(si) = si {
        si.elapsed().as_millis() as i64
    } else {
        STARTUP_INSTANT
            .write()
            .unwrap()
            .get_or_insert(std::time::Instant::now())
            .elapsed()
            .as_millis() as i64
    }
}

/// Wait for a kill signal (e.g. SIGINT or OS-equivalent) sent to this process and return when received.
#[cfg(unix)]
pub fn wait_for_process_abort() {
    if let Ok(mut signals) = signal_hook::iterator::Signals::new([libc::SIGINT, libc::SIGTERM, libc::SIGQUIT]) {
        'wait_for_exit: loop {
            for signal in signals.wait() {
                match signal as libc::c_int {
                    libc::SIGINT | libc::SIGTERM | libc::SIGQUIT => {
                        break 'wait_for_exit;
                    }
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        panic!("unable to listen for OS signals");
    }
}

/// Helper for use in serde directives
#[inline(always)]
pub fn slice_is_empty<T>(s: &[T]) -> bool {
    s.is_empty()
}

/// Checks if a value is equal to its default.
/// This could be inefficient if default() is non-trivial. Should be used for things like
/// eliding serialization of zeroes and other default values with serde.
#[inline(always)]
pub fn is_default<V: Default + PartialEq>(v: &V) -> bool {
    V::default().eq(v)
}

/// Allocate and initialize a large array with a simple type.
/// This is a workaround for the fact that Box::new([ARRAY]) will overflow the stack if the
/// array is too large, a known issue with current Rust. It can go away when this is fixed.
/// None is returned if a memory allocation error occurs.
#[inline]
pub fn alloc_array<T: Copy, const N: usize>(initial_value: T) -> Option<Box<[T; N]>> {
    unsafe {
        let mem: *mut T = std::alloc::alloc(std::alloc::Layout::new::<[T; N]>()).cast();
        if mem.is_null() {
            return None;
        }
        for i in 0..N {
            mem.add(i).write(initial_value);
        }
        return Some(Box::from_raw(mem.cast()));
    }
}

/// Allocate and initialize a large array using a generator.
/// This is a workaround for the fact that Box::new([ARRAY]) will overflow the stack if the
/// array is too large, a known issue with current Rust. It can go away when this is fixed.
/// None is returned if a memory allocation error occurs.
#[inline]
pub fn alloc_array_with<T, F: FnMut(usize) -> T, const N: usize>(mut f: F) -> Option<Box<[T; N]>> {
    unsafe {
        let mem: *mut T = std::alloc::alloc(std::alloc::Layout::new::<[T; N]>()).cast();
        if mem.is_null() {
            return None;
        }
        for i in 0..N {
            mem.add(i).write(f(i));
        }
        return Some(Box::from_raw(mem.cast()));
    }
}

#[cold]
#[inline(never)]
pub extern "C" fn unlikely_branch() {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ms_monotonic;

    #[test]
    fn monotonic_clock_sanity_check() {
        let start = ms_monotonic();
        assert!(start >= 0);
        std::thread::sleep(Duration::from_millis(500));
        let end = ms_monotonic();
        // per docs:
        //
        // The thread may sleep longer than the duration specified due to scheduling specifics or
        // platform-dependent functionality. It will never sleep less.
        //
        assert!((end - start).abs() >= 500);
        assert!((end - start).abs() < 750);
    }
}
