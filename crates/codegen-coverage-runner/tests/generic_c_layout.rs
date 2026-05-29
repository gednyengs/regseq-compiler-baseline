//! Behavioral layout-fidelity tests for the generic-C backend banks 2-3
//! (regseq-backends-06, task c1). These live in the runner crate because
//! it owns `toolchain::compile_and_run` -- the backend's host-`cc`
//! verification harness.
//!
//! The proof is backend-internal (the same pattern task b used): bridge an
//! aggregate snippet -> `emit_generic_c` -> compile the emitted TU. The
//! section 14 `_Static_assert(sizeof/_Alignof/offsetof)` guards make the
//! TU self-checking, so a clean compile *is* the proof that the lowering
//! reproduces rustc's layout (no green codegen-coverage fixture is
//! produced -- that needs a body + diag-log, tasks d/e). The wrong-offset
//! injection test proves the guards bite: an inconsistent offset fails the
//! compile on the `_Static_assert`.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::toolchain::{self, ToolchainError};
use regseq_backends::emit_generic_c;
use regseq_model::{
    CrateId, DefPath, Endian, EnumShape, FieldLayout, FnAbi, FnSig, GenericArgs, Instance,
    InstanceKind, IntKind, Item, ItemId, ItemKind, Layout, LayoutKind, Linkage, Provenance,
    RegseqModel, Safety, ScalarLayouts, SizeAlign, SourceFileId, Span, StableCrateId, TagEncoding,
    TagScalar, TargetInfo, Ty, TyId, UintKind, VariantLayout, WrappingRange,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp `.rs` file removed on drop (matches `tests/bridge_smoke.rs`).
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("ccr-c1-{label}-{}-{nanos}.rs", std::process::id()));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A `#![no_std]` snippet exercising `repr(Rust)` reordering, a union, a
/// tuple field, and an array field. Each type is "touched" by a
/// straight-line function taking `&T`, which forces Phase-2 discovery of
/// its `Ty` + an `InstanceKind` root (the established
/// `instance_completeness` pattern) without introducing non-RPO basic
/// blocks. (`#[repr(packed)]` is exercised separately by a hand-built
/// model below: the bridge currently surfaces `repr(packed)` layouts with
/// `pack: None`, which the model validator rejects, so it cannot be
/// bridged here -- that is a bridge concern, out of this task's scope.)
const AGGREGATES_SNIPPET: &str = r#"#![no_std]

// repr(Rust): mixed-size fields are reordered by rustc, so verbatim field
// emission would not reproduce the offsets.
pub struct Reordered {
    a: u8,
    b: u32,
    c: u16,
}

pub union Onion {
    a: u32,
    b: u8,
}

pub struct WithTuple {
    t: (u8, u32, u16),
}

pub struct WithArray {
    arr: [u32; 4],
}

// Each type is "touched" by a `&T` parameter, which surfaces its `Ty` +
// an `InstanceKind` root. The bodies return a constant (rather than reading
// a field) so they lower under the task-d1 scalar subset: layout discovery
// comes from the parameter type, not the body.
pub fn t_reordered(_p: &Reordered) -> u32 { 0 }
pub fn t_union(_p: &Onion) -> u32 { 0 }
pub fn t_tuple(_p: &WithTuple) -> u32 { 0 }
pub fn t_array(_p: &WithArray) -> u32 { 0 }
"#;

/// A `#![no_std]` snippet exercising the four enum shapes task c2 lowers:
/// a `repr(u32)` fieldless `Direct` enum (section 9.8, collapses to a
/// tag-only struct), a niche `Option<&u32>` (section 9.9, collapses to a
/// single-field struct), a multi-field-payload `repr(Rust)` enum, and the
/// shifted-alignment `enum Shift { A(u8), B(u32) }` (tag u8 @0, A.0 @1,
/// B.0 @4 -- the case the literal `union_start` form could not place).
/// Each is touched by a straight-line function so Phase-2 surfaces its
/// `Ty::Enum` + an `InstanceKind::Enum` root.
const ENUMS_SNIPPET: &str = r#"#![no_std]

#[repr(u32)]
pub enum Status {
    Idle   = 0,
    Active = 1,
    Error  = 0xFF,
    Reset  = 0xDEAD,
}

pub enum Payload {
    A(u8),
    B { x: u32, y: u16 },
    C,
}

pub enum Shift {
    A(u8),
    B(u32),
}

pub fn t_status(_s: &Status) -> u32 { 0 }
pub fn t_option(_x: Option<&u32>) -> u32 { 0 }
pub fn t_payload(_p: &Payload) -> u32 { 0 }
pub fn t_shift(_s: &Shift) -> u32 { 0 }
"#;

#[test]
fn bridged_enums_emit_a_tu_that_compiles_clean() {
    let snippet = TempSnippet::new("enums", ENUMS_SNIPPET);
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok");

    // Sanity: the union-of-views machinery surfaced for the multi-view
    // enums (Payload / Shift), and the collapsed forms for Status / Option.
    assert!(
        tu.contains("uint32_t tag;"),
        "the Direct tag view surfaced:\n{tu}"
    );
    assert!(
        tu.contains("union {"),
        "a multi-view enum surfaced a union:\n{tu}"
    );
    assert!(
        tu.contains("offsetof(struct ") && tu.contains("u._tag.tag)"),
        "nested member-designator tag guards:\n{tu}"
    );

    // Compile the TU + a trivial entry: the layout guards passing IS the
    // proof that the emitted offsets reproduce rustc's enum layout for all
    // four shapes (Direct fieldless, Niche, multi-field payload, shifted).
    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect("enum TU must compile (layout guards hold) + run");
    assert_eq!(run.exit_code, Some(0), "must exit 0: {}", run.stderr);
    assert_eq!(run.stderr, "", "clean stderr");
}

#[test]
fn bridged_aggregates_emit_a_tu_that_compiles_clean() {
    let snippet = TempSnippet::new("aggregates", AGGREGATES_SNIPPET);
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok");

    // Sanity: the discovered aggregates surfaced with their layout guards.
    // (Type names carry the bridge-rendered DefPath, i.e. a crate-name
    // prefix, so match on the trailing nominal name + shape.)
    assert!(tu.contains("_Reordered {"), "Reordered must be emitted");
    assert!(tu.contains("_Onion {"), "the union must be emitted");
    assert!(tu.contains("union "), "a C union tag must be emitted");
    assert!(
        tu.contains("struct tuple_"),
        "the tuple struct must be emitted"
    );
    assert!(
        tu.contains("_Static_assert(offsetof("),
        "per-field offset guards must be present"
    );

    // Compile the TU + a trivial entry. The layout guards passing IS the
    // proof that the emitted offsets reproduce rustc's layout.
    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect("aggregate TU must compile (layout guards hold) + run");
    assert_eq!(run.exit_code, Some(0), "must exit 0: {}", run.stderr);
    assert_eq!(run.stderr, "", "clean stderr");
}

// ---------------------------------------------------------------------------
// Wrong-offset injection: a hand-built layout with an offset that C cannot
// honor (a `uint32_t` claimed at a misaligned offset in a non-packed
// struct) -> the emitted `_Static_assert(offsetof ...)` fails the compile.
// ---------------------------------------------------------------------------

fn target_model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("c1-wrong", StableCrateId(1));
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

/// Push a single-field-vector `Ty::Struct` + `InstanceKind::Struct` root
/// named `rendered` into `model`.
fn push_struct(model: &mut RegseqModel, rendered: &str, layout: Layout) {
    let rid = model.strings.intern(rendered);
    let item = model.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind: ItemKind::Struct,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    });
    let ty = model.types.intern(Ty::Struct {
        item,
        args: GenericArgs::new(),
        layout,
    });
    model.instances.push(Instance {
        item,
        generic_args: GenericArgs::new(),
        emission_name: rid,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Struct { layout: ty },
    });
}

#[test]
fn hand_built_packed_struct_emits_a_tu_that_compiles_clean() {
    // #[repr(packed)] Packed { x: u8 @0, y: u32 @1 }, size 5 align 1. The
    // `__attribute__((packed, aligned(1)))` makes offsetof(y)==1 and
    // sizeof==5 hold, so the layout guards pass and the TU compiles clean.
    // (Hand-built because the bridge cannot currently surface a valid
    // repr(packed) layout -- see AGGREGATES_SNIPPET.)
    let mut model = target_model();
    let u8t = model.types.intern(Ty::Uint(UintKind::U8));
    let u32t = model.types.intern(Ty::Uint(UintKind::U32));
    let x = FieldLayout {
        name: model.strings.intern("x"),
        ty: u8t,
        offset: 0,
    };
    let y = FieldLayout {
        name: model.strings.intern("y"),
        ty: u32t,
        offset: 1,
    };
    push_struct(
        &mut model,
        "Packed",
        Layout {
            size: 5,
            align: 1,
            pack: Some(1),
            uninhabited: false,
            kind: LayoutKind::Struct { fields: vec![x, y] },
        },
    );

    let tu = emit_generic_c(&model).expect("emit must return Ok");
    assert!(tu.contains("__attribute__((packed, aligned(1)))"));
    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect("packed TU must compile (guards hold) + run");
    assert_eq!(run.exit_code, Some(0), "must exit 0: {}", run.stderr);
}

#[test]
fn wrong_offset_fails_the_compile_on_the_static_assert() {
    let mut model = target_model();
    let rid = model.strings.intern("Bad");
    let item = model.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind: ItemKind::Struct,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    });
    let u32t = model.types.intern(Ty::Uint(UintKind::U32));
    // A single `uint32_t` field CLAIMED at offset 1 -- impossible in a
    // non-packed struct (C aligns it to 4), so offsetof != 1.
    let bad_field = FieldLayout {
        name: model.strings.intern("v"),
        ty: u32t,
        offset: 1,
    };
    let ty = model.types.intern(Ty::Struct {
        item,
        args: GenericArgs::new(),
        layout: Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![bad_field],
            },
        },
    });
    let emission = model.strings.intern("Bad");
    model.instances.push(Instance {
        item,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Struct { layout: ty },
    });

    let tu = emit_generic_c(&model).expect("emit must return Ok");
    // The guard line is present and asserts the (wrong) injected offset.
    assert!(tu.contains("_Static_assert(offsetof(struct Bad, v) == 1"));

    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let err = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect_err("a wrong offset must fail the compile on the _Static_assert");
    assert!(
        matches!(err, ToolchainError::CompileFailed { .. }),
        "expected CompileFailed, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Task-07 robustness fixes: hand-built models whose emitted TU compiles clean
// under host `cc` (the layout guards + valid declarator syntax are the proof).
// ---------------------------------------------------------------------------

fn synth_span() -> Span {
    Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    }
}

fn mk_item(model: &mut RegseqModel, kind: ItemKind, rendered: &str) -> ItemId {
    let rid = model.strings.intern(rendered);
    model.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind,
        span: synth_span(),
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

fn mk_field(model: &mut RegseqModel, name: &str, ty: TyId, offset: u64) -> FieldLayout {
    FieldLayout {
        name: model.strings.intern(name),
        ty,
        offset,
    }
}

fn slayout(size: u64, align: u64, fields: Vec<FieldLayout>) -> Layout {
    Layout {
        size,
        align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

/// Intern a non-generic `Ty::Struct` without registering an instance (so it
/// is only emitted if reached by containment from a real root).
fn intern_struct(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = mk_item(model, ItemKind::Struct, rendered);
    model.types.intern(Ty::Struct {
        item: it,
        args: GenericArgs::new(),
        layout,
    })
}

/// Intern a `Ty::Struct` AND register an `InstanceKind::Struct` root.
fn root_struct(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let ty = intern_struct(model, rendered, layout);
    let it = match &model.types[ty] {
        Ty::Struct { item, .. } => *item,
        _ => unreachable!(),
    };
    let emission = model.strings.intern(rendered);
    model.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: synth_span(),
        kind: InstanceKind::Struct { layout: ty },
    });
    ty
}

/// Intern a no-argument `fn() -> output` function-pointer `Ty`.
fn fn_ptr_returning(model: &mut RegseqModel, output: TyId) -> TyId {
    let sig = model.sigs.intern(FnSig {
        inputs: vec![],
        output,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    model.types.intern(Ty::FnPtr(sig))
}

/// Append a trivial entry, compile the TU + entry under host `cc`, and assert
/// a clean exit (so all type definitions / declarators are valid C).
fn assert_compiles(tu: &str, what: &str) {
    let full = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = toolchain::compile_and_run(&full, "regseq_main", cc())
        .unwrap_or_else(|e| panic!("{what} must compile: {e}"));
    assert_eq!(
        run.exit_code,
        Some(0),
        "{what}: must exit 0: {}",
        run.stderr
    );
    assert_eq!(run.stderr, "", "{what}: clean stderr");
}

#[test]
fn slice_element_aggregates_compile() {
    // Fix A: a struct/tuple element reachable only via a slice (forward-decl
    // suffices) AND an array element (ordered complete-first) both compile.
    let mut m = target_model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));

    // Sa { s: &[InnerA] } -- InnerA reachable only through the slice.
    let va = mk_field(&mut m, "v", u32t, 0);
    let inner_a = intern_struct(&mut m, "InnerA", slayout(4, 4, vec![va]));
    let slice_a = m.types.intern(Ty::Slice(inner_a));
    let ref_a = m.types.intern(Ty::Ref(slice_a, Provenance::SharedBorrow));
    let sa_field = mk_field(&mut m, "s", ref_a, 0);
    root_struct(&mut m, "Sa", slayout(16, 8, vec![sa_field]));

    // Sb { s: &[[InnerB; 4]] } -- array element ordered before the slice.
    let vb = mk_field(&mut m, "v", u32t, 0);
    let inner_b = intern_struct(&mut m, "InnerB", slayout(4, 4, vec![vb]));
    let arr_b = m.types.intern(Ty::Array(inner_b, 4));
    let slice_b = m.types.intern(Ty::Slice(arr_b));
    let ref_b = m.types.intern(Ty::Ref(slice_b, Provenance::SharedBorrow));
    let sb_field = mk_field(&mut m, "s", ref_b, 0);
    root_struct(&mut m, "Sb", slayout(16, 8, vec![sb_field]));

    let tu = emit_generic_c(&m).expect("slice-element aggregates emit Ok (no panic)");
    assert!(
        tu.contains("(*data)[4]"),
        "array-element slice data slot:\n{tu}"
    );
    assert_compiles(&tu, "slice-element aggregates");
}

#[test]
fn nested_const_ref_compiles() {
    // Fix B: `& &u32` = `const uint32_t *const *` is valid, correctly-placed C.
    let mut m = target_model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let inner_ref = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let outer_ref = m.types.intern(Ty::Ref(inner_ref, Provenance::SharedBorrow));
    let f = mk_field(&mut m, "p", outer_ref, 0);
    root_struct(&mut m, "Sc", slayout(8, 8, vec![f]));

    let tu = emit_generic_c(&m).expect("nested ref emits Ok");
    assert!(tu.contains("const uint32_t *const *p;"), "got:\n{tu}");
    assert_compiles(&tu, "nested const ref");
}

#[test]
fn pad_filler_collisions_compile() {
    // Fix C: a struct with a real `_pad0` field + a gap gets a distinct `_pad1`
    // filler; a union with a real `_pad` member gets the always-emitted
    // byte-covering `_raw` member (d2.3a Change A subsumes the union `_pad`
    // filler; `_pad` != `_raw`, so no collision). Both compile clean.
    let mut m = target_model();
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));

    let p0 = mk_field(&mut m, "_pad0", u8t, 0);
    let x = mk_field(&mut m, "x", u32t, 8);
    root_struct(&mut m, "Sd", slayout(12, 4, vec![p0, x]));

    let pad = mk_field(&mut m, "_pad", u8t, 0);
    let union_item = mk_item(&mut m, ItemKind::Union, "Ue");
    let union_ty = m.types.intern(Ty::Union {
        item: union_item,
        args: GenericArgs::new(),
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![pad] },
        },
    });
    let emission = m.strings.intern("Ue");
    m.instances.push(Instance {
        item: union_item,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: synth_span(),
        kind: InstanceKind::Union { layout: union_ty },
    });

    let tu = emit_generic_c(&m).expect("pad-collision model emits Ok");
    assert!(tu.contains("uint8_t _pad0;") && tu.contains("unsigned char _pad1["));
    assert!(tu.contains("uint8_t _pad;") && tu.contains("unsigned char _raw[4];"));
    assert_compiles(&tu, "pad-filler collisions");
}

#[test]
fn fn_ptr_return_declarators_compile() {
    // Fix D: a fn pointer returning a fn pointer, and one returning a
    // pointer-to-array, both recurse the declarator into valid C.
    let mut m = target_model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));

    let inner_fn = fn_ptr_returning(&mut m, i32t);
    let cb = fn_ptr_returning(&mut m, inner_fn);

    let arr = m.types.intern(Ty::Array(u32t, 4));
    let aref = m.types.intern(Ty::Ref(arr, Provenance::SharedBorrow));
    let cb2 = fn_ptr_returning(&mut m, aref);

    let cb_field = mk_field(&mut m, "cb", cb, 0);
    let cb2_field = mk_field(&mut m, "cb2", cb2, 8);
    root_struct(&mut m, "Sf", slayout(16, 8, vec![cb_field, cb2_field]));

    let tu = emit_generic_c(&m).expect("fn-ptr-return model emits Ok");
    assert!(tu.contains("int32_t (*(*cb)(void))(void);"), "got:\n{tu}");
    assert!(
        tu.contains("const uint32_t (*(*cb2)(void))[4];"),
        "got:\n{tu}"
    );
    assert_compiles(&tu, "fn-ptr return declarators");
}

#[test]
fn shared_ref_to_fn_ptr_places_const_on_the_value_and_compiles() {
    // Task 09 (07 #3): `&fn() -> i32` places `const` on the fn-pointer VALUE
    // (`int32_t (*const (*p))(void)`), not the return type. The exact spelling
    // pins the const level; the clean compile proves it is valid C.
    let mut m = target_model();
    let i32t = m.types.intern(Ty::Int(IntKind::I32));
    let fnptr = fn_ptr_returning(&mut m, i32t);
    let r = m.types.intern(Ty::Ref(fnptr, Provenance::SharedBorrow));
    let f = mk_field(&mut m, "p", r, 0);
    root_struct(&mut m, "Sg", slayout(8, 8, vec![f]));

    let tu = emit_generic_c(&m).expect("ref-to-fn-ptr model emits Ok");
    assert!(
        tu.contains("int32_t (*const (*p))(void);"),
        "const on the fn-ptr value:\n{tu}"
    );
    assert!(
        !tu.contains("const int32_t (*"),
        "const must not land on the return type:\n{tu}"
    );
    assert_compiles(&tu, "shared ref to fn-ptr const placement");
}

// ---------------------------------------------------------------------------
// Task c2: enum union-of-views (section 13 / D9). Hand-built models pin the
// shifted-alignment absolute offset that the literal `union_start` form
// could not place, and prove the `_Static_assert` guards bite for an enum.
// ---------------------------------------------------------------------------

/// Register an `InstanceKind::Enum` root for a hand-built enum `Layout`.
fn root_enum(model: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = mk_item(model, ItemKind::Enum, rendered);
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
        span: synth_span(),
        kind: InstanceKind::Enum { layout: ty },
    });
    ty
}

/// The shifted-alignment `enum Shift { A(u8), B(u32) }` shape: tag u8 @0,
/// A.0 @1, B.0 @4, size 8 align 4.
fn shift_shape(m: &mut RegseqModel) -> EnumShape {
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let a0 = mk_field(m, "0", u8t, 1);
    let b0 = mk_field(m, "0", u32t, 4);
    let a_name = m.strings.intern("A");
    let b_name = m.strings.intern("B");
    EnumShape::Multiple {
        tag: TagScalar {
            offset: 0,
            scalar_ty: u8t,
            valid_range: WrappingRange { start: 0, end: 1 },
        },
        encoding: TagEncoding::Direct,
        variants: vec![
            VariantLayout {
                name: a_name,
                fields: vec![a0],
                uninhabited: false,
                discriminant: 0,
                tag_value: Some(0),
            },
            VariantLayout {
                name: b_name,
                fields: vec![b0],
                uninhabited: false,
                discriminant: 1,
                tag_value: Some(1),
            },
        ],
    }
}

#[test]
fn shifted_alignment_direct_enum_places_payload_at_absolute_offset() {
    // enum Shift { A(u8), B(u32) } -- the union-of-views places B.0's
    // uint32_t at its ABSOLUTE offset 4 (the case the literal union_start
    // form could not place); the nested-designator guard asserts it and
    // the TU compiles clean (the guards hold = layout-fidelity proof).
    let mut m = target_model();
    let shape = shift_shape(&mut m);
    root_enum(
        &mut m,
        "Shift",
        Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum { shape },
        },
    );

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("_Static_assert(offsetof(struct Shift, u._v1._0) == 4,"),
        "B.0's uint32_t must land at absolute offset 4:\n{tu}"
    );
    assert!(
        tu.contains("_Static_assert(offsetof(struct Shift, u._tag.tag) == 0,"),
        "tag @0:\n{tu}"
    );
    assert_compiles(&tu, "shifted-alignment enum");
}

#[test]
fn wrong_enum_field_offset_fails_the_compile_on_the_static_assert() {
    // A multi-view Direct enum whose B variant claims its uint32_t at the
    // impossible offset 1: the _v1 view places the uint32_t at its natural
    // offset 4, so offsetof != 1 and the _Static_assert fails the compile.
    let mut m = target_model();
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let a0 = mk_field(&mut m, "0", u8t, 1);
    let bad_b0 = mk_field(&mut m, "0", u32t, 1); // impossible: u32 cannot sit @1
    let a_name = m.strings.intern("A");
    let b_name = m.strings.intern("B");
    let shape = EnumShape::Multiple {
        tag: TagScalar {
            offset: 0,
            scalar_ty: u8t,
            valid_range: WrappingRange { start: 0, end: 1 },
        },
        encoding: TagEncoding::Direct,
        variants: vec![
            VariantLayout {
                name: a_name,
                fields: vec![a0],
                uninhabited: false,
                discriminant: 0,
                tag_value: Some(0),
            },
            VariantLayout {
                name: b_name,
                fields: vec![bad_b0],
                uninhabited: false,
                discriminant: 1,
                tag_value: Some(1),
            },
        ],
    };
    root_enum(
        &mut m,
        "Bad",
        Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum { shape },
        },
    );

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("_Static_assert(offsetof(struct Bad, u._v1._0) == 1"),
        "the guard asserts the (wrong) injected offset:\n{tu}"
    );

    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let err = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect_err("a wrong enum field offset must fail the compile on the _Static_assert");
    assert!(
        matches!(err, ToolchainError::CompileFailed { .. }),
        "expected CompileFailed, got {err:?}"
    );
}
