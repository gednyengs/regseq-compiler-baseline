# regseq-driver-89: Bridge `framework_type` recognition + materialization (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (tasks **T03+T04**),
> contract **C05**, requirements **R06**, **R02/I01** (boundary preserved),
> invariant **I08**.

## Description

Teach the **active bridge** to recognize `#[rseq::framework_type("regseq-error")]`
on an enum item and record `AttrData::FrameworkType(FrameworkType::RegseqError)`
(from regseq-model-98) onto that enum's `ItemId`. Unlike diag-log (which is
**call-driven**, fired at the Call terminator), `RegseqError` is a *type*, so
recognition must be **item/type-driven** — a materialization pass that walks enum
items.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-driver/src/bridge/diag_log/recognition.rs` — the
  `recognise_log_intrinsic_attr` precedent to mirror: read the tool attr via
  `tcx.get_attrs_by_path(def_id, [Symbol("rseq"), Symbol("framework_type")])`,
  extract the dash-form string `"regseq-error"` → `FrameworkType::RegseqError`.
- `crates/regseq-driver/src/bridge/diag_log/mod.rs` — `materialise_log_intrinsics`:
  the pass shape to mirror, but walking enum items instead of call sites.
- `crates/regseq-driver/src/bridge/assign.rs` — `item_attrs` is sized one-Vec
  per item; the side table stores **`DefPathHash`, not `DefId`** — recover the
  `DefId` via `model.item_def_paths[item]` → `tcx.def_path_hash_to_def_id`
  (the existing ADT-resolution path; **skip synthetic / non-source items**).
- `crates/regseq-driver/src/bridge/mod.rs` — the existing ADT
  `def_path_hash_to_def_id` resolution (the model side of recovery).
- `crates/regseq-driver/src/bridge/ty.rs` — where enum items are interned
  (`ItemKind::Enum`); confirm enum items are populated in `model.items` at
  materialization time.
- Recognition must fire even when `RegseqError` appears **only as the entry's
  return type** — preserving the **R02/I01** boundary distinction (nothing about
  authored-internal use is marked).

## Implementation Notes

- Tool-attribute path is `[rseq, framework_type]` (fixtures enable it with
  `#![register_tool(rseq)]`, same mechanism as `#[rseq::intrinsic("…")]`).
- Recording is **idempotent**; an unmarked enum carries no `FrameworkType` attr.
- Split mirrors the precedent: a pure recognizer + a materialization pass.

## Non-Goals

- No model variant (regseq-model-98).
- No backend mapping/adapter/classifier (regseq-backends-*).
- No facade dependency (D06).

## Test Plan

1. Recognizer unit tests (mirror `recognition.rs`): a synthetic
   `#[rseq::framework_type("regseq-error")]` → `Some(RegseqError)`; absent / typo /
   wrong string / multiple → `None`.
2. Bridge integration: bridge a minimal fixture whose enum carries the attr →
   the attr lands on that enum's `ItemId` and **nowhere else**; an unmarked enum
   carries none; an enum used **only** as the entry return type is still
   recognized; a synthetic/non-source item is skipped (no panic, no mis-attr).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`cargo clippy -p regseq-driver --
-D warnings`); `RUST_TEST_THREADS=1 cargo test -p regseq-driver --
--test-threads=1`.

## Acceptance Criteria

1. A marked enum's `ItemId` carries exactly one `FrameworkType` attr; idempotent;
   unmarked enums carry none (**C05**, **R06**, **I08**).
2. `DefId` is recovered via `item_def_paths`/`def_path_hash_to_def_id`; synthetic
   items skipped.
3. Recognition is item/type-driven (fires for return-type-only use), preserving the
   authored-internal boundary (**R02/I01**).
4. fmt + scoped clippy clean; `regseq-driver` tests green.
