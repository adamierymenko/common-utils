/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::{mem::MaybeUninit, ops::Deref, sync::RwLockReadGuard};

#[allow(unused)]
pub struct MappedReadGuard<'a, S, T: ?Sized> {
    r: *const T,
    rg: RwLockReadGuard<'a, S>,
}

impl<'a, S, T: ?Sized> MappedReadGuard<'a, S, T> {
    /// Get a read guard that points to a field in an object another read guard locks.
    #[inline(always)]
    pub fn map<F: FnOnce(&S) -> &T>(rg: RwLockReadGuard<'a, S>, f: F) -> Self {
        #[allow(invalid_value)]
        let mut m = MappedReadGuard { r: unsafe { MaybeUninit::uninit().assume_init() }, rg };
        m.r = f(&m.rg);
        m
    }

    /// Get a read guard that points to a field in an object another read guard locks.
    #[inline(always)]
    pub fn maybe_map<F: FnOnce(&S) -> Option<&T>>(rg: RwLockReadGuard<'a, S>, f: F) -> Option<Self> {
        #[allow(invalid_value)]
        let mut m = MappedReadGuard { r: unsafe { MaybeUninit::uninit().assume_init() }, rg };
        if let Some(ptr) = f(&m.rg) {
            m.r = ptr;
            return Some(m);
        }
        return None;
    }
}

impl<'a, S, T: ?Sized> Deref for MappedReadGuard<'a, S, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.r }
    }
}

unsafe impl<'a, S, T: ?Sized> Send for MappedReadGuard<'a, S, T> where T: Send {}
unsafe impl<'a, S, T: ?Sized> Sync for MappedReadGuard<'a, S, T> where T: Sync {}
