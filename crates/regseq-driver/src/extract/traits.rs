use rustc_hir::def::Res;
use rustc_hir::{FnDecl, FnRetTy, GenericArg, ImplItemKind, ItemKind, QPath, TyKind};
use rustc_middle::ty::TyCtxt;

use super::callback_context::CallbackContext;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for trait callback families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitSurface {
    pub lifecycle_impls: Vec<LifecycleImpl>,
    pub serial_impls: Vec<SerialImpl>,
    pub serial_console_impls: Vec<SerialConsoleImpl>,
    pub power_management_impls: Vec<PowerManagementImpl>,
}

/// Extracted `impl Lifecycle for DriverType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleImpl {
    pub driver_type: String,
    pub context: CallbackContext,
}

/// Extracted `impl Serial for DriverType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialImpl {
    pub driver_type: String,
    pub context: CallbackContext,
}

/// Extracted `impl SerialConsole for DriverType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConsoleImpl {
    pub driver_type: String,
    pub context: CallbackContext,
}

/// Extracted `impl PowerManagement for DriverType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerManagementImpl {
    pub driver_type: String,
    pub context: CallbackContext,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraitError {
    MissingMethod { trait_name: String, method: String },
    InvalidReturnType { trait_name: String, method: String },
    InvalidReceiver { trait_name: String, method: String },
    InvalidSignature { trait_name: String, method: String },
}

// ---------------------------------------------------------------------------
// Expected method shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnSpec {
    Unit,
    ResultUnitRegseqError,
    OptionRxChar,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraParamSpec {
    None,
    RefNamed(&'static str),
    RefSlicePrimitive(&'static str),
    Primitive(&'static str),
}

struct MethodSpec {
    name: &'static str,
    extra_param: ExtraParamSpec,
    return_spec: ReturnSpec,
}

const LIFECYCLE_METHODS: &[MethodSpec] = &[
    MethodSpec {
        name: "init",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
    MethodSpec {
        name: "deinit",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
];

const SERIAL_METHODS: &[MethodSpec] = &[
    MethodSpec {
        name: "port_open",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
    MethodSpec {
        name: "port_close",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::Unit,
    },
    MethodSpec {
        name: "configure",
        extra_param: ExtraParamSpec::RefNamed("regseq::SerialConfig"),
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
    MethodSpec {
        name: "tx_byte",
        extra_param: ExtraParamSpec::Primitive("u8"),
        return_spec: ReturnSpec::Unit,
    },
    MethodSpec {
        name: "rx_char",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::OptionRxChar,
    },
    MethodSpec {
        name: "tx_ready",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::Bool,
    },
    MethodSpec {
        name: "tx_empty",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::Bool,
    },
    MethodSpec {
        name: "flush_rx_fifo",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::Unit,
    },
    MethodSpec {
        name: "flush_tx_fifo",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::Unit,
    },
    MethodSpec {
        name: "send_break",
        extra_param: ExtraParamSpec::Primitive("bool"),
        return_spec: ReturnSpec::Unit,
    },
];

const SERIAL_CONSOLE_METHODS: &[MethodSpec] = &[MethodSpec {
    name: "console_write",
    extra_param: ExtraParamSpec::RefSlicePrimitive("u8"),
    return_spec: ReturnSpec::Unit,
}];

const PM_METHODS: &[MethodSpec] = &[
    MethodSpec {
        name: "suspend",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
    MethodSpec {
        name: "resume",
        extra_param: ExtraParamSpec::None,
        return_spec: ReturnSpec::ResultUnitRegseqError,
    },
];

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract recognized trait callback-family implementations from HIR.
pub fn extract_trait_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<TraitSurface, TraitError> {
    let mut lifecycle_impls = Vec::new();
    let mut serial_impls = Vec::new();
    let mut serial_console_impls = Vec::new();
    let mut power_management_impls = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        let ItemKind::Impl(impl_block) = &item.kind else {
            continue;
        };

        let Some(trait_ref) = &impl_block.of_trait else {
            continue;
        };

        let Some(canonical_path) = extract_trait_identity(tcx, trait_ref.trait_ref.path) else {
            continue;
        };

        let Some(driver_type) = extract_self_type_name(impl_block.self_ty) else {
            continue;
        };

        match canonical_path.as_str() {
            "regseq::Lifecycle" => {
                validate_methods(tcx, impl_block, "Lifecycle", LIFECYCLE_METHODS)?;
                lifecycle_impls.push(LifecycleImpl {
                    driver_type,
                    context: CallbackContext::FrameworkManaged,
                });
            }
            "regseq::Serial" => {
                validate_methods(tcx, impl_block, "Serial", SERIAL_METHODS)?;
                serial_impls.push(SerialImpl {
                    driver_type,
                    context: CallbackContext::SubsystemSerialized,
                });
            }
            "regseq::SerialConsole" => {
                validate_methods(tcx, impl_block, "SerialConsole", SERIAL_CONSOLE_METHODS)?;
                serial_console_impls.push(SerialConsoleImpl {
                    driver_type,
                    context: CallbackContext::SubsystemSerialized,
                });
            }
            "regseq::PowerManagement" => {
                validate_methods(tcx, impl_block, "PowerManagement", PM_METHODS)?;
                power_management_impls.push(PowerManagementImpl {
                    driver_type,
                    context: CallbackContext::FrameworkManaged,
                });
            }
            _ => {}
        }
    }

    Ok(TraitSurface {
        lifecycle_impls,
        serial_impls,
        serial_console_impls,
        power_management_impls,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_methods<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_block: &rustc_hir::Impl<'tcx>,
    trait_name: &str,
    expected: &[MethodSpec],
) -> Result<(), TraitError> {
    for spec in expected {
        let found = impl_block.items.iter().find(|item_ref| {
            let item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                owner_id: item_ref.owner_id,
            });
            item.ident.name.as_str() == spec.name
        });

        let Some(item_ref) = found else {
            return Err(TraitError::MissingMethod {
                trait_name: trait_name.to_string(),
                method: spec.name.to_string(),
            });
        };

        let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
            owner_id: item_ref.owner_id,
        });

        let ImplItemKind::Fn(ref sig, _) = impl_item.kind else {
            return Err(TraitError::MissingMethod {
                trait_name: trait_name.to_string(),
                method: spec.name.to_string(),
            });
        };

        if !is_mut_ref_self(sig.decl) {
            return Err(TraitError::InvalidReceiver {
                trait_name: trait_name.to_string(),
                method: spec.name.to_string(),
            });
        }

        if !matches_extra_param(tcx, sig.decl, spec.extra_param) {
            return Err(TraitError::InvalidSignature {
                trait_name: trait_name.to_string(),
                method: spec.name.to_string(),
            });
        }

        if !matches_return_spec(tcx, &sig.decl.output, spec.return_spec) {
            return Err(TraitError::InvalidReturnType {
                trait_name: trait_name.to_string(),
                method: spec.name.to_string(),
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Type inspection helpers
// ---------------------------------------------------------------------------

fn extract_self_type_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        path.segments
            .last()
            .map(|s| s.ident.name.as_str().to_string())
    } else {
        None
    }
}

fn extract_trait_identity<'tcx>(tcx: TyCtxt<'tcx>, path: &rustc_hir::Path<'tcx>) -> Option<String> {
    let def_id = match path.res {
        Res::Def(_, def_id) => def_id,
        _ => return None,
    };
    let def_path = tcx.def_path_str(def_id);

    let canonical = if def_path.ends_with("regseq::Lifecycle") {
        Some("regseq::Lifecycle")
    } else if def_path.ends_with("regseq::Serial") {
        Some("regseq::Serial")
    } else if def_path.ends_with("regseq::SerialConsole") {
        Some("regseq::SerialConsole")
    } else if def_path.ends_with("regseq::PowerManagement") {
        Some("regseq::PowerManagement")
    } else {
        None
    }?;

    Some(canonical.to_string())
}

fn is_mut_ref_self(decl: &rustc_hir::FnDecl<'_>) -> bool {
    if decl.inputs.is_empty() {
        return false;
    }
    if let rustc_hir::TyKind::Ref(_, mut_ty) = &decl.inputs[0].kind {
        mut_ty.mutbl.is_mut()
    } else {
        false
    }
}

fn matches_extra_param<'tcx>(
    tcx: TyCtxt<'tcx>,
    decl: &FnDecl<'tcx>,
    expected: ExtraParamSpec,
) -> bool {
    let Some(extra_inputs) = decl.inputs.get(1..) else {
        return matches!(expected, ExtraParamSpec::None);
    };

    match expected {
        ExtraParamSpec::None => extra_inputs.is_empty(),
        ExtraParamSpec::RefNamed(name) => {
            extra_inputs.len() == 1 && matches_ref_named(tcx, &extra_inputs[0], name)
        }
        ExtraParamSpec::RefSlicePrimitive(elem) => {
            extra_inputs.len() == 1 && matches_ref_slice_primitive(&extra_inputs[0], elem)
        }
        ExtraParamSpec::Primitive(name) => {
            extra_inputs.len() == 1 && matches_primitive(&extra_inputs[0], name)
        }
    }
}

fn matches_return_spec<'tcx>(
    tcx: TyCtxt<'tcx>,
    output: &FnRetTy<'tcx>,
    expected: ReturnSpec,
) -> bool {
    match expected {
        ReturnSpec::Unit => is_unit_return(output),
        ReturnSpec::ResultUnitRegseqError => matches_result_unit_regseq_error(tcx, output),
        ReturnSpec::OptionRxChar => matches_option_named(tcx, output, "regseq::RxChar"),
        ReturnSpec::Bool => is_named_return(output, "bool"),
    }
}

fn is_unit_return(output: &FnRetTy<'_>) -> bool {
    match output {
        FnRetTy::DefaultReturn(_) => true,
        FnRetTy::Return(ty) => {
            matches!(&ty.kind, rustc_hir::TyKind::Tup(elems) if elems.is_empty())
        }
    }
}

fn is_named_return(output: &FnRetTy<'_>, name: &str) -> bool {
    match output {
        FnRetTy::DefaultReturn(_) => false,
        FnRetTy::Return(ty) => {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
                path.segments
                    .last()
                    .map(|s| s.ident.name.as_str() == name)
                    .unwrap_or(false)
            } else {
                false
            }
        }
    }
}

fn matches_result_unit_regseq_error<'tcx>(tcx: TyCtxt<'tcx>, output: &FnRetTy<'tcx>) -> bool {
    let FnRetTy::Return(ty) = output else {
        return false;
    };
    let TyKind::Path(QPath::Resolved(_, path)) = &ty.kind else {
        return false;
    };
    let Some(last) = path.segments.last() else {
        return false;
    };
    if last.ident.name.as_str() != "Result" {
        return false;
    }
    let rustc_hir::GenericArgs { args, .. } = match last.args {
        Some(args) => args,
        None => return false,
    };
    if args.len() != 2 {
        return false;
    }
    let unit_ok = match &args[0] {
        GenericArg::Type(ty) => matches!(&ty.kind, TyKind::Tup(elems) if elems.is_empty()),
        _ => false,
    };
    let err_ok = match &args[1] {
        GenericArg::Type(ty) => path_matches_suffix(tcx, *ty, "regseq::RegseqError"),
        _ => false,
    };
    unit_ok && err_ok
}

fn matches_option_named<'tcx>(
    tcx: TyCtxt<'tcx>,
    output: &FnRetTy<'tcx>,
    expected_inner: &str,
) -> bool {
    let FnRetTy::Return(ty) = output else {
        return false;
    };
    let TyKind::Path(QPath::Resolved(_, path)) = &ty.kind else {
        return false;
    };
    let Some(last) = path.segments.last() else {
        return false;
    };
    if last.ident.name.as_str() != "Option" {
        return false;
    }
    let rustc_hir::GenericArgs { args, .. } = match last.args {
        Some(args) => args,
        None => return false,
    };
    if args.len() != 1 {
        return false;
    }
    match &args[0] {
        GenericArg::Type(ty) => path_matches_suffix(tcx, *ty, expected_inner),
        _ => false,
    }
}

fn matches_ref_named<'tcx>(tcx: TyCtxt<'tcx>, ty: &rustc_hir::Ty<'tcx>, expected: &str) -> bool {
    let TyKind::Ref(_, inner) = &ty.kind else {
        return false;
    };
    path_matches_suffix(tcx, inner.ty, expected)
}

fn matches_ref_slice_primitive(ty: &rustc_hir::Ty<'_>, expected_elem: &str) -> bool {
    let TyKind::Ref(_, inner) = &ty.kind else {
        return false;
    };
    let TyKind::Slice(elem_ty) = &inner.ty.kind else {
        return false;
    };
    matches_primitive(elem_ty, expected_elem)
}

fn matches_primitive(ty: &rustc_hir::Ty<'_>, expected: &str) -> bool {
    match &ty.kind {
        TyKind::Path(QPath::Resolved(_, path)) => path
            .segments
            .last()
            .map(|s| s.ident.name.as_str() == expected)
            .unwrap_or(false),
        _ => false,
    }
}

fn path_matches_suffix<'tcx, A>(
    tcx: TyCtxt<'tcx>,
    ty: &rustc_hir::Ty<'tcx, A>,
    expected_suffix: &str,
) -> bool {
    match &ty.kind {
        TyKind::Path(QPath::Resolved(_, path)) => match path.res {
            Res::Def(_, def_id) => tcx.def_path_str(def_id).ends_with(expected_suffix),
            _ => {
                let segments = path
                    .segments
                    .iter()
                    .map(|seg| seg.ident.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                segments.ends_with(expected_suffix)
            }
        },
        _ => false,
    }
}
