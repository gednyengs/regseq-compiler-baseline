//! Tests for the generic-C backend driver (task c1 banks 2-3).
//!
//! The bank-1 prelude tests (task b) remain; the new tests build small
//! hand-constructed models exercising the section 14 field-offset emitter,
//! D1 naming + the `_clash_<n>` net, the section 3.2 fat structs, and the
//! enum / `dyn` scope hooks. Compile-level layout-fidelity proof (the
//! `_Static_assert` guards holding / biting against a host `cc`) lives in
//! the `codegen-coverage-runner` behavioral tests, which own the toolchain.

use super::*;
use regseq_model::{
    AutoTrait, CrateId, DefPath, Endian, EnumShape, FieldLayout, FnAbi, FnSig, GenericArgs,
    Instance, InstanceKind, IntKind, Item, ItemKind, Layout, LayoutKind, Linkage, Mutability,
    Provenance, Safety, ScalarLayouts, SizeAlign, SourceFileId, Span, StableCrateId, TagEncoding,
    TagScalar, TargetInfo, Ty, TyId, UintKind, VariantIdx, VariantLayout, VariantRange,
    WrappingRange,
};

// ---------------------------------------------------------------------------
// Task-b prelude tests (unchanged): bank 1 is model-independent, banks 2-5
// are empty for the trivial model, so the emitted TU is the prelude.
// ---------------------------------------------------------------------------

fn trivial_model() -> RegseqModel {
    RegseqModel::with_primary("trivial", StableCrateId(1))
}

#[test]
fn emit_returns_ok_and_never_not_implemented() {
    let tu = emit_generic_c(&trivial_model()).expect("emit must return Ok");
    assert!(!tu.is_empty());
}

#[test]
fn tu_starts_with_the_includes() {
    let tu = emit_generic_c(&trivial_model()).unwrap();
    assert!(tu.contains("#include <stdint.h>"));
    assert!(tu.contains("#include <stddef.h>"));
    assert!(tu.contains("#include <stdio.h>"));
    assert!(tu.contains("#include <stdlib.h>"));
    assert!(tu.contains("#include <string.h>"));
    assert!(tu.contains("#include <math.h>"));
    let inc = tu.find("#include <stdint.h>").unwrap();
    let helpers = tu.find("regseq_add_int8_t").unwrap();
    assert!(inc < helpers);
}

#[test]
fn tu_contains_rust_str_panic_and_unreachable() {
    let tu = emit_generic_c(&trivial_model()).unwrap();
    assert!(tu.contains("struct rust_str {"));
    assert!(tu.contains("const uint8_t *data;"));
    assert!(tu.contains("_Noreturn static inline void regseq_panic(const char *msg)"));
}

#[test]
fn emit_is_byte_identical_across_two_calls() {
    let a = emit_generic_c(&trivial_model()).unwrap();
    let b = emit_generic_c(&trivial_model()).unwrap();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Hand-built model scaffolding for the task-c1 layout / naming tests.
// ---------------------------------------------------------------------------

/// A 64-bit target with realistic C scalar sizes, so `size_align_of` and
/// the hand-set `FieldLayout` offsets stay mutually consistent.
fn model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("c1", StableCrateId(1));
    let triple = model.strings.intern("x86_64");
    let data_layout = model.strings.intern("dl");
    let sa = |size, align| SizeAlign { size, align };
    model.target = TargetInfo {
        triple,
        pointer_width: 64,
        endian: Endian::Little,
        data_layout,
        scalar_layouts: ScalarLayouts {
            bool_: sa(1, 1),
            char_: sa(4, 4),
            int: [sa(1, 1), sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16), sa(8, 8)],
            uint: [sa(1, 1), sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16), sa(8, 8)],
            float: [sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16)],
            fn_ptr: sa(8, 8),
            data_ptr: sa(8, 8),
        },
    };
    model
}

fn span() -> Span {
    Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    }
}

fn item(model: &mut RegseqModel, kind: ItemKind, rendered: &str) -> regseq_model::ItemId {
    let rid = model.strings.intern(rendered);
    model.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind,
        span: span(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    })
}

fn fl(model: &mut RegseqModel, name: &str, ty: TyId, offset: u64) -> FieldLayout {
    FieldLayout {
        name: model.strings.intern(name),
        ty,
        offset,
    }
}

fn struct_layout(size: u64, align: u64, pack: Option<u32>, fields: Vec<FieldLayout>) -> Layout {
    Layout {
        size,
        align,
        pack,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

/// Intern a nominal `Ty::Struct` (non-generic) and its `Item`, returning
/// its `TyId`. Does NOT register an instance, so the struct is only
/// discovered if some other root reaches it by containment (used for the
/// `dyn` vtable type, which names the trait but is not itself emitted).
fn intern_struct(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = item(model, ItemKind::Struct, rendered);
    model.types.intern(Ty::Struct {
        item: it,
        args: GenericArgs::new(),
        layout,
    })
}

/// Register an `InstanceKind::Struct` root for an already-interned
/// `Ty::Struct`.
fn register_struct_root(model: &mut RegseqModel, ty: TyId, rendered: &str) {
    let it = match &model.types[ty] {
        Ty::Struct { item, .. } => *item,
        other => panic!("register_struct_root on non-struct: {other:?}"),
    };
    let emission = model.strings.intern(rendered);
    model.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Struct { layout: ty },
    });
}

/// Intern a nominal `Ty::Struct` AND register it as a root.
fn push_struct_instance(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let ty = intern_struct(model, rendered, layout);
    register_struct_root(model, ty, rendered);
    ty
}

fn uint(model: &mut RegseqModel, kind: UintKind) -> TyId {
    model.types.intern(Ty::Uint(kind))
}

// ---------------------------------------------------------------------------
// B2: layout fidelity (Change C / F) -- string-level structural assertions.
// ---------------------------------------------------------------------------

#[test]
fn repr_rust_struct_sorts_fields_pads_and_guards() {
    // Point { a: u8 @ off 4, b: u32 @ off 0 }, size 8 align 4 (repr(Rust)
    // reordered: b precedes a in memory). Source order is [a, b].
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let a = fl(&mut m, "a", u8t, 4);
    let b = fl(&mut m, "b", u32t, 0);
    push_struct_instance(&mut m, "Point", struct_layout(8, 4, None, vec![a, b]));

    let tu = emit_generic_c(&m).unwrap();
    // Fields emitted in offset order: b (uint32_t) then a (uint8_t).
    let b_pos = tu.find("uint32_t b;").expect("b field");
    let a_pos = tu.find("uint8_t a;").expect("a field");
    assert!(b_pos < a_pos, "fields must be sorted by offset");
    // Trailing pad from byte 5 to size 8.
    assert!(tu.contains("unsigned char _pad"));
    assert!(tu.contains("__attribute__((aligned(4)))"));
    assert!(tu.contains("_Static_assert(sizeof(struct Point) == 8, \"Point size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Point) == 4, \"Point align\");"));
    assert!(tu.contains("_Static_assert(offsetof(struct Point, b) == 0, \"Point.b offset\");"));
    assert!(tu.contains("_Static_assert(offsetof(struct Point, a) == 4, \"Point.a offset\");"));
}

#[test]
fn packed_struct_emits_packed_attribute_with_layout_align() {
    // #[repr(packed)] P { x: u8 @0, y: u32 @1 }, size 5 align 1.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let x = fl(&mut m, "x", u8t, 0);
    let y = fl(&mut m, "y", u32t, 1);
    push_struct_instance(&mut m, "Packed", struct_layout(5, 1, Some(1), vec![x, y]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("__attribute__((packed, aligned(1)))"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Packed) == 1, \"Packed align\");"));
    assert!(tu.contains("_Static_assert(offsetof(struct Packed, y) == 1, \"Packed.y offset\");"));
}

#[test]
fn array_field_uses_declarator_placement() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let f = fl(&mut m, "arr", arr, 0);
    push_struct_instance(&mut m, "WithArr", struct_layout(16, 4, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("uint32_t arr[4];"));
    assert!(tu.contains("_Static_assert(sizeof(struct WithArr) == 16, \"WithArr size\");"));
}

#[test]
fn union_emits_c_union_with_zero_offset_guards() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let u8t = uint(&mut m, UintKind::U8);
    let x = fl(&mut m, "x", u32t, 0);
    let y = fl(&mut m, "y", u8t, 0);
    let it = item(&mut m, ItemKind::Union, "U");
    let ty = m.types.intern(Ty::Union {
        item: it,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![x, y] },
        },
    });
    let emission = m.strings.intern("U");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Union { layout: ty },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("union U {"));
    assert!(tu.contains("uint32_t x;"));
    assert!(tu.contains("uint8_t y;"));
    assert!(tu.contains("_Static_assert(offsetof(union U, x) == 0, \"U.x offset\");"));
    assert!(tu.contains("_Static_assert(offsetof(union U, y) == 0, \"U.y offset\");"));
    assert!(tu.contains("_Static_assert(sizeof(union U) == 4, \"U size\");"));
}

#[test]
fn tuple_field_emits_tuple_struct_with_indexed_fields() {
    // struct Pair { t: (u8, u32) }. Tuple (u8,u32) reorders to u32@0, u8@4,
    // size 8 align 4; source order [u8, u32] -> fields _0, _1.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let t0 = fl(&mut m, "_0", u8t, 4);
    let t1 = fl(&mut m, "_1", u32t, 0);
    let tuple = m.types.intern(Ty::Tuple {
        fields: vec![u8t, u32t],
        layout: struct_layout(8, 4, None, vec![t0, t1]),
    });
    let f = fl(&mut m, "t", tuple, 0);
    push_struct_instance(&mut m, "Pair", struct_layout(8, 4, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct tuple_"), "tuple struct must be named");
    // Indexed field names, sorted by offset (_1 @0 before _0 @4).
    assert!(tu.contains("uint32_t _1;"));
    assert!(tu.contains("uint8_t _0;"));
    // The containing struct references the tuple by value.
    assert!(tu.contains(" t;"));
}

#[test]
fn slice_field_emits_rust_slice_fat_struct() {
    // struct WithSlice { s: &[u8] }, s @0 size 16, struct size 16 align 8.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let slice = m.types.intern(Ty::Slice(u8t));
    let slice_ref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let f = fl(&mut m, "s", slice_ref, 0);
    push_struct_instance(&mut m, "WithSlice", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct rust_slice_uint8_t {"));
    assert!(tu.contains("const uint8_t *data;"));
    assert!(tu.contains("size_t len;"));
    assert!(tu.contains("struct rust_slice_uint8_t s;"));
    // The fat struct is defined before the struct that contains it.
    let slice_def = tu.find("struct rust_slice_uint8_t {").unwrap();
    let holder_def = tu.find("struct WithSlice {").unwrap();
    assert!(
        slice_def < holder_def,
        "topo: contained slice precedes holder"
    );
}

#[test]
fn ref_to_array_uses_parenthesized_pointer_declarator() {
    // struct HasArrRef { p: &[u32; 4] } -- a thin pointer to a sized array.
    // C declarator placement requires `uint32_t (*p)[4]`, not the
    // array-of-pointers `uint32_t *p[4]`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let aref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let f = fl(&mut m, "p", aref, 0);
    push_struct_instance(&mut m, "HasArrRef", struct_layout(8, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint32_t (*p)[4];"),
        "pointer-to-array needs parenthesized declarator"
    );
}

#[test]
fn slice_of_array_data_slot_uses_parenthesized_pointer() {
    // struct HasSliceOfArr { s: &[[u32; 4]] } -- the rust_slice `data` slot
    // is a pointer to the array element, so `const uint32_t (*data)[4]`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let slice = m.types.intern(Ty::Slice(arr));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let f = fl(&mut m, "s", sref, 0);
    push_struct_instance(&mut m, "HasSliceOfArr", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("(*data)[4];"), "slice-of-array data slot");
}

// ---------------------------------------------------------------------------
// B3: naming + determinism (Change A).
// ---------------------------------------------------------------------------

#[test]
fn clash_net_disambiguates_same_base_name() {
    // Two distinct structs whose items render the same name -> the second
    // gets a `_clash_1` suffix.
    let mut m = model();
    let i32a = m.types.intern(Ty::Int(regseq_model::IntKind::I32));
    let fa = fl(&mut m, "v", i32a, 0);
    push_struct_instance(&mut m, "Dup", struct_layout(4, 4, None, vec![fa]));
    let fb = fl(&mut m, "v", i32a, 0);
    push_struct_instance(&mut m, "Dup", struct_layout(4, 4, None, vec![fb]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct Dup {"));
    assert!(tu.contains("struct Dup_clash_1 {"));
}

#[test]
fn emit_is_byte_identical_for_a_rich_model() {
    let build = || {
        let mut m = model();
        let u8t = uint(&mut m, UintKind::U8);
        let u32t = uint(&mut m, UintKind::U32);
        let a = fl(&mut m, "a", u8t, 4);
        let b = fl(&mut m, "b", u32t, 0);
        push_struct_instance(&mut m, "Point", struct_layout(8, 4, None, vec![a, b]));
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

// ---------------------------------------------------------------------------
// B4 scope hook: dyn (trait_obj shape only).
// ---------------------------------------------------------------------------

#[test]
fn dyn_field_emits_trait_obj_shape_and_vtable_forward_decl() {
    // struct Holder { r: &dyn Reader }. The Dynamic's vtable_ty names the
    // trait. r @0 size 16, struct size 16 align 8.
    let mut m = model();
    // Reader names the trait via the Dynamic's vtable_ty; it is interned
    // (for the name) but not rooted, so it is not itself emitted.
    let reader = intern_struct(&mut m, "Reader", struct_layout(4, 4, None, Vec::new()));
    let dynamic = m.types.intern(Ty::Dynamic {
        auto_traits: vec![AutoTrait::Send],
        vtable_ty: reader,
    });
    let dyn_ref = m.types.intern(Ty::Ref(dynamic, Provenance::SharedBorrow));
    let f = fl(&mut m, "r", dyn_ref, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct Reader_VTable;"), "VTable forward-decl");
    assert!(tu.contains("struct trait_obj_Reader {"));
    assert!(tu.contains("void *data;"));
    assert!(tu.contains("const struct Reader_VTable *vtable;"));
    assert!(tu.contains("struct trait_obj_Reader r;"));
}

// ---------------------------------------------------------------------------
// Cycle detection: a by-value containment cycle is a hard error.
// ---------------------------------------------------------------------------

#[test]
fn by_value_cycle_is_a_hard_error() {
    // Two structs that contain each other by value (model corruption; rustc
    // would reject this for sized types).
    let mut m = model();
    let a_item = item(&mut m, ItemKind::Struct, "A");
    let b_item = item(&mut m, ItemKind::Struct, "B");

    // Pre-allocate the TyIds by interning placeholders, then we need each
    // struct's field to point at the other. Build B first (field -> A), but
    // A is not interned yet; intern A as a 1-field struct pointing at a
    // not-yet-final B id. We instead construct the mutual reference by
    // interning A referencing B and B referencing A using their final ids.
    //
    // Intern order: reserve B's id by interning a temporary, then A, then
    // re-point. Simpler: intern A with a field ty equal to B's id, and B
    // with a field ty equal to A's id, computing ids up front.
    let a_ty_id = TyId::from_u32(m.types.len() as u32); // A will be next
    let b_ty_id = TyId::from_u32(m.types.len() as u32 + 1); // then B

    let a_field = fl(&mut m, "b", b_ty_id, 0);
    let a_ty = m.types.intern(Ty::Struct {
        item: a_item,
        args: GenericArgs::new(),
        layout: struct_layout(8, 8, None, vec![a_field]),
    });
    let b_field = fl(&mut m, "a", a_ty_id, 0);
    let b_ty = m.types.intern(Ty::Struct {
        item: b_item,
        args: GenericArgs::new(),
        layout: struct_layout(8, 8, None, vec![b_field]),
    });
    assert_eq!(a_ty, a_ty_id);
    assert_eq!(b_ty, b_ty_id);

    let emission = m.strings.intern("A");
    m.instances.push(Instance {
        item: a_item,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Struct { layout: a_ty },
    });

    assert!(matches!(
        emit_generic_c(&m),
        Err(EmitError::ContainmentCycle)
    ));
}

// ---------------------------------------------------------------------------
// Task-07 robustness fixes.
//
// Fix A: discovery edges (name + forward-decl) are split from by-value
// ordering edges (complete-before), so a slice-only-reachable element no
// longer panics, a non-array element is forward-decl-only (no false cycle),
// and an array element is ordered complete-first.
// ---------------------------------------------------------------------------

#[test]
fn slice_of_tuple_reachable_only_via_slice_is_discovered_no_panic() {
    // struct Holder { s: &[(u32, u32)] } -- the tuple aggregate is reachable
    // ONLY through the slice element. Before Fix A it was never discovered,
    // so spelling the fat struct's `data` field panicked on the NameTable.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let t0 = fl(&mut m, "_0", u32t, 0);
    let t1 = fl(&mut m, "_1", u32t, 4);
    let tuple = m.types.intern(Ty::Tuple {
        fields: vec![u32t, u32t],
        layout: struct_layout(8, 4, None, vec![t0, t1]),
    });
    let slice = m.types.intern(Ty::Slice(tuple));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let f = fl(&mut m, "s", sref, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).expect("element is discovered, must not panic");
    // The tuple element is named + forward-declared + defined.
    assert!(
        tu.contains("struct tuple_"),
        "tuple element discovered + named"
    );
    // The slice fat struct spells `data` as a pointer to the tuple element.
    assert!(tu.contains("*data;"), "slice data slot present:\n{tu}");
}

#[test]
fn slice_of_struct_reachable_only_via_slice_is_discovered() {
    // struct Holder { s: &[Inner] } where Inner is reachable ONLY via slice.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let iv = fl(&mut m, "v", u32t, 0);
    let inner = intern_struct(&mut m, "Inner", struct_layout(4, 4, None, vec![iv]));
    let slice = m.types.intern(Ty::Slice(inner));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let f = fl(&mut m, "s", sref, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).expect("Inner is discovered, must not panic");
    assert!(
        tu.contains("struct Inner;"),
        "Inner forward-declared (bank 2)"
    );
    assert!(tu.contains("struct Inner {"), "Inner body emitted (bank 3)");
}

#[test]
fn self_referential_slice_field_is_not_a_containment_cycle() {
    // struct Inner { xs: &[Inner] } -- Inner contains a slice of itself by a
    // *pointer* (the fat struct's `data`), not by value: the discovery edge
    // names Inner but no ordering edge is added, so it is NOT a cycle.
    let mut m = model();
    let l = m.types.len() as u32;
    let inner_id = TyId::from_u32(l + 2); // after Slice (l) and Ref (l+1)
    let slice = m.types.intern(Ty::Slice(inner_id));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let it = item(&mut m, ItemKind::Struct, "Inner");
    let xs = fl(&mut m, "xs", sref, 0);
    let inner = m.types.intern(Ty::Struct {
        item: it,
        args: GenericArgs::new(),
        layout: struct_layout(16, 8, None, vec![xs]),
    });
    assert_eq!(inner, inner_id);
    register_struct_root(&mut m, inner, "Inner");

    let tu = emit_generic_c(&m).expect("self-ref via a slice pointer is not a cycle");
    assert!(tu.contains("struct Inner {"), "Inner body emitted");
}

#[test]
fn slice_of_array_of_struct_orders_element_before_the_slice() {
    // struct Holder { s: &[[Inner; 4]] }. The slice `data` is
    // `const struct Inner (*data)[4]`, and a C array element must be complete
    // even behind a pointer, so Inner is ordered (complete) before the slice.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let iv = fl(&mut m, "v", u32t, 0);
    let inner = intern_struct(&mut m, "Inner", struct_layout(4, 4, None, vec![iv]));
    let arr = m.types.intern(Ty::Array(inner, 4));
    let slice = m.types.intern(Ty::Slice(arr));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let f = fl(&mut m, "s", sref, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(16, 8, None, vec![f]));

    let tu = emit_generic_c(&m).expect("array-element slice emits Ok");
    let inner_def = tu.find("struct Inner {").expect("Inner body");
    let slice_data = tu.find("(*data)[4]").expect("slice-of-array data slot");
    assert!(
        inner_def < slice_data,
        "Inner must be complete before the slice struct:\n{tu}"
    );
}

#[test]
fn cyclic_array_element_slice_is_a_clean_error_not_a_panic() {
    // struct Inner { xs: &[[Inner; 4]] } -- Inner needs the slice complete as
    // a by-value field while the slice needs Inner complete as its array
    // element: not C-representable. It surfaces as a clean ContainmentCycle,
    // not a panic / silent mis-emission.
    let mut m = model();
    let l = m.types.len() as u32;
    let inner_id = TyId::from_u32(l + 3); // after Array (l), Slice (l+1), Ref (l+2)
    let arr = m.types.intern(Ty::Array(inner_id, 4));
    let slice = m.types.intern(Ty::Slice(arr));
    let sref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let it = item(&mut m, ItemKind::Struct, "Inner");
    let xs = fl(&mut m, "xs", sref, 0);
    let inner = m.types.intern(Ty::Struct {
        item: it,
        args: GenericArgs::new(),
        layout: struct_layout(16, 8, None, vec![xs]),
    });
    assert_eq!(inner, inner_id);
    register_struct_root(&mut m, inner, "Inner");

    assert!(matches!(
        emit_generic_c(&m),
        Err(EmitError::ContainmentCycle)
    ));
}

// ---------------------------------------------------------------------------
// Fix B: pointer/ref `const` is placed *within* the declarator, adjacent to
// the type it qualifies (no prefix-to-the-whole-inner bug).
// ---------------------------------------------------------------------------

/// Emit a single-field `struct Holder { p: <field_ty> }` (field at offset 0,
/// pointer-sized) and return the TU; used by the declarator-placement tests.
fn emit_pointer_field(m: &mut RegseqModel, field_ty: TyId) -> String {
    let f = fl(m, "p", field_ty, 0);
    push_struct_instance(m, "Holder", struct_layout(8, 8, None, vec![f]));
    emit_generic_c(m).unwrap()
}

#[test]
fn shared_ref_to_scalar_places_const_before_the_pointee() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let r = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let tu = emit_pointer_field(&mut m, r);
    assert!(tu.contains("const uint32_t *p;"), "got:\n{tu}");
}

#[test]
fn mut_ref_to_scalar_has_no_const() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let r = m.types.intern(Ty::Ref(u32t, Provenance::UniqueBorrow));
    let tu = emit_pointer_field(&mut m, r);
    assert!(tu.contains("uint32_t *p;"), "got:\n{tu}");
    assert!(!tu.contains("const uint32_t *p;"), "&mut must not be const");
}

#[test]
fn const_raw_ptr_to_scalar_places_const() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let p = m.types.intern(Ty::Ptr(u32t, Mutability::Not));
    let tu = emit_pointer_field(&mut m, p);
    assert!(tu.contains("const uint32_t *p;"), "got:\n{tu}");
}

#[test]
fn nested_shared_ref_qualifies_each_level_correctly() {
    // `& &u32` = pointer to const (pointer to const u32) =
    // `const uint32_t *const *`. The inner pointer is const (it is the const
    // pointee of the outer shared ref), placed as `*const` after the star --
    // never `const const`, never on the wrong level.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let inner_ref = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let outer_ref = m.types.intern(Ty::Ref(inner_ref, Provenance::SharedBorrow));
    let tu = emit_pointer_field(&mut m, outer_ref);
    assert!(tu.contains("const uint32_t *const *p;"), "got:\n{tu}");
    assert!(!tu.contains("const const"), "no doubled const:\n{tu}");
}

// ---------------------------------------------------------------------------
// Fix C: synthetic padding members never collide with a real field name.
// ---------------------------------------------------------------------------

#[test]
fn struct_pad_filler_avoids_a_real_pad0_field() {
    // struct S { _pad0: u8 @0, x: u32 @8 } -- the [1, 8) gap needs a filler;
    // it must not reuse the real `_pad0` field name.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let p0 = fl(&mut m, "_pad0", u8t, 0);
    let x = fl(&mut m, "x", u32t, 8);
    push_struct_instance(&mut m, "S", struct_layout(12, 4, None, vec![p0, x]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint8_t _pad0;"),
        "real _pad0 field present:\n{tu}"
    );
    assert!(
        tu.contains("unsigned char _pad1["),
        "filler skips the taken _pad0:\n{tu}"
    );
    assert!(
        !tu.contains("unsigned char _pad0["),
        "filler must not reuse _pad0:\n{tu}"
    );
}

#[test]
fn union_pad_filler_avoids_a_real_pad_field() {
    // union U { _pad: u8 } with layout size 4 -> a trailing filler that must
    // not collide with the real `_pad` member.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let p = fl(&mut m, "_pad", u8t, 0);
    let it = item(&mut m, ItemKind::Union, "U");
    let ty = m.types.intern(Ty::Union {
        item: it,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![p] },
        },
    });
    let emission = m.strings.intern("U");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Union { layout: ty },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint8_t _pad;"),
        "real _pad member present:\n{tu}"
    );
    assert!(
        tu.contains("unsigned char _pad0["),
        "filler skips the taken _pad:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// Fix D: a fn-pointer return that is itself a fn pointer / pointer-to-array
// recurses the declarator instead of concatenating a bare return type.
// ---------------------------------------------------------------------------

/// Intern a no-argument `fn() -> output` function-pointer `Ty`.
fn fn_ptr_returning(m: &mut RegseqModel, output: TyId) -> TyId {
    let sig = m.sigs.intern(FnSig {
        inputs: vec![],
        output,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    m.types.intern(Ty::FnPtr(sig))
}

#[test]
fn fn_ptr_returning_fn_ptr_recurses_the_declarator() {
    // `cb: fn() -> fn() -> i32` -> `int32_t (*(*cb)(void))(void)`.
    let mut m = model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let inner = fn_ptr_returning(&mut m, i32t);
    let outer = fn_ptr_returning(&mut m, inner);
    let f = fl(&mut m, "cb", outer, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(8, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("int32_t (*(*cb)(void))(void);"),
        "nested fn-ptr return must recurse:\n{tu}"
    );
}

#[test]
fn fn_ptr_returning_pointer_to_array_recurses_the_declarator() {
    // `cb: fn() -> &[u32; 4]` -> `const uint32_t (*(*cb)(void))[4]`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let aref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let fnptr = fn_ptr_returning(&mut m, aref);
    let f = fl(&mut m, "cb", fnptr, 0);
    push_struct_instance(&mut m, "Holder", struct_layout(8, 8, None, vec![f]));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint32_t (*(*cb)(void))[4];"),
        "pointer-to-array return must recurse:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// Task 09 (07 follow-up #3): a Ref / Ptr to a fn-pointer pointee places
// `const` on the fn-pointer VALUE (`int32_t (*const (*p))(void)`), not on the
// function's return type.
// ---------------------------------------------------------------------------

#[test]
fn shared_ref_to_fn_ptr_places_const_on_the_fn_ptr_value() {
    // `&fn() -> i32` -> the fn-pointer value is const: the qualifier sits
    // adjacent to the fn-pointer's own `*` (`int32_t (*const (*p))(void)`),
    // never as a leading `const` on the return type.
    let mut m = model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let fnptr = fn_ptr_returning(&mut m, i32t);
    let r = m.types.intern(Ty::Ref(fnptr, Provenance::SharedBorrow));
    let tu = emit_pointer_field(&mut m, r);
    assert!(
        tu.contains("int32_t (*const (*p))(void);"),
        "const on the fn-ptr value:\n{tu}"
    );
    assert!(
        !tu.contains("const int32_t (*"),
        "const must not land on the return type:\n{tu}"
    );
}

#[test]
fn const_raw_ptr_to_fn_ptr_places_const_on_the_fn_ptr_value() {
    // `*const fn() -> i32` -> same const-on-value placement as a shared ref.
    let mut m = model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let fnptr = fn_ptr_returning(&mut m, i32t);
    let p = m.types.intern(Ty::Ptr(fnptr, Mutability::Not));
    let tu = emit_pointer_field(&mut m, p);
    assert!(
        tu.contains("int32_t (*const (*p))(void);"),
        "const on the fn-ptr value:\n{tu}"
    );
    assert!(!tu.contains("const int32_t (*"), "got:\n{tu}");
}

#[test]
fn mut_ref_to_fn_ptr_has_no_const_on_the_fn_ptr_value() {
    // `&mut fn() -> i32` -> no const anywhere: `int32_t (*(*p))(void)`.
    let mut m = model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let fnptr = fn_ptr_returning(&mut m, i32t);
    let r = m.types.intern(Ty::Ref(fnptr, Provenance::UniqueBorrow));
    let tu = emit_pointer_field(&mut m, r);
    assert!(
        tu.contains("int32_t (*(*p))(void);"),
        "no const for &mut:\n{tu}"
    );
    assert!(
        !tu.contains("*const (*p)"),
        "no const on the fn-ptr value:\n{tu}"
    );
    assert!(
        !tu.contains("const int32_t (*"),
        "no const on the return type:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// Task c2: the section 13 / D9 offset-faithful union-of-views enum body.
// Structural assertions over the emitted TU (compile-level layout-fidelity
// proof lives in the codegen-coverage-runner behavioral tests).
// ---------------------------------------------------------------------------

/// Build a `LayoutKind::Enum` layout (`pack` is always `None` -- enums are
/// never `repr(packed)`).
fn enum_layout(size: u64, align: u64, shape: EnumShape) -> Layout {
    Layout {
        size,
        align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum { shape },
    }
}

/// Intern a `Ty::Enum` AND register it as an `InstanceKind::Enum` root.
fn push_enum_instance(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = item(model, ItemKind::Enum, rendered);
    let ty = model.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout,
    });
    let emission = model.strings.intern(rendered);
    model.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Enum { layout: ty },
    });
    ty
}

/// A `VariantLayout` with a source-interned name.
fn variant(
    model: &mut RegseqModel,
    name: &str,
    fields: Vec<FieldLayout>,
    discriminant: u128,
    tag_value: Option<u128>,
) -> VariantLayout {
    VariantLayout {
        name: model.strings.intern(name),
        fields,
        uninhabited: false,
        discriminant,
        tag_value,
    }
}

#[test]
fn empty_enum_emits_a_size_faithful_empty_struct() {
    // enum Void {} -- uninhabited, size 0 align 1: zero views -> an empty
    // struct (a gnu11 extension; sizeof 0).
    let mut m = model();
    push_enum_instance(&mut m, "Void", enum_layout(0, 1, EnumShape::Empty));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct Void {\n}"), "empty body:\n{tu}");
    assert!(tu.contains("_Static_assert(sizeof(struct Void) == 0, \"Void size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Void) == 1, \"Void align\");"));
    assert!(!tu.contains("unsigned char _pad0[0]"), "no zero-length pad");
}

#[test]
fn single_scalar_none_fieldless_emits_size_faithful_struct() {
    // A single-variant fieldless enum with no tag scalar: zero views -> a
    // size-faithful filler struct (hand-built size 1 to exercise the pad).
    let mut m = model();
    let shape = EnumShape::Single {
        variant: variant(&mut m, "Idle", Vec::new(), 0, None),
        scalar: None,
    };
    push_enum_instance(&mut m, "E", enum_layout(1, 1, shape));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct E {"));
    assert!(
        tu.contains("unsigned char _pad0[1];"),
        "size-faithful pad:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(sizeof(struct E) == 1, \"E size\");"));
    assert!(!tu.contains("union"), "zero-view enum has no union:\n{tu}");
    assert!(!tu.contains(" tag;"), "scalar:None has no tag view");
}

#[test]
fn fieldless_direct_enum_collapses_to_tag_only_struct() {
    // Section 9.8: a `repr(u32)` fieldless `Direct` enum -> a single tag
    // view -> collapsed `struct Status { uint32_t tag; }`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u32t,
        valid_range: WrappingRange {
            start: 0,
            end: 0xDEAD,
        },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "Idle", Vec::new(), 0, Some(0)),
            variant(&mut m, "Active", Vec::new(), 1, Some(1)),
        ],
    };
    push_enum_instance(&mut m, "Status", enum_layout(4, 4, shape));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("uint32_t tag;"), "collapsed tag field:\n{tu}");
    assert!(
        !tu.contains("union"),
        "single view collapses, no union:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Status, tag) == 0, \"Status.tag offset\");")
    );
    assert!(tu.contains("_Static_assert(sizeof(struct Status) == 4, \"Status size\");"));
    assert!(tu.contains("__attribute__((aligned(4)))"));
}

#[test]
fn niche_enum_collapses_to_single_field_struct() {
    // Section 9.9: `Option<&u32>` is `Niche` (no tag view) with `None`
    // fieldless + `Some` carrying the pointer at offset 0 -> a single view
    // -> collapsed `struct Opt { const uint32_t *_0; }`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let some_field = fl(&mut m, "0", ref_ty, 0);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_ty,
        valid_range: WrappingRange { start: 1, end: 0 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(1),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(0),
                end: VariantIdx::from_u32(0),
            },
            niche_start: 0,
        },
        variants: vec![
            variant(&mut m, "None", Vec::new(), 0, Some(0)),
            variant(&mut m, "Some", vec![some_field], 1, None),
        ],
    };
    push_enum_instance(&mut m, "Opt", enum_layout(8, 8, shape));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint32_t *_0;"),
        "collapsed single pointer field:\n{tu}"
    );
    assert!(!tu.contains("union"), "niche single view collapses:\n{tu}");
    assert!(!tu.contains(" tag;"), "niche enum has no tag view:\n{tu}");
    assert!(tu.contains("_Static_assert(offsetof(struct Opt, _0) == 0, \"Opt._0 offset\");"));
    assert!(tu.contains("_Static_assert(sizeof(struct Opt) == 8, \"Opt size\");"));
}

#[test]
fn multi_view_direct_enum_emits_union_of_views() {
    // The shifted-alignment case the literal `union_start` form cannot
    // place: enum Shift { A(u8), B(u32) } -- tag u8 @0, A.0 @1, B.0 @4,
    // size 8 align 4. Three views: `_tag`, `_v0` (A), `_v1` (B).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let a0 = fl(&mut m, "0", u8t, 1);
    let b0 = fl(&mut m, "0", u32t, 4);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "A", vec![a0], 0, Some(0)),
            variant(&mut m, "B", vec![b0], 1, Some(1)),
        ],
    };
    push_enum_instance(&mut m, "Shift", enum_layout(8, 4, shape));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("    union {\n"),
        "multi-view union wrapper:\n{tu}"
    );
    assert!(tu.contains("    } _tag;\n"), "tag view member:\n{tu}");
    assert!(tu.contains("    } _v0;\n"), "variant A view member:\n{tu}");
    assert!(tu.contains("    } _v1;\n"), "variant B view member:\n{tu}");
    assert!(tu.contains("    } u;\n"), "union member:\n{tu}");
    // Nested member-designator guards at the absolute offsets.
    assert!(
        tu.contains("_Static_assert(offsetof(struct Shift, u._tag.tag) == 0,"),
        "tag @0:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Shift, u._v0._0) == 1,"),
        "A.0 @1:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Shift, u._v1._0) == 4,"),
        "B.0 @4:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(sizeof(struct Shift) == 8, \"Shift size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Shift) == 4, \"Shift align\");"));
    assert!(tu.contains("__attribute__((aligned(4)))"));
}

#[test]
fn enum_emit_is_byte_identical_across_two_calls() {
    let build = || {
        let mut m = model();
        let u8t = uint(&mut m, UintKind::U8);
        let u32t = uint(&mut m, UintKind::U32);
        let a0 = fl(&mut m, "0", u8t, 1);
        let b0 = fl(&mut m, "0", u32t, 4);
        let tag = TagScalar {
            offset: 0,
            scalar_ty: u8t,
            valid_range: WrappingRange { start: 0, end: 1 },
        };
        let shape = EnumShape::Multiple {
            tag,
            encoding: TagEncoding::Direct,
            variants: vec![
                variant(&mut m, "A", vec![a0], 0, Some(0)),
                variant(&mut m, "B", vec![b0], 1, Some(1)),
            ],
        };
        push_enum_instance(&mut m, "Shift", enum_layout(8, 4, shape));
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}
