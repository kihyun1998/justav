use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};

/// A reference-counted, optionally shared byte buffer.
///
/// Cloning a `Buffer` creates a new reference to the same underlying data
/// (cheap, O(1)). The data is shared and immutable while multiple references
/// exist. Use [`make_writable`](Buffer::make_writable) to get exclusive
/// mutable access — it will copy-on-write if there are other references.
#[derive(Clone)]
pub struct Buffer {
    inner: Arc<Vec<u8>>,
}

impl Buffer {
    /// Allocate a new buffer of `size` bytes, initialized to zero.
    pub fn alloc(size: usize) -> Result<Self> {
        if size == 0 {
            return Ok(Self {
                inner: Arc::new(Vec::new()),
            });
        }
        Ok(Self {
            inner: Arc::new(vec![0u8; size]),
        })
    }

    /// Create a buffer from existing data (takes ownership).
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self {
            inner: Arc::new(data),
        }
    }

    /// Create a buffer from a byte slice (copies the data).
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            inner: Arc::new(data.to_vec()),
        }
    }

    /// Returns the byte length of the buffer.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the buffer has zero length.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns an immutable reference to the underlying bytes.
    pub fn data(&self) -> &[u8] {
        &self.inner
    }

    /// Returns true if this is the only reference (safe to mutate in-place).
    pub fn is_writable(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
    }

    /// Returns the number of active references to this buffer.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Ensure exclusive ownership. If other references exist, the data is
    /// copied into a new allocation. Returns a mutable slice to the data.
    pub fn make_writable(&mut self) -> &mut [u8] {
        Arc::make_mut(&mut self.inner).as_mut_slice()
    }

    /// Resize the buffer. If growing, new bytes are zeroed.
    /// If other references exist, the data is copied first.
    pub fn resize(&mut self, new_size: usize) {
        Arc::make_mut(&mut self.inner).resize(new_size, 0);
    }
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("len", &self.len())
            .field("refs", &self.ref_count())
            .finish()
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.data()
    }
}

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.data() == other.data()
    }
}

impl Eq for Buffer {}

// ────────────────────────────────────────────────────────────────────────────
// BufferPool
// ────────────────────────────────────────────────────────────────────────────

/// A thread-safe pool of reusable buffers.
///
/// Avoids repeated allocation by keeping a cache of previously used buffers.
/// When a buffer is requested, the pool returns a cached one if available
/// (and the correct size), otherwise allocates a new one.
pub struct BufferPool {
    pool: Mutex<Vec<Vec<u8>>>,
    buf_size: usize,
    max_count: usize,
}

impl BufferPool {
    /// Create a new pool that hands out buffers of `buf_size` bytes.
    /// At most `max_count` buffers are cached; excess are dropped.
    pub fn new(buf_size: usize, max_count: usize) -> Result<Self> {
        if max_count == 0 {
            return Err(Error::InvalidArgument("max_count must be > 0".into()));
        }
        Ok(Self {
            pool: Mutex::new(Vec::with_capacity(max_count)),
            buf_size,
            max_count,
        })
    }

    /// Get a buffer from the pool. Returns a cached buffer reset to zeros if
    /// available, otherwise allocates a new one.
    pub fn get(&self) -> Result<Buffer> {
        let mut pool = self.pool.lock().map_err(|_| Error::InvalidState("pool lock poisoned".into()))?;
        if let Some(mut vec) = pool.pop() {
            vec.fill(0);
            Ok(Buffer::from_vec(vec))
        } else {
            Buffer::alloc(self.buf_size)
        }
    }

    /// Return a buffer's underlying storage to the pool for reuse.
    ///
    /// The buffer must be the sole owner (ref_count == 1) and the correct
    /// size; otherwise it is silently dropped.
    pub fn put(&self, mut buf: Buffer) {
        if buf.ref_count() != 1 || buf.len() != self.buf_size {
            return; // Cannot reclaim shared or wrong-sized buffers.
        }
        // Take ownership of the inner Vec.
        let vec = Arc::make_mut(&mut buf.inner);
        let vec = std::mem::take(vec);

        if let Ok(mut pool) = self.pool.lock() && pool.len() < self.max_count {
            pool.push(vec);
        }
    }

    /// Returns the number of buffers currently cached in the pool.
    pub fn cached_count(&self) -> usize {
        self.pool.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// The size of buffers this pool hands out.
    pub fn buffer_size(&self) -> usize {
        self.buf_size
    }
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferPool")
            .field("buf_size", &self.buf_size)
            .field("max_count", &self.max_count)
            .field("cached", &self.cached_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════
    // Buffer tests
    // ═══════════════════════════════════════════════════

    // ── Positive ──

    #[test]
    fn alloc_zeroed() {
        let buf = Buffer::alloc(16).unwrap();
        assert_eq!(buf.len(), 16);
        assert!(buf.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn from_vec() {
        let buf = Buffer::from_vec(vec![1, 2, 3]);
        assert_eq!(buf.data(), &[1, 2, 3]);
    }

    #[test]
    fn from_slice() {
        let buf = Buffer::from_slice(&[10, 20]);
        assert_eq!(buf.data(), &[10, 20]);
    }

    #[test]
    fn clone_shares_data() {
        let a = Buffer::from_vec(vec![1, 2, 3]);
        let b = a.clone();
        assert_eq!(a.data(), b.data());
        assert_eq!(a.ref_count(), 2);
        assert_eq!(b.ref_count(), 2);
    }

    #[test]
    fn is_writable_single_ref() {
        let buf = Buffer::from_vec(vec![1]);
        assert!(buf.is_writable());
    }

    #[test]
    fn not_writable_after_clone() {
        let a = Buffer::from_vec(vec![1]);
        let _b = a.clone();
        assert!(!a.is_writable());
    }

    #[test]
    fn make_writable_single_ref_no_copy() {
        let mut buf = Buffer::from_vec(vec![1, 2, 3]);
        let ptr_before = buf.data().as_ptr();
        let data = buf.make_writable();
        data[0] = 99;
        // Same allocation (no copy needed).
        assert_eq!(buf.data().as_ptr(), ptr_before);
        assert_eq!(buf.data()[0], 99);
    }

    #[test]
    fn make_writable_cow_on_shared() {
        let a = Buffer::from_vec(vec![1, 2, 3]);
        let mut b = a.clone();
        assert_eq!(a.ref_count(), 2);

        // b gets its own copy.
        let data = b.make_writable();
        data[0] = 99;

        assert_eq!(a.data(), &[1, 2, 3]); // a unchanged
        assert_eq!(b.data(), &[99, 2, 3]); // b modified
        assert!(b.is_writable());
    }

    #[test]
    fn resize_grow() {
        let mut buf = Buffer::from_vec(vec![1, 2]);
        buf.resize(4);
        assert_eq!(buf.data(), &[1, 2, 0, 0]);
    }

    #[test]
    fn resize_shrink() {
        let mut buf = Buffer::from_vec(vec![1, 2, 3, 4]);
        buf.resize(2);
        assert_eq!(buf.data(), &[1, 2]);
    }

    #[test]
    fn equality() {
        let a = Buffer::from_vec(vec![1, 2, 3]);
        let b = Buffer::from_slice(&[1, 2, 3]);
        assert_eq!(a, b);
    }

    #[test]
    fn as_ref_slice() {
        let buf = Buffer::from_vec(vec![5, 6]);
        let s: &[u8] = buf.as_ref();
        assert_eq!(s, &[5, 6]);
    }

    #[test]
    fn debug_format() {
        let buf = Buffer::alloc(8).unwrap();
        let dbg = format!("{buf:?}");
        assert!(dbg.contains("Buffer"));
        assert!(dbg.contains("len: 8"));
    }

    #[test]
    fn send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Buffer>();
        assert_sync::<Buffer>();
    }

    // ── Negative / Edge ──

    #[test]
    fn alloc_zero_size() {
        let buf = Buffer::alloc(0).unwrap();
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn empty_buffer() {
        let buf = Buffer::from_vec(Vec::new());
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.data(), &[]);
    }

    #[test]
    fn drop_cloned_restores_writable() {
        let a = Buffer::from_vec(vec![1]);
        let b = a.clone();
        assert!(!a.is_writable());
        drop(b);
        assert!(a.is_writable());
    }

    // ═══════════════════════════════════════════════════
    // BufferPool tests
    // ═══════════════════════════════════════════════════

    // ── Positive ──

    #[test]
    fn pool_get_returns_correct_size() {
        let pool = BufferPool::new(64, 4).unwrap();
        let buf = pool.get().unwrap();
        assert_eq!(buf.len(), 64);
        assert!(buf.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn pool_put_and_reuse() {
        let pool = BufferPool::new(32, 4).unwrap();
        let buf = pool.get().unwrap();
        assert_eq!(pool.cached_count(), 0);

        pool.put(buf);
        assert_eq!(pool.cached_count(), 1);

        // Next get should reuse the cached buffer.
        let buf2 = pool.get().unwrap();
        assert_eq!(buf2.len(), 32);
        assert_eq!(pool.cached_count(), 0);
    }

    #[test]
    fn pool_put_zeroes_on_reuse() {
        let pool = BufferPool::new(4, 4).unwrap();
        let mut buf = pool.get().unwrap();
        buf.make_writable().copy_from_slice(&[1, 2, 3, 4]);
        pool.put(buf);

        let buf2 = pool.get().unwrap();
        // Pool zeros the buffer before handing it out.
        assert!(buf2.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn pool_respects_max_count() {
        let pool = BufferPool::new(8, 2).unwrap();
        let b1 = pool.get().unwrap();
        let b2 = pool.get().unwrap();
        let b3 = pool.get().unwrap();

        pool.put(b1);
        pool.put(b2);
        pool.put(b3); // Should be silently dropped (pool full).
        assert_eq!(pool.cached_count(), 2);
    }

    #[test]
    fn pool_rejects_shared_buffer() {
        let pool = BufferPool::new(8, 4).unwrap();
        let buf = pool.get().unwrap();
        let _clone = buf.clone();

        // buf has ref_count > 1, pool should reject it.
        pool.put(buf);
        assert_eq!(pool.cached_count(), 0);
    }

    #[test]
    fn pool_rejects_wrong_size() {
        let pool = BufferPool::new(8, 4).unwrap();
        let buf = Buffer::from_vec(vec![0; 16]); // Wrong size.
        pool.put(buf);
        assert_eq!(pool.cached_count(), 0);
    }

    #[test]
    fn pool_buffer_size() {
        let pool = BufferPool::new(1024, 8).unwrap();
        assert_eq!(pool.buffer_size(), 1024);
    }

    #[test]
    fn pool_debug_format() {
        let pool = BufferPool::new(16, 4).unwrap();
        let dbg = format!("{pool:?}");
        assert!(dbg.contains("BufferPool"));
    }

    // ── Negative ──

    #[test]
    fn pool_zero_max_count() {
        assert!(BufferPool::new(8, 0).is_err());
    }

    // ── Concurrency ──

    #[test]
    fn pool_concurrent_get_put() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(BufferPool::new(64, 8).unwrap());
        let mut handles = Vec::new();

        for _ in 0..8 {
            let pool = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let buf = pool.get().unwrap();
                    assert_eq!(buf.len(), 64);
                    pool.put(buf);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All threads done, pool should have some cached buffers.
        assert!(pool.cached_count() <= 8);
    }

    #[test]
    fn buffer_send_across_threads() {
        use std::thread;

        let buf = Buffer::from_vec(vec![42, 43, 44]);
        let handle = thread::spawn(move || {
            assert_eq!(buf.data(), &[42, 43, 44]);
            buf
        });
        let buf = handle.join().unwrap();
        assert_eq!(buf.data(), &[42, 43, 44]);
    }

    #[test]
    fn buffer_shared_across_threads() {
        use std::sync::Arc;
        use std::thread;

        let buf = Arc::new(Buffer::from_vec(vec![1, 2, 3]));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let buf = Arc::clone(&buf);
            handles.push(thread::spawn(move || {
                assert_eq!(buf.data(), &[1, 2, 3]);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
