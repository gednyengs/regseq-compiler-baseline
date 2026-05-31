# regseq-backends-25: Generic-C fn-pointer-return declarator (`fn_signature`) (Phase-1)

## Description

A C function whose **return type is a function pointer** is emitted with a
malformed declarator. For a Rust `fn coerce() -> fn() -> i32` the backend emits

```c
int32_t (*)(void) coerce(void);          // MALFORMED — does not compile
```

instead of the well-formed declarator

```c
int32_t (*coerce(void))(void);
```

`fn_signature` builds the parameters through the declarator-aware
[`types::declare`] (so a fn-pointer *parameter* is spelled correctly) but builds
the **return type** with the bare [`types::c_type`] and then *string-concatenates*
`"{ret} {emission}({params})"`. Bare-type concatenation cannot form the C
declarator `R (*name(params))(...)` for a function-pointer (or pointer-to-array)
return — the same invalid `int (*)(void) (*decl)(void)` shape that the field /
local path already avoids by recursing the declarator (see
`mod_tests.rs::fn_ptr_returning_fn_ptr_recurses_the_declarator`, which proves
`fn_ptr_decl` + `declare` handle exactly this nesting for struct fields).

Confirmed empirically: a throwaway unit test emitting a `coerce() -> fn() -> i32`
prototype produced `int32_t (*)(void) coerce(void);` (reverted after capture). The
gap is currently masked at the codegen-coverage layer: `b_subtype` deliberately
does the CK-11 Subtype coercion as a *local assignment*, not a fn return, with a
comment noting it dodges "the separate fn-pointer-return declarator path"
(`crates/codegen-coverage-runner/fixtures/b_subtype/input.rs:56-61`). No matrix
row is failing, so this is **non-gating** — a latent declarator-correctness bug,
not a coverage hole.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/mod.rs:920` — `fn_signature`. The bug:
  - line ~965-970: `let ret = … types::c_type(model, names, out_ty);` (bare type).
  - line ~978: `format!("{quals}{noreturn}{ret} {emission}({params_str})")`
    (string concatenation — cannot wrap a fn-ptr-return declarator).
  - line ~953: parameters correctly use `types::declare(model, names, eff, &pname)`.
- `crates/regseq-backends/src/generic_c/types.rs:76` — `declare(model, names, ty,
  decl)`: the declarator-aware helper. `decl` is the thing being declared (a name,
  or a nested declarator). Passing the function's `name(params)` declarator here
  is the fix shape.
- `crates/regseq-backends/src/generic_c/types.rs:261` — `fn_ptr_decl`: already
  spells `R (*decl)(params)` by recursing `declare` with the inner declarator, so
  routing the fn header through `declare` reuses this for the return type.
- `crates/regseq-backends/src/generic_c/mod_tests.rs:846` /`:863` —
  `fn_ptr_returning_fn_ptr_recurses_the_declarator` /
  `…_pointer_to_array_recurses_the_declarator`: the field-context proofs the
  fn-header path should match.
- `crates/codegen-coverage-runner/fixtures/b_subtype/input.rs:56-61` — the
  workaround comment documenting the avoided path.
- Matrix `TY-13` (`docs/.../phase-1/01-construct-surface-and-matrix.md:198`,
  "`FnPtr(FnSigId)` → `T (*)(args)`") — covered for fields/locals/params; the
  return-position declarator is the untested sub-case.

## Implementation Notes

Route the function header through the same declarator machinery the params use:
build the declarator `"{emission}({params_str})"` and spell the whole signature as
`types::declare(model, names, out_ty, &declarator)` (prepending `quals` /
`_Noreturn`), instead of `c_type(out_ty)` + concatenation. Preserve every existing
behavior for the common (non-fn-ptr-return) case — a scalar/aggregate/void return
must remain byte-identical (e.g. `uint32_t add(uint32_t, uint32_t)`), so the
existing `scalar_function_emits_prototype_and_body` and the whole golden corpus
stay green. Watch the ZST-return → `void` and `Never` → `_Noreturn void` special
cases (they bypass the type entirely today; keep them).

## Non-Goals

- Not a new matrix row — TY-13 is already covered; this is declarator correctness
  for the return position.
- Not the closure/reify fn-pointer *value* lowering (PC-1/PC-3, backends-20 + the
  vtable task) — those are already done.
- No change to parameter, field, local, or static declarator emission (already
  correct).

## Test Plan

1. Build + scoped tests green.
2. Backend unit test (`mod_tests.rs`): a function instance `coerce() -> fn() -> i32`
   emits the prototype `int32_t (*coerce(void))(void);` (and a pointer-to-array
   return variant, mirroring the existing field-context pair). The pre-fix
   malformed `int32_t (*)(void) coerce(void);` must no longer appear.
3. A `kind=rust` codegen-coverage fixture exercising a `fn returning a fn pointer`
   that is then called — compiles-and-runs under the dual oracle; re-run
   `cargo run -p codegen-coverage-runner`, ledger byte-stable. (Optionally fold the
   CK-11 witness back into a fn-return shape so `b_subtype` no longer needs the
   local-assignment workaround — note the decision either way.)
4. Regression: `scalar_function_emits_prototype_and_body` and the golden corpus
   unchanged for non-fn-ptr returns.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`,
`codegen-coverage-runner`); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable (run the
runner twice + diff).

## Acceptance Criteria

1. A function with a fn-pointer (and pointer-to-array) return type emits a
   well-formed C declarator (`R (*name(params))(...)`), proven by a backend unit
   test; the malformed bare-concatenation form is gone.
2. Non-fn-ptr returns are byte-identical (scalar test + golden corpus green).
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
