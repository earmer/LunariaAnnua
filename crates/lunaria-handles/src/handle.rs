use std::{
    fmt,
    mem::{self, ManuallyDrop},
    num::NonZeroU64,
};

const NO_FREE_SLOT: u32 = u32::MAX;

/// Generational handle encoded as `(version << 32) | index`.
///
/// Live versions are odd, reserving `0` for `nil` so `Option<HandleKey>` is 8 bytes.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandleKey(NonZeroU64);

impl HandleKey {
    /// Decodes a raw 64-bit register/FFI handle; `0` and even versions are invalid.
    pub const fn from_raw(raw: u64) -> Option<Self> {
        if (raw >> 32) & 1 == 0 {
            return None;
        }
        // SAFETY: bit 32 is 1, so `raw` is non-zero.
        Some(Self(unsafe { NonZeroU64::new_unchecked(raw) }))
    }

    const fn new(index: u32, version: u32) -> Self {
        debug_assert!(version & 1 == 1);
        let raw = ((version as u64) << 32) | (index as u64);
        // SAFETY: `version` is odd, so bit 32 is 1 and `raw` is non-zero.
        Self(unsafe { NonZeroU64::new_unchecked(raw) })
    }

    pub const fn raw(self) -> u64 {
        self.0.get()
    }

    pub const fn index(self) -> u32 {
        self.raw() as u32
    }

    pub const fn version(self) -> u32 {
        (self.raw() >> 32) as u32
    }
}

impl fmt::Debug for HandleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HandleKey({}:v{})", self.index(), self.version())
    }
}

union SlotUnion<T> {
    value: ManuallyDrop<T>,
    next_free: u32,
}

struct Slot<T> {
    storage: SlotUnion<T>,
    version: u32, // odd = occupied; even = vacant
}

impl<T> Slot<T> {
    fn occupy(&mut self, value: T) -> (u32, u32) {
        debug_assert!(self.version & 1 == 0);
        // SAFETY: vacant slots always store `next_free`.
        let next_free = unsafe { self.storage.next_free };
        self.version |= 1;
        self.storage.value = ManuallyDrop::new(value);
        (next_free, self.version)
    }

    fn vacate(&mut self, version: u32, next_free: u32) -> Option<T> {
        if self.version != version {
            return None;
        }
        self.version = self.version.wrapping_add(1);
        // SAFETY: matching an odd version proves this slot owns one live value.
        let value = unsafe { ManuallyDrop::take(&mut self.storage.value) };
        self.storage.next_free = next_free;
        Some(value)
    }

    fn get(&self, version: u32) -> Option<&T> {
        // SAFETY: `version` is odd, so a match proves this slot is occupied.
        (self.version == version).then(|| unsafe { &*self.storage.value })
    }

    fn get_mut(&mut self, version: u32) -> Option<&mut T> {
        // SAFETY: `version` is odd, so a match proves this slot is occupied.
        (self.version == version).then(|| unsafe { &mut *self.storage.value })
    }
}

/// Dense generational storage shared by RID and weak-reference lookups.
pub struct SlotMap<T> {
    slots: Vec<Slot<T>>,
    free_head: u32,
    len: usize,
}

impl<T> SlotMap<T> {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: NO_FREE_SLOT,
            len: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            ..Self::new()
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, value: T) -> HandleKey {
        let (index, version) = if self.free_head == NO_FREE_SLOT {
            let index = u32::try_from(self.slots.len())
                .ok()
                .filter(|&i| i != NO_FREE_SLOT)
                .expect("SlotMap full");
            let storage = SlotUnion {
                value: ManuallyDrop::new(value),
            };
            self.slots.push(Slot {
                storage,
                version: 1,
            });
            (index, 1)
        } else {
            let index = self.free_head;
            let (next_free, version) = self.slots[index as usize].occupy(value);
            self.free_head = next_free;
            (index, version)
        };
        self.len += 1;
        HandleKey::new(index, version)
    }

    pub fn get(&self, key: HandleKey) -> Option<&T> {
        self.slots.get(key.index() as usize)?.get(key.version())
    }

    pub fn get_mut(&mut self, key: HandleKey) -> Option<&mut T> {
        self.slots.get_mut(key.index() as usize)?.get_mut(key.version())
    }

    /// Validates a weak handle and borrows its entry if still alive.
    pub fn upgrade(&self, key: HandleKey) -> Option<&T> {
        self.get(key)
    }

    pub fn remove(&mut self, key: HandleKey) -> Option<T> {
        let value =
            self.slots.get_mut(key.index() as usize)?.vacate(key.version(), self.free_head)?;
        self.free_head = key.index();
        self.len -= 1;
        Some(value)
    }
}

impl<T> Default for SlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for SlotMap<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            for slot in &mut self.slots {
                if slot.version & 1 == 1 {
                    // SAFETY: occupied slot owns one live value; `Drop` runs once.
                    unsafe { ManuallyDrop::drop(&mut slot.storage.value) };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn raw_encoding_and_nil_niche() {
        for bad in [0, 1, 2_u64 << 32] {
            assert_eq!(HandleKey::from_raw(bad), None);
        }
        let raw = (3_u64 << 32) | 17;
        let key = HandleKey::from_raw(raw).unwrap();
        assert_eq!((key.index(), key.version(), key.raw()), (17, 3, raw));
        assert_eq!(mem::size_of::<Option<HandleKey>>(), 8);
    }

    #[test]
    fn stale_handles_rejected_after_slot_reuse() {
        let mut map = SlotMap::new();
        let old = map.insert(String::from("old"));
        assert_eq!(map.remove(old).as_deref(), Some("old"));
        let new = map.insert(String::from("new"));
        assert_eq!((old.index(), old.version(), new.version()), (new.index(), 1, 3));
        assert_eq!(map.get(old), None);
        assert_eq!(map.remove(old), None);
        assert_eq!(map.upgrade(new).map(String::as_str), Some("new"));
        map.get_mut(new).unwrap().push('!');
        assert_eq!((map.get(new).map(String::as_str), map.len()), (Some("new!"), 1));
    }

    #[test]
    fn version_wrap_preserves_parity_and_rejects_old_generation() {
        let mut map = SlotMap::new();
        let first = map.insert(1);
        assert_eq!(map.remove(first), Some(1));
        map.slots[0].version = u32::MAX - 1;
        let last = map.insert(2);
        assert_eq!(
            (last.version(), map.remove(last), map.slots[0].version),
            (u32::MAX, Some(2), 0)
        );
        let wrapped = map.insert(3);
        assert_eq!((wrapped.version(), map.get(last), map.get(wrapped)), (1, None, Some(&3)));
    }

    #[test]
    fn values_dropped_exactly_once() {
        struct Spy(Rc<Cell<usize>>);
        impl Drop for Spy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut map = SlotMap::new();
        let removed = map.insert(Spy(Rc::clone(&drops)));
        map.insert(Spy(Rc::clone(&drops)));
        drop(map.remove(removed));
        assert_eq!(drops.get(), 1);
        drop(map);
        assert_eq!(drops.get(), 2);
    }
}
