//! The device-interrupt placement facts — the ONE queryable source (R17/D14).
//!
//! Every backend (generic_c, linux_c, zephyr_c) derives its interrupt
//! register/enable/disable/unregister emission positions from this module and
//! nowhere else; the derived order is the C01 grammar (I07). No backend may
//! hard-code a divergent order. The representation is harness-owned (GRILL
//! O2) and deliberately minimal — an ordered emission-point list — so it can
//! change under the trace tests.
//!
//! The facts (GRILL D02/D09/D12/D14, contract C01):
//!
//!   * **Init preamble** — the interrupt registration runs LAST among the
//!     resource preambles, after the bus-resource ops and the `map_region`
//!     keystone (D14), with the line registered DISABLED (D02; linux
//!     `IRQF_NO_AUTOEN`, zephyr `IRQ_CONNECT` without the stock immediate
//!     `irq_enable`). Multiple resources register in declaration order.
//!   * **Enable point** — only after `on_init` returns `Ok` (D02/R05); never
//!     on the `Err` path (I01).
//!   * **`on_init`-Err unwind** — the reverse unwind includes the interrupt
//!     unregister (R16); the line was never enabled, so no disable step.
//!     Resources unwind in reverse declaration order, BEFORE the other
//!     resource postambles (reverse of init, where interrupt came last).
//!   * **Deinit postamble** — `on_deinit` → disable → deregister-slot
//!     (D09/R06), per resource in reverse declaration order, BEFORE the other
//!     resource postambles.

use regseq_model::DriverInterrupts;

/// One interrupt lifecycle operation on one resource (an index into
/// [`DriverInterrupts::resources`] — the backend resolves names/lines/flags
/// from the resource's binding itself; this source owns only the ordering).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqStep {
    /// Register the handler with the line DISABLED.
    RegisterDisabled {
        /// Index into `DriverInterrupts::resources`.
        resource: usize,
    },
    /// Enable the line.
    Enable {
        /// Index into `DriverInterrupts::resources`.
        resource: usize,
    },
    /// Disable the line.
    Disable {
        /// Index into `DriverInterrupts::resources`.
        resource: usize,
    },
    /// Deregister the handler (the deregister SLOT: zephyr_c renders its
    /// structurally-absent disposition here instead of code — C02).
    Unregister {
        /// Index into `DriverInterrupts::resources`.
        resource: usize,
    },
}

/// Where in the synthesized lifecycle a batch of [`IrqStep`]s is emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqPhase {
    /// In the init preamble, AFTER every other resource preamble (bus ops +
    /// `map_region` keystone; D14).
    InitPreambleAfterResources,
    /// Immediately after `on_init` returns `Ok` (D02/R05); absent on `Err`.
    AfterOnInitOk,
    /// On the `on_init`-Err reverse unwind, BEFORE the other resource
    /// postambles (R16).
    InitErrUnwind,
    /// In the deinit postamble, after the `on_deinit` call and BEFORE the
    /// other resource postambles (D09/R06).
    DeinitPostambleBeforeResources,
}

/// The full ordered placement contract for one driver's interrupt surface:
/// every `(phase, step)` in emission order within its phase. Backends filter
/// by the phase whose seam they are emitting; the ORDER WITHIN each phase is
/// normative (I07) and the phases' positions are documented on [`IrqPhase`].
pub fn irq_placement(interrupts: &DriverInterrupts) -> Vec<(IrqPhase, IrqStep)> {
    let n = interrupts.resources.len();
    let mut steps = Vec::with_capacity(n * 5);
    // Init preamble: register each line disabled, declaration order.
    for resource in 0..n {
        steps.push((
            IrqPhase::InitPreambleAfterResources,
            IrqStep::RegisterDisabled { resource },
        ));
    }
    // Post-`on_init`-Ok: enable each line, declaration order.
    for resource in 0..n {
        steps.push((IrqPhase::AfterOnInitOk, IrqStep::Enable { resource }));
    }
    // `on_init`-Err unwind: unregister in reverse declaration order (the
    // line was never enabled — no disable step).
    for resource in (0..n).rev() {
        steps.push((IrqPhase::InitErrUnwind, IrqStep::Unregister { resource }));
    }
    // Deinit postamble: disable then deregister-slot per resource, reverse
    // declaration order.
    for resource in (0..n).rev() {
        steps.push((
            IrqPhase::DeinitPostambleBeforeResources,
            IrqStep::Disable { resource },
        ));
        steps.push((
            IrqPhase::DeinitPostambleBeforeResources,
            IrqStep::Unregister { resource },
        ));
    }
    steps
}

/// The steps of one phase, in emission order — the per-seam query the
/// backends call.
pub fn irq_steps_for(interrupts: &DriverInterrupts, phase: IrqPhase) -> Vec<IrqStep> {
    irq_placement(interrupts)
        .into_iter()
        .filter_map(|(p, s)| (p == phase).then_some(s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::{InterruptForm, InterruptResource, StringId};

    fn interrupts(n: usize) -> DriverInterrupts {
        DriverInterrupts {
            resources: (0..n)
                .map(|i| InterruptResource {
                    name: StringId::from_u32(i as u32),
                    id: StringId::from_u32((10 + i) as u32),
                    form: InterruptForm::Single,
                    binding: Default::default(),
                })
                .collect(),
            handlers: Vec::new(),
        }
    }

    #[test]
    fn single_resource_order_is_the_c01_grammar() {
        // The full contract for the M1 shape: register-disabled (preamble,
        // after resources) -> enable (after on_init Ok) -> unwind unregister
        // -> deinit disable -> deinit unregister.
        let placement = irq_placement(&interrupts(1));
        assert_eq!(
            placement,
            vec![
                (
                    IrqPhase::InitPreambleAfterResources,
                    IrqStep::RegisterDisabled { resource: 0 }
                ),
                (IrqPhase::AfterOnInitOk, IrqStep::Enable { resource: 0 }),
                (IrqPhase::InitErrUnwind, IrqStep::Unregister { resource: 0 }),
                (
                    IrqPhase::DeinitPostambleBeforeResources,
                    IrqStep::Disable { resource: 0 }
                ),
                (
                    IrqPhase::DeinitPostambleBeforeResources,
                    IrqStep::Unregister { resource: 0 }
                ),
            ]
        );
    }

    #[test]
    fn multi_resource_registers_forward_and_tears_down_reverse() {
        let d = interrupts(2);
        assert_eq!(
            irq_steps_for(&d, IrqPhase::InitPreambleAfterResources),
            vec![
                IrqStep::RegisterDisabled { resource: 0 },
                IrqStep::RegisterDisabled { resource: 1 },
            ]
        );
        assert_eq!(
            irq_steps_for(&d, IrqPhase::AfterOnInitOk),
            vec![
                IrqStep::Enable { resource: 0 },
                IrqStep::Enable { resource: 1 }
            ]
        );
        assert_eq!(
            irq_steps_for(&d, IrqPhase::InitErrUnwind),
            vec![
                IrqStep::Unregister { resource: 1 },
                IrqStep::Unregister { resource: 0 },
            ]
        );
        assert_eq!(
            irq_steps_for(&d, IrqPhase::DeinitPostambleBeforeResources),
            vec![
                IrqStep::Disable { resource: 1 },
                IrqStep::Unregister { resource: 1 },
                IrqStep::Disable { resource: 0 },
                IrqStep::Unregister { resource: 0 },
            ]
        );
    }

    #[test]
    fn unwind_has_no_disable_step() {
        // The line is registered disabled and never enabled on the Err path:
        // the unwind carries unregister only (R16/D02).
        let steps = irq_steps_for(&interrupts(1), IrqPhase::InitErrUnwind);
        assert!(steps
            .iter()
            .all(|s| matches!(s, IrqStep::Unregister { .. })));
    }

    #[test]
    fn no_resources_means_no_steps() {
        assert!(irq_placement(&interrupts(0)).is_empty());
    }
}
