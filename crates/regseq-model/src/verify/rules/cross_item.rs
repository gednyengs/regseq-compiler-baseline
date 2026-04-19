//! Cross-item-level rule bucket (task 55).
//!
//! Two checks live here:
//!
//! - **Cross-reference resolution** (spec.md Section 13 line 1431):
//!   every [`TypeDefId`], [`FunctionId`], and [`RegisterBlockId`]
//!   appearing in a function body, function signature, resource
//!   binding slot, or an [`ImplBlock::target_type`] must match a
//!   declared entity in the model. Raises the appropriate
//!   `Unresolved*Id` variant on the first failure.
//! - **Source-location coverage** (spec.md Section 13 line 1432):
//!   every [`DeclId`] assigned during Phase 5 Pass 1 (module,
//!   type-def, function, register block/descriptor, resource,
//!   property, user-data, constant) has an entry in
//!   [`RegseqModel::source_locations`]. Raises
//!   [`VerifierErrorKind::MissingSourceLocation`].
//!
//! The walker is fail-fast: it returns the first violation. Declared
//! entities are collected in one pass, then the reference-resolution
//! pass walks items in document order (Vec order for module items and
//! driver-struct fields). Lookups use [`BTreeSet`] so both presence
//! tests and (when needed) reverse iteration are deterministic.
//!
//! [`TypeDefId`]: crate::identity::TypeDefId
//! [`FunctionId`]: crate::identity::FunctionId
//! [`RegisterBlockId`]: crate::identity::RegisterBlockId
//! [`DeclId`]: crate::identity::DeclId
//! [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
//! [`ImplBlock::target_type`]: crate::model::impl_block::ImplBlock::target_type
//! [`VerifierErrorKind::MissingSourceLocation`]: super::super::VerifierErrorKind::MissingSourceLocation

use std::collections::BTreeSet;

use crate::identity::{
    DeclId, FunctionId, RegisterBlockId, RegisterDescriptorId, SourceOrigin, TypeDefId,
};
use crate::ir::{
    AggregateKind, IntrinsicMetadata, MirIntrinsic, Operand, Place, Projection, RegseqType, Rvalue,
    StatementKind, TerminatorKind,
};
use crate::model::driver::DriverStruct;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::module::Module;
use crate::model::register::RegisterBlock;
use crate::model::regseq_model::RegseqModel;
use crate::model::resources::SyncResource;
use crate::model::type_def::{TypeDef, TypeDefKind};

use super::super::{VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance};

/// Run cross-item-level rules in deterministic order, returning the
/// first violation.
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    let declared = DeclaredIds::collect(model);
    resolve_references(model, &declared)?;
    source_location_coverage(model, &declared)?;
    Ok(())
}

/// Sets of declared entity ids.
///
/// Typed wrappers like [`TypeDefId`] do not derive `Ord`, so the kind
/// buckets below key on [`DeclId`] directly and the lookup sites
/// convert via `.decl_id()`.
///
/// `all_decls` captures every [`DeclId`] that should have an entry in
/// [`RegseqModel::source_locations`]. Iteration is in `DeclId` order
/// (canonical `BTreeSet` ordering) so the source-location coverage
/// pass reports the first violation deterministically.
///
/// [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
struct DeclaredIds {
    type_defs: BTreeSet<DeclId>,
    functions: BTreeSet<DeclId>,
    register_blocks: BTreeSet<DeclId>,
    register_descriptors: BTreeSet<DeclId>,
    all_decls: BTreeSet<DeclId>,
}

impl DeclaredIds {
    fn collect(model: &RegseqModel) -> Self {
        let mut d = DeclaredIds {
            type_defs: BTreeSet::new(),
            functions: BTreeSet::new(),
            register_blocks: BTreeSet::new(),
            register_descriptors: BTreeSet::new(),
            all_decls: BTreeSet::new(),
        };
        d.visit_module(&model.root_module);
        d
    }

    fn visit_module(&mut self, m: &Module) {
        self.all_decls.insert(m.id.decl_id());

        for c in &m.constants {
            self.all_decls.insert(c.id.decl_id());
        }
        for f in &m.free_functions {
            self.functions.insert(f.id.decl_id());
            self.all_decls.insert(f.id.decl_id());
        }
        for t in &m.type_defs {
            self.type_defs.insert(t.id.decl_id());
            self.all_decls.insert(t.id.decl_id());
        }
        for r in &m.register_descriptors {
            self.register_descriptors.insert(r.id.decl_id());
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &m.register_blocks {
            self.register_blocks.insert(r.id.decl_id());
            self.all_decls.insert(r.id.decl_id());
        }
        for impl_block in &m.impl_blocks {
            self.visit_impl_block(impl_block);
        }
        if let Some(driver) = &m.driver {
            self.visit_driver(driver);
        }
        for impl_block in &m.trait_impls {
            self.visit_impl_block(impl_block);
        }
        for impl_block in &m.driver_impl_blocks {
            self.visit_impl_block(impl_block);
        }
        for sub in &m.submodules {
            self.visit_module(sub);
        }
    }

    fn visit_impl_block(&mut self, impl_block: &ImplBlock) {
        for f in &impl_block.methods {
            self.functions.insert(f.id.decl_id());
            self.all_decls.insert(f.id.decl_id());
        }
        for c in &impl_block.associated_constants {
            self.all_decls.insert(c.id.decl_id());
        }
    }

    fn visit_driver(&mut self, driver: &DriverStruct) {
        for r in &driver.clocks {
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &driver.resets {
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &driver.pinctrl {
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &driver.interrupts {
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &driver.mmio {
            self.all_decls.insert(r.id.decl_id());
        }
        for r in &driver.sync {
            self.all_decls.insert(r.id.decl_id());
        }
        for p in &driver.properties {
            self.all_decls.insert(p.id.decl_id());
        }
        for u in &driver.user_data {
            self.all_decls.insert(u.id.decl_id());
        }
    }

    fn has_type_def(&self, id: TypeDefId) -> bool {
        self.type_defs.contains(&id.decl_id())
    }

    fn has_function(&self, id: FunctionId) -> bool {
        self.functions.contains(&id.decl_id())
    }

    fn has_register_block(&self, id: RegisterBlockId) -> bool {
        self.register_blocks.contains(&id.decl_id())
    }

    fn has_register_descriptor(&self, id: RegisterDescriptorId) -> bool {
        self.register_descriptors.contains(&id.decl_id())
    }
}

// --- Reference resolution -------------------------------------------

fn resolve_references(model: &RegseqModel, declared: &DeclaredIds) -> Result<(), VerifierError> {
    // Walk top-down in the same canonical order as the declaration
    // collector above so error output is deterministic and stable
    // across runs.
    resolve_module(model, declared, &model.root_module)
}

fn resolve_module(
    model: &RegseqModel,
    declared: &DeclaredIds,
    m: &Module,
) -> Result<(), VerifierError> {
    for f in &m.free_functions {
        resolve_function(model, declared, f)?;
    }
    for t in &m.type_defs {
        resolve_type_def(model, declared, t)?;
    }
    for rb in &m.register_blocks {
        resolve_register_block(model, declared, rb)?;
    }
    for impl_block in &m.impl_blocks {
        resolve_impl_block(model, declared, impl_block)?;
    }
    if let Some(driver) = &m.driver {
        resolve_driver(model, declared, driver)?;
    }
    for impl_block in &m.trait_impls {
        resolve_impl_block(model, declared, impl_block)?;
    }
    for impl_block in &m.driver_impl_blocks {
        resolve_impl_block(model, declared, impl_block)?;
    }
    for sub in &m.submodules {
        resolve_module(model, declared, sub)?;
    }
    Ok(())
}

fn resolve_impl_block(
    model: &RegseqModel,
    declared: &DeclaredIds,
    impl_block: &ImplBlock,
) -> Result<(), VerifierError> {
    let context = impl_block.target_type.decl_id();
    check_type_def_id(
        model,
        declared,
        context,
        impl_block.target_type,
        "impl block",
    )?;
    for f in &impl_block.methods {
        resolve_function(model, declared, f)?;
    }
    Ok(())
}

fn resolve_function(
    model: &RegseqModel,
    declared: &DeclaredIds,
    f: &Function,
) -> Result<(), VerifierError> {
    let context = f.id.decl_id();
    for p in &f.params {
        visit_type(model, declared, context, &p.ty, "function parameter")?;
    }
    visit_type(
        model,
        declared,
        context,
        &f.return_type,
        "function return type",
    )?;

    for local in &f.body.local_decls {
        visit_type(model, declared, context, &local.ty, "local declaration")?;
    }
    for block in &f.body.basic_blocks {
        for stmt in &block.statements {
            visit_statement_kind(model, declared, context, &stmt.kind)?;
        }
        visit_terminator_kind(model, declared, context, &block.terminator.kind)?;
    }
    Ok(())
}

fn resolve_register_block(
    model: &RegseqModel,
    declared: &DeclaredIds,
    rb: &RegisterBlock,
) -> Result<(), VerifierError> {
    // Fields are walked in document order (Vec order). A field with
    // `descriptor == None` represents a reserved gap and does not
    // need resolution.
    let context = rb.id.decl_id();
    for field in &rb.fields {
        if let Some(descriptor) = field.descriptor {
            check_register_descriptor_id(
                model,
                declared,
                context,
                descriptor,
                "register block field",
            )?;
        }
    }
    Ok(())
}

fn resolve_type_def(
    model: &RegseqModel,
    declared: &DeclaredIds,
    t: &TypeDef,
) -> Result<(), VerifierError> {
    let context = t.id.decl_id();
    match &t.kind {
        TypeDefKind::Struct { fields } => {
            for field in fields {
                visit_type(model, declared, context, &field.ty, "struct field")?;
            }
        }
        TypeDefKind::Enum { variants } => {
            for variant in variants {
                for field in &variant.fields {
                    visit_type(model, declared, context, &field.ty, "enum variant field")?;
                }
            }
        }
    }
    Ok(())
}

fn resolve_driver(
    model: &RegseqModel,
    declared: &DeclaredIds,
    driver: &DriverStruct,
) -> Result<(), VerifierError> {
    let driver_context = driver.type_id.decl_id();
    check_type_def_id(
        model,
        declared,
        driver_context,
        driver.type_id,
        "driver struct",
    )?;

    for r in &driver.mmio {
        check_register_block_id(
            model,
            declared,
            r.id.decl_id(),
            r.decl.register_block,
            "mmio resource",
        )?;
    }
    for r in &driver.sync {
        resolve_sync(model, declared, r)?;
    }
    for u in &driver.user_data {
        visit_type(
            model,
            declared,
            u.id.decl_id(),
            &u.inner_type,
            "user data field",
        )?;
    }
    Ok(())
}

fn resolve_sync(
    model: &RegseqModel,
    declared: &DeclaredIds,
    sync: &SyncResource,
) -> Result<(), VerifierError> {
    if let Some(ty) = &sync.payload_type {
        visit_type(model, declared, sync.id.decl_id(), ty, "sync payload type")?;
    }
    Ok(())
}

// --- Traversal helpers ----------------------------------------------

fn visit_type(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    ty: &RegseqType,
    site: &'static str,
) -> Result<(), VerifierError> {
    match ty {
        RegseqType::Scalar(_) | RegseqType::Bool | RegseqType::Unit | RegseqType::Never => Ok(()),
        RegseqType::Option(inner) | RegseqType::Slice(inner) => {
            visit_type(model, declared, context, inner, site)
        }
        RegseqType::Result(ok, err) => {
            visit_type(model, declared, context, ok, site)?;
            visit_type(model, declared, context, err, site)
        }
        RegseqType::Array(inner, _) | RegseqType::Ref(inner, _) => {
            visit_type(model, declared, context, inner, site)
        }
        RegseqType::Defined(target) => check_type_def_id(model, declared, context, *target, site),
    }
}

fn visit_statement_kind(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    kind: &StatementKind,
) -> Result<(), VerifierError> {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            visit_place(model, declared, context, place)?;
            visit_rvalue(model, declared, context, rvalue)
        }
        StatementKind::SetDiscriminant { place, .. } => {
            visit_place(model, declared, context, place)
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => Ok(()),
        StatementKind::MirIntrinsic(intrinsic) => {
            visit_mir_intrinsic(model, declared, context, intrinsic)
        }
    }
}

fn visit_mir_intrinsic(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    intrinsic: &MirIntrinsic,
) -> Result<(), VerifierError> {
    match intrinsic {
        MirIntrinsic::Assume(op) => visit_operand(model, declared, context, op),
        MirIntrinsic::CopyNonOverlapping { src, dst, count } => {
            visit_operand(model, declared, context, src)?;
            visit_operand(model, declared, context, dst)?;
            visit_operand(model, declared, context, count)
        }
    }
}

fn visit_terminator_kind(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    kind: &TerminatorKind,
) -> Result<(), VerifierError> {
    match kind {
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => Ok(()),
        TerminatorKind::SwitchInt { discriminant, .. } => {
            visit_operand(model, declared, context, discriminant)
        }
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            check_function_id(model, declared, context, *callee, "call terminator")?;
            for arg in args {
                visit_operand(model, declared, context, arg)?;
            }
            visit_place(model, declared, context, destination)
        }
        TerminatorKind::IntrinsicCall {
            args,
            metadata,
            destination,
            ..
        } => {
            for arg in args {
                visit_operand(model, declared, context, arg)?;
            }
            visit_place(model, declared, context, destination)?;
            if let Some(meta) = metadata {
                visit_intrinsic_metadata(model, declared, context, meta)?;
            }
            Ok(())
        }
        TerminatorKind::Drop { place, .. } => visit_place(model, declared, context, place),
    }
}

fn visit_intrinsic_metadata(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    metadata: &IntrinsicMetadata,
) -> Result<(), VerifierError> {
    match metadata {
        IntrinsicMetadata::Register { register_block, .. } => check_register_block_id(
            model,
            declared,
            context,
            *register_block,
            "intrinsic register metadata",
        ),
        // Non-register metadata variants do not carry DeclIds.
        _ => Ok(()),
    }
}

fn visit_place(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    place: &Place,
) -> Result<(), VerifierError> {
    for proj in &place.projection {
        if let Projection::Field { ty, .. } = proj {
            visit_type(model, declared, context, ty, "place projection field")?;
        }
    }
    Ok(())
}

fn visit_operand(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    operand: &Operand,
) -> Result<(), VerifierError> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => visit_place(model, declared, context, place),
        // `Operand::Constant` points at a ConstantId; there is no
        // `UnresolvedConstantId` error variant in this task's error
        // vocabulary, so we do not surface a cross-item error for
        // dangling constant references here. Task 56/57 can revisit.
        Operand::Constant(_) | Operand::Literal(_) => Ok(()),
    }
}

fn visit_rvalue(
    model: &RegseqModel,
    declared: &DeclaredIds,
    context: DeclId,
    rvalue: &Rvalue,
) -> Result<(), VerifierError> {
    match rvalue {
        Rvalue::Use(op) => visit_operand(model, declared, context, op),
        Rvalue::Repeat(op, _) => visit_operand(model, declared, context, op),
        Rvalue::Ref(_, place) => visit_place(model, declared, context, place),
        Rvalue::Cast(_, op, ty) => {
            visit_operand(model, declared, context, op)?;
            visit_type(model, declared, context, ty, "cast target type")
        }
        Rvalue::BinaryOp(_, lhs, rhs) => {
            visit_operand(model, declared, context, lhs)?;
            visit_operand(model, declared, context, rhs)
        }
        Rvalue::UnaryOp(_, op) => visit_operand(model, declared, context, op),
        Rvalue::Discriminant(place) => visit_place(model, declared, context, place),
        Rvalue::Aggregate(kind, operands) => {
            if let AggregateKind::Array(ty) = kind {
                visit_type(model, declared, context, ty, "array aggregate element")?;
            }
            for op in operands {
                visit_operand(model, declared, context, op)?;
            }
            Ok(())
        }
    }
}

// --- ID resolution primitives ----------------------------------------

fn check_type_def_id(
    model: &RegseqModel,
    declared: &DeclaredIds,
    referencing: DeclId,
    target: TypeDefId,
    site: &'static str,
) -> Result<(), VerifierError> {
    if declared.has_type_def(target) {
        return Ok(());
    }
    Err(VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::UnresolvedTypeDefId {
            referencing,
            target,
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", referencing.index()),
            lookup_origin(model, referencing),
        ),
        details: format!(
            "{site} at DeclId {} references undeclared TypeDefId {}",
            referencing.index(),
            target.decl_id().index()
        ),
    })
}

fn check_function_id(
    model: &RegseqModel,
    declared: &DeclaredIds,
    referencing: DeclId,
    target: FunctionId,
    site: &'static str,
) -> Result<(), VerifierError> {
    if declared.has_function(target) {
        return Ok(());
    }
    Err(VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::UnresolvedFunctionId {
            referencing,
            target,
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", referencing.index()),
            lookup_origin(model, referencing),
        ),
        details: format!(
            "{site} at DeclId {} references undeclared FunctionId {}",
            referencing.index(),
            target.decl_id().index()
        ),
    })
}

fn check_register_block_id(
    model: &RegseqModel,
    declared: &DeclaredIds,
    referencing: DeclId,
    target: RegisterBlockId,
    site: &'static str,
) -> Result<(), VerifierError> {
    if declared.has_register_block(target) {
        return Ok(());
    }
    Err(VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::UnresolvedRegisterBlockId {
            referencing,
            target,
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", referencing.index()),
            lookup_origin(model, referencing),
        ),
        details: format!(
            "{site} at DeclId {} references undeclared RegisterBlockId {}",
            referencing.index(),
            target.decl_id().index()
        ),
    })
}

fn check_register_descriptor_id(
    model: &RegseqModel,
    declared: &DeclaredIds,
    referencing: DeclId,
    target: RegisterDescriptorId,
    site: &'static str,
) -> Result<(), VerifierError> {
    if declared.has_register_descriptor(target) {
        return Ok(());
    }
    Err(VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::UnresolvedRegisterDescriptorId {
            referencing,
            target,
        },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", referencing.index()),
            lookup_origin(model, referencing),
        ),
        details: format!(
            "{site} at DeclId {} references undeclared RegisterDescriptorId {}",
            referencing.index(),
            target.decl_id().index()
        ),
    })
}

// --- Source-location coverage ---------------------------------------

fn source_location_coverage(
    model: &RegseqModel,
    declared: &DeclaredIds,
) -> Result<(), VerifierError> {
    // `all_decls` is a BTreeSet, so iteration is by DeclId order --
    // deterministic across runs per the task spec requirement.
    //
    // Every DeclId in `all_decls` belongs to a user-authored item
    // (module, function, type definition, resource, register
    // block/descriptor, constant, property, user-data field) surfaced
    // by Phase 5 Pass 1. The spec requires a real `Source` origin for
    // each of these -- `CompilerGenerated` slots are a hard
    // violation, not a fallback.
    for decl_id in &declared.all_decls {
        match model.source_locations.get(decl_id) {
            Some(SourceOrigin::Source(_)) => {}
            Some(SourceOrigin::CompilerGenerated) => {
                return Err(VerifierError {
                    level: VerifierLevel::CrossItem,
                    kind: VerifierErrorKind::MissingSourceLocation { decl_id: *decl_id },
                    provenance: VerifierProvenance::for_item(
                        format!("decl#{}", decl_id.index()),
                        SourceOrigin::CompilerGenerated,
                    ),
                    details: format!(
                        "DeclId {} has a CompilerGenerated source origin; \
                         user-sourced declarations must have a real Source origin",
                        decl_id.index()
                    ),
                });
            }
            None => {
                return Err(VerifierError {
                    level: VerifierLevel::CrossItem,
                    kind: VerifierErrorKind::MissingSourceLocation { decl_id: *decl_id },
                    provenance: VerifierProvenance::for_item(
                        format!("decl#{}", decl_id.index()),
                        SourceOrigin::CompilerGenerated,
                    ),
                    details: format!(
                        "DeclId {} has no entry in RegseqModel::source_locations",
                        decl_id.index()
                    ),
                });
            }
        }
    }
    Ok(())
}

// --- Shared helpers -------------------------------------------------

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
