# regseq-model-45: Rule 1 — Intrinsic Recognition

## Description

Implement recognition rule 1: convert rustc `mir::TerminatorKind::Call`
terminators whose callee `DefId` is registered as a Regseq
intrinsic into Regseq IR `IntrinsicCall` terminators. Method
intrinsics preserve `&self` as the first argument; free-function
intrinsics call directly. Uses the Phase 4 in-memory intrinsic
registry.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 10
  intrinsic recognition mechanism, RegseqIntrinsic enum)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 IntrinsicCall terminator; Section 8 recognition
  rule 1)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (Section 2 abstraction types — intrinsic method tables with
  canonical names)
- `crates/regseq-model/src/ir/intrinsic.rs` (RegseqIntrinsic,
  IntrinsicMetadata)
- `crates/regseq-model/src/ir/terminator.rs` (IntrinsicCall
  variant)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-44)
- `crates/regseq-driver/src/extract/intrinsic_registry.rs`
  (`IntrinsicRegistry` — `HashMap<DefId, String>` built by
  Phase 4 extraction, passed into construction via
  `ExtractionRegistries.intrinsic`)

## Implementation Notes

### Recognition logic

At Pass 2 construction setup (task 43), walk
`ExtractionRegistries.intrinsic.entries()` and build a
`IntrinsicLookup: HashMap<DefId, RegseqIntrinsic>` by parsing
each canonical-name string through a
`parse_canonical_intrinsic_name(&str) -> Option<RegseqIntrinsic>`
helper. Any unparseable canonical name raises
`ConstructionError::UnknownIntrinsicCanonicalName` at setup
(not per call).

This lookup is built once at Pass 2 start and passed alongside
the Pass-1 `CrossRefMaps` (from `build_pass1_output`) into
`build_body`. The registry's `DefId` keys come from Phase 4
extraction and match the function `DefId`s in
`cross_refs.functions`.

When translating a `mir::TerminatorKind::Call { func, args,
destination, target, .. }` (task 43):

1. Resolve the callee `DefId` from `func` (typically
   `func.const_fn_def()` or pattern-matching on
   `Operand::Constant` with a `FnDef` const).
2. Look up the `DefId` in `IntrinsicLookup`.
3. If found, emit:

```rust
TerminatorKind::IntrinsicCall {
    intrinsic: <RegseqIntrinsic variant>,
    args: <mapped args, with &self preserved as args[0] for method intrinsics>,
    metadata: None,   // tasks 50-52 populate this
    destination: <mapped destination>,
    target: <mapped target>,
    error_target: None, // task 46 populates this
}
```

4. If the `DefId` is not in `IntrinsicLookup`, fall through to a
   regular `TerminatorKind::Call` (from task 43).

### Canonical names

`parse_canonical_intrinsic_name` must cover every variant in
`RegseqIntrinsic` (documented in compiler-knowledge.md
Section 2): `clock_enable`, `clock_disable`, `clock_get_rate`,
`reset_assert`, `reset_deassert`, `reset_toggle`,
`pinctrl_apply`, `mmio_read`, `mmio_read_field`, `mmio_write`,
`mmio_write_field`, `busy_wait`, `sleep`, `log_error`,
`log_warn`, `log_info`, `log_debug`, `poll_until`.

### Method vs free-function distinction

The registry does not distinguish. The distinction follows from
the function's signature in the model: method intrinsics have a
`Receiver` `Param` (their `args[0]` carries the `&self` operand);
free-function intrinsics have only `Regular` params. The
recognition helper preserves the MIR call's arg order verbatim
and does not need to know the distinction.

### Cross-references

The `Function` for the intrinsic itself (e.g., `Clock::enable`)
must already exist in the model. Task 41 (Pass 1) registers
intrinsic methods on the abstraction type's impl block; task 47
(cross-crate extraction) attaches their bodies. Recognition does
not look at the intrinsic's body -- only at the canonical name
on the callee.

## Non-Goals

- No `?`-pattern folding (task 46).
- No metadata population (tasks 50-52).
- No deref collapse (task 48).
- No cross-crate body extraction (task 47).
- No closure / `poll_until` handling beyond emitting the
  `PollUntil` variant (closure support deferred).

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Compile the PL011 fixture and translate a helper that calls
  `clock_get_rate()` and `pinctrl_apply()` without `?`. Assert
  each becomes an `IntrinsicCall` with the correct
  `RegseqIntrinsic` variant, `args[0]` is the `&self` operand,
  `metadata: None`, `error_target: None`.
- Fixture calling a free-function intrinsic (`busy_wait(...)`).
  Assert no Receiver arg is prepended and args match the source
  call's arity.
- Fixture with a Call to a function NOT in the intrinsic
  registry; assert fall-through to regular `Call`.
- Hand-built `IntrinsicRegistry` with an unknown canonical name
  (e.g., `"future_intrinsic"`) passed into
  `build_declaration_skeleton`; assert
  `ConstructionError::UnknownIntrinsicCanonicalName` is raised
  at setup.
- Assert `parse_canonical_intrinsic_name` round-trips every
  variant in `RegseqIntrinsic`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the canonical-name
parser covers every `RegseqIntrinsic` variant, the receiver
preservation matches spec.md Section 10, and unknown canonical
names hard-error rather than silently fall through. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `parse_canonical_intrinsic_name` covers every variant in
  `RegseqIntrinsic`.
- Recognition produces `IntrinsicCall` for registered DefIds and
  falls through to `Call` for unregistered ones.
- `&self` receiver preserved as `args[0]` for method intrinsics.
- Negative tests for unknown canonical name and unregistered
  DefId pass.
- Codex review approved.
