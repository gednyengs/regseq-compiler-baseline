//! Traversal-friendly access patterns for the declaration model.
//!
//! These helpers expose construct-local declarations and operation evidence
//! without destroying construct ownership. They return references with
//! family/construct context rather than flattening into one global bag.

use super::callbacks::CallbackImplDecl;
use super::operations::{
    DurationCtorSiteDecl, LogCallSiteDecl, PmGateDecl, TimedDelayCallSiteDecl,
};
use super::registers::{PollCallSiteDecl, WrapperCallSiteDecl};
use super::resources::{
    ClockCallSiteDecl, ClockFieldDecl, InterruptResourceDecl, IsrBindingDecl, MmioFieldDecl,
    PinCtrlCallSiteDecl, PinCtrlFieldDecl, ResetCallSiteDecl, ResetFieldDecl,
};
use super::synchronization::{
    CompletionOperationDecl, GuardScopeDecl, LockOperationDecl, SyncFieldDecl,
};
use super::DeclarationModel;

/// A call-site evidence record with its owning construct family label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyCallSite<'a> {
    /// Construct family that owns this call site (e.g. `"clock"`, `"register_access"`).
    pub family: &'static str,
    /// The function containing this call site.
    pub containing_fn: &'a str,
}

impl DeclarationModel {
    /// Iterate all resource field names across all resource sub-families.
    ///
    /// Returns `(family, field_name)` pairs preserving construct ownership.
    pub fn all_resource_field_names(&self) -> Vec<(&'static str, &str)> {
        let mut out = Vec::new();
        for f in &self.resources.mmio {
            out.push(("mmio", f.field_name.as_str()));
        }
        for f in &self.resources.interrupts.resources {
            out.push(("interrupts", f.field_name.as_str()));
        }
        for f in &self.resources.clock.fields {
            out.push(("clock", f.field_name.as_str()));
        }
        for f in &self.resources.reset.fields {
            out.push(("reset", f.field_name.as_str()));
        }
        for f in &self.resources.pinctrl.fields {
            out.push(("pinctrl", f.field_name.as_str()));
        }
        for f in &self.synchronization.sync_fields {
            out.push(("synchronization", f.field_name.as_str()));
        }
        out
    }

    /// Iterate all call-site evidence across all construct families.
    ///
    /// Each entry carries the owning family label. Construct-local ownership
    /// is preserved; this is a read-only cross-reference.
    pub fn all_call_sites(&self) -> Vec<FamilyCallSite<'_>> {
        let mut out = Vec::new();
        for s in &self.registers.wrapper_call_sites {
            out.push(FamilyCallSite {
                family: "register_access",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.registers.poll_call_sites {
            out.push(FamilyCallSite {
                family: "register_access",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.resources.clock.call_sites {
            out.push(FamilyCallSite {
                family: "clock",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.resources.reset.call_sites {
            out.push(FamilyCallSite {
                family: "reset",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.resources.pinctrl.call_sites {
            out.push(FamilyCallSite {
                family: "pinctrl",
                containing_fn: &s.containing_fn,
            });
        }
        for o in &self.synchronization.lock_operations {
            out.push(FamilyCallSite {
                family: "synchronization",
                containing_fn: &o.containing_fn,
            });
        }
        for o in &self.synchronization.completion_operations {
            out.push(FamilyCallSite {
                family: "synchronization",
                containing_fn: &o.containing_fn,
            });
        }
        for g in &self.synchronization.guard_scopes {
            out.push(FamilyCallSite {
                family: "synchronization",
                containing_fn: &g.containing_fn,
            });
        }
        for s in &self.timed_delay.call_sites {
            out.push(FamilyCallSite {
                family: "timed_delay",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.timed_delay.duration_ctors {
            out.push(FamilyCallSite {
                family: "timed_delay",
                containing_fn: &s.containing_fn,
            });
        }
        for s in &self.logging.call_sites {
            out.push(FamilyCallSite {
                family: "logging",
                containing_fn: &s.containing_fn,
            });
        }
        out
    }

    /// Return all callback impl declarations across all families.
    pub fn all_callback_impls(&self) -> Vec<&CallbackImplDecl> {
        let mut out = Vec::new();
        out.extend(self.callbacks.lifecycle_impls.iter());
        out.extend(self.callbacks.serial_impls.iter());
        out.extend(self.callbacks.serial_console_impls.iter());
        out.extend(self.callbacks.power_management_impls.iter());
        out
    }

    /// Return all register wrapper call sites.
    pub fn wrapper_call_sites(&self) -> &[WrapperCallSiteDecl] {
        &self.registers.wrapper_call_sites
    }

    /// Return all poll call sites.
    pub fn poll_call_sites(&self) -> &[PollCallSiteDecl] {
        &self.registers.poll_call_sites
    }

    /// Return all MMIO field declarations.
    pub fn mmio_fields(&self) -> &[MmioFieldDecl] {
        &self.resources.mmio
    }

    /// Return all interrupt resource declarations.
    pub fn interrupt_resources(&self) -> &[InterruptResourceDecl] {
        &self.resources.interrupts.resources
    }

    /// Return all ISR binding declarations.
    pub fn isr_bindings(&self) -> &[IsrBindingDecl] {
        &self.resources.interrupts.isr_bindings
    }

    /// Return all clock field declarations.
    pub fn clock_fields(&self) -> &[ClockFieldDecl] {
        &self.resources.clock.fields
    }

    /// Return all clock call sites.
    pub fn clock_call_sites(&self) -> &[ClockCallSiteDecl] {
        &self.resources.clock.call_sites
    }

    /// Return all reset field declarations.
    pub fn reset_fields(&self) -> &[ResetFieldDecl] {
        &self.resources.reset.fields
    }

    /// Return all reset call sites.
    pub fn reset_call_sites(&self) -> &[ResetCallSiteDecl] {
        &self.resources.reset.call_sites
    }

    /// Return all pinctrl field declarations.
    pub fn pinctrl_fields(&self) -> &[PinCtrlFieldDecl] {
        &self.resources.pinctrl.fields
    }

    /// Return all pinctrl call sites.
    pub fn pinctrl_call_sites(&self) -> &[PinCtrlCallSiteDecl] {
        &self.resources.pinctrl.call_sites
    }

    /// Return all synchronization field declarations.
    pub fn sync_fields(&self) -> &[SyncFieldDecl] {
        &self.synchronization.sync_fields
    }

    /// Return all lock operation declarations.
    pub fn lock_operations(&self) -> &[LockOperationDecl] {
        &self.synchronization.lock_operations
    }

    /// Return all completion operation declarations.
    pub fn completion_operations(&self) -> &[CompletionOperationDecl] {
        &self.synchronization.completion_operations
    }

    /// Return all guard scope declarations.
    pub fn guard_scopes(&self) -> &[GuardScopeDecl] {
        &self.synchronization.guard_scopes
    }

    /// Return all timed-delay call sites.
    pub fn timed_delay_call_sites(&self) -> &[TimedDelayCallSiteDecl] {
        &self.timed_delay.call_sites
    }

    /// Return all duration constructor sites.
    pub fn duration_ctor_sites(&self) -> &[DurationCtorSiteDecl] {
        &self.timed_delay.duration_ctors
    }

    /// Return all logging call sites.
    pub fn log_call_sites(&self) -> &[LogCallSiteDecl] {
        &self.logging.call_sites
    }

    /// Return all PM gate declarations.
    pub fn pm_gates(&self) -> &[PmGateDecl] {
        &self.pm_gate.gates
    }

    /// Collect all unique `containing_fn` values across the entire model.
    pub fn all_containing_fns(&self) -> Vec<&str> {
        let mut fns: Vec<&str> = self
            .all_call_sites()
            .iter()
            .map(|s| s.containing_fn)
            .collect();
        fns.sort();
        fns.dedup();
        fns
    }
}
