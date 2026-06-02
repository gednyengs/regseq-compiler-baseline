# regseq-model-98: Model `AttrData::FrameworkType` (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (task **T02**),
> contract **C05**, invariant **I08**. RegseqError generic-C Phase 2.

## Description

Add the model-side carrier for the RegseqError framework type: a new
**append-only** variant `AttrData::FrameworkType(FrameworkType)` plus a closed
`FrameworkType { RegseqError }` enum. This is the only model change in the whole
feature; the bridge (regseq-driver-89) records it and the backend
(regseq-backends-27/28) reads it.

The active model does **not** reference `framework_type`/`FrameworkType` today
(verified: no hits in `crates/regseq-model/src/`). The four historical
"framework type" tasks (`regseq-driver-26/28`, `regseq-facade-20/02`) are the
**old extraction pipeline** (`phase=None`), not the active bridge — do not reuse.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-model/src/domain_attr.rs` — `AttrData` is `#[repr(u8)]` with the
  single variant `Intrinsic(IntrinsicKind) = 0`. **Append-only**: the new variant
  takes discriminant `1`; `Intrinsic` must stay `0` (serialized-discriminant
  stability). There is an existing discriminant-stability test
  (`attr_data_intrinsic_discriminant_is_stable`-style) to mirror.
- `crates/regseq-model/src/layout.rs` — `VariantLayout { name: StringId, … }`. Not
  edited here, but it is what the backend later keys the errno table on (by name),
  so variant **order is not** load-bearing.
- Determinism: mirror `regseq-model-96` (deterministic `domain_attrs` JSON
  serialization) — the new variant must serialize deterministically
  (see memory [[codegen-ledger-determinism]]).

## Implementation Notes

- `FrameworkType` is a small closed enum with `RegseqError` (leave room to append
  future framework types). Derive the same traits `IntrinsicKind` carries
  (serde/Debug/Clone/Copy/PartialEq/Eq as applicable).
- Pure data addition — **no** recognition, bridge, or backend logic here.

## Non-Goals

- No bridge recognition or materialization (regseq-driver-89).
- No backend mapping/adapter (regseq-backends-26/27/28).
- No facade-crate change (D06: identity is the attribute, not a crate).

## Test Plan

1. Construct `AttrData::FrameworkType(FrameworkType::RegseqError)`; assert
   round-trips through serde unchanged.
2. Tag-byte / discriminant-stability test: the **`AttrData::FrameworkType`** variant
   serializes at discriminant `1` and **`AttrData::Intrinsic`** remains `0`;
   separately assert the inner `FrameworkType::RegseqError` is a stable value (`0`).
3. Deterministic serialization (no map-ordering nondeterminism), mirroring
   `regseq-model-96`.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`cargo clippy -p regseq-model --
-D warnings`); `cargo test -p regseq-model`.

## Acceptance Criteria

1. `AttrData` gains exactly one new variant at discriminant `1`; `Intrinsic` stays
   `0` (**I08**, **C05**).
2. `FrameworkType::RegseqError` exists; serde round-trips; serialization is
   deterministic.
3. fmt + scoped clippy clean; `regseq-model` tests green.
