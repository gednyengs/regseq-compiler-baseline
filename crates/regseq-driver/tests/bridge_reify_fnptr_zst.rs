//! regseq-backends-20: a DIRECT fn-item -> fn-pointer coercion
//! (`let f: fn(i32)->i32 = dbl;`) lowers in rustc MIR to
//! `Cast(PointerCoercion(ReifyFnPointer), Const::ZST FnDef, FnPtr)`. The
//! operand is the zero-sized `FnDef` constant; lowered naively it becomes a
//! bare model `Const::ZST`, which discards the `FnDef` identity (the model
//! `Const::ZST` is a single canonical representative for all zero-sized types)
//! and leaves the backend unable to name the reified function.
//!
//! The bridge fix recovers the `FnDef` identity at the cast site (where the
//! rustc operand type is still available), materialises its Function instance,
//! and lowers the operand to a `Const::Reloc` -- the same provenance shape a
//! function pointer baked into a `const`/`static` already uses, which the
//! backend's `reify_fn_pointer` Reloc path emits as `&dbl`.
//!
//! This test bridges the surface end-to-end (including `validate()`) and
//! asserts the reify cast's operand is a `Const::Reloc` resolving to the `dbl`
//! Function instance -- NOT a bare `Const::ZST`.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{
    CastKind, Const, InstanceKind, Operand, PointerCoercion, RegseqModel, Rvalue, StatementKind,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its path.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-reify-zst-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both succeed.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = try_bridge_fixture_path(&path);
    let _ = std::fs::remove_file(&path);
    match outcome {
        BridgeOutcome::Ok(model) => model,
        other => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
    }
}

const SRC: &str = r#"
#![no_std]
fn dbl(x: i32) -> i32 { x * 2 }

#[no_mangle]
pub extern "C" fn regseq_main() -> i32 {
    let f: fn(i32) -> i32 = dbl;
    f(21)
}
"#;

#[test]
fn reify_fn_def_constant_lowers_to_reloc() {
    let m = bridge_ok("direct", SRC);

    // Find the `dbl` Function instance (its emission_name ends with `::dbl`).
    let dbl_id = (0..m.instances.len())
        .map(|i| regseq_model::InstanceId::from_u32(i as u32))
        .find(|id| {
            matches!(&m.instances[*id].kind, InstanceKind::Function(_))
                && m.strings
                    .resolve(m.instances[*id].emission_name)
                    .ends_with("::dbl")
        })
        .expect("dbl Function instance must be materialised");
    let dbl_ref = m.instance_ref_of(dbl_id);

    // Locate the ReifyFnPointer cast and assert its operand is Const::Reloc(dbl).
    let mut saw_reify = false;
    for idx in 0..m.instances.len() {
        let id = regseq_model::InstanceId::from_u32(idx as u32);
        let InstanceKind::Function(data) = &m.instances[id].kind else {
            continue;
        };
        let Some(body) = &data.body else { continue };
        for block in body.basic_blocks.iter() {
            for stmt in block.statements.iter() {
                let StatementKind::Assign(_, Rvalue::Cast(kind, op, _)) = &stmt.kind else {
                    continue;
                };
                if *kind != CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer) {
                    continue;
                }
                saw_reify = true;
                let Operand::Constant(c) = op else {
                    panic!("ReifyFnPointer operand is not a constant: {op:?}");
                };
                match m.consts.resolve(*c) {
                    Const::Reloc(target) => assert_eq!(
                        *target, dbl_ref,
                        "ReifyFnPointer Reloc must target the `dbl` instance"
                    ),
                    other => panic!(
                        "ReifyFnPointer operand should be Const::Reloc, got {other:?} \
                         (a bare Const::ZST would be the un-fixed lowering)"
                    ),
                }
            }
        }
    }
    assert!(
        saw_reify,
        "no ReifyFnPointer cast found in the bridged model"
    );
}
