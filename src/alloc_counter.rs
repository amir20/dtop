//! A test-only allocation-counting global allocator.
//!
//! This wraps the system allocator and, while enabled on the current thread,
//! counts every `alloc`/`realloc` call. It is used by allocation-regression
//! tests to measure how many heap allocations a hot path performs so we can
//! drive that number as close to zero as possible and prevent regressions.
//!
//! Counting is thread-local so that tests running in parallel (the default for
//! `cargo test`) do not pollute each other's measurements.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

/// Global allocator that counts allocations on the current thread when enabled.
pub struct CountingAllocator;

#[inline]
fn record_alloc() {
    // `try_with` avoids panicking if accessed during thread-local teardown.
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_alloc();
        // SAFETY: forwarding to the system allocator with the same layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarding to the system allocator with the original ptr/layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_alloc();
        // SAFETY: forwarding to the system allocator with the same layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // A realloc that grows a buffer is itself an allocation event we care
        // about (e.g. a `Vec` outgrowing its capacity mid-render).
        record_alloc();
        // SAFETY: forwarding to the system allocator with the original ptr/layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Runs `f`, returning the number of allocations performed on the current
/// thread during its execution.
///
/// Nested/parallel measurements on the same thread are not supported; this is
/// intended to wrap a single hot-path call in a test.
pub fn count_allocations<F: FnOnce()>(f: F) -> u64 {
    // Disable counting on drop so a panic inside `f` cannot leave `ENABLED`
    // stuck at `true`. The test harness reuses worker threads, so a leaked
    // flag would silently pollute later measurements on the same thread (and
    // count allocations performed during unwinding).
    struct DisableOnDrop;
    impl Drop for DisableOnDrop {
        fn drop(&mut self) {
            ENABLED.with(|e| e.set(false));
        }
    }

    COUNT.with(|c| c.set(0));
    let _guard = DisableOnDrop;
    ENABLED.with(|e| e.set(true));
    f();
    COUNT.with(|c| c.get())
}
