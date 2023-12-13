/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::ops::Deref;

/// Container for an object that never dies, by design (leaks on drop)
///
/// This is used when you want an object that lives for the duration of a running process, but
/// don't want to use a static variable so that e.g. you can have more than one of them.
/// It's useful in cases such as async code with complex interdependencies where an Arc<> would
/// ordinarily be used but where using one leads to situations that would leak. Using this
/// instead explicitly documents in your code that the object in question is immortal and will
/// leak if dropped.
///
/// Semantics are similar to Arc<> in that only non-mutable references can be obtained and
/// the object can be cloned. These can also be copied, as they are just pointers.
#[derive(Clone, Copy)]
pub struct Immortal<T>(*mut T);

impl<T> Immortal<T> {
    #[inline(always)]
    pub fn new(obj: T) -> Self {
        Self(Box::into_raw(Box::new(obj)))
    }
}

impl<T> AsRef<T> for Immortal<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        unsafe { &*self.0 }
    }
}

impl<T> Deref for Immortal<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

unsafe impl<T> Sync for Immortal<T> where T: Sync {}
unsafe impl<T> Send for Immortal<T> where T: Send {}

// Unit tests generated with CodeLlama-Instruct-34B
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let immortal = Immortal::new(10);
        assert_eq!(*immortal.as_ref(), 10);
    }

    #[test]
    fn test_deref() {
        let immortal = Immortal::new(10);
        assert_eq!(*immortal, 10);
    }

    #[test]
    fn test_as_ref() {
        let immortal = Immortal::new(10);
        assert_eq!(*immortal.as_ref(), 10);
    }
}
