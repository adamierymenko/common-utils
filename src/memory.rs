/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

// This is a collection of functions that use "unsafe" to do things with memory that should in fact
// be safe. Some of these may eventually get stable standard library replacements.

#[allow(unused_imports)]
use std::mem::{needs_drop, size_of, MaybeUninit};

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[allow(unused_imports)]
use std::ptr::copy_nonoverlapping;

/// Implement this trait to mark a struct as safe to cast (in place) from a byte array.
/// To be safe it must contain alignment-neutral objects which basically means bytes. For
/// integers they should be represented as byte arrays e.g. [u8; 4] for u32.
pub unsafe trait FlatBuffer: Sized {}

/// Our version of the not-yet-stable array_chunks method in slice.
#[inline(always)]
pub fn array_chunks_exact<T, const S: usize>(a: &[T]) -> impl Iterator<Item = &[T; S]> {
    let mut i = 0;
    let l = a.len();
    std::iter::from_fn(move || {
        let j = i + S;
        if j <= l {
            let next = unsafe { &*a.as_ptr().add(i).cast() };
            i = j;
            Some(next)
        } else {
            None
        }
    })
}

/// Obtain a view into an array cast as another array.
/// This will panic if the template parameters would result in out of bounds access.
#[inline(always)]
pub fn array_range<T, const S: usize, const START: usize, const LEN: usize>(a: &[T; S]) -> &[T; LEN] {
    assert!((START + LEN) <= S);
    unsafe { &*a.as_ptr().add(START).cast::<[T; LEN]>() }
}

/// Get a reference to a raw object as a byte array.
/// The template parameter S must be less than or equal to the size of the object in bytes or this will panic.
#[inline(always)]
pub fn as_byte_array<T: Copy, const S: usize>(o: &T) -> &[u8; S] {
    assert!(S <= size_of::<T>());
    unsafe { &*(o as *const T).cast() }
}

/// Get a reference to a raw object as a byte array.
/// The template parameter S must be less than or equal to the size of the object in bytes or this will panic.
#[inline(always)]
pub fn as_byte_array_mut<T: Copy, const S: usize>(o: &mut T) -> &mut [u8; S] {
    assert!(S <= size_of::<T>());
    unsafe { &mut *(o as *mut T).cast() }
}

/// Transmute an object to a byte array.
/// The template parameter S must equal the size of the object in bytes or this will panic.
#[inline(always)]
pub fn to_byte_array<T: Copy, const S: usize>(o: T) -> [u8; S] {
    assert_eq!(S, size_of::<T>());
    assert!(!std::mem::needs_drop::<T>());
    unsafe { *(&o as *const T).cast() }
}

/// Cast a byte slice into a flat struct.
/// This will panic if the slice is too small or the struct requires drop.
#[inline(always)]
pub fn cast_to_struct<T: FlatBuffer>(b: &[u8]) -> &T {
    assert!(b.len() >= size_of::<T>());
    assert!(!std::mem::needs_drop::<T>());
    unsafe { &*b.as_ptr().cast() }
}

/// The missing get-ip-octets-as-reference function for IpAddr
#[inline(always)]
pub fn ip_octets_ref(sa: &IpAddr) -> &[u8] {
    assert_eq!(size_of::<Ipv4Addr>(), 4);
    assert_eq!(size_of::<Ipv6Addr>(), 16);
    match sa {
        IpAddr::V4(ip4) => unsafe { &*(ip4 as *const Ipv4Addr).cast::<[u8; 4]>() },
        IpAddr::V6(ip6) => unsafe { &*(ip6 as *const Ipv6Addr).cast::<[u8; 16]>() },
    }
}
