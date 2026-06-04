//! Per-callee diagnostic-logging intrinsic recognition.

use regseq_model::{AttrData, DefPathData, IntrinsicKind, ItemId, LogLevel, MmioOp, RegseqModel};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

pub(super) fn recognise_log_intrinsic_attr(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_id: DefId,
) -> Option<IntrinsicKind> {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("intrinsic"),
    ];
    let mut attrs = tcx.get_attrs_by_path(def_id, &path);
    let attr = attrs.next()?;
    if attrs.next().is_some() {
        return None;
    }
    let list = attr.meta_item_list()?;
    let [nested] = list.as_slice() else {
        return None;
    };
    let lit = nested.lit()?;
    let rustc_ast::LitKind::Str(name, _) = lit.kind else {
        return None;
    };
    intrinsic_kind_for_name(name.as_str())
}

pub(super) fn callee_name_matches_kind(
    model: &RegseqModel,
    callee_def_path: &regseq_model::DefPath,
    kind: &IntrinsicKind,
) -> bool {
    let Some(leaf) = callee_def_path.segments.last() else {
        return false;
    };
    let DefPathData::ValueNs(name) = leaf.data else {
        return false;
    };
    fn_name_matches_kind(model.strings.resolve(name), kind)
}

pub(super) fn record_log_intrinsic(model: &mut RegseqModel, item: ItemId, kind: IntrinsicKind) {
    let attrs = &mut model.domain_attrs.item_attrs[item];
    if !attrs
        .iter()
        .any(|attr| matches!(attr, AttrData::Intrinsic(_)))
    {
        attrs.push(AttrData::Intrinsic(kind));
    }
}

#[cfg(test)]
fn intrinsic_name_for_fn_name(name: &str) -> Option<regseq_model::Symbol> {
    use regseq_model::Symbol;
    match name {
        "log_debug" | "log-debug" => Some(Symbol::new("log-debug")),
        "log_info" | "log-info" => Some(Symbol::new("log-info")),
        "log_warn" | "log-warn" => Some(Symbol::new("log-warn")),
        "log_error" | "log-error" => Some(Symbol::new("log-error")),
        _ => None,
    }
}

fn intrinsic_kind_for_name(name: &str) -> Option<IntrinsicKind> {
    // Mirror the model's closed `IntrinsicKind::from_name` vocabulary. Matched
    // here directly on the borrowed `&str` (the model API takes a `Symbol`,
    // whose `new` requires a `'static` string). Keep in sync with
    // `regseq_model::IntrinsicKind::from_name`.
    match name {
        "log-debug" => Some(IntrinsicKind::Log(LogLevel::Debug)),
        "log-info" => Some(IntrinsicKind::Log(LogLevel::Info)),
        "log-warn" => Some(IntrinsicKind::Log(LogLevel::Warn)),
        "log-error" => Some(IntrinsicKind::Log(LogLevel::Error)),
        "mmio_read" => Some(IntrinsicKind::Mmio(MmioOp::Read)),
        "mmio_read_field" => Some(IntrinsicKind::Mmio(MmioOp::ReadField)),
        "mmio_write" => Some(IntrinsicKind::Mmio(MmioOp::Write)),
        "mmio_write_field" => Some(IntrinsicKind::Mmio(MmioOp::WriteField)),
        "mmio_modify" => Some(IntrinsicKind::Mmio(MmioOp::Modify)),
        "poll_until" => Some(IntrinsicKind::PollUntil),
        _ => None,
    }
}

fn fn_name_matches_kind(name: &str, kind: &IntrinsicKind) -> bool {
    match kind {
        IntrinsicKind::Log(level) => matches!(
            (name, level),
            ("log_debug" | "log-debug", LogLevel::Debug)
                | ("log_info" | "log-info", LogLevel::Info)
                | ("log_warn" | "log-warn", LogLevel::Warn)
                | ("log_error" | "log-error", LogLevel::Error)
        ),
        // Register-access wrapper methods on `Mmio<T>`. The `#[rseq::intrinsic]`
        // attribute is the authority; the method-name match is a redundant gate.
        IntrinsicKind::Mmio(op) => matches!(
            (name, op),
            ("read", MmioOp::Read)
                | ("read_field", MmioOp::ReadField)
                | ("write", MmioOp::Write)
                | ("write_field", MmioOp::WriteField)
                | ("modify", MmioOp::Modify)
        ),
        IntrinsicKind::PollUntil => name == "poll_until" || name == "until",
    }
}

#[cfg(test)]
mod tests {
    use regseq_model::{
        AttrData, DefPath, DefPathData, DefPathSegment, IndexVec, ItemId, LogLevel, StableCrateId,
    };

    use super::*;

    fn model_with_item_attrs(count: usize) -> RegseqModel {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        model.domain_attrs.item_attrs = IndexVec::from_elem(Vec::new(), count);
        model
    }

    fn def_path(model: &mut RegseqModel, leaf: &str) -> DefPath {
        let leaf = model.strings.intern(leaf);
        let rendered = model.strings.intern(&format!("test::{leaf:?}"));
        DefPath {
            crate_id: model.primary_crate,
            segments: vec![DefPathSegment {
                data: DefPathData::ValueNs(leaf),
                disambiguator: 0,
            }],
            rendered,
        }
    }

    fn assert_records(leaf: &str, level: LogLevel) {
        let mut model = model_with_item_attrs(1);
        let def_path = def_path(&mut model, leaf);
        let kind = IntrinsicKind::from_name(intrinsic_name_for_fn_name(leaf).unwrap())
            .expect("log name should map");
        assert!(callee_name_matches_kind(&model, &def_path, &kind));

        record_log_intrinsic(&mut model, ItemId::from_u32(0), kind);

        assert_eq!(
            model.domain_attrs.item_attrs[ItemId::from_u32(0)],
            vec![AttrData::Intrinsic(IntrinsicKind::Log(level))]
        );
    }

    #[test]
    fn recognises_each_log_level() {
        assert_records("log-debug", LogLevel::Debug);
        assert_records("log-info", LogLevel::Info);
        assert_records("log-warn", LogLevel::Warn);
        assert_records("log-error", LogLevel::Error);
    }

    #[test]
    fn ignores_unrelated_callees() {
        let mut model = model_with_item_attrs(1);
        let def_path = def_path(&mut model, "regular_function");

        assert!(!callee_name_matches_kind(
            &model,
            &def_path,
            &IntrinsicKind::Log(LogLevel::Debug)
        ));
        assert!(model.domain_attrs.item_attrs[ItemId::from_u32(0)].is_empty());
    }

    #[test]
    fn recording_is_idempotent_for_same_callee() {
        let mut model = model_with_item_attrs(1);
        let def_path = def_path(&mut model, "log-warn");
        let kind = IntrinsicKind::Log(LogLevel::Warn);
        assert!(callee_name_matches_kind(&model, &def_path, &kind));

        record_log_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
        record_log_intrinsic(&mut model, ItemId::from_u32(0), kind);

        assert_eq!(
            model.domain_attrs.item_attrs[ItemId::from_u32(0)],
            vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))]
        );
    }
}
