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
    CrateId, DefPath, Endian, FieldLayout, GenericArgs, Instance, InstanceKind, Item, ItemKind,
    Layout, LayoutKind, Linkage, RegseqModel, ScalarLayouts, SizeAlign, SourceFileId, Span,
    StableCrateId, TargetInfo, Ty, UintKind,
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

pub fn t_reordered(_p: &Reordered) -> u32 { 0 }
pub fn t_union(p: &Onion) -> u32 { unsafe { p.a } }
pub fn t_tuple(_p: &WithTuple) -> u32 { 0 }
pub fn t_array(_p: &WithArray) -> u32 { 0 }
"#;

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
