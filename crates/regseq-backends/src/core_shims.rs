//! Backend-shared classification of bodyless `core` helpers.
//!
//! Classification is model-structural.  Each C backend remains responsible
//! for spelling the classified helper in its own native translation unit.

use regseq_model::{
    DefPathData, FnAbi, FnSig, FunctionInstanceFlavor, GenericArg, InstanceId, ItemKind, Linkage,
    Mutability, Provenance, RegseqModel, Safety, Ty, UintKind,
};

#[derive(Clone, Copy)]
pub(crate) enum BackendShim {
    NullMut,
    U64Default,
    PartialEqNe { eq: InstanceId },
}

impl BackendShim {
    pub(crate) fn param_name(self, index: usize) -> Option<&'static str> {
        match (self, index) {
            (Self::PartialEqNe { .. }, 0) => Some("lhs"),
            (Self::PartialEqNe { .. }, 1) => Some("rhs"),
            _ => None,
        }
    }

    pub(crate) fn dependency(self) -> Option<InstanceId> {
        match self {
            Self::PartialEqNe { eq } => Some(eq),
            Self::NullMut | Self::U64Default => None,
        }
    }
}

fn named_segment(
    model: &RegseqModel,
    segment: &regseq_model::DefPathSegment,
    expected: &str,
    value_namespace: bool,
) -> bool {
    if segment.disambiguator != 0 {
        return false;
    }
    match &segment.data {
        DefPathData::ValueNs(name) if value_namespace => model.strings.resolve(*name) == expected,
        DefPathData::TypeNs(name) if !value_namespace => model.strings.resolve(*name) == expected,
        _ => false,
    }
}

fn exact_core_item(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    let path = &model.items[item].def_path;
    if path.crate_id == model.primary_crate || path.crate_id.index() >= model.crates.len() {
        return false;
    }
    let crate_info = &model.crates[path.crate_id];
    model.strings.resolve(crate_info.name) == "core"
        && crate_info.stable_crate_id == model.item_def_paths[item].stable_crate_id
        && crate_info.stable_crate_id.0 != 0
}

fn null_mut_path(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    let segments = &model.items[item].def_path.segments;
    matches!(segments.as_slice(), [module, function]
        if named_segment(model, module, "ptr", false)
            && named_segment(model, function, "null_mut", true))
}

fn partial_eq_ne_path(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    let segments = &model.items[item].def_path.segments;
    matches!(segments.as_slice(), [module, trait_item, method]
        if named_segment(model, module, "cmp", false)
            && named_segment(model, trait_item, "PartialEq", false)
            && named_segment(model, method, "ne", true))
}

fn u64_default_path(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    let segments = &model.items[item].def_path.segments;
    matches!(segments.as_slice(), [module, .., impl_scope, method]
        if named_segment(model, module, "default", false)
            && matches!(&impl_scope.data, DefPathData::Impl)
            && named_segment(model, method, "default", true))
}

fn concrete_eq_path(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    let segments = &model.items[item].def_path.segments;
    matches!(segments.as_slice(), [.., impl_scope, method]
        if matches!(&impl_scope.data, DefPathData::Impl)
            && named_segment(model, method, "eq", true))
}

fn valid_null_mut_signature(
    model: &RegseqModel,
    instance: &regseq_model::Instance,
    sig: &FnSig,
) -> bool {
    let Ty::Ptr(pointee, Mutability::Mut) = &model.types[sig.output] else {
        return false;
    };
    sig.inputs.is_empty()
        && sig.abi == FnAbi::Rust
        && !sig.c_variadic
        && sig.safety == Safety::Safe
        && matches!(instance.generic_args.as_slice(), [GenericArg::Type(arg)] if arg == pointee)
}

fn valid_partial_eq_ne_signature(model: &RegseqModel, sig: &FnSig) -> bool {
    let [lhs, rhs] = sig.inputs.as_slice() else {
        return false;
    };
    matches!(&model.types[*lhs], Ty::Ref(_, Provenance::SharedBorrow))
        && &model.types[*lhs] == &model.types[*rhs]
        && matches!(&model.types[sig.output], Ty::Bool)
        && sig.abi == FnAbi::Rust
        && !sig.c_variadic
        && sig.safety == Safety::Safe
}

fn valid_u64_default_signature(model: &RegseqModel, sig: &FnSig) -> bool {
    sig.inputs.is_empty()
        && matches!(&model.types[sig.output], Ty::Uint(UintKind::U64))
        && sig.abi == FnAbi::Rust
        && !sig.c_variadic
        && sig.safety == Safety::Safe
}

/// Classify the exact bodyless core helpers owned by every C backend.
pub(crate) fn classify(
    model: &RegseqModel,
    id: InstanceId,
) -> Result<Option<BackendShim>, &'static str> {
    let instance = &model.instances[id];
    let Some(data) = instance.kind.as_function() else {
        return Ok(None);
    };
    if data.body.is_some()
        || data.flavor != FunctionInstanceFlavor::Item
        || model.items[instance.item].kind != ItemKind::Function
        || model.items[instance.item].linkage != Linkage::Default
        || !exact_core_item(model, instance.item)
    {
        return Ok(None);
    }
    let sig = model.sigs.resolve(data.fn_sig);
    if null_mut_path(model, instance.item) && valid_null_mut_signature(model, instance, sig) {
        return Ok(Some(BackendShim::NullMut));
    }
    if u64_default_path(model, instance.item) && valid_u64_default_signature(model, sig) {
        return Ok(Some(BackendShim::U64Default));
    }
    if !partial_eq_ne_path(model, instance.item) || !valid_partial_eq_ne_signature(model, sig) {
        return Ok(None);
    }

    let mut matches = model
        .instances
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let candidate_id = InstanceId::from_u32(index as u32);
            let candidate_data = candidate.kind.as_function()?;
            (candidate_data.body.is_some()
                && candidate_data.flavor == FunctionInstanceFlavor::Item
                && candidate_data.fn_sig == data.fn_sig
                && model.items[candidate.item].kind == ItemKind::Function
                && model.items[candidate.item].def_path.crate_id == model.primary_crate
                && concrete_eq_path(model, candidate.item))
            .then_some(candidate_id)
        });
    let Some(eq) = matches.next() else {
        return Err(
            "backend-owned core::cmp::PartialEq::ne has no bodyful concrete eq with the matching monomorphized signature",
        );
    };
    if matches.next().is_some() {
        return Err(
            "backend-owned core::cmp::PartialEq::ne has ambiguous bodyful concrete eq matches",
        );
    }
    Ok(Some(BackendShim::PartialEqNe { eq }))
}
