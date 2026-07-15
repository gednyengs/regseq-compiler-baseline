//! Recognition for the locked, single-element `NetworkInterface` surface.

use std::collections::{HashMap, HashSet};

use regseq_model::{
    ClassTag, DriverNetworkInterface, InstanceRef, ItemId, NetworkInterfaceOps,
};
use rustc_middle::ty::{self, Mutability, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;

use super::const_::instance_ref_for_function_provenance;
use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError, NetworkDiagnostic};

const INTERFACE_TRAIT: &str = "NetworkInterface";
const EXCLUDED_TRAITS: &[&str] = &["Network", "DriverNetwork", "NetDevice"];
const OPERATIONS: &[&str] = &[
    "set_published",
    "set_admin_enabled",
    "transmit",
    "receive",
    "report_carrier",
    "report_dormant",
    "query_operational_state",
    "get_property",
    "set_property",
    "read_counters",
    "register_observer",
    "unregister_observer",
];
const REQUIRED: &[&str] = &[
    "set_admin_enabled",
    "transmit",
    "report_carrier",
    "query_operational_state",
    "get_property",
    "read_counters",
];

pub(crate) fn resolve_network_interfaces(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let all_impls = all_trait_impls_by_driver(tcx);
    for (item, driver_def) in targets(cx) {
        let traits = all_impls.get(&driver_def).cloned().unwrap_or_default();
        let driver = cx
            .model
            .device_lifecycle
            .drivers
            .get(&item)
            .map(|d| cx.model.strings.resolve(d.descriptor.name).to_string())
            .unwrap_or_else(|| tcx.item_name(driver_def).to_string());
        let network_class = cx
            .model
            .device_lifecycle
            .drivers
            .get(&item)
            .is_some_and(|d| matches!(d.descriptor.device_class, ClassTag::Network));

        if let Some((surface, _, _)) = traits
            .iter()
            .find(|(name, _, _)| EXCLUDED_TRAITS.contains(&name.as_str()))
        {
            return Err(BridgeError::NetworkValidation(
                NetworkDiagnostic::ExcludedSurface {
                    driver,
                    surface: surface.clone(),
                },
            ));
        }

        let interface = traits
            .iter()
            .find(|(name, _, _)| name == INTERFACE_TRAIT)
            .cloned();
        if !network_class && interface.is_some() {
            return Err(BridgeError::NetworkValidation(
                NetworkDiagnostic::WrongDeviceClass { driver },
            ));
        }
        let Some((_, trait_def, impl_def)) = interface else {
            continue;
        };

        let declared = tcx
            .associated_items(trait_def)
            .in_definition_order()
            .filter(|item| item.is_fn())
            .map(|item| item.name().to_string())
            .collect::<HashSet<_>>();
        if let Some(operation) = declared
            .iter()
            .find(|name| !OPERATIONS.contains(&name.as_str()))
        {
            return Err(bad_signature(
                &driver,
                operation,
                "operation is outside the locked `NetworkInterface` vocabulary",
            ));
        }
        if let Some(operation) = OPERATIONS
            .iter()
            .copied()
            .find(|name| !declared.contains(*name))
        {
            return Err(bad_signature(
                &driver,
                operation,
                "operation is missing from the locked `NetworkInterface` declaration",
            ));
        }

        let overrides = overridden_methods(tcx, trait_def, impl_def);
        let missing = REQUIRED
            .iter()
            .copied()
            .filter(|name| !overrides.contains(*name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(BridgeError::NetworkValidation(
                NetworkDiagnostic::IncompleteInterface {
                    driver,
                    missing: missing.join(", "),
                },
            ));
        }
        validate_signatures(tcx, trait_def, &overrides, &driver)?;

        let driver_ty = tcx.type_of(driver_def).instantiate_identity();
        let ops = NetworkInterfaceOps {
            set_published: resolve_method(
                cx, trait_def, driver_ty, "set_published", &overrides, &driver,
            )?,
            set_admin_enabled: resolve_method(
                cx, trait_def, driver_ty, "set_admin_enabled", &overrides, &driver,
            )?,
            transmit: resolve_method(
                cx, trait_def, driver_ty, "transmit", &overrides, &driver,
            )?,
            receive: resolve_method(
                cx, trait_def, driver_ty, "receive", &overrides, &driver,
            )?,
            report_carrier: resolve_method(
                cx, trait_def, driver_ty, "report_carrier", &overrides, &driver,
            )?,
            report_dormant: resolve_method(
                cx, trait_def, driver_ty, "report_dormant", &overrides, &driver,
            )?,
            query_operational_state: resolve_method(
                cx,
                trait_def,
                driver_ty,
                "query_operational_state",
                &overrides,
                &driver,
            )?,
            get_property: resolve_method(
                cx, trait_def, driver_ty, "get_property", &overrides, &driver,
            )?,
            set_property: resolve_method(
                cx, trait_def, driver_ty, "set_property", &overrides, &driver,
            )?,
            read_counters: resolve_method(
                cx, trait_def, driver_ty, "read_counters", &overrides, &driver,
            )?,
            register_observer: resolve_method(
                cx, trait_def, driver_ty, "register_observer", &overrides, &driver,
            )?,
            unregister_observer: resolve_method(
                cx, trait_def, driver_ty, "unregister_observer", &overrides, &driver,
            )?,
        };
        cx.model.network_interface.drivers.insert(
            item,
            DriverNetworkInterface { ops },
        );
    }
    Ok(())
}

fn targets(cx: &BridgeCtxt<'_>) -> Vec<(ItemId, DefId)> {
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
            let Some(adt) = trait_ref.self_ty().ty_adt_def() else { continue };
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
    let method = trait_method(cx.input.tcx, trait_def, name).expect("network method");
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

fn validate_signatures(
    tcx: TyCtxt<'_>,
    trait_def: DefId,
    overrides: &HashSet<String>,
    driver: &str,
) -> Result<(), BridgeError> {
    for name in overrides {
        let method = trait_method(tcx, trait_def, name).expect("network trait method");
        let sig = tcx.fn_sig(method).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        let output = sig.output();
        let mutable_receiver = !matches!(
            name.as_str(),
            "query_operational_state" | "get_property" | "read_counters"
        );
        expect_receiver(inputs.first().copied(), mutable_receiver)
            .map_err(|detail| bad_signature(driver, name, &detail))?;
        let result = match name.as_str() {
            "set_published" | "set_admin_enabled" => {
                expect(tcx, inputs, output, &["bool"], "isize")
            }
            "transmit" => {
                expect(tcx, inputs, output, &["OpaquePacketBuffer"], "Option<SubmissionVerdict>")
            }
            "receive" => expect(tcx, inputs, output, &["OpaquePacketBuffer"], "()"),
            "report_carrier" => expect(tcx, inputs, output, &["bool"], "()"),
            "report_dormant" => expect(tcx, inputs, output, &["bool"], "isize"),
            "query_operational_state" => expect(tcx, inputs, output, &[], "OperationalState"),
            "get_property" => expect(
                tcx,
                inputs,
                output,
                &["NetworkProperty"],
                "Option<NetworkPropertyValue>",
            ),
            "set_property" => expect(
                tcx,
                inputs,
                output,
                &["NetworkProperty", "NetworkPropertyValue"],
                "isize",
            ),
            "read_counters" => expect(tcx, inputs, output, &[], "InterfaceCounters"),
            "register_observer" | "unregister_observer" => {
                expect(tcx, inputs, output, &["usize"], "isize")
            }
            _ => Ok(()),
        };
        result.map_err(|detail| bad_signature(driver, name, &detail))?;
    }
    Ok(())
}

fn expect_receiver(receiver: Option<Ty<'_>>, mutable: bool) -> Result<(), String> {
    let expected = if mutable { "&mut self" } else { "&self" };
    let Some(receiver) = receiver else {
        return Err(format!("expected `{expected}` receiver"));
    };
    let matches = match receiver.kind() {
        TyKind::Ref(_, _, mutability) => {
            (*mutability == Mutability::Mut) == mutable
        }
        _ => false,
    };
    matches
        .then_some(())
        .ok_or_else(|| format!("expected `{expected}` receiver, found `{receiver}`"))
}

fn expect(
    tcx: TyCtxt<'_>,
    inputs: &[Ty<'_>],
    output: Ty<'_>,
    args: &[&str],
    result: &str,
) -> Result<(), String> {
    if inputs.len() != args.len() + 1 {
        return Err(format!("expected receiver plus {} operands", args.len()));
    }
    for (actual, expected) in inputs[1..].iter().zip(args) {
        expect_type(tcx, *actual, expected)?;
    }
    expect_type(tcx, output, result)
}

fn expect_type(tcx: TyCtxt<'_>, ty: Ty<'_>, expected: &str) -> Result<(), String> {
    let matches = match expected {
        "()" => ty.is_unit(),
        "bool" => matches!(ty.kind(), TyKind::Bool),
        "usize" => matches!(ty.kind(), TyKind::Uint(ty::UintTy::Usize)),
        "isize" => matches!(ty.kind(), TyKind::Int(ty::IntTy::Isize)),
        name if name.starts_with("Option<") => {
            let inner = &name[7..name.len() - 1];
            matches!(ty.kind(), TyKind::Adt(adt, args)
                if adt.is_enum()
                    && adt.variants().len() == 2
                    && args.types().next().is_some_and(|value| type_name(tcx, value) == inner))
        }
        name => type_name(tcx, ty) == name,
    };
    matches
        .then_some(())
        .ok_or_else(|| format!("expected `{expected}`, found `{ty}`"))
}

fn type_name(tcx: TyCtxt<'_>, ty: Ty<'_>) -> String {
    ty.ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).to_string())
        .unwrap_or_default()
}

fn bad_signature(driver: &str, operation: &str, detail: &str) -> BridgeError {
    BridgeError::NetworkValidation(NetworkDiagnostic::BadSignature {
        driver: driver.to_string(),
        operation: operation.to_string(),
        detail: detail.to_string(),
    })
}
