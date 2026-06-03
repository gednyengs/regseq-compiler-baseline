# regseq-driver-91: Bridge — certify `core::result::Result` identity (`SpecialAdt::Result`) (Phase-2a)

> Source: Codex review of the (archived) `regseq-backends-33`. The generic-C entry
> classifier must accept **exactly** `Result<(), RegseqError>` (GRILL **D08**,
> contract **C03**), but it can only do so if the model carries a
> **compiler-certified** `Result` identity — a rendered def-path *string* is a
> display value, not identity (`DefPath.rendered` ≠ `DefPathHash`). This task adds
> that certified signal; `regseq-backends-34` consumes it. Mirrors the existing
> `SpecialAdt` precedent. Memory [[assessment-soundness-pass]].

## Description

The bridge already classifies known ADTs into `SpecialAdt` and lowers nominal flags
onto the model `Item` (`is_phantom_data`, `is_unsafe_cell`, `is_manually_drop`, plus
a `CVoid` lang-item arm). There is **no `Result` classification** today. Add one:
recognize `core::result::Result` via its compiler diagnostic item and record a
nominal `is_result` flag on the model `Item`. This is a generic identity primitive
(not RegseqError-specific) — anything later needing to identify `Result` reuses it.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-driver/src/bridge/ty.rs:496` — `translate_adt`'s `special`
  computation: `is_phantom_data()` / `is_unsafe_cell()` / `ManuallyDrop`, and the
  `CVoid` arm at `:507` (`tcx.is_lang_item(adt.did(), LangItem::CVoid)`) — the exact
  shape to mirror for a diagnostic-item gate.
- `crates/regseq-driver/src/bridge/ty.rs:571` — the `SpecialAdt` enum (add a
  `Result` variant).
- `crates/regseq-driver/src/bridge/ty.rs:678` — where the special classification is
  lowered to the `Item`'s nominal flags (`is_phantom_data: matches!(special, …)`,
  etc.) — add `is_result: matches!(special, SpecialAdt::Result)`.
- The model `Item` struct (where `is_phantom_data`/`is_unsafe_cell`/
  `is_manually_drop` live, in `regseq-model`) — add the `is_result` field.
- **Verified diagnostic item:** `core::result::Result` carries
  `#[rustc_diagnostic_item = "Result"]` (pinned nightly,
  `library/core/src/result.rs:555`), so `tcx.get_diagnostic_item(sym::Result)`
  returns its `DefId` — the certified gate (implementer pins the exact `Symbol`).

## Implementation Notes

- Add `SpecialAdt::Result`, gated on
  `tcx.get_diagnostic_item(sym::Result) == Some(adt.did())`, placed consistently
  with the `CVoid` arm. Lower it to a new `Item.is_result` nominal flag exactly as
  the sibling flags at `ty.rs:678`.
- The `is_result` model field is **append-only** and defaults `false`; it must not
  reorder or otherwise perturb existing `Item`/model JSON serialization (mirror how
  `is_manually_drop` is declared) — determinism bar [[codegen-ledger-determinism]].

## Non-Goals

- No change to the entry classifier (that is `regseq-backends-34`).
- No other diagnostic-item recognition; no `?`/`Try` work.

## Test Plan

1. Bridge/model test: a fixture/model whose function returns `Result<_, _>` → the
   `Result` enum `Item` carries `is_result == true`; a custom two-variant enum (e.g.
   `enum MyResult { Good, Bad }`) → `is_result == false`.
2. Serde/determinism: the new field round-trips; existing model/`domain_attrs` JSON
   shape is unchanged (no reorder); existing bridge tests + goldens unaffected (I02).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-model`);
`RUST_TEST_THREADS=1 cargo test -p regseq-driver -p regseq-model -- --test-threads=1`;
gate `pass`; ledger byte-stable.

## Acceptance Criteria

1. `Item.is_result == true` **iff** the ADT is `core::result::Result` (certified via
   the diagnostic item), mirroring the `SpecialAdt` / nominal-flag precedent.
2. Non-`Result` enums carry `is_result == false`; the field round-trips
   deterministically (append-only, no serialization reorder).
3. Existing goldens/tests unaffected (**I02**); fmt + scoped clippy clean; gate
   `pass`.
