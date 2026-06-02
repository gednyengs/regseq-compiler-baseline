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
    // The helper block is now demand-driven (plan 10), so a no-arithmetic model
    // emits no `regseq_{add,..}_<T>` wrapper. Anchor the ordering check on
    // `regseq_panic`, emitted by the static `PRELUDE` after the includes.
    let inc = tu.find("#include <stdint.h>").unwrap();
    let after_includes = tu.find("regseq_panic").unwrap();
    assert!(inc < after_includes);
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
        is_result: false,
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
    // Change A: the always-emitted byte-covering `_raw` member + its zero-offset
    // guard; the size/align asserts are unchanged (layout-neutral).
    assert!(
        tu.contains("unsigned char _raw[4];"),
        "union _raw member:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(offsetof(union U, x) == 0, \"U.x offset\");"));
    assert!(tu.contains("_Static_assert(offsetof(union U, y) == 0, \"U.y offset\");"));
    assert!(tu.contains("_Static_assert(offsetof(union U, _raw) == 0, \"U._raw offset\");"));
    assert!(tu.contains("_Static_assert(sizeof(union U) == 4, \"U size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(union U) == 4, \"U align\");"));
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
fn tuple_struct_field_emits_indexed_c_name() {
    // A tuple struct `struct TS(u32)` — the bridge interns its single field
    // with rustc's raw numeric index name "0", which is NOT a valid C
    // identifier. The backend must emit it as `_0` (the same spelling
    // tuples use) in the member declaration AND the offsetof guard, so the
    // TU compiles. The raw `0` must NOT appear as a member designator.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let f0 = fl(&mut m, "0", u32t, 0);
    push_struct_instance(&mut m, "TS", struct_layout(4, 4, None, vec![f0]));

    let tu = emit_generic_c(&m).unwrap();
    // Member declaration uses the valid-C `_0` name.
    assert!(tu.contains("uint32_t _0;"), "tuple-struct field _0:\n{tu}");
    // The offsetof guard uses the same `_0` name (single source of truth).
    assert!(
        tu.contains("_Static_assert(offsetof(struct TS, _0) == 0, \"TS._0 offset\");"),
        "offsetof guard must use _0:\n{tu}"
    );
    // The raw numeric name `0` is never spelled as a member declarator or
    // designator (would be invalid C).
    assert!(
        !tu.contains("uint32_t 0;"),
        "raw numeric member name must not be emitted:\n{tu}"
    );
    assert!(
        !tu.contains("offsetof(struct TS, 0)"),
        "raw numeric designator must not be emitted:\n{tu}"
    );
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
    // The vtable field points at the real vtable_ty struct (here named "Reader"),
    // forward-declared by that name -- not an invented `_VTable` alias (vtable task).
    assert!(tu.contains("struct Reader;"), "vtable struct forward-decl");
    assert!(tu.contains("struct trait_obj_Reader {"));
    assert!(tu.contains("void *data;"));
    assert!(tu.contains("const struct Reader *vtable;"));
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
fn union_raw_member_subsumes_the_pad_filler() {
    // Change A: a union whose largest member (u8) does not span layout.size (4)
    // previously needed a `_pad` filler; the always-emitted `_raw[4]` now pins
    // sizeof, so there is no `_pad` filler at all (a real `_pad` field, which is
    // not `_raw`, is untouched).
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
        tu.contains("unsigned char _raw[4];"),
        "the _raw member pins sizeof (subsumes the _pad filler):\n{tu}"
    );
    assert!(
        !tu.contains("unsigned char _pad0["),
        "no separate _pad filler remains:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(sizeof(union U) == 4, \"U size\");"));
}

#[test]
fn union_raw_member_avoids_a_real_raw_field() {
    // Change A collision-proofing: a union with a real field named `_raw` gets a
    // collision-proof byte-covering member `_raw0`; the byte-image init
    // designator matches (`union_raw_member` yields `_raw0`).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let r = fl(&mut m, "_raw", u8t, 0);
    let it = item(&mut m, ItemKind::Union, "U");
    let union_ty = m.types.intern(Ty::Union {
        item: it,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![r] },
        },
    });
    let bytes = m.consts.intern(Const::Bytes(le32(9)));
    push_static(&mut m, "URAW", union_ty, bytes);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint8_t _raw;"),
        "real _raw member present:\n{tu}"
    );
    assert!(
        tu.contains("unsigned char _raw0[4];"),
        "collision-proof byte-covering member:\n{tu}"
    );
    assert!(
        tu.contains("(union U){ ._raw0 = { 9, 0, 0, 0 } }"),
        "the byte image init designates the collision-proof _raw0:\n{tu}"
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
// backends-25: a *function* whose RETURN type is a fn pointer / pointer-to-array
// wraps the function declarator (`R (*name(params))(...)`) instead of the
// malformed bare-type concatenation `R (*)(...) name(params)`. `fn_signature`
// routes the return type through `types::declare`, the same declarator machinery
// the params (and the field-context tests above) already use.
// ---------------------------------------------------------------------------

#[test]
fn fn_returning_fn_ptr_wraps_the_declarator() {
    // `coerce() -> fn() -> i32` -> `int32_t (*coerce(void))(void);`, NOT the
    // pre-fix malformed `int32_t (*)(void) coerce(void);`.
    let mut m = model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let ret_fnptr = fn_ptr_returning(&mut m, i32t);
    let sig = fnsig(&mut m, vec![], ret_fnptr, FnAbi::Rust, false);
    fn_instance(&mut m, "coerce", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("int32_t (*coerce(void))(void);"),
        "fn-ptr return must wrap the declarator:\n{tu}"
    );
    assert!(
        !tu.contains("(*)(void) coerce"),
        "the malformed bare-type concatenation must be gone:\n{tu}"
    );
}

#[test]
fn fn_returning_pointer_to_array_wraps_the_declarator() {
    // `dims() -> &[u32; 4]` -> `const uint32_t (*dims(void))[4];`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 4));
    let aref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let sig = fnsig(&mut m, vec![], aref, FnAbi::Rust, false);
    fn_instance(&mut m, "dims", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint32_t (*dims(void))[4];"),
        "pointer-to-array return must wrap the declarator:\n{tu}"
    );
}

#[test]
fn fn_returning_scalar_is_unchanged() {
    // Regression guard: a non-fn-ptr return stays byte-identical to the
    // pre-fix `<type> <name>(<params>)` spelling.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    fn_instance(&mut m, "ident", sig, None);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t ident(uint32_t);"),
        "scalar return prototype must be unchanged:\n{tu}"
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
    // view. Change A (d2.3b): the one view is wrapped in an ANONYMOUS
    // union+struct + a byte-covering `_raw[4]`; gnu11 member promotion keeps
    // `e.tag` access (and `offsetof(struct Status, tag)`) unchanged.
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
    assert!(tu.contains("uint32_t tag;"), "promoted tag field:\n{tu}");
    assert!(
        tu.contains("unsigned char _raw[4];"),
        "the anonymous-union byte-covering _raw member:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Status, tag) == 0, \"Status.tag offset\");"),
        "promoted tag stays offsetof-addressable at 0:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Status, _raw) == 0, \"Status._raw offset\");"),
        "the promoted _raw member is at offset 0:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(sizeof(struct Status) == 4, \"Status size\");"));
    assert!(tu.contains("__attribute__((aligned(4)))"));
}

#[test]
fn niche_enum_collapses_to_single_field_struct() {
    // Section 9.9: `Option<&u32>` is `Niche` (no tag view) with `None`
    // fieldless + `Some` carrying the pointer at offset 0 -> a single view.
    // Change A (d2.3b): the one view is wrapped in an ANONYMOUS union+struct +
    // a byte-covering `_raw[8]`; member promotion keeps `e._0` access unchanged.
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
        "promoted single pointer field:\n{tu}"
    );
    assert!(
        tu.contains("unsigned char _raw[8];"),
        "the anonymous-union byte-covering _raw member:\n{tu}"
    );
    assert!(!tu.contains(" tag;"), "niche enum has no tag view:\n{tu}");
    assert!(tu.contains("_Static_assert(offsetof(struct Opt, _0) == 0, \"Opt._0 offset\");"));
    assert!(tu.contains("_Static_assert(offsetof(struct Opt, _raw) == 0, \"Opt._raw offset\");"));
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
fn c_abi_substituted_input_and_output_marshal_through_body() {
    // `#[repr(transparent)] struct W(u32);` and `extern "C" fn id_w(w: W) -> W`
    // with the input AND output C-ABI-substituted to the inner u32. The
    // prototype/definition declare the ABI type (`uint32_t`), the body keeps
    // using the original `W` local, and the boundary is bridged by marshalling:
    // an input copy at the prologue, a return copy back to the ABI type, and a
    // reinterpret of the argument at the call site.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    // W { _0: u32 } -- a single-field repr-identical newtype.
    let w_field = fl(&mut m, "0", u32t, 0);
    let w_ty = push_struct_instance(&mut m, "W", struct_layout(4, 4, None, vec![w_field]));

    // id_w: extern "C" fn(W) -> W, both positions substituted to u32.
    let sig = fnsig(&mut m, vec![w_ty], w_ty, FnAbi::C, false);
    while m.sig_c_abi_lowerings.len() <= sig.as_u32() as usize {
        m.sig_c_abi_lowerings.push(None);
    }
    m.sig_c_abi_lowerings[sig] = Some(CAbiLowering {
        inputs: vec![CAbiShape::Substituted {
            emit_as: u32t,
            reason: SubstReason::TransparentChain,
        }],
        output: CAbiShape::Substituted {
            emit_as: u32t,
            reason: SubstReason::TransparentChain,
        },
    });
    // Body: `_0 = _1; return _0;` (copy the W arg to the W return place).
    let id_body = build_body(
        1,
        vec![w_ty, w_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Copy(loc(1))),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    let id_w = fn_instance(&mut m, "id_w", sig, Some(id_body));

    // Caller: `regseq_main() { let r = id_w(w); }` -- a W-typed arg local.
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let main_sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    let main_body = build_body(
        0,
        // _0: unit, _1: W (the argument value), _2: W (the call result)
        vec![unit, w_ty, w_ty],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(id_w),
                    args: Box::new([Operand::Copy(loc(1))]),
                    destination: loc(2),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    fn_instance(&mut m, "regseq_main", main_sig, Some(main_body));

    let tu = emit_generic_c(&m).unwrap();

    // Prototype + definition spell the ABI type for both positions.
    assert!(
        tu.contains("uint32_t id_w(uint32_t"),
        "ABI-typed prototype/definition:\n{tu}"
    );
    // The body keeps the original W local (return place + arg local) and bridges
    // via a representation-identical memcpy at the prologue.
    assert!(
        tu.contains("struct W _0;"),
        "body keeps original-typed return place:\n{tu}"
    );
    assert!(
        tu.contains("memcpy(") && tu.contains("__abi"),
        "input marshalling copies the ABI param into the body local:\n{tu}"
    );
    // The call site reinterprets the W argument to the ABI type.
    assert!(
        tu.contains("id_w(") && tu.matches("memcpy(").count() >= 3,
        "input prologue + return + call-site marshalling all present:\n{tu}"
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
fn bytes_static_initializer_reconstructs_typed_byte_array() {
    // Change B: a non-scalar `static` arrives as `Const::Bytes`; a `[u8; 1]`
    // consumer reconstructs as a typed `uint8_t[1]` initializer, not a deferral.
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
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint8_t BUF[1] = { ((uint8_t)(120U)) };"),
        "byte array reconstructs as a typed uint8_t[1] initializer:\n{tu}"
    );
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
    // Plan 10 (fine-grained demand): only the used helper + its result struct
    // are defined. The other ops for u32, the same op for other types, and the
    // whole saturating-cast family must be absent.
    assert!(
        tu.contains("struct regseq_overflow_uint32_t {"),
        "the used result struct is defined:\n{tu}"
    );
    assert!(
        !tu.contains("regseq_sub_uint32_t") && !tu.contains("regseq_mul_uint32_t"),
        "unused ops for the same type are NOT defined:\n{tu}"
    );
    assert!(
        !tu.contains("regseq_add_int8_t") && !tu.contains("struct regseq_overflow_int8_t"),
        "the same op for an unused type is NOT defined:\n{tu}"
    );
    // The section-11 gated comment + any cast helper are absent (the always-on
    // block header mentions "section 11", so key on the gated comment instead).
    assert!(
        !tu.contains("saturating FloatToInt") && !tu.contains("regseq_f64_to_"),
        "the unused saturating-cast family is NOT defined:\n{tu}"
    );
}

#[test]
fn helper_block_is_empty_for_a_no_arithmetic_model() {
    // Plan 10 (de-clobber): a model that uses no `*WithOverflow` and no
    // `FloatToInt` emits NONE of the ~96 helper definitions -- no wrappers, no
    // result structs, no saturating casts, and neither section comment.
    // Markers below are the gated section comments + helper tokens; the
    // always-on block header mentions "section 10.1 + section 11", so those
    // bare strings are NOT used as absence markers.
    let tu = emit_generic_c(&trivial_model()).unwrap();
    for tok in [
        "regseq_add_",
        "regseq_sub_",
        "regseq_mul_",
        "struct regseq_overflow_",
        "regseq_f64_to_",
        "{add,sub,mul}WithOverflow",
        "saturating FloatToInt",
    ] {
        assert!(
            !tu.contains(tok),
            "no-arithmetic TU must not define helper `{tok}`:\n{tu}"
        );
    }
}

#[test]
fn cmp_materializes_into_the_ordering_tag_cell() {
    // `BinOp::Cmp` (BO-25): `_0: Ordering = Cmp(_1, _2)`. `Ordering` is a
    // `repr(i8)` fieldless `Direct` enum (Less = -1, Equal = 0, Greater = 1),
    // lowered to `struct core_cmp_Ordering` with an `i8` tag cell. The three-way
    // scalar `(x > y) - (x < y)` (an i8 in {-1, 0, 1}) is exactly the variant
    // discriminant == tag value, so it is written into the tag cell at the
    // assign site (mirroring `assign_with_overflow`), NOT emitted as a bare
    // `int8_t` rvalue assigned straight to the enum-struct place.
    let mut m = model();
    let i8t = m.types.intern(Ty::Int(IntKind::I8));
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let tag = TagScalar {
        offset: 0,
        scalar_ty: i8t,
        valid_range: WrappingRange {
            start: (-1i8) as u8 as u128,
            end: 1,
        },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(
                &mut m,
                "Less",
                Vec::new(),
                (-1i8) as u8 as u128,
                Some((-1i8) as u8 as u128),
            ),
            variant(&mut m, "Equal", Vec::new(), 0, Some(0)),
            variant(&mut m, "Greater", Vec::new(), 1, Some(1)),
        ],
    };
    let ord = push_enum_instance(&mut m, "core_cmp_Ordering", enum_layout(1, 1, shape));
    let sig = fnsig(&mut m, vec![i32t, i32t], ord, FnAbi::Rust, false);
    // _0 Ordering (ret), _1 i32 (arg), _2 i32 (arg).
    let body = build_body(
        2,
        vec![ord, i32t, i32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::BinaryOp(BinOp::Cmp, Operand::Copy(loc(1)), Operand::Copy(loc(2))),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "cmp", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    // The three-way scalar is written into the tag cell, cast to the i8 tag
    // type -- the same `<enum>.tag = ...;` form `emit_set_discriminant` uses.
    assert!(
        tu.contains("_0.tag = (int8_t)(((_1) > (_2)) - ((_1) < (_2)));"),
        "Cmp writes the three-way scalar into the Ordering tag cell:\n{tu}"
    );
    // The bare-int8_t nested-rvalue spelling must NOT be assigned straight to
    // the enum-struct place (the bug under fix).
    assert!(
        !tu.contains("_0 = ((int8_t)"),
        "no bare int8_t assigned to the Ordering struct place:\n{tu}"
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
fn reify_fn_pointer_from_reloc_constant_operand_matches_definition_symbol() {
    // regseq-backends-20: the rustc-shaped ReifyFnPointer operand. A direct
    // fn-item -> fn-pointer coercion (`let f: fn(i32)->i32 = k;`) bridges as
    // `Cast(PointerCoercion(ReifyFnPointer), Operand::Constant(_), FnPtr)` whose
    // constant is the zero-sized FnDef value. The bridge (regseq-backends-20)
    // recovers the FnDef identity at the cast site and lowers that operand to a
    // `Const::Reloc` to the resolved function instance -- the shape the backend
    // actually receives -- which `reify_fn_pointer` resolves through its Reloc
    // path. (The bare `Const::ZST` is NOT used: the model `Const::ZST` carries
    // no FnDef item/args, so it cannot name a target; resolution lives in the
    // bridge where the FnDef type is still available.)
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let k_item = item(&mut m, ItemKind::Function, "k");
    let args = vec![regseq_model::GenericArg::Type(u32t)];
    let k_sig = fnsig(&mut m, vec![u32t], u32t, FnAbi::Rust, false);
    let k_id = push_fn_mono(
        &mut m,
        k_item,
        "k",
        args.clone(),
        k_sig,
        Some(identity_body(u32t)),
    );

    let fnptr_ty = m.types.intern(Ty::FnPtr(k_sig));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(k_id)));
    let csig = fnsig(&mut m, vec![], fnptr_ty, FnAbi::Rust, false);
    // _0: fnptr (return). The reify operand is the Const::Reloc constant.
    let cbody = build_body(
        0,
        vec![fnptr_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Cast(
                    CastKind::PointerCoercion(regseq_model::PointerCoercion::ReifyFnPointer),
                    Operand::Constant(reloc),
                    fnptr_ty,
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "reifier_const", csig, Some(cbody));

    let tu = emit_generic_c(&m).unwrap();
    let sym = internal_symbol(&m, "k", &args);
    assert!(
        tu.contains(&format!("_0 = (&{sym});")),
        "ReifyFnPointer of a Const::Reloc operand spells the suffixed symbol:\n{tu}"
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
    // sync with the *emittable* helper superset (Change B step 1 / plan 10):
    // every reserved name is defined when the full library is requested.
    // `helper_block` is now demand-driven, so feed it `HelperUsage::all()`; the
    // reservation stays full even though any one TU emits only a subset.
    let prelude_text = format!(
        "{}{}",
        prelude::PRELUDE,
        prelude::helper_block(&prelude::HelperUsage::all())
    );
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
fn reloc_bearing_bytes_const_reconstructs_the_pointer() {
    // Change B pointer arm: a relocation-bearing `Const::Bytes` over a raw
    // `*const u32` consumer -- a complete pointer-width `Byte::Reloc` run --
    // reconstructs as the d1 reloc designator (`&A` cast to the consumer type).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ptr_ty = m.types.intern(Ty::Ptr(u32t, Mutability::Not));
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
    // A pointer-width run of Byte::Reloc(A) (the 64-bit-target pointer slot).
    let reloc_byte = regseq_model::Byte::Reloc(m.instance_ref_of(a_id));
    let bytes = m.consts.intern(Const::Bytes(vec![reloc_byte; 8]));
    let p_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "P",
    );
    let p_em = m.strings.intern("P");
    m.instances.push(Instance {
        item: p_item,
        generic_args: GenericArgs::new(),
        emission_name: p_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: ptr_ty,
            init: Some(bytes),
        },
    });
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("P = ((const uint32_t *)&A);"),
        "a pointer-width Reloc run reconstructs as the reloc designator:\n{tu}"
    );
}

#[test]
fn const_aggregate_reference_carrier_reconstructs_the_pointer() {
    // Change C: the `Const::Aggregate([Reloc, Uint])` reference carrier over a
    // thin `&u32` consumer reconstructs as the d1 reloc designator (the `Uint`
    // metadata placeholder is dropped for a sized pointee).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
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
    let len = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, len]));
    let r_item = item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "R",
    );
    let r_em = m.strings.intern("R");
    m.instances.push(Instance {
        item: r_item,
        generic_args: GenericArgs::new(),
        emission_name: r_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: ref_ty,
            init: Some(carrier),
        },
    });
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("((const uint32_t *)&A)"),
        "the thin reference carrier reconstructs as the reloc designator:\n{tu}"
    );
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
fn ptr_metadata_of_dyn_emits_vtable_slot() {
    // PtrMetadata of a `&dyn` reads the fat pointer's second slot, `.vtable`
    // (mirroring slice/str `.len`) -- the vtable task implemented this.
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
    let tu = emit_generic_c(&m).expect("PtrMetadata of a dyn pointer emits the vtable slot");
    assert!(
        tu.contains(").vtable"),
        "PtrMetadata of dyn reads the vtable slot"
    );
}

#[test]
fn ref_of_dyn_deref_emits_fat_reborrow() {
    // A re-borrow of a whole dyn place (`&(*p)`) copies the fat pointer (its
    // `{ data, vtable }` identity) -- the fat base, mirroring the slice/str
    // re-borrow. The vtable task implemented this (no longer fail-loud).
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
    let tu = emit_generic_c(&m).expect("re-borrow of a dyn deref copies the fat pointer");
    assert!(tu.contains("_0 = _1;"), "dyn re-borrow copies the fat base");
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

// ---------------------------------------------------------------------------
// Task d2.1: section 13 enum tag-ops over the c2 union-of-views. Structural
// assertions (navigator agreement + the lowered shapes) + the d2.2 fail-loud
// negatives + determinism. Compile-and-run proof lives in the
// codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

/// `enum Shift { A(u8), B(u32) }` -- the shifted-alignment multi-view Direct
/// enum (tag u8 @0, A.0 @1, B.0 @4, size 8 align 4): three views (`_tag`,
/// `_v0`, `_v1`), so the body navigator must use the `u.<member>.` prefix.
fn push_shift_enum(m: &mut RegseqModel) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let u32t = uint(m, UintKind::U32);
    let a0 = fl(m, "0", u8t, 1);
    let b0 = fl(m, "0", u32t, 4);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(m, "A", vec![a0], 0, Some(0)),
            variant(m, "B", vec![b0], 1, Some(1)),
        ],
    };
    push_enum_instance(m, "Shift", enum_layout(8, 4, shape))
}

/// `enum AB { A, B(u32) }` -- a multi-view Direct enum with a **fieldless**
/// variant `A` (tag-only) and a payload-bearing `B` (tag u8 @0, B.0 @4, size 8
/// align 4): two views (`_tag`, `_v1`).
fn push_ab_enum(m: &mut RegseqModel) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let u32t = uint(m, UintKind::U32);
    let b0 = fl(m, "0", u32t, 4);
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
            variant(m, "B", vec![b0], 1, Some(1)),
        ],
    };
    push_enum_instance(m, "AB", enum_layout(8, 4, shape))
}

/// `Option<&u32>` -- a `Niche` enum collapsing to an inline single-field
/// struct (the pointer @0 *is* the tag; None is the niched variant with
/// `tag_value: Some(0)`). Returns `(enum_ty, &u32 ref ty)`.
fn push_opt_ref_enum(m: &mut RegseqModel) -> (TyId, TyId) {
    let u32t = uint(m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let some_field = fl(m, "0", ref_ty, 0);
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
            variant(m, "None", Vec::new(), 0, Some(0)),
            variant(m, "Some", vec![some_field], 1, None),
        ],
    };
    let opt = push_enum_instance(m, "Opt", enum_layout(8, 8, shape));
    (opt, ref_ty)
}

/// An inline `Niche` enum whose niche cell is a sub-byte of a nested tuple
/// field (the `Option<(u8, NonZeroU8)>` shape): untagged `Some` holds a
/// `(u8, u8)` tuple @0; the niche cell is at offset 1 (inside the tuple), so
/// it does **not** coincide with one top-level inline scalar field.
fn push_opt_tuple_niche_enum(m: &mut RegseqModel) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let t0 = fl(m, "_0", u8t, 0);
    let t1 = fl(m, "_1", u8t, 1);
    let tup = m.types.intern(Ty::Tuple {
        fields: vec![u8t, u8t],
        layout: struct_layout(2, 1, None, vec![t0, t1]),
    });
    let some_field = fl(m, "0", tup, 0);
    let tag = TagScalar {
        offset: 1,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 1, end: 255 },
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
            variant(m, "None", Vec::new(), 0, Some(0)),
            variant(m, "Some", vec![some_field], 1, None),
        ],
    };
    push_enum_instance(m, "OptT", enum_layout(2, 1, shape))
}

/// `Option<bool>` shape: untagged `Some(bool)` holds the bool @0 (the niche
/// cell IS the bool); `None` is niched with `tag_value = Some(2)` (bool's first
/// invalid bit pattern). Inline (one view), the niche cell coincides with the
/// bool field by offset+type -- but `_Bool` normalizes, so a const literal
/// can't carry a raw 2 (the d2.1 soundness-pass finding).
fn push_opt_bool_niche_enum(m: &mut RegseqModel) -> TyId {
    let boolt = m.types.intern(Ty::Bool);
    let some_field = fl(m, "0", boolt, 0);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: boolt,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(1),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(0),
                end: VariantIdx::from_u32(0),
            },
            niche_start: 2,
        },
        variants: vec![
            variant(m, "None", Vec::new(), 0, Some(2)),
            variant(m, "Some", vec![some_field], 1, None),
        ],
    };
    push_enum_instance(m, "OptBool", enum_layout(1, 1, shape))
}

/// The `EmitError::Unsupported` label `m` fails with (panics if `m` emits Ok
/// or a different error).
fn unsupported_reason(m: &RegseqModel) -> &'static str {
    match emit_generic_c(m) {
        Err(EmitError::Unsupported(s)) => s,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn enum_downcast_field_spells_multiview_navigator_path() {
    // `(_1 as B).0` on a multi-view enum -> `_1.u._v1._0`, the SAME member
    // path `emit_enum` spells in its `offsetof` guard (navigator agreement).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let e_ty = push_shift_enum(&mut m);
    let sig = fnsig(&mut m, vec![e_ty], u32t, FnAbi::Rust, false);
    let place = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Downcast(VariantIdx::from_u32(1)),
            PlaceElem::Field(FieldIdx::from_u32(0), u32t),
        ]),
    };
    let body = build_body(
        1,
        vec![u32t, e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Copy(place)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "read_b", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = _1.u._v1._0;"),
        "downcast+field spells the variant view path:\n{tu}"
    );
    assert!(
        tu.contains("offsetof(struct Shift, u._v1._0)"),
        "the definition's offsetof guard spells the same path:\n{tu}"
    );
}

#[test]
fn inline_enum_downcast_field_has_no_union_prefix() {
    // `(_1 as Some).0` on an inline niche enum -> `_1._0` (no `u._v1.` prefix),
    // matching the collapsed single-field struct definition.
    let mut m = model();
    let (opt, ref_ty) = push_opt_ref_enum(&mut m);
    let sig = fnsig(&mut m, vec![opt], ref_ty, FnAbi::Rust, false);
    let place = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Downcast(VariantIdx::from_u32(1)),
            PlaceElem::Field(FieldIdx::from_u32(0), ref_ty),
        ]),
    };
    let body = build_body(
        1,
        vec![ref_ty, opt],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Copy(place)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "get_some", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = _1._0;"),
        "inline enum field has no union prefix:\n{tu}"
    );
    assert!(
        !tu.contains("u._v1._0"),
        "inline enum must not use a union member path:\n{tu}"
    );
}

#[test]
fn direct_discriminant_maps_tag_to_semantic_discriminant() {
    // A fieldless Direct enum with explicit discriminants (10, 20) != storage
    // index: `Discriminant` is a ternary over the inline `_tag` read mapping
    // each tag_value to the matched variant's semantic discriminant.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 255 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "A", Vec::new(), 10, Some(10)),
            variant(&mut m, "B", Vec::new(), 20, Some(20)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "E", enum_layout(1, 1, shape));
    let u8r = uint(&mut m, UintKind::U8);
    let sig = fnsig(&mut m, vec![e_ty], u8r, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![u8r, e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Discriminant(loc(1)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "disc", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_1.tag == ((uint8_t)(10U)) ?"),
        "ternary over the inline tag read:\n{tu}"
    );
    assert!(
        tu.contains("((uint8_t)(20U))"),
        "fallthrough is the last variant's discriminant:\n{tu}"
    );
}

#[test]
fn single_enum_discriminant_is_the_variant_discriminant_constant() {
    // `enum Only { A = 5 }` (Single { scalar: Some }): Discriminant is the
    // constant 5 (the discriminant, not the storage index 0).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 5, end: 5 },
    };
    let shape = EnumShape::Single {
        variant: variant(&mut m, "A", Vec::new(), 5, Some(5)),
        scalar: Some(tag),
    };
    let e_ty = push_enum_instance(&mut m, "Only", enum_layout(1, 1, shape));
    let u8r = uint(&mut m, UintKind::U8);
    let sig = fnsig(&mut m, vec![e_ty], u8r, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![u8r, e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Discriminant(loc(1)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "disc", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0 = ((uint8_t)(5U));"),
        "single-variant discriminant is the constant:\n{tu}"
    );
}

#[test]
fn niche_discriminant_reads_the_cell_at_the_assign_site() {
    // `Option<&u32>`: the niche cell has no `_tag` view, so Discriminant reads
    // it via a section 14.7 byte-storage memcpy into a temp then a ternary with
    // the untagged-variant fallback.
    let mut m = model();
    let (opt, _ref_ty) = push_opt_ref_enum(&mut m);
    let usize_t = uint(&mut m, UintKind::Usize);
    let sig = fnsig(&mut m, vec![opt], usize_t, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![usize_t, opt],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Discriminant(loc(1)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "disc", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("memcpy(&_nd, (const unsigned char *)&(_1) + 0, sizeof(_nd));"),
        "niche read via byte storage at TagScalar.offset:\n{tu}"
    );
    assert!(
        tu.contains("_nd == ((uint64_t)(0ULL)) ?"),
        "ternary over the niche-cell temp (raw uint storage) with the untagged fallback:\n{tu}"
    );
}

#[test]
fn enum_construct_direct_writes_tag_and_payload() {
    // `Shift::B(_1)`: the tag write through `u._tag` and the payload write
    // through the `_v1` view (disjoint union members).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let e_ty = push_shift_enum(&mut m);
    let sig = fnsig(&mut m, vec![u32t], e_ty, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![e_ty, u32t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::Enum(e_ty, VariantIdx::from_u32(1)),
                    Box::new([Operand::Copy(loc(1))]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "mk_b", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0.u._tag.tag = ((uint8_t)(1U));"),
        "tag write through the _tag view:\n{tu}"
    );
    assert!(
        tu.contains("_0.u._v1._0 = _1;"),
        "payload write through the _v1 view:\n{tu}"
    );
}

#[test]
fn enum_construct_fieldless_direct_writes_only_tag() {
    // `E::B` of a fieldless Direct enum (collapsed inline to a tag-only
    // struct): only the inline `tag` member is written, no payload view.
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
                    AggregateKind::Enum(e_ty, VariantIdx::from_u32(1)),
                    Box::new([]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "mk_b", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("_0.tag = ((uint8_t)(1U));"),
        "fieldless construct writes only the inline tag:\n{tu}"
    );
    assert!(
        !tu.contains("_v"),
        "no payload view for a fieldless variant:\n{tu}"
    );
}

#[test]
fn set_discriminant_niche_writes_cell_and_untagged_is_a_noop() {
    // SetDiscriminant on an `Option<&u32>` local: the niched None writes the
    // niche cell via a section 14.7 memcpy; the untagged Some is a no-op
    // (elided), so only the memcpy is emitted.
    let mut m = model();
    let (opt, _r) = push_opt_ref_enum(&mut m);
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let sig = fnsig(&mut m, vec![opt], unit, FnAbi::Rust, false);
    let body = build_body(
        1,
        vec![unit, opt],
        vec![bb(
            vec![
                st(StatementKind::SetDiscriminant(
                    loc(1),
                    VariantIdx::from_u32(0),
                )),
                st(StatementKind::SetDiscriminant(
                    loc(1),
                    VariantIdx::from_u32(1),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "setd", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("memcpy((unsigned char *)&(_1) + 0, &(uint64_t){((uint64_t)(0ULL))}, sizeof(uint64_t));"),
        "niche cell write via raw-storage byte memcpy:\n{tu}"
    );
    // The untagged Some SetDiscriminant is a no-op: exactly one niche memcpy.
    assert_eq!(
        tu.matches("memcpy((unsigned char *)&(_1) + 0,").count(),
        1,
        "the untagged variant SetDiscriminant is elided:\n{tu}"
    );
}

#[test]
fn enum_construct_niche_niched_writes_only_the_cell() {
    // `Option<&u32>::None` (niche niched, fieldless): construction writes only
    // the niche cell (no payload), via the section 14.7 memcpy.
    let mut m = model();
    let (opt, _r) = push_opt_ref_enum(&mut m);
    let sig = fnsig(&mut m, vec![], opt, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![opt],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::Enum(opt, VariantIdx::from_u32(0)),
                    Box::new([]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "mk_none", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("memcpy((unsigned char *)&(_0) + 0, &(uint64_t){((uint64_t)(0ULL))}, sizeof(uint64_t));"),
        "niche niched construction writes only the niche cell (raw-storage memcpy):\n{tu}"
    );
}

#[test]
fn const_enum_value_fieldless_direct_multiview_designates_the_tag() {
    // A single-member `Const::EnumValue` for a fieldless variant of a
    // multi-view Direct enum -> `(struct AB){ .u._tag.tag = <tv> }` (the shared
    // navigator's nested tag designator).
    let mut m = model();
    let e_ty = push_ab_enum(&mut m);
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let sig = fnsig(&mut m, vec![], e_ty, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "ka", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct AB){ .u._tag.tag = ((uint8_t)(0U)) }"),
        "fieldless multi-view EnumValue designates the navigator tag path:\n{tu}"
    );
}

#[test]
fn const_enum_value_inline_niche_none_serializes_to_raw() {
    // Change B (d2.3b): a single-member `Const::EnumValue` for
    // `Option<&u32>::None` (an inline niched variant, niche value 0 at the
    // 8-byte pointer cell @0) serializes uniformly to the anonymous-union
    // `_raw` byte image -> `(struct Opt){ ._raw = { 0, 0, 0, 0, 0, 0, 0, 0 } }`.
    let mut m = model();
    let (opt, _r) = push_opt_ref_enum(&mut m);
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let sig = fnsig(&mut m, vec![], opt, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![opt],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "knone", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct Opt){ ._raw = { 0, 0, 0, 0, 0, 0, 0, 0 } }"),
        "inline niche None serializes the niche value into _raw:\n{tu}"
    );
}

#[test]
fn const_enum_value_payload_bearing_direct_multiview_uses_raw() {
    // A `Const::EnumValue` for a payload-bearing Direct variant in a multi-view
    // enum serializes tag + scalar payload to the `_raw` byte image (Change E):
    // `AB::B(42)` -- tag u8 `1` @0, payload u32 `42` @4, size 8 LE -> bytes
    // { 1, 0, 0, 0, 42, 0, 0, 0 }.
    let mut m = model();
    let e_ty = push_ab_enum(&mut m);
    let child = m.consts.intern(Const::Uint(UintKind::U32, 42));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![child],
    });
    let sig = fnsig(&mut m, vec![], e_ty, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "kb", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct AB){ .u._raw = { 1, 0, 0, 0, 42, 0, 0, 0 } }"),
        "payload-bearing multi-view EnumValue serializes to the _raw byte image:\n{tu}"
    );
}

#[test]
fn const_enum_value_inline_nested_niche_cell_serializes_to_raw() {
    // Change B (d2.3b, #4): an inline niche `Const::EnumValue` whose niche cell
    // is a sub-byte of a NESTED aggregate field now serializes uniformly to the
    // anonymous-union `_raw` byte image (the niche value 0 at offset 1; size 2)
    // -> `(struct OptT){ ._raw = { 0, 0 } }`. M2-a could not have written a
    // nested designator; `_raw` is just bytes.
    let mut m = model();
    let opt_t = push_opt_tuple_niche_enum(&mut m);
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let sig = fnsig(&mut m, vec![], opt_t, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![opt_t],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "knone_t", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct OptT){ ._raw = { 0, 0 } }"),
        "a nested-niche-cell const serializes the niche value into _raw:\n{tu}"
    );
}

#[test]
fn const_enum_value_inline_bool_niche_serializes_to_raw() {
    // Change B (d2.3b, #3): an inline niche `Const::EnumValue` whose niche cell
    // IS a `_Bool` field now serializes uniformly to the anonymous-union `_raw`
    // byte image. `Option<bool>::None` (niche value 2 @0, size 1) ->
    // `(struct OptBool){ ._raw = { 2 } }`. `_raw` is just bytes, so byte 2
    // round-trips where `(_Bool){2}` would have normalized to 1.
    let mut m = model();
    let opt = push_opt_bool_niche_enum(&mut m);
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let sig = fnsig(&mut m, vec![], opt, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![opt],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "knone_bool", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct OptBool){ ._raw = { 2 } }"),
        "inline bool-niche None serializes byte 2 into _raw (no normalization):\n{tu}"
    );
}

#[test]
fn no_task_d2_1_label_remains_in_body_emit() {
    // The d2.1 seams are filled: no `task d2.1` label may remain in the body
    // emitter source.
    let src = include_str!("body_emit.rs");
    assert!(
        !src.contains("task d2.1"),
        "no task d2.1 deferral label may remain in body_emit.rs"
    );
}

#[test]
fn no_task_d2_2_label_remains_in_backend() {
    // The d2.2 seams are filled (Changes A-F): no `task d2.2` label may remain
    // in body_emit.rs / mod.rs / enum_emit.rs. The deferred shapes carry
    // distinct `task d2.3` / vtable-task labels instead.
    for (name, src) in [
        ("body_emit.rs", include_str!("body_emit.rs")),
        ("mod.rs", include_str!("mod.rs")),
        ("enum_emit.rs", include_str!("enum_emit.rs")),
    ] {
        assert!(
            !src.contains("task d2.2"),
            "no task d2.2 deferral label may remain in {name}"
        );
    }
}

#[test]
fn enum_body_model_is_byte_identical_across_two_calls() {
    let build = || {
        let mut m = model();
        let u32t = uint(&mut m, UintKind::U32);
        let e_ty = push_shift_enum(&mut m);
        let sig = fnsig(&mut m, vec![u32t], e_ty, FnAbi::Rust, false);
        let body = build_body(
            1,
            vec![e_ty, u32t],
            vec![bb(
                vec![st(StatementKind::Assign(
                    loc(0),
                    Rvalue::Aggregate(
                        AggregateKind::Enum(e_ty, VariantIdx::from_u32(1)),
                        Box::new([Operand::Copy(loc(1))]),
                    ),
                ))],
                tk(TerminatorKind::Return),
            )],
        );
        fn_instance(&mut m, "mk_b", sig, Some(body));
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

// ---------------------------------------------------------------------------
// Task d2.2 (Changes A-F): non-scalar const lowering -- structural assertions
// + the d2.3 / vtable fail-loud carve-outs. Compile-and-run proof lives in the
// codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

/// Push a `const` (`static FOO`) instance of declared type `ty` with
/// initializer `init`, returning its `InstanceId` (so a carrier / reloc can
/// target it).
fn push_static(
    m: &mut RegseqModel,
    name: &str,
    ty: TyId,
    init: regseq_model::ConstId,
) -> InstanceId {
    let it = item(
        m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        name,
    );
    let em = m.strings.intern(name);
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: em,
        span: span(),
        kind: InstanceKind::Static {
            ty,
            init: Some(init),
        },
    })
}

/// Little-endian `Byte::Value` image of a `u32`.
fn le32(v: u32) -> Vec<regseq_model::Byte> {
    v.to_le_bytes()
        .iter()
        .map(|b| regseq_model::Byte::Value(*b))
        .collect()
}

#[test]
fn multi_view_enum_struct_carries_raw_union_member() {
    // Change A: every multi-view enum's `union u` carries a full-object
    // `unsigned char _raw[size]`; the size/align asserts are unchanged.
    let mut m = model();
    let _ = push_shift_enum(&mut m); // enum Shift, size 8 align 4.
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("unsigned char _raw[8];"),
        "the _raw byte-image union member:\n{tu}"
    );
    assert!(tu.contains("_Static_assert(sizeof(struct Shift) == 8, \"Shift size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Shift) == 4, \"Shift align\");"));
}

#[test]
fn str_reference_carrier_builds_rust_str_fat_struct() {
    // Change C: a `&str` carrier lowers to the rust_str fat struct with a
    // `const uint8_t *` data cast and a `size_t` length; the CMS stays
    // `uint8_t[N]` (1-byte data -- Change F leaves it unchanged).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let str_ty = m.types.intern(Ty::Str);
    let ref_str = m.types.intern(Ty::Ref(str_ty, Provenance::SharedBorrow));
    let cms_bytes = m.consts.intern(Const::Bytes(vec![
        regseq_model::Byte::Value(b'h'),
        regseq_model::Byte::Value(b'i'),
    ]));
    let arr = m.types.intern(Ty::Array(u8t, 2));
    let cms = push_static(&mut m, "CMS0", arr, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 2));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));
    push_static(&mut m, "S", ref_str, carrier);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct rust_str){ .data = (const uint8_t *)(&CMS0), .len = "),
        "rust_str fat carrier:\n{tu}"
    );
    assert!(
        tu.contains("const uint8_t CMS0[2] ="),
        "str CMS stays uint8_t[2]:\n{tu}"
    );
}

#[test]
fn slice_reference_carrier_uses_slice_data_ptr_type() {
    // Change C: a `&[u32]` carrier lowers to the fat struct with `.data` cast
    // via slice_data_ptr_type (`const uint32_t *`, not a hardcoded const); the
    // CMS is re-typed to `uint32_t[2]` (Change F).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let slice_u32 = m.types.intern(Ty::Slice(u32t));
    let ref_slice = m.types.intern(Ty::Ref(slice_u32, Provenance::SharedBorrow));
    let mut bytes = le32(1);
    bytes.extend(le32(2));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr = m.types.intern(Ty::Array(u8t, 8));
    let cms = push_static(&mut m, "CMS1", arr, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 2));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));
    push_static(&mut m, "SL", ref_slice, carrier);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(".data = (const uint32_t *)(&CMS1), .len = "),
        "slice carrier .data via slice_data_ptr_type:\n{tu}"
    );
    assert!(
        tu.contains("rust_slice"),
        "the fat slice struct surfaced:\n{tu}"
    );
    assert!(
        tu.contains("uint32_t CMS1[2] ="),
        "slice CMS re-typed to uint32_t[2]:\n{tu}"
    );
}

#[test]
fn const_memory_static_retyped_to_nonchar_array_pointee() {
    // Change F: a `&[u32; 4]` reference const re-types the CMS from the
    // bridge's `uint8_t[16]` to `uint32_t[4]` (same-type, aligned); the thin
    // reference is the reloc designator cast to the array pointer type.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    let mut bytes = le32(10);
    bytes.extend(le32(20));
    bytes.extend(le32(30));
    bytes.extend(le32(40));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms = push_static(&mut m, "CMS2", arr_u8, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));
    push_static(&mut m, "R", ref_arr, carrier);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMS2[4] = { ((uint32_t)(10U)), ((uint32_t)(20U)), ((uint32_t)(30U)), ((uint32_t)(40U)) };"),
        "non-char CMS re-typed to uint32_t[4]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMS2"),
        "CMS2 is not declared as a byte array:\n{tu}"
    );
    assert!(
        tu.contains("(*)[4])&CMS2)"),
        "the array reference is a pointer-to-array cast at the re-typed CMS:\n{tu}"
    );
}

// `enum EB { A(u8), B(&'static [u32; 4]) }` -- a multi-view `Direct` enum whose
// B variant holds a reference to an anonymous `[u32; 4]` (a bridge
// `ConstMemoryStatic`, typed `uint8_t[16]`). Returns `(e_ty, B's ref_arr ty,
// the CMS instance)`. Shared by the two d2.3b CMS-re-typing regression tests.
fn push_eb_reloc_enum(m: &mut RegseqModel) -> (TyId, InstanceId) {
    let u8t = uint(m, UintKind::U8);
    let u32t = uint(m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    let a0 = fl(m, "0", u8t, 8);
    let b0 = fl(m, "0", ref_arr, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(m, "A", vec![a0], 0, Some(0)),
            variant(m, "B", vec![b0], 1, Some(1)),
        ],
    };
    let e_ty = push_enum_instance(m, "EB", enum_layout(16, 8, shape));
    let mut bytes = le32(10);
    bytes.extend(le32(20));
    bytes.extend(le32(30));
    bytes.extend(le32(40));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms = push_static(m, "CMSE", arr_u8, cms_bytes);
    (e_ty, cms)
}

#[test]
fn const_enum_value_reloc_payload_retypes_the_cms() {
    // d2.3b soundness (8th catch), the `const` / `Const::EnumValue` form: a
    // multi-view `Direct` enum const whose payload is a reference to an anonymous
    // `ConstMemoryStatic` (`EB::B(&[10,20,30,40])`) lowers via the `_c<idx>` typed
    // view -> the emitted `(const uint32_t(*)[4])&CMSE` cast must read a CMS
    // re-typed to its pointee `uint32_t[4]`, NOT the bridge's `uint8_t[16]` (a
    // mismatched-type cast is strict-aliasing UB under the pinned -std=gnu11). The
    // CMS scan's `Const::EnumValue` arm recovers the pointee through the variant
    // field type.
    let mut m = model();
    let (e_ty, cms) = push_eb_reloc_enum(&mut m);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![carrier],
    });
    let sig = fnsig(&mut m, vec![], e_ty, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![e_ty],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Use(Operand::Constant(ev)),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "kb", sig, Some(body));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMSE[4]"),
        "enum-reloc const re-types the CMS to its pointee uint32_t[4]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMSE"),
        "CMSE is not left as a byte array (would be a strict-aliasing UB cast):\n{tu}"
    );
    assert!(
        tu.contains(".u._c1 = {"),
        "the reloc routes through the _c1 typed view:\n{tu}"
    );
}

#[test]
fn enum_reloc_static_bytes_retypes_the_cms() {
    // d2.3b soundness (8th catch), the `static` / `Const::Bytes` form: `static X:
    // EB = EB::B(&[..])` arrives as `Const::Bytes` with a `Byte::Reloc`;
    // `enum_from_bytes` lowers it via the `_c<idx>` typed view, so the CMS scan's
    // multi-view `Direct` branch (`scan_bytes`, selecting the active variant via
    // the shared `direct_enum_active_variant`) must re-type the CMS to
    // `uint32_t[4]` -- else the emitted cast is strict-aliasing UB.
    let mut m = model();
    let (e_ty, cms) = push_eb_reloc_enum(&mut m);
    // tag u8 `1` @0, then the &[u32;4] reloc (8-byte pointer) @8.
    let mut img = vec![regseq_model::Byte::Value(1)];
    img.extend(std::iter::repeat_n(regseq_model::Byte::Uninit, 7));
    img.extend(std::iter::repeat_n(
        regseq_model::Byte::Reloc(m.instance_ref_of(cms)),
        8,
    ));
    let xinit = m.consts.intern(Const::Bytes(img));
    push_static(&mut m, "XE", e_ty, xinit);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMSE[4]"),
        "enum-reloc static re-types the CMS to its pointee uint32_t[4]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMSE"),
        "CMSE is not left as a byte array (would be a strict-aliasing UB cast):\n{tu}"
    );
    assert!(
        tu.contains(".u._c1 = {"),
        "the static reloc routes through the _c1 typed view:\n{tu}"
    );
}

#[test]
fn multiview_niche_untagged_static_reloc_lowers_via_typed_view() {
    // d2.3b Fix #2 (workflow soundness catch): a MULTI-VIEW niche enum whose
    // UNTAGGED variant holds a relocation, in the `static` / `Const::Bytes` form,
    // was previously rejected unconditionally (NICHE_TAGGED_RELOC_MSG, even though
    // the reloc is in the *untagged* variant). It now routes through the existing
    // `u._v<untagged>` typed view (the bytes analog of the value-position M1-lite),
    // and the CMS scan's niche branch re-types the pointee. `NM { Niched(u8),
    // Untagged(&u32) }` -- 2 payload views (multi-view), niche-encoded, untagged
    // variant 1 holds the &u32 reloc.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let ref_u32 = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let v0f = fl(&mut m, "0", u8t, 0);
    let v1f = fl(&mut m, "0", ref_u32, 0);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_u32,
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
            variant(&mut m, "Niched", vec![v0f], 0, Some(2)),
            variant(&mut m, "Untagged", vec![v1f], 1, None),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "NM", enum_layout(8, 8, shape));
    // the anonymous &u32 allocation: a bridge CMS typed uint8_t[4].
    let cms_bytes = m.consts.intern(Const::Bytes(le32(7)));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 4));
    let cms = push_static(&mut m, "CMSN", arr_u8, cms_bytes);
    // static X: NM = NM::Untagged(&CMSN): an 8-byte pointer reloc @0.
    let img: Vec<regseq_model::Byte> =
        std::iter::repeat_n(regseq_model::Byte::Reloc(m.instance_ref_of(cms)), 8).collect();
    let xinit = m.consts.intern(Const::Bytes(img));
    push_static(&mut m, "XN", e_ty, xinit);
    // Previously `emit_generic_c` returned Err here; Fix #2 lowers it.
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(".u._v1._0 ="),
        "niche untagged static reloc routes via the u._v<untagged> typed view:\n{tu}"
    );
    assert!(
        tu.contains("uint32_t CMSN"),
        "the niche-untagged reloc re-types the CMS to its uint32_t pointee:\n{tu}"
    );
}

#[test]
fn const_memory_static_char_retyped_to_uint32_array() {
    // Change F: a `&[char]` reference const declares the CMS as `uint32_t[n]`
    // (4 bytes/elem, since Ty::Char renders as uint32_t), never `uint8_t[4n]`.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let char_ty = m.types.intern(Ty::Char);
    let slice_char = m.types.intern(Ty::Slice(char_ty));
    let ref_slice = m
        .types
        .intern(Ty::Ref(slice_char, Provenance::SharedBorrow));
    // Two chars 'A' (0x41) and 'B' (0x42), 4 bytes each, LE.
    let cms_bytes = m.consts.intern(Const::Bytes(vec![
        regseq_model::Byte::Value(0x41),
        regseq_model::Byte::Value(0),
        regseq_model::Byte::Value(0),
        regseq_model::Byte::Value(0),
        regseq_model::Byte::Value(0x42),
        regseq_model::Byte::Value(0),
        regseq_model::Byte::Value(0),
        regseq_model::Byte::Value(0),
    ]));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 8));
    let cms = push_static(&mut m, "CMS3", arr_u8, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 2));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));
    push_static(&mut m, "RC", ref_slice, carrier);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMS3[2] = { (uint32_t)65U, (uint32_t)66U };"),
        "Ty::Char CMS re-typed to uint32_t[2] (4 bytes/elem):\n{tu}"
    );
    assert!(
        !tu.contains("CMS3[8]"),
        "the char CMS is not a uint8_t[8] byte array:\n{tu}"
    );
}

#[test]
fn typed_valtree_struct_aggregate_const() {
    // Change D: a struct valtree `Const::Aggregate` pairs children with the
    // source-order fields, designating by the field C name.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    // struct Pt { a: u8 @4, b: u32 @0 } (repr(Rust) reordered); source order [a, b].
    let a = fl(&mut m, "a", u8t, 4);
    let b = fl(&mut m, "b", u32t, 0);
    let pt = intern_struct(&mut m, "Pt", struct_layout(8, 4, None, vec![a, b]));
    let ca = m.consts.intern(Const::Uint(UintKind::U8, 7));
    let cb = m.consts.intern(Const::Uint(UintKind::U32, 9));
    let agg = m.consts.intern(Const::Aggregate(vec![ca, cb]));
    register_struct_root(&mut m, pt, "Pt");
    push_static(&mut m, "P", pt, agg);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct Pt){ .a = ((uint8_t)(7U)), .b = ((uint32_t)(9U)) }"),
        "struct valtree aggregate designates source-order field names:\n{tu}"
    );
}

#[test]
fn typed_valtree_array_aggregate_const_is_brace_init_at_static() {
    // Change D: an array valtree `Const::Aggregate` at a static-init position
    // is a brace list (no compound-literal cast -- C forbids initializing an
    // array object from one).
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 3));
    let c0 = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let c1 = m.consts.intern(Const::Uint(UintKind::U32, 2));
    let c2 = m.consts.intern(Const::Uint(UintKind::U32, 3));
    let agg = m.consts.intern(Const::Aggregate(vec![c0, c1, c2]));
    push_static(&mut m, "ARR", arr, agg);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t ARR[3] = { ((uint32_t)(1U)), ((uint32_t)(2U)), ((uint32_t)(3U)) };"),
        "array valtree aggregate is a brace-init at a static position:\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_payload_serializes_to_raw() {
    // Change E: see also const_enum_value_payload_bearing_direct_multiview_uses_raw.
    // A multi-view bool-niche niched variant writes byte 2 directly into _raw
    // (no `_Bool` normalization), proving the multi-view bool-niche path.
    let mut m = model();
    let boolt = m.types.intern(Ty::Bool);
    let u32t = uint(&mut m, UintKind::U32);
    // Artificial 2-view niche enum: variant 0 "Some" (untagged) holds bool@0;
    // variant 1 "Big" carries a u32@4 view (forcing a 2nd view -> multi-view);
    // variant 1 is the niched variant (tag_value 2 in the bool niche cell @0).
    let some_b = fl(&mut m, "0", boolt, 0);
    let big_u = fl(&mut m, "0", u32t, 4);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: boolt,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(0),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(1),
                end: VariantIdx::from_u32(1),
            },
            niche_start: 2,
        },
        variants: vec![
            variant(&mut m, "Some", vec![some_b], 0, None),
            variant(&mut m, "Big", vec![big_u], 1, Some(2)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "NB", enum_layout(8, 4, shape));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![],
    });
    push_static(&mut m, "N", e_ty, ev);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("unsigned char _raw[8];"),
        "multi-view enum has a _raw member:\n{tu}"
    );
    assert!(
        tu.contains("(struct NB){ .u._raw = { 2, 0, 0, 0, 0, 0, 0, 0 } }"),
        "multi-view bool-niche writes raw byte 2 (not _Bool-normalized):\n{tu}"
    );
}

#[test]
fn bytes_over_union_initializes_the_raw_member() {
    // Change B (#1): a `Const::Bytes` over a union lowers to `(union U){ ._raw =
    // { bytes } }`, initializing the byte-covering `_raw` member; a member read
    // type-puns the bytes at runtime. The largest member (u32 @0) spans size 4.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let u8t = uint(&mut m, UintKind::U8);
    let x = fl(&mut m, "x", u32t, 0);
    let y = fl(&mut m, "y", u8t, 0);
    let uit = item(&mut m, ItemKind::Union, "U");
    let union_ty = m.types.intern(Ty::Union {
        item: uit,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![x, y] },
        },
    });
    let bytes = m.consts.intern(Const::Bytes(le32(7)));
    push_static(&mut m, "UB", union_ty, bytes);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(union U){ ._raw = { 7, 0, 0, 0 } }"),
        "union byte image initializes the _raw member:\n{tu}"
    );
}

#[test]
fn bytes_over_union_with_a_reloc_is_a_phase1_limitation() {
    // Change B (#2-reloc): a `Byte::Reloc` in a union byte image (a union
    // holding a reference) stays fail-loud -- a `char[]` _raw cannot hold an
    // address constant.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let p = fl(&mut m, "p", ref_ty, 0);
    let uit = item(&mut m, ItemKind::Union, "UR");
    let union_ty = m.types.intern(Ty::Union {
        item: uit,
        args: GenericArgs::new(),
        layout: Layout {
            size: 8,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![p] },
        },
    });
    // A target static + an 8-byte reloc image (the pointer slot).
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let tgt = push_static(&mut m, "T", u32t, one);
    let r = m.instance_ref_of(tgt);
    let bytes = m
        .consts
        .intern(Const::Bytes(vec![regseq_model::Byte::Reloc(r); 8]));
    push_static(&mut m, "UR", union_ty, bytes);
    let reason = unsupported_reason(&m);
    assert!(
        reason.contains("relocation byte in a union byte image")
            && reason.contains("char[] cannot hold an address constant"),
        "a union byte image with a reloc fails loud (a char[] cannot hold an address): {reason}"
    );
}

#[test]
fn typed_valtree_aggregate_over_union_is_d2_3() {
    // Change D: a `Const::Aggregate` carries no active FieldIdx, so a union
    // consumer cannot be lowered (matching pretty.rs) -> task d2.3.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let x = fl(&mut m, "x", u32t, 0);
    let uit = item(&mut m, ItemKind::Union, "U2");
    let union_ty = m.types.intern(Ty::Union {
        item: uit,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![x] },
        },
    });
    let child = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let agg = m.consts.intern(Const::Aggregate(vec![child]));
    push_static(&mut m, "UA", union_ty, agg);
    assert!(
        unsupported_reason(&m).contains("not expected (rustc emits no union valtree"),
        "typed valtree aggregate over a union is the #5 defensive guard"
    );
}

#[test]
fn const_memory_static_incompatible_pointee_types_is_d2_3() {
    // Change F: the content-dedup may merge two reference consts with identical
    // bytes onto one CMS; incompatible pointee types fail the whole emit loud.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let u64t = uint(&mut m, UintKind::U64);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let arr_u64 = m.types.intern(Ty::Array(u64t, 2));
    let ref_u32 = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    let ref_u64 = m.types.intern(Ty::Ref(arr_u64, Provenance::SharedBorrow));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms_bytes = m
        .consts
        .intern(Const::Bytes(vec![regseq_model::Byte::Value(0); 16]));
    let cms = push_static(&mut m, "CMSX", arr_u8, cms_bytes);
    let cms_ref = m.instance_ref_of(cms);
    let zero = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let reloc1 = m.consts.intern(Const::Reloc(cms_ref));
    let carrier1 = m.consts.intern(Const::Aggregate(vec![reloc1, zero]));
    let reloc2 = m.consts.intern(Const::Reloc(cms_ref));
    let carrier2 = m.consts.intern(Const::Aggregate(vec![reloc2, zero]));
    push_static(&mut m, "R1", ref_u32, carrier1);
    push_static(&mut m, "R2", ref_u64, carrier2);
    let reason = unsupported_reason(&m);
    assert!(
        reason.contains("incompatible pointee types")
            && reason.contains("bridge content-hash dedup"),
        "a CMS referenced at incompatible pointee types fails loud (#6): {reason}"
    );
}

#[test]
fn const_aggregate_carrier_to_dyn_is_vtable_task() {
    // Change C: a reference carrier to a `dyn` pointee is the vtable task.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let dref = dyn_ref(&mut m);
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let a = push_static(&mut m, "A", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(a)));
    let zero = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, zero]));
    push_static(&mut m, "D", dref, carrier);
    assert!(
        unsupported_reason(&m).contains("dyn pointee: vtable task"),
        "a dyn reference carrier is the vtable task"
    );
}

#[test]
fn multiview_enum_const_with_pointer_payload_uses_typed_view() {
    // Change C/D (d2.3b, #2-reloc EnumValue form): a multi-view Direct enum
    // const whose payload carries a relocation routes the tag + reloc through
    // the `_c<idx>` typed full view -- the reloc lands in a TYPED pointer slot
    // (`_raw`, a char[], cannot hold an address constant), one union member, no
    // double-init. enum E { A, B(&u32) }: tag u8 @0, B.0 (&u32) @8, size 16.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let b0 = fl(&mut m, "0", ref_ty, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "A", Vec::new(), 0, Some(0)),
            variant(&mut m, "B", vec![b0], 1, Some(1)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "EP", enum_layout(16, 8, shape));
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let a = push_static(&mut m, "A", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(a)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![reloc],
    });
    push_static(&mut m, "EV", e_ty, ev);
    let tu = emit_generic_c(&m).unwrap();
    // Only the payload-bearing variant B (index 1) gets a `_c1` view; the
    // fieldless A (index 0) gets none.
    assert!(
        tu.contains(
            "(struct EP){ .u._c1 = { .tag = ((uint8_t)(1U)), ._0 = ((const uint32_t *)&A) } }"
        ),
        "a Direct reloc const routes tag + reloc through the _c1 typed view:\n{tu}"
    );
    // The `_c1` typed full view is emitted at absolute offsets with its guards.
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct EP, u._c1.tag) == 0, \"EP.u._c1.tag offset\");"
        ),
        "the _c1 tag sub-field sits at offset 0:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct EP, u._c1._0) == 8, \"EP.u._c1._0 offset\");"),
        "the _c1 payload field sits at its absolute offset 8:\n{tu}"
    );
}

#[test]
fn nonscalar_const_emit_is_byte_identical_across_two_calls() {
    // B5 determinism: a model with a re-typed CMS, a multi-view enum const, a
    // fat carrier, and a typed valtree aggregate emits byte-identically.
    let build = || {
        let mut m = model();
        let u8t = uint(&mut m, UintKind::U8);
        let u32t = uint(&mut m, UintKind::U32);
        // re-typed CMS via a &[u32;2] reference const.
        let arr_u32 = m.types.intern(Ty::Array(u32t, 2));
        let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
        let mut bytes = le32(1);
        bytes.extend(le32(2));
        let cms_bytes = m.consts.intern(Const::Bytes(bytes));
        let arr_u8 = m.types.intern(Ty::Array(u8t, 8));
        let cms = push_static(&mut m, "CMS", arr_u8, cms_bytes);
        let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
        let zero = m.consts.intern(Const::Uint(UintKind::Usize, 0));
        let carrier = m.consts.intern(Const::Aggregate(vec![reloc, zero]));
        push_static(&mut m, "R", ref_arr, carrier);
        // multi-view enum const.
        let e_ty = push_ab_enum(&mut m);
        let child = m.consts.intern(Const::Uint(UintKind::U32, 42));
        let ev = m.consts.intern(Const::EnumValue {
            variant_idx: VariantIdx::from_u32(1),
            payload: vec![child],
        });
        push_static(&mut m, "EV", e_ty, ev);
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

// ---------------------------------------------------------------------------
// Task d2.3a (Changes A-E): the non-scalar-const carve-outs -- structural
// assertions + the rewritten fail-loud limitation/guard labels. Compile-and-run
// proof lives in the codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

/// `enum EArr { A, B([u32; 2]) }` -- a multi-view Direct enum whose payload
/// variant holds a 2-element u32 array (tag u8 @0, B.0 @4, size 12 align 4).
fn push_array_payload_enum(m: &mut RegseqModel) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let u32t = uint(m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 2));
    let b0 = fl(m, "0", arr, 4);
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
            variant(m, "B", vec![b0], 1, Some(1)),
        ],
    };
    push_enum_instance(m, "EArr", enum_layout(12, 4, shape))
}

/// A multi-view niche enum `enum NU { A(&u32), B(u32) }` where A is the
/// untagged variant (its `&u32` niche cell @0) and B is niched (`tag_value`
/// `Some(0)`, the null pattern, stored in the @0 niche cell) with its own
/// payload field `u32 @8` (non-overlapping). Both variants carry a non-ZST
/// payload, so the enum is multi-view (`_v0`, `_v1`) with NO `_tag` view; size
/// 16 align 8. Returns `(enum_ty, &u32 ref ty)`.
fn push_niche_multiview_enum(m: &mut RegseqModel) -> (TyId, TyId) {
    let u32t = uint(m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let a0 = fl(m, "0", ref_ty, 0);
    let b0 = fl(m, "0", u32t, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_ty,
        valid_range: WrappingRange { start: 1, end: 0 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(0),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(1),
                end: VariantIdx::from_u32(1),
            },
            niche_start: 0,
        },
        variants: vec![
            variant(m, "A", vec![a0], 0, None),
            variant(m, "B", vec![b0], 1, Some(0)),
        ],
    };
    let e_ty = push_enum_instance(m, "NU", enum_layout(16, 8, shape));
    (e_ty, ref_ty)
}

#[test]
fn const_enum_value_multiview_array_payload_serializes_into_raw() {
    // Change C: a multi-view Direct enum const `EArr::B([10, 20])` recurses the
    // 2-element u32 array payload at offset 4 (LE) into the `_raw` byte image:
    // tag u8 1 @0, [10, 20] @4 -> { 1,0,0,0, 10,0,0,0, 20,0,0,0 }.
    let mut m = model();
    let e_ty = push_array_payload_enum(&mut m);
    let ten = m.consts.intern(Const::Uint(UintKind::U32, 10));
    let twenty = m.consts.intern(Const::Uint(UintKind::U32, 20));
    let arr_const = m.consts.intern(Const::Aggregate(vec![ten, twenty]));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![arr_const],
    });
    push_static(&mut m, "EVA", e_ty, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct EArr){ .u._raw = { 1, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0 } }"),
        "array payload serializes into _raw at the right offsets:\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_nested_enum_payload_serializes_into_raw() {
    // Change C: a multi-view Direct enum const whose payload is itself a nested
    // enum value serializes the nested tag + field into `_raw`. Outer::W(AB::B(42)):
    // outer tag u8 1 @0; nested AB @4 -> nested tag u8 1 @4, payload u32 42 @8;
    // size 12 -> { 1,0,0,0, 1,0,0,0, 42,0,0,0 }.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let ab = push_ab_enum(&mut m); // AB { A, B(u32) }, size 8 align 4.
    let w0 = fl(&mut m, "0", ab, 4);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "V", Vec::new(), 0, Some(0)),
            variant(&mut m, "W", vec![w0], 1, Some(1)),
        ],
    };
    let outer = push_enum_instance(&mut m, "Outer", enum_layout(12, 4, shape));
    let fortytwo = m.consts.intern(Const::Uint(UintKind::U32, 42));
    let inner = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![fortytwo],
    });
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![inner],
    });
    push_static(&mut m, "OUT", outer, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct Outer){ .u._raw = { 1, 0, 0, 0, 1, 0, 0, 0, 42, 0, 0, 0 } }"),
        "nested enum payload serializes its tag + field into _raw:\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_niche_untagged_uses_typed_view() {
    // Change D (M1-lite): a multi-view niche enum's untagged variant routes its
    // const (incl. a relocation payload) through the existing `u._v0` typed view
    // -- one typed designator, NOT a `_raw` byte image, no double-init.
    let mut m = model();
    let (e_ty, _ref_ty) = push_niche_multiview_enum(&mut m);
    let u32t = uint(&mut m, UintKind::U32);
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let tgt = push_static(&mut m, "Y", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(tgt)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![reloc],
    });
    push_static(&mut m, "EVNU", e_ty, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(".u._v0._0 = "),
        "niche untagged routes through the typed view _v0._0:\n{tu}"
    );
    assert!(
        tu.contains("&Y"),
        "the reloc payload is an address constant:\n{tu}"
    );
    assert!(
        !tu.contains("(struct NU){ .u._raw"),
        "no _raw byte image for the niche-untagged route:\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_niched_tagged_still_uses_raw() {
    // Change D scope: only the niche UNTAGGED variant takes the typed-view
    // route; a niched-tagged variant (`tag_value` Some) still serializes a
    // no-reloc payload to `_raw`. NU::B(5): niche tag 0 @0 (8 bytes), payload u32
    // 5 @8 -> { 0,0,0,0,0,0,0,0, 5,0,0,0, 0,0,0,0 }.
    let mut m = model();
    let (e_ty, _ref_ty) = push_niche_multiview_enum(&mut m);
    let five = m.consts.intern(Const::Uint(UintKind::U32, 5));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![five],
    });
    push_static(&mut m, "EVNB", e_ty, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct NU){ .u._raw = { 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0 } }"),
        "a niched-tagged variant serializes to _raw:\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_niche_nonuntagged_tagless_fails_loud() {
    // d2.3a 7th-soundness-catch fix (Change D): M1-lite keys on the authoritative
    // `untagged_variant` certificate, NOT a `tag_value.is_none()` proxy. A
    // MALFORMED niche shape -- a non-untagged variant (index 1) carrying
    // `tag_value: None` (the bridge would assign `Some(niche)`; this shape is not
    // bridge-producible) -- must NOT slip into the typed-view route, which would
    // emit an in-range payload with no niche tag that the runtime niche read
    // mis-resolves to the untagged variant. It instead falls through to the
    // `_raw` serializer and fails loud.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let a0 = fl(&mut m, "0", ref_ty, 0);
    let b0 = fl(&mut m, "0", u32t, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_ty,
        valid_range: WrappingRange { start: 1, end: 0 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(0),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(1),
                end: VariantIdx::from_u32(1),
            },
            niche_start: 0,
        },
        // Variant 1 is a niched variant but MALFORMED: `tag_value: None`.
        variants: vec![
            variant(&mut m, "A", vec![a0], 0, None),
            variant(&mut m, "B", vec![b0], 1, None),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "NUM", enum_layout(16, 8, shape));
    let five = m.consts.intern(Const::Uint(UintKind::U32, 5));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![five],
    });
    push_static(&mut m, "EVNUM", e_ty, ev);
    assert!(
        unsupported_reason(&m).contains("tagless non-untagged variant"),
        "a malformed non-untagged tagless niche variant fails loud, not mis-routed via M1-lite"
    );
}

#[test]
fn multiview_direct_zst_field_reloc_payload_fails_loud() {
    // d2.3a soundness (defensive, Change C/D): a multi-view Direct variant whose
    // only payload field is ZST takes the fieldless tag-init shortcut, but a
    // RELOCATION wrongly attached to that ZST field (a certificate violation --
    // a relocation occupies a pointer slot, never a zero-sized field) must fail
    // loud, not be silently dropped. `enum E { A(()), B(u32) }` is multi-view
    // (the `_tag` + `B`'s `_v1` views); the const `E::A(&T)` attaches a reloc to
    // A's ZST `()` field.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let unit_ty = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let a0 = fl(&mut m, "0", unit_ty, 0);
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
    let e_ty = push_enum_instance(&mut m, "EZ", enum_layout(8, 4, shape));
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let t = push_static(&mut m, "T", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(t)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![reloc],
    });
    push_static(&mut m, "EVZ", e_ty, ev);
    let reason = unsupported_reason(&m);
    assert!(
        reason.contains("zero-sized enum payload field")
            && reason.contains("certificate violation"),
        "a reloc on a ZST payload field fails loud (not silently dropped): {reason}"
    );
}

#[test]
fn zero_size_union_emits_raw_and_bytes_lowers() {
    // Change A edge: a zero-size union (an all-ZST field) still emits a
    // byte-covering `_raw[0]` (a GNU zero-length array, valid under the pinned
    // gnu11 flags), and a `Const::Bytes(empty)` over it lowers via the `_raw`
    // member rather than failing loud (every emitted union carries `_raw`).
    let mut m = model();
    let unit_ty = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let a = fl(&mut m, "a", unit_ty, 0);
    let uit = item(&mut m, ItemKind::Union, "Z");
    let union_ty = m.types.intern(Ty::Union {
        item: uit,
        args: GenericArgs::new(),
        layout: Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![a] },
        },
    });
    let bytes = m.consts.intern(Const::Bytes(vec![]));
    push_static(&mut m, "ZU", union_ty, bytes);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("unsigned char _raw[0];"),
        "a zero-size union still carries a byte-covering _raw member:\n{tu}"
    );
    assert!(
        tu.contains("(union Z){ ._raw = {  } }"),
        "a zero-size union byte image lowers via _raw (not fail-loud):\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// Task d2.3b (Changes A-E): the inline anonymous-union `_raw` + the `_c<idx>`
// typed full view -- structural assertions + collision-proofing. The
// compile-AND-RUN closures (#2-reloc both forms, #3, both #4 cases incl. nested
// `_Bool`) live in the codegen-coverage-runner behavioral tests.
// ---------------------------------------------------------------------------

#[test]
fn inline_enum_raw_member_is_collision_proof() {
    // Change A collision-proofing: a single-view inline enum whose PROMOTED view
    // field is literally named `_raw` (a Rust variant field `_raw` sanitizes to
    // `_raw`) gets a COLLISION-PROOF byte-covering member `_raw0` -- the array
    // member cannot duplicate the promoted field (a `duplicate member` C error).
    // `enum_raw_member` returns `_raw0`, so the niche const designates `_raw0`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    // A struct-style `Some` field literally named `_raw` (an inline niche enum).
    let some_field = fl(&mut m, "_raw", ref_ty, 0);
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
    let opt = push_enum_instance(&mut m, "OptR", enum_layout(8, 8, shape));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    push_static(&mut m, "ON", opt, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("const uint32_t *_raw;"),
        "the real `_raw` view field is present (promoted):\n{tu}"
    );
    assert!(
        tu.contains("unsigned char _raw0[8];"),
        "the byte-covering member is collision-proofed to `_raw0`:\n{tu}"
    );
    assert!(
        tu.contains("(struct OptR){ ._raw0 = { 0, 0, 0, 0, 0, 0, 0, 0 } }"),
        "the niche const designates the collision-proof `_raw0`:\n{tu}"
    );
}

#[test]
fn multiview_direct_emits_c_views_per_payload_variant_at_absolute_offsets() {
    // Change C: a multi-view Direct enum emits a `_c<idx>` typed full view per
    // payload-bearing variant (`struct { <tag>; <fields>; }`) at ABSOLUTE
    // offsets, with `offsetof` guards. `enum Shift { A(u8), B(u32) }` (tag u8 @0,
    // A.0 @1, B.0 @4): both A and B are payload-bearing -> `_c0` and `_c1`.
    let mut m = model();
    let _ = push_shift_enum(&mut m);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("    } _c0;\n"),
        "variant A typed full view:\n{tu}"
    );
    assert!(
        tu.contains("    } _c1;\n"),
        "variant B typed full view:\n{tu}"
    );
    // _c0 = { tag u8 @0, A.0 u8 @1 }; _c1 = { tag u8 @0, B.0 u32 @4 }.
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct Shift, u._c0.tag) == 0, \"Shift.u._c0.tag offset\");"
        ),
        "the _c0 tag sub-field @0:\n{tu}"
    );
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct Shift, u._c0._0) == 1, \"Shift.u._c0._0 offset\");"
        ),
        "the _c0 payload field at its absolute offset 1:\n{tu}"
    );
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct Shift, u._c1._0) == 4, \"Shift.u._c1._0 offset\");"
        ),
        "the _c1 payload field at its absolute offset 4:\n{tu}"
    );
    // Layout-neutral: the runtime size / align asserts are unchanged.
    assert!(tu.contains("_Static_assert(sizeof(struct Shift) == 8, \"Shift size\");"));
    assert!(tu.contains("_Static_assert(_Alignof(struct Shift) == 4, \"Shift align\");"));
}

#[test]
fn c_view_tag_subfield_is_collision_proof() {
    // Change C collision-proofing: a multi-view Direct variant whose payload
    // field is literally named `tag` gets a COLLISION-PROOF `_c<idx>` tag
    // sub-field `tag0` (the synthetic tag cannot duplicate the payload field).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    // enum ET { A, B { tag: u32 } }: outer tag u8 @0, B.tag (u32) @4, size 8.
    let b_tag = fl(&mut m, "tag", u32t, 4);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "A", Vec::new(), 0, Some(0)),
            variant(&mut m, "B", vec![b_tag], 1, Some(1)),
        ],
    };
    push_enum_instance(&mut m, "ET", enum_layout(8, 4, shape));
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct ET, u._c1.tag0) == 0, \"ET.u._c1.tag0 offset\");"
        ),
        "the _c1 synthetic tag sub-field is collision-proofed to tag0 @0:\n{tu}"
    );
    assert!(
        tu.contains(
            "_Static_assert(offsetof(struct ET, u._c1.tag) == 4, \"ET.u._c1.tag offset\");"
        ),
        "the payload field named `tag` keeps its name at its offset 4:\n{tu}"
    );
}

#[test]
fn const_enum_value_inline_reloc_uses_typed_single_view() {
    // Change B (d2.3b): an inline RELOCATION const keeps the typed single view
    // (the single view IS the whole object; a reloc cannot enter `_raw`).
    // `Option<&u32>::Some(&Y)` -> `(struct Opt){ ._0 = ((const uint32_t *)&Y) }`.
    let mut m = model();
    let (opt, _r) = push_opt_ref_enum(&mut m);
    let u32t = uint(&mut m, UintKind::U32);
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let y = push_static(&mut m, "Y", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(y)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![reloc],
    });
    push_static(&mut m, "OS", opt, ev);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("(struct Opt){ ._0 = ((const uint32_t *)&Y) }"),
        "an inline reloc const uses the typed single view, not _raw:\n{tu}"
    );
    assert!(
        !tu.contains("(struct Opt){ ._raw"),
        "an inline reloc const must NOT serialize into _raw (a char[] cannot hold a reloc):\n{tu}"
    );
}

#[test]
fn const_enum_value_multiview_direct_reloc_via_bytes_uses_typed_view() {
    // Change D (d2.3b, #2-reloc Bytes form): a multi-view Direct enum `static`
    // arrives as `Const::Bytes` with a `Byte::Reloc` in the pointer slot; the
    // bytes path decodes the Direct tag slice (-> variant B), then routes the
    // reloc through the `_c1` typed view. enum E { A, B(&u32) }: tag u8 @0,
    // B.0 (&u32) @8, size 16. Bytes: tag 1 @0, reloc @8..16.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let b0 = fl(&mut m, "0", ref_ty, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "A", Vec::new(), 0, Some(0)),
            variant(&mut m, "B", vec![b0], 1, Some(1)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "EPB", enum_layout(16, 8, shape));
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let y = push_static(&mut m, "Y", u32t, one);
    let r = m.instance_ref_of(y);
    // tag 1 @0, 7 padding bytes, an 8-byte reloc run @8.
    let mut bytes = vec![regseq_model::Byte::Value(1)];
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Value(0), 7));
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Reloc(r), 8));
    let img = m.consts.intern(Const::Bytes(bytes));
    push_static(&mut m, "EVB", e_ty, img);
    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains(
            "(struct EPB){ .u._c1 = { .tag = ((uint8_t)(1U)), ._0 = ((const uint32_t *)&Y) } }"
        ),
        "the bytes-with-reloc form routes tag + reloc through the _c1 typed view:\n{tu}"
    );
}

#[test]
fn niche_tagged_reloc_is_an_external_invariant_defensive_guard() {
    // Change E / B4: a niche enum whose NICHED (non-untagged) variant carries a
    // relocation payload is an external-rustc-invariant non-case (rustc
    // niche-fills only fieldless variants). Hand-built (not bridge-producible),
    // it stays a fail-loud DEFENSIVE guard with the external-invariant message,
    // NOT a silent miscompile. `enum E { A(&u32), B(&u32) }` niche, untagged A,
    // niched B (tag_value Some(0)) carrying a reloc payload.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let a0 = fl(&mut m, "0", ref_ty, 0);
    let b0 = fl(&mut m, "0", ref_ty, 8);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_ty,
        valid_range: WrappingRange { start: 1, end: 0 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(0),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(1),
                end: VariantIdx::from_u32(1),
            },
            niche_start: 0,
        },
        // Variant 1 is niched (tag_value Some) but MALFORMED: it carries a
        // reloc payload (rustc would make it fieldless).
        variants: vec![
            variant(&mut m, "A", vec![a0], 0, None),
            variant(&mut m, "B", vec![b0], 1, Some(0)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "NTR", enum_layout(16, 8, shape));
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let y = push_static(&mut m, "Y", u32t, one);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(y)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![reloc],
    });
    push_static(&mut m, "NTRV", e_ty, ev);
    let reason = unsupported_reason(&m);
    assert!(
        reason.contains("niched (non-untagged) variant")
            && reason.contains("rustc niche-fills only fieldless variants"),
        "a niche-tagged reloc is the external-invariant defensive guard: {reason}"
    );
}

#[test]
fn multiview_reloc_byte_image_with_a_padding_reloc_fails_loud() {
    // d2.3b post-close soundness fix (Change D): a `Byte::Reloc` in a multi-view
    // Direct enum byte image that lands OUTSIDE every typed payload field slice
    // (here in the inter-field padding between the tag @0 and B.0 @8) is not
    // producible by rustc (a relocation occupies a pointer slot, never padding);
    // left unchecked, `reconstruct_multiview_reloc_from_bytes` would silently
    // drop it. It must fail loud instead. enum E { A(u8), B(&u32) } size 16.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let a0 = fl(&mut m, "0", u8t, 1);
    let b0 = fl(&mut m, "0", ref_ty, 8);
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
    let e_ty = push_enum_instance(&mut m, "EPP", enum_layout(16, 8, shape));
    let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
    let y = push_static(&mut m, "Y", u32t, one);
    let r = m.instance_ref_of(y);
    // tag 1 @0; a stray reloc at byte 1 (padding, NOT the B.0 pointer @8); the
    // pointer slot @8..16 is all-zero.
    let mut bytes = vec![regseq_model::Byte::Value(1), regseq_model::Byte::Reloc(r)];
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Value(0), 14));
    let img = m.consts.intern(Const::Bytes(bytes));
    push_static(&mut m, "EPPV", e_ty, img);
    let reason = unsupported_reason(&m);
    assert!(
        reason.contains("outside every typed field slice")
            && reason.contains("must land in a typed pointer slot"),
        "a reloc in enum padding fails loud, not silently dropped: {reason}"
    );
}

#[test]
fn no_task_d2_3b_limitation_label_remains_in_backend() {
    // d2.3b B4 / acceptance 6: the three Phase-1 const limitations (#2-reloc,
    // #3, #4) are now lowered, so no `task d2.3b` limitation label may remain in
    // the const seam (the residual niche-tagged-reloc reject names the external
    // rustc invariant instead).
    for (name, src) in [
        ("body_emit.rs", include_str!("body_emit.rs")),
        ("enum_emit.rs", include_str!("enum_emit.rs")),
    ] {
        assert!(
            !src.contains("task d2.3b"),
            "no task d2.3b limitation label may remain in {name}"
        );
    }
}

#[test]
fn d2_3b_const_emit_is_byte_identical_across_two_calls() {
    // B5 determinism: a model exercising the inline anonymous-union `_raw`
    // (incl. the collision-proof name), the `_c<idx>` reloc const, and an
    // inline niche const emits byte-identically across two calls.
    let build = || {
        let mut m = model();
        let u32t = uint(&mut m, UintKind::U32);
        let u8t = uint(&mut m, UintKind::U8);
        // inline bool-niche const -> `_raw`.
        let optb = push_opt_bool_niche_enum(&mut m);
        let none = m.consts.intern(Const::EnumValue {
            variant_idx: VariantIdx::from_u32(0),
            payload: vec![],
        });
        push_static(&mut m, "NB", optb, none);
        // multi-view Direct reloc const -> `_c1`.
        let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
        let b0 = fl(&mut m, "0", ref_ty, 8);
        let tag = TagScalar {
            offset: 0,
            scalar_ty: u8t,
            valid_range: WrappingRange { start: 0, end: 1 },
        };
        let shape = EnumShape::Multiple {
            tag,
            encoding: TagEncoding::Direct,
            variants: vec![
                variant(&mut m, "A", Vec::new(), 0, Some(0)),
                variant(&mut m, "B", vec![b0], 1, Some(1)),
            ],
        };
        let e_ty = push_enum_instance(&mut m, "EPR", enum_layout(16, 8, shape));
        let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
        let y = push_static(&mut m, "Y", u32t, one);
        let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(y)));
        let ev = m.consts.intern(Const::EnumValue {
            variant_idx: VariantIdx::from_u32(1),
            payload: vec![reloc],
        });
        push_static(&mut m, "EVR", e_ty, ev);
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

#[test]
fn no_bare_task_d2_3_label_remains_in_backend() {
    // d2.3a B4: every bare `task d2.3` deferral label is rewritten -- to a
    // Phase-1 limitation (`task d2.3b`), the bridge follow-up
    // (`task d2.3-bridge-dedup`), or a defensive guard. Only those qualified
    // forms may remain in the backend source.
    for (name, src) in [
        ("body_emit.rs", include_str!("body_emit.rs")),
        ("mod.rs", include_str!("mod.rs")),
    ] {
        let cleaned = src
            .replace("task d2.3b", "")
            .replace("task d2.3-bridge-dedup", "");
        assert!(
            !cleaned.contains("task d2.3"),
            "no bare task d2.3 deferral label may remain in {name}"
        );
    }
}

#[test]
fn d2_3a_const_emit_is_byte_identical_across_two_calls() {
    // B5 determinism: a model with a union static (_raw byte image), a
    // multi-view aggregate-payload enum const (serialized _raw), and a
    // niche-untagged reloc const (the typed view) emits byte-identically.
    let build = || {
        let mut m = model();
        // union static.
        let u32t = uint(&mut m, UintKind::U32);
        let u8t = uint(&mut m, UintKind::U8);
        let x = fl(&mut m, "x", u32t, 0);
        let y = fl(&mut m, "y", u8t, 0);
        let uit = item(&mut m, ItemKind::Union, "U");
        let union_ty = m.types.intern(Ty::Union {
            item: uit,
            args: GenericArgs::new(),
            layout: Layout {
                size: 4,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Union { fields: vec![x, y] },
            },
        });
        let ubytes = m.consts.intern(Const::Bytes(le32(7)));
        push_static(&mut m, "UB", union_ty, ubytes);
        // aggregate-payload multi-view enum const.
        let e_ty = push_array_payload_enum(&mut m);
        let ten = m.consts.intern(Const::Uint(UintKind::U32, 10));
        let twenty = m.consts.intern(Const::Uint(UintKind::U32, 20));
        let arr_const = m.consts.intern(Const::Aggregate(vec![ten, twenty]));
        let ev = m.consts.intern(Const::EnumValue {
            variant_idx: VariantIdx::from_u32(1),
            payload: vec![arr_const],
        });
        push_static(&mut m, "EVA", e_ty, ev);
        // niche-untagged reloc const.
        let (nu, _r) = push_niche_multiview_enum(&mut m);
        let one = m.consts.intern(Const::Uint(UintKind::U32, 1));
        let tgt = push_static(&mut m, "Y", u32t, one);
        let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(tgt)));
        let nev = m.consts.intern(Const::EnumValue {
            variant_idx: VariantIdx::from_u32(0),
            payload: vec![reloc],
        });
        push_static(&mut m, "EVNU", nu, nev);
        emit_generic_c(&m).unwrap()
    };
    assert_eq!(build(), build());
}

#[test]
fn const_memory_static_via_aggregate_operand_is_re_typed() {
    // Change F (pre-pass thoroughness): a CMS reference reached through a struct
    // `Rvalue::Aggregate` operand (not a bare `Use`) is still re-typed -- the
    // pre-pass threads the aggregate field consumer type.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 2));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    // struct Holder { p: &[u32; 2] } -- one pointer field @0.
    let p = fl(&mut m, "p", ref_arr, 0);
    let holder = intern_struct(&mut m, "Holder", struct_layout(8, 8, None, vec![p]));
    register_struct_root(&mut m, holder, "Holder");
    // The CMS the field points at.
    let mut bytes = le32(7);
    bytes.extend(le32(8));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 8));
    let cms = push_static(&mut m, "CMSA", arr_u8, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let zero = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, zero]));
    // A function builds `Holder { p: <carrier> }` via Rvalue::Aggregate.
    let sig = fnsig(&mut m, vec![], holder, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![holder],
        vec![bb(
            vec![st(StatementKind::Assign(
                loc(0),
                Rvalue::Aggregate(
                    AggregateKind::Struct(holder),
                    Box::new([Operand::Constant(carrier)]),
                ),
            ))],
            tk(TerminatorKind::Return),
        )],
    );
    fn_instance(&mut m, "mk_holder", sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMSA[2] ="),
        "CMS reached via an aggregate operand is re-typed to uint32_t[2]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMSA"),
        "the aggregate-operand CMS is not left as a byte array:\n{tu}"
    );
}

#[test]
fn const_memory_static_via_call_argument_is_re_typed_and_cast() {
    // Change F + the call-argument consumer-type threading: a `&[u32; 4]`
    // carrier passed as a Direct call argument is re-typed (pre-pass scans the
    // call arg at the param type) and the call site casts the reloc to the
    // parameter pointer type (emission threads the same param type).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    // The callee `consume(&[u32; 4])`.
    let consume_sig = fnsig(&mut m, vec![ref_arr], unit, FnAbi::Rust, false);
    let consume = fn_instance(&mut m, "consume", consume_sig, None);
    // The CMS the carrier points at.
    let mut bytes = le32(1);
    bytes.extend(le32(2));
    bytes.extend(le32(3));
    bytes.extend(le32(4));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms = push_static(&mut m, "CMSC", arr_u8, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let zero = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, zero]));
    // `regseq_main` calls `consume(<carrier>)`.
    let main_sig = fnsig(&mut m, vec![], unit, FnAbi::Rust, false);
    let body = build_body(
        0,
        vec![unit],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(consume),
                    args: Box::new([Operand::Constant(carrier)]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    fn_instance(&mut m, "caller", main_sig, Some(body));

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMSC[4] ="),
        "the call-argument CMS is re-typed to uint32_t[4]:\n{tu}"
    );
    assert!(
        tu.contains("(*)[4])&CMSC)"),
        "the call argument casts the reloc to the parameter pointer type:\n{tu}"
    );
}

#[test]
fn const_memory_static_via_inline_enum_byte_image_is_re_typed() {
    // Change F (pre-pass thoroughness, soundness): a CMS reference nested inside
    // an INLINE enum byte image (an `Option<&[u32; 4]>` niche static whose Some
    // pointer field is a reloc run to a CMS) is still re-typed -- the byte-image
    // scan descends the inline enum's payload-variant fields.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    // Option<&[u32;4]>: niche enum, Some(&[u32;4]) @0 (the pointer is the niche),
    // None niched (tag_value 0 = null), size 8 align 8, inline (one view).
    let some_field = fl(&mut m, "0", ref_arr, 0);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: ref_arr,
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
    let opt = push_enum_instance(&mut m, "OptArr", enum_layout(8, 8, shape));
    // The CMS the Some pointer field points at.
    let mut bytes = le32(10);
    bytes.extend(le32(20));
    bytes.extend(le32(30));
    bytes.extend(le32(40));
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms = push_static(&mut m, "CMSE", arr_u8, cms_bytes);
    // The enum static's byte image: an 8-byte pointer reloc run to the CMS.
    let reloc_byte = regseq_model::Byte::Reloc(m.instance_ref_of(cms));
    let enum_bytes = m.consts.intern(Const::Bytes(vec![reloc_byte; 8]));
    push_static(&mut m, "OE", opt, enum_bytes);

    let tu = emit_generic_c(&m).unwrap();
    assert!(
        tu.contains("uint32_t CMSE[4] ="),
        "CMS reached via an inline enum byte image is re-typed to uint32_t[4]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMSE"),
        "the inline-enum-referenced CMS is not left as a byte array:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// Section 8 diagnostic-logging -> printf: structural assertions (task e, B3).
// The format-template parser, the (Modifier, Ty) -> (specifier, cast) mapping,
// the C-string escaper, and the section-8 emission seam's defensive paths.
// Compile-AND-run proof of the observable output lives in the
// codegen-coverage-runner `generic_c_bodies.rs` behavioral tests.
// ---------------------------------------------------------------------------

use super::body_emit::{
    c_escape, diag_specifier_and_cast_for, level_tag, parse_placeholder, parse_template,
    require_default_modifier, Modifier, Segment,
};

#[test]
fn parse_template_default_placeholder_and_literals() {
    // `x={}` -> Literal("x=") then a default placeholder.
    let segs = parse_template(b"x={}").unwrap();
    assert_eq!(
        segs,
        vec![
            Segment::Literal(b"x=".to_vec()),
            Segment::Placeholder(Modifier::None),
        ]
    );
}

#[test]
fn parse_template_accepts_all_eight_bridge_hex_forms() {
    // The bridge's actual accepted set, incl. the uppercase-padded `:08X` /
    // `:#010X` beyond section 26.7's table, each into the {upper, alternate,
    // width} tuple.
    let cases: &[(&[u8], Modifier)] = &[
        (
            b"{:x}",
            Modifier::Hex {
                upper: false,
                alternate: false,
                width: 0,
            },
        ),
        (
            b"{:X}",
            Modifier::Hex {
                upper: true,
                alternate: false,
                width: 0,
            },
        ),
        (
            b"{:#x}",
            Modifier::Hex {
                upper: false,
                alternate: true,
                width: 0,
            },
        ),
        (
            b"{:#X}",
            Modifier::Hex {
                upper: true,
                alternate: true,
                width: 0,
            },
        ),
        (
            b"{:08x}",
            Modifier::Hex {
                upper: false,
                alternate: false,
                width: 8,
            },
        ),
        (
            b"{:08X}",
            Modifier::Hex {
                upper: true,
                alternate: false,
                width: 8,
            },
        ),
        (
            b"{:#010x}",
            Modifier::Hex {
                upper: false,
                alternate: true,
                width: 10,
            },
        ),
        (
            b"{:#010X}",
            Modifier::Hex {
                upper: true,
                alternate: true,
                width: 10,
            },
        ),
    ];
    for (src, want) in cases {
        let segs = parse_template(src).unwrap();
        assert_eq!(segs, vec![Segment::Placeholder(*want)], "form {src:?}");
    }
}

#[test]
fn parse_template_brace_escapes_and_embedded_newline() {
    // `{{`/`}}` -> literal `{`/`}`; an embedded `\n` is ordinary literal text.
    let segs = parse_template(b"{{x}}\n{}").unwrap();
    assert_eq!(
        segs,
        vec![
            Segment::Literal(b"{x}\n".to_vec()),
            Segment::Placeholder(Modifier::None),
        ]
    );
}

#[test]
fn parse_template_empty_yields_no_segments() {
    assert_eq!(parse_template(b"").unwrap(), Vec::<Segment>::new());
}

#[test]
fn parse_template_rejects_out_of_subset_and_malformed_braces() {
    // Positional / named / out-of-subset specs and malformed braces all fail
    // loud (the bridge already rejects these defensively).
    for bad in [
        &b"{0}"[..], // positional
        b"{name}",   // named
        b"{:?}",     // debug
        b"{:b}",     // binary
        b"{:5}",     // unsupported width
        b"{:04x}",   // unsupported pad width
        b"{",        // unclosed
        b"a}b",      // stray close
    ] {
        assert!(
            matches!(parse_template(bad), Err(EmitError::Unsupported(_))),
            "{bad:?} must be Unsupported"
        );
    }
}

#[test]
fn parse_placeholder_empty_is_default() {
    assert_eq!(parse_placeholder(b"").unwrap(), Modifier::None);
}

#[test]
fn specifier_default_arms_reuse_the_printf_widths_and_casts() {
    // The admissible numeric defaults: each (Ty) -> (conversion, cast).
    let cases: &[(Ty, &str, &str)] = &[
        (Ty::Bool, "%d", "(int)"),
        (Ty::Int(IntKind::I8), "%d", "(int)"),
        (Ty::Int(IntKind::I16), "%d", "(int)"),
        (Ty::Int(IntKind::I32), "%d", "(int)"),
        (Ty::Int(IntKind::I64), "%lld", "(long long)"),
        (Ty::Uint(UintKind::U8), "%u", "(unsigned)"),
        (Ty::Uint(UintKind::U16), "%u", "(unsigned)"),
        (Ty::Uint(UintKind::U32), "%u", "(unsigned)"),
        (Ty::Uint(UintKind::U64), "%llu", "(unsigned long long)"),
    ];
    for (ty, spec, cast) in cases {
        let got = diag_specifier_and_cast_for(Modifier::None, ty).unwrap();
        assert_eq!(got, (spec.to_string(), cast.to_string()), "default {ty:?}");
    }
}

#[test]
fn specifier_hex_inserts_ll_for_64bit_and_two_step_signed_cast() {
    // Signed hex uses the two-step (varargs-unsigned)(unsigned-of-source-width)
    // cast so a negative value renders its source-width two's-complement; the
    // `ll` length modifier is inserted for 64-bit operands.
    let m = Modifier::Hex {
        upper: false,
        alternate: false,
        width: 0,
    };
    assert_eq!(
        diag_specifier_and_cast_for(m, &Ty::Int(IntKind::I8)).unwrap(),
        ("%x".to_string(), "(unsigned)(uint8_t)".to_string())
    );
    assert_eq!(
        diag_specifier_and_cast_for(m, &Ty::Int(IntKind::I16)).unwrap(),
        ("%x".to_string(), "(unsigned)(uint16_t)".to_string())
    );
    assert_eq!(
        diag_specifier_and_cast_for(m, &Ty::Int(IntKind::I32)).unwrap(),
        ("%x".to_string(), "(unsigned)(uint32_t)".to_string())
    );
    assert_eq!(
        diag_specifier_and_cast_for(m, &Ty::Int(IntKind::I64)).unwrap(),
        ("%llx".to_string(), "(unsigned long long)".to_string())
    );
    assert_eq!(
        diag_specifier_and_cast_for(m, &Ty::Uint(UintKind::U32)).unwrap(),
        ("%x".to_string(), "(unsigned)".to_string())
    );
    // Padding + alternate + uppercase + 64-bit compose: `{:#010X}` on a u64.
    let padded = Modifier::Hex {
        upper: true,
        alternate: true,
        width: 10,
    };
    assert_eq!(
        diag_specifier_and_cast_for(padded, &Ty::Uint(UintKind::U64)).unwrap(),
        ("%#010llX".to_string(), "(unsigned long long)".to_string())
    );
}

#[test]
fn specifier_rejects_hex_on_non_integer_and_default_on_non_admissible() {
    let hex = Modifier::Hex {
        upper: false,
        alternate: false,
        width: 0,
    };
    // Hex on a bool is integer-only -> Unsupported.
    assert!(matches!(
        diag_specifier_and_cast_for(hex, &Ty::Bool),
        Err(EmitError::Unsupported(_))
    ));
    // A non-admissible Ty (Char / Isize / Usize) -> Unsupported even though
    // the broader `printf_spec` would accept all three.
    for ty in [Ty::Char, Ty::Int(IntKind::Isize), Ty::Uint(UintKind::Usize)] {
        assert!(
            matches!(
                diag_specifier_and_cast_for(Modifier::None, &ty),
                Err(EmitError::Unsupported(_))
            ),
            "default on {ty:?} must be Unsupported"
        );
        assert!(
            matches!(
                diag_specifier_and_cast_for(hex, &ty),
                Err(EmitError::Unsupported(_))
            ),
            "hex on {ty:?} must be Unsupported"
        );
    }
}

#[test]
fn require_default_modifier_rejects_hex_on_str() {
    assert!(require_default_modifier(Modifier::None).is_ok());
    assert!(matches!(
        require_default_modifier(Modifier::Hex {
            upper: false,
            alternate: false,
            width: 0
        }),
        Err(EmitError::Unsupported(_))
    ));
}

#[test]
fn c_escape_octal_escapes_nonprintable_and_does_not_double_percent() {
    // Literals are `fwrite`n raw (not a printf format), so a `%` is NOT doubled.
    assert_eq!(c_escape(b"50% done"), "50% done");
    // Quote / backslash / control escapes; a non-ASCII byte -> 3-digit octal.
    assert_eq!(c_escape(b"a\"b\\c\n\t\r"), "a\\\"b\\\\c\\n\\t\\r");
    assert_eq!(c_escape(&[0xff, b'z']), "\\377z");
    // An interior NUL renders `\000` (three C-literal chars, one decoded byte).
    assert_eq!(c_escape(&[b'a', 0, b'b']), "a\\000b");
}

#[test]
fn level_tag_maps_every_severity() {
    assert_eq!(level_tag(LogLevel::Error), "ERROR");
    assert_eq!(level_tag(LogLevel::Warn), "WARN");
    assert_eq!(level_tag(LogLevel::Info), "INFO");
    assert_eq!(level_tag(LogLevel::Debug), "DEBUG");
}

#[test]
fn no_task_e_label_remains_in_body_emit() {
    // The section-8 seam is filled; no `task e` deferral label may remain.
    let src = include_str!("body_emit.rs");
    assert!(
        !src.contains("task e"),
        "no task e deferral label may remain in body_emit.rs"
    );
}

/// A section-8 value argument for the [`emit_diag_call`] structural harness,
/// interpreted in the harness's own model so no cross-model handle leaks.
enum DiagArg {
    /// A `u32` numeric-constant operand (the `diag_numeric_ty` constant arm).
    ConstU32(u32),
    /// A non-static `&str` place operand (a fresh `&str` body local) -- the
    /// section-8.3 stack-buffer path.
    StrPlace,
    /// A `u32` numeric operand read from a body local whose `debug_name`
    /// renders to the given C identifier -- exercises the numeric scratch
    /// buffer's `fresh_temp_name` collision discipline (a local named `_n`
    /// must not be shadowed by the `snprintf` buffer).
    NumLocalNamed(&'static str),
}

/// Build a model whose `regseq_main` calls a `Log(Info)`-marked callee with a
/// `&str`-literal `template` as `args[0]`, the given `value_args`, and `target`,
/// then return `emit_generic_c`. The single structural harness for the
/// section-8 interception (incl. the defensive arity / `None`-target paths a
/// bridged model never reaches).
fn emit_diag_call(
    template: &str,
    value_args: Vec<DiagArg>,
    target: Option<BlockId>,
) -> Result<String, EmitError> {
    let mut m = model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let str_ty = m.types.intern(Ty::Str);
    let ref_str = m.types.intern(Ty::Ref(str_ty, Provenance::SharedBorrow));

    // The Log callee: excluded from emission; only its intrinsic tag matters.
    let log_sig = fnsig(&mut m, vec![], unit, FnAbi::Rust, false);
    let log_id = fn_instance(&mut m, "log_info", log_sig, None);
    let log_item = m.instances[log_id].item;
    while m.domain_attrs.item_attrs.len() <= log_item.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[log_item]
        .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));

    // args[0]: the &str-literal template carrier (Aggregate([Reloc->CMS, len])).
    let raw: Vec<regseq_model::Byte> = template.bytes().map(regseq_model::Byte::Value).collect();
    let n = raw.len() as u64;
    let cms_bytes = m.consts.intern(Const::Bytes(raw));
    let arr = m.types.intern(Ty::Array(u8t, n));
    let cms = push_static(&mut m, "TPL", arr, cms_bytes);
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, n as u128));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));

    let u32t = uint(&mut m, UintKind::U32);
    // Build the value operands in-model. A `StrPlace` / `NumLocalNamed` gets a
    // fresh body local; `loc(0)` is the unit return place, so the first arg
    // local is 1. `named` records `(local, debug_name)` to apply post-build.
    let mut args: Vec<Operand> = vec![Operand::Constant(carrier)];
    let mut locals = vec![unit];
    let mut named: Vec<(u32, &'static str)> = Vec::new();
    for varg in value_args {
        match varg {
            DiagArg::ConstU32(v) => {
                let cid = m.consts.intern(Const::Uint(UintKind::U32, v as u128));
                args.push(Operand::Constant(cid));
            }
            DiagArg::StrPlace => {
                let idx = locals.len() as u32;
                locals.push(ref_str);
                args.push(Operand::Copy(loc(idx)));
            }
            DiagArg::NumLocalNamed(name) => {
                let idx = locals.len() as u32;
                locals.push(u32t);
                named.push((idx, name));
                args.push(Operand::Copy(loc(idx)));
            }
        }
    }

    let main_sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    let mut body = build_body(
        0,
        locals,
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(log_id),
                    args: args.into_boxed_slice(),
                    destination: loc(0),
                    target,
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    for (idx, name) in named {
        let sid = m.strings.intern(name);
        body.local_decls[LocalId::from_u32(idx)].debug_name = Some(sid);
    }
    let main_id = fn_instance(&mut m, "regseq_main", main_sig, Some(body));
    let it = m.instances[main_id].item;
    m.items[it].linkage = Linkage::NoMangle;

    emit_generic_c(&m)
}

#[test]
fn diag_call_emits_tag_specifier_cast_and_goto() {
    // Happy path: a numeric-constant arg under `{}` -> the tag merged into the
    // leading literal `fwrite` (decoded count 9), one `snprintf`+`fwrite` block
    // (the `[24]` buffer, the `%u` specifier, the `(unsigned)` cast), the
    // trailing `\n` `fwrite`, and `goto bb_<target>` with no destination
    // assignment. Also exercises the `diag_numeric_ty` constant arm.
    let tu = emit_diag_call(
        "x={}",
        vec![DiagArg::ConstU32(7)],
        Some(BlockId::from_u32(1)),
    )
    .expect("happy-path diag call must emit");
    // The tag merges into the leading literal run; count is the decoded length.
    assert!(
        tu.contains("fwrite(\"[INFO] x=\", 1, 9, stdout);"),
        "merged tag + leading literal fwrite:\n{tu}"
    );
    // The numeric snprintf+fwrite block: [24] buffer, reused specifier + cast.
    // The scratch names come from `fresh_temp_name` -> `_n` / `_l` when no body
    // local takes those bases (see `diag_call_numeric_scratch_avoids_local_name`
    // for the collision case).
    assert!(
        tu.contains("char _n[24]; int _l = snprintf(_n, sizeof _n, \"%u\", (unsigned)("),
        "snprintf block + specifier + cast:\n{tu}"
    );
    assert!(
        tu.contains("fwrite(_n, 1, _l, stdout);"),
        "numeric fwrite of the returned length:\n{tu}"
    );
    // The trailing `\n` merges into the final literal fwrite.
    assert!(
        tu.contains("fwrite(\"\\n\", 1, 1, stdout);"),
        "trailing newline fwrite:\n{tu}"
    );
    assert!(tu.contains("goto bb_1;"), "goto target:\n{tu}");
    // No residual task-e printf format string / %s / stack buffer in the diag
    // path (the prelude's `fprintf`/`%s` panic text is unaffected).
    assert!(
        !tu.contains("[INFO] x=%u") && !tu.contains("_logstr"),
        "no residual printf format / stack-buffer in the diag path:\n{tu}"
    );
}

#[test]
fn diag_call_numeric_scratch_avoids_local_name() {
    // Regression (soundness, mirrors the `Repeat` loop-var test): the numeric
    // `snprintf` scratch buffer / length names come from `fresh_temp_name`, so
    // they cannot collide with a body-local C name. A body local whose
    // `debug_name` renders to `_n` (the scratch base) used as the numeric
    // operand would, under hardcoded `_n`/`_l`, be shadowed by the block's
    // `char _n[24]` -> `(unsigned)(_n)` would read the buffer pointer, not the
    // value (a silent miscompute the green fixtures could not catch).
    let tu = emit_diag_call(
        "v={}",
        vec![DiagArg::NumLocalNamed("_n")],
        Some(BlockId::from_u32(1)),
    )
    .expect("named-local numeric diag call must emit");
    // The operand local renders as `_n`; the scratch buffer must be bumped off
    // it (`fresh_temp_name` -> `_n_1`), never declaring `char _n[`.
    assert!(
        !tu.contains("char _n[24]"),
        "the snprintf buffer must not shadow the `_n` operand local:\n{tu}"
    );
    assert!(
        tu.contains("char _n_1[24]; int _l = snprintf(_n_1, sizeof _n_1, \"%u\", (unsigned)(_n));"),
        "bumped scratch buffer; the `_n` operand read un-shadowed:\n{tu}"
    );
    assert!(
        tu.contains("fwrite(_n_1, 1, _l, stdout);"),
        "fwrite of the bumped buffer + length:\n{tu}"
    );
}

#[test]
fn diag_call_non_static_str_args_emit_value_fwrites() {
    // Two non-static `&str` place args in one call -> two value `fwrite`s of
    // the section-3.2 fat pointer's full byte range (`(op).data` / `(op).len`),
    // with no stack buffer, no cap, no `%s`. The literal runs between/around
    // them flush at their decoded byte counts.
    let tu = emit_diag_call(
        "a={} b={}",
        vec![DiagArg::StrPlace, DiagArg::StrPlace],
        Some(BlockId::from_u32(1)),
    )
    .expect("non-static str diag call must emit");
    // The leading + middle literal runs, decoded counts.
    assert!(
        tu.contains("fwrite(\"[INFO] a=\", 1, 9, stdout);"),
        "leading literal fwrite:\n{tu}"
    );
    assert!(
        tu.contains("fwrite(\" b=\", 1, 3, stdout);"),
        "middle literal fwrite:\n{tu}"
    );
    // Two value fwrites of `.data`/`.len` (exactly `.len` bytes, no over-read).
    assert_eq!(
        tu.matches(").data, 1, (").count(),
        2,
        "two non-static &str value fwrites of `.data`/`.len`:\n{tu}"
    );
    assert_eq!(
        tu.matches(").len, stdout);").count(),
        2,
        "each `&str` fwrite writes exactly `.len` bytes:\n{tu}"
    );
    // No residual stack buffer / cap / `%s` from the task-e path.
    assert!(
        !tu.contains("_logstr") && !tu.contains("[256]") && !tu.contains("=%s"),
        "no residual buffer / cap / %s in the diag path:\n{tu}"
    );
}

#[test]
fn diag_call_literal_fwrite_count_is_decoded_byte_length() {
    // The literal-run fwrite COUNT is the decoded byte length, NOT the escaped
    // C-literal length: an interior `\0` (rendered `\000`) counts as one byte
    // and an embedded `\n` counts as one byte. Template "a\0b\nc={}" -> the
    // leading run "[INFO] a\0b\nc=" is 7 + 6 = 13 decoded bytes.
    let tu = emit_diag_call(
        "a\u{0}b\nc={}",
        vec![DiagArg::ConstU32(7)],
        Some(BlockId::from_u32(1)),
    )
    .expect("interior-NUL/newline template must emit");
    assert!(
        tu.contains("fwrite(\"[INFO] a\\000b\\nc=\", 1, 13, stdout);"),
        "decoded byte count (interior NUL = 1, embedded \\n = 1):\n{tu}"
    );
}

#[test]
fn diag_call_zero_placeholder_template_merges_into_one_literal_fwrite() {
    // No placeholders -> the tag + literal + trailing `\n` merge into a single
    // literal `fwrite` whose count is the decoded byte length (17, not 16).
    let tu = emit_diag_call("connected", vec![], Some(BlockId::from_u32(1)))
        .expect("zero-placeholder template must emit");
    assert!(
        tu.contains("fwrite(\"[INFO] connected\\n\", 1, 17, stdout);"),
        "single merged literal fwrite:\n{tu}"
    );
    assert!(tu.contains("goto bb_1;"), "goto target:\n{tu}");
}

#[test]
fn diag_call_arity_mismatch_fails_loud() {
    // One placeholder, zero value operands -> the I-DiagLog-CallArity check.
    assert!(matches!(
        emit_diag_call("{}", vec![], Some(BlockId::from_u32(1))),
        Err(EmitError::Unsupported(msg)) if msg.contains("arity")
    ));
}

#[test]
fn diag_call_none_target_fails_loud() {
    // A Log callee always returns unit; a `None` target is model corruption.
    assert!(matches!(
        emit_diag_call("", vec![], None),
        Err(EmitError::Unsupported(msg)) if msg.contains("no target")
    ));
}

// ---------------------------------------------------------------------------
// regseq-backends-27: entry-aware emit API + return-shape classifier (T07/T01).
// The classifier (`classify_entry_return`) + shim resolution (`resolve_entry_shim`)
// are exercised directly over hand-built models; `emit_generic_c_for_entry` is
// exercised for the byte-identical unit-void path; `generate_main_c` for both
// shim shapes.
// ---------------------------------------------------------------------------

/// Rust unit `()`.
fn unit_ty(m: &mut RegseqModel) -> TyId {
    m.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    })
}

/// Mark an item's source with the `FrameworkType(RegseqError)` attr (the bridge
/// records this in regseq-driver-89); grows `item_attrs` to reach `item`.
fn mark_framework_error(m: &mut RegseqModel, it: regseq_model::ItemId) {
    let need = it.as_u32() as usize + 1;
    while m.domain_attrs.item_attrs.len() < need {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[it].push(AttrData::FrameworkType(
        regseq_model::FrameworkType::RegseqError,
    ));
}

/// A fieldless multi-variant enum like `RegseqError`; `marked` toggles the
/// `FrameworkType` attr on its item.
fn error_enum(m: &mut RegseqModel, rendered: &str, marked: bool) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let it = item(m, ItemKind::Enum, rendered);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 8 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(m, "InvalidParameter", Vec::new(), 0, Some(0)),
            variant(m, "OutOfMemory", Vec::new(), 1, Some(1)),
        ],
    };
    let ty = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: enum_layout(1, 1, shape),
    });
    if marked {
        mark_framework_error(m, it);
    }
    ty
}

/// Set an item's compiler-certified `Result` identity flag (regseq-driver-91
/// records this from rustc's `Result` diagnostic item).
fn mark_result(m: &mut RegseqModel, it: regseq_model::ItemId) {
    m.items[it].is_result = true;
}

/// A niche-encoded two-variant `Result`-SHAPED enum: variant 0 (niched) carries
/// `ok_field_ty`, variant 1 (untagged) carries `err_field_ty`, mirroring rustc's
/// variant order. `rendered` is the item's display path, `ok_name`/`err_name`
/// the variant names, and `is_result` toggles the certified `Item.is_result`
/// identity flag. Used by the negatives to mint look-alikes that render or shape
/// like `core::result::Result` without its certified identity.
#[allow(clippy::too_many_arguments)]
fn result_shaped_enum(
    m: &mut RegseqModel,
    rendered: &str,
    ok_name: &str,
    err_name: &str,
    ok_field_ty: TyId,
    err_field_ty: TyId,
    is_result: bool,
) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let it = item(m, ItemKind::Enum, rendered);
    if is_result {
        mark_result(m, it);
    }
    let ok_field = fl(m, "0", ok_field_ty, 0);
    let err_field = fl(m, "0", err_field_ty, 0);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(1),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(0),
                end: VariantIdx::from_u32(0),
            },
            niche_start: 9,
        },
        variants: vec![
            variant(m, ok_name, vec![ok_field], 0, Some(9)),
            variant(m, err_name, vec![err_field], 1, None),
        ],
    };
    m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: enum_layout(1, 1, shape),
    })
}

/// A genuine `core::result::Result<ok_field, err_field>`: certified
/// `is_result == true`, variants named `Ok`/`Err`, Ok = variant 0 (niched),
/// Err = variant 1 (untagged, carrying the niche-bearing payload).
fn result_enum(m: &mut RegseqModel, ok_field_ty: TyId, err_field_ty: TyId) -> TyId {
    result_shaped_enum(
        m,
        "core::result::Result",
        "Ok",
        "Err",
        ok_field_ty,
        err_field_ty,
        true,
    )
}

#[test]
fn classify_unit_return_is_unit_void() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    assert_eq!(classify_entry_return(&m, unit), EntryReturnShape::UnitVoid);
}

#[test]
fn classify_result_unit_marked_err_is_err_result() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, unit, err);
    assert_eq!(
        classify_entry_return(&m, result),
        EntryReturnShape::ErrResult { err_enum_ty: err }
    );
}

#[test]
fn classify_result_unmarked_err_enum_rejects_naming_the_shape() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::PlainErr", false);
    let result = result_enum(&mut m, unit, err);
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("not marked") && reason.contains("regseq-error"),
            "reject names the unmarked-enum shape: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_non_unit_ok_rejects_naming_the_shape() {
    // Result<u32, RegseqError>: a non-unit Ok payload.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, u32t, err);
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("non-unit Ok"),
            "reject names the non-unit-Ok shape: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_result_non_enum_err_rejects() {
    // Result<(), u32>: the Err payload is not an enum at all.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let u32t = uint(&mut m, UintKind::U32);
    let result = result_enum(&mut m, unit, u32t);
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("not a regseq-error-marked enum"),
            "reject names the non-enum-Err shape: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_non_unit_zst_ok_is_still_rejected() {
    // Result<[u8; 0], RegseqError>: the Ok payload is zero-sized but is NOT
    // `()`, so it must still be rejected (the check is exact-unit, not any-ZST).
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let zst_arr = m.types.intern(Ty::Array(u8t, 0));
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, zst_arr, err);
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("non-unit Ok"),
            "a non-`()` ZST Ok payload is still non-unit: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_multi_field_err_is_rejected() {
    // A two-variant niche enum whose Err variant carries TWO fields is not the
    // `Result<(), E>` shape, even if one field is a marked enum.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err_enum = error_enum(&mut m, "crate::RegseqError", true);
    let u8t = uint(&mut m, UintKind::U8);
    let it = item(&mut m, ItemKind::Enum, "core::result::Result");
    // A genuine, certified Result so the classifier reaches the Err-payload
    // structural check (a multi-field Err) rather than the identity gate.
    mark_result(&mut m, it);
    let ok_field = fl(&mut m, "0", unit, 0);
    let err_field0 = fl(&mut m, "0", err_enum, 0);
    let err_field1 = fl(&mut m, "1", u8t, 1);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Niche {
            untagged_variant: VariantIdx::from_u32(1),
            niche_variants: VariantRange {
                start: VariantIdx::from_u32(0),
                end: VariantIdx::from_u32(0),
            },
            niche_start: 9,
        },
        variants: vec![
            variant(&mut m, "Ok", vec![ok_field], 0, Some(9)),
            variant(&mut m, "Err", vec![err_field0, err_field1], 1, None),
        ],
    };
    let result = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: enum_layout(2, 1, shape),
    });
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("single regseq-error-marked enum"),
            "a multi-field Err is rejected: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_scalar_return_rejects_naming_the_expected_shapes() {
    // A bare `u32` return is neither `()` nor `Result<(), _>`.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    match classify_entry_return(&m, u32t) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("Result<(), RegseqError>"),
            "reject names the expected shapes: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// regseq-backends-34: certified `core::result::Result` identity (D08/C03/R09).
// The classifier now gates on the compiler-certified `Item.is_result` flag
// (regseq-driver-91), NOT a rendered def-path string, with an `Ok`/`Err`
// variant-name guard so the `Ok = variants[0]` / `Err = variants[1]` adapter
// binding is sound by construction.
// ---------------------------------------------------------------------------

#[test]
fn classify_result_look_alike_without_is_result_is_rejected() {
    // Test Plan 1 (the key case): an item that *renders* as `core::result::Result`
    // with variants `Ok`/`Err`, a unit Ok payload, and a marked-enum Err payload --
    // structurally identical to a genuine Result -- but with `is_result == false`.
    // Identity is certified by the compiler flag, not the display string, so this
    // look-alike must be rejected (D08/C03/R09).
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let look_alike = result_shaped_enum(
        &mut m,
        "core::result::Result",
        "Ok",
        "Err",
        unit,
        err,
        false,
    );
    match classify_entry_return(&m, look_alike) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("is_result == false") && reason.contains("core::result::Result"),
            "reject names the failed identity certification, not the display string: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_custom_result_carrier_is_rejected() {
    // Test Plan 2: a custom carrier shaped like a niche `Result` but with
    // `is_result == false` (`enum MyResult { Good, Bad(RegseqError) }`) is never the
    // certified `core::result::Result` and must be rejected (D08).
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let carrier = result_shaped_enum(&mut m, "crate::MyResult", "Good", "Bad", unit, err, false);
    match classify_entry_return(&m, carrier) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("is_result == false"),
            "the custom carrier fails the identity certification: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_certified_result_with_wrong_variant_names_is_rejected() {
    // Test Plan 3 (defensive): a certified Result (`is_result == true`) whose
    // variants are NOT named `Ok`/`Err` is rejected, keeping the `variants[0]` = Ok
    // / `variants[1]` = Err adapter binding sound by construction.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_shaped_enum(&mut m, "core::result::Result", "Yes", "No", unit, err, true);
    match classify_entry_return(&m, result) {
        EntryReturnShape::Reject(reason) => assert!(
            reason.contains("not named") && reason.contains("Ok") && reason.contains("Err"),
            "reject names the wrong-variant-name shape: {reason}"
        ),
        other => panic!("expected Reject, got {other:?}"),
    }
}

#[test]
fn classify_certified_result_unit_marked_err_is_err_result() {
    // Test Plan 4 (positive regression): a genuine `Result<(), RegseqError>` --
    // certified `is_result == true`, variants `Ok`/`Err`, unit Ok, marked-enum Err
    // -- is the ErrResult path; `()` stays the unit-void path.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, unit, err);
    assert_eq!(
        classify_entry_return(&m, result),
        EntryReturnShape::ErrResult { err_enum_ty: err }
    );
    assert_eq!(classify_entry_return(&m, unit), EntryReturnShape::UnitVoid);
}

#[test]
fn resolve_shim_unit_void_returns_void_shim_with_sanitized_symbol() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    assert_eq!(
        resolve_entry_shim(&m, "regseq_main").unwrap(),
        EntryShim::UnitVoid {
            entry: "regseq_main".to_string()
        }
    );
}

#[test]
fn resolve_shim_default_entry_err_result_returns_adapter_symbol() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    assert_eq!(
        resolve_entry_shim(&m, "regseq_main").unwrap(),
        EntryShim::ErrResult {
            adapter: "regseq_main__regseq_exit".to_string()
        }
    );
}

#[test]
fn resolve_shim_absent_entry_fails_loud() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    fn_instance(&mut m, "other_fn", sig, None);
    match resolve_entry_shim(&m, "regseq_main") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("no function instance") && msg.contains("regseq_main"),
            "names the missing entry: {msg}"
        ),
        other => panic!("expected EntryShape error, got {other:?}"),
    }
}

#[test]
fn resolve_shim_ambiguous_entry_fails_loud() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    fn_instance(&mut m, "a_helper", sig, None);
    fn_instance(&mut m, "b_helper", sig, None);
    match resolve_entry_shim(&m, "helper") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("ambiguous") && msg.contains("2"),
            "names the ambiguous matches: {msg}"
        ),
        other => panic!("expected EntryShape error, got {other:?}"),
    }
}

#[test]
fn resolve_shim_entry_with_arguments_fails_loud() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], unit, FnAbi::C, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    match resolve_entry_shim(&m, "regseq_main") {
        Err(EmitError::EntryShape(msg)) => {
            assert!(msg.contains("argument"), "names the arguments shape: {msg}")
        }
        other => panic!("expected EntryShape error, got {other:?}"),
    }
}

#[test]
fn resolve_shim_custom_entry_with_error_return_is_rejected_d09() {
    // A manifest-custom entry (non-default suffix) returning an error is deferred
    // this phase (D09).
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "my_custom_entry", sig, None);
    match resolve_entry_shim(&m, "my_custom_entry") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("manifest-custom entry") && msg.contains("error-returning"),
            "names the custom-entry+error shape: {msg}"
        ),
        other => panic!("expected EntryShape error, got {other:?}"),
    }
}

#[test]
fn resolve_shim_custom_entry_unit_void_is_allowed() {
    // A custom suffix returning unit is fine (only error-returning custom entries
    // are rejected, D09).
    let mut m = model();
    let unit = unit_ty(&mut m);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    fn_instance(&mut m, "my_custom_entry", sig, None);
    assert_eq!(
        resolve_entry_shim(&m, "my_custom_entry").unwrap(),
        EntryShim::UnitVoid {
            entry: "my_custom_entry".to_string()
        }
    );
}

#[test]
fn emit_for_entry_unit_void_tu_is_identical_to_emit_generic_c() {
    let mut m = model();
    let unit = unit_ty(&mut m);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C, false);
    fn_instance(&mut m, "regseq_main", sig, None);

    let emit = emit_generic_c_for_entry(&m, "regseq_main").expect("unit-void entry must emit");
    assert_eq!(
        emit.shim,
        EntryShim::UnitVoid {
            entry: "regseq_main".to_string()
        }
    );
    // I02/R05: the unit-void TU is byte-identical to the plain emit.
    assert_eq!(emit.tu, emit_generic_c(&m).unwrap());
}

#[test]
fn emit_for_entry_propagates_reject_as_a_loud_emit_error() {
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![], u32t, FnAbi::C, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    assert!(matches!(
        emit_generic_c_for_entry(&m, "regseq_main"),
        Err(EmitError::EntryShape(_))
    ));
}

#[test]
fn generate_main_c_unit_void_is_byte_identical_to_the_historical_void_shim() {
    // I02/R05: the void shim bytes must not change. This pins the exact string
    // the prior `generate_main_c(entry: &str)` produced.
    let expected = "/* Generated by codegen-coverage-runner (Doc 2 D3 harness entry). */\n\
                    extern void regseq_main(void);\n\
                    \n\
                    int main(void) {\n\
                    \x20\x20\x20\x20regseq_main();\n\
                    \x20\x20\x20\x20return 0;\n\
                    }\n";
    assert_eq!(
        generate_main_c(&EntryShim::unit_void("regseq_main")),
        expected
    );
}

#[test]
fn generate_main_c_err_result_emits_int_shim_calling_the_adapter() {
    let main_c = generate_main_c(&EntryShim::ErrResult {
        adapter: "regseq_main__regseq_exit".to_string(),
    });
    assert!(main_c.contains("extern int regseq_main__regseq_exit(void);"));
    assert!(main_c.contains("return regseq_main__regseq_exit();"));
    assert!(main_c.contains("int main(void)"));
    assert!(
        !main_c.contains("return 0;"),
        "the int shim returns the adapter, not 0"
    );
}

#[test]
fn entry_shim_unit_void_constructor_matches_struct_form() {
    assert_eq!(
        EntryShim::unit_void("foo"),
        EntryShim::UnitVoid {
            entry: "foo".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// regseq-backends-28: the hand-emitted RegseqError -> errno boundary adapter +
// symbolic switch + demand-driven <errno.h> (PLAN T06/T08 + the adapter-switch
// half of T10). The adapter is emitted into the TU by `emit_generic_c_for_entry`
// when the entry classifies as `ErrResult`; the unit-void path stays unchanged.
// ---------------------------------------------------------------------------

/// The nine C02 rows as `(VariantLayout.name, tag_value, errno symbol)`. The tag
/// value doubles as the variant discriminant (a `#[repr]`-default fieldless enum
/// numbers its variants 0..). `GenericFailure` / `HardwareError` deliberately
/// share `EIO` (D04).
const C02_ROWS: &[(&str, u128, &str)] = &[
    ("InvalidParameter", 0, "EINVAL"),
    ("OutOfMemory", 1, "ENOMEM"),
    ("HardwareError", 2, "EIO"),
    ("Timeout", 3, "ETIMEDOUT"),
    ("Unsupported", 4, "EOPNOTSUPP"),
    ("DeviceBusy", 5, "EBUSY"),
    ("PermissionDenied", 6, "EPERM"),
    ("DeviceNotFound", 7, "ENODEV"),
    ("GenericFailure", 8, "EIO"),
];

/// A full nine-variant `RegseqError` (the C02 table), a fieldless Direct enum
/// (the `input_Color` shape). `marked` toggles the `FrameworkType` attr so the
/// entry classifier recognizes it.
fn regseq_error_enum_full(m: &mut RegseqModel, rendered: &str, marked: bool) -> TyId {
    let u8t = uint(m, UintKind::U8);
    let it = item(m, ItemKind::Enum, rendered);
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 8 },
    };
    let mut variants = Vec::new();
    for (idx, (name, tv, _sym)) in C02_ROWS.iter().enumerate() {
        variants.push(variant(m, name, Vec::new(), idx as u128, Some(*tv)));
    }
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants,
    };
    let ty = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: enum_layout(1, 1, shape),
    });
    if marked {
        mark_framework_error(m, it);
    }
    ty
}

/// Build a model whose `regseq_main` entry returns `Result<(), RegseqError>` for
/// a `RegseqError` carrying `variants` (each `(name, tag_value)`), `marked` per
/// the framework attr. Returns the emitted TU (the adapter is appended).
fn emit_err_result_entry(m: &mut RegseqModel, err_ty: TyId) -> String {
    let unit = unit_ty(m);
    let result = result_enum(m, unit, err_ty);
    let sig = fnsig(m, vec![], result, FnAbi::Rust, false);
    fn_instance(m, "regseq_main", sig, None);
    let emit = emit_generic_c_for_entry(m, "regseq_main").expect("ErrResult entry must emit");
    assert_eq!(
        emit.shim,
        EntryShim::ErrResult {
            adapter: "regseq_main__regseq_exit".to_string()
        }
    );
    emit.tu
}

#[test]
fn err_result_entry_emits_adapter_and_calls_the_faithful_entry() {
    // C03 / R01-R04: the ErrResult entry emits an in-TU adapter that calls the
    // faithfully-emitted entry, reads the Result discriminant (section 13 niche
    // read), returns 0 on Ok, and reads + switches the RegseqError tag on Err.
    let mut m = model();
    let err = regseq_error_enum_full(&mut m, "crate::RegseqError", true);
    let tu = emit_err_result_entry(&mut m, err);

    assert!(
        tu.contains("int regseq_main__regseq_exit(void) {"),
        "the adapter is hand-emitted in the TU:\n{tu}"
    );
    assert!(
        tu.contains("struct core_result_Result __r = regseq_main();"),
        "the adapter calls the faithful entry, capturing the Result struct:\n{tu}"
    );
    // Section 13 niche read of the Result discriminant (memcpy into a raw temp),
    // then `return 0;` on the Ok discriminant (R04).
    assert!(
        tu.contains("memcpy(&__nd, (const unsigned char *)&(__r) + 0, sizeof(__nd));"),
        "the Result discriminant is read via the section 13 niche memcpy:\n{tu}"
    );
    assert!(
        tu.contains("__disc = (__nd == ((uint8_t)(9U)) ? ((uint8_t)(0U)) : ((uint8_t)(1U)));"),
        "the niche read ternary maps the cell to Ok=0 / Err=1:\n{tu}"
    );
    assert!(
        tu.contains("if (__disc == ((uint8_t)(0U))) {\n        return 0;\n    }"),
        "Ok returns 0 (R04):\n{tu}"
    );
    // Section 13 Direct tag read of the inner RegseqError, then the switch.
    assert!(
        tu.contains("switch (__r._0.tag) {"),
        "the RegseqError variant is read via its section 13 Direct tag view:\n{tu}"
    );
}

#[test]
fn adapter_switch_is_exhaustive_over_all_nine_c02_rows_plus_ok() {
    // The adapter-switch half of T10 (D07): every C02 row plus the Ok->0 arm is
    // verified through the adapter, reusing the regseq-backends-26 table.
    let mut m = model();
    let err = regseq_error_enum_full(&mut m, "crate::RegseqError", true);
    let tu = emit_err_result_entry(&mut m, err);

    for (name, tv, sym) in C02_ROWS {
        // Reuse the C02 table (regseq-backends-26): the adapter symbol must agree.
        assert_eq!(
            super::errno::errno_symbol(name),
            Some(*sym),
            "C02 row {name} maps to {sym}"
        );
        let arm = format!("    case ((uint8_t)({tv}U)): return {sym};\n");
        assert!(
            tu.contains(&arm),
            "the adapter switch has the {name} -> {sym} arm ({arm:?}):\n{tu}"
        );
    }
    // The Ok -> 0 arm and the corrupt-discriminant defense.
    assert!(tu.contains("return 0;"), "the Ok arm returns 0:\n{tu}");
    assert!(
        tu.contains("    default: return EIO;\n"),
        "default: EIO is the corrupt-discriminant defense:\n{tu}"
    );
}

#[test]
fn adapter_codes_are_symbolic_and_errno_h_is_demand_driven() {
    // I03 / R07 / C02: emitted codes are symbolic errno identifiers and the
    // `<errno.h>` include is emitted only when the adapter is.
    let mut m = model();
    let err = regseq_error_enum_full(&mut m, "crate::RegseqError", true);
    let tu = emit_err_result_entry(&mut m, err);

    assert!(
        tu.contains("#include <errno.h>"),
        "the adapter TU pulls in <errno.h> on demand:\n{tu}"
    );
    assert!(
        tu.contains("return EINVAL;"),
        "symbolic errno emission:\n{tu}"
    );
    assert!(
        tu.contains("return ETIMEDOUT;"),
        "symbolic errno emission:\n{tu}"
    );

    // A unit-void TU must NOT pull in <errno.h> (demand-driven, I02).
    let mut mv = model();
    let unit = unit_ty(&mut mv);
    let sig = fnsig(&mut mv, vec![], unit, FnAbi::C, false);
    fn_instance(&mut mv, "regseq_main", sig, None);
    let void_tu = emit_generic_c_for_entry(&mv, "regseq_main").unwrap().tu;
    assert!(
        !void_tu.contains("errno.h"),
        "the unit-void TU has no demand-driven <errno.h>:\n{void_tu}"
    );
}

#[test]
fn unknown_marked_variant_fails_emit_never_eio() {
    // R08 / I05: a marked RegseqError variant absent from the C02 table fails
    // emit (naming the variant), never silently mapping to EIO.
    let mut m = model();
    let u8t = uint(&mut m, UintKind::U8);
    let it = item(&mut m, ItemKind::Enum, "crate::RegseqError");
    let tag = TagScalar {
        offset: 0,
        scalar_ty: u8t,
        valid_range: WrappingRange { start: 0, end: 1 },
    };
    let shape = EnumShape::Multiple {
        tag,
        encoding: TagEncoding::Direct,
        variants: vec![
            variant(&mut m, "InvalidParameter", Vec::new(), 0, Some(0)),
            variant(&mut m, "Frobnicate", Vec::new(), 1, Some(1)),
        ],
    };
    let err = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout: enum_layout(1, 1, shape),
    });
    mark_framework_error(&mut m, it);
    let unit = unit_ty(&mut m);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_main", sig, None);

    match emit_generic_c_for_entry(&m, "regseq_main") {
        Err(EmitError::ErrnoMapping(name)) => assert_eq!(name, "Frobnicate"),
        other => panic!("expected ErrnoMapping naming Frobnicate, got {other:?}"),
    }
}

#[test]
fn adapter_emission_is_byte_identical_across_two_calls() {
    // I02 / ledger byte-stability: the adapter path is a pure function of the
    // model, so two emits agree byte-for-byte.
    let mut m = model();
    let err = regseq_error_enum_full(&mut m, "crate::RegseqError", true);
    let unit = unit_ty(&mut m);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_main", sig, None);

    let a = emit_generic_c_for_entry(&m, "regseq_main").unwrap().tu;
    let b = emit_generic_c_for_entry(&m, "regseq_main").unwrap().tu;
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// regseq-backends-31: fail-loud negatives through the PUBLIC emit seam (T13/T14).
// `unknown_marked_variant_fails_emit_never_eio` (regseq-backends-28, above) is
// the T13 backend unit through `emit_generic_c_for_entry`. These pin every
// D08/D09-rejected entry shape at that same public seam -- the single
// enforcement point both consumers call -- so the reject is proven at emit, not
// only at the test-only `resolve_entry_shim` resolver (regseq-backends-27).
// The harness-integration half lives in
// `codegen-coverage-runner/tests/regseq_error_negatives.rs`.
// ---------------------------------------------------------------------------

#[test]
fn emit_for_entry_rejects_non_unit_ok_at_emit() {
    // D08: `Result<u32, RegseqError>` -- a non-unit Ok payload.
    let mut m = model();
    let u32t = uint(&mut m, UintKind::U32);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, u32t, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    match emit_generic_c_for_entry(&m, "regseq_main") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("non-unit Ok"),
            "the public emit seam names the non-unit-Ok shape: {msg}"
        ),
        other => panic!("expected EntryShape from emit, got {other:?}"),
    }
}

#[test]
fn emit_for_entry_rejects_unmarked_err_enum_at_emit() {
    // D08: `Result<(), PlainErr>` where the Err enum is NOT marked regseq-error.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::PlainErr", false);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    match emit_generic_c_for_entry(&m, "regseq_main") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("not marked") && msg.contains("regseq-error"),
            "the public emit seam names the unmarked-Err shape: {msg}"
        ),
        other => panic!("expected EntryShape from emit, got {other:?}"),
    }
}

#[test]
fn emit_for_entry_rejects_entry_with_arguments_at_emit() {
    // D08: an entry taking arguments is unsupported.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let u32t = uint(&mut m, UintKind::U32);
    let sig = fnsig(&mut m, vec![u32t], unit, FnAbi::C, false);
    fn_instance(&mut m, "regseq_main", sig, None);
    match emit_generic_c_for_entry(&m, "regseq_main") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("argument"),
            "the public emit seam names the arguments shape: {msg}"
        ),
        other => panic!("expected EntryShape from emit, got {other:?}"),
    }
}

#[test]
fn emit_for_entry_rejects_custom_entry_with_error_return_at_emit() {
    // D09: a manifest-custom entry (non-default suffix) combined with an
    // error-returning signature is deferred this phase. The reject fires in
    // `resolve_entry` before the adapter is built, so a valid (marked) enum still
    // yields the entry-shape error, not an errno-mapping miss.
    let mut m = model();
    let unit = unit_ty(&mut m);
    let err = error_enum(&mut m, "crate::RegseqError", true);
    let result = result_enum(&mut m, unit, err);
    let sig = fnsig(&mut m, vec![], result, FnAbi::Rust, false);
    fn_instance(&mut m, "my_custom_entry", sig, None);
    match emit_generic_c_for_entry(&m, "my_custom_entry") {
        Err(EmitError::EntryShape(msg)) => assert!(
            msg.contains("manifest-custom entry")
                && msg.contains("my_custom_entry")
                && msg.contains("error-returning"),
            "the public emit seam names both halves of the custom-entry+error shape: {msg}"
        ),
        other => panic!("expected EntryShape from emit, got {other:?}"),
    }
}
