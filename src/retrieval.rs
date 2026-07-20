use crate::{AtomDb, Bond, Cell, CellReceipt, Digest, Error};
use atom_retrieval_field::{
    Arc, Cue, FieldConfig, FieldReport, activate, passages_with_overlap, terms,
};
pub use atom_retrieval_field::{ContextPacket, Evidence, EvidenceThread, RetrievalCue};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

const TERM_DOMAIN: &[u8] = b"atom-db/retrieval/term/v1\0";
const MENTIONS: &[u8] = b"atom-db/retrieval/mentions/v1";
const FROM_SOURCE: &[u8] = b"atom-db/retrieval/from-source/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetrievalConfig {
    pub field: FieldConfig,
    pub passage_bytes: usize,
    pub max_document_bytes: usize,
    pub max_terms_per_passage: usize,
    pub max_query_terms: usize,
    pub max_evidence: usize,
    pub max_context_bytes: usize,
    pub minimum_cue_support: usize,
    /// Trailing bytes shared between consecutive passages so an answer
    /// spanning a boundary survives whole in the next window. Zero keeps
    /// the pre-Stage-8 hard-boundary behavior.
    pub passage_overlap_bytes: usize,
    /// Stage 8: rare cues inject more activation (document-frequency
    /// information gain) instead of a flat signal per term.
    pub information_gain: bool,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            field: FieldConfig::default(),
            passage_bytes: 768,
            max_document_bytes: 8 * 1024 * 1024,
            max_terms_per_passage: 256,
            max_query_terms: 24,
            max_evidence: 8,
            max_context_bytes: 8 * 1024,
            minimum_cue_support: 2,
            passage_overlap_bytes: 128,
            information_gain: true,
        }
    }
}

impl RetrievalConfig {
    fn validate(self) -> Result<Self, String> {
        self.field.validate()?;
        if !(64..=65_536).contains(&self.passage_bytes) {
            return Err("passage bytes must be between 64 and 65536".into());
        }
        if self.passage_overlap_bytes >= self.passage_bytes / 2 {
            return Err("passage overlap must stay below half the passage size".into());
        }
        if self.max_document_bytes < self.passage_bytes {
            return Err("document budget must cover at least one passage".into());
        }
        for (name, value, limit) in [
            ("terms per passage", self.max_terms_per_passage, 16_384),
            ("query terms", self.max_query_terms, 256),
            ("evidence count", self.max_evidence, 4_096),
            ("cue support", self.minimum_cue_support, 256),
        ] {
            if value == 0 || value > limit {
                return Err(format!("{name} must be between 1 and {limit}"));
            }
        }
        if self.max_context_bytes == 0 || self.max_context_bytes > 64 * 1024 * 1024 {
            return Err("context bytes must be between 1 and 67108864".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RememberReceipt {
    pub cell: CellReceipt,
    pub source: Digest,
    pub passages: Vec<Digest>,
    pub unique_terms: usize,
}

/// Stage 8 derived index: the retrieval graph plus per-term document
/// frequency, rebuilt only when the durable snapshot sequence changes.
/// A query against an unchanged snapshot pays no rebuild.
#[derive(Debug, Default)]
struct DerivedIndex {
    snapshot_sequence: u64,
    nodes: Vec<Digest>,
    node_index: BTreeMap<Digest, usize>,
    edges: Vec<Arc>,
    edge_ids: Vec<Digest>,
    passage_ids: BTreeSet<Digest>,
    passage_sources: BTreeMap<Digest, BTreeSet<Digest>>,
    /// mentions-bond count per term atom (document frequency).
    term_passages: BTreeMap<Digest, u64>,
    /// How many times this index has been rebuilt (telemetry for the
    /// unchanged-snapshot law).
    rebuilds: usize,
}

#[derive(Debug)]
pub struct Retriever {
    config: RetrievalConfig,
    index: Mutex<DerivedIndex>,
}

impl Retriever {
    pub fn new(config: RetrievalConfig) -> Result<Self, String> {
        Ok(Self {
            config: config.validate()?,
            index: Mutex::new(DerivedIndex::default()),
        })
    }

    /// How many times the derived index has been rebuilt. After the first
    /// query, an unchanged snapshot must never move this counter.
    pub fn index_rebuilds(&self) -> usize {
        self.index
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rebuilds
    }

    pub fn remember(
        &self,
        db: &mut AtomDb,
        source: &str,
        document: &str,
    ) -> Result<RememberReceipt, Error> {
        if source.trim().is_empty() || document.trim().is_empty() {
            return Err(Error::Invalid(
                "source and document must not be empty".into(),
            ));
        }
        if document.len() > self.config.max_document_bytes {
            return Err(Error::Invalid(
                "document exceeds retrieval ingestion budget".into(),
            ));
        }
        let passage_texts = passages_with_overlap(
            document,
            self.config.passage_bytes,
            self.config.passage_overlap_bytes,
        )
        .map_err(Error::Invalid)?;
        let mut cell = db.begin_cell();
        let source = cell.put_atom(source.as_bytes());
        let mentions = cell.put_atom(MENTIONS);
        let from_source = cell.put_atom(FROM_SOURCE);
        let mut passage_ids = Vec::new();
        let mut unique_terms = BTreeSet::new();
        for passage in passage_texts {
            let passage_id = cell.put_atom(passage.as_bytes());
            passage_ids.push(passage_id);
            cell.put_bond(Bond {
                source: passage_id,
                relation: from_source,
                target: source,
            });
            for term in terms(&passage, self.config.max_terms_per_passage) {
                unique_terms.insert(term.clone());
                let term_id = cell.put_atom(term_bytes(&term));
                cell.put_bond(Bond {
                    source: term_id,
                    relation: mentions,
                    target: passage_id,
                });
            }
        }
        let cell = db.commit_cell(cell)?;
        db.sync()?;
        Ok(RememberReceipt {
            cell,
            source,
            passages: passage_ids,
            unique_terms: unique_terms.len(),
        })
    }

    pub fn retrieve(&self, db: &mut AtomDb, query: &str) -> Result<ContextPacket<Digest>, Error> {
        let snapshot_sequence = db.snapshot().sequence();
        let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        if index.snapshot_sequence != snapshot_sequence {
            let rebuilds = index.rebuilds + 1;
            *index = build_index(db, snapshot_sequence)?;
            index.rebuilds = rebuilds;
        }
        let index = &*index;

        let query_terms = terms(query, self.config.max_query_terms);
        let mut cues = Vec::new();
        let mut field_cues = Vec::new();
        let mut known_terms = Vec::new();
        let total_passages = index.passage_ids.len() as u64;
        for term in query_terms {
            let identity = identity(&term_bytes(&term));
            let node = index.node_index.get(&identity).copied();
            cues.push(RetrievalCue {
                term: term.clone(),
                identity,
                known: node.is_some(),
            });
            if let Some(node) = node {
                known_terms.push(term.clone());
                let activation = cue_activation(
                    identity,
                    total_passages,
                    index,
                    self.config.information_gain,
                );
                field_cues.push(Cue { node, activation });
            }
        }
        let field =
            activate(&index.edges, &field_cues, self.config.field).map_err(Error::Invalid)?;
        self.packet(
            db,
            query,
            snapshot_sequence,
            cues,
            known_terms,
            &index.nodes,
            &index.edge_ids,
            &index.passage_ids,
            &index.passage_sources,
            field,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn packet(
        &self,
        db: &mut AtomDb,
        query: &str,
        snapshot_sequence: u64,
        cues: Vec<RetrievalCue<Digest>>,
        known_terms: Vec<String>,
        nodes: &[Digest],
        edge_ids: &[Digest],
        passage_ids: &BTreeSet<Digest>,
        passage_sources: &BTreeMap<Digest, BTreeSet<Digest>>,
        field: FieldReport,
    ) -> Result<ContextPacket<Digest>, Error> {
        let mut evidence = Vec::new();
        let mut context_bytes = 0;
        for hit in &field.hits {
            let Some(identity) = nodes.get(hit.node).copied() else {
                continue;
            };
            if !passage_ids.contains(&identity) || evidence.len() == self.config.max_evidence {
                continue;
            }
            let Some(bytes) = db.get_atom(identity)? else {
                continue;
            };
            let full = String::from_utf8_lossy(&bytes);
            let remaining = self.config.max_context_bytes.saturating_sub(context_bytes);
            if remaining == 0 {
                break;
            }
            let (text, truncated) = excerpt(&full, remaining);
            context_bytes += text.len();
            let mut sources = Vec::new();
            for source in passage_sources.get(&identity).into_iter().flatten() {
                if let Some(bytes) = db.get_atom(*source)? {
                    sources.push(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            let supporting_cues = hit
                .supporting_cues
                .iter()
                .filter_map(|index| known_terms.get(*index).cloned())
                .collect::<Vec<_>>();
            let threads = hit
                .threads
                .iter()
                .filter_map(|thread| {
                    let cue = known_terms.get(thread.cue_index)?.clone();
                    let atoms = thread
                        .nodes
                        .iter()
                        .filter_map(|node| nodes.get(*node).copied())
                        .collect();
                    let bonds = thread
                        .edges
                        .iter()
                        .filter_map(|edge| edge_ids.get(*edge).copied())
                        .collect();
                    Some(EvidenceThread { cue, atoms, bonds })
                })
                .collect();
            evidence.push(Evidence {
                identity,
                text,
                sources,
                activation: hit.activation,
                depth: hit.depth,
                incoming_signals: hit.incoming_signals,
                supporting_cues,
                threads,
                truncated,
            });
        }
        let required = self
            .config
            .minimum_cue_support
            .min(known_terms.len())
            .max(1);
        let answerable = evidence
            .first()
            .is_some_and(|item| item.supporting_cues.len() >= required);
        Ok(ContextPacket {
            query: query.to_string(),
            snapshot_sequence,
            answerable,
            insufficient_evidence: !answerable,
            cues,
            evidence,
            rounds: field.rounds,
            explored_edges: field.explored_edges,
            working_set_peak: field.working_set_peak,
            promoted_edges: field.promoted_edges,
            stabilized: field.stabilized,
            budget_exhausted: field.budget_exhausted,
            context_bytes,
        })
    }
}

impl Default for Retriever {
    fn default() -> Self {
        Self::new(RetrievalConfig::default()).expect("default retrieval config is valid")
    }
}

/// Rebuild the derived index from the committed bond set. Called only when
/// the snapshot sequence moved.
fn build_index(db: &mut AtomDb, snapshot_sequence: u64) -> Result<DerivedIndex, Error> {
    let mentions = identity(MENTIONS);
    let from_source = identity(FROM_SOURCE);
    let mut index = DerivedIndex {
        snapshot_sequence,
        ..DerivedIndex::default()
    };
    for (edge_id, bond) in db.all_bonds() {
        let source = intern(bond.source, &mut index.nodes, &mut index.node_index);
        let target = intern(bond.target, &mut index.nodes, &mut index.node_index);
        let conductance = if bond.relation == mentions {
            index.passage_ids.insert(bond.target);
            *index.term_passages.entry(bond.source).or_insert(0) += 1;
            1_400
        } else if bond.relation == from_source {
            index
                .passage_sources
                .entry(bond.source)
                .or_default()
                .insert(bond.target);
            600
        } else {
            900
        };
        let edge = index.edge_ids.len();
        index.edge_ids.push(edge_id);
        index.edges.push(Arc {
            from: source,
            to: target,
            edge,
            conductance_per_mille: conductance,
        });
        index.edges.push(Arc {
            from: target,
            to: source,
            edge,
            conductance_per_mille: conductance,
        });
    }
    Ok(index)
}

/// Stage 8 information gain: a cue's injected activation scales with how
/// rare its term is across the committed passages (1 + log2(1 + N/df)).
/// With information gain disabled, every cue injects the flat pre-Stage-8
/// signal. Deterministic for a given snapshot and configuration.
fn cue_activation(
    term: Digest,
    total_passages: u64,
    index: &DerivedIndex,
    information_gain: bool,
) -> u64 {
    const BASE: u64 = 1_000_000;
    if !information_gain {
        return BASE;
    }
    let df = index.term_passages.get(&term).copied().unwrap_or(0);
    if total_passages == 0 || df == 0 {
        return BASE;
    }
    let gain = 1.0 + (1.0 + total_passages as f64 / df as f64).log2();
    (BASE as f64 * gain) as u64
}

fn identity(bytes: &[u8]) -> Digest {
    Cell::new().put_atom(bytes)
}

fn term_bytes(term: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(TERM_DOMAIN.len() + term.len());
    bytes.extend_from_slice(TERM_DOMAIN);
    bytes.extend_from_slice(term.as_bytes());
    bytes
}

fn intern(identity: Digest, nodes: &mut Vec<Digest>, index: &mut BTreeMap<Digest, usize>) -> usize {
    if let Some(node) = index.get(&identity) {
        return *node;
    }
    let node = nodes.len();
    nodes.push(identity);
    index.insert(identity, node);
    node
}

fn excerpt(text: &str, budget: usize) -> (String, bool) {
    if text.len() <= budget {
        return (text.to_string(), false);
    }
    let mut end = budget;
    while !text.is_char_boundary(end) {
        end -= 1
    }
    (text[..end].to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "atom-db-retrieval-{label}-{}-{}.atoms",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn multi_cue_retrieval_returns_grounded_provenance_without_mutation() {
        let path = path("grounded");
        let mut db = AtomDb::open(&path).unwrap();
        let retriever = Retriever::default();
        retriever.remember(
            &mut db, "observer-leases.md",
            "A writer crash makes the operating system release its lease. Lease recovery permits a replacement writer.",
        ).unwrap();
        retriever
            .remember(&mut db, "astronomy.md", "Earth orbits the Sun.")
            .unwrap();
        drop(db);
        let mut db = AtomDb::open_read_only(&path).unwrap();
        let before = db.stats().unwrap();
        let packet = retriever
            .retrieve(&mut db, "writer crash lease recovery")
            .unwrap();
        let after = db.stats().unwrap();
        assert!(packet.answerable);
        assert!(packet.evidence[0].text.contains("writer crash"));
        assert!(packet.evidence[0].supporting_cues.len() >= 3);
        assert_eq!(packet.evidence[0].sources, vec!["observer-leases.md"]);
        assert!(!packet.evidence[0].threads.is_empty());
        assert_eq!(before, after);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unknown_cues_fail_closed() {
        let path = path("unknown");
        let mut db = AtomDb::open(&path).unwrap();
        let packet = Retriever::default()
            .retrieve(&mut db, "quasar marmalade")
            .unwrap();
        assert!(packet.insufficient_evidence);
        assert!(packet.evidence.is_empty());
        assert!(packet.cues.iter().all(|cue| !cue.known));
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unchanged_snapshot_never_rebuilds_the_index() {
        let path = path("cache-law");
        let mut db = AtomDb::open(&path).unwrap();
        let retriever = Retriever::default();
        retriever
            .remember(&mut db, "a.md", "leases recover after writer crashes.")
            .unwrap();
        assert_eq!(retriever.index_rebuilds(), 0);
        retriever.retrieve(&mut db, "writer crash lease").unwrap();
        assert_eq!(retriever.index_rebuilds(), 1);
        // Unchanged snapshot: identical result, no rebuild.
        let first = retriever.retrieve(&mut db, "writer crash lease").unwrap();
        let second = retriever.retrieve(&mut db, "writer crash lease").unwrap();
        assert_eq!(first, second);
        assert_eq!(retriever.index_rebuilds(), 1);
        // A committed change moves the snapshot and forces one rebuild.
        retriever
            .remember(&mut db, "b.md", "astronomy has nothing to do with leases.")
            .unwrap();
        retriever.retrieve(&mut db, "writer crash lease").unwrap();
        assert_eq!(retriever.index_rebuilds(), 2);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rare_cues_inject_more_activation_than_common_cues() {
        let path = path("gain-law");
        let mut db = AtomDb::open(&path).unwrap();
        let retriever = Retriever::default();
        // "common" appears in every passage; "zephyr" appears in exactly one.
        retriever
            .remember(&mut db, "d1.md", "common words everywhere. common again.")
            .unwrap();
        retriever
            .remember(&mut db, "d2.md", "common ground, common sense.")
            .unwrap();
        retriever
            .remember(&mut db, "d3.md", "a zephyr is anything but common.")
            .unwrap();
        let packet = retriever.retrieve(&mut db, "common zephyr").unwrap();
        assert!(packet.answerable);
        // The passage supported by the rare cue must win despite the common
        // term appearing in three times as many passages.
        assert_eq!(packet.evidence[0].sources, vec!["d3.md"]);
        assert!(
            packet.evidence[0]
                .supporting_cues
                .contains(&"zephyr".to_string())
        );
        // Direct law check: the rare cue's injected activation dominates.
        let index = retriever.index.lock().unwrap_or_else(|e| e.into_inner());
        let total = index.passage_ids.len() as u64;
        let zephyr = identity(&term_bytes("zephyr"));
        let common = identity(&term_bytes("common"));
        let rare_activation = cue_activation(zephyr, total, &index, true);
        let common_activation = cue_activation(common, total, &index, true);
        assert!(rare_activation > common_activation);
        drop(index);
        drop(db);
        fs::remove_file(path).unwrap();
    }
}
