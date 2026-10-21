//! The memory the core itself holds. In the Android app the core shares its process with Kotlin
//! and Android's drawing, so the process's figures can't tell them apart; there, every allocation
//! of the core goes through `Counting`, which hands it to the system's allocator and keeps count
//! (the app's memory panel shows it). The CLI and the TUI keep the system's allocator as it is.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// The system's allocator, counting the bytes held now (allocated, not freed yet) and the most
/// ever held at once. Two atomic additions per allocation: nothing next to the allocation itself.
pub struct Counting {
    held: AtomicUsize,
    peak: AtomicUsize,
}

impl Counting {
    pub const fn new() -> Counting {
        Counting { held: AtomicUsize::new(0), peak: AtomicUsize::new(0) }
    }

    /// The bytes held now.
    pub fn held(&self) -> usize {
        self.held.load(Relaxed)
    }

    /// The most bytes held at once.
    pub fn peak(&self) -> usize {
        self.peak.load(Relaxed)
    }

    fn add(&self, bytes: usize) {
        let now = self.held.fetch_add(bytes, Relaxed) + bytes;
        self.peak.fetch_max(now, Relaxed);
    }

    fn remove(&self, bytes: usize) {
        self.held.fetch_sub(bytes, Relaxed);
    }
}

impl Default for Counting {
    fn default() -> Counting {
        Counting::new()
    }
}

// SAFETY: every call goes to `System` with the same arguments, and its result comes back as it is;
// the counting only reads the sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let block = unsafe { System.alloc(layout) };
        if !block.is_null() {
            self.add(layout.size());
        }
        block
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let block = unsafe { System.alloc_zeroed(layout) };
        if !block.is_null() {
            self.add(layout.size());
        }
        block
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        unsafe { System.dealloc(block, layout) };
        self.remove(layout.size());
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(block, layout, size) };
        // On failure the old block stays, and so does its count.
        if !moved.is_null() {
            self.remove(layout.size());
            self.add(size);
        }
        moved
    }
}

/// The core's allocator in the Android app.
#[cfg(all(feature = "ffi", target_os = "android"))]
#[global_allocator]
pub static CORE: Counting = Counting::new();

/// The bytes the core holds now and its peak, where they are counted (the Android app).
#[cfg(all(feature = "ffi", target_os = "android"))]
pub fn counted() -> Option<(usize, usize)> {
    Some((CORE.held(), CORE.peak()))
}

/// Elsewhere the system's allocator counts nothing.
#[cfg(not(all(feature = "ffi", target_os = "android")))]
pub fn counted() -> Option<(usize, usize)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_what_is_held() {
        let counting = Counting::new();
        let layout = Layout::from_size_align(1000, 8).unwrap();
        unsafe {
            let a = counting.alloc(layout);
            let b = counting.alloc_zeroed(layout);
            assert!(!a.is_null() && !b.is_null());
            assert_eq!(*b, 0);
            assert_eq!((counting.held(), counting.peak()), (2000, 2000));
            // Grown: the old size out, the new one in.
            let a = counting.realloc(a, layout, 5000);
            assert!(!a.is_null());
            assert_eq!((counting.held(), counting.peak()), (6000, 6000));
            counting.dealloc(b, layout);
            counting.dealloc(a, Layout::from_size_align(5000, 8).unwrap());
        }
        // All freed; the peak stays.
        assert_eq!((counting.held(), counting.peak()), (0, 6000));
        assert_eq!((Counting::default().held(), Counting::default().peak()), (0, 0));
    }

    #[test]
    fn not_counted_off_android() {
        assert_eq!(counted(), None);
    }
}
