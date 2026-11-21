use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error},
    cell::Cell,
    marker::PhantomData,
    mem::needs_drop,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};

const DEFAULT_CAPACITY: usize = 2 * 1024 * 1024;
const CHUNK_ALIGNMENT: usize = 16;

/// A saved position in a [`FrameArena`] for bulk reclamation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    owner: NonNull<u8>,
    ptr: NonNull<u8>,
}

/// Exclusive allocation inside a [`FrameArena`]. Never runs `Drop` for `T`.
pub struct FrameBox<'arena, T> {
    ptr: NonNull<T>,
    _marker: PhantomData<&'arena mut T>,
}

impl<T> FrameBox<'_, T> {
    pub const fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<T> Deref for FrameBox<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: valid and initialized for the arena's borrow lifetime.
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for FrameBox<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: each non-ZST allocation is disjoint and `&mut self` is exclusive.
        unsafe { self.ptr.as_mut() }
    }
}

/// Fixed-capacity, downward-bumping allocator for one frame's plain data.
///
/// Allocation takes `&self` so multiple values coexist; reset takes `&mut self`
/// so references cannot escape a reset. Drop-needing types fail at compile time.
pub struct FrameArena {
    start: NonNull<u8>,
    end: NonNull<u8>,
    ptr: Cell<NonNull<u8>>,
}

// SAFETY: uniquely owns its backing chunk and is `!Sync` (via `Cell`).
unsafe impl Send for FrameArena {}

impl FrameArena {
    pub fn with_capacity(capacity: usize) -> Self {
        let (start, end) = if capacity == 0 {
            let d = NonNull::dangling();
            (d, d)
        } else {
            let layout =
                Layout::from_size_align(capacity, CHUNK_ALIGNMENT).expect("invalid arena capacity");
            // SAFETY: `capacity > 0` and `CHUNK_ALIGNMENT` is a power of two.
            let start = NonNull::new(unsafe { alloc(layout) })
                .unwrap_or_else(|| handle_alloc_error(layout));
            // SAFETY: `start` points to an allocation of `capacity` bytes.
            (start, unsafe { start.add(capacity) })
        };
        Self {
            start,
            end,
            ptr: Cell::new(end),
        }
    }

    /// Allocates raw bytes for `layout` by bumping downward and masking alignment.
    ///
    /// The returned pointer is uninitialized and must not be dereferenced after
    /// a reset that invalidates its position.
    pub fn alloc_layout(&self, layout: Layout) -> Option<NonNull<u8>> {
        if layout.size() == 0 {
            // SAFETY: `Layout::align()` is always a non-zero power of two.
            let p = ptr::without_provenance_mut(layout.align());
            return Some(unsafe { NonNull::new_unchecked(p) });
        }
        let addr = self.ptr.get().addr().get().checked_sub(layout.size())? & !(layout.align() - 1);
        if addr < self.start.addr().get() {
            return None;
        }
        // SAFETY: `addr >= self.start.addr() > 0` and lies within `[start, end)`.
        let ptr = self.start.with_addr(unsafe { NonZeroUsize::new_unchecked(addr) });
        self.ptr.set(ptr);
        Some(ptr)
    }

    /// Allocates and initializes a value, panicking if the chunk is exhausted.
    pub fn alloc<T>(&self, value: T) -> FrameBox<'_, T> {
        self.try_alloc(value).expect("FrameArena capacity exhausted")
    }

    /// Attempts to allocate a value without growing or running destructors.
    ///
    /// ```compile_fail
    /// let arena = lunaria_handles::FrameArena::with_capacity(64);
    /// let _ = arena.try_alloc(String::from("not POD"));
    /// ```
    pub fn try_alloc<T>(&self, value: T) -> Option<FrameBox<'_, T>> {
        const { assert!(!needs_drop::<T>(), "FrameArena forbids Drop") };
        let ptr = self.alloc_layout(Layout::new::<T>())?.cast::<T>();
        // SAFETY: `alloc_layout` returned a unique, aligned slot for `T`.
        unsafe { ptr.write(value) };
        Some(FrameBox {
            ptr,
            _marker: PhantomData,
        })
    }

    pub fn capacity(&self) -> usize {
        self.end.addr().get() - self.start.addr().get()
    }

    /// Bytes consumed from the chunk end, including alignment padding.
    pub fn used(&self) -> usize {
        self.end.addr().get() - self.ptr.get().addr().get()
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            owner: self.start,
            ptr: self.ptr.get(),
        }
    }

    /// Restores a saved chunk position without running destructors.
    ///
    /// Checkpoints are reusable positions, not branch-capability tokens. Any
    /// in-bounds checkpoint from this arena may be restored while no allocations
    /// are borrowed, regardless of intervening resets or allocation branches.
    pub fn reset_to(&mut self, cp: Checkpoint) {
        assert_eq!(cp.owner, self.start, "checkpoint from another FrameArena");
        assert!(cp.ptr >= self.start && cp.ptr <= self.end, "checkpoint outside FrameArena");
        self.ptr.set(cp.ptr);
    }

    /// Reclaims the entire chunk without running destructors.
    pub fn reset(&mut self) {
        self.ptr.set(self.end);
    }
}

impl Default for FrameArena {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl Drop for FrameArena {
    fn drop(&mut self) {
        if let Some(cap) = NonZeroUsize::new(self.capacity()) {
            // SAFETY: `start` was allocated with `(cap, CHUNK_ALIGNMENT)` in `with_capacity`.
            unsafe {
                dealloc(
                    self.start.as_ptr(),
                    Layout::from_size_align_unchecked(cap.get(), CHUNK_ALIGNMENT),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downward_allocation_and_alignment() {
        #[repr(align(64))]
        struct Aligned(u8);
        let arena = FrameArena::with_capacity(256);
        let byte = arena.alloc(7_u8);
        let aligned = arena.alloc(Aligned(9));
        assert_eq!((*byte, aligned.0), (7, 9));
        assert!(aligned.as_ptr().addr() < byte.as_ptr().addr());
        assert_eq!(aligned.as_ptr().addr() % 64, 0);
        assert!(arena.used() >= 65);
    }

    #[test]
    fn reset_and_checkpoint_reuse_addresses() {
        let mut arena = FrameArena::with_capacity(128);
        let first = arena.alloc(11_u64).as_ptr();
        let cp = arena.checkpoint();
        let second = arena.alloc(22_u64).as_ptr();
        assert_eq!(arena.used(), 16);
        arena.reset_to(cp);
        assert_eq!((arena.used(), arena.alloc(33_u64).as_ptr()), (8, second));
        arena.reset();
        assert_eq!((arena.used(), arena.alloc(44_u64).as_ptr()), (0, first));
    }

    #[test]
    fn exhaustion_and_zero_capacity_are_non_destructive() {
        struct Zst;
        let arena = FrameArena::with_capacity(32);
        let cp = arena.checkpoint();
        assert!(arena.try_alloc([0_u64; 8]).is_none());
        assert_eq!(arena.checkpoint(), cp);
        arena.alloc(Zst);
        assert_eq!(arena.used(), 0);
        let empty = FrameArena::with_capacity(0);
        assert_eq!((empty.capacity(), empty.used()), (0, 0));
        assert!(empty.try_alloc(1_u8).is_none());
        empty.alloc(Zst);
        assert_eq!(empty.used(), 0);
    }

    #[test]
    #[should_panic(expected = "another FrameArena")]
    fn cross_arena_checkpoint_panics() {
        let mut first = FrameArena::with_capacity(64);
        let second = FrameArena::with_capacity(64);
        first.reset_to(second.checkpoint());
    }

    #[test]
    fn checkpoints_are_reusable_across_allocation_branches() {
        let mut arena = FrameArena::with_capacity(64);
        let start = arena.checkpoint();
        arena.alloc(1_u64);
        let branch = arena.checkpoint();
        arena.alloc(2_u64);
        arena.reset_to(start);
        arena.alloc(3_u64);
        arena.alloc(4_u64);

        arena.reset_to(branch);
        assert_eq!(arena.used(), 8);
    }

    #[test]
    #[should_panic(expected = "checkpoint outside FrameArena")]
    fn out_of_bounds_checkpoint_panics() {
        let mut arena = FrameArena::with_capacity(64);
        let beyond_end = arena.end.addr().get() + 1;
        let cp = Checkpoint {
            owner: arena.start,
            ptr: arena.end.with_addr(NonZeroUsize::new(beyond_end).unwrap()),
        };
        arena.reset_to(cp);
    }
}
