//! Interpreter context for MIR body interpretation.
//!
//! Provides metadata from the declaration model needed to classify MIR
//! operations: register block field names, MMIO identities, resource
//! fields, and helper function names.

use std::collections::{BTreeMap, BTreeSet};

use regseq_analysis::AnalysisInput;
use regseq_model::declaration::mir_body::MirLocalDecl;

use crate::lowering::plan::ResourceKind;

/// Shared context for MIR interpretation, derived from analysis metadata.
///
/// Built once per `AnalysisInput` and shared across all function
/// interpretations. Contains declaration-grounded metadata for
/// classifying MIR operations.
pub struct MirInterpreterContext {
    /// MMIO field names on the driver struct (e.g. "regs").
    pub mmio_fields: BTreeSet<String>,
    /// Known register field names within MMIO blocks.
    pub register_field_names: BTreeSet<String>,
    /// Per-register-block ordered field name list (index → name).
    pub register_block_fields: Vec<Vec<String>>,
    /// Register block type paths (e.g. "registers::FooRegisters").
    pub register_block_types: BTreeSet<String>,
    /// Optional resource fields with their resource kind.
    pub resource_fields: Vec<(String, ResourceKind)>,
    /// Names of reachable helper functions.
    pub helper_names: BTreeSet<String>,
    /// Driver struct field names in declaration order.
    pub driver_field_names: Vec<String>,
    /// Known register wrapper type path prefixes (e.g. "regseq::ReadWrite").
    pub register_wrapper_types: BTreeSet<String>,
    /// Known resource type path prefixes (e.g. "regseq::Clock").
    pub resource_type_paths: BTreeSet<String>,
}

impl MirInterpreterContext {
    /// Build from an `AnalysisInput`.
    pub fn from_analysis(input: &AnalysisInput) -> Self {
        let mut mmio_fields = BTreeSet::new();
        let mut register_field_names = BTreeSet::new();
        let mut register_block_fields = Vec::new();
        let mut register_block_types = BTreeSet::new();
        let mut resource_fields = Vec::new();
        let mut driver_field_names = Vec::new();

        // MMIO fields.
        for mmio in &input.declaration().resources.mmio {
            mmio_fields.insert(mmio.field_name.clone());
        }

        // Register blocks and their fields.
        for block in input.register_blocks() {
            register_block_types.insert(block.name.clone());
            let field_names: Vec<String> = block.fields.iter().map(|f| f.name.clone()).collect();
            for name in &field_names {
                register_field_names.insert(name.clone());
            }
            register_block_fields.push(field_names);
        }

        // Optional resource fields.
        for clock in &input.declaration().resources.clock.fields {
            resource_fields.push((clock.field_name.clone(), ResourceKind::Clock));
        }
        for reset in &input.declaration().resources.reset.fields {
            resource_fields.push((reset.field_name.clone(), ResourceKind::Reset));
        }
        for pinctrl in &input.declaration().resources.pinctrl.fields {
            resource_fields.push((pinctrl.field_name.clone(), ResourceKind::PinCtrl));
        }

        // Helper function names from MIR reachability.
        let helper_names = input.mir_reachable_helper_names();

        // Driver struct fields.
        for mmio in &input.declaration().resources.mmio {
            driver_field_names.push(mmio.field_name.clone());
        }
        for prop in &input.declaration().properties.fields {
            driver_field_names.push(prop.field_name.clone());
        }
        for irq in &input.declaration().resources.interrupts.resources {
            driver_field_names.push(irq.field_name.clone());
        }
        for clock in &input.declaration().resources.clock.fields {
            driver_field_names.push(clock.field_name.clone());
        }
        for reset in &input.declaration().resources.reset.fields {
            driver_field_names.push(reset.field_name.clone());
        }
        for pinctrl in &input.declaration().resources.pinctrl.fields {
            driver_field_names.push(pinctrl.field_name.clone());
        }

        // Known register wrapper types.
        let register_wrapper_types = ["regseq::ReadWrite", "regseq::ReadOnly", "regseq::WriteOnly"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Known resource type paths.
        let resource_type_paths = ["regseq::Clock", "regseq::ResetLine", "regseq::PinCtrl"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        MirInterpreterContext {
            mmio_fields,
            register_field_names,
            register_block_fields,
            register_block_types,
            resource_fields,
            helper_names,
            driver_field_names,
            register_wrapper_types,
            resource_type_paths,
        }
    }

    /// Check if a type path refers to a register wrapper.
    pub fn is_register_wrapper_type(&self, path: &str) -> bool {
        self.register_wrapper_types
            .iter()
            .any(|prefix| path.starts_with(prefix))
    }

    /// Check if a type path refers to an MMIO wrapper.
    pub fn is_mmio_type(&self, path: &str) -> bool {
        path.starts_with("regseq::Mmio")
    }

    /// Check if a name is a known helper function.
    pub fn is_helper(&self, name: &str) -> bool {
        self.helper_names.contains(name)
    }

    /// Return the resource kind for a field name, if it is a resource.
    pub fn resource_kind(&self, field_name: &str) -> Option<ResourceKind> {
        self.resource_fields
            .iter()
            .find(|(name, _)| name == field_name)
            .map(|(_, kind)| kind.clone())
    }

    /// Look up a register field name by block index and field index.
    pub fn register_field_name(&self, block_idx: usize, field_idx: usize) -> Option<&str> {
        self.register_block_fields
            .get(block_idx)
            .and_then(|fields| fields.get(field_idx))
            .map(|s| s.as_str())
    }
}

/// Per-function context for MIR interpretation.
pub struct FnContext<'a> {
    /// Shared interpreter context.
    pub shared: &'a MirInterpreterContext,
    /// Local variable declarations for this function.
    pub local_decls: &'a [MirLocalDecl],
    /// Number of function arguments (locals 1..=arg_count are params).
    pub arg_count: usize,
    /// Whether this function has a `&mut self` receiver (local 1).
    pub has_self_receiver: bool,
    /// Local name lookup cache.
    pub local_names: BTreeMap<u32, String>,
}

impl<'a> FnContext<'a> {
    /// Build from function metadata and shared context.
    pub fn new(
        shared: &'a MirInterpreterContext,
        local_decls: &'a [MirLocalDecl],
        arg_count: usize,
        receiver: &str,
    ) -> Self {
        let has_self_receiver = receiver == "ref_mut" || receiver == "ref";

        let mut local_names = BTreeMap::new();
        for decl in local_decls {
            if let Some(ref name) = decl.name {
                local_names.insert(decl.index, name.clone());
            }
        }

        FnContext {
            shared,
            local_decls,
            arg_count,
            has_self_receiver,
            local_names,
        }
    }

    /// Get the debug name for a local, if available.
    pub fn local_name(&self, index: u32) -> Option<&str> {
        self.local_names.get(&index).map(|s| s.as_str())
    }
}
