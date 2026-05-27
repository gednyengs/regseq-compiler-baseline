# regseq-backends-05: Generic-C prelude + 5-bank emission envelope + panic/helpers (Phase-1 task b)

## Description

Begin the generic-C backend body behind the `emit_generic_c` seam
(`regseq-backends-04`, `9e3f46b`): implement the **model-independent
scaffolding** — the §15 **five-bank translation-unit driver**, the **static
prelude** (bank 1: includes + `struct rust_str` + the §16 panic
infrastructure), and the **arithmetic/cast helper library** (§10/§11
`*WithOverflow` + saturating `FloatToInt`). This is **task b** of the
Phase-1 a→e sequence (`.harness/HANDOFF.md`).

After this task, `emit_generic_c` returns a **real** translation unit — the
prelude + helper block, with banks 2–5 (types / declarations / definitions)
produced by empty hooks that tasks c/d fill — instead of
`Err(EmitError::NotImplemented)`. The TU compiles standalone (prelude is
well-formed C); the panic routine and helpers are exercised by dedicated
tests. **No green codegen-coverage fixture is produced by this task** — a
green fixture needs a function body (task d) + diag-log (task e), so the
first end-to-end behavioral fixture is task e. Task b's verification is
backend-internal: the prelude/helpers/panic **compile and behave correctly**
(tested by emitting them + a hand-written C exerciser through the host `cc`),
and the 5-bank driver assembles a deterministic TU.

### Scope boundaries this task pins (flagged for review — see § Review)

1. **Model-independent only.** Task b emits what does **not** depend on the
   model's types/instances: the static prelude + the fixed helper block + the
   bank-ordering driver. The per-`T` fat-pointer structs (`rust_slice_<T>`,
   `trait_obj_<Trait>`, §3.2) are **type-driven → task c**; only the
   monomorphic `struct rust_str` ships in the prelude here. Bank 2–5 contents
   are **task c/d**; task b ships their hooks as empty.
2. **Helper block = fixed, not on-demand.** The `*WithOverflow` result
   structs + `__builtin_*_overflow` wrappers and the saturating `FloatToInt`
   helpers are emitted as a **fixed block covering the primitive integer /
   float type set**, `static inline` (so unused ones don't warn under
   `-Wall`). Rationale: matches the HANDOFF split (b = helper *definitions*,
   d = their *use* at BinaryOp/Cast sites) and keeps b self-contained (no
   model-walk, no demand registry, no coupling to d). Bloat is acceptable for
   P1 (host validation, not size-constrained). `_Float16`/`_Float128`
   helpers are host-gated (§7 Doc-3 / spec §11). (Reviewer: confirm
   fixed-block vs on-demand.)
3. **`AssertKind` messages: routine + catalog here; per-site formatting in
   d.** Task b ships `regseq_panic` (§16) + the `Unreachable` trap message +
   the fixed `AssertKind` message strings (§16 table). The operand-bearing
   messages (`BoundsCheck{len,index}`, `MisalignedPointerDereference`) are
   *formatted at the `Assert` terminator site* (task d, which has the operand
   values); task b provides the routine + the message catalog the site uses.

## Input Context

### Design source — the C-emission spec (`docs/discussions/regseq-model/generic-c-backend.md`)

- **§15 Declaration order, signatures & C-ABI prototypes** (lines 988–1044):
  the **five-bank** single-TU structure — (1) prelude [includes, §3.2
  fat-pointer typedefs, §16 panic, §10/§11 helpers], (2) type forward-decls,
  (3) type definitions (topo-sorted by by-value containment), (4)
  declarations (prototypes + `extern` statics), (5) definitions. Task b
  builds the **driver** + bank 1; banks 2–5 are c/d. Note the **diag-log
  exclusion** (an `IntrinsicKind::Log` instance is skipped in banks 4/5) — a
  driver concern task b's hooks must accommodate but task e wires.
- **§16 Host panic & `AssertKind` messages** (lines 1048–1078): the
  `_Noreturn static void regseq_panic(const char *msg)` routine (→ **stderr**
  + `abort()`, keeping the §8 stdout oracle clean); the 8-row `AssertKind`
  message table; `Unreachable` → the trap with `"internal error: entered
  unreachable code"`.
- **§3.2 Fat-pointer C struct shapes** (lines 116–145): `struct rust_str {
  const uint8_t *data; size_t len; }` is the one **monomorphic** typedef for
  the prelude. `rust_slice_<T>` / `trait_obj_<Trait>` are per-instantiation →
  **task c**.
- **§10 / §10.1 `Rvalue` / `BinOp`** (lines 820–863): the `*WithOverflow`
  helpers — `__builtin_{add,sub,mul}_overflow(a,b,&tmp)` returning a
  `struct { T v; _Bool overflow; }` per integer type `T`. (Shift masking +
  Div/Rem guarding are *use-site* rules → task d; task b emits no shift/div
  helper.)
- **§11 Cast C-emission** (lines 866–889): the saturating `FloatToInt`
  helper, per (source-float `F` × dest-int `DI`): `if (isnan(x)) return 0; if
  (x <= (F)DI_MIN) return DI_MIN; if (x >= (F)DI_MAX) return DI_MAX; return
  (DI)x;`. (`Transmute` via `memcpy`, `PtrToPtr`, etc. are use-site → task d.)
- (Context, **not** task b) §12 `Const` initializers (892–913), §13 enum,
  §14 field-offset — tasks c/d.

### Design source — the architecture decisions (`docs/discussions/generic-c-backend/phase-1/02-architecture-decisions.md`)

- **D2** (lines 80–140) — declaration order: forward-declare everything; the
  only hard order is the bank-3 by-value-containment topo-sort. Task b builds
  the 5-bank frame; the topo-sort itself is task c (it needs the types).
- **D4** (lines 178–216) — host panic/abort: the `regseq_panic` shape +
  `Assert`/`Unreachable`/`panic=abort` routing + the `AssertKind` table that
  §16 codifies.
- **D7** (lines 311–379) — Rvalue/operators: the rationale for the
  `*WithOverflow` builtins + (use-site) `-fwrapv` / shift-domain rules.
- **D8** (lines 383–414) — casts: the saturating-cast-helper contract +
  host assumptions.

### Code anchors

- `crates/regseq-backends/src/lib.rs` — the `emit_generic_c(&RegseqModel) ->
  Result<String, EmitError>` seam + `#[non_exhaustive] EmitError` from task a.
  Task b moves the implementation into a `generic_c` module and **keeps the
  public `emit_generic_c` / `EmitError` re-exported from `lib.rs`** so the
  harness's `regseq_backends::emit_generic_c` path is unchanged.
- `crates/regseq-model/src/` — the IR. Task b touches it minimally (the
  driver takes `&RegseqModel`; bank-1 emission is model-independent). Confirm
  the `RegseqModel` import + any field the driver skeleton reads (e.g. the
  instance list it will *later* iterate) against HEAD.
- `crates/codegen-coverage-runner/src/toolchain.rs` —
  `compile_and_run(tu_c, entry, cc)` + `generate_main_c`. Task b's
  **behavioral** tests reuse this to compile + run the prelude + a
  hand-written exerciser (see Test Plan). The runner already depends on
  `regseq-backends`, so this is the natural home for the compile-and-run
  tests (the runner is the backend's verification harness).

### Prior tasks

- `regseq-backends-04` (`9e3f46b`, done): the harness + the `emit_generic_c`
  seam stub this task fills + the `toolchain` module the tests reuse.
- Doc-1 status reconciliation (`111da9b`): the matrix's `covered` rows are
  the in-P1-scope set; coverage is tracked by the ledger. Task b closes **no**
  rows to green (no end-to-end fixture yet) — see Non-Goals.

## Implementation Notes

### Change A: backend module structure

**Anchor**: `crates/regseq-backends/src/lib.rs`.

Introduce a `generic_c` module; move the emit entry there; keep the public
surface re-exported:

```
crates/regseq-backends/src/
  lib.rs              // pub use generic_c::{emit_generic_c, EmitError};  (+ crate docs)
  generic_c/
    mod.rs            // the 5-bank driver: emit_generic_c body + EmitError
    prelude.rs        // the static prelude + the helper block (Change C/D)
```

`EmitError` keeps `#[non_exhaustive]` and its `NotImplemented` variant
**retained in the enum but no longer returned on the task-b happy path** —
the bank-2–5 hooks return `Ok(String::new())` (Change B), so
`emit_generic_c` returns `Ok(tu)` for any valid model. (The variant stays
as a placeholder; c/d may add real error variants — e.g. the §15 by-value
cycle = model corruption — or `NotImplemented` may be removed when nothing
uses it.) `lib.rs` is a thin re-export; the harness import path does not
change. (Tasks c/d add `generic_c/{types.rs, body.rs, …}`.)

### Change B: the five-bank driver (`generic_c/mod.rs`, §15 / D2)

`emit_generic_c` assembles the TU as the five banks **in order**:

```rust
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(prelude::PRELUDE);          // bank 1 (Change C) — model-independent
    out.push_str(&prelude::helper_block());  // bank 1 cont. (Change D)
    out.push_str(&bank2_type_forward_decls(model)?);  // task c hook -> "" for now
    out.push_str(&bank3_type_definitions(model)?);    // task c hook -> "" for now
    out.push_str(&bank4_declarations(model)?);        // task c/d hook -> "" for now
    out.push_str(&bank5_definitions(model)?);         // task d hook -> "" for now
    Ok(out)
}
```

Each `bankN_*` is a private function returning `Result<String, EmitError>`;
task b ships them returning `Ok(String::new())` with a `// TODO(task c/d)`
marker citing the §15 bank + the Doc-1 rows it will close. The driver's
**ordering + assembly** is the deliverable; the bank bodies are c/d. The
diag-log-exclusion rule (§15) is documented at the bank-4/5 hooks for task e.
Keep the output a **pure function of the model** (no wall-clock / iteration
nondeterminism) — the snapshot oracle depends on byte-stability
([[codegen-ledger-determinism]]).

### Change C: the static prelude (`generic_c/prelude.rs`, §15 bank 1 + §16 + §3.2)

A `pub const PRELUDE: &str` (or a builder returning a fixed string) with, in
order:

1. **Includes**: `<stdint.h>`, `<stddef.h>`, `<stdio.h>`, `<stdlib.h>`,
   `<string.h>`, `<math.h>` (the saturating helper uses `isnan`).
2. **`struct rust_str`** (§3.2): `struct rust_str { const uint8_t *data;
   size_t len; };`. (Only the monomorphic str fat-pointer; slices/trait-objs
   are task c.)
3. **Panic infrastructure** (§16 / D4):
   ```c
   _Noreturn static inline void regseq_panic(const char *msg) {
       fprintf(stderr, "panic: %s\n", msg);
       abort();
   }
   ```
   plus a `regseq_unreachable()` (or a documented call to `regseq_panic`)
   carrying `"internal error: entered unreachable code"`.
   **`static inline`, pinned** (a faithful refinement of §16's illustrative
   `static void`): an unused `static inline` function does **not** warn under
   `-Wall`, so the standalone prelude (a TU with no `Assert`/`Unreachable`
   site, e.g. task b's own compile test) is warning-free. A plain `static`
   routine would warn "defined but not used" in that case. Behavior
   (`_Noreturn`, stderr text, `abort`) is unchanged.
4. **The `AssertKind` message catalog** (§16 table): the fixed message
   strings as C string constants / `#define`s the task-d `Assert` site uses
   (the operand-bearing ones — `BoundsCheck`, `Misaligned` — are `printf`-style
   templates formatted at the site). Document each against its `AssertKind`.

All file-scope helpers are `static inline` (single TU; `static inline` is
the idiom that is warning-free even when unused — see the panic note above).
The prelude must compile standalone with the §7 flags (`-std=gnu11 -fwrapv
-Wall`) with **no warnings**; the implementer verifies this (the Change-E
empty-`main` behavioral test asserts `cc` stderr is warning-free).

### Change D: the arithmetic / cast helper block (`generic_c/prelude.rs`, §10/§11)

A `pub fn helper_block() -> String` (or a second `const`) emitting, as
`static inline` functions:

- **`*WithOverflow`** (§10.1): for each integer type `T` in the primitive set
  (`int8_t … int64_t`, `__int128`, `intptr_t`; `uint8_t … uint64_t`,
  `__uint128_t`, `size_t`), a result `struct { T v; _Bool overflow; }` + an
  `add/sub/mul` wrapper around `__builtin_{add,sub,mul}_overflow`. (Names must
  be collision-free + stable; document the naming scheme.)
- **Saturating `FloatToInt`** (§11): for each (`F` ∈ {`float`,`double`}, `DI`
  ∈ the integer set), the bounds-checked helper from §11. The bounds use the
  type's `*_MIN`/`*_MAX` (or computed bounds where a macro is absent, e.g.
  `__int128`).
- **`_Float16` / `_Float128` gating — pinned to source-level `#if`.** The
  `_Float16`- and `_Float128`-source saturating helpers are emitted
  **wrapped in C preprocessor `#if` guards** on the type's availability
  macro — `_Float16` under `#if defined(__FLT16_MANT_DIG__)`, `_Float128`
  under `#if defined(__FLT128_MANT_DIG__)` (the implementer **confirms the
  exact macro against the pinned host `cc`** — gcc/clang on the §7 toolchain;
  adjust to e.g. `__SIZEOF_FLOAT128__` if that is what the toolchain
  defines). This is **not** a Rust-side host probe: the emitted `.c` is
  **byte-identical across hosts** (the `#if` block is always present in the
  text; the preprocessor conditionally compiles it), preserving the golden-
  snapshot's machine-independence (Doc 3 §7). Do **not** branch the emitted
  text on the build host.

The block is a **fixed superset** (Scope decision 2). All `static inline` so
unused helpers are warning-free. The block compiles standalone under the §7
flags.

### Change E: tests

Following task a's split (structural vs behavioral):

- **Structural unit tests in `crates/regseq-backends/`** (`tests/` or inline
  `#[cfg(test)]`): `emit_generic_c` on a trivial/empty `RegseqModel` returns a
  TU that (a) starts with the includes, (b) contains `struct rust_str`,
  `regseq_panic`, the `AssertKind` messages, and the helper signatures, (c) is
  **byte-identical across two calls** (determinism). No `cc` here (fast).
- **Behavioral tests in `crates/codegen-coverage-runner/tests/`** (reusing
  `toolchain::compile_and_run`): emit the prelude/helper block + a
  **hand-written C exerciser `main`** and assert runtime behavior:
  - prelude + empty `main` → compiles clean, exits 0.
  - a `main` that calls `regseq_panic("boom")` → non-zero exit + stderr
    `"panic: boom\n"` (the §16 stderr contract; reuse the task-a panic-fixture
    pattern).
  - a `main` exercising a saturating helper: `f64 1e30 → int32` saturates to
    `INT32_MAX`; `NaN → 0`; `-1e30 → INT32_MIN` (§11).
  - a `main` exercising `add/mul WithOverflow`: a non-overflowing case
    (`overflow == 0`, `v` correct) + an overflowing case (`overflow == 1`).
  (Reviewer: confirm this cross-crate test placement — the backend's
  behavioral C tests living in the runner crate because it owns `toolchain`.)

### Change F: regenerate the ledger (sanity, not coverage)

Run `codegen-coverage-runner` once; the ledger verdict stays `not-pass`
(still no green fixtures) and byte-stable. Commit the regenerated ledger only
if it changed (it should **not** change — task b adds no fixtures and closes
no rows; the matrix is untouched). If it does change, that is a signal to
investigate before closing.

## Non-Goals

- **No bank 2–5 content.** Type forward-decls/definitions (banks 2–3, §13/§14
  enum/aggregate, the topo-sort), function prototypes/`extern` statics (bank
  4), and function bodies / static initializers (bank 5) are tasks c/d. Task
  b ships their hooks empty.
- **No per-`T` fat-pointer structs** (`rust_slice_<T>`, `trait_obj_<Trait>`)
  — task c. Only `struct rust_str` ships here.
- **No use-site lowering.** Shift masking, Div/Rem guarding (§10.1), the
  `BinaryOp`/`Cast`/`Const` C expressions (§10/§11/§12), and the per-site
  `Assert` message formatting (§16) are task d. Task b emits only the helper
  *definitions* + the panic routine + the message *catalog*.
- **No green codegen-coverage fixture / no Doc-1 row flips.** An end-to-end
  green fixture needs a body (d) + diag-log (e). Task b adds no fixture and
  edits no matrix status (the matrix status column is the spec axis, stable —
  [[codegen-ledger-determinism]] / HANDOFF § Decisions locked).
- **No harness changes.** `codegen-coverage-runner` is consumed read-only
  (its `toolchain` is reused by tests); the seam signature is unchanged.
- **No deps added** beyond what `regseq-backends` already has (it needs only
  `regseq-model`). No `cc`/codegen crates.
- Do **not** modify the spec, Doc 1/2/3, the bridge, the model, or
  `test-ir-fixtures`.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends` + `-p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (existing 24 runner tests unchanged + the new structural +
  behavioral tests). Per the HANDOFF gotcha use `RUST_TEST_THREADS=1` if the
  E0463 quirk appears.

### B2: prelude + helpers compile + behave (Change E)
- Structural: trivial-model TU contains includes / `rust_str` / `regseq_panic`
  / `AssertKind` messages / helper sigs; byte-identical across two emits.
- Behavioral (via `toolchain::compile_and_run`): prelude compiles clean;
  `regseq_panic` → stderr `"panic: boom\n"` + non-zero exit; saturating cast
  (1e30→INT32_MAX, NaN→0, -1e30→INT32_MIN); `*WithOverflow` (overflow flag +
  value) — all assert the §10/§11/§16 contracts.

### B3: standalone prelude compile under the pinned flags
The emitted prelude + helper block compiles with `cc -std=gnu11 -fwrapv
-Wall` with **no warnings** (the behavioral test's empty-`main` case covers
this; assert the `cc` stderr is warning-free).

### B4: determinism + ledger sanity
- `emit_generic_c` is byte-stable across calls (B2 structural).
- `codegen-coverage-runner` run: verdict still `not-pass`, ledger byte-stable,
  **unchanged** vs HEAD (task b closes no rows).

### B5: fmt + clippy
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-backends -p codegen-coverage-runner --all-targets --
  -D warnings` clean (scoped to the touched crates per
  [[clippy-all-targets-test-drift]]).

### B6: diff scope
`git diff --stat` confined to `crates/regseq-backends/**` (new `generic_c`
module + tests), `crates/codegen-coverage-runner/tests/**` (new behavioral
test), and the registration. No spec/doc/bridge/model/matrix/ledger changes
(ledger only if a re-run legitimately rewrites byte-identical output → no
diff).

## Review

### Files to send to Codex
- This spec.
- `generic-c-backend.md` §15 / §16 / §3.2 / §10–§11 (the emission contract).
- Doc 2 D2 / D4 / D7 / D8 (the decisions).
- `crates/regseq-backends/src/lib.rs` (the seam) +
  `crates/codegen-coverage-runner/src/toolchain.rs` (the reused test harness).

### Reviewer focus
- **Scope discipline (b vs c/d)**: is the model-independent/dependent split
  clean? Does anything in Changes A–D reach into bank 2–5 / use-site / per-`T`
  territory?
- **The three scope decisions** (Description): (1) only `rust_str` in the
  prelude (slices/trait-objs → c); (2) fixed helper block vs on-demand; (3)
  `AssertKind` routine+catalog here, per-site formatting in d. AGREE or
  propose alternative.
- **§16 / §10 / §11 fidelity**: does `regseq_panic`, the `AssertKind`
  catalog, the `*WithOverflow` structs/wrappers, and the saturating helper
  match the spec exactly (stderr text, `__builtin_*_overflow` shape, the
  bounds-before-cast order, NaN→0)?
- **`_Float16`/`_Float128` host-gating**: is the chosen mechanism (`#if`
  availability vs omission) sound + warning-free under `-Wall`?
- **Determinism**: is `emit_generic_c` a pure function of the model (no
  iteration-order/wall-clock nondeterminism)?
- **Cross-crate test placement**: behavioral C tests in the runner crate
  (reusing `toolchain`) vs duplicating cc-invocation in `regseq-backends` —
  acceptable?
- **Driver extensibility**: do the bank-2–5 hooks give c/d a clean seam
  (incl. the diag-log-exclusion accommodation for e)?
- Citation drift vs HEAD.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/05-generic-c-prelude-and-envelope.md —
Phase-1 task b of the generic-C backend: the model-INDEPENDENT scaffolding
(5-bank TU driver + static prelude + panic + the *WithOverflow / saturating
helper block) behind the emit_generic_c seam. Banks 2-5, per-T fat structs,
and use-site lowering are explicitly LATER tasks (c/d/e). Identify:
- Scope discipline: does b stay model-independent, or leak into c/d/e?
- Soundness of the 3 scope decisions (rust_str-only prelude; fixed helper
  block; AssertKind routine+catalog-here / formatting-in-d).
- §16/§10/§11 fidelity (regseq_panic stderr text; AssertKind messages;
  __builtin_*_overflow result struct; saturating bounds-before-cast + NaN->0).
- _Float16/_Float128 host-gating soundness under -Wall.
- emit_generic_c determinism (pure function of the model).
- Cross-crate test placement (behavioral C tests in the runner crate).
- Scope creep vs the Non-Goals; citation drift vs HEAD.
Read: generic-c-backend.md §15/§16/§3.2/§10-11; Doc 2 D2/D4/D7/D8;
regseq-backends/src/lib.rs; codegen-coverage-runner/src/toolchain.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
SCOPE-DISCIPLINE (b vs c/d): PASS / FAIL with citation
SCOPE-DECISIONS: (1)/(2)/(3) AGREE / DISAGREE_<rationale>
SPEC-FIDELITY (§16/§10/§11): PASS / FAIL with citation
FLOAT16/128 GATING: PASS / FAIL with citation
DETERMINISM: PASS / FAIL
TEST-PLACEMENT: PASS / FAIL
SCOPE-CREEP: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-backends -p codegen-coverage-runner --all-targets --
  -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (existing + new structural + behavioral tests).
- The emitted prelude + helper block compiles under `cc -std=gnu11 -fwrapv
  -Wall` with no warnings; `regseq_panic` + saturating + `*WithOverflow`
  behave per §16/§10/§11.
- `emit_generic_c` byte-stable across calls; the `codegen-coverage-runner`
  ledger verdict stays `not-pass` and unchanged.
- `git diff --stat` confined to the Change-A–E file set + registration; no
  spec/doc/bridge/model/matrix changes.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Module structure**: `regseq-backends/src/generic_c/{mod.rs,prelude.rs}`;
   `emit_generic_c` / `EmitError` re-exported from `lib.rs` (harness import
   path unchanged).
2. **Five-bank driver**: `emit_generic_c` assembles banks 1–5 in §15 order;
   bank 1 is emitted; banks 2–5 are empty hooks with `// TODO(task c/d)`
   markers citing their §15 bank + Doc-1 rows.
3. **Static prelude** (§15 bank 1): includes, `struct rust_str`,
   `regseq_panic` (→ stderr `"panic: %s\n"` + `abort`), the `Unreachable`
   trap message, the 8 `AssertKind` message strings — all present, compiling
   under the §7 flags with no warnings.
4. **Helper block** (§10/§11): `static inline` `*WithOverflow`
   structs+wrappers (`__builtin_*_overflow`) for the integer set + saturating
   `FloatToInt` helpers for the (float × int) set; `_Float16`/`_Float128`
   helpers wrapped in source-level `#if` availability guards (emitted C
   byte-identical across hosts); warning-free.
5. **Behavioral correctness** (Change E / B2): via `toolchain::compile_and_run`
   — `regseq_panic` → stderr + non-zero exit; saturating cast
   (1e30→INT32_MAX, NaN→0, -1e30→INT32_MIN); `*WithOverflow` flag+value.
6. **Determinism**: `emit_generic_c` byte-identical across two calls.
7. **No coverage/scope creep**: no green fixture, no Doc-1 status flip, no
   bank 2–5 content, no per-`T` fat structs, no use-site lowering; ledger
   verdict unchanged (`not-pass`); diff confined to the Quality-Gate file set.
8. **Seam intact**: the bank-2–5 hooks return `Ok(String::new())`, so
   `emit_generic_c` returns `Ok(tu)` for **any** valid model — it **never**
   returns `Err(NotImplemented)` on the task-b happy path. The hooks keep a
   `Result` return type so c/d can later surface model-corruption errors (§15
   by-value cycle), but task b's hooks do not error.
