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
    let id = model.items.push(Item {
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
    });
    // Keep the `item_def_paths` side table parallel + globally unique so
    // `instance_ref_of` (used by `Const::Reloc` resolution, task d1) works on
    // these hand-built models. c1 / c2 tests do not read it.
    model.item_def_paths.push(regseq_model::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: id.as_u32() as u64,
    });
    id
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

// ---------------------------------------------------------------------------
// Task d1: banks 4-5 (prototypes / extern statics + bodies / static inits),
// the section 7 envelope + section 7.1 terminators + section 6/10/11/12/16
// value lowering, and the d1/d2 fail-loud boundary. Structural assertions
// over the emitted TU; the compile-and-run proof lives in the
// codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

use regseq_model::{
    AggregateKind, AssertKind, AttrData, BasicBlock, BinOp, BlockId, Body, CAbiLowering, CAbiShape,
    CallCallee, CastKind, Const, FieldIdx, FnSigId, FunctionInstanceData, FunctionInstanceFlavor,
    InstanceId, IntrinsicKind, LocalDecl, LocalId, LogLevel, NonDivergingIntrinsic, Operand, Place,
    PlaceElem, PointerCoercion, Rvalue, Statement, StatementKind, SubstReason, SwitchTargets,
    Terminator, TerminatorKind, UnOp, UNIT_LAYOUT,
};

fn fnsig(
    m: &mut RegseqModel,
    inputs: Vec<TyId>,
    output: TyId,
    abi: FnAbi,
    variadic: bool,
) -> FnSigId {
    m.sigs.intern(FnSig {
        inputs,
        output,
        abi,
        c_variadic: variadic,
        safety: Safety::Safe,
    })
}

fn ld(ty: TyId) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    }
}

fn st(kind: StatementKind) -> Statement {
    Statement { kind, span: span() }
}

fn tk(kind: TerminatorKind) -> Terminator {
    Terminator { kind, span: span() }
}

fn bb(stmts: Vec<Statement>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        statements: stmts.into_boxed_slice(),
        terminator,
    }
}

fn build_body(arg_count: usize, local_tys: Vec<TyId>, blocks: Vec<BasicBlock>) -> Body {
    let mut body = Body {
        arg_count,
        ..Default::default()
    };
    for ty in local_tys {
        body.push_local(ld(ty));
    }
    for block in blocks {
        body.basic_blocks.push(block);
    }
    body
}

fn loc(n: u32) -> Place {
    Place::from(LocalId::from_u32(n))
}

fn fn_instance(
    m: &mut RegseqModel,
    rendered: &str,
    sig: FnSigId,
    body: Option<Body>,
) -> InstanceId {
    let it = item(m, ItemKind::Function, rendered);
    let emission = m.strings.intern(rendered);
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    })
}

/// Push a `Function` monomorphization for an *existing* `item` with explicit
/// `generic_args` and `emission_name`, returning its `InstanceId`. Models the
/// bridge minting one `emission_name` (the per-item, generic-agnostic
/// `DefPath.rendered`) for every monomorphization of a generic item -- the
/// shape the task-11 instance-symbol fix disambiguates.
fn push_fn_mono(
    m: &mut RegseqModel,
    fn_item: regseq_model::ItemId,
    rendered: &str,
    generic_args: GenericArgs,
    sig: FnSigId,
    body: Option<Body>,
) -> InstanceId {
    let emission = m.strings.intern(rendered);
    m.instances.push(Instance {
        item: fn_item,
        generic_args,
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    })
}

fn never(m: &mut RegseqModel) -> TyId {
    m.types.intern(Ty::Never)
}

/// `add(a: u32, b: u32) -> u32 { a + b }` -- the canonical d1 scalar body.
#[test]
fn scalar_function_emits_prototype_and_body() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t, u32t], u32t, FnAbi::Rust, false);
    let body = build_body(
        2,
        vec![u32t, u32t, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::BinaryOp(BinOp::Add, Operand::Copy(loc(1)), Operand::Copy(loc(2))),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "add", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t add(uint32_t, uint32_t);"),
        "proto:\n{tu}"
    );
    assert!(
        tu.contains("uint32_t add(uint32_t _1, uint32_t _2) {"),
        "def:\n{tu}"
    );
    assert!(tu.contains("uint32_t _0;"), "return-place decl:\n{tu}");
    assert!(tu.contains("_0 = ((_1) + (_2));"), "add stmt:\n{tu}");
    assert!(tu.contains("return _0;"), "return:\n{tu}");
}

#[test]
fn static_emits_extern_decl_and_initializer() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let init = m.consts.intern(Const::Uint(UintKind::U32, 42));
    let it = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "FOO",
    );
    let emission = m.strings.intern("FOO");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: u32t,
            init: Some(init),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("extern const uint32_t FOO;"),
        "extern decl:\n{tu}"
    );
    assert!(
        tu.contains("const uint32_t FOO = ((uint32_t)(42U));"),
        "static def:\n{tu}"
    );
}

#[test]
fn mut_static_is_not_const() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let init = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let it = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Mut,
        },
        "MUT",
    );
    let emission = m.strings.intern("MUT");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: u32t,
            init: Some(init),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("extern uint32_t MUT;"), "extern decl:\n{tu}");
    assert!(
        !tu.contains("const uint32_t MUT"),
        "mut static is not const:\n{tu}"
    );
}

#[test]
fn foreign_function_emits_extern_decl_only() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let id = fn_instance(&mut m, "ext_fn", sig, None);
    let it = m.instances[id].item;
    m.items[it].linkage = Linkage::Foreign;

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("extern uint32_t ext_fn(uint32_t);"),
        "extern decl:\n{tu}"
    );
    // No definition: the only `ext_fn(` occurrence is the prototype.
    assert_eq!(
        tu.matches("ext_fn(").count(),
        1,
        "no body for foreign fn:\n{tu}"
    );
}

#[test]
fn weak_link_section_and_inline_qualifiers() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(
                    m.consts.intern(Const::Uint(UintKind::U32, 0)),
                )),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    let id = fn_instance(&mut m, "wfn", sig, Some(body));
    let it = m.instances[id].item;
    m.items[it].linkage = Linkage::Weak;
    m.items[it].inline_hint = regseq_model::InlineHint::Hint;
    let section = m.strings.intern(".init");
    m.items[it].link_section = Some(section);

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("__attribute__((weak))"), "weak:\n{tu}");
    assert!(
        tu.contains("__attribute__((section(\".init\")))"),
        "section:\n{tu}"
    );
    assert!(tu.contains("inline "), "inline:\n{tu}");
}

#[test]
fn never_output_function_is_noreturn() {
    let mut m = model();
    let never_ty = never(&mut m);
    let sig = fnsig(&mut m, vec![], never_ty, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![never_ty],
        vec![bb(vec![], tk(TerminatorKind::Unreachable))],
    );
    fn_instance(&mut m, "dies", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("_Noreturn void dies(void)"), "_Noreturn:\n{tu}");
    assert!(
        tu.contains("regseq_unreachable();"),
        "unreachable trap:\n{tu}"
    );
}

#[test]
fn diag_log_instance_is_excluded_from_banks_4_and_5() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(
                    m.consts.intern(Const::Uint(UintKind::U32, 0)),
                )),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    let id = fn_instance(&mut m, "zz_logfn", sig, Some(body));
    let it = m.instances[id].item;
    while m.domain_attrs.item_attrs.len() <= it.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[it].push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        !tu.contains("zz_logfn"),
        "log-marked instance excluded:\n{tu}"
    );
}

#[test]
fn c_abi_substituted_parameter_uses_emit_as_type() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let u64t = uint(&mut m, UintKind::U64);
    // f(x: <u64 raw>) -> u32, with the input C-ABI-substituted to u32.
    let sig = fnsig(&mut m, vec![u64t], u32t, FnAbi::C, false);
    // Populate the parallel C-ABI side table for this signature.
    while m.sig_c_abi_lowerings.len() <= sig.as_u32() as usize {
        m.sig_c_abi_lowerings.push(None);
    }
    m.sig_c_abi_lowerings[sig] = Some(CAbiLowering {
        inputs: vec![CAbiShape::Substituted {
            emit_as: u32t,
            reason: SubstReason::TagScalar,
        }],
        output: CAbiShape::Direct,
    });
    fn_instance(&mut m, "cf", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t cf(uint32_t);"),
        "substituted param:\n{tu}"
    );
    assert!(
        !tu.contains("cf(uint64_t)"),
        "raw type must not be emitted:\n{tu}"
    );
}

#[test]
fn c_variadic_appends_ellipsis() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::C, true);
    fn_instance(&mut m, "vf", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t vf(uint32_t, ...);"),
        "variadic:\n{tu}"
    );
}

#[test]
fn switch_int_emits_c_switch() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let body = build_body(
        1,
        vec![u32t, u32t],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(1)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(1)), (1, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
        ],
    );
    fn_instance(&mut m, "sw", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("switch (_1) {"), "switch:\n{tu}");
    assert!(tu.contains("case 0ULL: goto bb_1;"), "case 0:\n{tu}");
    assert!(tu.contains("case 1ULL: goto bb_2;"), "case 1:\n{tu}");
    assert!(tu.contains("default: goto bb_3;"), "default:\n{tu}");
}

#[test]
fn divergent_call_emits_unreachable() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let never_ty = never(&mut m);
    let dsig = fnsig(&mut m, vec![], never_ty, FnAbi::Rust, false);
    let diverge = fn_instance(&mut m, "diverge", dsig, None);

    let csig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![u32t],
        vec![bb(
            vec![],
            tk(TerminatorKind::Call {
                callee: CallCallee::Direct(diverge),
                args: Box::new([]),
                destination: loc(0),
                target: None,
            }),
        )],
    );
    fn_instance(&mut m, "caller", csig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("diverge(); __builtin_unreachable();"),
        "divergent call:\n{tu}"
    );
}

#[test]
fn indirect_call_through_fn_pointer() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let cb_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let cb_ty = m.types.intern(Ty::FnPtr(cb_sig));
    let sig = fnsig(&mut m, vec![cb_ty, u32t], u32t, FnAbi::Rust, false);
    let body = build_body(
        2,
        vec![u32t, cb_ty, u32t],
        vec![bb(
            vec![],
            tk(TerminatorKind::Call {
                callee: CallCallee::Indirect(Operand::Copy(loc(1))),
                args: Box::new([Operand::Copy(loc(2))]),
                destination: loc(0),
                target: Some(BlockId::from_u32(1)),
            }),
        )
        .clone()],
    );
    // Append a trivial return block so bb_1 exists.
    let mut body = body;
    body.basic_blocks
        .push(bb(vec![], tk(TerminatorKind::Return)));
    fn_instance(&mut m, "call_indirect", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = (_1)(_2); goto bb_1;"),
        "indirect call:\n{tu}"
    );
}

#[test]
fn assume_intrinsic_uses_portable_form() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let boolt = m.types.intern(Ty::Bool);
    let sig = fnsig(&mut m, vec![boolt], u32t, FnAbi::Rust, false);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let body = build_body(
        1,
        vec![u32t, boolt],
        vec![bb(
            vec![
                st(StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(
                    Operand::Copy(loc(1)),
                ))),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "with_assume", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("if (!(_1)) __builtin_unreachable();"),
        "assume:\n{tu}"
    );
}

#[test]
fn copy_nonoverlapping_reads_named_fields_in_c_order() {
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let ptr = m.types.intern(Ty::Ptr(u8t, Mutability::Mut));
    let usize_t = uint(&mut m, UintKind::Usize);
    let sig = fnsig(&mut m, vec![ptr, ptr, usize_t], unit, FnAbi::Rust, false);
    let body = build_body(
        3,
        vec![unit, ptr, ptr, usize_t],
        vec![bb(
            vec![st(StatementKind::Intrinsic(
                NonDivergingIntrinsic::CopyNonOverlapping(regseq_model::CopyNonOverlapping {
                    src: Operand::Copy(loc(1)),
                    dst: Operand::Copy(loc(2)),
                    count: Operand::Copy(loc(3)),
                }),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "do_copy", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    // C order is (dst, src, count): dst is _2, src is _1.
    assert!(
        tu.contains("memcpy(_2, _1, (_3) * sizeof(*(_2)));"),
        "memcpy named-field order:\n{tu}"
    );
}

#[test]
fn assert_routes_fixed_and_operand_messages_to_regseq_panic() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let boolt = m.types.intern(Ty::Bool);
    let usize_t = uint(&mut m, UintKind::Usize);
    let sig = fnsig(
        &mut m,
        vec![boolt, usize_t, usize_t],
        u32t,
        FnAbi::Rust,
        false,
    );
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let body = build_body(
        3,
        vec![u32t, boolt, usize_t, usize_t],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Assert {
                    cond: Operand::Copy(loc(1)),
                    expected: true,
                    msg: AssertKind::DivisionByZero(Operand::Copy(loc(2))),
                    target: BlockId::from_u32(1),
                }),
            ),
            bb(
                vec![],
                tk(TerminatorKind::Assert {
                    cond: Operand::Copy(loc(1)),
                    expected: true,
                    msg: AssertKind::BoundsCheck {
                        len: Operand::Copy(loc(2)),
                        index: Operand::Copy(loc(3)),
                    },
                    target: BlockId::from_u32(2),
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
        ],
    );
    fn_instance(&mut m, "asserts", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(
            "if ((_1) == 1) goto bb_1; else { regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO); }"
        ),
        "fixed-message assert:\n{tu}"
    );
    assert!(
        tu.contains("REGSEQ_MSG_BOUNDS_CHECK"),
        "operand-bearing template:\n{tu}"
    );
    assert!(
        tu.contains("regseq_panic(_m);"),
        "operand-bearing panic:\n{tu}"
    );
}

#[test]
fn invalid_enum_construction_is_operand_bearing() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let boolt = m.types.intern(Ty::Bool);
    let sig = fnsig(&mut m, vec![boolt, u32t], u32t, FnAbi::Rust, false);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let body = build_body(
        2,
        vec![u32t, boolt, u32t],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Assert {
                    cond: Operand::Copy(loc(1)),
                    expected: true,
                    msg: AssertKind::InvalidEnumConstruction(Operand::Copy(loc(2))),
                    target: BlockId::from_u32(1),
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
        ],
    );
    fn_instance(&mut m, "invenum", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("REGSEQ_MSG_INVALID_ENUM_CONSTRUCTION") && tu.contains("regseq_panic(_m);"),
        "InvalidEnumConstruction interpolates its operand:\n{tu}"
    );
}

#[test]
fn reloc_to_function_is_address_of_emission_name() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let target_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let target = fn_instance(&mut m, "target_fn", target_sig, None);
    let fp_ty = m.types.intern(Ty::FnPtr(target_sig));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(target)));
    let it = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "FP",
    );
    let emission = m.strings.intern("FP");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: fp_ty,
            init: Some(reloc),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("(&target_fn)"), "reloc to function:\n{tu}");
}

#[test]
fn reloc_to_static_is_cast_address() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let a_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "A",
    );
    let a_em = m.strings.intern("A");
    let a_id = m.instances.push(Instance {
        item: a_item,
        generic_args: GenericArgs::new(),
        emission_name: a_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: u32t,
            init: Some(one),
        },
    });
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(a_id)));
    let pa_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "PA",
    );
    let pa_em = m.strings.intern("PA");
    m.instances.push(Instance {
        item: pa_item,
        generic_args: GenericArgs::new(),
        emission_name: pa_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: ref_ty,
            init: Some(reloc),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("&A"), "reloc to static address:\n{tu}");
    assert!(
        tu.contains("(const uint32_t *)&A"),
        "reloc cast to consumer ptr:\n{tu}"
    );
}

#[test]
fn body_bearing_model_is_byte_identical_across_two_calls() {
    let build = || {
        let mut m = model();
        let u32t = uint(&mut m, UintKind::U32);
        let sig = fnsig(&mut m, vec![u32t, u32t], u32t, FnAbi::Rust, false);
        let body = build_body(
            2,
            vec![u32t, u32t, u32t],
            vec![bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::BinaryOp(BinOp::Mul, Operand::Copy(loc(1)), Operand::Copy(loc(2))),
                ))],
                tk(TerminatorKind::Return),
            )],
        );
        fn_instance(&mut m, "mul", sig, Some(body));
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

// --- the d1/d2 fail-loud boundary: one negative test per construct family ---

/// A one-block function returning `u32` whose single statement is `stmt`.
fn fn_with_stmt(m: &mut RegseqModel, rendered: &str, stmt: Statement) -> InstanceId {
    let u32t = uint(m, UintKind::U32);
    let sig = fnsig(m, vec![], u32t, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![u32t],
        vec![bb(vec![stmt], tk(TerminatorKind::Return))],
    );
    fn_instance(m, rendered, sig, Some(body))
}

#[test]
fn struct_aggregate_construct_and_field_access_spell_definition_names() {
    // struct Point { a: u8 @4, b: u32 @0 } (repr(Rust) reordered). A fn
    // builds `Point { a, b }` from two args and reads `.b`: construction is a
    // designated initializer with the sanitize(name) fields (MIR order a, b)
    // and access spells `.b` (by name, not a positional index).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let a = fl(&mut m, "a", u8t, 4);
    let b = fl(&mut m, "b", u32t, 0);
    let point = intern_struct(&mut m, "Point", struct_layout(8, 4, None, vec![a, b]));
    let sig = fnsig(&mut m, vec![u8t, u32t], u32t, FnAbi::Rust, false);
    // locals: _0 u32 (ret), _1 u8 (arg a), _2 u32 (arg b), _3 Point.
    let body = build_body(
        2,
        vec![u32t, u8t, u32t, point],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Aggregate(
                        AggregateKind::Struct(point),
                        Box::new([Operand::Copy(loc(1)), Operand::Copy(loc(2))]),
                    ),
                )),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(3),
                        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), u32t)]),
                    })),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "build_point", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct Point){ .a = _1, .b = _2 }"),
        "designated initializer in MIR field order:\n{tu}"
    );
    assert!(
        tu.contains("_0 = _3.b;"),
        "field access spells the definition name, not a positional index:\n{tu}"
    );
}

#[test]
fn tuple_aggregate_construct_and_field_access_use_indexed_names() {
    // (u8, u32): _0 (u8) @4, _1 (u32) @0. A fn builds the tuple and reads ._1.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let t0 = fl(&mut m, "_0", u8t, 4);
    let t1 = fl(&mut m, "_1", u32t, 0);
    let tuple = m.types.intern(Ty::Tuple {
        fields: vec![u8t, u32t],
        layout: struct_layout(8, 4, None, vec![t0, t1]),
    });
    let sig = fnsig(&mut m, vec![u8t, u32t], u32t, FnAbi::Rust, false);
    // locals: _0 u32 (ret), _1 u8 (arg), _2 u32 (arg), _3 tuple.
    let body = build_body(
        2,
        vec![u32t, u8t, u32t, tuple],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Aggregate(
                        AggregateKind::Tuple(tuple),
                        Box::new([Operand::Copy(loc(1)), Operand::Copy(loc(2))]),
                    ),
                )),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(3),
                        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), u32t)]),
                    })),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "build_tuple", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("._0 = _1, ._1 = _2 }"),
        "tuple compound literal uses _<idx> names:\n{tu}"
    );
    assert!(tu.contains("_0 = _3._1;"), "tuple field access:\n{tu}");
}

#[test]
fn union_aggregate_constructs_the_active_field() {
    // union U { x: u32, y: u8 } -- build it with the `x` field active.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let u8t = uint(&mut m, UintKind::U8);
    let x = fl(&mut m, "x", u32t, 0);
    let y = fl(&mut m, "y", u8t, 0);
    let it = item(&mut m, ItemKind::Union, "U");
    let union_ty = m.types.intern(Ty::Union {
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
    let sig = fnsig(&mut m, vec![u32t], union_ty, FnAbi::Rust, false);
    // locals: _0 U (ret), _1 u32 (arg).
    let body = build_body(
        1,
        vec![union_ty, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::Union(union_ty, FieldIdx::from_u32(0)),
                    Box::new([Operand::Copy(loc(1))]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "build_union", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = (union U){ .x = _1 };"),
        "union literal sets the active field:\n{tu}"
    );
}

#[test]
fn set_discriminant_is_unsupported() {
    let mut m = model();
    fn_with_stmt(
        &mut m,
        "setdisc",
        st(StatementKind::SetDiscriminant(
            loc(0),
            VariantIdx::from_u32(0),
        )),
    );
    assert!(matches!(emit_generic_c(&m), Err(EmitError::Unsupported(_))));
}

#[test]
fn const_aggregate_is_unsupported() {
    let mut m = model();
    let agg = m.consts.intern(Const::Aggregate(vec![]));
    fn_with_stmt(
        &mut m,
        "constagg",
        st(StatementKind::Assign(
            loc(0),
            Rvalue::Use(Operand::Constant(agg)),
        )),
    );
    assert!(matches!(emit_generic_c(&m), Err(EmitError::Unsupported(_))));
}

#[test]
fn reborrow_of_a_whole_slice_is_an_identity_copy() {
    // `fn reborrow(s: &[u8]) -> &[u8] { &*s }` -- `_0 = &(*_1)` re-borrows the
    // whole slice, which is the fat pointer copied (`_0 = _1;`), not a
    // pointer-to-pointer.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let slice = m.types.intern(Ty::Slice(u8t));
    let slice_ref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![slice_ref], slice_ref, FnAbi::Rust, false);
    // _0: &[u8] (ret), _1: &[u8] (arg).
    let deref = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Deref]),
    };
    let body = build_body(
        1,
        vec![slice_ref, slice_ref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Ref(Provenance::SharedBorrow, deref),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "reborrow", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = _1;"),
        "a whole-slice re-borrow copies the fat pointer:\n{tu}"
    );
}

#[test]
fn bytes_static_initializer_is_unsupported() {
    // Boundary 6: an address-taken `const` surfaces as a ConstMemoryStatic
    // whose own `Bytes` initializer fails loud at the bank-5 static def.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let arr = m.types.intern(Ty::Array(u8t, 1));
    let bytes = m
        .consts
        .intern(Const::Bytes(vec![regseq_model::Byte::Value(b'x')]));
    let it = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "BUF",
    );
    let emission = m.strings.intern("BUF");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: arr,
            init: Some(bytes),
        },
    });
    assert!(matches!(emit_generic_c(&m), Err(EmitError::Unsupported(_))));
}

// --- Codex Task Review fixes: NullPtr, FloatToInt(const), *WithOverflow, wide switch ---

#[test]
fn null_ptr_static_emits_typed_null_pointer() {
    // NullPtr(pointee) consumed by a `*const u32` static -> `(const uint32_t *)0`
    // (the consumer pointer type), not a scalar cast of the pointee.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ptr_ty = m.types.intern(Ty::Ptr(u32t, Mutability::Not));
    let init = m.consts.intern(Const::NullPtr(u32t)); // payload is the pointee
    let it = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "NP",
    );
    let emission = m.strings.intern("NP");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: ptr_ty,
            init: Some(init),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("NP = ((const uint32_t *)0);"),
        "typed null pointer:\n{tu}"
    );
}

#[test]
fn float_to_int_from_constant_uses_saturating_helper() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let f64t = m.types.intern(Ty::Float(regseq_model::FloatKind::F64));
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let c = m.consts.intern(Const::Float(
        regseq_model::FloatKind::F64,
        2.5f64.to_bits() as u128,
    ));
    let body = build_body(
        0,
        vec![u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(CastKind::FloatToInt, Operand::Constant(c), u32t),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    let _ = f64t;
    fn_instance(&mut m, "f2i", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("regseq_f64_to_uint32_t("),
        "FloatToInt(const) uses the saturating helper:\n{tu}"
    );
}

#[test]
fn with_overflow_writes_both_tuple_fields_via_the_helper() {
    // checked_add-style: `_0: (u32, bool) = AddWithOverflow(_1, _2)`. The
    // prelude helper is called once into a temp, then both `(T, bool)` fields
    // are written (`_0` = the value, `_1` = the overflow flag).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let boolt = m.types.intern(Ty::Bool);
    // tuple (u32, bool): _0 (u32) @0, _1 (bool) @4, size 8 align 4.
    let f0 = fl(&mut m, "_0", u32t, 0);
    let f1 = fl(&mut m, "_1", boolt, 4);
    let tup = m.types.intern(Ty::Tuple {
        fields: vec![u32t, boolt],
        layout: struct_layout(8, 4, None, vec![f0, f1]),
    });
    let sig = fnsig(&mut m, vec![u32t, u32t], tup, FnAbi::Rust, false);
    // _0 tuple (ret), _1 u32 (arg), _2 u32 (arg).
    let body = build_body(
        2,
        vec![tup, u32t, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::BinaryOp(
                    BinOp::AddWithOverflow,
                    Operand::Copy(loc(1)),
                    Operand::Copy(loc(2)),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "checked_add", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("struct regseq_overflow_uint32_t _ov = regseq_add_uint32_t((_1), (_2));"),
        "helper called once into a temp:\n{tu}"
    );
    assert!(
        tu.contains("_0._0 = _ov.v;") && tu.contains("_0._1 = _ov.overflow;"),
        "both (T, bool) fields written:\n{tu}"
    );
}

#[test]
fn wide_switch_int_compares_128bit_halves() {
    let mut m = model();
    let u128t = uint(&mut m, UintKind::U128);
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u128t], u32t, FnAbi::Rust, false);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    // A case value above u64::MAX exercises the halves construction.
    let big: u128 = (1u128 << 70) | 5;
    let body = build_body(
        1,
        vec![u32t, u128t],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(1)),
                    targets: SwitchTargets {
                        cases: Box::new([(big, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                ))],
                tk(TerminatorKind::Return),
            ),
        ],
    );
    fn_instance(&mut m, "wsw", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(__uint128_t)(_1)"),
        "128-bit discr compare:\n{tu}"
    );
    assert!(
        tu.contains("(((__uint128_t)") && tu.contains("ULL << 64)"),
        "128-bit case via halves:\n{tu}"
    );
}

#[test]
fn transmute_lowers_to_memcpy() {
    // f32 -> u32 bit reinterpret: `_0 = transmute(_1)` -> memcpy between
    // equal-sizeof objects (never a value cast, which would convert).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let f32t = m.types.intern(Ty::Float(regseq_model::FloatKind::F32));
    let sig = fnsig(&mut m, vec![f32t], u32t, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![u32t, f32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(CastKind::Transmute, Operand::Copy(loc(1)), u32t),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "bits", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("memcpy(&(_0), &(_1), sizeof(_0));"),
        "transmute -> memcpy:\n{tu}"
    );
}

#[test]
fn zst_destination_transmute_is_elided_not_misemitted() {
    // A transmute into a unit (ZST) destination writes 0 bytes: the ZST local
    // is never declared, so it must be elided rather than emitting a
    // `memcpy(&<undeclared>, ...)`.
    let mut m = model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let zst = m.consts.intern(Const::ZST);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    // _0 u32 (ret), _1 unit (inner): `_1 = transmute(ZST)` is elided.
    let body = build_body(
        0,
        vec![u32t, unit],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(1),
                    Rvalue::Cast(CastKind::Transmute, Operand::Constant(zst), unit),
                )),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Constant(zero)),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "zst_transmute", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        !tu.contains("memcpy"),
        "ZST transmute must be elided:\n{tu}"
    );
}

#[test]
fn call_argument_with_deferred_projection_is_unsupported() {
    // A deferred place projection (here a `Downcast`, task d2.1) reached
    // through a call argument must fail loud, validated before any ZST elision.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let callee_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let callee = fn_instance(&mut m, "callee", callee_sig, None);

    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let field_arg = Operand::Copy(Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Downcast(VariantIdx::from_u32(0))]),
    });
    let body = build_body(
        1,
        vec![u32t, u32t],
        vec![bb(
            vec![],
            tk(TerminatorKind::Call {
                callee: CallCallee::Direct(callee),
                args: Box::new([field_arg]),
                destination: loc(0),
                target: Some(BlockId::from_u32(1)),
            }),
        )],
    );
    let mut body = body;
    body.basic_blocks
        .push(bb(vec![], tk(TerminatorKind::Return)));
    fn_instance(&mut m, "calls_with_field_arg", sig, Some(body));

    assert!(matches!(emit_generic_c(&m), Err(EmitError::Unsupported(_))));
}

// ---------------------------------------------------------------------------
// Task 11: instance-symbol uniqueness + linkage-aware naming (d1 follow-up).
// A generic item's monomorphizations share one bridge-minted emission_name,
// so the backend must derive distinct, linkage-aware C symbols. Structural
// assertions over the emitted TU; the compile-and-run proof lives in the
// codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

/// The task-11 internal-symbol form for `rendered` at `args`:
/// `sanitize(rendered) + __<ArgsKey:x>` (mirrors `naming::instance_symbol_base`).
fn internal_symbol(m: &RegseqModel, rendered: &str, args: &GenericArgs) -> String {
    format!(
        "{}__{:x}",
        rendered,
        regseq_model::args_key_for_generic_args(m, args).0
    )
}

/// A one-block `id`-style body `{ _0 = _1; return _0; }` for an `(T) -> T`
/// monomorphization of element type `ty`.
fn identity_body(ty: TyId) -> Body {
    build_body(
        1,
        vec![ty, ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Copy(loc(1))),
            ))],
            tk(TerminatorKind::Return),
        )],
    )
}

#[test]
fn default_generic_monos_get_distinct_suffixed_symbols() {
    // fn id<T>(x: T) -> T used as id::<u32> and id::<i64>: two Function
    // instances sharing one item (one bridge-minted emission_name "id"),
    // distinct generic_args, Default linkage. The fix derives distinct
    // suffixed symbols, so the TU has no duplicate symbol (pre-fix both
    // emitted bare "id").
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let i64t = m.types.intern(Ty::Int(IntKind::I64));
    let id_item = item(&mut m, ItemKind::Function, "id");

    let args_u32 = vec![regseq_model::GenericArg::Type(u32t)];
    let args_i64 = vec![regseq_model::GenericArg::Type(i64t)];

    let sig_u32 = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    push_fn_mono(
        &mut m,
        id_item,
        "id",
        args_u32.clone(),
        sig_u32,
        Some(identity_body(u32t)),
    );
    let sig_i64 = fnsig(&mut m, vec![i64t], i64t, FnAbi::Rust, false);
    push_fn_mono(
        &mut m,
        id_item,
        "id",
        args_i64.clone(),
        sig_i64,
        Some(identity_body(i64t)),
    );

    let tu = emit_generic_c(&m).unwrap();
    let sym_u32 = internal_symbol(&m, "id", &args_u32);
    let sym_i64 = internal_symbol(&m, "id", &args_i64);
    assert_ne!(sym_u32, sym_i64, "distinct monos -> distinct symbols");
    assert!(
        tu.contains(&format!("uint32_t {sym_u32}(uint32_t);")),
        "u32 mono prototype:\n{tu}"
    );
    assert!(
        tu.contains(&format!("int64_t {sym_i64}(int64_t);")),
        "i64 mono prototype:\n{tu}"
    );
    assert!(
        !tu.contains(" id("),
        "the colliding unsuffixed symbol must be gone:\n{tu}"
    );
}

#[test]
fn external_instance_symbol_is_sanitized_with_no_generic_suffix() {
    // External linkage (Foreign) sanitises the bridge-minted DefPath -- the
    // bridge mints `emission_name = def_path.rendered` (e.g. `crate::ext::cb`),
    // NOT a bare ABI symbol, so a raw-verbatim emit would be invalid C -- but
    // adds NO generic suffix, so the ABI symbol is stable across monos. A
    // generic instance makes the absence of a `__<hex>` suffix observable.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let it = item(&mut m, ItemKind::Function, "crate::ext::cb");
    m.items[it].linkage = Linkage::Foreign;
    let args = vec![regseq_model::GenericArg::Type(u32t)];
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    push_fn_mono(&mut m, it, "crate::ext::cb", args, sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("extern uint32_t crate_ext_cb(uint32_t);"),
        "sanitized external decl, no generic suffix:\n{tu}"
    );
    assert!(
        !tu.contains("crate_ext_cb__"),
        "external symbol carries no generic suffix:\n{tu}"
    );
    assert!(
        !tu.contains("crate::ext::cb"),
        "the raw DefPath must not leak into C:\n{tu}"
    );
}

#[test]
fn default_nongeneric_instance_symbol_is_sanitize_with_no_suffix() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    fn_instance(&mut m, "plain_fn", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t plain_fn(void);"),
        "non-generic prototype:\n{tu}"
    );
    assert!(
        !tu.contains("plain_fn__"),
        "non-generic symbol has no generic suffix:\n{tu}"
    );
}

#[test]
fn call_and_reloc_to_generic_instance_match_definition_symbol() {
    // A generic Default instance g::<u32>; a caller that calls it and a static
    // FP = &g. The call site and the Reloc must both spell g's suffixed symbol
    // (body_emit sites :711 Call and :932 Reloc routed through the table).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let g_item = item(&mut m, ItemKind::Function, "g");
    let args = vec![regseq_model::GenericArg::Type(u32t)];
    let g_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let g_id = push_fn_mono(
        &mut m,
        g_item,
        "g",
        args.clone(),
        g_sig,
        Some(identity_body(u32t)),
    );

    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let csig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let cbody = build_body(
        0,
        vec![u32t],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(g_id),
                    args: Box::new([Operand::Constant(zero)]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    fn_instance(&mut m, "caller", csig, Some(cbody));

    let fp_ty = m.types.intern(Ty::FnPtr(g_sig));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(g_id)));
    let fp_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "FP",
    );
    let fp_em = m.strings.intern("FP");
    m.instances.push(Instance {
        item: fp_item,
        generic_args: GenericArgs::new(),
        emission_name: fp_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: fp_ty,
            init: Some(reloc),
        },
    });

    let tu = emit_generic_c(&m).unwrap();
    let sym = internal_symbol(&m, "g", &args);
    assert!(
        tu.contains(&format!("uint32_t {sym}(uint32_t _1) {{")),
        "definition spells the suffixed symbol:\n{tu}"
    );
    assert!(
        tu.contains(&format!("{sym}(((uint32_t)(0U)))")),
        "call site spells the same symbol:\n{tu}"
    );
    assert!(
        tu.contains(&format!("(&{sym})")),
        "Reloc spells the same symbol:\n{tu}"
    );
}

#[test]
fn reify_fn_pointer_to_generic_instance_matches_definition_symbol() {
    // ReifyFnPointer of a generic instance (the body_emit FnDef-scan site
    // :621) spells the same suffixed symbol as the definition.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let h_item = item(&mut m, ItemKind::Function, "h");
    let args = vec![regseq_model::GenericArg::Type(u32t)];
    let h_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    push_fn_mono(
        &mut m,
        h_item,
        "h",
        args.clone(),
        h_sig,
        Some(identity_body(u32t)),
    );

    let fndef_ty = m.types.intern(Ty::FnDef(h_item, args.clone()));
    let fnptr_ty = m.types.intern(Ty::FnPtr(h_sig));
    let csig = fnsig(&mut m, vec![], fnptr_ty, FnAbi::Rust, false);
    // _0: fnptr (return), _1: fndef (ZST inner local).
    let cbody = build_body(
        0,
        vec![fnptr_ty, fndef_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(
                    CastKind::PointerCoercion(regseq_model::PointerCoercion::ReifyFnPointer),
                    Operand::Copy(loc(1)),
                    fnptr_ty,
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "reifier", csig, Some(cbody));

    let tu = emit_generic_c(&m).unwrap();
    let sym = internal_symbol(&m, "h", &args);
    assert!(
        tu.contains(&format!("_0 = (&{sym});")),
        "ReifyFnPointer spells the suffixed symbol:\n{tu}"
    );
}

#[test]
fn internal_symbol_colliding_with_prelude_helper_is_clash_renamed() {
    // A Default instance whose emission_name is exactly a prelude helper
    // (`regseq_panic`) must be `_clash_1`-renamed, never shadow the helper.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_panic", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t regseq_panic_clash_1(void);"),
        "instance symbol clash-renamed off the prelude helper:\n{tu}"
    );
    assert!(
        tu.contains("_Noreturn static inline void regseq_panic(const char *msg)"),
        "the real prelude helper is untouched:\n{tu}"
    );
}

#[test]
fn two_internal_instances_with_same_base_get_clash_suffix() {
    // Two distinct Default function items rendering the same name -> the
    // second instance symbol is `_clash_1`-disambiguated (the cross-item
    // sanitize-collision net).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig1 = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    fn_instance(&mut m, "dup", sig1, None);
    let sig2 = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    fn_instance(&mut m, "dup", sig2, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t dup(void);"),
        "first keeps the base:\n{tu}"
    );
    assert!(
        tu.contains("uint32_t dup_clash_1(uint32_t);"),
        "second is clash-renamed:\n{tu}"
    );
}

#[test]
fn generic_instance_model_is_byte_identical_across_two_calls() {
    let build = || {
        let mut m = model();
        let u32t = uint(&mut m, UintKind::U32);
        let i64t = m.types.intern(Ty::Int(IntKind::I64));
        let id_item = item(&mut m, ItemKind::Function, "id");
        let sig_u32 = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
        push_fn_mono(
            &mut m,
            id_item,
            "id",
            vec![regseq_model::GenericArg::Type(u32t)],
            sig_u32,
            Some(identity_body(u32t)),
        );
        let sig_i64 = fnsig(&mut m, vec![i64t], i64t, FnAbi::Rust, false);
        push_fn_mono(
            &mut m,
            id_item,
            "id",
            vec![regseq_model::GenericArg::Type(i64t)],
            sig_i64,
            Some(identity_body(i64t)),
        );
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

#[test]
fn prelude_ordinary_idents_all_defined_in_prelude() {
    // The reserved-identifier mirror in `prelude_ordinary_idents` must stay in
    // sync with the actually-emitted prelude + helper block (Change B step 1):
    // every reserved name appears as a function definition in the prelude text.
    let prelude_text = format!("{}{}", prelude::PRELUDE, prelude::helper_block());
    for ident in prelude_ordinary_idents() {
        assert!(
            prelude_text.contains(&format!("{ident}(")),
            "reserved prelude ident `{ident}` is not emitted as a function"
        );
    }
}

// ---------------------------------------------------------------------------
// Task d2: structured-data lowering (B3 structural + B4 fail-loud negatives +
// B5 determinism). Compile-and-run proof lives in the codegen-coverage-runner
// behavioral tests; these pin the emitted shapes + the deferral seams.
// ---------------------------------------------------------------------------

/// A minimal 2-variant fieldless `Direct` enum `E` (registered as a root),
/// used by the d2.1 negative tests that need a `Ty::Enum` predecessor.
fn push_simple_enum(m: &mut RegseqModel) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(m, "A", Vec::new(), 0, Some(0)),
            variant(m, "B", Vec::new(), 1, Some(1)),
        ],
    };
    push_enum_instance(m, "E", enum_layout(1, 1, shape))
}

#[test]
fn unsize_of_array_ref_builds_a_fat_slice() {
    // `fn(p: &[u32; 4]) -> &[u32] { p }` -- the Unsize coercion builds the
    // section 3.2 fat slice with the `.data` cast to the element pointer type
    // and `.len` the source array length.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let arr_ref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let slice = m.types.intern(Ty::Slice(u32t));
    let slice_ref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![arr_ref], slice_ref, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![slice_ref, arr_ref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(
                    CastKind::PointerCoercion(PointerCoercion::Unsize),
                    Operand::Copy(loc(1)),
                    slice_ref,
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "as_slice", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct rust_slice_uint32_t){ .data = (const uint32_t *)(_1), .len = 4 }"),
        "Unsize builds the fat slice with the element-pointer cast + length:\n{tu}"
    );
}

#[test]
fn str_construction_via_unsize_and_ptr_metadata() {
    // Hand-built (a runtime &str without a string literal is awkward to
    // bridge -- a literal is a Const::Aggregate carrier, task d2.2): Unsize a
    // `&[u8; 3]` to `&str` then read its metadata. `.data` is cast to the
    // monomorphic rust_str element pointer; PtrMetadata reads `.len`.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let usize_t = uint(&mut m, UintKind::Usize);
    let arr = m.types.intern(Ty::Array(u8t, 3));
    let arr_ref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let str_ty = m.types.intern(Ty::Str);
    let str_ref = m.types.intern(Ty::Ref(str_ty, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![arr_ref], usize_t, FnAbi::Rust, false);
    // _0 usize (ret), _1 &[u8;3] (arg), _2 &str.
    let body = build_body(
        1,
        vec![usize_t, arr_ref, str_ref],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(2),
                    Rvalue::Cast(
                        CastKind::PointerCoercion(PointerCoercion::Unsize),
                        Operand::Copy(loc(1)),
                        str_ref,
                    ),
                )),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(loc(2))),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "as_str_len", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct rust_str){ .data = (const uint8_t *)(_1), .len = 3 }"),
        "str Unsize builds rust_str with the monomorphic data cast:\n{tu}"
    );
    assert!(
        tu.contains("_0 = (_2).len;"),
        "PtrMetadata of a &str reads the .len slot:\n{tu}"
    );
}

#[test]
fn fat_pointer_deref_index_and_from_end_read_the_data_and_len_slots() {
    // `fn(s: &[u32], i: usize) -> u32` reading `(*s)[i]` and `(*s)[len - 1]`:
    // a fat Deref yields `.data` (not `(*s)`), the runtime index uses it, and
    // a from_end ConstantIndex reads the `.len` slot.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let usize_t = uint(&mut m, UintKind::Usize);
    let slice = m.types.intern(Ty::Slice(u32t));
    let slice_ref = m.types.intern(Ty::Ref(slice, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![slice_ref, usize_t], u32t, FnAbi::Rust, false);
    // _0 u32 (ret), _1 &[u32] (arg), _2 usize (arg), _3 u32.
    let deref_index = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Deref, PlaceElem::Index(LocalId::from_u32(2))]),
    };
    let deref_from_end = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Deref,
            PlaceElem::ConstantIndex {
                offset: 1,
                min_length: 1,
                from_end: true,
            },
        ]),
    };
    let body = build_body(
        2,
        vec![u32t, slice_ref, usize_t, u32t],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Use(Operand::Copy(deref_index)),
                )),
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(deref_from_end)),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "slice_read", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = _1.data[_2];"),
        "fat Deref + Index reads the .data slot, not (*s):\n{tu}"
    );
    assert!(
        tu.contains("_3 = _1.data[_1.len - 1];"),
        "from_end ConstantIndex reads the .len slot:\n{tu}"
    );
}

#[test]
fn array_subslice_under_ref_is_a_thin_sub_array_pointer() {
    // `fn(a: &[u32; 6]) -> &[u32; 3] { &(*a)[1..4] }` -- a sized-array Subslice
    // under a Ref is a thin pointer to the sub-array, offset by `from`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr6 = m.types.intern(Ty::Array(u32t, 6));
    let arr6_ref = m.types.intern(Ty::Ref(arr6, Provenance::SharedBorrow));
    let arr3 = m.types.intern(Ty::Array(u32t, 3));
    let arr3_ref = m.types.intern(Ty::Ref(arr3, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![arr6_ref], arr3_ref, FnAbi::Rust, false);
    // _0 &[u32;3] (ret), _1 &[u32;6] (arg). place = (*_1)[1..4].
    let subslice = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Deref,
            PlaceElem::Subslice {
                from: 1,
                to: 4,
                from_end: false,
            },
        ]),
    };
    let body = build_body(
        1,
        vec![arr3_ref, arr6_ref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Ref(Provenance::SharedBorrow, subslice),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "sub", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = (const uint32_t (*)[3])&((*_1)[1]);"),
        "array Subslice under a Ref is a thin sub-array pointer:\n{tu}"
    );
}

#[test]
fn nested_array_subslice_under_ref_accumulates_offsets() {
    // `fn() -> &[u32; 2] { &_1[1..5][1..3] }` -- the final address is `_1[2]`,
    // not `_1[1]`: both array Subslice `from` offsets contribute.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr8 = m.types.intern(Ty::Array(u32t, 8));
    let arr2 = m.types.intern(Ty::Array(u32t, 2));
    let arr2_ref = m.types.intern(Ty::Ref(arr2, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![], arr2_ref, FnAbi::Rust, false);
    // _0 &[u32;2] (ret), _1 [u32;8]. place = _1[1..5][1..3].
    let subslice = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Subslice {
                from: 1,
                to: 5,
                from_end: false,
            },
            PlaceElem::Subslice {
                from: 1,
                to: 3,
                from_end: false,
            },
        ]),
    };
    let body = build_body(
        0,
        vec![arr2_ref, arr8],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Ref(Provenance::SharedBorrow, subslice),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "nested_sub", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = (const uint32_t (*)[2])&(_1[2]);"),
        "nested array Subslice under a Ref accumulates offsets:\n{tu}"
    );
    assert!(
        !tu.contains("_0 = (const uint32_t (*)[2])&(_1[1]);"),
        "nested array Subslice must not use only the final offset:\n{tu}"
    );
}

#[test]
fn array_aggregate_and_repeat_lower_at_the_assign_site() {
    // `[a, b, c]` is per-element stores; `[x; 4]` is a fill loop (C forbids
    // array assignment).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr3 = m.types.intern(Ty::Array(u32t, 3));
    let arr4 = m.types.intern(Ty::Array(u32t, 4));
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    // _0 u32 (ret), _1 u32 (arg), _2 [u32;3], _3 [u32;4].
    let c0 = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let body = build_body(
        1,
        vec![u32t, u32t, arr3, arr4],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(2),
                    Rvalue::Aggregate(
                        AggregateKind::Array(arr3),
                        Box::new([
                            Operand::Copy(loc(1)),
                            Operand::Copy(loc(1)),
                            Operand::Constant(c0),
                        ]),
                    ),
                )),
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Repeat(Operand::Copy(loc(1)), 4),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "arrays", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_2[0] = _1; _2[1] = _1; _2[2] = ((uint32_t)(0U));"),
        "array aggregate is per-element stores:\n{tu}"
    );
    assert!(
        tu.contains("for (size_t _ri = 0; _ri < 4; _ri++) _3[_ri] = _1;"),
        "Repeat is a fill loop:\n{tu}"
    );
}

#[test]
fn fat_raw_ptr_aggregate_builds_data_len_thin_drops_metadata() {
    // `AggregateKind::RawPtr(Slice(u32), Mut)` builds `{ data, len }`; a sized
    // (thin) pointee aggregate is just the data operand (metadata dropped).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let usize_t = uint(&mut m, UintKind::Usize);
    let slice = m.types.intern(Ty::Slice(u32t));
    let raw_slice = m.types.intern(Ty::Ptr(slice, Mutability::Mut));
    let elem_ptr = m.types.intern(Ty::Ptr(u32t, Mutability::Mut));
    let raw_thin = m.types.intern(Ty::Ptr(u32t, Mutability::Mut));
    let sig = fnsig(
        &mut m,
        vec![elem_ptr, usize_t],
        raw_slice,
        FnAbi::Rust,
        false,
    );
    // _0 *mut [u32] (ret), _1 *mut u32 (data, arg), _2 usize (len, arg),
    // _3 *mut u32 (thin raw ptr).
    let body = build_body(
        2,
        vec![raw_slice, elem_ptr, usize_t, raw_thin],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Aggregate(
                        AggregateKind::RawPtr(slice, Mutability::Mut),
                        Box::new([Operand::Copy(loc(1)), Operand::Copy(loc(2))]),
                    ),
                )),
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Aggregate(
                        AggregateKind::RawPtr(u32t, Mutability::Mut),
                        Box::new([Operand::Copy(loc(1)), Operand::Copy(loc(2))]),
                    ),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "raw_ptrs", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = (struct rust_slice_uint32_t){ .data = (uint32_t *)(_1), .len = _2 };"),
        "fat raw pointer is {{ data, len }}:\n{tu}"
    );
    assert!(
        tu.contains("_3 = _1;"),
        "thin raw pointer aggregate is the data operand, metadata dropped:\n{tu}"
    );
}

#[test]
fn thin_raw_ptr_aggregate_lowers_metadata_before_dropping_it() {
    // A sized RawPtr aggregate still emits the data operand, but its metadata
    // operand must be lowered first so deferred constants fail loud.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let raw_thin = m.types.intern(Ty::Ptr(u32t, Mutability::Mut));
    let sig = fnsig(&mut m, vec![raw_thin], raw_thin, FnAbi::Rust, false);
    let metadata = m.consts.intern(Const::Aggregate(vec![]));
    let body = build_body(
        1,
        vec![raw_thin, raw_thin],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::RawPtr(u32t, Mutability::Mut),
                    Box::new([Operand::Copy(loc(1)), Operand::Constant(metadata)]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "thin_raw_ptr_bad_metadata", sig, Some(body));

    assert!(matches!(emit_generic_c(&m), Err(EmitError::Unsupported(_))));
}

// --- B4: the deferred seams fail loud with the right family ---

/// Assert that emitting `m` returns `EmitError::Unsupported`.
fn assert_unsupported(m: &RegseqModel) {
    assert!(matches!(emit_generic_c(m), Err(EmitError::Unsupported(_))));
}

#[test]
fn downcast_place_is_unsupported_d2_1() {
    let mut m = model();
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    let dst = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::Downcast(VariantIdx::from_u32(0))]),
    };
    fn_with_stmt(
        &mut m,
        "dc",
        st(StatementKind::Assign(
            dst,
            Rvalue::Use(Operand::Constant(zero)),
        )),
    );
    assert_unsupported(&m);
}

#[test]
fn enum_discriminant_is_unsupported_d2_1() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let e_ty = push_simple_enum(&mut m);
    let sig = fnsig(&mut m, vec![e_ty], u32t, FnAbi::Rust, false);
    // _0 u32 (ret), _1 E (arg).
    let body = build_body(
        1,
        vec![u32t, e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Discriminant(loc(1)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "disc", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn non_enum_discriminant_is_constant_zero() {
    // section 21.2.8: a Discriminant of a non-enum predecessor is the constant 0.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![u32t, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Discriminant(loc(1)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "disc0", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("_0 = 0;"), "non-enum discriminant is 0:\n{tu}");
}

#[test]
fn enum_aggregate_is_unsupported_d2_1() {
    let mut m = model();
    let e_ty = push_simple_enum(&mut m);
    let sig = fnsig(&mut m, vec![], e_ty, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::Enum(e_ty, VariantIdx::from_u32(0)),
                    Box::new([]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "mk_enum", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn const_enum_value_is_unsupported_d2_1() {
    let mut m = model();
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    fn_with_stmt(
        &mut m,
        "cev",
        st(StatementKind::Assign(
            loc(0),
            Rvalue::Use(Operand::Constant(ev)),
        )),
    );
    assert_unsupported(&m);
}

#[test]
fn reloc_bearing_bytes_const_is_unsupported_d2_2() {
    // A relocation-bearing Const::Bytes (a non-scalar const carrying a pointer
    // slot) still fails loud at the const-lowering seam (task d2.2).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let target_sig = fnsig(&mut m, vec![], u32t, FnAbi::Rust, false);
    let target = fn_instance(&mut m, "tgt", target_sig, None);
    let bytes = m.consts.intern(Const::Bytes(vec![regseq_model::Byte::Reloc(
        m.instance_ref_of(target),
    )]));
    fn_with_stmt(
        &mut m,
        "relocbytes",
        st(StatementKind::Assign(
            loc(0),
            Rvalue::Use(Operand::Constant(bytes)),
        )),
    );
    assert_unsupported(&m);
}

#[test]
fn const_aggregate_reference_carrier_is_unsupported_d2_2() {
    // The Const::Aggregate([Reloc, Uint]) reference carrier (transitively
    // coupled to a Bytes static def) fails loud at the const seam (task d2.2).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let a_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "A",
    );
    let a_em = m.strings.intern("A");
    let a_id = m.instances.push(Instance {
        item: a_item,
        generic_args: GenericArgs::new(),
        emission_name: a_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: u32t,
            init: Some(one),
        },
    });
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(a_id)));
    let usize_t = uint(&mut m, UintKind::Usize);
    let len = m.consts.intern(Const::Uint(UintKind::Usize, 3));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, len]));
    let _ = usize_t;
    fn_with_stmt(
        &mut m,
        "carrier",
        st(StatementKind::Assign(
            loc(0),
            Rvalue::Use(Operand::Constant(carrier)),
        )),
    );
    assert_unsupported(&m);
}

/// A `&dyn` reference type (the `vtable_ty` is a scalar -- the trait label is
/// deterministic without a nominal vtable struct).
fn dyn_ref(m: &mut RegseqModel) -> TyId {
    let u32t = uint(m, UintKind::U32);
    let dynamic = m.types.intern(Ty::Dynamic {
        auto_traits: vec![],
        vtable_ty: u32t,
    });
    m.types.intern(Ty::Ref(dynamic, Provenance::SharedBorrow))
}

#[test]
fn unsize_to_dyn_is_unsupported_vtable() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let u32_ref = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let dref = dyn_ref(&mut m);
    let sig = fnsig(&mut m, vec![u32_ref], dref, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![dref, u32_ref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(
                    CastKind::PointerCoercion(PointerCoercion::Unsize),
                    Operand::Copy(loc(1)),
                    dref,
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "to_dyn", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn closure_fn_pointer_cast_is_unsupported_vtable() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let zero = m.consts.intern(Const::Uint(UintKind::U32, 0));
    fn_with_stmt(
        &mut m,
        "clofn",
        st(StatementKind::Assign(
            loc(0),
            Rvalue::Cast(
                CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer),
                Operand::Constant(zero),
                u32t,
            ),
        )),
    );
    assert_unsupported(&m);
}

#[test]
fn ptr_metadata_of_dyn_is_unsupported_vtable() {
    let mut m = model();
    let usize_t = uint(&mut m, UintKind::Usize);
    let dref = dyn_ref(&mut m);
    let sig = fnsig(&mut m, vec![dref], usize_t, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![usize_t, dref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(loc(1))),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "dyn_meta", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn ref_of_dyn_deref_is_unsupported_vtable() {
    let mut m = model();
    let dref = dyn_ref(&mut m);
    let sig = fnsig(&mut m, vec![dref], dref, FnAbi::Rust, false);
    let deref = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Deref]),
    };
    let body = build_body(
        1,
        vec![dref, dref],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Ref(Provenance::SharedBorrow, deref),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "ref_dyn", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn struct_aggregate_with_array_field_lowers_per_field() {
    // struct S { xs: [u32; 2] @0, n: u32 @8 }. A compound literal cannot
    // initialize the array member from an array expression, so the aggregate
    // lowers per-field at the assign site: the array via memcpy, the scalar by
    // assignment.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr2 = m.types.intern(Ty::Array(u32t, 2));
    let xs = fl(&mut m, "xs", arr2, 0);
    let n = fl(&mut m, "n", u32t, 8);
    let s = intern_struct(&mut m, "S", struct_layout(12, 4, None, vec![xs, n]));
    let sig = fnsig(&mut m, vec![u32t], s, FnAbi::Rust, false);
    // _0 S (ret), _1 u32 (arg v), _2 [u32; 2].
    let body = build_body(
        1,
        vec![s, u32t, arr2],
        vec![bb(
            vec![
                st(StatementKind::Assign(
                    loc(2),
                    Rvalue::Repeat(Operand::Copy(loc(1)), 2),
                )),
                st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Aggregate(
                        AggregateKind::Struct(s),
                        Box::new([Operand::Copy(loc(2)), Operand::Copy(loc(1))]),
                    ),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "build_s", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("memcpy(&_0.xs, &(_2), sizeof(_0.xs));"),
        "array field lowers via memcpy:\n{tu}"
    );
    assert!(
        tu.contains("_0.n = _1;"),
        "scalar field by assignment:\n{tu}"
    );
    assert!(
        !tu.contains("(struct S){"),
        "no array-bearing compound literal:\n{tu}"
    );
}

#[test]
fn ref_of_packed_field_is_unsupported_misaligned() {
    // #[repr(packed)] P { x: u8 @0, y: u32 @1 }. Taking the address of `y`
    // (`&raw const p.y`) yields a possibly-misaligned pointer -> fail loud
    // (section 14.7); a plain value load of the same field would be fine.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let x = fl(&mut m, "x", u8t, 0);
    let y = fl(&mut m, "y", u32t, 1);
    let p = intern_struct(&mut m, "P", struct_layout(5, 1, Some(1), vec![x, y]));
    let raw = m.types.intern(Ty::Ptr(u32t, Mutability::Not));
    let sig = fnsig(&mut m, vec![p], raw, FnAbi::Rust, false);
    // _0 *const u32 (ret), _1 P (arg). `_0 = &raw const _1.y`.
    let yplace = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), u32t)]),
    };
    let body = build_body(
        1,
        vec![raw, p],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::RawPtr(Mutability::Not, yplace),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "addr_packed", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn by_value_array_call_argument_is_unsupported() {
    // Passing a `[u32; 3]` by value to a call would decay to a pointer in C
    // (losing Rust by-value array semantics) -> fail loud (boundary 6).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr3 = m.types.intern(Ty::Array(u32t, 3));
    let callee_sig = fnsig(&mut m, vec![arr3], u32t, FnAbi::Rust, false);
    let callee = fn_instance(&mut m, "takes_array", callee_sig, None);
    let sig = fnsig(&mut m, vec![arr3], u32t, FnAbi::Rust, false);
    // _0 u32 (ret), _1 [u32; 3] (arg).
    let mut body = build_body(
        1,
        vec![u32t, arr3],
        vec![bb(
            vec![],
            tk(TerminatorKind::Call {
                callee: CallCallee::Direct(callee),
                args: Box::new([Operand::Copy(loc(1))]),
                destination: loc(0),
                target: Some(BlockId::from_u32(1)),
            }),
        )],
    );
    body.basic_blocks
        .push(bb(vec![], tk(TerminatorKind::Return)));
    fn_instance(&mut m, "passes_array", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn array_subslice_ref_through_packed_field_is_unsupported() {
    // #[repr(packed)] P { x: u8 @0, arr: [u32; 4] @1 }. `&raw const p.arr[1..3]`
    // takes the address of the sub-array through the packed `arr` field -> a
    // possibly-misaligned pointer -> fail loud (section 14.7).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr4 = m.types.intern(Ty::Array(u32t, 4));
    let arr2 = m.types.intern(Ty::Array(u32t, 2));
    let raw2 = m.types.intern(Ty::Ptr(arr2, Mutability::Not));
    let x = fl(&mut m, "x", u8t, 0);
    let arr_field = fl(&mut m, "arr", arr4, 1);
    let p = intern_struct(
        &mut m,
        "P",
        struct_layout(17, 1, Some(1), vec![x, arr_field]),
    );
    let sig = fnsig(&mut m, vec![p], raw2, FnAbi::Rust, false);
    let sub = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Field(FieldIdx::from_u32(1), arr4),
            PlaceElem::Subslice {
                from: 1,
                to: 3,
                from_end: false,
            },
        ]),
    };
    let body = build_body(
        1,
        vec![raw2, p],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::RawPtr(Mutability::Not, sub),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "sub_packed", sig, Some(body));
    assert_unsupported(&m);
}

#[test]
fn repeat_loop_var_avoids_a_colliding_body_local() {
    // A body local literally named `_ri` forces the Repeat loop variable to a
    // fresh, non-colliding name.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr2 = m.types.intern(Ty::Array(u32t, 2));
    let sig = fnsig(&mut m, vec![u32t], arr2, FnAbi::Rust, false);
    // _0 [u32; 2] (ret), _1 u32 (arg, debug-named `_ri`).
    let mut body = build_body(
        1,
        vec![arr2, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Repeat(Operand::Copy(loc(1)), 2),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    let ri = m.strings.intern("_ri");
    body.local_decls[LocalId::from_u32(1)].debug_name = Some(ri);
    fn_instance(&mut m, "fill", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    // The arg local renders as `_ri`; the loop var must not reuse it.
    assert!(
        tu.contains("for (size_t _ri_1 = 0;"),
        "loop var is renamed off the colliding `_ri` local:\n{tu}"
    );
    assert!(
        tu.contains("_0[_ri_1] = _ri;"),
        "the renamed loop var indexes, the local supplies the value:\n{tu}"
    );
}

#[test]
fn structured_data_model_is_byte_identical_across_two_calls() {
    let build = || {
        let mut m = model();
        let u8t = uint(&mut m, UintKind::U8);
        let u32t = uint(&mut m, UintKind::U32);
        let a = fl(&mut m, "a", u8t, 4);
        let b = fl(&mut m, "b", u32t, 0);
        let point = intern_struct(&mut m, "Point", struct_layout(8, 4, None, vec![a, b]));
        let sig = fnsig(&mut m, vec![u8t, u32t], u32t, FnAbi::Rust, false);
        let body = build_body(
            2,
            vec![u32t, u8t, u32t, point],
            vec![bb(
                vec![
                    st(StatementKind::Assign(
                        loc(3),
                        Rvalue::Aggregate(
                            AggregateKind::Struct(point),
                            Box::new([Operand::Copy(loc(1)), Operand::Copy(loc(2))]),
                        ),
                    )),
                    st(StatementKind::Assign(
                        loc(0),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(3),
                            projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), u32t)]),
                        })),
                    )),
                ],
                tk(TerminatorKind::Return),
            )],
        );
        fn_instance(&mut m, "build_point", sig, Some(body));
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}
