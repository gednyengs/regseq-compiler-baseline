//! Same-type `Result`-`?` MIR-normalizer coverage (regseq-driver-92).
//!
//! Drives the production-faithful bridge (`try_bridge_fixture_path`:
//! `mir_opt_level=0` + the regseq cleanup-pass whitelist + `panic=abort`)
//! over `?`-bearing fixtures and asserts:
//!
//!   1. Same-type `Result`-`?` (unit / non-unit / multiple / live-drop)
//!      bridges to a direct discriminant-match shape with no
//!      `branch`/`from_residual`/`ControlFlow`/`Infallible` residue
//!      (Test Plan 1, Acceptance Criteria 1/2/4).
//!   2. Cross-type `?`, `Option`-`?`, and a `needs_drop`-`Ok` `?` each
//!      fail loud with `BridgeError::UnsupportedQuestionMark` rather than
//!      the vague `NonRigidTy` the un-normalized projection would produce
//!      (Test Plan 2, Acceptance Criterion 3).
//!   3. A same-type `?` fixture emits + `cc`-compiles + runs with the
//!      expected exit, exercising the Ok-path value flow, the Err-path
//!      propagation, and drop preservation on BOTH the continue and the
//!      `from_residual` successors at runtime (Test Plan 3, Acceptance
//!      Criterion 2).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::toolchain::{self, EntryShim};
use regseq_backends::emit_generic_c;
use regseq_driver::bridge::BridgeError;
use regseq_model::{
    AggregateKind, Body, InstanceKind, RegseqModel, Rvalue, StatementKind, TerminatorKind,
    VariantIdx,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// `Result::Err` variant index in the model (matches the rustc-side
/// `enum Result<T, E> { Ok, Err }` ordering).
const ERR_VARIANT: VariantIdx = VariantIdx::from_u32(1);

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
        let path = std::env::temp_dir().join(format!(
            "ccr-qmark-{label}-{}-{nanos}.rs",
            std::process::id()
        ));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Bridge an inline `?`-fixture source through the production-faithful
/// helper and return the typed outcome.
fn bridge(label: &str, src: &str) -> BridgeOutcome {
    let snippet = TempSnippet::new(label, src);
    try_bridge_fixture_path(&snippet.path)
}

/// Bridge a fixture that must succeed, returning the model.
fn bridge_ok(label: &str, src: &str) -> RegseqModel {
    match bridge(label, src) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("fixture {label:?} must bridge to Ok, got {other:?}"),
    }
}

/// Map a rendered `DefPath` to the C identifier the backend emits.
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

/// The emitted C name of the function instance whose path ends in `suffix`.
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

/// The materialised body of the function instance whose path ends in
/// `suffix`.
fn fn_body<'m>(model: &'m RegseqModel, suffix: &str) -> &'m Body {
    for inst in model.instances.iter() {
        if let InstanceKind::Function(data) = &inst.kind {
            let rendered = model.strings.resolve(inst.emission_name);
            if rendered.ends_with(suffix) {
                return data
                    .body
                    .as_ref()
                    .unwrap_or_else(|| panic!("instance {suffix:?} has no materialised body"));
            }
        }
    }
    panic!("no function instance whose path ends in {suffix:?}");
}

/// Assert the model carries none of the `?`-desugar residue the
/// normalizer must eliminate (Acceptance Criterion 1): no `ControlFlow`,
/// `Infallible`, or `from_residual` Item survives. (`branch` /
/// `from_residual` callees never reach interning because the calls are
/// consumed before translation.)
fn assert_no_question_mark_residue(model: &RegseqModel, label: &str) {
    for item in model.items.iter() {
        let path = model.strings.resolve(item.def_path.rendered);
        for needle in ["ControlFlow", "Infallible", "from_residual", "FromResidual"] {
            assert!(
                !path.contains(needle),
                "{label}: model Item `{path}` carries forbidden `?`-desugar residue `{needle}`"
            );
        }
    }
}

/// Assert `body` has the direct discriminant-match shape the normalizer
/// rewrites to: a `Discriminant` read feeding a `SwitchInt`, plus a
/// direct `Err(..)` aggregate (the rewritten `from_residual` arm).
fn assert_direct_match_shape(body: &Body, label: &str) {
    let has_discriminant = body.basic_blocks.iter().any(|block| {
        block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Discriminant(_))
            )
        })
    });
    let has_switch_int = body
        .basic_blocks
        .iter()
        .any(|block| matches!(block.terminator.kind, TerminatorKind::SwitchInt { .. }));
    let has_err_aggregate = body.basic_blocks.iter().any(|block| {
        block.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign(_, Rvalue::Aggregate(AggregateKind::Enum(_, variant), _))
                    if *variant == ERR_VARIANT
            )
        })
    });
    assert!(
        has_discriminant && has_switch_int,
        "{label}: expected a discriminant match (Discriminant + SwitchInt) in the rewritten body"
    );
    assert!(
        has_err_aggregate,
        "{label}: expected a direct `Err(..)` aggregate from the rewritten `from_residual` arm"
    );
}

// ---------------------------------------------------------------------------
// Test Plan 1 -- same-type `Result`-`?` bridges to the direct-match shape.
// ---------------------------------------------------------------------------

/// Discarded unit `Ok` (`helper()?;`): the `RemoveZsts` pass elides the
/// `Continue` payload bind, so the recognizer must accept the no-bind
/// shape.
#[test]
fn unit_discarded_question_mark_bridges_to_direct_match() {
    let src = r#"#![no_std]
pub enum E { A }
fn helper() -> Result<(), E> { Ok(()) }
fn does_q() -> Result<(), E> {
    helper()?;
    Ok(())
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    let model = bridge_ok("unit", src);
    assert_no_question_mark_residue(&model, "unit");
    assert_direct_match_shape(fn_body(&model, "does_q"), "unit");
}

/// Used non-unit `Ok` (`let v = helper_val()?;`): the `Continue` payload
/// bind is present and must be redirected to read the `?`-operand. The
/// error enum has two variants so it is NOT a ZST -- the residual flows
/// through a real local (`ext = (dst as Break).0; copy; from_residual(move ..)`),
/// exercising the recognizer's residual-chain trace (vs the elided-ZST
/// constant arm the single-variant fixtures hit).
#[test]
fn nonunit_used_question_mark_bridges_to_direct_match() {
    let src = r#"#![no_std]
pub enum E { A, B }
fn helper_val() -> Result<u32, E> { Ok(7) }
fn does_q() -> Result<u32, E> {
    let v = helper_val()?;
    Ok(v)
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    let model = bridge_ok("nonunit", src);
    assert_no_question_mark_residue(&model, "nonunit");
    assert_direct_match_shape(fn_body(&model, "does_q"), "nonunit");
}

/// Multiple `?` in one body: each cluster must be recognized and
/// rewritten independently (robust to the cleanup passes' renumbering).
/// Both `?` operate on the same `Result<u32, E>` monomorphization -- the
/// point of this case is two `?` clusters, not two monomorphizations (a
/// distinct core-generic-ADT instance-cardinality concern, orthogonal to
/// `?`).
#[test]
fn multiple_question_marks_bridge_to_direct_match() {
    let src = r#"#![no_std]
pub enum E { A }
fn helper() -> Result<u32, E> { Ok(1) }
fn helper2() -> Result<u32, E> { Ok(7) }
fn does_q() -> Result<u32, E> {
    let a = helper()?;
    let b = helper2()?;
    Ok(a | b)
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    let model = bridge_ok("multi", src);
    assert_no_question_mark_residue(&model, "multi");
    assert_direct_match_shape(fn_body(&model, "does_q"), "multi");
}

/// Early-return path with a live local that needs drop across the `?`:
/// the rewrite must `goto` the original `from_residual` successor (which
/// holds the drop), never a synthesized bare return (Acceptance
/// Criterion 2). The drop survives as a `Call` terminator (Drop-as-Call).
#[test]
fn live_drop_question_mark_bridges_and_preserves_drop() {
    let src = r#"#![no_std]
pub enum E { A }
pub struct Guard(pub u8);
impl Drop for Guard { fn drop(&mut self) {} }
fn helper() -> Result<(), E> { Ok(()) }
fn does_q() -> Result<(), E> {
    let _g = Guard(1);
    helper()?;
    Ok(())
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    let model = bridge_ok("livedrop", src);
    assert_no_question_mark_residue(&model, "livedrop");
    let body = fn_body(&model, "does_q");
    assert_direct_match_shape(body, "livedrop");
    // The drop on the early-return path survives the rewrite: a
    // Drop-as-Call `Call` terminator remains in the body.
    let has_call = body
        .basic_blocks
        .iter()
        .any(|block| matches!(block.terminator.kind, TerminatorKind::Call { .. }));
    assert!(
        has_call,
        "livedrop: the live local's drop must survive as a Drop-as-Call terminator"
    );
}

// ---------------------------------------------------------------------------
// Test Plan 2 -- out-of-scope `?` fails loud (not as a vague `NonRigidTy`).
// ---------------------------------------------------------------------------

/// Cross-type `?` (`E` -> `F` via `From`) is a real residual conversion
/// the same-type normalizer cannot do.
#[test]
fn cross_type_question_mark_fails_loud() {
    let src = r#"#![no_std]
pub enum E { A }
pub enum F { X }
impl From<E> for F { fn from(_: E) -> F { F::X } }
fn helper() -> Result<(), E> { Ok(()) }
fn does_q() -> Result<(), F> {
    helper()?;
    Ok(())
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    match bridge("cross", src) {
        BridgeOutcome::Rejected(BridgeError::UnsupportedQuestionMark { .. }) => {}
        other => panic!("cross-type `?` must fail loud as UnsupportedQuestionMark, got {other:?}"),
    }
}

/// `Option`-`?` is a different carrier (the residual is `Option<Infallible>`),
/// outside the same-type `Result` scope.
#[test]
fn option_question_mark_fails_loud() {
    let src = r#"#![no_std]
fn opt_helper() -> Option<u32> { Some(1) }
fn does_q() -> Option<u32> {
    let v = opt_helper()?;
    Some(v)
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    match bridge("option", src) {
        BridgeOutcome::Rejected(BridgeError::UnsupportedQuestionMark { .. }) => {}
        other => panic!("`Option`-`?` must fail loud as UnsupportedQuestionMark, got {other:?}"),
    }
}

/// A `needs_drop` `Ok` payload (`Result<String, E>`) cannot be faithfully
/// preserved by the move-out rewrite. (`alloc::String` makes this an
/// `extern crate alloc` fixture.)
#[test]
fn needs_drop_ok_question_mark_fails_loud() {
    let src = r#"#![no_std]
extern crate alloc;
use alloc::string::String;
pub enum E { A }
fn helper_string() -> Result<String, E> { Ok(String::new()) }
fn does_q() -> Result<String, E> {
    let s = helper_string()?;
    Ok(s)
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_q(); }
"#;
    match bridge("needsdrop", src) {
        BridgeOutcome::Rejected(BridgeError::UnsupportedQuestionMark { .. }) => {}
        other => {
            panic!("`needs_drop`-`Ok` `?` must fail loud as UnsupportedQuestionMark, got {other:?}")
        }
    }
}

/// A direct `FromResidual::from_residual` call with NO paired `branch`
/// (an unstable `try_trait_v2` shape, not the `?` desugar). Phase-1
/// mono-collection skips the `from_residual` lang item, so without an
/// UNCONDITIONAL defensive scan this stray `core` call would slip past
/// normalization (the `plans.is_empty()` early-out fires before the scan)
/// and surface as a body-less core call / `NonRigidTy` instead of failing
/// loud. Regression for the regseq-driver-92 guard-hole: the scan must run
/// even when no `branch` cluster exists.
#[test]
fn stray_from_residual_without_branch_fails_loud() {
    let src = r#"#![no_std]
#![feature(try_trait_v2)]
use core::convert::Infallible;
use core::ops::FromResidual;
pub enum E { A }
fn does_fromres() -> Result<(), E> {
    let residual: Result<Infallible, E> = Err(E::A);
    FromResidual::from_residual(residual)
}
#[no_mangle]
pub extern "C" fn regseq_main() { let _ = does_fromres(); }
"#;
    match bridge("strayfromres", src) {
        BridgeOutcome::Rejected(BridgeError::UnsupportedQuestionMark { .. }) => {}
        other => panic!(
            "a stray `from_residual` with no `branch` must fail loud as UnsupportedQuestionMark, got {other:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// Test Plan 3 -- end-to-end emit + cc + run with the expected exit.
// ---------------------------------------------------------------------------

/// A same-type `?` fixture that exercises BOTH arms and drop preservation
/// at runtime: `compute(true)` flows the `Ok(42)` payload through the `?`
/// bind; `compute(false)` propagates `Err` through the rewritten arm; and
/// `Guard`'s drop must run exactly once per call (on the continue
/// successor for the Ok path and on the `from_residual` successor for the
/// Err path). A correct lowering exits 0; any wrong arm or a dropped/
/// double drop calls the abort callback.
#[test]
fn end_to_end_question_mark_runs_and_preserves_drops() {
    let src = r#"#![no_std]
extern "C" {
    fn regseq_note_drop();
    fn regseq_drops() -> u32;
    fn regseq_test_fail();
}
pub enum E { Bad, Other }
struct Guard;
impl Drop for Guard { fn drop(&mut self) { unsafe { regseq_note_drop(); } } }
fn helper(ok: bool) -> Result<u32, E> { if ok { Ok(42) } else { Err(E::Bad) } }
fn compute(ok: bool) -> Result<u32, E> {
    let _g = Guard;
    let v = helper(ok)?;
    Ok(v)
}
#[no_mangle]
pub extern "C" fn regseq_main() {
    match compute(true) {
        Ok(42) => {}
        _ => unsafe { regseq_test_fail(); }
    }
    match compute(false) {
        Err(E::Bad) => {}
        _ => unsafe { regseq_test_fail(); }
    }
    unsafe {
        if regseq_drops() != 2 { regseq_test_fail(); }
    }
}
"#;
    let model = bridge_ok("e2e", src);
    assert_no_question_mark_residue(&model, "e2e");
    assert_direct_match_shape(fn_body(&model, "compute"), "e2e");

    let tu = emit_generic_c(&model).expect("same-type `?` body must emit");
    let entry = fn_cname(&model, "regseq_main");
    let note = fn_cname(&model, "regseq_note_drop");
    let drops = fn_cname(&model, "regseq_drops");
    let fail = fn_cname(&model, "regseq_test_fail");
    let harness = format!(
        "static unsigned __qmark_drops = 0;\n\
         void {note}(void) {{ __qmark_drops++; }}\n\
         unsigned {drops}(void) {{ return __qmark_drops; }}\n\
         void {fail}(void) {{ fprintf(stderr, \"qmark e2e failed\\n\"); abort(); }}\n"
    );
    let tu = format!("{tu}\n{harness}");

    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("same-type `?` TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "a correct same-type `?` lowering (both arms + drops preserved) exits 0; stderr: {}",
        run.stderr
    );
}

/// A same-type `?` whose `Err` payload needs drop (`Result<u32, DropErr>`).
/// The `Ok` payload is `Copy`, so the rewrite is NOT rejected (only the
/// `Ok` payload is `needs_drop`-gated), and the rewritten `Err` arm moves
/// the `DropErr` out of the `?`-operand into the return aggregate. This
/// locks in that the move-out is drop-faithful: the propagated `DropErr`
/// is dropped EXACTLY once -- no double-drop from the move, no leak.
/// (Codex Q4 lock-in for the regseq-driver-92 review.)
#[test]
fn needs_drop_err_question_mark_propagates_and_drops_once() {
    let src = r#"#![no_std]
extern "C" {
    fn regseq_note_drop();
    fn regseq_drops() -> u32;
    fn regseq_test_fail();
}
struct DropErr;
impl Drop for DropErr { fn drop(&mut self) { unsafe { regseq_note_drop(); } } }
fn helper(ok: bool) -> Result<u32, DropErr> { if ok { Ok(7) } else { Err(DropErr) } }
fn compute(ok: bool) -> Result<u32, DropErr> {
    let v = helper(ok)?;
    Ok(v)
}
#[no_mangle]
pub extern "C" fn regseq_main() {
    match compute(true) {
        Ok(7) => {}
        _ => unsafe { regseq_test_fail(); }
    }
    match compute(false) {
        Ok(_) => unsafe { regseq_test_fail(); }
        Err(_e) => {}
    }
    unsafe {
        if regseq_drops() != 1 { regseq_test_fail(); }
    }
}
"#;
    let model = bridge_ok("needsdroperr", src);
    assert_no_question_mark_residue(&model, "needsdroperr");
    assert_direct_match_shape(fn_body(&model, "compute"), "needsdroperr");

    let tu = emit_generic_c(&model).expect("needs_drop-`Err` same-type `?` body must emit");
    let entry = fn_cname(&model, "regseq_main");
    let note = fn_cname(&model, "regseq_note_drop");
    let drops = fn_cname(&model, "regseq_drops");
    let fail = fn_cname(&model, "regseq_test_fail");
    let harness = format!(
        "static unsigned __qmark_drops = 0;\n\
         void {note}(void) {{ __qmark_drops++; }}\n\
         unsigned {drops}(void) {{ return __qmark_drops; }}\n\
         void {fail}(void) {{ fprintf(stderr, \"qmark needs_drop-err failed\\n\"); abort(); }}\n"
    );
    let tu = format!("{tu}\n{harness}");

    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .expect("needs_drop-`Err` same-type `?` TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "the propagated `Err(DropErr)` must drop exactly once (no double-drop, no leak); stderr: {}",
        run.stderr
    );
}
