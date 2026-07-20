use crate::{AtomDb, Bond, Cell, CellReceipt, Digest, Error};
use atom_retrieval_field::{Arc, Cue, FieldConfig, FieldReport, activate, passages, terms};
pub use atom_retrieval_field::{ContextPacket, Evidence, EvidenceThread, RetrievalCue};
use std::collections::{BTreeMap, BTreeSet};

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
        }
    }
}

impl RetrievalConfig {
    fn validate(self) -> Result<Self, String> {
        self.field.validate()?;
        if !(64..=65_536).contains(&self.passage_bytes) {
            return Err("passage bytes must be between 64 and 65536".into());
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

#[derive(Clone, Copy, Debug)]
pub struct Retriever {
    config: RetrievalConfig,
}

impl Retriever {
    pub fn new(config: RetrievalConfig) -> Result<Self, String> {
        Ok(Self {
            config: config.validate()?,
        })
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
        let passage_texts =
            passages(document, self.config.passage_bytes).map_err(Error::Invalid)?;
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
        let query_terms = terms(query, self.config.max_query_terms);
        let mentions = identity(MENTIONS);
        let from_source = identity(FROM_SOURCE);
        let mut nodes = Vec::new();
        let mut node_index = BTreeMap::new();
        let mut edges = Vec::new();
        let mut edge_ids = Vec::new();
        let mut passage_ids = BTreeSet::new();
        let mut passage_sources: BTreeMap<Digest, BTreeSet<Digest>> = BTreeMap::new();
        for (edge_id, bond) in db.all_bonds() {
            let source = intern(bond.source, &mut nodes, &mut node_index);
            let target = intern(bond.target, &mut nodes, &mut node_index);
            let conductance = if bond.relation == mentions {
                passage_ids.insert(bond.target);
                1_400
            } else if bond.relation == from_source {
                passage_sources
                    .entry(bond.source)
                    .or_default()
                    .insert(bond.target);
                600
            } else {
                900
            };
            let edge = edge_ids.len();
            edge_ids.push(edge_id);
            edges.push(Arc {
                from: source,
                to: target,
                edge,
                conductance_per_mille: conductance,
            });
            edges.push(Arc {
                from: target,
                to: source,
                edge,
                conductance_per_mille: conductance,
            });
        }

        let mut cues = Vec::new();
        let mut field_cues = Vec::new();
        let mut known_terms = Vec::new();
        for term in query_terms {
            let identity = identity(&term_bytes(&term));
            let node = node_index.get(&identity).copied();
            cues.push(RetrievalCue {
                term: term.clone(),
                identity,
                known: node.is_some(),
            });
            if let Some(node) = node {
                known_terms.push(term);
                field_cues.push(Cue {
                    node,
                    activation: 1_000_000,
                });
            }
        }
        let field = activate(&edges, &field_cues, self.config.field).map_err(Error::Invalid)?;
        self.packet(
            db,
            query,
            snapshot_sequence,
            cues,
            known_terms,
            nodes,
            edge_ids,
            passage_ids,
            passage_sources,
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
        nodes: Vec<Digest>,
        edge_ids: Vec<Digest>,
        passage_ids: BTreeSet<Digest>,
        passage_sources: BTreeMap<Digest, BTreeSet<Digest>>,
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
}
