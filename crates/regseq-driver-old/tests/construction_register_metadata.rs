//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Pass 2 Rule 7 register-intrinsic metadata tests (regseq-model-51).
//!
//! These tests drive `build_body` against inline specimens that
//! declare a local `Mmio<T>` transport + per-register modules
//! emulating the `register_block!` expansion, and verify Rule 7
//! attaches the correct `IntrinsicMetadata::Register` payload at
//! every recognized register-intrinsic call. Local specimens are
//! used (rather than the real `regseq` facade) because cross-crate
//! `Mmio` locals are rejected by `map_ty` and because the facade
//! fixture hits an unrelated module-ordering gap in Pass 1
//! (documented in `construction_log_metadata.rs`). The translation
//! path under test is identical to the production one; only the
//! `IntrinsicLookup` source differs -- tests seed it from the
//! specimen's local method `DefId`s via `IntrinsicLookup::from_entries`.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once};

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_driver_old::construction::{
    build_body, build_pass1_output, ConstantIndex, ConstructionError, DerefCollapseLookup,
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, LogArgUnwrap, OrderedMarkers,
    Pass1Output, PinStateLookup, PinStateRegistry, RegisterIndex, TryMarkers, TypeDefIndex,
};
use regseq_driver_old::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver_old::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver_old::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model_old::identity::{RegisterBlockId, SymbolInterner};
use regseq_model_old::ir::{IntrinsicMetadata, RegseqIntrinsic, TerminatorKind};
use regseq_model_old::model::AccessKind;

// ---------------------------------------------------------------------------
// rustc harness (mirror of construction_intrinsic_recognition.rs)
// ---------------------------------------------------------------------------

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn with_specimen<R, F>(source: &str, test_name: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &Pass1Output) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();

    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback_cell: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>> =
        Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let pass1 =
                build_pass1_output(tcx, &registries).expect("Pass 1 must succeed on this specimen");
            let f = self
                .callback
                .lock()
                .unwrap()
                .take()
                .expect("callback taken once");
            let result = f(tcx, &pass1);
            *self.out.lock().unwrap() = Some(result);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks::<R> {
        out: out.clone(),
        callback: callback_cell,
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();

    result
}

// ---------------------------------------------------------------------------
// HIR helpers
// ---------------------------------------------------------------------------

fn find_impl_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let impl_ty = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{impl_ty}") != self_ty {
                continue;
            }
            for ii in impl_block.items {
                let impl_item = tcx.hir_impl_item(*ii);
                if let rustc_hir::ImplItemKind::Fn(_, _) = &impl_item.kind {
                    if impl_item.ident.name.as_str() == method {
                        return impl_item.owner_id.def_id.to_def_id();
                    }
                }
            }
        }
    }
    panic!("method `{self_ty}::{method}` not found in specimen");
}

fn find_free_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Fn { ident, .. } = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id.to_def_id();
            }
        }
    }
    panic!("free function `{name}` not found in specimen");
}

// ---------------------------------------------------------------------------
// Specimen. `regseq::register_block!` is emulated by hand so the
// `pub struct Regs` and sibling register modules land at the SAME
// module level as `#[rseq::driver]` -- Pass 1's per-module walk
// needs register blocks in the same module as any Mmio-resource
// struct (see construction_log_metadata.rs PL011 end-to-end note).
//
// The local `Mmio<T>` struct carries `#[rseq::abstraction("mmio")]`
// to match the production facade's declaration. Intrinsic
// recognition is driven by `IntrinsicLookup::from_entries` against
// the local method `DefId`s (no `#[rseq::intrinsic]` attribute on
// the specimen -- that attribute is reserved for facade items).
// ---------------------------------------------------------------------------

const REGISTER_SPECIMEN: &str = r#"
#![no_std]

use core::marker::PhantomData;
use core::ops::Deref;

pub struct RegisterId<W, R> {
    _w: PhantomData<W>,
    _r: PhantomData<R>,
}

impl<W, R> Copy for RegisterId<W, R> {}
impl<W, R> Clone for RegisterId<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W, R> RegisterId<W, R> {
    pub const fn new() -> Self {
        Self { _w: PhantomData, _r: PhantomData }
    }
}

#[derive(Clone, Copy)]
pub struct Field<W, R> {
    pub mask: W,
    pub shift: W,
    _r: PhantomData<R>,
}

impl<W, R> Field<W, R> {
    pub const fn new(mask: W, shift: W) -> Self {
        Self { mask, shift, _r: PhantomData }
    }
}

#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _r: PhantomData<R>,
}

impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self { mask, value, _r: PhantomData }
    }
}

pub struct ReadOnlyAccess;
pub struct WriteOnlyAccess;
pub struct ReadWriteAccess;
pub trait ReadAccess {}
pub trait WriteAccess {}
impl ReadAccess for ReadOnlyAccess {}
impl ReadAccess for ReadWriteAccess {}
impl WriteAccess for WriteOnlyAccess {}
impl WriteAccess for ReadWriteAccess {}

pub trait RegWidth: Copy {}
impl RegWidth for u32 {}

pub trait RegisterOf<Block> {
    type Access;
}

#[repr(transparent)]
pub struct ReadOnly<T, R> {
    _v: T,
    _r: PhantomData<R>,
}

#[repr(transparent)]
pub struct WriteOnly<T, R> {
    _v: T,
    _r: PhantomData<R>,
}

#[repr(transparent)]
pub struct ReadWrite<T, R> {
    _v: T,
    _r: PhantomData<R>,
}

#[repr(transparent)]
pub struct Reserved<T> {
    _v: T,
}

#[rseq::abstraction("mmio")]
pub struct Mmio<T> {
    inner: T,
}

impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<Block> Mmio<Block> {
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        loop {}
    }

    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(&self, _field: Field<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        loop {}
    }

    pub fn write<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>, _value: W)
    where
        R::Access: WriteAccess,
    {
        loop {}
    }

    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(&self, _values: FieldValue<W, R>)
    where
        R::Access: WriteAccess,
    {
        loop {}
    }

}

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Regs {
    pub dr: ReadWrite<u32, dr::Register>,
    pub fr: ReadOnly<u32, fr::Register>,
    pub lcr_h: ReadWrite<u32, lcr_h::Register>,
    pub icr: WriteOnly<u32, icr::Register>,
}

pub mod dr {
    use super::{Field, ReadWriteAccess, RegisterId, RegisterOf, Regs};
    pub enum Register {}
    pub const OFFSET: u32 = 0;
    pub const REG: RegisterId<u32, Register> = RegisterId::new();
    pub const DATA: Field<u32, Register> = Field::new(0xFF as u32, 0 as u32);
    impl RegisterOf<Regs> for Register {
        type Access = ReadWriteAccess;
    }
}

pub mod fr {
    use super::{Field, ReadOnlyAccess, RegisterId, RegisterOf, Regs};
    pub enum Register {}
    pub const OFFSET: u32 = 4;
    pub const REG: RegisterId<u32, Register> = RegisterId::new();
    pub const TXFF: Field<u32, Register> = Field::new((1u32 << 5) as u32, 5 as u32);
    impl RegisterOf<Regs> for Register {
        type Access = ReadOnlyAccess;
    }
}

pub mod lcr_h {
    use super::{Field, FieldValue, ReadWriteAccess, RegisterId, RegisterOf, Regs};
    pub enum Register {}
    pub const OFFSET: u32 = 8;
    pub const REG: RegisterId<u32, Register> = RegisterId::new();
    pub mod FEN {
        use super::{Field, FieldValue, Register};
        pub const FIELD: Field<u32, Register> =
            Field::new((1u32 << 4) as u32, 4 as u32);
        pub const ENABLED: FieldValue<u32, Register> =
            FieldValue::new((1u32 << 4) as u32, (1u32 << 4) as u32);
        pub const DISABLED: FieldValue<u32, Register> =
            FieldValue::new((1u32 << 4) as u32, 0 as u32);
    }
    impl RegisterOf<Regs> for Register {
        type Access = ReadWriteAccess;
    }
}

pub mod icr {
    use super::{ReadOnlyAccess, RegisterId, RegisterOf, Regs, WriteOnlyAccess};
    pub enum Register {}
    pub const OFFSET: u32 = 12;
    pub const REG: RegisterId<u32, Register> = RegisterId::new();
    impl RegisterOf<Regs> for Register {
        type Access = WriteOnlyAccess;
    }
}

pub fn read_dr(regs: &Mmio<Regs>) -> u32 {
    regs.read(dr::REG)
}

pub fn read_fr_txff(regs: &Mmio<Regs>) -> u32 {
    regs.read_field(fr::TXFF)
}

pub fn write_icr(regs: &Mmio<Regs>, value: u32) {
    regs.write(icr::REG, value)
}

pub fn write_lcr_fen_enabled(regs: &Mmio<Regs>) {
    regs.write_field(lcr_h::FEN::ENABLED)
}

/// Stand-in for the facade's `Mmio::<Block>::modify`. Specialized
/// to `lcr_h::Register` so the body translation test has a
/// monomorphic MIR body to walk. Mirrors the facade's
/// read-modify-write shape: one read + one write on the same
/// register marker.
pub fn mmio_modify_lcr_h(
    regs: &Mmio<Regs>,
    reg: RegisterId<u32, lcr_h::Register>,
    _values: FieldValue<u32, lcr_h::Register>,
) {
    let cur: u32 = regs.read(reg);
    regs.write(reg, cur);
}

pub fn modify_lcr_fen_disabled(regs: &Mmio<Regs>) {
    mmio_modify_lcr_h(regs, lcr_h::REG, lcr_h::FEN::DISABLED)
}

pub fn volatile_dead_temp_dr(regs: &Mmio<Regs>) {
    let _ = regs.read(dr::REG);
}

#[rseq::driver(name = "register-smoke")]
pub struct Drv {}
"#;

// ---------------------------------------------------------------------------
// Snapshot + translation helper
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RegCallSnapshot {
    intrinsic: RegseqIntrinsic,
    register_block: RegisterBlockId,
    register_offset: u32,
    access_width: u32,
    field: Option<(u64, u32, String)>,
    error_target_is_some: bool,
    /// The basic block index that carried this IntrinsicCall; used
    /// by the volatile dead-temp test to assert the `BodySourceMap`
    /// records a non-compiler-generated span at this slot.
    block_index: usize,
    /// `BodySourceMap.blocks[block_index].terminator` kind discriminator:
    /// `true` when the translated body recorded an authored source
    /// span at this terminator slot, `false` when the body resolved
    /// to `SourceOrigin::CompilerGenerated`. Volatile dead-temp
    /// reads must be retained with a real span.
    terminator_has_authored_span: bool,
}

struct ScenarioCtx<'tcx> {
    tcx: TyCtxt<'tcx>,
    /// Local intrinsic lookup keyed by each `Mmio<T>` method's `DefId`.
    intrinsic_lookup: IntrinsicLookup,
    /// Synthetic abstraction registry mapping the specimen's local
    /// `Mmio` ADT to the canonical `"mmio"` identifier. Pass 2's
    /// receiver-type check resolves through this registry to confirm
    /// the `args[0]` type is a registered transport abstraction
    /// before extracting the `Block` generic argument.
    abstraction_registry: AbstractionTypeRegistry,
    /// The `lcr_h` / `dr` / `fr` / `icr` byte offsets pulled from the
    /// built register block.
    dr_offset: u32,
    fr_offset: u32,
    lcr_h_offset: u32,
    icr_offset: u32,
    block_id: RegisterBlockId,
}

fn build_scenario<'tcx>(tcx: TyCtxt<'tcx>, pass1: &Pass1Output) -> ScenarioCtx<'tcx> {
    // Register block identity: the specimen places `Regs` at the
    // crate root, so `pass1.model.root_module.register_blocks` has
    // exactly one entry.
    assert_eq!(pass1.model.root_module.register_blocks.len(), 1);
    let block = &pass1.model.root_module.register_blocks[0];
    let resolve_offset = |name: &str| -> u32 {
        block
            .fields
            .iter()
            .find(|f| pass1.model.symbols.resolve(f.name) == name)
            .unwrap_or_else(|| panic!("register block has no `{name}` field"))
            .byte_offset
    };
    let scenario_block_id = block.id;

    // Build the intrinsic lookup from the four `Mmio` methods by
    // locating each method's `DefId` in the HIR.
    let read = find_impl_fn_def_id(tcx, "Mmio<Block>", "read");
    let read_field = find_impl_fn_def_id(tcx, "Mmio<Block>", "read_field");
    let write = find_impl_fn_def_id(tcx, "Mmio<Block>", "write");
    let write_field = find_impl_fn_def_id(tcx, "Mmio<Block>", "write_field");
    let entries = vec![
        (read, "mmio_read"),
        (read_field, "mmio_read_field"),
        (write, "mmio_write"),
        (write_field, "mmio_write_field"),
    ];
    let lookup = IntrinsicLookup::from_entries(tcx, entries.into_iter())
        .expect("canonical names must parse");

    // Seed a synthetic AbstractionTypeRegistry mapping the local
    // `Mmio` struct's `DefId` to the canonical `"mmio"` identifier.
    // The production registry walks the facade crate only, so the
    // local-specimen Mmio would otherwise not appear in the
    // registry and Rule 7's receiver-type check would reject every
    // call.
    let mmio_did = find_struct_def_id(tcx, "Mmio");
    let abstraction_registry =
        AbstractionTypeRegistry::from_entries([(mmio_did, "mmio".to_string())]);

    ScenarioCtx {
        tcx,
        intrinsic_lookup: lookup,
        abstraction_registry,
        dr_offset: resolve_offset("dr"),
        fr_offset: resolve_offset("fr"),
        lcr_h_offset: resolve_offset("lcr_h"),
        icr_offset: resolve_offset("icr"),
        block_id: scenario_block_id,
    }
}

/// Find the top-level `pub struct <name>` or `pub enum <name>` in the
/// specimen's crate root. Used to resolve the local `Mmio` ADT's
/// [`DefId`] for the synthetic abstraction registry.
fn find_struct_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        match &item.kind {
            rustc_hir::ItemKind::Struct(ident, ..) | rustc_hir::ItemKind::Enum(ident, ..)
                if ident.name.as_str() == name =>
            {
                return item.owner_id.def_id.to_def_id();
            }
            _ => {}
        }
    }
    panic!("type `{name}` not found in specimen")
}

fn translate_body<'tcx>(
    ctx: &ScenarioCtx<'tcx>,
    pass1: &Pass1Output,
    method_def_id: DefId,
    register_index_override: Option<&RegisterIndex>,
    intrinsic_lookup_override: Option<&IntrinsicLookup>,
) -> Result<Vec<RegCallSnapshot>, ConstructionError> {
    // `RegisterFieldMeta.name` symbols are interned in Pass 1's
    // `SymbolInterner`, so we must resolve them through the SAME
    // interner Pass 1 used to compute the RegisterIndex -- not the
    // fresh Pass 2 interner below.
    let pass1_symbols = &pass1.model.symbols;
    let tcx = ctx.tcx;
    let function_id = *pass1
        .cross_refs
        .functions
        .get(&method_def_id)
        .unwrap_or_else(|| {
            panic!("method DefId {method_def_id:?} missing from cross_refs.functions")
        });
    let deref_lookup = DerefCollapseLookup::build(tcx, &ctx.abstraction_registry)
        .expect("Deref lang item resolves");
    let ordered_markers = OrderedMarkers::default();
    let try_markers = TryMarkers::default();
    let log_arg = LogArgUnwrap::default();
    let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
    let constants = ConstantIndex::new(&pass1.cross_refs.constants);
    let functions = FunctionIndex::new(&pass1.cross_refs.functions);
    let lookup: &IntrinsicLookup = intrinsic_lookup_override.unwrap_or(&ctx.intrinsic_lookup);
    let reg_index: &RegisterIndex =
        register_index_override.unwrap_or(&pass1.cross_refs.register_index);
    let mut interner = SymbolInterner::new();
    let body = build_body(
        tcx,
        method_def_id,
        function_id,
        &type_def_index,
        &constants,
        &functions,
        lookup,
        &deref_lookup,
        &ordered_markers,
        &try_markers,
        &log_arg,
        reg_index,
        &pass1.cross_refs.register_blocks,
        &ctx.abstraction_registry,
        &PinStateLookup::default(),
        &mut interner,
    )?;

    let mut calls: Vec<RegCallSnapshot> = Vec::new();
    for (idx, bb) in body.basic_blocks.iter().enumerate() {
        let TerminatorKind::IntrinsicCall {
            intrinsic,
            metadata,
            error_target,
            ..
        } = &bb.terminator.kind
        else {
            continue;
        };
        let Some(IntrinsicMetadata::Register {
            register_block,
            register_offset,
            access_width,
            field,
        }) = metadata
        else {
            continue;
        };
        let field_snap = field
            .as_ref()
            .map(|f| (f.mask, f.shift, pass1_symbols.resolve(f.name).to_string()));
        let terminator_has_authored_span = matches!(
            body.source_map.blocks[idx].terminator,
            regseq_model_old::identity::SourceOrigin::Source(_),
        );
        calls.push(RegCallSnapshot {
            intrinsic: *intrinsic,
            register_block: *register_block,
            register_offset: *register_offset,
            access_width: *access_width,
            field: field_snap,
            error_target_is_some: error_target.is_some(),
            block_index: idx,
            terminator_has_authored_span,
        });
    }
    Ok(calls)
}

// ---------------------------------------------------------------------------
// Pass 1 basic assertion: RegisterIndex is populated
// ---------------------------------------------------------------------------

#[test]
fn pass1_register_index_has_per_register_entries() {
    let len = with_specimen(REGISTER_SPECIMEN, "rm_pass1_populated", |_tcx, pass1| {
        pass1.cross_refs.register_index.len()
    });
    // The specimen defines four register marker types (dr, fr,
    // lcr_h, icr). Every non-reserved marker must land in the
    // index -- so the count should be at least 4.
    assert!(
        len >= 4,
        "Rule 7 RegisterIndex must contain entries for dr/fr/lcr_h/icr, got len={len}",
    );
}

// ---------------------------------------------------------------------------
// Full-register read / write metadata
// ---------------------------------------------------------------------------

#[test]
fn full_register_read_carries_metadata_without_field() {
    let (calls, dr_offset, block_id) =
        with_specimen(REGISTER_SPECIMEN, "rm_read_dr", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            let read_dr = find_free_fn_def_id(tcx, "read_dr");
            let calls = translate_body(&ctx, pass1, read_dr, None, None)
                .expect("read_dr translation must succeed");
            (calls, ctx.dr_offset, ctx.block_id)
        });
    assert_eq!(
        calls.len(),
        1,
        "read_dr must produce one register intrinsic call"
    );
    let c = &calls[0];
    assert_eq!(c.intrinsic, RegseqIntrinsic::RegisterRead);
    assert_eq!(c.register_block, block_id);
    assert_eq!(c.register_offset, dr_offset);
    assert_eq!(c.access_width, 32);
    assert!(c.field.is_none());
    assert!(
        !c.error_target_is_some,
        "MMIO register intrinsics are infallible; error_target must be None",
    );
}

#[test]
fn full_register_write_carries_metadata_without_field() {
    let (calls, icr_offset, block_id) =
        with_specimen(REGISTER_SPECIMEN, "rm_write_icr", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            let did = find_free_fn_def_id(tcx, "write_icr");
            let calls =
                translate_body(&ctx, pass1, did, None, None).expect("write_icr translation");
            (calls, ctx.icr_offset, ctx.block_id)
        });
    assert_eq!(calls.len(), 1);
    let c = &calls[0];
    assert_eq!(c.intrinsic, RegseqIntrinsic::RegisterWrite);
    assert_eq!(c.register_block, block_id);
    assert_eq!(c.register_offset, icr_offset);
    assert_eq!(c.access_width, 32);
    assert!(c.field.is_none());
}

// ---------------------------------------------------------------------------
// Field read / write metadata
// ---------------------------------------------------------------------------

#[test]
fn read_field_carries_field_mask_shift_name() {
    let (calls, fr_offset, block_id) =
        with_specimen(REGISTER_SPECIMEN, "rm_read_fr_txff", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            let did = find_free_fn_def_id(tcx, "read_fr_txff");
            let calls =
                translate_body(&ctx, pass1, did, None, None).expect("read_fr_txff translation");
            (calls, ctx.fr_offset, ctx.block_id)
        });
    assert_eq!(calls.len(), 1);
    let c = &calls[0];
    assert_eq!(c.intrinsic, RegseqIntrinsic::RegisterReadField);
    assert_eq!(c.register_block, block_id);
    assert_eq!(c.register_offset, fr_offset);
    assert_eq!(c.access_width, 32);
    let (mask, shift, name) = c.field.as_ref().expect("read_field must carry field meta");
    assert_eq!(*mask, 1u64 << 5);
    assert_eq!(*shift, 5);
    assert_eq!(name, "TXFF");
}

#[test]
fn write_field_named_value_carries_field_mask_shift_name() {
    let (calls, lcr_offset, block_id) =
        with_specimen(REGISTER_SPECIMEN, "rm_write_fen", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            let did = find_free_fn_def_id(tcx, "write_lcr_fen_enabled");
            let calls = translate_body(&ctx, pass1, did, None, None)
                .expect("write_lcr_fen_enabled translation");
            (calls, ctx.lcr_h_offset, ctx.block_id)
        });
    assert_eq!(calls.len(), 1);
    let c = &calls[0];
    assert_eq!(c.intrinsic, RegseqIntrinsic::RegisterWriteField);
    assert_eq!(c.register_block, block_id);
    assert_eq!(c.register_offset, lcr_offset);
    assert_eq!(c.access_width, 32);
    let (mask, shift, name) = c
        .field
        .as_ref()
        .expect("write_field named-value must carry field meta");
    assert_eq!(*mask, 1u64 << 4);
    assert_eq!(*shift, 4);
    assert_eq!(name, "FEN");
}

// ---------------------------------------------------------------------------
// `Mmio::modify` body: two metadata-tagged IntrinsicCalls, read then write,
// both targeting the same register_block + register_offset.
// ---------------------------------------------------------------------------

#[test]
fn modify_body_contains_read_then_write_with_matching_metadata() {
    let (calls, lcr_offset, block_id) =
        with_specimen(REGISTER_SPECIMEN, "rm_modify_body", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            // `modify` was registered by Pass 1's impl-block walk as
            // a local inherent method of `Mmio<Block>`. Its DefId
            // lives in `cross_refs.functions`.
            let modify_def = find_free_fn_def_id(tcx, "mmio_modify_lcr_h");
            let calls = translate_body(&ctx, pass1, modify_def, None, None)
                .expect("Mmio::modify body translation must succeed");
            (calls, ctx.lcr_h_offset, ctx.block_id)
        });
    // The specimen's `modify` body reads once then writes once.
    assert_eq!(
        calls.len(),
        2,
        "Mmio::modify body must expose exactly two register intrinsic calls (read + write)",
    );
    assert_eq!(calls[0].intrinsic, RegseqIntrinsic::RegisterRead);
    assert_eq!(calls[1].intrinsic, RegseqIntrinsic::RegisterWrite);
    for c in &calls {
        assert_eq!(c.register_block, block_id);
        assert_eq!(c.access_width, 32);
        assert!(
            !c.error_target_is_some,
            "MMIO register intrinsics are infallible; error_target must be None",
        );
    }
    // Both intrinsic calls target the `lcr_h` register's offset
    // (the one `modify` was called against). Field metadata is
    // None because both underlying intrinsics operate on the full
    // register (`read` / `write`).
    assert_eq!(
        calls[0].register_offset, lcr_offset,
        "modify body's RegisterRead must target `lcr_h`'s byte offset",
    );
    assert_eq!(
        calls[1].register_offset, lcr_offset,
        "modify body's RegisterWrite must target `lcr_h`'s byte offset",
    );
    for c in &calls {
        assert!(
            c.field.is_none(),
            "modify body's read/write use `RegisterId` carriers (not fields); `field` must be None",
        );
    }
}

// ---------------------------------------------------------------------------
// Volatile dead-temp read must be retained.
// ---------------------------------------------------------------------------

#[test]
fn volatile_dead_temp_read_is_retained() {
    let (calls, dr_offset) = with_specimen(REGISTER_SPECIMEN, "rm_dead_temp", |tcx, pass1| {
        let ctx = build_scenario(tcx, pass1);
        let did = find_free_fn_def_id(tcx, "volatile_dead_temp_dr");
        let calls = translate_body(&ctx, pass1, did, None, None)
            .expect("volatile_dead_temp_dr translation");
        (calls, ctx.dr_offset)
    });
    assert_eq!(calls.len(), 1, "volatile dead-temp read must remain in IR");
    assert_eq!(calls[0].intrinsic, RegseqIntrinsic::RegisterRead);
    assert_eq!(calls[0].register_offset, dr_offset);
    assert!(calls[0].field.is_none());
    // Volatile reads have visible side effects (MMIO) and must keep
    // the user's authored span -- `BodySourceMap` must record a
    // real `Source(...)` entry at the terminator slot, not
    // `CompilerGenerated`.
    assert!(
        calls[0].terminator_has_authored_span,
        "volatile dead-temp RegisterRead must retain the user's authored span in BodySourceMap",
    );
}

// ---------------------------------------------------------------------------
// Negative: cross-block marker raises CrossBlockFieldUsage.
// ---------------------------------------------------------------------------

#[test]
fn cross_block_marker_raises_error() {
    use regseq_driver_old::construction::register_metadata::{RegisterEntry, RegisterIndex};
    use regseq_model_old::identity::{DeclId, RegisterBlockId};

    let outcome: Result<(), String> =
        with_specimen(REGISTER_SPECIMEN, "rm_cross_block", |tcx, pass1| {
            let ctx = build_scenario(tcx, pass1);
            // Pick the real `dr::Register` marker DefId by asking
            // the built index for any entry (the specimen has four
            // markers). The corresponding entry's block is swapped
            // for a fabricated id.
            let real = pass1.cross_refs.register_index.clone();
            let marker_did = find_register_marker_did(tcx);
            let entry = real
                .lookup(marker_did)
                .expect("fixture marker must be in the built index");
            let mut bogus_index = RegisterIndex::default();
            bogus_index.insert(
                marker_did,
                RegisterEntry {
                    block: RegisterBlockId::from(DeclId::new(u32::MAX)),
                    offset: entry.offset,
                    access_width: entry.access_width,
                    access_kind: entry.access_kind,
                    fields: entry.fields.clone(),
                },
            );
            let did = find_free_fn_def_id(tcx, "read_dr");
            match translate_body(&ctx, pass1, did, Some(&bogus_index), None) {
                Ok(_) => Err("expected CrossBlockFieldUsage, got Ok".to_string()),
                Err(ConstructionError::CrossBlockFieldUsage { .. }) => Ok(()),
                Err(other) => Err(format!("expected CrossBlockFieldUsage, got {other:?}")),
            }
        });
    outcome.unwrap();
}

/// Find the first `Register` marker type DefId declared inside a
/// per-register submodule of the specimen's crate root.
fn find_register_marker_did<'tcx>(tcx: TyCtxt<'tcx>) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        let rustc_hir::ItemKind::Mod(_, m) = item.kind else {
            continue;
        };
        for sub_id in m.item_ids {
            let sub = tcx.hir_item(*sub_id);
            if let rustc_hir::ItemKind::Enum(ident, ..) = &sub.kind {
                if ident.name.as_str() == "Register" {
                    return sub.owner_id.def_id.to_def_id();
                }
            }
        }
    }
    panic!("no per-register `Register` marker found in specimen");
}

// ---------------------------------------------------------------------------
// Negative: wrong access kind raises WrongRegisterAccessKind.
// ---------------------------------------------------------------------------

#[test]
fn wrong_access_kind_raises_error() {
    let outcome: Result<(), String> =
        with_specimen(REGISTER_SPECIMEN, "rm_wrong_access", |tcx, pass1| {
            let ctx_base = build_scenario(tcx, pass1);
            // Build an intrinsic lookup that mis-labels `read_field`
            // as `mmio_write_field`. Translating `read_fr_txff` then
            // asks Rule 7 to validate a WriteField against the
            // ReadOnly `fr` register -- mismatch -> error.
            let read_field = find_impl_fn_def_id(tcx, "Mmio<Block>", "read_field");
            let read_m = find_impl_fn_def_id(tcx, "Mmio<Block>", "read");
            let write_m = find_impl_fn_def_id(tcx, "Mmio<Block>", "write");
            let write_field_m = find_impl_fn_def_id(tcx, "Mmio<Block>", "write_field");
            let entries = vec![
                (read_m, "mmio_read"),
                (read_field, "mmio_write_field"), // deliberate mis-mapping
                (write_m, "mmio_write"),
                (write_field_m, "mmio_write_field"),
            ];
            let twisted_lookup = IntrinsicLookup::from_entries(tcx, entries.into_iter()).unwrap();

            let did = find_free_fn_def_id(tcx, "read_fr_txff");
            match translate_body(&ctx_base, pass1, did, None, Some(&twisted_lookup)) {
                Ok(_) => Err("expected WrongRegisterAccessKind, got Ok".to_string()),
                Err(ConstructionError::WrongRegisterAccessKind {
                    access_kind,
                    intrinsic,
                    ..
                }) => {
                    assert_eq!(access_kind, AccessKind::ReadOnly);
                    assert_eq!(intrinsic, RegseqIntrinsic::RegisterWriteField);
                    Ok(())
                }
                Err(other) => Err(format!("expected WrongRegisterAccessKind, got {other:?}")),
            }
        });
    outcome.unwrap();
}

// Keep an unused HashMap import alive for future hand-built tests.
#[allow(dead_code)]
fn _hashmap_stub() -> HashMap<DefId, RegisterBlockId> {
    HashMap::new()
}
