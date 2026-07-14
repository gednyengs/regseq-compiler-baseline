//! Recognition for the locked, single-element `SerialPort` surface.

use std::collections::{HashMap, HashSet};

use regseq_model::{
    ClassTag, DriverSerialPort, InstanceRef, ItemId, SerialEnumBinding, SerialPortOps,
    SerialTypeBindings, VariantIdx,
};
use rustc_middle::ty::{self, Mutability, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;

use super::const_::instance_ref_for_function_provenance;
use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError, SerialDiagnostic};

const PORT_TRAIT: &str = "SerialPort";
const OPERATIONS: &[&str] = &[
    "configure",
    "get_config",
    "transmit",
    "receive",
    "set_signals",
    "get_signals",
    "subscribe_events",
    "open",
    "close",
    "purge",
    "await_drain",
    "abort",
];
const REJECTED_TRAITS: &[&str] = &[
    "Serial",
    "SerialRuntimeLineConfig",
    "SerialInterruptDriven",
    "SerialQueueControl",
    "SerialModemControl",
    "SerialConsole",
    "SerialBulkTransfer",
    "SerialShutdown",
    "SerialShutdownOps",
    "SerialBreak",
];

const STATUS_VARIANTS: &[&str] = &[
    "Ok",
    "Unsupported",
    "Busy",
    "NoActiveTransfer",
    "InvalidParameter",
    "BufferTooSmall",
];
const DATA_BITS_VARIANTS: &[&str] = &["Five", "Six", "Seven", "Eight", "Nine"];
const STOP_BITS_VARIANTS: &[&str] = &["Half", "One", "OneAndHalf", "Two"];
const PARITY_VARIANTS: &[&str] = &["None", "Odd", "Even", "Mark", "Space"];
const FLOW_VARIANTS: &[&str] = &["None", "RtsCts", "DtrDsr", "Rs485"];
const DIRECTION_VARIANTS: &[&str] = &["Receive", "Transmit", "Both"];

/// Recognize every serial-class driver's one `SerialPort` impl. Authored
/// overrides are capabilities; inherited defaults are intentionally absent.
pub(crate) fn resolve_serial_ports(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let port_impls = trait_impls_by_driver(tcx, PORT_TRAIT);
    let all_impls = all_trait_impls_by_driver(tcx);
    let targets = serial_targets(cx);

    for (driver_def, traits) in &all_impls {
        if traits.iter().any(|(name, _, _)| name == PORT_TRAIT)
            && !targets.iter().any(|(_, def)| def == driver_def)
        {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::WrongDeviceClass {
                    driver: tcx.item_name(*driver_def).to_string(),
                },
            ));
        }
    }

    for (item_id, driver_def) in targets {
        let driver_name = {
            let id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(id).to_string()
        };
        let traits = all_impls.get(&driver_def).cloned().unwrap_or_default();
        if let Some((legacy, _, _)) = traits
            .iter()
            .find(|(name, _, _)| REJECTED_TRAITS.contains(&name.as_str()))
        {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::ExcludedSurface {
                    driver: driver_name,
                    surface: legacy.clone(),
                },
            ));
        }

        let Some((trait_def, impl_def)) = port_impls.get(&driver_def).copied() else {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::MissingPort {
                    driver: driver_name,
                },
            ));
        };
        let overrides = overridden_methods(tcx, trait_def, impl_def);
        let unknown = overrides
            .iter()
            .find(|name| !OPERATIONS.contains(&name.as_str()));
        if let Some(method) = unknown {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::BadSignature {
                    driver: driver_name,
                    operation: method.clone(),
                    detail: "operation is outside the locked serial-port vocabulary".into(),
                },
            ));
        }

        let driver_ty = tcx.type_of(driver_def).instantiate_identity();
        let mut types = SerialTypeBindings::default();
        validate_overrides(tcx, trait_def, &overrides, &driver_name, &mut types)?;
        let mut resolve = |name: &str| -> Result<Option<InstanceRef>, BridgeError> {
            if !overrides.contains(name) {
                return Ok(None);
            }
            resolve_method(cx, trait_def, driver_ty, name)
        };
        let ops = SerialPortOps {
            configure: resolve("configure")?,
            get_config: resolve("get_config")?,
            transmit: resolve("transmit")?,
            receive: resolve("receive")?,
            set_signals: resolve("set_signals")?,
            get_signals: resolve("get_signals")?,
            subscribe_events: resolve("subscribe_events")?,
            open: resolve("open")?,
            close: resolve("close")?,
            purge: resolve("purge")?,
            await_drain: resolve("await_drain")?,
            abort: resolve("abort")?,
        };
        cx.model
            .serial_port
            .drivers
            .insert(item_id, DriverSerialPort { ops, types });
    }
    Ok(())
}

fn serial_targets(cx: &BridgeCtxt<'_>) -> Vec<(ItemId, DefId)> {
    let tcx = cx.input.tcx;
    cx.model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter(|item| {
            matches!(
                cx.model.device_lifecycle.drivers[item]
                    .descriptor
                    .device_class,
                ClassTag::Serial
            )
        })
        .filter_map(|item| {
            recover_source_def_id(tcx, cx.model.item_def_paths[item]).map(|def| (item, def))
        })
        .collect()
}

fn all_trait_impls_by_driver(tcx: TyCtxt<'_>) -> HashMap<DefId, Vec<(String, DefId, DefId)>> {
    let mut result: HashMap<DefId, Vec<(String, DefId, DefId)>> = HashMap::new();
    // `hir_crate_items().definitions()` does not enumerate an empty impl block.
    // Such an impl is meaningful here: it declares the one serial port while
    // leaving every defaulted operation absent. The compiler's local trait-impl
    // index is complete for both empty and non-empty impls.
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

fn trait_impls_by_driver(tcx: TyCtxt<'_>, name: &str) -> HashMap<DefId, (DefId, DefId)> {
    all_trait_impls_by_driver(tcx)
        .into_iter()
        .filter_map(|(driver, traits)| {
            traits
                .into_iter()
                .find(|(candidate, _, _)| candidate == name)
                .map(|(_, trait_def, impl_def)| (driver, (trait_def, impl_def)))
        })
        .collect()
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
) -> Result<Option<InstanceRef>, BridgeError> {
    let Some(method) = trait_method(cx.input.tcx, trait_def, name) else {
        return Ok(None);
    };
    let args = cx.input.tcx.mk_args(&[driver_ty.into()]);
    let Ok(Some(instance)) = ty::Instance::try_resolve(
        cx.input.tcx,
        ty::TypingEnv::fully_monomorphized(),
        method,
        args,
    ) else {
        return Ok(None);
    };
    Ok(Some(instance_ref_for_function_provenance(cx, instance)?))
}

fn validate_overrides(
    tcx: TyCtxt<'_>,
    trait_def: DefId,
    overrides: &HashSet<String>,
    driver: &str,
    types: &mut SerialTypeBindings,
) -> Result<(), BridgeError> {
    for name in overrides {
        let method = trait_method(tcx, trait_def, name).expect("override belongs to trait");
        let sig = tcx.fn_sig(method).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        let output = sig.output();
        let bad = |detail: String| {
            BridgeError::SerialValidation(SerialDiagnostic::BadSignature {
                driver: driver.to_string(),
                operation: name.clone(),
                detail,
            })
        };
        match name.as_str() {
            "configure" => {
                expect_arity(inputs, 2).map_err(bad)?;
                capture_params(tcx, inputs[1], driver, types)?;
                capture_status(output, driver, types)?;
            }
            "get_config" => {
                expect_arity(inputs, 1).map_err(bad)?;
                capture_params(tcx, output, driver, types)?;
            }
            "transmit" => {
                expect_arity(inputs, 2).map_err(bad)?;
                validate_buffer(tcx, inputs[1], "SerialTxBuffer", false)
                    .map_err(bad)?;
                expect_uint(output, ty::UintTy::Usize).map_err(bad)?;
            }
            "receive" => {
                expect_arity(inputs, 2).map_err(bad)?;
                validate_buffer(tcx, inputs[1], "SerialRxBuffer", true)
                    .map_err(bad)?;
                expect_uint(output, ty::UintTy::Usize).map_err(bad)?;
            }
            "set_signals" => {
                expect_arity(inputs, 2).map_err(bad)?;
                validate_bool_struct(
                    tcx,
                    inputs[1],
                    "ModemSignalSet",
                    &[
                        "dtr", "rts", "cts", "dsr", "dcd", "ri", "changed_cts",
                        "changed_dsr", "changed_dcd", "changed_ri",
                    ],
                )
                .map_err(bad)?;
                capture_status(output, driver, types)?;
            }
            "get_signals" => {
                expect_arity(inputs, 1).map_err(bad)?;
                validate_bool_struct(
                    tcx,
                    output,
                    "ModemSignalSet",
                    &[
                        "dtr", "rts", "cts", "dsr", "dcd", "ri", "changed_cts",
                        "changed_dsr", "changed_dcd", "changed_ri",
                    ],
                )
                .map_err(bad)?;
            }
            "subscribe_events" => {
                expect_arity(inputs, 2).map_err(bad)?;
                validate_bool_struct(
                    tcx,
                    inputs[1],
                    "SerialEventMask",
                    &[
                        "rx_data_ready",
                        "tx_progress_or_idle",
                        "line_error",
                        "signal_change",
                    ],
                )
                .map_err(bad)?;
                capture_status(output, driver, types)?;
            }
            "open" | "close" => {
                expect_arity(inputs, 1).map_err(bad)?;
                capture_status(output, driver, types)?;
            }
            "purge" | "abort" => {
                expect_arity(inputs, 2).map_err(bad)?;
                types.direction = Some(capture_enum(
                    inputs[1],
                    DIRECTION_VARIANTS,
                    driver,
                    "SerialDirection",
                )?);
                capture_status(output, driver, types)?;
            }
            "await_drain" => {
                expect_arity(inputs, 1).map_err(bad)?;
                if !matches!(output.kind(), TyKind::Bool) {
                    return Err(bad("expected bool return".into()));
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn expect_arity(inputs: &[Ty<'_>], count: usize) -> Result<(), String> {
    (inputs.len() == count)
        .then_some(())
        .ok_or_else(|| format!("expected {count} inputs including receiver, found {}", inputs.len()))
}

fn expect_uint(ty: Ty<'_>, expected: ty::UintTy) -> Result<(), String> {
    matches!(ty.kind(), TyKind::Uint(found) if *found == expected)
        .then_some(())
        .ok_or_else(|| format!("expected `{expected:?}`, found `{ty}`"))
}

fn expect_adt<'tcx>(ty: Ty<'tcx>, expected: &str) -> Result<ty::AdtDef<'tcx>, String> {
    let Some(adt) = ty.ty_adt_def() else {
        return Err(format!("expected `{expected}`, found `{ty}`"));
    };
    let actual = ty.to_string();
    actual
        .rsplit("::")
        .next()
        .is_some_and(|leaf| leaf == expected)
        .then_some(adt)
        .ok_or_else(|| format!("expected `{expected}`, found `{ty}`"))
}

fn validate_bool_struct(
    tcx: TyCtxt<'_>,
    ty: Ty<'_>,
    expected: &str,
    fields: &[&str],
) -> Result<(), String> {
    let adt = expect_adt(ty, expected)?;
    let actual = adt
        .all_fields()
        .map(|field| field.name.to_string())
        .collect::<Vec<_>>();
    if actual != fields {
        return Err(format!(
            "expected `{expected}` fields {fields:?} in order, found {actual:?}"
        ));
    }
    for field in adt.all_fields() {
        let field_ty = tcx.type_of(field.did).instantiate_identity();
        if !matches!(field_ty.kind(), TyKind::Bool) {
            return Err(format!(
                "expected `{expected}.{}` to be bool, found `{field_ty}`",
                field.name
            ));
        }
    }
    Ok(())
}

fn validate_buffer(
    tcx: TyCtxt<'_>,
    ty: Ty<'_>,
    expected: &str,
    receive: bool,
) -> Result<(), String> {
    let adt = expect_adt(ty, expected)?;
    let expected_fields: &[&str] = if receive {
        &["data", "len", "errors"]
    } else {
        &["data", "len"]
    };
    let fields = adt.all_fields().collect::<Vec<_>>();
    let actual = fields
        .iter()
        .map(|field| field.name.to_string())
        .collect::<Vec<_>>();
    if actual != expected_fields {
        return Err(format!(
            "expected `{expected}` fields {expected_fields:?} in order, found {actual:?}"
        ));
    }
    for field in fields {
        let field_ty = tcx.type_of(field.did).instantiate_identity();
        match field.name.as_str() {
            "data" => match field_ty.kind() {
                TyKind::RawPtr(pointee, mutability)
                    if matches!(pointee.kind(), TyKind::Uint(ty::UintTy::U8))
                        && (*mutability == Mutability::Mut) == receive => {}
                _ => {
                    return Err(format!(
                        "expected `{expected}.data` to be `{}`, found `{field_ty}`",
                        if receive { "*mut u8" } else { "*const u8" }
                    ))
                }
            },
            "len" => expect_uint(field_ty, ty::UintTy::Usize)?,
            "errors" => match field_ty.kind() {
                TyKind::RawPtr(pointee, Mutability::Mut)
                    if expect_adt(*pointee, "LineErrorFlags").is_ok() =>
                {
                    validate_bool_struct(
                        tcx,
                        *pointee,
                        "LineErrorFlags",
                        &["break_detected", "framing", "parity", "overrun"],
                    )?;
                }
                _ => {
                    return Err(format!(
                        "expected `{expected}.errors` to be `*mut LineErrorFlags`, found `{field_ty}`"
                    ))
                }
            },
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn capture_status(
    ty: Ty<'_>,
    driver: &str,
    types: &mut SerialTypeBindings,
) -> Result<(), BridgeError> {
    let binding = capture_enum(ty, STATUS_VARIANTS, driver, "SerialStatus")?;
    if let Some(existing) = &types.status {
        if existing != &binding {
            return Err(signature_error(
                driver,
                "SerialStatus",
                "inconsistent enum identity across operations",
            ));
        }
    }
    types.status = Some(binding);
    Ok(())
}

fn capture_params(
    tcx: TyCtxt<'_>,
    ty: Ty<'_>,
    driver: &str,
    types: &mut SerialTypeBindings,
) -> Result<(), BridgeError> {
    let adt = expect_adt(ty, "SerialParams")
        .map_err(|detail| signature_error(driver, "SerialParams", &detail))?;
    let mut seen = HashSet::new();
    for field in adt.all_fields() {
        let name = field.name.as_str();
        seen.insert(name.to_string());
        let field_ty = tcx.type_of(field.did).instantiate_identity();
        match name {
            "baud_rate" => expect_uint(field_ty, ty::UintTy::U32)
                .map_err(|detail| signature_error(driver, "SerialParams", &detail))?,
            "data_bits" => {
                types.data_bits = Some(capture_enum(
                    field_ty,
                    DATA_BITS_VARIANTS,
                    driver,
                    "SerialDataBits",
                )?)
            }
            "stop_bits" => {
                types.stop_bits = Some(capture_enum(
                    field_ty,
                    STOP_BITS_VARIANTS,
                    driver,
                    "SerialStopBits",
                )?)
            }
            "parity" => {
                types.parity = Some(capture_enum(
                    field_ty,
                    PARITY_VARIANTS,
                    driver,
                    "SerialParity",
                )?)
            }
            "flow_control_mode" => {
                types.flow_control = Some(capture_enum(
                    field_ty,
                    FLOW_VARIANTS,
                    driver,
                    "SerialFlowControl",
                )?)
            }
            _ => {
                return Err(signature_error(
                    driver,
                    "SerialParams",
                    &format!("unexpected field `{name}`"),
                ))
            }
        }
    }
    for required in [
        "baud_rate",
        "data_bits",
        "stop_bits",
        "parity",
        "flow_control_mode",
    ] {
        if !seen.contains(required) {
            return Err(signature_error(
                driver,
                "SerialParams",
                &format!("missing field `{required}`"),
            ));
        }
    }
    Ok(())
}

fn capture_enum(
    ty: Ty<'_>,
    expected: &[&str],
    driver: &str,
    type_name: &str,
) -> Result<SerialEnumBinding, BridgeError> {
    let Some(adt) = ty.ty_adt_def().filter(|adt| adt.is_enum()) else {
        return Err(signature_error(driver, type_name, "expected an enum type"));
    };
    let actual = adt
        .variants()
        .iter_enumerated()
        .map(|(_, variant)| variant.name.to_string())
        .collect::<Vec<_>>();
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(signature_error(
            driver,
            type_name,
            &format!("expected variants {expected:?} in order, found {actual:?}"),
        ));
    }
    Ok(SerialEnumBinding {
        variants: adt
            .variants()
            .iter_enumerated()
            .map(|(idx, _)| VariantIdx::from_u32(idx.as_u32()))
            .collect(),
    })
}

fn signature_error(driver: &str, operation: &str, detail: &str) -> BridgeError {
    BridgeError::SerialValidation(SerialDiagnostic::BadSignature {
        driver: driver.to_string(),
        operation: operation.to_string(),
        detail: detail.to_string(),
    })
}
