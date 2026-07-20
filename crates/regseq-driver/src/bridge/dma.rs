//! Recognition for the locked DMA buffer and asynchronous-channel elements.

use std::collections::{HashMap, HashSet};

use regseq_model::{
    AsyncTransferChannel, DmaBufferOps, DmaChannelOps, DriverDma, InstanceRef, ItemId,
    OwnedDmaBuffer,
};
use rustc_middle::ty::{self, Mutability, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;
use rustc_span::sym;

use super::const_::instance_ref_for_function_provenance;
use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError, DmaDiagnostic};

const BUFFER_TRAIT: &str = "DmaBuffer";
const CHANNEL_TRAIT: &str = "AsyncTransferChannel";
const EXCLUDED_TRAITS: &[&str] = &["DmaRegion", "DmaChannel", "DmaStreamBuffer", "DmaMapping"];
const BUFFER_OPS: &[&str] = &[
    "allocate",
    "free",
    "cpu_address",
    "length",
    "transfer_address",
    "set_valid_length",
];
const BUFFER_REQUIRED: &[&str] = &["allocate", "free", "cpu_address", "length"];
const CHANNEL_OPS: &[&str] = &[
    "acquire",
    "configure",
    "prepare",
    "submit",
    "activate",
    "release",
    "status",
    "pause",
    "resume",
    "suspend",
    "reload",
    "terminate",
    "synchronize",
    "reclaim",
    "on_completion",
];
const CHANNEL_REQUIRED: &[&str] = &[
    "acquire",
    "configure",
    "prepare",
    "submit",
    "activate",
    "release",
];

pub(crate) fn resolve_dma(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let all_impls = all_trait_impls_by_driver(tcx);
    for (item, driver_def) in dma_targets(cx) {
        let traits = all_impls.get(&driver_def).cloned().unwrap_or_default();
        let driver = cx
            .model
            .device_lifecycle
            .drivers
            .get(&item)
            .map(|d| cx.model.strings.resolve(d.descriptor.name).to_string())
            .unwrap_or_else(|| tcx.item_name(driver_def).to_string());

        if let Some((surface, _, _)) = traits
            .iter()
            .find(|(name, _, _)| EXCLUDED_TRAITS.contains(&name.as_str()))
        {
            return Err(BridgeError::DmaValidation(DmaDiagnostic::ExcludedSurface {
                driver,
                surface: surface.clone(),
            }));
        }

        let buffer = traits
            .iter()
            .find(|(name, _, _)| name == BUFFER_TRAIT)
            .cloned();
        let channel = traits
            .iter()
            .find(|(name, _, _)| name == CHANNEL_TRAIT)
            .cloned();
        if buffer.is_none() && channel.is_none() {
            continue;
        }

        let driver_ty = tcx.type_of(driver_def).instantiate_identity();
        let buffer = buffer
            .map(|(_, trait_def, impl_def)| {
                build_buffer(cx, trait_def, impl_def, driver_ty, &driver)
            })
            .transpose()?;
        let channel = channel
            .map(|(_, trait_def, impl_def)| {
                build_channel(cx, trait_def, impl_def, driver_ty, &driver)
            })
            .transpose()?;
        cx.model
            .dma
            .drivers
            .insert(item, DriverDma { buffer, channel });
    }
    Ok(())
}

fn build_buffer<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    trait_def: DefId,
    impl_def: DefId,
    driver_ty: Ty<'tcx>,
    driver: &str,
) -> Result<OwnedDmaBuffer, BridgeError> {
    let overrides = overridden_methods(cx.input.tcx, trait_def, impl_def);
    reject_unknown(driver, BUFFER_TRAIT, &overrides, BUFFER_OPS)?;
    let missing = BUFFER_REQUIRED
        .iter()
        .copied()
        .filter(|name| !overrides.contains(*name))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(BridgeError::DmaValidation(
            DmaDiagnostic::IncompleteBuffer {
                driver: driver.to_string(),
                missing: missing.join(", "),
            },
        ));
    }
    validate_buffer_signatures(cx.input.tcx, trait_def, &overrides, driver)?;
    let mut resolve = |name| resolve_method(cx, trait_def, driver_ty, name, &overrides, driver);
    Ok(OwnedDmaBuffer {
        ops: DmaBufferOps {
            allocate: resolve("allocate")?,
            free: resolve("free")?,
            cpu_address: resolve("cpu_address")?,
            length: resolve("length")?,
            transfer_address: resolve("transfer_address")?,
            set_valid_length: resolve("set_valid_length")?,
        },
    })
}

fn build_channel<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    trait_def: DefId,
    impl_def: DefId,
    driver_ty: Ty<'tcx>,
    driver: &str,
) -> Result<AsyncTransferChannel, BridgeError> {
    let overrides = overridden_methods(cx.input.tcx, trait_def, impl_def);
    reject_unknown(driver, CHANNEL_TRAIT, &overrides, CHANNEL_OPS)?;
    let missing = CHANNEL_REQUIRED
        .iter()
        .copied()
        .filter(|name| !overrides.contains(*name))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(BridgeError::DmaValidation(
            DmaDiagnostic::IncompleteChannel {
                driver: driver.to_string(),
                missing: missing.join(", "),
            },
        ));
    }
    if overrides.contains("terminate") && !overrides.contains("synchronize") {
        return Err(BridgeError::DmaValidation(
            DmaDiagnostic::TerminateNeedsSynchronize {
                driver: driver.to_string(),
            },
        ));
    }
    validate_channel_signatures(cx.input.tcx, trait_def, &overrides, driver)?;
    let mut resolve = |name| resolve_method(cx, trait_def, driver_ty, name, &overrides, driver);
    Ok(AsyncTransferChannel {
        ops: DmaChannelOps {
            acquire: resolve("acquire")?,
            configure: resolve("configure")?,
            prepare: resolve("prepare")?,
            submit: resolve("submit")?,
            activate: resolve("activate")?,
            release: resolve("release")?,
            status: resolve("status")?,
            pause: resolve("pause")?,
            resume: resolve("resume")?,
            suspend: resolve("suspend")?,
            reload: resolve("reload")?,
            terminate: resolve("terminate")?,
            synchronize: resolve("synchronize")?,
            reclaim: resolve("reclaim")?,
            on_completion: resolve("on_completion")?,
        },
    })
}

fn dma_targets(cx: &BridgeCtxt<'_>) -> Vec<(ItemId, DefId)> {
    let tcx = cx.input.tcx;
    cx.model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item| {
            recover_source_def_id(tcx, cx.model.item_def_paths[item]).map(|def| (item, def))
        })
        .collect()
}

fn all_trait_impls_by_driver(tcx: TyCtxt<'_>) -> HashMap<DefId, Vec<(String, DefId, DefId)>> {
    let mut result: HashMap<DefId, Vec<(String, DefId, DefId)>> = HashMap::new();
    for impls in tcx.all_local_trait_impls(()).values() {
        for local_impl in impls {
            let impl_def = local_impl.to_def_id();
            let trait_ref = tcx.impl_trait_ref(impl_def).skip_binder();
            let Some(adt) = trait_ref.self_ty().ty_adt_def() else {
                continue;
            };
            result.entry(adt.did()).or_default().push((
                tcx.item_name(trait_ref.def_id).to_string(),
                trait_ref.def_id,
                impl_def,
            ));
        }
    }
    result
}

fn overridden_methods(tcx: TyCtxt<'_>, trait_def: DefId, impl_def: DefId) -> HashSet<String> {
    tcx.associated_items(impl_def)
        .in_definition_order()
        .filter(|item| {
            item.is_fn()
                && item
                    .trait_item_def_id()
                    .is_some_and(|method| tcx.trait_of_assoc(method) == Some(trait_def))
        })
        .map(|item| item.name().to_string())
        .collect()
}

fn trait_method(tcx: TyCtxt<'_>, trait_def: DefId, name: &str) -> Option<DefId> {
    tcx.associated_items(trait_def)
        .in_definition_order()
        .find(|item| item.is_fn() && item.name().as_str() == name)
        .map(|item| item.def_id)
}

fn resolve_method<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    trait_def: DefId,
    driver_ty: Ty<'tcx>,
    name: &str,
    overrides: &HashSet<String>,
    driver: &str,
) -> Result<Option<InstanceRef>, BridgeError> {
    if !overrides.contains(name) {
        return Ok(None);
    }
    let method = trait_method(cx.input.tcx, trait_def, name).expect("overridden DMA method");
    let args = cx.input.tcx.mk_args(&[driver_ty.into()]);
    let instance = ty::Instance::try_resolve(
        cx.input.tcx,
        ty::TypingEnv::fully_monomorphized(),
        method,
        args,
    )
    .ok()
    .flatten()
    .ok_or_else(|| {
        bad_signature(
            driver,
            name,
            "authored operation could not be resolved to a monomorphic instance",
        )
    })?;
    Ok(Some(instance_ref_for_function_provenance(cx, instance)?))
}

fn reject_unknown(
    driver: &str,
    surface: &str,
    overrides: &HashSet<String>,
    allowed: &[&str],
) -> Result<(), BridgeError> {
    if let Some(operation) = overrides
        .iter()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        return Err(bad_signature(
            driver,
            operation,
            &format!("operation is outside the locked `{surface}` vocabulary"),
        ));
    }
    Ok(())
}

fn validate_buffer_signatures(
    tcx: TyCtxt<'_>,
    trait_def: DefId,
    overrides: &HashSet<String>,
    driver: &str,
) -> Result<(), BridgeError> {
    for name in overrides {
        let method = trait_method(tcx, trait_def, name).expect("DMA buffer method");
        let sig = tcx.fn_sig(method).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        let output = sig.output();
        let result = match name.as_str() {
            "allocate" => {
                expect_signature(inputs, output, &["DmaBufferRequest"], "DmaBufferHandle")
            }
            "free" => expect_signature(inputs, output, &["DmaBufferHandle"], "()"),
            "cpu_address" => (|| {
                expect_arity(inputs, 2)?;
                expect_adt(inputs[1], "DmaBufferHandle")?;
                expect_u8_ptr(output, Mutability::Mut)
            })(),
            "length" => expect_signature(inputs, output, &["DmaBufferHandle"], "usize"),
            "transfer_address" => (|| {
                expect_arity(inputs, 2)?;
                expect_adt(inputs[1], "DmaBufferHandle")?;
                expect_option(tcx, output, "u64")
            })(),
            "set_valid_length" => {
                expect_signature(inputs, output, &["DmaBufferHandle", "usize"], "DmaStatus")
            }
            _ => Ok(()),
        };
        result.map_err(|detail| bad_signature(driver, name, &detail))?;
    }
    Ok(())
}

fn validate_channel_signatures(
    tcx: TyCtxt<'_>,
    trait_def: DefId,
    overrides: &HashSet<String>,
    driver: &str,
) -> Result<(), BridgeError> {
    for name in overrides {
        let method = trait_method(tcx, trait_def, name).expect("DMA channel method");
        let sig = tcx.fn_sig(method).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        let output = sig.output();
        let result = match name.as_str() {
            "acquire" => expect_signature(inputs, output, &[], "DmaChannelHandle"),
            "configure" | "reload" => expect_signature(
                inputs,
                output,
                &["DmaChannelHandle", "DmaTransferConfig"],
                "DmaStatus",
            ),
            "prepare" => {
                expect_signature(inputs, output, &["DmaChannelHandle"], "DmaRequestHandle")
            }
            "submit" | "reclaim" => {
                expect_signature(inputs, output, &["DmaRequestHandle"], "DmaStatus")
            }
            "activate" | "pause" | "resume" | "suspend" | "terminate" | "synchronize" => {
                expect_signature(inputs, output, &["DmaChannelHandle"], "DmaStatus")
            }
            "release" => expect_signature(inputs, output, &["DmaChannelHandle"], "()"),
            "status" => (|| {
                expect_arity(inputs, 2)?;
                expect_adt(inputs[1], "DmaChannelHandle")?;
                expect_option(tcx, output, "DmaRuntimeStatus")
            })(),
            "on_completion" => expect_signature(inputs, output, &["DmaStatus", "usize"], "()"),
            _ => Ok(()),
        };
        result.map_err(|detail| bad_signature(driver, name, &detail))?;
    }
    Ok(())
}

fn expect_signature(
    inputs: &[Ty<'_>],
    output: Ty<'_>,
    expected_inputs: &[&str],
    expected_output: &str,
) -> Result<(), String> {
    expect_arity(inputs, expected_inputs.len() + 1)?;
    for (actual, expected) in inputs[1..].iter().zip(expected_inputs) {
        expect_type(*actual, expected)?;
    }
    expect_type(output, expected_output)
}

fn expect_arity(inputs: &[Ty<'_>], count: usize) -> Result<(), String> {
    (inputs.len() == count).then_some(()).ok_or_else(|| {
        format!(
            "expected {count} inputs including receiver, found {}",
            inputs.len()
        )
    })
}

fn expect_type(ty: Ty<'_>, expected: &str) -> Result<(), String> {
    match expected {
        "()" if matches!(ty.kind(), TyKind::Tuple(fields) if fields.is_empty()) => Ok(()),
        "usize" if matches!(ty.kind(), TyKind::Uint(ty::UintTy::Usize)) => Ok(()),
        "u64" if matches!(ty.kind(), TyKind::Uint(ty::UintTy::U64)) => Ok(()),
        _ => expect_adt(ty, expected),
    }
}

fn expect_adt(ty: Ty<'_>, expected: &str) -> Result<(), String> {
    let actual = ty
        .ty_adt_def()
        .map(|adt| adt.did())
        .map(|_| ty.to_string())
        .and_then(|name| name.rsplit("::").next().map(str::to_string));
    (actual.as_deref() == Some(expected))
        .then_some(())
        .ok_or_else(|| format!("expected `{expected}`, found `{ty}`"))
}

fn expect_option(tcx: TyCtxt<'_>, ty: Ty<'_>, expected_inner: &str) -> Result<(), String> {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return Err(format!("expected `Option<{expected_inner}>`, found `{ty}`"));
    };
    if tcx.get_diagnostic_name(adt.did()) != Some(sym::Option) {
        return Err(format!("expected `Option<{expected_inner}>`, found `{ty}`"));
    }
    let inner = args
        .types()
        .next()
        .ok_or_else(|| format!("expected `Option<{expected_inner}>`, found `{ty}`"))?;
    expect_type(inner, expected_inner)
}

fn expect_u8_ptr(ty: Ty<'_>, mutability: Mutability) -> Result<(), String> {
    match ty.kind() {
        TyKind::RawPtr(pointee, found)
            if *found == mutability && matches!(pointee.kind(), TyKind::Uint(ty::UintTy::U8)) =>
        {
            Ok(())
        }
        _ => Err(format!("expected `*mut u8`, found `{ty}`")),
    }
}

fn bad_signature(driver: &str, operation: &str, detail: &str) -> BridgeError {
    BridgeError::DmaValidation(DmaDiagnostic::BadSignature {
        driver: driver.to_string(),
        operation: operation.to_string(),
        detail: detail.to_string(),
    })
}
