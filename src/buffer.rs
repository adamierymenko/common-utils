/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::error::Error;
use std::fmt::{Debug, Display};
use std::io::{Read, Write};
use std::mem::{size_of, MaybeUninit};

use crate::pool::PoolFactory;
use crate::unlikely_branch;

const OUT_OF_BOUNDS_MSG: &str = "Buffer access out of bounds";

pub struct OutOfBoundsError;

impl Display for OutOfBoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(OUT_OF_BOUNDS_MSG)
    }
}

impl Debug for OutOfBoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Error for OutOfBoundsError {}

impl From<OutOfBoundsError> for std::io::Error {
    fn from(_: OutOfBoundsError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, OUT_OF_BOUNDS_MSG)
    }
}

/// An I/O buffer with extensions for efficiently reading and writing various objects.
///
/// WARNING: Structures can only be handled through raw read/write here if they are
/// tagged a Copy, meaning they are safe to just copy as raw memory. Care must also
/// be taken to ensure that access to them is safe on architectures that do not support
/// unaligned access. In vl1/protocol.rs this is accomplished by only using byte arrays
/// (including for integers) and accessing via things like u64::from_be_bytes() etc.
///
/// Needless to say anything with non-Copy internal members or that depends on Drop to
/// not leak resources or other higher level semantics won't work here, but Rust should
/// not let you tag that as Copy in safe code.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Buffer<const L: usize>(usize, [u8; L]);

impl<const L: usize> Default for Buffer<L> {
    #[inline(always)]
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl<const L: usize> Buffer<L> {
    pub const CAPACITY: usize = L;

    /// Create an empty zeroed buffer.
    #[inline(always)]
    pub fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }

    /// Create an empty zeroed buffer on the heap without intermediate stack allocation.
    /// This can be used to allocate buffers too large for the stack.
    #[inline(always)]
    pub fn new_boxed() -> Box<Self> {
        unsafe { Box::from_raw(std::alloc::alloc_zeroed(std::alloc::Layout::new::<Self>()).cast()) }
    }

    /// Create an empty buffer without internally zeroing its memory.
    ///
    /// This is unsafe because unwritten memory in the buffer will have undefined contents.
    /// This means that some of the append_X_get_mut() functions may return mutable references to
    /// undefined memory contents rather than zeroed memory.
    #[inline(always)]
    pub unsafe fn new_without_memzero() -> Self {
        Self(0, MaybeUninit::uninit().assume_init())
    }

    pub const fn capacity(&self) -> usize {
        Self::CAPACITY
    }

    #[inline]
    pub fn from_bytes(b: &[u8]) -> Result<Self, OutOfBoundsError> {
        let l = b.len();
        if l <= L {
            let mut tmp = Self::new();
            tmp.0 = l;
            tmp.1[0..l].copy_from_slice(b);
            Ok(tmp)
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.1[..self.0]
    }

    #[inline(always)]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.1[..self.0]
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.1.as_ptr()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.1.as_mut_ptr()
    }

    #[inline(always)]
    pub fn as_bytes_after(&self, start: usize) -> Result<&[u8], OutOfBoundsError> {
        if start <= self.0 {
            Ok(&self.1[start..self.0])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn as_bytes_after_mut(&mut self, start: usize) -> Result<&mut [u8], OutOfBoundsError> {
        if start <= self.0 {
            Ok(&mut self.1[start..self.0])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn as_byte_range(&self, start: usize, end: usize) -> Result<&[u8], OutOfBoundsError> {
        if end <= self.0 {
            Ok(&self.1[start..end])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.1[0..self.0].fill(0);
        self.0 = 0;
    }

    /// Load array into buffer.
    /// This will panic if the array is larger than L.
    #[inline(always)]
    pub fn set_to(&mut self, b: &[u8]) {
        let len = b.len();
        self.0 = len;
        self.1[0..len].copy_from_slice(b);
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Set the size of this buffer's data.
    ///
    /// This will panic if the specified size is larger than L. If the size is larger
    /// than the current size uninitialized space will be zeroed.
    #[inline]
    pub fn set_size(&mut self, s: usize) {
        let prev_len = self.0;
        self.0 = s;
        if s > prev_len {
            self.1[prev_len..s].fill(0);
        }
    }

    /// Get a mutable reference to the entire buffer regardless of the current 'size'.
    #[inline(always)]
    pub unsafe fn entire_buffer_mut(&mut self) -> &mut [u8; L] {
        &mut self.1
    }

    /// Set the size of the data in this buffer without checking bounds or zeroing new space.
    #[inline(always)]
    pub unsafe fn set_size_unchecked(&mut self, s: usize) {
        self.0 = s;
    }

    /// Get a byte from this buffer without checking bounds.
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, i: usize) -> u8 {
        *self.1.get_unchecked(i)
    }

    /// Append a structure and return a mutable reference to its memory.
    #[inline(always)]
    pub fn append_struct_get_mut<T: Copy>(&mut self) -> Result<&mut T, OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + size_of::<T>();
        if end <= L {
            self.0 = end;
            Ok(unsafe { &mut *self.1.as_mut_ptr().add(ptr).cast() })
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    /// Append a fixed size array and return a mutable reference to its memory.
    #[inline(always)]
    pub fn append_bytes_fixed_get_mut<const S: usize>(&mut self) -> Result<&mut [u8; S], OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + S;
        if end <= L {
            self.0 = end;
            Ok(unsafe { &mut *self.1.as_mut_ptr().add(ptr).cast() })
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    /// Append a runtime sized array and return a mutable reference to its memory.
    #[inline(always)]
    pub fn append_bytes_get_mut(&mut self, s: usize) -> Result<&mut [u8], OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + s;
        if end <= L {
            self.0 = end;
            Ok(&mut self.1[ptr..end])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_padding(&mut self, b: u8, count: usize) -> Result<(), OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + count;
        if end <= L {
            self.0 = end;
            self.1[ptr..end].fill(b);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_bytes(&mut self, buf: &[u8]) -> Result<(), OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + buf.len();
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(buf);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_bytes_fixed<const S: usize>(&mut self, buf: &[u8; S]) -> Result<(), OutOfBoundsError> {
        let ptr = self.0;
        let end = ptr + S;
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(buf);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_u8(&mut self, i: u8) -> Result<(), OutOfBoundsError> {
        let ptr = self.0;
        if ptr < L {
            self.0 = ptr + 1;
            self.1[ptr] = i;
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_u16(&mut self, i: u16) -> Result<(), OutOfBoundsError> {
        let i = i.to_be_bytes();
        let ptr = self.0;
        let end = ptr + 2;
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(&i);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_u32(&mut self, i: u32) -> Result<(), OutOfBoundsError> {
        let i = i.to_be_bytes();
        let ptr = self.0;
        let end = ptr + 4;
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(&i);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn append_u64(&mut self, i: u64) -> Result<(), OutOfBoundsError> {
        let i = i.to_be_bytes();
        let ptr = self.0;
        let end = ptr + 8;
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(&i);
            Ok(())
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn bytes_fixed_at<const S: usize>(&self, ptr: usize) -> Result<&[u8; S], OutOfBoundsError> {
        if (ptr + S) <= self.0 {
            unsafe { Ok(&*self.1.as_ptr().cast::<u8>().add(ptr).cast::<[u8; S]>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn bytes_fixed_at_mut<const S: usize>(&mut self, ptr: usize) -> Result<&mut [u8; S], OutOfBoundsError> {
        if (ptr + S) <= self.0 {
            unsafe { Ok(&mut *self.1.as_mut_ptr().cast::<u8>().add(ptr).cast::<[u8; S]>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn struct_at<T: Copy>(&self, ptr: usize) -> Result<&T, OutOfBoundsError> {
        if (ptr + size_of::<T>()) <= self.0 {
            unsafe { Ok(&*self.1.as_ptr().cast::<u8>().add(ptr).cast::<T>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn struct_mut_at<T: Copy>(&mut self, ptr: usize) -> Result<&mut T, OutOfBoundsError> {
        if (ptr + size_of::<T>()) <= self.0 {
            unsafe { Ok(&mut *self.1.as_mut_ptr().cast::<u8>().add(ptr).cast::<T>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn u8_at(&self, ptr: usize) -> Result<u8, OutOfBoundsError> {
        if ptr < self.0 {
            Ok(self.1[ptr])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn u16_at(&self, ptr: usize) -> Result<u16, OutOfBoundsError> {
        let end = ptr + 2;
        debug_assert!(end <= L);
        if end <= self.0 {
            Ok(u16::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 2]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn u32_at(&self, ptr: usize) -> Result<u32, OutOfBoundsError> {
        let end = ptr + 4;
        debug_assert!(end <= L);
        if end <= self.0 {
            Ok(u32::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 4]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn u64_at(&self, ptr: usize) -> Result<u64, OutOfBoundsError> {
        let end = ptr + 8;
        debug_assert!(end <= L);
        if end <= self.0 {
            Ok(u64::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 8]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_struct<T: Copy>(&self, cursor: &mut usize) -> Result<&T, OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + size_of::<T>();
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            unsafe { Ok(&*self.1.as_ptr().cast::<u8>().add(ptr).cast::<T>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_bytes_fixed<const S: usize>(&self, cursor: &mut usize) -> Result<&[u8; S], OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + S;
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            unsafe { Ok(&*self.1.as_ptr().cast::<u8>().add(ptr).cast::<[u8; S]>()) }
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_bytes(&self, l: usize, cursor: &mut usize) -> Result<&[u8], OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + l;
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            Ok(&self.1[ptr..end])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_u8(&self, cursor: &mut usize) -> Result<u8, OutOfBoundsError> {
        let ptr = *cursor;
        debug_assert!(ptr < L);
        if ptr < self.0 {
            *cursor = ptr + 1;
            Ok(self.1[ptr])
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_u16(&self, cursor: &mut usize) -> Result<u16, OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + 2;
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            Ok(u16::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 2]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_u32(&self, cursor: &mut usize) -> Result<u32, OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + 4;
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            Ok(u32::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 4]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }

    #[inline(always)]
    pub fn read_u64(&self, cursor: &mut usize) -> Result<u64, OutOfBoundsError> {
        let ptr = *cursor;
        let end = ptr + 8;
        debug_assert!(end <= L);
        if end <= self.0 {
            *cursor = end;
            Ok(u64::from_be_bytes(unsafe {
                *self.1.as_ptr().add(ptr).cast::<[u8; 8]>()
            }))
        } else {
            unlikely_branch();
            Err(OutOfBoundsError)
        }
    }
}

impl<const L: usize> Write for Buffer<L> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let ptr = self.0;
        let end = ptr + buf.len();
        if end <= L {
            self.0 = end;
            self.1[ptr..end].copy_from_slice(buf);
            Ok(buf.len())
        } else {
            unlikely_branch();
            Err(std::io::Error::new(std::io::ErrorKind::Other, OUT_OF_BOUNDS_MSG))
        }
    }

    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<const L: usize> AsRef<[u8]> for Buffer<L> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const L: usize> AsMut<[u8]> for Buffer<L> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_bytes_mut()
    }
}

impl<const L: usize> From<[u8; L]> for Buffer<L> {
    #[inline(always)]
    fn from(a: [u8; L]) -> Self {
        Self(L, a)
    }
}

impl<const L: usize> From<&[u8; L]> for Buffer<L> {
    #[inline(always)]
    fn from(a: &[u8; L]) -> Self {
        Self(L, *a)
    }
}

/// Implements std::io::Read for a buffer and a cursor.
pub struct BufferReader<'a, 'b, const L: usize>(&'a Buffer<L>, &'b mut usize);

impl<'a, 'b, const L: usize> BufferReader<'a, 'b, L> {
    #[inline(always)]
    pub fn new(b: &'a Buffer<L>, cursor: &'b mut usize) -> Self {
        Self(b, cursor)
    }
}

impl<'a, 'b, const L: usize> Read for BufferReader<'a, 'b, L> {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        buf.copy_from_slice(self.0.read_bytes(buf.len(), self.1)?);
        Ok(buf.len())
    }
}

pub struct PooledBufferFactory<const L: usize>;

impl<const L: usize> PooledBufferFactory<L> {
    #[inline(always)]
    pub fn new() -> Self {
        Self {}
    }
}

impl<const L: usize> PoolFactory<Buffer<L>> for PooledBufferFactory<L> {
    #[inline(always)]
    fn create(&self) -> Buffer<L> {
        Buffer::new()
    }

    #[inline(always)]
    fn reset(&self, obj: &mut Buffer<L>) {
        obj.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Buffer;

    #[test]
    fn buffer_basic_u64() {
        let mut b = Buffer::<8>::new();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
        assert!(b.append_u64(1234).is_ok());
        assert_eq!(b.len(), 8);
        assert!(!b.is_empty());
        assert_eq!(b.read_u64(&mut 0).unwrap(), 1234);
        b.clear();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
    }

    #[test]
    fn buffer_basic_u32() {
        let mut b = Buffer::<4>::new();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
        assert!(b.append_u32(1234).is_ok());
        assert_eq!(b.len(), 4);
        assert!(!b.is_empty());
        assert_eq!(b.read_u32(&mut 0).unwrap(), 1234);
        b.clear();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
    }

    #[test]
    fn buffer_basic_u16() {
        let mut b = Buffer::<2>::new();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
        assert!(b.append_u16(1234).is_ok());
        assert_eq!(b.len(), 2);
        assert!(!b.is_empty());
        assert_eq!(b.read_u16(&mut 0).unwrap(), 1234);
        b.clear();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
    }

    #[test]
    fn buffer_basic_u8() {
        let mut b = Buffer::<1>::new();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
        assert!(b.append_u8(128).is_ok());
        assert_eq!(b.len(), 1);
        assert!(!b.is_empty());
        assert_eq!(b.read_u8(&mut 0).unwrap(), 128);
        b.clear();
        assert_eq!(b.len(), 0);
        assert!(b.is_empty());
    }

    #[test]
    fn buffer_sizing() {
        const SIZE: usize = 100;

        for _ in 0..1000 {
            let v = [0u8; SIZE];
            let mut b = Buffer::<SIZE>::new();
            assert!(b.append_bytes(&v).is_ok());
            assert_eq!(b.len(), SIZE);
            b.set_size(10);
            assert_eq!(b.len(), 10);
            unsafe {
                b.set_size_unchecked(8675309);
            }
            assert_eq!(b.len(), 8675309);
        }
    }
}
