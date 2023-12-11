/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::fmt::Debug;
use std::io::Write;
use std::mem::{needs_drop, size_of, MaybeUninit};
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};

use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug)]
pub struct OutOfCapacityError<T>(pub T);

impl<T> std::fmt::Display for OutOfCapacityError<T> {
    fn fmt(&self, stream: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt("ArrayVec out of space", stream)
    }
}

impl<T: std::fmt::Debug> ::std::error::Error for OutOfCapacityError<T> {
    fn description(&self) -> &str {
        "ArrayVec out of space"
    }
}

/// A simple vector backed by a static sized array with no memory allocations and no overhead construction.
pub struct ArrayVec<T, const C: usize> {
    pub(crate) s: usize,
    pub(crate) a: [MaybeUninit<T>; C],
}

impl<T, const C: usize> Default for ArrayVec<T, C> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, const C: usize> Clone for ArrayVec<T, C> {
    #[inline]
    fn clone(&self) -> Self {
        debug_assert!(self.s <= C);
        Self {
            s: self.s,
            a: unsafe {
                let mut tmp: [MaybeUninit<T>; C] = MaybeUninit::uninit().assume_init();
                for i in 0..self.s {
                    tmp.get_unchecked_mut(i).write(self.a[i].assume_init_ref().clone());
                }
                tmp
            },
        }
    }
}

impl<T: Clone, const C: usize, const S: usize> From<[T; S]> for ArrayVec<T, C> {
    #[inline]
    fn from(v: [T; S]) -> Self {
        if S <= C {
            let mut tmp = Self::new();
            for i in 0..S {
                tmp.push(v[i].clone());
            }
            tmp
        } else {
            panic!();
        }
    }
}

impl<const C: usize> ToString for ArrayVec<u8, C> {
    #[inline]
    fn to_string(&self) -> String {
        crate::hex::to_string(self.as_bytes())
    }
}

impl<const C: usize> Write for ArrayVec<u8, C> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for i in buf.iter() {
            if self.try_push(*i).is_err() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "ArrayVec out of space"));
            }
        }
        Ok(buf.len())
    }

    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<T, const C: usize> TryFrom<Vec<T>> for ArrayVec<T, C> {
    type Error = OutOfCapacityError<T>;

    #[inline(always)]
    fn try_from(mut value: Vec<T>) -> Result<Self, Self::Error> {
        let mut tmp = Self::new();
        for x in value.drain(..) {
            tmp.try_push(x)?;
        }
        Ok(tmp)
    }
}

impl<T: Clone, const C: usize> TryFrom<&Vec<T>> for ArrayVec<T, C> {
    type Error = OutOfCapacityError<T>;

    #[inline(always)]
    fn try_from(value: &Vec<T>) -> Result<Self, Self::Error> {
        let mut tmp = Self::new();
        for x in value.iter() {
            tmp.try_push(x.clone())?;
        }
        Ok(tmp)
    }
}

impl<T: Clone, const C: usize> TryFrom<&[T]> for ArrayVec<T, C> {
    type Error = OutOfCapacityError<T>;

    #[inline(always)]
    fn try_from(value: &[T]) -> Result<Self, Self::Error> {
        let mut tmp = Self::new();
        for x in value.iter() {
            tmp.try_push(x.clone())?;
        }
        Ok(tmp)
    }
}

impl<T, const C: usize> ArrayVec<T, C> {
    #[inline(always)]
    pub fn new() -> Self {
        assert_eq!(size_of::<[T; C]>(), size_of::<[MaybeUninit<T>; C]>());
        Self { s: 0, a: unsafe { MaybeUninit::uninit().assume_init() } }
    }

    #[inline]
    pub fn push(&mut self, v: T) {
        let i = self.s;
        if i < C {
            unsafe { self.a.get_unchecked_mut(i).write(v) };
            self.s = i + 1;
        } else {
            panic!();
        }
    }

    #[inline]
    pub fn try_push(&mut self, v: T) -> Result<(), OutOfCapacityError<T>> {
        if self.s < C {
            let i = self.s;
            unsafe { self.a.get_unchecked_mut(i).write(v) };
            self.s = i + 1;
            Ok(())
        } else {
            Err(OutOfCapacityError(v))
        }
    }

    /// Get a raw byte slice view of the contents of this vector.
    /// This is only available for Copy types and will panic if the type needs_drop().
    #[inline(always)]
    pub fn as_bytes(&self) -> &[T]
    where
        T: Copy,
    {
        assert!(!std::mem::needs_drop::<T>());
        unsafe { &*slice_from_raw_parts(self.a.as_ptr().cast(), self.s) }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.s == 0
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.s
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        C
    }

    #[inline(always)]
    pub fn capacity_remaining(&self) -> usize {
        C - self.s
    }

    #[inline(always)]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.as_ref().iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.as_mut().iter_mut()
    }

    #[inline(always)]
    pub fn first(&self) -> Option<&T> {
        if self.s != 0 {
            Some(unsafe { self.a.get_unchecked(0).assume_init_ref() })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn last(&self) -> Option<&T> {
        if self.s != 0 {
            Some(unsafe { self.a.get_unchecked(self.s - 1).assume_init_ref() })
        } else {
            None
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.s > 0 {
            let i = self.s - 1;
            debug_assert!(i < C);
            self.s = i;
            Some(unsafe { self.a.get_unchecked(i).assume_init_read() })
        } else {
            None
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        if needs_drop::<T>() {
            for i in 0..self.s {
                unsafe { self.a.get_unchecked_mut(i).assume_init_drop() };
            }
        }
        self.s = 0;
    }

    #[inline]
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.as_mut().sort();
    }

    #[inline]
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.as_mut().sort_unstable();
    }
}

impl<T, const C: usize> ArrayVec<T, C>
where
    T: Copy,
{
    /// Push a slice of copyable objects, panic if capacity exceeded.
    #[inline]
    pub fn push_slice(&mut self, v: &[T]) {
        let start = self.s;
        let end = self.s + v.len();
        if end <= C {
            for i in start..end {
                unsafe { self.a.get_unchecked_mut(i).write(*v.get_unchecked(i - start)) };
            }
            self.s = end;
        } else {
            panic!();
        }
    }
}

impl<T, const C: usize> Drop for ArrayVec<T, C> {
    #[inline(always)]
    fn drop(&mut self) {
        if needs_drop::<T>() {
            for i in 0..self.s {
                unsafe { self.a.get_unchecked_mut(i).assume_init_drop() };
            }
        }
    }
}

impl<T, const C: usize> AsRef<[T]> for ArrayVec<T, C> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        unsafe { &*slice_from_raw_parts(self.a.as_ptr().cast(), self.s) }
    }
}

impl<T, const C: usize> AsMut<[T]> for ArrayVec<T, C> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { &mut *slice_from_raw_parts_mut(self.a.as_mut_ptr().cast(), self.s) }
    }
}

impl<T, const C: usize> PartialEq for ArrayVec<T, C>
where
    T: PartialEq,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        let tmp: &[T] = self.as_ref();
        tmp.eq(other.as_ref())
    }
}

impl<T, const C: usize> Eq for ArrayVec<T, C> where T: Eq {}

impl<T, const L: usize> PartialOrd for ArrayVec<T, L>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T, const L: usize> Ord for ArrayVec<T, L>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T, const L: usize> Debug for ArrayVec<T, L>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for x in self.iter() {
            x.fmt(f)?;
        }
        f.write_str("]")
    }
}

impl<T, const L: usize> Serialize for ArrayVec<T, L>
where
    T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        let sl: &[T] = self.as_ref();
        for i in 0..self.s {
            seq.serialize_element(&sl[i])?;
        }
        seq.end()
    }
}

struct ArrayVecVisitor<'de, T: Deserialize<'de>, const L: usize>(std::marker::PhantomData<&'de T>);

impl<'de, T, const L: usize> serde::de::Visitor<'de> for ArrayVecVisitor<'de, T, L>
where
    T: Deserialize<'de>,
{
    type Value = ArrayVec<T, L>;

    #[inline]
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(format!("up to {} elements", L).as_str())
    }

    #[inline]
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut a = ArrayVec::<T, L>::new();
        while let Some(x) = seq.next_element()? {
            if !a.try_push(x).is_ok() {
                return Err(serde::de::Error::custom("capacity exceeded"));
            }
        }
        return Ok(a);
    }
}

impl<'de, T: Deserialize<'de> + 'de, const L: usize> Deserialize<'de> for ArrayVec<T, L>
where
    T: Deserialize<'de>,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<ArrayVec<T, L>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ArrayVecVisitor(std::marker::PhantomData::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::ArrayVec;

    #[test]
    fn popability() {
        let mut v = ArrayVec::<usize, 128>::new();
        for i in 0..128 {
            v.push(i);
        }
        assert_eq!(v.len(), 128);
        for _ in 0..128 {
            assert!(v.pop().is_some());
        }
        assert!(v.pop().is_none());
    }
    #[test]
    fn bounds() {
        let mut v = ArrayVec::<usize, 128>::new();
        for i in 0..128 {
            v.push(i);
        }
        assert_eq!(v.len(), 128);
        assert!(v.try_push(1000).is_err());
        assert_eq!(v.len(), 128);
    }
    #[test]
    fn clear() {
        let mut v = ArrayVec::<usize, 128>::new();
        for i in 0..128 {
            v.push(i);
        }
        assert_eq!(v.len(), 128);
        v.clear();
        assert!(v.pop().is_none());
    }
    #[test]
    fn order() {
        let mut v = ArrayVec::<usize, 128>::new();
        for i in 0..128 {
            v.push(i);
        }
        assert_eq!(v.len(), 128);

        assert!(v.first() == Some(&0));
        assert!(v.last() == Some(&127));

        let size: usize = 128;
        for i in 0..size {
            let popped_val = v.pop();
            assert!(popped_val.is_some());
            assert!(popped_val == Some((size - 1) - i));
        }
    }
}
