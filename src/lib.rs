//! Atom DB: a dependency-free persistence and cognitive-recall experiment.
//!
//! Durable truth contains only immutable atoms and bonds. Cognitive state is a
//! separate, deterministic observation field and cannot mutate stored facts.

mod cell;
mod cognitive;
mod digest;
mod store;

pub use cell::{Cell, CellReceipt, RootVersion, Snapshot};
pub use cognitive::{
    ActivatedAtom, BondMemory, CognitiveConfig, CognitiveEngine, LearningScope, RecallReport,
    RecallThread,
};
pub use digest::{Digest, digest};
pub use store::{AccessMode, AtomDb, Bond, Error, Stats};
