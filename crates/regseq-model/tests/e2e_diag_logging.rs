//! P1a-E2E-02: diagnostic-logging end-to-end fixture.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/diag_logging.rs` through
//! the full phase-1a `regseq-driver` pipeline and asserts the
//! post-bridge `RegseqModel` carries the spec.md section 26.6 shape:
//! four flat-arg `Call` Terminators (one per severity), each with a
//! `&'static str` fmt operand plus a flat `u32` value, each callee
//! `Item` decorated with `IntrinsicKind::Log(LogLevel::*)`, no
//! surviving `LogArg` carrier ADT, and a frozen pretty-printed
//! snapshot.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::HashSet;

use regseq_model::{
    AttrData, Body, BridgeInput, CallCallee, Const, DefPathData, InstanceId, IntrinsicKind, ItemId,
    LogLevel, Operand, Provenance, RegseqModel, TerminatorKind, Ty, TypeRecovery, UintKind,
};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

/// A single post-peel diagnostic-logging Call collected from the
/// fixture's `emit_logs` body, paired with the owning body so the
/// per-call operand type recovery uses the right local declarations.
struct LogCall<'m> {
    args: &'m [Operand],
    body: &'m Body,
    callee_inst: InstanceId,
    callee_item: ItemId,
}

#[test]
fn diag_logging_post_peel_shape_and_attrs() {
    let mut model = bridge_fixture(fixture_names::DIAG_LOGGING);

    // Section A: validation pass.
    let bridge_input = BridgeInput::matching(&model);
    model.validate(&bridge_input).expect("validate must pass");

    // Section B: locate the four post-peel Call terminators.
    let calls = collect_log_calls(&model);
    assert_eq!(
        calls.len(),
        4,
        "expected one Call per severity (debug/info/warn/error)"
    );

    // Section C: assert post-peel arity + flat-arg shape per Call.
    //
    // Note on args[0] Ty recovery: `regseq_model::operand_ty` returns
    // `Malformed` for `Operand::Constant(_)` carrying a
    // `Const::Aggregate` because aggregate consts do not store a
    // surface Ty. We therefore pin the fmt template's Ty via TWO
    // structural checks rather than one operand_ty call:
    //   (1) the const's payload matches the §9.7 reference-typed
    //       Const::Aggregate([Reloc, Uint(Usize, len)]) shape, AND
    //   (2) the callee's declared FnSig inputs[0] is
    //       Ty::Ref(Ty::Str, SharedBorrow) per §26.6.
    // Together (1)+(2) constrain the same invariant a single
    // operand_ty assertion would constrain if recovery were possible.
    for call in &calls {
        // C5: arity = 1 fmt + 1 flat for the one-`u32` fixture (per
        // spec.md:11792+ post-DiagLogPeel arity is 1 + N).
        assert_eq!(
            call.args.len(),
            2,
            "diag_logging fixture passes one u32 -> post-peel arity = 2"
        );

        // C5a: args[0] is an `Operand::Constant(...)` carrying the
        // `&'static str` format template (spec.md:9658+, 9728-9740).
        let fmt_const_id = match call.args[0] {
            Operand::Constant(id) => id,
            ref other => panic!(
                "post-peel args[0] must be Operand::Constant for &str fmt template; got {other:?}"
            ),
        };

        // C5b: args[0]'s const carries the §9.7 reference-typed const
        // shape for a `&'static str`. Per spec.md:9734-9737 the
        // post-Regseq-lowering Const shape for the fmt template is
        // `Const::Aggregate([Reloc(<static>), Uint(Usize, len)])`.
        assert_str_const_aggregate(&model, fmt_const_id);

        // C5b (continued): the callsite operand Ty matches the §26.6
        // exception, i.e. `Ty::Ref(Ty::Str, SharedBorrow)`
        // (spec.md:9728-9729). After the post-DiagLogPeel DCE pass
        // the log callee's declared FnSig drops the LogArg slice arg,
        // so inputs[0] is the &str fmt template; we verify the
        // callee's declared input matches the §26.6 invariant.
        let callee_data = model.instances[call.callee_inst]
            .kind
            .as_function()
            .expect("log callee resolves to a Function Instance");
        let callee_sig = model.sigs.resolve(callee_data.fn_sig());
        assert!(
            !callee_sig.inputs.is_empty(),
            "log callee FnSig must declare at least the &str fmt input"
        );
        let fmt_input_ty = callee_sig.inputs[0];
        match model.types.resolve(fmt_input_ty) {
            Ty::Ref(pointee, Provenance::SharedBorrow) => {
                assert_eq!(
                    model.types.resolve(*pointee),
                    &Ty::Str,
                    "args[0]'s callsite Ty must be &str (Ref to Ty::Str)"
                );
            }
            other => panic!(
                "args[0]'s callsite Ty must be Ty::Ref(Ty::Str, SharedBorrow); got {other:?}"
            ),
        }

        // C5c: args[1] is a flat `u32` operand (NOT a slice
        // aggregate, NOT a `LogArg::U32` const wrapper). The
        // operand's resolved Ty is `Ty::Uint(UintKind::U32)`
        // (spec.md:9742-9760).
        let arg1_ty = match regseq_model::operand_ty(&model, call.body, &call.args[1]) {
            TypeRecovery::Recovered(id) => id,
            other => panic!("args[1] operand type recovery did not resolve: {other:?}"),
        };
        assert_eq!(
            model.types.resolve(arg1_ty),
            &Ty::Uint(UintKind::U32),
            "post-peel args[1] must be Ty::Uint(UintKind::U32)"
        );

        // C6: no 2-input pre-peel shape survives -- args[1] must not
        // resolve to a slice / array of LogArg (Operand::Constant
        // would still be possible for `&[]` but only when the carrier
        // survives, which Section D rules out separately).
        assert!(
            !matches!(call.args[1], Operand::Constant(_)),
            "post-peel args[1] is the loaded u32 operand, not a slice constant"
        );
    }

    // Section D: LogArg carrier-survival negative assertion.
    assert_no_logarg_in_model(&model);

    // Section E: assert each callee carries IntrinsicKind::Log(_).
    let observed_levels = collect_log_levels(&model, &calls);
    assert_levels_match_fixture(&observed_levels);

    // E12: no spurious AttrData variants on the four callee Items.
    //
    // The exhaustive `match` below is intentional and load-bearing AS A
    // COMPILE-TIME GUARD: it enumerates every AttrData variant rather
    // than using a catch-all, so when future variants land per spec.md
    // §1474-1489 (anticipated additions to AttrData / IntrinsicKind),
    // this file stops compiling -- forcing each new variant to be
    // handled explicitly here. The correct response to that compile
    // failure is to decide whether the new variant is permitted on a log
    // callee Item and either (a) accept it with an explicit no-op arm, or
    // (b) panic with a "spurious AttrData on log callee" message (as done
    // for FrameworkType below). Do NOT add a catch-all `_ =>` arm because
    // that silently admits new variants.
    for call in &calls {
        let attrs = &model.domain_attrs.item_attrs[call.callee_item];
        for attr in attrs.iter() {
            // Explicit handling per the guard contract above: a log callee
            // Item may only carry the Intrinsic(Log) attribute. Any other
            // AttrData variant (such as the framework-type marker) is
            // spurious here and fails the test loudly rather than being
            // silently admitted.
            match attr {
                AttrData::Intrinsic(IntrinsicKind::Log(_)) => {}
                AttrData::Intrinsic(_) => {
                    panic!("spurious non-log intrinsic (mmio/poll) on log callee")
                }
                AttrData::FrameworkType(_) => {
                    panic!("spurious AttrData on log callee: FrameworkType")
                }
                AttrData::InterruptResource { .. } => {
                    panic!("spurious AttrData on log callee: InterruptResource")
                }
                AttrData::IsrBinding { .. } => {
                    panic!("spurious AttrData on log callee: IsrBinding")
                }
                AttrData::Atomic { .. } => {
                    panic!("spurious AttrData on log callee: Atomic")
                }
                AttrData::SyncResource { .. } => {
                    panic!("spurious AttrData on log callee: SyncResource")
                }
                AttrData::ClockResource { .. } => {
                    panic!("spurious AttrData on log callee: ClockResource")
                }
                AttrData::PinctrlResource { .. } => {
                    panic!("spurious AttrData on log callee: PinctrlResource")
                }
                AttrData::PinStateMarker { .. } => {
                    panic!("spurious AttrData on log callee: PinStateMarker")
                }
                AttrData::DmaRegionResource { .. } => {
                    panic!("spurious AttrData on log callee: DmaRegionResource")
                }
                AttrData::DmaChannelResource { .. } => {
                    panic!("spurious AttrData on log callee: DmaChannelResource")
                }
                AttrData::DmaCompletionBinding { .. } => {
                    panic!("spurious AttrData on log callee: DmaCompletionBinding")
                }
            }
        }
    }

    // Section F: pretty-printed snapshot.
    let rendered = render_default(&model);
    assert!(!rendered.is_empty(), "render must be non-empty");
    insta::assert_snapshot!("diag_logging_render", rendered);
}

/// Locates the fixture's `emit_logs` Function Instance and collects
/// every post-peel log Call from its body (B3-B4). A call is a "log
/// call" when its callee resolves to a Direct InstanceId whose
/// backing Item carries an `AttrData::Intrinsic(IntrinsicKind::Log(_))`
/// entry.
fn collect_log_calls(model: &RegseqModel) -> Vec<LogCall<'_>> {
    let emit_logs_body = find_emit_logs_body(model)
        .expect("diag_logging fixture must produce an emit_logs Function Instance with a body");
    let mut out = Vec::new();
    for block in emit_logs_body.basic_blocks.iter() {
        let TerminatorKind::Call { callee, args, .. } = &block.terminator.kind else {
            continue;
        };
        let CallCallee::Direct(callee_inst) = callee else {
            continue;
        };
        // Spec.md §26.6 names the canonical predicate as
        // `model.instance_intrinsic(id) == Some(IntrinsicKind::Log(_))`;
        // the helper landed by regseq-driver-49 (model.rs:1810-1833)
        // wraps the bounds-checked `domain_attrs.item_attrs[item]`
        // lookup. Routing through it (rather than indexing
        // `domain_attrs.item_attrs` here) keeps the test aligned with
        // the spec's canonical form and inherits any future
        // preconditions the helper grows.
        if !matches!(
            model.instance_intrinsic(*callee_inst),
            Some(IntrinsicKind::Log(_)),
        ) {
            continue;
        }
        let callee_item = model.instances[*callee_inst].item;
        out.push(LogCall {
            args: args.as_ref(),
            body: emit_logs_body,
            callee_inst: *callee_inst,
            callee_item,
        });
    }
    out
}

/// Locates the body of the fixture's `emit_logs` Function Instance by
/// matching the terminal `def_path` segment name. The diag_logging
/// fixture defines exactly one `emit_logs` function (per
/// `crates/test-ir-fixtures/fixtures/diag_logging.rs:120`).
fn find_emit_logs_body(model: &RegseqModel) -> Option<&Body> {
    for instance in model.instances.iter() {
        let Some(data) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = data.body.as_ref() else {
            continue;
        };
        let name = item_terminal_segment_name(model, instance.item);
        if name.as_deref() == Some("emit_logs") {
            return Some(body);
        }
    }
    None
}

/// Section D: assert no `LogArg` ADT or Item survives the
/// post-DiagLogPeel residue-driven DCE (spec.md:11962-11963).
///
/// The walk covers both branches of the carrier-survival rule:
///
/// - D7: every `Ty::Adt` / `Ty::Struct` / `Ty::Enum` / `Ty::Union`
///   in the type interner must NOT resolve to an Item whose
///   def_path terminal segment renders as `LogArg`.
/// - D8: every Item in `model.items` must NOT have a rendered
///   `def_path` containing the substring `"LogArg"`.
fn assert_no_logarg_in_model(model: &RegseqModel) {
    for (ty_id, ty) in model.types.iter() {
        let referenced_item = match ty {
            Ty::Struct { item, .. } | Ty::Enum { item, .. } | Ty::Union { item, .. } => Some(*item),
            _ => None,
        };
        if let Some(item) = referenced_item {
            if item.index() >= model.items.len() {
                continue;
            }
            let name = item_terminal_segment_name(model, item);
            assert_ne!(
                name.as_deref(),
                Some("LogArg"),
                "TyInterner at TyId {} still resolves to a LogArg ADT post-elim",
                ty_id.as_u32(),
            );
        }
    }

    for item in model.items.iter() {
        let rendered = model.strings.resolve(item.def_path.rendered);
        assert!(
            !rendered.contains("LogArg"),
            "Item def_path rendered form still mentions LogArg: {rendered}"
        );
    }
}

/// Section E: for each collected Call, pull the `LogLevel` carried
/// by the callee Item's `AttrData::Intrinsic(IntrinsicKind::Log(_))`
/// attribute and return the multiset.
fn collect_log_levels(model: &RegseqModel, calls: &[LogCall<'_>]) -> HashSet<LogLevel> {
    let mut out = HashSet::new();
    for call in calls {
        let attrs = &model.domain_attrs.item_attrs[call.callee_item];
        let log_attrs: Vec<&AttrData> = attrs
            .iter()
            .filter(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::Log(_))))
            .collect();
        assert_eq!(
            log_attrs.len(),
            1,
            "callee Item must carry exactly one IntrinsicKind::Log(_) attr"
        );
        // `log_attrs` was filtered to Intrinsic(Log) entries above, so any
        // other AttrData variant (such as the framework-type marker) cannot
        // appear here.
        let AttrData::Intrinsic(IntrinsicKind::Log(level)) = log_attrs[0] else {
            unreachable!("log_attrs holds only Intrinsic(Log) entries")
        };
        out.insert(level.clone());
    }
    out
}

/// Section E (continued): assert the four observed levels exactly
/// equal the locked four-severity vocabulary (spec.md section 26.2 +
/// spec.md:1542-1551).
fn assert_levels_match_fixture(observed: &HashSet<LogLevel>) {
    let expected: HashSet<LogLevel> = [
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        observed, &expected,
        "observed LogLevel set must equal the full section 26.2 vocabulary"
    );
}

/// Asserts the const at `id` matches the §9.7 reference-typed const
/// shape for `&'static str`: `Const::Aggregate([Reloc(<static>),
/// Uint(Usize, len)])` per spec.md:9734-9737. This is the structural
/// signature of a `&str` fat pointer after Regseq const lowering --
/// any other Const variant would not have come from a string-literal
/// operand-context Ty of `&str`.
fn assert_str_const_aggregate(model: &RegseqModel, id: regseq_model::ConstId) {
    let parts = match model.consts.resolve(id) {
        Const::Aggregate(parts) => parts,
        other => panic!(
            "args[0] const must be Const::Aggregate([Reloc, Uint(Usize, len)]) per §9.7; got {other:?}"
        ),
    };
    assert_eq!(
        parts.len(),
        2,
        "&str aggregate const must carry exactly (pointer, length) pair"
    );
    match model.consts.resolve(parts[0]) {
        Const::Reloc(_) => (),
        other => panic!(
            "&str aggregate const's first element must be Const::Reloc(<static>); got {other:?}"
        ),
    }
    match model.consts.resolve(parts[1]) {
        Const::Uint(UintKind::Usize, _) => (),
        other => panic!(
            "&str aggregate const's second element must be Const::Uint(Usize, len); got {other:?}"
        ),
    }
}

/// Returns the terminal `DefPath` segment name of `item`, when the
/// segment is name-bearing. Anonymous segments (`Impl`, `Closure`,
/// etc.) yield `None`.
fn item_terminal_segment_name(model: &RegseqModel, item: ItemId) -> Option<String> {
    let segment = model.items[item].def_path.segments.last()?;
    let name_id = match segment.data {
        DefPathData::TypeNs(id)
        | DefPathData::ValueNs(id)
        | DefPathData::MacroNs(id)
        | DefPathData::LifetimeNs(id)
        | DefPathData::OpaqueLifetime(id)
        | DefPathData::AnonAssocTy(id) => id,
        _ => return None,
    };
    Some(model.strings.resolve(name_id).to_string())
}
