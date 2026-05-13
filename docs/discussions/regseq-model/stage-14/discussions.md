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

This file covers the stage-14 clusters in the order they
were drafted:

- **D-14-1 + D-14-3 cluster** [LOCKED 2026-05-12]
  (Q7 intrinsic recognition mechanism + storage;
  DomainAttr sibling-table architecture).
- **D-14-2 cluster** [LOCKED 2026-05-13]
  (diagnostic-logging vocabulary + bridge contract;
  9 sub-decisions D-14-2.1 through D-14-2.9).
- **D-14-4 cluster** [DRAFTING; opened 2026-05-13]
  (`render_const` composer renderer per-arm specs;
  12 sub-decisions D-14-4.1 through D-14-4.12 allocated;
  D-14-4.1 LOCKED at Phase A as the first sub-decision).

Remaining clusters (D-14-5 `render_instance`; D-14-6
top-level `render`; D-14-7 `#[naked]` rejection per
Finding #2; D-14-8 stale-forward-pointer retarget per
Finding #3; D-14-9 Specimen 70 split per Finding #4;
`core::ffi::c_void` rejection enforcement) land as
separate entries in later passes.

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

### D-14-4 -- `render_const` composer renderer

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

**Decision.** Sub-decisions land iteratively as Phase
A drafting proceeds. **D-14-4.1 (Aspect A1, API
signature) locks first** as the load-bearing gate that
constrains D-14-4.3 / D-14-4.4 / D-14-4.5 / D-14-4.6 /
D-14-4.7 / D-14-4.8 (the consumer-Ty-dependent arms)
and D-14-4.9 (type-context threading). The remaining
11 sub-decisions get drafted in the order pinned by
`status.md` "Next action" Phase A plan:

1. ~~D-14-4.1~~ (API signature) -- LOCKED 2026-05-13.
2. D-14-4.2 (scalar variants) -- next; locks the
   underscore-decimal convention; resolves specimen
   inconsistency.
3. D-14-4.10 / D-14-4.11 / D-14-4.12 (policy-shaped
   sub-decisions: sentinel collapsing, hex admission,
   float non-finite).
4. D-14-4.3 / D-14-4.7 / D-14-4.8 (single-variant
   simple shapes: ZST, NullPtr, Reloc).
5. D-14-4.4 / D-14-4.5 / D-14-4.6 (multi-arm dispatch:
   Aggregate, EnumValue, Bytes -- Charon / mrustc
   inline consultation triggered here if dispatch
   ambiguity surfaces).
6. D-14-4.9 (type-context threading) -- recursion-shape
   binding D-14-4.4 / D-14-4.5 / D-14-4.6.

The cluster's main `D-14-4` `[LOCKED]` status applies
once every sub-decision under it closes.

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

**Cross-stage friction.** None anticipated at cluster
open. Stage-13 D-13-6 (per-arm rendering for body-level
constructs including `Operand::Constant(ConstId)`) is
already specified to render via `<render_const(id)>`
without `const ` prefix (per `pretty-printer-snapshot.md`
§7.2 line 653 + specimen line 1140-1141). The
operand-level wrapper's deliberate divergence from
rustc's `Display for ConstOperand` (which prefixes
`const `) is **stage-13 territory**; D-14-4 honors that
divergence by producing the bare value form. No D-13-6
reopening anticipated.

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

---

## 2. Cross-cutting decisions recap

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
- **D-14-4 cluster** -- DRAFTING (opened 2026-05-13).
  12 sub-decisions D-14-4.1 through D-14-4.12
  allocated; **D-14-4.1** (API signature
  `render_const(ConstId, TyId) -> String`),
  **D-14-4.2** (scalar variants Bool / Char / Int /
  Uint / Float with rustc-mirrored underscore-decimal
  convention), **D-14-4.10** (`Int` / `Uint` sentinel
  collapsing matching rustc: 12 signed MIN/MAX + 6
  unsigned MAX entries; target-pointer-width-independent
  for `Isize`/`Usize`), **D-14-4.11** (integer hex
  form admission: decimal-only for v1; no
  `PrettyPrintOptions::integer_hex` toggle; future-
  extension boundary recorded but not pre-allocated),
  and **D-14-4.12** (Float infinity / NaN
  representation: rustc-mirrored underscore-broken
  `inf_f<width>` / `-inf_f<width>` / `NaN_f<width>`
  form; NaN bit-pattern collapsing matches rustc; all
  4 FloatKind widths covered; negative zero is finite
  not non-finite) LOCKED at Phase A. **All three
  policy-shaped sub-aspects of D-14-4.2 now closed.**
  Remaining 7 sub-decisions (D-14-4.3 through
  D-14-4.9 -- the type-context-dependent variants ZST
  / Aggregate / EnumValue / Bytes / NullPtr / Reloc +
  type-context threading) open for subsequent Phase A
  drafting passes per `status.md` "Next action" plan.
  Phase B Codex review pending cluster close.

Remaining stage-14 clusters (D-14-5 `render_instance`;
D-14-6 top-level `render`; D-14-7 / D-14-8 / D-14-9
external-review Findings; `core::ffi::c_void` rejection
enforcement) are open for later passes within stage 14.

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
