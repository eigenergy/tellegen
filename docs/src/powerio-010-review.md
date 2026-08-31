# PowerIO 0.10 consumer review

PowerIO 0.10.0 is the baseline for this review. The corrections found while
integrating Tellegen are being developed in
[powerio#453](https://github.com/eigenergy/powerio/pull/453) and
[powerio#454](https://github.com/eigenergy/powerio/pull/454) for PowerIO 1.0.0.
The 0.10.0 tag and its stored files remain unchanged.

Tellegen consumes PowerIO modules at its public boundaries. `DcNetwork` and
`AcNetwork` are private solver workspaces built from a PowerIO problem instance.
The browser and CLI save PowerIO case and solution modules; Tellegen defines no
second portable network, study, or experiment format.

## Preparation semantics

The 0.10.0 preparation entry points exposed numerical arrays, but they did not
compile every part of the supplied OPF instance. A consumer could therefore ask
PowerIO to prepare one problem and silently solve another.

The PowerIO 1.0.0 candidate preparation data addresses the complete instance:

- `Empty` and `NetworkGeneratorCost` objectives compile explicitly. Unsupported
  terms return a typed error.
- Each constraint family carries an active mask aligned with its numerical
  arrays. `ConstraintSelection::Only` validates every named identity.
- Bus, generator, and branch identities remain aligned with the dense arrays.
  Analysis row mappings are separate from source row mappings.
- Three winding transformers use stable synthesized winding identities. A
  synthesized winding has an analysis row and no source row.
- DC and AC preparation expose the same identity and selection rules. AC also
  records which thermal limits were supplied and which were synthesized.

These fields let Tellegen interpret solve results without rebuilding an
`IndexedNetwork` or relying on an undocumented lowering order.

## Objective and result conventions

PowerIO problem objectives now contain only declared physical or economic
terms. Numerical regularization belongs to the solver formulation and is
reported separately by Tellegen.

DC and AC solution modules use convention neutral result fields:

- an active demand marginal is the change in the declared objective per unit
  of added active demand;
- an AC reactive demand marginal follows the same rule for reactive demand;
- the from end and to end thermal limit multipliers are stored separately;
- marginal and multiplier units are objective units per selected power unit.

The values are LMPs only when the declared objective and power unit make that
interpretation valid. Tellegen does not hardcode a currency or collapse the two
thermal constraints into a signed column.

Finite difference tests in PowerIO and Tellegen check demand signs, rating
derivatives, both directional thermal multipliers, and the weighted marginal
value vector product used by capacity planning.

## Stored values and upgrades

The new solution columns change the stored value API and are part of the
PowerIO 1.0.0 candidate. They fit in `powerio.module/1`, so the module envelope
version does not change. The migration guide covers reading 0.10 modules and
the new result fields. A stored 0.10 objective containing the retired
regularization token loads with a diagnostic rather than becoming a current
public objective.

When Tellegen commits edits, it retains valid module diagnostics, history,
extensions, and producer data; replaces the module value with the committed
network; and severs obsolete source targets. A saved exact solution contains
the amended OPF instance that was solved.

## Other corrections

The same review found several smaller public contract gaps:

- `HistoryId` and `HistoryKind` join the existing history types at the PowerIO
  facade.
- MCP responses retain the documented schema and PowerIO version header.
- the susceptance migration text distinguishes the negative public branch
  susceptance from the positive solver edge weight;
- incidence formulas use dimensionally valid matrix products;
- DC and AC instance network replacement is checked and rebinds compatible
  initial state data.

Tellegen pins the reviewed PowerIO commit while both pull requests are open.
The pin is replaced by the published PowerIO release before Tellegen is
packaged.
