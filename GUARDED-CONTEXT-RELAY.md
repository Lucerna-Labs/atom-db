# Stage 4: guarded context relay

## Hypothesis

Exact context lanes stop harmful cross-context habits, but they also prevent a
useful lesson from reaching a nearby situation. A bounded field derived only
from the immutable bond graph can relay learned conductance between related
contexts while rejecting an opposing context.

This is not embedding similarity and it does not inspect atom text. Context is
the weighted shape around context atoms in the graph.

## Local laws

1. Every context atom begins with field weight `1000`.
2. Field weight propagates for a bounded number of graph hops.
3. Source degree divides outgoing weight and target degree penalizes generic
   hubs.
4. Only the strongest bounded set of field atoms survives.
5. Two fields are compared by deterministic weighted Jaccard overlap.
6. A trace below the relay guard contributes no learned conductance.
7. A trace above the guard contributes in proportion to compatibility.
8. Multiple eligible traces share a bounded contribution; they cannot amplify
   conductance without limit.
9. Only highly compatible observations merge into one trace.
10. Trace count is finite. When full, the least-observed trace is evicted with
    a deterministic tie break.
11. Observation half-life erodes trace memory and removes empty traces.
12. Context traces remain derived cognitive state and cannot modify durable
    atoms or bonds.

Defaults bound a field to 64 atoms over two hops, guard relay below `50/1000`,
merge only at `800/1000`, and retain at most 32 traces.

## Three-law comparison

The executable experiment trains the ambiguous cue `bank` toward `money` in a
finance context, then observes an untrained but structurally related banking
context and an opposing nature context.

| Learning law | Trained finance | Related banking | Opposing nature |
| --- | --- | --- | --- |
| Global | Learns | Receives the habit | Also receives the habit |
| Exact contextual | Learns | Receives nothing | Receives nothing |
| Guarded relay | Learns | Receives weighted transfer | Rejected by guard |

The graph measured finance-to-banking compatibility as `103/1000` and
finance-to-nature compatibility as `18/1000`. With the `50/1000` relay guard,
banking inherited bank-to-money conductance `1044`, while nature remained at
the untouched prior `1000`. The fully trained finance field measured `1432`.

This changed observable recall: banking money activation rose from `175000`
under exact lanes to `182700` under guarded relay. Nature still selected river
(`236250`) over money (`175000`). Global learning produced the known failure:
nature selected money (`250600`) over river (`149310`).

## Falsification gates

Stage 4 fails if any of these occur:

1. a related field above the guard receives no positive transfer;
2. an opposing field below the guard receives learned conductance;
3. relay makes related recall no better than the exact-lane control;
4. guarded nature recall stops preferring river;
5. field or trace capacity is exceeded;
6. eviction or equal-score behavior is nondeterministic;
7. learned conductance escapes its configured bounds;
8. observation changes the durable fact count;
9. the implementation adds a third-party dependency.

## Honest boundary

The result shows selective structural generalization on one deliberately small
graph. The guard has a real but narrow margin (`103` versus `18`); it is not
evidence of language understanding, universal semantics, or consciousness.
Graph topology can still be misleading, especially in sparse or badly linked
data. A broader adversarial corpus must test false friends, generic hubs,
conflicting traces, capacity pressure, and long-run erosion before guarded
relay becomes the default learning law.
