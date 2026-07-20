# Stage 7: Retrieval Field

## Question

Can an LLM recover useful local context from Atom DB without adding a search
engine, embedding runtime, external index, or third-party dependency?

## First-principles answer

The universe does not fetch a record from a central catalog. A need creates a
local disturbance. Compatible structures conduct that disturbance, independent
paths intersect, weak alternatives decay, and a bounded pattern stabilizes.

Atom DB applies that mechanism as a retrieval membrane:

1. Encoding converts a source document into bounded passage atoms.
2. The same deterministic transducer converts passage words and query words
   into namespaced term atoms.
3. Immutable `mentions` bonds connect terms to passages.
4. Immutable `from-source` bonds connect passages to provenance.
5. Every known query term injects an independent signal.
6. Signals traverse source-target arcs without turning shared relation atoms
   into accidental global hubs.
7. Passages touched by several cues form intersections and rank above
   single-cue matches.
8. Only a bounded set of evidence exits the field as an LLM context packet.

The durable substrate still contains only atoms, bonds, cells, and roots.
Retrieval has no authority to rewrite truth.

## Storage membrane

`Retriever::remember` segments UTF-8 text into passages of at most 768 bytes by
default. One atomic cell contains:

- the source label;
- each passage;
- the fixed retrieval relation atoms;
- normalized, namespaced term atoms;
- every term-to-passage and passage-to-source bond.

If the process fails before the cell commit membrane balances, none of the
document becomes visible. Repeated content continues to obey Atom DB's normal
content-addressed deduplication laws.

## Query membrane

`Retriever::retrieve` performs no writes. It rebuilds a derived source-target
graph from the committed bond set, injects up to 24 normalized cues, and uses a
finite field:

- maximum depth: 4;
- frontier capacity: 128 nodes;
- total node budget: 4,096;
- propagation: 700 per mille;
- term-passage conductance: 1,400 per mille;
- ordinary semantic conductance: 900 per mille;
- passage-source conductance: 600 per mille.

Every cue retains its own signal lane. Each `(node, cue)` pair propagates at
most once, so a later multi-hop cue may still join a node discovered by an
earlier cue without allowing cycles to amplify forever. A candidate's primary
rank is the number of distinct query cues reaching it, followed by total
activation, depth, and stable node identity. The strongest predecessor for each
cue reconstructs an exact atom/bond thread.

The field terminates when it has no unexpanded candidates or exhausts a declared
depth/node boundary. The packet exposes both `stabilized` and
`budget_exhausted`; it never hides an incomplete search behind a confident
result.

## LLM context packet

The CLI emits schema-1 JSON:

~~~powershell
cargo run --release -- retrieve knowledge.atoms "How does writer lease recovery work after a crash?"
~~~

The packet contains:

- normalized known and unknown cues;
- the store snapshot sequence observed;
- bounded passage text and a truncation flag;
- source labels;
- activation, depth, and incoming-signal counts;
- every supporting cue;
- exact atom and bond paths from each cue;
- traversal budget measurements;
- `answerable` and `insufficient_evidence` as explicit opposites.

Stored passage text is evidence, not instruction. An LLM adapter must delimit it
as untrusted data and must not treat text inside evidence as higher-priority
control input.

## Conservation laws

1. Ingestion and query use the same term transducer.
2. A retrieval query cannot create or mutate a durable fact.
3. Relation identities label arcs but do not become shared traversal hubs.
4. Each reported supporting cue has a reconstructible path to the evidence.
5. Evidence text obeys the declared context-byte budget.
6. Unknown cues cannot produce fabricated evidence.
7. Budget exhaustion is observable.
8. Ranking is deterministic for the same committed snapshot and configuration.
9. The implementation uses no third-party dependency.

## First experiment

The Stage 7 artifact contains two sources:

- `observer-leases.md`: writer-crash and lease-recovery evidence;
- `astronomy.md`: an unrelated Earth/Sun passage.

Query:

~~~text
How does writer lease recovery work after a crash?
~~~

Observed result:

- `answerable=true`;
- four known cues converged: `writer`, `lease`, `recovery`, `crash`;
- the observer-leases passage ranked first at depth 1;
- activation: `3,920,000`;
- four independent incoming signals and four exact provenance threads;
- one unrelated passage did not enter the context packet;
- field stabilized after 3 rounds without exhausting its budget;
- returned evidence text: 105 bytes.

Negative query:

~~~text
Where is the marmalade quasar?
~~~

Both cues were unknown. The result contained no evidence and returned
`insufficient_evidence=true`.

The durable artifact is `artifacts/stage7-retrieval-field.atoms`. The concise
measurement record is `artifacts/stage7-retrieval-field-report.txt`.

## Falsification boundary

This stage establishes bounded lexical graph retrieval, not general semantic
understanding. It currently:

- ~~rebuilds the derived graph from all bonds for each query~~ **(resolved in
  Stage 8: the index is cached against the snapshot sequence)**
- ~~has no persistent reverse index or checkpoint~~ **(partially resolved in
  Stage 8: an in-process derived index with rebuild telemetry; a durable
  checkpoint remains open)**
- uses conservative lexical normalization rather than embeddings;
- does not infer synonyms unless semantic bonds encode them;
- ~~does not persist cognitive conductance learning~~ **(resolved in Stage 9:
  reinforcement is a durable fact — see [STAGE-9-FEEDBACK-LOOP.md](STAGE-9-FEEDBACK-LOOP.md))**;
- returns evidence but does not invoke or supervise an LLM.

The next experiment should only add a capability if it has a measurable law.
Candidate laws include incremental traversal indexes, information-gain ranking,
and explicit feedback persistence. External embeddings, if explored, should be
optional on-ramps and must never become part of durable fact identity.

See [STAGE-8-INDEX-GAIN-OVERLAP.md](STAGE-8-INDEX-GAIN-OVERLAP.md) for the
resolved index, information-gain, and overlapping-window laws.
