# Stage 9: Feedback Loop

## Question

Can retrieval outcomes become durable facts that lawfully reshape future
retrieval — memory that helps conducts better, memory that misleads decays —
without a side channel, a background learner, or a new dependency?

## The law

Reinforcement is a first-class durable fact, written explicitly by the
caller, never implicitly by a query.

One `reinforce(passage, polarity)` call commits one atomic cell containing:

- a feedback relation atom whose bytes encode
  `atom-db/retrieval/feedback/v1/<polarity>/<count>`;
- one bond `(passage, relation, feedback-marker)`.

`<count>` is the pair's new total (previous count + 1, read from the
snapshot-current derived index). Because the relation identity includes the
count, events never deduplicate away, and because only the maximum count per
(passage, polarity) applies, the folded state is always the latest — the
append-only history *is* the counter.

## Conductance folding

On rebuild, the derived index folds feedback into the traversal graph:

```text
conductance(into reinforced passage) =
    clamp(1400 + 250 * strengthen_count - 400 * weaken_count, 1, 4000)
```

- Only arcs **into** a reinforced passage shift; the reverse arc keeps base
  conductance, so reinforcement cannot turn evidence into a global hub.
- Strengthen is gentler (+250) than weaken is harsh (−400): false positives
  cost more to bury than to boost.
- Feedback bonds never appear as traversal arcs.
- Queries remain read-only; the store stats are identical before and after
  any retrieve (law test `reinforce_rejects_unknown_atoms_and_queries_stay_read_only`).

## Measured

- `reinforcement_lifts_helpful_evidence_and_is_durable`: two strengthens
  raise a weak passage's activation monotonically; the effect survives a
  reopen (facts, not process state).
- `weakening_buries_misleading_evidence`: four weakens drop a misleading
  passage's activation or bury it out of the packet entirely.
- Guards: reinforcing a nonexistent atom fails closed; receipts expose the
  count and effective delta for telemetry.

CLI:

```powershell
cargo run --release -- reinforce data.atoms <passage-id>            # strengthen
cargo run --release -- reinforce data.atoms <passage-id> weaken
```

## Conservation laws (additions)

1. A query never writes; reinforcement is the only feedback path and is
   always an explicit caller decision.
2. Feedback counts come only from committed feedback bonds; the latest
   count supersedes, never sums history.
3. Conductance shifts are clamped and directional (into evidence only).
4. Ranking stays deterministic for a given snapshot and configuration,
   feedback included.

## The Ordo Pro hook

This is the membrane Ordo Pro's self-learning tree plugs into: after a
turn, the caller reinforces the evidence the answer actually used
(strengthen) or the evidence that misled (weaken). Learning is durable
fact, and every conductance change is reconstructible from the store.

## Falsification boundary

- Feedback applies at passage granularity, not per (term, passage) arc.
- Weakened evidence is suppressed, never deleted (durable truth is
  immutable; suppression is observational).
- No temporal decay: counts are monotonic until superseded by new events.
- The caller decides what "helped"; the substrate does not judge answers.
