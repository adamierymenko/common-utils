use std::alloc::{alloc, dealloc, Layout};
use std::io::Write;
use std::mem::size_of;
use std::ops::{Index, IndexMut, RangeBounds};
use std::ptr::{
    copy_nonoverlapping, drop_in_place, slice_from_raw_parts, slice_from_raw_parts_mut, write_bytes, NonNull,
};
use std::slice::SliceIndex;
use std::sync::Mutex;

const INDIVIDUAL_BUFFER_ALIGN: usize = size_of::<u32>();
const POOL_ALIGN: usize = size_of::<*mut u8>();
const BUFFER_HEADER_SIZE: usize = size_of::<u32>() * 2;
const BUF_HDR_CAPACITY_POOLED_FLAG: u32 = 0x80000000;

/// Thin buffer that can be allocated one by one or as part of a pool of contiguous memory.
///
/// Buffers can be created one by one or in bulk using Pool. Buffers may also be cloned, but
/// cloning a pooled buffer results in a one-off allocated buffer rather than a pooled one.
///
/// When a buffer is dropped it either deallocates or returns automatically to its pool
/// depending on how it was created.
///
/// Internally a Buf just consists of one pointer, making it a simple value with near zero
/// overhead to pass between functions.
#[repr(transparent)]
pub struct Buf(NonNull<u32>);

impl Buf {
    /// Maximum allowed capacity of an individual buffer.
    pub const MAX_CAPACITY: usize = 0x7fffffff; // must leave left-most bit as flag

    /// Allocate an individual buffer with the given capacity.
    /// Capacity must be less than MAX_CAPACITY or this panics.
    #[inline]
    pub fn new(buf_capacity: usize) -> Self {
        assert!(buf_capacity <= Buf::MAX_CAPACITY && buf_capacity > 0 && (buf_capacity % 8) == 0);
        unsafe {
            let b: *mut u32 = alloc(Layout::from_size_align_unchecked(
                buf_capacity + BUFFER_HEADER_SIZE,
                INDIVIDUAL_BUFFER_ALIGN,
            ))
            .cast();
            assert!(!b.is_null());
            *b = 0;
            *b.add(1) = buf_capacity as u32;
            Self(NonNull::new_unchecked(b))
        }
    }

    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = &u8> {
        self.as_slice().iter()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        unsafe { *self.0.as_ptr() as usize }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        unsafe { *self.0.as_ptr() == 0 }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        unsafe { (*self.0.as_ptr().add(1) & 0x7fffffff) as usize }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        unsafe { *self.0.as_ptr() = 0 };
    }
    /// Resize the buffer, writing `val` into any new indexes that were created.
    /// This will panic if `new_size` exceeds this buffer's capacity.
    #[inline]
    pub fn resize(&mut self, new_size: usize, val: u8) {
        assert!(new_size <= self.capacity());
        unsafe {
            let old_size = *self.0.as_ptr() as usize;
            *self.0.as_ptr() = new_size as u32;
            if new_size >= old_size {
                write_bytes(self.0.as_ptr().cast::<u8>().add(8 + old_size), val, new_size - old_size);
            }
        }
    }

    /// Clear buffer, resize, and fill with a value.
    /// This will panic if `new_size` exceeds this buffer's capacity.
    #[inline]
    pub fn clear_and_resize(&mut self, new_size: usize, val: u8) {
        assert!(new_size <= self.capacity());
        unsafe {
            *self.0.as_ptr() = new_size as u32;
            write_bytes(self.0.as_ptr().cast::<u8>().add(8), val, new_size);
        }
    }

    /// Set the size of data in this buffer without checking or initializing.
    /// This does not check that new_size does not exceed capacity or zero the content
    /// of the buffer, so it is unsafe.
    #[inline(always)]
    pub unsafe fn set_size(&mut self, new_size: usize) {
        *self.0.as_ptr() = new_size as u32;
    }

    /// Attempt to append a slice and return true on success or false if the slice is too big.
    /// This does the same thing as std::io::Write::write() but just returns a bool.
    /// The buffer will not have mutated if false is returned.
    #[inline]
    #[must_use]
    pub fn append(&mut self, buf: &[u8]) -> bool {
        let old_len = self.len();
        let new_len = old_len + buf.len();
        if new_len <= self.capacity() {
            unsafe {
                *self.0.as_ptr() = new_len as u32;
                copy_nonoverlapping(buf.as_ptr(), self.0.as_ptr().cast::<u8>().add(8 + old_len), buf.len())
            };
            true
        } else {
            false
        }
    }
    /// Attempt to append `val` to the buffer `num` times.
    /// Returns true on success or false if capacity is exceeded.
    /// The buffer will not have mutated if false is returned.
    #[inline]
    #[must_use]
    pub fn repeat(&mut self, num: usize, val: u8) -> bool {
        let old_len = self.len();
        let new_len = old_len + num;
        if new_len <= self.capacity() {
            unsafe {
                *self.0.as_ptr() = new_len as u32;
                write_bytes(self.0.as_ptr().cast::<u8>().add(8 + old_len), val, num);
            };
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    #[inline(always)]
    pub fn copy_within(&mut self, src: impl RangeBounds<usize>, dest: usize) {
        self.as_mut().copy_within(src, dest)
    }
}

impl<I: SliceIndex<[u8]>> Index<I> for Buf {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(self.as_ref(), index)
    }
}
impl<I: SliceIndex<[u8]>> IndexMut<I> for Buf {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(self.as_mut(), index)
    }
}

impl AsRef<[u8]> for Buf {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        unsafe { &*slice_from_raw_parts(self.0.as_ptr().cast::<u8>().add(8), *self.0.as_ptr() as usize) }
    }
}

impl AsMut<[u8]> for Buf {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *slice_from_raw_parts_mut(self.0.as_ptr().cast::<u8>().add(8), *self.0.as_ptr() as usize) }
    }
}

impl Write for Buf {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.append(buf) {
            Ok(buf.len())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "insufficient capacity",
            ))
        }
    }

    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for Buf {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let cap = *self.0.as_ptr().add(1);
            if (cap & BUF_HDR_CAPACITY_POOLED_FLAG) != 0 {
                self.clear();
                let pool_inner = &mut **self
                    .0
                    .as_ptr()
                    .cast::<u8>()
                    .sub(size_of::<*mut PoolInner>())
                    .cast::<*mut PoolInner>();
                let mut pool = pool_inner.pool.lock().unwrap();
                assert_ne!(pool.0, pool_inner.pool_end);
                *pool.0 = self.0.as_ptr();
                pool.0 = pool.0.add(1);
                if pool.0 == pool_inner.pool_end && pool.1 {
                    drop(pool);
                    PoolInner::dealloc(pool_inner);
                }
            } else {
                dealloc(
                    self.0.as_ptr().cast(),
                    Layout::from_size_align_unchecked((cap as usize) + BUFFER_HEADER_SIZE, INDIVIDUAL_BUFFER_ALIGN),
                );
            }
        }
    }
}

impl Clone for Buf {
    #[inline]
    fn clone(&self) -> Self {
        let mut c = Buf::new(self.capacity());
        let _ = c.append(self.as_slice());
        c
    }
}

impl PartialEq for Buf {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl Eq for Buf {}

unsafe impl Send for Buf {}
unsafe impl Sync for Buf {}

struct PoolInner {
    buf_capacity: usize,
    layout: Layout,
    pool: Mutex<(*mut *mut u32, bool)>, // pool cursor, dropped flag
    pool_start: *mut *mut u32,
    pool_end: *mut *mut u32,
}

impl PoolInner {
    /// Drop and deallocate PoolInner
    unsafe fn dealloc(self_ptr: *mut Self) {
        let layout = (*self_ptr).layout;
        drop_in_place(self_ptr);
        dealloc(self_ptr.cast(), layout)
    }
}

/// A thread-safe contiguous pool of Buf objects.
pub struct Pool(*mut PoolInner);

impl Pool {
    /// Allocate a pool of buffers as a single contiguous chunk of memory.
    ///
    /// * 'buf_capacity' - Capacity of each buffer, must be divisible by 8
    /// * 'pool_capacity' - Total number of buffers to allocate
    pub fn new(buf_capacity: usize, pool_capacity: usize) -> Self {
        assert!(buf_capacity <= Buf::MAX_CAPACITY && buf_capacity > 0 && (buf_capacity % 8) == 0 && pool_capacity > 0);
        unsafe {
            // Allocate memory for PoolInner followed by pool_capacity Buf objects with each prefixed
            // by a pointer back to PoolInner.
            let layout = Layout::from_size_align_unchecked(
                size_of::<PoolInner>()
                    + (size_of::<*mut u32>() * pool_capacity)
                    + ((buf_capacity + BUFFER_HEADER_SIZE + size_of::<*mut PoolInner>()) * pool_capacity),
                POOL_ALIGN,
            );
            let mem = alloc(layout).cast::<PoolInner>();
            assert!(!mem.is_null());

            // Array of pointers to buf objects starts immediately after PoolInner.
            let mut pool: *mut *mut u32 = mem.add(1).cast();
            let pool_end = pool.add(pool_capacity);

            // Initialize PoolInner at the start of congiuously allocated memory.
            std::ptr::write(
                mem,
                PoolInner {
                    buf_capacity,
                    layout,
                    pool: Mutex::new((pool_end, false)),
                    pool_start: pool,
                    pool_end,
                },
            );

            // Fill pool with pointers to where Buf objects would be. Bufs come right after the PoolInner
            // struct and the array of pool pointers themselves. Pooled bufs contain a pointer back to
            // their PoolInner parent structure right before the buffer header and buffer in memory. The
            // pooled flag (most significant bit in capacity field of header) tells them to return themselves
            // to the pool on drop instead of deallocating.
            let mut ptr: *mut u8 = pool_end.cast(); // actual buffers start after pool pointer array
            let buf_hdr_cap = BUF_HDR_CAPACITY_POOLED_FLAG | (buf_capacity as u32);
            let size_of_each_buf = buf_capacity + BUFFER_HEADER_SIZE + size_of::<*mut PoolInner>();
            while pool != pool_end {
                *ptr.cast::<*mut PoolInner>() = mem;
                let buf_start: *mut u32 = ptr.add(size_of::<*mut PoolInner>()).cast();
                ptr = ptr.add(size_of_each_buf);
                *buf_start = 0;
                *buf_start.add(1) = buf_hdr_cap;
                *pool = buf_start;
                pool = pool.add(1);
            }

            Self(mem.cast())
        }
    }

    /// Get the number of remaining free items in this pool.
    #[inline]
    pub fn pool_remaining(&self) -> usize {
        unsafe { (*self.0).pool.lock().unwrap().0.offset_from((*self.0).pool_start) as usize }
    }

    /// Get a buffer from the pool, or allocate a standalone buffer if the pool is empty.
    ///
    /// Buffers allocated from the pool will return themselves on drop, while standalone buffers
    /// will automatically free their memory.
    #[inline]
    pub fn get(&self) -> Buf {
        unsafe {
            let mut pool = (*self.0).pool.lock().unwrap();
            if pool.0 != (*self.0).pool_start {
                pool.0 = pool.0.sub(1);
                Buf(NonNull::new_unchecked(*pool.0))
            } else {
                Buf::new((*self.0).buf_capacity)
            }
        }
    }

    /// Get a buffer from the pool or direct allocation if min_capacity is larger than pool buffer capacity.
    #[inline]
    pub fn get_with_min_capacity(&self, min_capacity: usize) -> Buf {
        if unsafe { (*self.0).buf_capacity } >= min_capacity {
            self.get()
        } else {
            Buf::new(min_capacity)
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            let pool_inner = &mut *self.0;
            let mut pool = pool_inner.pool.lock().unwrap();
            if pool.0 == pool_inner.pool_end && pool.1 {
                drop(pool);
                PoolInner::dealloc(pool_inner);
            } else {
                // If all buffers haven't been returned yet, set a flag to cause the buffer's drop
                // method to deallocate the pool when this condition is met.
                pool.1 = true;
            }
        }
    }
}

unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn alloc_free() {
        let p = Pool::new(1024, 1024);
        assert_eq!(p.pool_remaining(), 1024);
        let mut buffers = Vec::new();
        for _ in 0..512 {
            buffers.push(p.get());
        }
        assert_eq!(p.pool_remaining(), 512);
        for _ in 0..512 {
            buffers.push(Buf::new(1024));
        }
        let bytes = [0x1; 1024];
        for b in buffers.iter_mut() {
            assert!(b.append(&bytes));
        }
        assert_eq!(p.pool_remaining(), 512);
        buffers.clear();
        assert_eq!(p.pool_remaining(), 1024);
        for _ in 0..2048 {
            buffers.push(p.get());
        }
        assert_eq!(p.pool_remaining(), 0);
        for b in buffers.iter_mut() {
            assert!(b.append(&bytes));
        }
        buffers.clear();
        assert_eq!(p.pool_remaining(), 1024);
    }
}
