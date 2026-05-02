# Expectations on stage 3 (incoming)

Stage 3 expands the `Ty` placeholder
(`crates/regseq-model/src/placeholders.rs`) into the full type
vocabulary. This file lists what earlier stages have asked stage 3
to deliver.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- [done] **Pin the canonical `Ty` representation rule.** F4-2
  instance ordering depends on a deterministic `ArgsKey`
  fingerprint, which in turn depends on the canonical-form rule
  for `Ty`. Stage 1 reserved both the rule and the sort
  obligation; stage 3 must pin the representation so two builds
  produce identical `ArgsKey` values for equivalent type
  arguments. See the F4-2 section of `stage-1/spec-snapshot.md`
  (DefPath / Determinism sections).
  - **Variant set locked** by D1.1 (see `spec-snapshot.md` Section
    1.2; 19 rigid post-mono variants).
  - **Canonical form / `ArgsKey` rule locked by D3 + D4**
    (`spec-snapshot.md` Sections 3.1, 3.2). The canonical form is a
    **deep canonicalizer** distinct from the BiMap's in-build
    `Hash + Eq`: it walks `Ty` content with per-IR-identifier
    termination rules (TyId resolves via `model.types.resolve`;
    ItemId terminates at DefPathHash; FnSigId expands to FnSig
    contents and recurses; StringId resolves to string content;
    ConstId pinned in stage 4). For ADT-shaped variants the
    canonical encoding is `(variant_kind_tag, DefPathHash,
    canonical_args)` with Layout *not* traversed -- breaks
    self-reference cycles. Within a build the BiMap's structural
    dedup ensures content-equal Tys have equal TyIds (a fast
    path); cross-build, the deep canonicalizer is the
    authoritative comparator.

- [done] **Define the layout-pointing `Ty` variants.**
  `InstanceKind`'s `Struct { layout: TyId }`,
  `Enum { layout: TyId }`, and `Union { layout: TyId }` reference
  `Ty` entries that stage 3 must populate with field structure
  (`Ty::Struct { fields }`, `Ty::Enum { variants }`,
  `Ty::Union { fields }`) and resolved layout (per the C-semantics
  methodology rule: `repr(C)` / `repr(packed)` become a resolved
  `Layout` field, not an annotation passthrough).
  - **Variant separation locked** by D1.2 (`spec-snapshot.md` Section
    1.3).
  - **Layout-on-Ty position locked** by D2 (`spec-snapshot.md`
    Section 2.1): `Ty::Struct/Enum/Union/Tuple` carry an inline
    `Layout` field populated from `tcx.layout_of`'s answer.
    Tuples reuse `LayoutKind::Struct` with synthetic field
    names (`_0`, `_1`, ...) and have no `Item` -- the layout
    lives inline on `Ty::Tuple` itself.
  - **Layout content locked by D11** (`spec-snapshot.md` Section
    2.2): full `Layout` schema with `size`, `align`,
    `pack: Option<u32>`, `uninhabited`, and a `LayoutKind` enum
    discriminating Struct / Union / Enum content. `repr(packed)`
    is the only repr knob that becomes a structural field; all
    other repr effects fall out of resolved bytes.

- [forwarded] **Fill `pretty.rs::render_ty` stub.** Owned by
  stage 13 (type-system pretty-printing); see
  `../stage-13/expectations.md`. Originally reserved at stage 3
  as the open D14 decision but **deferred** -- the renderer's
  input depends on `Ty` content references that only become
  fully populated at stage 11 (`FnSig`) and stage 12 (`Item` /
  DefPath). The stage-1 stub at
  `crates/regseq-model/src/pretty.rs` still panics with
  `unimplemented!("stage 3 fills render_ty")` (legacy message;
  stage 13 may update it on landing). Lean (per the historical
  D14 entry in `discussions.md`, preserved as input for stage
  13): Rust-source-style notation (`&'r T`, `*const T`,
  `[T; N]`, etc.) per Charon precedent.

- [done] **Pointer / reference provenance field.** Resolved by
  D7/D8 (`spec-snapshot.md` Section 1.8). The locked alphabet is the
  two-variant `Provenance := { SharedBorrow, UniqueBorrow }`
  carried on `Ty::Ref` only; raw provenance is encoded by the
  variant choice `Ty::Ptr` (`Ty::RawPtr` was renamed to `Ty::Ptr`
  for symmetric pairing with `Ty::Ref`). Advisory bits
  (`frozen` / `unpin` / `global`) dropped -- no v1 codegen
  consumer materializes them, and rustc's
  `rustc_abi/src/lib.rs:2134-2137` allows their absence.

- [done] **Niche optimization for `Option<TyId>`.**
  Stage 1's compile-time assertion that `Option<TyId>` is 4 bytes
  is structural; D2 (explicit-Layout-on-Ty) does not change
  `TyId`'s representation; stage 3's choice of `NonMaxU32` for
  `TyId`'s niche mechanism preserves the assertion. The
  niche-encoded `Option<TyId>` is preserved per D12 (locked via
  `TagEncoding::Niche` in D11). See `spec-snapshot.md` Section 4.4
  for the canonical `Option<&u32>` worked example.

## Bridge concerns surfaced by stage 1 (act on these alongside
##  stage 3 work)

- [done] **`Ty` interning.** Resolved by D3 + D4
  (`spec-snapshot.md` Sections 3.1, 3.2). The structural BiMap
  (`TyInterner` -- `storage: IndexVec<TyId, Ty>` +
  `lookup: HashMap<Ty, TyId>`) lives **inside** `RegseqModel.types`,
  mirroring the stage-1 `StringInterner` precedent. Identity rule
  is the nominal/structural split: ADT-shaped variants
  (Struct/Enum/Union/FnDef/Closure) compare by `(variant_kind_tag,
  item, args)` -- the variant tag is load-bearing so
  `Ty::Closure(item, args)` and `Ty::Struct{item, args, layout}`
  for the same closure remain distinct entries; non-ADT variants
  derive structurally except `Ty::Tuple` which excludes its
  inline `Layout` from `Hash + Eq` (layout is functionally
  determined by `fields`, mirroring the ADT layout-exclusion
  reasoning). ADT forward references
  resolve via TyId reservation. The bridge additionally holds a
  *bridge-local* short-circuit cache `HashMap<rustc::Ty<'tcx>,
  TyId>` (forced bridge-side by Q1; dies with bridge call per Q2)
  and dispatches between two paths -- generic non-ADT
  (`translate_ty`) and ADT-shaped reservation (`translate_adt`
  using `find` before reserve).
