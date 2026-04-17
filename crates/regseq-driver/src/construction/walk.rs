//! Pass 1 HIR traversal: walks the local crate's module tree and
//! populates a [`RegseqModel`] root module (and all submodules) with
//! declaration-level items.
//!
//! Every ID allocation goes through [`DeclIdAllocator`]; every name goes
//! through [`SymbolInterner`]; every source span is resolved by
//! [`record_span`]. Pass 1 does not touch function bodies -- they are
//! left as default empty placeholders.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rustc_hir::def_id::{DefId, LocalModDefId, CRATE_DEF_ID};
use rustc_hir::ItemKind;
use rustc_middle::ty::TyCtxt;

use regseq_model::identity::{
    ConstantId, DeclId, FunctionId, ModuleId, RegisterBlockId, RegisterDescriptorId, SourceOrigin,
    Symbol, SymbolInterner, TypeDefId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, RegseqType,
    ScalarType, Terminator, TerminatorKind,
};
use regseq_model::model::{
    Attrs, Constant, FieldDef, Function, Module, ModuleKind, Param, ParamKind, RegisterDescriptor,
    TypeDef, TypeDefClassification, TypeDefKind, VariantDef,
};

use super::id_allocator::DeclIdAllocator;
use super::{ConstructionError, ErrorProvenance, ExtractionRegistries};
use crate::construction::record_span;

/// Traversal-wide context. Held as a single struct so the many helpers
/// don't have to pass the same 5 parameters everywhere.
pub struct BuildContext<'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub registries: &'a ExtractionRegistries<'a>,
    pub symbols: &'a mut SymbolInterner,
    pub source_locations: &'a mut BTreeMap<DeclId, SourceOrigin>,
    pub allocator: &'a mut DeclIdAllocator,
    pub driver_modules: Vec<ErrorProvenance>,
    /// Crate-wide `DefId -> TypeDefId` map built as type definitions are
    /// assigned. Cross-references (e.g., `Mmio<T>` → register block)
    /// query this map rather than re-walking modules.
    pub type_def_map: HashMap<DefId, TypeDefId>,
    /// Crate-wide `DefId -> RegisterBlockId` map, populated when each
    /// `#[rseq::register_block]` struct is encountered.
    pub register_block_map: HashMap<DefId, RegisterBlockId>,
}

impl<'a, 'tcx> BuildContext<'a, 'tcx> {
    fn alloc_and_record(&mut self, def_id: DefId) -> DeclId {
        let id = self.allocator.alloc();
        record_span(
            self.tcx,
            self.symbols,
            self.source_locations,
            id,
            self.tcx.def_span(def_id),
        );
        id
    }

    fn intern(&mut self, s: &str) -> Symbol {
        self.symbols.intern(s)
    }
}

/// Walk the module rooted at `module_def_id` and populate `target`.
pub fn populate_module_items_and_submodules(
    cx: &mut BuildContext<'_, '_>,
    target: &mut Module,
    module_def_id: DefId,
) -> Result<(), ConstructionError> {
    // Determine if this is a crate-root or submodule walk. Either way we
    // enumerate free items whose parent module is `module_def_id`.
    let items: Vec<rustc_hir::ItemId> = collect_items_in_module(cx.tcx, module_def_id);

    // --- driver discovery + module-kind determination ---
    for item_id in &items {
        let item = cx.tcx.hir_item(*item_id);
        let def_id = item.owner_id.def_id.to_def_id();
        if let ItemKind::Struct(..) = &item.kind {
            if has_rseq_attr(cx.tcx, def_id, "driver") {
                target.kind = ModuleKind::Driver;
                cx.driver_modules
                    .push(ErrorProvenance::for_def_id(cx.tcx, def_id));
            }
        }
    }

    // --- item population ---
    // Submodule population requires a second pass because modules are
    // items themselves.
    let mut submodule_defs: Vec<DefId> = Vec::new();
    let mut impls_by_type: HashMap<DefId, Vec<DefId>> = HashMap::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for item_id in &items {
        let item = cx.tcx.hir_item(*item_id);
        let def_id = item.owner_id.def_id.to_def_id();

        match &item.kind {
            ItemKind::Mod(_ident, _module) => {
                submodule_defs.push(def_id);
            }
            ItemKind::Struct(ident, _generics, variant_data) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;

                if has_rseq_attr(cx.tcx, def_id, "driver") {
                    // Driver struct: build a DriverStruct AND a TypeDef
                    // in the same module.
                    let type_def = super::driver::build_driver_struct(
                        cx,
                        target,
                        def_id,
                        ident.name.as_str(),
                        variant_data,
                    )?;
                    cx.type_def_map.insert(def_id, type_def.id);
                    target.type_defs.push(type_def);
                } else if has_rseq_attr(cx.tcx, def_id, "register_block") {
                    let block = super::registers::build_register_block(
                        cx,
                        def_id,
                        ident.name.as_str(),
                        variant_data,
                    )?;
                    cx.register_block_map.insert(def_id, block.id);
                    target.register_blocks.push(block);
                } else {
                    let type_def =
                        build_type_def_for_struct(cx, def_id, ident.name.as_str(), variant_data);
                    cx.type_def_map.insert(def_id, type_def.id);
                    target.type_defs.push(type_def);
                }
            }
            ItemKind::Enum(ident, _generics, enum_def) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let type_def = build_type_def_for_enum(cx, def_id, name_str, enum_def);
                cx.type_def_map.insert(def_id, type_def.id);
                target.type_defs.push(type_def);
            }
            ItemKind::Const(ident, _generics, _ty, _body_id) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let constant = build_constant(cx, def_id, name_str);
                target.constants.push(constant);
            }
            ItemKind::Static(_mutbl, ident, _ty, _body_id) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let constant = build_constant(cx, def_id, name_str);
                target.constants.push(constant);
            }
            ItemKind::Fn { ident, .. } => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let func = build_function_stub(cx, def_id, name_str);
                target.free_functions.push(func);
            }
            ItemKind::Impl(_impl_block) => {
                // Collect impl; process after the type-def sweep has seen
                // everything so target_type resolution is possible.
                if let Some(target_def) = impl_self_def_id(cx.tcx, def_id) {
                    impls_by_type.entry(target_def).or_default().push(def_id);
                }
            }
            _ => {}
        }
    }

    // --- impl block processing (deferred until type defs are assigned) ---
    // Use the crate-wide `DefId -> TypeDefId` map that tracks every
    // type assigned so far. This lets `impl Foo` blocks resolve whether
    // `Foo` is declared in the same module, a sibling module, or a
    // submodule already processed earlier in the walk.
    for (target_def, impls) in &impls_by_type {
        for &impl_def in impls {
            let impl_block_info =
                super::impls::build_impl_block(cx, impl_def, *target_def, &cx.type_def_map.clone());
            match impl_block_info {
                super::impls::ImplBlockAttachment::NonDriverInherent(block)
                | super::impls::ImplBlockAttachment::NonDriverTrait(block) => {
                    target.impl_blocks.push(block);
                }
                super::impls::ImplBlockAttachment::DriverInherent(block) => {
                    target.driver_impl_blocks.push(block);
                }
                super::impls::ImplBlockAttachment::DriverTrait(block) => {
                    target.trait_impls.push(block);
                }
                super::impls::ImplBlockAttachment::Skipped => {}
            }
        }
    }

    // --- dependency sets ---
    // Backend-focused abstractions/intrinsics accounting runs later in
    // Pass 2 (body analysis). Pass 1 leaves the sets empty.

    // --- submodules ---
    for sub_def_id in submodule_defs {
        let mut submodule = empty_module_for(cx, sub_def_id);
        populate_module_items_and_submodules(cx, &mut submodule, sub_def_id)?;
        target.submodules.push(submodule);
    }

    // --- register descriptor synthesis ---
    // Recognize `register_bitfields!`-style descriptor modules at this
    // scope: any submodule named like `<UPPER>_REG` containing a
    // `Register` struct becomes a descriptor on *this* module.
    synthesize_register_descriptors_from_submodules(cx, target);

    Ok(())
}

fn collect_items_in_module(tcx: TyCtxt<'_>, module_def_id: DefId) -> Vec<rustc_hir::ItemId> {
    let local = module_def_id.as_local().unwrap_or(CRATE_DEF_ID);
    let mod_id = LocalModDefId::new_unchecked(local);
    let mut items: Vec<rustc_hir::ItemId> = Vec::new();
    let module_items = tcx.hir_module_items(mod_id);
    for item_id in module_items.free_items() {
        items.push(item_id);
    }
    items
}

fn check_duplicate(
    tcx: TyCtxt<'_>,
    module_def_id: DefId,
    seen: &mut std::collections::HashSet<String>,
    name: &str,
) -> Result<(), ConstructionError> {
    if !seen.insert(name.to_owned()) {
        return Err(ConstructionError::DuplicateItemName {
            module: ErrorProvenance::for_def_id(tcx, module_def_id),
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, name: &str) -> bool {
    def_has_rseq_attr(tcx, def_id, name)
}

/// Public-to-submodules helper: does `def_id` carry `#[rseq::<name>]`?
pub(crate) fn def_has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, name: &str) -> bool {
    for attr in tcx.get_all_attrs(def_id) {
        if let rustc_hir::Attribute::Unparsed(item) = attr {
            let segs = &item.path.segments;
            if segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == name {
                return true;
            }
        }
    }
    false
}

/// Public-to-submodules helper: render a HIR type as a Regseq type.
pub(crate) fn render_ty_for_field(tcx: TyCtxt<'_>, ty: &rustc_hir::Ty<'_>) -> RegseqType {
    render_ty(tcx, ty)
}

/// Public-to-submodules helper: render a HIR type as a human-readable
/// string for diagnostic payloads.
pub(crate) fn rendered_ty_string(ty: &rustc_hir::Ty<'_>) -> String {
    match &ty.kind {
        rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) => path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str().to_owned())
            .collect::<Vec<_>>()
            .join("::"),
        rustc_hir::TyKind::Tup([]) => "()".to_owned(),
        rustc_hir::TyKind::Array(_, _) => "[..]".to_owned(),
        _ => "<unknown>".to_owned(),
    }
}

/// Public-to-submodules helper: build a Constant for an associated
/// const declaration.
pub(crate) fn build_associated_constant(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
) -> regseq_model::model::Constant {
    super::walk::build_constant(cx, def_id, name)
}

fn empty_module_for(cx: &mut BuildContext<'_, '_>, def_id: DefId) -> Module {
    let id = ModuleId::from(cx.alloc_and_record(def_id));
    let name = cx
        .tcx
        .opt_item_name(def_id)
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| format!("mod_{}", id.decl_id().index()));
    let name_sym = cx.symbols.intern(&name);
    Module {
        id,
        name: name_sym,
        kind: ModuleKind::Normal,
        submodules: Vec::new(),
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: Vec::new(),
        impl_blocks: Vec::new(),
        driver: None,
        trait_impls: Vec::new(),
        driver_impl_blocks: Vec::new(),
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    }
}

fn synthesize_register_descriptors_from_submodules(
    cx: &mut BuildContext<'_, '_>,
    target: &mut Module,
) {
    // Look through submodules for `*_REG`-named modules containing a
    // top-level `struct Register`. Emit an empty-fields descriptor and
    // record the submodule's own source span (task 51 refines the
    // field/named_value extraction).
    let mut descriptors: Vec<RegisterDescriptor> = Vec::new();
    for sub in &target.submodules {
        let name_str = cx.symbols.resolve(sub.name).to_owned();
        if !name_str.ends_with("_REG") {
            continue;
        }
        let has_register = sub
            .type_defs
            .iter()
            .any(|td| cx.symbols.resolve(td.name) == "Register");
        if !has_register {
            continue;
        }
        let id = RegisterDescriptorId::from(cx.allocator.alloc());
        // Reuse the submodule's source location so Pass 1 never emits
        // `CompilerGenerated` entries in the model (spec obligation 8).
        let sub_origin = cx
            .source_locations
            .get(&sub.id.decl_id())
            .cloned()
            .unwrap_or(SourceOrigin::CompilerGenerated);
        cx.source_locations.insert(id.decl_id(), sub_origin);
        let desc_name = cx.symbols.intern(&name_str);
        descriptors.push(RegisterDescriptor {
            id,
            name: desc_name,
            access_width: 32,
            fields: Vec::new(),
        });
    }
    target.register_descriptors.extend(descriptors);
}

fn impl_self_def_id(tcx: TyCtxt<'_>, impl_def: DefId) -> Option<DefId> {
    let self_ty = tcx.type_of(impl_def).instantiate_identity();
    self_ty.ty_adt_def().map(|adt| adt.did())
}

// ---------------------------------------------------------------------------
// Item builders (simple cases)
// ---------------------------------------------------------------------------

fn build_type_def_for_struct(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
    variant_data: &rustc_hir::VariantData<'_>,
) -> TypeDef {
    let id = TypeDefId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let classification = classify(cx, def_id);
    let attrs = collect_rseq_attrs(cx.tcx, def_id);

    let fields = variant_data
        .fields()
        .iter()
        .map(|f| FieldDef {
            name: cx.symbols.intern(f.ident.name.as_str()),
            ty: render_ty(cx.tcx, f.ty),
        })
        .collect();

    TypeDef {
        id,
        name: name_sym,
        kind: TypeDefKind::Struct { fields },
        classification,
        attrs,
    }
}

fn build_type_def_for_enum(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
    enum_def: &rustc_hir::EnumDef<'_>,
) -> TypeDef {
    let id = TypeDefId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let classification = classify(cx, def_id);
    let attrs = collect_rseq_attrs(cx.tcx, def_id);

    let variants = enum_def
        .variants
        .iter()
        .map(|v| {
            let vname = cx.symbols.intern(v.ident.name.as_str());
            let fields = v
                .data
                .fields()
                .iter()
                .map(|f| FieldDef {
                    name: cx.symbols.intern(f.ident.name.as_str()),
                    ty: render_ty(cx.tcx, f.ty),
                })
                .collect();
            VariantDef {
                name: vname,
                fields,
                discriminant: None,
            }
        })
        .collect();

    TypeDef {
        id,
        name: name_sym,
        kind: TypeDefKind::Enum { variants },
        classification,
        attrs,
    }
}

/// Collect every `#[rseq::<name>(...)]` attribute on `def_id` and emit
/// a corresponding [`AttrEntry`] in the returned [`Attrs`] bag. Complex
/// key/value payloads are rendered as a `Unit` marker for Pass 1; task
/// 52 refines this into structured values.
fn collect_rseq_attrs(tcx: TyCtxt<'_>, def_id: DefId) -> Attrs {
    use regseq_model::model::{AttrEntry, AttrValue};
    let mut entries = Vec::new();
    for attr in tcx.get_all_attrs(def_id) {
        if let rustc_hir::Attribute::Unparsed(item) = attr {
            let segs = &item.path.segments;
            if segs.len() == 2 && segs[0].as_str() == "rseq" {
                entries.push(AttrEntry {
                    key: format!("rseq::{}", segs[1].as_str()),
                    value: AttrValue::Unit,
                });
            }
        }
    }
    Attrs { entries }
}

fn classify(cx: &BuildContext<'_, '_>, def_id: DefId) -> TypeDefClassification {
    if let Some(name) = cx.registries.abstraction_types.lookup(def_id) {
        if let Some(ab) = abstraction_from_canonical(name) {
            return TypeDefClassification::Abstraction(ab);
        }
    }
    if let Some(name) = cx.registries.framework_types.lookup(def_id) {
        if let Some(ft) = framework_from_canonical(name) {
            return TypeDefClassification::Framework(ft);
        }
    }
    TypeDefClassification::UserDefined
}

fn abstraction_from_canonical(s: &str) -> Option<regseq_model::ir::RegseqAbstraction> {
    use regseq_model::ir::RegseqAbstraction as A;
    Some(match s {
        "clock" => A::Clock,
        "reset" => A::Reset,
        "pinctrl" | "pin_ctrl" => A::PinCtrl,
        "interrupt" => A::Interrupt,
        "mmio" => A::Mmio,
        "mutex" => A::Mutex,
        "spin_lock" | "spinlock" => A::SpinLock,
        "irq_spin_lock" | "irq_spinlock" => A::IrqSpinLock,
        "completion" => A::Completion,
        _ => return None,
    })
}

fn framework_from_canonical(s: &str) -> Option<regseq_model::model::FrameworkTypeKind> {
    use regseq_model::model::FrameworkTypeKind as F;
    Some(match s {
        "regseq-error" | "regseq_error" => F::RegseqError,
        "serial-errors" | "serial_errors" => F::SerialErrors,
        "rx-char" | "rx_char" => F::RxChar,
        "serial-config" | "serial_config" => F::SerialConfig,
        "data-bits" | "data_bits" => F::DataBits,
        "parity" => F::Parity,
        "stop-bits" | "stop_bits" => F::StopBits,
        "flow-control" | "flow_control" => F::FlowControl,
        "duration" => F::Duration,
        "irq-result" | "irq_result" => F::IrqResult,
        _ => return None,
    })
}

pub(crate) fn build_constant(cx: &mut BuildContext<'_, '_>, def_id: DefId, name: &str) -> Constant {
    let id = ConstantId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let ty = ty_to_regseq(cx.tcx.type_of(def_id).instantiate_identity())
        .unwrap_or(RegseqType::Scalar(ScalarType::U32));
    let value = eval_const(cx.tcx, def_id).unwrap_or_else(|| fallback_literal(&ty));
    Constant {
        id,
        name: name_sym,
        ty,
        value,
    }
}

fn eval_const(tcx: TyCtxt<'_>, def_id: DefId) -> Option<LiteralValue> {
    let value = tcx.const_eval_poly(def_id).ok()?;
    let scalar = value.try_to_scalar()?;
    let bits = scalar.to_bits(scalar.size()).discard_err()?;
    let ty = tcx.type_of(def_id).instantiate_identity();
    Some(scalar_to_literal(ty, bits))
}

fn scalar_to_literal(ty: rustc_middle::ty::Ty<'_>, bits: u128) -> LiteralValue {
    use rustc_middle::ty::TyKind;
    match ty.kind() {
        TyKind::Uint(uint) => match uint {
            rustc_middle::ty::UintTy::U8 => LiteralValue::U8(bits as u8),
            rustc_middle::ty::UintTy::U16 => LiteralValue::U16(bits as u16),
            rustc_middle::ty::UintTy::U32 => LiteralValue::U32(bits as u32),
            rustc_middle::ty::UintTy::U64 => LiteralValue::U64(bits as u64),
            rustc_middle::ty::UintTy::Usize => LiteralValue::Usize(bits as u64),
            rustc_middle::ty::UintTy::U128 => LiteralValue::U64(bits as u64),
        },
        TyKind::Int(int) => match int {
            rustc_middle::ty::IntTy::I8 => LiteralValue::I8(bits as i8),
            rustc_middle::ty::IntTy::I16 => LiteralValue::I16(bits as i16),
            rustc_middle::ty::IntTy::I32 => LiteralValue::I32(bits as i32),
            rustc_middle::ty::IntTy::I64 => LiteralValue::I64(bits as i64),
            rustc_middle::ty::IntTy::Isize => LiteralValue::Isize(bits as i64),
            rustc_middle::ty::IntTy::I128 => LiteralValue::I64(bits as i64),
        },
        TyKind::Bool => LiteralValue::Bool(bits != 0),
        _ => LiteralValue::U32(bits as u32),
    }
}

fn fallback_literal(ty: &RegseqType) -> LiteralValue {
    match ty {
        RegseqType::Bool => LiteralValue::Bool(false),
        RegseqType::Scalar(ScalarType::U8) => LiteralValue::U8(0),
        RegseqType::Scalar(ScalarType::U16) => LiteralValue::U16(0),
        RegseqType::Scalar(ScalarType::U64) => LiteralValue::U64(0),
        RegseqType::Scalar(ScalarType::I32) => LiteralValue::I32(0),
        _ => LiteralValue::U32(0),
    }
}

fn build_function_stub(cx: &mut BuildContext<'_, '_>, def_id: DefId, name: &str) -> Function {
    let id = FunctionId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let (params, return_type) = function_signature(cx, def_id);
    let body = empty_body(cx.tcx, cx.symbols, def_id, params.len());
    Function {
        id,
        name: name_sym,
        params,
        return_type,
        body,
        attrs: Attrs::default(),
    }
}

pub(crate) fn function_signature(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
) -> (Vec<Param>, RegseqType) {
    let sig = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
    let mut params = Vec::new();
    let assoc = cx.tcx.opt_associated_item(def_id);
    let receiver_count = match assoc {
        Some(item) if item.is_method() => 1,
        _ => 0,
    };
    for (i, input) in sig.inputs().iter().enumerate() {
        let kind = if i < receiver_count {
            ParamKind::Receiver
        } else {
            ParamKind::Regular
        };
        let name = if i < receiver_count {
            "self".to_owned()
        } else {
            format!("arg{i}")
        };
        let name_sym = cx.symbols.intern(&name);
        let ty = ty_to_regseq(*input).unwrap_or(RegseqType::Unit);
        params.push(Param {
            kind,
            name: name_sym,
            ty,
        });
    }
    let return_type = ty_to_regseq(sig.output()).unwrap_or(RegseqType::Unit);
    (params, return_type)
}

pub(crate) fn empty_body(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    def_id: DefId,
    arg_count: usize,
) -> Body {
    let span = tcx.def_span(def_id);
    let origin = super::resolve_origin(tcx, symbols, span);
    let mut local_decls = Vec::with_capacity(arg_count + 1);
    local_decls.push(LocalDecl {
        ty: RegseqType::Unit,
        mutable: true,
        name: None,
    });
    for _ in 0..arg_count {
        local_decls.push(LocalDecl {
            ty: RegseqType::Unit,
            mutable: false,
            name: None,
        });
    }
    let locals_source = (0..local_decls.len())
        .map(|_| SourceOrigin::CompilerGenerated)
        .collect();
    Body {
        local_decls,
        arg_count,
        basic_blocks: vec![BasicBlock {
            statements: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
            },
        }],
        source_map: BodySourceMap {
            body_span: origin,
            locals: locals_source,
            blocks: vec![BasicBlockSourceMap {
                statements: Vec::new(),
                terminator: SourceOrigin::CompilerGenerated,
            }],
        },
        ordered_regions: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Type rendering helpers
// ---------------------------------------------------------------------------

pub(crate) fn ty_to_regseq(ty: rustc_middle::ty::Ty<'_>) -> Option<RegseqType> {
    use rustc_middle::ty::TyKind;
    Some(match ty.kind() {
        TyKind::Bool => RegseqType::Bool,
        TyKind::Tuple(list) if list.is_empty() => RegseqType::Unit,
        TyKind::Never => RegseqType::Never,
        TyKind::Uint(u) => RegseqType::Scalar(match u {
            rustc_middle::ty::UintTy::U8 => ScalarType::U8,
            rustc_middle::ty::UintTy::U16 => ScalarType::U16,
            rustc_middle::ty::UintTy::U32 => ScalarType::U32,
            rustc_middle::ty::UintTy::U64 => ScalarType::U64,
            rustc_middle::ty::UintTy::Usize => ScalarType::Usize,
            rustc_middle::ty::UintTy::U128 => ScalarType::U64,
        }),
        TyKind::Int(i) => RegseqType::Scalar(match i {
            rustc_middle::ty::IntTy::I8 => ScalarType::I8,
            rustc_middle::ty::IntTy::I16 => ScalarType::I16,
            rustc_middle::ty::IntTy::I32 => ScalarType::I32,
            rustc_middle::ty::IntTy::I64 => ScalarType::I64,
            rustc_middle::ty::IntTy::Isize => ScalarType::Isize,
            rustc_middle::ty::IntTy::I128 => ScalarType::I64,
        }),
        TyKind::Array(inner, _) => {
            let inner_ty = ty_to_regseq(*inner)?;
            RegseqType::Array(Box::new(inner_ty), 0)
        }
        TyKind::Adt(adt, _) => {
            let name = adt.did().index.as_u32();
            // Without a DefId->TypeDefId map here, fall back to U32 for
            // unknown named types. The driver-field categorizer
            // consults a map explicitly.
            let _ = name;
            return None;
        }
        _ => return None,
    })
}

fn render_ty(tcx: TyCtxt<'_>, ty: &rustc_hir::Ty<'_>) -> RegseqType {
    // Very small HIR-to-RegseqType mapper used for generic struct field
    // types. Unknown shapes become Unit so Pass 1 doesn't fail on
    // esoteric inputs.
    match &ty.kind {
        rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) => {
            if let Some(seg) = path.segments.last() {
                match seg.ident.name.as_str() {
                    "u8" => RegseqType::Scalar(ScalarType::U8),
                    "u16" => RegseqType::Scalar(ScalarType::U16),
                    "u32" => RegseqType::Scalar(ScalarType::U32),
                    "u64" => RegseqType::Scalar(ScalarType::U64),
                    "usize" => RegseqType::Scalar(ScalarType::Usize),
                    "i8" => RegseqType::Scalar(ScalarType::I8),
                    "i16" => RegseqType::Scalar(ScalarType::I16),
                    "i32" => RegseqType::Scalar(ScalarType::I32),
                    "i64" => RegseqType::Scalar(ScalarType::I64),
                    "isize" => RegseqType::Scalar(ScalarType::Isize),
                    "bool" => RegseqType::Bool,
                    _ => RegseqType::Unit,
                }
            } else {
                RegseqType::Unit
            }
        }
        rustc_hir::TyKind::Tup([]) => RegseqType::Unit,
        _ => {
            let _ = tcx;
            RegseqType::Unit
        }
    }
}
