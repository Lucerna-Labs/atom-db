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

The implementation is Rust standard library only. `Cargo.toml` has no dependencies, and SHA-256 is implemented and tested in this repository.

## Hypothesis

If immutable distinctions can name themselves, durable facts can only follow facts they reference, and each accepted fact is independently verifiable, then useful structured memory can be built from atoms and bonds without planting a conventional database ontology in the substrate.

## Run

~~~powershell
cargo run --release -- demo artifacts\universe.atoms
cargo run --release -- verify artifacts\universe.atoms
cargo run --release -- cognitive-demo artifacts\cognitive-stage2.atoms
~~~

Basic operations:

~~~powershell
cargo run --release -- init data.atoms
cargo run --release -- put data.atoms Earth
cargo run --release -- get data.atoms <64-hex-atom-id>
cargo run --release -- bond data.atoms <source-id> <relation-id> <target-id>
cargo run --release -- bonds data.atoms <source-id>
~~~

## Conservation laws

1. An atom's identity must equal the digest of its domain and exact bytes.
2. A bond's identity must equal the digest of its three ordered atom identities.
3. A bond cannot exist before all three member atoms exist.
4. A complete frame must balance against its stored checksum.
5. Causal sequence numbers are contiguous and monotonic.
6. Repeating an atom or bond changes no durable state.
7. A torn final frame is removed; corruption inside a complete frame is never silently repaired.

## Current boundary

This is the first executable substrate, not yet a general-purpose replacement for established databases. It deliberately has a single writer, rebuilds its in-memory observation structure on open, supports outgoing traversal only, and does not yet compact or replicate. Those capabilities must emerge as separately falsifiable primitive layers rather than being smuggled into Stage 1.

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

## Experiment status

Atom DB is intentionally experimental. Stage 1 establishes immutable atoms,
ternary bonds, verified framing, and crash recovery. Stage 2 adds a derived
cognitive observation field. The first cognitive run learned a useful finance
thread and also exposed a habit bias strong enough to override an opposing
context. That mixed outcome is preserved as evidence under `artifacts/` rather
than presented as a solved intelligence system.

## License

MIT. See [LICENSE](LICENSE).
