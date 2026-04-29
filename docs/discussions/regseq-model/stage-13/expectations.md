# Expectations on stage 13 (incoming)

Stage 13 covers domain-specific extensions: intrinsics, ordered
regions, register access, ISR classification, error-cleanup
analysis, callback context inference.

The convention is described in `../memory.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Pin the intrinsic vocabulary (Q7).** Per Q7, the bridge
  identifies domain intrinsics by the `NAME` argument of
  `#[rseq::intrinsic("NAME")]` on the callee, not by the
  callee's path. Names form a closed vocabulary maintained in
  the bridge; unknown names are rejected. Stage 13 ships the
  closed vocabulary list and the bridge-side recognition.

- **Single-level IR (Q3).** Domain extensions live as additional
  variants or tags on existing IR variants -- not as a separate
  IR layer. For example, an ISR subkind of `Function` can be
  added to `InstanceKind::Function` (e.g., as an `is_isr: bool`
  field, or as a new `InstanceKind::Isr` variant). Stage 1's
  closed six-variant `InstanceKind` is the closed baseline; any
  stage-13 extension appends to it (with a corresponding
  append-only entry in `regseq_instance_kind_rank`).

- **Promote items into the `regseq` facade (Q6).** The `regseq`
  crate (`crates/regseq/`) is currently empty per Q6's lazy
  promotion plan. Stage 13 promotes items from each spec's
  `crate::regseq` submodule into the facade incrementally as
  redesign work touches them. The crate's symmetric
  `register_tool` declarations and `[package.metadata.regseq]`
  marker are already in place.

- **Fill `pretty.rs::render_instance` stub.** Stage-1 stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 13 fills render_instance")`. Stage 13
  replaces the body with a renderer producing
  `Item::<args>` per the spec sigil for `InstanceId`.

- **Domain-specific IDs (e.g., `RegisterId`, `IsrId`).** Per the
  spec's "IDs added at later stages" note, stage 13 may
  introduce additional typed IDs. They follow the same
  conventions (`newtype_index!` macro, niche-encoded u32, hand-
  written serde, single namespace per kind).

## Bridge concerns

- **Domain attribute parsing.** `#[rseq::*]` attributes become
  structured `DomainAttr` on `Item` / `Instance` per the
  C-semantics methodology rule (don't pass annotations through
  as opaque source-side metadata; structure them per the
  consumer's needs).

- **Recognition surface.** Bridge code identifies intrinsics by
  attribute-name lookup against the closed vocabulary. Mirror
  the "no defensive fallbacks" rule: unknown names are
  rejected.
