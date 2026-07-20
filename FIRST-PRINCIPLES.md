# First-principles derivation

## The outcome

A database is matter that remembers distinctions after the observer and the machine process disappear, then lets a later observer recover those distinctions and their relationships without accepting invented or corrupted history.

## Remove inherited names

Before implementation, remove table, row, column, document, key/value, query language, transaction, page, index, cache, client, server, and cluster. Each is a possible later arrangement, not a primitive of memory.

## Primitive stack

1. **Distinction** — finite bytes differ from other finite bytes.
2. **Identity** — deterministic mapping gives equal distinctions equal names.
3. **Set** — equal identities occupy one durable fact, so repetition is stable.
4. **Relation** — an ordered triple connects existing distinctions without assigning application meaning to the substrate.
5. **Causality** — a fact may reference only identities already admitted.
6. **Boundary** — lengths and frame markers separate one durable fact from the next.
7. **Invariant** — identity and checksum must recompute exactly during recovery.
8. **Fixed point** — replaying the durable prefix reconstructs the same observed state; inserting the same fact again leaves that state unchanged.

These choices are grounded in the supplied mathematical-primitives catalog's set, relation/graph, hash, checksum, causality, loop-invariant, and fixed-point families. The catalog file is an OOXML Word document despite its `.md` suffix.

## Why two hashes are stored conceptually

The content identity answers "what fact is this?" The frame checksum answers "did these exact causal and framing bytes arrive intact?" Combining them would confuse identity with transport integrity. Both currently use the repository's SHA-256 primitive with separate domain prefixes.

## Falsification gates

Stage 1 fails if any of these are true:

1. the SHA-256 implementation misses a standard vector;
2. equal bytes produce different atom identities;
3. inserting the same atom creates another fact;
4. a bond can precede a member atom;
5. atoms or bonds change across close/reopen;
6. a torn final frame is treated as a fact;
7. completed corruption is silently accepted or discarded;
8. causal sequence discontinuity is accepted;
9. recovery produces different state from the same durable bytes;
10. the crate acquires a runtime dependency.

Passing earns the right to test the next primitive layer. It does not prove that the substrate is already a production database.

## Stage 5: atomic cells

Stage 5 derives a multi-fact visibility law from the boundary primitive. A
begin membrane declares a canonical cell identity and bounded counts. Contained
facts remain provisional until a matching commit membrane becomes durable.
Recovery discards an open cell back to its begin boundary, so a crash exposes
either the earlier fixed point or the complete new fixed point.

Named roots are immutable transitions inside cells. Each transition includes
the previous target and previous root-version identity, making the root history
a causal chain. Snapshots are observations of the root field at a committed
sequence boundary and can be reconstructed after reopen.

## Stage 6: observer leases

Stage 6 derives access coordination from causal authority. Exactly one writer
may extend the sequence, while multiple observers may inspect completed
prefixes. The writer lease belongs to the operating-system file handle and
vanishes with process death; it is not represented as a durable fact.

Observers have no repair authority. They stop at an incomplete frame or open
cell, report the provisional tail, and retain the earlier fixed point until an
explicit refresh reconstructs a newer verified frontier.

## Stage 2: cognitive observation

Stage 2 adds no new durable fact kind. It constructs a deterministic activation
field over existing ternary bonds. Association, context, attention, working
memory, reinforcement, inhibition, decay, recall threads, intersections, and
explanation are observations over truth rather than truth themselves.

## Candidate next experiments

- symmetric bond observation without duplicating durable truth;
- immutable checkpoint atoms that accelerate replay and verify their source prefix;
- compare-and-append boundaries for concurrent observers;
- segment roots and proof paths for localized verification;
- graph-shaped query composition built from bond traversal;
- survival experiments under bit flips, truncation, reordering, and lost writes.

No candidate is promoted merely because conventional databases contain an analogous feature. It must state a primitive, an invariant, and a falsifier.
