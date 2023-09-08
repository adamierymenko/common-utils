use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::io::Write;
use std::mem::size_of;
use std::ptr::{copy_nonoverlapping, slice_from_raw_parts, slice_from_raw_parts_mut};
use std::sync::Mutex;

/// Thin buffer represented as just one pointer, with bulk allocation capability.
///
/// Buffers can be created one by one or in bulk using Pool. Buffers may also be cloned, but
/// cloning a pooled buffer results in a one-off allocated buffer rather than a pooled one.
#[repr(transparent)]
pub struct Buf(*mut u32);

impl Buf {
    /// Maximum allowed capacity of an individual buffer.
    pub const MAX_CAPACITY: usize = 0x7fffffff; // must leave left-most bit as flag

    /// Create a new buffer with the given maximum capacity.
    /// Capacity must be less than MAX_CAPACITY or this panics.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity <= Self::MAX_CAPACITY);
        unsafe {
            let b: *mut u32 = alloc_zeroed(Layout::from_size_align_unchecked(capacity + 8, 4)).cast();
            assert!(!b.is_null());
            *b.add(1) = capacity as u32;
            Self(b)
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        unsafe { *self.0 = 0 };
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        unsafe { *self.0 as usize }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        unsafe { (*self.0.add(1) & 0x7fffffff) as usize }
    }

    /// Attempt to append a slice and return true on success or false if the slice is too big.
    /// This does the same thing as std::io::Write::write() but just returns a bool.
    #[inline]
    #[must_use]
    pub fn append(&mut self, buf: &[u8]) -> bool {
        let new_len = self.len() + buf.len();
        if new_len <= self.capacity() {
            unsafe {
                *self.0 = new_len as u32;
                copy_nonoverlapping(buf.as_ptr(), self.0.cast::<u8>().add(8), buf.len())
            };
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { &*slice_from_raw_parts(self.0.cast::<u8>().add(8), *self.0 as usize) }
    }
}

impl AsRef<[u8]> for Buf {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsMut<[u8]> for Buf {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *slice_from_raw_parts_mut(self.0.cast::<u8>().add(8), *self.0 as usize) }
    }
}

impl Write for Buf {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let new_len = self.len() + buf.len();
        if new_len <= self.capacity() {
            unsafe {
                *self.0 = new_len as u32;
                copy_nonoverlapping(buf.as_ptr(), self.0.cast::<u8>().add(8), buf.len())
            };
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
            let cap = *self.0.add(1);
            if cap.wrapping_shr(31) == 1 {
                let pool_inner = &mut *self
                    .0
                    .cast::<u8>()
                    .offset(-(size_of::<*mut PoolInner>() as isize))
                    .cast::<PoolInner>();
                let mut pool = pool_inner.pool.lock().unwrap();
                pool.0.push(self.0);
                if pool.1 && pool.0.len() == pool_inner.pool_capacity {
                    drop(pool);
                    pool_inner.dealloc();
                }
            } else {
                dealloc(self.0.cast(), Layout::from_size_align_unchecked((cap as usize) + 8, 4));
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

struct PoolInner {
    buf_capacity: usize,
    pool_capacity: usize,
    pool: Mutex<(Vec<*mut u32>, bool)>, // pool, dropped flag
}

fn calc_pool_mem(buf_capacity: usize, pool_capacity: usize) -> usize {
    assert_eq!((buf_capacity % 8), 0);
    size_of::<PoolInner>() + ((buf_capacity + 8 + size_of::<*mut PoolInner>()) * pool_capacity)
}

impl PoolInner {
    unsafe fn dealloc(&mut self) {
        dealloc(
            (self as *mut Self).cast(),
            Layout::from_size_align_unchecked(calc_pool_mem(self.buf_capacity, self.pool_capacity), 8),
        )
    }
}

/// A thread-safe contiguous pool of Buf objects.
pub struct Pool(*mut PoolInner);

impl Pool {
    /// Create a new buffer pool from a single contiguous allocation
    ///
    /// * 'buf_capacity' - Capacity of each buffer, must be divisible by 8
    /// * 'pool_capacity' - Total number of buffers to allocate
    pub fn new(buf_capacity: usize, pool_capacity: usize) -> Self {
        assert!(buf_capacity <= Buf::MAX_CAPACITY);
        unsafe {
            // Allocate memory for PoolInner followed by pool_capacity Buf objects with each prefixed
            // by a pointer back to PoolInner.
            let mem = alloc_zeroed(Layout::from_size_align_unchecked(
                calc_pool_mem(buf_capacity, pool_capacity),
                8,
            ))
            .cast::<PoolInner>();
            assert!(!mem.is_null());

            // Initialize PoolInner
            std::ptr::write(
                mem,
                PoolInner {
                    buf_capacity,
                    pool_capacity,
                    pool: Mutex::new((Vec::with_capacity(pool_capacity), false)),
                },
            );

            // Fill pool with Buf objects initialized from contiguous memory after PoolInner struct.
            let mut pool = (&mut *mem).pool.lock().unwrap();
            let mut buf_ptr: *mut u8 = mem.add(1).cast();
            let buf_size = buf_capacity + 8 + size_of::<usize>();
            for _ in 0..pool_capacity {
                *buf_ptr.cast::<*mut PoolInner>() = mem;
                *buf_ptr.add(size_of::<*mut PoolInner>() + 4).cast::<u32>() = 0x80000000 | (buf_capacity as u32);
                pool.0.push(buf_ptr.add(size_of::<*mut PoolInner>()).cast());
                buf_ptr = buf_ptr.add(buf_size);
            }
            drop(pool);

            Self(mem.cast())
        }
    }

    /// Get a buffer from the pool, or allocate a standalone buffer if the pool is empty.
    ///
    /// Buffers allocated from the pool will return themselves on drop, while standalone buffers
    /// will automatically free their memory.
    ///
    /// A pool's buffer memory is not freed until all buffers returned by get() have been
    /// dropped. Dropping a pool flags it to be dropped when the last Buf is returned but memory
    /// isn't actually released until all Buf objects are no longer in use.
    #[inline]
    pub fn get(&self) -> Buf {
        if let Some(b) = unsafe { (&mut *self.0).pool.lock().unwrap().0.pop() } {
            Buf(b)
        } else {
            Buf::new(unsafe { (&*self.0).buf_capacity })
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            let pool_inner = &mut *self.0;
            let mut pool = pool_inner.pool.lock().unwrap();
            if pool.0.len() == pool_inner.pool_capacity && pool.1 {
                drop(pool);
                pool_inner.dealloc();
            } else {
                pool.1 = true; // causes dealloc to happen when last buf returned
            }
        }
    }
}

unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}
