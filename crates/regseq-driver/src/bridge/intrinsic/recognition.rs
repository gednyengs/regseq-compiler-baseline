//! Generic per-callee `#[rseq::intrinsic("...")]` recognition.
//!
//! This is the shared dispatch used by every intrinsic family — diagnostic
//! logging and register-access MMIO/poll. It
//! reads the attribute, maps the closed NAME vocabulary to an
//! `IntrinsicKind`, redundantly gates on the callee's function name, and
//! records the marker on the model. It is construct-neutral on purpose;
//! family-specific logic lives in each construct's bridge module.

use regseq_model::{
    AtomicOp, AttrData, ClockOp, ConfigOp, DeferredWorkOp, DefPathData, FetchOp, IntrinsicKind,
    ItemId, LogLevel, MmioOp, PinctrlOp, RegseqModel,
};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

pub(crate) fn recognise_intrinsic_attr(
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

pub(crate) fn callee_name_matches_kind(
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

pub(crate) fn record_intrinsic(model: &mut RegseqModel, item: ItemId, kind: IntrinsicKind) {
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
        // Bus config-space intrinsics (bus-resource-management construct #3 M5).
        "read_config" => Some(IntrinsicKind::Config(ConfigOp::Read)),
        "write_config" => Some(IntrinsicKind::Config(ConfigOp::Write)),
        // Timed-CPU-delay-and-sleep intrinsics (timed-cpu-delay-and-sleep
        // construct): the `regseq::delay::{busy_wait,sleep}` facade, whose
        // re-exports resolve to the attributed `intrinsics::timed_delay::*`
        // originals.
        "busy_wait" => Some(IntrinsicKind::BusyWait),
        "sleep" => Some(IntrinsicKind::Sleep),
        // Atomic-memory-access-and-ordering intrinsics (atomicity construct):
        // the `Atomic<T>` wrapper methods + the `regseq::atomic::fence` free
        // function. Keep in sync with `regseq_model::IntrinsicKind::from_name`.
        "atomic_load" => Some(IntrinsicKind::Atomic(AtomicOp::Load)),
        "atomic_store" => Some(IntrinsicKind::Atomic(AtomicOp::Store)),
        "atomic_fetch_add" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Add))),
        "atomic_fetch_sub" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Sub))),
        "atomic_fetch_or" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Or))),
        "atomic_fetch_and" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::And))),
        "atomic_fetch_xor" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Xor))),
        "atomic_swap" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Swap))),
        "atomic_compare_exchange" => Some(IntrinsicKind::Atomic(AtomicOp::CompareExchange)),
        "atomic_bit_test_set" => Some(IntrinsicKind::Atomic(AtomicOp::BitTestSet)),
        "atomic_bit_test_clear" => Some(IntrinsicKind::Atomic(AtomicOp::BitTestClear)),
        "atomic_fence" => Some(IntrinsicKind::Atomic(AtomicOp::Fence)),
        // Peripheral-clock-management intrinsics (clock construct): the `Clock`
        // wrapper methods (`enable`/`disable`/`get_rate`). Keep in sync with
        // `regseq_model::IntrinsicKind::from_name`.
        "clock_enable" => Some(IntrinsicKind::Clock(ClockOp::Enable)),
        "clock_disable" => Some(IntrinsicKind::Clock(ClockOp::Disable)),
        "clock_get_rate" => Some(IntrinsicKind::Clock(ClockOp::GetRate)),
        // Pin-controller-and-GPIO-configuration intrinsic (pinctrl
        // state-selection construct): the `PinCtrl::apply::<S>()` /
        // `OptionalPinCtrl::apply::<S>()` wrapper method. Keep in sync with
        // `regseq_model::IntrinsicKind::from_name`.
        "pinctrl_apply" => Some(IntrinsicKind::Pinctrl(PinctrlOp::ApplyState)),
        // Deferred-work-item-scheduling intrinsic (deferred-work-item-scheduling
        // construct): the `Work::submit()` wrapper method. Keep in sync with
        // `regseq_model::IntrinsicKind::from_name`. (Codex pre-finalize
        // BLOCKING-1: this driver-side mapping — not the model `from_name` — is
        // what `materialise_intrinsics` keys on, so without it `Work::submit`
        // stays untagged and never reaches `emit_deferred_work`.)
        "deferred_work_submit" => Some(IntrinsicKind::DeferredWork(DeferredWorkOp::Submit)),
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
        // The `regseq::bus::{read_config,write_config}` free functions; the
        // `#[rseq::intrinsic]` attribute is the authority, the name a redundant
        // gate (bus-resource-management construct #3 M5).
        IntrinsicKind::Config(op) => matches!(
            (name, op),
            ("read_config", ConfigOp::Read) | ("write_config", ConfigOp::Write)
        ),
        // The `regseq::delay::{busy_wait,sleep}` free functions
        // (timed-cpu-delay-and-sleep construct); the `#[rseq::intrinsic]`
        // attribute is the authority, the name a redundant gate.
        IntrinsicKind::BusyWait => name == "busy_wait",
        IntrinsicKind::Sleep => name == "sleep",
        // The `Atomic<T>` wrapper methods + the `regseq::atomic::fence` free
        // function (atomicity construct). The `#[rseq::intrinsic("atomic_*")]`
        // attribute is the authority; the leaf method/fn name is a redundant
        // gate (the leaf is the short form: `load`, `fetch_add`, `fence`, …).
        IntrinsicKind::Atomic(op) => match op {
            AtomicOp::Load => name == "load",
            AtomicOp::Store => name == "store",
            AtomicOp::FetchModify(FetchOp::Add) => name == "fetch_add",
            AtomicOp::FetchModify(FetchOp::Sub) => name == "fetch_sub",
            AtomicOp::FetchModify(FetchOp::Or) => name == "fetch_or",
            AtomicOp::FetchModify(FetchOp::And) => name == "fetch_and",
            AtomicOp::FetchModify(FetchOp::Xor) => name == "fetch_xor",
            AtomicOp::FetchModify(FetchOp::Swap) => name == "swap",
            AtomicOp::CompareExchange => name == "compare_exchange",
            AtomicOp::BitTestSet => name == "bit_test_set",
            AtomicOp::BitTestClear => name == "bit_test_clear",
            AtomicOp::Fence => name == "fence",
        },
        // The `Clock` wrapper methods (peripheral-clock-management construct).
        // The `#[rseq::intrinsic("clock_*")]` attribute is the authority; the
        // leaf method name is a redundant gate (the leaf is the short form:
        // `enable`, `disable`, `get_rate`).
        IntrinsicKind::Clock(op) => match op {
            ClockOp::Enable => name == "enable",
            ClockOp::Disable => name == "disable",
            ClockOp::GetRate => name == "get_rate",
        },
        // The `PinCtrl`/`OptionalPinCtrl` wrapper method (pinctrl
        // state-selection construct). The `#[rseq::intrinsic("pinctrl_apply")]`
        // attribute is the authority; the leaf method name is a redundant gate
        // (the leaf is the short form: `apply`).
        IntrinsicKind::Pinctrl(op) => match op {
            PinctrlOp::ApplyState => name == "apply",
        },
        // The `Work::submit()` wrapper method (deferred-work-item-scheduling
        // construct). The `#[rseq::intrinsic("deferred_work_submit")]` attribute
        // is the authority; the leaf method name is a redundant gate (the leaf is
        // the short form: `submit`).
        IntrinsicKind::DeferredWork(op) => match op {
            DeferredWorkOp::Submit => name == "submit",
        },
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

        record_intrinsic(&mut model, ItemId::from_u32(0), kind);

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
    fn recognises_config_space_intrinsics() {
        // Bus-resource-management construct #3 M5: the `regseq::bus::read_config`/
        // `write_config` free functions map to `IntrinsicKind::Config(op)` and the
        // redundant fn-name gate accepts them.
        for (leaf, kind) in [
            ("read_config", IntrinsicKind::Config(regseq_model::ConfigOp::Read)),
            ("write_config", IntrinsicKind::Config(regseq_model::ConfigOp::Write)),
        ] {
            assert_eq!(intrinsic_kind_for_name(leaf), Some(kind.clone()));
            let mut model = model_with_item_attrs(1);
            let def_path = def_path(&mut model, leaf);
            assert!(callee_name_matches_kind(&model, &def_path, &kind));
            record_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
            assert_eq!(
                model.domain_attrs.item_attrs[ItemId::from_u32(0)],
                vec![AttrData::Intrinsic(kind)]
            );
        }
    }

    #[test]
    fn recognises_timed_delay_intrinsics() {
        // Timed-cpu-delay-and-sleep construct: the `regseq::delay::busy_wait`/
        // `sleep` facade functions map to `IntrinsicKind::BusyWait`/`Sleep`,
        // and the redundant fn-name gate accepts their leaf names (the
        // re-export resolves to the attributed original, leaf `busy_wait` /
        // `sleep`).
        for (leaf, kind) in [
            ("busy_wait", IntrinsicKind::BusyWait),
            ("sleep", IntrinsicKind::Sleep),
        ] {
            assert_eq!(intrinsic_kind_for_name(leaf), Some(kind.clone()));
            let mut model = model_with_item_attrs(1);
            let def_path = def_path(&mut model, leaf);
            assert!(callee_name_matches_kind(&model, &def_path, &kind));
            record_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
            assert_eq!(
                model.domain_attrs.item_attrs[ItemId::from_u32(0)],
                vec![AttrData::Intrinsic(kind)]
            );
        }
        // A mismatched leaf name is rejected by the redundant gate.
        let mut model = model_with_item_attrs(1);
        let dp = def_path(&mut model, "sleep");
        assert!(!callee_name_matches_kind(
            &model,
            &dp,
            &IntrinsicKind::BusyWait
        ));
    }

    #[test]
    fn recognises_atomic_intrinsics() {
        // Atomicity construct: the `atomic_*` tags map to `IntrinsicKind::Atomic`
        // and the redundant fn-name gate accepts each op's short leaf name
        // (`load`, `fetch_add`, `compare_exchange`, `bit_test_clear`, `fence`, …).
        let cases = [
            ("atomic_load", "load", AtomicOp::Load),
            ("atomic_store", "store", AtomicOp::Store),
            (
                "atomic_fetch_add",
                "fetch_add",
                AtomicOp::FetchModify(FetchOp::Add),
            ),
            ("atomic_swap", "swap", AtomicOp::FetchModify(FetchOp::Swap)),
            (
                "atomic_compare_exchange",
                "compare_exchange",
                AtomicOp::CompareExchange,
            ),
            ("atomic_bit_test_set", "bit_test_set", AtomicOp::BitTestSet),
            (
                "atomic_bit_test_clear",
                "bit_test_clear",
                AtomicOp::BitTestClear,
            ),
            ("atomic_fence", "fence", AtomicOp::Fence),
        ];
        for (tag, leaf, op) in cases {
            let kind = IntrinsicKind::Atomic(op);
            assert_eq!(intrinsic_kind_for_name(tag), Some(kind.clone()), "tag {tag}");
            let mut model = model_with_item_attrs(1);
            let def_path = def_path(&mut model, leaf);
            assert!(
                callee_name_matches_kind(&model, &def_path, &kind),
                "leaf {leaf}"
            );
            record_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
            assert_eq!(
                model.domain_attrs.item_attrs[ItemId::from_u32(0)],
                vec![AttrData::Intrinsic(kind)]
            );
        }
        // A mismatched leaf name is rejected by the redundant gate.
        let mut model = model_with_item_attrs(1);
        let dp = def_path(&mut model, "store");
        assert!(!callee_name_matches_kind(
            &model,
            &dp,
            &IntrinsicKind::Atomic(AtomicOp::Load)
        ));
    }

    #[test]
    fn recognises_deferred_work_submit_intrinsic() {
        // Deferred-work-item-scheduling construct: the `deferred_work_submit`
        // tag maps to `IntrinsicKind::DeferredWork(Submit)` and the redundant
        // fn-name gate accepts the op's short leaf name (`submit`). This is the
        // driver-side vocabulary `materialise_intrinsics` keys on (Codex
        // BLOCKING-1) — without it `Work::submit` never reaches lowering.
        let kind = IntrinsicKind::DeferredWork(DeferredWorkOp::Submit);
        assert_eq!(intrinsic_kind_for_name("deferred_work_submit"), Some(kind.clone()));
        let mut model = model_with_item_attrs(1);
        let submit_path = def_path(&mut model, "submit");
        assert!(callee_name_matches_kind(&model, &submit_path, &kind));
        record_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
        assert_eq!(
            model.domain_attrs.item_attrs[ItemId::from_u32(0)],
            vec![AttrData::Intrinsic(kind)]
        );
        // A mismatched leaf name is rejected by the redundant gate; the reserved
        // `submit_unique` op is not in the vocabulary.
        assert_eq!(intrinsic_kind_for_name("deferred_work_submit_unique"), None);
        let mut model = model_with_item_attrs(1);
        let dp = def_path(&mut model, "submit_unique");
        assert!(!callee_name_matches_kind(
            &model,
            &dp,
            &IntrinsicKind::DeferredWork(DeferredWorkOp::Submit)
        ));
    }

    #[test]
    fn recording_is_idempotent_for_same_callee() {
        let mut model = model_with_item_attrs(1);
        let def_path = def_path(&mut model, "log-warn");
        let kind = IntrinsicKind::Log(LogLevel::Warn);
        assert!(callee_name_matches_kind(&model, &def_path, &kind));

        record_intrinsic(&mut model, ItemId::from_u32(0), kind.clone());
        record_intrinsic(&mut model, ItemId::from_u32(0), kind);

        assert_eq!(
            model.domain_attrs.item_attrs[ItemId::from_u32(0)],
            vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))]
        );
    }

}
