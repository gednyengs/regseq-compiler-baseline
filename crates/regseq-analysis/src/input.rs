//! Analysis-facing bootstrap input layer.
//!
//! Pairs a normalized `DeclarationModel` with its corresponding
//! `JoinedArtifact` and provides read-only query helpers that span
//! both inputs. Later lowering work consumes this layer rather than
//! managing the two models independently.
//!
//! Design rules:
//! - read-only: no mutation of the underlying models
//! - rustc-independent: no compiler types
//! - family ownership preserved: no flattening into one untyped bag
//! - no backend-specific policy

use std::collections::BTreeSet;

use regseq_model::binding::BusBinding;
use regseq_model::declaration::bodies::{collect_call_targets, BodyDeclSet, FnBodyDecl};
use regseq_model::declaration::callbacks::{CallbackDeclSet, CallbackFamily, CallbackImplDecl};
use regseq_model::declaration::driver::BusType;
use regseq_model::declaration::operations::{
    DurationCtorSiteDecl, LogCallSiteDecl, LoggingDeclSet, PmGateDecl, PmGateDeclSet,
    TimedDelayCallSiteDecl, TimedDelayDeclSet,
};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyLintDecl};
use regseq_model::declaration::registers::{
    PollCallSiteDecl, RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl,
    WrapperCallSiteDecl,
};
use regseq_model::declaration::resources::ResourceDeclSet;
use regseq_model::declaration::synchronization::{
    CompletionOperationDecl, GuardScopeDecl, LockOperationDecl, SyncFieldDecl,
    SynchronizationDeclSet,
};
use regseq_model::declaration::DeclarationModel;
use regseq_model::joined::{
    ClockJoinEntry, InterruptJoinEntry, JoinOutcome, JoinedArtifact, MmioJoinEntry,
    PinCtrlJoinEntry, PropertyJoinEntry, ResetJoinEntry,
};

/// Read-only analysis input pairing a declaration model with its joined artifact.
///
/// This is the stable entry point for Phase D lowering consumers. It
/// delegates to existing model traversal where adequate and adds
/// cross-model summary helpers where needed.
#[derive(Debug, Clone)]
pub struct AnalysisInput {
    decl: DeclarationModel,
    joined: JoinedArtifact,
    bus_binding: Option<BusBinding>,
}

impl AnalysisInput {
    /// Create a new analysis input from a declaration model, its joined artifact,
    /// and an optional Layer 2 bus binding.
    pub fn new(
        decl: DeclarationModel,
        joined: JoinedArtifact,
        bus_binding: Option<BusBinding>,
    ) -> Self {
        Self {
            decl,
            joined,
            bus_binding,
        }
    }

    /// Borrow the underlying declaration model.
    pub fn declaration(&self) -> &DeclarationModel {
        &self.decl
    }

    /// Borrow the underlying joined artifact.
    pub fn joined(&self) -> &JoinedArtifact {
        &self.joined
    }

    // ----------------------------------------------------------------
    // Bus identity and metadata
    // ----------------------------------------------------------------

    /// Return the Layer 1 authored bus identity, if declared.
    pub fn bus_identity(&self) -> Option<&BusType> {
        self.decl.driver.bus.as_ref()
    }

    /// Return the Layer 2 normalized bus binding, if present.
    pub fn bus_binding(&self) -> Option<&BusBinding> {
        self.bus_binding.as_ref()
    }

    // ----------------------------------------------------------------
    // Callback-family presence
    // ----------------------------------------------------------------

    /// Return the set of callback families present in the declaration model.
    pub fn callback_families_present(&self) -> BTreeSet<CallbackFamily> {
        let mut out = BTreeSet::new();
        if !self.decl.callbacks.lifecycle_impls.is_empty() {
            out.insert(CallbackFamily::Lifecycle);
        }
        if !self.decl.callbacks.serial_impls.is_empty() {
            out.insert(CallbackFamily::Serial);
        }
        if !self.decl.callbacks.serial_console_impls.is_empty() {
            out.insert(CallbackFamily::SerialConsole);
        }
        if !self.decl.callbacks.power_management_impls.is_empty() {
            out.insert(CallbackFamily::PowerManagement);
        }
        out
    }

    /// Return all callback impl declarations (delegates to the declaration model).
    pub fn all_callback_impls(&self) -> Vec<&CallbackImplDecl> {
        self.decl.all_callback_impls()
    }

    /// Borrow the full callback declaration set.
    pub fn callbacks(&self) -> &CallbackDeclSet {
        &self.decl.callbacks
    }

    // ----------------------------------------------------------------
    // Resource/property join outcome queries
    // ----------------------------------------------------------------

    /// Return a summary of property join outcomes.
    pub fn property_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "properties",
            joined_complete: self
                .joined
                .properties_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .properties_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.properties_decl_only.len(),
            binding_only: self.joined.properties_binding_only.len(),
        }
    }

    /// Return a summary of MMIO join outcomes.
    pub fn mmio_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "mmio",
            joined_complete: self
                .joined
                .mmio_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .mmio_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.mmio_decl_only.len(),
            binding_only: self.joined.mmio_binding_only.len(),
        }
    }

    /// Return a summary of interrupt join outcomes.
    pub fn interrupt_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "interrupts",
            joined_complete: self
                .joined
                .interrupts_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .interrupts_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.interrupts_decl_only.len(),
            binding_only: self.joined.interrupts_binding_only.len(),
        }
    }

    /// Return a summary of clock join outcomes.
    pub fn clock_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "clocks",
            joined_complete: self
                .joined
                .clocks_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .clocks_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.clocks_decl_only.len(),
            binding_only: self.joined.clocks_binding_only.len(),
        }
    }

    /// Return a summary of reset join outcomes.
    pub fn reset_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "resets",
            joined_complete: self
                .joined
                .resets_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .resets_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.resets_decl_only.len(),
            binding_only: self.joined.resets_binding_only.len(),
        }
    }

    /// Return a summary of pin controller join outcomes.
    pub fn pinctrl_join_summary(&self) -> ResourceJoinSummary {
        ResourceJoinSummary {
            family: "pinctrl",
            joined_complete: self
                .joined
                .pinctrl_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::Matched)
                .count(),
            joined_incomplete: self
                .joined
                .pinctrl_joined
                .iter()
                .filter(|e| e.outcome == JoinOutcome::MatchedIncomplete)
                .count(),
            decl_only: self.joined.pinctrl_decl_only.len(),
            binding_only: self.joined.pinctrl_binding_only.len(),
        }
    }

    /// Return summaries for all six joinable resource/property families.
    pub fn all_join_summaries(&self) -> Vec<ResourceJoinSummary> {
        vec![
            self.property_join_summary(),
            self.mmio_join_summary(),
            self.interrupt_join_summary(),
            self.clock_join_summary(),
            self.reset_join_summary(),
            self.pinctrl_join_summary(),
        ]
    }

    // ----------------------------------------------------------------
    // Joined entry accessors (per-family)
    // ----------------------------------------------------------------

    /// Return all joined property entries.
    pub fn properties_joined(&self) -> &[PropertyJoinEntry] {
        &self.joined.properties_joined
    }

    /// Return all joined MMIO entries.
    pub fn mmio_joined(&self) -> &[MmioJoinEntry] {
        &self.joined.mmio_joined
    }

    /// Return all joined interrupt entries.
    pub fn interrupts_joined(&self) -> &[InterruptJoinEntry] {
        &self.joined.interrupts_joined
    }

    /// Return all joined clock entries.
    pub fn clocks_joined(&self) -> &[ClockJoinEntry] {
        &self.joined.clocks_joined
    }

    /// Return all joined reset entries.
    pub fn resets_joined(&self) -> &[ResetJoinEntry] {
        &self.joined.resets_joined
    }

    /// Return all joined pin controller entries.
    pub fn pinctrl_joined(&self) -> &[PinCtrlJoinEntry] {
        &self.joined.pinctrl_joined
    }

    // ----------------------------------------------------------------
    // Property lint visibility
    // ----------------------------------------------------------------

    /// Return all property lints from the declaration model.
    pub fn property_lints(&self) -> &[PropertyLintDecl] {
        &self.decl.properties.lints
    }

    /// Borrow the full property declaration set.
    pub fn properties(&self) -> &PropertyDeclSet {
        &self.decl.properties
    }

    // ----------------------------------------------------------------
    // Declaration-only operation-evidence families
    // (no binding-side join path -- consumed directly by Phase D)
    // ----------------------------------------------------------------

    /// Borrow the full resource declaration set.
    pub fn resources(&self) -> &ResourceDeclSet {
        &self.decl.resources
    }

    /// Borrow the full register declaration set.
    pub fn registers(&self) -> &RegisterDeclSet {
        &self.decl.registers
    }

    /// Return all register block declarations.
    pub fn register_blocks(&self) -> &[RegisterBlockDecl] {
        &self.decl.registers.blocks
    }

    /// Return all register descriptor declarations.
    pub fn register_descriptors(&self) -> &[RegisterDescriptorDecl] {
        &self.decl.registers.descriptors
    }

    /// Return all register wrapper call sites.
    pub fn wrapper_call_sites(&self) -> &[WrapperCallSiteDecl] {
        self.decl.wrapper_call_sites()
    }

    /// Return all poll call sites.
    pub fn poll_call_sites(&self) -> &[PollCallSiteDecl] {
        self.decl.poll_call_sites()
    }

    /// Borrow the full synchronization declaration set.
    pub fn synchronization(&self) -> &SynchronizationDeclSet {
        &self.decl.synchronization
    }

    /// Return all synchronization field declarations.
    pub fn sync_fields(&self) -> &[SyncFieldDecl] {
        self.decl.sync_fields()
    }

    /// Return all lock operation declarations.
    pub fn lock_operations(&self) -> &[LockOperationDecl] {
        self.decl.lock_operations()
    }

    /// Return all completion operation declarations.
    pub fn completion_operations(&self) -> &[CompletionOperationDecl] {
        self.decl.completion_operations()
    }

    /// Return all guard scope declarations.
    pub fn guard_scopes(&self) -> &[GuardScopeDecl] {
        self.decl.guard_scopes()
    }

    /// Borrow the full timed-delay declaration set.
    pub fn timed_delay(&self) -> &TimedDelayDeclSet {
        &self.decl.timed_delay
    }

    /// Return all timed-delay call sites.
    pub fn timed_delay_call_sites(&self) -> &[TimedDelayCallSiteDecl] {
        self.decl.timed_delay_call_sites()
    }

    /// Return all duration constructor sites.
    pub fn duration_ctor_sites(&self) -> &[DurationCtorSiteDecl] {
        self.decl.duration_ctor_sites()
    }

    /// Borrow the full logging declaration set.
    pub fn logging(&self) -> &LoggingDeclSet {
        &self.decl.logging
    }

    /// Return all logging call sites.
    pub fn log_call_sites(&self) -> &[LogCallSiteDecl] {
        self.decl.log_call_sites()
    }

    /// Borrow the full PM gate declaration set.
    pub fn pm_gate(&self) -> &PmGateDeclSet {
        &self.decl.pm_gate
    }

    /// Return all PM gate declarations.
    pub fn pm_gates(&self) -> &[PmGateDecl] {
        self.decl.pm_gates()
    }

    // ----------------------------------------------------------------
    // Authored body capture
    // ----------------------------------------------------------------

    /// Borrow the full body declaration set.
    pub fn bodies(&self) -> &BodyDeclSet {
        &self.decl.bodies
    }

    /// Return all callback function bodies.
    pub fn callback_bodies(&self) -> Vec<&FnBodyDecl> {
        self.decl.bodies.callbacks()
    }

    /// Return all helper function bodies.
    pub fn helper_bodies(&self) -> Vec<&FnBodyDecl> {
        self.decl.bodies.helpers()
    }

    /// Return all ISR function bodies.
    pub fn isr_bodies(&self) -> Vec<&FnBodyDecl> {
        self.decl.bodies.isrs()
    }

    /// Return all free function bodies.
    pub fn free_function_bodies(&self) -> Vec<&FnBodyDecl> {
        self.decl.bodies.free_functions()
    }

    /// Look up a function body by name.
    pub fn body_by_name(&self, name: &str) -> Option<&FnBodyDecl> {
        self.decl.bodies.by_name(name)
    }

    /// Compute the set of helper and free-function names transitively
    /// reachable from callbacks and ISRs through call edges.
    pub fn reachable_helper_names(&self) -> BTreeSet<String> {
        let non_callback_names: BTreeSet<String> = self
            .decl
            .bodies
            .functions
            .iter()
            .filter(|f| f.is_helper() || f.is_free_function())
            .map(|f| f.name.clone())
            .collect();

        let mut reachable = BTreeSet::new();
        let mut worklist: Vec<String> = Vec::new();

        // Seed from callbacks and ISRs.
        for f in &self.decl.bodies.functions {
            if f.is_callback() || f.is_isr() {
                for t in collect_call_targets(&f.body) {
                    if non_callback_names.contains(&t) && reachable.insert(t.clone()) {
                        worklist.push(t);
                    }
                }
            }
        }

        // Transitive closure through helpers and free functions.
        while let Some(name) = worklist.pop() {
            if let Some(func) = self.decl.bodies.by_name(&name) {
                for t in collect_call_targets(&func.body) {
                    if non_callback_names.contains(&t) && reachable.insert(t.clone()) {
                        worklist.push(t);
                    }
                }
            }
        }

        reachable
    }
}

/// Per-family summary of join outcomes.
///
/// Counts how many entries are fully joined, partially joined (incomplete),
/// declaration-only, or binding-only for a single resource/property family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceJoinSummary {
    /// Family label (e.g. `"properties"`, `"mmio"`).
    pub family: &'static str,
    /// Number of fully matched (complete) join entries.
    pub joined_complete: usize,
    /// Number of matched-but-incomplete join entries.
    pub joined_incomplete: usize,
    /// Number of declaration-only entries (no matching binding).
    pub decl_only: usize,
    /// Number of binding-only entries (no matching declaration).
    pub binding_only: usize,
}

impl ResourceJoinSummary {
    /// Total entries across all join outcome categories.
    pub fn total(&self) -> usize {
        self.joined_complete + self.joined_incomplete + self.decl_only + self.binding_only
    }

    /// Whether all entries are fully joined with no leftovers.
    pub fn is_fully_joined(&self) -> bool {
        self.joined_incomplete == 0 && self.decl_only == 0 && self.binding_only == 0
    }

    /// Whether any entries are only partially joined.
    pub fn has_incomplete(&self) -> bool {
        self.joined_incomplete > 0
    }

    /// Whether any entries remain unmatched on either side.
    pub fn has_unmatched(&self) -> bool {
        self.decl_only > 0 || self.binding_only > 0
    }
}
