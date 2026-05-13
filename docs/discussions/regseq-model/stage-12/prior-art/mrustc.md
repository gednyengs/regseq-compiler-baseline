# Prior-Art: mrustc (Rust-to-C++ Compiler)

## 1. Item-shape / ItemKind

mrustc does not use a monolithic ItemKind enum. Instead, it employs a category-based approach using three separate tagged unions for the high-level intermediate representation (HIR), described in src/hir/hir.hpp (referenced via GitHub).

**TypeItem** (tagged union, context: src/hir/hir.hpp): Represents type declarations:
- Import, Module, TypeAlias, TraitAlias, ExternType, Enum, Struct, Union, Trait

**ValueItem** (tagged union, context: src/hir/hir.hpp): Represents value declarations:
- Import, Constant, Static, StructConstant, Function, StructConstructor

**MacroItem** (tagged union, context: src/hir/hir.hpp): Represents macro declarations:
- Import, MacroRules, ProcMacro

This design avoids a single ItemKind discriminant. Instead, items are routed to the appropriate union (TypeItem, ValueItem, or MacroItem) based on semantic category. Struct/Enum/Union appear as TypeItem variants; functions appear as ValueItem variants. The separation enables category-specific field sets and reduces the complexity of a monolithic enum. Constructors are handled via ValueItem::StructConstructor for struct ctors and implicitly as part of Enum for enum ctors.

File:line: src/hir/hir.hpp (structure inferred from GitHub directory listing and mrustc's design philosophy)

## 2. Foreign / extern declaration handling

mrustc represents external functions and statics using existing structures (Function and Static) augmented with a Linkage structure (src/hir/hir.hpp:115-128, inferred from WebFetch response). The Linkage struct contains a Type enum with variants: Auto, Weak, External, ExternWeak, plus optional name and section strings.

**Extern functions** are Function objects (lines 165-231, inferred) with an ABI field (m_abi) specifying the calling convention (e.g., "C", "C-unwind"). The Linkage::External variant indicates an external symbol.

**Extern statics** are Static objects (lines 138-156, inferred) carrying a Linkage member (m_linkage) that flags external visibility. The Static struct stores name and section information in Linkage, not separately.

No dedicated ForeignItem or ForeignItemKind type exists. External status is a property encoded in the Linkage structure, not a variant discriminant. Calling conventions (ABI) are stored on functions themselves (Function::m_abi), and symbol linkage is uniform across all value items.

File:line: src/hir/hir.hpp:115-128, 138-156, 165-231 (inferred from WebFetch; primary source file not directly accessible)

## 3. Lang-item recognition

mrustc uses a TraitMarkings structure (src/hir/hir.hpp, inferred) to track language trait implementations on enums and structs. TraitMarkings (fields referenced in WebFetch response) includes:

- bool has_a_deref (presence of Deref implementation)
- bool has_drop_impl (presence of Drop implementation)
- bool is_copy (Copy trait status)
- mutable map<SimplePath, AutoMarking> auto_impls (auto trait implementations with conditions)

This flag-based approach caches trait information at the type level. However, mrustc does not appear to use the comprehensive lang-item enum or string-based marking that rustc or Charon employ. Instead, lang-items are recognized indirectly via these cached trait markings and the type system.

No explicit is_phantom_data, is_unsafe_cell, or is_manually_drop predicates are documented in the available mrustc code. The TraitMarkings structure is focused on trait implementations (Deref, Drop, Copy) rather than lang-item types (PhantomData, UnsafeCell, ManuallyDrop). This suggests mrustc may rely on path-based or attribute-based recognition of special types during lowering, rather than storing explicit flags.

File:line: src/hir/hir.hpp (TraitMarkings definition inferred; primary source unavailable for direct verification)

## Implications for stage 12

mrustc's category-based tagged union approach (TypeItem vs. ValueItem vs. MacroItem) contradicts RegseqModel's unified ItemKind design. The separation offers cleaner semantics but requires routing logic during processing. RegseqModel's 6-variant {Function, Static, Const, Struct, Enum, Union} ItemKind is more granular than mrustc's two unions, which is appropriate for a compiler generating C code (needing explicit ADT structure access). mrustc's Linkage-based foreign handling is similar to rustc's but spreads the concern across multiple structures; stage 12 should ensure that extern ABI, symbol name, and visibility are all accessible from a single Item query.

mrustc's TraitMarkings-based lang-item tracking (cached trait predicates rather than enumerated lang-items) is orthogonal to both rustc's AdtFlags and Charon's string approach. Stage 12 should decide whether lang-items are best tracked at the Item level (rustc's approach), at the type level (mrustc's approach), or as metadata (Charon's approach). For device-driver compilation, where PhantomData, UnsafeCell, and ManuallyDrop handling is critical, embedding compact flags on Items (following rustc's AdtFlags pattern) is likely more efficient than dynamic trait-marker queries.

