# Prior-Art: Charon (LLBC IR)

## 1. Item-shape / ItemKind

Charon separates item kinds into three orthogonal structures: ItemKind (for context), TypeDeclKind (for ADT structure), and separate FunDecl/GlobalDecl/TraitDecl structs (src/ast/gast.rs:97-130 and src/ast/types.rs:460-476).

ItemKind (4 variants, src/ast/gast.rs:97-130) captures the scope of a function/global:

- TopLevel
- Closure { info: ClosureInfo }
- TraitDecl { trait_ref, item_name, has_default }
- TraitImpl { impl_ref, trait_ref, item_name, reuses_default }

TypeDeclKind (5 variants, src/ast/types.rs:460-476) represents ADT structure:

- Struct(Vector<FieldId, Field>)
- Enum(Vector<VariantId, Variant>)
- Union(Vector<FieldId, Field>)
- Opaque
- Alias(Ty)
- Error(String)

Functions and globals are not represented as ItemKind variants; instead, they are FunDecl (src/ast/gast.rs:134-151) and GlobalDecl (src/ast/gast.rs:176-192) structs carrying a kind: ItemKind field (indicating scope context) plus signature/body/generics. Struct/Enum/Union are NOT ItemKind variants; they appear only in TypeDeclKind. Traits and trait impls are separated into TraitDecl (src/ast/gast.rs:241+) structs with their own structure.

Function bodies are stored via a Result<Body, Opaque> field (src/ast/gast.rs:150), where Opaque signals an external or opaque function. GlobalDecl bodies are always reduced to an initializer function ID (src/ast/gast.rs:191).

File:line: src/ast/gast.rs:97-192, src/ast/types.rs:460-476

## 2. Foreign / extern declaration handling

Charon does not expose a dedicated ForeignItem or ForeignItemKind. Instead, external functions are represented as ordinary FunDecl with body: Err(Opaque) (src/ast/gast.rs:150, comment at line 148: "Opaque functions are: external functions, or local functions tagged as opaque"). The ItemKind::TopLevel scope indicates a non-trait, non-closure function; whether it is extern is determined by the presence/absence of a body.

Static items and constants are represented as GlobalDecl (src/ast/gast.rs:176-192) with kind: ItemKind and a global_kind: GlobalKind discriminant (Static vs. NamedConst vs. AnonConst, lines 162-172). External statics are (presumably) distinguished via the same Opaque body encoding as functions.

No explicit "extern ABI" field appears at the Item level; Charon abstracts away calling conventions. Signature access is uniform: FunDecl::signature (a FunSig, line 141).

File:line: src/ast/gast.rs:134-192, 162-172

## 3. Lang-item recognition

Charon stores lang-items as an optional string field on ItemMeta (src/ast/meta.rs:231). The comment at line 231 reads: "If the item is built-in, record its internal builtin identifier." This suggests a string-based approach: lang_item: Option<String> stores the lang-item name (e.g., "phantom_data", "unsafe_cell", "manually_drop").

No AdtFlags bitfield, no LangItem enum, and no predicate methods (is_phantom_data, is_unsafe_cell, etc.) are exposed in the primary AST. Lang-item recognition is deferred to post-processing: it appears that Charon stores the lang-item name as a string and relies on path/attribute-based matching during extraction from Rust source.

No dedicated Drop trait or drop_in_place recognition mechanism is visible in the sampled code; Charon likely treats Drop as an ordinary trait and drop_in_place as a function with a special lang-item marker.

File:line: src/ast/meta.rs:220-232

## Implications for stage 12

Charon's separation of ItemKind (scope context) from TypeDeclKind (ADT structure) differs fundamentally from RegseqModel's design, which unifies them. Charon's approach would require stage 12 to split ItemKind into a scope discriminant and a separate ADT classifier, adding indirection. Charon's Result<Body, Opaque> encoding for external functions aligns with rustc_hir's has_body boolean but is less explicit. Charon's string-based lang-item storage (src/ast/meta.rs:231) is less efficient than rustc's bitflags or enum-based approach; stage 12 should consider a compact representation (bitflags or enum index) for performance-critical checks during code generation. Charon's lack of an explicit ForeignItem struct mirrors rustc_public's design but offers less clarity than rustc_hir; stage 12 should determine whether separate foreign-item handling is necessary for extern "C" function ABI and symbol visibility control in device-driver compilation.

