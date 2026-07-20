use crate::{AtomDb, Digest};
use std::collections::{BTreeMap, BTreeSet};

const PER_MILLE: u128 = 1_000;
const BASE_CONDUCTANCE: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CognitiveConfig {
    pub working_memory_capacity: usize,
    pub max_depth: usize,
    pub cue_activation: u64,
    pub context_activation: u64,
    pub propagation_per_mille: u16,
    pub highway_threshold: u32,
    pub observation_half_life: u64,
}

impl Default for CognitiveConfig {
    fn default() -> Self {
        Self {
            working_memory_capacity: 12,
            max_depth: 5,
            cue_activation: 1_000_000,
            context_activation: 350_000,
            propagation_per_mille: 700,
            highway_threshold: 1_100,
            observation_half_life: 32,
        }
    }
}

impl CognitiveConfig {
    pub fn validate(self) -> Result<Self, String> {
        if self.working_memory_capacity == 0 || self.working_memory_capacity > 4_096 {
            return Err("working-memory capacity must be between 1 and 4096".into());
        }
        if self.max_depth == 0 || self.max_depth > 64 {
            return Err("recall depth must be between 1 and 64".into());
        }
        if self.cue_activation == 0 || self.context_activation == 0 {
            return Err("cue and context activation must be nonzero".into());
        }
        if self.propagation_per_mille == 0 || self.propagation_per_mille > 1_000 {
            return Err("propagation must be between 1 and 1000 per mille".into());
        }
        if self.highway_threshold < BASE_CONDUCTANCE {
            return Err("highway threshold cannot be below untouched conductance".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BondMemory {
    pub attempts: u64,
    pub successes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedAtom {
    pub identity: Digest,
    pub activation: u64,
    pub depth: usize,
    pub incoming_signals: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallThread {
    pub atoms: Vec<Digest>,
    pub bonds: Vec<Digest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallReport {
    pub cue: Digest,
    pub contexts: Vec<Digest>,
    pub ranked: Vec<ActivatedAtom>,
    pub target: Option<Digest>,
    pub target_found: bool,
    pub thread: Option<RecallThread>,
    pub intersections: Vec<Digest>,
    pub explored_bonds: usize,
    pub working_set_peak: usize,
    pub promoted_hops: usize,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug)]
struct Arc {
    bond: Digest,
    to: Digest,
}

#[derive(Clone, Copy, Debug)]
struct Trace {
    activation: u64,
    depth: usize,
    incoming_signals: usize,
    predecessor: Option<(Digest, Digest)>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Candidate {
    activation: u64,
    incoming_signals: usize,
    strongest_signal: u64,
    predecessor: Option<(Digest, Digest)>,
}

#[derive(Debug)]
pub struct CognitiveEngine {
    config: CognitiveConfig,
    bond_memory: BTreeMap<Digest, BondMemory>,
    observations: u64,
}

impl CognitiveEngine {
    pub fn new(config: CognitiveConfig) -> Result<Self, String> {
        Ok(Self {
            config: config.validate()?,
            bond_memory: BTreeMap::new(),
            observations: 0,
        })
    }

    pub fn config(&self) -> CognitiveConfig {
        self.config
    }

    pub fn observations(&self) -> u64 {
        self.observations
    }

    pub fn bond_memory(&self, identity: Digest) -> BondMemory {
        self.bond_memory.get(&identity).copied().unwrap_or_default()
    }

    pub fn bond_conductance(&self, identity: Digest) -> u32 {
        let Some(memory) = self.bond_memory.get(&identity) else {
            return BASE_CONDUCTANCE;
        };
        let quality =
            ((memory.successes as u128 + 1) * 1_000 / (memory.attempts as u128 + 2)) as u32;
        let experience = memory.attempts.min(100) as u32 * 4;
        (500 + quality + experience).clamp(250, 2_000)
    }

    pub fn recall(
        &mut self,
        db: &AtomDb,
        cue: Digest,
        contexts: &[Digest],
        target: Option<Digest>,
    ) -> Result<RecallReport, String> {
        if !db.contains_atom(cue) {
            return Err(format!("cue atom {cue} does not exist"));
        }
        for context in contexts {
            if !db.contains_atom(*context) {
                return Err(format!("context atom {context} does not exist"));
            }
        }
        if let Some(target) = target
            && !db.contains_atom(target)
        {
            return Err(format!("target atom {target} does not exist"));
        }

        let graph = build_graph(db);
        let mut traces = BTreeMap::new();
        traces.insert(
            cue,
            Trace {
                activation: self.config.cue_activation,
                depth: 0,
                incoming_signals: 1,
                predecessor: None,
            },
        );
        for context in contexts {
            let trace = traces.entry(*context).or_insert(Trace {
                activation: 0,
                depth: 0,
                incoming_signals: 0,
                predecessor: None,
            });
            trace.activation = trace
                .activation
                .saturating_add(self.config.context_activation);
            trace.incoming_signals += 1;
        }

        let mut active: Vec<(Digest, u64)> = traces
            .iter()
            .map(|(identity, trace)| (*identity, trace.activation))
            .collect();
        sort_active(&mut active);
        active.truncate(self.config.working_memory_capacity);
        let mut expanded = BTreeSet::new();
        let mut explored_bonds = BTreeSet::new();
        let mut promoted_hops = 0usize;
        let mut working_set_peak = active.len();

        for depth in 1..=self.config.max_depth {
            if active.is_empty() {
                break;
            }
            let mut candidates: BTreeMap<Digest, Candidate> = BTreeMap::new();
            for (from, activation) in &active {
                expanded.insert(*from);
                let Some(arcs) = graph.get(from) else {
                    continue;
                };
                let degree = arcs.len().max(1) as u128;
                for arc in arcs {
                    explored_bonds.insert(arc.bond);
                    if expanded.contains(&arc.to) {
                        continue;
                    }
                    let conductance = self.bond_conductance(arc.bond);
                    if conductance >= self.config.highway_threshold {
                        promoted_hops += 1;
                    }
                    let transmitted = ((*activation as u128)
                        .saturating_mul(self.config.propagation_per_mille as u128)
                        .saturating_mul(conductance as u128)
                        / PER_MILLE
                        / PER_MILLE
                        / degree)
                        .min(u64::MAX as u128) as u64;
                    if transmitted == 0 {
                        continue;
                    }
                    let candidate = candidates.entry(arc.to).or_default();
                    candidate.activation = candidate.activation.saturating_add(transmitted);
                    candidate.incoming_signals += 1;
                    if transmitted > candidate.strongest_signal {
                        candidate.strongest_signal = transmitted;
                        candidate.predecessor = Some((*from, arc.bond));
                    }
                }
            }

            let mut next: Vec<(Digest, Candidate)> = candidates.into_iter().collect();
            next.sort_by(|(left_id, left), (right_id, right)| {
                right
                    .activation
                    .cmp(&left.activation)
                    .then_with(|| left_id.cmp(right_id))
            });
            next.truncate(self.config.working_memory_capacity);
            working_set_peak = working_set_peak.max(next.len());
            active.clear();
            for (identity, candidate) in next {
                let should_replace = traces
                    .get(&identity)
                    .is_none_or(|existing| candidate.activation > existing.activation);
                if should_replace {
                    traces.insert(
                        identity,
                        Trace {
                            activation: candidate.activation,
                            depth,
                            incoming_signals: candidate.incoming_signals,
                            predecessor: candidate.predecessor,
                        },
                    );
                }
                active.push((identity, candidate.activation));
            }
        }

        let target_found = target.is_some_and(|identity| traces.contains_key(&identity));
        let thread = target
            .filter(|_| target_found)
            .and_then(|identity| reconstruct_thread(identity, &traces));
        if target.is_some() {
            self.observe_feedback(&explored_bonds, thread.as_ref());
        }

        let mut ranked: Vec<ActivatedAtom> = traces
            .into_iter()
            .map(|(identity, trace)| ActivatedAtom {
                identity,
                activation: trace.activation,
                depth: trace.depth,
                incoming_signals: trace.incoming_signals,
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .activation
                .cmp(&left.activation)
                .then_with(|| left.identity.cmp(&right.identity))
        });
        let intersections = ranked
            .iter()
            .filter(|atom| atom.incoming_signals > 1)
            .map(|atom| atom.identity)
            .collect::<Vec<_>>();
        let explanation = explain(
            cue,
            contexts,
            target,
            target_found,
            thread.as_ref(),
            (explored_bonds.len(), working_set_peak, promoted_hops),
        );
        Ok(RecallReport {
            cue,
            contexts: contexts.to_vec(),
            ranked,
            target,
            target_found,
            thread,
            intersections,
            explored_bonds: explored_bonds.len(),
            working_set_peak,
            promoted_hops,
            explanation,
        })
    }

    fn observe_feedback(
        &mut self,
        explored_bonds: &BTreeSet<Digest>,
        thread: Option<&RecallThread>,
    ) {
        let successful = thread
            .map(|thread| thread.bonds.iter().copied().collect::<BTreeSet<_>>())
            .unwrap_or_default();
        for identity in explored_bonds {
            let memory = self.bond_memory.entry(*identity).or_default();
            memory.attempts = memory.attempts.saturating_add(1);
            if successful.contains(identity) {
                memory.successes = memory.successes.saturating_add(1);
            }
        }
        self.observations = self.observations.saturating_add(1);
        if self.config.observation_half_life != 0
            && self
                .observations
                .is_multiple_of(self.config.observation_half_life)
        {
            self.apply_half_life();
        }
    }

    fn apply_half_life(&mut self) {
        self.bond_memory.retain(|_, memory| {
            memory.attempts /= 2;
            memory.successes /= 2;
            memory.attempts != 0 || memory.successes != 0
        });
    }
}

fn build_graph(db: &AtomDb) -> BTreeMap<Digest, Vec<Arc>> {
    let mut graph: BTreeMap<Digest, Vec<Arc>> = BTreeMap::new();
    for (identity, bond) in db.all_bonds() {
        let members = [bond.source, bond.relation, bond.target];
        for from_index in 0..members.len() {
            for to_index in 0..members.len() {
                if from_index != to_index {
                    graph.entry(members[from_index]).or_default().push(Arc {
                        bond: identity,
                        to: members[to_index],
                    });
                }
            }
        }
    }
    for arcs in graph.values_mut() {
        arcs.sort_by_key(|arc| (arc.bond, arc.to));
        arcs.dedup_by_key(|arc| (arc.bond, arc.to));
    }
    graph
}

fn sort_active(active: &mut [(Digest, u64)]) {
    active.sort_by(|(left_id, left), (right_id, right)| {
        right.cmp(left).then_with(|| left_id.cmp(right_id))
    });
}

fn reconstruct_thread(target: Digest, traces: &BTreeMap<Digest, Trace>) -> Option<RecallThread> {
    let mut atoms = vec![target];
    let mut bonds = Vec::new();
    let mut current = target;
    let mut seen = BTreeSet::new();
    while let Some((from, bond)) = traces.get(&current)?.predecessor {
        if !seen.insert(current) {
            return None;
        }
        bonds.push(bond);
        atoms.push(from);
        current = from;
    }
    atoms.reverse();
    bonds.reverse();
    Some(RecallThread { atoms, bonds })
}

fn explain(
    cue: Digest,
    contexts: &[Digest],
    target: Option<Digest>,
    target_found: bool,
    thread: Option<&RecallThread>,
    metrics: (usize, usize, usize),
) -> String {
    let (explored_bonds, working_set_peak, promoted_hops) = metrics;
    let outcome = match target {
        Some(identity) if target_found => format!("target {identity} entered working memory"),
        Some(identity) => format!("target {identity} did not enter working memory"),
        None => "recall completed without supervised feedback".into(),
    };
    let path = thread.map_or(0, |thread| thread.bonds.len());
    format!(
        "cue={cue}; contexts={}; {outcome}; thread_hops={path}; explored_bonds={explored_bonds}; working_set_peak={working_set_peak}; promoted_hops={promoted_hops}",
        contexts.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bond;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_file(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "atom-db-cognition-{name}-{}-{nonce}.atoms",
            std::process::id()
        ))
    }

    fn atom(db: &mut AtomDb, text: &str) -> Digest {
        db.put_atom(text.as_bytes()).unwrap()
    }

    fn bond(db: &mut AtomDb, source: Digest, relation: Digest, target: Digest) -> Digest {
        db.put_bond(Bond {
            source,
            relation,
            target,
        })
        .unwrap()
    }

    #[test]
    fn recall_forms_an_explainable_thread() {
        let path = temp_file("thread");
        let mut db = AtomDb::open(&path).unwrap();
        let earth = atom(&mut db, "Earth");
        let orbits = atom(&mut db, "orbits");
        let sun = atom(&mut db, "Sun");
        let relation = bond(&mut db, earth, orbits, sun);
        let mut mind = CognitiveEngine::new(CognitiveConfig::default()).unwrap();
        let report = mind.recall(&db, earth, &[], Some(sun)).unwrap();
        assert!(report.target_found);
        assert_eq!(report.thread.unwrap().bonds, vec![relation]);
        assert!(report.explanation.contains("entered working memory"));
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn context_changes_attention_without_changing_truth() {
        let path = temp_file("context");
        let mut db = AtomDb::open(&path).unwrap();
        let bank = atom(&mut db, "bank");
        let associated = atom(&mut db, "associated-with");
        let money = atom(&mut db, "money");
        let river = atom(&mut db, "river");
        let finance = atom(&mut db, "finance-context");
        let nature = atom(&mut db, "nature-context");
        bond(&mut db, bank, associated, money);
        bond(&mut db, bank, associated, river);
        bond(&mut db, finance, associated, money);
        bond(&mut db, nature, associated, river);
        let mut mind = CognitiveEngine::new(CognitiveConfig::default()).unwrap();
        let financial = mind.recall(&db, bank, &[finance], None).unwrap();
        let natural = mind.recall(&db, bank, &[nature], None).unwrap();
        assert!(activation(&financial, money) > activation(&financial, river));
        assert!(activation(&natural, river) > activation(&natural, money));
        assert_eq!(db.stats().unwrap().facts, 10);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn feedback_reinforces_success_and_inhibits_failure() {
        let path = temp_file("feedback");
        let mut db = AtomDb::open(&path).unwrap();
        let cue = atom(&mut db, "cue");
        let relation = atom(&mut db, "relation");
        let found = atom(&mut db, "found");
        let absent = atom(&mut db, "unconnected");
        let edge = bond(&mut db, cue, relation, found);
        let mut success = CognitiveEngine::new(CognitiveConfig::default()).unwrap();
        success.recall(&db, cue, &[], Some(found)).unwrap();
        assert!(success.bond_conductance(edge) > BASE_CONDUCTANCE);
        let mut failure = CognitiveEngine::new(CognitiveConfig::default()).unwrap();
        failure.recall(&db, cue, &[], Some(absent)).unwrap();
        assert!(failure.bond_conductance(edge) < BASE_CONDUCTANCE);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn working_memory_is_bounded_under_fanout() {
        let path = temp_file("capacity");
        let mut db = AtomDb::open(&path).unwrap();
        let cue = atom(&mut db, "cue");
        let relation = atom(&mut db, "relation");
        for index in 0..32 {
            let target = atom(&mut db, &format!("target-{index}"));
            bond(&mut db, cue, relation, target);
        }
        let config = CognitiveConfig {
            working_memory_capacity: 4,
            ..CognitiveConfig::default()
        };
        let mut mind = CognitiveEngine::new(config).unwrap();
        let report = mind.recall(&db, cue, &[], None).unwrap();
        assert!(report.working_set_peak <= 4);
        drop(db);
        fs::remove_file(path).unwrap();
    }

    fn activation(report: &RecallReport, identity: Digest) -> u64 {
        report
            .ranked
            .iter()
            .find(|atom| atom.identity == identity)
            .unwrap()
            .activation
    }
}
