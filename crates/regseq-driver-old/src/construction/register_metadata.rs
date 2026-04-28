//! Rule 7 register-access metadata for Pass 2 (task regseq-model-51).
//!
//! Build a typed [`RegisterIndex`] during Pass 1 that maps every
//! register marker type's rustc [`DefId`] to a small record carrying
//! the enclosing block's id, byte offset, access width, access kind,
//! and a table of field-constant [`DefId`] -> [`RegisterFieldMeta`]
//! entries. Pass 2 consults the index at every register intrinsic
//! `Call` site to attach
//! [`IntrinsicMetadata::Register`](regseq_model_old::ir::IntrinsicMetadata::Register)
//! without re-parsing HIR or re-walking the module tree.
//!
//! The marker-type -> entry mapping is the single source of truth for
//! Rule 7; there is no separate `OFFSET`-constant fallback at
//! recognition time. Cross-block field misuse and access-kind
//! mismatches are defended through
//! [`ConstructionError::CrossBlockFieldUsage`] and
//! [`ConstructionError::WrongRegisterAccessKind`] respectively.

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_hir::ItemKind;
use rustc_middle::ty::{self, TyCtxt};

use regseq_model_old::identity::{RegisterBlockId, RegisterDescriptorId, SymbolInterner};
use regseq_model_old::ir::{IntrinsicMetadata, RegisterFieldMeta, RegseqIntrinsic};
use regseq_model_old::model::{AccessKind, RegisterBlock, RegisterDescriptor};

use crate::extract::abstraction_types::AbstractionTypeRegistry;

use super::{ConstructionError, ErrorProvenance};

// ---------------------------------------------------------------------------
// RegisterEntry / RegisterIndex
// ---------------------------------------------------------------------------

/// Per-register metadata recovered from a `register_block!` expansion.
///
/// Keyed by the register's marker-type rustc [`DefId`] (the
/// `pub enum Register {}` declared inside each per-register module).
#[derive(Debug, Clone)]
pub struct RegisterEntry {
    /// Enclosing register block's model id.
    pub block: RegisterBlockId,
    /// Byte offset of this register within the block.
    pub offset: u32,
    /// Register access width in bits (8 / 16 / 32).
    pub access_width: u32,
    /// Access kind recorded on the register-block's matching field.
    pub access_kind: AccessKind,
    /// Field constants (both direct `Field<W, R>` constants and
    /// `FIELD` / named-value `FieldValue<W, R>` constants inside
    /// per-field submodules) keyed by their rustc [`DefId`], each
    /// mapped to the same bit-field-level metadata.
    pub fields: HashMap<DefId, RegisterFieldMeta>,
}

/// Pass 1 cross-reference table mapping register marker types to
/// [`RegisterEntry`] records.
#[derive(Debug, Default, Clone)]
pub struct RegisterIndex {
    entries: HashMap<DefId, RegisterEntry>,
}

impl RegisterIndex {
    /// Build the index by walking every local-crate HIR module that
    /// carries at least one `#[rseq::register_block]` struct and
    /// its sibling per-register modules.
    ///
    /// Each non-`Reserved` field of a [`RegisterBlock`] maps to one
    /// [`RegisterEntry`]. Field-level `mask`/`shift`/`name`
    /// metadata is sourced from the matching [`RegisterDescriptor`]'s
    /// [`BitfieldDef`](regseq_model_old::model::BitfieldDef) list --
    /// the single source of truth built by Pass 1's descriptor
    /// extraction. The HIR walk only recovers the field constant
    /// [`DefId`]s that recognition needs to key into that
    /// metadata.
    ///
    /// `register_blocks` pairs each block's authored [`DefId`] with
    /// the built [`RegisterBlock`]. `descriptors` is the full
    /// [`RegisterDescriptor`] list built by Pass 1's descriptor
    /// extraction, indexed by [`RegisterDescriptorId`].
    pub fn build(
        tcx: TyCtxt<'_>,
        register_block_map: &HashMap<DefId, RegisterBlockId>,
        register_blocks: &[(DefId, RegisterBlock)],
        descriptors: &HashMap<RegisterDescriptorId, RegisterDescriptor>,
        interner: &mut SymbolInterner,
    ) -> Self {
        let mut out = RegisterIndex::default();
        for (block_def_id, block) in register_blocks {
            let Some(&block_id) = register_block_map.get(block_def_id) else {
                continue;
            };
            debug_assert_eq!(block.id, block_id);
            let parent_mod = tcx.parent(*block_def_id);
            populate_entries_for_block(tcx, parent_mod, block, descriptors, &mut out, interner);
        }
        out
    }

    /// Insert a pre-built [`RegisterEntry`]. Used internally by
    /// [`Self::build`] and by tests that construct synthetic
    /// specimens.
    pub fn insert(&mut self, marker: DefId, entry: RegisterEntry) {
        self.entries.insert(marker, entry);
    }

    /// Look up the entry for a register marker type's [`DefId`].
    pub fn lookup(&self, marker: DefId) -> Option<&RegisterEntry> {
        self.entries.get(&marker)
    }

    /// Number of registered marker types.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index carries no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Pass 1 population
// ---------------------------------------------------------------------------

/// Scan the HIR items inside `parent_mod` for one per-register
/// submodule per non-`Reserved` field of `block` and populate `out`
/// with the recovered [`RegisterEntry`] records.
fn populate_entries_for_block(
    tcx: TyCtxt<'_>,
    parent_mod: DefId,
    block: &RegisterBlock,
    descriptors: &HashMap<RegisterDescriptorId, RegisterDescriptor>,
    out: &mut RegisterIndex,
    interner: &mut SymbolInterner,
) {
    // Map of per-register module name -> HIR submodule DefId.
    let submodules = collect_child_modules(tcx, parent_mod);
    for field in &block.fields {
        if matches!(field.access_kind, AccessKind::Reserved) {
            continue;
        }
        let field_name = interner.resolve(field.name).to_owned();
        let Some(reg_mod_def_id) = submodules.get(field_name.as_str()).copied() else {
            continue;
        };
        let Some(marker_def_id) = find_register_marker(tcx, reg_mod_def_id) else {
            continue;
        };
        let access_width = block.block_attributes.access_width;

        // Build `bit_field_name -> RegisterFieldMeta` from the
        // descriptor's bitfield list. Every entry here is
        // authoritative: mask/shift/name come from Pass 1's
        // descriptor extraction, which is the single source of
        // truth for bit-field identity. Descriptor-less blocks
        // yield an empty bitfield list; in that case the
        // resulting `RegisterEntry.fields` is also empty and
        // Rule 7's `*Field` metadata resolution falls through to
        // `field: None` for those registers.
        let bitfield_meta: HashMap<String, RegisterFieldMeta> =
            match field.descriptor.and_then(|id| descriptors.get(&id)) {
                Some(desc) => desc
                    .fields
                    .iter()
                    .map(|bf| {
                        let name_str = interner.resolve(bf.name).to_owned();
                        let mask = compute_mask(bf.offset, bf.width);
                        let meta = RegisterFieldMeta {
                            mask,
                            shift: bf.offset,
                            name: bf.name,
                        };
                        (name_str, meta)
                    })
                    .collect(),
                None => HashMap::new(),
            };

        // Walk the register module HIR to discover the constant
        // `DefId`s that downstream recognition will key into the
        // bit-field metadata. Each constant is mapped to a bit-field
        // name based on its declared position (top-level const name
        // or per-field submodule name); the bitfield's metadata then
        // resolves through `bitfield_meta`.
        let mut fields = HashMap::new();
        collect_field_constants(tcx, reg_mod_def_id, &bitfield_meta, &mut fields);

        out.insert(
            marker_def_id,
            RegisterEntry {
                block: block.id,
                offset: field.byte_offset,
                access_width,
                access_kind: field.access_kind,
                fields,
            },
        );
    }
}

/// Compute the bit-mask for a descriptor bitfield given its offset
/// (LSB) and width (bit count).
fn compute_mask(offset: u32, width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        ((1u64 << width) - 1) << offset
    }
}

/// Return a `name -> DefId` map of every direct submodule of
/// `parent_mod`.
fn collect_child_modules(tcx: TyCtxt<'_>, parent_mod: DefId) -> HashMap<String, DefId> {
    let mut out: HashMap<String, DefId> = HashMap::new();
    for item_id in children_of(tcx, parent_mod) {
        let item = tcx.hir_item(item_id);
        if let ItemKind::Mod(ident, _) = &item.kind {
            let name = ident.name.as_str().to_owned();
            out.insert(name, item.owner_id.def_id.to_def_id());
        }
    }
    out
}

fn children_of(tcx: TyCtxt<'_>, module_def_id: DefId) -> Vec<rustc_hir::ItemId> {
    let Some(local) = module_def_id.as_local() else {
        return Vec::new();
    };
    let mod_id = rustc_hir::def_id::LocalModDefId::new_unchecked(local);
    tcx.hir_module_items(mod_id).free_items().collect()
}

/// Find the `Register` marker item inside a per-register module and
/// return its [`DefId`]. The `register_block!` macro emits
/// `pub enum Register {}`; the older `register_bitfields!` macro
/// emits `pub struct Register;`. Both are accepted.
fn find_register_marker(tcx: TyCtxt<'_>, reg_mod: DefId) -> Option<DefId> {
    for item_id in children_of(tcx, reg_mod) {
        let item = tcx.hir_item(item_id);
        match &item.kind {
            ItemKind::Enum(ident, ..) | ItemKind::Struct(ident, ..)
                if ident.name.as_str() == "Register" =>
            {
                return Some(item.owner_id.def_id.to_def_id());
            }
            _ => {}
        }
    }
    None
}

/// Walk a per-register module's HIR items and map every field
/// constant's [`DefId`] to the matching bit-field's
/// [`RegisterFieldMeta`] from the descriptor-backed `bitfield_meta`.
///
/// Two macro patterns are supported:
/// - `register_block!` top-level `pub const <FIELD>: Field<...>` --
///   the constant's name IS the bit-field name.
/// - `register_bitfields!` and `register_block!`-with-named-values
///   per-field submodule containing `FIELD` + `FieldValue` constants --
///   the submodule's name is the bit-field name; every constant
///   inside the submodule inherits the same metadata.
///
/// Compiler-facing constants (`OFFSET`, `REG`) are skipped. Any
/// constant that does not match a known bit-field by name is also
/// skipped -- its `DefId` never appears in the IR as a valid field
/// operand anyway, so there is nothing to attach metadata to.
fn collect_field_constants(
    tcx: TyCtxt<'_>,
    reg_mod: DefId,
    bitfield_meta: &HashMap<String, RegisterFieldMeta>,
    out: &mut HashMap<DefId, RegisterFieldMeta>,
) {
    for item_id in children_of(tcx, reg_mod) {
        let item = tcx.hir_item(item_id);
        match &item.kind {
            ItemKind::Const(ident, _, _, _rhs) => {
                let name_str = ident.name.as_str();
                if name_str == "OFFSET" || name_str == "REG" {
                    continue;
                }
                if let Some(meta) = bitfield_meta.get(name_str) {
                    out.insert(item.owner_id.def_id.to_def_id(), meta.clone());
                }
            }
            ItemKind::Mod(ident, _) => {
                let field_name = ident.name.as_str();
                let Some(meta) = bitfield_meta.get(field_name) else {
                    continue;
                };
                let submod = item.owner_id.def_id.to_def_id();
                // Every `Const` child of the submodule (both the
                // `FIELD` descriptor and every `FieldValue` named
                // value) maps to the same bit-field metadata.
                for sub_id in children_of(tcx, submod) {
                    let sub_item = tcx.hir_item(sub_id);
                    if let ItemKind::Const(..) = &sub_item.kind {
                        out.insert(sub_item.owner_id.def_id.to_def_id(), meta.clone());
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 2 metadata resolution
// ---------------------------------------------------------------------------

/// Outcome of [`resolve_register_metadata`]: the structured metadata
/// to attach to the [`IntrinsicCall`] terminator.
pub(crate) struct RegisterMetadataResolution {
    pub metadata: IntrinsicMetadata,
}

/// Run Rule 7 metadata extraction for a register intrinsic call.
///
/// `intrinsic` is one of [`RegseqIntrinsic::RegisterRead`],
/// [`RegseqIntrinsic::RegisterReadField`], [`RegseqIntrinsic::RegisterWrite`],
/// or [`RegseqIntrinsic::RegisterWriteField`]. `args` is the MIR
/// Call's spanned operand list; the receiver is `args[0]` (type
/// `&Mmio<Block>`) and the register marker type parameter `R` is
/// recovered from the callee's monomorphized generic arguments
/// (`generic_args`).
///
/// The receiver type is verified to carry a `#[rseq::abstraction("mmio")]`
/// marker via [`AbstractionTypeRegistry`] so arbitrary single-type-param
/// ADTs cannot silently pass through as a receiver.
///
/// Returns `Ok(None)` when the callee is not one of the four register
/// intrinsics (caller falls through to the no-metadata path).
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_register_metadata<'tcx>(
    tcx: TyCtxt<'tcx>,
    intrinsic: RegseqIntrinsic,
    callee_def_id: DefId,
    generic_args: ty::GenericArgsRef<'tcx>,
    args: &[rustc_span::source_map::Spanned<rustc_middle::mir::Operand<'tcx>>],
    locals: &super::mir_mapping::LocalIdMap<'_, 'tcx>,
    register_index: &RegisterIndex,
    register_block_map: &HashMap<DefId, RegisterBlockId>,
    abstraction_registry: &AbstractionTypeRegistry,
    span: rustc_span::Span,
) -> Result<Option<RegisterMetadataResolution>, ConstructionError> {
    if !is_register_intrinsic(intrinsic) {
        return Ok(None);
    }

    let provenance = || ErrorProvenance {
        item_path: tcx.def_path_str(callee_def_id),
        span,
    };

    // 1. Resolve block identity from `args[0]`'s receiver type
    //    (`&Mmio<Block>`). The outer ADT must be registered as an
    //    `mmio` transport abstraction; only then is its single type
    //    parameter taken as the block type.
    let Some(first_arg) = args.first() else {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance(),
            rendered: "register intrinsic called with zero args".to_string(),
        });
    };
    let receiver_ty = first_arg.node.ty(locals.local_decls(), tcx);
    let Some(block_adt) = extract_mmio_block_adt(receiver_ty, abstraction_registry) else {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance(),
            rendered: format!(
                "register intrinsic receiver is not `&Mmio<Block>` (or the outer ADT is not registered as `#[rseq::abstraction(\"mmio\")]`): {}",
                receiver_ty,
            ),
        });
    };
    let Some(&block_id) = register_block_map.get(&block_adt) else {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance(),
            rendered: format!(
                "register intrinsic receiver block `{}` is not in the Pass 1 register-block map",
                tcx.def_path_str(block_adt),
            ),
        });
    };

    // 2. Resolve register identity from the intrinsic's `R` type
    //    parameter.  `Mmio<Block>::read<W, R>(&self, _reg:
    //    RegisterId<W, R>)` has generic args `[Block, W, R]` in
    //    that order (impl substs first, then fn substs). We walk
    //    `generic_args` to find an ADT substitution that lives
    //    inside one of the register block's sibling modules.
    let Some(marker_def_id) = find_marker_in_generic_args(generic_args, register_index) else {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance(),
            rendered: format!(
                "register intrinsic `{}` has no register-marker type argument",
                intrinsic.canonical_name(),
            ),
        });
    };
    let entry = register_index.lookup(marker_def_id).ok_or_else(|| {
        ConstructionError::UnsupportedConstShape {
            provenance: provenance(),
            rendered: format!(
                "register marker `{}` is not in the Pass 1 register index",
                tcx.def_path_str(marker_def_id),
            ),
        }
    })?;

    // 3. Cross-block sanity check.
    if entry.block != block_id {
        return Err(ConstructionError::CrossBlockFieldUsage {
            provenance: provenance(),
            receiver_block: block_id,
            register_block: entry.block,
            marker: tcx.def_path_str(marker_def_id),
        });
    }

    // 4. Access-kind validation.
    if !access_kind_permits(entry.access_kind, intrinsic) {
        return Err(ConstructionError::WrongRegisterAccessKind {
            provenance: provenance(),
            marker: tcx.def_path_str(marker_def_id),
            access_kind: entry.access_kind,
            intrinsic,
        });
    }

    // 5. Field identity, if applicable.
    let field = match intrinsic {
        RegseqIntrinsic::RegisterReadField | RegseqIntrinsic::RegisterWriteField => {
            extract_field_meta(args, entry)
        }
        _ => None,
    };

    Ok(Some(RegisterMetadataResolution {
        metadata: IntrinsicMetadata::Register {
            register_block: entry.block,
            register_offset: entry.offset,
            access_width: entry.access_width,
            field,
        },
    }))
}

/// Whether `intrinsic` is one of the four register-access intrinsics.
pub(crate) fn is_register_intrinsic(intrinsic: RegseqIntrinsic) -> bool {
    matches!(
        intrinsic,
        RegseqIntrinsic::RegisterRead
            | RegseqIntrinsic::RegisterReadField
            | RegseqIntrinsic::RegisterWrite
            | RegseqIntrinsic::RegisterWriteField,
    )
}

/// Destructure a `&Mmio<Block>` (or `Mmio<Block>`) receiver type and
/// return `Block`'s ADT [`DefId`]. The outer ADT must carry a
/// `#[rseq::abstraction("mmio")]` registration; any other ADT is
/// rejected so the receiver check does not accept arbitrary
/// single-type-parameter generics as a substitute for `Mmio<Block>`.
fn extract_mmio_block_adt<'tcx>(
    ty: ty::Ty<'tcx>,
    abstraction_registry: &AbstractionTypeRegistry,
) -> Option<DefId> {
    let inner = match ty.kind() {
        ty::TyKind::Ref(_, inner, _) => *inner,
        _ => ty,
    };
    let ty::TyKind::Adt(adt_def, args) = inner.kind() else {
        return None;
    };
    let canonical = abstraction_registry.lookup(adt_def.did())?;
    if canonical != "mmio" {
        return None;
    }
    // `Mmio<T>` has exactly one type argument -- `T`. Walk `args`
    // for the first type substitution and resolve its ADT.
    for arg in args.iter() {
        if let ty::GenericArgKind::Type(inner_ty) = arg.kind() {
            if let ty::TyKind::Adt(inner_adt, _) = inner_ty.kind() {
                return Some(inner_adt.did());
            }
        }
    }
    None
}

/// Walk the callee's monomorphized generic args looking for a type
/// substitution whose ADT [`DefId`] is a registered register marker.
fn find_marker_in_generic_args<'tcx>(
    args: ty::GenericArgsRef<'tcx>,
    index: &RegisterIndex,
) -> Option<DefId> {
    for arg in args.iter() {
        if let ty::GenericArgKind::Type(ty) = arg.kind() {
            if let ty::TyKind::Adt(adt_def, _) = ty.kind() {
                let did = adt_def.did();
                if index.lookup(did).is_some() {
                    return Some(did);
                }
            }
        }
    }
    None
}

/// Extract [`RegisterFieldMeta`] from the field-argument operand of
/// a `RegisterReadField` / `RegisterWriteField` call.
///
/// Accepts both of the constant shapes rustc produces at
/// `mir_drops_elaborated_and_const_checked` with `mir_opt_level = 0`:
///
/// - [`mir::Const::Unevaluated`] -- the typical form for a
///   named-constant reference (`cr::UARTEN::ENABLED`). Its
///   `UnevaluatedConst.def` is the constant's [`DefId`].
/// - [`mir::Const::Val`] -- the already-evaluated form for some
///   inline constant expressions; when the type is an `FnDef`-free
///   ADT referring to a named constant, it carries the constant's
///   [`DefId`] via the surrounding `ConstOperand.user_ty` chain.
///   The conservative fallback here returns [`None`] for this
///   shape.
///
/// Non-constant operands (composed `FieldValue::add` chains on the
/// write path, or locally-built values on either path) return
/// [`None`]; the caller records the call with `field: None` because
/// no single bit-field identity is recoverable from a composite
/// value.
fn extract_field_meta<'tcx>(
    args: &[rustc_span::source_map::Spanned<rustc_middle::mir::Operand<'tcx>>],
    entry: &RegisterEntry,
) -> Option<RegisterFieldMeta> {
    // The field operand is the last non-receiver arg. For
    // `mmio_read_field(self, field)` and `mmio_write_field(self,
    // value)` that is `args[1]`.
    let op = args.get(1)?;
    let rustc_middle::mir::Operand::Constant(const_op) = &op.node else {
        return None;
    };
    match const_op.const_ {
        rustc_middle::mir::Const::Unevaluated(unevaluated, _ty) => {
            entry.fields.get(&unevaluated.def).cloned()
        }
        rustc_middle::mir::Const::Ty(_ty, ty_const) => {
            // `Const::Ty` wraps a `ty::Const` whose `kind` may be
            // [`ty::ConstKind::Unevaluated`] for a named constant
            // reference. Pull the `DefId` out when it is; otherwise
            // fall through to `None`.
            let kind = ty_const.kind();
            if let rustc_middle::ty::ConstKind::Unevaluated(ty_unevaluated) = kind {
                return entry.fields.get(&ty_unevaluated.def).cloned();
            }
            None
        }
        rustc_middle::mir::Const::Val(..) => None,
    }
}

/// Whether a register with the given `access_kind` permits the
/// operation encoded by `intrinsic`. `Reserved` fields are never
/// accessible and therefore always reject; in practice Rule 7 never
/// emits a `Reserved`-kind entry into the [`RegisterIndex`].
fn access_kind_permits(kind: AccessKind, intrinsic: RegseqIntrinsic) -> bool {
    match intrinsic {
        RegseqIntrinsic::RegisterRead | RegseqIntrinsic::RegisterReadField => {
            matches!(kind, AccessKind::ReadOnly | AccessKind::ReadWrite)
        }
        RegseqIntrinsic::RegisterWrite | RegseqIntrinsic::RegisterWriteField => {
            matches!(kind, AccessKind::WriteOnly | AccessKind::ReadWrite)
        }
        _ => true,
    }
}
