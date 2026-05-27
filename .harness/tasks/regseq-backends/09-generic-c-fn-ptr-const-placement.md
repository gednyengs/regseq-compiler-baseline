# regseq-backends-09: Generic-C fn-pointer-value `const` placement (07 follow-up #3)

## Description

Close the **one residual** the `regseq-backends-07` soundness review found
(finding #3): a `Ref` / `Ptr` whose **pointee is a function pointer** places
`const` on the function's **return type** instead of on the function-pointer
**value**. For `&fn() -> i32` (a shared reference to a `fn` pointer) the
backend emits

```c
const int32_t (*(*p))(void)     /* WRONG: const on the RETURN type */
```

instead of the correct

```c
int32_t (*const (*p))(void)     /* const on the fn-pointer VALUE */
```

It **compiles** and does not miscompile, but the qualifier lands on the wrong
level (a *silent wrong `const`*, the same class as the original task-07 fix
#4) — the `_Static_assert` layout guards do not catch a qualifier error. The
task-07 implementer **documented this in-code** as out-of-verified-scope
(`types.rs` `declare_qualified` `Ty::FnPtr` branch). This task removes the
limitation so the `const`-placement story (07 Fix B) is complete for the
fn-pointer-pointee case too.

This is a **small, surgical** declarator fix — no new constructs, no behavior
change beyond the emitted qualifier position.

## Input Context

### Source of the finding
- The `regseq-backends-07` close assessment (`.harness/HANDOFF.md` § Update
  log, 07 entry): Codex soundness finding #3 (the 2 other findings were false
  positives; #3 is the real, minor residual). Verified against HEAD below.

### Spec anchors (`docs/discussions/regseq-model/generic-c-backend.md`)
- **§3.1** (lines 104–115): the `const` / mutability rule — a shared `Ref`
  to a non-`UnsafeCell` pointee is `const`-qualified; the qualifier must sit
  on the **pointee**, which for a fn-pointer pointee is the fn-pointer *value*
  (a `const` function *pointer*), **not** the function's return type.
- **§15** (starts at line ~1020; the "Nested `FnPtr` declarators recurse
  through the same table" sentence is at ~1080) — the declarator must keep
  composing correctly when the fn-pointer is itself a `const` pointee.

### Code anchors (`crates/regseq-backends/src/generic_c/types.rs`, at HEAD `3b66248` — the FnPtr declarator code is unchanged since task 07 `34a4ae1`)
- `declare_qualified(model, names, ty, top_const, decl)` — the `Ty::FnPtr(sig)`
  arm currently does `let inner = fn_ptr_decl(...); if top_const { format!(
  "const {inner}") } else { inner }`. The `format!("const {inner}")` is the
  bug: it prefixes `const` to the whole rendered fn-pointer declaration, which
  C parses as qualifying the **return type**.
- `fn_ptr_decl(model, names, sig, decl)` — builds the fn-pointer declarator as
  `declare(model, names, fn_sig.output, &format!("(*{decl})({params})"))`. The
  `*` here is the fn-pointer's own pointer; a `const` on the fn-pointer *value*
  must sit **immediately after this `*`** (`(*const {decl})({params})`).
- `ptr_to` / `pointer_decl` (07 Fix B): already thread `pointee_const` /
  `self_const`; this task extends the same discipline through the `FnPtr`
  declarator (the one declarator level Fix B did not cover).

### Prior task
- `regseq-backends-07` (`34a4ae1`, done): introduced `declare_qualified` +
  the `pointee_const` / `self_const` threading and documented the `FnPtr`
  punt. This task removes that punt. All existing tests must stay green.

## Implementation Notes

### Fix: place `const` on the fn-pointer value, not the return type

Thread the fn-pointer-value `const` into `fn_ptr_decl` (mirroring how
`ptr_to` already places `*const` for a const pointer value):

- Give `fn_ptr_decl` a `self_const: bool` parameter; when `true` it emits the
  declarator as `(*const {decl})({params})` (the `const` adjacent to the
  fn-pointer's `*`), else `(*{decl})({params})` as today.
- In `declare_qualified`'s `Ty::FnPtr(sig)` arm, pass `top_const` as that
  `self_const` (a `const`-qualified fn-pointer **value**) and **drop** the
  `format!("const {inner}")` prefix. The return type continues to render
  through the existing recursion (unqualified unless its own type says so).

The result, for `&fn() -> i32` (shared ref → `pointee_const = true` for the
fn-pointer, `self_const = false` for the outer ref):
`int32_t (*const (*p))(void)` (const on the fn-pointer value;
parenthesization is C-equivalent to `int32_t (*const *p)(void)`).
`&mut fn() -> i32` / `*mut fn() -> i32` → no `const`.

Keep the `ref_is_const` / `contains_unsafe_cell` logic unchanged — only the
**placement** of the resulting qualifier changes. Do not alter the non-`const`
fn-pointer path (the recursive declarator from 07 Fix D) — `fn() -> fn() -> i32`
and `fn() -> &[u32; 4]` must still emit the same valid C.

## Non-Goals
- No new constructs; no change to the §14 offset/padding math, discovery,
  ordering, naming, the enum representation, or banks 4/5.
- No green codegen-coverage fixture / no Doc-1 status flip; ledger verdict
  stays `not-pass`.
- No deps; no harness changes (reuse `toolchain` from tests).
- Const-qualifying a bare **function type** (not behind a pointer) remains a
  non-issue — a `fn` value only appears in C behind a pointer (a fn-pointer),
  which this task handles; there is no other reachable `top_const`-on-`FnPtr`
  path.

## Test Plan
### B1: regression — all existing tests stay green
`cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
(the c1 + 07 + c2 suite, incl. `fn_ptr_returning_fn_ptr_recurses_the_declarator`
and `fn_ptr_returning_pointer_to_array_recurses_the_declarator`, unchanged).

### B2: the fix (new tests)
- A declarator unit test: `&fn() -> i32` → the fn-pointer value is `const`
  (assert the emitted spelling places `const` adjacent to the fn-pointer `*`,
  e.g. contains `(*const ` and **not** a leading `const int32_t`); pin the
  exact expected string from a C-correct reading).
- `&mut fn() -> i32` (or `*mut fn() -> i32`) → **no** `const` on the
  fn-pointer value.
- A **compile** test (reuse `toolchain::compile_and_run`): a struct field of
  type `&fn() -> i32` emits a TU that compiles clean, and the field round-trips
  as a `const`-pointer-to-fn-pointer (a wrong-level `const` either fails to
  compile under a deliberate assignment-through-const probe, or — minimally —
  the emitted spelling is asserted exactly). Where the bridge cannot surface
  the shape, hand-build the model (cf. the c1/07 `repr(packed)` pattern).

### B3: determinism + ledger + scope
`emit_generic_c` byte-stable; ledger unchanged (`not-pass`); diff confined to
`crates/regseq-backends/src/generic_c/types.rs` + its tests (+ the runner test).

### B4: fmt + clippy
`cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
codegen-coverage-runner --all-targets -- -D warnings` clean.

## Review
### Files to send to Codex
- This spec; `generic_c/types.rs` (at HEAD); gc §3.1 / §15.

### Reviewer focus
- Is `int32_t (*const (*p))(void)` the C-correct + Rust-faithful spelling of
  `&fn() -> i32` (const on the fn-pointer value, not the return), and does the
  fix place it there for the shared-ref / `*const` cases while leaving
  `&mut` / `*mut` unqualified?
- Does the non-`const` recursive fn-pointer path (07 Fix D) stay byte-identical
  (`fn() -> fn() -> i32`, `fn() -> &[u32; 4]`)?
- Any other declarator level where the fn-pointer-value `const` is still
  mis-placed (e.g. a fn-pointer that is itself the const pointee inside an
  array, `&[fn() -> i32]`)?

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/09-generic-c-fn-ptr-const-placement.md —
a small follow-up fixing where `const` lands for a Ref/Ptr to a fn-pointer
(07 finding #3): the backend puts `const` on the fn's RETURN type
(`const int32_t (*(*p))(void)`) instead of the fn-pointer VALUE
(`int32_t (*const (*p))(void)`). The fix threads a `self_const` into
fn_ptr_decl to emit `(*const decl)(params)` and drops the `format!("const
{inner}")` prefix in declare_qualified's FnPtr arm. Verify the target spelling
is C-correct + Rust-faithful for `&fn()->i32` / `&mut fn()->i32` / `*const
fn()->i32`, that the non-const recursive fn-ptr path (07 Fix D) is unchanged,
and that no other declarator level mis-places the qualifier. Read
generic_c/types.rs at HEAD + gc §3.1/§15.
</task>
<grounding_rules>
- Cite file:line. Mark hypotheses. Don't speculate about unread code.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
FIX-CORRECTNESS (const on fn-ptr value, C + Rust faithful): PASS / FAIL with citation
NO-REGRESSION (07 Fix D recursive path unchanged): PASS / FAIL with citation
RESIDUAL-MISPLACEMENT (other declarator levels): list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (existing suite + the new fix tests).
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`).
- `git diff --stat` confined to `generic_c/types.rs` + tests + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria
1. **#3 fixed**: a `Ref` / `Ptr` to a fn-pointer pointee places `const` on the
   fn-pointer **value** (`int32_t (*const (*p))(void)` for `&fn() -> i32`), not
   the return type; `&mut` / `*mut` fn-pointer pointees carry no `const`.
2. **No regression**: the 07 Fix D recursive fn-pointer-return path
   (`fn() -> fn() -> i32`, `fn() -> &[u32; 4]`) is byte-identical; the full
   existing suite stays green; determinism preserved; ledger unchanged.
3. **Scope**: diff confined to `generic_c/types.rs` + tests + registration; no
   other module, doc, or harness change.
