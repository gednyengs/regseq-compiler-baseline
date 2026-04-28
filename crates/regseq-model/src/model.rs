//! Top-level `RegseqModel` struct, construction, and the
//! construction-invariants checker.
//!
//! `RegseqModel` is the owned root the bridge produces by extracting
//! one rustc compilation. All other stage-1 tables hang off it. Per
//! Q2 the model is fully owned (no `'tcx`); per Q6 the `crates`
//! table holds one entry per crate the model contains items from
//! (the spec crate plus regseq-aware deps).
//!
//! The 10 construction invariants listed in the spec section
//! "Construction invariants" are enforced where stage 1 has the
//! information to do so. Several invariants depend on
//! later-stage Item / Body / Ty / Const content (e.g., F4 item
//! ordering needs `Item.def_path`, which lands in stage 12); those
//! are explicitly deferred and re-enforced when the relevant stage
//! expands the placeholder. See the per-invariant comments in
//! `RegseqModel::validate`.

use serde::{Deserialize, Serialize};

use crate::crate_info::{CrateInfo, StableCrateId};
use crate::def_path::DefPathHash;
use crate::ids::{
    BodyId, ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, StringId, TyId,
};
use crate::index_vec::IndexVec;
use crate::instance::{GenericArg, Instance, InstanceKind};
use crate::placeholders::{Body, Const, FnSig, Item, Ty};
use crate::source::SourceFile;
use crate::string_interner::StringInterner;

/// The owned root struct returned by the bridge for one rustc
/// compilation.
///
/// Field order matches the spec's "Top-level RegseqModel" section.
/// `source_files[0]` is the synthesized-source sentinel
/// (per construction invariant 8); `RegseqModel::new` populates it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegseqModel {
    /// Crate inventory. One entry per crate the model contains
    /// items from (per Q6: spec crate + regseq facade + regseq-aware
    /// deps).
    pub crates: IndexVec<CrateId, CrateInfo>,
    /// Index in `crates` of the spec crate the bridge was invoked
    /// on.
    pub primary_crate: CrateId,

    /// Content-unique string interner; `StringId::ZERO` is the
    /// empty string, pre-interned by `StringInterner::new`.
    pub strings: StringInterner,

    /// Source files. `source_files[0]` is the synthesized sentinel.
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    /// Type table. Stage 3 expands `Ty`.
    pub types: IndexVec<TyId, Ty>,
    /// Const table. Stage 4 expands `Const`.
    pub consts: IndexVec<ConstId, Const>,

    /// Source-level item alphabet (generic forms + attributes,
    /// not the emission table). Stage 12 expands `Item`.
    pub items: IndexVec<ItemId, Item>,
    /// F4 cross-build identity side table; mirrors `items`.
    pub item_def_paths: IndexVec<ItemId, DefPathHash>,

    /// Function signatures. Stage 11 expands `FnSig`.
    pub sigs: IndexVec<FnSigId, FnSig>,

    /// MIR bodies. Stage 11 expands `Body`.
    pub bodies: IndexVec<BodyId, Body>,

    /// The primary monomorphized table -- iteration target for
    /// domain analyses and C codegen.
    pub instances: IndexVec<InstanceId, Instance>,
}

/// Path used in `InvariantViolation` to identify *where* a dangling
/// ID was discovered. Stable strings rather than enum variants so
/// the violation set stays small.
const SOURCE_FILE_PATH: &str = "SourceFile.path (StringId)";
const CRATE_INFO_NAME: &str = "CrateInfo.name (StringId)";
const INSTANCE_ITEM: &str = "Instance.item (ItemId)";
const INSTANCE_SYMBOL_NAME: &str = "Instance.symbol_name (StringId)";
const INSTANCE_SPAN_FILE: &str = "Instance.span.file (SourceFileId)";
const INSTANCE_GENERIC_ARG_TY: &str = "Instance.generic_args[..](TyId)";
const INSTANCE_GENERIC_ARG_CONST: &str = "Instance.generic_args[..](ConstId)";
const INSTANCE_KIND_FUNCTION_SIG: &str = "InstanceKind::Function.sig (FnSigId)";
const INSTANCE_KIND_FUNCTION_BODY: &str = "InstanceKind::Function.body (BodyId)";
const INSTANCE_KIND_STATIC_TY: &str = "InstanceKind::Static.ty (TyId)";
const INSTANCE_KIND_STATIC_INIT: &str = "InstanceKind::Static.init (ConstId)";
const INSTANCE_KIND_CONST_TY: &str = "InstanceKind::Const.ty (TyId)";
const INSTANCE_KIND_CONST_VALUE: &str = "InstanceKind::Const.value (ConstId)";
const INSTANCE_KIND_STRUCT_LAYOUT: &str = "InstanceKind::Struct.layout (TyId)";
const INSTANCE_KIND_ENUM_LAYOUT: &str = "InstanceKind::Enum.layout (TyId)";
const INSTANCE_KIND_UNION_LAYOUT: &str = "InstanceKind::Union.layout (TyId)";

/// One construction-invariant violation.
///
/// `RegseqModel::validate` reports the *first* violation it finds.
/// Stage 1 enforces the subset of invariants whose information is
/// available without later-stage content (notably: invariants 3, 6
/// are deferred to stage 12, and the spec-crate stable-id check
/// inside invariant 4 is bridge-input-dependent; see the
/// per-invariant comments in `validate`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantViolation {
    /// (1) An ID does not resolve to a valid entry in its table.
    DanglingId {
        /// What field carried the dangling reference (a stable
        /// human-readable label; see the `*_PATH` constants in this
        /// module for the canonical strings).
        field: &'static str,
        /// Logical index value (`Idx::index()` of the offending ID).
        value: u32,
        /// Length of the table the ID was supposed to address.
        table_len: usize,
    },

    /// (2) `item_def_paths.len() != items.len()`.
    ItemDefPathsLengthMismatch { items_len: usize, def_paths_len: usize },

    /// (4) `primary_crate.index() >= crates.len()`.
    PrimaryCrateOutOfRange { value: u32, crates_len: usize },

    /// (5) Two `CrateInfo` entries share a `StableCrateId`.
    DuplicateStableCrateId {
        stable_id: StableCrateId,
        first: CrateId,
        second: CrateId,
    },

    /// (8) `source_files[0]` is not the synthesized sentinel.
    SynthesizedSentinelInvalid {
        /// Brief reason ("source_files empty", "path != \"<synthesized>\"", etc.).
        reason: &'static str,
    },
}

impl RegseqModel {
    /// Empty model with the synthesized-source sentinel pre-populated
    /// at `SourceFileId::ZERO` and the primary crate registered at
    /// `CrateId::ZERO`.
    ///
    /// Bridge usage: call `with_primary(name, stable_id)`, then push
    /// items / bodies / instances / etc. as extraction proceeds.
    /// Real source files are pushed starting at `SourceFileId(1)`
    /// (the sentinel already occupies index 0).
    ///
    /// The constructor takes the primary crate's *raw* name and
    /// `StableCrateId` (rather than a pre-built `CrateInfo`) because
    /// the crate name has to be interned through the model's own
    /// `StringInterner` -- a pre-built `CrateInfo` would carry a
    /// `StringId` that came from somewhere else, which the model
    /// cannot resolve.
    pub fn with_primary(primary_name: &str, primary_stable_crate_id: StableCrateId) -> Self {
        let mut strings = StringInterner::new();

        // (8) source_files[0] is the synthesized sentinel.
        let synth_path = strings.intern("<synthesized>");
        let mut source_files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
        let pushed = source_files.push(SourceFile {
            path: synth_path,
            content: None,
            line_starts: vec![0],
        });
        debug_assert_eq!(pushed, SourceFileId::SYNTHESIZED);

        let primary_name_id = strings.intern(primary_name);
        let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
        let primary_id = crates.push(CrateInfo {
            name: primary_name_id,
            stable_crate_id: primary_stable_crate_id,
        });

        Self {
            crates,
            primary_crate: primary_id,
            strings,
            source_files,
            types: IndexVec::new(),
            consts: IndexVec::new(),
            items: IndexVec::new(),
            item_def_paths: IndexVec::new(),
            sigs: IndexVec::new(),
            bodies: IndexVec::new(),
            instances: IndexVec::new(),
        }
    }

    /// Validates the construction invariants enforceable at stage 1.
    ///
    /// Returns the first violation found, or `Ok(())` when the
    /// model is well-formed. Coverage:
    ///
    /// - Invariant 1 (IDs resolve): walked over `crates`,
    ///   `source_files`, and `instances` for every ID-bearing
    ///   field. `Item` / `Body` / `Ty` / `Const` / `FnSig` are
    ///   placeholders at stage 1, so no IDs ride inside them yet;
    ///   stage 3, 4, 11, and 12 extend coverage as their fields
    ///   land.
    /// - Invariant 2 (`item_def_paths` mirrors `items`): length
    ///   check.
    /// - Invariant 3 (items F4-sorted): deferred. The F4 sort key
    ///   reads `crates[item.def_path.crate_id].name`, segments,
    ///   and disambiguators; `Item.def_path` lands in stage 12.
    /// - Invariant 4 (`primary_crate` valid; corresponds to spec
    ///   crate): the index-bound half is checked here. The
    ///   "corresponds to spec crate" half is bridge-input-dependent
    ///   (matches `bridge_input.spec_crate_stable_id`) and lives
    ///   in the bridge.
    /// - Invariant 5 (`StableCrateId` unique across crates): direct
    ///   check.
    /// - Invariant 6 (`DefPath.crate_id` valid for every item):
    ///   deferred. Needs `Item.def_path` (stage 12).
    /// - Invariant 7 (no `'tcx`): structural; satisfied by Q1 and
    ///   the type definitions in this crate. Not a runtime check.
    /// - Invariant 8 (synthesized source-file sentinel): direct
    ///   check on `source_files[0]`.
    /// - Invariant 9 (Instance closure-completeness): vacuously
    ///   true at stage 1 because no stage-1 construct stores an
    ///   `InstanceId`. Stage 9 (terminators) and stage 12 (vtables)
    ///   add reference sites; coverage extends then.
    /// - Invariant 10 (Function body presence): walked here for
    ///   every Instance with `kind = Function { body: Some(b) }`.
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        // (8) Synthesized source-file sentinel at index 0.
        self.validate_synthesized_sentinel()?;

        // (4, partial) primary_crate index is in range.
        if self.primary_crate.index() >= self.crates.len() {
            return Err(InvariantViolation::PrimaryCrateOutOfRange {
                value: self.primary_crate.as_u32(),
                crates_len: self.crates.len(),
            });
        }

        // (1, partial) StringIds in the crates table resolve.
        for info in self.crates.iter() {
            self.check_string_id(info.name, CRATE_INFO_NAME)?;
        }

        // (1, partial) StringIds in source files resolve.
        for file in self.source_files.iter() {
            self.check_string_id(file.path, SOURCE_FILE_PATH)?;
        }

        // (5) StableCrateId unique across crates.
        self.validate_unique_stable_crate_ids()?;

        // (2) item_def_paths mirrors items in length.
        if self.item_def_paths.len() != self.items.len() {
            return Err(InvariantViolation::ItemDefPathsLengthMismatch {
                items_len: self.items.len(),
                def_paths_len: self.item_def_paths.len(),
            });
        }

        // (1) and (10) for instances.
        for instance in self.instances.iter() {
            self.validate_instance(instance)?;
        }

        Ok(())
    }

    fn validate_synthesized_sentinel(&self) -> Result<(), InvariantViolation> {
        let sf = self.source_files.iter().next().ok_or_else(|| {
            InvariantViolation::SynthesizedSentinelInvalid {
                reason: "source_files is empty",
            }
        })?;

        // The path StringId must resolve and the resolved string
        // must be exactly "<synthesized>".
        if sf.path.index() >= self.strings.len() {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel path StringId is dangling",
            });
        }
        if self.strings.resolve(sf.path) != "<synthesized>" {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel path is not \"<synthesized>\"",
            });
        }
        if sf.content.is_some() {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel content must be None",
            });
        }
        if sf.line_starts != [0] {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel line_starts must be vec![0]",
            });
        }
        Ok(())
    }

    fn validate_unique_stable_crate_ids(&self) -> Result<(), InvariantViolation> {
        // O(n^2) is fine: the `crates` table is bounded by the
        // crate-graph fan-in (typically <= a few dozen entries).
        // Hashing would obscure error reporting (we want the two
        // colliding CrateIds in the violation).
        for i in 0..self.crates.len() {
            for j in (i + 1)..self.crates.len() {
                let a = CrateId::from_u32(i as u32);
                let b = CrateId::from_u32(j as u32);
                if self.crates[a].stable_crate_id == self.crates[b].stable_crate_id {
                    return Err(InvariantViolation::DuplicateStableCrateId {
                        stable_id: self.crates[a].stable_crate_id,
                        first: a,
                        second: b,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_instance(&self, instance: &Instance) -> Result<(), InvariantViolation> {
        self.check_item_id(instance.item, INSTANCE_ITEM)?;
        self.check_string_id(instance.symbol_name, INSTANCE_SYMBOL_NAME)?;
        self.check_source_file_id(instance.span.file, INSTANCE_SPAN_FILE)?;

        for arg in &instance.generic_args {
            match *arg {
                GenericArg::Type(ty) => self.check_ty_id(ty, INSTANCE_GENERIC_ARG_TY)?,
                GenericArg::Const(c) => self.check_const_id(c, INSTANCE_GENERIC_ARG_CONST)?,
            }
        }

        match instance.kind {
            InstanceKind::Function { sig, body } => {
                self.check_fn_sig_id(sig, INSTANCE_KIND_FUNCTION_SIG)?;
                if let Some(b) = body {
                    // (10) Body presence matches kind: when
                    // `Some(b)`, b resolves to a valid Body.
                    self.check_body_id(b, INSTANCE_KIND_FUNCTION_BODY)?;
                }
            }
            InstanceKind::Static { ty, init } => {
                self.check_ty_id(ty, INSTANCE_KIND_STATIC_TY)?;
                self.check_const_id(init, INSTANCE_KIND_STATIC_INIT)?;
            }
            InstanceKind::Const { ty, value } => {
                self.check_ty_id(ty, INSTANCE_KIND_CONST_TY)?;
                self.check_const_id(value, INSTANCE_KIND_CONST_VALUE)?;
            }
            InstanceKind::Struct { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_STRUCT_LAYOUT)?;
            }
            InstanceKind::Enum { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_ENUM_LAYOUT)?;
            }
            InstanceKind::Union { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_UNION_LAYOUT)?;
            }
        }

        Ok(())
    }

    fn check_string_id(&self, id: StringId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.strings.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.strings.len(),
            });
        }
        Ok(())
    }

    fn check_source_file_id(
        &self,
        id: SourceFileId,
        field: &'static str,
    ) -> Result<(), InvariantViolation> {
        if id.index() >= self.source_files.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.source_files.len(),
            });
        }
        Ok(())
    }

    fn check_item_id(&self, id: ItemId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.items.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.items.len(),
            });
        }
        Ok(())
    }

    fn check_ty_id(&self, id: TyId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.types.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.types.len(),
            });
        }
        Ok(())
    }

    fn check_const_id(&self, id: ConstId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.consts.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.consts.len(),
            });
        }
        Ok(())
    }

    fn check_body_id(&self, id: BodyId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.bodies.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.bodies.len(),
            });
        }
        Ok(())
    }

    fn check_fn_sig_id(&self, id: FnSigId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.sigs.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.sigs.len(),
            });
        }
        Ok(())
    }
}
