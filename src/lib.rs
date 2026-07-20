//! Atom DB: a dependency-free persistence and cognitive-recall experiment.
//!
//! Durable truth contains only immutable atoms and bonds. Cognitive and
//! retrieval state are deterministic observation fields and cannot mutate facts.

mod cell;
mod cognitive;
mod digest;
mod retrieval;
mod store;

pub use cell::{Cell, CellReceipt, RootVersion, Snapshot};
pub use cognitive::{
    ActivatedAtom, BondMemory, CognitiveConfig, CognitiveEngine, LearningScope, RecallReport,
    RecallThread,
};
pub use digest::{Digest, digest};
pub use retrieval::{
    ContextPacket, Evidence, EvidenceThread, RememberReceipt, RetrievalConfig, RetrievalCue,
    Retriever,
};
pub use store::{AccessMode, AtomDb, Bond, Error, Stats};
