# Stage 1 Review Log

Summary of five adversarial review rounds against the stage-1
work, with each issue and how it was addressed. A reviewer working
from this artifact set should not re-flag any issue listed here
unless the resolution is itself unsound.

Thirty-nine issues raised across seven review passes; all addressed.

| Round | Reviewer | Target | Verdict | Issues |
|---|---|---|---|---|
| 1 | Codex (gpt-5.5) | `stage-1-research.md` | REVISE_BEFORE_PROCEEDING | 5 |
| 2 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (initial) | REVISE_BEFORE_IMPL | 6 |
| 3 | external (no repo access) | `stage-1-vocabulary.md` (post-round-2) | REVISE_BEFORE_IMPL | 5 |
| 4 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (implementation-readiness) | REVISE_BEFORE_APPLYING | 2 |
| 5 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (round-5 monomorphization + C-semantics revision) | REVISE | 6 |
| 5e | external (no repo access) | `stage-1-vocabulary.md` (post-round-5-Codex) | REVISE_BEFORE_IMPL | 10 |
| 6 | Codex (gpt-5.5) | `stage-1-vocabulary.md` (post-round-5e cascade check) | REVISE | 5 |

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

## Round 5 -- monomorphization + C-semantics revision

**Scope**: stage-1 vocabulary spec on the monomorphization story
(Instance, GenericArg, RegseqModel-level emission table) and on
the C-semantics framing of RegseqModel as a whole. Surfaced
between the close of step 4 (SourceFile + Position + Span) and
the start of step 5 (DefPath + Instance + GenericArg + ...) by
deeper interrogation of what RegseqModel must capture for its
two downstream stakeholders (domain analyses and the C codegen
backend). The conversation touched stage 11/12/13 design surface
and bent the stage-1 spec around the conclusions.

**Verdict**: REVISE (Codex round-5 found 6 issues; all addressed
before applying the patch). An independent external pass over
the patched spec followed and found 10 additional issues, all
HIGH/MEDIUM/LOW; addressed in the "Round 5 -- external follow-up"
subsection below.

### R5.1 -- INSTANCE_SHAPE (HIGH): Instance was a thin (item, args) tuple

The original `Instance { item: ItemId, args: GenericArgs }` did
not actually bake monomorphization. Consumers had to walk both
`items` and `instances` and reconcile them. Q2 in
`architecture.md` promised "bake monomorphization in the bridge,"
but the type didn't honor that promise. The thin shape also
forced consumers to reach into `Item` for kind discrimination
(struct vs. fn vs. const), which conflated source-side identity
with monomorphized emission units.

**Resolution**: redesigned `Instance` to be the unified
monomorphized table. Common fields: `item`, `generic_args`,
`symbol_name`, `span`, `kind`. `kind: InstanceKind` is a closed
six-variant enum (`Function { body }`, `Static { ty, init }`,
`Const { ty, value }`, `Struct { layout }`, `Enum { layout }`,
`Union { layout }`). Each variant carries only the IDs needed
to find its rich data in stage-3/4/11 tables. Field rename
`args` -> `generic_args` for clarity. The bridge interns
`(item, generic_args)` tuples; ordering pinned by F4-2
(deterministic across builds via the same key as F4 plus an
`ArgsKey` fingerprint defined in stage 3).

### R5.2 -- ROOTS (MEDIUM): `entry_points` baked one notion of "root"

The original `RegseqModel.entry_points: Vec<ItemId>` baked one
notion of "root" into the IR. Different consumers want different
root predicates (`#[no_mangle]` exports for codegen vs.
analysis-specific reachability roots vs. test-driver entry
points). Embedding one notion forced a one-size-fits-all
contract that doesn't actually fit any of them.

**Resolution**: removed `entry_points` entirely. Removed the
construction invariant tying it to `primary_crate`. Removed the
"Roots / entry points" subsection. Roots are consumer-derived
predicates over `instance.item`'s attributes, ABI, and
visibility (which live on `Item`, stage 12). Implications for
Item: stage 12 must surface source-level metadata
(`linkage: Linkage`, `abi: Abi`, `inline_hint`, `is_unsafe`,
etc.) so consumers can run their predicates.

### R5.3 -- RUST_TO_C (HIGH): C-semantics framing wasn't explicit

Decisions about how Rust constructs lower into RegseqModel
(`Drop` terminators, trait objects, `&mut T`, `repr(C)`,
`impl` blocks, closures, std collections) were ambiguous. Some
were implied by Q2 + Q3 but not pinned. Without an explicit
framing, future stages would re-litigate each construct.

**Resolution**: added a methodology rule to
`docs/discussions/regseq-model/memory.md` ("C semantics with
information preservation") establishing that RegseqModel is a
C-shaped IR, the bridge does the desugaring, and information
rustc gives us for free is preserved when downstream
stakeholders benefit -- but always per-construct interrogated.
Added a "Rust -> C lowering reference" section to
`stage-1-vocabulary.md` enumerating the already-decided
construct lowerings as a cross-stage check. The table is
descriptive (pins what is decided) and informs stage-3/4/9/11/12
work without dictating shape.

### R5.4 -- IMPL (MEDIUM): `impl` block representation unpinned

The original spec did not pin how `impl` blocks were represented
in RegseqModel. Could have been an Item kind (faithful to
rustc), a path scope only (lifted), or a hybrid. The choice
affects whether downstream consumers see `impl` as a first-class
construct (with associated methods nested inside) or as a flat
list of top-level entities.

**Resolution**: pinned full lifting. `impl` is a path scope only,
preserved in DefPath via `DefPathData::Impl`. Methods lift to
top-level Function Items with explicit receivers. Trait
declarations and associated types are erased; static dispatch
resolves directly, dynamic dispatch via synthesized vtables
(Static Instances of fn-pointer-struct type). Consequence:
`ItemKind` (stage 12) has six variants -- `Function`, `Static`,
`Const`, `Struct`, `Enum`, `Union` -- mirroring `InstanceKind`.
No `Impl`, `Trait`, or `AssocType` Item kinds. New section
"Lifting `impl` items" added to the spec.

### R5.5 -- SOURCE_SENTINEL (LOW): synthesized SourceFile sentinel

Bridge-synthesized Instances (drop glue, vtable shims, closure
captures) need source spans, but have no source location.
Without a sentinel rule, every consumer had to special-case
`Option<Span>`.

**Resolution**: reserved `SourceFileId::ZERO` for the synthesized
sentinel. Bridge populates `source_files[0]` as
`SourceFile { path: <interned "<synthesized>">, content: None,
line_starts: vec![0] }`. Real source files start at
`SourceFileId(1)`. Added construction invariant. Mirrors the
`StringId::ZERO == ""` pattern. Consumers don't need
`Option<Span>`: a synthesized span resolves through `Span::slice`
to `None` because the sentinel's `content` is `None`.

### R5.6 -- KIND_DISCRIMINATION (LOW): InstanceKind explicit vs. derived

Once `Instance` is the unified table, the question arose whether
`InstanceKind` should be an explicit enum on `Instance`, or
derivable from `instance.item.kind` (ergonomically cheaper but
forces an extra hop). Static and Const could also collapse into
a single `Data { ty, value, mutable: bool }` variant.

**Resolution**: explicit, with six variants (Function, Static,
Const, Struct, Enum, Union). Static and Const stay distinct
(they have different C lowering: const can be `#define`-style
or constexpr-baked, static gets a global). The explicit form
makes serialization shape clear and saves the hop through Item
on hot iteration paths. Field naming pinned: `generic_args`
(not `args`) for clarity at consumer sites.

---

## Round 5 -- external follow-up

**Scope**: independent external review of the round-5 patch
(Codex round-5 already addressed). Reviewer had no repo access;
attachments included the full round-5-revised spec, methodology
rules, prior review log, and rustc excerpts. Verdict:
REVISE_BEFORE_IMPL with 10 issues (5 HIGH, 4 MEDIUM, 1 LOW).
All addressed inline before the round-5 patch was committed.

### R5.7 -- INSTANCE_SHAPE (HIGH): interning + F4-2 rule incomplete for nominals

The round-5 interning rule said "key on full rustc instance
identity (`InstanceKind` + args)." That covers Function-like
constructs, but Regseq's `InstanceKind` includes
Struct/Enum/Union -- which are *types*, not callables, and
have no rustc `InstanceKind`. The rule was incomplete: an
implementer could not tell whether nominal Instances sort
under rustc `Item`, under a Regseq kind rank, or outside F4-2.

**Resolution**: defined a Regseq-level `RegseqInstanceKey`
with two explicit rank tables (pinned in the spec, not
deferred):
- `regseq_instance_kind_rank` for the six Regseq kinds
  (Function=0, Static=1, Const=2, Struct=3, Enum=4, Union=5).
- `rustc_instance_kind_sort_rank` for rustc's full
  `ty::InstanceKind` taxonomy (Item=0, Intrinsic=1, ...,
  AsyncDropGlue=14), used as `fn_shim_disambig` only for
  Function variants. Other Regseq kinds use `fn_shim_disambig
  = 0` unconditionally. F4-2 sorts on the same key. Both
  tables are append-only.

### R5.8 -- INSTANCE_SHAPE (HIGH): custom-naming uniqueness gap

The spec told consumers to walk `(item.def_path, generic_args)`
for custom names, but that tuple is *not* unique under rustc
shims sharing a `DefId` (e.g., `Item(foo)` vs.
`ReifyShim(foo)`). Custom-naming consumers using only the
tuple could collide names. The "always-unique by construction"
claim about `symbol_name` also lacked grounding in
`rustc-excerpts.md`.

**Resolution**: (a) added an explicit caveat to the
custom-naming guidance requiring `InstanceId` or
`symbol_name` as a disambiguating suffix; (b) weakened the
`symbol_name` claim from "always-unique by construction" to
"bridge-provided default symbol; uniqueness is guaranteed by
the bridge by construction (via rustc's mangler factoring in
the full InstanceKind)"; (c) added rustc excerpts for
`tcx.symbol_name`, `ty::SymbolName`, `Instance`, `InstanceKind`,
and `InstanceKind::def_id()` to `rustc-excerpts.md` (new
sections 7-8) so the spec's claims are grounded.

### R5.9 -- FORWARD_FIT (HIGH): synthesized vtable Items + `item_def_paths`

Round 5 declared vtables to be Static `Instance`s, but every
`Instance` has `item: ItemId`, and construction invariant 2
requires `item_def_paths[item_id]` to be the rustc
`DefPathHash` of `items[item_id].def_path`. Synthesized
vtables have no source-level `DefId`, so the invariant
couldn't be satisfied as written.

**Resolution**: added a "Bridge-synthesized Items" subsection
pinning the policy. Synthesized vtable Items live in `items`
alongside source-level items with: a synthetic `def_path`
under a reserved canonical form (`<vtable for Trait for
SelfTy>`); a synthetic `def_path_hash` derived from
`hash((trait_def_path_hash, self_ty_canonical_hash))`, stable
across builds; a `crate_id` of the triggering crate.
Construction invariant 2 extends to synthetic items. The
precise `DefPathData` segment representation (new variant or
reused existing) is deferred to stage 9/12 when vtable
lowering lands.

### R5.10 -- FORWARD_FIT (HIGH): extern fn signatures had no home

`InstanceKind::Function { body: Option<BodyId> }` reserved
`body: None` for foreign declarations, with the spec saying
"signature lives on Body." This left no place for the
signature of a body-less foreign declaration -- breaking C
codegen (no prototype) and analyses (no call shape).

**Resolution**: added `sig: FnSigId` to `Function` variant.
`Function { sig: FnSigId, body: Option<BodyId> }`. Forward-
declared `FnSigId` in the ID inventory; added `sigs:
IndexVec<FnSigId, FnSig>` to `RegseqModel`. The `sig` field
is always present (foreign decls included); it stores the
substituted signature for this monomorphization. Stage 11
defines `FnSig`; stage 1 reserves the table.

### R5.11 -- CASCADE (HIGH): undefined bridge root policy

After removing `entry_points`, invariant 9 said the bridge
"computes the closure from the consumer-relevant root set" --
undefined and seemingly consumer-influenced, contradicting
Q4's principle that the bridge produces RegseqModel uniformly.

**Resolution**: pinned an explicit bridge root policy. The
bridge computes the full rustc mono-item closure for the
primary crate plus all regseq-aware dependency crates (per Q6
discovery), plus all synthesized artifacts those mono-items
require (drop glue, vtable shims, etc., per rustc's
`MonoItem` collection logic). The set is bridge-determined
and not consumer-influenced. Downstream consumers filter the
resulting `instances` table for their own roots. Construction
invariant 9 reworded accordingly.

### R5.12 -- F4_2 (MEDIUM): rank table determinism hazard

The round-5 spec deferred the `instance_kind_disambig` rank
table to stage 3/11 -- recreating exactly the determinism
hazard that R2.2 fixed for `DefPathData`. Two bridge
implementations could pick different ranks and produce
different stable `InstanceId` assignments for the same
source.

**Resolution**: subsumed by R5.7. The
`rustc_instance_kind_sort_rank` table is now pinned in
stage 1, append-only, with the toolchain version explicitly
referenced.

### R5.13 -- IMPL_LIFTING (MEDIUM): default trait method bodies

The lifting principle covered inherent methods, trait impl
methods (with explicit overrides), associated constants,
associated types, and trait declarations -- but not default
method bodies. An impl that inherits a default has no
overriding implementing function body, so the spec was
ambiguous about where the body comes from.

**Resolution**: added a bullet to "Lifting impl items".
Default bodies live at the trait's DefPath (e.g.,
`crate::Foo::bar`); a single Function Item is created there.
When `impl Foo for X` doesn't override, the use-site
`<X as Foo>::bar()` resolves to an `Instance` whose `item` is
the trait's default-bar Item and whose `generic_args` include
`Self=X`. The body is the default body monomorphized for the
substitution. No impl-side Item is created for the inherited
default.

### R5.14 -- IMPL_LIFTING (MEDIUM): vtable shape underspecified

Round 5 described vtables as "structs of fn pointers." Per
rustc's `tcx.vtable_entries`, a vtable entry can be
`MetadataDropInPlace`, `MetadataSize`, `MetadataAlign`,
`Vacant`, `Method(Instance)`, or `TraitVPtr(TraitRef)` -- a
mixed aggregate, not just fn pointers.

**Resolution**: updated the lowering reference table and
"Where InstanceId references appear" text to describe vtables
as Static `Instance`s of mixed-aggregate type matching rustc's
full vtable-entry sequence (drop fn ptr + size + align +
per-method fn pointers + supertrait vtable pointers + Vacant
slots). Added rustc-excerpts.md section 9 grounding the
`VtblEntry` enum. Precise struct layout pinned at stage 9/12;
stage 1 reserves the policy.

### R5.15 -- C_SEMANTICS (MEDIUM): `&mut T -> T* restrict` overreach

Round 5's lowering reference said `&mut T` becomes
`T* restrict`. `restrict` is a C declaration-level promise
about pointer provenance, not a type-level qualifier. Stored
fields, returned references, and unsafe escape patterns may
invalidate it. Baking `restrict` into every `&mut T` pointer
is unsound.

**Resolution**: split mutability from provenance. `&T` lowers
to `Pointer { mutability: Const, provenance: SharedBorrow }`;
`&mut T` lowers to `Pointer { mutability: Mut, provenance:
UniqueBorrow }`; raw `*const T` / `*mut T` lower to
`provenance: Raw`. Codegen at stage 9+ decides per-context
where `restrict` is safe to emit (function parameters and
locals with no escape; not stored fields, returns, or
reborrowed raw pointers). Updated the methodology rule in
`memory.md` to remove the over-strong claim.

### R5.16 -- SENTINEL (LOW): naming consistency

The round-5 spec used `SourceFileId::ZERO` in the
synthesized-source-file section, but ID conventions only
documented constants for body-local IDs (`LocalId::RETURN_PLACE`,
`BlockId::START_BLOCK`). A step-5 implementer might be unsure
whether to use `ZERO` or a more descriptive name.

**Resolution**: added a documentary alias
`SourceFileId::SYNTHESIZED = SourceFileId::ZERO`. The macro-
generated `ZERO` constant remains valid; `SYNTHESIZED` is
preferred at use sites for readability. Spec updated to use
`SYNTHESIZED` in the synthesized-source-file section.

---

## Round 6 -- post-external-cascade check

**Scope**: final adversarial pass over the round-5+5e patched
spec, verifying that R5.7-R5.16 fixes did not introduce new
gaps or cascade inconsistencies. Codex reviewed across the same
12 concern axes used in round 5e. Verdict: REVISE with 5
findings (1 HIGH, 3 MEDIUM, 1 LOW). All addressed inline before
commit.

### R5.17 -- INTERNAL_CONSISTENCY (HIGH): invariant 2 didn't accommodate synthetic items

The R5.9 fix introduced bridge-synthesized vtable Items with
bridge-derived `DefPathHash`es, but construction invariant 2
still asserted `item_def_paths[item_id]` is rustc's
`DefPathHash` of the originating `DefId`. Synthetic vtable
items have no rustc `DefId`, so the invariant was unsatisfiable
for them.

**Resolution**: split invariant 2 into two cases. For
source-level Items: rustc's `DefPathHash` of the originating
`DefId`. For bridge-synthesized Items: the bridge-derived
stable hash from "Bridge-synthesized Items" (e.g.,
`hash((trait_def_path_hash, self_ty_canonical_hash))` for
vtables). Both forms remain stable across builds and globally
unique.

### R5.18 -- FORWARD_FIT (MEDIUM): FnSigId not in invariant 1's "every ID resolves" list

The R5.10 fix added `FnSigId` to the ID inventory and `sigs` to
RegseqModel, but construction invariant 1 still listed only
the pre-R5.10 IDs. An implementer following invariant 1
literally would not know to validate FnSigId references.

**Resolution**: added `FnSigId` to invariant 1's ID list, with
an explicit sub-clause: "every `Function.sig: FnSigId` resolves
to a valid entry in `sigs`, including for foreign declarations."

### R5.19 -- INTERNAL_CONSISTENCY (MEDIUM): rustc-excerpts.md section 7 still described old interning key

The R5.7 fix introduced `RegseqInstanceKey` (4-field key) and
made it canonical in `stage-1-vocabulary.md`. But
`rustc-excerpts.md` section 7's commentary still said "the
interning key in stage 1 is `(InstanceKind, args)` -- the same
shape rustc uses." This is stale.

**Resolution**: updated the section-7 explanatory paragraph to
describe `RegseqInstanceKey` (the four-field shape) and
explicitly reference `stage-1-vocabulary.md` as the
authoritative definition.

### R5.20 -- FORWARD_FIT (MEDIUM): vtable DefPathData punt was structurally ambiguous

The R5.9 fix punted the precise `DefPathData` segment
representation for synthetic vtable scopes to stage 9/12,
suggesting "a new `SyntheticVTable` variant or a reused
existing variant." But `DefPathData` is described as a closed
19-variant enum mirroring rustc. Adding a new variant to a
closed enum is a structural change that the punt did not
acknowledge as a stage-1 revision.

**Resolution**: tightened the punt wording. Stage 9/12 may
overload an existing variant, OR may add a new variant to
`DefPathData`. The latter is explicitly a stage-1 revision:
new variant appended, `def_path_data_sort_rank` table extended
with rank 19 (append-only), spec revision goes through the
same Codex/external review cadence as R4/R5. Stage 1 reserves
the slot but does not commit to the choice.

### R5.21 -- WORDING (LOW): SourceFileId::ZERO use-site inconsistency

R5.16 added the `SYNTHESIZED` alias and said spec text should
prefer it at use sites, but several spec spots still used the
underlying `SourceFileId::ZERO` constant in describing what a
synthesized span looks like.

**Resolution**: replaced use-site occurrences of
`SourceFileId::ZERO` with `SourceFileId::SYNTHESIZED` (in the
Instance field doc and in construction invariant 8). The alias
definition itself and the rendering-explanation paragraph
continue to reference `ZERO` since that is the underlying
constant.

---

## Notes for the next reviewer

Seven review passes covered different scopes (research synthesis,
initial spec, post-revision spec, implementation-readiness
revisions, the monomorphization + C-semantics pivot, the external
follow-up on the round-5 patch, and the post-cascade check).
Together they exercised: type-shape consistency, F1-F4
instantiation correctness,
methodology violations, rustc-internal traps, prior-art
over-extrapolation, implementation-feasibility ambiguity,
multi-crate-graph completeness (round 3), serde-mechanism /
trait-bound implementation correctness (round 4), and
monomorphization + IR shape vs. C-semantics framing (round 5).

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
