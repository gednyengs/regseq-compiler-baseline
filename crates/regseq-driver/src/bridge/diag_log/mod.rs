//! Diagnostic-logging-specific bridge support.
//!
//! The generic `#[rseq::intrinsic("...")]` recognition + materialisation that
//! used to live here (and was shared by register-access MMIO/poll) now lives
//! in the construct-neutral `crate::bridge::intrinsic` module. This module
//! keeps only the diagnostic-logging-specific concerns: the
//! `<LogArg<'_> as From<T>>::from` peeling recognizer, format-template
//! parsing, log-call validation, and the peel/dead-item-elim passes.

pub mod dead_item_elim;
pub mod format;
pub mod peel;
pub mod validation;

pub use format::{parse_format_template, FormatSpecifier, ParsedFormatTemplate};
pub use peel::diag_log_peel_pass;
pub use validation::validate_log_call;

use regseq_model::{DefPathData, InstanceId, InstanceKind, Ty};

/// Structural recognition of `<LogArg<'_> as From<T>>::from` Call
/// callees per spec.md section 26.4 + section 26.5.
///
/// Per the regseq-driver-56 amendment at spec.md:2354-2394, at a
/// direct call site the bridge resolves
/// `ty::Instance::try_resolve(.., fully_monomorphized, def_id,
/// generic_args)` before interning the callee Item, so
/// `InstanceKind::Item` keys on the resolved impl-method DefId
/// rather than the trait-declaration DefId. The recognizer therefore
/// matches a single canonical impl-method shape:
///
/// - the callee Item's def_path terminal segment is the value
///   `from`,
/// - the path contains a `DefPathData::Impl` ancestor segment, and
/// - the Function Instance's `FnSig.output` resolves to a
///   `Ty::Enum`, `Ty::Struct`, or `Ty::Union` whose Item's def_path
///   terminal segment renders as `LogArg`.
///
/// Shared between `peel.rs::peel_log_arg_from_wrapper` (per Phase-5
/// sub-pass) and `dead_item_elim::eliminate_diag_log_peel_residue`
/// (per regseq-driver-48 Change B) so the recognition logic stays
/// in one place.
pub(crate) fn is_from_logarg_instance(
    model: &regseq_model::RegseqModel,
    instance_id: InstanceId,
) -> bool {
    if instance_id.index() >= model.instances.len() {
        return false;
    }
    let instance = &model.instances[instance_id];
    let item = instance.item;
    if item.index() >= model.items.len() {
        return false;
    }
    let def_path = &model.items[item].def_path;

    // Canonical impl-method recognizer: terminal `ValueNs("from")`
    // segment, some `DefPathData::Impl` ancestor, and the Function
    // Instance's `FnSig.output` resolves to a `LogArg`-named ADT.
    let Some(last) = def_path.segments.last() else {
        return false;
    };
    let DefPathData::ValueNs(name_id) = last.data else {
        return false;
    };
    if model.strings.resolve(name_id) != "from" {
        return false;
    }
    if !def_path
        .segments
        .iter()
        .any(|s| matches!(s.data, DefPathData::Impl))
    {
        return false;
    }
    let InstanceKind::Function(function) = &instance.kind else {
        return false;
    };
    if function.fn_sig.index() >= model.sigs.len() {
        return false;
    }
    let sig = model.sigs.resolve(function.fn_sig);
    ty_terminal_segment_is_logarg(model, sig.output)
}

fn ty_terminal_segment_is_logarg(
    model: &regseq_model::RegseqModel,
    ty_id: regseq_model::TyId,
) -> bool {
    if ty_id.index() >= model.types.len() {
        return false;
    }
    let nominal_item = match model.types.resolve(ty_id) {
        Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => *item,
        _ => return false,
    };
    if nominal_item.index() >= model.items.len() {
        return false;
    }
    let path_id = model.items[nominal_item].def_path.rendered;
    let path = model.strings.resolve(path_id);
    path.rsplit("::")
        .next()
        .map(|seg| seg == "LogArg")
        .unwrap_or(false)
}
