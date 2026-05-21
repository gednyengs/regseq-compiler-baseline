# regseq-driver-65: First snippet batch (~23 rows) + mini-F3 dispatch arms (F2 of bridge-coverage-runner)

## Description

F2 of the bridge-coverage-runner strategy per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
§11. Land the first batch of LLM-authored snippets exercising
high-yield uncovered rows from Section 5 (interactions) and
Section 6 (rejections) of the matrix, alongside three new
`InspectionPredicate` dispatch arms required to verify the
chosen Section-5 rows.

Building on F1 (regseq-driver-64, closed at `e54ddf8` after the
rpath fix follow-up): the runner ships with a 10-row demo sidecar,
a 22-variant `InspectionPredicate` enum, and 5 implemented
dispatch arms (TyVariant / ItemKind / FunctionInstanceFlavor /
All / Any). F2 expands the sidecar by up to 23 rows and implements
three additional dispatch arms (SyntheticItemKind / ConstVariant /
CallDivergent) so the chosen Section-5 row set is verifiable
without violating F1's anti-silent-pass discipline. The remaining
14 dispatch arms stay `Unimplemented` for F3 proper.

### Why "mini-F3" inside F2

Strategy §11 schedules F3 ("inspection-predicate dispatch
hardening") after F2 ("first snippet batch"). The intent is that
F2 surfaces which predicates need real arms; F3 then closes them
out. In practice, the high-yield Section-5 interaction rows (the
matrix's load-bearing bug-discovery surface) naturally inspect via
`SyntheticItemKind` (VTableDropShim / VTableMethodShim /
ClosureDropGlue), `ConstVariant` (Const::Reloc), and
`CallDivergent` (`Call.target == None`) -- all `Unimplemented` in
F1. With F1's `Inspection::Unimplemented -> FAIL` rule, the chosen
rows would FAIL inspection even though the bridge correctly
materialised the construct. To get F2's verification loop usable,
F2 lands the three dispatch arms its row set requires. The
remaining 14 unimplemented predicates stay in F3's scope.

This deviates from strategy §11's strict F2-before-F3 ordering by
INTERLEAVING the smallest possible F3 slice (3 arms). Documented
in the harness expectations file as a strategy-deviation note.

### Authoring protocol

Per strategy §5: for each uncovered row, the implementer authors a
candidate `#![no_std]` snippet (with whatever feature gates the
shape requires); the runner verifies via the typed-outcome oracle;
if compile + inspection-pass (positive row) OR compile + matching
`BridgeError` variant (rejected row), the candidate is accepted.
Up to 3 retries before the row escalates to a row-replacement
follow-up task. Per-row authoring is sequential against the
single committed sidecar.

F2 does NOT pursue 30 rows hard; the strategy's "~30" estimate has
slack. F2 targets 23 (11 IX + 12 RJ) chosen for empirical
authorability. IX-014 is **escalated-before-attempt** based on
Codex round-2 empirical probing (the natural direct-consumption
snippet panics the bridge AND maps to `FunctionInstanceFlavor::Item`
rather than `ClosureOnceShim`; the generic-FnOnce shape hits the
IX-004/IX-010 OPEN BUG family). Rows that fail their 3-retry
budget DURING authoring are documented in the harness expectations
file as escalated; they stay OUT of the committed sidecar entirely
per the protocol in Change C step 5.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  (current change-log version v3 per
  `bridge-coverage-runner-strategy.md` change-log section; the
  H1 title at `:1` reads "v2" pending an editorial sweep but the
  body content is the v3 shape). Specific section anchors:
  - §5 (Snippet authoring strategy) -- the per-row LLM authoring
    + verification + 3-retry protocol.
  - §7 (Inspection predicates) -- the 22-variant enum + dispatch
    requirements. F2 adds three new dispatch arms (rationale in
    Description above).
  - §11 (Phasing) -- F2 = "First snippet batch: ~30 high-priority
    uncovered rows (Section 5 interactions + Section 6 rejection
    variants)". F3 = "Inspection-predicate dispatch hardening".
    F2 here interleaves 3 F3 arms; documented deviation.

### Matrix source

- `docs/discussions/regseq-model/bridge-coverage-matrix.md` (v2.1,
  Codex-approved). Per-row source anchors:
  - Section 5 (Interaction matrix) at
    `bridge-coverage-matrix.md:474-525` -- IX-001 / IX-007 /
    IX-008 / IX-012 / IX-013 / IX-014 / IX-015 / IX-017 /
    IX-019 / IX-021 / IX-022 / IX-023 demo-row sources.
  - Section 6.1 (`DslRejection`) at
    `bridge-coverage-matrix.md:537-548` -- RJ-003 / RJ-005 demo
    rows.
  - Section 6.3 (Top-level rejection variants) at
    `bridge-coverage-matrix.md:561-582` -- RJ-019 / RJ-022 /
    RJ-027 / RJ-028 demo rows.
  - RJ-028 sub-axes at `bridge-coverage-matrix.md:586-590`
    -- RJ-028-B / RJ-028-C demo rows.
  - RJ-031 sub-axes at `bridge-coverage-matrix.md:594-602`
    -- RJ-031-A through RJ-031-F demo rows.

### F1 runner state

- `crates/bridge-coverage-runner/Cargo.toml` -- the runner binary
  manifest.
- `crates/bridge-coverage-runner/src/inspect.rs` -- the
  `InspectionPredicate` enum (all 22 variants ship) +
  the 5 implemented dispatch arms + 17 `Unimplemented` fallthroughs.
  F2 adds 3 more dispatch arms (SyntheticItemKind, ConstVariant,
  CallDivergent); the remaining 14 stay `Unimplemented` for F3.
- `crates/bridge-coverage-runner/matrix_metadata/rows.toml` --
  the F1 10-row sidecar. F2 expands it by up to 23 rows (positional
  appending in the same file; escalated rows do NOT land).
- `crates/bridge-coverage-runner/snippets/` -- the F1 10-snippet
  corpus. F2 adds up to 23 snippets named `<row_id>.rs` per the F1
  naming convention at
  `crates/bridge-coverage-runner/snippets/README.md`.

### Bridge surface anchors (for the new dispatch arms)

- `crates/regseq-model/src/synthetic.rs:46` -- `SyntheticItemKind`
  enum (10+ variants). The `SyntheticItemKind` dispatch arm walks
  the model's synthetic-items array and matches the variant tag.
- `crates/regseq-model/src/const_value.rs:20` -- `Const` enum.
  The `ConstVariant` dispatch arm walks the model's consts array
  and matches the `Const` variant tag.
- `crates/regseq-model/src/terminator.rs:64` -- `TerminatorKind`
  enum, including `Call { callee, args, destination, target }`.
  The `CallDivergent` dispatch arm walks the model's body
  terminators and matches `Call.target == None` (when `arg ==
  true`) or `Call.target.is_some()` (when `arg == false`).

The implementer must confirm each anchor's HEAD line number during
work; line numbers in this spec are recorded against HEAD at task
file time.

### Prior tasks

- regseq-driver-64 (closed at `9f93c2b`, rpath fix at `e54ddf8`):
  F1 scaffold + 10-row demo. F2 consumes the runner end-to-end.
- regseq-driver-63 (closed at `174c7ff`): F0 typed-outcome surface.

## Implementation Notes

### Change A: three new `InspectionPredicate` dispatch arms

**Anchor**: `crates/bridge-coverage-runner/src/inspect.rs` --
extend the existing `check` function's match expression. The
arms currently implemented are at the file's main `match pred`
block (per regseq-driver-64 Change E).

Each new arm follows the same shape as the existing five:
walk the relevant slice of the `RegseqModel`; succeed if any
entry matches the requested variant tag; otherwise return
`InspectError::NotFound { variant, arg }`.

**Arm `SyntheticItemKind(arg)`**:

```rust
InspectionPredicate::SyntheticItemKind(arg) => {
    let needle = arg.as_str();
    let found = model
        .item_synthetic_kinds
        .iter()
        .any(|sk| synthetic_kind_variant_tag(sk) == needle);
    if found {
        Ok(())
    } else {
        Err(InspectError::NotFound {
            variant: "SyntheticItemKind",
            arg: arg.clone(),
        })
    }
}
```

Where `synthetic_kind_variant_tag(sk: &SyntheticItemKind) ->
&'static str` matches `sk` against each `SyntheticItemKind`
variant and returns a stable string tag (`"None"` /
`"ClosureCaptures"` / `"ClosureCall"` / `"ClosureDropGlue"` /
`"DropGlue"` / `"VTableType"` / `"VTableInstance"` /
`"VTableMethodShim"` / `"VTableDropShim"` / `"ConstMemoryStatic"`).
The function lives in `inspect.rs` adjacent to the existing
`ty_variant_tag` / `item_kind_variant_tag` helpers.

**Arm `ConstVariant(arg)`**:

```rust
InspectionPredicate::ConstVariant(arg) => {
    let needle = arg.as_str();
    let found = model
        .consts
        .iter()
        .any(|c| const_variant_tag(c) == needle);
    if found {
        Ok(())
    } else {
        Err(InspectError::NotFound {
            variant: "ConstVariant",
            arg: arg.clone(),
        })
    }
}
```

Where `const_variant_tag(c: &Const) -> &'static str` matches
each `Const` variant (`"Bool"` / `"Char"` / `"Int"` / `"Uint"` /
`"Float"` / `"ZST"` / `"Bytes"` / `"Aggregate"` / `"EnumValue"` /
`"NullPtr"` / `"Reloc"`).

**Arm `CallDivergent(arg)`**:

```rust
InspectionPredicate::CallDivergent(arg) => {
    let needs_divergent = *arg;
    let found = model
        .bodies
        .iter()
        .flat_map(|body| body.basic_blocks.iter())
        .filter_map(|bb| bb.terminator.as_ref())
        .any(|term| match &term.kind {
            TerminatorKind::Call { target, .. } => target.is_none() == needs_divergent,
            _ => false,
        });
    if found {
        Ok(())
    } else {
        Err(InspectError::NotFound {
            variant: "CallDivergent",
            arg: if needs_divergent { "true".into() } else { "false".into() },
        })
    }
}
```

The exact `model.bodies` traversal shape mirrors the implementer's
existing patterns in the F1 dispatch arms; the implementer adapts
naming to match.

**Test coverage**: add three module-level `#[cfg(test)] mod tests`
unit tests inside `inspect.rs` (mirroring the existing tests in
`matrix.rs`) that construct a minimal `RegseqModel` exercising
each new arm and assert the expected `Ok(())` / `NotFound` /
`Unimplemented` outcomes. Construction can re-use shapes from
`crates/regseq-model/tests/id_referential_integrity_validator.rs`
for synthesised models.

### Change B: 23 new sidecar rows + 23 new snippets

**Anchor for sidecar**:
`crates/bridge-coverage-runner/matrix_metadata/rows.toml` --
append up to 23 `[[row]]` entries after the existing 10 F1 entries
(IX-014 escalated-before-attempt, plus any further authoring-loop
escalations).
Cluster keys per strategy §8 (explicit, not derived).

**Anchor for snippets**:
`crates/bridge-coverage-runner/snippets/<row_id>.rs` -- one file
per row, naming convention per F1's
`crates/bridge-coverage-runner/snippets/README.md`.

### Row set (11 Section-5 positive + 12 Section-6 rejected; IX-014 escalated-before-attempt)

Per-row inspection predicate AND snippet-shape hypothesis. The
hypothesis is a STARTING POINT for the implementer's authoring
loop; the per-row 3-retry budget per strategy §5 allows the
implementer to refine the shape as needed.

#### Section 5 (positive rows; `expectation = positive`)

| Row ID | Construct | Inspection predicate | Snippet shape hypothesis |
| ------ | --------- | -------------------- | ------------------------ |
| IX-001 | Generic fn x concrete args | `All(ItemKind=Function, FunctionInstanceFlavor=Item)` -- loose proxy; precise "generic substitution at body materialisation" verification needs a `PlaceElem` or body-walk predicate (deferred to F3). The proxy still surfaces a regression if monomorphization fails to produce ANY Function-flavor Instance for the generic fn. | `pub fn id<T>(x: T) -> T { x } pub fn entry() -> u32 { id::<u32>(0) }` |
| IX-007 | dyn Trait x Drop -> VTableDropShim | `SyntheticItemKind=VTableDropShim` | trait + Drop impl on concrete + cast to `&dyn T` + drop site (forces vtable drop shim synthesis) |
| IX-008 | dyn Trait x generic method substitution at vtable | `SyntheticItemKind=VTableMethodShim` | trait with a method + impl + cast to `&dyn T` + method call through the dyn reference |
| IX-012 | Nested Drop ordering | `All(FunctionInstanceFlavor=DropGlue, ItemKind=Struct)` -- loose check; precise ordering check deferred to F4 stricter predicates | nested Drop struct (inner Drop type as a field of outer Drop type) + scope drop site |
| IX-013 | Closure x Drop captures (-> ClosureDropGlue) | `SyntheticItemKind=ClosureDropGlue` | closure capturing a Drop type by move + closure use site |
| ~~IX-014~~ | ~~FnOnce x Move consumption~~ | **DEFERRED FROM F2 (not committed)** | Empirically blocked: the bridge maps direct closure consumption to `FunctionInstanceFlavor::Item` (NOT `ClosureOnceShim`) per `crates/regseq-driver/src/bridge/body/terminator.rs:290-369`, AND the candidate snippet trips a bridge panic in Codex round-2 empirical probing. The natural alternative -- generic `F: FnOnce` bound -- hits the IX-004/IX-010 OPEN BUG family. F2 documents IX-014 as escalated-before-attempt in the harness expectations file with a follow-up direction: either (a) a future task adds a sub-variant-matching `SyntheticItemKind` predicate (e.g., `SyntheticItemKind=ClosureCall, sub=CallOnce`) and uses the direct-consumption path's actual outcome shape, or (b) investigation files a separate bug-discovery task for the bridge panic. |
| IX-015 | Generic fn x generic struct field access | `All(ItemKind=Function, ItemKind=Struct)` -- loose proxy; precise `Place::Field` projection verification needs the `PlaceElem` dispatch arm (deferred to F3). The proxy verifies both Items materialise post-monomorphization. | `pub struct W<T> { x: T } pub fn f<T>(w: W<T>) -> T { w.x } pub fn entry() -> u32 { f(W { x: 0_u32 }) }` |
| IX-017 | Diag-log intrinsic x generic caller | `FunctionInstanceFlavor=Intrinsic` | `#![feature(register_tool)] #![register_tool(rseq)]` + inlined `regseq::log` shim per `diag_logging.rs` + `pub fn f<T: ...>(t: T) { regseq::log::info!("{}", t) }` (concrete T at monomorphization) |
| IX-019 | Two adjacent end-of-scope drops | `FunctionInstanceFlavor=DropGlue` (loose; coverage is "at least one DropGlue instance present") | two `let _x = R;` `let _y = R;` (R: Drop) inside one function body |
| IX-021 | dyn Trait + Send auto-trait | `TyVariant=Dynamic` (loose; auto-trait-set granularity deferred) | `pub fn f(_d: &(dyn core::fmt::Debug + Send)) {}` -- BUT note `Debug` won't compile without `alloc`; substitute with a local trait + Send |
| IX-022 | Const::Reloc x call-site (reloc through &static) | `ConstVariant=Reloc` | `pub static X: u32 = 42; pub const P: &'static u32 = &X; pub fn entry() -> u32 { *P }` -- `&X` in const context evaluates to a `GlobalAlloc::Static` relocation, which the bridge maps to `Const::Reloc(InstanceRef)` at `crates/regseq-driver/src/bridge/const_.rs:414-426`. The natural function-pointer-literal shape (`const F: fn() = target;`) is BLOCKED in the current bridge -- `GlobalAlloc::Function` is rejected as `UnsupportedConst` per `:427-430`, so OP-003-K via function-pointer-literal is a separate bridge-extension task (not part of F2). |
| IX-023 | Diverging call x reachability (`Call.target == None`) | `CallDivergent=true` | `pub fn boom() -> ! { loop {} } pub fn entry() { boom() }` -- `boom()` call is diverging |

#### Section 6 (rejected rows; `expectation = rejected`)

| Row ID | Construct | Expected rejection | Snippet shape hypothesis |
| ------ | --------- | ------------------ | ------------------------ |
| RJ-003 | Atomic | `DslRejection` with `kind = "Atomic"` | `pub static A: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);` |
| RJ-005 | Rvalue (ThreadLocalRef) | `DslRejection` with `kind = "Rvalue"` | `#![feature(thread_local)] #[thread_local] pub static T: u32 = 0; pub fn f() -> u32 { unsafe { T } }` |
| RJ-019 | DynAutoOnly (`dyn Send` no principal) | `DynAutoOnly` | `pub fn f(_d: &dyn Send) {}` |
| RJ-022 | SubstantiveHrtb | `SubstantiveHrtb` | `pub fn f(_: for<'a> fn(&'a u32, &'a u32)) {}` -- both lifetime uses share one binder; substantive |
| RJ-027 | UnsupportedNeverInput | `UnsupportedNeverInput` | `#![feature(never_type)] pub fn f(_x: !) {}` |
| RJ-028-B | UnsupportedCAbiType::Output | `UnsupportedCAbiType` with `position = "Output"` | `pub extern "C" fn f() -> (u32, u32) { (1, 2) }` |
| RJ-028-C | UnsupportedCAbiType::Field | `UnsupportedCAbiType` with `position = "Field"` | `#[repr(C)] pub struct WithRustFn { f: fn() -> u32 } pub extern "C" fn boundary(_s: WithRustFn) {}` -- a Rust-ABI `fn()` field inside a `repr(C)` struct that crosses an `extern "C"` boundary. Known-good shape per `crates/regseq-driver/tests/bridge_fnsig.rs:398-413`. Arrays (`[u32; N]`) DO NOT trip the field rejection because the bridge recurses to admissible element types at `crates/regseq-driver/src/bridge/fnsig.rs:245-257`. |
| RJ-031-A | DiagLogFormat::DebugFormatterRejected | `DiagLogFormat` with `kind = "DebugFormatterRejected"` | inlined log shim + `regseq::log::info!("x={:?}", x)` |
| RJ-031-B | DiagLogFormat::PositionalArgRejected | `DiagLogFormat` with `kind = "PositionalArgRejected"` | inlined log shim + `regseq::log::info!("x={0}", x)` |
| RJ-031-C | DiagLogFormat::NamedArgRejected | `DiagLogFormat` with `kind = "NamedArgRejected"` | inlined log shim + `regseq::log::info!("x={name}", name = x)` |
| RJ-031-E | DiagLogFormat::UnclosedBrace | `DiagLogFormat` with `kind = "UnclosedBrace"` | inlined log shim + `regseq::log::info!("x={", x)` |
| RJ-031-F | DiagLogFormat::UnmatchedClosingBrace | `DiagLogFormat` with `kind = "UnmatchedClosingBrace"` | inlined log shim + `regseq::log::info!("x=}", x)` |

**RJ-031-D (NonSubsetModifier) deliberately omitted from F2**:
The bridge accepts the hex subset (`{:x}` / `{:X}` per spec.md
§26.7); rejecting non-subset modifiers like `{:b}` or `{:.5}`
requires the implementer to know which modifiers are inside vs.
outside the subset. Defer to F4 when the bridge's allowed-modifier
list is fully documented.

### Change C: per-snippet authoring + verification protocol (per strategy §5)

For each of the 23 rows above, the implementer follows this loop:

1. Author candidate snippet at
   `crates/bridge-coverage-runner/snippets/<row_id>.rs` per the
   shape hypothesis (or a refinement).
2. Append the row's `[[row]]` entry to
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
   with `cluster_key` set to the row's top-level ID prefix
   (`IX-001`, `RJ-031`, etc.).
3. Run the runner against the current sidecar:
   ```bash
   target/debug/bridge-coverage-runner \
     --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
     --snippets crates/bridge-coverage-runner/snippets/ \
     --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
     --out /tmp/brcr-f2/
   ```
4. Inspect `/tmp/brcr-f2/report.md`:
   - Positive row: `actual_outcome == Ok` AND `inspection ==
     PASS` -> candidate accepted.
   - Rejected row: `actual_outcome == Rejected(variant)` AND
     `variant matches expected_rejection` -> candidate accepted.
   - Otherwise: snippet authoring failed. Author a refined
     candidate (different feature gate, different shape, different
     forcing site) and retry from step 1.
5. Max 3 retries per row. After 3 retries WITHOUT a passing
   candidate:
   - The row's snippet file (if any) is removed from
     `crates/bridge-coverage-runner/snippets/`.
   - The row's `[[row]]` entry is NOT committed to the sidecar
     at `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.
     Escalated rows stay OUT of the committed sidecar entirely;
     the runner only sees passing rows. This avoids any sidecar
     drift-check edge cases.
   - The row is documented as escalated in the harness
     expectations file with the candidate shapes attempted +
     the empirical rejection reason at each attempt + a proposed
     follow-up direction.
6. After each candidate-accepted row, commit the snippet + sidecar
   addition together (one commit per row OR per coherent cluster
   of related rows). Do NOT commit a row whose snippet didn't yet
   pass; the sidecar drift check applies at every commit boundary.

The runner is the verification gate. NO snippet lands without a
runner-report-PASS row entry. NO sidecar entry lands without a
corresponding snippet that passes.

### Change D: harness expectations entry

**New file**: `.harness/expectations/regseq-driver-65.md`.

Record:

- Before-fix evidence: F1 runner ships 10 rows; matrix has ~133
  uncovered rows total.
- After-fix evidence: F2 adds 23 rows to the sidecar; per-row
  empirical PASS/FAIL/ESCALATE outcomes recorded with the actual
  `BridgeError` variant (for rejected rows) and the inspection
  predicate outcome (for positive rows).
- Strategy-deviation record: mini-F3 inside F2 (3 dispatch arms
  landed: SyntheticItemKind, ConstVariant, CallDivergent).
  Remaining 14 dispatch arms scoped for F3 proper.
- Per-row escalation log: any of the 23 rows that consumed their
  3-retry budget gets a sub-section documenting the candidate
  shapes attempted, the rejected outcomes, and the proposed
  follow-up direction. Escalated rows are NOT committed snippets.
- F3 readiness signal: the implementer's expectations file
  closes with a note stating whether the runner's dispatch
  coverage is now operationally sufficient for F3 to focus on
  the remaining 14 predicates without further mini-batches of
  interleaved arms.

## Non-Goals

- Do NOT implement the remaining 14 `InspectionPredicate` dispatch
  arms (the enum has 22 variants total; 5 ship in F1, 3 land in
  F2, 14 stay `Unimplemented` for F3). F2 lands ONLY the three
  the chosen row set requires (SyntheticItemKind / ConstVariant /
  CallDivergent). F3 dispatches the rest.
- Do NOT modify the F1 runner architecture (the seven modules at
  `crates/bridge-coverage-runner/src/`). F2 extends `inspect.rs`
  with three new dispatch arms ONLY; no other module is touched.
- Do NOT modify the F0 helper at
  `crates/test-ir-fixtures/src/lib.rs`. F0 is read-only consumed.
- Do NOT modify spec.md, pretty-printer.md, generic-c-backend.md,
  bridge-coverage-matrix.md, or bridge-coverage-runner-strategy.md.
  F2 is runner-content-only.
- Do NOT pursue more than 23 rows. The strategy says "~30"; F2
  targets 23 attempted with slack (IX-014 escalated-before-attempt,
  so the addressable set is 22 IX-014-excluded rows from the
  Section-5 high-yield set plus the 12 Section-6 candidates).
  The remaining ~109 uncovered matrix rows
  are F4's batch per strategy §11.
- Do NOT add new dependencies to `crates/bridge-coverage-runner/`
  beyond what F1 ships (`test-ir-fixtures`, `regseq-model`,
  `regseq-driver`, `toml`, `serde + derive`, `sha2`).
- Do NOT LLM-author all 23 snippets in a single shot. Per
  strategy §5, the per-row 3-retry loop runs sequentially; each
  row's outcome informs the next.
- Do NOT exceed 3 retries per row. Rows that fail their retry
  budget escalate (snippet not committed; expectations file
  records the escalation).
- Do NOT modify the playground at `tests/fixtures/playground/`.
- Do NOT include validator-failing rows (Section 7 of the matrix);
  those are constructed-model tests per strategy §2 out-of-scope.
- Do NOT include rows where the bridge has an OPEN BUG (IX-004,
  IX-010 -- both blocked-open-bridge-bug per the matrix).
- Do NOT include RJ-031-D (NonSubsetModifier) without first
  documenting which modifiers are inside vs. outside the bridge's
  accepted subset. Defer to F4.
- Do NOT relax F1's `Inspection::Unimplemented -> FAIL` rule. The
  three new dispatch arms close the gap for F2's row set; the
  remaining 14 stay FAIL until F3.

## Test Plan

### B1: workspace stays green

`cargo test --workspace -- --test-threads=1` green:

- All existing F1 smoke tests at
  `crates/bridge-coverage-runner/tests/smoke.rs` continue to pass
  (the smoke tests use mini-sidecars; the F2 sidecar expansion
  doesn't perturb the F1 corpus).
- The F1 unit tests under `inspect.rs` and `matrix.rs` continue
  to pass.
- Three new unit tests for the F2 dispatch arms (per Change A)
  pass.

### B2: runner reports all-pass on the expanded sidecar

After all 23 rows are committed (or escalated and documented),
the manual Mode A invocation against the expanded sidecar
produces a `report.md` with:

- `Total rows` = 10 (F1) + K (F2 committed), where K = 23 minus
  the number of further authoring-loop escalations per the
  per-snippet protocol in Change C (IX-014 is already excluded
  from the 23 attempted; further escalations only reduce K).
- `Pass` count = `Total rows` (every committed row passes per
  the runner's pass/fail rule from F1's report.rs; escalated
  rows are NOT in the sidecar and therefore NOT in this count).
- `Fail` count = 0.
- Per-row table includes every committed F2 row with its
  expected outcome class.

Escalated rows are documented separately in the expectations
file (Change D); they do not appear in the sidecar or report.

### B3: per-row empirical PASS recorded in expectations

The harness expectations file at
`.harness/expectations/regseq-driver-65.md` records a per-row
table listing every F2 row with:

- Row ID.
- Final inspection predicate (after retry refinement).
- Final snippet shape (after retry refinement).
- Actual `BridgeError` variant (for rejected rows) or
  `InspectionPredicate` outcome (for positive rows).
- PASS / ESCALATED status.

The implementer paste-includes the final report.md's per-row
table excerpt as evidence.

### B4: `cargo fmt` + `cargo clippy` clean

`cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets -- -D warnings` clean.
The three new dispatch arms must be clippy-clean; the new helper
functions (synthetic_kind_variant_tag, const_variant_tag) must
match the existing F1 helper-function naming + style.

### B5: F0 + F1 surface unchanged

`git diff main -- crates/test-ir-fixtures/` returns zero.
`git diff main -- crates/bridge-coverage-runner/src/main.rs
crates/bridge-coverage-runner/src/matrix.rs
crates/bridge-coverage-runner/src/snippet.rs
crates/bridge-coverage-runner/src/oracle.rs
crates/bridge-coverage-runner/src/cluster.rs
crates/bridge-coverage-runner/src/report.rs
crates/bridge-coverage-runner/build.rs
crates/bridge-coverage-runner/Cargo.toml` returns zero.
F2 modifies ONLY `inspect.rs` (3 new dispatch arms +
3 new tests), `rows.toml` (up to 23 appended rows),
`snippets/<row_id>.rs` (up to 23 new files), and the harness
expectations file.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/65-bridge-coverage-runner-f2-first-batch.md`).
- The runner strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  as design source-of-truth.
- The matrix doc
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md`) as
  the row-id authority.

### Reviewer focus

- **Row-set composition**: do the 23 attempted rows (plus IX-014
  escalated-before-attempt) actually cover
  "high-priority uncovered Section 5 + Section 6 rows" per strategy
  §11? Are any high-yield rows missing? Are any chosen rows
  empirically unauthorable in `#![no_std]` with reasonable feature
  gates?
- **Inspection-predicate assignment correctness**: for each
  Section-5 row, does the assigned predicate actually verify the
  row's construct, OR is it a loose proxy (in which case is the
  proxy documented)?
- **Mini-F3 dispatch arm scope**: the spec lands 3 arms
  (SyntheticItemKind, ConstVariant, CallDivergent). Are any of
  the chosen rows quietly assuming a 4th unimplemented arm
  (e.g., a row that subtly needs PlaceElem or RvalueVariant)?
- **Snippet-shape hypotheses**: are the per-row hypotheses
  plausible? Will any row likely consume its 3-retry budget given
  the hypothesis?
- **RJ-022 substantive HRTB shape**: the spec hypothesis is
  `for<'a> fn(&'a u32, &'a u32)`. Empirically, does the bridge
  classify this as substantive (rejected) or non-substantive
  (accepted)? Cross-reference
  `crates/regseq-driver/tests/bridge_ty.rs:447-470` (HRTB type
  alias declarations) and `crates/regseq-driver/tests/bridge_ty.rs:554-577`
  (the per-variant assertions confirming `hrtb` rejects via
  `SubstantiveHrtb`, `clean_hrtb` accepts, `chain_ref_hrtb`
  accepts). The substantive rule is at
  `crates/regseq-driver/src/bridge/ty.rs:1471-1477`.
- **Out-of-scope discipline**: F2 enumerates extensive Non-Goals.
  Confirm Implementation Notes does not creep into any.
- **Strategy-deviation framing**: the spec acknowledges F2
  interleaves 3 F3 arms. Is the deviation framed clearly enough,
  or should F2 split into two tasks (F2a = snippets only with
  F1-compatible predicates; F2b = the 3 dispatch arms)?
- **Authoring protocol clarity**: is the per-snippet
  author/verify/retry loop concrete enough for the implementer
  agent to follow?
- **Citation drift**: every file:line citation against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/65-bridge-coverage-runner-f2-first-batch.md.
Identify:
- Row-set composition (23 rows; high-yield Section 5 + 6 coverage).
- Inspection-predicate assignment correctness per Section-5 row.
- Mini-F3 dispatch arm scope (3 arms: SyntheticItemKind /
  ConstVariant / CallDivergent). Are 3 sufficient or are other
  arms implicitly needed?
- Snippet-shape hypothesis plausibility (per-row 3-retry budget
  risk).
- RJ-022 substantive HRTB shape correctness.
- Strategy-deviation framing (mini-F3 inside F2 vs. strict
  ordering per strategy §11).
- Out-of-scope creep against the 11 Non-Goals.
- Authoring protocol clarity for the implementer agent.
- Citation drift against HEAD.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- For RJ-022, you may inspect crates/regseq-driver/tests/bridge_ty.rs
  to verify the substantive-vs-non-substantive HRTB shape distinction.
- For inspection predicates, you may inspect
  crates/regseq-model/src/{ty,item,instance,synthetic,const_value,
  terminator}.rs to confirm the variant tags.
- For snippet shape hypotheses, you may dry-run a few in your head
  against the bridge code at crates/regseq-driver/src/bridge/.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ROW-SET COMPOSITION CHECK: PASS / FAIL with citation
INSPECTION-PREDICATE ASSIGNMENT CHECK: per-row PASS / suggestion
MINI-F3 DISPATCH ARM SCOPE CHECK: PASS / FAIL with citation
SNIPPET-SHAPE HYPOTHESIS PLAUSIBILITY: per-row PASS / hypothesis-flagged
RJ-022 SUBSTANTIVE HRTB SHAPE: AGREE / DISAGREE with alternative
STRATEGY-DEVIATION FRAMING: PASS / FAIL
OUT-OF-SCOPE DISCIPLINE: PASS / FAIL
AUTHORING PROTOCOL CLARITY: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - All existing F0/F1 tests pass unchanged.
  - The three new unit tests in
    `crates/bridge-coverage-runner/src/inspect.rs` for the new
    dispatch arms pass.
- Manual Mode A invocation per B2 produces a `report.md` with
  `Pass = Total rows` and `Fail = 0`.
- `git diff --stat main..` shows changes confined to:
  - `crates/bridge-coverage-runner/src/inspect.rs` (Change A: 3 new
    dispatch arms + 3 new unit tests + 3 new helper functions)
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change B: up to 23 appended rows)
  - `crates/bridge-coverage-runner/snippets/<row_id>.rs`
    (Change B: up to 23 new snippet files; fewer if rows
    escalate during the authoring loop)
  - `.harness/expectations/regseq-driver-65.md` (Change D)
  - `.harness/tasks.json` (registration entry)
- No F0 modifications.
- No F1 module modifications other than `inspect.rs`.
- No bridge / model / spec / pretty-printer / generic-c-backend /
  matrix / strategy doc changes.
- Codex Task Review verdict: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Three new dispatch arms land**:
   `crates/bridge-coverage-runner/src/inspect.rs` extends `check`
   with implemented arms for `SyntheticItemKind` / `ConstVariant`
   / `CallDivergent` plus the matching helper functions and three
   unit tests.

2. **Up to 23 new sidecar rows land**:
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
   contains up to 33 `[[row]]` entries total (10 F1 + 23 F2
   attempted; minus any further authoring-loop escalations).
   Every committed row carries an explicit `cluster_key`.

3. **Up to 23 new snippets land (modulo escalations)**:
   `crates/bridge-coverage-runner/snippets/<row_id>.rs` exists
   for every committed sidecar row. Snippet count = sidecar row
   count - F1 fixtures (which don't have snippet files).

4. **Runner reports all-pass on the committed sidecar**: manual
   Mode A run per B2 produces exit code 0 and a report with `Pass
   = Total rows`, `Fail = 0`.

5. **Per-row expectations recorded**: the harness expectations
   file at `.harness/expectations/regseq-driver-65.md` carries a
   per-row table with final predicate / snippet shape / actual
   outcome / PASS-or-ESCALATED status for every F2 candidate row.

6. **Escalated rows documented**: IX-014 (escalated-before-attempt
   per the row table) AND any of the 23 candidates that consumed
   its 3-retry budget without a passing candidate is documented
   in the expectations file with the candidate shapes attempted,
   the empirical rejection reason at each attempt, and a
   follow-up direction (replace with a different row, refine
   snippet shape with additional context, file a separate
   investigation task).

7. **F1 architecture untouched**: `git diff main --
   crates/bridge-coverage-runner/src/main.rs
   crates/bridge-coverage-runner/src/matrix.rs
   crates/bridge-coverage-runner/src/snippet.rs
   crates/bridge-coverage-runner/src/oracle.rs
   crates/bridge-coverage-runner/src/cluster.rs
   crates/bridge-coverage-runner/src/report.rs
   crates/bridge-coverage-runner/build.rs
   crates/bridge-coverage-runner/Cargo.toml` returns zero.

8. **F0 untouched**:
   `git diff main -- crates/test-ir-fixtures/` returns zero.

9. **No scope creep**: `git diff main` touches ONLY the files
   listed in Quality Gate's `git diff --stat` enumeration. No
   bridge / model / spec / pretty-printer / generic-c-backend /
   matrix / strategy doc changes.

10. **Mini-F3 deviation documented**: the harness expectations
    file's closing notes the 3 dispatch arms landed AS part of F2
    (strategy-deviation), enumerate the 14 dispatch arms remaining
    for F3 proper, and confirm F3's scope is now bounded.

11. **F3 readiness signal**: the expectations file's closing note
    states whether F3 can now proceed without further mini-F3
    interleaves in subsequent F* phases. This is the bridge
    between F2 and F3.
