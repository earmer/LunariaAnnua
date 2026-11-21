//! Foundation memory primitives for the Lunaria Annua runtime.
//!
//! This crate is deliberately dependency-free and contains:
//!
//! - `HandleKey`: the 64-bit `(version | 1) << 32 | index` handle encoding,
//!   where `0u64` means `nil` (see SPECIFICATION.md §3.3).
//! - `SlotMap<T>`: slot storage with odd/even version tags that rejects ABA.
//! - `FrameArena`: a downward-bumping frame allocator whose `reset` never runs
//!   destructors (see SPECIFICATION.md §3.4).
//!
//! Implementation lands in Step 1 of the roadmap (§8.2).
