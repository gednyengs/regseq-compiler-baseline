# regseq-backends-22: Generic-C tuple-struct field naming (`ManuallyDrop`) (Phase-1)

## Description

Tuple-struct fields emit invalid C. `core::mem::ManuallyDrop<T>` (and its inner
`core::mem::MaybeDangling`) are **tuple structs**; the backend emits their single
field using its raw rustc index name (e.g. `0`), which is not a valid C struct
member / member access. Containing a `ManuallyDrop` therefore produces invalid C.

Surfaced by the §06 push (b5_control_flow). Blocks **IT-13** (`is_manually_drop`),
and more broadly affects any tuple struct reaching emission.

## Input Context

### Code anchors (confirm at HEAD — per workflow diagnosis)

- The struct field-name emission path: `crates/regseq-backends/src/generic_c/layout_emit.rs`
  (member declaration) + the `Field` place projection in `body_emit.rs` (member
  access). For a tuple struct, `FieldLayout.name` is the numeric index; it must be
  mapped to a valid C identifier (the established convention for anon/tuple fields,
  e.g. `_0`,`_1`,… as already used for tuples — confirm tuples vs tuple-structs use
  the same spelling).

## Implementation Notes

Emit tuple-struct fields with the same valid-C-identifier spelling already used for
tuple types (`_0`,`_1`,…) — both the member declaration and the `.field`/`._0`
projection must agree. Verify the offsetof `_Static_assert` guard uses the same
name so it still compiles. Check this is the single source of truth (no second
field-naming path for tuple structs vs tuples).

## Non-Goals

- No change to named-field struct emission.
- Not `ManuallyDrop`'s drop-suppression semantics (it is a marker; the field-name
  emission is the only blocker).

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture containing a tuple struct AND a
   `core::mem::ManuallyDrop<T>` field, constructed + a field read + logged,
   compiles-and-runs → IT-13 green; re-run the runner, ledger byte-stable.
3. Backend unit test: a tuple-struct field emits `_0` in both the definition and
   the access, and the offsetof guard matches.

## Quality Gate

`cargo fmt --all -- --check`; `cargo clippy -p regseq-backends -p codegen-coverage-runner
--all-targets -- -D warnings` (scoped); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture with a tuple struct + `ManuallyDrop` field
   compiles-and-runs; IT-13 green in the ledger.
2. Tuple-struct member declaration, access, and offsetof guard all use the same
   valid-C name.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
