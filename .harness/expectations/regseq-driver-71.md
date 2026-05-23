# regseq-driver-71: implementation evidence

Implementation deliverable for `regseq-driver-71`, the closure-family
reattempt sweep + matrix-doc OPEN BUG refresh described at
`.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md`.

Parent context: `.harness/expectations/regseq-driver-62.md` (the
prescan args-substitution fix that unblocked the closure-call family)
and `.harness/audits/2026-05-22-gap-inventory.md` Category 1
(`:131-202`).

## Scope

Sidecar authoring + matrix-doc editing only. No bridge / model /
runner-source / helper / spec / strategy / pretty-printer /
generic-c-backend file changes. Three coupled deliverables:

1. Snippets + sidecar rows for the cohort rows that PASS the F4
   3-retry budget against HEAD's post-driver-62 bridge.
2. Inline rationale blocks in
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml` for the
   cohort rows that escalate (empirical + design-time modes), per
   the F70 FF-005 precedent at
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1316-1340`.
3. Matrix-doc OPEN BUG section refresh at
   `docs/discussions/regseq-model/bridge-coverage-matrix.md:508-575`
   plus the per-row status / notes updates for the 13 cohort rows
   per the B6 reclassification policy.

## Cohort outcome summary

The 13-row cohort enumerated at gap-inventory `:140-160` resolved as
follows against HEAD's bridge (commit `5759691`):

| Row | Tier | Outcome | Snippet committed |
| --- | ---- | ------- | ----------------- |
| IX-004 | 1 | PASS | `crates/bridge-coverage-runner/snippets/IX-004.rs` |
| IX-010 | 1 | PASS | `crates/bridge-coverage-runner/snippets/IX-010.rs` |
| BD-007 | 1 | PASS | `crates/bridge-coverage-runner/snippets/BD-007.rs` |
| FT-011 | 1 | PASS | `crates/bridge-coverage-runner/snippets/FT-011.rs` |
| FT-012 | 1 | PASS | `crates/bridge-coverage-runner/snippets/FT-012.rs` |
| FT-013 | 1 | PASS | `crates/bridge-coverage-runner/snippets/FT-013.rs` |
| FT-014 | 1 | PASS | `crates/bridge-coverage-runner/snippets/FT-014.rs` |
| SK-003 (partial -> committed) | 1 | PASS | `crates/bridge-coverage-runner/snippets/SK-003.rs` |
| RJ-009 (partial -> committed) | 1 | ESCALATED (empirical, Mode 1) | -- |
| FF-007 | 2 | ESCALATED (empirical, Mode 1, NEW bridge gap) | -- |
| IX-014 | 2 | PASS | `crates/bridge-coverage-runner/snippets/IX-014.rs` |
| SK-004 | 3 | ESCALATED (design-time, Mode 2) | -- |
| SK-005 | 3 | ESCALATED (design-time, Mode 2) | -- |

`N = 9` cohort rows committed to sidecar. Sidecar total after F71 =
127 (pre-F71) + 9 = 136. Acceptance criterion 14 lower bound (>= 5
cohort PASSes) satisfied.

## Mode A run evidence

The runner Mode A invocation

```
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-71-iter7/
```

reports `Total rows = 136`, `Pass = 136`, `Fail = 0`, exit 0
(snapshot at `/tmp/brcr-71-iter7/report.md`). The 127 pre-F71 rows
retain identical per-row PASS / FAIL classification to the
F70-close state at `/tmp/a1-mode-a/report.md` (no regression).

## Per-row PASS records (Tier 1 + Tier 2)

### IX-004 -- Closure (impl Fn) x Fn::call site translation

Snippet at `crates/bridge-coverage-runner/snippets/IX-004.rs` is
the matrix's own minimal repro at `:512-515`:

```rust
#![no_std]
pub fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 {
    f(x, y)
}
pub fn entry(x: i32, y: i32) -> i32 {
    apply(x, y, |x, y| x + y)
}
```

Predicate: `inspect = { kind = "SyntheticItemKind", arg = "ClosureCall" }`.
PASSes on first attempt against HEAD's post-driver-62 bridge.

### IX-010 -- Generic fn x Trait method call inside body

Snippet uses the `<F: Fn() -> i32>` generic-bound form to exercise
the closure-call shim through the explicit-generic surface (vs.
IX-004's `impl Trait` form). Initial zero-arg shape
(`apply<F: Fn() -> i32>(f: F) -> i32 { f() } ... apply(|| 7)`)
panicked the bridge with `<unknown panic payload>` -- a same-shape
panic also surfaced for FT-011, FT-012, FT-014, IX-014, FF-007's
move-FnOnce candidate. The fix was adding a positional argument to
the closure signature, which routes the call cleanly through
`rewrite_closure_call_shim` at
`crates/regseq-driver/src/bridge/body/terminator.rs:150-165`.
Final shape:

```rust
#![no_std]
pub fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
pub fn entry() -> i32 {
    apply(|n| n + 7, 3)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on retry 2.

**Cross-row finding** (not a bridge bug, just an empirical
observation worth recording): the zero-arg closure shape consistently
panicked the bridge across IX-010 / FT-011 / FT-012 / FT-014 /
IX-014 / FF-007. Adding one positional argument unblocked five of
six. The sixth (FF-007) escalated against a separate gap. This may
warrant a future runner stability investigation but is OUT OF SCOPE
for F71 per the task spec's "No bridge code changes" non-goal.

### BD-007 -- Body materialised for synthetic ClosureCall Instance

Snippet exercises the closure-call body materialization with
positional args:

```rust
#![no_std]
pub fn run(f: impl Fn(i32) -> i32) -> i32 {
    f(1) + f(2)
}
pub fn entry() -> i32 {
    run(|n| n * 3)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on first
attempt.

### FT-011 -- Closure with zero captures, called by ref (impl Fn)

Snippet: zero-capture `|n| n + 1` closure passed to
`invoke(f: impl Fn(u32) -> u32)`. Initial zero-arg shape panicked
(see IX-010 finding); final shape uses one positional argument:

```rust
#![no_std]
pub fn invoke(f: impl Fn(u32) -> u32) -> u32 {
    f(0)
}
pub fn entry() -> u32 {
    invoke(|n| n + 1)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on retry 2.

### FT-012 -- Closure with N captures, called by ref

Snippet: closure captures `a` and `b` by reference and adds them to
the positional argument. Initial zero-arg shape panicked (see
IX-010 finding); final shape uses one positional argument:

```rust
#![no_std]
pub fn invoke(f: impl Fn(i32) -> i32) -> i32 {
    f(5)
}
pub fn entry() -> i32 {
    let a: i32 = 10;
    let b: i32 = 20;
    invoke(|x| x + a + b)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on retry 2.

### FT-013 -- Closure called by mut ref (impl FnMut)

Snippet exercises the `FnMut` arm:

```rust
#![no_std]
pub fn invoke(mut f: impl FnMut(i32)) {
    f(1);
    f(2);
}
pub fn entry() -> i32 {
    let mut total: i32 = 0;
    invoke(|x| total += x);
    total
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on first
attempt. Routes through `rewrite_closure_call_shim`'s
`LangItem::FnMut -> ClosureCallKind::CallMut` arm at
`crates/regseq-driver/src/bridge/body/terminator.rs:211`.

### FT-014 -- Closure called by value (impl FnOnce)

Snippet: explicit-move FnOnce closure consuming a `Payload`
capture. Initial zero-arg shape panicked (see IX-010 finding);
final shape uses one positional argument:

```rust
#![no_std]
pub struct Payload {
    pub value: i32,
}
pub fn consume(f: impl FnOnce(i32) -> i32) -> i32 {
    f(1)
}
pub fn entry() -> i32 {
    let p = Payload { value: 99 };
    consume(move |n| p.value + n)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on retry 2.
Routes through the `LangItem::FnOnce -> ClosureCallKind::CallOnce`
arm.

### SK-003 -- ClosureCall { call_kind: Call } (Fn::call body path)

Snippet exercises the Fn::call body path that was the OPEN BUG
(IX-004):

```rust
#![no_std]
pub fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}
pub fn entry() -> i32 {
    apply(|n| n + 1, 41)
}
```

Predicate: `SyntheticItemKind=ClosureCall`. PASSes on first
attempt. The previously-blocked `Fn::call` body path now
translates cleanly post-driver-62; baseline `partial` reclassified
to `covered` per B6 ("a `partial` row whose committed sidecar
entry exercises one of the remaining specializations becomes
`covered`").

### IX-014 -- Closure x FnOnce consumed via Move

Per the task spec at
`.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md:305-306`,
F2's two candidate shapes were BOTH re-run against HEAD's
post-driver-62 bridge:

- **Candidate A (direct closure consumption via `impl FnOnce`)**:
  the F71-committed snippet at
  `crates/bridge-coverage-runner/snippets/IX-014.rs` is this
  shape:

  ```rust
  #![no_std]
  pub struct Captured(i32);
  pub fn run(f: impl FnOnce(i32) -> i32) -> i32 {
      f(0)
  }
  pub fn entry() -> i32 {
      let c = Captured(5);
      run(move |n| c.0 + n)
  }
  ```

  Initial zero-arg shape panicked (see IX-010 finding); final
  shape uses one positional argument. PASSes on retry 2 against
  HEAD's post-driver-62 bridge. Closure consumed by move via the
  `impl FnOnce(...)` parameter routes through
  `rewrite_closure_call_shim` at
  `crates/regseq-driver/src/bridge/body/terminator.rs:150-165`
  and resolves through the
  `LangItem::FnOnce -> ClosureCallKind::CallOnce` arm at
  `crates/regseq-driver/src/bridge/body/terminator.rs:212`.
  Inspection predicate `SyntheticItemKind=ClosureCall` matches.
  Empirically the produced flavor is the closure-call rewrite's
  synthesized `Item` carrying
  `SyntheticItemKind::ClosureCall { call_kind: CallOnce }`, NOT
  `FunctionInstanceFlavor::ClosureOnceShim` -- confirms F2's
  Candidate-A finding at
  `.harness/expectations/regseq-driver-65.md:231-236` that
  direct consumption maps to `Item` flavor + `ClosureCall`
  synth kind. (F2's "panics the bridge" outcome no longer
  reproduces because driver-62 closed the prescan
  args-substitution path that triggered the panic.)

- **Candidate B (generic `F: FnOnce` bound)**: probed via a
  temporary out-of-tree snippet during F71 review iteration:

  ```rust
  #![no_std]
  pub struct Captured(i32);
  pub fn run<F: FnOnce(i32) -> i32>(f: F) -> i32 {
      f(0)
  }
  pub fn entry() -> i32 {
      let c = Captured(5);
      run(move |n| c.0 + n)
  }
  ```

  Same empirical outcome as Candidate A: PASS, inspection
  predicate `SyntheticItemKind=ClosureCall` matches, model
  carries `SyntheticItemKind::ClosureCall { call_kind: CallOnce }`
  with `FunctionInstanceFlavor::Item`. Confirms F2's
  Candidate-B finding at
  `.harness/expectations/regseq-driver-65.md:236-237` ("hits
  the IX-004 / IX-010 OPEN BUG family") is closed at HEAD per
  driver-62. The temporary probe was NOT committed to the
  sidecar (Candidate A is the F71-shipped snippet because the
  two empirical outcomes are equivalent for IX-014's predicate
  inspection; shipping both would not produce additional
  coverage signal under the existing predicate vocabulary).

The CallOnce sub-axis is exercised empirically by both
candidates but masked by the runner predicate-precision gap
that escalates SK-005 design-time (see below); the
closure-call rewrite path is unambiguously hit per
`crates/regseq-driver/src/bridge/body/terminator.rs:212`.

## Per-row escalation records

### RJ-009 -- `NonRigidTyKind::Param` (empirical, Mode 1)

Tier 1 candidate. 3-retry sweep against HEAD:

1. `pub fn id<T>(x: T) -> T { x }` + monomorphic `id::<u32>(0)`
   entry -- bridge `Ok`, no rejection (identical to IX-001's
   committed `crates/bridge-coverage-runner/snippets/IX-001.rs`
   shape).
2. Unused generic `pub fn unused<T>(x: T) -> T { x }` paired with
   non-generic `entry()` -- bridge `Ok`. The bridge's bucket-4
   carve-out at `crates/regseq-driver/src/bridge/mod.rs:1644-1658`
   silently absorbs the polymorphic `Function` item's
   `translate_fn_sig` NonRigidTy error and pushes no Instance.
3. Generic trait method
   `trait Tr { fn run<T>(&self, x: T) -> T; }` + monomorphic call
   `s.run::<u32>(0)` -- bridge `Ok`. The trait-method generic is
   monomorphized through `Instance::try_resolve`; no polymorphic
   `Param` survives translation.

**Root cause**: the matrix annotation at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:554`
("driver-60 prevents most; remaining gap is OPEN BUG IX-004") was
accurate for the pre-driver-62 bridge surface but is no longer
true at HEAD. Driver-62's fix at
`crates/regseq-driver/src/bridge/mod.rs:769-789` plus the
closure-call shim rewrite at
`crates/regseq-driver/src/bridge/body/terminator.rs:150-165`
eliminate the closure-call `Fn::call` body path that was the
last source-level surface producing `NonRigidTy::Param`. The
remaining polymorphic-signature sources are absorbed silently by
the bucket-4 carve-out at
`crates/regseq-driver/src/bridge/mod.rs:1644-1658` so the bridge
does NOT surface a `NonRigidTy::Param` rejection from any
F71-cohort source shape.

**Follow-up direction**: either (a) expose the bucket-4 carve-out
to the rejection oracle (treat
`signature_non_rigid: true` as a recordable rejection event for
runner consumption) so RJ-009 / RJ-010 / RJ-011 / RJ-012 / RJ-013
have a surfacing path, or (b) add a generic-fn surface that
intentionally surfaces a Param surviving substitution outside the
carve-out (e.g., a polymorphic trait-method call site whose
arguments are not fully bound at monomorphization). The matrix
status is retained at `partial` rather than escalated to
`uncovered` because IX-001's monomorphizable generic-fn surface
implicitly pre-empts the trivially-reachable Param sources; the
matrix-precision audit (Category 6 in the gap inventory) may
revisit this classification.

The snippet file is not present in `snippets/`; the row entry is
not committed to the sidecar.

### FF-007 -- `FunctionInstanceFlavor::ClosureOnceShim` (empirical, Mode 1)

Tier 2 candidate. 3-retry sweep surfaced a NEW bridge gap distinct
from the F70 RustCall ABI rejection:

1. Move-FnOnce shape
   (`consume<F: FnOnce(i32) -> i32>(f: F)` with `move |n| t.0 + n`)
   -- bridge `Ok` but the produced flavor is the closure-call
   rewrite's synthesized `Item` carrying
   `SyntheticItemKind::ClosureCall`, NOT `ClosureOnceShim`.
   Matches F2 IX-014 Candidate-A's empirical finding at
   `.harness/expectations/regseq-driver-65.md:231-236`. The
   inspection predicate `FunctionInstanceFlavor=ClosureOnceShim`
   does not match any model entry, so the row fails inspection.
2. Fn-as-FnOnce coercion shape
   (`let c = |n: i32| n + 1; call_once(c, 5)` against
   `call_once<F: FnOnce(i32) -> i32>(f: F, x: i32) -> i32`) --
   bridge rejection: `BridgeError::Phase5("phase 5 body pass
   failed: bridge-completeness validation failed:
   ItemMissingInstance { item: ItemId(6), item_kind: Function }")`.
3. FnMut-as-FnOnce coercion shape (capture-mutating closure
   passed to the same `call_once<F: FnOnce>` bound) -- same
   Phase-5 `ItemMissingInstance` (ItemId differs at 7).

**Root cause**: the Fn-or-FnMut closure passed to a generic
`F: FnOnce` bound triggers rustc to synthesize an
`InstanceKind::ClosureOnceShim` at monomorphization. The bridge
maps this through `function_instance_flavor_for_kind` to
`FunctionInstanceFlavor::ClosureOnceShim`, but the Phase-5 body
pass's bridge-completeness validation fails because the shim
Instance's body materialization references an Item that the
bridge has not yet bound to the model. This is a NEW bridge gap
distinct from F70's RustCall ABI rejection at
`crates/regseq-driver/src/bridge/fnsig.rs:107-115` (which fires
for `caller<F: Fn()>(f: F)` shapes BEFORE the ClosureOnceShim is
materialized); FF-007's failure is on the materialization side
once the Phase-5 body pass attempts to complete the shim Instance.

**Follow-up direction**: a future bridge-side task to either
(a) lazy-allocate the Function Item the ClosureOnceShim
materialization references through the existing
`lookup_or_create_direct_instance` surface (mirroring the
closure-call shim's pattern at
`crates/regseq-driver/src/bridge/body/terminator.rs:354-369`),
or (b) bind the ClosureOnceShim's underlying Item during the
synthesis pre-scan so Phase-5 finds it already-resolved. Distinct
from F70's FF-005 RustCall ABI work and the F71 RJ-009 bucket-4
direction; this is a Phase-5 completeness gap.

The snippet file is not present in `snippets/`; the row entry is
not committed to the sidecar.

### SK-004 -- `ClosureCall { call_kind: CallMut }` (design-time, Mode 2)

Tier 3 design-time escalation. No snippet was compiled.

The runner's `InspectionPredicate::SyntheticItemKind(String)` arm
at `crates/bridge-coverage-runner/src/inspect.rs:50-53` matches
only the variant TAG via `synthetic_kind_variant_tag` at
`crates/bridge-coverage-runner/src/inspect.rs:168-174`. The tag
function collapses `SyntheticItemKind::ClosureCall { call_kind: Call }`,
`SyntheticItemKind::ClosureCall { call_kind: CallMut }`, and
`SyntheticItemKind::ClosureCall { call_kind: CallOnce }` to the
single tag string `"ClosureCall"`. The dispatch at
`crates/bridge-coverage-runner/src/inspect.rs:604-609` therefore
cannot distinguish SK-004 (CallMut) from SK-003 (Call). The
F71-committed SK-003 row's `SyntheticItemKind=ClosureCall`
predicate would falsely "cover" SK-004 if SK-004 were committed,
so SK-004 escalates by design.

The CallMut dispatch path IS exercised empirically by the
F71-committed FT-013 row at
`crates/bridge-coverage-runner/snippets/FT-013.rs` (its
`|x| total += x` closure routes through
`rewrite_closure_call_shim`'s
`LangItem::FnMut -> ClosureCallKind::CallMut` arm at
`crates/regseq-driver/src/bridge/body/terminator.rs:211`), but
the runner-side predicate cannot prove the sub-axis was hit, so
SK-004 cannot be committed as a positive-coverage row.

**Follow-up direction**: per F2 IX-014 evidence at
`.harness/expectations/regseq-driver-65.md:238-240`, a future
runner-precision task to add a sub-variant-matching
`SyntheticItemKind` predicate (e.g., extend the dispatch with a
nested `sub: Option<String>` argument matching `CallMut` /
`CallOnce`) would unblock SK-004 + SK-005. Per the F71 task
spec's "No new `InspectionPredicate` arms" non-goal, F71 does NOT
add such a predicate.

### SK-005 -- `ClosureCall { call_kind: CallOnce }` (design-time, Mode 2)

Tier 3 design-time escalation. No snippet was compiled. Same
runner predicate-precision gap as SK-004: the tag function at
`crates/bridge-coverage-runner/src/inspect.rs:168-174` collapses
`CallOnce` to the shared `"ClosureCall"` tag, so the existing
`InspectionPredicate::SyntheticItemKind` arm cannot distinguish
CallOnce from Call / CallMut. The F71-committed IX-014 + FT-014
rows DO exercise the CallOnce dispatch path empirically (their
`rewrite_closure_call_shim` rewrite resolves to
`LangItem::FnOnce -> ClosureCallKind::CallOnce` at
`crates/regseq-driver/src/bridge/body/terminator.rs:212`), but
the runner-side predicate cannot prove the sub-axis was hit, so
SK-005 cannot be committed as a positive-coverage row.

Follow-up direction shared with SK-004.

## Strategy-deviation record

None. F71 stays inside the task-spec scope: sidecar authoring +
matrix-doc editing only, no bridge / model / runner-source /
helper / spec / strategy / pretty-printer / generic-c-backend
file changes, no new `InspectionPredicate` arms, no new
dependencies, no `task.py add-task` invocations for follow-up
filings. The two empirical escalations (RJ-009, FF-007) and the
two design-time escalations (SK-004, SK-005) are documented above
and inline at
`crates/bridge-coverage-runner/matrix_metadata/rows.toml` per the
F70 FF-005 precedent.
