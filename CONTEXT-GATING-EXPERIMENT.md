# Context-gated plasticity experiment

## Question

Can Atom DB retain successful reinforcement while preventing a learned habit
from overriding an opposing context?

## Controlled comparison

Both cognitive engines receive the same six atoms, four bonds, finance context,
money target, and eight feedback observations. The control stores one global
conductance field. The candidate stores conductance under a normalized set of
context identities.

## Result

Both scopes learned finance identically:

- money activation: `328020`;
- river activation: `110600`;
- bank-to-money conductance: `1432`.

Under nature context, global learning still selected the finance habit:

- money activation: `250600`;
- river activation: `188020`.

Context-gated learning preserved the opposing context:

- money activation: `175000`;
- river activation: `297500`;
- untouched nature conductance: `1000` for both candidate bonds.

Durable facts remained `10` before and after both cognitive runs.

## Disposition

Context-gated plasticity passes this narrow experiment. It is not yet promoted
as a universal law. Exact context sets can create too many independent lanes,
and no cross-context generalization law has been established. Those are explicit
boundaries for later falsification.
