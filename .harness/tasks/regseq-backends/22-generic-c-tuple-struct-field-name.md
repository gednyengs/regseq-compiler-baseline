# regseq-backends-22: Generic-C tuple-struct field naming + `is_manually_drop` (IT-13) (Phase-1)

## Description

Two coupled gaps block **IT-13** (`is_manually_drop`):

1. **Tuple-struct field naming (the compile prerequisite).** Tuple-struct fields are
   emitted with their raw rustc index name (`0`), which is not a valid C identifier:
   `struct ... { uint32_t 0; }` / `offsetof(struct ..., 0)`. Only `Ty::Tuple` gets the
   valid `_0`/`_1` spelling; `Ty::Struct` fields run through `sanitize(name)`, which
   preserves the leading digit. Any tuple struct fails to compile — and
   `core::mem::ManuallyDrop<T>` (+ its inner `MaybeDangling`) ARE tuple structs, so a
   `ManuallyDrop` cannot even be contained.

2. **`IT-13` is a drop-glue *behavior*, not just compilation** (Codex spec review).
   The matrix row is that a `ManuallyDrop<D>` field is NOT dropped by the containing
   type's drop glue. Greening IT-13 requires a fixture that OBSERVES that suppression
   (D's `Drop` does not run), not merely that `ManuallyDrop` compiles.

Surfaced by the §06 push (b5_control_flow used a named-field Drop type to dodge gap 1).

## Input Context

### Code anchors (confirm at HEAD — per Codex review)

- `crates/regseq-backends/src/generic_c/layout_emit.rs:~143-176` — `Ty::Tuple` →
  `_0`/`_1`; `Ty::Struct` → `sanitize(name)`. Tuple structs take the `Struct` path.
- `crates/regseq-backends/src/generic_c/naming.rs:~67-77` — `sanitize` preserves a
  leading digit (so a tuple-struct field name `0` stays `0`).
- Drop-glue path: `is_manually_drop` has **no** generic-C consumption today
  (`rg is_manually_drop` → only tests/pretty). The drop-suppression behavior is
  expected to already live in rustc's drop-elaborated MIR (the bridge materialises
  it); confirm the synthesized drop glue for a `ManuallyDrop<D>` field already omits
  the `D::drop` call — if it does NOT reach the backend correctly, this task also
  consumes `is_manually_drop` in drop-glue emission.
- Matrix IT-13 (`docs/.../01-construct-surface-and-matrix.md:118`, "affects drop-glue
  emission").

## Implementation Notes

1. Emit tuple-struct fields with the same `_0`/`_1` spelling tuples use — member
   declaration, `.field` projection, AND the `offsetof` `_Static_assert` guard must
   all agree (single source of truth; don't fork tuple vs tuple-struct naming).
2. Confirm (and, if necessary, implement) `ManuallyDrop` drop-suppression so the
   IT-13 fixture observes it. Default expectation: rustc drop elaboration already
   omits the drop; the task verifies this end-to-end and only adds `is_manually_drop`
   handling if the materialised glue still drops the field.

## Non-Goals

- No change to named-field struct emission.
- Not general `MaybeDangling`/`ManuallyDrop` API surface beyond the field-name +
  drop-suppression behavior.

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture: `struct Hold { md: core::mem::ManuallyDrop<D> }`
   where `struct D { v: u32 }` has `impl Drop` writing a sentinel to a `static mut`.
   Construct + drop a `Hold` in an inner scope; the sentinel must show `D::drop` did
   NOT run (vs a plain `D` control that DOES) — logged. Compiles-and-runs → IT-13
   green; re-run the runner, ledger byte-stable.
3. Backend unit test: a tuple-struct field emits `_0` in definition, access, and the
   offsetof guard.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`, `codegen-coverage-runner`,
and `regseq-driver` if drop-glue handling is touched); single-threaded scoped tests;
ledger byte-stable.

## Acceptance Criteria

1. Tuple-struct member declaration, access, and offsetof guard use the same valid-C
   `_N` name; any tuple struct compiles.
2. A `kind=rust` fixture OBSERVES `ManuallyDrop<D>` drop-suppression (D's drop does
   not run); IT-13 green in the ledger.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
