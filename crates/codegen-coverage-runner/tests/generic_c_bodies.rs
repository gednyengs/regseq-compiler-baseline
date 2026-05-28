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

use codegen_coverage_runner::toolchain;
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
    let run =
        toolchain::compile_and_run(&tu, &entry, cc()).expect("scalar-body TU must compile + run");
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
    let run =
        toolchain::compile_and_run(&tu, &entry, cc()).expect("wrong-result TU must compile + run");
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
    let run = toolchain::compile_and_run(&tu, &entry, cc())
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
    let run =
        toolchain::compile_and_run(&tu, &entry, cc()).expect("wrong-result TU must compile + run");
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
    toolchain::compile_and_run(tu, entry, cc())
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
    let run =
        toolchain::compile_and_run(&tu, &entry, cc()).expect("bridged enum TU must compile + run");
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
    let run = toolchain::compile_and_run(&tu, &entry, cc()).expect("TU must compile + run");
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
