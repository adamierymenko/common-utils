/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::mem::transmute;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct OnceArc<T> {
    /// A simple pointer for fast reads if non-null, otherwise falls through to atomic access.
    /// This is written in init_once() but may not be instantly visible everywhere, but in this
    /// case accesses still work via the atomic pointer.
    fast_ptr: *mut T,
    /// The real "authoritative" pointer is atomic.
    ptr: AtomicPtr<T>,
}

impl<T> OnceArc<T> {
    #[inline]
    pub fn new() -> Self {
        Self { fast_ptr: null_mut(), ptr: AtomicPtr::new(null_mut()) }
    }

    /// Initialize the value of this OnceArc.
    /// This will panic if it is called more than once.
    #[inline]
    pub fn init_once(&self, obj: Arc<T>) {
        let obj: *mut T = unsafe { transmute(Arc::into_raw(obj)) };
        if self
            .ptr
            .compare_exchange(null_mut(), obj, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            unsafe {
                std::ptr::write_volatile(transmute(&self.fast_ptr as *const *mut T), obj);
            }
        } else {
            panic!("OnceArc can only be initialized once");
        }
    }

    /// Load this value, or return None if not yet initialized.
    #[inline(always)]
    pub fn load(&self) -> Option<&T> {
        if !self.fast_ptr.is_null() {
            Some(unsafe { &*self.fast_ptr })
        } else {
            let ptr = self.ptr.load(Ordering::Acquire);
            if ptr.is_null() {
                None
            } else {
                Some(unsafe { &*ptr })
            }
        }
    }

    /// Load this value or busy wait (with a short delay) until available.
    /// This should be used when the value is expected to be either available or initialized
    /// pretty much immediately, such as in the case of two concurrent dependencies initializing
    /// each other.
    #[inline(always)]
    pub fn load_wait(&self) -> &T {
        if !self.fast_ptr.is_null() {
            return unsafe { &*self.fast_ptr };
        } else {
            loop {
                let ptr = self.ptr.load(Ordering::Acquire);
                if ptr.is_null() {
                    std::thread::sleep(Duration::from_millis(1));
                } else {
                    return unsafe { &*ptr };
                }
            }
        }
    }
}

unsafe impl<T> Sync for OnceArc<T> {}
unsafe impl<T> Send for OnceArc<T> where T: Send {}

impl<T> Drop for OnceArc<T> {
    fn drop(&mut self) {
        let obj = self.ptr.load(Ordering::Acquire);
        if !obj.is_null() {
            unsafe { drop(Arc::from_raw(obj)) };
        }
    }
}
