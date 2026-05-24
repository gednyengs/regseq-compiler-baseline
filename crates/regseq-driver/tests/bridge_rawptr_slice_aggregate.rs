//! regseq-driver-76: building a raw pointer from parts whose result is a
//! slice (`*const [T]`) or str (`*const str`) raw pointer must bridge `Ok`,
//! not `Rejected(UnsupportedTy)`. `core::intrinsics::aggregate_raw_ptr`
//! lowers to `Rvalue::Aggregate(AggregateKind::RawPtr(pointee, mutability),
//! [data, meta])`; at HEAD the RawPtr arm translated the bare pointee in the
//! sized-context `translate_ty`, which rejects `Slice`/`Str`. The fix routes
//! the pointee through `translate_ptr_pointee` (the unsized-allowed context
//! the pointer-*type* paths already use), so the aggregate path matches the
//! type path. The sized case (`*const u32`) is marker-invariant and unchanged.
//!
//! Each case wraps the in-process bridge run in `catch_unwind` so a
//! regression surfaces as a test failure rather than aborting the binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{AggregateKind, Mutability, RegseqModel, Rvalue, StatementKind, Ty, UintKind};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its path.
/// The file is left in the temp dir; the OS reclaims it.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-rawptr-slice-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both succeed
/// (`BridgeOutcome::Ok` implies both), returning the model. The bridge call
/// is wrapped in `catch_unwind` so a regression panic is a test failure.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    match outcome {
        Ok(BridgeOutcome::Ok(model)) => model,
        Ok(other) => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
        Err(_) => panic!("bridge panicked (regression?) for {label}"),
    }
}

/// Bridge `source` and return the typed outcome without asserting success.
fn bridge_outcome(label: &str, source: &str) -> BridgeOutcome {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    outcome.unwrap_or_else(|_| panic!("bridge panicked for {label}"))
}

/// Collect the `(pointee TyId, mutability)` of every
/// `AggregateKind::RawPtr` aggregate rvalue across all instance bodies.
fn raw_ptr_aggregate_pointees(model: &RegseqModel) -> Vec<(regseq_model::TyId, Mutability)> {
    let mut found = Vec::new();
    for instance in model.instances.iter() {
        let Some(function) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        for block in body.basic_blocks.iter() {
            for statement in block.statements.iter() {
                if let StatementKind::Assign(
                    _,
                    Rvalue::Aggregate(AggregateKind::RawPtr(pointee, mutability), _),
                ) = &statement.kind
                {
                    found.push((*pointee, *mutability));
                }
            }
        }
    }
    found
}

/// Assert exactly one `AggregateKind::RawPtr` aggregate exists and return its
/// resolved pointee `Ty` plus mutability.
fn sole_raw_ptr_pointee(model: &RegseqModel, label: &str) -> (Ty, Mutability) {
    let pointees = raw_ptr_aggregate_pointees(model);
    assert_eq!(
        pointees.len(),
        1,
        "{label}: expected exactly one AggregateKind::RawPtr aggregate, got {}",
        pointees.len()
    );
    let (pointee_id, mutability) = pointees[0];
    (model.types.resolve(pointee_id).clone(), mutability)
}

/// B1 (slice): `raw_slice(data: *const u32, len: usize) -> *const [u32]`
/// bridges `Ok` + validates; pointee `Ty::Slice(u32)`, const.
#[test]
fn slice_rawptr_aggregate_bridges_ok() {
    let model = bridge_ok(
        "b1",
        "#![feature(core_intrinsics)]\npub fn raw_slice(data: *const u32, len: usize) -> *const [u32] {\n    core::intrinsics::aggregate_raw_ptr(data, len)\n}\n",
    );
    let (pointee, mutability) = sole_raw_ptr_pointee(&model, "b1");
    let Ty::Slice(elem) = pointee else {
        panic!("b1: expected Ty::Slice pointee, got {pointee:?}");
    };
    assert_eq!(*model.types.resolve(elem), Ty::Uint(UintKind::U32));
    assert_eq!(mutability, Mutability::Not);
}

/// B2 (mut slice): `*mut u32, usize -> *mut [u32]` bridges `Ok` + validates;
/// pointee `Ty::Slice`, mutability `Mut`.
#[test]
fn mut_slice_rawptr_aggregate_bridges_ok() {
    let model = bridge_ok(
        "b2",
        "#![feature(core_intrinsics)]\npub fn raw_mut_slice(data: *mut u32, len: usize) -> *mut [u32] {\n    core::intrinsics::aggregate_raw_ptr(data, len)\n}\n",
    );
    let (pointee, mutability) = sole_raw_ptr_pointee(&model, "b2");
    assert!(
        matches!(pointee, Ty::Slice(_)),
        "b2: expected Ty::Slice pointee, got {pointee:?}"
    );
    assert_eq!(mutability, Mutability::Mut);
}

/// B3 (str): `*const u8, usize -> *const str` bridges `Ok` + validates;
/// pointee `Ty::Str`.
#[test]
fn str_rawptr_aggregate_bridges_ok() {
    let model = bridge_ok(
        "b3",
        "#![feature(core_intrinsics)]\npub fn raw_str(data: *const u8, len: usize) -> *const str {\n    core::intrinsics::aggregate_raw_ptr(data, len)\n}\n",
    );
    let (pointee, mutability) = sole_raw_ptr_pointee(&model, "b3");
    assert_eq!(pointee, Ty::Str, "b3: expected Ty::Str pointee");
    assert_eq!(mutability, Mutability::Not);
}

/// B4 (sized regression): `*const u32` via `(data, ())` bridges `Ok` +
/// validates; pointee `Ty::Uint(U32)`, unchanged from HEAD.
#[test]
fn sized_rawptr_aggregate_unchanged() {
    let model = bridge_ok(
        "b4",
        "#![feature(core_intrinsics)]\npub fn raw_sized(data: *const u32) -> *const u32 {\n    core::intrinsics::aggregate_raw_ptr(data, ())\n}\n",
    );
    let (pointee, mutability) = sole_raw_ptr_pointee(&model, "b4");
    assert_eq!(pointee, Ty::Uint(UintKind::U32));
    assert_eq!(mutability, Mutability::Not);
}

/// B6 (out-of-scope boundary): dyn-from-parts still rejects. The fix does not
/// touch the DynMetadata Foreign hazard, so a `*const dyn Trait` raw-pointer
/// aggregate must stay `Rejected` (documents the deferred boundary).
#[test]
fn dyn_from_parts_still_rejected() {
    let outcome = bridge_outcome(
        "b6",
        "#![feature(core_intrinsics, ptr_metadata)]\npub trait T {}\npub fn raw_dyn(data: *const (), meta: core::ptr::DynMetadata<dyn T>) -> *const dyn T {\n    core::intrinsics::aggregate_raw_ptr(data, meta)\n}\n",
    );
    assert!(
        matches!(outcome, BridgeOutcome::Rejected(_)),
        "b6: dyn-from-parts must stay Rejected (deferred Foreign hazard), got {outcome:?}"
    );
}
