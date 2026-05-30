# regseq-backends-20: Generic-C ReifyFnPointer from a ZST `FnDef` constant (Phase-1)

## Description

Reifying a fn item to a fn-pointer value (`let f: fn(i32)->i32 = some_fn;`, MIR
`Cast(PointerCoercion(ReifyFnPointer), Const::ZST, FnPtr)`) fails to emit:
`unsupported construct: ReifyFnPointer of an unresolved function (task d2)`. The
operand is the zero-sized `FnDef` **constant**, but the backend's
`reify_fn_pointer` only resolves a `Const::Reloc` operand OR a *place* of `FnDef`
type — and `operand_ty(Operand::Constant(_))` returns `None`, so the FnDef-place
branch is unreachable for a constant operand, and it falls through to the error.

Surfaced by the §06 codegen-coverage push (b2_floats_casts, b5_control_flow).
Blocks matrix rows **CK-9** (FnPtrToPtr — needs a reified fn-ptr first), **CK-11**
(Subtype — same reify prerequisite), and **PC-1** (ReifyFnPointer).

(`IN-5`, the FunctionInstanceFlavor row, is NOT part of this task — Codex review
confirmed a plain fn-item reify never materializes a ReifyShim instance, and IN-5
is any non-Item shim flavor, already genuinely exercised by `b5_control_flow`'s
synthesized DropGlue shim. IN-5 was greened by adding it to that fixture's covers.)

## Input Context

### Code anchors (confirm at HEAD — per workflow diagnosis)

- `crates/regseq-backends/src/generic_c/body_emit.rs:~1637` — `reify_fn_pointer`:
  handles `Const::Reloc` + a `FnDef`-typed place; lacks the `Const::ZST` FnDef
  operand case.
- `crates/regseq-backends/src/generic_c/body_emit.rs:~1646-1658` — the place-branch
  scan that matches `item + generic_args` against `model.instances` to find the
  target instance's `emission_name`; reuse it for the constant case.
- `crates/regseq-backends/src/generic_c/body_emit.rs:~204` — `operand_ty` returns
  `None` for any `Operand::Constant`.
- Existing test `mod_tests.rs:~2678`
  (`reify_fn_pointer_to_generic_instance_matches_definition_symbol`) passes only
  because it hand-builds an `Operand::Copy(place)` of FnDef type — a surface rustc
  does NOT emit for this construct (it emits `Operand::Constant(Const::ZST)`).

## Implementation Notes

Resolve a `ReifyFnPointer` whose operand is a `Const::ZST` of `FnDef(item, args)`
type: read the `FnDef`'s item + generic args from the constant's type, run the
same instance-matching scan the place-branch uses, and emit `&<emission_name>`.
Keep the `Const::Reloc` path. Add a `kind=rust` regression surface (not just the
hand-built `Operand::Copy` test) so the rustc-emitted constant operand is covered.

## Non-Goals

- `IN-5` is out of scope (greened separately via the DropGlue shim, see Description).
- Not the closure→fn-ptr coercion (`ClosureFnPointer`, PC-3 — vtable task).

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture: `let f: fn(i32)->i32 = dbl; f(21)`
   (indirect call through the reified pointer) compiles-and-runs → PC-1 green;
   add `(f as *const ())` for CK-9; re-run `cargo run -p codegen-coverage-runner` →
   PC-1/CK-9 (and CK-11 if constructible) flip green, ledger byte-stable.
3. Backend unit test with the rustc-shaped `Operand::Constant(Const::ZST)` FnDef
   operand.

## Quality Gate

`cargo fmt --all -- --check`; `cargo clippy -p regseq-backends -p codegen-coverage-runner
--all-targets -- -D warnings` (scoped); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture reifying a fn item to a fn pointer + calling it indirectly
   compiles-and-runs; PC-1 (and CK-9) green in the ledger.
2. Backend unit test covering the `Const::ZST` FnDef reify operand passes.
3. Disposition recorded for CK-11 (green, or deferred with a reason).
4. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
