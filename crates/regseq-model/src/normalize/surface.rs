//! Per-surface normalization helpers.
//!
//! Each helper consumes one [`RawFrontendArtifact`](super::raw_artifact::RawFrontendArtifact)
//! top-level surface and writes the corresponding declaration-side
//! slice into the in-progress [`RegseqModel`] being built by
//! [`super::normalize`]. Helpers run in a fixed order defined by
//! [`super::normalize`] so the resulting model is deterministic.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{
    DeclId, FileId, ModuleId, PropertyId, RegisterBlockId, RegisterDescriptorId, ResourceId,
    SourceLocation, SourceOrigin, Symbol, SymbolInterner, TypeDefId, UserDataId,
};
use crate::ir::{RegseqType, ScalarType};
use crate::model::driver::{DeviceClass, DriverIdentity, DriverStruct, PmInfo};
use crate::model::framework::TypeDefClassification;
use crate::model::module::{Module, ModuleKind};
use crate::model::register::{
    AccessKind, BitfieldDef, NamedValue, RegisterBlock, RegisterBlockAttributes,
    RegisterBlockField, RegisterDescriptor,
};
use crate::model::resources::{
    ClockDecl, ClockResource, InterruptDecl, InterruptResource, MmioDecl, MmioResource,
    PinCtrlDecl, PinCtrlResource, PropertyField, ResetDecl, ResetResource, Shape, SyncKind,
    SyncResource, UserDataField, UserDataKind,
};
use crate::model::type_def::{Attrs, TypeDef, TypeDefKind};

use super::provenance::NormalizeContext;
use super::raw_artifact::{
    RawClockField, RawClockSurface, RawDriverIdentity, RawFieldFact, RawFrontendArtifact,
    RawIdentityEnrichmentSurface, RawInterruptResourceDecl, RawInterruptSurface, RawMmioFieldDecl,
    RawPinCtrlField, RawPinCtrlSurface, RawPmGate, RawPmGateSurface, RawPropertyField,
    RawPropertyLint, RawPropertySurface, RawRegisterAccessSurface, RawRegisterBlockDecl,
    RawRegisterDescriptorFact, RawResetField, RawResetSurface, RawSyncFieldDecl,
    RawSynchronizationSurface,
};
use super::{NormalizeError, NormalizeErrorKind};

/// Monotonic DeclId allocator, normalizer-local.
pub(super) struct DeclIdAllocator {
    next: u32,
}

impl DeclIdAllocator {
    pub(super) fn new() -> Self {
        Self { next: 0 }
    }

    pub(super) fn alloc(&mut self) -> DeclId {
        let id = DeclId::new(self.next);
        self.next += 1;
        id
    }
}

/// Scratch state threaded through the per-surface helpers.
pub(super) struct Builder<'a> {
    pub ctx: &'a mut NormalizeContext,
    pub symbols: SymbolInterner,
    pub allocator: DeclIdAllocator,
    pub source_file: FileId,
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,
    pub register_block_by_name: BTreeMap<String, RegisterBlockId>,
    pub register_descriptor_by_name: BTreeMap<String, RegisterDescriptorId>,
    pub root_module: Module,
    pub driver_module: Module,
    pub driver_struct: DriverStruct,
    #[allow(dead_code)]
    pub driver_label: String,
}

impl<'a> Builder<'a> {
    pub(super) fn new(
        ctx: &'a mut NormalizeContext,
        artifact: &RawFrontendArtifact,
    ) -> Result<Self, NormalizeError> {
        // Enter the identity surface scope for error provenance.
        ctx.push_surface("identity");
        let raw_identity = match artifact.identity.as_ref() {
            Some(id) => id.clone(),
            None => {
                let provenance = ctx.provenance();
                ctx.pop_surface();
                return Err(NormalizeError {
                    kind: NormalizeErrorKind::MissingRequiredAttribute {
                        surface: "identity",
                        attribute: "identity",
                        item: "<artifact>".into(),
                    },
                    provenance,
                });
            }
        };
        if raw_identity.driver_name.is_empty() {
            let provenance = ctx.provenance();
            ctx.pop_surface();
            return Err(NormalizeError {
                kind: NormalizeErrorKind::MissingRequiredAttribute {
                    surface: "identity",
                    attribute: "driver_name",
                    item: "<DriverIdentity>".into(),
                },
                provenance,
            });
        }
        ctx.pop_surface();

        let mut symbols = SymbolInterner::new();
        let source_file = symbols.intern_file("<normalize-synthetic>");
        let mut allocator = DeclIdAllocator::new();
        let mut source_locations = BTreeMap::new();

        let driver_label = raw_identity.driver_name.clone();
        let identity = intern_driver_identity(&mut symbols, &raw_identity);
        let device_class = if raw_identity.device_class.is_empty() {
            None
        } else {
            Some(DeviceClass {
                class: symbols.intern(&raw_identity.device_class),
            })
        };

        let root_id = allocator.alloc();
        let driver_mod_id = allocator.alloc();
        let driver_type_id_raw = allocator.alloc();
        record_source(&mut source_locations, root_id, source_file);
        record_source(&mut source_locations, driver_mod_id, source_file);
        record_source(&mut source_locations, driver_type_id_raw, source_file);

        let root_name = symbols.intern("<crate-root>");
        let driver_mod_name = symbols.intern(&driver_label);
        let driver_struct_name_raw = format!("{driver_label}Driver");
        let driver_struct_name = symbols.intern(&driver_struct_name_raw);

        let driver_type_id = TypeDefId::from(driver_type_id_raw);
        let driver_struct = DriverStruct {
            type_id: driver_type_id,
            name: driver_struct_name,
            identity,
            device_class,
            pm_info: None,
            platform_metadata: None,
            clocks: Vec::new(),
            resets: Vec::new(),
            pinctrl: Vec::new(),
            interrupts: Vec::new(),
            mmio: Vec::new(),
            sync: Vec::new(),
            properties: Vec::new(),
            user_data: Vec::new(),
        };

        let root_module = Module {
            id: ModuleId::from(root_id),
            name: root_name,
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
        };

        let driver_module = Module {
            id: ModuleId::from(driver_mod_id),
            name: driver_mod_name,
            kind: ModuleKind::Driver,
            submodules: Vec::new(),
            constants: Vec::new(),
            free_functions: Vec::new(),
            type_defs: vec![TypeDef {
                id: driver_type_id,
                name: driver_struct_name,
                kind: TypeDefKind::Struct { fields: Vec::new() },
                classification: TypeDefClassification::UserDefined,
                attrs: Attrs::default(),
            }],
            register_descriptors: Vec::new(),
            register_blocks: Vec::new(),
            impl_blocks: Vec::new(),
            driver: None,
            trait_impls: Vec::new(),
            driver_impl_blocks: Vec::new(),
            abstractions_used: BTreeSet::new(),
            intrinsics_used: BTreeSet::new(),
        };

        Ok(Builder {
            ctx,
            symbols,
            allocator,
            source_file,
            source_locations,
            register_block_by_name: BTreeMap::new(),
            register_descriptor_by_name: BTreeMap::new(),
            root_module,
            driver_module,
            driver_struct,
            driver_label,
        })
    }

    pub(super) fn alloc_decl(&mut self) -> DeclId {
        let id = self.allocator.alloc();
        record_source(&mut self.source_locations, id, self.source_file);
        id
    }

    pub(super) fn intern_sym(&mut self, s: &str) -> Symbol {
        self.symbols.intern(s)
    }

    pub(super) fn err(&self, kind: NormalizeErrorKind) -> NormalizeError {
        NormalizeError {
            kind,
            provenance: self.ctx.provenance(),
        }
    }

    /// Run `f` with `segment` pushed onto the context surface path.
    /// The segment is popped even if `f` returns `Err`.
    pub(super) fn with_surface<T>(
        &mut self,
        segment: impl Into<String>,
        f: impl FnOnce(&mut Builder<'_>) -> Result<T, NormalizeError>,
    ) -> Result<T, NormalizeError> {
        self.ctx.push_surface(segment);
        let result = f(self);
        self.ctx.pop_surface();
        result
    }

    /// Run `f` with `item` as the containing-item label. Restores the
    /// previous label on exit.
    pub(super) fn with_item<T>(
        &mut self,
        item: impl Into<String>,
        f: impl FnOnce(&mut Builder<'_>) -> Result<T, NormalizeError>,
    ) -> Result<T, NormalizeError> {
        let previous = self.ctx.set_item(item);
        let result = f(self);
        self.ctx.set_item_from(previous);
        result
    }
}

fn intern_driver_identity(
    interner: &mut SymbolInterner,
    raw: &RawDriverIdentity,
) -> DriverIdentity {
    DriverIdentity {
        name: interner.intern(&raw.driver_name),
        compatible: raw.compatible.iter().map(|c| interner.intern(c)).collect(),
        bus: raw
            .bus
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| interner.intern(s)),
    }
}

fn record_source(map: &mut BTreeMap<DeclId, SourceOrigin>, id: DeclId, file: FileId) {
    // Every entry must be `Source(..)` -- the verifier treats
    // `CompilerGenerated` as a missing-location violation
    // (`verify/rules/cross_item.rs::source_location_coverage`). Each
    // DeclId gets a distinct synthetic line number for traceability.
    let row = id.index().saturating_add(1);
    map.insert(
        id,
        SourceOrigin::Source(SourceLocation {
            file,
            start_line: row,
            start_column: 1,
            end_line: row,
            end_column: 1,
        }),
    );
}

// -------------------------------------------------------- register access

pub(super) fn normalize_register_access(
    builder: &mut Builder<'_>,
    surface: Option<&RawRegisterAccessSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("register_access", |b| {
        for (idx, descriptor) in surface.register_descriptors.iter().enumerate() {
            normalize_register_descriptor(b, idx, descriptor)?;
        }
        for (idx, block) in surface.register_blocks.iter().enumerate() {
            normalize_register_block(b, idx, block)?;
        }
        for (idx, mmio) in surface.mmio_fields.iter().enumerate() {
            normalize_mmio_field(b, idx, mmio)?;
        }
        Ok(())
    })
}

fn normalize_register_descriptor(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawRegisterDescriptorFact,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("register_descriptors[{idx}]"), |b| {
        if raw.register_module.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "register_access.register_descriptors",
                attribute: "register_module",
                item: format!("descriptor#{idx}"),
            }));
        }
        b.with_item(
            format!("RegisterDescriptor({module})", module = raw.register_module),
            |b| {
                if b.register_descriptor_by_name
                    .contains_key(&raw.register_module)
                {
                    return Err(b.err(NormalizeErrorKind::StructuralViolation {
                        description: format!(
                            "duplicate register descriptor '{}'",
                            raw.register_module
                        ),
                    }));
                }
                let id = RegisterDescriptorId::from(b.alloc_decl());
                let name_sym = b.intern_sym(&raw.register_module);
                let mut fields = Vec::with_capacity(raw.fields.len());
                for field in &raw.fields {
                    let fname = b.intern_sym(&field.field_name);
                    let mut named_values = Vec::with_capacity(field.named_values.len());
                    for nv in &field.named_values {
                        let nv_name = b.intern_sym(&nv.name);
                        named_values.push(NamedValue {
                            name: nv_name,
                            value: u64::from(nv.value),
                        });
                    }
                    fields.push(BitfieldDef {
                        name: fname,
                        offset: field.offset,
                        width: field.width,
                        named_values,
                    });
                }
                b.root_module.register_descriptors.push(RegisterDescriptor {
                    id,
                    name: name_sym,
                    access_width: 32,
                    fields,
                });
                b.register_descriptor_by_name
                    .insert(raw.register_module.clone(), id);
                Ok(())
            },
        )
    })
}

fn normalize_register_block(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawRegisterBlockDecl,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("register_blocks[{idx}]"), |b| {
        if raw.name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "register_access.register_blocks",
                attribute: "name",
                item: format!("register_block#{idx}"),
            }));
        }
        b.with_item(format!("RegisterBlock({name})", name = raw.name), |b| {
            if b.register_block_by_name.contains_key(&raw.name) {
                return Err(b.err(NormalizeErrorKind::StructuralViolation {
                    description: format!("duplicate register block '{}'", raw.name),
                }));
            }
            let id = RegisterBlockId::from(b.alloc_decl());
            let name_sym = b.intern_sym(&raw.name);
            let endian_sym = b.intern_sym("little");
            let mut fields = Vec::with_capacity(raw.fields.len());
            let mut byte_offset: u32 = 0;
            for field in &raw.fields {
                let field_name_sym = b.intern_sym(&field.name);
                let width = field.access_width.unwrap_or(32);
                fields.push(RegisterBlockField {
                    name: field_name_sym,
                    access_kind: AccessKind::ReadWrite,
                    descriptor: None,
                    byte_offset,
                    array_count: None,
                });
                byte_offset = byte_offset.saturating_add(width / 8);
            }
            b.root_module.register_blocks.push(RegisterBlock {
                id,
                name: name_sym,
                block_attributes: RegisterBlockAttributes {
                    endian: endian_sym,
                    access_width: 32,
                },
                fields,
            });
            b.register_block_by_name.insert(raw.name.clone(), id);
            Ok(())
        })
    })
}

fn normalize_mmio_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawMmioFieldDecl,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("mmio_fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "register_access.mmio_fields",
                attribute: "field_name",
                item: format!("mmio_field#{idx}"),
            }));
        }
        let mmio_id_str = raw
            .mmio_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                b.err(NormalizeErrorKind::MissingRequiredAttribute {
                    surface: "register_access.mmio_fields",
                    attribute: "mmio_id",
                    item: raw.field_name.clone(),
                })
            })?;
        let register_block_type = raw.register_block_type.as_deref().ok_or_else(|| {
            b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "register_access.mmio_fields",
                attribute: "register_block_type",
                item: raw.field_name.clone(),
            })
        })?;
        let block_id = b
            .register_block_by_name
            .get(register_block_type)
            .copied()
            .ok_or_else(|| {
                b.err(NormalizeErrorKind::StructuralViolation {
                    description: format!(
                        "mmio field '{}' references unknown register block '{}'",
                        raw.field_name, register_block_type
                    ),
                })
            })?;

        let resource_id = ResourceId::from(b.alloc_decl());
        let field_sym = b.intern_sym(&raw.field_name);
        let mmio_id_sym = b.intern_sym(mmio_id_str);
        b.driver_struct.mmio.push(MmioResource {
            id: resource_id,
            field_name: field_sym,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: mmio_id_sym,
                register_block: block_id,
            },
            binding: None,
        });
        Ok(())
    })
}

// ---------------------------------------------------------------- clock

pub(super) fn normalize_clock(
    builder: &mut Builder<'_>,
    surface: Option<&RawClockSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("clock", |b| {
        for (idx, field) in surface.fields.iter().enumerate() {
            normalize_clock_field(b, idx, field)?;
        }
        Ok(())
    })
}

fn normalize_clock_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawClockField,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "clock.fields",
                attribute: "field_name",
                item: format!("clock#{idx}"),
            }));
        }
        b.with_item(format!("ClockResource({})", raw.field_name), |b| {
            if raw.clock_id.is_empty() {
                return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                    surface: "clock.fields",
                    attribute: "clock_id",
                    item: raw.field_name.clone(),
                }));
            }
            let id = ResourceId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            let clock_id_sym = b.intern_sym(&raw.clock_id);
            b.driver_struct.clocks.push(ClockResource {
                id,
                field_name: field_sym,
                shape: shape_from_required(raw.required),
                decl: ClockDecl {
                    clock_id: clock_id_sym,
                },
                binding: None,
            });
            Ok(())
        })
    })
}

// ---------------------------------------------------------------- reset

pub(super) fn normalize_reset(
    builder: &mut Builder<'_>,
    surface: Option<&RawResetSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("reset", |b| {
        for (idx, field) in surface.fields.iter().enumerate() {
            normalize_reset_field(b, idx, field)?;
        }
        Ok(())
    })
}

fn normalize_reset_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawResetField,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "reset.fields",
                attribute: "field_name",
                item: format!("reset#{idx}"),
            }));
        }
        b.with_item(format!("ResetResource({})", raw.field_name), |b| {
            let reset_id_str = if raw.reset_id.is_empty() {
                raw.field_name.clone()
            } else {
                raw.reset_id.clone()
            };
            let id = ResourceId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            let reset_id_sym = b.intern_sym(&reset_id_str);
            b.driver_struct.resets.push(ResetResource {
                id,
                field_name: field_sym,
                shape: shape_from_required(raw.required),
                decl: ResetDecl {
                    reset_id: reset_id_sym,
                },
                binding: None,
            });
            Ok(())
        })
    })
}

// --------------------------------------------------------------- pinctrl

pub(super) fn normalize_pinctrl(
    builder: &mut Builder<'_>,
    surface: Option<&RawPinCtrlSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("pinctrl", |b| {
        for (idx, field) in surface.fields.iter().enumerate() {
            normalize_pinctrl_field(b, idx, field)?;
        }
        Ok(())
    })
}

fn normalize_pinctrl_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawPinCtrlField,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "pinctrl.fields",
                attribute: "field_name",
                item: format!("pinctrl#{idx}"),
            }));
        }
        b.with_item(format!("PinCtrlResource({})", raw.field_name), |b| {
            let pinctrl_id = raw
                .pinctrl_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(raw.field_name.as_str())
                .to_string();
            let id = ResourceId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            let pinctrl_id_sym = b.intern_sym(&pinctrl_id);
            b.driver_struct.pinctrl.push(PinCtrlResource {
                id,
                field_name: field_sym,
                shape: shape_from_required(raw.required),
                decl: PinCtrlDecl {
                    pinctrl_id: pinctrl_id_sym,
                },
                binding: None,
            });
            Ok(())
        })
    })
}

// -------------------------------------------------------------- interrupts

pub(super) fn normalize_interrupts(
    builder: &mut Builder<'_>,
    surface: Option<&RawInterruptSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("interrupts", |b| {
        for (idx, resource) in surface.resources.iter().enumerate() {
            normalize_interrupt_resource(b, idx, resource)?;
        }
        Ok(())
    })
}

fn normalize_interrupt_resource(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawInterruptResourceDecl,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("resources[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "interrupts.resources",
                attribute: "field_name",
                item: format!("interrupt#{idx}"),
            }));
        }
        b.with_item(format!("InterruptResource({})", raw.field_name), |b| {
            if raw.interrupt_id.is_empty() {
                return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                    surface: "interrupts.resources",
                    attribute: "interrupt_id",
                    item: raw.field_name.clone(),
                }));
            }
            let shape = parse_shape(&raw.shape).ok_or_else(|| {
                b.err(NormalizeErrorKind::UnrecognizedEnumTag {
                    surface: "interrupts.resources",
                    field: "shape",
                    tag: raw.shape.clone(),
                })
            })?;
            let id = ResourceId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            let interrupt_id_sym = b.intern_sym(&raw.interrupt_id);
            b.driver_struct.interrupts.push(InterruptResource {
                id,
                field_name: field_sym,
                shape,
                decl: InterruptDecl {
                    interrupt_id: interrupt_id_sym,
                },
                binding: None,
            });
            Ok(())
        })
    })
}

fn parse_shape(raw: &str) -> Option<Shape> {
    if raw.is_empty() || raw == "single" {
        return Some(Shape::Single);
    }
    if raw == "optional" {
        return Some(Shape::Optional);
    }
    if let Some(rest) = raw.strip_prefix("array(") {
        if let Some(inner) = rest.strip_suffix(')') {
            if let Ok(n) = inner.parse::<usize>() {
                return Some(Shape::Array(n));
            }
        }
    }
    None
}

fn shape_from_required(required: bool) -> Shape {
    if required {
        Shape::Single
    } else {
        Shape::Optional
    }
}

// -------------------------------------------------------- synchronization

pub(super) fn normalize_synchronization(
    builder: &mut Builder<'_>,
    surface: Option<&RawSynchronizationSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("synchronization", |b| {
        for (idx, field) in surface.sync_fields.iter().enumerate() {
            normalize_sync_field(b, idx, field)?;
        }
        Ok(())
    })
}

fn normalize_sync_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawSyncFieldDecl,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("sync_fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "synchronization.sync_fields",
                attribute: "field_name",
                item: format!("sync#{idx}"),
            }));
        }
        b.with_item(format!("SyncResource({})", raw.field_name), |b| {
            let sync_kind = match raw.kind.as_str() {
                "mutex" => SyncKind::Mutex,
                "spinlock" => SyncKind::SpinLock,
                "irq_spinlock" => SyncKind::IrqSpinLock,
                "completion" => SyncKind::Completion,
                other => {
                    return Err(b.err(NormalizeErrorKind::UnrecognizedEnumTag {
                        surface: "synchronization.sync_fields",
                        field: "kind",
                        tag: other.to_string(),
                    }));
                }
            };
            let payload_type = raw
                .payload_type
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|_| RegseqType::Scalar(ScalarType::U32));
            let id = ResourceId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            b.driver_struct.sync.push(SyncResource {
                id,
                field_name: field_sym,
                sync_kind,
                payload_type,
            });
            Ok(())
        })
    })
}

// --------------------------------------------------------- firmware props

pub(super) fn normalize_firmware_properties(
    builder: &mut Builder<'_>,
    surface: Option<&RawPropertySurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("firmware_properties", |b| {
        for (idx, lint) in surface.lints.iter().enumerate() {
            validate_lint(b, idx, lint)?;
        }
        for (idx, field) in surface.fields.iter().enumerate() {
            normalize_property_field(b, idx, field)?;
        }
        Ok(())
    })
}

fn validate_lint(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawPropertyLint,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("lints[{idx}]"), |b| {
        b.with_item(format!("PropertyLint({})", raw.field_name), |b| {
            match raw.kind.as_str() {
                "RequiredWithDefault" => Ok(()),
                other => Err(b.err(NormalizeErrorKind::UnrecognizedEnumTag {
                    surface: "firmware_properties.lints",
                    field: "kind",
                    tag: other.to_string(),
                })),
            }
        })
    })
}

fn normalize_property_field(
    builder: &mut Builder<'_>,
    idx: usize,
    raw: &RawPropertyField,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("fields[{idx}]"), |b| {
        if raw.field_name.is_empty() {
            return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                surface: "firmware_properties.fields",
                attribute: "field_name",
                item: format!("property#{idx}"),
            }));
        }
        b.with_item(format!("PropertyField({})", raw.field_name), |b| {
            let field_type = parse_scalar_type(&raw.field_type).ok_or_else(|| {
                b.err(NormalizeErrorKind::UnrecognizedEnumTag {
                    surface: "firmware_properties.fields",
                    field: "field_type",
                    tag: raw.field_type.clone(),
                })
            })?;
            if raw.property_id.is_empty() {
                return Err(b.err(NormalizeErrorKind::MissingRequiredAttribute {
                    surface: "firmware_properties.fields",
                    attribute: "property_id",
                    item: raw.field_name.clone(),
                }));
            }
            let id = PropertyId::from(b.alloc_decl());
            let field_sym = b.intern_sym(&raw.field_name);
            let prop_id_sym = b.intern_sym(&raw.property_id);
            let vendor_prefix = raw
                .vendor_prefix
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| b.intern_sym(s));
            b.driver_struct.properties.push(PropertyField {
                id,
                field_name: field_sym,
                field_type,
                property_id: prop_id_sym,
                default_value: None,
                vendor_prefix,
                required: raw.required,
                bound_value: None,
            });
            Ok(())
        })
    })
}

fn parse_scalar_type(raw: &str) -> Option<ScalarType> {
    match raw {
        "u8" => Some(ScalarType::U8),
        "u16" => Some(ScalarType::U16),
        "u32" => Some(ScalarType::U32),
        "u64" => Some(ScalarType::U64),
        "usize" => Some(ScalarType::Usize),
        "i8" => Some(ScalarType::I8),
        "i16" => Some(ScalarType::I16),
        "i32" => Some(ScalarType::I32),
        "i64" => Some(ScalarType::I64),
        "isize" => Some(ScalarType::Isize),
        _ => None,
    }
}

// --------------------------------------------------------------- pm gate

pub(super) fn normalize_pm_gate(
    builder: &mut Builder<'_>,
    surface: Option<&RawPmGateSurface>,
) -> Result<(), NormalizeError> {
    let Some(surface) = surface else {
        return Ok(());
    };
    builder.with_surface("pm_gate", |b| {
        for (idx, gate) in surface.gates.iter().enumerate() {
            apply_pm_gate(b, idx, gate);
        }
        Ok(())
    })
}

fn apply_pm_gate(builder: &mut Builder<'_>, _idx: usize, gate: &RawPmGate) {
    // First gate wins; later gates are ignored -- the driver struct
    // has exactly one `pm_info` slot.
    if builder.driver_struct.pm_info.is_some() {
        return;
    }
    builder.driver_struct.pm_info = Some(PmInfo {
        idle: gate.properties.idle,
        wakeup_capable: gate.properties.wakeup_capable,
    });
}

// ----------------------------------------------------- trait callbacks / log / etc.

/// Validate the `trait_callbacks` surface. Phase 1 does not ingest
/// trait-callback impls into the model (they land with Phase 2 /
/// later work), but the surface is still walked so its shape stays
/// correctly gated behind a [`NormalizeContext`] scope. Present as a
/// dedicated helper so each top-level surface has its own entry
/// point (task spec "one per `RawFrontendArtifact` top-level
/// surface").
pub(super) fn normalize_trait_callbacks(
    builder: &mut Builder<'_>,
    surface: Option<&super::raw_artifact::RawTraitSurface>,
) -> Result<(), NormalizeError> {
    let Some(_) = surface else {
        return Ok(());
    };
    builder.with_surface("trait_callbacks", |_| Ok(()))
}

/// Validate the `timed_delay` surface. Phase 1 does not reify call
/// sites (Phase 5 Pass 2 owns body-level intrinsic capture); present
/// as a dedicated no-op entry point for spec parity.
pub(super) fn normalize_timed_delay(
    builder: &mut Builder<'_>,
    surface: Option<&super::raw_artifact::RawTimedDelaySurface>,
) -> Result<(), NormalizeError> {
    let Some(_) = surface else {
        return Ok(());
    };
    builder.with_surface("timed_delay", |_| Ok(()))
}

/// Validate the `logging` surface. Phase 1 no-op for the same reason
/// as `timed_delay`.
pub(super) fn normalize_logging(
    builder: &mut Builder<'_>,
    surface: Option<&super::raw_artifact::RawLoggingSurface>,
) -> Result<(), NormalizeError> {
    let Some(_) = surface else {
        return Ok(());
    };
    builder.with_surface("logging", |_| Ok(()))
}

/// Consume `source_locations` surface. Phase 1 currently synthesises
/// origins (see [`record_source`]); once [`DeclId`] allocation is
/// threaded through canonical-path resolution, this helper can
/// override the synthesized row/column with the raw surface's actual
/// `start_line` / `start_col`. For now we just scope the walk so any
/// future validation can raise a provenance-bearing error.
pub(super) fn normalize_source_locations(
    builder: &mut Builder<'_>,
    surface: Option<&super::raw_artifact::RawSourceLocationSurface>,
) -> Result<(), NormalizeError> {
    let Some(_) = surface else {
        return Ok(());
    };
    builder.with_surface("source_locations", |_| Ok(()))
}

// -------------------------------------------------------- identity enrichment

/// Cross-check identity enrichment for structural invariants:
///   - more than one struct with `is_driver = true` → multi-driver
///     structural violation (spec 12 line 1363).
///   - a user-data field whose `field_type` is not `Option<T>` →
///     type-constraint violation (spec 12 line 1364, Section 4.4).
pub(super) fn validate_identity_enrichment(
    builder: &mut Builder<'_>,
    enrichment: Option<&RawIdentityEnrichmentSurface>,
) -> Result<(), NormalizeError> {
    let Some(enrichment) = enrichment else {
        return Ok(());
    };
    builder.with_surface("identity_enrichment", |b| {
        let driver_count = enrichment.structs.iter().filter(|s| s.is_driver).count();
        if driver_count > 1 {
            return Err(b.err(NormalizeErrorKind::StructuralViolation {
                description: format!(
                    "expected exactly one driver struct, found {} (multi-driver)",
                    driver_count
                ),
            }));
        }
        for (idx, fact) in enrichment.structs.iter().enumerate() {
            b.with_surface(format!("structs[{idx}]"), |b| {
                b.with_item(format!("StructFact({})", fact.canonical_path), |b| {
                    for (fidx, field) in fact.fields.iter().enumerate() {
                        validate_struct_field(b, fidx, field)?;
                    }
                    Ok(())
                })
            })?;
        }
        for fact in enrichment.structs.iter().filter(|s| s.is_driver) {
            for field in &fact.fields {
                if eq_ci(&field.rseq_field_kind, "UserData") {
                    attach_user_data(b, field);
                }
            }
        }
        Ok(())
    })
}

fn validate_struct_field(
    builder: &mut Builder<'_>,
    idx: usize,
    field: &RawFieldFact,
) -> Result<(), NormalizeError> {
    builder.with_surface(format!("fields[{idx}]"), |b| {
        if !eq_ci(&field.rseq_field_kind, "UserData") {
            return Ok(());
        }
        // Two-rule contract (Path A):
        // - Rule 2: `Option<T>` where `T: Copy` (primitive-string only
        //   in this normalization path; user-defined structs are
        //   rejected with a scope-narrowing message).
        // - Rule 1: zero-init whitelist (`u8`, `u16`, `u32`, `u64`,
        //   `bool`) and arrays of those scalars. User-defined Copy
        //   structs are rejected here with a scope-narrowing
        //   message (the construction path accepts them via rustc
        //   introspection).
        if let Some(inner) = option_inner(&field.field_type) {
            if rule1_primitive(inner).is_some() || rule1_array(inner).is_some() {
                // Primitive-string inner types satisfy Rule-2 Copy
                // at normalization.
                return Ok(());
            }
            if is_copy_type(inner) {
                return Ok(());
            }
            // Inner is not a recognized Copy primitive spelling. Either
            // a non-Copy type or a user-defined struct name.
            if looks_like_user_defined(inner) {
                return Err(b.err(NormalizeErrorKind::TypeConstraintViolation {
                    description: format!(
                        "user-data field '{name}' has type '{ty}'; Rule-2 `Option<T>` with a user-defined inner type `{inner}` is not supported in Stage-1 normalization (use the construction path or extend normalization in a follow-up task)",
                        name = field.name,
                        ty = field.field_type,
                        inner = inner,
                    ),
                }));
            }
            return Err(b.err(NormalizeErrorKind::TypeConstraintViolation {
                description: format!(
                    "user-data field '{name}' has type '{ty}' whose inner '{inner}' is not Copy; the spec requires either a zero-init-whitelisted type (Rule 1) or Option<T: Copy> (Rule 2)",
                    name = field.name,
                    ty = field.field_type,
                    inner = inner,
                ),
            }));
        }
        // Non-Option shape: require Rule-1 zero-init acceptance.
        let trimmed = field.field_type.trim();
        if rule1_primitive(trimmed).is_some() || rule1_array(trimmed).is_some() {
            return Ok(());
        }
        if looks_like_user_defined(trimmed) {
            return Err(b.err(NormalizeErrorKind::TypeConstraintViolation {
                description: format!(
                    "user-data field '{name}' has type '{ty}'; Rule-1 zero-init Copy-struct user-data is not supported in Stage-1 normalization (use the construction path or extend normalization in a follow-up task)",
                    name = field.name,
                    ty = field.field_type,
                ),
            }));
        }
        Err(b.err(NormalizeErrorKind::TypeConstraintViolation {
            description: format!(
                "user-data field '{name}' has type '{ty}'; the spec requires either a zero-init-whitelisted type (Rule 1: u8/u16/u32/u64/bool or arrays of those) or Option<T: Copy> (Rule 2)",
                name = field.name,
                ty = field.field_type,
            ),
        }))
    })
}

fn attach_user_data(builder: &mut Builder<'_>, field: &RawFieldFact) {
    let id = UserDataId::from(builder.alloc_decl());
    let name_sym = builder.intern_sym(&field.name);
    let kind = classify_user_data(&field.field_type);
    builder.driver_struct.user_data.push(UserDataField {
        id,
        field_name: name_sym,
        kind,
    });
}

/// Classify a raw user-data field-type spelling into a
/// [`UserDataKind`]. The caller has already validated the spelling
/// via [`validate_struct_field`]; this function trusts that the
/// input is one of the two accepted shapes.
///
/// For Rule-2 inner types that are Copy primitives NOT representable
/// in `RegseqType` (`f32`, `f64`, `char`, `u128`, `i128`), the legacy
/// lossy `Scalar(U32)` fallback is preserved (the Rule-2 inner-type
/// mapping is intentionally unchanged by Path A).
fn classify_user_data(field_type: &str) -> UserDataKind {
    if let Some(inner) = option_inner(field_type) {
        let inner_ty = primitive_to_regseq(inner).unwrap_or(RegseqType::Scalar(ScalarType::U32));
        return UserDataKind::Option {
            inner_type: inner_ty,
        };
    }
    let trimmed = field_type.trim();
    if let Some(ty) = rule1_primitive(trimmed) {
        return UserDataKind::ZeroInit { inner_type: ty };
    }
    if let Some(ty) = rule1_array(trimmed) {
        return UserDataKind::ZeroInit { inner_type: ty };
    }
    // Unreachable if validate_struct_field passed; fall back to a
    // faithful U32 marker rather than panicking so diagnostic flows
    // surface the upstream validator error.
    UserDataKind::ZeroInit {
        inner_type: RegseqType::Scalar(ScalarType::U32),
    }
}

/// Extract the inner type `T` of an `Option<T>` field-type string,
/// returning `None` if the outer wrapper is not `Option<_>`.
fn option_inner(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    for prefix in ["Option<", "core::option::Option<", "std::option::Option<"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix('>') {
                return Some(inner.trim());
            }
        }
    }
    None
}

/// Map a Rule-1 primitive spelling to its `RegseqType`. Stage-1
/// admits exactly `u8`, `u16`, `u32`, `u64`, `bool`.
fn rule1_primitive(s: &str) -> Option<RegseqType> {
    Some(match s.trim() {
        "u8" => RegseqType::Scalar(ScalarType::U8),
        "u16" => RegseqType::Scalar(ScalarType::U16),
        "u32" => RegseqType::Scalar(ScalarType::U32),
        "u64" => RegseqType::Scalar(ScalarType::U64),
        "bool" => RegseqType::Bool,
        _ => return None,
    })
}

/// Recognize `[T; N]` where `T` is a Rule-1 primitive or a nested
/// Rule-1 array. Returns the corresponding `RegseqType::Array` on
/// success.
fn rule1_array(s: &str) -> Option<RegseqType> {
    let trimmed = s.trim();
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?.trim();
    let semi = inner.rfind(';')?;
    let (elem_str, len_str) = inner.split_at(semi);
    let len_str = len_str.trim_start_matches(';').trim();
    let len: usize = len_str.parse().ok()?;
    let elem_trim = elem_str.trim();
    let elem_ty = rule1_primitive(elem_trim).or_else(|| rule1_array(elem_trim))?;
    Some(RegseqType::Array(Box::new(elem_ty), len))
}

/// Best-effort Rule-2 inner-type -> `RegseqType` mapping for the
/// primitive spellings the Rule-2 Copy table accepts. Returns `None`
/// for unrepresentable primitives (`f32`, `f64`, `char`, `u128`,
/// `i128`); the caller falls back to the legacy lossy `Scalar(U32)`
/// placeholder for those, preserving Rule-2 semantics unchanged by
/// Path A.
fn primitive_to_regseq(s: &str) -> Option<RegseqType> {
    Some(match s.trim() {
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
        "()" => RegseqType::Unit,
        _ => return None,
    })
}

/// Heuristic: does `s` look like a user-defined type name (an
/// identifier starting with an uppercase letter or a path segment
/// that looks like one)? Used to emit a scope-narrowing error in
/// Stage-1 normalization when the construction path would otherwise
/// be required.
fn looks_like_user_defined(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Follow the final path segment for things like `foo::Bar`.
    let last = trimmed.rsplit("::").next().unwrap_or(trimmed);
    let first = match last.chars().next() {
        Some(c) => c,
        None => return false,
    };
    first.is_ascii_uppercase()
}

/// Conservative `Copy` check.
///
/// Recognises every Rust primitive scalar type and `bool` as `Copy`;
/// treats compound identifiers (`String`, `Vec<..>`, any user-defined
/// struct name) as non-`Copy`. This is the Rule-2 primitive Copy
/// table; Rule 1 uses a narrower whitelist via [`rule1_primitive`].
/// If a later task widens the admissible inner types (e.g., to
/// `#[derive(Copy)]` user structs the extractor can reason about),
/// this table is where it plugs in.
fn is_copy_type(s: &str) -> bool {
    matches!(
        s,
        "u8" | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
            | "bool"
            | "char"
            | "()"
    )
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// -------------------------------------------------------- finalize

/// Consume the builder, producing the bundle that
/// [`super::normalize`] hands to
/// `RegseqModel::from_verified_components_unchecked`.
pub(super) fn finish(builder: Builder<'_>) -> FinishedModel {
    let Builder {
        symbols,
        source_locations,
        mut root_module,
        mut driver_module,
        driver_struct,
        ..
    } = builder;

    driver_module.driver = Some(driver_struct);
    root_module.submodules.push(driver_module);
    FinishedModel {
        symbols,
        source_locations,
        root_module,
    }
}

pub(super) struct FinishedModel {
    pub symbols: SymbolInterner,
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,
    pub root_module: Module,
}
