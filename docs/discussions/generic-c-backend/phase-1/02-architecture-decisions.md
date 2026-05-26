# Phase 1 · Doc 2 — Architecture decisions

**Status: SKELETON.** Each decision below carries a **locked direction**
(already agreed — do not re-litigate) and must be filled with a
**concrete resolution + rationale** (architect rule: settle here, not in
implementation). Decisions that change the IR→C contract are written into
the `../../regseq-model/generic-c-backend.md` spec delta. Fill after Doc 1
(reference exact constructs); Codex first-contact on the genuinely open
forks (D2 declaration order, D3 file layout, D5 terminators).

> **Resolve-template per decision:** *Problem* → *Options considered* →
> **Decision** → *Rationale / consequences* → *Spec-delta?* (does this
> change `generic-c-backend.md`).

## D1 — Naming (identifiers for emitted C)

**Locked direction.** Lean on `Instance.emission_name` (bridge-minted,
"uniqueness guaranteed") for functions/statics/consts; interned
`FieldLayout.name` for fields; derive **type** names (struct/enum/union/
tuple/closure) from `DefPath`. Must be deterministic across the
build-equivalence quad (for golden-output tests).
**To resolve:** the exact type-name scheme; confirm `emission_name`'s
uniqueness/determinism scope; collision handling; the `DefPath`-qualified
form (also feeds D3). *(TO FILL.)*

## D2 — Declaration order (C declare-before-use)  ⚠ open fork

**Locked direction.** Forward-declare *everything* up front (function
prototypes + type forward-decls + `extern` statics), then definitions in
deterministic (F4) order; the only hard ordering constraint is a
topological sort of **type definitions** by *by-value containment*
(pointers/recursion broken by forward-decls).
**To resolve:** the exact prelude structure; the containment topo-sort +
cycle handling; where the line between fwd-decl and definition sits.
Codex first-contact. *(TO FILL.)*

## D3 — File layout / translation-unit partitioning  ⚠ open fork

**Locked direction.** **Single translation unit first** (for the
validation loop); encode the module hierarchy in `DefPath`-qualified
names so a later multi-file partition is a *no-rename* `DefPath`-prefix
split. Multi-file is gated/deferred.
**To resolve:** single-`.c` vs `.c`+`.h`; the include/guard story if a
`.h` is emitted; the harness `main()`/entry contract (how a fixture's
functions get invoked so logs fire). Codex first-contact. *(TO FILL.)*

## D4 — Host panic / abort strategy

**Locked direction.** `Assert` failure, `Unreachable`, and `panic=abort`
lower to a host trap: `printf` the message (where available) + `abort()`
/ `__builtin_trap()`.
**To resolve:** the exact runtime contract per case; what message each
`AssertKind` prints; interaction with the diag-log stdout oracle (so a
panic is distinguishable in golden output). *(TO FILL.)*

## D5 — Terminator C-emission  ⚠ KNOWN GAP (spec §7 defers all six)

**Locked direction.** Fully pin (not "approach") the C lowering for every
terminator; this is the core Phase-1 spec delta.
**To resolve, per variant:** `Return` (incl. unit/`void` + `Never`);
`Goto` (→ C `goto` label); `SwitchInt` (→ C `switch` vs if-chain — decide,
incl. the otherwise arm); `Call` — **`Direct`** (named call) vs
**`Indirect`** (call through fn-ptr operand), arg/destination/`target=None`
(divergent) handling; `Assert` (→ D4 trap path on failure, else fall
through to `target`); `Unreachable` (→ D4); MIR **`Drop`-as-`Call`** (no
separate variant — emit the call to drop-glue). Codex first-contact.
*(TO FILL — write into `generic-c-backend.md` §7.)*

## D6 — Diagnostic-log call interception

**Locked direction.** The log-marked item
(`AttrData::Intrinsic(IntrinsicKind::Log(level))` on the callee item) is a
**recognition tag**: intercept at each `CallCallee::Direct` call site and
emit the §8 `printf`; **do NOT emit a C definition** for the marked item
(its body is an empty marker). Args arrive already peeled flat
(`[fmt_const, a1..aN]`). Recognition is location-independent (attribute +
leaf fn name; keyed by `DefPathHash`).
**To resolve:** the exact call-site dispatch + format-template
re-parse/specifier mapping (cross-ref §8); how the marked item is excluded
from the instance-emission walk (D2 ordering). *(TO FILL.)*

## Open-decision log

Record any NEW fork surfaced while filling Doc 1/3 here, with the same
resolve-template, so nothing is silently decided in implementation.
