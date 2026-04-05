//! MIR place resolution.
//!
//! Resolves `MirPlace` projections into semantic identities grounded
//! in declaration metadata. Used to classify Call receivers and
//! assignment targets.

use regseq_model::declaration::mir_body::{MirPlace, MirProjectionElem, MirTy};

use crate::lowering::plan::ResourceKind;

use super::context::FnContext;

/// Semantic identity of a resolved MIR place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedPlace {
    /// The return place (local 0).
    ReturnPlace,
    /// The `self` reference (local 1 with Deref for methods).
    SelfRef,
    /// A named field on the driver struct.
    DriverField { name: String },
    /// An MMIO block field on the driver struct.
    MmioBlock { mmio_field: String },
    /// A register within an MMIO block.
    RegisterAccess {
        mmio_field: String,
        register: String,
    },
    /// An optional resource field (clock, reset, pinctrl).
    ResourceField { name: String, kind: ResourceKind },
    /// A local variable.
    Local { index: u32, name: Option<String> },
    /// A function parameter.
    Param { index: u32, name: Option<String> },
    /// Unresolvable place.
    Unknown,
}

/// Resolve a `MirPlace` to its semantic identity.
///
/// Uses the function's local declarations and the shared declaration
/// metadata to classify what a place refers to (register, resource,
/// driver field, etc.).
pub fn resolve_place(ctx: &FnContext<'_>, place: &MirPlace) -> ResolvedPlace {
    let local = place.local;

    // Return place.
    if local == 0 {
        if place.projection.is_empty() {
            return ResolvedPlace::ReturnPlace;
        }
        return ResolvedPlace::ReturnPlace;
    }

    // Self reference (local 1 for methods with receiver).
    if local == 1 && ctx.has_self_receiver {
        return resolve_self_projection(ctx, &place.projection);
    }

    // Function parameter.
    if local <= ctx.arg_count as u32 {
        let name = ctx.local_name(local).map(|s| s.to_string());
        if place.projection.is_empty() {
            return ResolvedPlace::Param { index: local, name };
        }
        return ResolvedPlace::Param { index: local, name };
    }

    // Local variable.
    let name = ctx.local_name(local).map(|s| s.to_string());
    ResolvedPlace::Local { index: local, name }
}

/// Resolve a projection chain on `self`.
fn resolve_self_projection(ctx: &FnContext<'_>, projection: &[MirProjectionElem]) -> ResolvedPlace {
    if projection.is_empty() {
        return ResolvedPlace::SelfRef;
    }

    // Walk the projection chain.
    let mut current = SelfWalkState::SelfRef;

    for elem in projection {
        match elem {
            MirProjectionElem::Deref => {
                // Dereferencing &mut self gives us the driver struct.
                // Stay at current level.
            }
            MirProjectionElem::Field { ty, .. } => {
                current = advance_field(ctx, &current, ty);
            }
            _ => {
                // Other projections (Index, Downcast, etc.) don't change
                // the high-level semantic identity.
            }
        }
    }

    match current {
        SelfWalkState::SelfRef => ResolvedPlace::SelfRef,
        SelfWalkState::DriverField { name } => {
            // Check if this field is a known resource.
            if let Some(kind) = ctx.shared.resource_kind(&name) {
                return ResolvedPlace::ResourceField { name, kind };
            }
            // Check if this field is an MMIO field.
            if ctx.shared.mmio_fields.contains(&name) {
                return ResolvedPlace::MmioBlock { mmio_field: name };
            }
            ResolvedPlace::DriverField { name }
        }
        SelfWalkState::MmioBlock { mmio_field } => ResolvedPlace::MmioBlock { mmio_field },
        SelfWalkState::RegisterAccess {
            mmio_field,
            register,
        } => ResolvedPlace::RegisterAccess {
            mmio_field,
            register,
        },
    }
}

/// Internal state for walking self projections.
enum SelfWalkState {
    SelfRef,
    DriverField {
        name: String,
    },
    MmioBlock {
        mmio_field: String,
    },
    RegisterAccess {
        mmio_field: String,
        register: String,
    },
}

/// Advance the walk state through a Field projection.
fn advance_field(ctx: &FnContext<'_>, state: &SelfWalkState, ty: &MirTy) -> SelfWalkState {
    match state {
        SelfWalkState::SelfRef => {
            // Field on self → driver struct field.
            let name = identify_field_from_type(ctx, ty);
            if ctx.shared.is_mmio_type(&name) {
                // This is an MMIO wrapper field.
                let mmio_field = find_mmio_field_name(ctx, ty);
                SelfWalkState::MmioBlock { mmio_field }
            } else {
                SelfWalkState::DriverField { name }
            }
        }
        SelfWalkState::DriverField { name } => {
            // Projecting through an Option (e.g., self.clock deref).
            SelfWalkState::DriverField { name: name.clone() }
        }
        SelfWalkState::MmioBlock { mmio_field } => {
            // Field on an MMIO block → register access.
            if ctx.shared.is_register_wrapper_type(&type_path(ty)) {
                let register = identify_register_from_type(ctx, ty);
                SelfWalkState::RegisterAccess {
                    mmio_field: mmio_field.clone(),
                    register,
                }
            } else {
                // Projecting through Mmio's inner register block.
                SelfWalkState::MmioBlock {
                    mmio_field: mmio_field.clone(),
                }
            }
        }
        SelfWalkState::RegisterAccess {
            mmio_field,
            register,
        } => SelfWalkState::RegisterAccess {
            mmio_field: mmio_field.clone(),
            register: register.clone(),
        },
    }
}

/// Extract the type path string from a MirTy.
fn type_path(ty: &MirTy) -> String {
    match ty {
        MirTy::Adt { path, .. } => path.clone(),
        MirTy::Ref { ty: inner, .. } => type_path(inner),
        _ => String::new(),
    }
}

/// Identify a field name from its type.
///
/// For MMIO wrappers, returns the MMIO type path.
/// For other fields, tries to match against known driver fields.
fn identify_field_from_type(ctx: &FnContext<'_>, ty: &MirTy) -> String {
    let path = type_path(ty);
    if path.is_empty() {
        return "unknown".into();
    }

    // Check if this is an MMIO type.
    if ctx.shared.is_mmio_type(&path) {
        return path;
    }

    // Check if this is a resource type.
    for prefix in &ctx.shared.resource_type_paths {
        if path.starts_with(prefix) {
            // Return the first matching resource field.
            if let Some((name, _)) = ctx.shared.resource_fields.first() {
                return name.clone();
            }
        }
    }

    // Check for Option wrapper around resource types.
    if let MirTy::Adt { path: outer, args } = ty {
        if outer.contains("Option") && !args.is_empty() {
            let inner_path = type_path(&args[0]);
            for prefix in &ctx.shared.resource_type_paths {
                if inner_path.starts_with(prefix) {
                    for (name, _) in &ctx.shared.resource_fields {
                        if inner_path.contains("Clock") && name.contains("clock") {
                            return name.clone();
                        }
                        if inner_path.contains("Reset") && name.contains("reset") {
                            return name.clone();
                        }
                        if inner_path.contains("PinCtrl") && name.contains("pinctrl") {
                            return name.clone();
                        }
                    }
                }
            }
        }
    }

    path
}

/// Find the MMIO field name from an MMIO type.
fn find_mmio_field_name(ctx: &FnContext<'_>, _ty: &MirTy) -> String {
    // Use the first MMIO field name from the declaration.
    ctx.shared
        .mmio_fields
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(|| "regs".into())
}

/// Identify a register name from a register wrapper type.
///
/// The type path carries the register module (e.g.,
/// `regseq::ReadWrite<u32, registers::CR_REG::Register>` → "cr").
fn identify_register_from_type(ctx: &FnContext<'_>, ty: &MirTy) -> String {
    if let MirTy::Adt { args, .. } = ty {
        // The register type is typically the second generic argument.
        if args.len() >= 2 {
            let reg_type_path = type_path(&args[1]);
            // Extract register name from path like "registers::CR_REG::Register".
            let parts: Vec<&str> = reg_type_path.split("::").collect();
            if let Some(reg_module) = parts.iter().rev().nth(1) {
                // Convert "CR_REG" to "cr" by stripping _REG suffix and lowercasing.
                let name = reg_module
                    .strip_suffix("_REG")
                    .unwrap_or(reg_module)
                    .to_lowercase();
                if ctx.shared.register_field_names.contains(&name) {
                    return name;
                }
            }
        }
    }
    // Also check Ref wrapper.
    if let MirTy::Ref { ty: inner, .. } = ty {
        return identify_register_from_type(ctx, inner);
    }
    "unknown".into()
}
