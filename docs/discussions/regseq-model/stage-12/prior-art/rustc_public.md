# Prior-Art: rustc_public (Stable IR)

## 1. Item-shape / ItemKind

Rustc_public exposes ItemKind as a minimal 4-variant enum (rustc_public/src/lib.rs:120-125):

- Fn
- Static
- Const
- Ctor(CtorKind)

CtorKind (lines 128-131) has 2 variants: Const and Fn. This is a stable public IR designed for compiler plugins and external tools. Notably, struct/enum/union type definitions are NOT ItemKind variants; instead, they are represented as separate CrateItem instances, and ADT kind is recovered from the type system via CrateItem::ty() (line 169). Traits and trait impls are similarly absent from ItemKind; they are accessed via separate queries (all_trait_decls, all_trait_impls, lines 212-217).

Functions are referenced by CrateItem opaque identifiers; the function's signature and MIR body are accessed via CrateItem methods: body() (line 149), expect_body() (line 143), has_body() (line 153). The ItemKind enum itself carries no field data; it is purely a discriminant. Struct/enum/union constructors are handled via the Ctor(CtorKind) variant, collapsing struct and enum constructors into a single ItemKind case.

**Per-Instance body access (separate from CrateItem).** The `Instance` type at `rustc_public/src/mir/mono.rs` exposes a per-Instance body accessor `Instance::body() -> Option<Body>` at lines 53-55. This is structurally separate from `CrateItem::body()` at `rustc_public/src/lib.rs:149` -- `CrateItem` accessors are keyed by the source-level item DefId, while `Instance::body()` returns the post-monomorphization body for the specific (DefId, GenericArgs) instance. Two monomorphizations of a generic fn yield two distinct Instances, each with its own `Instance::body()` return value (post-mono substitution applied).

File:line: rustc_public/src/lib.rs:120-125, 128-131, 141-183; rustc_public/src/mir/mono.rs:53-55

## 2. Foreign / extern declaration handling

Rustc_public exposes an is_foreign_item() method on CrateItem (line 173), but does not expose a dedicated foreign item type. The public IR abstracts away ForeignItem as a detail. Extern functions and statics are represented as ordinary Fn and Static ItemKind variants; the is_foreign_item() boolean predicate distinguishes them from domestic items. No explicit "body=None" encoding or has_body field is exposed at the ItemKind level; the is_foreign_item() boolean answers whether a body is available (implied by !is_foreign_item() -> has_body()).

This design suggests that rustc_public treats foreign status as a property of the item (a query), not a structural feature of ItemKind.

File:line: rustc_public/src/lib.rs:173-175

## 3. Lang-item recognition

Rustc_public does not expose lang-item recognition directly in its public API. The stable IR assumes lang-items have been collected and registered during the earlier rustc compilation phase. No LangItem enum, no AdtFlags, and no is_phantom_data/is_unsafe_cell predicates are exposed.

The public IR is designed to work with an already-compiled crate where lang-items are fixed and opaque. A consumer tool (e.g., Miri or a plugin) must rely on rustc's internal lang_items query or external metadata to identify special types.

File:line: rustc_public/src/lib.rs does not define lang-item handling.

## Implications for stage 12

Rustc_public's abstraction of ADT kind via the type system (not ItemKind variants) contradicts RegseqModel's design to expose Struct/Enum/Union as first-class variants. This difference arises because rustc_public is a minimal stable interface (only 4 ItemKind variants) for external tools that do not need fine-grained control over ADT structure; RegseqModel compiling device drivers to C requires explicit ADT variant access and inline body representation. Rustc_public's is_foreign_item() boolean (line 173) is simpler than rustc_hir's ForeignItem struct but less informative; stage 12 should decide whether a boolean or a dedicated ForeignItem variant better serves the device-driver compilation pipeline. Rustc_public's lack of exposed lang-item predicates suggests that stage 12 should embed lang-item recognition early (as AdtFlags do in rustc_middle) or defer to a pre-compilation indexing step, rather than performing expensive lookups during IR lowering.

