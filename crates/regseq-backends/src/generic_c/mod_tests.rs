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
    AutoTrait, CrateId, DefPath, Endian, EnumShape, FieldLayout, GenericArgs, Instance,
    InstanceKind, Item, ItemKind, Layout, LayoutKind, Linkage, Provenance, ScalarLayouts,
    SizeAlign, SourceFileId, Span, StableCrateId, TargetInfo, Ty, TyId, UintKind, VariantLayout,
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
// B4: scope hooks -- enum (forward-decl only) + dyn (trait_obj shape only).
// ---------------------------------------------------------------------------

#[test]
fn enum_is_forward_declared_but_body_is_deferred_to_c2() {
    let mut m = model();
    let name = m.strings.intern("Idle");
    let it = item(&mut m, ItemKind::Enum, "E");
    let shape = EnumShape::Single {
        variant: VariantLayout {
            name,
            fields: Vec::new(),
            uninhabited: false,
            discriminant: 0,
            tag_value: None,
        },
        scalar: None,
    };
    let ty = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: Layout {
            size: 1,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum { shape },
        },
    });
    let emission = m.strings.intern("E");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Enum { layout: ty },
    });

    let tu = emit_generic_c(&m).unwrap();
    assert!(tu.contains("struct E;"), "enum must be forward-declared");
    assert!(
        tu.contains("TODO(task c2): enum body for E"),
        "enum body is the c2 hook"
    );
    assert!(
        !tu.contains("_Static_assert(sizeof(struct E)"),
        "no enum body / layout guards in c1"
    );
}

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
