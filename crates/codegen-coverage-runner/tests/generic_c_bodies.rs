//! Behavioral body-lowering tests for the generic-C backend banks 4-5
//! (regseq-backends-10, task d1). The first task whose verification *runs*
//! the emitted program, not just compiles it.
//!
//! The bridged tests compile + run a `void regseq_main(void)` whose lowered
//! body computes a scalar value and calls an extern abort-callback on a
//! wrong result. A clean exit (code 0) *is* the proof the section 7 body
//! lowering computed correctly; a wrong lowering takes the abort path
//! (non-zero exit). The callback is a real `extern "C"` call (kept by
//! rustc's optimizer, unlike `unreachable_unchecked`), so the comparison +
//! conditional call survive into the emitted C and the runtime check is
//! genuine. Hand-built models pin shapes the bridge cannot surface
//! (`Assert` -> section 16 message, the cast table, `Foreign` / `Weak` /
//! `link_section` / `inline` linkage, a `Reloc` static initializer).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::toolchain::{self, EntryShim};
use regseq_backends::emit_generic_c;
use regseq_model::{
    AggregateKind, AssertKind, BasicBlock, BinOp, BlockId, Body, CallCallee, CastKind, Const,
    CrateId, DefPath, DefPathHash, Endian, EnumShape, FieldIdx, FieldLayout, FloatKind, FnAbi,
    FnSig, FnSigId, FunctionInstanceData, FunctionInstanceFlavor, GenericArgs, Instance,
    InstanceId, InstanceKind, Item, ItemId, ItemKind, Layout, LayoutKind, LocalDecl, LocalId,
    Mutability, Operand, Place, PlaceElem, Provenance, RegseqModel, Rvalue, Safety, ScalarLayouts,
    SizeAlign, SourceFileId, Span, StableCrateId, Statement, StatementKind, SwitchTargets,
    TagEncoding, TagScalar, TargetInfo, Terminator, TerminatorKind, Ty, TyId, UintKind, VariantIdx,
    VariantLayout, VariantRange, WrappingRange,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp `.rs` file removed on drop.
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
            std::env::temp_dir().join(format!("ccr-d1-{label}-{}-{nanos}.rs", std::process::id()));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Map a rendered `DefPath` to the C identifier the backend emits for it
/// (the bridge mints `emission_name = def_path.rendered`, which the backend
/// sanitises -- this mirrors `naming::sanitize`).
fn c_ident(s: &str) -> String {
    s.replace("::", "_")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The emitted C name of the function instance whose rendered path ends in
/// `suffix` (e.g. `regseq_main`).
fn fn_cname(model: &RegseqModel, suffix: &str) -> String {
    for inst in model.instances.iter() {
        if matches!(inst.kind, InstanceKind::Function(_)) {
            let rendered = model.strings.resolve(inst.emission_name);
            if rendered.ends_with(suffix) {
                return c_ident(rendered);
            }
        }
    }
    panic!("no function instance whose path ends in {suffix:?}");
}

/// A scalar driver whose `regseq_main` computes `combine(2, 3) == 2 ^ 3 == 1`
/// via a non-inlined helper (so the `Call` terminator is genuinely emitted)
/// and calls the abort-callback if the result is not `expected`. With
/// `expected = 1` the callback is never reached (correct lowering -> exit 0);
/// with a wrong `expected` it always fires (abort -> non-zero exit). `^` is a
/// pure `BinOp` (no overflow check, no helper call), so the helper body pulls
/// in no undefined symbol.
fn scalar_snippet(expected: u32) -> String {
    format!(
        r#"#![no_std]

extern "C" {{
    fn regseq_test_fail();
}}

#[no_mangle]
#[inline(never)]
pub extern "C" fn combine(a: u32, b: u32) -> u32 {{
    a ^ b
}}

#[no_mangle]
pub extern "C" fn regseq_main() {{
    let r = combine(2, 3);
    if r != {expected} {{
        unsafe {{ regseq_test_fail(); }}
    }}
}}
"#
    )
}

#[test]
fn bridged_scalar_body_runs_and_exits_zero_on_a_correct_result() {
    let snippet = TempSnippet::new("scalar-ok", &scalar_snippet(1));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for a d1-subset body");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    // Sanity: the section 7 envelope surfaced (the `regseq_main` definition).
    assert!(
        tu.contains("uint32_t") && tu.contains(&format!("void {entry}(void)")),
        "prototypes / definitions surfaced:\n{tu}"
    );

    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("scalar-body TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "a correct lowering exits 0; stderr: {}",
        run.stderr
    );
}

#[test]
fn bridged_scalar_body_aborts_on_a_wrong_result() {
    // The same body with a wrong expected value always takes the abort path,
    // proving the comparison + conditional call lowering fires at runtime.
    let snippet = TempSnippet::new("scalar-bad", &scalar_snippet(2));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("wrong-result TU must compile + run");
    assert_ne!(
        run.exit_code,
        Some(0),
        "a wrong result must abort (non-zero)"
    );
    assert!(
        run.stderr.contains("wrong result"),
        "the abort path ran: {}",
        run.stderr
    );
}

/// A structured-data driver (task d2): `regseq_main` builds a `repr(Rust)`
/// struct (reordered fields) and reads each back, builds + indexes a fixed
/// array, and takes a **runtime** slice (`&arr`) it indexes. Each wrong
/// result calls the extern abort-callback. With correct lowering nothing
/// aborts (exit 0); a wrong field offset / aggregate / slice lowering takes
/// the abort path.
///
/// The fixture deliberately uses only language primitives (struct / tuple
/// construction, field access, array + slice indexing), never a `core`
/// method call (`.len()` / `wrapping_add`, which the bridge surfaces as
/// body-less externs that would not link). The slice index `s[1]` is a MIR
/// primitive whose bounds-check Assert reads the runtime length via
/// `Len(*s)` -> `PtrMetadata` (the `.len` slot), so the runtime slice path
/// (Unsize + fat `Deref` + `Index` + `PtrMetadata`) is exercised end-to-end.
/// No string / array literal const is used (those are task d2.2): the slice
/// is a runtime borrow of a local array.
fn structured_snippet(field_expected: u32) -> String {
    format!(
        r#"#![no_std]

extern "C" {{
    fn regseq_test_fail();
}}

struct Point {{ a: u8, b: u32 }}

#[no_mangle]
#[inline(never)]
pub extern "C" fn make_point(a: u8, b: u32) -> u32 {{
    let p = Point {{ a, b }};
    // Read both reordered fields back: proves section 14 field access.
    p.b + (p.a as u32)
}}

#[no_mangle]
pub extern "C" fn regseq_main() {{
    if make_point(7, 100) != {field_expected} {{
        unsafe {{ regseq_test_fail(); }}
    }}
    let arr = [10u32, 20, 30];
    if arr[1] != 20 {{
        unsafe {{ regseq_test_fail(); }}
    }}
    let s: &[u32] = &arr;
    if s[1] != 20 {{
        unsafe {{ regseq_test_fail(); }}
    }}
}}
"#
    )
}

#[test]
fn bridged_structured_data_body_runs_and_exits_zero() {
    let snippet = TempSnippet::new("structured-ok", &structured_snippet(107));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for a d2 structured-data body");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("structured-data TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "correct field / array / slice lowering exits 0; stderr: {}",
        run.stderr
    );
}

#[test]
fn bridged_structured_data_body_aborts_on_a_wrong_field_result() {
    // A wrong expected field value takes the abort path, proving the struct
    // construction + field read lowering computes at runtime.
    let snippet = TempSnippet::new("structured-bad", &structured_snippet(999));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("wrong-result TU must compile + run");
    assert_ne!(run.exit_code, Some(0), "a wrong field result must abort");
    assert!(
        run.stderr.contains("wrong result"),
        "the abort path ran: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// Hand-built model scaffolding (mirrors generic_c_layout.rs) for shapes the
// bridge cannot surface here: an Assert -> section 16 panic, the cast table,
// section 17 linkage, a Reloc static initializer, a float constant.
// ---------------------------------------------------------------------------

fn target_model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("d1", StableCrateId(1));
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

fn mk_item(model: &mut RegseqModel, kind: ItemKind, rendered: &str) -> ItemId {
    let rid = model.strings.intern(rendered);
    let id = model.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind,
        span: span(),
        linkage: regseq_model::Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
        is_result: false,
    });
    model.item_def_paths.push(DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: id.as_u32() as u64,
    });
    id
}

fn uint(model: &mut RegseqModel, kind: UintKind) -> TyId {
    model.types.intern(Ty::Uint(kind))
}

fn fnsig(model: &mut RegseqModel, inputs: Vec<TyId>, output: TyId, abi: FnAbi) -> FnSigId {
    model.sigs.intern(FnSig {
        inputs,
        output,
        abi,
        c_variadic: false,
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
    model: &mut RegseqModel,
    rendered: &str,
    sig: FnSigId,
    body: Option<Body>,
) -> InstanceId {
    let it = mk_item(model, ItemKind::Function, rendered);
    let emission = model.strings.intern(rendered);
    model.instances.push(Instance {
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

/// Compile + run the TU with a trivial entry (or the named entry), asserting
/// the exit code.
fn assert_runs(tu: &str, entry: &str, what: &str) -> toolchain::RunOutput {
    toolchain::compile_and_run(tu, &EntryShim::unit_void(entry), cc())
        .unwrap_or_else(|e| panic!("{what} must compile: {e}"))
}

#[test]
fn assert_failure_prints_the_section_16_message_and_aborts() {
    // regseq_main: `if (cond == 1) goto ok; else regseq_panic(DivisionByZero)`.
    // `cond` is the constant `false`, so the assert always fails -> the
    // section 16 panic fires: stderr carries the message and the process
    // aborts (non-zero exit).
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let boolt = m.types.intern(Ty::Bool);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    let false_c = m.consts.intern(Const::Bool(false));
    let body = build_body(
        0,
        vec![unit],
        vec![
            bb(
                vec![],
                tk(TerminatorKind::Assert {
                    cond: Operand::Constant(false_c),
                    expected: true,
                    msg: AssertKind::DivisionByZero(Operand::Constant(false_c)),
                    target: BlockId::from_u32(1),
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    let _ = boolt;
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    // `regseq_main` is the harness entry; it must be a plain external symbol.
    let it = m.instances[id].item;
    m.items[it].linkage = regseq_model::Linkage::NoMangle;

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO);"),
        "assert routes to the section 16 panic:\n{tu}"
    );
    let run = assert_runs(&tu, "regseq_main", "assert-panic");
    assert_ne!(run.exit_code, Some(0), "a failing assert aborts");
    assert!(
        run.stderr.contains("attempt to divide by zero"),
        "the section 16 message reached stderr: {}",
        run.stderr
    );
}

#[test]
fn cast_and_binop_table_compiles_and_runs() {
    // regseq_main: r = (u32)((f64)x via IntToFloat) then FloatToInt back;
    // verify a few d1 casts + binops emit valid C that runs. Computes
    // `((a << 1) >> 1) ^ a` (== 0 for the masked-shift round trip on the low
    // bit being preserved) and a saturating float cast, asserting nothing at
    // runtime (the clean exit proves the lowering is valid C).
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u32t = uint(&mut m, UintKind::U32);
    let f64t = m.types.intern(Ty::Float(FloatKind::F64));
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    let c7 = m.consts.intern(Const::Uint(UintKind::U32, 7));
    let c1 = m.consts.intern(Const::Uint(UintKind::U32, 1));
    // locals: _0 unit (ret), _1 u32, _2 u32, _3 f64, _4 u32.
    let body = build_body(
        0,
        vec![unit, u32t, u32t, f64t, u32t],
        vec![bb(
            vec![
                // _1 = 7u32
                st(StatementKind::Assign(
                    loc(1),
                    Rvalue::Use(Operand::Constant(c7)),
                )),
                // _2 = (_1 << 1) >> 1   (masked shifts)
                st(StatementKind::Assign(
                    loc(2),
                    Rvalue::BinaryOp(BinOp::Shl, Operand::Copy(loc(1)), Operand::Constant(c1)),
                )),
                st(StatementKind::Assign(
                    loc(2),
                    Rvalue::BinaryOp(BinOp::Shr, Operand::Copy(loc(2)), Operand::Constant(c1)),
                )),
                // _3 = (f64)_2   (IntToFloat)
                st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Cast(CastKind::IntToFloat, Operand::Copy(loc(2)), f64t),
                )),
                // _4 = (u32)_3   (saturating FloatToInt)
                st(StatementKind::Assign(
                    loc(4),
                    Rvalue::Cast(CastKind::FloatToInt, Operand::Copy(loc(3)), u32t),
                )),
            ],
            tk(TerminatorKind::Return),
        )],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    let it = m.instances[id].item;
    m.items[it].linkage = regseq_model::Linkage::NoMangle;

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("regseq_f64_to_uint32_t("),
        "saturating cast helper:\n{tu}"
    );
    assert!(tu.contains("<< (("), "masked shift:\n{tu}");
    let run = assert_runs(&tu, "regseq_main", "cast-binop");
    assert_eq!(
        run.exit_code,
        Some(0),
        "cast/binop body runs: {}",
        run.stderr
    );
}

#[test]
fn float_constant_static_initializer_compiles() {
    // A `const double X = <hex-float>;` static -- the section 12 hex-float
    // constant expression must be valid C.
    let mut m = target_model();
    let f64t = m.types.intern(Ty::Float(FloatKind::F64));
    let init = m
        .consts
        .intern(Const::Float(FloatKind::F64, 1.5f64.to_bits() as u128));
    let it = mk_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "PI_ISH",
    );
    let emission = m.strings.intern("PI_ISH");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: f64t,
            init: Some(init),
        },
    });

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("const double PI_ISH = 0x1.8p0;"),
        "hex-float init:\n{tu}"
    );
    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = assert_runs(&tu, "regseq_main", "float-const");
    assert_eq!(run.exit_code, Some(0));
}

#[test]
fn foreign_and_never_linkage_compile() {
    // section 17: a Foreign extern declaration (decl only) + a `_Noreturn`
    // function that calls the foreign one divergently. The clean compile
    // proves the linkage spelling is valid C.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let never_ty = m.types.intern(Ty::Never);

    let dsig = fnsig(&mut m, vec![], never_ty, FnAbi::C);
    let diverge = fn_instance(&mut m, "host_abort", dsig, None);
    let dit = m.instances[diverge].item;
    m.items[dit].linkage = regseq_model::Linkage::Foreign;

    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    let body = build_body(
        0,
        vec![unit],
        vec![bb(
            vec![],
            tk(TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(diverge),
                args: Box::new([]),
                destination: loc(0),
                target: None,
            }),
        )],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    let it = m.instances[id].item;
    m.items[it].linkage = regseq_model::Linkage::NoMangle;

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("extern _Noreturn void host_abort(void);"),
        "foreign noreturn:\n{tu}"
    );
    assert!(
        tu.contains("host_abort(); __builtin_unreachable();"),
        "divergent call:\n{tu}"
    );
    // Provide a definition for the foreign abort callback so it links + runs.
    let tu = format!("{tu}\n_Noreturn void host_abort(void) {{ exit(0); }}\n");
    let run = assert_runs(&tu, "regseq_main", "foreign-noreturn");
    assert_eq!(run.exit_code, Some(0), "host_abort exits 0: {}", run.stderr);
}

#[test]
fn with_overflow_result_computes_both_components_at_runtime() {
    // Change G compile-and-run: `regseq_main` computes `AddWithOverflow(2, 3)`
    // into a `(u32, bool)` tuple and aborts (via the foreign fail callback) if
    // either component is wrong -- `_0` must be 5 and `_1` (overflow) must be
    // false. A clean exit proves the prelude helper + the two field writes
    // produced the right `(T, bool)` at runtime. `overflowing_add` itself is a
    // core method (a body-less extern that would not link), so the
    // `*WithOverflow` op is hand-built.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u32t = uint(&mut m, UintKind::U32);
    let boolt = m.types.intern(Ty::Bool);
    // tuple (u32, bool): _0 (u32) @0, _1 (bool) @4, size 8 align 4.
    let f0 = FieldLayout {
        name: m.strings.intern("_0"),
        ty: u32t,
        offset: 0,
    };
    let f1 = FieldLayout {
        name: m.strings.intern("_1"),
        ty: boolt,
        offset: 4,
    };
    let tup = m.types.intern(Ty::Tuple {
        fields: vec![u32t, boolt],
        layout: Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![f0, f1],
            },
        },
    });

    // Foreign abort callback `regseq_test_fail()`.
    let fail_sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    let fail = fn_instance(&mut m, "regseq_test_fail", fail_sig, None);
    let fit = m.instances[fail].item;
    m.items[fit].linkage = regseq_model::Linkage::Foreign;

    let c2 = m.consts.intern(Const::Uint(UintKind::U32, 2));
    let c3 = m.consts.intern(Const::Uint(UintKind::U32, 3));
    let c5 = m.consts.intern(Const::Uint(UintKind::U32, 5));
    let f0p = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(0), u32t)]),
    };
    let f1p = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), boolt)]),
    };
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit (ret), _1 (u32, bool), _2 bool.
    let body = build_body(
        0,
        vec![unit, tup, boolt],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::BinaryOp(
                            BinOp::AddWithOverflow,
                            Operand::Constant(c2),
                            Operand::Constant(c3),
                        ),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::BinaryOp(BinOp::Eq, Operand::Copy(f0p), Operand::Constant(c5)),
                    )),
                ],
                // if `_0 != 5` (the Eq is false / 0) -> fail (bb3), else bb1.
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(1),
                    },
                }),
            ),
            bb(
                vec![],
                // if `overflow` (the bool is 1) -> fail (bb3), else bb2 (ok).
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(f1p),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    let it = m.instances[id].item;
    m.items[it].linkage = regseq_model::Linkage::NoMangle;

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("regseq_add_uint32_t("),
        "the *WithOverflow helper is wired:\n{tu}"
    );
    let fail_name = fn_cname(&m, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail_name}(void) {{ fprintf(stderr, \"wrong\\n\"); abort(); }}\n");
    let run = assert_runs(&tu, "regseq_main", "with-overflow");
    assert_eq!(
        run.exit_code,
        Some(0),
        "both (T, bool) components correct -> exit 0; stderr: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// Task d2.1: section 13 enum tag-ops (compile-AND-run). A `void regseq_main`
// constructs + matches enum values and aborts (via a Foreign fail callback)
// on a wrong discriminant / payload; a clean exit (code 0) proves the tag
// read/write + payload access reproduced rustc's discriminant semantics, and
// the section 13 `_Static_assert`s guard the layout at compile time. The
// hand-built shapes (Niche, repr(int) explicit discriminants) the bridge
// cannot easily surface follow the c1/d2 precedent; the bridged Direct
// fixture exercises the full bridge -> emit -> run pipeline.
// ---------------------------------------------------------------------------

fn enum_layout(size: u64, align: u64, shape: EnumShape) -> Layout {
    Layout {
        size,
        align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum { shape },
    }
}

fn variant(
    m: &mut RegseqModel,
    name: &str,
    fields: Vec<FieldLayout>,
    discriminant: u128,
    tag_value: Option<u128>,
) -> VariantLayout {
    VariantLayout {
        name: m.strings.intern(name),
        fields,
        uninhabited: false,
        discriminant,
        tag_value,
    }
}

fn fl(m: &mut RegseqModel, name: &str, ty: TyId, offset: u64) -> FieldLayout {
    FieldLayout {
        name: m.strings.intern(name),
        ty,
        offset,
    }
}

fn push_enum_instance(m: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = mk_item(m, ItemKind::Enum, rendered);
    let ty = m.types.intern(Ty::Enum {
        item: it,
        args: GenericArgs::new(),
        layout,
    });
    let emission = m.strings.intern(rendered);
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Enum { layout: ty },
    });
    ty
}

/// A Foreign `regseq_test_fail()` callback instance (the abort hook),
/// returning its `InstanceId` so a `Call` terminator can target it.
fn push_fail_callback(m: &mut RegseqModel, unit: TyId) -> InstanceId {
    let fail_sig = fnsig(m, vec![], unit, FnAbi::C);
    let fail = fn_instance(m, "regseq_test_fail", fail_sig, None);
    let fit = m.instances[fail].item;
    m.items[fit].linkage = regseq_model::Linkage::Foreign;
    fail
}

fn nomangle(m: &mut RegseqModel, id: InstanceId) {
    let it = m.instances[id].item;
    m.items[it].linkage = regseq_model::Linkage::NoMangle;
}

/// The Foreign fail callback as a real abort, appended to the emitted TU.
fn with_fail_callback(tu: &str) -> String {
    format!("{tu}\nvoid regseq_test_fail(void) {{ fprintf(stderr, \"wrong\\n\"); abort(); }}\n")
}

#[test]
fn direct_enum_construct_match_runs_and_exits_zero() {
    // `enum AB { A, B(u32) }`: construct `B(42)`, read its discriminant (the
    // Direct ternary) -> 1, then its payload (Downcast+Field) -> 42; abort on
    // a wrong result. A clean exit proves construction + Discriminant +
    // Downcast/Field reproduced rustc's semantics.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
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
            variant(&mut m, "A", Vec::new(), 0, Some(0)),
            variant(&mut m, "B", vec![b0], 1, Some(1)),
        ],
    };
    let ab = push_enum_instance(&mut m, "AB", enum_layout(8, 4, shape));

    let fail_id = push_fail_callback(&mut m, unit);
    let fortytwo = m.consts.intern(Const::Uint(UintKind::U32, 42));
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 AB, _2 u8 (disc), _3 u32 (payload).
    let body = build_body(
        0,
        vec![unit, ab, u8t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(ab, VariantIdx::from_u32(1)),
                            Box::new([Operand::Constant(fortytwo)]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), u32t),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(42, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok for a d2.1 enum body");
    assert!(
        tu.contains("union {") && tu.contains("} _tag;"),
        "the section 13 union-of-views surfaced:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "direct-enum");
    assert_eq!(
        run.exit_code,
        Some(0),
        "Direct construct/match runs; stderr: {}",
        run.stderr
    );
}

#[test]
fn niche_enum_construct_match_runs_and_exits_zero() {
    // `Option<&u32>`: construct `Some(&x)` + `None`, read each discriminant
    // (the niche assign-site memcpy path) -> 1 / 0, read Some's payload
    // (`*(Some.0)`) -> 7; abort on a wrong result. A clean exit proves the
    // niche cell read/write + the untagged_variant fallback.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
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
    let opt = push_enum_instance(&mut m, "Opt", enum_layout(8, 8, shape));

    let fail_id = push_fail_callback(&mut m, unit);
    let seven = m.consts.intern(Const::Uint(UintKind::U32, 7));
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 u32 x, _2 &u32 r, _3 Opt some, _4 Opt none, _5 u8 dS,
    // _6 u8 dN, _7 &u32 ptr, _8 u32 val.
    let body = build_body(
        0,
        vec![unit, u32t, ref_ty, opt, opt, u8t, u8t, ref_ty, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(seven)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Ref(Provenance::SharedBorrow, loc(1)),
                    )),
                    st(StatementKind::Assign(
                        loc(3),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(opt, VariantIdx::from_u32(1)),
                            Box::new([Operand::Copy(loc(2))]),
                        ),
                    )),
                    st(StatementKind::Assign(
                        loc(4),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(opt, VariantIdx::from_u32(0)),
                            Box::new([]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(5), Rvalue::Discriminant(loc(3)))),
                    st(StatementKind::Assign(loc(6), Rvalue::Discriminant(loc(4)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(5)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(6)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(7),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(3),
                            projection: Box::new([
                                PlaceElem::Downcast(VariantIdx::from_u32(1)),
                                PlaceElem::Field(FieldIdx::from_u32(0), ref_ty),
                            ]),
                        })),
                    )),
                    st(StatementKind::Assign(
                        loc(8),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(7),
                            projection: Box::new([PlaceElem::Deref]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(8)),
                    targets: SwitchTargets {
                        cases: Box::new([(7, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(3)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok for a niche enum body");
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "niche-enum");
    assert_eq!(
        run.exit_code,
        Some(0),
        "niche construct/match runs; stderr: {}",
        run.stderr
    );
}

#[test]
fn niche_bool_construct_match_distinguishes_some_true_from_none() {
    // `Option<bool>`: untagged `Some(b)` is the bool byte at @0 (0 or 1);
    // niched `None` carries `tag_value = 2` (bool's first invalid bit pattern).
    // Construct `Some(true)` + `None`, read each Discriminant, and read Some's
    // bool payload via Downcast+Field; abort on a wrong result. Regression
    // for the d2.1 soundness-pass finding: before the raw-storage-integer
    // niche fix, `render_tag_value(Bool, 2) = "1"` and `(_Bool){2}` normalized
    // to 1, so `Some(true)` (byte 1) collided with `None` -- this fixture's
    // `Discriminant(Some(true)) == 1 (Some)` assertion catches that.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let boolt = m.types.intern(Ty::Bool);
    let some_field = fl(&mut m, "0", boolt, 0);
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
            variant(&mut m, "None", Vec::new(), 0, Some(2)),
            variant(&mut m, "Some", vec![some_field], 1, None),
        ],
    };
    let opt = push_enum_instance(&mut m, "OptBool", enum_layout(1, 1, shape));

    let fail_id = push_fail_callback(&mut m, unit);
    let true_c = m.consts.intern(Const::Bool(true));
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 bool true, _2 OptBool some, _3 OptBool none, _4 u8 dS,
    // _5 u8 dN, _6 bool payload.
    let body = build_body(
        0,
        vec![unit, boolt, opt, opt, u8t, u8t, boolt],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(true_c)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(opt, VariantIdx::from_u32(1)),
                            Box::new([Operand::Copy(loc(1))]),
                        ),
                    )),
                    st(StatementKind::Assign(
                        loc(3),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(opt, VariantIdx::from_u32(0)),
                            Box::new([]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(4), Rvalue::Discriminant(loc(2)))),
                    st(StatementKind::Assign(loc(5), Rvalue::Discriminant(loc(3)))),
                ],
                // Some(true) must discriminate to 1 (Some); the buggy path
                // returns 0 (None), falling through `otherwise` to fail.
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(4)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                // None must discriminate to 0 (None).
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(5)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(6),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(2),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), boolt),
                        ]),
                    })),
                ))],
                // Some's payload must read back as `true` (1).
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(6)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(3)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok for an Option<bool> body");
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "niche-bool");
    assert_eq!(
        run.exit_code,
        Some(0),
        "Option<bool>: Some(true) discriminates as Some (not None), None as None; stderr: {}",
        run.stderr
    );
}

#[test]
fn repr_int_enum_discriminant_maps_tag_to_explicit_value() {
    // `enum E { A = 10, B = 20 }` (fieldless Direct, explicit discriminants !=
    // storage index): construct `B`, read its discriminant -> 20 (NOT the
    // storage index 1); abort if the ternary returned the raw cell / index.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
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

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 E, _2 u8 (disc).
    let body = build_body(
        0,
        vec![unit, e_ty, u8t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(e_ty, VariantIdx::from_u32(1)),
                            Box::new([]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(20, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "repr-int-enum");
    assert_eq!(
        run.exit_code,
        Some(0),
        "discriminant maps tag 20 -> semantic 20, not index 1; stderr: {}",
        run.stderr
    );
}

/// A bridged Direct enum: `classify(&E)` matches `enum E { A, B(u32), C { x,
/// y } }` and `regseq_main` constructs each variant and checks the result.
fn enum_snippet(b_expected: u32) -> String {
    format!(
        r#"#![no_std]

extern "C" {{
    fn regseq_test_fail();
}}

pub enum E {{ A, B(u32), C {{ x: u8, y: u16 }} }}

#[no_mangle]
#[inline(never)]
pub extern "C" fn classify(e: &E) -> u32 {{
    match e {{
        E::A => 0,
        E::B(v) => *v,
        E::C {{ x, y }} => (*x as u32) + (*y as u32),
    }}
}}

#[no_mangle]
pub extern "C" fn regseq_main() {{
    let a = E::A;
    if classify(&a) != 0 {{ unsafe {{ regseq_test_fail(); }} }}
    let b = E::B({b_expected});
    if classify(&b) != {b_expected} {{ unsafe {{ regseq_test_fail(); }} }}
    let c = E::C {{ x: 3, y: 100 }};
    if classify(&c) != 103 {{ unsafe {{ regseq_test_fail(); }} }}
}}
"#
    )
}

#[test]
fn bridged_direct_enum_body_runs_and_exits_zero() {
    let snippet = TempSnippet::new("enum-ok", &enum_snippet(42));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for a bridged enum body");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("bridged enum TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "correct enum construct/match lowering exits 0; stderr: {}",
        run.stderr
    );
}

/// A bridged Direct enum whose `regseq_main` builds `B(42)` but checks
/// `classify(&b) != 999`, so the wrong-result branch always fires.
fn enum_wrong_snippet() -> String {
    r#"#![no_std]

extern "C" {
    fn regseq_test_fail();
}

pub enum E { A, B(u32), C { x: u8, y: u16 } }

#[no_mangle]
#[inline(never)]
pub extern "C" fn classify(e: &E) -> u32 {
    match e {
        E::A => 0,
        E::B(v) => *v,
        E::C { x, y } => (*x as u32) + (*y as u32),
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let b = E::B(42);
    if classify(&b) != 999 {
        unsafe { regseq_test_fail(); }
    }
}
"#
    .to_string()
}

#[test]
fn bridged_direct_enum_body_aborts_on_a_wrong_payload() {
    // The constructed payload (42) differs from the expected check (999), so
    // the wrong-result branch fires -- proving the enum construction +
    // Downcast/Field payload read computes the real value at runtime.
    let snippet = TempSnippet::new("enum-bad", &enum_wrong_snippet());
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok");
    let entry = fn_cname(&model, "regseq_main");
    let fail = fn_cname(&model, "regseq_test_fail");
    let tu =
        format!("{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("TU must compile + run");
    assert_ne!(run.exit_code, Some(0), "a wrong payload must abort");
    assert!(
        run.stderr.contains("wrong result"),
        "the abort path ran: {}",
        run.stderr
    );
}

#[test]
fn single_scalar_none_enum_construct_read_runs_and_exits_zero() {
    // `enum Wrap { W(u32) }` (Single { scalar: None }, collapsed inline to a
    // single-field struct): construct `W(7)`, read its discriminant -> 0 (the
    // constant) and its payload (Downcast+Field) -> 7; abort on a wrong result.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u32t = uint(&mut m, UintKind::U32);
    let w0 = fl(&mut m, "0", u32t, 0);
    let shape = EnumShape::Single {
        variant: variant(&mut m, "W", vec![w0], 0, None),
        scalar: None,
    };
    let wrap = push_enum_instance(&mut m, "Wrap", enum_layout(4, 4, shape));

    let fail_id = push_fail_callback(&mut m, unit);
    let seven = m.consts.intern(Const::Uint(UintKind::U32, 7));
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 Wrap, _2 u32 (disc), _3 u32 (payload).
    let body = build_body(
        0,
        vec![unit, wrap, u32t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(wrap, VariantIdx::from_u32(0)),
                            Box::new([Operand::Constant(seven)]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(0)),
                            PlaceElem::Field(FieldIdx::from_u32(0), u32t),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(7, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "single-scalar-none");
    assert_eq!(
        run.exit_code,
        Some(0),
        "Single{{scalar:None}} construct/read runs; stderr: {}",
        run.stderr
    );
}

#[test]
fn single_scalar_some_enum_discriminant_runs_and_exits_zero() {
    // `enum Only { A = 5 }` (repr(u8), Single { scalar: Some }, inline tag-only
    // struct): construct `A`, read its discriminant -> 5 (the explicit value,
    // not the storage index 0); abort on a wrong result.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
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
    let only = push_enum_instance(&mut m, "Only", enum_layout(1, 1, shape));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 Only, _2 u8 (disc).
    let body = build_body(
        0,
        vec![unit, only, u8t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Aggregate(
                            AggregateKind::Enum(only, VariantIdx::from_u32(0)),
                            Box::new([]),
                        ),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(5, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "single-scalar-some");
    assert_eq!(
        run.exit_code,
        Some(0),
        "Single{{scalar:Some}} discriminant runs; stderr: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// Task d2.2 (Changes A-F): non-scalar const lowering (compile-AND-run). A
// `void regseq_main` reads back a reconstructed const and aborts (via the
// Foreign fail callback) on a wrong value; a clean exit (code 0) proves the
// byte-image / carrier / `_raw` lowering reproduced the value at runtime.
// ---------------------------------------------------------------------------

#[test]
fn array_reference_const_reads_re_typed_element_runs_and_exits_zero() {
    // Change C + F + B: `&[u32; 4]` carrier to a re-typed `ConstMemoryStatic`
    // (`uint32_t[4]`, sound + aligned). `regseq_main` reads `(*R)[2]` -> 30 and
    // aborts on a wrong value.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));

    // ConstMemoryStatic holding [10, 20, 30, 40] u32 LE = 16 bytes.
    let mut bytes = Vec::new();
    for v in [10u32, 20, 30, 40] {
        bytes.extend(
            v.to_le_bytes()
                .iter()
                .map(|b| regseq_model::Byte::Value(*b)),
        );
    }
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms_item = mk_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "CMS_ARR",
    );
    let cms_em = m.strings.intern("CMS_ARR");
    let cms = m.instances.push(Instance {
        item: cms_item,
        generic_args: GenericArgs::new(),
        emission_name: cms_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: arr_u8,
            init: Some(cms_bytes),
        },
    });
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 &[u32;4], _2 u32 (the read element).
    let body = build_body(
        0,
        vec![unit, ref_arr, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(carrier)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([
                                PlaceElem::Deref,
                                PlaceElem::ConstantIndex {
                                    offset: 2,
                                    min_length: 4,
                                    from_end: false,
                                },
                            ]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(30, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("uint32_t CMS_ARR[4] ="),
        "the CMS is re-typed to uint32_t[4], not uint8_t[16]:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "array-ref-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "(*R)[2] == 30 over the re-typed CMS; stderr: {}",
        run.stderr
    );
}

#[test]
fn array_slot_reference_const_through_indexed_place_is_re_typed() {
    // Change F SOUNDNESS REGRESSION (d2.2 close): a `&[u32; 4]` carrier to a
    // `ConstMemoryStatic` assigned through a NON-`Field` projection (an array
    // element `_1[0]`, a `ConstantIndex` place). The Change-F scan must narrow
    // that destination via the SAME walk the body emitter lowers the const
    // against (`body_emit::place_ty`); the old `place_consumer_ty` shortcut
    // returned `None` for `ConstantIndex` -> the CMS was missed, stayed the
    // bridge `uint8_t[16]`, and `regseq_main` read it through a
    // `(const uint32_t (*)[4])` cast = strict-aliasing/alignment UB. This test
    // pins the re-typing (`uint32_t CMS_IDX[4]`, not `uint8_t[16]`) AND a clean
    // run; it FAILS the structural assert without the fix.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr_u32 = m.types.intern(Ty::Array(u32t, 4));
    let ref_arr = m.types.intern(Ty::Ref(arr_u32, Provenance::SharedBorrow));
    // The assignment destination: a 1-element array of the reference.
    let arr_ref = m.types.intern(Ty::Array(ref_arr, 1));

    let mut bytes = Vec::new();
    for v in [10u32, 20, 30, 40] {
        bytes.extend(
            v.to_le_bytes()
                .iter()
                .map(|b| regseq_model::Byte::Value(*b)),
        );
    }
    let cms_bytes = m.consts.intern(Const::Bytes(bytes));
    let arr_u8 = m.types.intern(Ty::Array(u8t, 16));
    let cms_item = mk_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "CMS_IDX",
    );
    let cms_em = m.strings.intern("CMS_IDX");
    let cms = m.instances.push(Instance {
        item: cms_item,
        generic_args: GenericArgs::new(),
        emission_name: cms_em,
        span: span(),
        kind: InstanceKind::Static {
            ty: arr_u8,
            init: Some(cms_bytes),
        },
    });
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms)));
    let meta = m.consts.intern(Const::Uint(UintKind::Usize, 0));
    let carrier = m.consts.intern(Const::Aggregate(vec![reloc, meta]));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 [&[u32;4]; 1], _2 u32 (the read element).
    let elem0 = PlaceElem::ConstantIndex {
        offset: 0,
        min_length: 1,
        from_end: false,
    };
    let body = build_body(
        0,
        vec![unit, arr_ref, u32t],
        vec![
            bb(
                vec![
                    // _1[0] = carrier  (the non-Field-projection destination).
                    st(StatementKind::Assign(
                        Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([elem0.clone()]),
                        },
                        Rvalue::Use(Operand::Constant(carrier)),
                    )),
                    // _2 = (*_1[0])[2]
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([
                                elem0,
                                PlaceElem::Deref,
                                PlaceElem::ConstantIndex {
                                    offset: 2,
                                    min_length: 4,
                                    from_end: false,
                                },
                            ]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(30, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("uint32_t CMS_IDX[4] ="),
        "the CMS reached through an indexed place is re-typed to uint32_t[4], \
         not uint8_t[16]:\n{tu}"
    );
    assert!(
        !tu.contains("uint8_t CMS_IDX[16]"),
        "the CMS must NOT stay the bridge uint8_t[16] (the soundness bug):\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "array-slot-ref-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "(*_1[0])[2] == 30 over the re-typed CMS; stderr: {}",
        run.stderr
    );
}

#[test]
fn multiview_direct_enum_const_match_runs_and_exits_zero() {
    // Change E: a payload-bearing multi-view `Const::EnumValue` serializes to
    // the `_raw` byte image; `regseq_main` reads the discriminant (d2.1 tag
    // view) -> 1 and the payload (Downcast+Field) -> 42 and aborts on a wrong
    // value. Proves the `_raw` init + the in-place union reads agree.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
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
            variant(&mut m, "A", Vec::new(), 0, Some(0)),
            variant(&mut m, "B", vec![b0], 1, Some(1)),
        ],
    };
    let ab = push_enum_instance(&mut m, "AB", enum_layout(8, 4, shape));
    let fortytwo = m.consts.intern(Const::Uint(UintKind::U32, 42));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![fortytwo],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 AB (the const), _2 u8 (disc), _3 u32 (payload).
    let body = build_body(
        0,
        vec![unit, ab, u8t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(ev)),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), u32t),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(42, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains(".u._raw = { 1, 0, 0, 0, 42, 0, 0, 0 }"),
        "the payload-bearing enum const serializes to the _raw byte image:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "multiview-enum-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "_raw const init + in-place union reads agree; stderr: {}",
        run.stderr
    );
}

#[test]
fn typed_valtree_struct_const_field_read_runs_and_exits_zero() {
    // Change D: a struct valtree `Const::Aggregate` reconstructs as a designated
    // compound literal; `regseq_main` reads `p.b` -> 9 and aborts on a wrong
    // value. `Pt { a: u8 @4, b: u32 @0 }` (repr(Rust) reordered).
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let a = fl(&mut m, "a", u8t, 4);
    let b = fl(&mut m, "b", u32t, 0);
    let pt = {
        let it = mk_item(&mut m, ItemKind::Struct, "Pt");
        m.types.intern(Ty::Struct {
            item: it,
            args: GenericArgs::new(),
            layout: Layout {
                size: 8,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct { fields: vec![a, b] },
            },
        })
    };
    let ca = m.consts.intern(Const::Uint(UintKind::U8, 7));
    let cb = m.consts.intern(Const::Uint(UintKind::U32, 9));
    let agg = m.consts.intern(Const::Aggregate(vec![ca, cb]));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 Pt (the const), _2 u32 (the read field b).
    let body = build_body(
        0,
        vec![unit, pt, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(agg)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(1), u32t)]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(9, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("(struct Pt){ .a = ((uint8_t)(7U)), .b = ((uint32_t)(9U)) }"),
        "struct valtree designated literal:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "struct-valtree-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "p.b == 9 over the reconstructed struct const; stderr: {}",
        run.stderr
    );
}

#[test]
fn tuple_static_byte_image_compiles_and_runs() {
    // Change B: a `(u32, u64)` tuple `static` arrives as `Const::Bytes`; the
    // byte image reconstructs as a typed compound literal whose section 14
    // `_Static_assert`s hold at compile time (a wrong field offset fails `cc`).
    let mut m = target_model();
    let u32t = uint(&mut m, UintKind::U32);
    let u64t = uint(&mut m, UintKind::U64);
    // (u32 @0, u64 @8), size 16 align 8.
    let f0 = fl(&mut m, "_0", u32t, 0);
    let f1 = fl(&mut m, "_1", u64t, 8);
    let tup = m.types.intern(Ty::Tuple {
        fields: vec![u32t, u64t],
        layout: Layout {
            size: 16,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![f0, f1],
            },
        },
    });
    // Bytes: u32 1 @0 (+ 4 pad), u64 2 @8.
    let mut bytes = Vec::new();
    bytes.extend(
        1u32.to_le_bytes()
            .iter()
            .map(|b| regseq_model::Byte::Value(*b)),
    );
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Uninit, 4));
    bytes.extend(
        2u64.to_le_bytes()
            .iter()
            .map(|b| regseq_model::Byte::Value(*b)),
    );
    let init = m.consts.intern(Const::Bytes(bytes));
    let it = mk_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "PAIR",
    );
    let em = m.strings.intern("PAIR");
    m.instances.push(Instance {
        item: it,
        generic_args: GenericArgs::new(),
        emission_name: em,
        span: span(),
        kind: InstanceKind::Static {
            ty: tup,
            init: Some(init),
        },
    });

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("._0 = ((uint32_t)(1U))") && tu.contains("._1 = ((uint64_t)(2ULL))"),
        "tuple byte image reconstructs both fields:\n{tu}"
    );
    let tu = format!("{tu}\nvoid regseq_main(void) {{}}\n");
    let run = assert_runs(&tu, "regseq_main", "tuple-static");
    assert_eq!(run.exit_code, Some(0));
}

// ---- task d2.3a const carve-outs: compile-AND-run (B2) -----------------------

/// Push a `static <name>: <ty> = <init>;` instance, returning its `InstanceId`
/// (so a `Const::Reloc` can target it).
fn push_static(m: &mut RegseqModel, name: &str, ty: TyId, init: Const) -> InstanceId {
    let it = mk_item(
        m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        name,
    );
    let init = m.consts.intern(init);
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

#[test]
fn union_static_member_read_runs_and_exits_zero() {
    // #1 (Changes A + B): `#[repr(align(8))] union U { x: u32 }` (size 8, align 8
    // -- the largest member u32 spans 4 < 8, so the old code emitted a `_pad`
    // filler; the new `_raw[8]` pins sizeof). `static U: U = { x: 0xAB }`; read
    // `(*&U).x` -> 0xAB and abort on a wrong value. Proves the union `_raw` byte
    // image + the in-place member type-pun agree under the pinned gnu11 flags.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u32t = uint(&mut m, UintKind::U32);
    let x = fl(&mut m, "x", u32t, 0);
    let uit = mk_item(&mut m, ItemKind::Union, "U");
    let union_ty = m.types.intern(Ty::Union {
        item: uit,
        args: GenericArgs::new(),
        layout: Layout {
            size: 8,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union { fields: vec![x] },
        },
    });
    m.instances.push(Instance {
        item: uit,
        generic_args: GenericArgs::new(),
        emission_name: m.strings.intern("U"),
        span: span(),
        kind: InstanceKind::Union { layout: union_ty },
    });
    let ref_union = m.types.intern(Ty::Ref(union_ty, Provenance::SharedBorrow));
    // static U: 8 bytes, x = 0xAB @0, padding 0.
    let mut bytes = vec![regseq_model::Byte::Value(0xAB)];
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Value(0), 7));
    let u_static = push_static(&mut m, "U", union_ty, Const::Bytes(bytes));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(u_static)));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 &U, _2 u32 (the read member).
    let body = build_body(
        0,
        vec![unit, ref_union, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(reloc)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([
                                PlaceElem::Deref,
                                PlaceElem::Field(FieldIdx::from_u32(0), u32t),
                            ]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(0xAB, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("unsigned char _raw[8];"),
        "the union carries a byte-covering _raw member:\n{tu}"
    );
    assert!(
        tu.contains("(union U){ ._raw = {"),
        "the union static initializes via the _raw byte image:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "union-static");
    assert_eq!(
        run.exit_code,
        Some(0),
        "U.x == 0xAB type-punned through _raw; stderr: {}",
        run.stderr
    );
}

#[test]
fn multiview_enum_const_array_payload_runs_and_exits_zero() {
    // #2-noreloc (Change C): `enum E { A, B([u32; 2]) }`, `const X = E::B([10, 20])`;
    // the no-relocation array payload serializes into `_raw` recursively.
    // `regseq_main` reads the discriminant -> 1, then both elements
    // `(X as B).0[0]` -> 10 and `[1]` -> 20, aborting on a wrong tag / element.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let arr = m.types.intern(Ty::Array(u32t, 2));
    let b0 = fl(&mut m, "0", arr, 4);
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
    let e_ty = push_enum_instance(&mut m, "EArr", enum_layout(12, 4, shape));
    let ten = m.consts.intern(Const::Uint(UintKind::U32, 10));
    let twenty = m.consts.intern(Const::Uint(UintKind::U32, 20));
    let arr_const = m.consts.intern(Const::Aggregate(vec![ten, twenty]));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![arr_const],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    let elem = |offset: u64| Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Downcast(VariantIdx::from_u32(1)),
            PlaceElem::Field(FieldIdx::from_u32(0), arr),
            PlaceElem::ConstantIndex {
                offset,
                min_length: 2,
                from_end: false,
            },
        ]),
    };
    // _0 unit, _1 EArr (the const), _2 u8 (disc), _3 u32 (elem read).
    let body = build_body(
        0,
        vec![unit, e_ty, u8t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(ev)),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(elem(0))),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(10, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(elem(1))),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(20, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(3)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains(".u._raw = { 1, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0 }"),
        "the array payload serializes into _raw at the right offsets:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "multiview-array-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "B([10, 20]) tag + both elements read back; stderr: {}",
        run.stderr
    );
}

#[test]
fn multiview_niche_untagged_reloc_const_runs_and_exits_zero() {
    // M1-lite (Change D): a multi-view niche enum `enum NU { A(&u32), B(u32) }`
    // (A untagged, B niched with its own non-overlapping u32 @8), `const X =
    // NU::A(&Y)`. The relocation payload routes through the existing
    // `u._v0._0` typed view (no `_raw`, no double-init). `regseq_main` reads the
    // pointee `*((X as A).0)` -> 7 and aborts on a wrong value.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
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
        variants: vec![
            variant(&mut m, "A", vec![a0], 0, None),
            variant(&mut m, "B", vec![b0], 1, Some(0)),
        ],
    };
    let nu = push_enum_instance(&mut m, "NU", enum_layout(16, 8, shape));
    let y = push_static(&mut m, "Y", u32t, Const::Uint(UintKind::U32, 7));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(y)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![reloc],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 NU (the const), _2 u32 (the read pointee).
    let body = build_body(
        0,
        vec![unit, nu, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(ev)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Copy(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([
                                PlaceElem::Downcast(VariantIdx::from_u32(0)),
                                PlaceElem::Field(FieldIdx::from_u32(0), ref_ty),
                                PlaceElem::Deref,
                            ]),
                        })),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(7, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(2),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(1)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains(".u._v0._0 = "),
        "the niche-untagged reloc routes through the typed view _v0._0:\n{tu}"
    );
    assert!(
        !tu.contains("(struct NU){ .u._raw"),
        "no _raw byte image for the niche-untagged route:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "niche-untagged-reloc-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "*((X as A).0) == 7 via the typed view; stderr: {}",
        run.stderr
    );
}

// ---- task d2.3b const full closure: compile-AND-run (B2) ---------------------

/// Build `enum E { A(u8), B(&u32) }` (tag u8 @0, A.0 u8 @1, B.0 &u32 @8, size 16
/// align 8) -- a multi-view Direct enum whose B variant carries a relocation.
/// Returns `(enum_ty, &u32 ref ty)`.
fn push_ab_ref_enum(m: &mut RegseqModel) -> (TyId, TyId) {
    let u8t = uint(m, UintKind::U8);
    let u32t = uint(m, UintKind::U32);
    let ref_ty = m.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
    let a0 = fl(m, "0", u8t, 1);
    let b0 = fl(m, "0", ref_ty, 8);
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
    let e_ty = push_enum_instance(m, "E", enum_layout(16, 8, shape));
    (e_ty, ref_ty)
}

#[test]
fn direct_reloc_enum_const_via_typed_view_runs_and_exits_zero() {
    // #2-reloc EnumValue form (Change C/D): `const X: E = E::B(&Y)` routes the
    // tag + reloc through the `_c1` typed full view. `regseq_main` reads the
    // discriminant -> 1 (B) and the pointee `*((X as B).0)` -> 7, aborting on a
    // wrong tag / value. Proves the reloc lands in a TYPED pointer slot at
    // static-init and the runtime views read it back.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let (e_ty, ref_ty) = push_ab_ref_enum(&mut m);
    let y = push_static(&mut m, "Y", u32t, Const::Uint(UintKind::U32, 7));
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(y)));
    let ev = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![reloc],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 E (the const), _2 u8 (disc), _3 u32 (the read pointee).
    let body = build_body(
        0,
        vec![unit, e_ty, u8t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(ev)),
                    )),
                    st(StatementKind::Assign(loc(2), Rvalue::Discriminant(loc(1)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), ref_ty),
                            PlaceElem::Deref,
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(7, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains(".u._c1 = { .tag = ((uint8_t)(1U)), ._0 = ((const uint32_t *)&Y) }"),
        "the EnumValue reloc const routes through the _c1 typed view:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "direct-reloc-enum-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "disc == B and *((X as B).0) == 7 via the _c1 typed view; stderr: {}",
        run.stderr
    );
}

#[test]
fn direct_reloc_enum_static_via_typed_view_runs_and_exits_zero() {
    // #2-reloc Bytes form (Change D): `static X: E = E::B(&Y)` arrives as
    // `Const::Bytes` with a `Byte::Reloc` in the pointer slot. The bytes path
    // decodes the Direct tag (-> B) and routes the reloc through `_c1`.
    // `regseq_main` reads `&X`, the discriminant -> 1 and `*((*X as B).0)` -> 9.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let u32t = uint(&mut m, UintKind::U32);
    let (e_ty, ref_ty) = push_ab_ref_enum(&mut m);
    let ref_e = m.types.intern(Ty::Ref(e_ty, Provenance::SharedBorrow));
    let y = push_static(&mut m, "Y", u32t, Const::Uint(UintKind::U32, 9));
    let r = m.instance_ref_of(y);
    // tag 1 @0, 7 padding bytes, an 8-byte reloc run @8 (the &Y pointer slot).
    let mut bytes = vec![regseq_model::Byte::Value(1)];
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Value(0), 7));
    bytes.extend(std::iter::repeat_n(regseq_model::Byte::Reloc(r), 8));
    let x = push_static(&mut m, "X", e_ty, Const::Bytes(bytes));
    let xref = m.consts.intern(Const::Reloc(m.instance_ref_of(x)));

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 &E, _2 u8 (disc), _3 u32 (pointee).
    let body = build_body(
        0,
        vec![unit, ref_e, u8t, u32t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(xref)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Discriminant(Place {
                            local: LocalId::from_u32(1),
                            projection: Box::new([PlaceElem::Deref]),
                        }),
                    )),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(2)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(3),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Deref,
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), ref_ty),
                            PlaceElem::Deref,
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(9, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(3),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(2)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains(".u._c1 = { .tag = ((uint8_t)(1U)), ._0 = ((const uint32_t *)&Y) }"),
        "the static bytes-with-reloc form routes through the _c1 typed view:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "direct-reloc-enum-static");
    assert_eq!(
        run.exit_code,
        Some(0),
        "disc == B and *((*X as B).0) == 9 via the _c1 typed view; stderr: {}",
        run.stderr
    );
}

#[test]
fn inline_bool_niche_const_match_runs_and_exits_zero() {
    // #3 (Change A/B): `const N: Option<bool> = None;` lowers via the inline
    // anonymous-union `_raw` (niche byte 2), and `const S = Some(true)` via the
    // typed single view. `regseq_main` matches Discriminant(N) -> 0 (None),
    // Discriminant(S) -> 1 (Some), and reads Some's bool -> 1. Proves byte 2
    // round-trips where `(_Bool){2}` could not.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let boolt = m.types.intern(Ty::Bool);
    let some_field = fl(&mut m, "0", boolt, 0);
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
            variant(&mut m, "None", Vec::new(), 0, Some(2)),
            variant(&mut m, "Some", vec![some_field], 1, None),
        ],
    };
    let opt = push_enum_instance(&mut m, "OptBool", enum_layout(1, 1, shape));
    let none = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let true_c = m.consts.intern(Const::Bool(true));
    let some = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![true_c],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 None, _2 Some, _3 dN, _4 dS, _5 bool payload.
    let body = build_body(
        0,
        vec![unit, opt, opt, u8t, u8t, boolt],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(none)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Constant(some)),
                    )),
                    st(StatementKind::Assign(loc(3), Rvalue::Discriminant(loc(1)))),
                    st(StatementKind::Assign(loc(4), Rvalue::Discriminant(loc(2)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(4)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(5),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(2),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), boolt),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(5)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(3)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("(struct OptBool){ ._raw = { 2 } }"),
        "None serializes byte 2 into the inline _raw (no normalization):\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "inline-bool-niche-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "None disc 0 (byte 2), Some(true) disc 1 + bool 1; stderr: {}",
        run.stderr
    );
}

#[test]
fn inline_nested_nonbool_niche_const_match_runs_and_exits_zero() {
    // #4 non-bool nested (Change A/B): an inline niche enum `Option<(u8, u8)>`
    // shape whose niche cell is the SECOND tuple byte @1 (a non-bool nested
    // cell). `const N = None` (niche value 0) lowers via `_raw`; `const S =
    // Some((5, 7))` via the typed single view. `regseq_main` matches
    // Discriminant(N) -> 0, Discriminant(S) -> 1, and reads Some's tuple .1 -> 7.
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let t0 = fl(&mut m, "_0", u8t, 0);
    let t1 = fl(&mut m, "_1", u8t, 1);
    let tup = m.types.intern(Ty::Tuple {
        fields: vec![u8t, u8t],
        layout: Layout {
            size: 2,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![t0, t1],
            },
        },
    });
    let some_field = fl(&mut m, "0", tup, 0);
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
            variant(&mut m, "None", Vec::new(), 0, Some(0)),
            variant(&mut m, "Some", vec![some_field], 1, None),
        ],
    };
    let opt = push_enum_instance(&mut m, "OptT", enum_layout(2, 1, shape));
    let none = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![],
    });
    let five = m.consts.intern(Const::Uint(UintKind::U8, 5));
    let seven = m.consts.intern(Const::Uint(UintKind::U8, 7));
    let pair = m.consts.intern(Const::Aggregate(vec![five, seven]));
    let some = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![pair],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 None, _2 Some, _3 dN, _4 dS, _5 u8 (tuple .1).
    let body = build_body(
        0,
        vec![unit, opt, opt, u8t, u8t, u8t],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(none)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Constant(some)),
                    )),
                    st(StatementKind::Assign(loc(3), Rvalue::Discriminant(loc(1)))),
                    st(StatementKind::Assign(loc(4), Rvalue::Discriminant(loc(2)))),
                ],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(4)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(5),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(2),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(1)),
                            PlaceElem::Field(FieldIdx::from_u32(0), tup),
                            PlaceElem::Field(FieldIdx::from_u32(1), u8t),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(5)),
                    targets: SwitchTargets {
                        cases: Box::new([(7, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(3)),
                }),
            ),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("(struct OptT){ ._raw = { 0, 0 } }"),
        "None serializes the nested niche value into _raw:\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "inline-nested-nonbool-niche-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "None disc 0, Some((5,7)) disc 1 + tuple.1 == 7; stderr: {}",
        run.stderr
    );
}

#[test]
fn inline_nested_bool_niche_const_match_runs_and_exits_zero() {
    // #4 nested `_Bool` (the M2-a-IMPOSSIBLE case, Change A/B): `enum E {
    // A(struct S { x: u8, b: bool }), B }` with the niche in the NESTED bool
    // `S.b` @1. `const NB = B` (niche byte 2 @1) lowers via `_raw`; `const NA =
    // A(S { x: 5, b: true })` via the typed view. `regseq_main` matches
    // Discriminant(NB) -> 1 (B) and Discriminant(NA) -> 0 (A) + reads NA's x ->
    // 5 and b -> 1. The `_raw` byte image writes byte 2 at the bool's offset
    // with NO normalization (`(_Bool){2}` would have collided NB with A).
    let mut m = target_model();
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: regseq_model::UNIT_LAYOUT,
    });
    let u8t = uint(&mut m, UintKind::U8);
    let boolt = m.types.intern(Ty::Bool);
    // struct S { x: u8 @0, b: bool @1 } -- size 2 align 1.
    let sx = fl(&mut m, "x", u8t, 0);
    let sb = fl(&mut m, "b", boolt, 1);
    let s_ty = {
        let it = mk_item(&mut m, ItemKind::Struct, "S");
        m.types.intern(Ty::Struct {
            item: it,
            args: GenericArgs::new(),
            layout: Layout {
                size: 2,
                align: 1,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct {
                    fields: vec![sx, sb],
                },
            },
        })
    };
    let a0 = fl(&mut m, "0", s_ty, 0);
    let tag = TagScalar {
        offset: 1,
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
            variant(&mut m, "A", vec![a0], 0, None),
            variant(&mut m, "B", Vec::new(), 1, Some(2)),
        ],
    };
    let e_ty = push_enum_instance(&mut m, "EBN", enum_layout(2, 1, shape));
    let nb = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(1),
        payload: vec![],
    });
    let five = m.consts.intern(Const::Uint(UintKind::U8, 5));
    let true_c = m.consts.intern(Const::Bool(true));
    let s_const = m.consts.intern(Const::Aggregate(vec![five, true_c]));
    let na = m.consts.intern(Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![s_const],
    });

    let fail_id = push_fail_callback(&mut m, unit);
    let sig = fnsig(&mut m, vec![], unit, FnAbi::C);
    // _0 unit, _1 NB (=B), _2 NA (=A), _3 dB, _4 dA, _5 u8 x, _6 bool b.
    let body = build_body(
        0,
        vec![unit, e_ty, e_ty, u8t, u8t, u8t, boolt],
        vec![
            bb(
                vec![
                    st(StatementKind::Assign(
                        loc(1),
                        Rvalue::Use(Operand::Constant(nb)),
                    )),
                    st(StatementKind::Assign(
                        loc(2),
                        Rvalue::Use(Operand::Constant(na)),
                    )),
                    st(StatementKind::Assign(loc(3), Rvalue::Discriminant(loc(1)))),
                    st(StatementKind::Assign(loc(4), Rvalue::Discriminant(loc(2)))),
                ],
                // NB must discriminate to 1 (B); the buggy normalized path (byte
                // 2 -> 1) would mis-resolve to A (0) and fall through to fail.
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(3)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(1))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(5),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(2),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(0)),
                            PlaceElem::Field(FieldIdx::from_u32(0), s_ty),
                            PlaceElem::Field(FieldIdx::from_u32(0), u8t),
                        ]),
                    })),
                ))],
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(4)),
                    targets: SwitchTargets {
                        cases: Box::new([(0, BlockId::from_u32(2))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![st(StatementKind::Assign(
                    loc(6),
                    Rvalue::Use(Operand::Copy(Place {
                        local: LocalId::from_u32(2),
                        projection: Box::new([
                            PlaceElem::Downcast(VariantIdx::from_u32(0)),
                            PlaceElem::Field(FieldIdx::from_u32(0), s_ty),
                            PlaceElem::Field(FieldIdx::from_u32(1), boolt),
                        ]),
                    })),
                ))],
                // NA's x must read 5.
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(5)),
                    targets: SwitchTargets {
                        cases: Box::new([(5, BlockId::from_u32(3))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                // NA's b must read 1.
                tk(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(loc(6)),
                    targets: SwitchTargets {
                        cases: Box::new([(1, BlockId::from_u32(5))]),
                        otherwise: BlockId::from_u32(4),
                    },
                }),
            ),
            bb(
                vec![],
                tk(TerminatorKind::Call {
                    callee: CallCallee::Direct(fail_id),
                    args: Box::new([]),
                    destination: loc(0),
                    target: Some(BlockId::from_u32(5)),
                }),
            ),
            bb(vec![], tk(TerminatorKind::Return)),
        ],
    );
    let id = fn_instance(&mut m, "regseq_main", sig, Some(body));
    nomangle(&mut m, id);

    let tu = emit_generic_c(&m).expect("emit must return Ok");
    assert!(
        tu.contains("(struct EBN){ ._raw = { 0, 2 } }"),
        "B serializes byte 2 at the NESTED bool offset 1 via _raw (no normalization):\n{tu}"
    );
    let tu = with_fail_callback(&tu);
    let run = assert_runs(&tu, "regseq_main", "inline-nested-bool-niche-const");
    assert_eq!(
        run.exit_code,
        Some(0),
        "B disc 1 (nested byte 2 unnormalized), A disc 0 + x 5 + b 1; stderr: {}",
        run.stderr
    );
}

// ---- section 8 diagnostic-logging -> stdio (fwrite): compile-AND-run (B2) ----

/// The self-contained diag-log prelude shared by the B2 fixtures: the
/// `#![no_std]` + `register_tool(rseq)` log shim (mirroring
/// `test-ir-fixtures/fixtures/diag_logging.rs`) extended with a `LogArg`
/// variant + `From` impl for every arg type the B2 bodies log (all integer
/// widths, `bool`, `&str`) and the four severity macros. The intrinsic fns
/// carry the `#[rseq::intrinsic("log-<sev>")]` recognition tag; the bridge's
/// DiagLogPeel flattens each call to the `[fmt, v1, ..., vN]` shape and DCE
/// removes the `LogArg` carrier.
const DIAG_PRELUDE: &str = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
            impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
            impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
            impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
            impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use log_warn as warn;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}
"#;

/// Build a full diag-log snippet: the shared prelude + the supplied
/// `regseq_main` body, plus any extra item text (e.g. `#[inline(never)]`
/// identity helpers for non-static `&str` arguments).
fn diag_snippet(extra_items: &str, body: &str) -> String {
    format!(
        "{DIAG_PRELUDE}\n{extra_items}\n#[no_mangle]\npub extern \"C\" fn regseq_main() {{\n{body}\n}}\n"
    )
}

/// Bridge + emit + compile + run a diag-log snippet, returning its captured
/// stdout (asserting a clean exit). The single source of the B2 "log a
/// computed value -> observe stdout" loop.
fn run_diag(label: &str, extra_items: &str, body: &str) -> String {
    let snippet = TempSnippet::new(label, &diag_snippet(extra_items, body));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet {label:?} must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for a diag-log body");
    let entry = fn_cname(&model, "regseq_main");
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .unwrap_or_else(|e| panic!("diag-log TU {label:?} must compile + run: {e}"));
    assert_eq!(
        run.exit_code,
        Some(0),
        "diag-log {label:?} exits 0; stderr: {}\nstdout: {}",
        run.stderr,
        run.stdout
    );
    run.stdout
}

#[test]
fn diag_log_levels_print_the_four_bracketed_tags() {
    // One call per severity, each logging a computed value (so stdout proves
    // the exercised path). DL-2 (all four tags), DL-7 (interception), TM-5/8.
    let stdout = run_diag(
        "levels",
        "",
        r#"    let a: u32 = 2 + 3;
    regseq::log::debug!("d={}", a);
    regseq::log::info!("i={}", a + 1);
    regseq::log::warn!("w={}", a + 2);
    regseq::log::error!("e={}", a + 3);"#,
    );
    assert_eq!(
        stdout, "[DEBUG] d=5\n[INFO] i=6\n[WARN] w=7\n[ERROR] e=8\n",
        "four tags + computed values in order"
    );
}

#[test]
fn diag_log_numeric_widths_use_the_matching_specifier_and_cast() {
    // Each width logged through its `%u`/`%llu`/`%d`/`%lld` specifier with the
    // matching cast. The `u64` value is `> u32::MAX` (`(1 << 32) + 1`), so a
    // missing `(unsigned long long)` cast would truncate to `1` -- the golden
    // catches it. DL-3.
    let stdout = run_diag(
        "widths",
        "",
        r#"    let u8v: u8 = 200 + 50;
    let u16v: u16 = 60000 + 1;
    let u32v: u32 = 4000000000 + 1;
    let u64v: u64 = (1u64 << 32) + 1;
    let i8v: i8 = 100 - 110;
    let i16v: i16 = 30000 - 30001;
    let i32v: i32 = 2000000000 - 2000000001;
    let i64v: i64 = -5000000000 - 1;
    regseq::log::info!("u8={} u16={} u32={} u64={}", u8v, u16v, u32v, u64v);
    regseq::log::info!("i8={} i16={} i32={} i64={}", i8v, i16v, i32v, i64v);"#,
    );
    assert_eq!(
        stdout,
        "[INFO] u8=250 u16=60001 u32=4000000001 u64=4294967297\n\
         [INFO] i8=-10 i16=-1 i32=-1 i64=-5000000001\n",
    );
}

#[test]
fn diag_log_bool_prints_one_or_zero_via_int_cast() {
    // `bool` lowered through `(int)` -> `%d` -> 1 / 0. DL-3.
    let stdout = run_diag(
        "bool",
        "",
        r#"    let t: bool = 2 > 1;
    let f: bool = 1 > 2;
    regseq::log::info!("t={} f={}", t, f);"#,
    );
    assert_eq!(stdout, "[INFO] t=1 f=0\n");
}

#[test]
fn diag_log_hex_modifiers_compose_specifier_and_padding() {
    // The bridge's full accepted set (incl. the uppercase-padded `:08X` /
    // `:#010X` beyond section 26.7's table). DL-3 modifier composition.
    let stdout = run_diag(
        "hex",
        "",
        r#"    let v: u32 = 0xabc0 + 0xd;
    regseq::log::info!("x={:x} X={:X} ax={:#x} aX={:#X}", v, v, v, v);
    regseq::log::info!("z={:08x} Z={:08X} az={:#010x} aZ={:#010X}", v, v, v, v);"#,
    );
    assert_eq!(
        stdout,
        "[INFO] x=abcd X=ABCD ax=0xabcd aX=0XABCD\n\
         [INFO] z=0000abcd Z=0000ABCD az=0x0000abcd aZ=0X0000ABCD\n",
    );
}

#[test]
fn diag_log_64bit_hex_inserts_the_ll_length_modifier() {
    // A `u64` value `> u32::MAX` under `{:08x}` -> `%08llx`: without the `ll`
    // (or the `(unsigned long long)` cast) printf reads 32 bits and prints the
    // wrong value. Proves the mandatory 64-bit length modifier.
    let stdout = run_diag(
        "hex64",
        "",
        r#"    let v: u64 = (0xdeadbeefu64 << 8) | 0xff;
    regseq::log::info!("v={:08x} a={:#010X}", v, v);"#,
    );
    assert_eq!(stdout, "[INFO] v=deadbeefff a=0XDEADBEEFFF\n");
}

#[test]
fn diag_log_signed_hex_preserves_source_width_twos_complement() {
    // `{:x}` on a negative value renders its SOURCE-width two's-complement via
    // the two-step `(varargs-unsigned)(unsigned-of-source-width)` cast: -1i8 ->
    // `ff` (NOT `ffffffff`), -1i16 -> `ffff`, -1i32 -> `ffffffff`, -1i64 ->
    // `ffffffffffffffff`. A plain `(unsigned)` on -1i8 would wrongly sign-extend.
    let stdout = run_diag(
        "signedhex",
        "",
        r#"    let a: i8 = 5 - 6;
    let b: i16 = 5 - 6;
    let c: i32 = 5 - 6;
    let d: i64 = 5 - 6;
    regseq::log::info!("a={:x} b={:x} c={:x} d={:x}", a, b, c, d);"#,
    );
    assert_eq!(stdout, "[INFO] a=ff b=ffff c=ffffffff d=ffffffffffffffff\n");
}

#[test]
fn diag_log_static_str_emits_a_direct_c_string_literal() {
    // A static string literal argument -> direct C string literal `%s`. DL-4.
    let stdout = run_diag(
        "str-static",
        "",
        r#"    regseq::log::info!("dev={}", "uart0");"#,
    );
    assert_eq!(stdout, "[INFO] dev=uart0\n");
}

/// An `#[inline(never)]` identity over `&str`: its return value is a runtime
/// (non-static) `&str` operand, exercising the section-8.3 non-static `&str`
/// value `fwrite` path (`fwrite((op).data, 1, (op).len, stdout)`).
const STR_ID_HELPER: &str = "#[inline(never)]\nfn id(s: &str) -> &str { s }";

#[test]
fn diag_log_non_static_str_writes_its_full_byte_range() {
    // A runtime-borrowed `&str` -> a value `fwrite` of its full `.data`/`.len`
    // byte range, printed in full. DL-4 non-static path.
    let stdout = run_diag(
        "str-nonstatic",
        STR_ID_HELPER,
        r#"    regseq::log::info!("name={}", id("ethernet0"));"#,
    );
    assert_eq!(stdout, "[INFO] name=ethernet0\n");
}

#[test]
fn diag_log_non_static_str_long_prints_in_full_no_truncation() {
    // A non-static `&str` of length `>= 256` prints IN FULL under `fwrite`
    // (no buffer, no cap, no truncation, no marker). The task-e printf/%s path
    // with the 256-byte stack buffer would have truncated to the first 255
    // bytes; this is the regression guard for the removed truncation contract.
    // DL-4 non-static, full-length.
    let long = "a".repeat(300);
    let body = r#"    regseq::log::info!("{}", id(LONG));"#.to_string();
    let extra = format!("{STR_ID_HELPER}\nconst LONG: &str = \"{long}\";");
    let stdout = run_diag("str-long", &extra, &body);
    let expected = format!("[INFO] {}\n", "a".repeat(300));
    assert_eq!(stdout, expected);
}

#[test]
fn diag_log_empty_template_prints_just_the_tag() {
    // Zero placeholders -> `[INFO] connected\n`.
    let stdout = run_diag("empty", "", r#"    regseq::log::info!("connected");"#);
    assert_eq!(stdout, "[INFO] connected\n");
}

#[test]
fn diag_log_brace_and_percent_escapes_render_literally() {
    // `{{`/`}}` -> literal `{`/`}`; a literal `%` is doubled to `%%` in the C
    // format string (so it renders as a single `%`).
    let stdout = run_diag(
        "escapes",
        "",
        r#"    let n: u32 = 25 + 25;
    regseq::log::info!("{{x}} {}% done", n);"#,
    );
    assert_eq!(stdout, "[INFO] {x} 50% done\n");
}

#[test]
fn diag_log_embedded_newline_yields_multiple_output_lines() {
    // An embedded `\n` in the template is ordinary literal text -> two output
    // lines from one call (plus the lowering's trailing `\n`).
    let stdout = run_diag(
        "newline",
        "",
        r#"    let n: u32 = 1 + 6;
    regseq::log::info!("line1\nline2={}", n);"#,
    );
    assert_eq!(stdout, "[INFO] line1\nline2=7\n");
}

#[test]
fn diag_log_template_interior_nul_is_written_faithfully() {
    // An interior NUL in the format-template literal (valid UTF-8, length-
    // prefixed) is reproduced byte-for-byte. The task-e printf path (a NUL-
    // terminated C format string) would have truncated the output at the
    // first NUL, dropping everything after `[INFO] a`.
    let stdout = run_diag(
        "tpl-nul",
        "",
        r#"    let n: u32 = 3 + 4;
    regseq::log::info!("a\0b={}", n);"#,
    );
    assert_eq!(stdout, "[INFO] a\0b=7\n");
}

#[test]
fn diag_log_static_str_interior_nul_is_written_faithfully() {
    // An interior NUL in a STATIC `&str` value (CMS bytes emitted as a C string
    // literal `fwrite`n with the explicit decoded count) is reproduced. The
    // task-e `%s` would have stopped at the first NUL.
    let stdout = run_diag(
        "static-nul",
        "",
        r#"    regseq::log::info!("s={}", "x\0y");"#,
    );
    assert_eq!(stdout, "[INFO] s=x\0y\n");
}

#[test]
fn diag_log_non_static_str_interior_nul_is_written_faithfully() {
    // An interior NUL in a NON-STATIC `&str` value (`fwrite((op).data, 1,
    // (op).len, stdout)`) is reproduced -- `.len` includes the NUL byte, so
    // the full range prints. The task-e `%s` would have truncated at the NUL.
    let stdout = run_diag(
        "nonstatic-nul",
        STR_ID_HELPER,
        r#"    regseq::log::info!("d={}", id("p\0q"));"#,
    );
    assert_eq!(stdout, "[INFO] d=p\0q\n");
}
