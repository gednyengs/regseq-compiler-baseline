//! regseq-driver-61 Test Plan B1-B3 regression guards: every
//! driver-side `FnSig` intern site keeps the
//! `sig_c_abi_lowerings.len() == sigs.len()` invariant (spec.md
//! section 3 schema, section 12 step 5 validator rule) and the per-
//! entry "`Some` iff `FnAbi::C`" presence rule.
//!
//! Bridges three focused fixtures shipped by `test-ir-fixtures`:
//!
//! - `drop_glue_in_self_consuming_method` (B1): the Drop-as-Call
//!   collapse path at
//!   `crates/regseq-driver/src/bridge/body/terminator.rs:560+`. The
//!   bridge interns a fresh drop-glue `FnSig`
//!   (`unsafe fn(*mut R) -> ()`, `FnAbi::Rust`) via
//!   `lookup_or_create_direct_instance`.
//! - `closure_call_sig_sync` (B2): the closure-call `make_fn_sig`
//!   path at `terminator.rs:316-331`. Latent today; either the
//!   eager-synth path at `bridge/mod.rs:2278-2305` paints the slot
//!   first, or the bare-intern path is exercised -- either way the
//!   invariant must hold.
//! - `fn_ptr_translation_sig_sync` (B3): the `TyKind::FnPtr` branch
//!   at `crates/regseq-driver/src/bridge/ty.rs:135-179`. Exercises
//!   both `FnAbi::Rust` and `FnAbi::C` variants.

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::{
    args_key_for_generic_args, DefPathData, FnAbi, FunctionInstanceFlavor, GenericArg, Mutability,
    Safety, SyntheticItemKind, Ty,
};
use test_ir_fixtures::bridge_fixture;

const DROP_GLUE_FIXTURE: &str = "drop_glue_in_self_consuming_method";
const CLOSURE_CALL_FIXTURE: &str = "closure_call_sig_sync";
const FN_PTR_FIXTURE: &str = "fn_ptr_translation_sig_sync";

/// Spec.md section 3 invariant: every interned `FnSig` has a parallel
/// `sig_c_abi_lowerings` slot.
fn assert_parallel_table_length(model: &regseq_model::RegseqModel) {
    assert_eq!(
        model.sig_c_abi_lowerings.len(),
        model.sigs.len(),
        "spec.md section 3 invariant: sig_c_abi_lowerings.len() must equal sigs.len(); got {} vs {}",
        model.sig_c_abi_lowerings.len(),
        model.sigs.len(),
    );
}

/// Spec.md section 12 step 5 per-entry presence rule:
/// `sig_c_abi_lowerings[id]` is `Some(_)` iff the resolved sig has
/// `FnAbi::C`.
fn assert_some_iff_c_abi(model: &regseq_model::RegseqModel) {
    for (sig_id, sig) in model.sigs.iter() {
        let slot = &model.sig_c_abi_lowerings[sig_id];
        match (sig.abi, slot) {
            (FnAbi::C, Some(_)) => {}
            (FnAbi::Rust, None) => {}
            (FnAbi::C, None) => panic!(
                "spec.md section 12 step 5: FnSigId {} with FnAbi::C must carry Some(CAbiLowering); got None",
                sig_id.as_u32(),
            ),
            (FnAbi::Rust, Some(_)) => panic!(
                "spec.md section 12 step 5: FnSigId {} with FnAbi::Rust must carry None; got Some(_)",
                sig_id.as_u32(),
            ),
        }
    }
}

#[test]
fn drop_glue_in_self_consuming_method_bridges_without_parallel_table_desync() {
    let model = bridge_fixture(DROP_GLUE_FIXTURE);

    // Spec.md section 3 + section 12 step 5: structural invariants
    // hold after bridging. These are the load-bearing regression
    // assertions for this task -- pre-fix, bridging the fixture
    // panics inside `RegseqModel::remove_fn_sigs_and_remap` via the
    // peel-residue DCE pass before any test assertion runs.
    assert_parallel_table_length(&model);
    assert_some_iff_c_abi(&model);

    // Locate the struct `R` Item so we can name its TyId for the
    // drop-glue input-type assertion.
    let r_item_id = model
        .items
        .iter()
        .enumerate()
        .find_map(|(idx, item)| {
            let last = item.def_path.segments.last()?;
            match last.data {
                DefPathData::TypeNs(name_id) if model.strings.resolve(name_id) == "R" => {
                    Some(regseq_model::ItemId::from_u32(idx as u32))
                }
                _ => None,
            }
        })
        .expect("fixture must define a struct named R");
    let r_ty_id = model
        .types
        .iter()
        .find_map(|(ty_id, ty)| match ty {
            Ty::Struct { item, .. } if *item == r_item_id => Some(ty_id),
            _ => None,
        })
        .expect("model must intern Ty::Struct for the R Item");
    let unit_ty_id = model
        .types
        .iter()
        .find_map(|(ty_id, ty)| match ty {
            Ty::Tuple { fields, .. } if fields.is_empty() => Some(ty_id),
            _ => None,
        })
        .expect("model must intern the unit tuple Ty");
    let ptr_to_r_ty_id = model
        .types
        .iter()
        .find_map(|(ty_id, ty)| match ty {
            Ty::Ptr(pointee, Mutability::Mut) if *pointee == r_ty_id => Some(ty_id),
            _ => None,
        })
        .expect("model must intern Ty::Ptr(R, Mutability::Mut) for the drop-glue input");

    // Drop-as-Call collapse: exactly one Function Instance has
    // `flavor == DropGlue` and `virtual_idx == None`, anchored on a
    // synthetic Item carrying `SyntheticItemKind::DropGlue { .. }`.
    let drop_glue_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| {
            let Some(func) = inst.kind.as_function() else {
                return false;
            };
            func.flavor == FunctionInstanceFlavor::DropGlue && func.virtual_idx.is_none()
        })
        .collect();
    assert!(
        !drop_glue_instances.is_empty(),
        "expected at least one DropGlue-flavored Function Instance; got none",
    );

    // The drop-glue Instance for R (`<Reg as Drop>::drop` synthesised
    // glue) must be among them. Match by Item synthetic-kind shape
    // rather than by source-side `<Reg as Drop>::drop` def_path
    // (Codex-review-confirmed assertion shape). Verify the
    // `self_ty: ArgsKey` payload matches the canonical key for R's
    // TyId per the construction at
    // `crates/regseq-driver/src/bridge/synth/drop_glue.rs:22-26`.
    let expected_self_ty = args_key_for_generic_args(&model, &vec![GenericArg::Type(r_ty_id)]);
    let r_drop_glue_instances: Vec<&regseq_model::Instance> = drop_glue_instances
        .into_iter()
        .filter(|inst| {
            matches!(
                &model.item_synthetic_kinds[inst.item],
                SyntheticItemKind::DropGlue { self_ty } if *self_ty == expected_self_ty
            )
        })
        .collect();
    assert!(
        !r_drop_glue_instances.is_empty(),
        "expected at least one DropGlue Instance whose Item carries SyntheticItemKind::DropGlue {{ self_ty: <key for R> }}; got none. \
         Drop-glue Items in model: {:?}",
        model
            .item_synthetic_kinds
            .iter()
            .filter_map(|kind| match kind {
                SyntheticItemKind::DropGlue { self_ty } => Some(self_ty.clone()),
                _ => None,
            })
            .collect::<Vec<_>>(),
    );

    // At least one of those Instances must be the concrete drop-glue
    // synthesised at the `Into::into(self)` Drop terminator: its
    // FnSig shape is `unsafe fn(*mut R) -> ()` with `FnAbi::Rust`.
    let drop_glue_inst = r_drop_glue_instances
        .iter()
        .find(|inst| {
            let func = inst.kind.as_function().expect("checked above");
            let sig = model.sigs.resolve(func.fn_sig);
            sig.inputs == vec![ptr_to_r_ty_id]
                && sig.output == unit_ty_id
                && sig.abi == FnAbi::Rust
                && sig.safety == Safety::Unsafe
                && !sig.c_variadic
        })
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "no DropGlue Instance carries the expected `unsafe fn(*mut R) -> ()` FnSig shape; \
                 inspected: {:?}",
                r_drop_glue_instances
                    .iter()
                    .map(|inst| {
                        let func = inst.kind.as_function().unwrap();
                        (func.fn_sig, model.sigs.resolve(func.fn_sig).clone())
                    })
                    .collect::<Vec<_>>(),
            );
        });

    // Spec.md section 12 step 5: the drop-glue Rust-ABI FnSig slot is
    // `None` (already covered globally by `assert_some_iff_c_abi`,
    // but pinned here as a direct per-Instance assertion).
    let drop_glue_func = drop_glue_inst.kind.as_function().expect("checked above");
    assert_eq!(
        model.sig_c_abi_lowerings[drop_glue_func.fn_sig], None,
        "drop-glue FnSig (Rust ABI) must have a `None` sig_c_abi_lowerings slot per spec.md section 12 step 5",
    );
}

#[test]
fn closure_call_fixture_bridges_without_parallel_table_desync() {
    let model = bridge_fixture(CLOSURE_CALL_FIXTURE);

    assert_parallel_table_length(&model);
    assert_some_iff_c_abi(&model);

    // Whether the closure-call FnSig synth runs through the eager
    // `bridge/mod.rs:2278-2305` path (slot pre-paired) or the
    // `terminator.rs:316-331` bare-intern path (slot paired by
    // Change B's `ensure_cabi_lowering_slots` insertion), the
    // closure-call Function Instance(s) must still satisfy the
    // parallel-table invariant. The first-class assertion is the
    // global length + per-entry check above; this branch records
    // the structural presence of at least one closure-call Item
    // for trace clarity.
    let closure_call_count = model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. }))
        .count();
    assert!(
        closure_call_count >= 1,
        "fixture must produce at least one SyntheticItemKind::ClosureCall Item; got {closure_call_count}",
    );
}

#[test]
fn fn_ptr_translation_fixture_bridges_without_parallel_table_desync() {
    let model = bridge_fixture(FN_PTR_FIXTURE);

    assert_parallel_table_length(&model);
    assert_some_iff_c_abi(&model);

    // Spec.md section 12 step 5: at least one FnPtr-derived FnSig
    // with `FnAbi::Rust` lands with a `None` slot; at least one
    // FnPtr-derived FnSig with `FnAbi::C` lands with a
    // `Some(CAbiLowering)` slot and `inputs.len() == 1` matching
    // the source `extern "C" fn(u32) -> u32` shape.
    let fn_ptr_sig_ids: Vec<regseq_model::FnSigId> = model
        .types
        .iter()
        .filter_map(|(_, ty)| match ty {
            Ty::FnPtr(sig_id) => Some(*sig_id),
            _ => None,
        })
        .collect();
    assert!(
        !fn_ptr_sig_ids.is_empty(),
        "fixture must produce at least one Ty::FnPtr entry in TyInterner",
    );

    let has_rust_abi_fn_ptr_with_none = fn_ptr_sig_ids.iter().any(|sig_id| {
        let sig = model.sigs.resolve(*sig_id);
        sig.abi == FnAbi::Rust && model.sig_c_abi_lowerings[*sig_id].is_none()
    });
    assert!(
        has_rust_abi_fn_ptr_with_none,
        "expected at least one FnPtr-derived FnSig with FnAbi::Rust and a `None` sig_c_abi_lowerings slot",
    );

    let has_c_abi_fn_ptr_with_some = fn_ptr_sig_ids.iter().any(|sig_id| {
        let sig = model.sigs.resolve(*sig_id);
        sig.abi == FnAbi::C && sig.inputs.len() == 1 && model.sig_c_abi_lowerings[*sig_id].is_some()
    });
    assert!(
        has_c_abi_fn_ptr_with_some,
        "expected at least one FnPtr-derived FnSig with FnAbi::C, inputs.len() == 1, and a Some(CAbiLowering) sig_c_abi_lowerings slot; got: {:?}",
        fn_ptr_sig_ids
            .iter()
            .map(|sig_id| {
                let sig = model.sigs.resolve(*sig_id);
                (*sig_id, sig.abi, sig.inputs.len(), model.sig_c_abi_lowerings[*sig_id].is_some())
            })
            .collect::<Vec<_>>(),
    );
}
