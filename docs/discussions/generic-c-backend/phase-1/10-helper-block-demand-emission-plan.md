# Plan 10 — demand-driven emission of the arithmetic/cast helper block

> **Status:** plan-first (Codex-reviewed before implementation).
> **Scope:** one generic-C backend output-hygiene change. No surface
> expansion, no behavioral change to any compiled program. Re-blesses the
> golden corpus.

## Problem

Every emitted `generic.c` carries the **full** arithmetic/cast helper library,
regardless of whether the program uses any of it. `helper_block()`
(`crates/regseq-backends/src/generic_c/prelude.rs:124`) walks the fixed
`INT_TYPES` (12) and `FLOAT_TYPES` (4) tables and unconditionally emits:

- **Overflow family** — 12 × `struct regseq_overflow_<T>` + 36 ×
  `regseq_{add,sub,mul}_<T>` wrappers (`prelude.rs:143-156`).
- **Saturating-cast family** — 4 × 12 = 48 × `regseq_<f>_to_<T>`
  (`prelude.rs:161-179`).

~96 definitions in every translation unit. They are `static inline`, so the C
compiler dead-strips them and `-Wall` stays silent — harmless to the *binary*,
but they clobber the *source*. A driver that does zero overflow arithmetic and
zero float→int casts still ships all ~96.

## Why this is not the `-C overflow-checks` flag

These helper *definitions* are orthogonal to the overflow-checks flag. The
`*WithOverflow` wrappers are what **explicit** `a.overflowing_add(b)` /
`a.checked_add(b)` lower to (program semantics), not just compiler-inserted
overflow asserts. Gating the block on `overflow_checks` would be wrong: a build
with checks off still needs `regseq_add_i32` when the program calls
`checked_add`. The only sound fix is **demand-driven emission keyed on actual
use** — correct whether the use came from a compiler check or an explicit op.

## Root cause

The block is a **fixed superset by design** — the comment at `prelude.rs:112`
states "A fixed superset over the primitive type set (scope decision 2): the
helpers are *defined* here, *used* at task-d `BinaryOp` / `Cast` sites. All
helpers are `static inline`, so the unused ones are warning-free under
`-Wall`." That decision optimized for emit-side simplicity (no use tracking) and
leaned on the C compiler to drop the dead defs. The cost is source clobber.

The emission unit is clean to filter:

- `struct regseq_overflow_<T>` is referenced **only** as the temporary that
  catches a wrapper's return (`body_emit.rs:766`); the `(T,bool)` destination is
  a separate type-bank struct, and a nested `*WithOverflow` rvalue is rejected
  rather than lowered (`body_emit.rs:1746`), so there is no second reference. The
  struct and that type's wrappers are co-used — drop the wrappers and nothing
  references the struct. (Confirmed in Codex review.)
- Each saturating cast `regseq_<prefix>_to_<T>` is referenced only at the
  `CastKind::FloatToInt` site (`body_emit.rs:1812`).

## Fix

Emit only the helpers the bodies actually reference. Output **order** is
unchanged (`PRELUDE` → helper block → banks 2–5; C requires the `static inline`
defs ahead of their bank-5 call sites). Only the **computation order** changes:
build banks 2–5 first, determine the referenced helpers, then build the filtered
helper block, then concatenate in the original order.

Granularity: **fine** (per the scope decision) — emit exactly the `(op,T)`
wrappers and `(prefix,T)` casts used, the `struct regseq_overflow_<T>` for any
`T` with ≥1 used wrapper, and the section comments only for non-empty sections.

### Mechanism — collector recorded at the emit sites

Record helper use at the two emit sites as they fire, then build the filtered
block from the recorded set. Chosen over scanning the emitted C text because a
text scan can false-positive on a **model-supplied external symbol** whose name
coincides with a helper token (external symbols are emitted as-is, never
`_clash_`-renamed — `mod.rs:508,513`), and the collector keys on the body's IR
types instead, so it cannot mis-attribute.

`HelperUsage` carries two `BTreeSet<String>`s: the wrapper function names
(`regseq_add_int32_t`, …) and the cast function names (`regseq_f64_to_int8_t`, …)
emitted so far. Each entry is the **exact generated spelling** the call site
builds, so emission and inclusion key on the identical string — lockstep, no
divergence.

- `BodyEmit` (`body_emit.rs:208`, currently all-immutable borrows) gains a
  `helper_use: &'a RefCell<HelperUsage>` field. `assign_with_overflow`
  (`body_emit.rs:740`) records `regseq_{helper}_{ct}`; the `FloatToInt` arm
  (`body_emit.rs:1796`) records `regseq_{prefix}_to_{dest}`. Both record the same
  string they emit into the call.
- `emit_body` (`body_emit.rs:76`) takes `&RefCell<HelperUsage>`; only
  `bank5_definitions` calls it, so the thread is `emit_generic_c → bank5_definitions
  → emit_body` — three signatures. Banks 2–4 do not emit helper calls (see scope
  invariant below), so they need no collector.

Assembly reorders the **computation** (output order unchanged):

```
let bank5 = bank5_definitions(..., &usage)?;   // fills usage as bodies emit
out.push_str(PRELUDE);
out.push_str(&prelude::helper_block(&usage.borrow()));  // filtered
out.push_str(&bank2); out.push_str(&bank3); out.push_str(&bank4); out.push_str(&bank5);
```

`helper_block(&usage)` iterates the **fixed `INT_TYPES`/`FLOAT_TYPES` tables**
(not the recorded set) and emits a wrapper row iff its name is in
`usage.wrappers`, a `struct regseq_overflow_<T>` iff any of `add/sub/mul` for `T`
is in `usage.wrappers`, and a cast row iff its name is in `usage.casts` — so
output stays byte-stable.

**Scope invariant (only bank-5 bodies emit helper calls).** The only sites that
emit a `regseq_*` helper call are `body_emit.rs:766` (`*WithOverflow` temp) and
`body_emit.rs:1812` (`FloatToInt`). A nested `*WithOverflow` rvalue is rejected,
not lowered, so it adds no second reference (`body_emit.rs:1746`). Static / const
initializers cannot introduce one: bank-5 static emission delegates to
`static_initializer` → `const_expr` (`mod.rs:1755`, `body_emit.rs:2649,2723`)
over **folded** `Const` values whose vocabulary holds no `Cast` / `BinaryOp`
rvalue (`regseq-model/src/const_value.rs:20`). Hence collecting in bank-5 bodies
alone is complete. (Confirmed in Codex review.)

## Why this is determinism-safe

- The filtered block walks the fixed `INT_TYPES`/`FLOAT_TYPES` tables in their
  existing order and includes a row iff its name is in the recorded set →
  byte-stable, host-independent (same discipline as
  `golden-c-emit-process-determinism`).
- The recorded set is a `BTreeSet` filled by a deterministic body walk; emission
  order derives from the fixed tables, not from insertion order.
- Re-running the producer twice must yield byte-identical output (verified in CI
  by the existing `emit_is_byte_identical_across_two_calls` test and a fresh
  twice-run of the coverage producer).

## Collision-safety invariant (must hold)

`prelude_ordinary_idents()` (`mod.rs:558`) seeds the symbol-collision reservation
(`mod.rs:489`) with the **full** helper-name set. It **stays full** — emission
shrinks, reservation does not. Rationale: a derived instance symbol must never
take a name like `regseq_add_int32_t`, because a later body in the same TU could
force that helper to be emitted on demand; a shrunk reservation would open a
collision window. This is the load-bearing invariant for the change.

## Test migration

Three surfaces assume the full superset is always present:

1. **`prelude_ordinary_idents_all_defined_in_prelude`**
   (`mod_tests.rs:3108`) builds `PRELUDE + helper_block()` and asserts every
   reserved ident is defined. `helper_block` now takes `&HelperUsage`; feed it
   `HelperUsage::all()` (a constructor that inserts every wrapper/cast name from
   the fixed tables). The pin now reads "every reserved name is *emittable*" —
   preserving its purpose (reservation ⊇ emittable set).

2. **`tu_starts_with_the_includes`** (`mod_tests.rs:44`) uses
   `regseq_add_int8_t` as the ordering marker on `trivial_model()`. That helper
   no longer exists for a no-arithmetic model. Re-anchor the ordering assertion
   to an always-present token (`regseq_panic`, emitted by the static `PRELUDE`
   after the includes), which is the test's actual intent (includes lead).

3. **`generic_c_prelude.rs`** standalone tests build
   `emit_generic_c(empty) + exerciser` calling specific helpers. These verify
   the helper *library's* correctness (warning-free compile; saturating cast
   saturates; `*WithOverflow` reports flag+value) — a concern independent of
   demand-filtering. Repoint their TU to the **full** helper block
   (`PRELUDE + helper_block(all)`), not `emit_generic_c(empty)`.
   `prelude_compiles_with_no_warnings` then keeps proving the full library is
   warning-free standalone.

Tests that **stay green, no migration** (their models actually emit the helper):
`mod_tests.rs:2491` (add-u32 body), `mod_tests.rs:2425` (saturating cast), and
the body-runner helper-token assertions at `generic_c_bodies.rs:524,756`
(Codex-confirmed: their models use those helpers).

New coverage to add:

- **Unit (mod_tests):** `emit_generic_c(trivial_model())` contains **no**
  `regseq_add_`/`regseq_sub_`/`regseq_mul_`/`regseq_*_to_*` and no
  `struct regseq_overflow_` — the de-clobber proof.
- **Unit (mod_tests):** a model that uses exactly one op (e.g. `checked_add` on
  `u32`) emits `regseq_add_uint32_t` + `struct regseq_overflow_uint32_t` and
  **not** `regseq_sub_uint32_t` / other-`T` wrappers — the fine-grained proof.

## Verification

1. `cargo test -p regseq-backends` (mod_tests) — migrated + new unit tests green.
2. `cargo test -p codegen-coverage-runner --test generic_c_prelude` — standalone
   helper-library tests green against the full block.
3. **Re-bless the corpus:** every existing golden currently carries the full
   block, so this shrinks all of them. `cargo run -p codegen-coverage-runner
   --bless`, then `cargo run -p codegen-coverage-runner` → verdict `pass`, all
   fixtures green, drift 0. Inspect a panic/arith golden (`b6_overflow_add`,
   `b2_floats_casts`) to confirm the *used* helpers survive, and a no-arithmetic
   golden (e.g. `diag_*`) to confirm the block shrank.
4. **Determinism:** run the producer twice, `diff` the JSON ledger + a golden →
   byte-identical.
5. Gate clippy (lib targets): `cargo clippy --workspace -- -D warnings`.

## Out of scope

- The static `PRELUDE` (includes, `rust_str`, panic infra, `REGSEQ_MSG_*`
  `#define`s): tiny and `#define`s never warn. Left unconditional.
- The `REGSEQ_INT128_*` macros (`prelude.rs:135-137`): three `#define`s, harmless
  when unused. Left unconditional (a later cleanup could gate them on "any
  128-bit helper emitted"; not worth the branch now).
