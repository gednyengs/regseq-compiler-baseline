# Stage 1 Review Log

Summary of four adversarial review rounds against the stage-1
work, with each issue and how it was addressed. A reviewer working
from this artifact set should not re-flag any issue listed here
unless the resolution is itself unsound.

Eighteen issues raised across the four rounds; all addressed.

| Round | Reviewer | Target | Verdict | Issues |
|---|---|---|---|---|
| 1 | Codex (gpt-5.5) | `stage-1-research.md` | REVISE_BEFORE_PROCEEDING | 5 |
| 2 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (initial) | REVISE_BEFORE_IMPL | 6 |
| 3 | external (no repo access) | `stage-1-vocabulary.md` (post-round-2) | REVISE_BEFORE_IMPL | 5 |
| 4 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (implementation-readiness) | REVISE_BEFORE_APPLYING | 2 |

## Round 1 -- review of `stage-1-research.md`

**Scope**: prior-art synthesis and the F1-F4 sub-decisions
(span byte representation, span placement, source content, ItemId
determinism).

**Verdict**: REVISE_BEFORE_PROCEEDING.

### R1.1 -- METHODOLOGY (HIGH): F2 conflicted with Q2 in `architecture.md`

F2 locked "spans inline, no `SpanId` table," but Q2 in
`architecture.md` listed `SpanId` among RegseqModel's internal IDs
and showed `spans: HashMap<Span, SpanId>` in the bridge Tables
example.

**Resolution**: amended `architecture.md` Q2 to drop the `SpanId`
references (in the internal-IDs bullet and the Tables example),
with a forward pointer to F2; added a "Reconciliation with Q2"
subordinate note to F2 in `stage-1-research.md` documenting the
amendment and confirming the rest of Q2's pattern still applies
to the other IDs.

### R1.2 -- F4 (HIGH): sort key under-specified

F4's sort key was stated as "lexicographic on `Vec<DefPathSegment>`
(segments + disambiguator)" -- missing crate identity and missing
the rule that symbol comparison is by string content (not by
allocation-order numeric IDs).

**Resolution**: refined the F4 "Sort key" subordinate note to
specify `(crate_name: &str, crate_disambiguator: StableCrateId,
Vec<SegmentKey>)` where `SegmentKey = (data_kind, symbol_text:
&str, disambiguator)`, with explicit "compare on string content"
rule. Three structural points cover Q6 cross-crate placement,
content comparison, and disambiguator handling.

### R1.3 -- CONVERGENT (MEDIUM): pattern 1 evidence overstated

Convergent pattern 1 ("per-kind newtype IDs, u32 backing") claimed
"rustc, Charon, the codegen-style projects, and verification IRs
all use newtypes." Codex caught that codegen_c uses untyped
`CValue::Local(usize)`, Kani has no IR at all, c2rust uses
clang-AST keys.

**Resolution**: weakened the claim. rustc is now framed as the
strongest precedent; other projects are described as "owned/
context-indexed but not uniform on typed newtypes."

### R1.4 -- F4 (MEDIUM): diagnostic rationale wrong

F4 justified the side `DefPathHash` table partly by "did this item
change between two builds?" -- but `DefPathHash` identifies the
*path*, not the body content.

**Resolution**: replaced the rationale with "cross-build diff
alignment of the items table" (matching a row in build A to a row
in build B by hash, not by index). Body-change detection is
explicitly noted as out of scope and would need a separate body
fingerprint.

### R1.5 -- TRAP (MEDIUM): F1 file-local-offset trap not stated

F1 chose `Span { file, lo: u32, hi: u32 }` and assumed
`content[lo..hi]` slices source text, but rustc's `BytePos` is a
*global* offset across the whole `SourceMap`. The bridge must
normalize to file-local offsets before storage.

**Resolution**: added F1 subordinate note stating that `lo`/`hi`
are file-local byte offsets, derived by subtracting
`SourceFile.start_pos`; with a validation rule when content is
present (`lo <= hi`, `hi <= content.len()`).

---

## Round 2 -- review of `stage-1-vocabulary.md`

**Scope**: the spec proper. Type sketches, ID inventory,
StringInterner, SourceFile/Span, DefPath form, Instance,
top-level RegseqModel, F4 in detail, serialization, pretty-
printer interface.

**Verdict**: REVISE_BEFORE_IMPL.

### R2.1 -- TYPE_SHAPE (HIGH): `DefPathData` missing pinned-rustc variants

The `DefPathData` enum was specified with 13 variants. Pinned rustc
has 19. Missing variants: `LateAnonConst`,
`DesugaredAnonymousLifetime`, `OpaqueLifetime(Symbol)`,
`AnonAssocTy(Symbol)`, `SyntheticCoroutineBody`, `NestedStatic`.
Per the spec's own no-defensive-fallback rule, the bridge would
wrongly reject items carrying these variants as "future
additions."

**Resolution**: walked the rustc-src enum
(`rustc_hir/src/definitions.rs:272-321`) and partitioned. Decision
was to include all 19 variants; closed-vocabulary scope is
*against the pinned toolchain*, with toolchain bumps triggering
explicit additions. Source-construct-level restrictions ("no
async functions in driver specs," "no RPITIT") live at a
different layer (bridge / stage-12), not at the DefPath
vocabulary level. Added a "Closed-vocabulary scope" paragraph
making this distinction explicit.

### R2.2 -- F4 (MEDIUM): sort-key discriminant rank unpinned

The F4 comparator used `data_kind: u8 (DefPathData discriminant)`,
but the spec did not pin a stable rank table. Two implementers
could produce different `ItemId` orderings on the same source if
the enum layout differed.

**Resolution**: added an explicit `def_path_data_sort_rank(data) ->
u8` function in the F4 section, with the 19 variants ranked
explicitly (0..18). Stability rule: never re-rank earlier entries;
new variants append at the end.

### R2.3 -- TYPE_SHAPE (MEDIUM): `Idx::new` const-ness gap

The body-local constants used `LocalId::new(0)` and
`BlockId::new(0)` in const contexts, but `Idx::new` is a trait
method and not declared `const fn`. As written this would not
compile.

**Resolution**: added an inherent `pub const fn from_u32(idx: u32)
-> Self` requirement to the `newtype_index!` analogue sketch.
Updated the body-local constants (`LocalId::RETURN_PLACE`,
`BlockId::START_BLOCK`) to use `from_u32`.

### R2.4 -- TRAP (MEDIUM): bridge `SourceFile` keying rule under-specified

The spec wrote `HashMap<rustc_span::SourceFile, SourceFileId>` for
the bridge's transient lookup table, but rustc lookups return
`Arc<SourceFile>` and `SourceFile` is not naturally hashable in a
unique way. An implementer would have to invent the keying rule.

**Resolution**: specified `HashMap<rustc_span::StableSourceFileId,
SourceFileId>`. `StableSourceFileId` is a Hash128 of filename plus
crate-id, already on every `SourceFile`'s `stable_id` field;
rustc's blessed unique identifier within the crate graph. Added
explanatory text: not keying on `Arc<SourceFile>` pointer identity
(less semantic, fragile to internal Arc-cloning) nor on `FileName`
(could collide across crates with same-named files).

### R2.5 -- FEASIBILITY (LOW): `DefPathHash` byte encoding ambiguous

`DefPathHash(pub [u8; 16])` worked for equality but underspecified
JSON shape and cross-platform dump conventions.

**Resolution**: split into named fields:
```rust
pub struct DefPathHash {
    pub stable_crate_id: StableCrateId,
    pub local_hash:      u64,
}
```
Mirrors rustc's `(StableCrateId, Hash64)` form exactly.

### R2.6 -- COVERAGE (LOW): `Item.def_path` vs `item_def_paths` consistency

Construction invariant 2 only required matching lengths between the
two parallel tables. Stage-12 authors could pick incompatible
ownership rules.

**Resolution**: tightened invariant 2 to require
`item_def_paths[item_id]` to equal the rustc `DefPathHash`
corresponding to `items[item_id].def_path`. Stage 12 inherits this
consistency rule when it specifies the placement of `def_path` on
`Item`. Removed the corresponding deferred entry from "Open items."

---

---

## Round 3 -- external review of `stage-1-vocabulary.md` (post-round-2)

**Scope**: the spec proper, after round-2 fixes had landed. External
reviewer with no repo access, working from the file-attachment
package and the `stage-1-review-prompt.md` prompt.

**Verdict**: REVISE_BEFORE_IMPL.

**Cross-checks confirmed correct**: F1 instantiation (12 B
file-local end-exclusive); F2 reflection (no SpanId); DefPathData
19-variant enum and rank table; F4 comparator avoiding raw DefId /
SymbolIndex / StringId; `Instance = (ItemId, GenericArgs)` shape;
Q1/Q2 owned + rustc-free posture.

### R3.1 -- TYPE_SHAPE (HIGH): multi-crate identity gap

The spec carried *one* `crate_name` and *one* `crate_disambiguator`
at the model root, but Q6 puts items from multiple crates into one
RegseqModel. The F4 sort key required *per-item* crate identity,
yet `DefPath` only had segments + rendered. An implementer would
have to invent a side mapping or rely on the rendered string for
crate identity.

**Resolution**: added `CrateId` as a stage-1 ID kind. Added
`crates: IndexVec<CrateId, CrateInfo>` and `primary_crate: CrateId`
at the model root, replacing the prior `crate_name` /
`crate_disambiguator` fields. `CrateInfo` is `{ name: StringId,
stable_crate_id: StableCrateId }`. Added `crate_id: CrateId` field
to `DefPath` (rustc-aligned: rustc's own `DefPath` carries `krate:
CrateNum`). F4 sort key updated to read crate identity via
`crates[item.def_path.crate_id]`. Construction invariants 4-7
pin `primary_crate` validity, crates-table uniqueness, valid
`crate_id` references, and entry-point membership in the primary
crate.

### R3.2 -- FEASIBILITY (HIGH): serde contract inconsistency

The spec said IDs *derive* `Serialize` / `Deserialize` while also
promising "bare integer" JSON. A derive on a niche-encoded ID would
emit the internal niche-shifted value, not the logical index. Same
problem for `StringInterner`: derive would emit both `storage` and
`lookup`, but the conventions table promised a flat string array.

**Resolution**: replaced "derive" with custom `Serialize` /
`Deserialize` for IDs and `StringInterner`. IDs implement serde via
`#[serde(into = "u32", from = "u32")]` plus `From` impls going
through `Idx::index()` / `from_u32`, with deserialize-time niche-
range validation. `StringInterner` implements serde manually with
on-wire shape `Vec<String>` (storage only, lookup rebuilt) and
deserialize-time validation (index 0 is empty, no duplicates).
Other types (`SourceFile`, `Span`, `DefPath`, `CrateInfo`, etc.)
continue to derive. Header sentence in the serialization-conventions
section updated to acknowledge the split. Per-type table now
includes a "mechanism" column.

### R3.3 -- FEASIBILITY (MEDIUM): `DefPath.rendered` under-specified

`rendered` was described as "in the format `def_path_str` produces
(with disambiguator suffixes when verbose)" without pinning verbose
vs non-verbose. Non-verbose can collapse distinct impls to identical
strings.

**Resolution**: pinned *always-verbose* rendering. Disambiguator
suffix on every nonzero-disambiguator segment; anonymous namespaces
rendered as `{namespace#N}`; matches rustc's `as_sym(verbose=true)`
convention. Added a construction invariant: `rendered` is the
canonical verbose rendering of `(crates[crate_id].name, segments)`.
Two RegseqModels for the same source must produce byte-identical
`rendered` strings.

### R3.4 -- FEASIBILITY (MEDIUM): line-numbering off-by-one

`line_starts` documented as zero-based, `Position` was 1-based, and
`line_text(line: u32)` did not state which convention. Inconsistent
output risk.

**Resolution**: pinned `line_text` to 1-based input matching
`Position`. Documented that the returned slice excludes the line
terminator (whether `\n` or `\r\n`). Updated the `line_starts`
field comment to make the internal-vs-external indexing
relationship explicit (`line N is at line_starts[N - 1]`).

### R3.5 -- TRAP (MEDIUM): file_local_hi at file boundary

The bridge invariant resolved `lo` via `lookup_byte_offset` but did
not specify how `hi` was computed. An end-exclusive `hi` lying on
the next file's `start_pos` would resolve to the next file under a
fresh `lookup_byte_offset` call, since rustc partitions on
`start_pos <= pos`.

**Resolution**: tightened the bridge invariant. Step 1 resolves
`lo` to find the SourceFile `sf`; step 2 explicitly computes
`file_local_hi = SpanData.hi - sf.start_pos` using `sf` selected in
step 1, not a fresh lookup. Validation now includes
`hi - sf.start_pos <= sf.unnormalized_source_len` to reject spans
that cross file boundaries.

---

## Round 4 -- implementation-readiness revision review

**Scope**: two proposed revisions to `stage-1-vocabulary.md` surfaced
by an implementation-readiness analysis just before stage-1 step-1
implementation began. Revision A reopens the serde-mechanism
paragraph addressed in R3.2; revision B adds `'static` to the `Idx`
trait bound. This round reviewed *only those two revisions*, not the
whole spec.

**Verdict**: REVISE_BEFORE_APPLYING. Both revisions accepted in
substance; wording tightened per the findings below before being
applied.

### R4.1 -- SERDE (MEDIUM): correct R3.2's `from = "u32"` mechanism

R3.2 required ID newtypes to use `#[serde(into = "u32", from =
"u32")]` plus `From<Id> for u32` and `From<u32> for Id`. The
infallible `From<u32>` path delegates to the inherent `from_u32`,
which panics on niche-violating input (e.g., `u32::MAX` when the
niche reserves the top value). Serde surfaces panics poorly compared
to a clean `D::Error::custom(...)`. Additionally, the original
paragraph's blanket "not derived" claim conflicts with the
conversion-attribute derive route, and the prior wording did not
explicitly ban `#[serde(transparent)]` over the niche-bearing inner
field (which would also leak the internal representation).

**Resolution**: rewrote the paragraph at lines 100-110 of
`stage-1-vocabulary.md`. The new wording (a) bans field-derive and
`#[serde(transparent)]` on ID newtypes explicitly, (b) names two
acceptable routes -- hand-rolled `impl Serialize` / `impl
Deserialize` *or* `#[serde(into = "u32", try_from = "u32")]` with
`TryFrom<u32, Error: Display>` -- and (c) explicitly rules out the
infallible `from = "u32"` shape. Either route must round-trip the
*logical index* and reject niche-violating values at deserialize
time. The conventions table (line 823) and the section header (lines
814-817) were synced to allow either route. R3.2's resolution is
strengthened (not invalidated) by R4.1 -- the wire-shape contract
and niche-validation requirement carry through unchanged.

### R4.2 -- TRAIT_BOUND (LOW): make `'static` explicit on `Idx`

The `Idx` trait declaration (line 58) carried `Copy + Eq + Hash + Ord
+ Debug` but no `'static`. The IR is owned end-to-end by Q2 (`'tcx`
dies at the bridge) and the model invariant later in the spec ("No
`'tcx` references"). All stage-1 IDs are `u32`-backed and naturally
`'static`. The trait bound did not surface this.

**Resolution**: added `'static` to the `Idx` trait bound at line 58
of `stage-1-vocabulary.md`. Appended a paragraph to the rationale
calling out that the bound makes the existing owned/`'tcx`-free IR
invariant explicit at the trait boundary, with rustc's
`rustc_index::Idx` (also `'static`-bound) as secondary precedent.
Initial draft justified `'static` via speculative `TypeId`/type-
erased-registry use cases; this was rejected as defensive-fallback
overreach (cf. methodology rule "no defensive fallbacks; toolchain is
pinned") and replaced with the IR-invariant rationale.

---

## Notes for the next reviewer

Four rounds covered different scopes (research synthesis, initial
spec, post-revision spec, implementation-readiness revisions) and
surfaced different categories of issues. Together they exercised:
type-shape consistency, F1-F4 instantiation correctness, methodology
violations, rustc-internal traps, prior-art over-extrapolation,
implementation-feasibility ambiguity, multi-crate-graph completeness
(round 3), and serde-mechanism / trait-bound implementation
correctness (round 4).

Areas that *have not* been adversarially reviewed yet, and which
the next reviewer might profitably probe:

- *Forward-fit with stages 3-13.* The spec forward-declares `TyId`,
  `ConstId`, `BodyId`, `Item`, etc. Whether the table shapes will
  actually accommodate what those stages produce is not
  empirically verified.
- *Cross-stage interaction.* Domain extensions (stage 13:
  intrinsics, registers, ISRs) need to ride on the stage-1
  vocabulary. Whether the convention scales to that workload has
  not been stress-tested.
- *Pretty-printer interface.* Sketched but not exercised against
  any concrete IR fragment.
- *Implementation feasibility under the regseq-model crate's
  constraints.* The crate has zero rustc deps; whether every
  rustc-side type the spec mirrors has a clean owned analogue is
  a feasibility check that has not been run.
