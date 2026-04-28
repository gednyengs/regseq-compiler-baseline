//! Driver-struct-level rule bucket (task 56).
//!
//! Three checks live here, in deterministic-firing order:
//!
//! - **Resource attribute completeness** (spec.md Section 13 line
//!   1426): every clock/reset/pinctrl/interrupt/mmio/sync resource
//!   has its declaration-side identifier(s) populated. Empty
//!   identifier strings count as unpopulated.
//! - **No duplicate IDs** (spec.md Section 13 line 1426): within a
//!   single resource kind, every declared id string is unique.
//! - **User-data fields are admitted by one of two rules**: Rule 1
//!   (zero-init-friendly whitelist) or Rule 2 (`Option<T>` where
//!   `T: Copy`). Every `DriverStruct.user_data` field's
//!   `UserDataKind` is matched, and the inner type of either variant
//!   is validated to be `Copy` (the walker recurses into user-defined
//!   structs/enums so nested non-Copy fields are caught). Raised as
//!   [`VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy`] on
//!   violations.
//!
//! All three rules walk every Driver module the model carries (the
//! verifier framework does not require the "exactly one Driver
//! module" invariant -- that lives in [`super::module`] -- so this
//! bucket tolerates pathological input by visiting each Driver
//! module independently).

use std::collections::BTreeMap;

use crate::identity::{DeclId, SourceOrigin, Symbol};
use crate::ir::{RegseqType, ScalarType};
use crate::model::driver::DriverStruct;
use crate::model::framework::{FrameworkTypeKind, TypeDefClassification};
use crate::model::module::Module;
use crate::model::regseq_model::RegseqModel;
use crate::model::resources::{SyncKind, UserDataField, UserDataKind};
use crate::model::type_def::{TypeDef, TypeDefKind};

use super::super::{VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance};

/// Run every driver-struct-level check in deterministic order,
/// returning the first violation.
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    walk_drivers(model, &mut |d| resource_attribute_completeness(model, d))?;
    walk_drivers(model, &mut |d| no_duplicate_resource_ids(model, d))?;
    walk_drivers(model, &mut |d| user_data_option_copy(model, d))?;
    Ok(())
}

// --- Resource attribute completeness --------------------------------

fn resource_attribute_completeness(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<(), VerifierError> {
    for r in &driver.clocks {
        check_id_symbol(model, r.id.decl_id(), r.decl.clock_id, "clock_id")?;
    }
    for r in &driver.resets {
        check_id_symbol(model, r.id.decl_id(), r.decl.reset_id, "reset_id")?;
    }
    for r in &driver.pinctrl {
        check_id_symbol(model, r.id.decl_id(), r.decl.pinctrl_id, "pinctrl_id")?;
    }
    for r in &driver.interrupts {
        check_id_symbol(model, r.id.decl_id(), r.decl.interrupt_id, "interrupt_id")?;
    }
    for r in &driver.mmio {
        check_id_symbol(model, r.id.decl_id(), r.decl.mmio_id, "mmio_id")?;
    }
    for r in &driver.sync {
        // SyncResource has no declaration-side `id` Symbol -- its
        // identity is the (`field_name`, `sync_kind`) pair. Validate
        // that the field name is a non-empty Symbol and (for non-
        // Completion kinds) the payload type is present.
        check_id_symbol(model, r.id.decl_id(), r.field_name, "field_name")?;
        if r.sync_kind != SyncKind::Completion && r.payload_type.is_none() {
            return Err(missing_attribute(
                model,
                r.id.decl_id(),
                "payload_type",
                "sync resource",
            ));
        }
    }
    Ok(())
}

fn check_id_symbol(
    model: &RegseqModel,
    decl_id: DeclId,
    sym: Symbol,
    attribute: &'static str,
) -> Result<(), VerifierError> {
    let resolved = model.symbols.resolve(sym);
    if resolved.is_empty() {
        return Err(missing_attribute(model, decl_id, attribute, "resource"));
    }
    Ok(())
}

fn missing_attribute(
    model: &RegseqModel,
    decl_id: DeclId,
    attribute: &'static str,
    site: &'static str,
) -> VerifierError {
    VerifierError {
        level: VerifierLevel::DriverStruct,
        kind: VerifierErrorKind::ResourceAttributeIncomplete {
            resource: decl_id,
            attribute,
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", decl_id.index()),
            lookup_origin(model, decl_id),
        ),
        details: format!(
            "{site} (DeclId {}) is missing required declaration-side attribute '{}'",
            decl_id.index(),
            attribute
        ),
    }
}

// --- No duplicate resource IDs --------------------------------------

fn no_duplicate_resource_ids(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<(), VerifierError> {
    check_unique(
        model,
        "clock",
        driver
            .clocks
            .iter()
            .map(|r| (r.id.decl_id(), r.decl.clock_id)),
    )?;
    check_unique(
        model,
        "reset",
        driver
            .resets
            .iter()
            .map(|r| (r.id.decl_id(), r.decl.reset_id)),
    )?;
    check_unique(
        model,
        "pinctrl",
        driver
            .pinctrl
            .iter()
            .map(|r| (r.id.decl_id(), r.decl.pinctrl_id)),
    )?;
    check_unique(
        model,
        "interrupt",
        driver
            .interrupts
            .iter()
            .map(|r| (r.id.decl_id(), r.decl.interrupt_id)),
    )?;
    check_unique(
        model,
        "mmio",
        driver.mmio.iter().map(|r| (r.id.decl_id(), r.decl.mmio_id)),
    )?;
    Ok(())
}

fn check_unique<I>(model: &RegseqModel, kind: &'static str, iter: I) -> Result<(), VerifierError>
where
    I: Iterator<Item = (DeclId, Symbol)>,
{
    // BTreeMap keyed on the resolved-string form so iteration order is
    // deterministic and the first-detected duplicate is stable across
    // runs.
    let mut seen: BTreeMap<String, DeclId> = BTreeMap::new();
    for (decl_id, sym) in iter {
        let id_string = model.symbols.resolve(sym).to_owned();
        if id_string.is_empty() {
            // Empty ids are reported separately by
            // resource_attribute_completeness; skip here so we don't
            // double-report.
            continue;
        }
        if let Some(first) = seen.insert(id_string.clone(), decl_id) {
            // Restore the first occurrence's witness so the error
            // reports the second-seen DeclId, but the reported "id"
            // string is what's duplicated.
            seen.insert(id_string.clone(), first);
            return Err(VerifierError {
                level: VerifierLevel::DriverStruct,
                kind: VerifierErrorKind::DuplicateResourceId {
                    id: id_string.clone(),
                    kind,
                },
                provenance: VerifierProvenance::for_item(
                    format!("decl#{}", decl_id.index()),
                    lookup_origin(model, decl_id),
                ),
                details: format!(
                    "{kind} resource at DeclId {} duplicates id '{}' (first seen at DeclId {})",
                    decl_id.index(),
                    id_string,
                    first.index()
                ),
            });
        }
    }
    Ok(())
}

// --- User-data fields are Option<T> where T: Copy -------------------

fn user_data_option_copy(model: &RegseqModel, driver: &DriverStruct) -> Result<(), VerifierError> {
    let type_defs = collect_type_defs(model);
    for u in &driver.user_data {
        let accepted = match &u.kind {
            // Rule 2: Option<T> admits any T: Copy.
            UserDataKind::Option { inner_type } => is_copy(inner_type, &type_defs, &mut Vec::new()),
            // Rule 1: ZeroInit admits only the Stage-1 whitelist
            // (u8/u16/u32/u64/bool, arrays of accepted types, and
            // user-defined Copy structs recursively composed of
            // accepted fields). Signed ints, usize/isize, floats,
            // char, u128/i128, Option, Result, Slice, Ref, Unit,
            // Never, framework types, abstraction types, enums are
            // all rejected.
            UserDataKind::ZeroInit { inner_type } => {
                rule1_whitelist(inner_type, &type_defs, &mut Vec::new())
            }
        };
        if !accepted {
            let inner = match &u.kind {
                UserDataKind::Option { inner_type } | UserDataKind::ZeroInit { inner_type } => {
                    inner_type
                }
            };
            return Err(user_data_not_copy(model, u, inner));
        }
    }
    Ok(())
}

/// Verifier-side Rule-1 zero-init whitelist predicate.
///
/// Admits the same shapes the construction-side walker does, but on
/// the portable [`RegseqType`] surface (the verifier does not have
/// access to `rustc` types). Returns `true` iff `ty` is:
/// - A whitelisted scalar (`u8`, `u16`, `u32`, `u64`).
/// - `Bool`.
/// - An `Array` whose element is itself accepted.
/// - A `Defined(TypeDefId)` resolving to a user-defined struct whose
///   fields are every one accepted.
///
/// Every other variant (signed integers and platform-width scalars
/// via `is_scalar_rule1`; `Unit`, `Never`, `Option`, `Result`,
/// `Slice`, `Ref`; enum/union variants of user-defined types; and
/// abstraction / framework types) is rejected.
fn rule1_whitelist(
    ty: &RegseqType,
    type_defs: &BTreeMap<DeclId, &TypeDef>,
    visiting: &mut Vec<DeclId>,
) -> bool {
    match ty {
        RegseqType::Bool => true,
        RegseqType::Scalar(s) => is_scalar_rule1(*s),
        RegseqType::Array(inner, _) => rule1_whitelist(inner, type_defs, visiting),
        RegseqType::Defined(target) => {
            let decl_id = target.decl_id();
            if visiting.contains(&decl_id) {
                return true;
            }
            let Some(def) = type_defs.get(&decl_id) else {
                // Dangling reference: the cross-item rule bucket
                // already reports this separately. Mirror `is_copy`
                // and return `true` here to avoid double-reporting.
                return true;
            };
            match &def.classification {
                TypeDefClassification::UserDefined => {
                    visiting.push(decl_id);
                    let accepted = match &def.kind {
                        TypeDefKind::Struct { fields } => fields
                            .iter()
                            .all(|f| rule1_whitelist(&f.ty, type_defs, visiting)),
                        // Enums are rejected under Rule 1 regardless
                        // of whether every variant happens to be
                        // field-free (the construction-side walker
                        // applies the same narrowing).
                        TypeDefKind::Enum { .. } => false,
                    };
                    visiting.pop();
                    accepted
                }
                // Abstraction types (Clock, Mutex, etc.) are
                // resource handles, not zero-init data.
                TypeDefClassification::Abstraction(_) => false,
                // Framework types are scalar wrappers at the facade;
                // they are out of the Stage-1 user-data whitelist.
                TypeDefClassification::Framework(_) => false,
            }
        }
        RegseqType::Unit
        | RegseqType::Never
        | RegseqType::Option(_)
        | RegseqType::Result(..)
        | RegseqType::Slice(_)
        | RegseqType::Ref(..) => false,
    }
}

/// Stage-1 Rule-1 scalar predicate. Admits exactly `u8`, `u16`,
/// `u32`, `u64`; rejects every other [`ScalarType`] variant
/// (signed ints, platform-width `Usize`/`Isize`).
fn is_scalar_rule1(s: ScalarType) -> bool {
    matches!(
        s,
        ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
    )
}

fn user_data_not_copy(
    model: &RegseqModel,
    field: &UserDataField,
    offending: &RegseqType,
) -> VerifierError {
    let decl_id = field.id.decl_id();
    VerifierError {
        level: VerifierLevel::DriverStruct,
        kind: VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy {
            field: decl_id,
            offending_ty: format!("{:?}", offending),
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", decl_id.index()),
            lookup_origin(model, decl_id),
        ),
        details: format!(
            "user-data field (DeclId {}) inner type {:?} is not accepted; spec requires \
             either a zero-init-whitelisted type (Rule 1) or Option<T> where T: Copy (Rule 2)",
            decl_id.index(),
            offending
        ),
    }
}

/// Build a `BTreeMap` from `TypeDefId.decl_id()` to `&TypeDef`.
///
/// Keyed by the underlying [`DeclId`] because `TypeDefId` does not
/// implement `Ord`. The walker uses this map to recurse into
/// user-defined struct/enum field types when checking `Copy`.
fn collect_type_defs(model: &RegseqModel) -> BTreeMap<DeclId, &TypeDef> {
    let mut map: BTreeMap<DeclId, &TypeDef> = BTreeMap::new();
    fn walk<'a>(map: &mut BTreeMap<DeclId, &'a TypeDef>, m: &'a Module) {
        for t in &m.type_defs {
            map.insert(t.id.decl_id(), t);
        }
        for sub in &m.submodules {
            walk(map, sub);
        }
    }
    walk(&mut map, &model.root_module);
    map
}

/// Return `true` if the given `RegseqType` is `Copy`.
///
/// Recursive types (`Defined(TypeDefId)` cycles) are guarded with a
/// `visiting` stack so a struct that references itself (impossible in
/// safe Rust without indirection, but trivial to express in the
/// model) does not cause unbounded recursion. A type currently on
/// the stack is treated as `Copy` (the cycle does not introduce a
/// non-Copy field).
fn is_copy(
    ty: &RegseqType,
    type_defs: &BTreeMap<DeclId, &TypeDef>,
    visiting: &mut Vec<DeclId>,
) -> bool {
    match ty {
        RegseqType::Scalar(s) => is_scalar_copy(*s),
        RegseqType::Bool | RegseqType::Unit | RegseqType::Never => true,
        // Reference is Copy when shared, not when mutable. Mutable
        // references are NOT Copy in Rust.
        RegseqType::Ref(_, mutability) => {
            matches!(mutability, crate::ir::Mutability::Shared)
        }
        // `Slice<T>` is `[T]` -- an unsized type, never Copy.
        RegseqType::Slice(_) => false,
        // Composite types are Copy iff every component is Copy.
        RegseqType::Option(inner) | RegseqType::Array(inner, _) => {
            is_copy(inner, type_defs, visiting)
        }
        RegseqType::Result(ok, err) => {
            is_copy(ok, type_defs, visiting) && is_copy(err, type_defs, visiting)
        }
        RegseqType::Defined(target) => {
            let decl_id = target.decl_id();
            if visiting.contains(&decl_id) {
                return true;
            }
            let Some(def) = type_defs.get(&decl_id) else {
                // The dangling-reference case is reported separately
                // by the cross-item rule bucket. Avoid double-failing
                // by treating as Copy here.
                return true;
            };
            // Per .harness/tasks/regseq-model/56-post-phase2-verifier.md:89-94,
            // the user-data Copy check reads `TypeDef.classification`
            // and only walks fields recursively for user-defined
            // structs/enums. Abstraction types (Clock, Mutex, etc.)
            // are resource handles and are not Copy. Framework types
            // are scalars or scalar-like wrappers (Duration, DataBits,
            // etc.); they are Copy by construction since the Layer-1
            // facade exports them as plain Copy types.
            match &def.classification {
                TypeDefClassification::UserDefined => {
                    visiting.push(decl_id);
                    let res = match &def.kind {
                        TypeDefKind::Struct { fields } => {
                            fields.iter().all(|f| is_copy(&f.ty, type_defs, visiting))
                        }
                        TypeDefKind::Enum { variants } => variants
                            .iter()
                            .all(|v| v.fields.iter().all(|f| is_copy(&f.ty, type_defs, visiting))),
                    };
                    visiting.pop();
                    res
                }
                TypeDefClassification::Abstraction(_) => false,
                TypeDefClassification::Framework(kind) => is_framework_copy(*kind),
            }
        }
    }
}

/// Whether a Layer-1 framework type is `Copy`.
///
/// Mirrors the `#[derive(Copy)]` decoration on the facade crate's
/// `#[rseq::framework_type("...")]` items at
/// `crates/regseq/src/{serial,time,error,irq}.rs`. Hand-pinned
/// because the model does not surface a `Copy` flag on `TypeDef` --
/// the source of truth is the facade. Keep this table in sync if
/// the facade changes its derives.
///
/// Current facade derives:
/// - DataBits, Parity, StopBits, FlowControl: enums, derive Copy.
/// - SerialConfig: struct of Copy fields, but does NOT derive Copy.
/// - SerialErrors: struct, derives Copy.
/// - RxChar: struct, derives Copy.
/// - Duration: struct, derives Copy.
/// - RegseqError: enum, derives Copy.
/// - IrqResult: enum, derives Copy.
fn is_framework_copy(kind: FrameworkTypeKind) -> bool {
    match kind {
        FrameworkTypeKind::DataBits
        | FrameworkTypeKind::Parity
        | FrameworkTypeKind::StopBits
        | FrameworkTypeKind::FlowControl
        | FrameworkTypeKind::IrqResult => true,
        FrameworkTypeKind::Duration => true,
        FrameworkTypeKind::RxChar => true,
        FrameworkTypeKind::SerialErrors => true,
        FrameworkTypeKind::RegseqError => true,
        // SerialConfig is a struct of Copy fields but the facade
        // does NOT derive `Copy` on it (see
        // `crates/regseq/src/serial.rs` -- only the
        // `#[rseq::framework_type("serial-config")]` attribute is
        // applied; no `#[derive(Copy)]`). Reject it.
        FrameworkTypeKind::SerialConfig => false,
    }
}

fn is_scalar_copy(_s: ScalarType) -> bool {
    // All Rust integer types are Copy; the model's ScalarType set is
    // limited to those.
    true
}

// --- Walking ---------------------------------------------------------

fn walk_drivers<F>(model: &RegseqModel, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&DriverStruct) -> Result<(), VerifierError>,
{
    walk_modules(&model.root_module, &mut |m: &Module| {
        if let Some(d) = &m.driver {
            f(d)?;
        }
        Ok(())
    })
}

fn walk_modules<F>(root: &Module, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&Module) -> Result<(), VerifierError>,
{
    f(root)?;
    for s in &root.submodules {
        walk_modules(s, f)?;
    }
    Ok(())
}

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
