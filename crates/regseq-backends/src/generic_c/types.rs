//! `Ty`->C spelling (`generic-c-backend.md` section 2 / section 3) plus the
//! section 3.2 fat-pointer struct shapes.
//!
//! [`declare`] is the C declarator-aware spelling: it places a declarator
//! (a field / local name, possibly empty) into the correct syntactic slot
//! for the type, which matters for arrays (`T name[N]`) and function
//! pointers (`R (*name)(args)`). [`c_type`] is the bare type spelling
//! (`declare` with an empty declarator), used where no declarator is
//! needed (e.g. the `data` slot element type of a fat-pointer struct).
//!
//! References / pointers to *unsized* pointees (`Str` / `Slice` /
//! `Dynamic`) do not lower to `T*`; they lower to a by-value fat struct
//! (section 3.2) whose name is resolved through the [`super::NameTable`].

use regseq_model::{FloatKind, IntKind, Mutability, Provenance, RegseqModel, Ty, TyId, UintKind};

use super::{slice_data_const, NameTable, Node};

/// The C spelling of an integer scalar (section 2).
fn int_spelling(kind: IntKind) -> &'static str {
    match kind {
        IntKind::I8 => "int8_t",
        IntKind::I16 => "int16_t",
        IntKind::I32 => "int32_t",
        IntKind::I64 => "int64_t",
        IntKind::I128 => "__int128",
        IntKind::Isize => "intptr_t",
    }
}

/// The C spelling of an unsigned scalar (section 2).
fn uint_spelling(kind: UintKind) -> &'static str {
    match kind {
        UintKind::U8 => "uint8_t",
        UintKind::U16 => "uint16_t",
        UintKind::U32 => "uint32_t",
        UintKind::U64 => "uint64_t",
        UintKind::U128 => "__uint128_t",
        UintKind::Usize => "size_t",
    }
}

/// The C spelling of a float scalar (section 2).
fn float_spelling(kind: FloatKind) -> &'static str {
    match kind {
        FloatKind::F16 => "_Float16",
        FloatKind::F32 => "float",
        FloatKind::F64 => "double",
        FloatKind::F128 => "_Float128",
    }
}

/// Join a base type spelling with a (possibly empty) declarator.
fn with_declarator(base: &str, decl: &str) -> String {
    if decl.is_empty() {
        base.to_string()
    } else {
        format!("{base} {decl}")
    }
}

/// The bare C type spelling for a value of `ty` (section 2 / section 3),
/// the form `d`/`e` use for locals, parameters, returns, and fields.
pub(crate) fn c_type(model: &RegseqModel, names: &NameTable, ty: TyId) -> String {
    declare(model, names, ty, "")
}

/// C declarator-aware spelling: produce the declaration of a value of
/// type `ty` named by `decl` (a field / variable name, possibly empty).
///
/// Declarator placement follows C syntax: arrays attach `[N]` to the
/// declarator, function pointers wrap it as `(*decl)`, and a reference /
/// pointer to a *sized* pointee attaches `*`. A reference / pointer to an
/// *unsized* pointee is a by-value fat struct (section 3.2), not a
/// pointer.
pub(crate) fn declare(model: &RegseqModel, names: &NameTable, ty: TyId, decl: &str) -> String {
    declare_qualified(model, names, ty, false, decl)
}

/// [`declare`] with an explicit outer `const` qualifier. `top_const` marks
/// the outermost type constructor of `ty` as `const`-qualified and places
/// the qualifier *adjacent to the type it qualifies*: before the base
/// spelling for a scalar / aggregate, propagated to the element for an array,
/// and after the star (`T *const`) for a pointer value. This is what spells
/// nested pointers such as `& &u32` correctly (`const uint32_t *const *`)
/// rather than prefixing `const` to the whole rendered inner (which would
/// emit `const const uint32_t **` and qualify the wrong level).
fn declare_qualified(
    model: &RegseqModel,
    names: &NameTable,
    ty: TyId,
    top_const: bool,
    decl: &str,
) -> String {
    let q = |base: &str| -> String {
        if top_const {
            format!("const {base}")
        } else {
            base.to_string()
        }
    };
    match &model.types[ty] {
        Ty::Bool => with_declarator(&q("_Bool"), decl),
        Ty::Char => with_declarator(&q("uint32_t"), decl),
        Ty::Int(kind) => with_declarator(&q(int_spelling(*kind)), decl),
        Ty::Uint(kind) => with_declarator(&q(uint_spelling(*kind)), decl),
        Ty::Float(kind) => with_declarator(&q(float_spelling(*kind)), decl),
        // `Never` in return position is `void`; it is a ZST elsewhere and
        // is omitted as a field, so this spelling is only reached by `d`/`e`
        // for a divergent return.
        Ty::Never => with_declarator(&q("void"), decl),
        Ty::Struct { .. } | Ty::Tuple { .. } | Ty::Closure(_, _) | Ty::Enum { .. } => {
            with_declarator(&q(&format!("struct {}", names.agg_name(ty))), decl)
        }
        Ty::Union { .. } => with_declarator(&q(&format!("union {}", names.agg_name(ty))), decl),
        // A `const` array is an array of `const` elements: the qualifier
        // propagates to the element base (`const uint32_t arr[4]`).
        Ty::Array(elem, len) => {
            declare_qualified(model, names, *elem, top_const, &format!("{decl}[{len}]"))
        }
        Ty::Ref(pointee, prov) => pointer_decl(
            model,
            names,
            ty,
            *pointee,
            ref_is_const(model, *pointee, *prov),
            top_const,
            decl,
        ),
        Ty::Ptr(pointee, mutability) => pointer_decl(
            model,
            names,
            ty,
            *pointee,
            *mutability == Mutability::Not,
            top_const,
            decl,
        ),
        Ty::FnPtr(sig) => {
            let inner = fn_ptr_decl(model, names, *sig, decl);
            // Const-qualifying a function type is exotic and only reachable
            // via a (rare) pointer-to-fn-ptr, outside this task's verified
            // scope; a prefix keeps it syntactically valid.
            if top_const {
                format!("const {inner}")
            } else {
                inner
            }
        }
        // `FnDef` is a ZST function-identity tag with no C value emission.
        Ty::FnDef(_, _) => with_declarator(&q("/* FnDef ZST */"), decl),
        // Bare unsized markers never appear as a value type in a valid
        // model (they only occur behind `Ref`/`Ptr`, handled above).
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } => {
            with_declarator(&q("void /* bare unsized marker */"), decl)
        }
    }
}

/// `Ref(t, SharedBorrow)` is `const T*` unless `t` contains an
/// `UnsafeCell` (section 3.1); `Ref(t, UniqueBorrow)` is mutable `T*`.
fn ref_is_const(model: &RegseqModel, pointee: TyId, prov: Provenance) -> bool {
    match prov {
        Provenance::SharedBorrow => !model.contains_unsafe_cell(pointee),
        Provenance::UniqueBorrow => false,
    }
}

/// Declarator for a `Ref` / `Ptr`. An unsized pointee lowers to a
/// by-value fat struct (section 3.2); a sized pointee lowers to a pointer
/// via [`ptr_to`]. `pointee_const` qualifies the pointed-to type; `self_const`
/// qualifies this pointer *value* itself (set when it is the const pointee of
/// an enclosing shared reference, e.g. the inner pointer of `& &u32`).
fn pointer_decl(
    model: &RegseqModel,
    names: &NameTable,
    ref_ty: TyId,
    pointee: TyId,
    pointee_const: bool,
    self_const: bool,
    decl: &str,
) -> String {
    if let Some(fat) = unsized_pointee_struct_name(model, names, ref_ty) {
        // The fat struct is a by-value aggregate, so `self_const` prefixes
        // its base spelling like any other leaf.
        let base = format!("struct {fat}");
        let qualified = if self_const {
            format!("const {base}")
        } else {
            base
        };
        return with_declarator(&qualified, decl);
    }
    ptr_to(model, names, pointee, pointee_const, self_const, decl)
}

/// Declare a pointer to a sized `pointee` named by `decl`. `pointee_const`
/// qualifies the pointed-to type (placed adjacent to it by the recursive
/// [`declare_qualified`]); `self_const` qualifies the pointer value itself
/// (`T *const decl`). The pointer operator binds looser than the array `[N]`
/// and function `(...)` declarators, so a pointer to an array or function
/// pointer parenthesizes the declarator (`uint32_t (*p)[4]`, not the
/// array-of-pointers `uint32_t *p[4]`).
fn ptr_to(
    model: &RegseqModel,
    names: &NameTable,
    pointee: TyId,
    pointee_const: bool,
    self_const: bool,
    decl: &str,
) -> String {
    let needs_parens = matches!(model.types[pointee], Ty::Array(_, _) | Ty::FnPtr(_));
    // `self_const` sits immediately after this level's star (`*const`), so it
    // qualifies the pointer; `pointee_const` is placed by the recursion.
    let star = if self_const { "*const " } else { "*" };
    let ptr_decl = if needs_parens {
        format!("({star}{decl})")
    } else {
        format!("{star}{decl}")
    };
    declare_qualified(model, names, pointee, pointee_const, &ptr_decl)
}

/// If `ref_ty` is a `Ref` / `Ptr` to an unsized pointee, the name of the
/// by-value fat struct it lowers to (section 3.2): `rust_str` for `Str`
/// (bank 1, monomorphic), the per-element `rust_slice_<T>` for `Slice`,
/// or the per-trait `trait_obj_<Trait>` for `Dynamic`. Returns `None`
/// for a sized pointee.
pub(crate) fn unsized_pointee_struct_name(
    model: &RegseqModel,
    names: &NameTable,
    ref_ty: TyId,
) -> Option<String> {
    let pointee = match &model.types[ref_ty] {
        Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
        _ => return None,
    };
    match &model.types[pointee] {
        Ty::Str => Some("rust_str".to_string()),
        Ty::Slice(elem) => Some(
            names
                .name(&Node::Slice {
                    elem: *elem,
                    const_data: slice_data_const(model, ref_ty),
                })
                .to_string(),
        ),
        Ty::Dynamic { .. } => Some(names.name(&Node::TraitObj { dynamic: pointee }).to_string()),
        _ => None,
    }
}

/// Function-pointer declarator `R (*decl)(params)` (section 2). The return
/// type `R` is rendered through the recursive [`declare`] with the function
/// declarator `(*decl)(params)` placed *inside* it, so a return that is
/// itself a function pointer or pointer-to-array recurses the declarator
/// (`fn() -> fn() -> i32` becomes `int32_t (*(*decl)(void))(void)`) instead of
/// the invalid bare-type concatenation `int (*)(void) (*decl)(void)`.
fn fn_ptr_decl(
    model: &RegseqModel,
    names: &NameTable,
    sig: regseq_model::FnSigId,
    decl: &str,
) -> String {
    let fn_sig = model.sigs.resolve(sig);
    let params = if fn_sig.inputs.is_empty() {
        "void".to_string()
    } else {
        fn_sig
            .inputs
            .iter()
            .map(|input| c_type(model, names, *input))
            .collect::<Vec<_>>()
            .join(", ")
    };
    declare(model, names, fn_sig.output, &format!("(*{decl})({params})"))
}

/// Emit the section 3.2 `struct rust_slice_<T>` body: a `data` pointer
/// (const per the element + borrow flavor) and a `size_t len`.
pub(crate) fn emit_slice_struct(
    model: &RegseqModel,
    names: &NameTable,
    struct_name: &str,
    elem: TyId,
    const_data: bool,
) -> String {
    // The `data` slot is a (const?) pointer to the element type; use the
    // declarator builder so a non-trivial element (e.g. `[u32; 4]`) gets
    // correct C syntax (`const uint32_t (*data)[4]`). The pointer member
    // itself is not const (only its pointee, per `const_data`).
    let data = ptr_to(model, names, elem, const_data, false, "data");
    format!(
        "struct {struct_name} {{\n\
         \x20\x20\x20\x20{data};\n\
         \x20\x20\x20\x20size_t len;\n\
         }};\n"
    )
}

/// Emit the section 3.2 / section 3.3 `struct trait_obj_<Trait>` body:
/// an unqualified `void *data` slot (section 3.3 erasure) and a
/// `const <Trait>_VTable *vtable`. The VTable definition, the vtable
/// statics, and virtual dispatch are deferred (scope decision 2); only the
/// shape plus the VTable forward-decl (emitted in bank 2) ship here.
pub(crate) fn emit_trait_obj_struct(struct_name: &str, vtable_name: &str) -> String {
    format!(
        "struct {struct_name} {{\n\
         \x20\x20\x20\x20void *data;\n\
         \x20\x20\x20\x20const struct {vtable_name} *vtable;\n\
         }};\n"
    )
}
