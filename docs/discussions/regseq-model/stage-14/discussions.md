# Stage 14 -- Discussion Record

Distilled record of the design dialogue producing the
stage-14 phase-1 foundation extensions spec. Each entry
captures the question raised, options considered, the
decision, the reasoning chain, methodology rules invoked,
prior-art evidence, and cross-references to
`spec-snapshot.md` (the structural-spec working copy) and
`pretty-printer-snapshot.md` (the rendering-spec working
copy per stage-13 D-13-9 spec split). Round-by-round
Codex audit trail lives in `review-log.md` (created on
first review round).

Status of decisions in this file mirrors the planned
in-place edits + new sections in `spec-snapshot.md`
(structural spec: `IntrinsicKind` enum, `AttrData` enum,
`DomainAttrTables` struct, new §3.6 Domain attribute
tables, §3.4 Phase-4 side-table list extended,
§3.5.2 14th pull-through helper, §8.0 in-place
correction per external-review Finding #1) and
`pretty-printer-snapshot.md` (renderer specs --
deferred to a later cluster within stage 14;
external-review Findings #2 + #4 land there at merger
close).

D-tag convention: prefixed `D-14-K` form per
`../README.md` §5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; ready for Codex
  review.
- *(no status tag)* -- cluster main entry with one or
  more sub-decisions locked but not all 12 closed yet;
  Phase A drafting in progress. The cluster's main entry
  picks up `[LOCKED]` once every sub-decision under it
  closes.
- `[CLOSED-BY-ABSORPTION]` -- subsumed into a later
  D-tag during work; retained for history.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.

This file covers the stage-14 clusters in close order
(all closed clusters listed; surface-state current as
of 2026-05-13):

- **D-14-1 + D-14-3 cluster** [LOCKED Phase B 2026-05-12
  at R3 MINOR / Lockability:yes / Close-out:YES]
  (Q7 intrinsic recognition mechanism + storage;
  DomainAttr sibling-table architecture).
- **D-14-2 cluster** [LOCKED Phase B 2026-05-13 at R6
  APPROVE / Lockability:yes / Close-out:YES]
  (diagnostic-logging vocabulary + bridge contract;
  9 sub-decisions D-14-2.1 through D-14-2.9).
- **D-14-10 cluster** [LOCKED 2026-05-13 at R3 MINOR;
  mid-cluster carve-out]
  (Generic-C backend codegen carve-out --
  `generic-c-backend.md` companion file; mid-cluster
  surfacing between D-14-2 and D-14-4).
- **D-14-4 cluster** [LOCKED Phase B 2026-05-13 at R4
  APPROVE / Lockability:yes / Close-out:YES]
  (`render_const` composer renderer per-arm specs;
  12 sub-decisions D-14-4.1 through D-14-4.12;
  REOPENS D-13-6 per D-14-4.9 for `render_operand`
  signature extension).
- **D-14-5 cluster** [LOCKED Phase B 2026-05-13 at R4
  APPROVE / Lockability:yes / Close-out:YES]
  (`render_instance` composer renderer per-variant
  specs; 8 sub-decisions D-14-5.1 through D-14-5.8 +
  19 specimens at §9.11).
- **D-14-6 cluster** [LOCKED Phase B 2026-05-13 at R6
  APPROVE / Lockability:yes / Close-out:YES]
  (Top-level `render(&self) -> String` composer;
  6 sub-decisions D-14-6.1 through D-14-6.6 + 5
  specimens at §9.12).
- **D-14-7 cluster** [LOCKED Phase B 2026-05-13 at R6
  APPROVE / Lockability:yes / Close-out:YES]
  (`#[naked]` rejection in v1 per external-review
  Finding #2; 9 sub-decisions D-14-7.1 through
  D-14-7.9; REOPENS D-12-1.3 + D-9-9 + D-13-4.3 at
  merger close).
- **D-14-8 cluster** [LOCKED Phase B 2026-05-13 at R2
  APPROVE / Lockability:yes / Close-out:YES]
  (stale `emission_name` forward-pointer retarget at
  `spec-snapshot.md` §9.2 paragraph 3 per external-
  review Finding #3; pure spec hygiene; no
  sub-decisions; no D-tag flip; 2-round Phase B
  trajectory R1 MINOR -> R2 APPROVE).
- **D-14-9 cluster** [LOCKED Phase B 2026-05-13 at R2
  APPROVE / Lockability:yes / Close-out:YES]
  (Specimen 70 split per external-review Finding #4
  at `pretty-printer-snapshot.md` §9.9 around
  Specimen 70; pure pretty-printer-spec hygiene; no
  sub-decisions; no D-tag flip on D-13-7; 2-round
  Phase B trajectory R1 MINOR -> R2 APPROVE; split
  drops impossible `opaque_handle_eligible: true`
  state from the Function specimen and adds a new
  Specimen 70b carrying the bridge-outcome on a
  non-generic ZST `repr(C)` Struct per `spec-snapshot.md`
  §8.0 invariant 19).
- **`core::ffi::c_void` rejection enforcement
  cluster** [LOCKED Phase B 2026-05-13 at R3 APPROVE
  / Lockability:yes / Close-out:YES]
  (no new D-tag; stage-3 D16 + D1.4 already
  specify; stage-14 adds the §15.1 enforcement
  narrative paragraph plus an inline definition
  of the opaque-struct pattern. Pure spec hygiene;
  no D-tag flip; 3-round Phase B trajectory R1
  MINOR -> R2 MINOR -> R3 APPROVE).

**Remaining stage-14 work** (open for the next
drafting session):

- Per-stage cross-cluster Codex review on the full
  `spec-snapshot.md` + `pretty-printer-snapshot.md`
  (anticipated 2-4 rounds), then hand-off to
  merger per `../README.md` §5.5 7-step contract.

Two mid-cluster substantive threads surfaced during the
D-14-1 / D-14-3 discussion:

1. The **operational-vs-declarative attribute
   architecture question** -- prompted by considering
   whether the operational-intrinsic surface (Q7-recognized
   `#[rseq::intrinsic("NAME")]` callees) and the
   declarative-property surface (`#[rseq::mmio(...)]`,
   `#[rseq::interrupt(...)]`, ...) are one architecture
   or two parallel architectures. Resolved by the rustc
   evidence pass showing that the recognition mechanism
   is naturally unified (one
   `tcx.get_attrs_by_path` codepath handles both); only
   the storage shape was the real question. Folded into
   D-14-3.0 as the cross-cutting decision.

2. The **Instance attribute access mechanism question**
   -- prompted by the user's observation that "Instances
   access their attributes through their Item reference."
   Resolved by reading stage-12 D-12-1.8 (§3.5.2
   Instance-side pull-through helpers), which locks the
   `self.items[self.instances[id].item].<field>` access
   pattern for Item-level metadata. The implication for
   `DomainAttrTables` is that `instance_attrs` is
   structurally redundant -- Item-keyed storage plus the
   pull-through helper covers all Instance-level access.
   Folded into D-14-3.1 as the key-kind set decision.

---

## 1. Decision discussions

### D-14-1 -- Q7 intrinsic recognition mechanism + storage [LOCKED]

**Question.** Q7 (the binding architectural decision
locked in stage 0) commits the project to recognizing
domain intrinsics by the `NAME` argument of
`#[rseq::intrinsic("NAME")]` on the callee, with the
vocabulary forming a closed set maintained in the
bridge. The §8.0 forward-pointer paragraph in
`spec.md` (the closing paragraph of §8.0 before §8.1
"Lifting `impl` items") sketched the recognition
mechanism + storage as a forward-pointer; external-review
Finding #1 (2026-05-07) identified two errors in that
sketch:
- (a) `tcx.intrinsic_raw(did)` is named as the
  recognition mechanism, but `intrinsic_raw` is
  rustc-internal-only (matches `#[rustc_intrinsic]`,
  not tool-namespace custom attributes).
- (b) The paragraph says the resolved intrinsic identity
  is "not stored on Item." Per Q2's eager-translation
  rule, `'tcx` does not survive past the bridge call;
  downstream codegen cannot re-query rustc, so the
  resolved intrinsic identity MUST live on
  `RegseqModel`.

Stage 14 owns the correction, the recognition codepath
selection, the resolved-intrinsic storage schema, and
the closed-vocabulary mechanism (with diagnostic-logging
entries only at stage 14; per-domain-family vocabulary
entries -- MMIO, atomic, interrupt, clock, ... -- relocate
to phase-2 stages 15-18+).

**Options considered.**

Aspect A1 (recognition codepath):
- (a) `tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`
  -- multi-segment attribute walker.
- (b) `tcx.get_attrs(did, sym::foo)` -- single-segment
  walker. Wrong shape for two-segment tool paths.
- (c) `find_attr!` macro on `tcx.get_all_attrs(did)`.
  Only matches `Attribute::Parsed` arms; skips
  `Attribute::Unparsed` (the arm tool attributes always
  land in). Unusable.
- (d) Manual `tcx.get_all_attrs(did).iter()` walk +
  `Attribute::path_matches`. Reimplements (a) inline.
- (e) `rustc_resolve` tool-namespace machinery direct
  consumption. Not query-consumer-facing; the bridge
  consumes the *result* of resolution via (a) after the
  attribute has been stamped into HIR.
- (f) `tcx.intrinsic_raw(did)` -- the external-review
  Finding #1 verification target.

Aspect A2 (`register_tool(rseq)` validation):
- (a) Bridge explicitly verifies `tcx.registered_tools(())`
  contains `sym::rseq` before walking attributes.
  Defensive.
- (b) Bridge trusts rustc's resolution: any
  `#[rseq::intrinsic(...)]` attribute that survives to
  MIR has already passed rustc's tool-prefix lookup; the
  bridge does not re-verify.

Aspect A3 (NAME extraction chain):
- (a) `attr.meta_item_list()` ->
  single-element `Option<ThinVec<MetaItemInner>>` ->
  `MetaItemInner::lit()` ->
  `LitKind::Str(Symbol, _)`. The standard `rustc_hir`
  + `rustc_ast` chain.
- (b) Direct token-stream parsing on `AttrArgs::Delimited`'s
  `DelimArgs`. More work than (a); reimplements
  `MetaItemKind::list_from_tokens`.

Aspect A4 (malformed-payload validation):
- (a) Bridge owns it. Four error shapes (no args, eq-style,
  non-string lit, multi-arg) converge on a single check:
  "exactly one inner item, and that item is `Lit` with
  `LitKind::Str`."
- (b) Trust rustc's pre-validation. Doesn't work: rustc
  has no schema for custom tool-attribute payloads (the
  attribute is `Unparsed` precisely because of this).

Aspect A5 (`IntrinsicKind` enum shape):
- (a) Flat variants per NAME (`Log_Debug`, `Log_Info`,
  ..., `Mmio_Read`, `Mmio_Write`, ...,
  `Atomic_Load_Relaxed`, ...). Simple at stage 14 but
  unwieldy as vocabulary grows.
- (b) Category + sub-discriminant per family
  (`Log(LogLevel)`, `Mmio(MmioOp)`, `Atomic(AtomicOp)`,
  ...). Mirrors domain-family taxonomy; each phase-2
  stage owns its sub-enum.
- (c) Single variant carrying a `(family, op)` tuple of
  enums. Hybrid of (a) and (b).

Aspect A6 (`IntrinsicInfo` wrapper):
- (a) Wrapper struct `IntrinsicInfo { kind, name,
  source_item }`. Original Finding #1 proposal.
- (b) Wrapper struct `IntrinsicInfo { kind }`. Drops
  redundant fields (source_item is the IndexVec key;
  name is recoverable from kind).
- (c) No wrapper -- store `IntrinsicKind` directly.

Aspect A7 (population timing + spec home):
- (a) Two-phase: Phase 2 recognition into a working
  cache keyed by `DefPathHash` + Phase 4 materialization
  into `domain_attrs.item_attrs[id]` (mirrors §3.4's
  `vtables` / `ConstMemoryStatic` / closure-captures
  pattern; Phase-2-only push would conflict with D-12-5.3
  post-sort ItemId assignment). New subsection §3.6
  under §3 (RegseqModel root).
- (b) Phase-2-only with push into
  `model.domain_attrs.item_attrs[id]` during Item
  discovery. Conflicts with D-12-5.3: final `ItemId`s
  are not assigned until Phase 4. Would require an
  unstated remap or placeholder ItemId protocol.
- (c) Phase 3 (F4 Sort) -- recognition there is
  source-data extraction, not ordering work; doesn't
  fit the phase's role.
- (d) Phase 4 only -- forces recognition to walk
  rustc state after F4 sort has already committed the
  Item set; awkward and out of step with §3.4's
  Phase-2-recognition pattern.
- (e) Fold spec home into §8 (Item) rather than §3.
  §3 already hosts `RegseqModel` field schemas;
  intrinsic storage
  is a `RegseqModel` field.

**Decision.** **A1 (a) + A2 (b) + A3 (a) + A4 (a) +
A5 (b) + A6 (c) + A7 (a)**:
- Recognition via
  `tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`.
- No bridge-side `register_tool` re-validation
  (rustc enforces).
- NAME extraction via the `meta_item_list -> .lit() ->
  LitKind::Str` chain.
- Bridge owns malformed-payload validation (single check
  on the inner shape).
- `IntrinsicKind` is a category + sub-discriminant
  closed enum.
- No wrapper struct; store via `AttrData::Intrinsic(kind)`
  in `domain_attrs.item_attrs[id]` (per D-14-3 sibling-
  table architecture; the redundant fields drop out
  naturally).
- Population is **two-phase**: Phase 2 recognition into a
  Phase-2 working cache keyed by `DefPathHash` + Phase 4
  materialization into `domain_attrs.item_attrs[id]`
  (mirroring §3.4's existing `vtables` /
  `ConstMemoryStatic` / closure-captures pattern). Spec
  home new §3.6 "Domain attribute tables" subsection.

Each aspect locks as its own sub-decision below.

**Reasoning.**
- A1 (a) is the only viable codepath after the evidence
  pass: (b) is structurally wrong (single-segment);
  (c) is unusable (skips `Unparsed`); (d) reimplements
  (a); (e) is not query-consumer-facing; (f) is the
  Finding #1 confirmation target (verified
  rustc-internal-only).
- A2 (b) follows from the rustc-resolution evidence:
  unregistered tools fail at `ToolPrelude` lookup
  (`rustc_resolve/src/ident.rs:691-694`); anything
  reaching MIR has already passed. Defensive re-check
  conflicts with README §2 "No defensive fallbacks."
- A3 (a) is the rustc-conventional path; (b) reimplements
  the same logic at a lower abstraction layer with no
  upside.
- A4 (a) is forced: rustc does NOT pre-validate
  tool-attribute payloads (`Attribute::Unparsed` exists
  precisely because there is no schema). The single
  check "exactly one inner item, `Lit` with
  `LitKind::Str`" covers all four malformed shapes.
- A5 (b) wins on three counts: (i) mirrors the domain-
  family taxonomy in the IR; (ii) reads naturally
  ("dispatch on `IntrinsicKind::Log(_)` for any logging
  operation"); (iii) each phase-2 stage owns its
  sub-enum (`MmioOp`, `InterruptOp`, ...) parallel to
  how it owns its sibling-table contributions. Cost is
  minimal at stage 14 (one variant + one 4-entry sub-enum).
- A6 (c) is forced once D-14-3 lands: with the unified
  `DomainAttrTables` design, `AttrData::Intrinsic(kind)`
  IS the per-Item record. A wrapper struct would
  introduce an extra type for no payload (the redundant
  `name` and `source_item` fields drop naturally).
- A7 (a) (two-phase: Phase-2 recognition into a working
  cache + Phase-4 materialization into the final table)
  is the only option compatible with D-12-5.3
  post-sort ItemId assignment. Recognition consumes
  rustc state, naturally Phase-2 work; final-table
  population is keyed by final `ItemId`s, which Phase 4
  is the first phase to provide. §3.4's existing
  `vtables` / `ConstMemoryStatic` / closure-captures
  pattern documents the same split; stage-14 inherits
  verbatim. Phase-2-only push (the rejected option (b))
  would conflict with D-12-5.3. §3 hosts `RegseqModel`'s
  schema (already the home for `items`, `instances`,
  `vtables`, etc.); the new `domain_attrs:
  DomainAttrTables` field belongs there too.

**Methodology rules invoked.**
- *No defensive fallbacks* -- locks A2 (b) (no bridge
  re-verification of `register_tool` registration).
- *C semantics with information preservation* -- the
  intrinsic-table stores resolved `IntrinsicKind`, not
  the raw NAME string; rustc has done the work via
  `get_attrs_by_path`, the bridge stores the result.
- *Compositional behavior beats special-case flags* --
  A5 (b)'s category + sub-discriminant shape composes
  naturally with the append-only `AttrData` enum
  (D-14-3.2); flat variants would create a single
  growing namespace shared by all families.
- *Compiler-theory-first* -- closed-vocabulary
  attribute-driven dispatch is a textbook compiler-
  design pattern; the rustc evidence shows the
  pattern's idiomatic shape.

**Prior-art evidence.**
- `prior-art/rustc.md` §Q1 (D-14-1.1): `get_attrs_by_path`
  is the natural high-level API for two-segment tool
  paths;`rustc_middle/src/ty/mod.rs:1728-1739`;
  comparison via `Attribute::path_matches` at
  `rustc_hir/src/hir.rs:1351-1356`; custom tool
  attributes always land in `Attribute::Unparsed` per
  `rustc_hir/src/hir.rs:1265-1267` doc comment.
- `prior-art/rustc.md` §Q1 alternatives (D-14-1.1):
  `find_attr!` at `rustc_hir/src/attrs/mod.rs:34-54`
  only matches `Parsed`; unusable for tool attributes.
- `prior-art/rustc.md` §Q2 (D-14-1 Finding #1
  verification): `intrinsic_raw` provider at
  `rustc_middle/src/ty/util.rs:1685-1701` gates on
  `tcx.has_attr(def_id, sym::rustc_intrinsic)` (single-
  segment built-in); wrong hook for tool attributes.
- `prior-art/rustc.md` §Q3 (D-14-1.2): tool-prefix
  resolution at `rustc_resolve/src/ident.rs:691-694` +
  `:1896-1904`; unregistered tools fail at
  `ToolPrelude` lookup; bridge does not need to
  re-verify `tcx.registered_tools(())`.
- `prior-art/rustc.md` §Q4 (D-14-1.3): parsing chain
  `attr.meta_item_list() -> Option<ThinVec<MetaItemInner>>`
  at `rustc_hir/src/hir.rs:1312-1323`;
  `MetaItemInner::lit() -> Option<&MetaItemLit>` at
  `rustc_ast/src/attr/mod.rs:672-677`;
  `LitKind::Str(Symbol, StrStyle)` at
  `rustc_ast/src/ast.rs:2250-2253`.
- `prior-art/rustc.md` §Q4 malformed payloads
  (D-14-1.4): the four malformed shapes (no args /
  eq-style / non-string lit / multi-arg) and the
  single convergent check.

**Spec cross-reference.**
- `spec-snapshot.md` §3 (RegseqModel root) -- gains new
  field `domain_attrs: DomainAttrTables` per D-14-3.1.
- `spec-snapshot.md` §3.X (new "Domain attribute tables"
  subsection per D-14-1.7) -- hosts the `DomainAttrTables`
  struct, `AttrData` enum, `IntrinsicKind` enum,
  recognition codepath, population rules, validation
  contract.
- `spec-snapshot.md` §3.5.2 (Instance-side pull-through
  helpers) -- gains 14th helper
  `instance_intrinsic(...)` per D-14-1.6.
- `spec-snapshot.md` §8.0 -- the forward-pointer paragraph
  closing §8.0 (just before §8.1 "Lifting `impl` items")
  is rewritten in place per Finding #1 to reference
  §3.6.

**Sub-decisions.**

#### D-14-1.1 -- Recognition codepath: `get_attrs_by_path` [LOCKED]

**Decision.** Bridge uses
`tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`
to locate the `#[rseq::intrinsic("NAME")]` attribute on
a callee `DefId`.

**Reasoning.** Multi-segment tool paths require a
multi-segment walker; `get_attrs_by_path` is rustc's
canonical API for this. The alternatives (single-segment
`get_attrs`, the `find_attr!` macro, manual walks) are
structurally wrong or strictly more work. See D-14-1
Aspect A1 + the rustc-evidence cites.

**Spec impact.** Bridge codepath -- documented in §3.X
of `spec-snapshot.md` as the canonical recognition
pattern. Phase-2 stages reuse the same codepath with
different tail segments (`&[sym::rseq, sym::mmio]`,
`&[sym::rseq, sym::interrupt]`, etc.) for their
declarative attributes.

#### D-14-1.2 -- `register_tool` validation: not bridge's responsibility [LOCKED]

**Decision.** The bridge does NOT verify
`tcx.registered_tools(())` contains `sym::rseq` before
walking `#[rseq::intrinsic(...)]` attributes. rustc
enforces the registration at name-resolution time
(`rustc_resolve/src/ident.rs:691-694`); any tool-attribute
that survives to MIR has already passed.

**Reasoning.** Defensive re-validation conflicts with
README §2 "No defensive fallbacks": if rustc has done
the check, the bridge doesn't repeat it. The pinned
toolchain rules out the question "what if rustc's
behavior changes."

**Spec impact.** Negative claim -- the bridge codepath
description in §3.X notes that no `register_tool` check
is performed. Documented to forestall future drift
toward defensive checks.

#### D-14-1.3 -- Attribute argument parsing chain [LOCKED]

**Decision.** NAME extraction follows the chain:

```rust
fn extract_intrinsic_name(attr: &Attribute) -> Result<Symbol, ParseError> {
    let list = attr.meta_item_list().ok_or(ParseError::NotDelimited)?;
    if list.len() != 1 { return Err(ParseError::WrongArgCount); }
    let lit = list[0].lit().ok_or(ParseError::NotALiteral)?;
    match lit.kind {
        LitKind::Str(sym, _) => Ok(sym),
        _ => Err(ParseError::NonStringLiteral),
    }
}
```

**Reasoning.** Standard rustc parsing chain
(`meta_item_list -> .lit() -> LitKind::Str`) is the
idiomatic shape. Direct token-stream parsing on
`DelimArgs` would reimplement
`MetaItemKind::list_from_tokens` for no benefit.

**Spec impact.** Pseudocode lands in `spec-snapshot.md`
§3.X as the recognition pseudo-implementation. Real
implementation in `regseq-driver` follows the same
shape during impl sweep #1.

#### D-14-1.4 -- Malformed-payload validation: bridge's responsibility [LOCKED]

**Decision.** The bridge owns rejection of malformed
`#[rseq::intrinsic(...)]` payloads. Four error shapes
converge on a single check:

| Source form | `AttrArgs` | `meta_item_list()` | Bridge rejects with |
| --- | --- | --- | --- |
| `#[rseq::intrinsic]` | `Empty` | `None` | `ParseError::NotDelimited` |
| `#[rseq::intrinsic = "NAME"]` | `Eq` | `None` | `ParseError::NotDelimited` |
| `#[rseq::intrinsic("NAME", extra)]` | `Delimited` (len > 1) | `Some(vec)` len ≠ 1 | `ParseError::WrongArgCount` |
| `#[rseq::intrinsic(42)]` / `#[rseq::intrinsic(foo)]` | `Delimited` (non-string lit) | `Some(vec)` but `.lit()` mismatched | `ParseError::NonStringLiteral` |

**Reasoning.** rustc does NOT pre-validate tool-attribute
payloads (`Attribute::Unparsed` is precisely the "no
schema" arm). Validation is wholly the bridge's
responsibility. The single check "exactly one inner
item, and it's `Lit` with `LitKind::Str`" covers all
four shapes uniformly.

**Spec impact.** Error taxonomy lands in
`spec-snapshot.md` §3.X. Diagnostics are bridge-side
(impl sweep #1); the spec commits to the error shapes,
not the exact diagnostic strings.

#### D-14-1.5 -- `IntrinsicKind` enum shape: category + sub-discriminant [LOCKED]

**Decision.** `IntrinsicKind` is a closed,
append-only enum with one variant per domain family;
each variant carries a per-family sub-enum:

```rust
pub enum IntrinsicKind {
    Log(LogLevel),
    // stage 16 appends: Mmio(MmioOp),
    // stage 17 appends: Interrupt(InterruptOp),
    // stage 18 appends: Atomic(AtomicOp),
    // ...
}

pub enum LogLevel { Debug, Info, Warn, Error }
```

Diagnostic-logging is the sole stage-14 family per the
phase-1 foundation scope. The exact `LogLevel` variant
set + intrinsic NAME mapping is D-14-2 territory.

**Reasoning.** Category + sub-discriminant wins on three
counts:
- The IR shape mirrors the domain-family taxonomy. A
  consumer that wants "any logging operation" matches
  `IntrinsicKind::Log(_)`; flat variants force
  enumeration of every member.
- Each phase-2 stage owns its sub-enum (parallel to
  how it owns its sibling-table contributions). Stage
  16 adds `MmioOp`; stage 17 adds `InterruptOp`; etc.
- Closed-vocabulary lookup `IntrinsicKind::from_name`
  reads naturally:
  ```rust
  match name.as_str() {
      "log_debug" => Some(IntrinsicKind::Log(LogLevel::Debug)),
      "log_info"  => Some(IntrinsicKind::Log(LogLevel::Info)),
      // ...
      _ => None,
  }
  ```

Flat variants would force a single growing namespace
shared by all families (`Log_Debug`, `Log_Info`, ...,
`Mmio_Read`, `Mmio_Write`, ..., `Atomic_Load_Relaxed`,
...); the taxonomy gets lost in the IR shape.

**Spec impact.** `IntrinsicKind` enum defined in
`spec-snapshot.md` §3.X with the stage-14 single-variant
shape; phase-2 stages append variants per D-14-3.2.

#### D-14-1.6 -- Storage shape: `AttrData::Intrinsic` in `DomainAttrTables` [LOCKED]

**Decision.** Resolved intrinsic identity is stored via
`AttrData::Intrinsic(IntrinsicKind)` as an entry in
`domain_attrs.item_attrs[id]` (the unified
`DomainAttrTables` architecture from D-14-3). No
`IntrinsicInfo` wrapper struct is introduced.

```rust
pub struct RegseqModel {
    // ... existing fields ...
    pub domain_attrs: DomainAttrTables,  // per D-14-3.1
}

pub struct DomainAttrTables {
    pub item_attrs:  IndexVec<ItemId, Vec<AttrData>>,
    pub field_attrs: HashMap<FieldPath, Vec<AttrData>>,
    pub span_attrs:  HashMap<Span, Vec<AttrData>>,
}

pub enum AttrData {
    Intrinsic(IntrinsicKind),
    // phase-2 stages append per D-14-3.2
}
```

The Instance-side pull-through helper per stage-12
D-12-1.8 (the §3.5.2 inventory) gains a 14th entry:

```rust
impl RegseqModel {
    pub fn instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind> {
        let item_id = self.instances[id].item;
        self.domain_attrs.item_attrs[item_id]
            .iter()
            .find_map(|attr| match attr {
                AttrData::Intrinsic(k) => Some(k),
            })
    }
}
```

(The exhaustive `match` will gain arms returning `None`
as phase-2 stages append `AttrData` variants.)

**Reasoning.** The proposed `IntrinsicInfo` wrapper
struct from Finding #1 carried three fields; the
evidence pass eliminated two as redundant:
- `source_item: ItemId` is the `item_attrs` IndexVec key.
- `name: StringId` is recoverable from the `kind`
  variant (each variant maps 1:1 to a fixed NAME;
  diagnostic messages can hard-code the mapping).

With both redundant fields dropped, the wrapper degenerates
to `IntrinsicInfo { kind: IntrinsicKind }` -- a
single-field wrapper that doesn't exist anywhere else
in the existing spec. Storing `AttrData::Intrinsic(kind)`
in the unified `item_attrs` table is cleaner and
consistent with D-14-3's architecture.

**Methodology rules invoked.**
- *Don't add abstractions beyond what the task requires*
  (project default + CLAUDE.md) -- favors no wrapper.
- *Compositional behavior beats special-case flags* --
  storage via the unified `AttrData` enum composes with
  every other domain family's storage; a separate
  `intrinsic_items` table would be a parallel mechanism.

**Spec impact.** Storage schema lands in
`spec-snapshot.md` §3.X. 14th pull-through helper lands
in §3.5.2 as an additional entry in the
`impl RegseqModel` block.

#### D-14-1.7 -- Population timing + spec home [LOCKED]

**Decision.** Population spans two phases of stage-12
D-12-5's canonical pipeline -- mirroring the
`vtables` / `ConstMemoryStatic` / closure-captures
pattern documented at §3.4:

- **Phase 2 -- recognition into a Phase-2 working cache.**
  During Item translation, the bridge calls
  `tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`
  for each candidate `DefId`. The result is parsed per
  D-14-1.3 (`meta_item_list -> .lit() -> LitKind::Str`)
  and mapped to `IntrinsicKind` per D-14-1.5. Recognized
  intrinsics enter a Phase-2 working cache keyed by
  `DefPathHash` (the stable Phase-2-available handle;
  available via `tcx.def_path_hash(did)`). The cache
  lives in bridge construction state, not in
  `RegseqModel`.

  Iterator-multiplicity dispositions per D-14-1.4:
  - 0 attributes: no entry.
  - 1 attribute: extract NAME, lookup
    `IntrinsicKind::from_name`, insert into cache. An
    unknown NAME is rejected per Q7's closed-vocabulary
    rule.
  - > 1 attributes: bridge rejects with
    `BridgeError::IntrinsicAttrMultiple` at recognition
    time (defends I-DomainAttr-Intrinsic-Singleton per
    D-14-3.4 ahead of validation).

- **Phase 4 -- materialization into
  `domain_attrs.item_attrs`.** Phase 4 assigns final
  `ItemId`s in F4-sorted order (per D-12-5.3 post-sort
  ItemId assignment rule), then walks each Item to
  populate side-tables: `item_def_paths[id]`,
  `item_synthetic_kinds[id]`, and -- new per this
  decision -- `domain_attrs.item_attrs[id]`. For each
  Item, look up its `DefPathHash` in the Phase-2
  intrinsic cache; if present, push
  `AttrData::Intrinsic(kind)` to
  `domain_attrs.item_attrs[id]`. The `IndexVec`
  backbone is initialised at the start of Phase 4 to
  `IndexVec::from_elem(Vec::new(), items.len())`.

`field_attrs` and `span_attrs` follow the same
two-phase pattern (Phase 2 recognition into per-family
caches keyed by Phase-2-stable handles; Phase 4
materialization into the final sibling tables).
Phase-2 stages that introduce these families specify
the analogous recognition + materialization pseudocode
at their merger close.

Spec home: new subsection §3.6 (RegseqModel root)
titled "Domain attribute tables." Hosts the
`DomainAttrTables` struct, `AttrData` enum,
`IntrinsicKind` enum, recognition + materialization
pseudocode (two-phase per above), validation contract,
attribute-namespace storage rule (D-14-3.5), per-family
target-table specification table (D-14-3.6), and
cross-references to §3.4 / §3.3 / §3.5.2 / §8.0.

**Reasoning.** Recognition is naturally Phase 2 work
because it consumes rustc state (`tcx.get_attrs_by_path`,
`DefId` -> `DefPathHash`). Materialization MUST wait for
Phase 4 because `domain_attrs.item_attrs` is keyed by
final `ItemId`s, and D-12-5.3 explicitly forbids
incremental ItemId assignment during Phase 2 (the F4
sort discipline requires post-sort assignment to
preserve cross-build determinism per §16). §3.4's
existing pattern for `vtables` / `ConstMemoryStatic`
documents the same two-phase split -- Phase-2 cache
keyed by stable handle, Phase-4-or-later
re-key-to-ItemId. Stage-14 inherits this convention
verbatim; the cluster's first review surfaced an earlier
draft that conflated recognition with population and
attempted to push to `model.domain_attrs.item_attrs[id]`
during Phase 2, which would have used a non-final or
placeholder `ItemId` and silently required an unstated
remap.

§3 hosts `RegseqModel`'s field schemas (already the home
for `items`, `instances`, `vtables`, `crates`, etc.).
The new `domain_attrs` field belongs there. Folding into
§8 (Item) would misframe the storage: `DomainAttrTables`
is a `RegseqModel` field, not an `Item` field.

**Spec impact.** New §3.6 subsection. §3.4's Phase-4
side-table population list gains a bullet for
`domain_attrs.item_attrs` (alongside `item_def_paths`
and `item_synthetic_kinds`). The existing §8.0
forward-pointer paragraph (Finding #1 correction
target) is rewritten to reference §3.6.

---

### D-14-2 -- Diagnostic-logging vocabulary + bridge contract [LOCKED]

**Question.** D-14-1.5 locked `IntrinsicKind::Log(LogLevel)`
as the operational-intrinsic carrier shape and left the
exact `LogLevel` variant set, NAME mapping, intrinsic
function signatures, argument carrier, macro contract,
bridge translation rule, format-template grammar, and
backend lowering contract to D-14-2. The taxonomy work at
`regseq-taxonomy-work/research/construct-discovery-v3/
unified-taxonomy/50-discussions/03-diagnostic-logging/
refined-construct.md` (v2 candidate) is the source-of-truth
DSL surface; stage 14 adopts it as the binding contract for
phase-1's sole domain extension.

**Decision.** Diagnostic-logging is **the only phase-1
domain extension** per README §1.3 (foundation
observability carve-out). The construct is intrinsic-heavy
(no trait, no callback, no typed Rust resource on the
author surface). The author surface is four severity-named
declarative macros (`regseq::log::{error,warn,info,debug}!`)
over a single underlying primitive (`emit_diagnostic`)
captured by the bridge as four distinct intrinsic callees
that recognise per the Q7 mechanism (D-14-1). The bridge
peels the macro's `LogArg` slice carrier at body-construction
time (DiagLogPeel pass) so the IR carries a flat-arg Call
shape; the backend lowers directly to the target OS's
native severity emitter (Pipeline A guarantee --
zero-runtime-cost on the emit path). Format-template
validation and per-arg-type validation run at recognition
time. Atomic-context analysis is deferred to phase-2
stage 17 (interrupt construct); `register_log_module` glue
is deferred to phase-2 stage 15 (driver-identity /
lifecycle).

The cluster lands as 9 sub-decisions (D-14-2.1 through
D-14-2.9). Each pins one facet of the surface; together
they close the diagnostic-logging spec end-to-end. D-14-2.5
is the load-bearing architectural decision (bridge
translation contract); the rest are mostly mechanical pins
of the refined-construct.md surface adapted to RegseqModel
conventions.

**Reasoning.** Three load-bearing rationales drive the
cluster:

1. **Pipeline A guarantee is non-negotiable.** The refined-
   construct.md explicitly mandates that the `LogArg` enum,
   the carrier slice, and the `log_<sev>` function call
   itself do NOT appear in generated target code. Diagnostic
   logging is on every device driver's emit path; runtime
   overhead per emit (allocating a slice, dispatching on a
   variant tag, calling through a function pointer) is
   structurally unacceptable. The decision to model `LogArg`
   as a recognised bridge-level shape (not a regular
   RegseqModel ADT) is forced by this guarantee.

2. **Bridge does the desugaring (README §2).** Drop-glue
   calls become regular Calls, trait objects become vtables,
   closures become structs+methods, generics get
   monomorphised, `impl` blocks are lifted -- the bridge
   converts MIR shapes that don't fit C-shape lowering into
   shapes that do. Diagnostic logging fits the pattern: the
   MIR carrier-and-slice shape is convenient for Rust
   type-checking but doesn't survive C lowering. The bridge
   peels it. Backend lowering operates on the peeled shape,
   which matches the C-shape (variadic printf-family call)
   it eventually targets.

3. **Compositional behaviour beats special-case flags
   (README §2).** No new side table for diagnostic-logging
   call sites; no new `TerminatorKind::EmitDiagnostic`
   variant; no new flag on Call. The peeled Call rides on
   the existing Call infrastructure with the
   `I-DiagLog-CallArity` invariant gated by
   `IntrinsicKind::Log(_)` on the callee. The exception
   clauses at §23.6.2 B-4 and B-6 are the only mechanism
   needed to accommodate the desugared shape.

**Methodology rules invoked** (across all sub-decisions;
named per rule).

- *Bridge does the desugaring* (README §2 "C semantics
  with information preservation"). Drives D-14-2.5
  Option A.
- *Compositional behaviour beats special-case flags*
  (README §2). Rejects side-table / specialised-terminator
  alternatives in D-14-2.5; rejects per-call-site
  fingerprint storage on `DomainAttrTables`; rejects a
  parallel `is_log_intrinsic` flag on Call.
- *Compiler-theory-first* (README §2). The
  peel-at-recognition-time pattern is the established
  desugaring discipline; not a Regseq invention.
- *No defensive fallbacks* (README §2). The macro's
  canonical-shape contract pins the bridge's
  pattern-match target; canonical-shape mismatch is a
  programming error (bridge-internal panic), not an
  author-facing path.
- *Spec is mutable* (README §2). The refined-construct.md
  taxonomy work changed phase-1's prior framing of
  diagnostic logging from a deferred phase-2 concern to a
  phase-1 carve-out; stage 14's scope absorbed the change
  cleanly.

**Prior-art evidence.** The
`regseq-taxonomy-work/.../03-diagnostic-logging/
refined-construct.md` v2 candidate is the primary source.
It captures cross-OS evidence (Linux `pr_*` / `dev_*` /
`printk`; Zephyr `LOG_*`; FreeBSD `log` / `device_printf`;
Windows `DbgPrintEx` / `WPP` / `ETW`; macOS `os_log`) and
adopts the four refinement outcomes (P1-P4) that
restructured the construct from the v1 LOCKED page. The
key architectural choices the taxonomy work landed --
single `emit_diagnostic` operation parameterised by
severity (P1); no DSL-level gating (P2); compiler
atomic-context analysis instead of uniform per-op property
(P3); Regseq-owned format template + closed typed
argument carrier (P4) -- are inherited verbatim by
D-14-2.

**Spec impact** (cluster-level):

- §3.6.3 IntrinsicKind: NAME mapping locked at dash form
  (`log-debug` / `log-info` / `log-warn` / `log-error`).
  Removes the "stage-14 working commitment" disclaimer.
  Cross-references new §26.
- §23.6.2 B-4 and B-6: exception clauses for
  `IntrinsicKind::Log(_)` callees. Names the
  `I-DiagLog-CallArity` invariant.
- §25.7 intro: rewritten from "two passes" to "three
  passes."
- §25.7.3 DiagLogPeel: new subsection. Body-construction-
  time pass.
- §25.7.4 Pass scheduling: renumbered (was §25.7.3);
  diagram updated to thread DiagLogPeel through Phase 5
  body-materialisation step alongside Drop-as-Call.
- §26 Diagnostic logging (intrinsic family): new
  top-level section. Hosts the full author-side surface,
  intrinsic shape, `LogArg` carrier, macro contract,
  bridge translation contract, template grammar,
  validation diagnostics, backend lowering record, and
  deferrals.

**Cross-stage friction.** D-9-10 (Terminator-site
invariants registry) is **REOPENED** by D-14-2.5 (the
`I-DiagLog-CallArity` exception clauses at B-4 and B-6
were stated as uniform invariants pre-stage-14). Flip in
`decisions.md` at merger close with annotation "(revised
in stage 14 D-14-2.5: B-4 and B-6 carved out for
`IntrinsicKind::Log(_)` callees per I-DiagLog-CallArity
invariant)". D-13-6 (per-arm rendering for body-level
constructs including `Call`) is **NOT REOPENED**: the
`Call` per-arm spec renders whatever args the Call
carries element-by-element regardless of arity, so the
post-DiagLogPeel flat-arg shape renders naturally with no
spec change.

**Inbound expectations filed** (per README §2 forward-
expectations rule):

- `../stage-15/expectations.md`: `register_log_module`
  glue (per-target binding); device-context joining for
  Linux `dev_*` lowering.
- `../stage-17/expectations.md`: call-site
  atomic-context fact for §26.8 D5.

#### D-14-2.1 -- Severity vocabulary + intrinsic NAME mapping [LOCKED]

**Decision.** `LogLevel` is the closed 4-variant enum
`{ Debug, Info, Warn, Error }`. Each variant maps 1:1 to a
distinct intrinsic NAME in dash form:

| `LogLevel` | Intrinsic NAME | Function name |
|------------|----------------|---------------|
| `Debug`    | `log-debug`    | `log_debug`   |
| `Info`     | `log-info`     | `log_info`    |
| `Warn`     | `log-warn`     | `log_warn`    |
| `Error`    | `log-error`    | `log_error`   |

Per-level intrinsics (four distinct callees), not one
intrinsic parameterised by a level operand. No `Trace`
variant; no `Critical` variant.

**Reasoning.** Three points:
- **Four levels are the cross-OS intersection.**
  refined-construct.md's review-02 cross-OS pass confirms
  every target's native severity-banded emitter family has
  at least these four; some targets have more (Linux has
  `pr_emerg` / `pr_alert` / `pr_crit` / `pr_notice` for
  six levels above + below the four; Windows DbgPrintEx
  has `DPFLTR_TRACE_LEVEL` for sub-debug verbosity), but
  the four-tuple is the minimum portable surface. Adding
  more would force per-target downcast policies.
- **Per-level intrinsics over a single-with-level-operand.**
  Each backend binds severity to a distinct compile-time
  emitter (`pr_warn` vs `pr_err`, not `pr_dispatch(WARN, ...)`
  vs `pr_dispatch(ERR, ...)`); per-level intrinsics match
  the natural lowering pattern. Recognition is also
  cleaner: the bridge resolves callee identity to severity
  via the existing `IntrinsicKind::from_name` lookup, with
  no runtime operand inspection required.
- **Dash-form NAMEs match the taxonomy convention.** The
  refined-construct.md surface uses dash form
  (`#[rseq::intrinsic("log-error")]`), which generalises
  to other domain families (`mmio-read32`,
  `atomic-load-acquire`, `interrupt-mask`, ...). Function
  names use underscore form (Rust identifier
  convention); the divergence is documented at §3.6.3.

**Methodology rules invoked.**
- *Closed-vocabulary recognition* (Q7) -- the four NAMEs
  are the entire log-family vocabulary; unknowns are
  rejected.
- *Source declares its own compiler-relevant attributes*
  (README §2) -- the dash-form NAME is what the source
  writes; the bridge keys on what the source writes
  verbatim, not on a rewritten form.

**Prior-art evidence.** refined-construct.md's
"Operations / Canonical paths" + "Per-primitive record /
per_target_lowering" tables; the four-level closure
matches Linux `pr_err` / `pr_warn` / `pr_info` /
`pr_debug`, Zephyr `LOG_ERR` / `LOG_WRN` / `LOG_INF` /
`LOG_DBG`, FreeBSD `LOG_ERR` / `LOG_WARNING` / `LOG_INFO`
/ `LOG_DEBUG`, Windows `DPFLTR_ERROR_LEVEL` /
`DPFLTR_WARNING_LEVEL` / `DPFLTR_INFO_LEVEL` /
`DPFLTR_TRACE_LEVEL`, and macOS `os_log_error` / `os_log`
(default) / `os_log_info` / `os_log_debug`.

**Spec impact.** §3.6.3 `IntrinsicKind::from_name`
finalised at dash form. §26.2 hosts the canonical
LogLevel + NAME mapping table.

#### D-14-2.2 -- Intrinsic function shape [LOCKED]

**Decision.** Four facade-side stub functions, one per
`LogLevel`, identical signature shape:

```rust
#[inline(never)]
#[rseq::intrinsic("log-error")]
pub fn log_error(fmt: &'static str, args: &[LogArg<'_>]) {
    unimplemented!()
}
```

Two parameters: `fmt: &'static str` template + `args:
&[LogArg<'_>]` slice. Return type `()`. Body
`unimplemented!()`. Both attributes load-bearing:
`#[inline(never)]` for Q7 recognition (blocks rustc MIR
inliner); `#[rseq::intrinsic("log-<sev>")]` for stable
NAME identity per D-14-1.

Location: `regseq::log::intrinsics::log_<sev>` in
`#[doc(hidden)] pub mod intrinsics`. Author surface is
the macros at `regseq::log::<sev>!` (per D-14-2.4); the
intrinsic functions are signaled-non-author surface by
the `intrinsics` module name + `#[doc(hidden)]` +
prefix (`log_`) per refined-construct.md's discipline.

**Reasoning.** The two-parameter signature is the
canonical surface DiagLogPeel pattern-matches against
(D-14-2.5). `#[inline(never)]` defends the call site
against rustc's MIR inliner; refined-construct.md notes
this is the load-bearing concern (LLVM's later inliner
is secondary -- it runs after the bridge anyway).
`unimplemented!()` body produces an explicit
runtime-panic signal when the Regseq pass doesn't run
(build-pipeline-misconfiguration signal, not a soundness
trap).

**Methodology rules invoked.**
- *Source declares its own compiler-relevant attributes*
  -- `#[inline(never)]` + `#[rseq::intrinsic(...)]` both
  declared at the function definition, not injected.
- *No defensive fallbacks* -- `unimplemented!()` body is
  fine; we don't need a runtime fallback emitter for the
  "Regseq pass didn't run" case.

**Spec impact.** §26.3 records the canonical stub
signature, required attributes, body discipline, location
convention, and the post-bridge Item metadata link to
`IntrinsicKind::Log(level)` via `domain_attrs.item_attrs`.

#### D-14-2.3 -- `LogArg` argument carrier [LOCKED]

**Decision.** Closed 10-variant enum at
`regseq::log::intrinsics::LogArg`, `#[doc(hidden)]`:

```rust
pub enum LogArg<'a> {
    U8(u8),   U16(u16), U32(u32), U64(u64),
    I8(i8),   I16(i16), I32(i32), I64(i64),
    Bool(bool),
    Str(&'a str),
}
```

Width-explicit (one variant per native width). `From<T>`,
`From<&T>`, `From<&mut T>` impls per value-typed variant;
`From<&'a str>` for `Str`. Platform-width aliases
(`usize` / `isize`) route via `#[cfg(target_pointer_width
= ...)]` to the matching variant. One level of
indirection only (no `&&T`, `&mut &T` impls).

Deliberately rejected (named list): `u128`, `i128`,
`f32`, `f64`, `*const T`, `*mut T`, `c_char` as distinct
type, `&T` for non-supported `T`, `Option<T>`,
`Result<T, E>`, `&[T]`, `Vec<T>`, `String`, `CString`,
`Ty::FnDef`, `Ty::FnPtr`, `Ty::Closure`, tuple / array /
slice values, interior-mutability wrappers
(`core::cell::Cell<T>`, `core::cell::RefCell<T>`,
`core::sync::atomic::AtomicU*` / `AtomicI*`,
`core::mem::MaybeUninit<T>`), opaque `impl Trait` whose
concrete monomorphised type is not admissible, and custom
structs / enums. **Catch-all closure rule:** the admissible
set is closed by the `From` impl set; any monomorphised
`Ty` lacking a matching `From<...> for LogArg<'a>` impl
surfaces D-14-2.7 D4, whether or not it appears on the
named list. The named list is documentation; the `From`
matrix is authoritative.

**Reasoning.** Width-explicit (Option B per refined-
construct.md) over a width-collapsed pair (Option A:
`Uint(u64)` + `Int(i64)`) because the variant tag is
self-describing for backend lowering (`LogArg::U32`
implies `%u` on Linux; `LogArg::U64` implies `%llu`); a
width-collapsed pair would force the backend to re-derive
the original width from out-of-band metadata. Per-type
`From` impls (not a marker trait) ensure the unimplemented-
trait error message points at the original argument
expression at the macro call site (D-14-2.7
diagnostic-quality requirement).

Pointer support deliberately dropped: the DSL exposes no
raw pointers in author code (refined-construct.md confirms),
so a `Ptr` variant and a `:p` modifier (§26.7) would be
unused surface. Address formatting routes through
hex-formatting an integer-typed address acquired from a
DSL operation (e.g., MMIO base address).

Reference support added per refined-construct.md: `From<&T>`
+ `From<&mut T>` impls allow authors to pass references
directly without explicit deref; one level of indirection
keeps the impl matrix bounded.

**Methodology rules invoked.**
- *Closed-vocabulary* over open-ended -- the rejected-types
  list is closed; new types reopen the cluster.
- *Compositional behaviour* -- `From` impls compose with
  Rust's coercion rules naturally; no marker trait
  needed.

**Spec impact.** §26.4 records the enum, the `From` impl
matrix, the platform-width routing, the
one-level-indirection rule, and the rejected-types list.
§23.6.2 B-6 exception clause cites §26.4's admissible
set.

#### D-14-2.4 -- Macro contract / post-expansion shape [LOCKED]

**Decision.** Four declarative macros at `regseq::log::<sev>!`
(no proc-macro crate), each `#[macro_export]`-ed.
Canonical expansion shape, identical across severities;
shown for `error`:

```rust
#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::intrinsics::log_error(
            $fmt,
            &[ $( $crate::log::intrinsics::LogArg::from($arg) ),* ],
        )
    };
}
```

The expansion shape is part of the construct contract.
Three invariants the bridge keys on:
- Callee resolves to one of the four
  `regseq::log::intrinsics::log_<sev>` functions.
- First arg is a `&'static str` literal (the
  `$fmt:literal` matcher).
- Second arg is a slice literal of `LogArg::from(<expr>)`
  calls; one wrapper per author argument.

Argument-evaluation contract: args are evaluated at the
call site unconditionally. No lazy evaluation. Target-aware
codegen-level elision is deferred (per D-14-2.8 / §26.10).

**Reasoning.** Declarative `macro_rules!` over proc-macro
because the expansion is mechanical (template + slice
construction) and stays in the source crate's
`regseq-macros` boundary cleanly; proc-macros would
require a heavier-weight build dependency and a procedural
crate compilation step for every consumer. The
`$fmt:literal` matcher enforces D-14-2.7 D6 ergonomically
at the macro level (the error message at the
literal-mismatch site is the structural macro error, not
a bridge-emitted diagnostic).

The fixed expansion shape is the DiagLogPeel pattern-
match target. Future macro revisions that break this
shape must update the bridge's canonical-shape assertion
(§25.7.3 step 1).

**Methodology rules invoked.**
- *Source declares its own compiler-relevant attributes*
  -- the macro is the author surface; the expansion is
  the bridge surface; the divide is explicit.

**Spec impact.** §26.5 records the macros, the canonical
expansion shape, the three invariants, the argument-
evaluation contract, and the future-macro discipline.

#### D-14-2.5 -- Bridge translation contract; DiagLogPeel; `I-DiagLog-CallArity` invariant [LOCKED]

**Decision.** The bridge translates source-level
diagnostic-logging Calls into a flat-arg Call shape at
body-construction time. The MIR carrier-and-slice shape
(`Call { callee: log_<sev>, args: [fmt, slice_of_LogArg_from_calls] }`)
is recognised by the bridge during Phase 5 body
materialisation; the bridge peels the
`LogArg::from(<expr_i>)` wrappers and drops the slice
aggregate, synthesising the desugared Call shape `Call {
callee: log_<sev>, args: [fmt, <expr_1>, ..., <expr_N>] }`
in RegseqModel. The `LogArg` enum, the carrier slice, and
any intermediate locals introduced by the macro to
materialise them do NOT appear in RegseqModel. The pass
is named **DiagLogPeel**; mechanics at §25.7.3.

**Reference-companion normalisation.** §26.4's `From<&T>`
/ `From<&mut T>` impls accept reference-typed arguments at
the macro surface (so authors can write `info!("{}", &x)`
without explicit `*x` deref). The `From` impl bodies
dereference internally, so semantically the reference is
consumed to produce a value-typed `LogArg`. DiagLogPeel
mirrors this at the IR level: when the peel recognises a
reference-companion `From` impl on `<expr_i>`, it
synthesises a `Statement::Assign(_tmp, Rvalue::Use(
Operand::Copy(*<expr_i>)))` load before the flat Call and
substitutes `_tmp` (value-typed) as the call's arg. The IR
Call's args therefore carry uniformly value-typed Operands;
no reference-companion Operand survives into RegseqModel.
This single-peel normalisation discipline keeps the §26.9
backend specifier table value-typed-only and avoids
per-target reference-deref logic.

**Named invariant.** The peeled Call's shape is captured
by the `I-DiagLog-CallArity` invariant (§26.6 canonical
statement):

> A Call whose callee carries
> `AttrData::Intrinsic(IntrinsicKind::Log(_))` has args
> `[Operand::Constant(<&'static str literal>), <expr_1>,
> ..., <expr_N>]` where `N >= 0` is the number of `{}`
> placeholders in the literal. `args.len() == 1 + N`.

§23.6.2 B-4 and B-6 carry exception clauses gated by this
invariant. Specifically:
- B-4 (Call args length == callee signature arity) does
  NOT match the declared callee signature `fn(&'static
  str, &[LogArg<'_>])`'s 2 inputs; arity follows the
  desugared shape (`1 + N`).
- B-6 (per-arg type match) does NOT match the declared
  callee signature's `&[LogArg<'_>]` second-input type;
  per-arg types are validated against the format-template
  placeholder kinds (§26.8 D2) and the `LogArg`-admissible
  set (§26.8 D4).

**Pass placement.** DiagLogPeel runs in **Phase 5 body
materialisation** alongside Drop-as-Call collapse
(§25.7.1) -- both at per-rustc-Terminator translation
time. By Phase 5, intrinsic recognition (D-14-1.7 Phase 2)
and materialisation into `domain_attrs.item_attrs`
(D-14-1.7 Phase 4) are complete; DiagLogPeel reads the
materialised sibling table at body-construction time to
decide whether each `Call` enters the peel branch
(callee carries `IntrinsicKind::Log(_)`) or the regular
Call-translation branch.

**Transient fingerprint discipline.** DiagLogPeel produces
a call-site fingerprint at step 5 (severity + format
template + per-arg `Ty` + span) consumed immediately by
§26.8 validation. The fingerprint is **NOT stored** on
`RegseqModel`. Both severity (via
`model.instance_intrinsic(callee_instance_id)`) and
per-arg `Ty` (via `operand_ty(args[i])`) are re-derivable
from the IR; no side table is needed.

**Canonical-shape assertion is bridge-internal.** The MIR
shape DiagLogPeel pattern-matches against is pinned by the
macro contract (D-14-2.4). Shape mismatch can only arise
from (a) a future macro revision that breaks the contract
(must update the assertion) or (b) authors bypassing the
macro to call `regseq::log::intrinsics::log_<sev>`
directly with a hand-rolled slice (the `#[doc(hidden)]`
+ `intrinsics` module signaling discourages this; if it
happens, treating it as a programmer error -- panic with a
`bug!`-style diagnostic -- matches the project's
"no-defensive-fallbacks" rule).

**Architectural choice (Options A / B / C / D).**

The cluster considered four ways to handle the
`LogArg`-carrier shape:

- **Option A** (selected): Bridge desugars to flat-arg
  Call. `LogArg::from()` calls and slice aggregate
  removed; IR Call carries peeled operands.
- **Option B**: Bridge stores peeled fingerprint in a
  side table; IR Call retains MIR shape. Rejected -- adds
  a new sibling-table family (would reopen D-14-3) for
  information the IR can carry once peeled.
- **Option C**: New `TerminatorKind::EmitDiagnostic`
  variant. Rejected -- variant on `IntrinsicKind`
  (D-14-1.5) plus regular `Call` carries the distinction;
  parallel terminator-level flag violates the
  composition-over-flags rule.
- **Option D**: Backend peels at lowering, IR retains
  MIR shape. Rejected -- forces each backend to
  reimplement the peel; the bridge half-does the work
  (recognition without peel) leaves the IR shape only
  conventionally pinned.

Option A wins on three project rules: *bridge does the
desugaring* (README §2 "C semantics with information
preservation"); *compositional behaviour beats
special-case flags* (rejects C and any side-table flag);
*don't add abstractions beyond what the task requires*
(rejects B's new sibling-table family).

**Reasoning.** Pipeline A (zero runtime cost on the emit
path) is the load-bearing constraint. The DSL surface
cannot pay a slice-allocation + variant-dispatch cost
per log call site -- diagnostic logging is on every
driver's emit path and competitive with bare `printk` /
`printf` is the bar. The peel collapses the carrier-shape
machinery into a single direct call to the target's
native printf-family emitter at lowering time. The peel
must happen once (in the bridge) so backends consume a
stable IR shape without per-backend reimplementation.

The `I-DiagLog-CallArity` invariant is the cleanest way
to capture the shape change while keeping the regular
`Call` terminator infrastructure. The invariant is gated
by `IntrinsicKind::Log(_)` on the callee, so the
divergence is bounded and explicit.

**Methodology rules invoked.**
- *Bridge does the desugaring* (README §2) -- explicit
  precedent for drop-glue, vtables, closures,
  monomorphisation.
- *C semantics with information preservation* -- C-shape
  for printf-family is variadic; IR matches.
- *Compositional behaviour beats special-case flags* --
  reject Option C; reject side-table flag.
- *No defensive fallbacks* -- canonical-shape mismatch is
  a panic, not a fallback.
- *Don't add abstractions beyond what the task requires*
  -- reject Option B's new sibling table.

**Prior-art evidence.** Drop-as-Call collapse at §25.7.1
is the closest existing precedent: a bridge-construction-
time pass that recognises a canonical rustc terminator
shape (rustc `Drop`) and synthesises a Call shape that
fits RegseqModel's vocabulary. DiagLogPeel mirrors this
pattern; the placement at §25.7 alongside Drop-as-Call
is intentional.

mrustc / Charon both have analogous "desugar-at-bridge"
passes for closures (Charon `transform_closures.rs`;
mrustc `closures.cpp`) and for trait-object method calls
(stage-11 D-11-1 + §25.7.2 vtable-indirect rewrite). The
"recognise + peel" discipline is well-established in
peer compilers.

**Cross-stage friction.** D-9-10 REOPENED. The
exception clauses at B-4 and B-6 carve out
`IntrinsicKind::Log(_)` callees from the uniform
arity-and-type-match rules stage 9 pinned. Flip in
`decisions.md` at merger close.

**Spec impact.** §23.6.2 B-4 and B-6 amended with
exception clauses citing `I-DiagLog-CallArity`. §25.7
intro reframed from "two passes" to "three passes."
§25.7.3 (new): DiagLogPeel mechanics. §25.7.4
(renumbered from §25.7.3): Pass scheduling diagram
updated. §26.6: canonical statement of the
`I-DiagLog-CallArity` invariant.

#### D-14-2.6 -- Format template grammar [LOCKED]

**Decision.** The Regseq format template is a restricted
printf-style subset, owned by Regseq (not `core::fmt`),
with closed leading modifier subset for v1:

- **Template form**: `&'static str` literal at the macro
  call site (D-14-2.7 D6).
- **Placeholders**: `{}` only -- no named arguments, no
  positional indexes, no dynamic precision, no `Display` /
  `Debug` formatter identification.
- **Modifier subset**: closed at the leading set
  `{default, :x, :X, :#x, :#X, :08x, :#010x}`. Wider
  coverage (other widths, decimal padding, alignment
  specifiers) is deferred to the experiments track at
  `docs/discussions/logging-and-printing/`; future
  extensions don't carry automatic backwards-compatibility.
- **Escape sequences**: `{{` and `}}` are literal `{` /
  `}` in rendered output, not placeholders.
- **Trailing newline forbidden**: D-14-2.7 D7; backend
  lowering appends the target-appropriate terminator.
- **Embedded newlines OK**: mid-template `\n` renders as
  visible line break; multi-line emissions from one call
  supported.
- **Empty template legal**: `info!("connected")` is
  well-formed (0 placeholders + 0 args).

The `:p` modifier (raw-pointer formatting in stock Rust)
is NOT in the subset (no raw pointers on the DSL surface);
address formatting goes through hex-formatting an
integer-typed address.

**Reasoning.** Closed leading subset for v1 because:
(a) the project's "closed-vocabulary recognition" rule
favors enumerating the supported set explicitly;
(b) cross-OS evidence (refined-construct.md review-02)
confirms the leading subset is the intersection of all
target printf-style accepted modifier sets, so
implementations don't risk per-target divergence on the
v1 surface; (c) wider coverage requires per-target
verification (some modifiers don't translate cleanly --
e.g., `%*d` dynamic-precision is unsupported on Zephyr
`cbprintf` per refined-construct.md) and the experiments
track is the right home for that verification work.

The `{{` / `}}` escape sequences are needed to allow
literal brace characters in author messages; treating them
as escape-sequence literal text (not as placeholders)
matches `core::fmt`'s own convention and is what authors
expect. The empty-template + zero-args case is a
non-trivial corner: a literal "connected" log line is
common in driver-state messages.

Trailing-newline forbidden because every target's lowering
appends a terminator (literal `\n` on Linux / FreeBSD /
Windows / generic-C; framework-appended on Zephyr / macOS);
a template trailing newline would produce a double
terminator on every target. Embedded newlines mid-template
are unaffected: they render as visible line breaks at
every target.

**Methodology rules invoked.**
- *Closed-vocabulary recognition* -- the modifier subset
  is closed; new modifiers reopen the cluster.
- *Cross-OS evidence-based scoping* -- the leading
  subset's intersection-of-targets framing comes from
  refined-construct.md review-02.

**Spec impact.** §26.7 records the template grammar,
modifier subset table, escape sequences, trailing-newline
rule, and empty-template legality.

#### D-14-2.7 -- Bridge-side validation diagnostics [LOCKED]

**Decision.** The bridge validates each post-DiagLogPeel
call-site fingerprint against the template grammar
(D-14-2.6), the `LogArg` carrier (D-14-2.3), and the
selected binding target's per-call stack-budget bound.
Eight author-facing diagnostics D1-D8 (seven
bridge-enforced; one deferred):

| #  | Rule                                                                                       | Stage-14 status         |
|----|--------------------------------------------------------------------------------------------|-------------------------|
| D1 | `placeholder_count == argument_count`                                                      | bridge-enforced (target-independent) |
| D2 | Per-position placeholder-modifier / operand-type compatibility                             | bridge-enforced (target-independent) |
| D3 | Every modifier in template is a member of the closed Regseq subset                         | bridge-enforced (target-independent) |
| D4 | Every operand's source-Rust `Ty` is admissible per D-14-2.3                                | bridge-enforced + macro-level (defense in depth; target-independent) |
| D5 | Call site reachable from atomic context without compatible target emitter                  | **DEFERRED to stage 17** |
| D6 | `args[0]` is a `&'static str` literal Constant (checked at DiagLogPeel step 1 pre-Regseq-const-lowering) | bridge-enforced (target-independent) |
| D7 | Template does not end in `\n`, `\r`, or `\r\n`                                              | bridge-enforced (target-independent) |
| D8 | `&str` operand count in args[1..] <= selected binding target's `max_str_args_per_call`     | bridge-enforced (**target-aware**; Generic-C: 8 per §26.9; Linux: deferred to stage 15) |

D5 is deferred to **phase-2 stage 17 (interrupt / ISR
family)** because the call-site context fact depends on
the interrupt construct's call-site context analysis.
Filed as inbound expectation on
`../stage-17/expectations.md`. Phase-1 lowerings operate
as if every call site is atomic-safe.

D8 is **target-aware**: the bound is declared per binding
target (`max_str_args_per_call`). Stage 14 binds
Generic-C at 8; Linux's bound declaration is deferred to
phase-2 stage 15 because it couples to Linux's `&str`
materialisation strategy (`printk` built-in copy vs.
stack buffer vs. `kasprintf`), which stage 15 owns as
part of the `pr_*` / `dev_*` lowering pin. Filed as
inbound expectation on `../stage-15/expectations.md`.
The bridge validation pass receives the binding-target
identity as operational context and evaluates D8 per
(call site, target) pair.

**Diagnostic-message quality requirement (binding on
impl).** D4's author-facing message MUST report against
the original author argument expression, not against the
bridge-synthesised `LogArg::from(...)` wrapper (which is
gone after DiagLogPeel anyway). Implementations use
`#[diagnostic::on_unimplemented]` on `From` impls, custom
proc-macro diagnostics, or equivalent Rust-toolchain
affordances to preserve the author-side span. The exact
message text is not part of the contract; the
span-preservation requirement IS.

**Reasoning.** Seven bridge-enforced rules cover the
closed surface end-to-end: six target-independent (arity
D1; per-position compatibility D2; modifier closure D3;
argument closure D4; template form D6; template
terminator D7) + one target-aware (per-call stack budget
D8). The sequence runs on the fingerprint produced at
DiagLogPeel step 5 (§25.7.3), still within Phase 5 body
materialisation. Errors collect per the bridge's existing
error-handling convention (which gathers diagnostics per
call site and reports together at end of Phase 5).

D5 deferral preserves the v2 surface from
refined-construct.md while honoring the cross-stage
boundary: atomic-context analysis is interrupt-construct
territory.

D8 was added to bound per-call stack use against the
per-`%s`-site temporary-buffer allocation inherent in
C-string-sink lowering (the Generic-C contract at §26.9
allocates one fixed-size stack buffer per non-static
`&str` operand at the call site; an unbounded number of
`%s` args would consume unbounded stack). The
target-aware framing (rather than a single uniform
bound) acknowledges that each binding backend's
materialisation strategy has a different stack-budget
profile -- Generic-C's stack-buffer strategy needs the
bound; future Linux strategies using `printk`'s built-in
format-string copy may declare a higher or no bound.
The "count all `&str`" simplification (rather than
"count only non-static `&str`") avoids bridge-side
provenance analysis at the cost of a strictly tighter
bound; the trade-off is acceptable because realistic
driver-logging patterns are 0-3 `&str` args per call.

The diagnostic-quality requirement is the load-bearing
ergonomic constraint. Users writing
`info!("value is {}", my_struct)` should see an error
that names `my_struct: MyStruct` as the unsupported
argument, not "the trait `From<MyStruct> for LogArg` is
not implemented at expansion site
`$crate::log::intrinsics::LogArg::from($arg)`". The
bridge can preserve the call-site span through DiagLogPeel
step 5 (the fingerprint includes the original Call's
span); the `From`-impl error path requires
`#[diagnostic::on_unimplemented]` or equivalent to
preserve naming.

**Methodology rules invoked.**
- *Closed-vocabulary recognition* -- D3 and D4 enforce
  the closed sets pinned by D-14-2.3 and D-14-2.6.
- *Author-side span preservation* -- diagnostic-quality
  requirement is what makes the construct usable.

**Spec impact.** §26.8 records the validation table
(D1-D8), the D5 deferral, the D8 target-aware framing,
and the diagnostic-message quality requirement. §25.7.3
"Per-position validation" paragraph lists D1-D4 + D6 +
D7 + D8 as the bridge-side diagnostics that run on the
fingerprint, with D8 evaluated against the build's
selected binding target. §26.9 Generic-C section
declares `max_str_args_per_call = 8`; Linux's bound
declaration deferred to stage 15.

**Inbound expectations filed** (in addition to
D-14-2.8's existing items):
- `../stage-15/expectations.md`: Linux's
  `max_str_args_per_call` declaration, alongside the
  Linux `&str` materialisation strategy choice.

#### D-14-2.8 -- Deferrals and scope boundaries [LOCKED]

**Decision.** Three deferrals + four explicit out-of-scope
items.

**Deferred to phase-2 stage 15 (regseq-trait scaffolding +
Lifecycle + driver-struct discovery + RegseqError):**
- `register_log_module` -- compiler-emitted glue at
  driver-module entry. Per-target binding (`pr_fmt` setup
  on Linux, `LOG_MODULE_REGISTER` on Zephyr, etc.).
  Depends on driver-identity / lifecycle (stage 15).
- Device-context joining -- compiler-injected device
  handle for Linux `dev_*` lowering. Depends on
  driver-struct discovery (stage 15).

**Deferred to phase-2 stage 17 (interrupt / ISR family):**
- Atomic-context analysis (§26.8 D5) -- call-site context
  fact owned by interrupt construct.

**Deferred to a future revision (not phase-2-scheduled):**
- Target-aware codegen-level elision for `debug!`
  -- preserving each target's native laziness mechanism
  (Linux `pr_debug` dynamic-debug branch, Zephyr
  compile-time `LOG_LEVEL` elision, ...). The phase-1
  contract evaluates arguments at the call site
  unconditionally.
- Wider format-template modifier coverage -- experiments
  track at `docs/discussions/logging-and-printing/`.
- `&str` null-termination at backend lowering time --
  backend implementation concern; not a spec axis.

**Explicitly out of scope:**
- DSL-level emission gating. No `debug_logging` toggle.
  Emission suppression delegated to target's native
  filter.
- `emit_structured_event` (Track-B extension).
- Function-style stable logging API (preserve macro form).
- Driver-passed device handle on author surface.

**Reasoning.** Each deferral has a clear cross-stage
dependency:
- `register_log_module` + device-context joining: need
  driver-identity / lifecycle machinery (stage 15).
- Atomic-context analysis: needs interrupt construct
  (stage 17).
- Codegen-level elision: needs the design space
  explored uniformly across targets; premature
  optimisation in phase-1 risks baking in a poor
  per-target binding.

Explicit out-of-scope items match refined-construct.md
P2 (no DSL-level gating); preserve the construct's
author-surface invariants.

**Methodology rules invoked.**
- *Domain extensions iterate per stage in phase 2*
  (README §2) -- defer cross-construct concerns to the
  stage that owns the relevant abstractions.
- *Spec is mutable* -- deferred items reopen the cluster
  at the relevant phase-2 stage with current evidence.

**Spec impact.** §26.10 records all deferrals and
out-of-scope items.

**Inbound expectations filed.**
- `../stage-15/expectations.md`: `register_log_module`
  per-target glue + device-context joining.
- `../stage-17/expectations.md`: call-site
  atomic-context fact for §26.8 D5.

#### D-14-2.9 -- Backend lowering contract [LOCKED]

**Decision.** Per-target lowering table for the
post-DiagLogPeel flat-arg Call shape. Two binding targets
(phase-1 + phase-2 stage 15 implementation territory) plus
four informational targets.

**Generic-C (phase-1 implementation sweep #1; binding):**
- Severity rendered as leading bracketed tag
  (`[ERROR]` / `[WARN]` / `[INFO]` / `[DEBUG]`).
- Single emitter `printf`; single output stream (stdout).
- Backend prepends the tag + space, appends a literal
  `\n` to the author template, and emits one `printf`
  call per author log site.
- Per-arg specifier derived from each peeled operand's
  RegseqModel `Ty` (e.g., `Ty::Uint(U32)` -> `%u`;
  `Ty::Ref(Ty::Str, _)` -> `%s`).

**Linux kernel (phase-2 stage 15 implementation;
binding):**
- Severity rendered to `pr_*` family (`pr_err` / `pr_warn`
  / `pr_info` / `pr_debug`).
- `dev_*` variants (`dev_err` etc.) compose with the
  device-context joining stage-15 owns.
- Linux kernel `printk`-family appends newline
  internally; templates without trailing newline
  compose cleanly.
- Per-arg specifier follows Linux kernel `printk` width
  conventions (no length-suffix on `%u` / `%x` for
  `u32`).

**Informational (not currently scheduled):** Zephyr
(`LOG_*` + `LOG_MODULE_REGISTER`); FreeBSD (`log(LOG_*,
...)` + `device_printf`); Windows (`DbgPrintEx` for
debug builds; `WPP` / `ETW` for production); macOS
(`os_log` family, not `IOLog`). Each carries its severity
mapping, format-lowering notes, context-safety notes, and
debug-filtering mechanisms per refined-construct.md's
per-target table.

**Pipeline A uniformity.** All targets consume the same
DiagLogPeel-output IR shape (the flat-arg Call). No
target reconstructs the `LogArg` slice; pipeline B
(runtime dispatch on `LogArg` variant tags) is
explicitly NOT supported.

**Reasoning.** Generic-C `[ERROR] message` format
chosen over stderr/stdout split or alternative leading-
tag conventions (`<ERROR>`, `ERROR:`, etc.) for
readability + scriptable parsing (`grep -E
'^\[(ERROR|WARN)\]'`); single stdout simplifies test-
fixture capture (the phase-1 test discipline relies on
captured stdout per README §1.3). Linux kernel `pr_*`
selected as the phase-2 stage 15 binding (pre-device-
context); `dev_*` defers to the device-context joining
deferred from D-14-2.8.

Recording informational targets preserves the cross-OS
contract from refined-construct.md against
implementation drift if a future stage extends the
backend matrix. They are not binding for phase-1 /
phase-2 stage 15.

**Methodology rules invoked.**
- *Dual backend* (README §2) -- generic-C (phase-1) and
  Linux kernel (phase-2 stage 15) are separate codegen
  paths.
- *Information preservation* -- the per-target lowering
  rules encode information rustc gives us (operand `Ty`)
  for backend consumption; no re-derivation needed.

**Spec impact.** §26.9 records all per-target lowering
contracts, the per-`Ty`-specifier table, and the
Pipeline A uniformity rule.

---

### D-14-3 -- DomainAttr sibling-table architecture [LOCKED]

**Question.** Stage-4 §1287-1299's forward-pointer
(absorbed into the existing `RegseqModel` root comment
at `spec-snapshot.md:326-334`) enumerated four candidate
key kinds for sibling-table storage of structured DSL
property attributes (`ItemId / InstanceId / FieldPath /
Span`), but did not commit to a concrete schema. Stage
12 D-12-1.5 reaffirmed the forward-pointer ("Stage 14
picks the per-property keying"). Stage 14 is now
responsible for:
- Choosing the storage architecture (unified
  discriminated-union table vs per-family typed tables).
- Choosing the key-kind set (subset of stage-4's
  enumerated four).
- Specifying the storage invariants (ordering,
  duplicates, validation).
- Specifying the boundary between this architecture
  and typed-Item-field storage (per D-12-1.3 typed
  Item fields like `linkage`, `inline_hint`,
  `is_unsafe`, lang-item flags).

Per D-14-1.6's storage shape commitment, this is the
load-bearing architectural decision for stage 14:
D-14-3's outcome flows back through every phase-2
stage's domain-attribute spec.

**Options considered.**

Aspect B1 (storage architecture):
- (a) Per-family typed tables on `RegseqModel`. Each
  domain family adds its own field (`intrinsic_items:
  IndexVec<ItemId, Option<IntrinsicKind>>`,
  `mmio_field_attrs: HashMap<FieldPath, MmioAttr>`,
  ...). Type-safe per-family payloads; one slot per
  key.
- (b) Unified `DomainAttrTables` with a discriminated
  `AttrData` enum. One sibling table per key kind, each
  storing `Vec<AttrData>`. Unified architecture; multiple
  attributes per key allowed.
- (c) Hybrid: unified discriminated union for some
  key-kinds; per-family for others.

Aspect B2 (key-kind set, instantiated tables):
- (a) All four key kinds from stage-4 forward-pointer:
  `item_attrs`, `instance_attrs`, `field_attrs`,
  `span_attrs`. Full architecture.
- (b) Subset: `item_attrs`, `field_attrs`, `span_attrs`.
  `instance_attrs` dropped because Instance access to
  Item-level metadata goes through the §3.5.2
  pull-through pattern locked at D-12-1.8.
- (c) Lean: only `item_attrs` at stage 14; phase-2
  stages add other tables as occupants arrive (stage
  16 adds `field_attrs`; etc.).

Aspect B3 (`Vec<AttrData>` ordering):
- (a) Insertion order. Deterministic via upstream Item-
  walk order per stage-12 D-12-5.
- (b) Canonical sort by `AttrData` variant rank + payload
  identity. More robust to bridge-implementation churn
  but more work.

Aspect B4 (per-family duplicate invariants):
- (a) Validator-enforced (bridge population may produce
  duplicates; validator catches at construction time).
- (b) Type-system-enforced via `Option<T>` slots in
  per-family typed tables -- not available under B1 (b).

Aspect B5 (attribute-namespace storage line):
- (a) Implicit: stage authors decide per-decision where
  metadata lands. Risk of drift.
- (b) Explicit binding rule:
  `#[rseq::*]`-tool-namespace attributes -> sibling-table
  `AttrData` entries; recognized Rust attributes (and
  language built-ins like lang-items) -> typed Item
  fields per D-12-1.3.

Aspect B6 (per-family target-table specification):
- (a) Each phase-2 stage spec section names the sibling
  table its `AttrData` variant populates.
- (b) Inferred from `AttrData` variant payload type
  (e.g., a `FieldPath`-carrying variant lands in
  `field_attrs`). Risk: not all variants carry their
  key kind in the payload.
- (c) Both -- prefer (a) for clarity but allow (b) as
  fallback.

**Decision.** **B1 (b) + B2 (b) + B3 (a) + B4 (a) +
B5 (b) + B6 (a)**:
- Unified `DomainAttrTables` with discriminated `AttrData`
  enum.
- Three sibling tables (`item_attrs`, `field_attrs`,
  `span_attrs`); `instance_attrs` dropped.
- Insertion-order `Vec<AttrData>` storage.
- Per-family duplicate invariants validator-enforced.
- Explicit attribute-namespace storage rule
  (`#[rseq::*]` -> sibling tables; recognized Rust
  attributes -> typed Item fields per D-12-1.3).
- Per-family target-table specification mandatory in
  each phase-2 stage's spec section.

Each aspect locks as its own sub-decision below.

**Reasoning.**
- B1 (b) wins over (a) on uniformity + future-stage
  coupling: each phase-2 stage adds one `AttrData`
  variant rather than extending `RegseqModel`'s root
  schema. The cross-stage coupling on the `AttrData`
  enum is structurally parallel to how `InstanceKind`
  evolves (Q3 single-level IR rule; append-only
  ordering per stage-1 R5.1 / R5.2); this is the
  established project pattern, not friction.
- `Vec<AttrData>` (vs `Option<AttrData>`) addresses the
  multi-attribute-per-key case naturally: a single
  Item might be both an intrinsic and have a future
  domain marker. The "exactly one per key per family"
  invariant becomes per-family validator territory
  (D-14-3.4).
- B2 (b) follows from D-12-1.8: Instances reach Item-
  level metadata via pull-through helpers
  (`self.items[self.instances[id].item].<field>`);
  source `#[rseq::*]` attributes always attach to
  source-level entities (Items, fields, spans), never
  to post-mono Instances. An `instance_attrs` table
  would duplicate data, not add capability.
- B3 (a) is sufficient because upstream Item-walk
  order (Phase 2 of stage-12 D-12-5's canonical
  pipeline) is already deterministic. Canonical sort
  is an option to revisit if bridge-implementation
  churn surfaces stability issues; not needed in v1.
- B4 (a) is forced by B1 (b): unified discriminated-
  union storage doesn't have type-system-enforced
  one-per-key slots; validator handles it. The
  precedent is stages 5-11's walk-implementation-
  deferred invariants registry pattern (per stage-11
  D-11-6 joint visitor).
- B5 (b) makes explicit what stage-12 was drawing
  implicitly via D-12-1.3 (typed Item fields for
  rustc-derived metadata) + D-12-1.5 (DomainAttr
  forward-pointer for `#[rseq::*]`-derived metadata).
  Explicit binding rule prevents future drift toward
  "this metadata could go either way" debates per
  domain family.
- B6 (a) makes the target-table mapping discoverable
  in each phase-2 stage's spec section. Stage 14
  commits stage-14's mapping (`Intrinsic` ->
  `item_attrs`); phase-2 stages commit theirs at
  their own merger close.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* --
  the `AttrData` discriminated union composes cleanly;
  per-family typed tables would create N parallel
  storage mechanisms.
- *Compiler-theory-first* -- sibling-table storage for
  sparse per-key metadata is standard compiler design;
  the project's existing typed-Item-fields surface
  (D-12-1.3) handles the dense per-key case via direct
  Item fields.
- *Spec is mutable* -- B2 (b) refines stage-4 §1287-1299's
  4-key-kind enumeration into 3 actual tables based on
  the stage-12 D-12-1.8 evidence. Stage 4's framing was
  a candidate inventory, not a locked commitment.
- *Forward expectations live in `stage-N/expectations.md`*
  -- B6 (a) makes the per-family target-table mapping
  a stage-N commitment; phase-2 stages file inbound
  expectations referencing the architecture defined
  here.

**Prior-art evidence.**
- `spec-snapshot.md` §3.5.2 (D-12-1.8) -- Instance-side
  pull-through helpers; 13 existing entries; the
  established access pattern that B2 (b) relies on.
- `spec-snapshot.md` §3 (RegseqModel root) lines
  326-334 -- the stage-4 §1287-1299 forward-pointer in
  its current location; the inventory B2 (b) refines.
- `spec-snapshot.md` §8.0 -- D-12-1.5 + Finding #1
  territory (the typed-Item-fields paragraph followed by
  the operations-attribute forward-pointer at the close
  of §8.0); the typed-Item-field-vs-DomainAttr line that
  B5 (b) formalizes.
- `spec-snapshot.md` `regseq_instance_kind_rank`
  (stage-1 R5.1 / R5.2) -- precedent for append-only
  enum ordering that B3 + the `AttrData` rank inherit.
- `spec-snapshot.md` §15.1 rejection table (stage-3 +
  stages 5-11) -- precedent for validator-enforced
  invariants registries that B4 (a) follows.

**Spec cross-reference.**
- `spec-snapshot.md` §3 (RegseqModel root) -- gains
  new field `domain_attrs: DomainAttrTables`.
- `spec-snapshot.md` §3.X (new "Domain attribute tables"
  subsection) -- hosts the full architecture
  specification.
- `spec-snapshot.md` §3.5.2 (Instance-side pull-through
  helpers) -- documentation note that per-family
  pull-through helpers (like `instance_intrinsic`) extend
  this inventory as phase-2 stages land.
- `spec-snapshot.md` §3 root comment lines 326-334 --
  in-place edit at merger close to reflect "three
  sibling tables" rather than "four key kinds."

**Sub-decisions.**

#### D-14-3.0 -- Unified DomainAttrTables vs per-family typed tables [LOCKED]

**Decision.** Unified `DomainAttrTables` with a
discriminated `AttrData` enum:

```rust
pub struct DomainAttrTables {
    pub item_attrs:  IndexVec<ItemId, Vec<AttrData>>,
    pub field_attrs: HashMap<FieldPath, Vec<AttrData>>,
    pub span_attrs:  HashMap<Span, Vec<AttrData>>,
}

pub enum AttrData {
    Intrinsic(IntrinsicKind),  // stage 14
    // phase-2 stages append variants per D-14-3.2
}
```

**Reasoning.** Per the D-14-3 Aspect B1 rationale: each
phase-2 stage adds one `AttrData` variant rather than
extending `RegseqModel`'s root schema; uniformity for
consumers walking domain attributes; storage
consolidation at the architectural level. The
per-family-typed-tables alternative (B1 (a)) was
considered and rejected -- the type-safety win is small
(one match-arm per consumer) and the uniformity loss
is real.

#### D-14-3.1 -- Three sibling tables; `instance_attrs` dropped [LOCKED]

**Decision.** `DomainAttrTables` instantiates three
sibling tables -- `item_attrs`, `field_attrs`,
`span_attrs`. `instance_attrs` is dropped from the
stage-4 §1287-1299 forward-pointer enumeration.

**Reasoning.** Stage-12 D-12-1.8's §3.5.2 Instance-side
pull-through pattern handles Instance access to Item-
level metadata via
`self.items[self.instances[id].item].<field>`. Source
`#[rseq::*]` attributes always attach to source-level
entities (Items, fields, spans); generic instantiations
inherit Item-level attributes via the pull-through
helper. A separate `instance_attrs` table would be a
second copy of the same data.

Per-Instance state that genuinely depends on the
monomorphization context (e.g., `virtual_idx` on
`InstanceKind::Function` per stage-9 D-9-3 / stage-12
D-12-1.4) lives inline in `InstanceKind`, not in a
sibling table.

If a future need surfaces for per-Instance bridge-
computed domain attributes, "Spec is mutable" applies:
the architecture can be extended at that point. v1
commits to three tables.

**Spec impact.** The stage-4 §1287-1299 forward-pointer
text in `spec-snapshot.md` §3 (lines 326-334) gets an
in-place edit at merger close to reflect three sibling
tables.

#### D-14-3.2 -- `AttrData` append-only ordering [LOCKED]

**Decision.** The `AttrData` enum variant ordering is
append-only. Stage 14 fixes the initial variant set:

```rust
pub enum AttrData {
    Intrinsic(IntrinsicKind) = 0,
    // phase-2 stages append at discriminant 1, 2, 3, ...
}
```

Phase-2 stages append new variants at the end; existing
discriminants remain immutable so serialized RegseqModels
stay stable across stage closures. This mirrors the
existing `regseq_instance_kind_rank` pattern (stage-1
R5.1 / R5.2) and the `IntKind` / `UintKind` / `FloatKind`
append-only commitments (stage-3 §10).

**Reasoning.** F4 / F4-2 cross-build identity requires
stable serialized form. Stage-1's append-only enum
discipline is already the project's norm for closed
enums extended across stages.

**Spec impact.** The `AttrData` enum definition in
§3.X carries an explicit append-only ordering note,
parallel to §10's primitive-scalar-discriminants note.

#### D-14-3.3 -- `Vec<AttrData>` insertion-order storage [LOCKED]

**Decision.** Entries in each `Vec<AttrData>` are stored
in insertion order. The order is established during Phase
2 recognition (per the per-family caches in §3.6.4) and
materialised verbatim into the final sibling-table
`Vec`s during Phase 4 -- the Phase-4 materialisation pass
walks the per-family Phase-2 caches in their deterministic
encounter order and pushes onto the destination `Vec` in
that same order. The bridge thus produces a deterministic
final ordering on `domain_attrs.{item_attrs, field_attrs,
span_attrs}` without re-sorting at Phase 4.

**Reasoning.** Insertion order is sufficient given that
upstream walk order is deterministic. Canonical sort by
`AttrData` variant rank + payload identity (option B3 (b))
was considered and deferred -- it's more robust to
bridge-implementation churn but adds work without solving
any v1 problem. Note that R1's BLOCK fix split recognition
(Phase 2) from materialisation (Phase 4); this decision
preserves the Phase-2-deterministic encounter order
through to the Phase-4-populated final sibling tables,
not the other way around.

**Spec impact.** §3.6.2 carries an invariant: "Within a
single `Vec<AttrData>`, entries appear in the order they
were encountered by the Phase-2 per-family recognition
walk; the Phase-4 materialisation pass preserves this
order when populating the destination `Vec`. Walk-order
determinism is inherited from stage-12 D-12-5 Phase 2."

#### D-14-3.4 -- Per-family invariants validator-enforced [LOCKED]

**Decision.** Per-family invariants over
`Vec<AttrData>` are enforced by `RegseqModel::validate`,
not by the type system. Stage 14 commits the first
invariant:

- **I-DomainAttr-Intrinsic-Singleton.** For any
  `ItemId id`, the set
  `domain_attrs.item_attrs[id]
  .iter()
  .filter(|attr| matches!(attr, AttrData::Intrinsic(_)))`
  has cardinality at most 1.

Phase-2 stages specify analogous per-family invariants
for their own `AttrData` variants (e.g., stage 16 might
commit at-most-one `AttrData::Mmio` per `FieldPath`).

**Reasoning.** Unified discriminated-union storage
doesn't have type-system-enforced one-per-key slots;
validator handles it. The precedent is stages 5-11's
walk-implementation-deferred invariants registries.

**Spec impact.** §3.6 includes the I-DomainAttr-*
invariant family; §3.3 Phase B step 10 walks
`domain_attrs` and enforces the family.

#### D-14-3.5 -- Attribute-namespace storage rule [LOCKED]

**Decision.** Explicit binding rule for where
attribute-derived metadata lands:

| Source attribute namespace | Storage |
| --- | --- |
| `#[rseq::*]` tool-namespace custom attributes | `DomainAttrTables` sibling-table `AttrData` entry |
| Recognized Rust attributes (`#[inline]`, `#[link_section]`, `#[no_mangle]`, etc.) and language built-ins (lang-items) | Typed Item field per D-12-1.3 |

Consequences:

1. **For phase-2 stage authors**: when introducing a new
   domain family, the per-family attribute payload
   (`MmioAttr`, `InterruptAttr`, etc.) is always a
   sibling-table `AttrData` variant -- never a typed
   Item field. The choice is settled by the namespace,
   not by the family's semantics.

2. **For future Rust-attribute admissions**: if v1
   later admits, say, `#[cold]` or `#[target_feature(...)]`,
   the metadata lands as a new typed Item field per
   D-12-1.3 -- never in `DomainAttrTables`.
   (External-review Finding #2's `#[naked]` rejection
   is consistent: the field was on the typed-Item-fields
   side; the rejection removes it; no migration to
   `DomainAttrTables`.)

**Reasoning.** Stage 12 D-12-1.3 (typed Item fields for
recognized Rust attributes) + D-12-1.5 (DomainAttr
forward-pointer for `#[rseq::*]`-derived metadata)
implicitly drew this line; stage 14 makes it explicit
to prevent future drift toward "either-side" debates.
The retroactive consistency check holds: every existing
typed Item field (`linkage`, `link_section`,
`inline_hint`, `is_unsafe`, `is_phantom_data`,
`is_unsafe_cell`, `is_manually_drop`) derives from a
Rust attribute or language built-in -- none from
`#[rseq::*]`.

**Spec impact.** §3.X carries the binding rule as a
named subsection. The §8 area gains a forward-pointer
comment summarizing the rule with a back-reference to
§3.X.

#### D-14-3.6 -- Per-family target-table specification [LOCKED]

**Decision.** Each phase-2 stage's spec section MUST
name which sibling table (`item_attrs`, `field_attrs`,
`span_attrs`) its `AttrData` variant populates. Stage
14 commits the first mapping:

| AttrData variant | Target sibling table | Source attribute |
| --- | --- | --- |
| `Intrinsic(IntrinsicKind)` | `item_attrs` | `#[rseq::intrinsic("NAME")]` on a function Item |

Anticipated phase-2 entries (not binding; phase-2
stages own the actual commitments):

| AttrData variant | Target sibling table | Source attribute |
| --- | --- | --- |
| `Mmio(MmioAttr)` (stage 16) | `field_attrs` | `#[rseq::mmio(...)]` on register-bank fields |
| `Interrupt(InterruptAttr)` (stage 17) | `item_attrs` | `#[rseq::interrupt(...)]` on handler function Items |
| `Atomic(AtomicAttr)` (stage 18) | (TBD by stage 18) | `#[rseq::*]` markup on atomic-resource fields/items |

**Reasoning.** Without an explicit per-family mapping,
the bridge population codepath would need to infer the
target table from the `AttrData` variant's payload type
(e.g., a `FieldPath`-carrying variant lands in
`field_attrs`). That works for variants whose payload
carries the key kind, but not for variants whose
payload is keyed externally (e.g., `Intrinsic(IntrinsicKind)`
doesn't carry an `ItemId` -- the key is determined by
the bridge's discovery context). Explicit per-stage
mapping avoids the inference ambiguity.

**Spec impact.** §3.X carries the stage-14 mapping
table; phase-2 stages extend the table at their merger
close. The mapping is part of the per-family attribute
spec, not part of the architecture spec itself.

---

### D-14-4 -- `render_const` composer renderer [LOCKED]

**Question.** Stage-13 D-13-4.6 locked the
building-block-vs-composer architectural division
(stage 13 owns building blocks `render_ty` / `render_item`
/ body-level renderers; stage 14 owns composers
`render_const` / `render_instance` / top-level `render`).
Stage-4 D-4-K deferred the per-arm rendering specs for
the closed 11-variant `Const` enum into stage 14
explicitly. D-14-4 closes that deferral: it specifies
the per-arm rendering rules across the 11 `Const`
variants (`Bool`, `Char`, `Int`, `Uint`, `Float`, `ZST`,
`Bytes`, `Aggregate`, `EnumValue`, `NullPtr`, `Reloc`)
per `spec-snapshot.md` §9.7. The pretty-printer-side
spec lands in `pretty-printer-snapshot.md` (the
consumer-side merger artifact per D-13-9 spec split);
the structural `Const` enum itself stays in `spec.md`
§9.7.

The cluster covers 12 sub-decisions corresponding to
12 aspects of the composer's contract: the API
signature (load-bearing, gates the others); the
scalar-variant rendering shape; per-variant dispatch
rules for the type-context-dependent variants
(`ZST` / `Aggregate` / `EnumValue` / `Bytes` /
`NullPtr` / `Reloc`); the recursion mechanism for
nested children; and three policy-shaped sub-decisions
(sentinel collapsing, hex admission, float
infinity / NaN representation) that decide RegseqModel
conventions where rustc evidence offers a default.

D-14-4 is **the first of three composer-renderer
clusters** (D-14-4 / D-14-5 / D-14-6). Closure of all
three completes stage 14's owned rendering surface and
unblocks the per-stage cross-cluster Codex review at
stage close.

**Options considered.**

Aspect A1 (API signature):
- (a) `render_const(ConstId, TyId) -> String` -- pair
  form mirroring rustc's
  `pretty_print_const_value_tcx(tcx, ct, ty, fmt)` at
  `rustc_middle/src/mir/pretty.rs:1851`. The consumer's
  `TyId` is required for `Bytes` / `Aggregate` /
  `ZST` / `NullPtr` / `Reloc` arms whose rendering
  shape depends on type, not const.
- (b) `render_const(ConstId) -> String` -- Ty-less form.
  Forces a type-erased rendering: `Bytes` always as raw
  bytes; `Aggregate` always as `[child, ...]` regardless
  of struct / tuple / array shape. Loses fidelity vs the
  existing specimen contract.
- (c) `render_const(ConstId, &Ty) -> String` -- borrowed
  `Ty`. Inconsistent with stage-13's `render_ty(TyId)` /
  stage-12's `render_def_path(ItemId)` -- both take ID
  forms; the `PrettyPrinter` has `&model` and resolves
  internally.

Aspect A2 (scalar variants: `Bool` / `Char` / `Int` /
`Uint` / `Float`):
- (a) Decimal value + rustc-style underscore-prefixed
  width suffix: `42_i32`, `42_u32`, `42_usize`,
  `'a'` (char Debug), `1.5f32`, `0f64`.
- (b) C-style suffix without underscore (mixing in the
  current specimen contract at `pretty-printer-snapshot.md:1204
  / 1212 / 1226 / 1287`): `42i32`, `42u32`, `16usize`.
- (c) Source-faithful style honoring the original Rust
  literal form (decimal / hex / octal / binary
  preserved). Requires bridge to carry the source form,
  which `Const` does not preserve.

Aspect A3 (`ZST` per-arm rendering):
- (a) Consumer-Ty-driven dispatch: `()`, `<path>` (for
  fieldless struct / `PhantomData` / `FnDef`), with
  bridge-rejection-shaped fallback for unsupported Ty
  shapes.
- (b) Always render `<zst>` placeholder regardless of
  consumer Ty. Loses information; not user-pretty.
- (c) Always render the consumer's path (via
  `render_def_path` / `render_ty`). Doesn't cover
  `Ty::Tuple(_)` unit case naturally.

Aspect A4 (`Aggregate` per-arm dispatch):
- (a) Dispatch on consumer `Ty.kind()`:
  `Ty::Array(_)` -> `[c1, c2, ...]`;
  `Ty::Tuple(fields)` -> `(c1, c2, ...)` (`(c1,)` for
  len 1); `Ty::Struct { layout, .. }` -> unit / tuple /
  named-field forms per the struct's ctor shape.
  Mirrors rustc's `mir/pretty.rs:1907-1966`.
- (b) Structural `Aggregate([c1, c2, ...])` form
  regardless of consumer Ty. Type-erased; loses
  fidelity.
- (c) Reuse `render_rvalue`'s `AggregateKind` per-arm
  shapes from `pretty-printer-snapshot.md` §7.3.2. Same
  surface form as (a); just refactors where the
  dispatch lives.

Aspect A5 (`EnumValue` per-arm rendering):
- (a) Variant-shape dispatch via
  `Ty::Enum.layout.kind`:
  - unit variant -> `<enum_path>::<variant_name>`
  - tuple variant ->
    `<enum_path>::<variant_name>(c1, c2, ...)`
  - named-field variant ->
    `<enum_path>::<variant_name> { f1: c1, f2: c2 }`
  Variant name lookup via `EnumShape.Single.variant`
  or `EnumShape.Multiple.variants[variant_idx]` per
  `pretty-printer-snapshot.md` §3.4.
- (b) Always render `<enum_path>::<variant_name>(c1, c2)`
  regardless of variant shape (treat all variants as
  tuple-style). Loses Rust idiom.
- (c) Render via `discriminant: payload` form (raw
  discriminant value + bracketed payload). Not a Rust
  literal form; diagnostic-only.

Aspect A6 (`Bytes` per-arm dispatch):
- (a) Consumer-Ty-driven dispatch, mirroring rustc's
  special cases at `mir/pretty.rs:1874-1898`:
  - All-uninit shortcut: `<uninit>` (rustc-mirrored).
  - `Ty::Ref(_, Ty::Str)` all-Value bytes -> `"hello"`
    (UTF-8 decode, lossy).
  - `Ty::Ref(_, Ty::Slice(u8))` all-Value bytes ->
    `b"hello"`.
  - `Ty::Array(u8, N)` all-Value bytes -> `*b"hello"`
    (rustc deref form).
  - else / mixed Uninit-Value-Reloc -> structural
    fallback `bytes![0x00, 0x01, <uninit>, <reloc:path>]`
    (RegseqModel-specific; rustc has only a debug
    fallback here, but RegseqModel's `Byte` three-state
    form gives full structural access).
- (b) Always render `bytes![...]` structural form
  regardless of consumer Ty. Misses the `"hello"` /
  `b"hello"` ergonomic shapes for the common cases.
- (c) Render raw bytes only (no consumer-Ty special
  cases). Same loss as (b).

Aspect A7 (`NullPtr(TyId)` per-arm):
- (a) `0x0 as <render_ty(TyId)>` -- rustc-mirrored
  typed-cast form per `ty/print/pretty.rs:1816-1826`.
- (b) `null::<TyId>` -- function-style form. Not a Rust
  expression literal; harder to read.
- (c) `<null>` placeholder. Loses pointee type
  information.

Aspect A8 (`Reloc(InstanceRef)` per-arm):
- (a) Resolved-Item-kind dispatch:
  - Function ref with `Ty::FnDef` / `Ty::FnPtr` consumer
    -> `<render_def_path(InstanceId)>` (no `as` cast --
    function paths name values directly; matches
    rustc's `pretty_print_const_scalar_ptr` FnPtr arm).
  - Static / vtable / synth `ConstMemoryStatic` with
    pointer consumer Ty -> `<render_def_path> as
    <render_ty(TyId)>`.
- (b) Always `<render_def_path> as <render_ty(TyId)>`
  regardless of resolved-Item-kind. Adds redundant
  cast on Function refs.
- (c) Always bare `<render_def_path>`. Loses pointer
  type information for Static / vtable / synth refs.

Aspect A9 (type-context threading for recursive
children):
- (a) Caller resolves per-field types from parent Ty's
  layout and passes per-child Ty in (mirrors rustc's
  `try_destructure_mir_constant_for_user_output`
  returning `(child ConstValue, child Ty)` pairs).
  `PrettyPrinter` has `&model` and walks layouts
  internally.
- (b) Renderer takes a `parent_ty` and walks the parent
  layout itself. Couples renderer to layout-walking
  logic.
- (c) `ConstId` carries its own Ty as a side-table
  entry. Requires bridge to populate a parallel
  `const_tys` table; redundant with consumer-context
  Ty.

Aspect A10 (sentinel collapsing for `Int` / `Uint`):
- (a) Match rustc: collapse `T::MIN` / `T::MAX` at
  exact-bits equality across all primitive widths +
  `usize` / `isize`.
- (b) Decimal-only; no sentinel collapsing. Mask
  constants like `0xFFFF_FFFF` always render as
  `4294967295_u32` (verbose).
- (c) Selective collapsing for specific widths
  (e.g., u8/u16 only). Arbitrary.

Aspect A11 (integer hex form admission):
- (a) Decimal-only. Matches rustc convention; revise
  existing `0xFFu8` specimen at
  `pretty-printer-snapshot.md:1241` to `255_u8`.
- (b) Hex form admitted with `0x` prefix
  (`0xFFu8`, `0xFFFF_u16`, etc.). Bridge would need
  to carry source-form hint or auto-detect from value
  size, neither of which is currently in Const.
- (c) Add `PrettyPrintOptions::integer_hex: bool` for
  caller-controlled toggle. Future work; out of v1
  scope.

Aspect A12 (float infinity / NaN representation):
- (a) Rustc-mirrored underscore-broken form:
  `inf_f32`, `NaN_f32`, `-inf_f32`. Parses
  unambiguously and is diagnostically useful.
- (b) Function-call form: `f32::INFINITY`,
  `f32::NAN`. Names sentinel constants from the
  standard library. Less compact.
- (c) Hex bit-pattern form: `0x7f800000_f32` for inf.
  Diagnostically opaque.

**Decision.** All 12 sub-decisions LOCKED at Phase A
between 2026-05-13 and 2026-05-13 in a single drafting
session. The locking order pinned by the Phase A
drafting plan was:

1. ~~D-14-4.1~~ (API signature `render_const(ConstId,
   TyId) -> String`) -- the load-bearing gate
   constraining all consumer-Ty-dependent arms.
2. ~~D-14-4.2~~ (scalar variants Bool / Char / Int /
   Uint / Float -- underscore-decimal convention).
3. ~~D-14-4.10~~ (`Int` / `Uint` sentinel collapsing).
4. ~~D-14-4.11~~ (integer hex form admission:
   decimal-only for v1).
5. ~~D-14-4.12~~ (`Float` infinity / NaN
   representation -- closes scalar-variant surface).
6. ~~D-14-4.3~~ (ZST per-arm: 7 consumer-Ty
   categories with composite-ZST recursion).
7. ~~D-14-4.7~~ (NullPtr per-arm: typed-cast form
   across 5 thin-pointer consumer Ty kinds).
8. ~~D-14-4.8~~ (Reloc per-arm: Function /
   FnPtr-coercion / Static / ConstMemoryStatic
   b"..." special case).
9. ~~D-14-4.4~~ (Aggregate per-arm: Array / Tuple /
   Struct structural + `&str` / `&[u8]` fat-pointer
   special cases).
10. ~~D-14-4.5~~ (EnumValue per-arm:
    `EnumShape::Single` / `Multiple` variant dispatch).
11. ~~D-14-4.6~~ (Bytes per-arm: all-uninit shortcut
    + `*b"..."` byte-array case + `bytes![...]`
    structural fallback).
12. ~~D-14-4.9~~ (type-context threading +
    operand-callsite wiring; REOPENS stage-13
    D-13-6's `render_operand` signature).

All sub-decisions are individually LOCKED below; the
main cluster entry is LOCKED with this aggregated
status.

**Methodology rules invoked** (cluster-level; named per
rule per `../README.md` §2).

- *C semantics with information preservation*. The
  consumer-Ty-driven dispatch (A1 (a)) preserves the
  type information rustc gives us for free; the
  alternative (Ty-less rendering, A1 (b)) discards
  information that's load-bearing for fidelity.
- *Spec is mutable*. The specimen inconsistency in
  `pretty-printer-snapshot.md` (mixing `42_i32` and
  `1u8` / `42i32` forms) reflects pre-grounded
  drafting; D-14-4.2 lock revises the specimens
  in place to the rustc-mirrored underscore-decimal
  form.
- *Compiler-theory-first*. Rustc's per-variant
  rendering forms are well-established compiler
  pretty-printing conventions (e.g., underscore-broken
  non-finite floats parse unambiguously; sentinel
  collapsing for `T::MIN` / `T::MAX` mirrors source
  literals); RegseqModel adopts them rather than
  inventing parallel forms.
- *No defensive fallbacks*. The toolchain is pinned;
  the per-variant rustc forms are stable. No
  alt-renderings or "what if rustc changes" fallbacks.
- *Code does not reference discussion-folder paths*.
  Stage-14 introduces new method signatures and per-arm
  rules with no in-code citations to
  `pretty-printer.md` / `spec.md` paths; rules restate
  inline where load-bearing or reference D-tag
  identifiers.

**Prior-art evidence.** Full rustc-evidence pass at
`prior-art/rustc-const.md` (Q1-Q7 covering API
signature, per-variant rustc forms, recursion shape,
operand-wrapper layering). 8 file-line landmarks
appended to `../rustc-references.md` under a new
"Const rendering (MIR pretty-printer)" section
(`pretty_print_const_value_tcx` at
`rustc_middle/src/mir/pretty.rs:1851`;
`pretty_print_const_scalar_int` at
`rustc_middle/src/ty/print/pretty.rs:1776`;
`ConstInt` Debug at
`rustc_middle/src/ty/consts/int.rs:51-138`; etc.).
Stable-IR analog (`rustc_public/src/mir/pretty.rs:339-345`)
delegates back through `tcx.mir_const_pretty()` rather
than reimplementing dispatch, so it carries no
independent prior-art weight; Charon / mrustc
consultations deferred per the §2 per-stage framing
rule, triggered inline during D-14-4.4 (Aggregate
dispatch) / D-14-4.6 (Bytes dispatch) drafting if
those sub-decisions surface design ambiguity not
settled by rustc evidence alone.

**Spec impact** (cluster-level; per-sub-decision
specifics land as each sub-decision locks).

- New section in `pretty-printer-snapshot.md` (location
  TBD during drafting -- `§6` expansion / new `§6.5` /
  new top-level `§10` are the candidate placements).
  Hosts the `render_const` API signature, per-arm
  rendering rules, recursion mechanism, and policy
  conventions (sentinel collapsing, hex admission,
  float non-finite).
- `pretty-printer-snapshot.md` §8 per-method ownership
  table line 943: existing `render_const(ConstId)` row
  updates to `render_const(ConstId, TyId)` per D-14-4.1
  signature lock. Same row's "Inputs" / "Description"
  columns update accordingly.
- `pretty-printer-snapshot.md` §3.3 sigil conventions
  line 230: existing `ConstId` row updates to note the
  TyId-pair input convention.
- `pretty-printer-snapshot.md` §9 worked-example
  specimens at lines 1140 / 1152 / 1204 / 1212 / 1226 /
  1241 / 1287: existing specimens revised in place
  during D-14-4.2 lock to the underscore-decimal form
  (`1u8` -> `1_u8`, `42i32` -> `42_i32`, `42u32` ->
  `42_u32`, `16usize` -> `16_usize`, `0xFFu8` ->
  `255_u8`).
- `pretty-printer-snapshot.md` §9 new specimens: ~5-10
  added across the per-arm rendering rules per D-13-7
  conventions. Each Const variant gets ≥1
  specimen at the per-arm spec level (per D-13-7);
  dispatch-arm variants (D-14-4.4 / D-14-4.5 /
  D-14-4.6) get ≥1 specimen per arm.
- No `spec.md` (structural-spec) edits expected -- the
  `Const` enum itself is locked at stage 4 D-4-K;
  D-14-4 only touches the rendering surface.

**Cross-stage friction.** **D-13-6 REOPENED** by
D-14-4.9 with a **three-signature cascade** (expanded
per R1 finding -- the initial single-signature REOPEN
was incomplete; `render_rvalue` lacked the context
for `Rvalue::Use`'s consumer Ty, and
`render_generic_args` lacked access to the enclosing
Item's generic-parameter list):

- `render_operand(&Operand, &Body) -> String` ->
  `render_operand(&Operand, &Body, expected_ty: TyId)
  -> String`.
- `render_rvalue(&Rvalue, &Body) -> String` ->
  `render_rvalue(&Rvalue, &Body, expected_ty: TyId)
  -> String`.
- `render_generic_args(&GenericArgs) -> String` ->
  `render_generic_args(&GenericArgs, item: ItemId)
  -> String`.

The `expected_ty` extensions thread the
consumer-context Ty through `render_statement` Assign
-> `render_rvalue(_, _, place_ty)` ->
`render_operand(_, _, derived_operand_ty)` ->
`render_const(id, expected_ty)` per D-14-4.1's
pair-form. The `item` extension lets
`render_generic_args` resolve the declared type of
`GenericArg::Const(c)` from the enclosing Item's
generic-parameter list. The extensions propagate to
all `render_operand` callers (§7.4 / §7.6 / §7.7 /
§7.9 per-arm specs), to `render_rvalue` callers
(§7.4 Assign), and to `render_generic_args` callers
(§6.4 ADT type rendering + §10.1.* composer-renderer
callsites). Flip in `decisions.md` at merger close
with annotation "(revised in stage 14 D-14-4.9:
`render_operand` and `render_rvalue` signatures
extended with `expected_ty: TyId`;
`render_generic_args` signature extended with
`item: ItemId`)". Stage-13 D-13-6's deliberate
divergence from rustc's `Display for ConstOperand`
(no `const ` prefix at the operand layer) is
preserved -- D-14-4.9 only extends signatures for
type threading; the value-level form produced by
`render_const(id, expected_ty)` remains bare (no
`const ` prefix). See D-14-4.9 below for the full
audit.

**Cross-stage friction with §9.7 bridge dispatch.**
Per R1.3 finding, the bridge gains an invariant: when
producing `Const::Aggregate([Reloc, Uint])` for a
fat-pointer consumer Ty per §9.7's `Slice { data,
meta }` dispatch + `Ref(_, dyn Trait)` synthesis
paths, the bridge MUST ensure both
`Ty::Ptr(pointee, Mutability::Not)` and
`Ty::Uint(UintKind::Usize)` are present in
`model.types`. The invariant lands at
`spec-snapshot.md` §9.7's reference-typed-const-handling
subsection and is consumed by the renderer at
D-14-4.9 §10.1.8 fat-pointer fallback recursion.
No separate D-tag REOPEN since the invariant lives
within stage-9 §9.7's existing dispatch area.

**Sub-decisions.**

#### D-14-4.1 -- API signature: `render_const(ConstId, TyId) -> String` [LOCKED]

**Decision.** The composer renderer takes a pair
`(ConstId, TyId)`:

```rust
impl PrettyPrinter<'_> {
    pub fn render_const(&self, id: ConstId, ty: TyId) -> String;
}
```

The `TyId` is the **consumer-context type** at the use
site -- the type of the position holding the `ConstId`:

- `InstanceKind::Static { ty, init }` -> `init.unwrap()`
  is rendered with `ty` as the consumer type.
- `InstanceKind::Const { ty, value }` -> `value` is
  rendered with `ty` as the consumer type.
- Aggregate / EnumValue recursion -> each child
  `ConstId` is rendered with its per-field Ty resolved
  from the parent Ty's layout (per D-14-4.9, to be
  locked).
- `Operand::Constant(id)` at a Call site (per
  `pretty-printer-snapshot.md` §7.2) -> the operand's
  Ty is supplied by the surrounding Statement / Rvalue
  / Terminator context that stage-13 D-13-6 already
  threads.

The renderer's contract: given the pair, return the
Rust-source-style rendered string for the const value.
Per-variant rendering rules are pinned by D-14-4.2
through D-14-4.8 (in subsequent sub-decision locks).

**Reasoning.** Three load-bearing reasons drive A1 (a)
over the alternatives:

1. **Rustc-mirroring is forced for fidelity.** Several
   per-variant arms (`Bytes` -> `"hello"` vs `b"hello"`
   vs `*b"hello"`; `Aggregate` -> `[...]` vs `(...)` vs
   `Path { f: v }`; `ZST`; `NullPtr`; `Reloc`) dispatch
   on the consumer's `Ty.kind()`, not on the const
   itself. Rustc's `pretty_print_const_value_tcx(tcx,
   ct, ty, fmt)` at `rustc_middle/src/mir/pretty.rs:1851`
   takes the pair for the same reason. Without the
   `TyId`, RegseqModel would either render type-erased
   (losing fidelity) or carry redundant type
   side-tables (information duplication). The pair form
   is the cleanest realisation of *C semantics with
   information preservation*.
2. **Stage-4's existing rule pins the convention.**
   `spec-snapshot.md` §9.7 line 2861 already says
   explicitly: "Type comes from the consumer's context
   (the owning Item's `ty` or the InstanceKind's
   payload)." The pair-form signature is the natural
   API realisation of that rule.
3. **ID-form consistency with stage-13 / stage-12
   building blocks.** `render_ty(TyId)` (stage-13
   D-13-5) and `render_def_path(ItemId)` (stage-12
   D-12-6) take ID forms (not borrowed references). The
   `PrettyPrinter` has `&model` and resolves IDs
   internally. `render_const(ConstId, TyId)` follows
   the same convention. A1 (c) (borrowed `&Ty`) would
   introduce a stylistic inconsistency for no upside.

The existing `pretty-printer-snapshot.md` specimen
contract at lines 1140 (`Operand::Constant(<ConstId
for 42_i32>) -> 42_i32`) already anticipates the
type-aware rendering: the suffix `_i32` is the
type information; a Ty-less renderer would either drop
the suffix or carry a side-table copy. The lock here
makes the pair form explicit.

**Spec impact.** Pinned at `pretty-printer-snapshot.md`
§8 per-method ownership table: existing row
`render_const(ConstId)` updates to
`render_const(ConstId, TyId)` with Inputs column
`ConstId, TyId` and Description column noting "per-arm
Const value (composes `render_ty` for type-context
arms; pair-form mirrors rustc per-variant dispatch)."
The §3.3 sigil convention row for `ConstId` updates to
note the TyId-pair input convention. The §1.3 composer
listing at the section header updates the
`render_const` signature line accordingly.
The §6.7 `render_generic_args` per-arg dispatch line
for `GenericArg::Const(c)` updates to pair-form
`render_const(c, ty)`, with the parameter's declared
`TyId` resolved from the enclosing Item's
generic-parameter list.

**Stage-13 wiring deferred to D-14-4.9.** The §7.2
`render_operand` per-arm table row for
`Operand::Constant(id)` (line 661 -> `<render_const(id)>`)
is **not edited at this sub-decision**. Stage-9 §20.1
locks `Operand::Constant(ConstId)` without a per-operand
TyId, so threading the consumer Ty for the operand
wrapper is a callsite-wiring question that the
`render_rvalue` / `render_statement` /
`render_terminator` callers must answer: they have the
type context available (rvalue expected type, Call
argument-type table) and supply it on the call to
`render_operand`. Pinning the exact wiring shape is
**D-14-4.9's territory** (extends the original
"type-context threading for recursive children"
scope to include operand-callsite wiring uniformly).
Until D-14-4.9 locks, the §7.2 line stays as
`<render_const(id)>` (one-arg form) -- a
transient-inconsistency-flagged-here-for-traceability;
the §3.3 sigil-table line takes the pair-form
authoritative wording.

**Worked example.**

Concrete dispatch sketch (illustrative; per-arm
specs land in D-14-4.2 through D-14-4.8):

```text
render_const(ConstId, TyId) -> String:
  let c = model.consts[ConstId];
  let ty_kind = model.types[TyId].kind();
  match (c, ty_kind) {
    // D-14-4.2 (scalar variants):
    (Const::Bool(b), _)            => format!("{b}"),
    (Const::Int(k, v), _)          => render_int(k, v, signed=true),
    (Const::Uint(k, v), _)         => render_int(k, v, signed=false),
    (Const::Float(k, bits), _)     => render_float(k, bits),
    (Const::Char(c), _)            => render_char(c),
    // D-14-4.3 (ZST):
    (Const::ZST, Ty::Tuple([]))    => "()".into(),
    (Const::ZST, Ty::Struct{..})   => self.render_def_path(item),
    (Const::ZST, Ty::FnDef(i, _))  => self.render_def_path(item_of(i)),
    // D-14-4.4 / D-14-4.5 (Aggregate / EnumValue):
    (Const::Aggregate(cs), Ty::Array(_, _))  => render_array(cs),
    (Const::Aggregate(cs), Ty::Tuple(_))     => render_tuple(cs),
    (Const::Aggregate(cs), Ty::Struct{..})   => render_struct(cs),
    (Const::EnumValue{var, pl}, Ty::Enum{..})=> render_enum(var, pl),
    // D-14-4.6 (Bytes):
    (Const::Bytes(bs), Ty::Ref(_, Ty::Str))     => render_str(bs),
    (Const::Bytes(bs), Ty::Ref(_, Ty::Slice_u8))=> render_bytes_slice(bs),
    (Const::Bytes(bs), Ty::Array(u8, _))        => render_bytes_array(bs),
    (Const::Bytes(bs), _)                       => render_bytes_fallback(bs),
    // D-14-4.7 (NullPtr):
    (Const::NullPtr(pty), _) => format!("0x0 as {}", self.render_ty(pty)),
    // D-14-4.8 (Reloc):
    (Const::Reloc(r), Ty::FnDef|FnPtr) => self.render_def_path(...),
    (Const::Reloc(r), _)               => format!("{} as {}", ..., ...),
  }
}
```

The illustrative pseudocode above shows the dispatch
shape only; per-arm details (forms / fallbacks / sigil
choices) are pinned by subsequent sub-decisions D-14-4.2
through D-14-4.9.

#### D-14-4.2 -- Scalar variants: `Bool` / `Char` / `Int` / `Uint` / `Float` [LOCKED]

**Decision.** The five scalar `Const` variants render
as Rust-source-style literal forms with a
rustc-mirrored underscore-decimal convention. Width is
self-determined by the variant's payload tag
(`IntKind` / `UintKind` / `FloatKind`); no external
`TyId` is consulted for the scalar arms (the consumer
`TyId` from D-14-4.1's pair-form is ignored in the
scalar dispatch path).

Per-arm forms:

| Const variant         | Rendered form                       | Notes                                                                                                  |
|-----------------------|-------------------------------------|--------------------------------------------------------------------------------------------------------|
| `Bool(false)`         | `false`                             | No suffix.                                                                                             |
| `Bool(true)`          | `true`                              | No suffix.                                                                                             |
| `Char(u32)`           | `'<ch>'` (Rust char-literal Debug)  | Quoted; Rust escape rules apply (`'\n'`, `'\\''`, `'\u{1f600}'`, etc.). Bridge has validated the `u32` is a valid Unicode scalar (`spec.md` §9.7 line 2836); invalid `u32` cannot reach the renderer. |
| `Int(I8, v)`          | `<dec>_i8`                          | `<dec>` is the sign-extended decimal value (e.g., `Int(I8, u128::MAX) -> -1_i8`).                      |
| `Int(I16, v)`         | `<dec>_i16`                         |                                                                                                        |
| `Int(I32, v)`         | `<dec>_i32`                         |                                                                                                        |
| `Int(I64, v)`         | `<dec>_i64`                         |                                                                                                        |
| `Int(I128, v)`        | `<dec>_i128`                        |                                                                                                        |
| `Int(Isize, v)`       | `<dec>_isize`                       | Pointer-sized integer; rendering does not depend on target pointer width (decimal is faithful).        |
| `Uint(U8, v)`         | `<dec>_u8`                          | `<dec>` is the zero-extended decimal value (the low `width` bits of the u128).                         |
| `Uint(U16, v)`        | `<dec>_u16`                         |                                                                                                        |
| `Uint(U32, v)`        | `<dec>_u32`                         |                                                                                                        |
| `Uint(U64, v)`        | `<dec>_u64`                         |                                                                                                        |
| `Uint(U128, v)`       | `<dec>_u128`                        |                                                                                                        |
| `Uint(Usize, v)`      | `<dec>_usize`                       |                                                                                                        |
| `Float(F16, bits)`    | `<val>f16`                          | `<val>` is the IEEE-754 binary16 decimal display of the bit-pattern; non-finite forms split out by D-14-4.12. |
| `Float(F32, bits)`    | `<val>f32`                          | Per IEEE-754 binary32.                                                                                 |
| `Float(F64, bits)`    | `<val>f64`                          | Per IEEE-754 binary64.                                                                                 |
| `Float(F128, bits)`   | `<val>f128`                         | Per IEEE-754 binary128.                                                                                |

Three policy-shaped sub-aspects split out to dedicated
sub-decisions (locks below; not yet drafted):
- **D-14-4.10** -- `Int` / `Uint` sentinel collapsing
  (`i32::MAX` / `i32::MIN` / `u32::MAX` / etc. -- the
  D-14-4.2 table above reflects the non-sentinel
  decimal form; D-14-4.10 layers sentinel collapsing
  on top).
- **D-14-4.11** -- `Int` / `Uint` hex form admission
  (the D-14-4.2 table above commits to decimal-only;
  D-14-4.11 confirms this is final-form for v1).
- **D-14-4.12** -- `Float` infinity / NaN representation
  (the `<val>` column in the table above is finite-only;
  D-14-4.12 specifies the non-finite shapes
  `inf_f32` / `NaN_f32` / `-inf_f32` etc.).

**Decimal-rendering precision** for `Int` / `Uint`: the
renderer prints the canonical decimal representation of
the value at its declared width:
- For `Int(k, v)`: re-interpret the low `width(k)` bits
  of `v` as a signed two's-complement integer of that
  width, then print using Rust's `Display` for the
  resulting signed type. The `u128` storage holds the
  sign-extended form per `spec.md` §9.7 line 2843;
  decoding is `(v as i128) >> (128 - width(k))` for
  width < 128, identity for `I128`, with `Isize`
  using the target-pointer-width column from §11
  `ScalarLayouts`.
- For `Uint(k, v)`: print the low `width(k)` bits of
  `v` as an unsigned decimal. `width(k)` is 8 / 16 /
  32 / 64 / 128 / target-pointer-width for U8 / U16 /
  U32 / U64 / U128 / Usize.

No leading-zero or digit-grouping conventions are
applied at the renderer level (rustc does not, either).
The output is a single decimal numeral followed by the
underscore-suffix.

**Char-rendering precision.** The renderer constructs a
`char` from the `u32` payload (bridge has validated
validity per `spec.md` §9.7 line 2836-2839) and emits
Rust's standard char-literal Debug form: ASCII printable
characters render literally inside single quotes
(`'a'`, `'7'`, `'?'`); the standard escapes apply
(`'\n'`, `'\t'`, `'\r'`, `'\\'`, `'\''`, `'\"'`, `'\0'`);
non-printable / non-ASCII characters render as
Unicode escape `'\u{XXXX}'` with the minimum-width
hex form. This matches rustc's
`pretty_print_const_scalar_int` Char arm at
`rustc_middle/src/ty/print/pretty.rs:1812-1814` which
delegates to `char::Debug`.

**Float-rendering precision (finite values).** The
renderer reconstitutes the IEEE-754 value from the
`u128` bit-pattern at the variant's `FloatKind` width,
then emits the apfloat-equivalent decimal display:

- `Float(F16, bits)`: load the low 16 bits as a
  `binary16` and render the shortest round-trippable
  decimal (e.g., `1.5f16`, `0f16`, `-0f16`).
- `Float(F32, bits)`: load the low 32 bits as a
  `binary32` (e.g., `1.5f32`, `3.14f32`).
- `Float(F64, bits)`: load the low 64 bits as a
  `binary64`.
- `Float(F128, bits)`: load all 128 bits as a
  `binary128`.

Integer-valued finite floats render without a fractional
part (`0f64`, `-1f32`, `42f64`) to match rustc's
apfloat-Display convention. Non-integer finite floats
render with a fractional part (`1.5f32`, `3.14f64`).
The display chooses the shortest round-trippable
decimal per IEEE-754; the renderer does not impose a
fixed precision.

**Reasoning.**

1. **Rustc-mirroring is the established compiler
   pretty-printing convention.** The underscore-decimal
   form (`42_i32`) is produced by rustc's `ConstInt`
   Debug alternate form
   (`rustc_middle/src/ty/consts/int.rs:51-138`); the
   apfloat-Display + width-suffix form (`1.5f32`) is
   produced by `pretty_print_const_scalar_int` at
   `rustc_middle/src/ty/print/pretty.rs:1787-1810`; the
   char-Debug form (`'a'`) is produced by the same
   function's Char arm at line 1812-1814. The forms
   are well-established, unambiguous (each parses as a
   Rust literal), and diagnostically familiar to any
   reader who has read rustc-emitted MIR dumps.
   *Compiler-theory-first* (README §2) -- adopt the
   established convention.
2. **Width is self-determined; no external Ty
   consultation needed for scalars.** `IntKind` /
   `UintKind` / `FloatKind` tags carry the width
   information directly (per `spec.md` §10.0 line
   3597-3625). The scalar arms can render without
   consulting D-14-4.1's `TyId` parameter -- they
   ignore it. This is the natural compiler-theory
   answer: where the tag is self-disambiguating, the
   external type context is redundant. The pair-form
   API still applies (the renderer is uniform), but
   the implementation short-circuits the TyId for
   scalar arms.
3. **Resolving the specimen inconsistency.** The
   existing specimens at
   `pretty-printer-snapshot.md:1140 / 1152 / 1204 /
   1212 / 1226 / 1241 / 1287` mix `42_i32`-style
   underscore form with `1u8` / `42i32` / `42u32` /
   `16usize` / `0xFFu8` non-underscore (and one hex)
   forms. The mix is pre-grounded drafting drift, not
   a designed convention. *Spec is mutable* (README §2)
   -- revise the specimens to the
   rustc-mirrored underscore-decimal form during this
   lock.

**Spec impact** (D-14-4.2):

- New section `pretty-printer-snapshot.md` §10
  "Composer renderers" with §10.1
  `render_const(ConstId, TyId) -> String` opener
  (introduction + signature reminder + dispatch
  overview).
- New subsection `pretty-printer-snapshot.md` §10.1.1
  "Scalar variants -- `Bool` / `Char` / `Int` /
  `Uint` / `Float`" hosting the per-arm table above +
  the decimal / char / float precision rules.
- `pretty-printer-snapshot.md` §10 (Cross-references)
  renumbered to §11 (one-time renumbering; no other
  references in the file reach §10 of this file).
- `pretty-printer-snapshot.md` §11 (now Cross-references)
  table gains a new row for `spec.md` §9.7
  (`Const` vocabulary) pointing to
  `pretty-printer.md` §10.1.
- `pretty-printer-snapshot.md` §9 specimen revisions
  (in-place, audit-shape-driven by *Spec is mutable*):
  - Line 1140-1141 -- already correct (`42_i32`).
    No change.
  - Line 1152 -- `0_u8` -- already correct
    (underscore-prefixed). No change.
  - Line 1204-1205 -- `<ConstId for 1u8>` ->
    `<ConstId for 1_u8>`; output `1u8` -> `1_u8`.
  - Line 1212 -- `<ConstId for 42i32>` ->
    `<ConstId for 42_i32>`; output reference unchanged
    (already says `42_i32` in narrative text).
  - Line 1226 -- `<ConstId for 42u32>` ->
    `<ConstId for 42_u32>`; output `42u32` -> `42_u32`.
  - Line 1241 -- `<ConstId for 0xFFu8>` ->
    `<ConstId for 255_u8>`; output `0xFFu8` ->
    `255_u8` (hex form rejected; D-14-4.11
    confirms decimal-only).
  - Line 1287 -- `<ConstId for 16usize>` ->
    `<ConstId for 16_usize>`; output `16usize` ->
    `16_usize`.
  - Line 1149 -- already correct (`42_i32`).
- Cross-reference text at `pretty-printer-snapshot.md`
  line 664 ("`Constant` is rendered bare; the const
  value's own rendering encodes its const-ness
  (`42i32`, `true`, `&\"...\"`, etc.).") updated to
  use underscore form for consistency: `42_i32`.

**Worked example.**

```text
Const::Bool(false)          ->  false
Const::Bool(true)           ->  true
Const::Char(0x61)           ->  'a'                  // ASCII printable
Const::Char(0x0A)           ->  '\n'                 // standard escape
Const::Char(0x1F600)        ->  '\u{1f600}'          // grinning-face emoji
Const::Int(I32, 42)         ->  42_i32
Const::Int(I32, u128::MAX)  ->  -1_i32               // sign-extended u128 storage; (u128::MAX as i128) as i32 == -1
Const::Int(I8, 0x7f)        ->  127_i8
Const::Uint(U8, 0xFF)       ->  255_u8               // not 0xFFu8 (D-14-4.11 decimal-only)
Const::Uint(U32, 0)         ->  0_u32
Const::Uint(Usize, 16)      ->  16_usize
Const::Float(F32, 0x3f800000) -> 1f32                // 1.0 finite
Const::Float(F32, 0x40490fdb) -> 3.1415927f32        // approximately pi
Const::Float(F64, 0)        ->  0f64                 // positive zero
```

The forms `i32::MAX` / `i32::MIN` / `u32::MAX` etc.
(rustc's sentinel collapsing) are NOT applied at this
sub-decision; D-14-4.10 layers that on top once
locked. Non-finite floats (`inf_f32`, `NaN_f32`,
`-inf_f32`) are NOT covered here; D-14-4.12 specifies
those forms.

#### D-14-4.10 -- `Int` / `Uint` sentinel collapsing [LOCKED]

**Decision.** The renderer collapses width-bounds
sentinels for `Int` and `Uint` scalar variants to
rustc-mirrored symbolic forms at exact-bits equality.
The full set of sentinels admitted (18 entries):

| Variant + value                                 | Rendered form  | Notes                                                                                              |
|-------------------------------------------------|----------------|----------------------------------------------------------------------------------------------------|
| `Int(I8, raw)` where decoded == `i8::MIN`       | `i8::MIN`      | `decoded == -128`. No `_i8` suffix on sentinel form.                                               |
| `Int(I8, raw)` where decoded == `i8::MAX`       | `i8::MAX`      | `decoded == 127`.                                                                                  |
| `Int(I16, raw)` where decoded == `i16::MIN`     | `i16::MIN`     |                                                                                                    |
| `Int(I16, raw)` where decoded == `i16::MAX`     | `i16::MAX`     |                                                                                                    |
| `Int(I32, raw)` where decoded == `i32::MIN`     | `i32::MIN`     |                                                                                                    |
| `Int(I32, raw)` where decoded == `i32::MAX`     | `i32::MAX`     |                                                                                                    |
| `Int(I64, raw)` where decoded == `i64::MIN`     | `i64::MIN`     |                                                                                                    |
| `Int(I64, raw)` where decoded == `i64::MAX`     | `i64::MAX`     |                                                                                                    |
| `Int(I128, raw)` where decoded == `i128::MIN`   | `i128::MIN`    |                                                                                                    |
| `Int(I128, raw)` where decoded == `i128::MAX`   | `i128::MAX`    |                                                                                                    |
| `Int(Isize, raw)` where decoded == `isize::MIN` | `isize::MIN`   | Target-pointer-width-independent: `isize::MIN`, NOT `i64::MIN`, even on 64-bit targets.            |
| `Int(Isize, raw)` where decoded == `isize::MAX` | `isize::MAX`   |                                                                                                    |
| `Uint(U8, raw)` where decoded == `u8::MAX`      | `u8::MAX`      | `decoded == 255`. Unsigned MIN (0) is NOT collapsed (`0_u8` decimal is already concise).           |
| `Uint(U16, raw)` where decoded == `u16::MAX`    | `u16::MAX`     |                                                                                                    |
| `Uint(U32, raw)` where decoded == `u32::MAX`    | `u32::MAX`     |                                                                                                    |
| `Uint(U64, raw)` where decoded == `u64::MAX`    | `u64::MAX`     |                                                                                                    |
| `Uint(U128, raw)` where decoded == `u128::MAX`  | `u128::MAX`    |                                                                                                    |
| `Uint(Usize, raw)` where decoded == `usize::MAX`| `usize::MAX`   | Target-pointer-width-independent: `usize::MAX`, NOT `u64::MAX`, even on 64-bit targets.            |

Sentinel detection is performed at the width-decoded
value layer (not at raw u128 bits): the renderer
decodes per the D-14-4.2 width-decoding rule, then
tests `decoded == T::MIN` / `decoded == T::MAX` for the
declared width T. This means:

- For `Int(I8, raw)`, decode is `raw as i8` (truncate to
  low 8 bits, reinterpret as signed); MIN/MAX tests are
  against `i8::MIN` / `i8::MAX` of the host `i8` type.
- For `Uint(U8, raw)`, decode is `raw as u8`; MAX test
  is against `u8::MAX` of the host `u8` type.
- For `Isize` / `Usize`, the host's pointer width is
  irrelevant: the comparison uses the target's pointer
  width from `spec.md` §11 `ScalarLayouts` (the
  `int[Isize].size` / `uint[Usize].size` entries).
  The symbolic form `isize::MIN` / `usize::MAX` is
  rendered regardless of target; the compared sentinel
  value is the target's `isize` / `usize` MIN / MAX
  (cross-target snapshot stability requires this).

Sentinel forms are NOT suffixed (e.g., `i32::MAX`, NOT
`i32::MAX_i32`). The type name in the sentinel form
already carries the width information unambiguously.

Decimal-form fallback: if the decoded value does not
match any sentinel constant of its declared width, the
underscore-decimal form from D-14-4.2 applies
(`<dec>_<width_suffix>`).

**Reasoning.** Three load-bearing reasons drive matching
rustc:

1. **Compiler-theory-first**: rustc's `ConstInt` Debug
   alternate-form
   (`rustc_middle/src/ty/consts/int.rs:60-79` for
   signed; `:103-113` for unsigned) is the established
   pretty-printing convention for MIR const dumps.
   Driver authors who read `cargo --emit=mir` output
   will recognise `i32::MAX` and `u32::MAX` forms
   immediately; introducing a parallel convention
   (decimal-only) would diverge from familiar tooling
   for no upside.
2. **Diagnostic value for register-bank patterns**:
   driver code uses mask constants extensively
   (e.g., `pl011`'s flag/control registers carry
   patterns like `0xFFFF_FFFF` for "all bits set"
   masks; `0x80000000` for the high-bit pattern that's
   often `i32::MIN`). The symbolic forms surface the
   developer's intent: `u32::MAX` clearly reads as
   "all bits set," whereas `4294967295_u32` reads as
   "some specific decimal value, is that meaningful?"
   The collapsing is information-preserving and
   diagnostically richer.
3. **Round-trip safety**: `i32::MIN` / `i32::MAX` /
   `u32::MAX` are valid Rust source-level expressions
   that round-trip through Rust parsing back to the
   same constant. The form is a Rust literal expression
   in the strict sense (not a Rust literal token, but a
   const expression that evaluates to the same value).
   No information is lost.

The alternative -- decimal-only rendering -- was the
fallback option; it loses zero information but
produces less diagnostic output for what is in practice
the common register-mask case in driver specs.

**No unsigned MIN collapsing.** The signed sentinel
set includes both MIN and MAX (a `0x80000000` value
in a u128 storage for `Int(I32, _)` is distinguishable
from `0` via the sign bit). The unsigned sentinel set
includes MAX only because the MIN value (`0`) renders
as `0_u8` / `0_u32` / `0_usize` decimal which is
already concise and unambiguous. Matches rustc's
`ConstInt` Debug at `:103-113`.

**Why per-width sentinels, not just `T::MAX`?** The
sentinel form includes the type name (`i32::MAX`, not
`MAX`) because the type tag is load-bearing
information: `i32::MAX` (== `0x7FFFFFFF`) and
`u32::MAX` (== `0xFFFFFFFF`) are different values, and
the renderer must disambiguate. Rustc does the same
(`:60-69`).

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt the
  established rustc convention; do not invent a
  parallel form.
- *C semantics with information preservation*
  (README §2). The decoded value's width is preserved
  in the sentinel form's type-name prefix; no
  information loss.
- *No defensive fallbacks* (README §2). The pinned
  toolchain rules out the question "what if rustc's
  sentinel form changes"; the lock matches the current
  convention. If a future rustc release diverges, the
  spec amends at that point.

**Prior-art evidence.** `rustc_middle/src/ty/consts/int.rs:51-138`
(`impl fmt::Debug for ConstInt`) is the primary
evidence. Signed branch at lines 56-101; unsigned
branch at lines 102-137. The width-based dispatch
(`match size`) handles all 6 widths uniformly
(`I8`/`I16`/`I32`/`I64`/`I128`/`Isize`); the
`is_ptr_sized_integral` flag forces `isize`/`usize` for
the pointer-sized variant regardless of the size-byte
count. The MIR pretty-printer at
`rustc_middle/src/ty/print/pretty.rs:1806-1810` invokes
`{int:#?}` (alternate form), which is the form that
exhibits the sentinel collapsing -- the non-alternate
`{int:?}` form omits the width suffix on non-sentinel
values but still collapses sentinels. Stage 14 matches
the alternate-form behaviour throughout (both
sentinels and width-suffixed decimal).

**Spec impact** (D-14-4.10):

- `pretty-printer-snapshot.md` §10.1.1: extend the
  per-arm table for `Int` / `Uint` with a
  sentinel-collapsing note + the 18-entry sentinel
  table. Add to the "Decimal-rendering precision"
  subsection a sentinel-detection rule preceding the
  decimal-fallback rule.
- `pretty-printer-snapshot.md` §9 specimens: no
  revisions required at this lock (current §9
  specimens use mid-range values: 1, 42, 16, 255, 0;
  none of these are width-sentinels). Future specimens
  for `Int(I32, i32::MIN as u128)`/etc. will exercise
  the sentinel paths (lands during D-14-4 cluster
  close specimen-augmentation step).

**Worked example.**

```text
Const::Int(I8, 0x7F)                ->  i8::MAX           // 127 == i8::MAX
Const::Int(I8, u128::MAX - 0x7F)    ->  i8::MIN           // sign-extended -128 == i8::MIN
Const::Int(I32, 0x7FFFFFFF)         ->  i32::MAX          // 2147483647
Const::Int(I32, 0x80000000)         ->  i32::MIN          // decoded as i32: -2147483648
Const::Uint(U32, 0xFFFFFFFF)        ->  u32::MAX
Const::Uint(U32, 0)                 ->  0_u32             // NOT collapsed; u*::MIN is decimal
Const::Uint(U8, 0xFF)               ->  u8::MAX           // not 255_u8 anymore (D-14-4.10 supersedes)
Const::Int(Isize, isize::MIN as u128 sign-ext) -> isize::MIN  // target-pointer-width-independent
Const::Uint(Usize, usize::MAX as u128 zero-ext) -> usize::MAX // target-pointer-width-independent

// Non-sentinels still use D-14-4.2 decimal:
Const::Int(I32, 42)                 ->  42_i32
Const::Uint(U32, 1024)              ->  1024_u32
Const::Int(I8, 0)                   ->  0_i8
```

**Implication for D-14-4.2 specimens.** The
`Const::Uint(U8, 0xFF)` example in D-14-4.2's worked
example renders as `255_u8` per D-14-4.2 alone; with
D-14-4.10 layered on, it renders as `u8::MAX` (the
sentinel form supersedes). The §9.6 Aggregate-Union
specimen at line 1252
(`pl011::regs::MyUnion { byte: 255_u8 }`) is now stale
with D-14-4.10 applied -- the rendered output for
`Const::Uint(U8, 0xFF)` is `u8::MAX`, so the specimen
output becomes `pl011::regs::MyUnion { byte: u8::MAX }`.
Specimen revised in place per *Spec is mutable*
(README §2).

#### D-14-4.11 -- Integer hex form admission: decimal-only for v1 [LOCKED]

**Decision.** `Int` / `Uint` scalar rendering in v1 is
**decimal-only** with the underscore-decimal convention
per D-14-4.2 + the sentinel-collapsing rule per
D-14-4.10. No hex form admission. No
`PrettyPrintOptions::integer_hex: bool` toggle is
added to the existing 2-field
`PrettyPrintOptions { include_spans, include_debug_names }`
(stage-13 D-13-3).

**Future-extension boundary.** A hex render style is
recognised as a potentially desirable extension for
driver-spec contexts where register-bit patterns are
the dominant integer-literal shape (e.g., a UART
control register's `0xFF00_FF00` mask reads more
naturally than `4278255360_u32`). However:

- The v1 spec deliberately mirrors rustc's MIR
  pretty-printer output (no hex), which is the
  established compiler-pretty-printing convention
  driver authors will already be familiar with from
  `cargo --emit=mir` output.
- D-14-4.10's sentinel collapsing already covers the
  most common "all-bits-set" / "MIN" / "MAX" register
  patterns symbolically (`u32::MAX`, `i32::MIN`,
  etc.), which are even more readable than hex.
- Phase-1 testing (the DSL -> IR -> C -> captured-
  stdout integration tests landing in implementation
  sweep #1) will surface any register-bit-pattern
  cases where decimal rendering is genuinely
  insufficient. If so, a future revision can extend
  `PrettyPrintOptions` with an additive field
  `integer_render_style: IntegerRenderStyle` (closed
  3-variant enum `Decimal` / `Hex` / `Auto`, where
  `Auto` picks based on value heuristics like
  popcount or width / value ratio); the additive
  shape would not break the locked
  `render_const(ConstId, TyId) -> String` API
  signature from D-14-4.1.
- The future-revision path is documented but **NOT
  pre-allocated**: no `integer_render_style` field
  exists in v1's `PrettyPrintOptions`, no
  `IntegerRenderStyle` enum is declared. Per the
  *no-forward-looking-placeholders* rule in
  README §5.9 rule 4, only pinned content lands in
  the spec.

**Reasoning.** Three reasons drive decimal-only for v1:

1. **Rustc-mirroring is the established convention.**
   Rustc's `ConstInt` Debug at
   `rustc_middle/src/ty/consts/int.rs:80-101 +
   :114-133` always renders decimal for non-sentinel
   values; non-alternate-form (`{int:?}`) renders the
   same decimal without the width suffix. RegseqModel's
   `render_const` matches the alternate-form convention
   throughout. Introducing a parallel hex shape would
   diverge from rustc tooling driver authors
   recognise. *Compiler-theory-first* (README §2).
2. **Information-preserving + round-trip safe.** The
   decimal form fully encodes the value; the
   underscore-decimal suffix carries the width
   information. No information is lost; no
   round-trip-distorting heuristic is needed.
3. **The carve-out (`PrettyPrintOptions` toggle) is
   the right shape if it becomes needed, but should
   be evidence-driven.** Adding a toggle pre-emptively
   without phase-1 testing evidence would conflict with
   *No defensive fallbacks* (README §2): the spec
   should not pre-allocate hooks for hypothetical
   needs. Phase-1 testing is the evidence source; if
   register-bit-pattern decimal rendering causes
   diagnostic friction, the future-revision path is
   open.

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt rustc's
  decimal convention.
- *No defensive fallbacks* (README §2). Don't
  pre-allocate hooks (no `integer_render_style` field
  shipped in v1's `PrettyPrintOptions`).
- *No forward-looking placeholders* (README §5.9 rule
  4). The future-revision path is described in
  reasoning text but not pre-allocated in spec text.

**Prior-art evidence.** Rustc's `ConstInt` Debug
implementation at
`rustc_middle/src/ty/consts/int.rs:51-138` never
produces hex; the alternate-form
(`{int:#?}`) consumed by MIR pretty-printing at
`rustc_middle/src/ty/print/pretty.rs:1806-1810` uses
the same decimal convention. The non-alternate form
omits the width suffix but stays decimal. No rustc
codepath emits `0x...` for `ScalarInt` rendering in
the MIR pretty-printer. Stage 14's decimal-only
commitment matches this convention uniformly.

**Spec impact** (D-14-4.11):

- `pretty-printer-snapshot.md` §10.1.1: the existing
  policy-shaped-sub-aspect listing notes that
  D-14-4.11 confirms decimal-only. No change to the
  per-arm table or precision rules (D-14-4.2 already
  pinned decimal; D-14-4.11 just records the explicit
  commitment).
- No `PrettyPrintOptions` field additions. The
  existing 2-field shape from stage-13 D-13-3
  (`include_spans: bool`, `include_debug_names: bool`)
  is unchanged.
- No `IntegerRenderStyle` enum declaration. Future
  revision can add if needed; not pre-allocated.

**Worked example.**

```text
// All Int/Uint rendering uses D-14-4.2 + D-14-4.10
// uniformly:
Const::Uint(U32, 0xFF00_FF00)  ->  4278255360_u32     // decimal, NOT 0xFF00FF00_u32
Const::Uint(U16, 0x1234)       ->  4660_u16           // decimal, NOT 0x1234_u16
Const::Int(I8, 0x7F)           ->  i8::MAX            // sentinel collapsed (D-14-4.10)
Const::Uint(U32, 0xFFFFFFFF)   ->  u32::MAX           // sentinel collapsed (D-14-4.10)

// Hex literal at the SOURCE level
// (e.g., Rust source `let x: u32 = 0xFF00_FF00;`) is
// preserved by the bridge as Uint(U32, 0xFF00FF00),
// then rendered as the decimal 4278255360_u32. The
// source-level form is lost in the IR (bridge does
// not carry source-form hints).
```

#### D-14-4.12 -- Float infinity / NaN representation [LOCKED]

**Decision.** Non-finite `Float` values render in a
rustc-mirrored underscore-broken form:
`<repr>_f<width>` where `<repr>` is one of
`inf` / `-inf` / `NaN` per IEEE-754 classification.
The full set across the four `FloatKind` widths
(12 entries):

| Variant + classification               | Rendered form  | Notes                                                   |
|----------------------------------------|----------------|---------------------------------------------------------|
| `Float(F16, +inf bit pattern)`         | `inf_f16`      | All NaN payload / signaling-bit variations collapse.    |
| `Float(F16, -inf bit pattern)`         | `-inf_f16`     |                                                         |
| `Float(F16, any NaN bit pattern)`      | `NaN_f16`      | All NaN bit patterns render as the single `NaN_f<width>` form (quiet / signaling / payload-bearing all collapse). |
| `Float(F32, +inf bit pattern)`         | `inf_f32`      |                                                         |
| `Float(F32, -inf bit pattern)`         | `-inf_f32`     |                                                         |
| `Float(F32, any NaN bit pattern)`      | `NaN_f32`      |                                                         |
| `Float(F64, +inf bit pattern)`         | `inf_f64`      |                                                         |
| `Float(F64, -inf bit pattern)`         | `-inf_f64`     |                                                         |
| `Float(F64, any NaN bit pattern)`      | `NaN_f64`      |                                                         |
| `Float(F128, +inf bit pattern)`        | `inf_f128`     |                                                         |
| `Float(F128, -inf bit pattern)`        | `-inf_f128`    |                                                         |
| `Float(F128, any NaN bit pattern)`     | `NaN_f128`     |                                                         |

The underscore between value and width suffix is
load-bearing for non-finite values only: `inff32`
would parse as the single identifier `inff32` (or
similar ambiguous token) under Rust lexer rules,
whereas `inf_f<width>` reads unambiguously as
"non-finite floating-point value of type `f<width>`"
to a human reader. Finite values do NOT use the
underscore (per D-14-4.2: `1.5f32`, `0f64`,
`42f64`) -- the form is a parseable Rust float
literal without the underscore.

Classification rule:
- The renderer reconstitutes the IEEE-754 value at
  the variant's `FloatKind` width per D-14-4.2's
  finite-precision rule, then invokes the apfloat
  classification:
  - `is_finite() == true` (includes both positive
    zero and negative zero) -> D-14-4.2 finite form
    (`<val>f<width>` no underscore).
  - `is_finite() == false` && positive infinity ->
    `inf_f<width>`.
  - `is_finite() == false` && negative infinity ->
    `-inf_f<width>`.
  - NaN (regardless of payload, signaling bit, or
    sign bit) -> `NaN_f<width>`.

The form `<val>` for finite values comes from
apfloat's `Display` impl; for non-finite, the renderer
emits the literal text `inf` / `-inf` / `NaN`
directly (not via apfloat's `Display`, which would
also produce these tokens but routes through the
underscore-conditional path in rustc's
`pretty_print_const_scalar_int` Float arm).

**NaN bit-pattern distinction is out of v1 scope.**
IEEE-754 distinguishes:
- *Quiet NaN* (qNaN): the most-significant fraction
  bit is set.
- *Signaling NaN* (sNaN): the most-significant
  fraction bit is clear (and at least one other
  fraction bit is set).
- *NaN payload*: the remaining fraction bits encode
  domain-specific information (e.g., diagnostic
  pointer payloads).
- *Sign bit on NaN*: IEEE-754 permits a sign bit on
  NaN values, though the sign is operationally
  meaningless for NaN.

V1 renders all NaN bit patterns as the single
`NaN_f<width>` form, losing this distinction at the
rendering surface. The `Const::Float(_, u128)`
storage still preserves the bit pattern; only the
*rendered* form collapses. Matches rustc's MIR
pretty-printer behaviour
(`rustc_middle/src/ty/print/pretty.rs:1787-1804`
emits `NaN` via the apfloat `Display` regardless of
NaN type).

If future driver-spec requirements surface
NaN-payload distinction needs (e.g., diagnostic
sentinel NaNs encoding device-state hints), a future
revision can add a NaN-bit-pattern rendering option
to `PrettyPrintOptions` parallel to D-14-4.11's
hex-admission future-extension boundary. Not
pre-allocated per README §5.9 rule 4.

**Negative zero is finite.** `Float(F32, 0x80000000)`
(negative zero) renders as `-0f32` per D-14-4.2's
finite-rendering rule -- apfloat's `Display`
distinguishes `+0` and `-0` by sign bit. Negative
zero is NOT a non-finite case; the underscore form
does NOT apply.

**Reasoning.** Three reasons drive matching rustc's
underscore-broken non-finite form:

1. **Compiler-theory-first**: rustc's
   `pretty_print_const_scalar_int` Float arm at
   `rustc_middle/src/ty/print/pretty.rs:1787-1804`
   produces this exact form via the format string
   `"{}{}f<width>"` with the underscore-conditional
   middle field. Driver authors familiar with MIR
   dumps will recognise the form immediately;
   introducing a parallel form (e.g.,
   `f32::INFINITY`, `f32::NAN`) would diverge from
   the established convention.
2. **Diagnostic readability**: the underscore is the
   minimum lexical break needed to disambiguate
   `<repr>` from `f<width>` in human reading.
   Alternatives considered:
   - `f32::INFINITY` / `f32::NAN` -- function-call /
     constant-path forms. More verbose, less compact
     for inline MIR dumps. Familiar from Rust
     standard library, but the MIR pretty-printer's
     compactness is preferred.
   - `0x7f800000_f32` (raw bit pattern + width
     suffix) -- diagnostically opaque; loses the
     "this is infinity" reading. Rejected for
     non-finites; rustc never produces this form.
   - Whitespace-separated `inf f32` -- conflicts
     with operand-rendering whitespace conventions.
   The underscore-broken form is the minimal change
   from the finite-rendering form that achieves
   unambiguous parsing for human readers.
3. **NaN collapsing matches rustc**: all NaN
   bit-pattern variations rendering as the single
   `NaN_f<width>` form is rustc's behaviour (apfloat
   `Display` emits `NaN` regardless of payload).
   Phase-1 testing for driver specs does not
   currently exercise NaN-payload sensitivity;
   matching rustc is the conservative choice.

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt rustc's
  established underscore-broken convention.
- *No defensive fallbacks* (README §2). Don't
  pre-allocate `PrettyPrintOptions` toggles for
  NaN-payload distinction; if phase-1 testing surfaces
  the need, a future revision adds the toggle.
- *No forward-looking placeholders* (README §5.9 rule
  4). NaN-payload distinction is described as a
  future-extension boundary but not pre-allocated in
  spec text or `PrettyPrintOptions` field set.
- *Compositional behaviour beats special-case flags*
  (README §2). Negative zero is finite (not a
  non-finite case); the existing finite-rendering
  rule covers it; no parallel "is_negative_zero" flag
  needed.

**Prior-art evidence.** Rustc's
`pretty_print_const_scalar_int` Float arm at
`rustc_middle/src/ty/print/pretty.rs:1787-1804`:

```rust
ty::Float(fty) => match fty {
    ty::FloatTy::F16 => {
        let val = Half::try_from(int).unwrap();
        write!(self, "{}{}f16", val,
            if val.is_finite() { "" } else { "_" })?;
    }
    ty::FloatTy::F32 => { ... "{}{}f32" ... }
    ty::FloatTy::F64 => { ... "{}{}f64" ... }
    ty::FloatTy::F128 => { ... "{}{}f128" ... }
}
```

`rustc_apfloat::ieee::{Half, Single, Double, Quad}`
provide the IEEE-754-decoded `Display` impls. For
non-finite values, `Display` produces the tokens
`inf` / `-inf` / `NaN` directly. The
`val.is_finite()` test is the load-bearing
classification.

**Spec impact** (D-14-4.12):

- `pretty-printer-snapshot.md` §10.1.1: extend the
  Float-rendering precision subsection (currently
  finite-only per D-14-4.2) with a
  non-finite-handling subsection containing the
  12-entry rendered-form table + the classification
  rule + the NaN-collapsing note + the negative-zero
  carve-out.
- `pretty-printer-snapshot.md` §10.1.1 policy-shaped
  sub-aspect listing: the D-14-4.12 entry is now
  LOCKED (was sketch-only).
- No `PrettyPrintOptions` field additions
  (NaN-payload distinction toggle is future-extension,
  not pre-allocated).
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise non-finite Float
  rendering; specimens for `Float(F32, 0x7f800000)`
  / `Float(F32, 0xff800000)` / `Float(F32, 0x7fc00000)`
  (positive infinity / negative infinity / NaN
  representatives) will land alongside D-14-4 cluster
  close specimen-augmentation step.

**Worked example.**

```text
Const::Float(F32, 0x7f800000)  ->  inf_f32       // +Infinity (sign=0, exp=all-1s, frac=0)
Const::Float(F32, 0xff800000)  ->  -inf_f32      // -Infinity (sign=1, exp=all-1s, frac=0)
Const::Float(F32, 0x7fc00000)  ->  NaN_f32       // quiet NaN (sign=0, exp=all-1s, frac MSB=1)
Const::Float(F32, 0x7fa00000)  ->  NaN_f32       // signaling NaN (sign=0, exp=all-1s, frac MSB=0, other frac bits != 0)
Const::Float(F32, 0xffc00000)  ->  NaN_f32       // negative quiet NaN (sign=1) -- all NaN bit patterns collapse
Const::Float(F64, 0x7ff0000000000000)        ->  inf_f64
Const::Float(F64, 0xfff0000000000000)        ->  -inf_f64
Const::Float(F64, 0x7ff8000000000000)        ->  NaN_f64

// Negative zero is finite (D-14-4.2 territory):
Const::Float(F32, 0x80000000)  ->  -0f32         // negative zero, finite
Const::Float(F32, 0x00000000)  ->  0f32          // positive zero, finite

// Finite values use D-14-4.2 (no underscore):
Const::Float(F32, 0x3f800000)  ->  1f32
Const::Float(F32, 0x40490fdb)  ->  3.1415927f32
```

The three policy-shaped sub-aspects from D-14-4.2 are
now closed: D-14-4.10 (`Int`/`Uint` sentinel
collapsing), D-14-4.11 (integer hex form admission:
decimal-only), D-14-4.12 (`Float` infinity / NaN
representation). The scalar-variant surface
(D-14-4.2 + D-14-4.10 + D-14-4.11 + D-14-4.12) is
fully spec'd.

#### D-14-4.3 -- `ZST` per-arm rendering [LOCKED]

**Question.** `Const::ZST` is the canonical single
representative for all ZST values per `spec.md` §9.7
("Single canonical representative for all ZSTs (`()`,
fieldless structs, `PhantomData<T>`, etc.). Type comes
from the consumer's context"). Rendering must produce
the natural Rust value form for the consumer's type
across the full set of ZST consumer Tys -- which is
broader than rustc's MIR pretty-printer covers (rustc
special-cases `ZeroSized` over `FnDef` and falls
through to debug-form for all other ZST consumer Tys
per `rustc_middle/src/mir/pretty.rs:1976-1988`).
RegseqModel can do better because the bridge preserves
full structural information (no `tcx` re-querying
needed) and the spec already enumerates the exhaustive
set of ZST-eligible `Ty` variants.

**Decision.** The renderer dispatches on the consumer
`Ty.kind()` across 7 categories of ZST consumer Ty.
For composite-ZST cases, the renderer recurses on the
same `ConstId` (the unique `Const::ZST` representative)
with the per-sub-field Ty as the new consumer-context
Ty:

| Consumer `Ty.kind()`                                              | Rendered form                                                                  | Notes                                                                                                                                                                |
|-------------------------------------------------------------------|--------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Tuple { fields: [] }`                                        | `()`                                                                           | Unit type. Common case.                                                                                                                                              |
| `Ty::Tuple { fields: [T1] }` (single-field ZST tuple)             | `(<r(zst, T1)>,)`                                                              | Trailing comma per Rust syntax for 1-tuples. Composite-ZST recursion.                                                                                                |
| `Ty::Tuple { fields: [T1, T2, ..., Tn] }` (n ≥ 2 ZST tuple)       | `(<r(zst, T1)>, <r(zst, T2)>, ..., <r(zst, Tn)>)`                              | Composite-ZST recursion per sub-field Ty.                                                                                                                            |
| `Ty::Array(_, 0)`                                                 | `[]`                                                                           | Empty array. (Note: stage-3 / stage-10 admit this Ty shape; the bridge produces `Const::ZST` for any 0-length array regardless of element type.)                     |
| `Ty::Array(elem, N)` where `N > 0` and array is ZST               | `[<r(zst, elem)>; N]`                                                          | Rust repeat syntax. Composite-ZST recursion on element Ty. Note `N` renders as a decimal usize literal (no `_usize` suffix; matches `Ty::Array` type-rendering form at §6.2). |
| `Ty::Struct { item, args, layout: { kind: Struct { fields: [] }, .. } }`  | `<render_def_path(item)><render_generic_args(args, item)>`           | Fieldless struct. Includes `core::marker::PhantomData<T>`, unit structs like `struct Foo;`, and any other fieldless struct.                                          |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` where `fields.len() > 0` and all `fields[i].ty` are ZST and field names are tuple-synthetic (`_0` / `_1` / ...)   | `<render_def_path(item)><render_generic_args(args, item)>(<r(zst, fields[0].ty)>, <r(zst, fields[1].ty)>, ...)` | Tuple-style struct with all-ZST fields. Tuple-vs-named distinction follows §3.4 (field names matching `_<digit>` pattern).                                           |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` where `fields.len() > 0` and all `fields[i].ty` are ZST and field names are non-synthetic (named-field struct)   | `<render_def_path(item)><render_generic_args(args, item)> { <fields[0].name>: <r(zst, fields[0].ty)>, ... }`   | Named-field struct with all-ZST fields.                                                                                                                              |
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }` where `V.fields.is_empty()`  | `<enum_path>::<V.name>` where `enum_path = <render_def_path(item)><render_generic_args(args, item)>` | Single-variant unit-variant enum (`enum E { V }`). Variant name lookup via `EnumShape::Single.variant.name` per §3.4.                                                |
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }` where `V.fields` non-empty, tuple-synthetic names, all ZST | `<enum_path>::<V.name>(<r(zst, V.fields[0].ty)>, ...)`                         | Single-variant tuple-variant enum with all-ZST fields.                                                                                                               |
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }` where `V.fields` non-empty, named, all ZST | `<enum_path>::<V.name> { <V.fields[0].name>: <r(zst, V.fields[0].ty)>, ... }`  | Single-variant named-field-variant enum with all-ZST fields.                                                                                                         |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Multiple { .. } }, .. } }`   | Cannot occur for ZST                                                           | Multi-variant enums carry a discriminant tag of nonzero size; cannot be ZST. Bridge cannot produce this pairing; renderer rejects with a panic (assertion).          |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Empty }, .. } }`             | Cannot occur for ZST                                                           | Zero-variant uninhabited enum has no values. Renderer rejects with a panic.                                                                                          |
| `Ty::Union { item, args, .. }`                                    | `<render_def_path(item)><render_generic_args(args, item)>`                           | Union (with single-ZST-field; rare in practice). Field-disambiguation is NOT preserved at the value level for unions; rendering produces just the path. If field disambiguation is needed downstream, it must come from the writer side (`SetDiscriminant`-style not applicable to unions; `AggregateKind::Union(_, FieldIdx)` carries the field). |
| `Ty::FnDef(item, args)`                                           | `<render_def_path(item)><render_generic_args(args, item)>`                           | Value-namespace path with generic args. **Mirrors rustc's only special-case at `mir/pretty.rs:1976-1982`.** Note: distinct from the type-rendering form at §6.4 (which adds `fn(<arg_tys>) -> <ret_ty> {...}` wrapping for the type); the value-rendering form is the bare path. |
| `Ty::Closure(item, args)` (non-capturing)                         | `<render_def_path(item)><render_generic_args(args, item)>`                           | The synthesized closure name (`parent::module::{closure#N}`) per stage-12 / stage-13 `render_def_path` cache.                                                          |
| `Ty::Never`                                                       | Cannot occur for ZST (uninhabited)                                             | `!` has no values. Renderer rejects with a panic.                                                                                                                    |
| Any other `Ty.kind()` (non-ZST scalar, ref, raw ptr, dynamic, opaque, etc.) | Cannot occur                                                                   | Non-ZST types cannot consume `Const::ZST`: the bridge would produce a non-ZST `Const` variant (Bool / Int / Uint / Float / Char / NullPtr / Reloc / Bytes /...) instead. Renderer rejects with a panic. |

Recursion shape: `r(zst, T)` denotes the recursive call
`self.render_const(zst_const_id, T)` where
`zst_const_id` is the SAME `ConstId` representing the
unique `Const::ZST` canonical representative. Per §9.7,
there is exactly one `Const::ZST` in
`model.consts` (the canonical-form rule); the
recursion uses this single ID for every composite-ZST
sub-field. The recursion terminates because each
recursive step descends into a structurally smaller
consumer Ty (a sub-field of a tuple / struct /
variant / element of an array) and eventually bottoms
out at a leaf ZST Ty (`Ty::Tuple([])`, fieldless
`Ty::Struct`, `Ty::FnDef`, `Ty::Closure`, etc.).

**Tuple-vs-named-field dispatch** for `Ty::Struct` and
`Ty::Enum` variant fields: follows the §3.4 rule (the
existing convention pinned by stage-13 D-13-2 +
stage-13 D-13-4): field names matching the synthetic
pattern `_<index>` (`_0`, `_1`, ...) signal
tuple-style; non-synthetic names signal
named-field-style. Empty-fields layout (`fields.len()
== 0`) signals unit-style.

**Composite-ZST validity.** The bridge produces
`Const::ZST` only for `ConstValue::ZeroSized` per
§9.7's dispatch table line. The bridge does NOT
decompose a composite ZST (e.g., `[(); 100]` or
`((), (), ())`) into an `Aggregate(...)`; per §9.7 the
canonical form is `Const::ZST` (single representative
for all ZSTs). The renderer's recursion is what
produces the rich per-Ty value form at the rendering
surface, not the storage form.

**Reasoning.**

1. **Information preservation for the rendering
   surface.** Per §9.7, `Const::ZST` is the canonical
   single representative -- the storage collapses all
   ZSTs to one entry. But at the rendering surface,
   driver authors need the natural Rust value form
   for diagnostic clarity: `()` should not render as
   `<zst>`; `core::marker::PhantomData::<T>` should
   not render as `<zst>`; `[(); 100]` should not
   render as `<zst>`. The renderer recursively walks
   the consumer Ty structure, producing the natural
   per-Ty form. *C semantics with information
   preservation* (README §2) -- rustc gives us the
   consumer Ty for free; use it to produce the
   richest rendering.
2. **Exhaustive consumer-Ty coverage.** Per §9.7,
   `Const::ZST` can be consumed at any ZST `Ty`. The
   exhaustive set is enumerable: unit-tuple, ZST
   array, fieldless / composite-ZST struct, single-
   variant enum, ZST union, FnDef, non-capturing
   Closure. The renderer covers all these explicitly
   rather than relying on a generic fallback. Cases
   that cannot occur (`Ty::Never`, `EnumShape::Empty`,
   `EnumShape::Multiple`, non-ZST scalar consumer Tys)
   are rejected with a panic per *No defensive
   fallbacks* -- the bridge cannot produce these
   pairings.
3. **Rustc-mirroring where rustc has a form;
   RegseqModel-richer where rustc falls back to
   debug.** Rustc's only ZST special-case is
   `(ConstValue::ZeroSized, ty::FnDef(d, s))` at
   `mir/pretty.rs:1976-1982`, which produces the
   value-namespace path. RegseqModel matches this
   verbatim. For other ZST consumer Tys, rustc falls
   through to the `{ct:?}: {ty}` debug form;
   RegseqModel produces the natural Rust value form
   (path for fieldless structs / closures; `()` for
   unit; etc.). The richer rendering is enabled by
   RegseqModel's structural-information preservation
   (the consumer Ty's `layout.kind` carries field
   names and variant info).

The composite-ZST recursion (tuples / arrays /
struct-with-all-ZST-fields / enum-variant-with-all-
ZST-fields) is RegseqModel-specific. Rustc would fall
through to the debug fallback for these. The recursion
shape (`r(zst, sub_field_ty)`) reuses the same `ConstId`
across sub-fields, which is non-obvious but correct
per §9.7's canonical-form rule.

**Methodology rules invoked.**

- *C semantics with information preservation* (README
  §2). The recursive per-Ty rendering preserves the
  rich type information rustc gives us.
- *No defensive fallbacks* (README §2). Cases that
  cannot occur per the bridge's §9.7 dispatch
  (`Ty::Never`, `EnumShape::Empty`, `EnumShape::Multiple`,
  non-ZST scalar consumer Tys) are rejected with a
  panic rather than handled with a generic fallback.
- *Compiler-theory-first* (README §2). The
  consumer-Ty-driven dispatch mirrors rustc's tuple-
  match pattern at `mir/pretty.rs:1872`; the
  composite-ZST recursion extends the same compiler-
  theory pattern uniformly.
- *Compositional behaviour beats special-case flags*
  (README §2). Composite-ZST cases fall out of the
  recursive rendering naturally; no parallel
  `is_composite_zst` flag or alternate-storage
  fallback needed.

**Prior-art evidence.** Rustc's only ZST special-case
at `rustc_middle/src/mir/pretty.rs:1976-1982`
(`ConstValue::ZeroSized` over `ty::FnDef`); the debug
fallback at line 1988 covers all other ZST consumer
Tys with no special handling. RegseqModel extends the
rendering surface; no prior-art parallel exists for
the composite-ZST recursion shape.

**Spec impact** (D-14-4.3):

- `pretty-printer-snapshot.md` §10.1.2: replace the
  current stub (`(Pending D-14-4.3 lock.)`) with the
  full dispatch-table content + recursion-shape +
  tuple-vs-named-field reference to §3.4 + cannot-
  occur cases + worked examples.
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise ZST rendering at the
  D-14-4 level; new ZST specimens (covering `()`,
  fieldless struct via `PhantomData`, FnDef, and a
  composite-ZST tuple like `((), (), ())`) will land
  alongside the D-14-4 cluster close
  specimen-augmentation step.

**Worked example.**

```text
// Common cases -- single-value ZSTs:
Const::ZST + Ty::Tuple([])                          ->  ()
Const::ZST + Ty::Struct(<UnitStruct>)               ->  pl011::UnitStruct
Const::ZST + Ty::Struct(<PhantomData<u32>>)         ->  core::marker::PhantomData::<u32>
Const::ZST + Ty::FnDef(<my_func>, [u32])            ->  pl011::my_func::<u32>
Const::ZST + Ty::Closure(<closure#0>, [u32])        ->  pl011::init::{closure#0}::<u32>
Const::ZST + Ty::Enum(<E where E={V}>, [])          ->  pl011::E::V

// Composite ZST recursion:
Const::ZST + Ty::Array(<()>, 0)                     ->  []
Const::ZST + Ty::Array(<()>, 100)                   ->  [(); 100]
Const::ZST + Ty::Array(<PhantomData<u32>>, 3)       ->  [core::marker::PhantomData::<u32>; 3]
Const::ZST + Ty::Tuple([<()>, <()>, <()>])          ->  ((), (), ())
Const::ZST + Ty::Tuple([<PhantomData<i32>>])        ->  (core::marker::PhantomData::<i32>,)
Const::ZST + Ty::Struct(<Marker<u8>>)               ->  pl011::Marker::<u8>
  // where Marker is `struct Marker<T> { _pd: PhantomData<T>, _u: () }`
  //   -> pl011::Marker::<u8> { _pd: core::marker::PhantomData::<u8>, _u: () }
Const::ZST + Ty::Struct(<TupleZST>)                 ->  pl011::TupleZST(core::marker::PhantomData::<u8>, ())
  // where TupleZST is `struct TupleZST(PhantomData<u8>, ())` (tuple-style)

// Single-variant enum with payload:
Const::ZST + Ty::Enum(<E where E={V(PhantomData<u32>)}>, []) -> pl011::E::V(core::marker::PhantomData::<u32>)
Const::ZST + Ty::Enum(<E where E={V{x:()}}>, [])             -> pl011::E::V { x: () }

// Cannot-occur cases (renderer panics):
Const::ZST + Ty::Never                              -> panic: ZST over uninhabited Ty
Const::ZST + Ty::Enum(<EnumShape::Empty>)           -> panic: ZST over zero-variant enum
Const::ZST + Ty::Enum(<EnumShape::Multiple>)        -> panic: multi-variant enums cannot be ZST
Const::ZST + Ty::Int(I32)                           -> panic: non-ZST scalar consumer; bridge bug
```

#### D-14-4.7 -- `NullPtr(TyId)` per-arm rendering [LOCKED]

**Decision.** `Const::NullPtr(pointee_ty)` renders in
the rustc-mirrored typed-cast form:

```text
0x0 as <render_ty(consumer_ty)>
```

where `consumer_ty` is the consumer-context pointer
Ty from D-14-4.1's pair-form (the pointer type at the
use site -- e.g., `*const u32`, `&mut u8`,
`fn(u32) -> bool`). The variant payload `pointee_ty`
is for canonical-equality only (per `spec.md` §9.7
line 2896-2899: "Per-pointee-type for canonical
equality: `NullPtr(t1) != NullPtr(t2)` whenever
`t1 != t2`"); the renderer uses the consumer Ty's
full pointer-type rendering (which carries
mutability + borrow-flavor + sig info that the
pointee-only payload does not).

**Admissible consumer Ty kinds.** Per `spec.md` §9.7's
bridge dispatch table (line 3243: `Scalar(Scalar::Int(_))
over a pointer type, all-zero -> NullPtr(pointee_ty)`),
the bridge produces `Const::NullPtr` only for
**thin** pointer types (single-scalar layout). The
admissible consumer Tys are:

| Consumer `Ty.kind()`                            | Rendered form                                                | Notes                                                                                       |
|-------------------------------------------------|--------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `Ty::Ref(T, Provenance::SharedBorrow)`          | `0x0 as &<render_ty(T)>`                                     | Thin shared reference (pointee `T` is sized).                                               |
| `Ty::Ref(T, Provenance::UniqueBorrow)`          | `0x0 as &mut <render_ty(T)>`                                 | Thin unique reference.                                                                      |
| `Ty::Ptr(T, Mutability::Not)`                   | `0x0 as *const <render_ty(T)>`                               | Thin raw const pointer.                                                                     |
| `Ty::Ptr(T, Mutability::Mut)`                   | `0x0 as *mut <render_ty(T)>`                                 | Thin raw mut pointer.                                                                       |
| `Ty::FnPtr(sig_id)`                             | `0x0 as <render_ty(sig)>` (yielding e.g. `fn(u32) -> bool`) | Function pointer type per §6.5. Function pointers are thin (one scalar).                    |
| Any non-thin / non-pointer Ty                   | Cannot occur                                                 | Fat pointers (`&dyn Trait`, `&[T]`, `&str`, raw fat pointers) are 2-scalar; the bridge would produce `Aggregate([NullPtr, NullPtr])` or similar, not a single `NullPtr`. Other consumer Tys are bridge bugs. Renderer panics. |

The renderer delegates the cast-type form to
`render_ty(consumer_ty)` per stage-13 D-13-5; the
form is `&<inner>` / `&mut <inner>` / `*const
<inner>` / `*mut <inner>` / `fn(<args>) -> <ret>`
per `pretty-printer-snapshot.md` §6.2 / §6.5.

**Consistency invariant** (bridge-guaranteed; not
re-validated at render time): for an `Operand` /
position holding a `Const::NullPtr(pointee_ty)`, the
consumer Ty is a thin pointer type whose pointee
equals `pointee_ty`. The bridge validates this at
construction per `spec.md` §3.3 Phase B step 9
("`NullPtr` pointee resolves"); the renderer trusts
the invariant and uses the consumer Ty for rendering
(richer than the bare pointee).

**Reasoning.**

1. **Rustc-mirroring is the established convention.**
   Rustc's `pretty_print_const_scalar_int` pointer-Ty
   arm at `rustc_middle/src/ty/print/pretty.rs:1816-1826`
   produces `0x{data:x} as <ty>` for any scalar over
   `Ref` / `RawPtr` / `FnPtr` (with `data = 0` for
   null). RegseqModel matches this form exactly for
   the null case. *Compiler-theory-first* (README §2).
2. **Consumer Ty is the authoritative source for the
   cast-type form.** The variant payload `pointee_ty`
   carries only the pointee for canonical-equality
   purposes (so that `*const u32` null and `*const
   u8` null are distinct values); the consumer Ty
   carries the full pointer-type structure
   (mutability + borrow-flavor + FnPtr signature).
   The rendering needs the full structure, so the
   consumer Ty wins. *C semantics with information
   preservation* (README §2) -- use the richer source.
3. **No fat-pointer null handling.** Fat-pointer null
   (`null::<dyn Trait>()`, `null::<[T]>()`,
   `null::<str>()`) is two-scalar at the rustc layout
   level; the bridge produces `Aggregate([NullPtr,
   NullPtr])` (or similar; the exact form is
   D-14-4.4's responsibility). The single
   `Const::NullPtr` value is reserved for thin
   pointers. *No defensive fallbacks* (README §2) --
   the bridge's dispatch is well-defined; the
   renderer doesn't add a fat-pointer fallback path.

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt rustc's
  typed-cast convention.
- *C semantics with information preservation*
  (README §2). Use the consumer Ty (richer) rather
  than the variant's pointee-only payload.
- *No defensive fallbacks* (README §2). Fat-pointer
  consumer Tys cannot occur for `Const::NullPtr` per
  the bridge dispatch; renderer rejects with panic.

**Prior-art evidence.** Rustc's
`pretty_print_const_scalar_int` pointer-Ty arm at
`rustc_middle/src/ty/print/pretty.rs:1816-1826`. The
`typed_value` helper at the same file produces
`<value> as <type>` form. Driver authors familiar
with MIR dumps recognise `0x0 as *const u32` for null
raw pointers, `0x0 as &u32` for null references
(though shared null references are UB in Rust; the
form still appears in MIR dumps for the value level).

**Spec impact** (D-14-4.7):

- `pretty-printer-snapshot.md` §10.1.6: replace the
  current stub (`(Pending D-14-4.7 lock.)`) with the
  dispatch table + admissible-Ty-kinds note +
  consistency-invariant note.
- No `spec.md` edits expected (the `Const::NullPtr`
  variant definition at §9.7 is unchanged; the
  rendering is consumer-side).
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise NullPtr rendering;
  specimens for null `*const u32` / `*mut u8` /
  `&u32` (illustrating the typed-cast form) will land
  alongside the D-14-4 cluster close
  specimen-augmentation step.

**Worked example.**

```text
Const::NullPtr(<u32 TyId>) + Ty::Ptr(<u32 TyId>, Mutability::Not)        ->  0x0 as *const u32
Const::NullPtr(<u8 TyId>)  + Ty::Ptr(<u8 TyId>, Mutability::Mut)         ->  0x0 as *mut u8
Const::NullPtr(<u32 TyId>) + Ty::Ref(<u32 TyId>, Provenance::SharedBorrow)  ->  0x0 as &u32
Const::NullPtr(<u32 TyId>) + Ty::Ref(<u32 TyId>, Provenance::UniqueBorrow) ->  0x0 as &mut u32
Const::NullPtr(<sig TyId>) + Ty::FnPtr(<sig FnSigId>)                    ->  0x0 as fn(u32) -> bool
  // (where the FnSig is fn(u32) -> bool)

// Cannot-occur cases (bridge cannot produce; renderer panics):
Const::NullPtr(_)          + Ty::Ref(<[u32] TyId>, _)                    ->  panic: fat-pointer consumer
Const::NullPtr(_)          + Ty::Ref(<dyn Trait TyId>, _)                ->  panic: fat-pointer consumer
Const::NullPtr(_)          + Ty::Int(I32)                                ->  panic: non-pointer consumer; bridge bug
```

#### D-14-4.8 -- `Reloc(InstanceRef)` per-arm rendering [LOCKED]

**Decision.** `Const::Reloc(instance_ref)` dispatches
on the resolved Instance's kind and synthetic-Item
classification. The renderer:

1. Resolves `instance_ref` to `InstanceId` via the
   model's instance-key mapping (F4-2; bridge-validated).
2. Looks up `instance = model.instances[InstanceId]`.
3. For Static instances, looks up
   `item_synthetic_kinds[instance.item]` to
   distinguish source-level statics from synth Items
   (vtable instance / ConstMemoryStatic).
4. Dispatches on `(instance.kind, consumer Ty)`:

| Resolved instance.kind / synth classification                      | Consumer `Ty.kind()`                                       | Rendered form                                                                                              |
|--------------------------------------------------------------------|------------------------------------------------------------|------------------------------------------------------------------------------------------------------------|
| `InstanceKind::Function { .. }`                                    | `Ty::FnDef(item, args)` where `(item, args)` matches       | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)>` (no cast)                   |
| `InstanceKind::Function { .. }`                                    | `Ty::FnPtr(sig_id)`                                        | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)> as <render_ty(consumer)>`    |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = None` (source-defined)     | Any `Ty::Ref(_, _, _)` or `Ty::Ptr(_, _)`     | `<render_def_path(instance.item)> as <render_ty(consumer)>`                                                |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = VTableInstance { .. }`     | Any pointer consumer Ty                       | `<render_def_path(instance.item)> as <render_ty(consumer)>` (synth path per §13.4 vtable-family)           |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = ConstMemoryStatic { .. }`  | `Ty::Ref(_, Ty::Array(<u8>, N), _)` (thin reference to byte array) AND init resolves to `Const::Bytes(vec)` of all `Byte::Value(_)` | `b"<escape-ascii of init bytes>"` (rustc-mirrored special case per `rustc_middle/src/ty/print/pretty.rs:1727-1753`) |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = ConstMemoryStatic { .. }`  | `Ty::Ref(_, Ty::Array(<u8>, N), _)` AND init Bytes is NOT all-Value (has Uninit / Reloc bytes) | `<render_def_path(instance.item)> as <render_ty(consumer)>` (structural fallback; special-case shape declined) |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = ConstMemoryStatic { .. }`  | Other thin-pointer consumer Tys (`Ty::Ptr(_, _)`, `Ty::Ref(_, non-byte-array, _)`)  | `<render_def_path(instance.item)> as <render_ty(consumer)>` (typed cast; the synth-Item path renders as the §13.4 const-memory-family DefPath) |
| `InstanceKind::Const { .. }` / `InstanceKind::Struct` / `InstanceKind::Enum` / `InstanceKind::Union` | Any consumer Ty                                                            | Cannot occur (bridge doesn't produce `Reloc` to non-Function-non-Static instances); renderer panics       |
| `InstanceKind::Function { .. }`                                    | Non-FnDef / non-FnPtr consumer Ty                          | Cannot occur (bridge produces `Reloc` to Function only when consumer Ty is FnDef/FnPtr); renderer panics  |
| `InstanceKind::Static { .. }`                                      | Non-pointer consumer Ty                                    | Cannot occur (bridge produces `Reloc` to Static only when consumer Ty is a pointer type); renderer panics |

**`b"..."` byte-string special case** (the only
RegseqModel-rich form beyond rustc-mirroring):

Pre-conditions to apply the special case:
1. Consumer Ty is `Ty::Ref(_, Ty::Array(u8_ty, N), _)`
   where `u8_ty` resolves to `Ty::Uint(UintKind::U8)`.
2. `instance_ref` resolves to an Instance with
   `InstanceKind::Static { ty: byte_array_ty, init }`.
3. `item_synthetic_kinds[instance.item] ==
   SyntheticItemKind::ConstMemoryStatic { .. }`.
4. `init == Some(bytes_const_id)` and
   `model.consts[bytes_const_id] == Const::Bytes(vec)`.
5. Every byte in `vec` is `Byte::Value(b)` (no
   `Byte::Uninit` or `Byte::Reloc(_)` bytes).
6. `vec.len() == N` (matches the consumer array length).

If all pre-conditions hold, render as `b"<escape>"`
where `<escape>` is the `byte_string.escape_ascii()`
form per Rust convention (printable ASCII renders
literally; non-printable / non-ASCII renders as
`\xHH` hex escapes; backslash and double-quote escape
as `\\` and `\"`). This mirrors rustc's
`pretty_print_byte_str` at `mir/pretty.rs:1831-1833`
(`write!(fmt, "b\"{}\"", byte_str.escape_ascii())`).

If any pre-condition fails, the structural fallback
applies: `<render_def_path(instance.item)> as
<render_ty(consumer)>` (typed-cast form, mirroring
the source-Static and VTable cases).

**Function-Reloc FnDef vs FnPtr dispatch.** The bridge
produces `Const::Reloc(fn_ref)` for both:
- Direct function-value references where the use-site
  type is `Ty::FnDef(item, args)` (no coercion).
- Function-to-FnPtr coercion sites where the use-site
  type is `Ty::FnPtr(sig)` (an explicit `as fn(...) ->
  ...` cast or implicit coercion to fn pointer).

The renderer distinguishes by consumer Ty:
- `Ty::FnDef(item, args)`: bare path (matches D-14-4.3
  ZST-over-FnDef form). Note: ZST and Reloc over the
  same `Ty::FnDef(item, args)` render identically; this
  is intentional -- the bridge may produce either
  `Const::ZST` or `Const::Reloc(fn_ref)` depending on
  the rustc representation (`ConstValue::ZeroSized` ->
  ZST per §9.7 line 3245; `ConstValue::Scalar(Scalar::Ptr(...))`
  -> Reloc per §9.7 line 3244), but the resulting
  rendered form is the same value form.
- `Ty::FnPtr(sig)`: typed-cast form
  `<path><args> as fn(...) -> ...` (the explicit fn-ptr
  coercion is preserved at the rendering surface).

**Reasoning.**

1. **Rustc-mirroring on the byte-string case.**
   Rustc's `pretty_print_const_scalar_ptr` byte-string
   arm at `ty/print/pretty.rs:1727-1753` produces
   `b"<bytes>"` for `Ref(_, Array(u8, N))` with
   memory-allocation provenance. RegseqModel matches
   this exactly for the `ConstMemoryStatic` synth
   Static case. The bridge-synthesized
   `ConstMemoryStatic { content_hash }` Item carries
   the byte payload in its `init` field per §9.7's
   reference-typed const handling; the renderer
   resolves through this chain to the byte payload.
   *Compiler-theory-first* (README §2).
2. **Typed-cast as the generic form.** For
   non-byte-string Statics (source-defined, vtable,
   ConstMemoryStatic with mixed Uninit / Reloc bytes,
   other pointee types), the typed-cast form
   `<path> as <ty>` matches D-14-4.7's NullPtr
   convention and is the natural value form for "a
   pointer to this named storage." Driver authors
   reading MIR dumps recognise the form. The synth
   Item's path (per §13.4 const-memory-family DefPath
   segments) is verbose but stable and traceable.
3. **FnDef-vs-FnPtr dispatch preserves coercion info.**
   The bridge distinguishes the two cases at the
   `ConstValue` level (one is `ZeroSized` over
   `Ty::FnDef`, the other is `Scalar::Ptr` over
   `Ty::FnPtr`); the renderer preserves the
   distinction by producing either the bare path or
   the typed-cast form. This matches how rustc
   distinguishes `(ConstValue::ZeroSized, ty::FnDef)`
   (no `const` prefix at the operand layer; bare path
   at the value layer) from `(ConstValue::Scalar::Ptr,
   ty::FnPtr)` (typed-cast form). *C semantics with
   information preservation* (README §2).

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt rustc's
  byte-string special case + typed-cast generic form.
- *C semantics with information preservation*
  (README §2). FnDef-vs-FnPtr distinction preserved
  through the rendering surface.
- *No defensive fallbacks* (README §2). Cases that
  cannot occur per the bridge's §9.7 dispatch
  (`InstanceKind::Const`/`Struct`/`Enum`/`Union`,
  Function-Reloc with non-FnDef/FnPtr consumer,
  Static-Reloc with non-pointer consumer) reject with
  panic; no generic fallback.
- *Compositional behaviour beats special-case flags*
  (README §2). The `b"..."` special case falls out of
  the consumer-Ty + Bytes-payload conditions; no
  `is_byte_string` flag on the Reloc variant.

**Prior-art evidence.** Rustc's
`pretty_print_const_scalar_ptr` at
`rustc_middle/src/ty/print/pretty.rs:1719-1773`
covers the byte-string special case + typed-cast
fallback. The Function-Reloc dispatch matches rustc's
behavior at the same file: FnPtr-typed scalar
pointers use the `typed_value(<value path>, <fn ptr
type>, " as ")` form at lines 1755-1768.

**Spec impact** (D-14-4.8):

- `pretty-printer-snapshot.md` §10.1.7: replace the
  current stub (`(Pending D-14-4.8 lock.)`) with the
  full dispatch table + b"..." special-case
  pre-conditions + cannot-occur cases.
- No `spec.md` edits expected (the `Const::Reloc`
  variant and `InstanceRef`/`SyntheticItemKind`
  schemas are unchanged).
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise Reloc rendering;
  specimens covering FnDef-Reloc, FnPtr-Reloc-coercion,
  Static-Reloc, and byte-string-Reloc (`b"hello"`)
  will land alongside the D-14-4 cluster close
  specimen-augmentation step.

**Worked example.**

```text
// Function-Reloc, FnDef consumer (no cast):
Const::Reloc(<fn:foo>) + Ty::FnDef(<foo>, [u32])  ->  pl011::foo::<u32>

// Function-Reloc, FnPtr consumer (typed cast):
Const::Reloc(<fn:foo>) + Ty::FnPtr(<sig u32->bool>)  ->  pl011::foo as fn(u32) -> bool

// Source-defined Static, typed cast:
Const::Reloc(<static:MY_REG>) + Ty::Ptr(<u32>, Mut)  ->  pl011::MY_REG as *mut u32

// ConstMemoryStatic, byte-array consumer (special case b"..."):
Const::Reloc(<synth:cm_<hash>>) + Ty::Ref(<[u8; 5]>, SharedBorrow)
  where init = Const::Bytes([Value(b'h'), Value(b'e'), Value(b'l'), Value(b'l'), Value(b'o')])
  ->  b"hello"

// ConstMemoryStatic, byte-array consumer (special case fails on mixed bytes):
Const::Reloc(<synth:cm_<hash>>) + Ty::Ref(<[u8; 4]>, SharedBorrow)
  where init = Const::Bytes([Value(0), Uninit, Value(1), Reloc(...)])
  ->  <regseq_synth>::const_memory_static::<hash> as &[u8; 4]
       (structural fallback; special case declined per pre-condition 5)

// ConstMemoryStatic, non-byte-array consumer (typed cast):
Const::Reloc(<synth:cm_<hash>>) + Ty::Ref(<MyStruct>, SharedBorrow)
  ->  <regseq_synth>::const_memory_static::<hash> as &MyStruct

// VTable Static, typed cast:
Const::Reloc(<vtable:Foo:Bar>) + Ty::Ptr(<u8>, Not)
  ->  <regseq_synth>::vtable::Foo::Bar as *const u8
       (synth path per §13.4 vtable-family DefPath segments)

// Cannot-occur cases (renderer panics):
Const::Reloc(<some>) + Ty::Bool                              -> panic: non-pointer consumer
Const::Reloc(<fn:foo>) + Ty::Ref(<u32>, _)                   -> panic: Function-Reloc with non-FnDef/FnPtr consumer
Const::Reloc(<resolves to InstanceKind::Const>) + any Ty    -> panic: bridge bug; Const instances are inlined, not Reloc'd
```

#### D-14-4.4 -- `Aggregate(Vec<ConstId>)` per-arm rendering [LOCKED]

**Decision.** `Const::Aggregate(children)` dispatches
on the consumer `Ty.kind()` across three structural
categories (Array / Tuple / Struct) plus two
fat-pointer special cases (`&str` and `&[u8]`).
Children render via recursive `render_const(c_i,
per_field_ty_i)` calls; the per-field-type
resolution from the parent consumer Ty's layout is
pinned by D-14-4.9.

| Consumer `Ty.kind()`                                                                              | Rendered form                                                                                                                                                | Notes                                                                                                                                                                                                                                |
|---------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Array(elem, N)`                                                                              | `[<r(c_0, elem)>, <r(c_1, elem)>, ..., <r(c_{N-1}, elem)>]`                                                                                                  | Children count must equal `N` (bridge-validated). Each child rendered with `elem` as consumer Ty.                                                                                                                                  |
| `Ty::Tuple { fields: [T_0] }` (1-tuple)                                                           | `(<r(c_0, T_0)>,)`                                                                                                                                           | Trailing comma per Rust 1-tuple syntax.                                                                                                                                                                                            |
| `Ty::Tuple { fields: [T_0, T_1, ..., T_{n-1}] }` (n >= 2)                                         | `(<r(c_0, T_0)>, <r(c_1, T_1)>, ..., <r(c_{n-1}, T_{n-1})>)`                                                                                                 | Each child rendered with corresponding `T_i` as consumer Ty.                                                                                                                                                                       |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` where `fields.is_empty()`    | `<render_def_path(item)><render_generic_args(args, item)>`                                                                                                         | Aggregate with empty children for fieldless struct. Bridge canonicalises to `Const::ZST` per §9.7 typically; if Aggregate arrives, renders identically to the D-14-4.3 fieldless form.                                            |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` (tuple-synthetic field names `_<index>` per §3.4) | `<render_def_path(item)><render_generic_args(args, item)>(<r(c_0, fields[0].ty)>, <r(c_1, fields[1].ty)>, ...)`                       | Tuple-style struct (`struct Foo(T1, T2)`). Field-name pattern `_<index>` signals tuple-style per §3.4.                                                                                                                            |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` (named-field, non-synthetic names) | `<render_def_path(item)><render_generic_args(args, item)> { <fields[0].name>: <r(c_0, fields[0].ty)>, <fields[1].name>: <r(c_1, fields[1].ty)>, ... }`        | Named-field struct (`struct Foo { x: T1, y: T2 }`). Children count must equal `fields.len()`.                                                                                                                                     |
| `Ty::Ref(_, Ty::Str, _)` (fat pointer to str) AND aggregate has exactly 2 children matching the fat-pointer shape (Reloc + Uint(Usize)) AND the Reloc resolves to a ConstMemoryStatic synth Static with all-Value Bytes that decode as valid UTF-8 | `"<utf8 decoded bytes>"` | rustc-mirrored special case per `rustc_middle/src/mir/pretty.rs:1874-1879`. UTF-8 decoding via `String::from_utf8_lossy(data)` (lossy for invalid bytes).                                                                          |
| `Ty::Ref(_, Ty::Slice(elem), _)` where `elem == Ty::Uint(U8)` AND aggregate has exactly 2 children matching the fat-pointer shape AND the Reloc resolves to a ConstMemoryStatic with all-Value Bytes | `b"<escape-ascii decoded bytes>"` | rustc-mirrored special case per `rustc_middle/src/mir/pretty.rs:1880-1888`. Byte-string via `escape_ascii()`.                                                                                                                       |
| Other `Ty::Ref(_, Ty::Slice(_), _)` (fat pointer to non-u8 slice) OR fat-pointer special-case pre-conditions fail | `(<r(c_0, _)>, <r(c_1, _)>) as <render_ty(consumer)>` -- per D-14-4.9 child-Ty resolution                                       | Structural fallback for non-special fat pointers; the child-Ty resolution from the consumer's fat-pointer ABI (`(data_ptr_ty, usize)`) is pinned by D-14-4.9.                                                                       |
| Other `Ty::Ref(_, _, _)` with 2-element Aggregate matching fat-pointer shape (`dyn Trait` references etc.) | `(<r(c_0, _)>, <r(c_1, _)>) as <render_ty(consumer)>` -- per D-14-4.9                                                                                          | Structural fallback for `dyn Trait` references and other fat-pointer cases.                                                                                                                                                       |
| `Ty::Enum { .. }` or `Ty::Union { .. }`                                                          | Cannot occur                                                                                                                                                 | Enum aggregates are produced as `Const::EnumValue` (D-14-4.5); union aggregates would need field disambiguation not present in `Const::Aggregate`. Renderer panics. (Note: `AggregateKind::Union` at the Rvalue level is different.)                                                                                |
| Any other `Ty.kind()` (Bool / Int / Uint / Float / Char / FnDef / FnPtr / Closure / scalar pointers / etc.) | Cannot occur                                                                                                                                                 | Non-aggregate consumer Tys do not consume `Const::Aggregate`; the bridge produces scalar/Reloc/EnumValue variants instead. Renderer panics (bridge bug).                                                                            |

**Children count invariant** (bridge-validated; not
re-checked at render time): for each Aggregate +
consumer-Ty pair, the children count matches the
structural cardinality of the consumer Ty:
- `Ty::Array(_, N)`: children count == N.
- `Ty::Tuple(fields)`: children count == fields.len().
- `Ty::Struct(layout)`: children count ==
  layout.kind.as_struct().fields.len().
- Fat-pointer Ref cases: children count == 2 (the
  data pointer and the metadata).

**Tuple-vs-named-field dispatch** for `Ty::Struct`
follows §3.4 (same rule as D-14-4.3): synthetic
`_<index>` field names signal tuple-style;
non-synthetic names signal named-field-style; empty
fields signal unit-style. Per stage-12 D-12-6, the
layout's field names are non-optional and resolved
at bridge time.

**Fat-pointer special-case pre-conditions**
(for both `&str` and `&[u8]` arms):

1. Aggregate has exactly 2 children.
2. `children[0]` resolves to `Const::Reloc(InstanceRef)`.
3. The InstanceRef resolves to an Instance with
   `InstanceKind::Static { ty, init: Some(bytes_const_id) }`
   and `item_synthetic_kinds[item] ==
   SyntheticItemKind::ConstMemoryStatic { .. }`.
4. `model.consts[bytes_const_id] == Const::Bytes(vec)`
   where every byte is `Byte::Value(b)` (no Uninit /
   Reloc).
5. `children[1]` resolves to `Const::Uint(UintKind::Usize, len)`
   where `len == vec.len() as u128`.
6. For the `&str` arm: `String::from_utf8(vec)` would
   succeed -- but rustc and RegseqModel both use
   `from_utf8_lossy()` so technically any byte
   sequence renders (invalid bytes become `U+FFFD`).
   No strict UTF-8 pre-condition.

If any pre-condition fails (except #6 which is
lossy-by-design), the structural fallback applies.

**Recursion shape.** `r(c_i, T_i)` denotes
`self.render_const(c_i, t_i)` where `t_i` is the
i-th sub-field's TyId resolved from the parent
consumer Ty's structural layout per D-14-4.9. The
resolution rule (pinned at D-14-4.9):
- `Ty::Array(elem, N)` -> per-child `t_i = elem`.
- `Ty::Tuple { fields }` -> per-child `t_i = fields[i]`.
- `Ty::Struct { layout: { kind: Struct { fields } } }` -> per-child `t_i = fields[i].ty`.
- Fat-pointer `Ty::Ref(_, _, _)` with fallback -> per-child Tys derived from the rustc fat-pointer ABI (data-ptr + usize).

**Reasoning.**

1. **Rustc-mirroring on Array / Tuple / Struct +
   `&str` / `&[u8]`.** Rustc's
   `pretty_print_const_value_tcx` at
   `mir/pretty.rs:1907-1966` dispatches on consumer
   `Ty.kind()` for `Array / Tuple / Adt`; the special
   cases at lines 1874-1898 cover `&str` / `&[u8]` /
   `[u8; N]` byte-string forms. RegseqModel matches
   rustc's per-arm forms exactly for the common
   cases. *Compiler-theory-first* (README §2).
2. **Exhaustive consumer-Ty coverage with
   bridge-rejection-shaped panics.** Per §9.7's
   dispatch table, `Const::Aggregate` is produced
   only for structural-aggregate consumer Tys (Array
   / Tuple / Struct) and for the fat-pointer wide-
   reference cases (`Slice` dispatch at line 3250).
   Other consumer Tys (Enum / Union / scalars /
   pointers-non-fat) cannot occur per the bridge's
   well-defined dispatch; renderer panics. *No
   defensive fallbacks* (README §2).
3. **Special-case detection is value-driven, not
   flag-driven.** The `b"..."` and `"..."` forms
   require verifying the fat-pointer pattern through
   the InstanceRef resolution chain (Reloc -> synth
   Static -> Bytes payload -> all-Value bytes). The
   detection is mechanical and falls out of existing
   structural data; no `is_byte_string` flag on the
   Aggregate variant. *Compositional behaviour beats
   special-case flags* (README §2).

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Adopt rustc's
  per-Ty-kind dispatch + `&str` / `&[u8]` special
  cases.
- *C semantics with information preservation*
  (README §2). Per-child consumer Tys preserved
  through the structural layout of the parent
  consumer Ty.
- *No defensive fallbacks* (README §2). Cannot-occur
  consumer Tys reject with panic.
- *Compositional behaviour beats special-case flags*
  (README §2). Byte-string detection is value-driven
  via pre-condition checks; no flag.
- *Spec is mutable* (README §2). Stage-13's existing
  AggregateKind rendering at §7.3.2 produces similar
  forms for Rvalue::Aggregate; this D-14-4.4 spec
  for Const::Aggregate is the value-level analog.

**Prior-art evidence.** Rustc's
`pretty_print_const_value_tcx` at
`rustc_middle/src/mir/pretty.rs:1851-1989` --
specifically the special-case branches at lines
1874-1898 (`&str` / `&[u8]` / `[u8; N]`) and the
structural branches at lines 1907-1966 (Array /
Tuple / Adt). The `comma_sep` helper at lines
1835-1849 handles the recursion. The `(child
ConstValue, child Ty)` pair returned by
`try_destructure_mir_constant_for_user_output` (line
1910) is rustc's analog of D-14-4.9's per-field-Ty
resolution.

**Spec impact** (D-14-4.4):

- `pretty-printer-snapshot.md` §10.1.3: replace the
  current stub (`(Pending D-14-4.4 lock.)`) with the
  full dispatch table + recursion shape +
  fat-pointer special-case pre-conditions + tuple-vs-
  named-field reference to §3.4 + cannot-occur cases.
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise Const::Aggregate
  rendering at the D-14-4 level (existing Aggregate
  specimens at lines 1211-1271 are for Rvalue::Aggregate,
  not Const::Aggregate); specimens for
  `[1_u32, 2_u32, 3_u32]` array const, `(1_i32, true)`
  tuple const, `Point { x: 0_i32, y: 1_i32 }` struct
  const, `"hello"` &str const, `b"data"` &[u8] const
  will land alongside the D-14-4 cluster close
  specimen-augmentation step.

**Worked example.**

```text
// Array Aggregate:
Const::Aggregate([<c1>, <c2>, <c3>]) + Ty::Array(<u8>, 3)
  where c1=Uint(U8,1), c2=Uint(U8,2), c3=Uint(U8,3)
  ->  [1_u8, 2_u8, 3_u8]

// Tuple Aggregate:
Const::Aggregate([<c1>, <c2>]) + Ty::Tuple([<i32>, <bool>])
  where c1=Int(I32,42), c2=Bool(true)
  ->  (42_i32, true)

// 1-tuple Aggregate (trailing comma):
Const::Aggregate([<c1>]) + Ty::Tuple([<i32>])
  ->  (42_i32,)

// Struct Aggregate (named-field):
Const::Aggregate([<c1>, <c2>]) + Ty::Struct(<Point>, [], { fields: [x: i32, y: i32] })
  ->  geom::Point { x: 0_i32, y: 1_i32 }

// Struct Aggregate (tuple-style, synthetic _<index> names):
Const::Aggregate([<c1>, <c2>]) + Ty::Struct(<Pair>, [], { fields: [_0: i32, _1: bool] })
  ->  pl011::Pair(0_i32, true)

// &str literal (special case):
Const::Aggregate([<Reloc(synth_cm)>, <Uint(Usize, 5)>]) + Ty::Ref(<str>, SharedBorrow)
  where synth_cm resolves to ConstMemoryStatic with init Const::Bytes([Value(b'h'), Value(b'e'), Value(b'l'), Value(b'l'), Value(b'o')])
  ->  "hello"

// &[u8] literal (special case):
Const::Aggregate([<Reloc(synth_cm)>, <Uint(Usize, 4)>]) + Ty::Ref(<[u8]>, SharedBorrow)
  where synth_cm resolves to ConstMemoryStatic with init Const::Bytes([Value(0), Value(1), Value(2), Value(3)])
  ->  b"\x00\x01\x02\x03"

// Fat-pointer fallback (&[u32] -- non-byte slice):
Const::Aggregate([<Reloc(synth_cm)>, <Uint(Usize, 3)>]) + Ty::Ref(<[u32]>, SharedBorrow)
  ->  (<regseq_synth>::const_memory_static::<hash> as *const u32, 3_usize) as &[u32]
       (structural fallback via D-14-4.9 child-Ty resolution)

// Cannot-occur cases (renderer panics):
Const::Aggregate([...]) + Ty::Enum(...)      -> panic: Enum aggregates use Const::EnumValue
Const::Aggregate([...]) + Ty::Int(I32)       -> panic: scalar consumer; bridge bug
Const::Aggregate([<c>]) + Ty::Array(<u8>, 2) -> panic: children count != N (bridge bug; should be caught at validation)
```

#### D-14-4.5 -- `EnumValue { variant_idx, payload }` per-arm rendering [LOCKED]

**Decision.** `Const::EnumValue { variant_idx,
payload }` dispatches on the consumer `Ty::Enum`'s
`EnumShape` to locate the named variant, then on the
variant's `fields` to determine the ctor form (unit /
tuple / named-field). Payload children render via
recursive `render_const(p_i, fields[i].ty)` per
D-14-4.9 child-Ty resolution.

| Consumer Ty + variant shape                                                                                                  | Rendered form                                                                                                          | Notes                                                                                                                                                  |
|------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }` AND `V.fields.is_empty()` AND `payload.is_empty()` | `<enum_path>::<V.name>` where `enum_path = <render_def_path(item)><render_generic_args(args, item)>`                         | Single-variant unit-variant enum. Variant index must be 0 (single variant).                                                                            |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` AND `V.fields` non-empty AND `payload.len() == V.fields.len()` AND tuple-synthetic field names | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, <r(p_1, V.fields[1].ty)>, ...)`                                       | Single-variant tuple-variant enum.                                                                                                                     |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` AND `V.fields` non-empty AND named field names                                                  | `<enum_path>::<V.name> { <V.fields[0].name>: <r(p_0, V.fields[0].ty)>, ... }`                                          | Single-variant named-field-variant enum.                                                                                                               |
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Multiple { variants, .. } }, .. } }` AND `variant_idx in 0..variants.len()` AND `variants[variant_idx].fields.is_empty()` AND `payload.is_empty()` | `<enum_path>::<V.name>` where `V = variants[variant_idx]`                                                              | Multi-variant enum, unit variant (e.g., `Option::None`).                                                                                               |
| `Ty::Enum { ... EnumShape::Multiple { variants, .. } }` AND `V = variants[variant_idx]` AND `V.fields` non-empty AND tuple-synthetic names                | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, ...)`                                                                 | Multi-variant enum, tuple variant (e.g., `Option::Some(42_i32)`).                                                                                      |
| `Ty::Enum { ... EnumShape::Multiple { variants, .. } }` AND `V = variants[variant_idx]` AND `V.fields` non-empty AND named names                          | `<enum_path>::<V.name> { <V.fields[0].name>: <r(p_0, V.fields[0].ty)>, ... }`                                          | Multi-variant enum, named-field variant (e.g., `Result::Err { reason: "oops" }`).                                                                      |
| `Ty::Enum { ... EnumShape::Empty }`                                                                                          | Cannot occur                                                                                                           | Zero-variant uninhabited enum has no values; renderer panics.                                                                                          |
| `variant_idx` out of bounds for the consumer's `EnumShape.variants`                                                          | Cannot occur                                                                                                           | Bridge-validated invariant per `spec.md` §3.3 Phase B step 9 (`EnumValue.variant_idx` is in-bounds for the consumer's `Ty::Enum`); renderer panics.    |
| `payload.len()` does not match `V.fields.len()`                                                                              | Cannot occur                                                                                                           | Bridge-validated invariant; renderer panics.                                                                                                           |
| Any non-`Ty::Enum` consumer Ty                                                                                               | Cannot occur                                                                                                           | `Const::EnumValue` is produced only for enum consumer Tys; renderer panics (bridge bug).                                                               |

**Variant name lookup**: per §3.4, the variant name is
resolved via the `EnumShape` field:
- `EnumShape::Single { variant: V, .. }` -> `V.name`
  (a `StringId`; resolved via `model.strings.resolve(V.name)`).
- `EnumShape::Multiple { variants, .. }` ->
  `variants[variant_idx].name`.

`VariantLayout.name` is non-optional per `spec.md`
§11 -- missing variant names are bridge bugs caught at
validation, not renderer-side fallback.

**Tuple-vs-named-field dispatch** for the variant's
fields follows §3.4 (same rule as D-14-4.3 and
D-14-4.4): synthetic `_<index>` names signal
tuple-style; non-synthetic names signal named-field-
style; empty fields signal unit-style.

**Reasoning.** Same three reasons as D-14-4.4
(Aggregate): rustc-mirroring, exhaustive consumer-Ty
coverage with bridge-rejection-shaped panics, value-
driven dispatch. The split between D-14-4.4 and
D-14-4.5 reflects the bridge dispatch at §9.7: rustc's
valtree-based aggregate dispatch produces either
`Aggregate(_)` (non-Adt) or `EnumValue { variant_idx,
payload }` (Adt) depending on the underlying type.
RegseqModel preserves this distinction structurally.

**Methodology rules invoked.** Same as D-14-4.4:
*Compiler-theory-first*, *C semantics with
information preservation*, *No defensive fallbacks*,
*Compositional behaviour beats special-case flags*.

**Prior-art evidence.** Rustc's
`pretty_print_const_value_tcx` Adt arm at
`rustc_middle/src/mir/pretty.rs:1929-1962`. The
variant lookup pattern (`def.variant(variant_idx)`)
+ ctor-kind dispatch (`Some(CtorKind::Const) |
Some(CtorKind::Fn) | None`) matches RegseqModel's
unit / tuple / named-field dispatch via §3.4's
field-name conventions.

**Spec impact** (D-14-4.5):

- `pretty-printer-snapshot.md` §10.1.4: replace the
  current stub (`(Pending D-14-4.5 lock.)`) with the
  full dispatch table + variant name lookup +
  tuple-vs-named-field reference to §3.4 + cannot-
  occur cases.
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise Const::EnumValue
  rendering; specimens covering
  `core::option::Option<u32>::Some(42_u32)`,
  `core::option::Option<u32>::None`,
  `pl011::Color::Red`, and a struct-variant enum will
  land alongside the D-14-4 cluster close
  specimen-augmentation step.

**Worked example.**

```text
// Multi-variant enum, unit variant:
Const::EnumValue { variant_idx: 0, payload: [] } + Ty::Enum(<Option<u32>>, [u32])
  where variants[0] = "None" (fields empty)
  ->  core::option::Option::<u32>::None

// Multi-variant enum, tuple variant:
Const::EnumValue { variant_idx: 1, payload: [<c>] } + Ty::Enum(<Option<u32>>, [u32])
  where variants[1] = "Some" (fields [_0: u32]); c = Uint(U32, 42)
  ->  core::option::Option::<u32>::Some(42_u32)

// Multi-variant enum, named-field variant:
Const::EnumValue { variant_idx: 0, payload: [<c>] } + Ty::Enum(<MyEnum>, [])
  where variants[0] = "Err" (fields [reason: &str]); c = Aggregate(...) rendering as "oops"
  ->  pl011::MyEnum::Err { reason: "oops" }

// Single-variant enum (only variant):
Const::EnumValue { variant_idx: 0, payload: [] } + Ty::Enum(<E>, []) where EnumShape::Single { variant: V where V.name="V", V.fields=[] }
  ->  pl011::E::V

// C-style enum (multi-variant, all unit):
Const::EnumValue { variant_idx: 0, payload: [] } + Ty::Enum(<Color>, []) where variants[0]="Red"
  ->  pl011::Color::Red

// Cannot-occur cases (renderer panics):
Const::EnumValue { variant_idx: 99, payload: [] } + Ty::Enum(<small_enum>, []) -> panic: variant_idx out of bounds
Const::EnumValue { variant_idx: 0, payload: [<c1>] } + Ty::Enum(...)         -> panic: payload arity mismatch with variant.fields
Const::EnumValue { ... } + Ty::Int(I32)                                       -> panic: non-enum consumer; bridge bug
```

#### D-14-4.6 -- `Bytes(Vec<Byte>)` per-arm rendering [LOCKED]

**Decision.** `Const::Bytes(vec)` dispatches on
consumer `Ty.kind()` across the byte-array case +
all-uninit shortcut + structural fallback. The bridge
produces `Const::Bytes` per §9.7's dispatch table for
`ConstValue::Indirect { alloc, .. }` cases NOT
matched by the `Slice { data, meta }` path (which
produces `Aggregate([Reloc, Uint])`) -- so the
consumer Tys for `Const::Bytes` are constrained to
those where rustc's structural valtree extraction
fails or the value has provenance bytes / non-trivial
init_mask.

| Bytes payload + consumer `Ty.kind()`                                                                                                                                                                                                                | Rendered form                                                                                                                | Notes                                                                                                                                                          |
|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| All-uninit bytes (every byte is `Byte::Uninit`)                                                                                                                                                                                                     | `<uninit>`                                                                                                                   | Rustc-mirrored shortcut per `rustc_middle/src/mir/pretty.rs:1864-1869` (`if ct.all_bytes_uninit(tcx)`). Applies for any consumer Ty.                            |
| All-`Byte::Value(b)` (no Uninit, no Reloc) AND consumer `Ty == Ty::Array(<u8>, N)` AND `vec.len() == N`                                                                                                                                              | `*b"<escape-ascii>"`                                                                                                         | Rustc-mirrored deref byte-array form per `rustc_middle/src/mir/pretty.rs:1889-1898` (`fmt.write_str("*")?; pretty_print_byte_str(fmt, byte_str)?;`).             |
| Other consumer Tys (composite structs / tuples / arrays / unions / any non-`Ty::Array(<u8>, _)` consumer) with mixed Uninit / Value / Reloc bytes | `bytes![<byte_0>, <byte_1>, ..., <byte_{n-1}>]` where each `<byte_i>` is one of: `0xHH` (for `Byte::Value(b)`; lowercase hex), `<uninit>` (for `Byte::Uninit`), `<reloc: <render_def_path>>` (for `Byte::Reloc(InstanceRef)`) | Structural fallback. RegseqModel-richer than rustc (rustc falls back to `{ct:?}: {ty}` debug form at `mir/pretty.rs:1988`). Surface mirrors the 3-state `Byte` enum encoding from §9.7. |
| `Const::Bytes` consumed at `Ty::Ref(_, Ty::Str, _)`, `Ty::Ref(_, Ty::Slice(u8), _)`, or `Ty::Ref(_, Ty::Slice(_), _)`                                                                                                                                | Cannot occur                                                                                                                 | Bridge produces `Aggregate([Reloc, Uint])` for these consumer Tys per §9.7 dispatch line 3250 (`Slice { data, meta }`); `Const::Bytes` would be a bridge bug. Renderer panics. |
| `Const::Bytes` consumed at a scalar consumer Ty (`Ty::Bool` / `Ty::Int(_)` / `Ty::Uint(_)` / `Ty::Float(_)` / `Ty::Char`)                                                                                                                            | Cannot occur                                                                                                                 | Bridge produces the corresponding scalar variant for these consumer Tys per §9.7 dispatch lines 3238-3242. Renderer panics.                                    |

**All-uninit shortcut precedence.** The all-uninit
check is evaluated FIRST, before the byte-array
special case. So `Const::Bytes([Uninit; N])` consumed
at `Ty::Array(<u8>, N)` renders as `<uninit>`, not
`*b"\xc0\xc0\xc0..."`. Rustc's behavior at
`mir/pretty.rs:1864-1869` matches: the all-uninit
check returns early before the type-dispatch logic.

**`bytes![...]` structural fallback form.** The
fallback wraps the byte-list in a `bytes![...]`
pseudo-macro syntax to clearly signal "this is a
byte-encoding fallback, not a Rust literal." Per-byte
forms:

- `Byte::Value(b)`: `0xHH` lowercase hex (e.g.,
  `0x00`, `0xff`, `0x3a`). Two-digit padding ensures
  visual alignment in dumps.
- `Byte::Uninit`: `<uninit>` (matching the
  all-uninit shortcut form).
- `Byte::Reloc(instance_ref)`: `<reloc:
  <render_def_path(resolved_item)>>` -- the resolved
  Item's path provides identity. For run-of-relocation
  bytes (per §9.7 line 2946-2961 well-formedness:
  maximal relocation runs are pointer-shaped), the
  renderer emits one `<reloc: path>` token per run,
  with the byte count omitted (the rendered form
  doesn't carry width info; consumers needing precise
  byte offsets look at the structural IR, not the
  rendered dump). [This is a minor diagnostic-vs-
  precision tradeoff; revisit if phase-1 testing
  surfaces alignment-debugging needs.]

**Byte ordering.** The bytes render in the order
they appear in the `Vec<Byte>` -- target-endian
ordering as the bridge captured them from rustc's
`Allocation.bytes`. The renderer does NOT re-order
or re-interpret per consumer-Ty endianness.

**Reasoning.**

1. **Rustc-mirroring on the two well-defined cases.**
   The all-uninit shortcut and the `*b"..."` byte-
   array form match rustc verbatim. *Compiler-theory-
   first* (README §2).
2. **RegseqModel-richer structural fallback.** Rustc
   falls back to the debug form (`{ct:?}: {ty}`) for
   non-special Bytes cases. RegseqModel has full
   structural access to the 3-state `Byte` enum
   (via the bridge's preservation of
   `Allocation.bytes` / `init_mask` / `provenance`
   per §9.7's three-state encoding); rendering a
   structural `bytes![...]` form is information-
   preserving and diagnostically useful. *C
   semantics with information preservation* (README
   §2).
3. **All-uninit precedence is conservative.** The
   common case of `MaybeUninit<T>::uninit()` produces
   an all-uninit Bytes payload; collapsing this to
   `<uninit>` saves dump verbosity and matches the
   developer's intent (the value is "not initialised,"
   not "happens to be all-zero-padded"). Rustc does
   the same.

**Methodology rules invoked.**

- *Compiler-theory-first* (README §2). Match rustc's
  two well-defined special cases.
- *C semantics with information preservation*
  (README §2). The structural fallback preserves the
  3-state Byte enum at the rendering surface.
- *No defensive fallbacks* (README §2). Consumer
  Tys that the bridge cannot pair with
  `Const::Bytes` (`&str`, `&[u8]`, scalars) reject
  with panic.
- *Compositional behaviour beats special-case flags*
  (README §2). Detection is value-driven (all-uninit
  check + consumer-Ty match); no flag.

**Prior-art evidence.** Rustc's
`pretty_print_const_value_tcx` byte cases at
`rustc_middle/src/mir/pretty.rs:1864-1898`. The
all-uninit shortcut at line 1864-1869; the byte-array
case at 1889-1898. The structural fallback at line
1988 (`write!(fmt, "{ct:?}: {ty}")`) is rustc's
generic debug form for unhandled cases;
RegseqModel's `bytes![...]` form is richer.

**Spec impact** (D-14-4.6):

- `pretty-printer-snapshot.md` §10.1.5: replace the
  current stub (`(Pending D-14-4.6 lock.)`) with the
  full dispatch table + all-uninit precedence note +
  `bytes![...]` structural fallback per-byte form
  spec + byte-ordering note + cannot-occur cases.
- `pretty-printer-snapshot.md` §9 specimens: no
  current specimens exercise Const::Bytes rendering;
  specimens covering all-uninit (`<uninit>`),
  `*b"hello"` byte-array, and a mixed structural
  fallback (`bytes![0x00, <uninit>, 0xff, <reloc:
  path>]`) will land alongside the D-14-4 cluster
  close specimen-augmentation step.

**Worked example.**

```text
// All-uninit shortcut:
Const::Bytes([Uninit, Uninit, Uninit, Uninit]) + Ty::Array(<u8>, 4)
  ->  <uninit>

// Byte-array deref form:
Const::Bytes([Value(b'h'), Value(b'e'), Value(b'l'), Value(b'l'), Value(b'o')]) + Ty::Array(<u8>, 5)
  ->  *b"hello"

// Byte-array with escape-ascii:
Const::Bytes([Value(0x00), Value(0x01), Value(0xFF)]) + Ty::Array(<u8>, 3)
  ->  *b"\x00\x01\xff"

// Structural fallback (mixed):
Const::Bytes([Value(0x00), Uninit, Value(0xff), Reloc(<some_item>)]) + Ty::Struct(<MyStruct>, [], ...)
  ->  bytes![0x00, <uninit>, 0xff, <reloc: pl011::some_item>]

// Cannot-occur cases (renderer panics):
Const::Bytes(_) + Ty::Ref(<str>, _)              -> panic: &str uses Const::Aggregate per §9.7
Const::Bytes(_) + Ty::Bool                       -> panic: scalar consumer; bridge bug
Const::Bytes([Value(b'x'); 5]) + Ty::Array(<u8>, 4) -> panic: bytes.len() != array.N (bridge bug)
```

#### D-14-4.9 -- Type-context threading + operand-callsite wiring [LOCKED]

**Question.** D-14-4.4 (Aggregate) and D-14-4.5
(EnumValue) recurse on per-sub-field consumer Tys
(`r(c_i, T_i)`); D-14-4.3 (ZST) does the same for
composite-ZST cases. The recursion shape requires a
mechanism for resolving each child's Ty from the
parent consumer Ty's structure. Additionally,
D-14-4.1 (API signature) deferred the
operand-callsite wiring question: how does
`render_operand` thread the consumer Ty for
`Operand::Constant(id)` arms, given that
`Operand::Constant(ConstId)` does not carry a TyId
per stage-9 §20.1?

D-14-4.9 pins both: (a) the per-sub-field Ty
resolution mechanism, and (b) the operand-callsite
wiring contract (which extends stage-13 D-13-6's
`render_operand` signature).

**Decision.**

**(a) Per-sub-field Ty resolution** (recursion shape):
The renderer walks the parent consumer Ty's
structural layout to extract per-child Tys. The
resolution rules are:

| Parent consumer Ty                                                                | Per-child Ty resolution                                                                                                |
|-----------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| `Ty::Array(elem, N)`                                                              | Every child has Ty `elem`.                                                                                             |
| `Ty::Tuple { fields }`                                                            | The i-th child has Ty `fields[i]`.                                                                                     |
| `Ty::Struct { layout: { kind: Struct { fields }, .. } }`                          | The i-th child has Ty `fields[i].ty`.                                                                                  |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }`        | The i-th payload child has Ty `V.fields[i].ty`.                                                                        |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Multiple { variants, .. } }, .. } }`        | For the variant at `variant_idx`, the i-th payload child has Ty `variants[variant_idx].fields[i].ty`.                  |
| `Ty::Ref(_, pointee, _)` (fat-pointer fallback per D-14-4.4)                      | The 2 children have **interned** Tys looked up via `TyInterner`: data pointer = `Ty::Ptr(pointee, Mutability::Not)`; metadata = `Ty::Uint(UintKind::Usize)`. **Bridge interning invariant** (added per R1): when the bridge produces a `Const::Aggregate([Reloc, Uint])` for a fat-pointer consumer Ty per `spec.md` §9.7 dispatch (the `Slice { data, meta }` line plus `Ref(_, dyn Trait)` synthesis paths), it MUST ensure both `Ty::Ptr(pointee, Mutability::Not)` and `Ty::Uint(UintKind::Usize)` are present in `model.types` (interning them if absent). The renderer trusts the invariant and looks up the existing TyIds at render time without creating non-interned TyIds. This preserves `spec.md` §10's closed-`Ty`-enum + all-interned-TyId invariant. |

The resolution is **structural-data-driven**: the
`PrettyPrinter` has `&model` and walks the consumer
Ty's layout via the existing layout machinery (per
`spec.md` §11). No caller-side resolution is required
for recursive children -- the renderer resolves
internally as it descends.

**(b) Operand-callsite wiring** (stage-13 D-13-6
REOPEN): Per the R1 review finding, two stage-13
signatures cascade through the REOPEN:

```rust
// BEFORE (stage-13 D-13-6):
fn render_operand(&self, op: &Operand, body: &Body) -> String;
fn render_rvalue(&self, rv: &Rvalue, body: &Body) -> String;
fn render_generic_args(&self, args: &GenericArgs) -> String;

// AFTER (D-14-4.9):
fn render_operand(&self, op: &Operand, body: &Body, expected_ty: TyId) -> String;
fn render_rvalue(&self, rv: &Rvalue, body: &Body, expected_ty: TyId) -> String;
fn render_generic_args(&self, args: &GenericArgs, item: ItemId) -> String;
```

`render_rvalue` extends because `Rvalue::Use(operand)`
needs to propagate the consumer Ty to the operand (the
operand inherits the Rvalue's result type), and the
existing two-arg signature (`&Rvalue, &Body`) provides
no source for that type. `render_generic_args` extends
because `GenericArg::Const(c)` needs the enclosing
Item's generic-parameter list to resolve the declared
type of each const arg.

The `expected_ty` parameter for `render_operand` /
`render_rvalue` is the consumer Ty at the operand's /
Rvalue's use site. Callers resolve and pass it from
their structural context:

| Caller                                                  | `expected_ty` resolution                                                                                                                  |
|---------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|
| `render_statement` Assign(place, rvalue)                | Threads `place_ty(place, body)` to `render_rvalue(rvalue, body, expected_ty)`. Other StatementKind operand callsites: `Assume(operand)` -> `Ty::Bool`; `CopyNonOverlapping { src, dst, count }` -> `src`/`dst` get `Ty::Ptr(Ty::Tuple([]), _)` (raw const/mut pointer; pointee unit per rustc's intrinsic signature); `count` -> `Ty::Uint(UintKind::Usize)`. |
| `render_rvalue` (per-Rvalue-kind dispatch)              | Per `Rvalue::Use(_)`: operand expected_ty = `expected_ty` (transparent). Per `Rvalue::Repeat(op, _)`: operand expected_ty = the array element type (extracted from `expected_ty = Ty::Array(elem, count)`). Per `Rvalue::BinaryOp(op, lhs, rhs)`: both operands have the BinOp input type (for arithmetic, equal to `expected_ty`; for comparison, derived from operand inspection -- both operands share the same source-MIR type). Per `Rvalue::UnaryOp(_, op)`: operand expected_ty = `expected_ty`. Per `Rvalue::Cast(kind, op, ty)`: operand expected_ty = the source type per CastKind. Per `Rvalue::Aggregate(kind, ops)`: operand expected_ty derived from AggregateKind's structural payload (per §7.3.2 derivation column). |
| `render_terminator_kind` Call args                      | Each Call arg's expected_ty = `callee_fn_sig.inputs[i]` (resolved per stage-9's Call shape; for `Direct(id)`, `callee_fn_sig = model.instances[id].kind.as_function().fn_sig`'s resolved `FnSig.inputs`; for `Indirect(_)`, the callee operand's `Ty::FnPtr(sig_id)` resolves the signature via the FnSigInterner). The Call's `discr` for SwitchInt has expected_ty = the discriminant scalar type. |
| `render_call_callee` Indirect callees                   | Indirect operand expected_ty = `Ty::FnPtr(sig_id)` (the callee's function-pointer type at this call site). |
| `render_assert_kind` message operands                   | Per AssertKind variant: `BoundsCheck { len, index }` -> both `Ty::Uint(UintKind::Usize)`; `Overflow(op, l, r)` -> both share the BinOp's input type (numeric primitive); `OverflowNeg(operand)` / `DivisionByZero(operand)` / `RemainderByZero(operand)` -> the numeric primitive being checked; `MisalignedPointerDereference { required, found }` -> both `Ty::Uint(UintKind::Usize)`; `InvalidEnumConstruction(found)` -> the discriminant scalar type per the source enum's tag. |

The renderer's per-arm dispatch for `Operand::Copy /
Move(place)` ignores `expected_ty` (the place's type
is already resolvable via `place_ty(place, body)`);
only `Operand::Constant(id)` consumes it (threading
to `render_const(id, expected_ty)` per D-14-4.1's
pair-form). This makes `expected_ty` semi-redundant
for Copy / Move operands but uniformly required by
the signature -- a small API simplification (the
caller always passes it; the renderer always accepts
it; whether the arm uses it is internal).

**Stage-13 D-13-6 REOPEN.** This sub-decision flips
D-13-6 to `[REOPENED]` at merger close, with
annotation: "(revised in stage 14 D-14-4.9:
`render_operand` and `render_rvalue` signatures
extended with `expected_ty: TyId` parameter to thread
consumer-context Ty through to `render_const` per
D-14-4.1's pair-form; `render_generic_args` signature
extended with `item: ItemId` parameter to resolve
`GenericArg::Const` declared types from the enclosing
Item's generic-parameter list)". The §7.2 / §7.3 /
§6.7 specs are rewritten in place per the merger
contract (§5.9 rule "refined content updates existing
sections in place"). The §7.4 / §7.5 / §7.6 / §7.7 /
§7.8 / §7.9 caller-side specs add `expected_ty`
resolution rules (per the caller table above).
Stage-13 §6.4 ADT-rendering callsites of
`render_generic_args` update to pass `item` (already
adjacent in `Ty::Struct/Enum/Union/Closure/FnDef`
payload).

The §7.2 `Constant(id)` arm spec changes from
`<render_const(id)>` to
`<render_const(id, expected_ty)>` (matching the
pair-form). The §10.1 `render_const` introduction's
operand-callsite paragraph (previously deferred to
D-14-4.9) updates to reflect the wiring.

**Reasoning.**

1. **Per-sub-field Ty resolution is structural-
   layout-driven; the renderer has `&model` to walk
   layouts.** Mirrors rustc's
   `try_destructure_mir_constant_for_user_output`
   pattern at `rustc_middle/src/mir/pretty.rs:1910`,
   which returns `(child ConstValue, child Ty)` pairs
   resolved from the parent's structure. Rustc does
   this via `tcx.layout_of(ty)` walking; RegseqModel
   does it via the bridge-preserved layout schemas at
   §11 (which carry per-field types directly --
   `FieldLayout.ty: TyId`). The resolution is
   mechanical and falls out of existing structural
   data. *C semantics with information preservation*
   (README §2).
2. **Operand-callsite wiring matches D-14-4.1's
   pair-form principle.** The same reasoning that
   forced the pair-form for `render_const` --
   consumer-context Ty is required for non-scalar
   Const arms, and `Const` storage doesn't carry the
   Ty -- forces the operand-callsite to thread the
   Ty through. The signature extension is a
   stage-13 D-13-6 REOPEN, but it's mechanical: every
   caller already has the expected Ty from its
   structural context (Rvalue / Statement /
   Terminator / Assert payloads carry sufficient Ty
   info).
3. **Bridge-interning invariant for fat-pointer
   fallback child Tys.** The fat-pointer ABI shape
   (`*const pointee, usize`) is not a structural type
   in RegseqModel's IR (the IR carries `Ty::Ref(_,
   pointee, _)` not the underlying pair); the
   renderer needs valid `TyId`s for the two child
   positions to satisfy `render_const`'s pair-form
   contract. **Resolution (per R1 finding):** the
   bridge interns the required auxiliary types --
   `Ty::Ptr(pointee, Mutability::Not)` for the data
   pointer and `Ty::Uint(UintKind::Usize)` for the
   metadata -- in `model.types` at the same time it
   produces the `Const::Aggregate([Reloc, Uint])` for
   a fat-pointer consumer Ty per `spec.md` §9.7's
   `Slice { data, meta }` dispatch (line 3250) +
   `Ref(_, dyn Trait)` synthesis paths. This
   preserves `spec.md` §10's closed-`Ty`-enum + all-
   interned-TyId invariant; the renderer never
   constructs non-interned TyIds. *No defensive
   fallbacks* (README §2) -- the renderer trusts the
   bridge invariant rather than re-interning at
   render time.

**Methodology rules invoked.**

- *C semantics with information preservation*
  (README §2). The per-sub-field Ty resolution
  preserves the structural type information rustc
  gives us via the layout machinery.
- *Compositional behaviour beats special-case flags*
  (README §2). Operand-callsite wiring is uniform
  via the signature extension; no per-arm flag or
  side-channel.
- *Spec is mutable* (README §2). Stage-13 D-13-6's
  `render_operand` signature revises in place per
  the merger contract.
- *No defensive fallbacks* (README §2). The
  bridge interning invariant for fat-pointer
  fallback child Tys keeps the IR canonical; the
  renderer trusts the invariant without re-interning.

**Cross-stage friction.** D-13-6 (Body-level
renderer per-arm specs) is **REOPENED** by D-14-4.9.
Three stage-13 signatures cascade in the REOPEN:

- `render_operand(&Operand, &Body)` ->
  `render_operand(&Operand, &Body, expected_ty: TyId)`.
- `render_rvalue(&Rvalue, &Body)` ->
  `render_rvalue(&Rvalue, &Body, expected_ty: TyId)`.
- `render_generic_args(&GenericArgs)` ->
  `render_generic_args(&GenericArgs, item: ItemId)`.

The `expected_ty` extensions propagate to §7.3-§7.9
per-arm caller-side specs; the `item` extension
propagates to §6.4 ADT-rendering callsites. Flip in
`decisions.md` at merger close with annotation
"(revised in stage 14 D-14-4.9: `render_operand` +
`render_rvalue` signatures extended with
`expected_ty: TyId`; `render_generic_args` signature
extended with `item: ItemId`)".

**Cross-stage friction with §9.7 bridge dispatch.**
D-14-4.9 also adds a small bridge-side invariant
(per R1 finding): when the bridge produces
`Const::Aggregate([Reloc, Uint])` for a fat-pointer
consumer Ty per `spec.md` §9.7 dispatch line 3250
(`Slice { data, meta }`) and the `Ref(_, dyn Trait)`
synthesis paths, it MUST ensure both
`Ty::Ptr(pointee, Mutability::Not)` and
`Ty::Uint(UintKind::Usize)` are present in
`model.types`. This is a small extension to §9.7 +
the §3.4 bridge construction pipeline; no separate
D-tag REOPEN since the invariant lives within
stage-9 §9.7's existing dispatch table area and is
spelled out at first introduction.

**Prior-art evidence.** Rustc's
`try_destructure_mir_constant_for_user_output` at
`rustc_middle/src/mir/pretty.rs:1910` returns
`(ConstValue, Ty)` pairs for per-child types. The
layout-walker pattern is the rustc analog of
RegseqModel's §11 layout-driven per-field Ty
resolution.

**Spec impact** (D-14-4.9, R1-revised):

- `pretty-printer-snapshot.md` §10.1.8: replace the
  stub with the per-sub-field Ty resolution rules
  (with fat-pointer fallback using bridge-interned
  TyIds, not synthetic-non-interned) + operand-
  callsite wiring + cascade explanation.
- `pretty-printer-snapshot.md` §7.2 (`render_operand`):
  signature extended to
  `render_operand(&Operand, &Body, expected_ty: TyId)
  -> String`; per-arm table updates the `Constant(id)`
  row to render via `<render_const(id, expected_ty)>`.
- `pretty-printer-snapshot.md` §7.3 (`render_rvalue`):
  signature extended to
  `render_rvalue(&Rvalue, &Body, expected_ty: TyId)
  -> String`; per-arm table gains the per-operand
  `expected_ty` derivation column; §7.3.2 AggregateKind
  table gains the analogous column.
- `pretty-printer-snapshot.md` §6.7 (`render_generic_args`):
  signature extended to
  `render_generic_args(&GenericArgs, item: ItemId)
  -> String`; `GenericArg::Const(c)` dispatch resolves
  the param type from the enclosing Item's
  generic-parameter list.
- `pretty-printer-snapshot.md` §6.4 (ADT type
  rendering): callsites of `render_generic_args` pass
  the adjacent `item` field.
- `pretty-printer-snapshot.md` §7.4 / §7.5 / §7.6 /
  §7.7 / §7.9: per-arm callsites of `render_operand`
  (and `render_rvalue` from §7.4 Assign) gain explicit
  `expected_ty` arguments + derivation columns.
- `pretty-printer-snapshot.md` §8 ownership table:
  `render_operand`, `render_rvalue`, and
  `render_generic_args` rows update to the new
  signatures.
- `decisions.md` at merger close: D-13-6 flips to
  `[REOPENED]` with the cascade annotation.

**Worked example.**

```text
// Aggregate child-Ty resolution (struct):
render_const(<Aggregate([c1, c2])>, Ty::Struct(<Point>, [], { fields: [x: i32, y: i32] }))
  -> dispatches to D-14-4.4 named-field arm
  -> recurses: render_const(c1, fields[0].ty == <i32>)
  ->          render_const(c2, fields[1].ty == <i32>)
  -> assembles: geom::Point { x: <r(c1)>, y: <r(c2)> }

// Operand-callsite wiring (Call argument):
// Caller: render_terminator on TerminatorKind::Call(callee, args, dest, ...)
// callee.fn_sig.inputs == [<i32>, <bool>]
// Threading per D-14-4.9:
render_operand(args[0], body, <i32>)  -> dispatches to Operand::Constant(c1) arm
                                       -> render_const(c1, <i32>) -> "42_i32"
render_operand(args[1], body, <bool>) -> Operand::Constant(c2) arm
                                       -> render_const(c2, <bool>) -> "true"

// Fat-pointer fallback child Tys (bridge-interned):
render_const(<Aggregate([reloc, len])>, Ty::Ref(<[u32]>, SharedBorrow))
  -> dispatches to D-14-4.4 structural fallback (special-case fails on non-u8 element)
  -> renderer looks up bridge-interned auxiliary TyIds:
       child0_ty = TyInterner lookup for Ty::Ptr(<u32 TyId>, Mutability::Not)
       child1_ty = TyInterner lookup for Ty::Uint(UintKind::Usize)
       (both guaranteed present in model.types per the bridge-interning invariant)
  -> recurses: render_const(reloc, child0_ty), render_const(len, child1_ty)
  -> wraps: (<r(reloc)>, <r(len)>) as &[u32]
```

### D-14-5 -- `render_instance` composer renderer [LOCKED -- Phase A 8/8 sub-decisions + 19 specimens; Phase B closed at R4 APPROVE 2026-05-13]

**Question.** Stage-13 D-13-4.6 locked the
building-block-vs-composer architectural division
(stage 13 owns building blocks `render_ty` / `render_item`
/ body-level renderers; stage 14 owns composers
`render_const` / `render_instance` / top-level `render`).
D-14-4 just closed the first composer (`render_const`)
at Phase B R4 APPROVE (2026-05-13). D-14-5 advances to
the second composer: `render_instance(InstanceId) -> String`,
which dispatches across the closed 6-variant
`InstanceKind` enum per `spec-snapshot.md` §9.4
(`Function { fn_sig, body, flavor, virtual_idx }` /
`Static { ty, init }` / `Const { ty, value }` /
`Struct { layout }` / `Enum { layout }` /
`Union { layout }`) and composes the stage-13 building
blocks (`render_def_path`, `render_ty`, body-level
renderers, attribute-block rendering) plus the
stage-14 D-14-4 `render_const` for `init` / `value`
ConstId positions.

The pretty-printer-side spec lands in
`pretty-printer-snapshot.md` §10.2 (introduced as
a stub at D-14-4 lock per the §10 Composer
renderers framing; subsequently populated by this
cluster with §10.2.1-§10.2.7 per-variant +
attribute-composition subsections at the
respective sub-decision locks). The structural
`InstanceKind` enum itself stays in `spec.md` §9.4
(no spec-side revisions anticipated at this
allocation; the cross-reference paragraph added by
D-14-5.5 pointing to `pretty-printer.md` §10.2 is
the only §9.4 edit).

D-14-5 is **the second of three composer-renderer
clusters** (D-14-4 / D-14-5 / D-14-6). Closure unblocks
D-14-6 (top-level `render(&self)`) which iterates
`model.instances` and composes per-Instance via
`render_instance`.

**Evidence basis.** Step 4 rustc evidence pass landed at
`prior-art/rustc-instance.md` (2026-05-13) surveying 10
load-bearing questions (Q1-Q10) across rustc's
`rustc_middle/src/mir/pretty.rs` body envelope machinery
(`write_mir_pretty` / `write_mir_fn` / `write_mir_intro`
/ `write_mir_sig`), the per-Instance dispatch surface
(`rustc_codegen_ssa/src/mono_item.rs` `MonoItem::define`
+ `predefine`), and the stable-IR analog
(`rustc_public/src/mir/mono.rs` `Instance::emit_mir` +
`rustc_public/src/mir/pretty.rs` `function_body`). The
evidence pass grounded 8 sub-decisions worth of
allocation surface; see the per-sub-decision rationale
in the rustc-instance.md "D-tag candidates" section.

The evidence surfaces three asymmetries with rustc's
prior art that load-bear on D-14-5:

1. **6-variant `InstanceKind` vs rustc's 3-variant
   `MonoItem`** -- RegseqModel adds Struct / Enum /
   Union as emission units (they generate C typedef
   declarations); rustc/LLVM emit nominal types
   implicitly at use sites. ADT-variant rendering has
   **no rustc precedent** (Q7).
2. **`body: None` foreign declarations** -- rustc's MIR
   pretty-printer skips foreign items entirely (they're
   absent from `tcx.mir_keys`); rustc_public's
   `Instance::emit_mir` returns `Ok(())`. RegseqModel
   cannot skip -- foreign declarations are emission
   units. D-14-5 must commit a declaration form (Q5).
3. **Static / Const value rendering** -- rustc renders
   Static / Const as MIR bodies that *construct* the
   value (CTFE); RegseqModel has pre-evaluated `ConstId`s
   so the natural form is single-line
   `static [mut ]PATH: TY = <render_const>;` (Q6).

Additional cross-cutting surfaces:

- **`unsafe` / `extern "ABI"` keyword decoration on the
  signature line** -- rustc's MIR pretty-printer omits
  these entirely; RegseqModel's existing §6.5 / §5.3
  decorators are inconsistently applied (Item-level
  for `is_unsafe`, FnPtr-level for `extern "ABI"`).
  D-14-5 unifies via the composer-from-scratch boundary
  (Q3).
- **DomainAttr attribute weaving** -- the `#[rseq::intrinsic(...)]`
  stream from `domain_attrs.item_attrs[item]` has no
  rustc precedent for ordering relative to codegen-flag
  attributes; D-14-5 commits a deterministic rule (Q4).

**Sub-decision allocation.** Eight sub-decisions
allocated:

- **D-14-5.1 -- API signature.** Single-argument
  `render_instance(InstanceId) -> String` form?
  Pair-form not needed because `InstanceKind` carries
  enough per-variant context (`fn_sig`, `body`, `ty`,
  `init`, `value`, `layout`). Mirrors rustc_public
  `Instance::emit_mir`'s `(self, w)` surface. *Question
  to lock:* single-arg vs explicit pair-form vs
  multi-method dispatch trio. **Load-bearing**: gates
  all per-variant signatures.

- **D-14-5.2 -- Function variant envelope.**
  Per-arm composition for `InstanceKind::Function {
  fn_sig, body, flavor, virtual_idx }`. Sub-questions:
  attribute-block composition (per-flag §5.3 stream +
  `domain_attrs.item_attrs` stream); signature line
  decoration (`[unsafe ][extern "ABI" ]fn PATH(args) -> Ret`);
  body-vs-declaration dispatch on `body: Option<Body>`
  (definition form `{ <envelope> }` vs declaration form
  `;`); `flavor` / `virtual_idx` as render inputs (per
  Q8 / Q9 evidence: neither surfaces; both are identity
  metadata only). **Load-bearing**: the largest
  per-variant surface; gates D-14-5.7 layering choice.

- **D-14-5.3 -- Static variant.**
  `InstanceKind::Static { ty, init }` rendering.
  Form: `[<attrs>]static [mut ]PATH: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];`.
  Sub-questions: rendering shape when `init = None`
  (foreign extern static); mutability prefix source
  (Item-side per stage-12 D-12-1.2 (c) vs
  Instance-side); single-line vs multi-line break for
  long initializers.

- **D-14-5.4 -- Const variant.**
  `InstanceKind::Const { ty, value }` rendering.
  Form: `[<attrs>]const PATH: <render_ty(ty)> = <render_const(value, ty)>;`.
  Sub-questions: confirm `value` is always populated
  per stage-12 D-12-2 (no None branch needed); recurse
  through `render_const` correctly given `value: ConstId`
  and `ty: TyId` are positional in the variant.

- **D-14-5.5 -- ADT variants (Struct / Enum / Union).**
  **Load-bearing architectural decision** with no rustc
  prior-art tiebreaker (per Q7). Two main options:
  (A) header-only rendering = §5.4 wrapper unmodified
  (`[<attrs>]struct PATH<args>`, etc.); (B) full
  definition body from §11 Layout (`struct PATH<args>
  { field_name: <render_ty(field.ty)>, ... }` /
  `enum PATH { Variant(...), Variant { fields }, ... }`
  / `union PATH { field_name: <render_ty(field.ty)>, ... }`).
  Option B's evidence preponderance: matches the
  §2.1 master Rust-source-style; surfaces layout info
  consistently with §11 schema; supports diagnostics
  needing field-level visibility. Sub-questions if
  Option B locked:
  - `EnumShape` dispatch (`Empty` -> `enum PATH {}`
    no body; `Single` -> one variant; `Multiple` ->
    per-variant body).
  - Per-variant ctor form (unit / tuple-style /
    named-field per §3.4 convention; reuses §6.4
    Tuple/Struct conventions).
  - `#[repr(...)]` decoration: whether to render
    `#[repr(C)]` / `#[repr(transparent)]` / `#[repr(packed)]`
    inline (likely deferred -- not in current `Item`
    typed-field set per §8.0).
  - Enum discriminant value rendering (`Variant = 5,`
    vs implicit) -- likely deferred to v1+1.
  - Generic-args via `<render_generic_args(args, item)>`
    consumed from the parent `Ty::Struct/Enum/Union.args`.

- **D-14-5.6 -- DomainAttr attribute weaving.**
  Per Q4. Sub-questions:
  - **Ordering rule** for the attribute block. Two
    streams: (i) Item-flag-derived attributes (§5.3
    hard-coded: `#[no_mangle]`, `#[inline]`, `#[link_section]`,
    `#[naked]`, lang-item flags, `#[opaque_handle]`);
    (ii) `domain_attrs.item_attrs[item]` entries.
    Candidates: Item-flags first then domain;
    domain first then Item-flags; lexical by path.
  - **Per-`AttrData` variant render rule.** Stage-14
    occupant: `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))`
    renders as `#[rseq::intrinsic("log-<sev>")]` with
    dash-form NAME reversed from §3.6.3
    `IntrinsicKind::from_name`. Phase-2 stages append
    per-family render rules at their merger close.
  - **Spec home** within `pretty-printer-snapshot.md`
    §10.2 (dedicated subsection vs inline within
    D-14-5.2 Function envelope spec).

- **D-14-5.7 -- `render_item` vs `render_instance`
  layering boundary.** Per Q3. **Load-bearing.** Choice:
  (a) Composer-from-scratch -- `render_instance` for
  Function variant produces the full envelope from
  `render_def_path` + fn_sig resolution + §5.3 helper,
  does NOT call `render_item`. `render_item` remains
  the standalone Item-level view for diagnostics.
  (b) Compose via `render_item` wrapper -- accepts the
  §5.4 wrapper as-is and appends per-Instance content.
  Constrains Function envelope to omit `extern "ABI"`
  decoration (which `render_item` doesn't carry) or
  forces a §5.4 signature change to accept ABI context.
  Recommendation per Q3 evidence: option (a) -- cleaner
  layering, no `render_item` signature change required.

- **D-14-5.8 -- Cross-stage friction watch.** No
  anticipated REOPENs of stage-13 D-tags from current
  evidence. Working assumption: the body envelope
  (§3.1 / D-13-2), per-flag rendering table (§5.3 /
  D-13-4.3), per-`ItemKind` variant rendering (§5.4 /
  D-13-4.5) are consumed as-is. If D-14-5.7 locks
  option (a) and factors the §5.3 helper out into a
  shared callable from both `render_item` and
  `render_instance`, that's a localized refactor of
  the stage-13 spec (text reorganization, not
  semantic change). Monitor at Phase B review. If a
  REOPEN does surface during drafting, register here
  with the friction-flip annotation pattern (mirrors
  D-14-4.9's REOPEN of D-13-6).

**Spec impact preview (Phase A drafting target).**

- `pretty-printer-snapshot.md` §10.2 -- replaced with
  full per-`InstanceKind` dispatch spec covering all
  6 variants + attribute-block composition rule +
  body-vs-declaration dispatch rule + ADT-body rule
  per D-14-5.5 lock.
- `pretty-printer-snapshot.md` §11 -- cross-reference
  row for §9 Instance / InstanceKind updates from
  "stage-14's `render_instance` once shipped" to the
  concrete §10.2 pointer.
- `pretty-printer-snapshot.md` §9 specimens -- ~10-15
  new specimens covering each InstanceKind variant
  (Function definition + Function foreign-declaration
  + Function with intrinsic attribute + Static
  initialized + Static foreign + Const + Struct +
  Enum + Union); standard placement in a new §9.11
  subsection per D-13-7 convention.
- `pretty-printer-snapshot.md` §8 ownership table --
  confirm `render_instance` row reflects the
  consumes-rather-than-composes relationship with
  `render_item` per D-14-5.7 lock.
- `pretty-printer-snapshot.md` §5 -- if D-14-5.7 (a)
  locks, factor the §5.3 per-flag table into a shared
  helper subsection consumable by both `render_item`
  (§5.4) and `render_instance` (§10.2). Minor
  reorganization; the table itself is unchanged.
- `spec-snapshot.md` §9.4 -- if D-14-5.5 (B) commits
  ADT body rendering, add a cross-reference paragraph
  pointing to `pretty-printer-snapshot.md` §10.2 for
  the per-variant rendering surface (mirrors §9.7
  Const pointer added at D-14-4).

**Methodology rules invoked at allocation** (cluster-level):

- *Compositional behavior beats special-case flags.*
  `render_instance` composes existing renderers
  (`render_def_path`, `render_ty`, `render_const`,
  body envelope) rather than carrying per-variant
  flag-style markers. Variant tags + the existing
  composition chain carry per-`InstanceKind`
  distinctions.
- *C semantics with information preservation.* Static /
  Const variants surface the pre-evaluated `ConstId`
  via `render_const` rather than re-rendering a
  rustc-style value-constructing body. RegseqModel
  preserves what rustc gives us for free (the
  evaluated value via bridge CTFE).
- *Compiler-theory-first.* The 8 sub-decisions trace
  to well-established pretty-printer composition
  patterns (rustc's MIR + HIR pretty-printers
  distinguish body-bearing from type-def-bearing
  entities; stable-IR `function_body` separates
  caller-supplied name from body envelope). RegseqModel
  adopts the pattern rather than inventing a parallel
  composition.
- *No defensive fallbacks.* Per Q8 / Q9 evidence,
  bridge-synthesised Function flavors (DropGlue /
  VTableShim / etc.) render uniformly via
  `render_def_path` -- no per-flavor branch, no
  fallback for unrecognized flavor.
- *Spec is mutable.* D-14-5.5 may surface
  representation-shape constraints requiring revision
  of the §11 Layout schema (e.g., explicit
  discriminant field). Plan: revise §11 per
  spec-is-mutable rather than introduce a parallel
  layout helper at the renderer level.
- *Code does not reference discussion-folder paths.*
  D-14-5 sub-decisions are referenced inline as
  `D-14-5.X` tags in spec text where load-bearing;
  no `stage-14/discussions.md` path citations.

**Phase A drafting plan.**

1. Lock D-14-5.1 (API signature) -- the gate.
2. Lock D-14-5.2 (Function variant envelope) -- the
   load-bearing variant; subsumes the attribute-block
   composition + signature decoration + body-vs-decl
   dispatch.
3. Lock D-14-5.3 / D-14-5.4 (Static / Const) -- small
   surgical adds; reuse D-14-4 `render_const` directly.
4. Lock D-14-5.5 (ADT variants) -- architectural choice
   between Options A/B per Q7. Per-variant per-arm
   spec follows the lock.
5. Lock D-14-5.6 (DomainAttr weaving) -- ordering rule
   + per-`AttrData` render rule.
6. Lock D-14-5.7 (`render_item` vs `render_instance`
   layering) -- subsumes the §5.3 helper factor-out
   recommendation.
7. Lock D-14-5.8 (cross-stage friction outcome) --
   final pass after spec text lands; record any
   REOPENs surfaced during drafting.
8. Add ~10-15 specimens to `pretty-printer-snapshot.md`
   §9 covering each InstanceKind variant + edge cases.
9. Per-stage Phase B Codex review on
   `pretty-printer-snapshot.md` D-14-5 contributions
   (anticipated 2-4 rounds; comparable to D-14-4's
   4-round trajectory for similarly multi-sub-decision
   composer surface).

**Step 6 prior-art consultation.** Per the D-14-4
precedent, Charon / mrustc / rustc_public **deferred**:
the rustc evidence pass grounded all 8 sub-decisions
without secondary-project consultation. Inline
consultation may surface during Phase A for D-14-5.5
(ADT body rendering shape ambiguity at discriminant /
repr decoration); per-project findings land at
`stage-14/prior-art/{charon,mrustc}-instance.md` if
consulted.

**Status.** **D-14-5 cluster LOCKED 2026-05-13 at
Phase B R4 APPROVE / Close-out:YES.** Phase A
drafting opened 2026-05-13 and **all 8 sub-decisions
LOCKED 2026-05-13**:
**D-14-5.1** (API signature single-arg) +
**D-14-5.2** (Function variant envelope -- attrs +
keyword decoration + arg-list + return + body-vs-decl
trailer) +
**D-14-5.3** (Static variant) +
**D-14-5.4** (Const variant) +
**D-14-5.5** (ADT variants Struct / Enum / Union --
Option B full definition body) +
**D-14-5.6** (attribute block composition rule --
two-stream interleaving: Item-flag §5.3-table-row-
ordered + `domain_attrs.item_attrs` Vec-index-ordered;
per-`AttrData::Intrinsic` render table) +
**D-14-5.7** (layering boundary -- composer-from-
scratch; shared `render_item_attribute_block` helper
factored from §5.3 / §5.5 keeps `render_item` and
`render_instance` Stream 1 (Item-flag) attribute
streams byte-identical -- `render_item` does NOT emit
Stream 2 domain-attribute rows) +
**D-14-5.8** (cross-stage friction outcome -- **zero
new stage-13 D-tag REOPENs**; pending merger-time
flips from prior clusters unchanged: D-9-10 from
D-14-2.5, D-13-6 from D-14-4.9, D-12-1.3 / D-9-9 /
D-13-4.3 from D-14-7 when locked).

**Specimens added** (Step 8 of the 9-step plan):
**19 specimens** (Specimen 110-128) landed at
`pretty-printer-snapshot.md` §9.11 covering each
`InstanceKind` variant + edge cases per D-13-7
specimen convention. Coverage:
- Function: plain (110), unsafe-raw-pointer (111),
  full Stream-1 attribute block (112), foreign C
  declaration (113), foreign C variadic (114),
  diagnostic-logging intrinsic Stream-1+Stream-2
  (115), bridge-synthesised drop glue (116),
  divergent Ty::Never return (117).
- Static: defined immutable (118), defined mutable
  with struct init (119), foreign extern (120).
- Const: scalar (121), reference-typed &str (122).
- Struct: unit-form empty (123), tuple-form
  synthetic-`_N` (124), named-field with lang-flag +
  generic args (125).
- Enum: multi-variant mixed ctor forms (126),
  empty / uninhabited EnumShape::Empty (127).
- Union: simple named-field (128).

**Remaining cluster work** (Step 9 of the 9-step
plan):
- Per-stage Phase B Codex review on the full
  D-14-5 spec contributions; anticipated 2-4
  rounds per the D-14-4 cluster trajectory. If
  Phase B surfaces stage-13 REOPEN-shaped findings,
  D-14-5.8 entry updates per "Spec is mutable".

#### D-14-5.1 -- API signature: `render_instance(InstanceId) -> String` [LOCKED]

**Decision.** The composer renderer takes a single
argument `InstanceId`:

```rust
impl PrettyPrinter<'_> {
    pub fn render_instance(&self, id: InstanceId) -> String;
}
```

No pair-form, no borrowed `&Instance`, no per-variant
multi-method dispatch trio. The `PrettyPrinter` has
`&model` and resolves the `InstanceId` internally to
`&self.model.instances[id]`. Per-variant dispatch is
internal: the body matches on `inst.kind` (the closed
6-variant `InstanceKind` enum per `spec.md` §9.4) and
produces a self-contained `String` covering the
Instance's full rendering (attribute block + kind
keyword + path + per-variant body / value / decl
trailer).

**Reasoning.** Three load-bearing reasons drive the
single-arg form:

1. **`InstanceKind` carries enough per-variant
   context; no consumer-Ty needed.** Unlike `Const`
   (where `Bytes` / `Aggregate` / `ZST` / `NullPtr` /
   `Reloc` arms dispatch on consumer Ty per D-14-4.1),
   `InstanceKind` embeds the rich data each variant
   needs internally:
   - `Function { fn_sig, body, flavor, virtual_idx }`
     -- `fn_sig` (resolved via `FnSigId`) supplies
     args + ret + safety + abi; `body` supplies locals
     + blocks (or `None` for foreign).
   - `Static { ty, init }` -- `ty` is the type for
     `: <ty>` decoration; `init` is the `ConstId` to
     thread through `render_const(init.unwrap(), ty)`.
   - `Const { ty, value }` -- same shape as Static.
   - `Struct { layout }` / `Enum { layout }` /
     `Union { layout }` -- `layout: TyId` resolves to
     `Ty::Struct/Enum/Union { layout: Layout, args:
     GenericArgs, .. }` which carries the field /
     variant table needed for the body rendering rule
     pinned at D-14-5.5.
   No use site exists where the *consumer* of an
   `InstanceId` reference (e.g., a vtable slot, a
   Reloc child, a Call callee) needs to override the
   Instance's own rendering with a different type
   context -- in every such position the Instance's
   internal `fn_sig` / `ty` / `layout` is
   authoritative.

2. **Rustc_public precedent.** The stable-IR analog
   `Instance::emit_mir(self, w)` at
   `rustc_public/src/mir/mono.rs:178` takes the
   Instance and a writer, no extra type-context
   argument. Body-bearing-only -- the stable-IR API
   is "give me a writer; I'll produce the rendering
   from my self-contained body or skip if None." The
   pair-form would only be needed if rendering
   depended on data the Instance doesn't carry --
   which, by §9.4's per-variant embedding rule, it
   doesn't.

3. **ID-form consistency with the renderer family.**
   `render_ty(TyId)` (stage-13 D-13-5),
   `render_def_path(ItemId)` (stage-12 D-12-6),
   `render_item(ItemId)` (stage-13 D-13-4.5),
   `render_const(ConstId, TyId)` (stage-14 D-14-4.1)
   all take ID forms. `render_instance(InstanceId)`
   follows the same convention. The `PrettyPrinter`
   has `&model` and resolves IDs internally; no caller
   need pre-resolve.

**Rejected alternatives.**

- **(b) Pair-form `render_instance(InstanceId, TyId)`** --
  no consumer-context Ty exists at use sites; the
  argument would always be redundant with the
  Instance's internal `fn_sig` / `ty` / `layout`. Adds
  callsite verbosity without information gain.
  Diverges from rustc_public's precedent.
- **(c) Multi-method dispatch trio
  `render_function(id) / render_static(id) /
  render_const_instance(id) / render_struct_def(id) /
  ...`** -- one method per InstanceKind variant.
  Exposes the variant dispatch at the API surface; the
  caller would need to inspect `inst.kind` before
  choosing the method. The top-level `render(&self)`
  (D-14-6) iterates `model.instances` and would have
  to dispatch externally rather than letting
  `render_instance` handle it. Inverts the
  encapsulation: the renderer's whole point is to
  hide per-variant dispatch behind one composer
  surface.
- **(d) Borrowed `render_instance(&Instance) ->
  String`** -- forces callers to first resolve
  `InstanceId -> &Instance` via
  `model.instances[id]`. Inconsistent with the rest
  of the renderer family (all take IDs) and gives
  the renderer no benefit (it already has `&model`).

**Methodology rules invoked.**

- *Compositional behavior beats special-case flags.*
  The single-arg form composes internally via the
  variant dispatch; per-variant rendering rules are
  pinned at D-14-5.2 through D-14-5.6. The unified
  API surface is the natural composition over the
  closed `InstanceKind` enum.
- *C semantics with information preservation.*
  RegseqModel's bridge has resolved `InstanceKind`
  to a fully-substituted post-mono shape per Q2;
  the renderer trusts that resolution and reads
  `fn_sig` / `ty` / `layout` directly. No re-derivation
  from a sibling type context.
- *No defensive fallbacks.* No "what if the consumer
  has a different type than the Instance carries"
  fallback; the closed `InstanceKind` covers all
  emission units and the bridge's invariants
  guarantee per-variant rich data is populated.
- *Compiler-theory-first.* The single-arg form
  matches rustc_public's stable-IR API precedent and
  the broader pretty-printer pattern (composition
  over a closed variant enum, dispatch internal).

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2 -- intro
  paragraph pins the API signature
  `render_instance(InstanceId) -> String` and the
  dispatch shape (closed 6-variant `InstanceKind`
  match producing a self-contained `String`).
  Per-variant subsections (§10.2.1 - §10.2.6) land
  at subsequent sub-decision locks; the intro
  notes that the per-variant rules are pending the
  respective locks.
- `pretty-printer-snapshot.md` §1.3 composer
  listing -- already shows
  `render_instance(InstanceId) -> String`; the
  D-14-5.1 lock confirms (no edit).
- `pretty-printer-snapshot.md` §8 per-method
  ownership table -- the `render_instance(InstanceId)`
  row already exists with stage-14 owner per D-13-4.6;
  Inputs column reads `InstanceId`, return `String`,
  Description column updates from "composes
  `render_item` + per-Instance content" to "composes
  `render_def_path` + per-`InstanceKind` content
  (signature decoration + body envelope per §10.2)"
  after D-14-5.7 layering lock (which deciding
  whether `render_item` is in the call graph).
  Held for now; final wording lands at D-14-5.7.
- `pretty-printer-snapshot.md` §3.3 sigil convention
  table row for `InstanceId` -- reads "rendered via
  `render_instance(InstanceId)` (stage 14)"; no edit
  needed (single-arg form matches the row's existing
  shape).
- `spec.md` §9 cross-reference at
  `pretty-printer-snapshot.md` §11 -- reads
  "Rendering: see `pretty-printer.md` §1.3 (composer
  ownership) + stage-14's `render_instance` once
  shipped (§10.2)"; D-14-5.1 lock makes §10.2 active
  but doesn't change the cross-reference wording
  (the "once shipped" qualifier becomes redundant
  but not yet incorrect -- §10.2 still has subsequent
  sub-decisions pending). Final wording cleanup at
  cluster close.

**Worked example.**

```rust
impl PrettyPrinter<'_> {
    pub fn render_instance(&self, id: InstanceId) -> String;
}

// Illustrative dispatch sketch -- per-variant content pinned
// at D-14-5.2 through D-14-5.6.
fn render_instance(&self, id: InstanceId) -> String {
    let inst = &self.model.instances[id];
    match &inst.kind {
        InstanceKind::Function { fn_sig, body, flavor: _, virtual_idx: _ } => {
            // D-14-5.2: attribute block + signature + body-or-decl.
            // Body envelope per §3.1 when body.is_some();
            // declaration form (trailing `;`) when body.is_none().
            todo!("D-14-5.2 spec")
        }
        InstanceKind::Static { ty, init } => {
            // D-14-5.3: [<attrs>]static [mut ]PATH: <render_ty(ty)>
            //                          [ = <render_const(init.unwrap(), ty)>];
            todo!("D-14-5.3 spec")
        }
        InstanceKind::Const { ty, value } => {
            // D-14-5.4: [<attrs>]const PATH: <render_ty(ty)>
            //                          = <render_const(value, ty)>;
            todo!("D-14-5.4 spec")
        }
        InstanceKind::Struct { layout } => {
            // D-14-5.5 Option B: [<attrs>]struct PATH<args>
            //                          { field: <render_ty(field.ty)>, ... }
            todo!("D-14-5.5 spec")
        }
        InstanceKind::Enum { layout } => {
            // D-14-5.5 Option B: [<attrs>]enum PATH<args>
            //                          { variant_arms_per_EnumShape }
            todo!("D-14-5.5 spec")
        }
        InstanceKind::Union { layout } => {
            // D-14-5.5 Option B: [<attrs>]union PATH<args>
            //                          { field: <render_ty(field.ty)>, ... }
            todo!("D-14-5.5 spec")
        }
    }
}
```

The `flavor` / `virtual_idx` fields on
`InstanceKind::Function` are identity-metadata only
per the prior-art Q8 / Q9 evidence -- they
disambiguate bridge-synthesised flavors at the
identity level (per §9.3 `fn_shim_disambig`) but
don't shape the rendered output. The renderer reads
the Instance's `def_path` (resolved via
`render_def_path(inst.item)`) which already encodes
the synthesised family in the path string per
stage-12 D-12-6 + §13.4.

The `_` patterns above are placeholders for the
illustrative sketch; the locked D-14-5.2 rendering
rule pins how the Function variant actually
destructures and composes its rendering.

#### D-14-5.2 -- Function variant envelope [LOCKED]

**Decision.** `InstanceKind::Function { fn_sig, body,
flavor, virtual_idx }` renders to a self-contained
block with three composable pieces: attribute block,
function-header line, and body-or-declaration trailer.

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <render_def_path(item)>(<arg-list>[, ...]) -> <render_ty(output)>[ { <body-envelope> }];
```

The header line is followed by **exactly one** of:
- ` { <body-envelope> }` when `body == Some(b)`
  (definition form).
- `;` when `body == None` (foreign-declaration form).

The square-bracketed components are conditional; the
arg-list grammar is `(_1: <render_ty(input_0)>, _2:
<render_ty(input_1)>, ..., _N: <render_ty(input_N-1)>)`
with `_1` through `_N` matching the §3.1 / §25.1
arg-locals convention (locals indexed `1..=arg_count`,
where `arg_count == fn_sig.inputs.len()`). When
`fn_sig.inputs.is_empty()`, render `()`; otherwise
comma-space separator between args. When
`fn_sig.c_variadic == true`, the arg list ends with
`, ...` (trailing comma + space + `...`) per Rust
source convention; per `spec.md` §14.3 construction
invariant `c_variadic == true` implies
`inputs.len() >= 1` so the leading comma is
guaranteed.

The return type `-> <render_ty(output)>` is **always
rendered**, including the unit case `-> ()`. Rust
source permits omitting `-> ()` but the body envelope
(§3.1) always declares `let mut _0: <render_ty(ret_ty)>;`,
and the header-vs-locals consistency outweighs the
source-omission idiom.

Per-aspect spec:

**Aspect 1 -- Attribute block.** The composition rule
(ordering of the per-flag §5.3 stream vs
`domain_attrs.item_attrs[item]` stream + per-`AttrData`
variant render rule) is pinned at §10.2.7 per D-14-5.6
LOCKED. For D-14-5.2 purposes, the attribute block is
a single pre-computed prefix string emitted before
the function-header line, multi-line with one
`#[...]` per line, ending with a trailing `\n`.
Default-suppression (§5.2) applies unchanged: zero
surviving attributes yields an empty prefix string
and the function-header line starts at column 0.

**Aspect 2 -- Function-header line decoration.**
Keyword order matches Rust source grammar
(`[unsafe ][extern "ABI" ]fn`), reading sources:
- `unsafe`: rendered when `Item.is_unsafe == true`.
  Source-of-truth is `Item.is_unsafe` (matches §5.3
  convention), not `fn_sig.safety`; the two agree by
  construction per the bridge's per-Item population
  rule, and reading from `Item` keeps `render_item`
  and `render_instance` consistent.
- `extern "ABI"`: rendered when `fn_sig.abi !=
  FnAbi::Rust` (default `Rust` suppressed). Per
  `spec.md` §3.4 / §14.3, `FnAbi` is the closed
  2-variant enum `{ Rust, C }` so the only non-default
  form in v1 is `extern "C"`. (Future ABI extension
  appends to `FnAbi` per `spec.md` §14.1.) Source:
  `fn_sig.abi` resolved via `model.sigs[fn_sig_id]`.
- `fn`: invariant keyword.
- `<render_def_path(inst.item)>`: path via stage-12
  D-12-6; encodes bridge-synthesised flavors (vtable
  shims, drop glue, closure call methods) in the
  path string per §13.4.

**Aspect 3 -- Body-vs-declaration trailer.** Dispatch
on `body: Option<Body>`:
- `body == Some(b)` -- definition form: ` {\n`
  followed by the body envelope per §3.1 (locals
  declaration block + optional `debug NAME => _N;`
  lines per §3.3 / §4.4 / D-13-3 gating + per-block
  content via `render_basic_block` per §7.10) followed
  by `\n}`. The body envelope is unchanged from
  stage-13 D-13-2; D-14-5.2 composes the wrapper, not
  the envelope content.
- `body == None` -- declaration form: `;` immediately
  after the return-type segment `-> <ret>` at the
  end of the header line (no space, no opening brace).
  Per D-14-5.2 aspect 4 always-render `-> Ret`, the
  return-type segment is unconditionally present, so
  the `;` lands after `<ret>`, not after the arg-list
  closing `)`. Foreign declarations
  (`Item.linkage == Linkage::Foreign` per the
  bridge's §3.1 invariant 10 / D-12-2.1) surface
  this form; the `#[foreign]` attribute (§5.3) and
  trailing `;` reinforce each other in the rendered
  output.

**Composition outline:**

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <path>(<args>[, ...]) -> <ret> {
    let mut _0: <ret>;         // RETURN_PLACE; arg-locals _1..=_N are
                                // declared in the header arg-list above,
                                // not redeclared here (per §3.1 / D-13-2)
    let mut _M: T;             // first non-arg local (M == arg_count + 1)
    ...
    [debug NAME => _N;]        // when opts.include_debug_names
    ...

    bb0: { ... }
    ...
}
```

or, for `body == None`:

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <path>(<args>[, ...]) -> <ret>;
```

`<attribute-block>` content composition is D-14-5.6
territory; for D-14-5.2 the worked examples below
illustrate the locked ordering of `#[no_mangle]`,
`#[inline(...)]`, `#[foreign]`, and
`#[rseq::intrinsic("NAME")]` per D-14-5.6 LOCKED
(Stream 1 / §5.3 per-flag stream emitted first via
the §5.6 helper; Stream 2 / `domain_attrs.item_attrs`
emitted after).

`flavor` / `virtual_idx` are **not rendering inputs**
per D-14-5.1 + Q8 / Q9 evidence. They disambiguate
shim identity at the model-construction layer; the
DefPath rendered via `render_def_path` already
encodes the synthesised family per §13.4.

**Reasoning.**

1. **Rust-source-style master decision (D-13-1).**
   The envelope mirrors Rust source verbatim:
   `unsafe extern "C" fn foo(args, ...) -> Ret { ... }`
   (or `;` for foreign declarations). RegseqModel's
   pretty-printer doesn't invent a parallel form;
   the master decision (§2.1) is to follow source
   convention with sigil decorations only where the
   IR carries information source lacks (e.g.,
   `_<idx>` sigils for locals, `bbN` block labels).
   Function envelope rendering inherits the master
   decision without exception.

2. **Keyword ordering pinned by Rust grammar.** Per
   the Rust Reference, function declaration prefixes
   are `<visibility> [const ] [async ] [unsafe ]
   [extern "ABI" ] fn`. RegseqModel's MIR-shaped IR
   has no `const fn` / `async fn` (post-MIR these
   are flattened to ordinary functions),
   visibility is implicit (every emitted Instance
   is module-public per the codegen-keyed
   monomorphisation rule, Q2), so the order
   reduces to `[unsafe ][extern "ABI" ]fn`. No
   degree of freedom on ordering.

3. **`extern "ABI"` source from `fn_sig.abi`, not
   `Item`.** Per `spec.md` §8.0 the Item schema has
   no `abi` field -- ABI lives Instance-side on the
   substituted `fn_sig` (§14.3). Reading
   `fn_sig.abi` is the only path. The §6.5
   fn-pointer rendering uses the same source for
   the same reason; D-14-5.2 inherits the
   convention.

4. **`unsafe` source from `Item.is_unsafe`, not
   `fn_sig.safety`.** Both sources agree by
   construction (the bridge populates `Item.is_unsafe`
   from the same `tcx.fn_sig(did).safety` that
   produces `fn_sig.safety`). Reading from `Item`
   matches the §5.3 convention and keeps
   `render_item` and `render_instance` consistent
   when both render the same Function Instance.
   Reading from `fn_sig` would split the convention
   across two surfaces.

5. **`c_variadic` handled at the arg-list level.**
   Per `spec.md` §14.3 construction invariant,
   `c_variadic == true` implies `abi == FnAbi::C` and
   `inputs.len() >= 1`. Rendering `, ...` after the
   last input is the Rust source form. No
   separate `is_variadic` attribute; the variadic
   marker lives on `FnSig` (not Item) and surfaces
   only in the arg-list.

6. **Always-render `-> Ret`.** Two reasons:
   (a) Header-line consistency with the body
   envelope's `let mut _0: <render_ty(ret)>;`
   declaration (§3.1) -- the renderer would otherwise
   show the return type in two different forms
   (omitted in header, declared in locals) for unit
   return.
   (b) Snapshot stability: diffing two builds where
   one has `() -> ()` and the other a non-unit
   return produces a single-line diff if the unit
   case explicitly renders `-> ()`; otherwise the
   diff includes the addition of a missing token.

7. **`body == None` declaration form per Q5 evidence.**
   Rustc's MIR pretty-printer skips foreign items
   entirely (they're absent from `tcx.mir_keys`);
   rustc_public's `Instance::emit_mir` returns
   `Ok(())` for body-less Instances. RegseqModel
   cannot skip -- foreign declarations are emission
   units (the C backend emits `extern int foo(int);`
   for them). The trailing `;` form is the natural
   Rust-source declaration shape, parallel to
   `extern "C" { fn foo(); }` block content with the
   `extern "C" {}` brace structure flattened (the
   bridge has already monomorphised away the
   declaration block; the foreign Instance lives in
   `model.instances` directly).

8. **Body envelope per §3.1 / D-13-2 untouched.**
   D-14-5.2 composes the wrapper around the §3.1
   envelope; it does not redefine the locals block,
   debug lines, basic-block content, or any
   body-level renderer per-arm spec. No D-13-2
   REOPEN required.

9. **No `flavor` / `virtual_idx` rendering.** Per
   D-14-5.1 reasoning (3) + Q8 / Q9 prior-art
   evidence. Identity metadata only; bridge-
   synthesised flavor encoded in DefPath.

**Rejected alternatives.**

- **(b) `extern "ABI"` decoration via `render_item`
  signature extension.** Would require §5.4 to
  accept ABI context (forcing all `render_item`
  callers to pass an Instance reference), breaking
  the §5.4 standalone-Item-view contract. Rejected
  in favor of composer-from-scratch composition (the
  formal layering choice locks at D-14-5.7;
  D-14-5.2 pre-commits to it because the Function
  envelope cannot be expressed within §5.4's
  Item-only signature).

- **(c) Body envelope inside attribute block.**
  E.g., render `body: None` as `#[no_body] fn ...
  { ... }` with an empty body. Diverges from Rust
  source convention; loses the trailing-`;`
  declaration shape. Rejected.

- **(d) Omit `-> ()` for unit-return functions.**
  Matches Rust source idiom but breaks
  header-vs-locals consistency (§3.1 declares
  `_0: ()` unconditionally) and adds tokenisation
  noise to snapshot diffs across return-type
  changes. Rejected.

- **(e) Render `flavor` / `virtual_idx` as
  trailing comment** (e.g., ` // flavor=DropGlue`).
  Diagnostic-only information; useful for some
  debugging but adds visual noise to the default
  snapshot output. Per `Code does not reference
  discussion-folder paths` + the master
  Rust-source-style decision, trailing-comment
  metadata is reserved for span / debuginfo modes
  (§4.2 / §4.4). A future revision may add a
  `PrettyPrintOptions::include_instance_metadata`
  flag if diagnostic friction surfaces; not
  pre-allocated per §5.9 rule 4.

**Methodology rules invoked.**

- *Compiler-theory-first.* Rust source grammar
  pins the keyword order; the body-vs-declaration
  trailer matches the standard pretty-printer
  pattern for body-bearing vs declaration-only
  items.
- *C semantics with information preservation.* The
  `extern "ABI"` decoration surfaces the FnAbi
  the bridge has already resolved; the `c_variadic`
  marker surfaces the structural FnSig field. No
  re-derivation, no auxiliary side-tables.
- *Compositional behavior beats special-case flags.*
  Body-vs-declaration dispatch is structural on
  `body: Option<Body>`; no parallel
  `is_declaration: bool` flag. `c_variadic`
  rendering composes the trailing `, ...` from the
  existing FnSig field rather than adding a
  variadic-specific render path.
- *No defensive fallbacks.* `fn_sig.inputs.len() ==
  arg_count` per stage-9 / stage-11 invariants
  (§25.1); bridge-bug otherwise. Renderer trusts
  the invariant. No "what if the arg counts
  mismatch" recovery.
- *Code does not reference discussion-folder paths.*
  Spec text refers to other D-tags (e.g., "per
  D-14-5.6" for the attribute-block ordering) and
  named architectural rules (§3.1 body envelope,
  §5.3 per-flag table) rather than line numbers in
  this discussions file.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2.1 -- new
  subsection "Function variant envelope" with
  the per-aspect spec (attribute block placement +
  signature decoration table + body-vs-decl
  trailer + composition outline). Includes the
  `c_variadic` arg-list rule + always-render
  `-> Ret` rule + foreign-declaration form per
  Q5 evidence.
- `pretty-printer-snapshot.md` §3.1 -- no edit.
  D-14-5.2 composes the existing body envelope;
  it does not redefine it. (The §3.1 envelope
  description already mentions "the function
  header line is composed by `render_instance`
  at stage 14"; D-14-5.2 lock makes that
  composition concrete.)
- `pretty-printer-snapshot.md` §5.3 -- no edit at
  D-14-5.2 lock. `Item.is_unsafe` rendering for
  `render_item`'s standalone-Item view is
  unchanged. The `extern "ABI"` decoration is
  per-Instance (Instance has fn_sig context);
  not added to §5.3.
- `pretty-printer-snapshot.md` §5.4 -- no edit at
  D-14-5.2 lock. The per-`ItemKind` wrapper for
  Function (`[#[attrs]\n][unsafe ]fn $path`)
  remains the standalone-Item view. Per D-14-5.7
  LOCKED, `render_instance` composes from scratch
  via the §5.6 shared `render_item_attribute_block`
  helper (factored from §5.3 + §5.5) and does NOT
  call `render_item`; the §5.6 helper is the
  shared Stream 1 composition source consumed by
  both renderers, ensuring byte-identical Stream 1
  attribute streams.
- `pretty-printer-snapshot.md` §6.5 -- no edit.
  Fn-pointer rendering uses the same
  `[unsafe ][extern "ABI" ]fn(<args>) -> <ret>`
  decoration; D-14-5.2 pre-commits to the same
  convention for `InstanceKind::Function`
  envelope rendering. Consistent surface form.
- `pretty-printer-snapshot.md` §8 per-method
  ownership table -- `render_instance(InstanceId)`
  row Description column updated post-D-14-5.7
  lock (deferred until the layering boundary
  pins).

**Worked examples.**

```text
// Plain function, no attributes, unit return:
fn pl011::write_byte(_1: &pl011::PL011, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Unsafe function, raw-pointer arg:
unsafe fn pl011::ptr_write(_1: *mut u8, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Function with codegen attribute block:
#[no_mangle]
#[inline(always)]
unsafe fn pl011::init(_1: *mut pl011::PL011) -> i32 {
    let mut _0: i32;
    bb0: {
        ...
    }
}

// Foreign declaration (body: None) with C ABI:
#[foreign]
extern "C" fn libc::printf(_1: *const u8) -> i32;

// Foreign declaration with C variadic (extern "C" fn foo(p: *const u8, ...) -> i32):
#[foreign]
extern "C" fn libc::printf_variadic(_1: *const u8, ...) -> i32;

// Diagnostic-logging intrinsic (post-bridge-recognition;
// attribute-block ordering pinned at §10.2.7 per
// D-14-5.6 LOCKED -- Stream 1 (Item-flag, here
// `#[inline(never)]` from Item.inline_hint = Never)
// emitted first via the §5.6 helper; Stream 2
// (`#[rseq::intrinsic("log-info")]` from
// AttrData::Intrinsic) emitted after):
#[inline(never)]
#[rseq::intrinsic("log-info")]
fn regseq::log::intrinsics::log_info(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: {
        // unimplemented!() body
        ...
    }
}

// Bridge-synthesised drop glue (DefPath encodes the family):
fn core::ptr::drop_in_place::<pl011::PL011>(_1: *mut pl011::PL011) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Bridge-synthesised vtable method shim (DefPath via §13.4):
fn <vtable_method_shim for dyn pl011::Writer for pl011::PL011::write>(_1: *mut (), _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}
```

Edge cases:

- **Zero-arg function**: `fn foo() -> Ret { ... }` --
  empty arg-list renders as `()`; no trailing comma.
- **Single-arg function**: `fn foo(_1: T) -> Ret { ... }` --
  no trailing comma (matches Rust source).
- **C variadic**: `fn foo(_1: T, ...) -> Ret;` --
  trailing `, ...` after the last fixed input.
  Per `spec.md` §14.3 invariant, `inputs.len() >= 1`
  always holds when `c_variadic == true`, so the
  leading comma is guaranteed.
- **Divergent return**: `fn panic_handler(_1: &PanicInfo) -> ! { ... }` --
  `Ty::Never` renders as `!` per §6.1 row; the
  `-> !` decoration is the standard Rust form.
- **Generic function (post-mono)**: the renderer
  sees the substituted post-mono Function Instance;
  generic-args are encoded in the DefPath via
  `render_def_path` (e.g.,
  `core::ptr::drop_in_place::<pl011::PL011>`). No
  `<T1, T2>` decoration on the header line itself
  (those live in the DefPath segment).

#### D-14-5.3 -- Static variant rendering [LOCKED]

**Decision.** `InstanceKind::Static { ty, init }`
renders as a single-line composer (with optional
multi-line attribute block above):

```text
[<attribute-block>\n]static [mut ]<render_def_path(item)>: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];
```

Per-aspect spec:

**Aspect 1 -- Attribute block.** Same composition
contract as D-14-5.2: multi-line `#[...]` prefix
ending with a trailing `\n` when non-empty; empty
prefix when zero attributes survive default-
suppression. Composition rule (ordering of the §5.3
per-flag stream vs `spec.md` §3.6.1
`domain_attrs.item_attrs[item]` stream) pinned at
§10.2.7 per D-14-5.6 LOCKED. Note: stage-14 ships
no `AttrData::Intrinsic` variant occupying Static
Items (intrinsic recognition targets Function Items
per §3.6.3 / D-14-1); the attribute block for
Static Instances reduces to Stream 1 (the §5.3
per-flag stream via the §5.6 shared helper) in v1.

**Aspect 2 -- Mutability prefix.** Read from
`model.items[inst.item].kind` per stage-12 D-12-1.2 (c):
- `ItemKind::Static { mutability: Mutability::Not }` -> `static `.
- `ItemKind::Static { mutability: Mutability::Mut }` -> `static mut `.

Sourcing from `ItemKind` (not from a parallel
Instance-side field) matches `spec.md` §8.0 + §9.4
("mutability lives on `ItemKind::Static`; statics
can't be generic so 1:1 Item:Instance always"). The
`render_item` standalone-Item view (§5.4) reads the
same source for consistency.

**Aspect 3 -- Path.** `<render_def_path(inst.item)>`
via stage-12 D-12-6. Bridge-synthesised statics
(vtable instances per §13.2, `ConstMemoryStatic`
per §13.3) carry their family-encoded paths
unchanged.

**Aspect 4 -- Type decoration.** `: <render_ty(ty)>`
always rendered. Source: `InstanceKind::Static.ty`,
the post-mono substituted static type per §9.4.

**Aspect 5 -- Initializer-vs-declaration trailer.**
Dispatch on `init: Option<ConstId>`:
- `init == Some(c)` -- definition form:
  ` = <render_const(c, ty)>;`. Composes the D-14-4
  pair-form: the Static's `ty` is the consumer-
  context type for the const value, satisfying
  D-14-4.1's API contract.
- `init == None` -- declaration form: `;`
  immediately after the type decoration (no `=`,
  no space).

The `init == None` case surfaces foreign extern
statics (`extern "C" { static FOO: u32; }` in Rust
source). Per `spec.md` §3.4 step 8 / D-12-2.2 the
biconditional `init.is_none() iff Item.linkage ==
Linkage::Foreign` holds by construction. The
`#[foreign]` attribute (§5.3) and trailing `;`
reinforce each other in the rendered output,
mirroring the Function `body == None` shape locked
at D-14-5.2.

**Reasoning.**

1. **Rust-source-style master decision (D-13-1).**
   The envelope mirrors Rust source: `static FOO: T
   = expr;` or `static mut FOO: T = expr;` for
   defined statics; `static FOO: T;` for foreign
   extern statics. RegseqModel inherits the source
   convention; the surface form is parallel to
   D-14-5.2's Function envelope (declaration form
   when the value/body is absent).

2. **Value composition via `render_const` pair-form.**
   D-14-4 locked the pair-form
   `render_const(ConstId, TyId)` per D-14-4.1; the
   `TyId` is the consumer-context type. For Static
   Instances, the consumer-context Ty is exactly
   `InstanceKind::Static.ty`. The composer reads
   `init.unwrap()` and threads `ty` directly. No
   layout walking required at the D-14-5.3 layer.

3. **Mutability source from `ItemKind`.** Per
   stage-12 D-12-1.2 (c) (mutability lives on
   parent `ItemKind::Static`, not on
   `InstanceKind::Static`), the Instance-side
   schema is `Static { ty: TyId, init:
   Option<ConstId> }` (2 fields). Reading mutability
   from `model.items[inst.item].kind` matches §5.4's
   `render_item` source.

4. **Always-render `: <ty>`.** Statics in Rust
   source always carry a type annotation (it's
   syntactically required, unlike fn return types
   which can be inferred to unit). The rendering
   inherits the source requirement; no
   suppression-when-unit branch.

5. **No `init` body envelope.** RegseqModel has
   pre-evaluated the `init` to a `ConstId` per the
   bridge's CTFE step (per Q2 + the Const evaluation
   phase in `spec.md` §9.7 "Const validation phase
   split"). Rendering the value as a single-line
   `<render_const(...)>` (rather than a body
   envelope as rustc does for static MIR bodies)
   is the natural form per the Q6 prior-art
   asymmetry: RegseqModel surfaces the evaluated
   value; rustc renders the value-constructing
   body. The composer's job is to expose the
   evaluated form.

6. **Foreign-declaration trailer matches D-14-5.2's
   Q5 rule.** The `init == None` declaration form
   is the same architectural decision as
   D-14-5.2's `body == None` Function-declaration
   form: surface emission units that rustc's MIR
   pretty-printer skips, using the natural Rust-
   source declaration shape.

**Rejected alternatives.**

- **Body-envelope rendering of `init`** (mirror
  rustc's static-body convention). Surfaces the
  evaluation graph rather than the evaluated
  value. Loses the Q6 prior-art preponderance:
  RegseqModel has already evaluated; re-rendering
  the body wastes the bridge's work. Rejected.

- **Render `init` value inline without `=` token**
  (e.g., `static FOO: T expr;`). Diverges from
  Rust source. Rejected.

- **Omit type annotation when const-self-typed**
  (some constants self-determine type). Diverges
  from Rust source requirement that statics
  carry explicit type. Rejected.

- **Source mutability from a hypothetical
  Instance-side `mutability` field.** Would
  require schema change at §9.4
  (`InstanceKind::Static`) -- but stage-12 D-12-1.2
  (c) already locked mutability as Item-side. The
  D-14-5.3 spec must read the existing source;
  no schema change.

**Methodology rules invoked.**

- *C semantics with information preservation.* The
  evaluated `ConstId` is surfaced directly through
  `render_const`; the renderer trusts the bridge's
  CTFE result rather than re-deriving from a body.
- *Compositional behavior beats special-case flags.*
  Mutability dispatch on
  `ItemKind::Static { mutability }`; no parallel
  `is_mut` flag. Init dispatch on
  `init: Option<ConstId>`; no parallel
  `is_declaration: bool` flag.
- *Compiler-theory-first.* The
  `static [mut ] PATH: T [= expr];` form is
  canonical Rust source; the foreign-declaration
  form is the parallel `;`-terminated shape from
  `extern "C" { ... }` blocks (the bridge has
  flattened the block structure away).
- *No defensive fallbacks.* `init == None`
  biconditional with `Item.linkage == Foreign`
  per `spec.md` §3.4 step 8; bridge-bug otherwise.
  Renderer trusts the invariant.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2.2 -- new
  subsection "Static variant rendering" with
  per-aspect spec + composition outline + worked
  examples (defined static, defined mutable static,
  foreign extern static).
- `pretty-printer-snapshot.md` §5.4 -- no edit.
  The §5.4 per-`ItemKind` row for
  `Static { mutability: Not }` /
  `Static { mutability: Mut }` already renders
  the kind-keyword form; D-14-5.3 composes a
  superset (kind-keyword + path + type +
  initializer trailer) but does not change the
  wrapper view.

**Worked example.**

```text
// Defined static, immutable:
static pl011::BASE_ADDR: u32 = 268472320_u32;

// Defined static, mutable:
static mut pl011::DRIVER_STATE: pl011::State = pl011::State {
    initialised: false, count: 0_u32,
};
// (Multi-field initializer renders on one line via
// render_const Aggregate dispatch per D-14-4.4; the
// linebreak in this example is illustrative for
// readability only -- the actual renderer emits
// it on one line.)

// Foreign extern static:
#[foreign]
static libc::stderr: *mut libc::FILE;

// Bridge-synthesised ConstMemoryStatic (per §13.3;
// DefPath encodes the synthetic family):
static <const_mem_static for "hello\0">: [u8; 7] = *b"hello\0\0";

// Bridge-synthesised vtable instance (per §13.2;
// DefPath encodes the synthetic family):
static <vtable for pl011::Writer for pl011::PL011>: pl011::__VTable<Writer, PL011> =
    pl011::__VTable { drop: ..., size: 24_usize, align: 8_usize, write: ... };
```

#### D-14-5.4 -- Const variant rendering [LOCKED]

**Decision.** `InstanceKind::Const { ty, value }`
renders as a single-line composer (with optional
multi-line attribute block above):

```text
[<attribute-block>\n]const <render_def_path(item)>: <render_ty(ty)> = <render_const(value, ty)>;
```

No mutability prefix (consts are immutable by
definition; Rust source forbids `const mut`). No
foreign-declaration form (consts cannot be foreign
in Rust; `extern "C" { const FOO: u32; }` is not
valid Rust source). `value: ConstId` is always
populated per stage-12 D-12-2 -- no `Option` wrap,
no None branch.

Per-aspect spec:

**Aspect 1 -- Attribute block.** Same composition
contract as D-14-5.2 / D-14-5.3: multi-line
`#[...]` prefix; composition rule pinned at
§10.2.7 per D-14-5.6 LOCKED. Stage-14 ships no
`AttrData::Intrinsic` variant occupying Const Items
(intrinsic recognition targets Function Items only);
the attribute block for Const Instances reduces to
Stream 1 (the §5.3 per-flag stream via the §5.6
shared helper) in v1.

**Aspect 2 -- Path + type + value.**
- Path: `<render_def_path(inst.item)>` via
  stage-12 D-12-6.
- Type: `: <render_ty(ty)>`, always rendered.
  Source: `InstanceKind::Const.ty`.
- Value: ` = <render_const(value, ty)>` always
  populated. Source: `InstanceKind::Const.value`
  threaded with `ty` as consumer context per
  D-14-4.1 pair-form.

**Aspect 3 -- Trailer.** `;` invariant. No
declaration-vs-definition dispatch (consts always
have a value).

**Reasoning.**

1. **Rust-source-style master decision (D-13-1).**
   The envelope mirrors Rust source `const NAME: T
   = expr;`. RegseqModel inherits the convention.

2. **`value` always populated per stage-12 D-12-2.**
   Const Items must carry an evaluated value at
   the Instance level (the bridge's Phase-3 CTFE
   step evaluates const-context expressions
   eagerly; unevaluated consts would surface as
   bridge bugs per "No defensive fallbacks"). The
   absence of a `None` branch simplifies the
   composer relative to D-14-5.3 (Static).

3. **Pair-form composition via `render_const`.**
   Same as D-14-5.3: the consumer-context `ty` is
   `InstanceKind::Const.ty`, threaded directly.

4. **No `unsafe const` form.** Rust source forbids
   `unsafe const`; the §5.3 keyword-prefix rule
   ("Only `fn` accepts the keyword; `unsafe static`
   is non-syntactic") generalises to all non-fn
   ItemKinds. `Item.is_unsafe == true` on a Const
   Item would be a bridge bug; the renderer trusts
   the invariant.

5. **No foreign declaration form.** Rust source
   has no foreign const declaration shape
   (`extern "C" { const FOO; }` is invalid); Const
   Items always carry a value. The composer has
   no None branch.

**Rejected alternatives.**

- **Body-envelope rendering of `value`.** Same
  Q6 prior-art preponderance as D-14-5.3: rustc's
  const-body is the evaluation graph; RegseqModel
  surfaces the evaluated form. Rejected.

- **Omit `: <ty>` when const value is
  scalar-self-typed** (e.g., elide `: i32` for an
  `Int(I32, _)` value because the type is
  apparent from the rendered const literal).
  Diverges from Rust source requirement that
  consts carry explicit type. Rejected.

**Methodology rules invoked.**

- *C semantics with information preservation.* The
  evaluated `ConstId` is surfaced directly; the
  bridge's CTFE result is the rendered form.
- *Compositional behavior beats special-case flags.*
  No `is_const_declaration` / `is_evaluated` flag;
  the `value: ConstId` is always populated
  structurally per stage-12 D-12-2.
- *Compiler-theory-first.* The
  `const PATH: T = expr;` form is canonical Rust
  source; no parallel form invented.
- *No defensive fallbacks.* `value` is always
  populated; no `Option<ConstId>` wrap, no
  defensive `value.unwrap_or(...)` recovery.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2.3 -- new
  subsection "Const variant rendering" with
  per-aspect spec + composition outline + worked
  examples (scalar const, struct const, enum
  const).
- `pretty-printer-snapshot.md` §5.4 -- no edit.

**Worked example.**

```text
// Scalar const (i32):
const pl011::TX_FIFO_SIZE: i32 = 16_i32;

// Scalar const at usize:
const pl011::MAX_BAUD: usize = 4000000_usize;

// Struct const:
const pl011::DEFAULT_CONFIG: pl011::Config = pl011::Config {
    baud_rate: 115200_u32, parity: pl011::Parity::None,
};
// (Multi-field const renders on one line via
// render_const Aggregate dispatch per D-14-4.4.)

// Enum-variant const:
const pl011::DEFAULT_PARITY: pl011::Parity = pl011::Parity::None;

// ZST const (e.g., a PhantomData marker):
const pl011::MARKER: core::marker::PhantomData<u32> = core::marker::PhantomData;

// Reference-typed const:
const pl011::GREETING: &str = "hello";
// (render_const Bytes / Aggregate dispatch per
// D-14-4.4 + D-14-4.6 fat-pointer special case.)
```

#### D-14-5.5 -- ADT variants (Struct / Enum / Union) rendering [LOCKED]

**Decision.** **Option B (full definition body)
locked.** Per the prior-art Q7 evidence: rustc has
no MIR-side rendering for ADT type-defs (rustc emits
nominal types implicitly at use sites), so there is
no precedent tiebreaker between Option A
(header-only) and Option B (full layout body).
Option B wins on three grounds: matches the §2.1
master Rust-source-style; surfaces `spec.md` §11
Layout structure consistently with how field
projections render at use sites; supports
field-level snapshot diagnostics for driver fixtures.

`InstanceKind::{ Struct, Enum, Union } { layout: TyId }`
each render to a multi-line definition body. The
`layout: TyId` resolves to
`Ty::{ Struct, Enum, Union } { item, args, layout }`
per `spec.md` §9.4 / §10; the renderer reads:
- `inst.item` (== `Ty::*.item`) for the DefPath.
- `Ty::*.args` for generic-args decoration.
- `Ty::*.layout` (a `Layout` value carrying
  `LayoutKind` per §11) for the body's field /
  variant structure.

**Struct variant form.** `InstanceKind::Struct {
layout }` resolves to `Ty::Struct { item, args, layout
= Layout { kind: LayoutKind::Struct { fields }, .. } }`.
Dispatch on `(fields.len(), first-field-name-pattern)`:

| Case                                                          | Rendered form                                                                       |
|---------------------------------------------------------------|-------------------------------------------------------------------------------------|
| `fields.is_empty()`                                           | `[<attrs>\n]struct <path><args>;` (unit form -- collapses Rust's three syntactic empty-struct shapes) |
| `fields[*].name` all match `_<idx>` synthetic pattern (per §3.4) | tuple form (see below)                                                              |
| `fields[*].name` carry real source names                      | named-field form (see below)                                                        |

Tuple form:

```text
[<attrs>\n]struct <render_def_path(item)><render_generic_args(args, item)>(<render_ty(fields[0].ty)>, <render_ty(fields[1].ty)>, ...);
```

Named-field form:

```text
[<attrs>\n]struct <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

**Enum variant form.** `InstanceKind::Enum { layout }`
resolves to `Ty::Enum { item, args, layout = Layout
{ kind: LayoutKind::Enum { shape }, .. } }`. Dispatch
on `shape: EnumShape` per §11.1:

| `EnumShape`                                          | Rendered form                                                                    |
|------------------------------------------------------|----------------------------------------------------------------------------------|
| `Empty`                                              | `[<attrs>\n]enum <path><args> {}` (uninhabited; zero-variant empty body)         |
| `Single { variant, .. }`                             | per-variant arm inside `{ ... }`, one entry                                      |
| `Multiple { variants, .. }`                          | per-variant arms inside `{ ... }`, comma-separated, trailing comma after last    |

Per-variant arm form (each `variant: VariantLayout`
per §11.1, dispatching on
`(variant.fields.len(), first-field-name-pattern)`):

| Case                                                              | Rendered form                                                             |
|-------------------------------------------------------------------|---------------------------------------------------------------------------|
| `variant.fields.is_empty()`                                       | `<variant.name>` (unit variant)                                           |
| `variant.fields[*].name` all match `_<idx>` synthetic pattern     | `<variant.name>(<render_ty(fields[0].ty)>, ...)` (tuple variant)          |
| `variant.fields[*].name` carry real source names                  | `<variant.name> { <fields[0].name>: <render_ty(fields[0].ty)>, ... }` (named-field variant) |

Multi-variant body composition:

```text
[<attrs>\n]enum <render_def_path(item)><render_generic_args(args, item)> {
    <variant_arm_0>,
    <variant_arm_1>,
    ...
    <variant_arm_N>,
}
```

(Trailing comma after the last arm matches Rust
source idiom for multi-line enum bodies.)

**Union variant form.** `InstanceKind::Union {
layout }` resolves to `Ty::Union { item, args, layout
= Layout { kind: LayoutKind::Union { fields }, .. } }`.
Unions only carry named-field bodies in Rust source
(no tuple-union, no unit-union); the renderer
unconditionally produces:

```text
[<attrs>\n]union <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

If `fields.is_empty()` (which Rust source forbids,
but the IR allows structurally), render
`union <path><args> {}` -- no defensive special-case;
a model bug surfaces at validation, not at render.

**Common decoration (all three variants).**

- **Attribute block.** Same composition contract as
  D-14-5.2 / D-14-5.3 / D-14-5.4: multi-line `#[...]`
  prefix; composition rule pinned at §10.2.7 per
  D-14-5.6 LOCKED. Stage-14 ships no
  `AttrData::Intrinsic` variant occupying ADT Items
  in v1 (intrinsic recognition targets Function
  Items per `spec.md` §3.6.3 / D-14-1); the
  attribute block for ADT Instances reduces to
  Stream 1 (the §5.3 per-flag stream via the §5.6
  shared helper) in v1. For ADTs, Stream 1 surfaces
  lang-item flags
  `#[lang = "phantom_data"]` / `#[lang = "unsafe_cell"]` /
  `#[lang = "manually_drop"]`, `#[opaque_handle]`
  eligibility, `#[link_section]`, etc. Phase-2
  declarative attributes
  (`#[rseq::mmio(...)]` / `#[rseq::interrupt(...)]`)
  may attach to ADT Instances when their respective
  stages land; render rules for those `AttrData`
  variants live in the per-family stages (15+) per
  `spec.md` §3.6.6.

- **Path + generic args.**
  `<render_def_path(inst.item)><render_generic_args(Ty::*.args, inst.item)>`
  per stage-12 D-12-6 + stage-13 §6.7 (latter revised
  by D-14-4.9 to take `item: ItemId` for generic-arg
  param-type resolution). Bridge-synthesised ADTs
  (vtable types per §13.2, closure-captures per §13.1)
  carry their family-encoded paths unchanged.

- **Field type rendering.** `<render_ty(field.ty)>`
  per stage-13 D-13-5. Field types may themselves be
  generic-instance ADTs (rendered as path-only forms
  per §6.4) -- the definition body shows the field's
  Ty path, not the field-type's definition body.
  Use-site vs definition-site distinction: §6.4 is
  the use-site path-only form for `Ty::Struct` /
  `Ty::Enum` / `Ty::Union`; §10.2.4 / §10.2.5 /
  §10.2.6 is the definition-site full-body form for
  the corresponding Instance.

**Deferred sub-aspects** (Phase 1 v1 scope reduction;
explicit out-of-scope decisions):

- **`#[repr(...)]` decoration.** `Item.kind`'s
  §8.0 typed fields cover lang-item flags
  (`is_phantom_data` etc.) but not generic repr
  (no `repr: Repr` enum). Per `spec.md` §11.3
  ("What's dropped from rustc's `LayoutData`"),
  RegseqModel intentionally does not carry the
  source `repr` annotation; repr effects are
  resolved into the layout bytes (size, align,
  pack, field offset). Reconstructing
  `#[repr(C)]` / `#[repr(transparent)]` /
  `#[repr(packed)]` / `#[repr(u8)]` decoration
  from the layout would require an inverse-derivation
  pass on `Layout` (e.g., compare field offsets to
  the C-ABI layout to test for `repr(C)`); this is
  significant logic not needed for v1's Phase-1
  closure. **v1 renders no `#[repr(...)]`
  decoration.** Future revision: add a layout-
  derivation helper or extend `Item` with a typed
  `repr` field (mutable per "Spec is mutable";
  not pre-allocated per §5.9 rule 4).

- **Enum variant discriminant values.** Rust source
  syntax `Variant = N,` for explicit discriminants
  carries the `repr(int)` value at the syntactic
  level. RegseqModel's `VariantLayout.discriminant`
  (per §11.1) carries the resolved discriminant
  bit-pattern, and `tag_value` the layout-level
  tag. Rendering `<variant_name> = <discriminant>,`
  inline would require: (a) deciding whether to
  render for all variants or only when `repr(int)`
  is set (which requires the deferred `#[repr(...)]`
  decoration); (b) deciding signed-vs-unsigned
  rendering per `TagScalar.scalar_ty`. Both add
  complexity orthogonal to the load-bearing
  Option A vs B decision. **v1 renders no
  discriminant values.** Future revision: add
  discriminant rendering when the deferred
  `#[repr(...)]` work surfaces.

- **Field offsets and pack annotations.**
  `FieldLayout.offset` and `Layout.pack` are IR-
  internal layout details; not surfaced in v1
  rendered output (consistent with Rust source
  not carrying offsets / pack as type decoration).

- **Layout-internal fields** (`size`, `align`,
  `uninhabited`). Carried structurally for
  codegen; not surfaced in v1 rendered output.

- **`uninhabited` enum / variant markers.**
  `EnumShape::Empty` renders as `enum <path> {}`;
  per-variant `VariantLayout.uninhabited == true`
  doesn't surface as a special decoration (the
  variant renders normally per its field shape).
  Future revision may surface a comment-form
  marker (`// uninhabited`) if diagnostic value
  emerges.

**Reasoning.**

1. **Master Rust-source-style decision (D-13-1).**
   Rendering an ADT Instance's full definition body
   matches the §2.1 master decision: the
   pretty-printer is a Rust-source-style surface.
   A header-only rendering for ADTs (Option A)
   would feel incongruous next to Function /
   Static / Const definitions that render their
   full body / value. Snapshot diffs of a driver
   fixture's IR dump need to surface struct
   layout changes (e.g., adding a field, changing
   a field type) -- which requires the body.

2. **Use-site vs definition-site distinction is
   architecturally clean.** §6.4 already renders
   ADT references at use sites as path-only
   (`pl011::PL011<u32>`); D-14-5.5 adds the
   definition-site rendering with the full body.
   This mirrors how rustc's HIR pretty-printer
   handles type defs (full body at the def site,
   path-only at use sites) versus rustc's MIR
   pretty-printer (renders bodies only -- no type
   def rendering, because MIR doesn't enumerate
   type defs). RegseqModel inherits the HIR-style
   def-site rendering since it tracks ADTs as
   emission units.

3. **Empty form collapsing is minimal-loss.** Rust
   source allows three syntactically distinct
   empty-struct forms (`struct Foo;` /
   `struct Foo {}` / `struct Foo()`); they're
   semantically identical and RegseqModel's
   layout carries no disambiguating bit. v1 picks
   the unit form `struct Foo;` because it's the
   most common idiomatic source form for ZST
   markers (`PhantomData`-like). Snapshot diffs
   that need to round-trip the exact source form
   would need a layout-side discriminator; not
   pre-allocated.

4. **Tuple-vs-named dispatch via `_<idx>` name
   pattern per §3.4.** The §3.4 sigil convention
   already commits to synthetic `_<idx>` names
   for tuple fields. The renderer inspects field
   names to decide style. A Rust source struct
   never mixes tuple and named fields, so a
   single-field-check is sufficient (the first
   field's name pattern determines the whole
   struct's style).

5. **EnumShape-driven dispatch.** §11.1's three
   EnumShape variants (Empty / Single /
   Multiple) already encode the variant
   structure; the renderer composes per-arm
   forms via the same field-name pattern check
   as Struct. No parallel variant-shape flag.

6. **Generic args via §6.7 helper.** D-14-4.9
   already revised `render_generic_args` to take
   `(args, item)` so const-generic params resolve
   correctly. ADT-Instance rendering inherits;
   no further signature changes.

7. **Field type renders via `render_ty`, not
   recursive definition body.** Field types may
   themselves be Struct / Enum / Union Instances;
   recursive definition-body rendering would
   produce a transitive closure (rendering a top-
   level struct would render every referenced
   ADT's body). v1 renders field types as path-
   only references (per §6.4) -- each top-level
   Instance gets its own definition rendering;
   cross-references are by path. This mirrors
   Rust source where a struct's field type is the
   path to another type def, not its inline body.

8. **Repr decoration deferral matches §11.3 "what
   we drop from rustc's LayoutData."** RegseqModel
   intentionally drops the source `repr`
   annotation in favor of resolved layout bytes;
   reconstructing source-syntactic `#[repr(...)]`
   from the layout adds significant work for low
   v1 benefit. Future revision per "Spec is
   mutable."

9. **No discriminant-value rendering in v1.**
   Requires (a) the deferred `repr(int)`
   decoration; (b) signed vs unsigned rendering
   logic. Both deferred for the same
   minimal-v1-scope reason. Specimens distinguish
   variants by name + body shape; discriminant
   numeric values are not snapshot-diff load-
   bearing in v1.

**Rejected alternatives.**

- **(A) Header-only rendering** (just the
  §5.4 wrapper). Per the prior-art Q7 evidence,
  rejected because it loses snapshot-diff
  fidelity for ADT layout changes; doesn't
  match §2.1 master Rust-source-style. The
  evidence preponderance favors Option B.

- **Recursive definition-body rendering** (each
  field type rendered with its full body
  inline). Produces a transitive closure of
  rendered output per top-level Instance;
  duplicates content across Instances; breaks
  the use-site vs definition-site
  architectural distinction. Rejected.

- **Render `#[repr(...)]` decoration in v1
  by layout inverse-derivation.** Significant
  logic (comparing field offsets to C-ABI
  layout; testing for transparent-newtype
  shape; testing for packed bit alignment;
  reconstructing `repr(int)` from
  `TagScalar.scalar_ty`). Deferred to a future
  revision; v1 ships without repr decoration.

- **Render discriminant values in v1.** Requires
  the deferred repr decoration (for the
  numeric-form choice) + signed-vs-unsigned
  dispatch. Deferred.

- **Render field offsets / pack as trailing
  comments** (e.g., `field: T, // @offset 16`).
  IR-internal info; not source-shape. Out of
  v1 scope; a future
  `PrettyPrintOptions::include_layout_offsets`
  flag could opt in for diagnostic purposes.
  Not pre-allocated per §5.9 rule 4.

- **Disambiguate empty-struct forms via Item-
  side flag** (e.g.,
  `Item.empty_struct_form: Option<UnitOrEmptyBracesOrEmptyParens>`).
  Schema change for minimal-v1 benefit; the
  collapsed unit form is the idiomatic source
  form for ZST markers. Rejected.

**Methodology rules invoked.**

- *C semantics with information preservation.*
  The renderer surfaces the §11 Layout structure
  the bridge has already resolved (field names,
  variant names, structural shape); no
  re-derivation from source.
- *Compositional behavior beats special-case flags.*
  Tuple-vs-named dispatch via the existing §3.4
  `_<idx>` name pattern; no parallel `is_tuple`
  flag. EnumShape dispatch composes the existing
  §11.1 variant enum; no parallel `enum_shape_v1`
  marker.
- *Compiler-theory-first.* The HIR-style def-site
  full-body rendering + use-site path-only
  rendering is the canonical pretty-printer
  pattern for nominal type defs. RegseqModel
  follows the precedent rather than inventing a
  parallel surface.
- *Spec is mutable.* The deferred
  `#[repr(...)]` / discriminant-value /
  empty-struct-form-disambiguation work may
  surface in a future revision; v1 commits
  to the minimal load-bearing surface (Option B
  full body without repr / discriminant
  decoration).
- *No defensive fallbacks.* Empty unions
  (`fields.is_empty()` on a Union variant)
  surface as `union <path> {}` without
  defensive rejection at render time; a model
  construction bug if it occurs, caught by
  validation per `spec.md` §11.x invariants.
- *Code does not reference discussion-folder paths.*
  Spec text refers to `spec.md` §11.x Layout
  schema and §3.4 sigil convention, not to
  discussion-doc line numbers.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2.4 -- new
  subsection "Struct variant rendering" with
  the three-case dispatch table (empty / tuple
  / named-field) + composition outlines +
  worked examples (unit struct, tuple struct,
  named-field struct, generic struct,
  bridge-synthesised closure-captures struct).
- `pretty-printer-snapshot.md` §10.2.5 -- new
  subsection "Enum variant rendering" with
  EnumShape dispatch (Empty / Single /
  Multiple) + per-variant ctor-form dispatch
  (unit / tuple / named-field) + composition
  outlines + worked examples (empty enum,
  single-variant niche enum, multi-variant
  enum with mixed ctor forms, generic enum).
- `pretty-printer-snapshot.md` §10.2.6 -- new
  subsection "Union variant rendering" with
  named-field form + composition outline +
  worked examples (simple union, generic union,
  bridge-synthesised vtable union).
- `pretty-printer-snapshot.md` §10.2 intro
  outline updated to mark §10.2.4 / §10.2.5 /
  §10.2.6 LOCKED.
- `pretty-printer-snapshot.md` §6.4 -- no edit;
  the use-site path-only form for `Ty::Struct` /
  `Ty::Enum` / `Ty::Union` remains unchanged.
  D-14-5.5 adds the definition-site form
  alongside; the two forms coexist (different
  callers).
- `spec.md` §9.4 -- add a short cross-reference
  paragraph in `InstanceKind::{Struct, Enum,
  Union}` notes pointing to
  `pretty-printer.md` §10.2.4 / §10.2.5 /
  §10.2.6 for the definition-body rendering
  surface. Mirrors §9.7's pointer added at
  D-14-4 cluster close.

**Worked examples.**

```text
// Unit struct (zero fields; collapses to unit form):
struct pl011::Marker;

// Tuple struct:
struct pl011::Coord(i32, i32);

// Generic tuple struct:
struct pl011::Pair<u32, u8>(u32, u8);

// Named-field struct:
struct pl011::Config {
    baud_rate: u32,
    parity: pl011::Parity,
}

// Generic named-field struct with attribute:
#[lang = "manually_drop"]
struct core::mem::ManuallyDrop<pl011::Resource> {
    value: pl011::Resource,
}

// Bridge-synthesised closure-captures struct (§13.1):
struct <closure_captures for crate::main::closure#0> {
    captured_x: u32,
    captured_ref: &pl011::PL011,
}

// Empty enum (uninhabited):
enum core::convert::Infallible {}

// Single-variant enum:
enum pl011::Singleton {
    Only,
}

// Multi-variant enum with mixed ctor forms:
enum pl011::Parity {
    None,
    Even,
    Odd,
}

enum pl011::Status {
    Ready,
    Busy(u32),
    Error { code: i32, retry: bool },
}

// Generic enum:
enum core::option::Option<u32> {
    None,
    Some(u32),
}

// Simple union:
union pl011::RegBytes {
    word: u32,
    bytes: [u8; 4],
}

// Generic union:
union pl011::Reg<u32> {
    value: u32,
    bytes: [u8; 4],
}

// Bridge-synthesised vtable struct (§13.2):
struct <vtable_ty for dyn pl011::Writer> {
    drop:  *mut fn(*mut ()),
    size:  usize,
    align: usize,
    write: *mut fn(*mut (), u8) -> (),
}
```

Edge cases:
- **Zero-variant `EnumShape::Empty`**: `enum <path> {}` (e.g., `core::convert::Infallible`, `enum Never {}`).
- **Single-variant fieldless `repr(int)` enum**
  (`EnumShape::Single { scalar: Some(_) }`): the
  `scalar` discriminant info is **not surfaced** in
  v1 rendered output (deferred per the
  discriminant-value sub-aspect). The variant
  arm renders normally as `<variant.name>,`.
- **Mixed-shape variants** (some unit + some
  tuple + some named-field): each variant
  renders per its own field-name pattern;
  Rust source allows mixed shapes.
- **Bridge-synthesised types**: DefPath encodes
  the family per §13.4; rendered output uses
  the bracketed `<...>` synthetic form
  unchanged from `render_def_path`.

#### D-14-5.6 -- Attribute block composition rule [LOCKED]

**Decision.** The attribute-block prefix (referenced
across D-14-5.2 / D-14-5.3 / D-14-5.4 / D-14-5.5
per-variant envelopes) is composed by **interleaving
two deterministic streams in fixed order: Item-flag
stream first, then `domain_attrs.item_attrs[item]`
stream**. Each `#[...]` renders on its own line, in
the order produced by the stream interleaving; the
attribute block ends with a trailing `\n` when
non-empty, empty otherwise.

```text
Stream 1 (Item-flag stream)         -- emitted first
Stream 2 (domain_attrs.item_attrs)  -- emitted second
```

**Stream 1 -- Item-flag stream.** Walks the Item's
typed fields per the `pretty-printer.md` §5.3
per-flag table in **table row order**. Default-
suppression (§5.2) applies per-row; only non-default
values emit their `#[...]` form. The table row order
(pinned at stage-13 D-13-4.3 LOCKED) is:

1. `linkage` (per `Item.linkage: Linkage`):
   - `Default` -> suppressed.
   - `NoMangle` -> `#[no_mangle]`.
   - `Foreign` -> `#[foreign]`.
   - `Weak` -> `#[linkage = "weak"]`.
2. `inline_hint` (per `Item.inline_hint: InlineHint`):
   - `None` -> suppressed.
   - `Hint` -> `#[inline]`.
   - `Always` -> `#[inline(always)]`.
   - `Never` -> `#[inline(never)]`.
3. `is_naked == true` -> `#[naked]` (per current
   §5.3 row; pending removal at D-14-7).
4. Lang-item flags (per `Item` lang-flag fields):
   - `is_phantom_data == true` ->
     `#[lang = "phantom_data"]`.
   - `is_unsafe_cell == true` ->
     `#[lang = "unsafe_cell"]`.
   - `is_manually_drop == true` ->
     `#[lang = "manually_drop"]`.
5. `link_section` (per `Item.link_section:
   Option<StringId>`):
   - `Some(s)` -> `#[link_section = "<s>"]`.
   - `None` -> suppressed.
6. `opaque_handle_eligible == true` ->
   `#[opaque_handle]`.

(The `is_unsafe == true` flag does **not** render as
an attribute; per §5.3 it surfaces as the `unsafe`
keyword prefix on the `fn` kind-keyword, handled at
D-14-5.2 aspect 2.)

**Stream 2 -- `domain_attrs.item_attrs[item]`
stream.** Walks the `Vec<AttrData>` per the
`spec.md` §3.6.2 "**`Vec<AttrData>` ordering is
bridge-walk-order deterministic**" rule (D-14-3.3),
in **Vec-index order**. The bridge populates the
vector in Phase-4 per the §3.6.4 materialization
pass, which preserves the Phase-2 deterministic
encounter order. The renderer reads the populated
order directly; no sort, no filter.

Per-`AttrData` variant render rules (stage-14
vocabulary):

| `AttrData` variant                                  | Rendered form                                          |
|-----------------------------------------------------|--------------------------------------------------------|
| `Intrinsic(IntrinsicKind::Log(LogLevel::Debug))`    | `#[rseq::intrinsic("log-debug")]`                      |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Info))`     | `#[rseq::intrinsic("log-info")]`                       |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Warn))`     | `#[rseq::intrinsic("log-warn")]`                       |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Error))`    | `#[rseq::intrinsic("log-error")]`                      |

The dash-form NAME (`log-debug` / `log-info` /
`log-warn` / `log-error`) is the canonical
intrinsic NAME from `spec.md` §3.6.3
`IntrinsicKind::from_name` reversed; the surface
form `#[rseq::intrinsic("<NAME>")]` mirrors the
DSL-level recognition shape per Q7 + D-14-1.

**Phase-2 family render rules append.** Future
`AttrData` variants (`Mmio` per stage 16,
`Interrupt` per stage 17, `Atomic` per stage 18,
...) carry their per-variant render rules per the
relevant stage's spec; the per-`AttrData` render
table extends append-only per the §3.6.2 closed-
append-only-union rule. Stage-14 commits only the
`Intrinsic` variant rendering; per-family stages
own their respective rows when they land.

**Spec home.** New subsection
`pretty-printer-snapshot.md` §10.2.7 "Attribute
block composition" cross-references §5.3 per-flag
table + §3.6.2 `AttrData` ordering + per-`AttrData`
variant render rule. The per-variant subsections
§10.2.1 / §10.2.2 / §10.2.3 / §10.2.4 / §10.2.5 /
§10.2.6 reference §10.2.7 for the block composition
contract.

**Reasoning.**

1. **Item-flag stream first by author-intent
   convention.** Rustc-derived `CodegenFnAttrs` /
   `AdtFlags` attributes (`#[no_mangle]`,
   `#[inline(...)]`, `#[lang(...)]`, etc.) are the
   first-class toolchain decorations a Rust source
   author writes. Tool-namespace attributes
   (`#[rseq::intrinsic(...)]`, `#[clippy::lint(...)]`,
   `#[serde(...)]`, etc.) typically follow because
   they're domain-specific extensions on top of the
   base item. Putting Item-flags first reads more
   naturally as a snapshot diff -- the toolchain
   semantics are at the top, the Regseq DSL
   semantics layered below.

2. **§5.3 table-row order pinned at D-13-4.3.**
   The §5.3 per-flag table's row order is the
   stage-13 commitment for `render_item`'s
   standalone-Item view. D-14-5.6 inherits the
   same order so `render_item` and
   `render_instance` produce the same Stream 1
   attribute stream for any Function / Static /
   Const Item. Per D-14-5.7 LOCKED, both renderers
   consume the §5.6 shared
   `render_item_attribute_block` helper that walks
   §5.3 in row order, guaranteeing byte-identical
   Stream 1 output. Consistency between the two
   renderers is load-bearing for the §8 ownership
   table.

3. **`domain_attrs.item_attrs[item]` Vec-index
   order per §3.6.2 D-14-3.3.** The bridge-walk-
   order determinism is already a stage-14
   commitment (§3.6.2 / D-14-3.3 LOCKED); the
   renderer reads the order directly. No sort
   step, no filter step.

4. **Per-`AttrData::Intrinsic` rendering mirrors
   the DSL recognition surface.** The
   `#[rseq::intrinsic("NAME")]` attribute on the
   facade-side stub functions (per §26.3 LOCKED)
   is exactly what the bridge recognises via
   `tcx.get_attrs_by_path(did, &[sym::rseq,
   sym::intrinsic])` (per §3.6.4 LOCKED Phase-2
   recognition); rendering the resolved
   `IntrinsicKind` back to the same surface form
   makes round-trips legible. The dash-form NAME
   `log-<sev>` (per §3.6.3 LOCKED) is the
   canonical NAME from `IntrinsicKind::from_name`;
   reversing the map for rendering is the
   one-to-one inverse.

5. **Phase-2 family append-only contract.**
   `AttrData` is closed and append-only per §3.6.2;
   future phase-2 stages append `Mmio` /
   `Interrupt` / `Atomic` variants with their own
   render rules. Stage-14 commits only `Intrinsic`
   rendering; the per-`AttrData` render table is
   structurally extensible per the same closed-
   append-only convention.

6. **Multi-line block layout per §5.5.** Each
   `#[...]` on its own line, in stream-emit order;
   block ends with trailing `\n` when non-empty.
   §5.5's existing rule covers Item-flag block
   layout; D-14-5.6 extends to the combined
   stream.

7. **Empty-block treatment.** Zero attributes (no
   Item-flag rows surviving suppression + empty
   `domain_attrs.item_attrs[item]`) produces no
   prefix string; the function-header line (or
   static / const single-line composer) starts at
   column 0. Per §5.2 default-suppression rule.

**Rejected alternatives.**

- **(b) Domain stream first, Item-flag stream
  second.** Reverses (a). Reads less naturally
  as a snapshot diff -- DSL-specific decorations
  at the top, toolchain decorations below. The
  rustc convention of CodegenFnAttrs being
  primary (because it shapes codegen / linkage /
  inlining) suggests they should appear first.
  Rejected.

- **(c) Lexical ordering by attribute path
  segments.** `#[clippy::lint]` < `#[foreign]` <
  `#[inline]` < `#[lang = "manually_drop"]` <
  `#[link_section]` < `#[naked]` <
  `#[no_mangle]` < `#[opaque_handle]` <
  `#[rseq::intrinsic("log-info")]`. Imposes a
  sort step (extra work) and produces an order
  that doesn't match author intent (most authors
  write codegen attrs before tool-namespace
  attrs). Rejected.

- **(d) Source-attribute-order preservation.**
  Read the source-level attribute order from
  rustc's `tcx.get_all_attrs(did)` walk and
  preserve it. RegseqModel's bridge doesn't carry
  source attribute order -- per Q2 eager-translation
  + `spec.md` §8.0 "typed Item-level fields" the
  Item carries resolved per-flag values, not the
  source attribute sequence. Source-order
  preservation would require schema extension
  (e.g., a `Vec<RawAttribute>` per Item).
  Rejected; out of v1 scope.

- **(e) Single stream via unified AttrData
  encoding.** Lift Item-flag rows into
  `AttrData::CodegenFlag(...)` variants and merge
  into the single `domain_attrs.item_attrs[item]`
  stream. Refactors §3.6.1 schema (Item-flag
  fields disappear); breaks stage-12 D-12-1.3's
  typed-Item-level-fields commitment. Rejected
  per `Spec is mutable` boundary -- the typed
  per-flag fields are an intentional stage-12
  decision, not a renderer convenience.

- **(f) Per-flag table re-order from §5.3
  D-13-4.3.** Reordering the §5.3 table rows
  would re-flow `render_item`'s output too,
  breaking the snapshot-stability contract for
  the standalone-Item view. Rejected; §5.3 order
  is pinned at stage-13 D-13-4.3.

**Methodology rules invoked.**

- *Compositional behavior beats special-case flags.*
  Stream interleaving composes from existing
  ordered sources (§5.3 table + §3.6.2 Vec); no
  parallel "attribute_render_order: Vec<AttrId>"
  side-table.
- *C semantics with information preservation.*
  Per-`AttrData::Intrinsic` rendering surfaces the
  intrinsic kind via the same `#[rseq::intrinsic("NAME")]`
  shape the bridge recognised; round-trip-legible.
- *Compiler-theory-first.* The two-stream ordering
  (rustc-derived first, tool-namespace second) is
  the typical author-intent pattern in Rust source;
  RegseqModel adopts it without inventing a
  parallel convention.
- *No defensive fallbacks.* Per-`AttrData::Intrinsic`
  variant rendering is exhaustive over the closed
  4-variant `LogLevel` (`Debug` / `Info` / `Warn` /
  `Error`); no fallback for unknown levels (the
  enum is closed). Phase-2 stages extend the
  per-`AttrData` render table for their family
  variants per the append-only contract.
- *Spec is mutable.* The phase-2 `AttrData`
  variants (`Mmio` / `Interrupt` / `Atomic`)
  contribute their own per-variant render rules
  at the relevant stage's lock. Stage 14 doesn't
  pre-allocate (§5.9 rule 4); the contract is
  the append-only `AttrData` variant ordering.
- *Code does not reference discussion-folder paths.*
  Spec text refers to §5.3 per-flag table,
  §3.6.2 `AttrData` ordering rule, §3.6.3
  `IntrinsicKind` NAME map; no discussion-doc
  line citations.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.2.7 -- new
  subsection "Attribute block composition" with
  two-stream interleaving rule + Stream 1 (§5.3
  table-row order recap) + Stream 2 (per-`AttrData`
  variant render table for stage-14 vocabulary) +
  multi-line layout convention + empty-block
  treatment + worked examples.
- `pretty-printer-snapshot.md` §10.2.1 / §10.2.2 /
  §10.2.3 / §10.2.4 / §10.2.5 / §10.2.6 -- each
  per-variant subsection's "attribute block"
  aspect updated from "composition rule deferred
  to D-14-5.6" to "per §10.2.7" cross-reference.
- `pretty-printer-snapshot.md` §10.2 intro
  outline -- §10.2.7 added to the locked-
  subsections list.
- `pretty-printer-snapshot.md` §5.3 -- no edit.
  The §5.3 table-row order remains the
  authoritative source for Stream 1; D-14-5.6
  references it without modification.
- `spec.md` §3.6.2 -- no edit. The bridge-walk-
  order determinism rule is already locked at
  D-14-3.3; D-14-5.6 consumes the existing
  contract.

**Worked examples.**

```text
// Function Item with no surviving attributes
// (empty attribute block):
fn pl011::ordinary_function(_1: u32) -> u32 {
    let mut _0: u32;
    bb0: { ... }
}

// Function Item with Item-flag stream only:
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn pl011::init(_1: *mut pl011::PL011) -> i32 {
    let mut _0: i32;
    bb0: { ... }
}

// Function Item with both streams (Item-flag first,
// domain stream second):
#[no_mangle]                                 // Stream 1: linkage = NoMangle
#[inline(never)]                             // Stream 1: inline_hint = Never
#[rseq::intrinsic("log-info")]               // Stream 2: AttrData::Intrinsic(Log(Info))
fn regseq::log::intrinsics::log_info(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: { ... }
}

// Function Item with multi-attribute Stream 1 + single Stream 2 entry:
#[no_mangle]                                 // Stream 1
#[inline]                                    // Stream 1
#[link_section = ".driver_data"]             // Stream 1
#[rseq::intrinsic("log-error")]              // Stream 2
unsafe fn pl011::log_error_facade(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: { ... }
}

// Foreign declaration (Stream 1 only; Stream 2 empty):
#[foreign]                                    // Stream 1: linkage = Foreign
extern "C" fn libc::printf(_1: *const u8) -> i32;

// Struct Item with lang-item flag (Stream 1 only):
#[lang = "manually_drop"]                    // Stream 1: is_manually_drop = true
struct core::mem::ManuallyDrop<pl011::Resource> {
    value: pl011::Resource,
}

// Struct Item with multiple lang-flags + opaque_handle (illustrative;
// PhantomData + opaque_handle on the same Item is unusual in practice
// but the renderer composes mechanically):
#[lang = "phantom_data"]                     // Stream 1: is_phantom_data = true
#[opaque_handle]                              // Stream 1: opaque_handle_eligible = true
struct pl011::OpaqueMarker<u32>;
```

Edge cases:

- **Both streams empty**: empty prefix string;
  function-header / single-line composer starts at
  column 0 (no leading `\n`).
- **Stream 1 empty, Stream 2 non-empty**: only
  domain-stream rows render (matches the
  inheritance from §5.2 default-suppression).
- **Stream 1 non-empty, Stream 2 empty**: only
  per-flag rows render (the typical case for v1
  pre-domain-attribute Items).
- **Multiple Stream 2 entries**: each renders on
  its own line in Vec-index order; e.g., if a
  future Item carries both
  `AttrData::Intrinsic(Log(Info))` and a
  phase-2-added `AttrData::Mmio(...)` entry, the
  emit order follows the Vec-index, not a
  semantic-priority sort.

#### D-14-5.7 -- `render_item` vs `render_instance` layering [LOCKED]

**Decision.** **Option (a) -- composer-from-scratch
-- locked.** `render_instance` does **not** call
`render_item`; instead it composes the full per-
variant envelope directly via:
- A shared **`render_item_attribute_block(item:
  ItemId) -> String`** helper factored out from
  `pretty-printer.md` §5.3 (per-flag table) and
  §5.5 (block layout). The helper produces the
  multi-line `#[...]` block (with trailing `\n`
  when non-empty) for the Stream 1 portion of
  D-14-5.6.
- D-14-5.6's two-stream interleaving rule for the
  full attribute block (Stream 1 via the shared
  helper + Stream 2 via the
  `domain_attrs.item_attrs[item]` walk).
- The per-variant kind-keyword + path + signature /
  type / value / body composition rules locked at
  D-14-5.2 through D-14-5.5.

`render_item` (stage-13 D-13-4 / D-13-4.3 / D-13-4.5)
remains the **standalone Item-level view**: a single-
line per-`ItemKind` form used for diagnostics,
Item-table dumps, error messages, and anywhere a
self-contained item header is needed. Its signature
and output shape stay unchanged. Internally,
`render_item` consumes the same
`render_item_attribute_block` helper that
`render_instance` does -- shared composition source
for the attribute block ensures the two surfaces
produce byte-identical Stream 1 (Item-flag)
attribute prefixes for any
Item.

**Helper extraction shape:**

```rust
impl PrettyPrinter<'_> {
    // Public composer (stage 14):
    pub fn render_instance(&self, id: InstanceId) -> String;

    // Public standalone view (stage 13, unchanged):
    pub fn render_item(&self, id: ItemId) -> String;

    // Shared internal helper (factored out from §5.3 + §5.5
    // by D-14-5.7; not part of the public composer surface,
    // but pinned in spec so test snapshots and downstream
    // consumers can rely on the shared composition):
    fn render_item_attribute_block(&self, id: ItemId) -> String;
}
```

The helper's signature is internal to the
`PrettyPrinter` impl (not part of the public
renderer family). The spec pins its existence
so `render_item` and `render_instance`'s
attribute block surfaces are guaranteed
identical at the Item-flag stream level.

**Reasoning.**

1. **`extern "ABI"` decoration forces composer-
   from-scratch.** Per D-14-5.2 aspect 2, the
   Function envelope decorates the kind-keyword
   line with `[unsafe ][extern "ABI" ]fn`. The
   `extern "ABI"` source is `fn_sig.abi`
   (Instance-side); `render_item` has no Instance
   context (it takes an `ItemId`, not an
   `InstanceId`), so it cannot emit
   `extern "ABI"` decoration. Two paths to
   reconcile:
   - Option (a): `render_instance` composes the
     header line itself (using the §5.3 attribute
     helper for the prefix); `render_item` stays
     as a standalone view that emits
     `[#[attrs]\n][unsafe ]fn $path` (no ABI).
   - Option (b): `render_item` accepts an optional
     ABI context argument so callers in
     `render_instance` can pass `fn_sig.abi`. Forces
     a §5.4 signature extension (`render_item(id,
     abi_override: Option<FnAbi>)` or similar);
     callers without ABI context (the standalone
     diagnostic surface) pass `None`. The signature
     change ripples to every `render_item` caller.

   Option (a) keeps `render_item`'s signature
   stable and pushes the ABI-aware composition
   into `render_instance`. This matches the
   prior-art Q3 recommendation.

2. **ADT definition body forces composer-from-
   scratch.** Per D-14-5.5, Struct / Enum / Union
   ADT Instances render with a full definition
   body (`struct PATH<args> { field: T, ... }` /
   `enum PATH<args> { Variant, ... }` /
   `union PATH<args> { field: T, ... }`).
   `render_item`'s §5.4 wrapper produces just
   `struct $path` / `enum $path` / `union $path`
   (no `<args>`, no body). Calling `render_item`
   and then appending the body would require
   either parsing the §5.4 output to inject the
   `<args>` decoration before the body, or
   double-emitting the path. Cleaner: `render_instance`
   composes its own header (`struct
   <render_def_path(item)><render_generic_args(...)>`)
   plus body, sharing only the attribute block
   helper.

3. **Static / Const trailers forces composer-
   from-scratch.** Per D-14-5.3 / D-14-5.4,
   Static / Const Instances render with a type
   decoration `: <render_ty(ty)>` and an
   initializer `= <render_const(...)>` /
   declaration `;` trailer. `render_item`'s §5.4
   wrapper produces just `static [mut ] $path` /
   `const $path` (no type, no initializer).
   Appending type + initializer is structurally
   straightforward, but the symmetry across all
   six per-variant envelopes (Function / Static /
   Const / Struct / Enum / Union) is cleaner if
   `render_instance` composes from scratch for
   every variant.

4. **Shared attribute helper preserves
   consistency.** The architectural concern with
   option (a) is divergence: if `render_instance`
   composes its own attribute block separately
   from `render_item`'s, the two surfaces could
   drift (e.g., `render_item` updates §5.3 row
   order, but `render_instance` lags). The
   factored `render_item_attribute_block` helper
   eliminates this: both renderers consume the
   same composition source. Pinning the helper's
   existence in spec makes the consistency
   contract explicit.

5. **Public surface unchanged.** Stage-13's
   `render_item(ItemId) -> String` API (per
   D-13-4 / D-13-4.5 LOCKED) keeps its
   single-argument signature; no callers update.
   The standalone-Item view continues to serve
   diagnostics / Item-table dumps with no
   per-Instance dependencies.

6. **§8 ownership table update is mechanical.**
   The §8 per-method ownership table row for
   `render_instance(InstanceId)` updates its
   Description column from the prior "composes
   `render_item` + per-Instance content" wording
   (a stage-13 D-13-4.6 forward-pointer) to
   "composes `render_def_path` + `render_ty` +
   body-level renderers + `render_const` +
   `render_item_attribute_block` (shared helper)
   per §10.2". This finalises the §8 row that
   D-14-5.1 / D-14-5.2 deferred.

**Rejected alternatives.**

- **(b) `render_item` wrapper + per-Instance
  content append.** Forces a §5.4 signature
  extension (`render_item(id, abi_override:
  Option<FnAbi>)`) to thread ABI context into
  the Function envelope; ripples to every
  `render_item` caller. Doesn't solve the ADT
  body-vs-header structural divergence
  (`render_item` can't produce `<args>` +
  body without further coupling). Rejected.

- **(c) Inline §5.3 table walk in
  `render_instance` without factoring a shared
  helper.** Duplicates the §5.3 per-flag
  composition logic across two renderers;
  invites drift. Rejected.

- **(d) Render the ABI prefix as an attribute
  `#[extern("C")]`** instead of as a keyword
  prefix on `fn`. Diverges from Rust source
  grammar (`extern "C"` is a function-decl
  keyword, not an attribute). Rejected per
  the Rust-source-style master decision
  D-13-1.

- **(e) Two parallel attribute-block helpers
  (one Item-side, one Instance-side).** Same
  drift risk as (c); two sources of truth for
  the §5.3 per-flag composition. Rejected.

- **(f) Eliminate `render_item` entirely;
  `render_instance` is the only renderer.**
  Removes a stage-13 D-13-4 public API.
  `render_item` serves diagnostic / Item-table
  use cases that don't need Instance context
  (e.g., printing Item metadata before
  Instance population, or printing Items
  per-DefPath in a debug dump). Stage-13's
  decision to ship `render_item` was deliberate
  (D-13-4 LOCKED); D-14-5.7 doesn't reopen.
  Rejected.

**Methodology rules invoked.**

- *Compiler-theory-first.* Composer-from-scratch
  is the canonical pattern when a per-Instance
  composer needs context (FnSig, layout) that
  the Item-level renderer doesn't carry. Rustc's
  HIR / MIR pretty-printer split follows the
  same pattern.
- *Compositional behavior beats special-case flags.*
  The shared `render_item_attribute_block` helper
  composes from the existing §5.3 table data; no
  parallel "should_render_in_instance_context"
  flag.
- *Spec is mutable.* The §5.3 / §5.5 helper
  factor-out is a localized stage-13 spec text
  reorganization (table content moves to a
  reusable subsection), not a semantic change.
  Stage-13's `render_item` public API is
  unchanged; the helper extraction is internal.
- *No defensive fallbacks.* The shared helper has
  no `Result` / `Option` return; it produces a
  `String` (possibly empty) for any valid
  `ItemId`. Bridge invariants per stage-12
  D-12-1.3 / D-12-2.x guarantee Item-flag fields
  are populated.
- *Code does not reference discussion-folder paths.*
  Spec text references §5.3 / §5.5 / §10.2.7 /
  D-14-5.2 / D-14-5.5 by tag and section
  number; no line citations.

**Spec impact.**

- `pretty-printer-snapshot.md` §5 -- new
  subsection `§5.6` (or appended to §5.5)
  pinning the `render_item_attribute_block(item:
  ItemId) -> String` shared helper as the
  composition source for both `render_item`
  (§5.4) and `render_instance` (§10.2 per-variant
  subsections via §10.2.7 attribute-block
  composition rule). The helper produces the
  multi-line `#[...]` block (with trailing `\n`
  when non-empty) for the Stream 1 portion of
  D-14-5.6. §5.3 per-flag table + §5.5 block
  layout are pinned as the helper's data sources;
  no row reorder, no rule change.
- `pretty-printer-snapshot.md` §8 per-method
  ownership table -- `render_instance(InstanceId)`
  row Description column finalised: "composes
  `render_def_path` + `render_ty` + body-level
  renderers + `render_const` (per D-14-4) +
  `render_item_attribute_block` (shared helper
  per §5.6) per §10.2; does **not** call
  `render_item`". `render_item(ItemId)` row
  unchanged at the public signature level;
  Description column gains a "uses
  `render_item_attribute_block` (shared helper)
  internally" note.
- `pretty-printer-snapshot.md` §10.2.1 / §10.2.2 /
  §10.2.3 / §10.2.4 / §10.2.5 / §10.2.6 -- each
  per-variant subsection's "attribute block"
  aspect cross-references §10.2.7 (composition
  rule) which in turn cross-references §5.6
  (shared helper).
- `pretty-printer-snapshot.md` §1.3 composer
  listing -- `render_instance` description
  finalised: "composes per-Instance content +
  shared `render_item_attribute_block` helper
  (§5.6); does NOT call `render_item`".
- Stage-13 D-13-4 / D-13-4.5 -- no REOPEN. The
  factor-out is a spec-text reorganization (§5.3
  table content cited from a shared helper
  subsection); the public API + table content
  are unchanged.

**Cross-stage friction.** No stage-13 D-tag flips.
The shared helper extraction is internal
re-organization within `pretty-printer.md` §5;
stage-13's `render_item` public API per D-13-4.5
is unchanged. The §5.3 per-flag table (D-13-4.3
LOCKED) is unchanged. The §5.5 block layout
(D-13-4 LOCKED) is unchanged. Future Codex review
at Phase B may flag the helper's interaction
with span / debuginfo modes (§4 gating); held for
review.

**Worked example.**

```rust
impl PrettyPrinter<'_> {
    pub fn render_instance(&self, id: InstanceId) -> String {
        let inst = &self.model.instances[id];
        let attr_block = self.render_item_attribute_block(inst.item);
        // Combined with domain_attrs stream per §10.2.7:
        let domain_attrs = self.render_domain_attribute_block(inst.item);
        let attrs = format!("{attr_block}{domain_attrs}");

        match &inst.kind {
            InstanceKind::Function { fn_sig, body, .. } => {
                // §10.2.1: attrs + [unsafe ][extern "ABI" ]fn PATH(args) -> Ret + { body or ; }
                let sig_line = self.render_function_signature_line(inst, fn_sig);
                let body_or_decl = match body {
                    Some(b) => format!(" {{\n{}\n}}", self.render_body_envelope(b)),
                    None    => ";".into(),
                };
                format!("{attrs}{sig_line}{body_or_decl}")
            }
            InstanceKind::Static { ty, init } => {
                // §10.2.2: attrs + static [mut ] PATH: <ty> [= <init>];
                let mut_prefix = match self.model.items[inst.item].kind {
                    ItemKind::Static { mutability: Mutability::Not } => "static ",
                    ItemKind::Static { mutability: Mutability::Mut } => "static mut ",
                    _ => unreachable!("InstanceKind::Static <-> ItemKind::Static"),
                };
                let path = self.render_def_path(inst.item);
                let ty_str = self.render_ty(*ty);
                let init_str = match init {
                    Some(c) => format!(" = {}", self.render_const(*c, *ty)),
                    None    => String::new(),
                };
                format!("{attrs}{mut_prefix}{path}: {ty_str}{init_str};")
            }
            InstanceKind::Const { ty, value } => {
                // §10.2.3: attrs + const PATH: <ty> = <value>;
                let path = self.render_def_path(inst.item);
                let ty_str = self.render_ty(*ty);
                let value_str = self.render_const(*value, *ty);
                format!("{attrs}const {path}: {ty_str} = {value_str};")
            }
            InstanceKind::Struct { layout } => {
                // §10.2.4: attrs + struct PATH<args>[(...) | { ... }];
                self.render_adt_struct(inst.item, *layout, &attrs)
            }
            InstanceKind::Enum { layout } => {
                // §10.2.5: attrs + enum PATH<args> { variants }
                self.render_adt_enum(inst.item, *layout, &attrs)
            }
            InstanceKind::Union { layout } => {
                // §10.2.6: attrs + union PATH<args> { fields }
                self.render_adt_union(inst.item, *layout, &attrs)
            }
        }
    }

    pub fn render_item(&self, id: ItemId) -> String {
        let attr_block = self.render_item_attribute_block(id);
        let item = &self.model.items[id];
        let kind_keyword = match &item.kind {
            ItemKind::Function => {
                let unsafe_prefix = if item.is_unsafe { "unsafe " } else { "" };
                format!("{unsafe_prefix}fn")
            }
            ItemKind::Static { mutability: Mutability::Not } => "static".into(),
            ItemKind::Static { mutability: Mutability::Mut } => "static mut".into(),
            ItemKind::Const => "const".into(),
            ItemKind::Struct => "struct".into(),
            ItemKind::Enum => "enum".into(),
            ItemKind::Union => "union".into(),
        };
        format!("{attr_block}{kind_keyword} {}", self.render_def_path(id))
    }

    fn render_item_attribute_block(&self, id: ItemId) -> String {
        // Shared helper: walks the §5.3 per-flag table in row order;
        // emits one #[...] line per non-default field; returns the
        // multi-line block (with trailing \n) or empty string.
        let item = &self.model.items[id];
        let mut lines = Vec::new();
        // 1. linkage (per §5.3 row 1)
        match item.linkage {
            Linkage::Default  => {}
            Linkage::NoMangle => lines.push("#[no_mangle]".into()),
            Linkage::Foreign  => lines.push("#[foreign]".into()),
            Linkage::Weak     => lines.push(r#"#[linkage = "weak"]"#.into()),
        }
        // 2. inline_hint (per §5.3 row 2)
        match item.inline_hint {
            InlineHint::None   => {}
            InlineHint::Hint   => lines.push("#[inline]".into()),
            InlineHint::Always => lines.push("#[inline(always)]".into()),
            InlineHint::Never  => lines.push("#[inline(never)]".into()),
        }
        // 3-6: remaining §5.3 rows...
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    }
}
```

(The sketch above is illustrative; the spec
text in `pretty-printer-snapshot.md` §5.6 +
§10.2 finalises the exact dispatch shape.
Stage-13's `render_item` public signature is
unchanged.)

#### D-14-5.8 -- Cross-stage friction outcome [LOCKED]

**Decision.** D-14-5 cluster Phase A drafting
(D-14-5.1 through D-14-5.7) introduces **zero new
stage-13 D-tag REOPENs**. The working assumption
recorded at cluster allocation -- "no anticipated
REOPENs of stage-13 D-tags from current evidence"
-- holds after the full Phase A drafting pass.

**Audit pass.** Per-sub-decision touchpoints into
stage-13 / earlier-stage spec, with verdict:

- **D-14-5.1 (API signature).** No stage-13 spec
  text touched. Adds `render_instance(InstanceId)
  -> String` signature finalisation to the §10.2
  intro; the stage-13 D-13-4.6 forward-pointer
  ("composer ownership listing in §1.3") referenced
  this signature without locking the form. D-14-5.1
  completes the forward-pointer; no REOPEN.

- **D-14-5.2 (Function variant envelope).**
  Composes §3.1 body envelope (D-13-2 LOCKED)
  unchanged; composes §5.3 per-flag table
  (D-13-4.3 LOCKED) unchanged; composes body-level
  renderers per §7 (D-13-6 -- already REOPENED by
  D-14-4.9 with the three-signature cascade; not
  re-touched by D-14-5.2). The Function envelope's
  `extern "ABI"` decoration sources `fn_sig.abi`
  per §14.3 (stage-3 LOCKED) and §6.5 fn-pointer
  rendering (D-13-5 LOCKED, unchanged). No new
  REOPENs.

- **D-14-5.3 (Static variant).** Composes
  `render_const` (D-14-4.1 LOCKED) unchanged;
  composes `render_def_path` (D-12-6 LOCKED)
  unchanged; reads `ItemKind::Static { mutability }`
  per stage-12 D-12-1.2 (c) LOCKED unchanged.
  Biconditional `init.is_none() iff Item.linkage
  == Linkage::Foreign` per `spec.md` §3.4 step 8 /
  D-12-2.2 LOCKED unchanged. No REOPENs.

- **D-14-5.4 (Const variant).** Composes
  `render_const` + `render_ty` + `render_def_path`
  unchanged; reads `value: ConstId` always-populated
  invariant per stage-12 D-12-2 LOCKED. No REOPENs.

- **D-14-5.5 (ADT variants Option B).**
  Composes `spec.md` §11 Layout schema (stage-3
  LOCKED) unchanged; reads `EnumShape` per §11.1
  LOCKED unchanged; reads `FieldLayout` /
  `VariantLayout.name: StringId` non-optionality per
  §11.0 LOCKED unchanged; consumes `render_ty`
  (D-13-5 LOCKED) unchanged; consumes
  `render_generic_args` (D-13-5 / D-14-4.9 LOCKED
  for the `(args, item)` signature) unchanged. The
  field-name pattern check (synthetic `_<idx>` per
  §3.4 / §11.0 tuple convention) reads existing
  schema text. Added a cross-reference paragraph
  to `spec.md` §9.4 InstanceKind notes (mirrors
  D-14-4's §9.7 Const pointer addition; §5.9-rule-
  allowed live cross-reference, not a structural
  edit). Use-site §6.4 path-only form coexists
  with definition-site full-body form; §6.4
  unchanged. No REOPENs.

- **D-14-5.6 (Attribute block composition).**
  Stream 1 walks §5.3 per-flag table in row order
  (D-13-4.3 LOCKED, unchanged); Stream 2 walks
  `domain_attrs.item_attrs[item]` per `spec.md`
  §3.6.2 D-14-3.3 bridge-walk-order determinism
  (stage-14 LOCKED, unchanged). Per-`AttrData`
  render table for `Intrinsic` variant uses the
  inverse of §3.6.3 `IntrinsicKind::from_name`
  (D-14-2.1 LOCKED) -- one-to-one inverse, no
  schema change. No REOPENs.

- **D-14-5.7 (Layering boundary).** Factored §5.6
  shared helper `render_item_attribute_block(item:
  ItemId) -> String` out of §5.3 + §5.5 content.
  The factor-out is **additive spec text**: §5.3
  table row order + content unchanged (D-13-4.3
  LOCKED); §5.5 block layout rule unchanged
  (D-13-4 LOCKED); §5.4 per-`ItemKind` wrapper +
  `render_item` public signature unchanged
  (D-13-4.5 LOCKED). The helper's existence is a
  new §5.6 subsection that captures the shared-
  composition contract; it doesn't modify locked
  stage-13 spec content. Analogous to D-14-4
  adding §10 Composer renderers without REOPENing
  stage-13's §6 / §7 building-block sections.
  Updated §8 ownership table rows for
  `render_instance` + `render_item` to reflect
  the helper-sharing relationship (these were
  stage-13 D-13-4.6 forward-pointers awaiting
  stage-14 finalisation, not REOPENs); added a
  new row for `render_item_attribute_block` as a
  stage-14 internal helper. No D-13-4 / D-13-4.3
  / D-13-4.5 / D-13-2 / D-13-6 REOPENs.

**Friction watch -- pending flips from earlier
clusters (unchanged by D-14-5).**

Pending at stage-14 merger close (carried from
prior cluster locks):

- **D-9-10 (Terminator-site invariants registry)
  -- REOPEN flagged by D-14-2.5.** Annotation
  "(revised in stage 14 D-14-2.5: B-4 and B-6
  carved out for `IntrinsicKind::Log(_)` callees
  per I-DiagLog-CallArity invariant)". Status
  flip in `decisions.md` at merger.

- **D-13-6 (Body-level renderer per-arm specs)
  -- REOPEN flagged by D-14-4.9.** Annotation
  "(revised in stage 14 D-14-4.9: `render_operand`
  and `render_rvalue` signatures extended with
  `expected_ty: TyId`; `render_generic_args`
  signature extended with `item: ItemId`)". Status
  flip in `decisions.md` at merger.

- **D-12-1.3 + D-9-9 + D-13-4.3 -- pending
  REOPEN flags from D-14-7 (when locked).**
  D-14-7's `#[naked]` rejection / `is_naked` field
  removal will flip these three D-tags at merger.
  D-14-5 doesn't touch the `is_naked` flag itself
  (the §5.3 row + helper walk include it as-is
  pending the D-14-7 removal).

**No new D-14-5-generated friction.**

**Phase B exposure risk.** The Phase B Codex
review on the full `pretty-printer-snapshot.md`
D-14-5 contributions may surface late findings.
Per `Spec is mutable`, the D-14-5.8 friction
outcome may be revised if Phase B uncovers
cascade effects not visible at Phase A drafting
time. Most likely surfacing patterns based on the
D-14-4 cluster's R1-R4 trajectory:
- Cascade-completeness review (does every
  per-variant subsection cross-reference the
  shared §5.6 helper consistently? does §1.3 /
  §8 / §10.2 intro outline agree on the helper's
  scope?).
- Specimen revision (the §10.2.x subsections
  embed worked examples; §9.11 specimens added
  per the 9-step plan step 8 -- consistency
  between the two surfaces tested at Phase B).
- Recap-text duplication review (status.md +
  spec-snapshot.md + pretty-printer-snapshot.md
  description fragments need consistent wording
  on the helper-from-scratch boundary).

**Methodology rules invoked.**

- *Spec is mutable.* If Phase B surfaces a
  stage-13 REOPEN-shaped finding, this entry is
  revised; cluster closure waits for the friction
  outcome to be confirmed against the Phase B
  review.
- *No defensive fallbacks.* D-14-5.8 records the
  current friction outcome; no defensive
  pre-emptive flagging of "potential future
  REOPENs" -- if a REOPEN materializes during
  Phase B or later, the relevant D-tag flips at
  that point.

**Spec impact.** No new spec edits at D-14-5.8
lock. The friction-outcome paragraph is recorded
in this audit-shape entry (and recapped in
`status.md`'s "Cross-stage friction" section)
without adding content to `spec-snapshot.md` or
`pretty-printer-snapshot.md`.

**Next steps (per the 9-step Phase A plan).**

1. **Specimens (Step 8) -- DONE.** 19 specimens
   (110-128) landed at `pretty-printer-snapshot.md`
   §9.11 covering each `InstanceKind` variant +
   edge cases per D-13-7 specimen convention. The
   §10.2.x subsections already embed worked
   examples for documentation; §9.11 specimens
   are the full snapshot-test corpus.

2. **Phase B Codex review (Step 9) -- CLOSED.**
   Per-stage Phase B review on
   `pretty-printer-snapshot.md` D-14-5
   contributions opened 2026-05-13. 4-round
   trajectory closed 2026-05-13 at R4 APPROVE /
   Lockability:yes / Close-out:YES: R1 BLOCK with
   7 findings (3 BLOCK + 3 MINOR + 1 NIT) -> R2
   BLOCK with 4 findings (1 BLOCK + 2 MINOR +
   1 NIT) -> R3 MINOR / Lockability:yes with 2
   findings (0 BLOCK + 2 MINOR + 0 NIT) -> R4
   APPROVE 0 findings. Total: 13 distinct findings
   closed across R1-R3 (5 R1/R2/R3 continuations
   + 8 fresh). All R1-R3 resolutions verified
   clean at R4. **No stage-13 D-tag REOPENs
   introduced** by any R1-R3 fix; the §5.6 helper
   factor-out (D-14-5.7) is additive spec text
   within §5, not a §5.3 / §5.5 table modification.
   Recap-text-duplication pattern (matching D-14-4):
   R1 caught canonical locations; R2-R3 caught
   audit-shape narrative + sigil-table +
   forward-pointer spots that the prior round
   missed.



Three cross-cutting decisions threaded through the
D-14-1 / D-14-3 discussion. Each is folded into the
relevant sub-decision above; recapped here for
discoverability.

### Operational-vs-declarative attribute architecture

Resolved by the rustc evidence pass + the D-14-3.0
unified-storage choice: the recognition mechanism is
naturally unified (one `tcx.get_attrs_by_path` codepath
handles both operational `#[rseq::intrinsic(...)]` and
declarative `#[rseq::mmio(...)]` attributes); storage is
also unified via the `AttrData` discriminated union.
Operational and declarative semantic roles differ only at
the consumer level (codegen dispatch vs property lookup),
not at the architectural level.

### Instance attribute access mechanism

Resolved by D-12-1.8 + D-14-3.1: Instances access Item-
level domain attributes via the §3.5.2 pull-through
pattern. `instance_attrs` is structurally redundant
under the established convention and dropped from
`DomainAttrTables`. Per-Instance state that genuinely
depends on monomorphization context lives inline in
`InstanceKind`, not as a sibling-table attribute.

### Stage-14 closure-spec preparation for impl sweep #1

(Cross-cutting question surfaced in memory.md.) Resolved
by adopting README §2's intra-stage spec-only discipline:
stage-14 spec stays declarative; implementation-orientation
prose (population codepath sketches, validator hook
points, etc.) lives at the level of the per-decision
"Spec impact" notes above and in the structural §3.X
content -- not as separate implementation notes. Impl
sweep #1 designs its codepath against the locked spec.
Spec-shaped clarifications surfaced during the sweep
amend stage 14 in-place per the merger-contract
re-open mechanism.

---

## 3. Status summary

- **D-14-1 + D-14-3 cluster** -- LOCKED 2026-05-12 at
  per-stage Phase A (audit-shape recorded). Phase B
  Codex review closed at R3 MINOR with Lockability:yes
  / Close-out:YES (3 rounds; 8 findings: 1 BLOCK + 4
  MINOR + 3 NIT).
- **D-14-2 cluster** -- LOCKED 2026-05-13 at per-stage
  Phase A. Phase B Codex review closed at R6 APPROVE
  with Lockability:yes / Close-out:YES (6 rounds; 14
  findings: 0 BLOCK + 11 MINOR + 3 NIT). 9 sub-decisions
  D-14-2.1 through D-14-2.9 LOCKED.
- **D-14-4 cluster** -- LOCKED 2026-05-13 at per-stage
  Phase B R4 APPROVE / Lockability:yes / Close-out:YES.
  Phase A LOCKED 2026-05-13. All 12 sub-decisions
  (D-14-4.1 through D-14-4.12) drafted in a single
  session covering the full `render_const(ConstId,
  TyId) -> String` composer-renderer surface.
  **D-14-4.1** (API signature; load-bearing pair-form);
  **D-14-4.2 / D-14-4.10 / D-14-4.11 / D-14-4.12**
  (scalar-variant surface: underscore-decimal
  convention + sentinel collapsing + decimal-only +
  non-finite forms); **D-14-4.3 / D-14-4.7 /
  D-14-4.8** (single-variant type-context-dependent
  shapes: ZST exhaustive dispatch + NullPtr typed-cast
  + Reloc with FnDef/FnPtr-coercion/Static/
  ConstMemoryStatic b"..." special-case);
  **D-14-4.4 / D-14-4.5 / D-14-4.6** (multi-arm
  dispatches: Aggregate Array/Tuple/Struct + `&str`/
  `&[u8]` fat-pointer specials; EnumValue per
  `EnumShape::Single`/`Multiple` variant dispatch;
  Bytes all-uninit + byte-array `*b"..."` +
  `bytes![...]` structural fallback); **D-14-4.9**
  (type-context threading + operand-callsite wiring
  -- REOPENS stage-13 D-13-6 to extend `render_operand`
  signature with `expected_ty: TyId` parameter). Spec
  contributions land at `pretty-printer-snapshot.md`
  new §10 Composer renderers (with §10.1.1 - §10.1.8
  subsections) + §11 Cross-references (renumbered
  from §10) + §9 specimen revisions to
  underscore-decimal form + §7.2 `render_operand`
  signature extension (per D-14-4.9). **Cross-stage
  friction**: D-13-6 REOPEN pending flip at merger
  close (annotation "revised in stage 14 D-14-4.9"
  for the `render_operand` signature extension).

- **D-14-5 cluster** -- LOCKED 2026-05-13 at
  Phase B R4 APPROVE / Lockability:yes / Close-out:YES.
  Phase A LOCKED 2026-05-13.
  All 8 sub-decisions LOCKED (D-14-5.1 API signature;
  D-14-5.2 Function variant envelope; D-14-5.3 Static
  variant; D-14-5.4 Const variant; D-14-5.5 ADT
  variants Option B full definition body; D-14-5.6
  attribute block composition two-stream interleaving;
  D-14-5.7 layering boundary composer-from-scratch
  with §5.6 shared helper; D-14-5.8 zero new
  stage-13 REOPENs). Step 4 rustc evidence pass
  landed at `prior-art/rustc-instance.md` (10
  questions Q1-Q10 across body envelope +
  per-MonoItem dispatch + ADT-def rendering
  surface). Spec contributions:
  `pretty-printer-snapshot.md` §10.2.1-§10.2.7
  per-variant + attribute-block subsections;
  §5.6 shared helper subsection; §10.2 intro
  outline + §8 ownership table + §1.3 composer
  listing finalised. `spec-snapshot.md` §9.4
  Consumer-side-rendering cross-reference
  paragraph added (mirrors §9.7 Const pointer
  added at D-14-4). **Cross-stage friction**:
  none new from D-14-5. Pending merger-time flips
  unchanged: D-9-10 from D-14-2.5, D-13-6 from
  D-14-4.9, plus D-12-1.3 / D-9-9 / D-13-4.3 when
  D-14-7 locks. **Specimens added**: 19 specimens
  (110-128) at `pretty-printer-snapshot.md` §9.11
  per D-13-7 convention, covering each
  `InstanceKind` variant + edge cases. **Remaining
  cluster work**: NONE -- Phase B Codex review
  closed at R4 APPROVE / Close-out:YES 2026-05-13.
  4-round trajectory: R1 BLOCK 7 findings (3B+3M+1N) ->
  R2 BLOCK 4 findings (1B+2M+1N) -> R3 MINOR
  Lockability:yes 2 findings (0B+2M+0N) -> R4
  APPROVE 0 findings. 13 distinct findings closed
  across R1-R3 (5 continuations + 8 fresh). No
  stage-13 D-tag REOPENs introduced by any fix.
  Total rounds match the D-14-4
  trajectory).

- **D-14-6 cluster** -- LOCKED 2026-05-13 at
  per-stage Phase B R6 APPROVE / Lockability:yes /
  Close-out:YES. All 6 sub-decisions (D-14-6.1
  through D-14-6.6) LOCKED. Spec contributions:
  `pretty-printer-snapshot.md` §10.3 (7 subsections
  §10.3.1-§10.3.7) + §1.3 + §8 finalisations + §9.12
  specimens 129-133 (5 specimens) +
  `prior-art/rustc-render.md` (6 Q-tags) +
  `rustc-references.md` Top-level render section (4
  file:line landmarks). 6-round Phase B trajectory
  (recap-text-duplication cascade); 14 findings
  closed (2B + 12M + 0N); Lockability:yes sustained
  across R2-R6. No stage-13 D-tag REOPENs from
  D-14-6. Closes the composer-renderer trio per
  stage-13 D-13-4.6 (the third and final of
  `render_const` / `render_instance` / `render`).

- **D-14-7 cluster** -- LOCKED 2026-05-13 at
  per-stage Phase B R6 APPROVE / Lockability:yes /
  Close-out:YES. Phase A LOCKED 2026-05-13. All 9
  sub-decisions (D-14-7.1 through D-14-7.9) drafted
  in a single session covering the external-review
  Finding #2 surface (`#[naked]` rejection in v1). **D-14-7.1** (load-
  bearing -- rejection decision + diagnostic shape:
  reject at Item translation; diagnostic names
  `#[naked]` + recommends phase-2 stage-17
  `#[rseq::interrupt(...)]` deferral path).
  **D-14-7.2** (recognition API:
  `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
  during Phase 2 Translation+Synthesis per
  stage-12 D-12-5). **D-14-7.3** (rejection-table
  placement: new "Item translation" phase row at
  §15.1, extending §15.1's header note to
  acknowledge a fourth rejection category; sibling
  to stage-9 D-9-9 row 8 at §23.6.1).
  **D-14-7.4 + D-14-7.5** (`Item.is_naked` field +
  `instance_is_naked` helper deletions at §8.0 +
  §3.5.2; 14-helper inventory downshifts to 13).
  **D-14-7.6** (pretty-printer per-flag table edits
  at §5.3 + §5.6 row-order recap + §5.1 atomic-
  shape example + §5.2 default-suppression
  narrative). **D-14-7.7** (specimen alignment at
  §9.9: Specimen 69 + Specimen 70 + Specimen 75
  input shapes drop `is_naked`; Specimen 70 output
  drops `#[naked]` rendered line; cross-finding
  parenthetical referencing D-14-9 / Finding #4
  added as hygiene -- since removed by D-14-9
  closing the cross-finding).
  **D-14-7.8** (§3.6.6 paragraph 3 finalisation
  from D-14-7-territory pre-lock wording to
  Stage-14-D-14-7 locked-state wording).
  **D-14-7.9** (cross-stage friction outcome --
  three REOPENs at merger close: D-12-1.3 +
  D-9-9 + D-13-4.3 with the documented
  annotations per `expectations.md` Finding #2).
  Step 4 rustc evidence pass landed at
  `prior-art/rustc-naked.md` (6 Q-tags Q1-Q6) +
  `rustc-references.md` "`#[naked]` handling"
  section (5 file:line landmarks). Spec
  contributions: `spec-snapshot.md` §3.5.2 +
  §3.6.6 + §8.0 + §15.1 edits;
  `pretty-printer-snapshot.md` §5.1 + §5.2 + §5.3
  + §5.6 + §9.9 edits. **Cross-stage friction**:
  three new REOPENs from D-14-7 (D-12-1.3, D-9-9,
  D-13-4.3) joining the pending merger-time flips
  from prior clusters (D-9-10 from D-14-2.5,
  D-13-6 from D-14-4.9). **Codex Phase B trajectory
  closed at R6 APPROVE / Lockability:yes / Close-
  out:YES (2026-05-13)**: 6-round trajectory R1
  MINOR (3 findings) -> R2-R5 each MINOR (1 finding
  per round; recap-text-duplication cascade
  mirroring D-14-6) -> R6 APPROVE 0 findings.
  **Total: 7 findings closed across R1-R5
  (0B+7M+0N)**. All 9 D-14-7 sub-decisions remain
  LOCKED at Phase A unchanged. Cluster crossed
  Lockability:yes at R1 (no BLOCK findings;
  architectural picture stable across the entire
  trajectory). The R5 cascade-breaking fix (bumping
  the four lagging surfaces to the full R5
  trajectory at once including R5 itself) ended
  the recap-text-duplication cascade in one pass;
  R6 confirmed APPROVE. **D-14-7 closes the first
  surgical-fix cluster of stage 14** (external-
  review Finding #2).

- **D-14-8 cluster** -- LOCKED 2026-05-13 at
  per-stage Phase B R2 APPROVE / Lockability:yes /
  Close-out:YES. 2-round Phase B trajectory R1
  MINOR 2 findings (0B+2M+0N; tracker line-number
  wording + placeholder D-9-? in audit cross-stage
  friction) -> R2 APPROVE 0 findings. Pure spec
  hygiene cluster landing external-review Finding
  #3 (the formerly stale "stage 13 (codegen)"
  forward-pointer at `spec-snapshot.md` §9.2
  paragraph 3; current retarget text begins at
  line 2452 after the Phase A edit). Single
  applied edit:
  the closing clause of the
  `Struct / Enum / Union` `emission_name`
  bridge-construction-rule bullet is replaced with
  the Finding #3 retarget wording committing
  phase-1 to determinism + uniqueness only and
  forwarding the full normative C-identifier
  construction algorithm (escaping for C
  keywords / non-ASCII / leading digits / double
  underscores, hash-suffix collision-resistance
  scheme, length cap and maximum-name fallback) to
  the C-backend spec's responsibility as out of
  phase-1 scope. **No sub-decisions allocated** --
  the directive is fully prescribed by
  `expectations.md` Finding #3 (verbatim
  replacement text); there is no design choice
  under-determined by the directive. **No D-tag
  flip** -- the §9.2 paragraph this cluster edits
  was authored by stage 9 (Instance schema) as a
  forward pointer; the forward pointer was
  superseded by stage 13's spec-split (D-13-9
  reframing stage 13 as the pretty-printer rather
  than codegen). The retarget is hygiene against
  the supersession, not a revision of any
  load-bearing decision -- stage 9's
  `emission_name` field shape, identity rule, and
  uniqueness guarantee are all unchanged. **Spec
  contribution**: `spec-snapshot.md` §9.2
  paragraph 3 (Struct / Enum / Union
  `emission_name` bullet, closing clause). **No
  cross-stage friction**. **Codex Phase B closed at
  R2 APPROVE / Lockability:yes / Close-out:YES
  2026-05-13** (2-round trajectory: R1 MINOR 2
  findings on tracker wording + placeholder D-tag in
  audit; R2 APPROVE 0 findings). See
  `review-log.md` D-14-8 cluster section for the
  round-by-round trail.

- **D-14-9 cluster** -- LOCKED 2026-05-13 at
  per-stage Phase B R2 APPROVE / Lockability:yes /
  Close-out:YES. 2-round Phase B trajectory R1 MINOR
  2 findings (0B+1M+1N; stale "three clusters"
  preamble wording + audit overstatement of how
  Specimen 70b differs from Specimen 73) -> R2
  APPROVE 0 findings.
  Pure pretty-printer-spec hygiene cluster landing
  external-review Finding #4: the Specimen 70 split
  at `pretty-printer-snapshot.md` §9.9. Specimen 70
  previously carried
  `kind: ItemKind::Function, ...,
  opaque_handle_eligible: true` and rendered
  `#[opaque_handle]` above `unsafe fn bsp::reset`,
  but `spec-snapshot.md` §8.0 invariant 19 restricts
  `opaque_handle_eligible: true` to
  `ItemKind::Struct` (non-generic, `repr(C)`, ZST);
  the specimen exercised a state the bridge will
  never produce. Two applied edits: (i) Specimen 70
  input shape downshifts `opaque_handle_eligible:
  true` to `opaque_handle_eligible: false` and the
  output drops the `#[opaque_handle]` line, leaving
  a clean Function attribute-stress specimen
  (no_mangle + inline(always) + link_section +
  unsafe); (ii) new Specimen 70b carrying
  `kind: ItemKind::Struct,
  def_path: <pl011::Handle>,
  opaque_handle_eligible: true`
  renders `#[opaque_handle]` above
  `struct pl011::Handle`, preserving coverage of the
  bridge-outcome `#[opaque_handle]` rendering on the
  only `ItemKind` that can carry it per §8.0
  invariant 19. The D-14-7 cross-finding
  parenthetical that forward-pointed to D-14-9 is
  also removed since the fix has now landed. **No
  sub-decisions allocated** -- the directive is
  fully prescribed by `expectations.md` Finding #4.
  **No D-tag flip on D-13-7** -- D-13-7's decision
  text (specimen-coverage policy, per-arm coverage
  rubric) is not revised; only the specimen content
  is. **Spec contribution**:
  `pretty-printer-snapshot.md` §9.9 Specimen 70 +
  Specimen 70b (specimen count drifts ~75 -> ~76 via
  the 70b suffix; gap-free 70 -> 70b -> 71 sequence
  acceptable per D-13-7.2's policy-bucket-exception
  spirit). **No cross-stage friction**. **Codex
  Phase B closed at R2 APPROVE / Lockability:yes /
  Close-out:YES 2026-05-13** (2-round trajectory:
  R1 MINOR 2 findings on preamble wording + audit
  overstatement; R2 APPROVE 0 findings). See
  `review-log.md` D-14-9 cluster section for the
  round-by-round trail.

- **`core::ffi::c_void` rejection enforcement
  cluster** -- LOCKED 2026-05-13 at per-stage Phase
  B R3 APPROVE / Lockability:yes / Close-out:YES.
  3-round Phase B trajectory R1 MINOR 3 findings
  (0B+2M+1N; Coroutine-family double-classification
  in the new §15.1 enforcement paragraph + stale
  line-number citations at six audit + tracker
  sites after the spec edit pushed lines down +
  dangling "Phase B." fragment at the audit
  closure check) -> R2 MINOR 1 finding (0B+1M+0N;
  missed stale line number at `memory.md` -- same
  class as R1-#2 but at a surface not touched in
  the R1 fix sweep) -> R3 APPROVE 0 findings.
  Total: **4 findings closed across R1-R2
  (0B+3M+1N; 3 rounds)**. Pure spec
  hygiene cluster landing the enforcement narrative
  at `spec-snapshot.md` §15.1. Verified the c_void
  row already exists in the §15.1 rejection table
  (`core::ffi::c_void` | type translation |
  recognised via `tcx.is_lang_item(LangItem::CVoid)`;
  opaque-struct pattern recommended) and the §15.0
  Special-ADT dispatch-ordering rule already gates
  the rejection algorithmically (the special-ADT
  check runs unconditionally before generic
  struct/enum/union translation per the normative
  ordering rule). One applied edit: a new
  "**Enforcement model**" paragraph + an
  "**Opaque-struct pattern**" paragraph added to
  the §15.1 introduction (after the four-category
  paragraph, before the table) explicitly naming
  which §15.1 rows the §15.0 gate enforces and
  defining the opaque-struct pattern inline
  (`#[repr(C)]` ZST struct with a single private
  zero-length-array field; admitted via the
  ordinary `Ty::Struct` path; lowers to
  incomplete-struct pointer at the C-ABI boundary
  per §14.5 + §8.0 invariant 19). The new
  paragraphs forward-reference Appendix A.1's
  `*mut c_void` -> `*mut OpaqueCtx` rewrite walked
  end-to-end. **No D-tag allocated** (stage-3 D16
  already specifies the rejection at the source-of-
  truth layer; stage-3 D1.4 recommends the opaque-
  struct pattern; D-14 enforcement narrative is
  pure spec hygiene rather than a new decision).
  **No D-tag flip** -- stage-3 D16 and D1.4 are
  unchanged. **No new sub-decisions**. **Spec
  contribution**: `spec-snapshot.md` §15.1
  introduction (~28 new lines covering the
  Enforcement-model + Opaque-struct-pattern
  paragraphs); no other surfaces touched
  (Appendix A.1 already complete; §15.0 ordering
  rule already normative; §15.1 c_void row already
  present). **No cross-stage friction**. **Codex
  Phase B closed at R3 APPROVE / Lockability:yes /
  Close-out:YES 2026-05-13** (3-round trajectory:
  R1 MINOR 3 findings on Coroutine double-
  classification + stale line numbers + dangling
  "Phase B." fragment; R2 MINOR 1 finding on a
  missed stale line number at `memory.md`; R3
  APPROVE 0 findings). See `review-log.md` cluster
  section for the round-by-round trail.

Remaining stage-14 work after this cluster locks:
per-stage cross-cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
(anticipated 2-4 rounds), then hand-off to merger
per `../README.md` §5.5 7-step contract.

---

### D-14-10 -- Generic-C backend codegen carve-out (companion file) [LOCKED at R3 MINOR / 2026-05-13]

**Question (mid-cluster surfacing; user-prompted,
2026-05-13).** External audit of stage-14/spec-snapshot.md
(run after D-14-2 LOCK, before D-14-4 cluster opening)
flagged ~600 lines of generic-C backend codegen content
embedded in the IR spec: §26.9 "Backend lowering contract"
(~190 lines; printf binding, per-`Ty` C specifier table,
`&str` NUL-termination contract, `max_str_args_per_call`);
Appendix A worked C-lowering examples (~400 lines covering
pointers, fat-pointers, niche enums, UnsafeCell, packed
structs, `&dyn`, closures, repr enums); §25.10 Body-to-C
function lowering; §19.6 + §20.4 C-semantics lowering
sketches; §22.7.3 C-emission shapes; §10 Ty "C-side
intent" column; §17.1 fat-pointer C struct table;
§17.2.1 trait-object "void *" prescription. The user
question: should this lift to a dedicated
`generic-c-backend.md` companion file mirroring D-13-9's
`pretty-printer.md` precedent?

**Options considered.**

I1 (scope of separation):
- (a) Full backend codegen lifts to `generic-c-backend.md`
  (per-`Ty` C lowering, fat-pointer C structs, per-PlaceElem
  /Operand C emission, intrinsic C-emission, Body-to-C,
  diagnostic-logging backend contract, worked examples).
  spec.md sections become pointers / shed columns.
- (b) Diagnostic-logging contract only (just §26.9) lifts.
  Half-pregnant; rest of backend material stays mixed.
- (c) Backend content stays in spec.md but reorganises into a
  dedicated "Backend" appendix. Postpones the artifact-split
  decision; spec.md remains heavyweight.

I2 (Linux content disposition):
- (a) Linux rows (§26.9 `pr_*`/`dev_*`, A.8 MMIO, A.9 atomic)
  drop wholesale; re-derive in phase-2 stages 15/16/17;
  inbound expectations filed.
- (b) Linux rows park in a `linux-backend-snapshot.md`
  placeholder at stage 14. Violates §5.9 rule 4.
- (c) Linux rows stay in stage-14 spec.md as informational.
  Cross-target leakage; defeats the carve-out.

I3 (cross-references back to spec.md):
- (a) Drop lifted material from spec.md; pointers replace it.
  §10 Ty "C-side intent" column drops; IR-side abstract
  facts (FnDef ZST, Closure identity-view, empty Tuple =
  unit) preserved as prose. §17.1 fat-pointer table flips
  from C-type column to abstract "data-slot mutability"
  column; §17.2.1 reframes to IR-side mutability-erasure
  rule (drops the "void *" / "C-level const" prescription).
- (b) Keep rows + summaries in spec.md; full content in
  `generic-c-backend.md`. Duplicates content.

I4 (methodology rule extension): README §5.9 gains rule 7
(backend-codegen artifact pattern); §5.5 step 5 / 6 / 7
amendments; §4 folder layout addition. Pattern generalises
so phase-2 backends each get their own `<target>-backend.md`.

I5 (`generic-c-backend.md` initial structure): 9-section
structure proposed (Mission / per-`Ty` / fat pointers /
PlaceElem / Operand / Intrinsic / Body-to-C / Diagnostic
logging / Worked examples).

I6 (D-14-2.9 status interaction):
- (a) D-14-2.9 stays LOCKED. D-14-10 changes the
  canonical-spec **location** (generic-C content moves from
  §26.9 to generic-c-backend.md §8); the substantive content
  of D-14-2.9 (Pipeline A uniformity rule, generic-C printf
  binding, `&str` NUL-term, max_str_args_per_call) survives
  intact. Linux rows that drop were already framed as
  forward-pointers to phase-2 stage 15 in D-14-2.9 text.
- (b) D-14-2.9 flips to REOPENED. Any content modification
  to an earlier decision's spec text reopens it.

**Decision.** **I1 (a) + I2 (a) + I3 (a) + I4
(methodology rule extension) + I5 (9-section structure)
+ I6 (a) -- D-14-2.9 stays LOCKED.**

Concrete commitments:
- Full backend codegen lifts to `generic-c-backend.md`.
  spec.md sections become pointers; IR-side facts preserved
  in prose under §10; §17.1 / §17.2.1 reframe IR-side.
- Linux content drops; inbound expectations filed on
  `stage-15/expectations.md`, `stage-16/expectations.md`,
  `stage-17/expectations.md`.
- README §5.9 gains rule 7; §5.5 steps 5/6/7 amend to route
  backend content into backend artifacts and run post-merge
  Codex review on each; §4 folder layout adds
  `generic-c-backend.md` plus the per-stage snapshot (plus
  the previously-omitted `pretty-printer-snapshot.md`).
- `generic-c-backend.md` initial 9-section structure: (1)
  Mission and scope; (2) Per-`Ty` C lowering table; (3)
  Reference and pointer C lowering (3.1 thin, 3.2 fat-pointer
  C structs, 3.3 trait-object `data` slot rendering, 3.4 C
  boundary rejection); (4) `PlaceElem` C lvalue forms; (5)
  `Operand` C lowering; (6) Intrinsic C-emission; (7)
  Body-to-C function lowering; (8) Diagnostic-logging
  backend contract (8.1-8.5); (9) Worked C-lowering examples
  (9 entries).
- Stage-14 working surface:
  `stage-14/generic-c-backend-snapshot.md`. The merger
  contract (§5.5 step 5) integrates this snapshot into
  top-level `generic-c-backend.md` at stage-14 close,
  alongside the trimmed `spec-snapshot.md`.
- D-14-2.9 stays LOCKED.

**Reasoning.**
- I1 (a) cleanest separation. Backend codegen is consumer-
  side concern; factors cleanly from IR-structure
  (producer-side). Mirrors the D-13-9 precedent for
  `pretty-printer.md`. Future phase-2 backends each get
  their own artifact under the same rule.
- I2 (a) honors §5.9 rule 4 (no forward-looking
  placeholders). Linux content was forward-looking from
  stage-14's perspective; phase-2 stage 15 owns Linux
  backend per §2 dual-backend rule. Re-derivation against
  pinned MIR + kernel-API evidence per §2 "re-derive from
  evidence" produces a sharper Linux spec than carrying
  stage-14 placeholders forward.
- I3 (a) avoids content duplication. IR-side abstract facts
  preserved as prose; reframed §17.2.1 expresses the
  IR-side rule (mutability discriminant erased at the dyn
  boundary because `contains_unsafe_cell` is undecidable on
  `Dynamic{..}`) without the backend-specific "C-level
  const" / "void *" framing.
- I4 generalises beyond this carve-out: phase-2 backend
  stages inherit the same pattern. The artifact-per-target
  approach matches §2's "backends are separate codegen
  paths" framing.
- I5's 9-section structure mirrors spec.md's organization
  (per-`Ty` -> references/pointers -> place/operand ->
  intrinsics -> body) so consumers can navigate the two
  artifacts in parallel.
- I6 (a) matches the D-13-9 precedent: rendering-related
  earlier D-tags whose content moved to `pretty-printer.md`
  were not flipped to REOPENED. Relocation is not a
  decision-content revision. The Linux content drop is a
  scope clarification (the forward-pointer was always to
  phase-2 stage 15), not a revocation of D-14-2.9's
  normative generic-C contract.

**Methodology rules invoked.**
- §2 dual-backend rule (backends are separate codegen
  paths; each gets its own merger artifact).
- §2 compositional-behavior rule (the IR's compositional
  shape determines backend output; no parallel flag-style
  backend-codegen markers in spec.md).
- §2 re-derive-from-evidence rule (Linux content
  re-derives in phase-2 against pinned evidence).
- §5.9 rule 4 (no forward-looking placeholders).
- §5.9 rule 6 (pretty-printer carve-out) extended to rule 7
  (backend-codegen carve-out); same shape.

**Prior-art evidence.**
- D-13-9 `pretty-printer.md` carve-out: LOCKED at R3 MINOR
  / 2026-05-04. Six sub-decisions cover scope (full lift),
  cross-references (drop deferred rows + add pointers),
  methodology rule extension (§5.9 rule 6 + §5.5 + §4),
  initial 10-section structure, working surface
  (`stage-13/pretty-printer-snapshot.md`), and
  consumed-decision absorption (D-13-8). D-14-10 mirrors
  all six structural decisions for the backend-codegen
  carve-out, with one addition (I6: D-14-2.9 LOCKED
  preservation).

**Spec impact.**
- `spec-snapshot.md`: §10 Ty table loses "C-side intent"
  column (IR-side facts preserved in prose beneath the
  table); §17.1 fat-pointer table flips from C-type
  column to abstract data-slot-mutability column; §17.2.1
  reframes IR-side (mutability-discriminant-erased rule
  replaces "void *" / "C-level const" framing); §19.6,
  §20.4, §22.7.3, §25.10 receive one-paragraph pointers;
  §26.9 collapses to a one-paragraph pointer plus the
  IR-side Pipeline A uniformity rule. Appendix A trimmed
  from 12 entries to 1 (renamed "Bridge-rejection
  examples"; A.12 c_void rejection survives with the
  C-lowering tail removed). Net: ~600 lines trimmed;
  IR-side invariants preserved.
- `generic-c-backend-snapshot.md` (new): 9-section file
  carrying the lifted content.
- README: §4 folder layout updated; §5.5 merger contract
  steps 5/6/7 amended; §5.9 preamble updated to "Seven
  rules"; rule 7 added.
- `stage-15/expectations.md`, `stage-16/expectations.md`,
  `stage-17/expectations.md`: each receives a "From stage
  14 (filed 2026-05-13, per D-14-10)" block.

**Sub-decisions.**

- **D-14-10.1** -- Scope: full backend codegen lifts
  (I1 (a)).
- **D-14-10.2** -- Linux content disposition: drop
  wholesale; phase-2 re-derivation (I2 (a)).
- **D-14-10.3** -- Cross-references: spec.md sections
  shed backend material and receive pointers; §10
  C-side intent column drops with IR-side facts preserved
  in prose; §17.1 / §17.2.1 reframe IR-side (I3 (a)).
- **D-14-10.4** -- Methodology rule extension: README
  §5.9 rule 7 + §5.5 step 5/6/7 + §4 folder layout (I4).
- **D-14-10.5** -- `generic-c-backend.md` initial
  9-section structure (I5).
- **D-14-10.6** -- Working surface:
  `stage-14/generic-c-backend-snapshot.md`.
- **D-14-10.7** -- D-14-2.9 stays LOCKED; D-14-10 is
  relocation-only at the decision level (I6 (a)).

**Codex Phase B trajectory.**

| Round | Verdict | Pass | Findings | Notes |
| ----- | ------- | ---- | -------- | ----- |
| R1    | BLOCK   | Pass B (spec.md) | 2 BLOCK + 2 MINOR + 1 NIT | 5 cited leak locations + §17 numbering + §10 unit/ZST + section-anchor accuracy. |
| R1    | MINOR   | Pass A (backend snapshot) | 1 BLOCK + 2 MINOR + 1 NIT | §7 fragment narrowed; §17.2.1 ref retargeted to §17.2.3; UintTy/IntTy renamed to UintKind/IntKind; §9 Linux-mention parenthetical dropped. |
| R2    | BLOCK   | Pass B (spec.md, exhaustive) | ~50 leak citations across 15+ sections | Scope expansion surfaced wider light-coupling. User-supplied threshold ("lift C generation strategy; keep light hints / clarifying comments") narrowed the iteration target. |
| R3    | MINOR   | Pass B (spec.md, relaxed threshold) | 6 strategy-level escapes | Opaque-handle declaration form; `Const::Aggregate` use-site rendering; Bool/Char niche storage lvalue; `__attribute__((noreturn))` at three call sites; `void *` example in §17.2.3 pointer; D8 narrative inline cap value + §26.7 trailing-newline pointer + §26.10 deferrals pointer. |

R3 MINOR findings addressed per the user threshold; locked
on 2026-05-13. Status flip per merger contract (§5.5 step 3)
at stage-14 close: `[LOCKED]`.

---

### D-14-6 -- Top-level `render(&self) -> String` composer renderer [LOCKED -- Phase A 6/6 sub-decisions (2026-05-13); Phase B closed at R6 APPROVE / Lockability:yes / Close-out:YES (2026-05-13)]

**Question.** Stage-13 D-13-4.6 locked the
building-block-vs-composer architectural division and
identified three composers as stage-14 territory:
`render_const` (locked at D-14-4 Phase B R4 APPROVE
2026-05-13), `render_instance` (locked at D-14-5 Phase B
R4 APPROVE 2026-05-13), and the top-level
`render(&self) -> String` (this cluster). D-14-6 advanced
to the third and final composer: a single zero-argument
method on `PrettyPrinter<'a>` that iterates
`model.instances` (Q2 codegen-keyed F4-2 sort per
`spec-snapshot.md` §16.3) and composes per-Instance
renderings via the now-locked
`render_instance(InstanceId) -> String` from D-14-5.

The pretty-printer-side spec landed at
`pretty-printer-snapshot.md` §10.3 (§10.3.1-§10.3.7
covering signature, iteration order, separator,
envelope, empty case, composition outline, worked
examples). The §1.3 composer listing entry for
`render(&self)` and §8 ownership table row for
`render(&self)` were finalised to locked-state wording
at D-14-6.1 / D-14-6.2 lock.

D-14-6 is **the final composer-renderer cluster** in
stage 14's phase-1 closure surface. Closure leaves stage
14 with one composer trio (`render_const` /
`render_instance` / `render`) fully specified and
unblocks the remaining stage-14 work
(D-14-7 / D-14-8 / D-14-9 external-review Findings;
`core::ffi::c_void` rejection enforcement).

**Evidence basis.** Step 4 rustc evidence pass landed at
`prior-art/rustc-render.md` (2026-05-13) surveying 6
load-bearing questions (Q1-Q6) across rustc's
`rustc_middle/src/mir/pretty.rs` top-level batch-render
machinery (`write_mir_pretty` at lines 317-369), the
iteration-order discipline
(`tcx.mir_keys(())` via
`rustc_mir_transform/src/lib.rs:328-359`), the codegen-
tier ordering supporting evidence
(`MonoItem::items_in_deterministic_order` at
`rustc_middle/src/mir/mono.rs:508-555`), and the
stable-IR analog
(`rustc_public` -- **no top-level batch renderer
exists**; per-Instance `Instance::emit_mir` only at
`mono.rs:178-180`). The evidence pass grounded 6
sub-decisions worth of allocation surface; see the
per-sub-decision rationale in `prior-art/rustc-render.md`
"Summary" section.

The evidence surfaces three observations that load-bear
on D-14-6:

1. **F4-2 already provides canonical iteration order.**
   Rustc's `mir_keys` walks HIR body-owners in
   source-declaration order (a content-INdependent
   traversal). RegseqModel's `model.instances` is **already
   F4-2 sorted** at IR-construction layer per `spec-snapshot.md`
   §16.3 (`RegseqInstanceKey = (regseq_kind_rank, item_id,
   generic_args_key, fn_shim_disambig)`), which is strictly
   more deterministic than rustc's HIR-walk order AND
   already groups by InstanceKind rank (Functions before
   Statics before Consts before Structs/Enums/Unions per
   §9.3). The IndexVec stores Instances in canonical order;
   the renderer does NOT need to re-sort.
2. **Stable-IR has no top-level batch renderer.**
   `rustc_public/src/mir/pretty.rs` provides
   `function_body` (per-body envelope) but no analog of
   `write_mir_pretty`; stable-IR consumers write their
   own iteration loop. **Confirms RegseqModel's
   `render(&self)` is genuinely RegseqModel-new at the
   stable-public-IR tier**, mirroring D-14-5's ADT-variant
   rendering finding (no rustc precedent for nominal-type
   rendering at the Instance level either).
3. **Rustc emits a 3-line WARNING + HINT comment file
   header** at `pretty.rs:324-326` with an explicit
   "format is unstable, do not parse" message.
   RegseqModel's `pretty-printer.md` is **explicitly
   designed as a snapshot artifact for `insta` snapshot
   tests + diagnostic output** per stage-1 Q5 / D-1-S2;
   the output IS the spec, and format stability is
   enforced by the snapshot test suite. The
   rustc-warning's premise (consumers might parse MIR
   pretty-print) does not apply.

Additional cross-cutting surfaces:

- **Per-Instance separator.** Rustc emits one blank line
  between items (no leading blank, no trailing blank);
  the first-flag mechanism at `pretty.rs:328-335` is
  load-bearing. D-14-6.3 mirrors this verbatim as
  evidence-preponderance default.
- **Empty-model case.** With no file-level header
  (D-14-6.4 recommendation per Q4), the natural
  empty-`model.instances` output is the empty string.
  No header, no items, no separator.

**Sub-decision allocation.** Six sub-decisions allocated:

- **D-14-6.1 -- API signature.** Confirm + lock
  `render(&self) -> String`. Single method on
  `PrettyPrinter<'a>`; no parameters (no
  `single: Option<InstanceId>` filter -- single-Instance
  dispatch is already covered by
  `render_instance(InstanceId)` from D-14-5); String
  return per stage-13 D-13-1 method-shape convention. The
  signature is already pinned by stage-13 D-13-4.6 +
  D-14-5's §1.3 / §8 ownership-table finalisation;
  D-14-6.1's role is to confirm + lock against the
  cluster's per-arm composition contract, not to revisit
  the architectural choice. **Load-bearing**: gates the
  per-arm composition outlines (D-14-6.2 onward).

- **D-14-6.2 -- Iteration order over `model.instances`.**
  Three candidate forms:
  - (A) Walk `model.instances.iter_enumerated()` verbatim
    in InstanceId order (F4-2 sorted by construction per
    §16.3). No re-sort.
  - (B) Re-sort by some other key at render time (e.g.,
    explicit `regseq_instance_kind_rank` grouping; lexical
    DefPath; span-based source order).
  - (C) Mixed -- F4-2 order BUT with explicit InstanceKind
    rank-group boundaries (e.g., emit `// === Functions
    ===` header between rank groups).
  Per Q2 evidence preponderance: (A). The F4-2 sort
  already groups by InstanceKind rank first; (B) would
  manually reconstruct what F4-2 provides; (C) adds
  diagnostic noise without architectural benefit (rank
  groups are visually distinct via the first rendered
  line of each `render_instance` output -- the attribute
  block when non-empty per §10.2.7, otherwise the
  kind-keyword line of
  each Instance's render). **Load-bearing**: the largest
  cluster decision; gates the iteration-loop outline.

- **D-14-6.3 -- Per-Instance separator.** Three candidate
  forms:
  - (A) Top-level composer emits `"\n\n"` between
    consecutive `render_instance(id)` outputs (which
    themselves end at `}` or `;` with no trailing
    newline); no leading or trailing separator
    (first-flag pattern adapted from rustc).
  - (B) Header-comment + blank line (e.g.,
    `// === Function: foo::bar ===\n` before each body).
  - (C) No separator (concatenation).
  Per Q3 evidence preponderance: (A). (B) duplicates
  each Instance's self-identifying prefix (attribute
  block when non-empty, otherwise the kind-keyword line);
  (C) places the next Instance's first byte directly
  after the prior Instance's closing byte on the same
  line.

- **D-14-6.4 -- File-level output envelope.** Four
  candidate forms:
  - (A) No file-level header / trailer. Output starts
    with the first rendered byte of the first
    `render_instance` output (attribute block when
    non-empty, otherwise the kind-keyword line); ends
    at the last rendered byte (`}` or `;`) with no
    trailing newline injected.
  - (B) Rustc-mirrored 3-line WARNING + HINT header.
  - (C) RegseqModel-specific metadata header (crate name +
    toolchain + target + emission timestamp).
  - (D) Minimal version-tag header (`// RegseqModel
    pretty-print v1`).
  Per Q4 evidence: (A). The rustc-warning's premise does
  not apply (RegseqModel pretty-print IS the spec, format
  stability enforced by `insta`); (C) duplicates
  `model.build_info` which already holds the same data;
  (D) is operationally redundant against the snapshot
  suite. Cleanest match for RegseqModel's
  consumer-managed-framing model.

- **D-14-6.5 -- Empty-model case.** Two candidate forms:
  - (A) Empty string `""`. Falls out naturally from
    (D-14-6.4 A) + empty `model.instances`.
  - (B) Placeholder comment (e.g., `// (empty model)`).
  Per Q5 evidence: (A). Empty `model.instances` arises
  legitimately in foundation-crate test fixtures
  exercising pure-Item surfaces, future-stage validator
  baselines, and rejection-path tests. A placeholder
  adds confusion without operational value; consumers
  reading an empty snapshot already know they're looking
  at an empty model.

- **D-14-6.6 -- Cross-stage friction outcome.** No
  anticipated REOPENs of stage-13 D-tags from current
  evidence. Working assumption: the §10.3 spec is a
  pure consumer-side composer over D-14-5's
  `render_instance`; no IR-structural changes anticipated;
  no `render_instance` signature extension required (the
  composer consumes the single-argument
  `render_instance(InstanceId)` form locked at D-14-5.1);
  no §10.2 per-variant subsection edits anticipated. If a
  REOPEN does surface during drafting (e.g., a per-arm
  composition outline requires `render_instance` to
  produce a slightly different shape at the top-level
  callsite vs the per-Instance callsite), register here
  with the friction-flip annotation pattern (mirrors
  D-14-4.9's REOPEN of D-13-6). Pending merger-time flips
  from prior clusters unchanged: D-9-10 (from D-14-2.5),
  D-13-6 (from D-14-4.9), plus D-12-1.3 / D-9-9 /
  D-13-4.3 (from D-14-7 when locked).

**Spec impact preview (Phase A drafting target).**

- `pretty-printer-snapshot.md` §10.3 -- replaced with full
  per-arm composer spec covering the iteration outline +
  per-Instance separator rule + file-level envelope rule
  + empty-model case rule + one worked composition example
  showing the assembled output shape.
- `pretty-printer-snapshot.md` §1.3 composer listing
  entry for `render(&self)` -- finalised from the
  D-14-5-era forward-pointer wording to the locked-state
  composition description.
- `pretty-printer-snapshot.md` §8 ownership table row for
  `render(&self)` -- Description column finalised to
  reflect the locked iteration + composition outline.
- `pretty-printer-snapshot.md` §11 cross-references -- no
  edits anticipated (no `spec.md` section is consumer-
  rendered specifically by `render(&self)` beyond what
  `render_instance` already covers).
- `pretty-printer-snapshot.md` §9.12 new specimens
  subsection -- 3-6 specimens per D-13-7 convention
  (single-Instance / multi-Instance / empty / mixed-
  InstanceKind). Specimen numbering continues from §9.11
  (Specimen 128 was D-14-5's last; D-14-6 begins at
  Specimen 129).
- `spec-snapshot.md` -- no edits anticipated. The
  iteration order rule lives at `spec-snapshot.md` §16.3
  unchanged; D-14-6.2 consumes the rule without revising
  it.

**Methodology rules invoked at allocation** (cluster-level):

- *Compositional behavior beats special-case flags.*
  `render(&self)` composes existing renderers
  (`render_instance` per Instance, plus the loop
  scaffolding) rather than carrying per-Instance flag-
  style markers. The F4-2 sort + InstanceKind rank
  grouping in §16.3 provide the iteration order without
  a parallel render-time sort.
- *C semantics with information preservation.* The render
  consumes `model.instances` (which already holds the
  resolved F4-2 ordering) rather than re-sorting from the
  HIR-walk source order rustc uses. Where rustc's
  ordering depends on input-order (HIR walk), RegseqModel
  uses the content-keyed sort that has already done the
  ordering work; store the result, not the input.
- *Code does not reference discussion-folder paths.* The
  §10.3 spec text references stable architectural tags
  (Q2's `regseq_instance_kind_rank` codename; §16.3 F4-2
  sort key); no `D-13-*` or `D-14-*` inline citations in
  user-visible spec text per the README §2 rule.
- *No defensive fallbacks.* Empty-model case (D-14-6.5)
  is an empty string, not a sentinel placeholder. The
  pinned toolchain produces F4-2-deterministic InstanceId
  ordering; render-time re-sort is not added as a
  "what if F4-2 produces a bad order" hedge.
- *Spec is mutable.* If Phase B Codex review surfaces a
  stage-13 REOPEN-shaped finding (e.g., `render_instance`
  needs a top-level-call-context flag), D-14-6.6 entry
  updates per the friction-flip pattern. v1 records the
  friction outcome as of architectural closure.

**Prior-art evidence basis.**
- `prior-art/rustc-render.md` (this cluster's Step 4
  evidence pass) -- 6 Q-tags surveying rustc's top-level
  batch-render machinery, iteration-order discipline,
  codegen-tier supporting evidence, and stable-IR analog
  absence.
- `prior-art/rustc-instance.md` (D-14-5's Step 4 evidence
  pass) -- Q1 already covered the upstream dispatch sites
  (`write_mir_pretty` / `MonoItem::define` /
  `Instance::emit_mir`); D-14-6 consumes the cross-
  references without re-deriving.
- `prior-art/rustc-const.md` (D-14-4's Step 4 evidence
  pass) -- not directly consulted; D-14-6's surface is
  iteration + envelope, not per-Const rendering.

**Phase A drafting plan.** Seven-step lock sequence:

1. Open D-14-6 entry in `stage-14/discussions.md` (this
   entry).
2. Lock D-14-6.1 (API signature; confirm + lock the
   pinned form per stage-13 + D-14-5).
3. Lock D-14-6.2 (iteration order; recommended (A) walk
   verbatim per F4-2 sort).
4. Lock D-14-6.3 + D-14-6.4 + D-14-6.5 together as the
   small-surgical-add trio (single-blank-separator +
   no-file-header + empty-string-empty-case).
5. Lock D-14-6.6 (friction outcome; expected zero new
   REOPENs).
6. Add 3-6 specimens to `pretty-printer-snapshot.md`
   §9.12 (single-Instance / multi-Instance / empty /
   mixed-InstanceKind).
7. Per-stage Phase B Codex review (anticipated 1-3
   rounds; surface smaller than D-14-4 + D-14-5).

**Phase B Codex review expectations.** 1-3 rounds
anticipated based on the surface size: D-14-4 closed at
R4 (12 sub-decisions + 34 specimens), D-14-5 closed at
R4 (8 sub-decisions + 19 specimens), D-14-6 has 6
sub-decisions + 3-6 specimens with strictly less
content-dependent surface (no per-arm dispatch tables;
no per-variant composition outlines beyond the single
iteration loop). The recap-text-duplication pattern
observed in D-14-4 / D-14-5 will likely surface (§1.3
composer listing + §8 ownership row + §10.3 spec text
each carry the loop description), but the duplication
surface is smaller; 1-2 rounds expected.

#### D-14-6.1 -- API signature: `render(&self) -> String` [LOCKED]

**Decision.** The composer renderer takes no
arguments beyond `&self`:

```rust
impl PrettyPrinter<'_> {
    pub fn render(&self) -> String;
}
```

No `single: Option<InstanceId>` filter parameter; no
write-sink form (`&mut W: io::Write`); no `Option`
return wrapping; no `Result`. The `PrettyPrinter` has
`&model` and iterates `self.model.instances` internally
per D-14-6.2; per-Instance dispatch composes through
`self.render_instance(id)` per D-14-5.1.

**Reasoning.** Three load-bearing reasons drive the
zero-argument form:

1. **Single-Instance dispatch is already covered.**
   D-14-5.1 locked `render_instance(InstanceId) -> String`
   as the per-Instance entry. A `single: Option<InstanceId>`
   filter on `render(&self)` would duplicate that
   surface; a consumer wanting a single Instance's
   rendering calls `render_instance(id)` directly. The
   top-level composer's job is the **batch** render --
   "render every Instance in `model.instances` in
   canonical order with the file-level framing rules
   pinned at D-14-6.3 / D-14-6.4 / D-14-6.5."
2. **Stage-13 D-13-1 method-shape inheritance.** Per
   `pretty-printer-snapshot.md` §2.3 (LOCKED at stage-13
   D-13-1), every renderer is a method on
   `PrettyPrinter<'a>` returning `String`. `render(&self)`
   inherits the convention. The `String`-return shape is
   acceptable for the v1 consumers (snapshot tests +
   diagnostics) per §2.3; a future `&mut String` /
   `&mut W: Write` overload is non-breaking extension.
3. **Pre-pinned upstream.** The signature is already
   pinned in `pretty-printer-snapshot.md` §1.3 composer
   listing (line 116-118) and §8 ownership table
   (line 1143), introduced at D-14-4 / D-14-5 lock as
   forward-pointers. D-14-6.1 confirms + locks against
   the cluster composition contract; it does not
   revisit the architectural choice.

**Rejected alternatives.**

- **(b) Filter form
  `render(&self, only: Option<InstanceId>) -> String`** --
  duplicates `render_instance(InstanceId)`. The single-
  Instance dispatch surface should not split across two
  API entry points.
- **(c) Write-sink form `render<W: io::Write>(&self, w:
  &mut W) -> io::Result<()>`** -- diverges from the
  stage-13 D-13-1 String-return convention; would force
  every other renderer in the family to match for
  consistency. Future extension allowed (D-13-1 §2.3
  explicitly notes the overload door); not chosen at
  v1.
- **(d) Lazy iterator form `render(&self) -> impl
  Iterator<Item = String>` (per-Instance chunks)** --
  defers concatenation to the consumer; reduces peak
  memory but adds API complexity. No v1 consumer
  surfaces the need; `String` return is the natural
  identity for the snapshot-test consumer.

**Methodology rules invoked.**

- *Compositional behavior beats special-case flags.* The
  zero-argument form composes via the per-Instance loop +
  `render_instance(InstanceId)`; the top-level surface
  hides the iteration scaffolding behind a single
  composer call.
- *Spec-13 D-13-1 method-shape inheritance.* The
  String-return convention propagates uniformly across
  the renderer family; `render(&self)` consumes the
  convention without revisiting.
- *No defensive fallbacks.* No `Result` wrapping --
  `render(&self)` cannot fail under the pinned spec's
  invariants. `model.instances` is always valid
  (F4-2-sorted; D-14-5.1 + D-14-5.* per-variant locks
  guarantee `render_instance` produces a valid String
  for every Instance under the closed `InstanceKind`
  enum).
- *Compiler-theory-first.* The single-method top-level
  composer matches the broader pretty-printer pattern
  (one canonical entry per artifact tier; rustc's
  `write_mir_pretty` is the rustc-side analog; modulo
  write-sink vs String-return which is the stage-13
  D-13-1 convention split).

**Spec impact.**

- `pretty-printer-snapshot.md` §10.3 -- intro paragraph
  pins the API signature `render(&self) -> String` and
  the no-argument no-Result shape. Per-arm composition
  outline (iteration + separator + envelope + empty case)
  follows at D-14-6.2 through D-14-6.5 locks.
- `pretty-printer-snapshot.md` §1.3 composer listing
  entry for `render(&self)` -- already exists at line
  116-118 with the locked-state form; D-14-6.1 lock
  confirms (no edit at this sub-decision; final wording
  refinement at D-14-6.2 / D-14-6.5 lock when the
  iteration + empty-case rules pin in).
- `pretty-printer-snapshot.md` §8 ownership table row
  for `render(&self)` -- exists at line 1143 with
  Description column "top-level model dump; iterates
  `model.instances`"; D-14-6.1 lock confirms (no edit).
  Description column finalises at D-14-6.2 / D-14-6.3 /
  D-14-6.4 / D-14-6.5 locks once the composition rules
  pin in.
- `pretty-printer-snapshot.md` §3.3 sigil convention
  table -- no row to add. `render(&self)` produces a
  document-level String, not a per-`<SomeId>` rendering;
  the sigil convention rows are per-ID-kind (`ItemId`,
  `InstanceId`, `TyId`, etc.). The top-level composer
  has no ID input.
- `pretty-printer-snapshot.md` §11 cross-references --
  no edits anticipated. `render(&self)` composes
  per-Instance via `render_instance` (which carries the
  §9.4 InstanceKind cross-reference per D-14-5) plus the
  file-level framing rules (which have no `spec.md`
  structural mirror). The §11 row for §9 already points
  at `pretty-printer.md` §10.2 via `render_instance`;
  D-14-6 inherits the pointer chain transitively.

**Worked example.**

```rust
impl PrettyPrinter<'_> {
    pub fn render(&self) -> String;
}

// Illustrative composition sketch -- per-arm rules
// pinned at D-14-6.2 (iteration) + D-14-6.3 (separator)
// + D-14-6.4 (envelope) + D-14-6.5 (empty case).
fn render(&self) -> String {
    // D-14-6.4 LOCKED: no file-level header. Output
    // starts with first Instance's first line; ends
    // with last Instance's last line.
    let mut buf = String::new();
    let mut first = true;
    // D-14-6.2 LOCKED: walk model.instances in
    // IndexVec/InstanceId order (F4-2 sorted by
    // construction per spec.md §16.3).
    for (id, _inst) in self.model.instances.iter_enumerated() {
        if first {
            first = false;
        } else {
            // D-14-6.3 LOCKED: top-level composer owns the
            // separator bytes; `render_instance` outputs end
            // at `}` or `;` with no trailing `\n`. The
            // two-byte `"\n\n"` produces the single blank
            // line between consecutive Instances.
            buf.push_str("\n\n");
        }
        buf.push_str(&self.render_instance(id));
    }
    // D-14-6.5 LOCKED: empty model.instances -> empty
    // String "" (the loop never runs).
    buf
}
```

The illustrative sketch shows the composition shape
locked by D-14-6.2 through D-14-6.5 in subsequent sub-
decisions; D-14-6.1's role is to confirm + lock the
**outer signature** (zero-argument String-return form).
The per-arm composition rules are placeholders that get
pinned in the subsequent sub-decisions; no implementation
prose -- the spec text describes the rule, not the code.

#### D-14-6.2 -- Iteration order over `model.instances` [LOCKED]

**Decision.** **Option (A) -- walk
`self.model.instances.iter_enumerated()` in
InstanceId order verbatim, with no render-time
re-sort.**

The IndexVec is already in canonical F4-2 sorted order
per `spec-snapshot.md` §16.3 (`RegseqInstanceKey =
(regseq_kind_rank, item_id, generic_args_key,
fn_shim_disambig)`); the renderer consumes that
ordering directly.

The iteration loop has the shape:

```text
for (id, _inst) in self.model.instances.iter_enumerated() {
    // ... render_instance(id) + separator per D-14-6.3
}
```

No render-time sort by InstanceKind rank, lexical
DefPath, span-based source order, or any other key.
No iteration filter beyond what `iter_enumerated()`
naturally provides (all `InstanceId` entries in `0 ..
model.instances.len()`).

**Reasoning.** Three load-bearing reasons drive
(A):

1. **F4-2 sort already groups by InstanceKind rank
   first.** Per `spec-snapshot.md` §9.3
   `regseq_instance_kind_rank`, the F4-2 key
   `(regseq_kind_rank, item_id, generic_args_key,
   fn_shim_disambig)` orders Function (rank 0) before
   Static (rank 1) before Const (rank 2) before
   Struct (rank 3) before Enum (rank 4) before Union
   (rank 5). The natural read order of
   `model.instances.iter_enumerated()` is therefore
   already rank-grouped -- the diagnostic-readability
   benefit that motivates alternative (B) is **already
   provided by F4-2** without a render-time sort.
2. **F4-2 is strictly more deterministic than rustc's
   precedent.** Rustc's `write_mir_pretty` walks
   `tcx.mir_keys(())` -- an HIR-walk source-declaration
   order (per Q2). Two builds of the same source
   produce identical orders, but the order itself is a
   property of the source layout (not the source
   content). RegseqModel's F4-2 is content-keyed: two
   builds satisfying the build-equivalence quad
   (§16.1) produce identical orders **independent of
   source layout** (refactoring a function's
   declaration position does not change F4-2). The
   renderer inheriting F4-2 inherits the strictly
   stronger guarantee.
3. **Compiler-theory: store the result, not the input.**
   The §16.3 sort canonicalises `(InstanceKey)` at
   IR-construction layer. A render-time re-sort would
   re-derive the same order from a subset of the
   sort-key fields (e.g., `regseq_kind_rank` alone),
   discarding the disambiguation work the F4-2 sort
   has already done. Per the README §2
   "C semantics with information preservation" /
   "store the result, not the input" rule: consume the
   F4-2 order directly; do not re-derive a partial form.

**Rejected alternatives.**

- **(B) Render-time re-sort by an alternative key**
  (e.g., explicit `regseq_instance_kind_rank` grouping,
  lexical `def_path_str`, span-based source order,
  `DefPathHash` numeric). All alternative keys are
  either (a) already part of F4-2's lexicographic sort
  (rank, ItemId via DefPath), in which case the re-sort
  is redundant; or (b) **less deterministic** than F4-2
  under the build-equivalence quad (span / source order
  changes under refactors; `DefPathHash` is content-keyed
  but lacks F4-2's rank prefix). No alternative key
  matches F4-2's combination of stability + readability.
- **(C) F4-2 order with explicit InstanceKind rank-group
  separators** (e.g., emit `// === Functions ===\n`
  between rank-0 and rank-1 Instances; analogous for
  each rank boundary). Adds diagnostic noise without
  architectural benefit: each Instance's first-line
  keyword (`fn` / `static` / `static mut` / `const` /
  `struct` / `enum` / `union`) already self-identifies
  its rank group; the rank-group transitions are
  visually obvious without explicit separators. The
  rustc precedent (`write_mir_pretty` at `pretty.rs:317-369`)
  emits no analog separators between body-kinds, and
  the rendering pattern is well-established without
  them. (C) trades signal-to-noise for explicitness in
  a case where the implicit form already conveys the
  information.
- **(D) Custom user-supplied ordering** (e.g.,
  `PrettyPrintOptions::instance_order: Option<Vec<InstanceId>>`).
  Future extension allowed per "Spec is mutable"; not
  v1. The current consumer model (snapshot tests +
  diagnostics) is well-served by the canonical F4-2
  order; consumer-supplied ordering is a
  presentation-tier customisation that does not need
  to live at the spec layer.

**Methodology rules invoked.**

- *C semantics with information preservation.* F4-2
  sort is the resolved canonical ordering; the
  renderer consumes the resolution rather than
  re-deriving from a partial key.
- *Compositional behavior beats special-case flags.*
  Iteration order falls out of the existing F4-2 sort
  + IndexVec scan; no per-Instance render-time flag
  needed to drive ordering.
- *No defensive fallbacks.* No "what if F4-2 produces
  a poor diagnostic order" hedge; the F4-2 rank-
  grouped order is the canonical form, and consumers
  reading the snapshot inherit it directly.
- *Compiler-theory-first.* Sorting at the IR-
  construction layer (cheap, one-time, content-keyed)
  beats sorting at the consumer layer (per-render,
  potentially per-snapshot-test); the upstream sort
  amortises across every consumer.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.3 -- iteration rule
  pinned: "Walks `model.instances.iter_enumerated()` in
  InstanceId order (canonical F4-2 sort per `spec.md`
  §16.3); no render-time re-sort." Cross-reference to
  `spec.md` §16.3 / §9.3 for the rank table.
- `pretty-printer-snapshot.md` §1.3 composer listing
  entry -- the existing "iterates `model.instances`
  (Q2 codegen-keyed)" wording is the locked-state
  form; D-14-6.2 confirms (no edit).
- `pretty-printer-snapshot.md` §8 ownership table row
  for `render(&self)` -- Description column updates
  from "top-level model dump; iterates
  `model.instances`" to "top-level model dump;
  iterates `model.instances.iter_enumerated()` in
  InstanceId order (F4-2 sorted per `spec.md` §16.3);
  per-Instance via `render_instance`".
- `spec-snapshot.md` -- no edits. §16.3 F4-2 sort rule
  is consumed as-is; D-14-6.2 cites the rule without
  revising it.

**Worked example.**

A model with three Instances:
- `id=0`: `InstanceKind::Function { fn_sig:
  fn(u8)->(), body: Some(_), .. }`, item =
  `pl011::write_byte` (F4 ItemId = 7).
- `id=1`: `InstanceKind::Static { ty: u32, init:
  Some(_) }`, item = `pl011::BASE_ADDR` (F4 ItemId = 12).
- `id=2`: `InstanceKind::Struct { layout: _ }`, item =
  `pl011::Resource` (F4 ItemId = 3).

`regseq_instance_kind_rank`: Function = 0, Static = 1,
Struct = 3. The F4-2 sort `(rank, item_id, args, shim)`
orders:
- `id=0` (rank 0, ItemId 7) -- first.
- `id=1` (rank 1, ItemId 12) -- second.
- `id=2` (rank 3, ItemId 3) -- third.

Note `id=2`'s ItemId (3) is numerically smaller than
`id=0`'s ItemId (7), but rank dominates -- Functions
(rank 0) precede Structs (rank 3) regardless of ItemId.
This is the rank-grouped order D-14-6.2 inherits.

`render(&self)` iterates `[id=0, id=1, id=2]` in that
order, calling `render_instance(0)` then
`render_instance(1)` then `render_instance(2)` with
the per-Instance separator pinned at D-14-6.3 between
consecutive calls.

#### D-14-6.3 -- Per-Instance separator [LOCKED]

**Decision.** **Single blank line between consecutive
Instance renderings, achieved by the top-level
composer emitting the two-byte sequence `"\n\n"`
between consecutive `render_instance(id)` outputs.**
No leading blank before the first Instance; no
trailing blank after the last Instance; no trailing
newline injected after the last Instance's last byte.

`render(&self)` adapts rustc's `write_mir_pretty`
first-flag pattern at `pretty.rs:328-335`: a loop-local
flag suppresses the separator on the first iteration
and emits it on subsequent iterations. The difference
from rustc is **which bytes the separator carries**:
rustc emits a single `\n` because each per-item
renderer terminates its last line with an implicit
newline from the writer; RegseqModel's
`render_instance(id)` outputs end at the last byte of
content (`}` for body forms and ADT definitions, `;`
for declaration forms and Static / Const) with no
trailing newline. The top-level composer therefore
owns the full `"\n\n"` separator: the first `\n`
terminates the prior Instance's last line, and the
second `\n` produces the single blank line.

**Reasoning.** Three reasons:

1. **Rustc-mirrored first-flag pattern.** rustc's
   `write_mir_pretty` is the upstream batch-renderer
   reference per `prior-art/rustc-render.md` Q3; its
   first-flag-guarded separator emit between items is
   the established convention. RegseqModel inherits
   the first-flag pattern; the separator-bytes
   contract diverges because the underlying
   per-Instance renderers carry different trailing-byte
   discipline.
2. **No duplication of the self-identifying first
   line.** Each `render_instance(id)` output begins
   with its own Instance-identifying material -- the
   Stream-1 + Stream-2 attribute block (per §10.2.7)
   when non-empty, otherwise the kind-keyword line
   (`fn`, `static`, `static mut`, `const`, `struct`,
   `enum`, `union`). An explicit header comment
   (e.g., `// === Function: foo::bar ===`) would
   duplicate the identification without adding signal.
3. **Minimal noise; maximal information density.**
   The single blank line is the minimum
   visual-separator that preserves Instance
   boundaries. Concatenation without separator
   (alternative C) produces visual run-together that
   defeats diagnostic readability; header comments
   (alternative B) consume vertical space without
   information gain.

**Rejected alternatives.**

- **(B) Header-comment + blank line.** E.g.,
  `// ===== Function: foo::bar =====\n\n<body>`.
  Scan-friendly for very large models (10s+
  Instances), but duplicates each Instance's
  self-identifying prefix (attribute block when
  present, otherwise keyword + path). Adds ~2-3
  lines of overhead per Instance. The
  diagnostic-output consumer model (snapshot tests
  + per-driver debug dump) is well-served by the
  single-blank form; large-model scan tools (if
  they surface) can be a presentation-tier extension
  rather than a spec-tier commitment.
- **(C) No separator (concatenation).** Each
  `render_instance(id)` output ends at `}` or `;`
  with no trailing newline; concatenation without
  separator would place the next Instance's first
  byte (an attribute line or a keyword line)
  directly after the prior Instance's closing
  byte on the same line. Bad readability;
  diagnostically misleading.
- **(D) Multi-blank separator** (three or more `\n`
  bytes). Wastes vertical space; rustc uses single-
  blank; no consumer model surfaces a need for
  multi-blank.

**Methodology rules invoked.**

- *Compositional behavior beats special-case flags.*
  The separator is a single trivial rule applied
  uniformly between every pair of consecutive
  Instances; no per-`InstanceKind` separator
  customisation.
- *C semantics with information preservation.* The
  top-level composer owns the separator bytes
  rather than relying on a per-Instance trailing-
  newline contract. The §9.11 specimens already
  establish that `render_instance` outputs end at
  the last content byte (no implicit trailing
  newline); the separator design respects that
  contract directly.
- *Compiler-theory-first.* The first-flag mechanism
  is the established pretty-printer pattern for
  list-with-separators rendering (cf. rustc's
  `write_mir_pretty`, every comma-separated list in
  pretty-printers across compilers).

**Spec impact.**

- `pretty-printer-snapshot.md` §10.3 -- separator
  rule pinned: top-level composer emits `"\n\n"`
  between consecutive `render_instance(id)` outputs;
  no leading or trailing separator; no trailing
  newline after the last Instance.
- `pretty-printer-snapshot.md` §8 ownership table row
  for `render(&self)` -- Description column carries
  the separator + envelope summary.
- `spec-snapshot.md` -- no edits.

**Worked example.** Continuing the D-14-6.2 worked
example (three Instances: `pl011::write_byte` Function,
`pl011::BASE_ADDR` Static, `pl011::Resource` Struct):

```text
fn pl011::write_byte(_1: u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = const ();
        return;
    }
}
                                  <-- single blank line per D-14-6.3
static pl011::BASE_ADDR: u32 = 268472320_u32;
                                  <-- single blank line per D-14-6.3
struct pl011::Resource {
    base: u32,
    state: u32,
}
```

No leading blank before `fn pl011::write_byte`; no
trailing blank after `struct pl011::Resource`'s
closing `}`.

#### D-14-6.4 -- File-level output envelope [LOCKED]

**Decision.** **Option (A) -- no file-level header,
no file-level trailer, no metadata wrapper.**

`render(&self)`'s output begins with the first
rendered byte of the first Instance's
`render_instance` output -- the first byte of its
Stream-1 + Stream-2 attribute block when non-empty
(per §10.2.7), otherwise the first byte of the
kind-keyword line (`fn` / `static` / `static mut` /
`const` / `struct` / `enum` / `union`). The output
ends with the last rendered byte of the last
Instance's `render_instance` output -- the closing
`}` for body forms and ADT definitions, or the `;`
for declaration forms and Static / Const. `render`
does not inject any trailing newline after the last
Instance. Empty `model.instances` produces the empty
String per D-14-6.5. No WARNING comment, no metadata
header, no version tag, no file boundaries.

**Reasoning.** Three reasons:

1. **Rustc-warning premise does not apply.**
   Rustc's `write_mir_pretty` emits a 3-line WARNING +
   HINT header at `pretty.rs:324-326` with the
   explicit message "format is unstable, do not
   parse." RegseqModel's `pretty-printer.md` is
   **designed as the spec** for pretty-printed
   output: format stability is enforced by the
   snapshot test suite (`insta` per stage-1 Q5 /
   D-1-S2 + stage-13 D-13-7). The output IS the spec.
   A "do not parse" warning would advise consumers
   against the very thing the snapshot suite is
   designed to enforce.
2. **`RegseqModel.build_info` already holds metadata.**
   The crate name + toolchain + target + emission
   timestamp (the metadata that alternative C would
   inject as a header) are already accessible via
   `RegseqModel.build_info` per `spec-snapshot.md`
   §3.1. A diagnostic consumer wanting the build
   provenance reads `model.build_info` directly; a
   snapshot consumer doesn't need the metadata in
   the rendered output (the snapshot suite manages
   its own framing). Embedding the metadata in
   `render(&self)`'s output would duplicate it
   across every snapshot file.
3. **Stable-IR analog operates without file-level
   wrapper.** `rustc_public` per
   `prior-art/rustc-render.md` Q1 has no top-level
   batch renderer; consumers explicitly own the
   file-level wrapper layer. RegseqModel's
   consumer-managed-framing model matches this
   stable-IR pattern (snapshot suite manages
   framing; per-driver debug dump tools manage
   framing). The renderer's job is content; framing
   is the consumer's responsibility.

**Rejected alternatives.**

- **(B) Rustc-mirrored 3-line WARNING + HINT header.**
  Premise does not apply (see Reasoning #1); would
  conflict with the snapshot-test consumer model.
- **(C) RegseqModel-specific metadata header**
  (crate + toolchain + target + timestamp).
  Duplicates `model.build_info`; bloats every
  snapshot with metadata-noise; defeats the
  consumer-managed-framing model.
- **(D) Minimal version-tag header** (`//
  RegseqModel pretty-print v1`). Operationally
  redundant against the snapshot suite (version
  stability is enforced at the snapshot-diff layer;
  the `Cargo.lock` toolchain pin + snapshot suite
  manage forward-compatibility together). Adds
  noise across every snapshot for unclear benefit.
- **(E) Trailer-only** (e.g., final `// end of
  RegseqModel`). Symmetric to (B); same conflicts.

**Methodology rules invoked.**

- *No defensive fallbacks.* No "what if consumers
  parse the output mechanically" hedge -- the
  snapshot suite enforces stability; consumers
  reading snapshots get a stable artifact under the
  pinned toolchain.
- *C semantics with information preservation.*
  Build metadata is stored once at
  `RegseqModel.build_info` rather than re-emitted
  inline in pretty-printed output; consumers needing
  it read it once at the structured form.
- *Compositional behavior beats special-case flags.*
  The render is the pure concatenation of
  per-Instance renderings + separator; no file-level
  decoration layer.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.3 -- envelope
  rule pinned: "No file-level header, no
  file-level trailer, no metadata wrapper. Output
  starts with the first rendered byte of the first
  Instance's `render_instance` output (attribute
  block when non-empty per §10.2.7, otherwise the
  kind-keyword line) and ends with the last
  rendered byte (`}` or `;`); no trailing newline
  injected by `render`."
- `pretty-printer-snapshot.md` §1.3 composer
  listing entry -- existing wording covers the no-
  header case implicitly; D-14-6.4 lock confirms
  no edit needed.
- `pretty-printer-snapshot.md` §8 ownership table
  row -- no envelope-specific addition required;
  the "top-level model dump" Description column
  wording naturally implies the no-wrapper form.
- `spec-snapshot.md` -- no edits.

**Worked example.** Continuing the D-14-6.2 / D-14-6.3
worked example (three Instances), the full
`render(&self)` output is:

```text
fn pl011::write_byte(_1: u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = const ();
        return;
    }
}

static pl011::BASE_ADDR: u32 = 268472320_u32;

struct pl011::Resource {
    base: u32,
    state: u32,
}
```

The output begins at column 0 of line 1 with
`fn pl011::write_byte(` (the kind-keyword line; this
Instance has no Stream-1 / Stream-2 attribute block).
The output ends at the closing `}` byte of
`struct pl011::Resource`; `render(&self)` does not
inject any trailing newline. No header above line 1;
no trailer below the final `}`.

#### D-14-6.5 -- Empty-model case [LOCKED]

**Decision.** **Option (A) -- empty string `""`.**

When `model.instances.is_empty()`, `render(&self)`
returns the empty `String`. No header (per D-14-6.4
no-wrapper rule); no items (the iteration loop never
runs); no separator (no consecutive pairs to
separate); no placeholder comment.

**Reasoning.** Three reasons:

1. **Natural identity for the concatenation
   operation.** `render(&self)` is structurally the
   reduction of `model.instances.iter_enumerated()`
   via the operation
   `accumulator + separator + render_instance(id)`.
   The identity element of concatenation is the
   empty string; empty `model.instances` reduces to
   the identity. No special-case handling required.
2. **Empty `model.instances` is legitimate.**
   Per `prior-art/rustc-render.md` Q5, empty
   `model.instances` arises in foundation-crate test
   fixtures exercising pure-Item surfaces (DefPath /
   IntrinsicKind / Layout) without monomorphizing
   any bodies; future-stage validator round-trip
   baselines; rejection-path test fixtures where
   construction errors out before any Instance is
   populated. In all three, the consumer expects
   "no Instances => no rendered output." A
   placeholder adds confusion ("did something get
   filtered? is this a stub?") without operational
   value.
3. **Snapshot-suite compatibility.** An empty
   `model.instances` produces a 0-byte snapshot file
   under `insta`. The diff machinery handles 0-byte
   snapshots as a first-class case (the snapshot
   compares to nothing; any non-empty output
   triggers a snapshot review). A placeholder
   comment would inject a synthetic non-empty
   form that obscures whether the test fixture is
   intentionally empty or unintentionally
   truncated.

**Rejected alternatives.**

- **(B) Placeholder comment** (e.g., `// (empty
  model)`, `// no instances`, `// 0 instances`).
  Adds noise for valid empty cases; obscures the
  empty-by-intent vs empty-by-truncation
  distinction. The consumer model already handles
  empty correctly; a placeholder is a workaround
  for a non-problem.
- **(C) Error / panic on empty.** Treats empty
  `model.instances` as invalid. Per Reasoning #2,
  empty is legitimate; rejecting it conflicts with
  the test-fixture / round-trip / rejection-path
  consumer cases.

**Methodology rules invoked.**

- *No defensive fallbacks.* No "what if empty
  model means something is wrong" hedge -- empty
  is valid; producing the empty-string identity
  is the correct response.
- *Compositional behavior beats special-case flags.*
  Empty case falls out of the loop being a no-op
  on empty input; no `if model.instances.is_empty()
  { return special_case }` branch needed.
- *Spec is mutable.* If a future consumer surfaces
  a need for empty-distinguishing output (e.g., a
  per-driver tool that wants to flag empty
  builds), the placeholder option (B) can be
  revisited; not at v1.

**Spec impact.**

- `pretty-printer-snapshot.md` §10.3 -- empty-case
  rule pinned: "When `model.instances.is_empty()`,
  `render(&self)` returns the empty string `""`."
- `pretty-printer-snapshot.md` §1.3 composer listing
  entry -- already covers the iteration form; the
  no-op-on-empty behaviour is implicit. D-14-6.5
  lock confirms no edit needed.
- `pretty-printer-snapshot.md` §8 ownership table --
  no edit; "top-level model dump" includes the empty
  case naturally.
- `spec-snapshot.md` -- no edits.

**Worked example.** For
`RegseqModel { instances: IndexVec::new(), .. }`:

```text

```

(0 bytes; the empty string. No newline; no
placeholder; no header.)

This is the operational case for `RegseqModel::new()`
test baselines and for fixtures exercising
construction-rejection paths (where the bridge errs
out before populating any Instance).

#### D-14-6.6 -- Cross-stage friction outcome [LOCKED]

**Decision.** **Zero new stage-13 D-tag REOPENs
from D-14-6 drafting.** The per-sub-decision audit
pass (D-14-6.1 through D-14-6.5) consumed
stage-13's spec surfaces (§2.3 method-shape per
D-13-1; §3.1 body envelope per D-13-2 -- consulted
transitively via `render_instance`; §1.3 composer
listing per D-13-4.6; §8 ownership table per
D-13-4.6) and `spec-snapshot.md`'s structural
surfaces (§3.1 `RegseqModel.instances` field;
§9.3 `regseq_instance_kind_rank`; §16.3 F4-2
sort key) **without modifying their locked content.**

The §10.3 §1.3 §8 spec contributions are **additive
spec text** -- D-14-6.2 / D-14-6.3 / D-14-6.4 /
D-14-6.5 pin rules at the §10.3 surface (which was a
stub) and refine the §1.3 / §8 entries (already
present with locked-state-compatible wording from
the D-14-5 close). No §16.3 / §9.3 / §3.1 / §2.3 /
§3.1-13 edits.

Pending merger-time flips from prior clusters
unchanged:
- **D-9-10** (from D-14-2.5) -- `dummy_static_init`
  rejection wording.
- **D-13-6** (from D-14-4.9) -- `render_operand` /
  `render_rvalue` signature extensions with
  `expected_ty: TyId`; `render_generic_args`
  signature extension with `item: ItemId`.
- **D-12-1.3 / D-9-9 / D-13-4.3** (from D-14-7 when
  the `#[naked]` rejection cluster locks) --
  `is_naked` field removal + `#[naked]` rejection-
  table row + per-flag rendering table edit.

D-14-6 adds **no new pending flips**.

**Reasoning.** Three reasons drive the zero-REOPEN
outcome:

1. **D-14-6 is a pure consumer-side composer.** The
   cluster's payload (§10.3 spec text + §9.12
   specimens + §1.3 / §8 finalisation) lives
   entirely within `pretty-printer-snapshot.md`.
   No `spec-snapshot.md` edits; no IR-structural
   changes; no `InstanceKind` field reshape; no new
   `RegseqModel` field. The producer-side spec is
   unchanged.
2. **`render_instance` consumed as-is.** D-14-6.1
   confirmed the zero-argument top-level form;
   D-14-6.2 confirmed that the loop dispatches via
   `self.render_instance(id)` with no signature
   extension. The D-14-5-era
   `render_instance(InstanceId) -> String` signature
   covers all top-level callsites unchanged. No
   stage-13 D-tag (§5.4 `render_item` signature;
   §1.3 composer listing entry; §8 ownership table
   row) requires modification.
3. **F4-2 sort consumed as-is.** D-14-6.2's
   iteration rule cites `spec-snapshot.md` §16.3 /
   §9.3 without revising. The F4-2 sort key is
   locked; the rank table is append-only;
   `model.instances` is canonical by construction.
   The renderer reads the resolved order; no
   render-time machinery extends the sort key.

**Methodology rules invoked.**

- *Spec is mutable.* If Phase B Codex review
  surfaces a stage-13 REOPEN-shaped finding (e.g.,
  per-arm composition outline at §10.3 requires
  `render_instance` to produce a slightly different
  shape at the top-level callsite vs the
  per-Instance callsite), D-14-6.6 entry updates
  per the friction-flip annotation pattern (mirrors
  D-14-4.9's REOPEN of D-13-6).
- *No defensive fallbacks.* Zero-REOPEN outcome
  recorded based on architectural evidence at lock
  time; if review surfaces a friction, register the
  flip with rationale.
- *Compiler-theory-first.* The composer-from-scratch
  pattern locked at D-14-5.7 (Option (a)) shapes
  D-14-6's surface to be a pure top-level
  scaffolding consumer of D-14-5's composer; no
  layer ambiguity between top-level and per-Instance
  surfaces.

**Spec impact.**

- `pretty-printer-snapshot.md` -- additive spec text
  only (no modifications to any pre-D-14-6 locked
  content beyond Description-column refinements at
  §1.3 / §8 that were already forward-pointed to
  D-14-6 lock).
- `spec-snapshot.md` -- no edits.
- `decisions.md` -- no new pending flips from
  D-14-6. The merger-time flips from D-14-2.5 /
  D-14-4.9 (and D-14-7 when it locks) are
  unchanged.

**Worked example.** If a Phase B Codex review
surfaces a finding like "the top-level
`render(&self)` callsite needs `render_instance` to
emit an additional `#[doc(hidden)]` decoration when
the parent context is a top-level dump but not when
called from a per-Instance debug path," then:

- D-14-6.6 entry updates to record the friction-flip
  ("revised post-Phase B: `render_instance`
  signature extended with a `caller_context:
  RenderContext` parameter").
- D-14-5.1's API signature LOCKED status flips to
  REOPENED in `decisions.md` at the next merger
  close with annotation "(revised in stage 14
  D-14-6.6: ...)".
- The §10.3 spec text references the
  RenderContext-aware composition rule.

**No such finding is anticipated**; this is the
contingency mechanism, not the expected outcome.

---

### D-14-7 -- `#[naked]` rejection in v1 (external-review Finding #2) [LOCKED -- Phase A 9/9 sub-decisions (2026-05-13); Phase B closed at R6 APPROVE / Lockability:yes / Close-out:YES (2026-05-13)]

**Question.** External-review Finding #2
(`expectations.md` Finding #2; original review at
`/Users/nyengele/Downloads/regseq-dsl-spec-review.md`)
surfaced an irreconcilable pair of locked decisions:
stage-12 D-12-1.3 carries `is_naked: bool` on `Item`
(§8.0 schema) under the framing "C-codegen emits naked
functions without prologue/epilogue (raw asm body only);
load-bearing for ISR entry stubs in kernel drivers,"
while stage-9 D-9-9 row 8 (§23.6.1) rejects
`TerminatorKind::InlineAsm` at terminator translation
because the v1 IR has no body carrier for raw asm. A
`#[naked]` function's MIR body is structurally a single
`InlineAsm`-only block; with no representable body, the
`is_naked: true` Item-side state is unreachable.

The reviewer outlined two resolution shapes:
(a) keep `is_naked` and lift `TerminatorKind::InlineAsm`
out of the rejection list to admit naked-function
bodies; (b) reject `#[naked]` outright and remove the
`is_naked` field. The reviewer's preferred framing was
to defer naked-ISR support to a future
`#[rseq::interrupt(...)]` domain extension that
synthesises a C/asm wrapper around an ordinary Rust
handler body. The
`stage-14/expectations.md` Finding #2 finalised the
preferred resolution: v1 rejects `#[naked]` at Item
translation, drops the `is_naked` field, reopens
D-12-1.3 (typed Item-level fields), D-9-9 (the §23.6.1
rejection table -- extended via §15.1 with the new
Item-translation phase, sibling to the §23.6.1
terminator-translation row), and D-13-4.3 (per-flag
rendering table).

D-14-7 is the cluster that lands the resolution. It is
the **fourth external-review Finding cluster** of stage
14 (Finding #1 folded into D-14-1; Finding #2 is this
cluster; Finding #3 is D-14-8 stale forward-pointer
retarget; Finding #4 is D-14-9 Specimen 70 split). It
is also the **first surgical-fix cluster** to close
after the composer-renderer trio (D-14-4 / D-14-5 /
D-14-6); the remaining stage-14 work is small surgical
fixes (D-14-8 + D-14-9 + c_void rejection enforcement)
plus the per-stage cross-cluster Codex review before
merger.

**Evidence basis.** Step 4 rustc evidence pass landed
at `prior-art/rustc-naked.md` (2026-05-13) surveying
6 questions Q1-Q6 across rustc's `#[naked]` handling
surface: attribute parsing (`NakedParser` at
`rustc_attr_parsing/src/attributes/codegen_attrs.rs:221-333`),
the source-of-truth flag (`CodegenFnAttrFlags::NAKED`
at `rustc_middle/src/middle/codegen_fn_attrs.rs:167`),
the body-shape contract under `#[naked]`
(`rustc_codegen_ssa/src/mir/naked_asm.rs:30-41` --
the START_BLOCK terminator must be
`TerminatorKind::InlineAsm`), the codegen-side
dispatch (`rustc_codegen_ssa/src/mono_item.rs:44-51` +
`:74-82` -- naked Fns become global asm blocks,
predefine pass skips them), the attribute-target
restrictions (allowed targets `Fn` + method variants
only; cross-attribute compatibility checked by
`NakedParser::finalize` at `:248-333`), and the
structural overlap with stage-9 D-9-9 row 8 (Q6
analysis -- Item-translation rejection is preferable
to deferring to the body-walk InlineAsm rejection for
diagnostic clarity + fail-fast + spec-vocabulary
discoverability). The Step 4 evidence is appended to
`rustc-references.md` under a new "`#[naked]` handling"
section with five file:line landmarks.

The evidence surfaces three observations that load-bear
on D-14-7:

1. **One per-DefId source-of-truth bit.** Bridge
   recognition reduces to a single API call
   (`tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`)
   per Q2. No HIR-attribute walk needed; the flag is
   downstream of `NakedParser`'s legality checks.
2. **Naked is strictly source-Item-level.** rustc's
   `codegen_instance_attrs` at
   `codegen_fn_attrs.rs:14-28` strips the NAKED flag
   for non-Item `InstanceKind`s (shims). Confirms the
   recognition point is Phase 2 (Translation+Synthesis)
   per stage-12 D-12-5 Item-walk, not Instance-walk.
3. **Existing InlineAsm rejection already catches
   naked.** A `#[naked]` Item that survived Item
   translation would fail at body translation when the
   bridge walks the START_BLOCK terminator
   (`TerminatorKind::InlineAsm` per Q3, rejected by
   stage-9 D-9-9 row 8 per Q6). Item-translation
   rejection is the **preferred** catch point, not the
   **only** catch point: cleaner diagnostic + earlier
   fail-fast + discoverability under "naked" keyword.
   Both rows remain in the spec as siblings (Item-
   translation row at §15.1 + terminator-translation
   row at §23.6.1).

**Sub-decision allocation.** Nine sub-decisions
allocated:

- **D-14-7.1 (load-bearing) -- Rejection decision +
  diagnostic content.** Three candidate forms:
  - (A) Reject `#[naked]` at Item translation in v1;
    diagnostic names the attribute explicitly and
    recommends the future `#[rseq::interrupt(...)]`
    domain extension for ISR-stub synthesis (mirrors
    D-9-9 row 8's diagnostic shape: name the construct
    + suggest the DSL alternative).
  - (B) Admit `#[naked]` Items; lift D-9-9 row 8's
    InlineAsm rejection inside naked-function bodies
    so the raw asm body translates as a body
    `TerminatorKind::InlineAsm` carrier.
  - (C) Defer the decision; keep `is_naked` field as a
    bridge-rejected sentinel pending phase-3 device
    pressure.
  Per Q6 evidence preponderance: (A). (B) reopens
  D-9-9 row 8 in the larger direction (admit instead
  of reject) and requires a new IR body shape for
  `InlineAsm`-terminator-only bodies that no v1
  driver fixture demands. (C) violates the
  no-defensive-fallbacks rule and `expectations.md`
  Finding #2's "smaller-scope option chosen over the
  reviewer's preferred domain-extension framing because
  v1 has no concrete ISR-stub fixture demanding the
  extension" framing -- a sentinel that can never be
  set is misleading API surface. **Load-bearing**:
  gates the field-deletion (D-14-7.4 / D-14-7.5) and
  the diagnostic-table-row contents (D-14-7.3).

- **D-14-7.2 -- Recognition API + pipeline phase.**
  Two candidate forms:
  - (A) `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
    during stage-12 D-12-5 Phase 2 (Translation+
    Synthesis) per-Item walk. Same source-of-truth
    bit rustc's own codegen consults at
    `rustc_codegen_ssa/src/mono_item.rs:46`.
  - (B) Manual HIR-attribute walk against
    `sym::naked`. Reimplements what rustc has already
    done; needs the bridge to re-derive
    `NakedParser`'s legality discrimination from raw
    `Attribute` shapes.
  Per Q1+Q2 evidence: (A). The `codegen_fn_attrs`
  query result is the canonical post-parsing state
  rustc surfaces; reimplementing the walk would
  bake in attribute-shape assumptions that
  `NakedParser` could change under a toolchain bump
  with no protective contract.

- **D-14-7.3 -- Rejection-table placement.** Three
  candidate forms:
  - (A) Add a new row to §15.1 with phase "Item
    translation"; extend the §15.1 header note to
    acknowledge a fourth category ("Item-attribute
    DSL rejections"). Sibling row to the stage-9
    D-9-9 §23.6.1 terminator-translation row;
    discoverable from the single bridge-rejection
    catalogue at §15.1.
  - (B) Open a new §8.3 "Item-translation rejections"
    table dedicated to Item-attribute rejections.
    Parallel rejection table to §15.1.
  - (C) Extend §23.6.1 with the Item-translation row.
    Semantically wrong (§23.6.1 is "Terminator-site
    invariants").
  Per `expectations.md` Finding #2's "§23.6.1 (or
  the Item-translation rejection-table area)"
  framing: (A). §15.1 already carries multiple
  rejection phases (type, call, place translation);
  adding "Item translation" as a fourth phase is
  consistent with existing structure, avoids
  proliferating rejection tables, and keeps the
  single discoverable catalogue rule
  (driver-author searching "naked" finds the
  rejection row directly).

- **D-14-7.4 -- `Item.is_naked` field deletion at
  §8.0.** No alternatives; the field has no
  representable state post-D-14-7.1. Edit deletes
  the `pub is_naked: bool,` line + the 6-line
  `// is_naked corresponds to #[naked] per rustc's
  CodegenFnAttrFlags::NAKED. C-codegen emits naked
  functions without prologue/epilogue (raw asm body
  only); load-bearing for ISR entry stubs in kernel
  drivers.` comment, and replaces with a 9-line
  rationale comment naming D-14-7 + the §15.1
  rejection row + the deferral path. The §8.0
  comment immediately preceding the deletion site
  ("Function-relevant flags") is updated from
  "a `static` is never naked or inline" to "a
  `static` has no inline hint and is never `unsafe
  fn`" to drop the obsolete naked reference.

- **D-14-7.5 -- `instance_is_naked` helper deletion
  at §3.5.2.** No alternatives; the helper mirrored
  the deleted field. Edit deletes the
  `pub fn instance_is_naked(&self, id: InstanceId)
  -> bool { /* ... */ }` line and updates the
  "Source-level metadata fields per D-12-1.3"
  comment block to acknowledge the D-14-7 revision.
  The 14-helper inventory paragraph
  ("14-helper v1 baseline inventory (13 from
  stage-12 D-12-1.8 + 1 domain-attribute pull-
  through from stage-14 D-14-1.6)") downshifts to
  13 total (12 from stage-12 D-12-1.8 as revised +
  1 from D-14-1.6) and acknowledges the D-14-7
  revision inline.

- **D-14-7.6 -- Pretty-printer per-flag table edits
  at §5.3 + §5.6 + §5.1.** Three coordinated edits:
  - §5.3 per-flag rendering table: delete the
    `is_naked = true` -> `#[naked]` row and the
    `is_naked = false` -> suppressed row (two-row
    surgical removal between the `is_unsafe` rows
    and the lang-flag rows).
  - §5.6 row-order recap: delete item 3
    (`Item.is_naked == true` -> `#[naked]`);
    renumber items 4-6 to 3-5; acknowledge the
    D-14-7 revision in the row-order-recap
    introductory sentence.
  - §5.1 attribute-decorator-syntax atomic-shape
    example list: drop `#[naked]` from the atomic
    example list (the attribute no longer surfaces
    from any field; the surviving atomic-shape
    examples are `#[no_mangle]` and
    `#[opaque_handle]`).
  Additionally: §5.2 default-suppression narrative
  ("not unsafe, not naked, no lang-item flags, not
  eligible for opaque-handle") drops "not naked,"
  matching the trimmed field list.

- **D-14-7.7 -- Specimen alignment with the field
  deletion.** Two coordinated specimen edits in
  `pretty-printer-snapshot.md` §9:
  - Specimen 69 (Bare function zero attributes)
    input shape drops `is_naked: false` (the field
    no longer exists; specimens cannot reference a
    non-existent schema field).
  - Specimen 70 (Function with full attribute
    stress) input shape drops `is_naked: true`; the
    output drops the `#[naked]` rendered line.
    A trailing parenthetical acknowledges the
    cross-finding overlap with D-14-9 / Finding #4
    (which addresses the `opaque_handle_eligible:
    true` on a Function shape -- a separate
    bridge-outcome ineligibility cleanup landing in
    D-14-9's surgical fix; D-14-7 only touches the
    `is_naked` side).
  - Specimen 75 (Union with lang-flag) input shape
    drops `is_naked: false`.
  No new specimens added; no specimen renumbering.

- **D-14-7.8 -- §3.6.6 paragraph 3 finalisation.**
  The §3.6.6 paragraph 3 in `spec-snapshot.md`
  currently uses pre-lock wording ("External-review
  Finding #2 (`#[naked]` rejection in v1; D-14-7
  territory) is consistent with this rule").
  Edit rewrites to locked-state wording ("Stage-14
  D-14-7 (`#[naked]` rejection in v1) is consistent
  with this rule. ... The Item-translation rejection
  row lives at §15.1; no `AttrData::Naked` variant
  exists."). Documents the architectural posture:
  `#[naked]` is a Rust attribute (not `#[rseq::*]`),
  so it had no path into `DomainAttrTables` even if
  it had been kept; the rejection removes the
  typed-fields-side representation without migrating
  it.

- **D-14-7.9 -- Cross-stage friction outcome.** Three
  REOPENs surface at merger close per the
  `expectations.md` Finding #2 mechanics:
  - **D-12-1.3 [REOPENED]** -- typed Item-level
    fields list loses the `is_naked: bool` entry.
    Annotation: "(revised in stage 14 D-14-7:
    `is_naked` field removed; v1 rejects `#[naked]`-
    attributed Items at Item translation.)"
  - **D-9-9 [REOPENED]** -- the rejection-vocabulary
    surface grows by one Item-translation row at
    §15.1, sibling to the §23.6.1 terminator-
    translation row 8 (InlineAsm). The §23.6.1
    9-row table itself stays at 9 rows; the §15.1
    table grows by one row. Annotation: "(revised
    in stage 14 D-14-7: rejection vocabulary
    extended with `#[naked]` Item-translation row
    at §15.1, sibling to InlineAsm terminator-
    translation row at §23.6.1.)"
  - **D-13-4.3 [REOPENED]** -- per-flag rendering
    table loses the `is_naked` row + §5.6 row-order
    recap loses item 3 + §5.1 atomic-shape example
    list loses `#[naked]`. Annotation: "(revised in
    stage 14 D-14-7: `#[naked]` row removed from
    per-flag rendering table -- flag no longer
    exists on Item.)"
  D-12-1.8 (Instance-side pull-through helpers)
  stays LOCKED -- its content is downstream of
  D-12-1.3's field list; helper-count update is
  absorbed implicitly. Only the upstream decision
  flips per the convention "flip the upstream
  decision when downstream automatic consequences
  land."

**Spec impact (landed at Phase A lock).**

- `spec-snapshot.md` §3.5.2 -- `instance_is_naked`
  helper deletion + helper-comment update +
  inventory-narrative count downshift (14 -> 13
  helpers).
- `spec-snapshot.md` §3.6.6 paragraph 3 --
  pre-lock D-14-7-territory wording rewritten to
  locked-state Stage-14-D-14-7 wording acknowledging
  the §15.1 rejection row + no `AttrData::Naked`
  variant.
- `spec-snapshot.md` §8.0 -- `is_naked` field block
  deletion (`pub is_naked: bool,` + 6-line comment)
  + 9-line replacement-comment naming D-14-7 + the
  §15.1 rejection row + the deferral path; adjacent
  "Function-relevant flags" comment dropped the
  obsolete "naked or inline" wording. The field-
  enumeration narrative paragraph rewords from "The
  8 Item-level fields above (...is_naked + 3 lang-
  item flags + opaque_handle_eligible)" to "The 7
  typed source-level metadata fields above (...3
  lang-item flags) plus the bridge-computed
  opaque_handle_eligible outcome" -- splitting the
  enumeration into 7 typed-source fields + 1
  bridge-computed outcome to avoid mis-counting
  `opaque_handle_eligible` as a typed source-level
  input. The trimmed surface (7 typed source-level
  fields) reflects the D-14-7 deletion of
  `is_naked`; the original D-12-1.3 framing carried
  8 such fields.
- `spec-snapshot.md` §15.1 -- new "Item-attribute
  DSL rejections" category in the header note +
  new row "`#[naked]` on a function Item | Item
  translation | ..." inserted between the
  `core::ptr::{read,write}_volatile` row and the
  `extern "C" { unwind: true }` row.
- `pretty-printer-snapshot.md` §5.1 -- atomic-shape
  example list trims `#[naked]`.
- `pretty-printer-snapshot.md` §5.2 -- default-
  suppression narrative trims "not naked,".
- `pretty-printer-snapshot.md` §5.3 -- per-flag
  table trims `is_naked = true` + `is_naked =
  false` rows.
- `pretty-printer-snapshot.md` §5.6 -- row-order
  recap renumbered (items 4-6 to 3-5) + D-14-7
  revision acknowledged in introductory sentence.
- `pretty-printer-snapshot.md` §9.9 specimens --
  Specimen 69 input drops `is_naked: false`;
  Specimen 70 input drops `is_naked: true` and
  output drops `#[naked]` rendered line, plus a
  trailing parenthetical referencing D-14-9 cross-
  finding; Specimen 75 input drops `is_naked:
  false`.
- `decisions.md` -- three pending merger-time
  REOPENs (D-12-1.3, D-9-9, D-13-4.3) + one new
  `## Stage 14` entry for D-14-7 (and D-14-7.1
  through D-14-7.9 sub-decisions as bulleted
  children).

**Methodology rules invoked at allocation** (cluster-level):

- *No defensive fallbacks.* The chosen resolution
  rejects rather than absorbing the cross-decision
  contradiction. Keeping `is_naked: bool` as a
  sentinel that v1 can never set would be the
  fallback pattern the rule prohibits; instead the
  field is removed and the rejection is named.
- *Compositional behavior beats special-case flags.*
  The §15.1 rejection-table row is the rejection
  mechanism. No parallel `is_naked` marker flag on
  Item is needed because the existing rejection
  mechanism (already used for c_void, atomic
  primitives, volatile primitives) covers the case.
  The composer-of-rejection-rows beats a
  field-flag-plus-elsewhere-rejection-check pattern.
- *C semantics with information preservation.* The
  bridge stores the resolved rejection outcome
  (rejection at Item translation), not the input
  shape (Rust attribute as `CodegenFnAttrs.flags`
  bit). The rejection is the consumer-side
  interpretation; the input shape is rustc's; the
  bridge does the interpretation work and produces
  no carrier for it.
- *Code does not reference discussion-folder paths.*
  The §15.1 rejection-row prose names "stage-9
  D-9-9 row 8 (§23.6.1)" as a sibling-row pointer
  (stable D-tag references per `../README.md` §2)
  and "phase-2 stage 17 `#[rseq::interrupt(...)]`
  domain extension" as the deferral pointer (stage
  number, not a discussion-folder path). No
  `stage-14/discussions.md` citations.
- *Spec is mutable.* When `expectations.md` Finding
  #2 surfaced the cross-decision contradiction
  between D-12-1.3 + D-9-9, the resolution is a
  spec revision (delete the field, add the
  rejection row), not a workaround in compiler
  code. The three REOPENs are the explicit
  acknowledgement that this is a spec change, not a
  silent fix.

**Prior-art evidence basis.**

- `prior-art/rustc-naked.md` (this cluster's Step 4
  evidence pass) -- 6 Q-tags covering attribute
  parsing, source-of-truth flag, body-shape
  contract, codegen-side dispatch, attribute-target
  restrictions, and structural overlap with stage-9
  D-9-9 row 8.
- `prior-art/rustc-instance.md` (D-14-5's Step 4
  evidence pass) -- not directly consulted; D-14-7's
  surface is rejection-at-Item-translation, not
  per-Instance rendering. The §23.6.1 row 8
  cross-reference is consumed from the existing
  stage-9 D-9-9 audit shape.

**Phase A drafting plan.** Nine-step lock sequence:

1. Open D-14-7 entry in `stage-14/discussions.md`
   (this entry).
2. Run Step 4 rustc evidence pass into
   `prior-art/rustc-naked.md` (done -- 6 Q-tags
   Q1-Q6).
3. Append `rustc-references.md` "`#[naked]`
   handling" section (5 file:line landmarks).
4. Lock D-14-7.1 (rejection decision + diagnostic
   shape) -- the load-bearing decision.
5. Lock D-14-7.2 + D-14-7.3 together (recognition
   API + rejection-table placement; both
   structural-spec sub-decisions).
6. Apply spec-snapshot.md edits (D-14-7.4 +
   D-14-7.5 + D-14-7.8 + part of D-14-7.3 = §15.1
   row insertion).
7. Apply pretty-printer-snapshot.md edits
   (D-14-7.6 + D-14-7.7).
8. Lock D-14-7.9 (cross-stage friction outcome --
   the three REOPENs).
9. Per-stage Phase B Codex review (anticipated 1-3
   rounds; surface smaller than D-14-4 + D-14-5 +
   D-14-6 because the cluster is a surgical-fix
   cluster, not an architecture cluster).

**Worked example.** Consider a v1 driver source like:

```rust
#[naked]
unsafe extern "C" fn isr_entry() {
    core::arch::naked_asm!(
        "...",
        options(noreturn)
    );
}
```

Bridge codepath:

1. **Item discovery (Phase 2 Translation+Synthesis).**
   Bridge walks the crate's Items; encounters
   `isr_entry` Item.
2. **Attribute check.** Bridge calls
   `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`.
   Returns `true`.
3. **Rejection.** Bridge emits the §15.1 Item-
   translation rejection diagnostic:
   > "`#[naked]` functions are not supported in v1.
   > The IR has no representable body shape for a
   > raw-asm-only function body; `TerminatorKind::InlineAsm`
   > is rejected at stage-9 D-9-9 row 8 (§23.6.1).
   > Defer to the future `#[rseq::interrupt(...)]`
   > domain extension (phase-2 stage 17), which
   > will synthesise a C/asm wrapper around an
   > ordinary Rust handler body."
4. **Pipeline halts** with `unsupported_construct!`-
   style error per §23.6.1 diagnostic discipline
   (the construct-rejection convention shared with
   stage-9 D-9-9 rejection rows). No partial
   `Item` is materialised for `isr_entry`; no
   `Instance` is interned for the function-item-ZST.

The driver author sees a diagnostic naming the
attribute they wrote (`#[naked]`) and the deferral
path (the future `#[rseq::interrupt(...)]` extension).
The Item-translation rejection fires before any body
translation work is performed.

**Contrast: if D-14-7 had NOT landed**, the bridge
would have created an `Item` with `is_naked: true`,
created the corresponding `Instance`, attempted to
walk the body, and rejected at step (4)'s body walk
on the InlineAsm terminator at §23.6.1 row 8. The
diagnostic would name `TerminatorKind::InlineAsm`
instead of `#[naked]` -- harder for the driver
author to map back to source. Item-translation
rejection is the diagnostic-clarity + fail-fast win.

---

### D-14-8 -- Stale `emission_name` forward-pointer retarget (external-review Finding #3) [LOCKED -- Phase A 2026-05-13; Phase B closed at R2 APPROVE / Lockability:yes / Close-out:YES (2026-05-13)]

**Question.** External-review Finding #3
(`expectations.md` Finding #3; original review at
`/Users/nyengele/Downloads/regseq-dsl-spec-review.md`)
flagged a stale forward pointer in the §9.2
`Instance` schema. The `emission_name` field
bullet describing `Struct / Enum / Union` C-side
identifier construction ends with the clause "the
construction rule is pinned in stage 13 (codegen)."
Stage 13 closed as the pretty-printer spec per
D-13-9 (top-level spec split into co-equal
`spec.md` + `pretty-printer.md` along the
producer-side / consumer-side rendering boundary),
**not** codegen. The forward pointer was authored
by stage 9 when stage 13's scope was still
expected to cover codegen; the spec-split
supersession left the forward pointer pointing at
a stage that does not own the algorithm. The
C-identifier-safe construction algorithm itself
(escaping for C keywords / non-ASCII / leading
digits / double underscores; hash-suffix
collision-resistance scheme; length cap and
maximum-name fallback) is **genuinely
unspecified** in any phase-1 stage.

The reviewer's resolution shape: retarget the
pointer to "future C-backend spec," because the
algorithm itself is C-backend territory under the
README §2 "Dual backend" rule (each backend's
codegen path owns its target-specific lowering
rules; the `<target>-backend.md` co-equal merger
artifacts per §5.9 rule 7 are the algorithm's
home). Phase 1 commits only to the two properties
the IR-side schema requires of `emission_name`:
**determinism** (the same `(item.def_path,
generic_args)` always produces the same name) and
**uniqueness across `instances`** (no two
`Instance`s collide). Both properties are
already preserved by `(DefPath, ArgsKey)` content
+ the §9.3 instance-interning-key contract; no
further spec text is needed at phase 1.

**Options considered.**

- (A) Apply the verbatim retarget per Finding #3:
  drop "the construction rule is pinned in stage
  13 (codegen)" and substitute the phase-1
  determinism + uniqueness commitment + C-backend-
  forward-pointer wording. No D-tag flip on any
  prior decision.
- (B) Pin the C-identifier construction algorithm
  in stage 14 itself, completing the spec gap
  rather than forwarding to a future stage.
- (C) Pin the algorithm in `generic-c-backend.md`
  (the phase-1 backend artifact introduced by
  D-14-10) within stage 14.

**Decision.** (A) -- apply the verbatim retarget.

**Reasoning.** The directive is fully prescribed
by `expectations.md` Finding #3, which already
gives the verbatim replacement text. There is no
design choice under-determined by the directive;
no sub-decisions need allocation. Three
considerations weigh against (B) and (C):

1. **C-backend territory under README §2 / §5.9
   rule 7.** Per the §5.9 rule 7 amendment
   (introduced by D-14-10), backend-codegen spec
   content (per-`Ty` C typedef tables, fat-pointer
   C struct shapes, intrinsic C-emission rules,
   per-target call shapes for the diagnostic-
   logging surface, name-mangling, ...) lives in
   target-specific `<target>-backend.md` artifacts
   co-equal with `spec.md` and `pretty-printer.md`.
   The C-identifier construction algorithm is a
   straight match for that category. Pinning it in
   `spec.md` (option B's home) would violate the
   producer-side / consumer-side split that §5.9
   rule 7 just codified.

2. **`generic-c-backend.md` is mid-stage and
   carve-out scope (D-14-10).** Option (C) pins
   the algorithm in `generic-c-backend.md` within
   stage 14. But D-14-10's locked scope is the
   _companion-file carve-out_ itself, not a
   commitment to land every C-backend
   algorithm before stage 14 closes. Phase-1
   `generic-c-backend.md` ships hosted-userspace
   integer-type conventions + `main()` + libc
   wiring + intrinsic C-emission contracts for the
   diagnostic-logging surface; the C-identifier
   construction algorithm is implementation-sweep
   territory (sweep #1 follows stage-14 close per
   README §2 "Spec before implementation"
   amendment). Pinning the algorithm here would
   re-expand stage 14 against the carve-out scope
   D-14-10 just locked.

3. **No driver fixture demands the algorithm
   yet.** Phase-1 fixtures are diagnostic-logging
   smoke tests with simple ADT shapes; their
   emission names land cleanly under any
   reasonable C-identifier scheme. The
   algorithm's load-bearing weight (collision
   resistance for monomorphized generics across
   `core` lang-items + driver `struct`s) only
   shows up under the phase-2 device-pressure
   fixtures (phase 3). Premature pinning
   without that pressure risks baking in shapes
   that need revision under phase-3 evidence
   per README §2 "spec / impl don't interleave"
   reasoning.

(A) is the smallest-scope option preserving all
three considerations -- pure forward-pointer
hygiene with no claim that the algorithm is
specified anywhere yet.

**Methodology rules invoked.**

- **README §2 "Dual backend".** Each backend's
  codegen path is a separate artifact; the
  C-identifier construction algorithm belongs in
  the backend artifact, not in the IR-side
  `spec.md`.
- **README §5.9 rule 7 (Backend-codegen spec lives
  in `<target>-backend.md`).** Forward pointer
  from `spec.md` to `<target>-backend.md` is the
  correct cross-reference shape.
- **README §2 "Spec before implementation".**
  Phase 1 spec carries no implementation-shaped
  prose for content that has no driver-fixture
  pressure yet; algorithm-shaped clarifications
  surface through the implementation sweep + phase
  3 device pressure.
- **README §2 "Forward expectations live in
  `stage-N/expectations.md`".** Finding #3
  authored at `stage-14/expectations.md`; this
  cluster lands it under the standard inbound-
  checklist discipline.

**Prior-art evidence.** No fresh rustc evidence
pass is required for this cluster. The directive
is prescriptive; the rustc machinery for
mangled-symbol construction
(`rustc_symbol_mangling::symbol_name`) is consulted
by `tcx.symbol_name(MonoItem::*)` for the
`Function / Static / Const` arms (already locked
at §9.2 paragraph 2 / line 2446-2447), and that
path is unchanged. The `Struct / Enum / Union`
arms have no rustc symbol (type-definition
instances are not callable items rustc itself
emits), so the bridge constructs the name; the
algorithm shape is C-backend territory.

`rustc-references.md` adds no new section --
existing entries already cover the `symbol_name`
codepath used by the callable arms; the
type-instance bridge-construction algorithm has no
rustc precedent because rustc has no equivalent
artifact.

**Cross-stage friction.** **None.** D-14-8 is
pure spec hygiene: no prior decision's locked
content is revised. Specifically:

- **Stage-9 inherited §9.2 `emission_name` text.**
  Field declaration at §9.2
  (`pub emission_name: StringId`), the canonical
  F4-2 instance-interning-key ordering at §9.3,
  the `(item.def_path, generic_args)` derivation
  basis, and the uniqueness contract are all
  unchanged by D-14-8. The retarget changes only
  the closing clause of paragraph 3's
  `Struct / Enum / Union` bullet. (D-12-2.4 folded
  `#[link_name]` resolution into `emission_name`
  for the callable arms; that fold is unchanged.)
- **Stage 13 D-13-9 spec-split.** D-13-9 already
  reframed stage 13 as the pretty-printer spec;
  the stale forward pointer is hygiene against
  that supersession, not a revision of D-13-9.
- **Stage 14 D-14-10 generic-C backend carve-
  out.** The retarget mentions "C-backend
  spec" as the algorithm's future home but does
  not commit to landing the algorithm in
  `generic-c-backend.md` within stage 14 (per
  reasoning point 2 above).

**No D-tag flip** at merger close. The pending
merger-time flip queue stays at five entries:
D-9-10 (D-14-2.5), D-13-6 (D-14-4.9), D-12-1.3 +
D-9-9 + D-13-4.3 (D-14-7.9). D-14-8 contributes
no flips.

**Spec contributions.**

- `spec-snapshot.md` §9.2 paragraph 3 (the
  `Struct / Enum / Union` `emission_name`
  bridge-construction-rule bullet, closing
  clause). The earlier two sentences of the bullet
  -- "bridge-constructed name derived
  deterministically from the rendered `DefPath`
  plus the canonical `ArgsKey` (§12.3), encoded
  into a C-identifier-safe form. Type-definition
  instances have no rustc symbol;" -- are
  unchanged.

**Worked example (before / after).**

Before (the stale clause, the only line edited):
```
   form. Type-definition instances have no rustc symbol; the
    construction rule is pinned in stage 13 (codegen).
```

After (the retargeted clause):
```
   form. Type-definition instances have no rustc symbol;
    phase-1 commits to determinism + uniqueness only; the full
    normative algorithm (escaping for C keywords / non-ASCII /
    leading digits / double underscores, hash-suffix collision-
    resistance scheme, length cap and maximum-name fallback) is
    the C-backend spec's responsibility and is out of phase-1
    scope.
```

The retargeted clause makes three claims:
1. Phase 1 commits to determinism + uniqueness
   (the two `emission_name` invariants the IR-side
   schema requires).
2. The full normative algorithm enumerates the
   substantive concerns (C-keyword escaping,
   non-ASCII handling, leading-digit handling,
   double-underscore handling, hash-suffix
   collision resistance, length cap with maximum-
   name fallback). Enumerating them establishes
   the surface area phase 1 is deferring rather
   than leaving the deferral vague.
3. The C-backend spec is the algorithm's home.
   This forward-points to the phase-1
   `generic-c-backend.md` and to each phase-2
   `<target>-backend.md` (per the §5.9 rule 7 dual-
   backend principle), without committing to a
   specific binding deadline.

**Closure check.** D-14-8 Phase A LOCKED 2026-05-13
with no sub-decisions; Phase B closed at R2 APPROVE /
Lockability:yes / Close-out:YES (2026-05-13) after a
2-round trajectory (R1 MINOR 2 findings: tracker
line-number wording surfaces + placeholder D-9-?
in the cross-stage friction section, both spec-
text consistency at audit + tracker layers; R2
APPROVE 0 findings after surgical wording fixes
landed). See `review-log.md` D-14-8 cluster
section for the round-by-round detail. **All
Phase A content unchanged through Phase B** -- the
two R1 findings were closed at the tracker /
audit-text layer only, leaving the §9.2 paragraph
3 spec edit + reasoning + methodology + cross-
stage friction conclusion + worked example
intact. Cluster closes Phase B.

---

### D-14-9 -- Specimen 70 split (external-review Finding #4) [LOCKED -- Phase A 2026-05-13; Phase B closed at R2 APPROVE / Lockability:yes / Close-out:YES (2026-05-13)]

**Question.** External-review Finding #4
(`expectations.md` Finding #4; original review at
`/Users/nyengele/Downloads/regseq-dsl-spec-review.md`)
flagged a structural inconsistency in
`pretty-printer-snapshot.md` §9.9 Specimen 70.
Specimen 70 ("Function with full attribute stress")
declared
`kind: ItemKind::Function, ...,
opaque_handle_eligible: true`
and rendered `#[opaque_handle]` above
`unsafe fn bsp::reset`, but `spec-snapshot.md` §8.0
invariant 19 ("`Item.opaque_handle_eligible`
consistency (non-generic only)") restricts
`opaque_handle_eligible: true` to
`Item.kind == ItemKind::Struct` (non-generic,
`#[repr(C)]`, ZST). The specimen exercised a state
the bridge will never produce -- a Function Item
carrying the eligibility flag is structurally
impossible.

The reviewer's resolution shape: split Specimen 70
into (i) a clean Function attribute-stress
specimen with `opaque_handle_eligible: false` (the
only valid Function-Item state) and (ii) a new
Specimen 70b carrying
`opaque_handle_eligible: true` on a Struct Item
(the only `ItemKind` that can structurally hold
the flag). The split preserves coverage of both
the Function attribute-stress shape (no_mangle +
inline + link_section + unsafe combined) and the
bridge-outcome `#[opaque_handle]` rendering shape,
without exercising the impossible Function +
opaque_handle combination.

D-14-9 is the cluster that lands the resolution.
It is the **fourth external-review Finding cluster**
of stage 14 (after D-14-1 absorbed Finding #1;
D-14-7 closed Finding #2; D-14-8 closed Finding #3)
and the **third surgical-fix cluster**. It is the
**second of two pretty-printer-spec hygiene
clusters** in stage 14 (D-14-7's Specimen 70 +
per-flag table edits being the first); D-14-9 is
the lighter of the two -- two specimen edits and a
removed forward-pointer parenthetical, with no
structural-spec change.

**Options considered.**

- (A) Apply the verbatim split per Finding #4:
  downshift Specimen 70's `opaque_handle_eligible`
  to `false` and drop the `#[opaque_handle]` output
  line, then add a new Specimen 70b on a Struct
  Item carrying `opaque_handle_eligible: true`.
  No D-tag flip on D-13-7. Specimen count drifts
  ~75 -> ~76 via the 70b suffix.
- (B) Renumber every specimen 70-onwards
  (Specimen 70 stays as the Struct-with-
  opaque-handle stress; old 71 -> 70; old 72 -> 71;
  ...). Avoids the 70b suffix at the cost of a
  ~30-specimen renumber churn.
- (C) Keep the impossible Function + opaque_handle
  combination as a specimen but annotate it as
  "documentation-only, the bridge cannot produce
  this state." Preserves specimen continuity at the
  cost of polluting the specimen-coverage rubric
  with bridge-impossible inputs.

**Decision.** (A) -- apply the verbatim split.

**Reasoning.** The directive is fully prescribed by
`expectations.md` Finding #4, which gives the
target shapes for both the modified Specimen 70 and
the new Specimen 70b. There is no design choice
under-determined by the directive; no sub-decisions
need allocation. Three considerations weigh against
(B) and (C):

1. **Renumber churn unnecessary per D-13-7.2.**
   D-13-7.2 (specimen-numbering policy) is explicit
   that policy-bucket exceptions like the 70b
   suffix are acceptable when the alternative is a
   wide renumber affecting unrelated specimens.
   The 70 -> 70b -> 71 sequence is gap-free and
   does not break any reader expectations.

2. **Bridge-impossible inputs damage the rubric
   (option C).** D-13-7's specimen-coverage policy
   demands the rubric reflect realisable bridge
   outputs only; "documentation-only" inputs
   establish a precedent for specimens that test
   policy rather than bridge behaviour, which
   undermines the rubric's value as a coverage
   ground-truth.

3. **The Finding #4 split is bridge-tractable.**
   Both Specimen 70 (clean Function attribute-
   stress) and Specimen 70b (Struct with the
   bridge-attested opaque-handle eligibility) are
   shapes the bridge will produce for plausible
   real-world drivers. The cross-coverage is
   preserved without the impossible combination.

(A) is the smallest-scope option preserving all
three considerations -- pure specimen-content
hygiene with no structural-spec change.

**Methodology rules invoked.**

- **README §5.9 rule 6 (Pretty-printer spec lives
  in `pretty-printer.md`).** Specimen content is
  pretty-printer-spec territory; the edit lands in
  `pretty-printer-snapshot.md` (the working file)
  and integrates into top-level `pretty-printer.md`
  at merger close.
- **D-13-7 + D-13-7.2 (specimen coverage +
  numbering policy).** D-13-7's per-arm coverage
  rubric is the spec D-14-9 honors (every
  specimen exercises a realisable bridge output);
  D-13-7.2's policy-bucket-exception spirit
  authorises the 70b suffix.
- **§8.0 invariant 19 (Struct-only opaque-handle
  eligibility).** The structural constraint the
  Function specimen violated; the split honors it.
- **README §2 "Forward expectations live in
  `stage-N/expectations.md`".** Finding #4 authored
  at `stage-14/expectations.md`; this cluster lands
  it under the standard inbound-checklist
  discipline.

**Prior-art evidence.** No fresh rustc evidence
pass required. The directive is prescriptive; the
opaque-handle eligibility constraint at §8.0
invariant 19 is already locked at stage-12 D-12-1.3
(the per-Item bridge-computed `opaque_handle_eligible`
field) + the §8.0 constraint clauses. The Struct
shape for Specimen 70b mirrors the baseline Struct
rendering shape from Specimen 73 at §9.9, while
using `pl011::Handle` (Specimen 73 uses
`pl011::PL011`), `opaque_handle_eligible: true`,
and the resulting `#[opaque_handle]` rendered line.

`rustc-references.md` adds no new section --
existing entries already cover opaque-handle
recognition / eligibility computation; D-14-9 is
specimen-content hygiene only.

**Cross-stage friction.** **None.** D-14-9 is pure
pretty-printer-spec hygiene: no prior decision's
locked content is revised. Specifically:

- **Stage-13 D-13-7 (specimen coverage rubric).**
  Decision text (~75 specimens, per-arm coverage
  policy, policy-bucket exceptions, numbering
  policy) is not revised; only the content of
  Specimen 70 is corrected and Specimen 70b added.
  Stage-13 D-13-7 stays LOCKED.
- **Stage-13 D-13-4.3 (per-flag rendering table).**
  The per-flag table row for `opaque_handle_eligible
  = true` -> `#[opaque_handle]` is unchanged;
  Specimen 70b exercises this row, Specimen 70
  exercises the
  `opaque_handle_eligible = false -> suppressed`
  row. No D-13-4.3 revision.
- **Stage-12 D-12-1.3 (typed Item-level fields
  including `opaque_handle_eligible`).** The field
  itself is unchanged (`opaque_handle_eligible:
  bool` per §8.0 line 2190); D-14-7 reopened
  D-12-1.3 separately for the `is_naked` field
  removal, which D-14-9 does not touch.
- **Stage-14 D-14-7 cross-finding parenthetical.**
  D-14-7's Specimen 70 edit added a parenthetical
  forward-pointing to D-14-9 ("...remains
  structurally ineligible per `spec.md` §8.0 --
  that bridge-outcome constraint is the target of
  stage-14 D-14-9 / external-review Finding #4...").
  D-14-9 lands the fix, so the parenthetical is now
  removed from the working surface; no D-14-7
  revision (the parenthetical was forward-pointing
  hygiene, not load-bearing).

**No D-tag flip** at merger close. The pending
merger-time flip queue stays at five entries:
D-9-10 (D-14-2.5), D-13-6 (D-14-4.9), D-12-1.3 +
D-9-9 + D-13-4.3 (D-14-7.9). D-14-9 contributes
no flips.

**Spec contributions.**

- `pretty-printer-snapshot.md` §9.9 Specimen 70
  (input shape: `opaque_handle_eligible: false`;
  output: drop `#[opaque_handle]` line; no other
  changes -- the attribute-stress shape no_mangle
  + inline(always) + link_section + unsafe stays).
- `pretty-printer-snapshot.md` §9.9 Specimen 70b
  (new specimen; input: `Item { kind:
  ItemKind::Struct, def_path: <pl011::Handle>,
  ..., linkage: Default, ..., is_phantom_data:
  false, is_unsafe_cell: false, is_manually_drop:
  false, opaque_handle_eligible: true }`; output:
  `#[opaque_handle]` above `struct pl011::Handle`).
- The D-14-7 forward-pointer parenthetical at the
  tail of the Specimen 70 entry is removed (since
  the fix has now landed).
- Specimen count drifts ~75 -> ~76 via the 70b
  suffix. Gap-free 70 -> 70b -> 71 sequence.

**Worked example (before / after).**

Before (Specimen 70 input, the problematic line):
```
opaque_handle_eligible: true, def_path: <bsp::reset> }`.
```

Before (Specimen 70 output, the bridge-impossible line):
```
#[opaque_handle]
```

After (Specimen 70 input -- corrected eligibility):
```
opaque_handle_eligible: false, def_path: <bsp::reset> }`.
```

After (Specimen 70 output -- bridge-impossible line removed; rest unchanged):
```text
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn bsp::reset
```

After (Specimen 70b -- new, preserves bridge-outcome rendering coverage):
```
Input: `Item { kind: ItemKind::Struct, def_path: <pl011::Handle>, ...,
linkage: Default, inline_hint: None, link_section: None, is_unsafe: false,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: true }`.
Output:
    #[opaque_handle]
    struct pl011::Handle
```

Specimen 70b exercises the
`opaque_handle_eligible: true` bridge-outcome on
the only `ItemKind` that can structurally carry
it per §8.0 invariant 19 (`ItemKind::Struct`,
non-generic, `#[repr(C)]`, ZST). The
`#[opaque_handle]` line emission honors §5.3's
per-flag table row
(`opaque_handle_eligible = true` ->
`#[opaque_handle]`).

**Closure check.** D-14-9 Phase A LOCKED 2026-05-13
with no sub-decisions; Phase B closed at R2 APPROVE /
Lockability:yes / Close-out:YES (2026-05-13) after
a 2-round trajectory (R1 MINOR 2 findings: stale
"three clusters" wording in the discussions.md
preamble cross-cluster-handoff sentence + audit
overstatement of how Specimen 70b differs from
Specimen 73 -- both spec-text consistency at the
audit + preamble layers; R2 APPROVE 0 findings
after surgical wording fixes landed). See
`review-log.md` D-14-9 cluster section for the
round-by-round detail. **All Phase A content
unchanged through Phase B** -- both R1 findings
were closed at the preamble / audit-text layer
only, leaving the §9.9 Specimen 70 + 70b edits +
reasoning + methodology + cross-stage friction
conclusion + worked example intact. Cluster closes
Phase B.

---

### `core::ffi::c_void` rejection enforcement (no D-tag; hygiene cluster) [LOCKED -- Phase A 2026-05-13; Phase B closed at R3 APPROVE / Lockability:yes / Close-out:YES (2026-05-13)]

**Question.** Stage-14 `expectations.md` and
`memory.md` enumerate
`core::ffi::c_void` rejection enforcement as the
final cluster within stage 14, alongside the four
external-review Finding clusters. Unlike the
Finding clusters, this cluster is not driven by an
external reviewer's gap report -- it is an
inherited expectation from stage 3 D16 (which
locked `core::ffi::c_void` as a v1 DSL rejection
without yet writing the enforcement narrative into
the §15 bridge translation rules) plus stage 3
D1.4 (which recommended the opaque-struct pattern
as the driver-author workaround for the rejection).
The question for stage 14: **is the rejection
already specified at the spec-text layer (no work
needed), or does the enforcement model need
additional spec text?**

D-14-7 + D-14-8 + D-14-9 closed the external-
review Finding clusters; this cluster closes the
fourth and final stage-14 surface, completing the
phase-1 spec.

**Evidence basis.** Existing §15 / §15.1 / §15.0 /
Appendix A.1 state verified at Phase A drafting:

1. **§15.0 dispatch-ordering rule (algorithmic
   enforcement).** §15.0's "Special-ADT dispatch
   order" subsection establishes the normative
   ordering: when the bridge encounters
   `TyKind::Adt(adt_def, args)`, it checks the
   closed special-ADT set (Lang-item flagged +
   DSL-rejected) **before** generic
   struct/enum/union translation. `c_void` appears
   in the DSL-rejected ADTs bullet with the
   recognition mechanism
   `tcx.is_lang_item(LangItem::CVoid)`. §15.0's
   closing sentence makes the ordering normative
   ("implementation must check the special set
   first, otherwise generic translation accidentally
   admits rejected core types"). This is the
   algorithmic enforcement gate -- not just a
   specification of *what* is rejected, but *where
   in the pipeline* the rejection is path-complete.

2. **§15.1 rejection table (enumerated rejections).**
   The §15.1 rejection table carries the c_void
   row: "`core::ffi::c_void` | type translation |
   recognised via `tcx.is_lang_item(LangItem::CVoid)`;
   opaque-struct pattern recommended". The row
   enumerates the rejection but does not by itself
   tie the row to the §15.0 algorithmic gate,
   leaving the enforcement model implicit.

3. **Appendix A.1 worked example (driver-author
   guidance).** Appendix A.1 walks the
   `*mut c_void` -> `*mut OpaqueCtx` rewrite
   end-to-end, showing the bridge diagnostic + the
   structurally-equivalent driver rewrite using
   `#[repr(C)] struct OpaqueCtx { _private:
   [u8; 0] }`. The Appendix establishes what the
   driver author sees + what they should write
   instead.

4. **Two opaque-struct-pattern references without
   a formal definition.** §15.1 rows for
   `Foreign(_)` and `core::ffi::c_void` both say
   "opaque-struct pattern recommended" but the
   pattern itself is not defined inline in §15 --
   it is only shown in Appendix A.1's worked
   example. A reader of §15 alone cannot resolve
   what "opaque-struct pattern" means without
   chasing Appendix A.1.

The evidence basis surfaces **two enforcement-model
gaps**, both in §15.1's introduction:

- (a) The §15.1 table reader cannot tell *which*
  rows in the "type translation" phase fall under
  the §15.0 dispatch-ordering gate (path-complete
  rejection) versus rows enforced at other check
  points (HRTB-related per §17.1, C-ABI
  admissibility per §14.5, etc.). The gate's
  applicability needs to be made explicit.
- (b) The opaque-struct pattern is referenced
  twice but never defined in §15. The shape
  (`#[repr(C)]` ZST struct with a single private
  zero-length-array field) deserves inline
  definition so §15 stands alone without forcing
  the reader to chase Appendix A.1.

**Options considered.**

- (A) Add two introductory paragraphs to §15.1
  (after the four-category narrative, before the
  table): an "**Enforcement model**" paragraph
  naming which rows the §15.0 gate enforces and
  pointing to the gate's normativity; and an
  "**Opaque-struct pattern**" paragraph defining
  the pattern inline + forward-referencing
  Appendix A.1's end-to-end rewrite. No D-tag
  allocated; no D-tag flip.
- (B) Define the opaque-struct pattern in a new
  top-level §14.6 subsection or §8.X subsection
  (since it's a recommended driver-author idiom
  that interacts with §14.5 C-ABI admissibility +
  §8.0 invariant 19 opaque-handle eligibility).
  Heavier-weight: introduces new structural
  surface for an idiom that ultimately is just
  documentation of a specific Rust struct shape.
- (C) Leave the spec as-is and treat the cluster
  as verification only (no spec edits); add a
  discussions.md audit entry confirming the
  existing §15.0 + §15.1 + Appendix A.1 surface
  is sufficient. Risks future readers chasing
  "where is the enforcement model documented?"
  without finding the integrated answer.

**Decision.** (A) -- add the two introductory
paragraphs.

**Reasoning.** Option (A) is the smallest-scope
edit that closes both enforcement-model gaps. The
§15.1 introduction is the natural home: the reader
arrives at the rejection table needing to know how
the rejections are enforced + what the recommended
pattern looks like, and a brief two-paragraph
preface gives them both without restructuring the
section. Option (B)'s subsection-level definition
is over-engineered for a pattern that fits in 4
sentences. Option (C)'s "verification only" framing
ignores the genuine reader-experience problem the
existing surfaces leave: the §15.0 ordering rule
is in one place, the rejection rows in another,
the worked example in an Appendix, and the
opaque-struct pattern definition nowhere.

**Methodology rules invoked.**

- **README §5.9 rule 5 (no D-tag inline
  references).** The new paragraphs reference
  §15.0 / §15.1 / §14.5 / §8.0 / Appendix A.1
  inline by section number but do not cite
  stage-3 D16 / D1.4 inline -- those attributions
  live in this audit entry, not in `spec.md`.
- **README §2 "C semantics with information
  preservation".** The opaque-struct pattern is
  the v1 idiom for an opaque pointee type that
  the bridge admits without special-casing; the
  inline definition documents the C-semantic
  outcome (incomplete-struct pointer at the C-ABI
  boundary).
- **README §2 "No defensive fallbacks".** The
  enforcement model wording does not introduce
  any new fallback paths -- it documents the
  existing §15.0 ordering rule's role.
- **README §2 "Forward expectations live in
  `stage-N/expectations.md`".** This cluster
  resolves an inherited stage-3 expectation
  (D16's rejection specification + D1.4's
  opaque-struct recommendation) that the stage-3
  authors deferred to a later stage's enforcement
  narrative work.

**Prior-art evidence.** No fresh rustc evidence
pass required. The recognition mechanism
(`tcx.is_lang_item(LangItem::CVoid)`) is already
specified at §15.0 and §15.1; the structural
shape of the opaque-struct pattern
(`#[repr(C)] struct T { _private: [u8; 0] }`) is
a long-standing FFI idiom in the Rust + C
interoperability community + Appendix A.1.

`rustc-references.md` adds no new section --
`LangItem::CVoid` is already mentioned in §15.0
narrative; no new file:line landmarks needed.

**Cross-stage friction.** **None.** This cluster
is pure spec hygiene: no prior decision's locked
content is revised. Specifically:

- **Stage-3 D16 (`core::ffi::c_void` v1
  rejection).** The rejection itself is unchanged;
  the new paragraphs document the enforcement
  mechanism + driver-author rewrite path.
- **Stage-3 D1.4 (opaque-struct pattern
  recommendation).** The recommendation is
  unchanged; the new paragraphs lift the inline
  definition from "implicit in Appendix A.1" to
  "explicit in §15.1 introduction".
- **Stage-9 D-9-9 + §15.0 / §15.1.** The
  rejection table architecture + dispatch-
  ordering rule are unchanged; the new paragraphs
  document the existing ordering rule's
  applicability to the §15.1 table rows.
- **§14.5 (C ABI admissibility) and §8.0
  invariant 19 (opaque-handle eligibility).**
  The C-ABI admissibility check + opaque-handle
  eligibility rule are unchanged; the new
  paragraphs forward-reference them as the
  downstream lowering context for the opaque-
  struct pattern.

**No D-tag flip** at merger close. The pending
merger-time flip queue stays at five entries:
D-9-10 (D-14-2.5), D-13-6 (D-14-4.9), D-12-1.3 +
D-9-9 + D-13-4.3 (D-14-7.9). This cluster
contributes no flips. **No D-tag allocated for
this cluster** -- stage-3 D16 already covers the
decision-level commitment.

**Spec contributions.**

- `spec-snapshot.md` §15.1 introduction: new
  "**Enforcement model**" paragraph (after the
  four-category narrative paragraph, before the
  rejection table) naming the §15.0 dispatch-
  ordering gate as the algorithmic enforcement
  for special-ADT rejections at the
  "type translation" phase, enumerating which
  rejection categories the gate intercepts, and
  noting that non-ADT type-translation
  rejections + Item-attribute + `PlaceElem`
  rows are enforced at their owning sections'
  check points.
- `spec-snapshot.md` §15.1 introduction: new
  "**Opaque-struct pattern**" paragraph defining
  the pattern inline
  (`#[repr(C)] struct OpaqueCtx { _private:
  [u8; 0] }`), explaining the admittance path
  via the ordinary `Ty::Struct` translation, the
  downstream C-ABI lowering to an
  incomplete-struct pointer per §14.5 + §8.0
  invariant 19, and forward-referencing
  Appendix A.1's `*mut c_void` -> `*mut
  OpaqueCtx` end-to-end rewrite.
- No other surfaces touched: §15.0 ordering rule
  + §15.1 c_void row + Appendix A.1 worked
  example all unchanged.

**Worked example (before / after).**

Before (§15.1 introduction):
- Four-category narrative paragraph naming the
  rejection-table categories (non-rigid TyKind,
  rigid-but-DSL-rejected, Item-attribute,
  closed PlaceElem vocabulary).
- (No enforcement-model paragraph.)
- (No opaque-struct-pattern definition.)
- Rejection table.

After (§15.1 introduction):
- Four-category narrative paragraph (unchanged).
- **Enforcement model** paragraph: special-ADT
  rejections at the "type translation" phase are
  enforced by §15.0's dispatch-ordering rule;
  non-ADT rejections at the same phase by their
  owning sections' check points; Item-attribute
  + PlaceElem rows at their respective walks.
- **Opaque-struct pattern** paragraph:
  `#[repr(C)]` ZST struct with a single private
  zero-length-array field; admitted via ordinary
  `Ty::Struct` path; lowers to incomplete-struct
  pointer at C-ABI boundary per §14.5 + §8.0
  invariant 19; Appendix A.1 walks the
  `*mut c_void` -> `*mut OpaqueCtx` rewrite.
- Rejection table (unchanged).

**Closure check.** Cluster Phase A LOCKED
2026-05-13 with no sub-decisions and no D-tag;
Phase B closed at R3 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13) after a 3-round
trajectory (R1 MINOR 3 findings: Coroutine family
double-classification in the new §15.1 enforcement
paragraph + stale line-number citations at six
audit + tracker sites + dangling "Phase B."
fragment at the audit closure check; R2 MINOR 1
finding: missed stale line number at `memory.md`
-- same class as R1-#2 but at a surface not
touched in the R1 fix sweep; R3 APPROVE 0
findings). Total: 4 findings closed across R1-R2
(0B+3M+1N; 3 rounds). See `review-log.md` cluster
section for the round-by-round detail. **All
Phase A content unchanged through Phase B** -- the
four R1+R2 findings were all closed at the
audit-text + tracker-wording + spec-introduction
text layer; the structural directive (two new
§15.1 introduction paragraphs naming the §15.0
gate + defining the opaque-struct pattern) is
intact. Cluster closes Phase B and **closes the
fourth and final stage-14 spec surface**;
remaining stage-14 work is the per-stage cross-
cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
then merger close per README §5.5 7-step contract.
