# regseq-driver-94: Classify lazily-interned core generic ADTs as `is_generic` (Phase-2b)

> Source: discovered while analyzing a failing RegseqError driver snippet that
> used `Result` at two monomorphizations. Root-caused + prototyped (applied →
> tested → reverted) and reviewed by Codex (`ENDORSE-WITH-CHANGES`). rustc-API
> semantics verified against the pinned `nightly-2026-02-01` source. Memory
> [[question-mark-unbridgeable]] flagged this exact gap ("flag core generic ADTs
> `is_generic`"). Unblocks realistic `Result<_, RegseqError>` driver code.

## Description

A driver that uses `core::result::Result` at **two different monomorphizations**
(e.g. a helper returning `Result<i32, RegseqError>` whose value is `?`-propagated
into a `regseq_main() -> Result<(), RegseqError>`) fails to bridge with:

```
BridgeError::Phase5("bridge-completeness validation failed:
  Bucket1MultipleInstances { item: ItemId(_), item_kind: Enum, count: 2 }")
```

**Root cause.** The bridge classifies every source `Item` into four buckets
(`crates/regseq-model/src/model.rs:3614-3742`). **Bucket 1** (non-generic,
sig-rigid, non-foreign) requires **exactly one** Instance per Item;
**Bucket 3** (`is_generic`) permits **0+** (`model.rs:3704-3707`). Source-defined
ADTs are interned by `eager_hir_enumeration`, which computes `is_generic`
correctly (`crates/regseq-driver/src/bridge/mod.rs:1263-1273`) — so a user's
`enum Foo<T>` lands in Bucket 3. But **core/external** ADTs (`Result`, `Option`,
…) are never in the source crate; they are interned **lazily** via
`translate_adt → intern_item → intern_source_item_handle`
(`crates/regseq-driver/src/bridge/ty.rs:491,606,624`), whose `PendingItem` push
**hardcodes `bridge_flags: regseq_model::BridgeItemFlags::default()`**
(`ty.rs:~702` ⇒ `is_generic: false`). So `Result<T,E>` is misfiled into Bucket 1
and a second monomorphization trips `Bucket1MultipleInstances`.

**Fix.** Compute `is_generic` on the lazy path the same way the eager path does,
via a **single shared helper** used by both paths (so the two intern sites cannot
drift in classification policy). `Result<T,E>` then lands in Bucket 3 and its
monomorphizations are permitted.

## Input Context

### Code anchors (confirmed at HEAD)

- **Bug site:** `crates/regseq-driver/src/bridge/ty.rs` — `intern_source_item_handle`
  (`:624`), its `PendingItem` push (`:~697-703`) hardcodes
  `bridge_flags: regseq_model::BridgeItemFlags::default()`. This is the only
  **behavior-changing** caller; the eager path (`mod.rs:1263-1273`) becomes a
  **refactor-only** caller of the same shared helper. (Reached for FnDef-typed
  items too via `ty.rs:~316` and
  the mono-item fn loop `mod.rs:~1030`, but source items recover their existing
  eager handle at `ty.rs:~631-633` and return **before** the push, so the change
  is a no-op for them; only items first-seen lazily — core/external ADTs + a few
  core fns — reach the push.)
- **Eager classification to factor out:** `mod.rs:1263-1273` — the exact
  `is_generic` formula incl. the trait-associated-fn carve-out:
  ```rust
  let is_generic = if matches!(tcx.def_kind(def_id), DefKind::AssocFn)
      && tcx.trait_of_assoc(def_id).is_some()
  { !tcx.generics_of(def_id).own_params.is_empty() }
  else { tcx.generics_of(def_id).count() > 0 };
  ```
- **Phase-4 flag propagation:** `crates/regseq-driver/src/bridge/assign.rs:39-43`
  — only **non-default** `BridgeItemFlags` are inserted into the sparse
  `item_bridge_flags`; the validator falls back to default (Bucket 1) on a miss.
  So `is_generic: true` (non-default) propagates and is read by the validator;
  non-generic core ADTs stay default ⇒ unchanged.
- **Validator buckets:** `model.rs:3704-3742` (Bucket 3 = 0+, Bucket 1 = exactly 1).
- **Instance creation is `is_generic`-agnostic:** `nominal_adt_instances_pass`
  (`mod.rs:2403-2489`) pushes one Instance per distinct `(item, args_key)` walking
  `model.types` — **no `is_generic` gate**. So both `Result` monos already get an
  Instance; the flip only relaxes the validator to permit them.
- **Per-mono name uniqueness:** `crates/regseq-backends/src/generic_c/naming.rs:155-166`
  — aggregate names append `generic_suffix = "__<hex args_key>"` (post-mono-unique,
  build-stable). `Result<i32,_>` and `Result<(),_>` get distinct C struct names.
- **The 3 other `is_generic` consumers are inert for ADTs:** `mod.rs:931`
  (orphan-cover; ADT skipped because it already has Instances), `mod.rs:1973`
  (function-instance construction; functions only), `mod.rs:2340`
  (`construct_nominal_adt_instances` eager-intern-of-missing-Ty; a *used* `Result`
  already has its `Ty`). The generic-C **backend reads neither `is_generic` nor
  `item_bridge_flags`** (verified: zero hits in `regseq-backends/src`) ⇒ emission
  is flag-agnostic.

### rustc-API semantics (verified vs pinned `nightly-2026-02-01`)

- `ty::Generics::count() == parent_count + own_params.len()`
  (`compiler/rustc_middle/src/ty/generics.rs:154`) — **includes inherited**
  generics; `own_params` is the item's own only (`:121`).
- `Result<T,E>`: `parent_count == 0`, `count() == 2` (its own `T,E`).
- `tcx.trait_of_assoc(def_id)` (`rustc_middle/src/ty/mod.rs:1894`) returns
  `Some(trait_id)` **only for items of a trait *declaration***, `None` for impl
  methods. ⇒ the reflexive `impl<T> From<T> for T`'s `from` (an **impl** method)
  is classified by `count()` (`>0` ⇒ generic), NOT the carve-out.
- Reflexive `impl<T> From<T> for T` is generic (`core/src/convert/mod.rs:782`).
  Under the current bug it is forced Bucket-1 (one covering prototype); under the
  fix it is correctly Bucket-3, and because the same-type `?` normalizer
  (regseq-driver-92) eliminated its only call (`from_residual → From::from`,
  `core/src/result.rs:2189`), it has **zero** monomorphizations ⇒ correctly not
  emitted. **This is the sole cause of the one golden delta below.**

## Implementation Notes

- **Extract a shared helper**, e.g.
  `fn source_bridge_item_flags(tcx: TyCtxt<'_>, def_id: DefId) -> BridgeItemFlags`,
  encapsulating the eager path's exact `is_generic` formula (incl. the
  `AssocFn && trait_of_assoc` → `own_params` carve-out) with
  `signature_non_rigid: false`. Call it from **both** `eager_hir_enumeration`
  (replacing the inline block at `mod.rs:1263-1273`) **and**
  `intern_source_item_handle`'s push (replacing `BridgeItemFlags::default()`).
  Do **not** re-derive a raw `count() > 0` in the lazy path: a lazily-interned
  core *trait-declaration* default method would then be misclassified (parent
  generics inflate `count()`); the carve-out is load-bearing for those, even
  though the current corpus doesn't exercise one.
- The eager path's behavior for source items must be **byte-for-byte unchanged**
  (the helper reproduces the same formula; this is a refactor + a new caller).
- **Out of scope to change:** the foreign-item flag path (`mod.rs:1311`, plain
  `count()`, no carve-out) — foreign Items are Bucket 2 regardless, so the flag is
  cosmetic there. Unifying it onto the helper is an optional consistency nit, not
  required; if done, it must not change any foreign Item's bucket.
- `signature_non_rigid` stays `false` here; the Bucket-4 non-rigid-signature flag
  for functions is still set on its own path (`mod.rs:~2039-2045`) and is
  unaffected.

## Non-Goals

- **No backend change** — generic-C already emits per-monomorphization core-ADT
  structs with unique names (`naming.rs:155-166`); confirmed by prototype.
- **No change to the eager path's classification *semantics*** — only factor it
  into a shared helper and add the lazy caller.
- **No new codegen-coverage matrix construct** — the multi-mono case exercises the
  existing RegseqError boundary (RE-1); it is a regression test, not a new row.
- No broadening to other `BridgeItemFlags` fields or other validator buckets.

## Test Plan

1. **Multi-mono bridges (regression for the bug).** A program using `Result` at two
   monomorphizations — `Result<i32, RegseqError>` (helper, `?`-propagated) +
   `Result<(), RegseqError>` (entry) — bridges to `Ok` (no
   `Bucket1MultipleInstances`), and the model's `Result` Item carries
   `is_generic == true` in `item_bridge_flags`. **The dual-mono source must be
   committed and permanent** — either an inline source string committed in a test
   (the `question_mark_e2e.rs` `TempSnippet` pattern, `tests/question_mark_e2e.rs:49-79`,
   counts: the snippet text lives in the committed file) or a committed
   `fixtures/` entry. Do **not** leave it as an ad-hoc/throwaway snippet with no
   committed home.
2. **Emit + cc + run.** The same dual-mono program emits **two distinct** `Result`
   structs (e.g. the unit/err niche form, size 1, and the `i32`/err tagged form,
   size 8), compiles with `cc`, and runs with the expected exit (Ok path ⇒ 0;
   Err path ⇒ the mapped errno). Assert no struct-name collision.
3. **Classification coverage (all mandatory).** Assert (via bridging + inspecting
   `item_bridge_flags`):
   - a lazily-interned `core::result::Result<T,E>` ⇒ `is_generic == true`;
   - a lazily-interned **non-generic core ADT** ⇒ `is_generic == false`
     (Bucket 1, unchanged). Use a concrete lazy core ADT (e.g. `core::fmt::Error`),
     **not** a source-defined ADT — only a lazily-interned one exercises the changed
     `ty.rs` push path; assert its absent/default flag resolves to `is_generic ==
     false`.
   - **the trait-assoc carve-out is preserved (not optional):** a **source**
     trait-declaration default method must stay Bucket 1 (`is_generic == false`)
     after the refactor. Add a focused regression for this, or extend a named
     existing trait-default-method test; do not rely solely on the suite staying
     green. (If raw `count()` were used instead of `own_params`, the trait's
     inherited `Self`/generics would inflate `count()` and misclassify it — this
     test guards exactly that.)
4. **Re-bless `regseq_error_timeout`.** Re-bless
   `crates/codegen-coverage-runner/fixtures/regseq_error_timeout/golden.c` (the
   dead reflexive `core::convert::...::from` prototype is dropped + two forward-
   decls reorder deterministically). Verify the re-blessed golden compiles + runs
   + matches `expected.exit = "ETIMEDOUT"`. Update the corpus snapshot test
   (`crates/codegen-coverage-runner/tests/corpus_e2e.rs`) accordingly.
5. **No other golden changes (I02).** Confirm `regseq_error_timeout/golden.c` is
   the **only** corpus golden that changes; every other golden is byte-identical.
6. **Gate + suite.** `cargo run -p codegen-coverage-runner` verdict `pass`, ledger
   **byte-stable** (run twice); full workspace test suite green.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-backends`,
`codegen-coverage-runner`) `-D warnings`; `RUST_TEST_THREADS=1 cargo test -p
regseq-driver -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`;
gate `pass`; ledger byte-stable.

## Acceptance Criteria

1. Core generic ADTs (`Result`, and generic ADTs reached lazily in general) are
   classified `is_generic == true` via a **shared helper** used by both the eager
   and lazy intern paths; used at **2+ monomorphizations** they bridge without
   `Bucket1MultipleInstances`.
2. The eager path's classification (incl. the trait-assoc carve-out) is preserved
   **verbatim**; no source Item changes bucket.
3. The dual-mono program bridges, emits **two distinct** `Result` structs (no name
   collision), `cc`-compiles, and runs with the expected exit.
4. `regseq_error_timeout/golden.c` is re-blessed (dead `From::from` prototype
   dropped) and is the **only** corpus golden that changes (all others byte-
   identical, **I02**); the re-blessed TU runs and matches `ETIMEDOUT`.
5. Gate `pass`, ledger byte-stable; full suite, fmt, and scoped clippy clean.
