# Expectations on stage 3 (incoming)

Stage 3 expands the `Ty` placeholder
(`crates/regseq-model/src/placeholders.rs`) into the full type
vocabulary. This file lists what earlier stages have asked stage 3
to deliver.

The convention is described in `../memory.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Pin the canonical `Ty` representation rule.** F4-2 instance
  ordering depends on a deterministic `ArgsKey` fingerprint, which
  in turn depends on the canonical-form rule for `Ty`. Stage 1
  reserved both the rule and the sort obligation; stage 3 must
  pin the representation so two builds produce identical
  `ArgsKey` values for equivalent type arguments. See the F4-2
  section of `stage-1/vocabulary.md` (DefPath / Determinism
  sections).

- **Define the layout-pointing `Ty` variants.** `InstanceKind`'s
  `Struct { layout: TyId }`, `Enum { layout: TyId }`, and
  `Union { layout: TyId }` reference `Ty` entries that stage 3
  must populate with field structure (`Ty::Struct { fields }`,
  `Ty::Enum { variants }`, `Ty::Union { fields }`) and resolved
  layout (per the C-semantics methodology rule:
  `repr(C)` / `repr(packed)` become a resolved `Layout` field, not
  an annotation passthrough).

- **Fill `pretty.rs::render_ty` stub.** The stage-1 stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 3 fills render_ty")`. Stage 3 replaces
  the body with a renderer that produces strings like `u32`,
  `*const u8`, `&mut T`, etc., per the type-rendering convention
  the spec leaves to stage 3.

- **Pointer / reference provenance field.** Per the C-semantics
  methodology rule, pointer types should carry a `provenance`
  enum (`SharedBorrow` for `&T`, `UniqueBorrow` for `&mut T`,
  `Raw` for `*const T` / `*mut T`) so codegen MAY emit
  `T* restrict` in declaration contexts where uniqueness is
  preserved. The provenance is decided per-construct, not baked
  into the type unconditionally.

- **Niche optimization for `Option<TyId>`.** Stage 1's
  `lib.rs` includes a compile-time assertion that
  `Option<TyId>` is 4 bytes. The assertion is structural and
  will pass automatically once `TyId` retains the niche-encoded
  representation (which the `newtype_index!` macro provides
  for free). Stage 3 should NOT change `TyId`'s representation
  in a way that breaks the assertion.

## Bridge concerns surfaced by stage 1 (act on these alongside
##  stage 3 work)

- **`Ty` interning.** Bridge work for stage 3 must intern Ty's
  through a `tcx.Ty<'tcx> -> TyId` mapping during construction.
  Mirror the F4 pattern used for items: deterministic ordering,
  bridge-internal HashMap that does not survive past bridge call.
