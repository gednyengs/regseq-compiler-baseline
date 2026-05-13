# Prior-Art: rustc (rustc_hir/rustc_middle)

## 1. Item-shape / ItemKind

Rustc exposes ItemKind as a 14-variant enum in rustc_hir::hir (rustc_hir/src/hir.rs:4397). The variants are:

- ExternCrate(Option<Symbol>, Ident)
- Use(&'hir UsePath<'hir>, UseKind)
- Static(Mutability, Ident, &'hir Ty<'hir>, BodyId)
- Const(Ident, &'hir Generics<'hir>, &'hir Ty<'hir>, ConstItemRhs<'hir>)
- Fn { sig: FnSig<'hir>, ident: Ident, generics: &'hir Generics<'hir>, body: BodyId, has_body: bool }
- Macro(Ident, &'hir ast::MacroDef, MacroKinds)
- Mod(Ident, &'hir Mod<'hir>)
- ForeignMod { abi: ExternAbi, items: &'hir [ForeignItemId] }
- GlobalAsm { asm: &'hir InlineAsm<'hir>, fake_body: BodyId }
- TyAlias(Ident, &'hir Generics<'hir>, &'hir Ty<'hir>)
- Enum(Ident, &'hir Generics<'hir>, EnumDef<'hir>)
- Struct(Ident, &'hir Generics<'hir>, VariantData<'hir>)
- Union(Ident, &'hir Generics<'hir>, VariantData<'hir>)
- Trait(Constness, IsAuto, Safety, Ident, &'hir Generics<'hir>, GenericBounds<'hir>, &'hir [TraitItemId])
- TraitAlias(Constness, Ident, &'hir Generics<'hir>, GenericBounds<'hir>)
- Impl(Impl<'hir>)

Struct/Enum/Union are first-class ItemKind variants (lines 4446-4448), not recovered from type-system. Functions carry their signature inline (FnSig<'hir>) plus a BodyId reference and a has_body boolean flag (lines 4415-4424). Bodies are stored out-of-line in a separate hashmap keyed by BodyId. Traits and Impls are ItemKind variants; trait items and impl items are separate from items (stored as TraitItemId and ImplItemId arrays).

File:line: rustc_hir/src/hir.rs:4397-4464

## 2. Foreign / extern declaration handling

Rustc separates foreign items into a distinct ForeignItem struct in rustc_hir::hir (rustc_hir/src/hir.rs:4567-4602). ForeignItemKind (lines 4590-4602) has 3 variants:

- Fn(FnSig<'hir>, &'hir [Option<Ident>], &'hir Generics<'hir>)
- Static(&'hir Ty<'hir>, Mutability, Safety)
- Type

A ForeignMod item (ItemKind::ForeignMod at line 4430) carries an ABI and a slice of ForeignItemId references. Foreign functions carry their signature directly (no BodyId), and a has_body=false encoding appears only in domestic ItemKind::Fn (line 4423). The extern ABI is stored on ForeignMod, not per-item. Extern item signature access is direct via the FnSig field.

File:line: rustc_hir/src/hir.rs:4429-4430, 4590-4602

## 3. Lang-item recognition

Rustc uses a hybrid mechanism (rustc_hir/src/lang_items.rs:165+ and rustc_middle/src/ty/adt.rs:30-63). Lang items are defined as a LangItem enum in rustc_hir (lang_items.rs lines 79-135), populated via a macro table specifying name, target (Struct/Trait/Fn), and generic requirements. For ADT lang-items, rustc uses AdtFlags bitflags (rustc_middle/src/ty/adt.rs:30-63) with dedicated flag bits: IS_PHANTOM_DATA (bit 4), IS_BOX (bit 6), IS_UNSAFE_CELL (bit 9), IS_MANUALLY_DROP (bit 7), IS_PIN (bit 11).

Recognition is dual-path: (1) Predicate-based via AdtDef methods is_phantom_data() (line 414), is_unsafe_cell() (line 426), is_manually_drop() (line 438), which test flag bits; (2) Query-based via TyCtxt::as_lang_item(def_id: DefId) -> Option<LangItem> (rustc_middle/src/middle/lang_items.rs:30-32), performing a reverse lookup from DefId to LangItem.

Traits like Drop (sym::drop, Target::Trait, lang_items.rs line 196) and drop_in_place (sym::drop_in_place, Target::Fn, line 336) are stored in a LanguageItems struct keyed by LangItem enum variant, indexed by u32. Path-string matching (e.g., "std::mem::drop") is avoided; all items are mapped by DefId bijection.

File:line: rustc_hir/src/lang_items.rs:79-144, 165-200+; rustc_middle/src/ty/adt.rs:30-63, 414-440

## Implications for stage 12

RegseqModel's design choice to expose Struct/Enum/Union as ItemKind variants aligns with rustc_hir's first-class representation. Rustc's separation of ForeignItem as a distinct data structure (vs. encoding in ItemKind) suggests stage 12 should consider a dedicated foreign-item dispatch path rather than reusing ItemKind::Function { body: None }. Rustc's dual recognition mechanism (predicate flags + bijective query) supports both fast checks and reverse lookups; stage 12's construction-invariant closure should track which lang-items appear in scope to enable early warning if a lang-item assumption (e.g., PhantomData presence) is violated. Rustc's has_body field (not body=None) cleanly distinguishes declared-but-unimplemented functions, a pattern RegseqModel should adopt.

