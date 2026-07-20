# Atom DB

> A Lucerna Labs first-principles database experiment.

Atom DB is a dependency-free database experiment derived from persistence's minimum requirements rather than from an existing database API.

It does not begin with tables, rows, documents, indexes, SQL, transactions, clients, servers, or schemas. The Stage 1 substrate contains only:

- **atoms**: immutable finite byte distinctions;
- **identity**: SHA-256 names derived from atom content;
- **bonds**: immutable `(source, relation, target)` connections, where every member is itself an atom;
- **causality**: one monotonic append order;
- **persistence**: self-delimiting frames with independent content identity and frame-integrity verification;
- **observation**: reconstruction of atoms and outgoing bonds from the durable fact stream.

The implementation uses the Rust standard library and first-party workspace
crates only. It has no third-party dependencies, and SHA-256 is implemented
and tested in this repository.

## Hypothesis

If immutable distinctions can name themselves, durable facts can only follow facts they reference, and each accepted fact is independently verifiable, then useful structured memory can be built from atoms and bonds without planting a conventional database ontology in the substrate.

## Run

~~~powershell
cargo run --release -- demo artifacts\universe.atoms
cargo run --release -- verify artifacts\universe.atoms
cargo run --release -- cognitive-demo artifacts\cognitive-stage2.atoms
cargo run --release -- context-demo artifacts\stage3-context-gating-final.atoms
cargo run --release -- relay-demo artifacts\stage4-guarded-relay.atoms
cargo run --release -- cell-demo artifacts\stage5-atomic-cells-final.atoms
cargo run --release -- lease-demo artifacts\stage6-observer-leases.atoms
cargo run --release -- remember artifacts\retrieval-local.atoms observer-leases.md "A writer crash releases its lease. Lease recovery permits a replacement writer."
cargo run --release -- retrieve artifacts\retrieval-local.atoms "How does writer lease recovery work after a crash?"
~~~

Basic operations:

~~~powershell
cargo run --release -- init data.atoms
cargo run --release -- put data.atoms Earth
cargo run --release -- get data.atoms <64-hex-atom-id>
cargo run --release -- bond data.atoms <source-id> <relation-id> <target-id>
cargo run --release -- bonds data.atoms <source-id>
cargo run --release -- remember data.atoms <source> <text...>
cargo run --release -- remember-file data.atoms <source> <file>
cargo run --release -- retrieve data.atoms <query...>
~~~

## Conservation laws

1. An atom's identity must equal the digest of its domain and exact bytes.
2. A bond's identity must equal the digest of its three ordered atom identities.
3. A bond cannot exist before all three member atoms exist.
4. A complete frame must balance against its stored checksum.
5. Causal sequence numbers are contiguous and monotonic.
6. Repeating an atom or bond changes no durable state.
7. A torn final frame is removed; corruption inside a complete frame is never silently repaired.
8. Facts inside an open cell are invisible until matching commit and begin membranes balance.
9. Every root transition extends the prior root-version identity as a causal chain.

## Current boundary

This is an executable local embedded substrate, not yet a general-purpose
replacement for established databases. Stage 5 provides atomic multi-fact
cells, append-only named roots, causal root history, and reconstructible
snapshots. Stage 6 adds one nonblocking OS-held writer lease, concurrent
read-only observers, non-destructive observation of provisional tails, and
refreshable committed views. Stage 7 adds atomic passage encoding and bounded,
multi-cue retrieval for LLM context. The storage substrate still lacks
persistent reverse-traversal indexes, index checkpoints, compaction, and
replication. Those capabilities must emerge as separately falsifiable
primitive layers rather than being smuggled into the substrate.

See [FIRST-PRINCIPLES.md](FIRST-PRINCIPLES.md) for the derivation and gates.

## Cognitive observation field

The optional cognitive layer treats the immutable bond graph as a local
associative field. Cues and contexts inject activation, bonds transmit bounded
activation, working-memory capacity limits each frontier, and multiple signals
form observable intersections. Explicit successful-target feedback reinforces
the responsible recall thread; explored alternatives are inhibited. Learned
conductance decays toward its neutral prior by a configurable half-life.

Cognitive measurements are derived and currently live only for the engine's
lifetime. They cannot create, mutate, delete, or reinterpret durable facts.
See [COGNITIVE-EXPERIMENT.md](COGNITIVE-EXPERIMENT.md) for its laws and
falsification boundary.

## Retrieval field

The Stage 7 retrieval membrane stores a source, bounded passages, normalized
term cues, and their bonds in one atomic cell. A query uses the same term
transducer as ingestion, injects all known cues simultaneously, and propagates
their signals through source-target arcs. Passages reached by several
independent cues form intersections and outrank one-cue matches.

Retrieval is read-only and returns a schema-1 JSON context packet containing
bounded evidence text, source labels, cue support, activation measurements,
and exact atom/bond threads. Unknown queries return
`insufficient_evidence=true` instead of inventing context. The field is split
into a focused first-party crate to preserve the 4,000-line crate ceiling. See
[RETRIEVAL-FIELD.md](RETRIEVAL-FIELD.md).

## Experiment status

Atom DB is intentionally experimental. Stage 1 establishes immutable atoms,
ternary bonds, verified framing, and crash recovery. Stage 2 adds a derived
cognitive observation field. The first cognitive run learned a useful finance
thread and also exposed a habit bias strong enough to override an opposing
context. That mixed outcome is preserved as evidence under `artifacts/` rather
than presented as a solved intelligence system.

Stage 3 locally compares global plasticity with context-gated plasticity. Both
learn the supervised finance route, but contextual lanes prevent that habit
from leaking into an opposing nature context. Context identity is normalized as
an order-independent, duplicate-free set.

Stage 4 adds guarded context relay. It constructs bounded context fields from
local graph topology, compares them with weighted overlap, and relays learned
conductance only above a compatibility guard. In the first three-law run it
improved an untrained but related banking recall while leaving the opposing
nature context untouched. Traces merge only at high compatibility, decay with
observation half-life, and obey a finite capacity with deterministic eviction.
See [GUARDED-CONTEXT-RELAY.md](GUARDED-CONTEXT-RELAY.md) for the mechanism,
numbers, and falsification boundary.

Stage 5 adds atomic cells. Matching begin and commit membranes make a bounded
group of atoms, bonds, and root transitions visible together. Incomplete cells
are removed back to their opening boundary, while complete corruption still
fails closed. Root transitions link to prior root-version identities, and
historical snapshots can be reconstructed after reopening. See
[ATOMIC-CELLS.md](ATOMIC-CELLS.md).

Stage 6 adds observer leases. A single writer authority coexists with multiple
read-only observers; a competing writer fails before file I/O, observers never
repair provisional bytes, and the operating system releases the writer lease
after a crash. A real child-process test verifies contention, concurrent
readers, forced writer termination, and immediate lease recovery. See
[OBSERVER-LEASES.md](OBSERVER-LEASES.md).

Stage 7 adds the first LLM retrieval membrane. The release experiment encoded
two sourced passages, reopened the store read-only, and resolved a natural
question through four independent cues converging on one passage. Each cue
returned its exact bond thread. A query whose cues were absent produced no
evidence and explicitly failed closed. The experiment is preserved under
`artifacts/stage7-retrieval-field.atoms`; see
[RETRIEVAL-FIELD.md](RETRIEVAL-FIELD.md) for the measured boundary.

## License

MIT. See [LICENSE](LICENSE).
