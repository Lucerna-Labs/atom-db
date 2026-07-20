//! Atom DB: a dependency-free persistence and cognitive-recall experiment.
//!
//! Durable truth contains only immutable atoms and bonds. Cognitive state is a
//! separate, deterministic observation field and cannot mutate stored facts.

mod cognitive;
mod digest;
mod store;

pub use cognitive::{
    ActivatedAtom, BondMemory, CognitiveConfig, CognitiveEngine, RecallReport, RecallThread,
};
pub use digest::{Digest, digest};
pub use store::{AtomDb, Bond, Error, Stats};
