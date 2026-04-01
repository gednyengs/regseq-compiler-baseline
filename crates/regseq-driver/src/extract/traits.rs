use rustc_hir::{FnRetTy, ImplItemKind, ItemKind};
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

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraitError {
    MissingMethod { trait_name: String, method: String },
    InvalidReturnType { trait_name: String, method: String },
    InvalidReceiver { trait_name: String, method: String },
}

// ---------------------------------------------------------------------------
// Expected method shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnShape {
    Unit,
    Result,
    Option,
    Bool,
}

struct MethodSpec {
    name: &'static str,
    return_shape: ReturnShape,
}

const LIFECYCLE_METHODS: &[MethodSpec] = &[
    MethodSpec {
        name: "init",
        return_shape: ReturnShape::Result,
    },
    MethodSpec {
        name: "deinit",
        return_shape: ReturnShape::Result,
    },
];

const SERIAL_METHODS: &[MethodSpec] = &[
    MethodSpec {
        name: "port_open",
        return_shape: ReturnShape::Result,
    },
    MethodSpec {
        name: "port_close",
        return_shape: ReturnShape::Unit,
    },
    MethodSpec {
        name: "configure",
        return_shape: ReturnShape::Result,
    },
    MethodSpec {
        name: "tx_byte",
        return_shape: ReturnShape::Unit,
    },
    MethodSpec {
        name: "rx_char",
        return_shape: ReturnShape::Option,
    },
    MethodSpec {
        name: "tx_ready",
        return_shape: ReturnShape::Bool,
    },
];

const SERIAL_CONSOLE_METHODS: &[MethodSpec] = &[MethodSpec {
    name: "console_write",
    return_shape: ReturnShape::Unit,
}];

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract recognized trait callback-family implementations from HIR.
pub fn extract_trait_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<TraitSurface, TraitError> {
    let mut lifecycle_impls = Vec::new();
    let mut serial_impls = Vec::new();
    let mut serial_console_impls = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        let ItemKind::Impl(impl_block) = &item.kind else {
            continue;
        };

        let Some(trait_ref) = &impl_block.of_trait else {
            continue;
        };

        let Some(last_seg) = trait_ref.trait_ref.path.segments.last() else {
            continue;
        };
        let trait_name = last_seg.ident.name.as_str();

        let Some(driver_type) = extract_self_type_name(impl_block.self_ty) else {
            continue;
        };

        match trait_name {
            "Lifecycle" => {
                validate_methods(tcx, impl_block, "Lifecycle", LIFECYCLE_METHODS)?;
                lifecycle_impls.push(LifecycleImpl {
                    driver_type,
                    context: CallbackContext::FrameworkManaged,
                });
            }
            "Serial" => {
                validate_methods(tcx, impl_block, "Serial", SERIAL_METHODS)?;
                serial_impls.push(SerialImpl {
                    driver_type,
                    context: CallbackContext::SubsystemSerialized,
                });
            }
            "SerialConsole" => {
                validate_methods(tcx, impl_block, "SerialConsole", SERIAL_CONSOLE_METHODS)?;
                serial_console_impls.push(SerialConsoleImpl {
                    driver_type,
                    context: CallbackContext::SubsystemSerialized,
                });
            }
            _ => {}
        }
    }

    Ok(TraitSurface {
        lifecycle_impls,
        serial_impls,
        serial_console_impls,
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

        if !matches_return_shape(&sig.decl.output, spec.return_shape) {
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

fn matches_return_shape(output: &FnRetTy<'_>, expected: ReturnShape) -> bool {
    match expected {
        ReturnShape::Unit => is_unit_return(output),
        ReturnShape::Result => is_named_return(output, "Result"),
        ReturnShape::Option => is_named_return(output, "Option"),
        ReturnShape::Bool => is_named_return(output, "bool"),
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
