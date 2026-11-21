//! Dependency-free runtime primitives: [`HandleKey`] / [`SlotMap`] (RID & weak `?T`)
//! and downward-bumping [`FrameArena`]. See `SPECIFICATION.md` §§3.3–3.4, §8.2.

mod arena;
mod handle;

pub use arena::{Checkpoint, FrameArena, FrameBox};
pub use handle::{HandleKey, SlotMap};
