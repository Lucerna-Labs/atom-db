# Stage 5: atomic cells

## Hypothesis

An application change is rarely one fact. If a bounded group of atoms, bonds,
and root transitions is surrounded by a verifiable membrane, recovery can make
the whole group visible or discard the whole group without introducing mutable
truth into the substrate.

The implementation calls that group a **cell**. This is the first-principles
equivalent of an atomic commit, derived from boundary, causality, identity, and
fixed-point recovery rather than from a conventional transaction API.

## Durable shape

A committed cell is written in canonical order:

1. a begin membrane containing the cell identity and bounded item counts;
2. new atom frames ordered by identity;
3. new bond frames ordered by identity;
4. root-transition frames ordered by root-name identity;
5. a commit membrane repeating the identity and counts.

The cell identity is SHA-256 over a domain-separated canonical manifest of the
contained atom, bond, and root-transition identities. Frame checksums still
protect transport integrity independently from content identity.

## Local laws

1. Nothing inside an open cell is observable.
2. Both membranes must agree on identity and item counts.
3. Every contained frame must independently balance its identity and checksum.
4. A contained bond may reference an earlier atom or an atom in the same cell.
5. A root name must be an atom.
6. A root target must be an existing fact or a fact in the same cell.
7. Every root transition records its prior target and prior version identity.
8. Root-version identities form an immutable causal chain, even when targets
   cycle through earlier values.
9. A root becomes current only after the commit membrane is durable.
10. Recovery truncates an incomplete cell back to its begin membrane.
11. Completed corruption still fails closed and is never silently discarded.
12. Cells and individual payloads have explicit finite capacities.

## Roots and snapshots

A root is an append-only statement that a name atom currently points to an atom
or bond. Updating or clearing a root writes another immutable transition; it
does not overwrite its history.

`snapshot()` captures the current root field at the durable sequence frontier.
`snapshot_at(sequence)` reconstructs an earlier field using cell commit
sequences, so no transition becomes historically visible before its closing
membrane. Historical snapshots survive close and reopen.

## Falsification gates

Stage 5 fails if:

1. any tested truncation inside a cell exposes a contained fact;
2. recovery removes facts in the committed prefix before the cell;
3. an invalid cell changes memory or file length;
4. a bond can commit before one of its member atoms;
5. mismatched membranes or counts are accepted;
6. root history fails to reconstruct after reopen;
7. a historical snapshot changes after a later commit;
8. repeated root cycles reuse a prior version identity;
9. legacy standalone atoms or bonds stop reopening;
10. the crate gains an external dependency.

## First release experiment

The local experiment committed two cells. The first admitted three atoms and a
root pointing to version one. The second admitted version two, a `supersedes`
bond, and a root transition to version two.

- first commit sequence: `5`
- second commit sequence: `10`
- atoms: `4`
- bonds: `1`
- active roots: `1`
- root transitions: `2`
- committed cells: `2`
- durable facts: `7`
- physical frames including membranes: `11`
- durable bytes: `1632`

After reopening, the first snapshot still resolved version one and the current
snapshot resolved version two.

## Honest boundary

Atomic cells solve multi-fact crash visibility, durable named roots, causal root
history, and reconstructible snapshots. They do not yet provide inter-process
writer exclusion, read-only concurrent handles, full directional bond indexes,
index checkpoints, reachability compaction, or replication. The current store
must still be treated as a single-process embedded database.
