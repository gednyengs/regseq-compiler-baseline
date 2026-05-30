//! regseq-driver-85: the `call_once` closure-call shim declares its
//! receiver **by value** (the captures struct) in its type-system
//! `FnSig`, but rustc reuses the closure's own by-reference MIR body for
//! all three `Fn`-family call kinds. That body declares the receiver
//! local `_1` as `&Captures` and projects it through a leading
//! `PlaceElem::Deref` (`(*_1).field0`). The backend faithfully emits
//! `(*_1)._0`, dereferencing a by-value struct local -> `cc` rejects
//! `invalid type argument of unary '*' (have struct ...)`.
//!
//! The bridge reconciles the by-value receiver decl with its body
//! projection at materialisation time: for a `CallOnce` shim whose
//! `FnSig` receiver is by value but whose materialised body local `_1`
//! is a reference, the body's `_1` decl is retyped to the by-value
//! receiver and the leading `Deref` is stripped from every place rooted
//! at `_1`. The emitted C then type-checks: `_1._0` on a by-value
//! struct.
//!
//! `call` / `call_mut` (receiver `&self` / `&mut self`) are left
//! untouched -- their `FnSig` receiver IS a reference, so it already
//! agrees with the body's by-reference `_1` and its genuine `Deref`.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{
    ClosureCallKind, InstanceKind, LocalId, Place, PlaceElem, RegseqModel, SyntheticItemKind, Ty,
    Visitor,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The OS reclaims the temp dir entry.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-call-once-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and return the validated model, panicking with a
/// diagnostic on any non-`Ok` outcome.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    match outcome {
        Ok(BridgeOutcome::Ok(model)) => model,
        Ok(other) => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
        Err(_) => panic!("bridge panicked for {label}"),
    }
}

/// Read-only place visitor that flags any place rooted at `_1` that
/// leads with a `Deref`.
struct ReceiverDerefScan {
    found: bool,
}

impl Visitor for ReceiverDerefScan {
    fn visit_place(&mut self, place: &Place) {
        if place.local == LocalId::from_u32(1)
            && matches!(place.projection.first(), Some(PlaceElem::Deref))
        {
            self.found = true;
        }
    }
}

/// Whether any place rooted at `_1` in `body` leads with a `Deref`.
fn body_has_leading_deref_on_local1(body: &regseq_model::Body) -> bool {
    let mut scan = ReceiverDerefScan { found: false };
    scan.super_body(body);
    scan.found
}

/// The body's receiver local `_1` type must AGREE with the shim's
/// `FnSig` receiver `inputs[0]`: a by-value `CallOnce` receiver must NOT
/// be a `Ty::Ref`, and no place rooted at `_1` may lead with a `Deref`.
#[test]
fn call_once_shim_receiver_decl_agrees_with_body_projection() {
    // A capturing `Fn` closure (Copy capture): rustc reuses the same
    // by-reference body for call / call_mut / call_once.
    let model = bridge_ok(
        "fn_copy",
        "#![no_std]\npub fn f() -> u32 {\n    let cap = 7u32;\n    let c = |x: u32| x + cap;\n    c(2)\n}\n",
    );

    let mut saw_call_once = false;
    for inst in model.instances.iter() {
        let SyntheticItemKind::ClosureCall { call_kind, .. } =
            &model.item_synthetic_kinds[inst.item]
        else {
            continue;
        };
        let InstanceKind::Function(fd) = &inst.kind else {
            continue;
        };
        let sig = model.sigs.resolve(fd.fn_sig);
        let receiver_sig_ty = *sig
            .inputs
            .first()
            .expect("a closure-call shim has a receiver");
        let body = fd
            .body
            .as_ref()
            .expect("closure-call shim body materialised");
        let receiver_local_ty = body.local_decls[LocalId::from_u32(1)].ty;

        match call_kind {
            ClosureCallKind::CallOnce => {
                saw_call_once = true;
                // The type-system receiver is by value: not a reference.
                assert!(
                    !matches!(model.types.resolve(receiver_sig_ty), Ty::Ref(..)),
                    "CallOnce FnSig receiver must be by value, got {:?}",
                    model.types.resolve(receiver_sig_ty),
                );
                // The body local `_1` must AGREE: same by-value type,
                // not a reference.
                assert_eq!(
                    receiver_local_ty,
                    receiver_sig_ty,
                    "CallOnce body receiver _1 type ({:?}) must equal the FnSig receiver ({:?})",
                    model.types.resolve(receiver_local_ty),
                    model.types.resolve(receiver_sig_ty),
                );
                // No place rooted at `_1` may lead with a Deref.
                assert!(
                    !body_has_leading_deref_on_local1(body),
                    "CallOnce body must not Deref the by-value receiver _1",
                );
            }
            ClosureCallKind::Call | ClosureCallKind::CallMut => {
                // call / call_mut take &self / &mut self: the FnSig
                // receiver is a reference and the body's genuine Deref
                // is correct -- leave it.
                assert!(
                    matches!(model.types.resolve(receiver_sig_ty), Ty::Ref(..)),
                    "{call_kind:?} FnSig receiver must be a reference, got {:?}",
                    model.types.resolve(receiver_sig_ty),
                );
                assert!(
                    matches!(model.types.resolve(receiver_local_ty), Ty::Ref(..)),
                    "{call_kind:?} body receiver _1 must stay a reference",
                );
            }
        }
    }
    assert!(
        saw_call_once,
        "fixture must produce a CallOnce closure-call shim",
    );
}
