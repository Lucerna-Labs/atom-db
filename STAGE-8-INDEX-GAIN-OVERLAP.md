# Stage 8: Index, Gain, Overlap

## Question

Can the retrieval field serve repeated LLM queries without rebuilding its
derived graph per query, rank rare cues above common ones, and keep answers
that span passage boundaries — all without new dependencies?

## The three laws

### 1. Unchanged snapshot, no rebuild

The derived retrieval graph (nodes, arcs, passage sets, per-term document
frequencies) is cached against the durable snapshot sequence. A query whose
snapshot has not moved pays zero rebuild; a committed change forces exactly
one rebuild, observable through `Retriever::index_rebuilds()`.

Measured: `unchanged_snapshot_never_rebuilds_the_index` — two identical
queries produce identical packets; the counter moves 0, then exactly 1
after a commit.

### 2. Information gain

Every cue no longer injects the same flat signal. A cue's activation scales
with how rare its term is across committed passages:

```text
activation = 1_000_000 * (1 + log2(1 + total_passages / term_passages))
```

`term_passages` is the term's document frequency, computed during the same
rebuild as law 1 — no second pass over the store. Ranking remains
deterministic for a given snapshot and configuration. The law can be
disabled (`RetrievalConfig::information_gain = false`) to fall back to the
Stage 7 flat signal for measurement.

Measured: `rare_cues_inject_more_activation_than_common_cues` — a term
appearing in one of three passages injects strictly more than a term
appearing in all three, and its supported evidence ranks first.

### 3. Overlapping windows

Answers spanning a passage boundary no longer fragment. Consecutive windows
share a sentence-aligned tail (`passage_overlap_bytes`, default 128),
always reserving room for the next fresh sentence; no window exceeds the
passage budget. `passages()` keeps the pre-Stage-8 hard-boundary behavior
as the overlap=0 special case.

Measured: `overlapping_windows_keep_boundary_answers_whole` — a sentence
forced across a boundary appears whole in at least two windows, every
window within budget.

## Compatibility

The Stage 7 artifact reproduces its documented result unchanged:
`answerable=true`, the same first evidence with the same four supporting
cues and provenance threads, field stabilized without budget exhaustion.
Activation magnitudes differ from Stage 7 (information gain); ranks do not.

## Conservation laws (additions)

1. Cache identity is derived only from the durable snapshot sequence.
2. A query never mutates the cache's observed snapshot.
3. Information gain uses only facts already committed (document frequency);
   it reads no external statistics.
4. Overlap never duplicates content beyond the declared allowance, and no
   window exceeds the passage budget.

## Falsification boundary

This stage does not yet provide:

- a causal-order incremental index (the store exposes bonds in hash order;
  the cache rebuilds on change rather than appending);
- conductance learning from retrieval outcomes (the feedback loop — the
  next candidate law, and the Ordo Pro integration hook);
- synonym or alias bonds (recall still depends on shared surface terms);
- embeddings of any kind.
