# regseq-model-74: DomainAttrTables, AttrData, IntrinsicKind, LogLevel, FieldPath (P1a-DS-09)

## Description

Implement the DomainAttr sibling-table data shapes that hold
structured DSL property attributes derived from the
`#[rseq::*]` tool namespace. Phase-1a's first concrete
domain-attribute family is the diagnostic-logging intrinsic
(`IntrinsicKind::Log(LogLevel)`); the data-shape surface
established here is the foundation phase-2 stages will append to.

This task lands the closed-but-append-only enums (`AttrData`,
`IntrinsicKind`, `LogLevel`), the keying type (`FieldPath`), the
sibling-table container (`DomainAttrTables`), and the
closed-vocabulary `IntrinsicKind::from_name` lookup that maps the
dash-form attribute NAME (`log-debug`, `log-info`, `log-warn`,
`log-error`) to the resolved `IntrinsicKind`. Per-family
duplicate invariants and the recognition codepath are out of
scope (validator-side / bridge-side respectively).

## Input Context

- `docs/discussions/regseq-model/spec.md` §3.6.1 (`DomainAttrTables`
  + `FieldPath`; `spec.md:1428-1473`)
- `docs/discussions/regseq-model/spec.md` §3.6.2 (`AttrData`;
  `spec.md:1474-1512`)
- `docs/discussions/regseq-model/spec.md` §3.6.3 (`IntrinsicKind`,
  `LogLevel`, `from_name`; `spec.md:1513-1564`)
- `docs/discussions/regseq-model/spec.md` §3.6.5 (Validation
  invariants -- referenced but **not** enforced here;
  `spec.md:1716-1769`)
- `docs/discussions/regseq-model/spec.md` §3.6.6 (Attribute-
  namespace storage rule; `spec.md:1770-1810`)
- `docs/discussions/regseq-model/spec.md` §3.6.7 (Per-family
  target-table mapping; `spec.md:1812-1834`)
- `docs/discussions/regseq-model/spec.md` §26.2 (Severity
  vocabulary + intrinsic NAME mapping for diag-logging)
- `crates/regseq-model/src/ids.rs` (`ItemId`, `Symbol`,
  `StringId`, `FieldIdx`, `VariantIdx`)
- `crates/regseq-model/src/source.rs` (`Span` -- the key for
  `span_attrs`)
- `crates/regseq-model/src/idx.rs` (`IndexVec` -- the backbone
  for `item_attrs`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-09 row)

## Implementation Notes

Add a new module `crates/regseq-model/src/domain_attr.rs`
(re-exported from `lib.rs`). The four types and the closed
lookup function below are the public surface.

### Public API -- DomainAttrTables and FieldPath

Per `spec.md:1428-1442`:

```rust
/// Sibling tables on `RegseqModel` for structured DSL property
/// attributes derived from the `#[rseq::*]` tool namespace.
/// Three sibling tables, one per source-level entity key kind.
pub struct DomainAttrTables {
    /// Attributes attached to source-level Items (functions,
    /// statics, consts, structs, enums, unions). Indexed by
    /// `ItemId`; sparse via empty `Vec`.
    pub item_attrs:  IndexVec<ItemId, Vec<AttrData>>,

    /// Attributes attached to fields of ADT Items. Indexed by
    /// `FieldPath` (the `(ItemId, VariantIdx, FieldIdx)`
    /// triple). Sparse via absence in the `HashMap`. First v1
    /// occupant arrives at stage 16 (`AttrData::Mmio`).
    pub field_attrs: HashMap<FieldPath, Vec<AttrData>>,

    /// Attributes attached to source-code Spans, for future use
    /// cases (diagnostic annotations, lint-style markers, etc.).
    /// Indexed by `Span`; sparse via absence. No v1 occupants.
    pub span_attrs:  HashMap<Span, Vec<AttrData>>,
}

/// Composite key for the field-attribute sibling table.
/// `variant` is `0` for Struct / Union; per-variant for Enum.
pub struct FieldPath {
    pub item:    ItemId,
    pub variant: VariantIdx,
    pub field:   FieldIdx,
}
```

### Public API -- AttrData (closed, append-only)

Per `spec.md:1474-1512`:

```rust
/// Closed, append-only discriminated union of per-domain
/// attribute payloads. Variant ordering is normative: stage 14
/// fixes `Intrinsic` at discriminant 0; phase-2 stages append
/// new variants at the end. Existing discriminants remain
/// immutable so serialized RegseqModels stay stable across
/// stage closures.
pub enum AttrData {
    Intrinsic(IntrinsicKind),        // discriminant 0; stage 14
    // phase-2 stages append at discriminant 1, 2, 3, ...:
    //   Mmio(MmioAttr)              -- stage 16
    //   Interrupt(InterruptAttr)    -- stage 17
    //   Atomic(AtomicAttr)          -- stage 18
}
```

### Public API -- IntrinsicKind, LogLevel, and from_name

Per `spec.md:1515-1551`:

```rust
/// Closed, append-only intrinsic-family taxonomy. One variant
/// per domain family; each variant carries a per-family sub-
/// enum of operations.
pub enum IntrinsicKind {
    Log(LogLevel),                  // discriminant 0; stage 14
    // phase-2 stages append at discriminant 1, 2, 3, ...:
    //   Mmio(MmioOp)                -- stage 16
    //   Interrupt(InterruptOp)      -- stage 17
    //   Atomic(AtomicOp)            -- stage 18
}

/// Diagnostic-logging severity carried by `IntrinsicKind::Log`.
pub enum LogLevel { Debug, Info, Warn, Error }

impl IntrinsicKind {
    /// Closed-vocabulary lookup mapping the attribute NAME
    /// `Symbol` (dash-form) to the resolved `IntrinsicKind`.
    /// Returns `None` for any name outside the closed set.
    pub fn from_name(name: Symbol) -> Option<Self> {
        match name.as_str() {
            "log-debug" => Some(IntrinsicKind::Log(LogLevel::Debug)),
            "log-info"  => Some(IntrinsicKind::Log(LogLevel::Info)),
            "log-warn"  => Some(IntrinsicKind::Log(LogLevel::Warn)),
            "log-error" => Some(IntrinsicKind::Log(LogLevel::Error)),
            // phase-2 stages append per-family arms here.
            _ => None,
        }
    }
}
```

### Conventions

- **Dash-form NAME mapping is normative.** The four NAME
  strings `"log-debug"`, `"log-info"`, `"log-warn"`,
  `"log-error"` per `spec.md:1542-1551` map to the four
  `LogLevel` variants. The Rust-side function items that
  carry the `#[rseq::intrinsic("...")]` attribute use the
  underscore form (`log_debug`, etc.) per `spec.md:1554-1564`;
  this divergence is intentional. NAMEs are taxonomic
  identifiers; Rust function names follow Rust's identifier
  convention. Do NOT collapse the two forms.
- **Append-only discriminant ordering.** `AttrData::Intrinsic`
  is variant index 0; future variants append at the end.
  `IntrinsicKind::Log` is variant index 0 within
  `IntrinsicKind`; future variants append. Use
  `#[repr(u8)]` if explicit discriminant pinning is needed
  (project convention). Document the closed-but-append-only
  contract in the type-level doc comment.
- **`Vec<AttrData>` ordering.** Bridge-walk-order deterministic
  per D-14-3.3 (`spec.md:1502-1512`). The validator does NOT
  re-sort; per-family duplicate invariants live elsewhere
  (P1a-VAL-03 at `spec.md:1748-1763`'s
  `I-DomainAttr-Intrinsic-Singleton`).
- **`item_attrs` backbone size.** Initialised by Phase 4 of the
  bridge construction pipeline as
  `IndexVec::from_elem(Vec::new(), model.items.len())` per
  `spec.md:1656-1665`. This task provides the type; the
  Phase-4 sizing convention is bridge-side
  (P1a-BR-01 / P1a-BR-08a).
- **`field_attrs` and `span_attrs` start empty.** Inserts are
  bridge-side per family (Phase 4 re-key from Phase-2 working
  caches per `spec.md:1660-1672`).
- **No `instance_attrs` table.** Per `spec.md:1462-1472`:
  Instance access to Item-level domain attributes goes through
  the §3.5.2 pull-through pattern; per-Instance state lives
  inline in `InstanceKind`. v1 commits to three sibling tables.
- **`FieldPath` for Struct / Union.** Per `spec.md:1438-1441`,
  `variant: VariantIdx` is `0` for Struct / Union; per-variant
  for Enum. The composite key is `(ItemId, VariantIdx,
  FieldIdx)`.
- **`from_name` is closed-vocabulary.** Returns `None` (not a
  fallback variant) for unknown NAMEs; the bridge side
  (P1a-BR-08a) translates `None` into
  `BridgeError::IntrinsicAttrUnknownName(name)`.
- **Serde**: `AttrData`, `IntrinsicKind`, `LogLevel`, `FieldPath`
  contain only `ItemId` / `VariantIdx` / `FieldIdx` (typed
  IDs; no `Symbol` / `StringId`); serde derives are admissible
  on these. `DomainAttrTables` contains `Span`-keyed
  `span_attrs`; `Span` is the §5.3 inline struct (no
  `Symbol` content) and is serde-friendly. **Caveat:**
  `IndexVec<I, T>` and `HashMap<K, V>` serde shapes follow
  the project's existing serde conventions; mirror what
  `crates/regseq-model/src/model.rs`'s sibling tables already
  do. Use `#[derive(Serialize, Deserialize)]` when the
  underlying containers support it; otherwise gate behind
  `#[serde(skip)]` per the brief's cross-cutting note and
  document the deferral.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on `AttrData`,
  `IntrinsicKind`, `LogLevel`, `FieldPath`. `Hash` on
  `FieldPath` is mandatory because it is a `HashMap` key.
  Derive `Clone, Debug, Default, PartialEq, Eq` on
  `DomainAttrTables` (the empty initial state is the natural
  `Default`).
- **Doc comments mandatory** on every public type, variant,
  field, and function per `SESSION.md` Coding Conventions.
- **No unicode symbols** in doc comments (project rule).

## Non-Goals

- **Validation invariants** in `spec.md:1716-1769` (top-level
  table well-formedness, nested ID well-formedness, per-family
  duplicate invariants like `I-DomainAttr-Intrinsic-Singleton`,
  `Vec<AttrData>` ordering rule) are validator-side
  (P1a-VAL-03).
- **Bridge recognition codepath** (`recognize_intrinsic_attr` +
  the `tcx.get_attrs_by_path` walker; `spec.md:1593-1715`) is
  bridge-side (P1a-BR-08a).
- **Phase-2 -> Phase-4 materialisation** (the working
  `intrinsic_cache: HashMap<DefPathHash, IntrinsicKind>` and
  the Phase-4 backbone-sizing + per-Item population pass) is
  bridge-side (P1a-BR-01 / P1a-BR-08a).
- **Per-family target-table mapping** for non-stage-14 families
  (`Mmio` -> `field_attrs`, `Interrupt` -> `item_attrs`, etc.
  per `spec.md:1820-1834`) is phase-2 territory; stage 14 only
  commits the `Intrinsic` -> `item_attrs` mapping. Do not
  invent placeholder variants for future families.
- **Pull-through helpers on `Instance`** (e.g.,
  `instance_intrinsic`) live on the Instance type
  (regseq-model-75 / P1a-DS-10 or follow-up); not in this
  task.
- **`#[rseq::intrinsic(...)]` recognition predicate** lives in
  P1a-BR-08a.
- **DiagLogPeel body-level pass and format-template grammar
  parsing** (`spec.md` §25.7.3 / §26.7) are P1a-BR-08b.

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/domain_attr.rs` `#[cfg(test)] mod
tests`.

Coverage requirements (every public type / function exercised
at least once per `SESSION.md` Testing Requirements):

1. **`LogLevel` construction.** Construct each of the 4
   variants `Debug`, `Info`, `Warn`, `Error`. Assert pairwise
   inequality.
2. **`LogLevel` derives.** Hash + Clone round-trip via
   `HashSet`.
3. **`IntrinsicKind::Log` construction.** Build
   `IntrinsicKind::Log(level)` for each of the 4 levels;
   assert distinctness.
4. **`IntrinsicKind::from_name` -- positive cases.** For each
   of the four NAMEs (`"log-debug"`, `"log-info"`,
   `"log-warn"`, `"log-error"`), assert
   `IntrinsicKind::from_name(intern(name)) ==
   Some(IntrinsicKind::Log(matching_level))`.
5. **`IntrinsicKind::from_name` -- negative cases.** For
   underscore form (`"log_debug"`), unknown family
   (`"mmio-read32"`), empty string, and a random Unicode
   NAME, assert `from_name` returns `None`. (Verifies the
   underscore-vs-dash divergence is enforced; the underscore
   form is the Rust-side function name, not the NAME
   identifier.)
6. **`IntrinsicKind::from_name` -- exhaustive arm coverage.**
   Confirm all four arms in the closed match are reachable
   (covered by item 4) and that the catch-all `_ => None`
   arm is reachable (covered by item 5).
7. **`AttrData::Intrinsic` construction.** Wrap each of the
   four `IntrinsicKind::Log` variants in
   `AttrData::Intrinsic`; assert distinctness across the
   four wrapped values.
8. **`AttrData::Intrinsic` discriminant pinning.** Assert
   that `AttrData::Intrinsic(_)` is variant index 0 (use
   `std::mem::discriminant` comparison or
   `#[repr(u8)]`-derived `as u8` cast if the project
   convention permits).
9. **`FieldPath` construction.** Construct
   `FieldPath { item: ItemId(0), variant: VariantIdx(0),
   field: FieldIdx(2) }`; assert clone equality; assert
   field-by-field accessor reads return the construction
   values.
10. **`FieldPath` Hash + HashMap key behavior.** Insert into a
    `HashMap<FieldPath, ()>`; round-trip lookup with a
    structurally identical `FieldPath` returns the inserted
    value; lookup with a `FieldPath` differing in any one
    field returns `None`.
11. **`DomainAttrTables::default` construction.** Construct
    via `Default::default()`; assert `item_attrs.is_empty()`,
    `field_attrs.is_empty()`, `span_attrs.is_empty()`.
12. **`DomainAttrTables` mutation -- `item_attrs`.** Push an
    `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))`
    onto `item_attrs[ItemId(0)]` (after sizing the backbone
    via `IndexVec::from_elem(Vec::new(), N)`); assert the
    pushed entry round-trips on subsequent indexing.
13. **`DomainAttrTables` mutation -- `field_attrs`.** Insert
    a `(FieldPath, vec![AttrData::Intrinsic(..)])` pair into
    `field_attrs`; assert lookup returns the inserted vec.
14. **`DomainAttrTables` mutation -- `span_attrs`.** Insert a
    `(Span, vec![AttrData::Intrinsic(..)])` pair; assert
    lookup returns the inserted vec.
15. **Multiple AttrData entries per key.** Push two
    `AttrData::Intrinsic` entries onto a single
    `item_attrs[ItemId(0)]`; assert the resulting `Vec` has
    length 2 and the entries are in insertion order.
    (Per-family duplicate invariants are validator-side; this
    test only verifies the storage shape supports multiple
    entries.)
16. **Serde round-trip -- `LogLevel`.** Each variant
    round-trips via `serde_json::to_string` / `from_str`.
17. **Serde round-trip -- `IntrinsicKind::Log(_)`.** Each of
    the 4 wrapped levels round-trips.
18. **Serde round-trip -- `AttrData::Intrinsic(_)`.** Each
    wrapped variant round-trips.
19. **Serde round-trip -- `FieldPath`.** A constructed
    `FieldPath` round-trips with `==`.
20. **Serde round-trip -- `DomainAttrTables`** (if the
    container types support serde per the project's existing
    sibling-table conventions). Otherwise, document
    skip-derive rationale per the brief's cross-cutting note.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-74 against its task spec at
.harness/tasks/regseq-model/74-domain-attr-tables.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/domain_attr.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- New module `crates/regseq-model/src/domain_attr.rs` created
  and re-exported from `crates/regseq-model/src/lib.rs`.
- Public API matches the spec verbatim:
  - `DomainAttrTables` with three sibling tables per
    `spec.md:1431-1435`.
  - `FieldPath { item, variant, field }` per
    `spec.md:1437-1441`.
  - `AttrData::Intrinsic(IntrinsicKind)` (closed, append-only)
    per `spec.md:1477`.
  - `IntrinsicKind::Log(LogLevel)` (closed, append-only) per
    `spec.md:1516`.
  - `LogLevel { Debug, Info, Warn, Error }` per
    `spec.md:1524`.
  - `IntrinsicKind::from_name(Symbol) -> Option<Self>` with the
    four `"log-<sev>"` arms per `spec.md:1542-1551`.
- All public types carry doc comments documenting the
  closed-but-append-only contract and the dash-vs-underscore
  NAME-form distinction.
- Test Plan items 1-20 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
