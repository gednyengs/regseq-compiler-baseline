//! The serial interrupt-source placement facts — the ONE queryable source for
//! the per-OS WHEN of each `irq_source_set(source, state)` intent (construct #7,
//! PLAN T05; R04/I01). Every backend derives its serial source arming/disarming
//! sites from this module and nowhere else; the author owns WHICH BITS (the
//! `irq_source_set` match arms), the compiler owns WHEN (these tables).
//!
//! This is the enum-form re-cut of the Track-1 named-hook tables (D03): each
//! site maps to a list of `(SerialIrqSource, IrqSourceState)` intents the
//! backend emits as calls to the author's `irq_source_set` with enum literals.
//! `rx_timeout` couples into `RxData` (Zephyr Z11), so there is no separate
//! source for it.
//!
//! - **generic_c** — the traced harness/synthesized-ISR schedule (the
//!   placement-contract oracle, R09/S05): startup arms RX+error; a TX pump arms
//!   TxReady; the synthesized ISR disarms TxReady once the TX cause is serviced;
//!   teardown disarms RX+error.
//! - **linux_c** — the `uart_ops` callback sites (R04): `start_tx`/`stop_tx`
//!   arm/disarm TxReady; `startup` arms RX+error; `shutdown` masks all;
//!   `stop_rx` disarms RxData.
//! - **zephyr_c** — the `uart_driver_api` `irq_{tx,rx,err}_{enable,disable}`
//!   slot bodies (R04).

use regseq_model::{IrqSourceKind, IrqStateKind};

/// One source-arming intent: arm/disarm a portable interrupt source. The
/// backend emits this as a call to the author's `irq_source_set` with the
/// `(source, state)` enum literals (resolved through the IR variant maps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScatterIntent {
    /// The portable source to arm/disarm.
    pub source: IrqSourceKind,
    /// The target state.
    pub state: IrqStateKind,
}

impl ScatterIntent {
    const fn arm(source: IrqSourceKind) -> Self {
        ScatterIntent {
            source,
            state: IrqStateKind::Armed,
        }
    }
    const fn disarm(source: IrqSourceKind) -> Self {
        ScatterIntent {
            source,
            state: IrqStateKind::Disarmed,
        }
    }
}

// ---------------------------------------------------------------------------
// generic_c: the synthesized harness/ISR schedule (the placement oracle).
// ---------------------------------------------------------------------------

/// The phases of the generic_c serial choreography at which source intents
/// scatter. The harness/synthesized-ISR emits the intents of each phase at that
/// phase; `expected.stdout` (T07) pins the resulting interleaving (R09).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericSerialPhase {
    /// After the port is enabled + line-formatted: arm RX + line-error.
    Startup,
    /// Before pumping a TX byte: arm TxReady.
    TxKick,
    /// Inside the synthesized ISR, having decoded a `tx_ready` cause: disarm
    /// TxReady (the M1 TX pump has nothing more to send).
    IsrTxDrained,
    /// Before the port is disabled: disarm RX + line-error.
    Teardown,
}

/// The source intents that scatter at a generic_c choreography phase (R04).
pub fn generic_scatter(phase: GenericSerialPhase) -> Vec<ScatterIntent> {
    match phase {
        GenericSerialPhase::Startup => vec![
            ScatterIntent::arm(IrqSourceKind::RxData),
            ScatterIntent::arm(IrqSourceKind::LineError),
        ],
        GenericSerialPhase::TxKick => vec![ScatterIntent::arm(IrqSourceKind::TxReady)],
        GenericSerialPhase::IsrTxDrained => {
            vec![ScatterIntent::disarm(IrqSourceKind::TxReady)]
        }
        GenericSerialPhase::Teardown => vec![
            ScatterIntent::disarm(IrqSourceKind::RxData),
            ScatterIntent::disarm(IrqSourceKind::LineError),
        ],
    }
}

// ---------------------------------------------------------------------------
// linux_c: the `uart_ops` callback sites (M2).
// ---------------------------------------------------------------------------

/// The Linux `uart_ops` callbacks that carry serial source arming (R04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxSerialSite {
    /// `start_tx` — the core wants to transmit.
    StartTx,
    /// `stop_tx` — the core has no more to transmit.
    StopTx,
    /// `startup` — open the port for IO.
    Startup,
    /// `shutdown` — close the port.
    Shutdown,
    /// `stop_rx` — the core wants RX quiesced (shutdown path).
    StopRx,
}

/// The source intents that scatter at a Linux `uart_ops` callback (R04).
pub fn linux_scatter(site: LinuxSerialSite) -> Vec<ScatterIntent> {
    match site {
        LinuxSerialSite::StartTx => vec![ScatterIntent::arm(IrqSourceKind::TxReady)],
        LinuxSerialSite::StopTx => vec![ScatterIntent::disarm(IrqSourceKind::TxReady)],
        LinuxSerialSite::Startup => vec![
            ScatterIntent::arm(IrqSourceKind::RxData),
            ScatterIntent::arm(IrqSourceKind::LineError),
        ],
        LinuxSerialSite::Shutdown => vec![
            ScatterIntent::disarm(IrqSourceKind::TxReady),
            ScatterIntent::disarm(IrqSourceKind::RxData),
            ScatterIntent::disarm(IrqSourceKind::LineError),
        ],
        LinuxSerialSite::StopRx => vec![ScatterIntent::disarm(IrqSourceKind::RxData)],
    }
}

// ---------------------------------------------------------------------------
// zephyr_c: the `uart_driver_api` irq slot bodies (M3).
// ---------------------------------------------------------------------------

/// The Zephyr `uart_driver_api` interrupt slot bodies that carry serial source
/// arming (R04). Each slot's body is exactly one source intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZephyrSerialSite {
    /// `irq_tx_enable`.
    IrqTxEnable,
    /// `irq_tx_disable`.
    IrqTxDisable,
    /// `irq_rx_enable`.
    IrqRxEnable,
    /// `irq_rx_disable`.
    IrqRxDisable,
    /// `irq_err_enable`.
    IrqErrEnable,
    /// `irq_err_disable`.
    IrqErrDisable,
}

/// The source intent that scatters into a Zephyr irq slot body (R04).
pub fn zephyr_scatter(site: ZephyrSerialSite) -> Vec<ScatterIntent> {
    match site {
        ZephyrSerialSite::IrqTxEnable => vec![ScatterIntent::arm(IrqSourceKind::TxReady)],
        ZephyrSerialSite::IrqTxDisable => vec![ScatterIntent::disarm(IrqSourceKind::TxReady)],
        ZephyrSerialSite::IrqRxEnable => vec![ScatterIntent::arm(IrqSourceKind::RxData)],
        ZephyrSerialSite::IrqRxDisable => vec![ScatterIntent::disarm(IrqSourceKind::RxData)],
        ZephyrSerialSite::IrqErrEnable => vec![ScatterIntent::arm(IrqSourceKind::LineError)],
        ZephyrSerialSite::IrqErrDisable => vec![ScatterIntent::disarm(IrqSourceKind::LineError)],
    }
}

// ---------------------------------------------------------------------------
// C02 source-ordering enforcement (T13 / O2 / G-E1).
// ---------------------------------------------------------------------------
//
// A typed, backend-agnostic gate over each backend's bring-up/teardown
// *schedule* — a lifecycle linearization whose `Arm`/`Disarm` intents are pulled
// from the scatter tables above (so the WHICH-source can't drift from the
// scaffold), with the `IcrClear`/`TeardownQuiesce`/`PortDisable` lifecycle anchors
// authored against the documented `on_init`/teardown contract (NOT mechanically
// derived from emission — the oracle trace-diff in `serial_pl011_e2e` is the
// behavioral backstop that the EMISSION matches this order). It machine-checks the
// two C02 **interrupt/source-ordering** edges that bear on the scatter:
//
//   edge 3 — clear-pending-before-unmask: every `Arm` is preceded by `IcrClear`
//            (else a stale latched cause fires the instant the source unmasks);
//   edge 4 — teardown disarms before/with disable: at every `PortDisable` no
//            source is left armed (else an interrupt fires after teardown).
//
// The configure-internal §3.3.8 wait-FR.BUSY + FEN-clear is a **C03** concern,
// verified by the configure-bracket assertions (T11) — NOT here (no
// double-ownership). `verify_c02` is called at serial emit time (build-failing)
// and exercised by the tests below incl. the negative-fixture rejection.

/// A lifecycle-ordered event in a backend's serial schedule. The `Arm`/`Disarm`
/// intents are pulled from the scatter tables above; `IcrClear`/`TeardownQuiesce`/
/// `PortDisable` are the lifecycle anchors the WHEN sites are ordered against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialOrderEvent {
    /// Latched interrupts cleared (W1C ICR) — the author's `on_init`, invoked by
    /// the scaffold before any source is armed.
    IcrClear,
    /// A source is armed at a compiler-emitted WHEN site (`irq_source_set(_,
    /// Armed)`) — pulled from the scatter tables above.
    Arm(IrqSourceKind),
    /// A source is disarmed at a compiler-emitted WHEN site (`irq_source_set(_,
    /// Disarmed)`) — e.g. Linux `stop_tx`/`stop_rx`/`shutdown` scatter.
    Disarm(IrqSourceKind),
    /// The authored teardown quiesce (`on_deinit` mask-all, IMSC=0) that the deinit
    /// scaffold INVOKES before the port disable — it disarms every source. Modeled
    /// DISTINCTLY from `Disarm` because the disarm itself is AUTHORED, not compiler
    /// scatter: the compiler-owned guarantee is only the ORDERING (quiesce before
    /// disable); that the quiesce actually masks all is confirmed behaviorally by
    /// the oracle teardown trace, NOT by this static gate (Zephyr teardown path).
    TeardownQuiesce,
    /// The port is taken out of service (CR cleared) — teardown.
    PortDisable,
}

/// A C02 partial-order violation found in a serial schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum C02Violation {
    /// A source was armed before latched interrupts were cleared (edge 3).
    ArmBeforeIcrClear(IrqSourceKind),
    /// The port was disabled with one or more sources still armed (edge 4).
    DisableWithArmedSources,
}

impl core::fmt::Display for C02Violation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            C02Violation::ArmBeforeIcrClear(s) => {
                write!(f, "C02 edge 3: {s:?} armed before ICR-clear (clear-pending-before-unmask)")
            }
            C02Violation::DisableWithArmedSources => {
                write!(f, "C02 edge 4: port disabled with sources still armed (disarm-before-disable)")
            }
        }
    }
}

fn intent_event(it: ScatterIntent) -> SerialOrderEvent {
    match it.state {
        IrqStateKind::Armed => SerialOrderEvent::Arm(it.source),
        IrqStateKind::Disarmed => SerialOrderEvent::Disarm(it.source),
    }
}

/// Verify the C02 interrupt/source-ordering edges over a lifecycle schedule
/// (R11 / O2 / G-E1). Backend-agnostic: every serial backend's schedule must
/// pass. Edge 3 — every `Arm` is preceded by an `IcrClear`; edge 4 — at every
/// `PortDisable` no source remains armed.
pub fn verify_c02(schedule: &[SerialOrderEvent]) -> Result<(), C02Violation> {
    let mut icr_cleared = false;
    // Indexed by `IrqSourceKind as usize` (TxReady=0, RxData=1, LineError=2).
    let mut armed = [false; 3];
    for ev in schedule {
        match ev {
            SerialOrderEvent::IcrClear => icr_cleared = true,
            SerialOrderEvent::Arm(s) => {
                if !icr_cleared {
                    return Err(C02Violation::ArmBeforeIcrClear(*s));
                }
                armed[*s as usize] = true;
            }
            SerialOrderEvent::Disarm(s) => armed[*s as usize] = false,
            // The authored on_deinit mask-all disarms every source (Zephyr teardown).
            SerialOrderEvent::TeardownQuiesce => armed = [false; 3],
            SerialOrderEvent::PortDisable => {
                if armed.iter().any(|&a| a) {
                    return Err(C02Violation::DisableWithArmedSources);
                }
            }
        }
    }
    Ok(())
}

/// The generic_c (oracle) serial C02 schedule — the `body_emit` scenario:
/// `on_init` (ICR clear) → Startup/TxKick/IsrTxDrained/Teardown scatter → port
/// disable. The teardown disarm is COMPILER scatter (the `Teardown` phase), so no
/// `TeardownQuiesce` anchor is needed.
pub fn generic_c02_schedule() -> Vec<SerialOrderEvent> {
    let mut s = vec![SerialOrderEvent::IcrClear];
    for ph in [
        GenericSerialPhase::Startup,
        GenericSerialPhase::TxKick,
        GenericSerialPhase::IsrTxDrained,
        GenericSerialPhase::Teardown,
    ] {
        s.extend(generic_scatter(ph).into_iter().map(intent_event));
    }
    s.push(SerialOrderEvent::PortDisable);
    s
}

/// The linux_c serial C02 schedule — `on_init` (ICR clear) → `startup` (arm
/// RX/error) → `start_tx`/`stop_tx`/`stop_rx` (runtime) → `shutdown` (disarm all)
/// → port disable. The teardown disarm is COMPILER scatter (`shutdown` disarms
/// every source), so no `TeardownQuiesce` anchor is needed.
pub fn linux_c02_schedule() -> Vec<SerialOrderEvent> {
    let mut s = vec![SerialOrderEvent::IcrClear];
    for site in [
        LinuxSerialSite::Startup,
        LinuxSerialSite::StartTx,
        LinuxSerialSite::StopTx,
        LinuxSerialSite::StopRx,
        LinuxSerialSite::Shutdown,
    ] {
        s.extend(linux_scatter(site).into_iter().map(intent_event));
    }
    s.push(SerialOrderEvent::PortDisable);
    s
}

/// The zephyr_c serial C02 schedule — `on_init` (ICR clear) → the fine
/// `irq_{tx,rx,err}_enable` arms (worst case: left armed at teardown) →
/// `TeardownQuiesce` (the authored `on_deinit` mask-all the deinit scaffold orders
/// before the disable) → port disable. Unlike Linux/generic, Zephyr's teardown
/// disarm is NOT compiler scatter — it is the authored quiesce, modeled explicitly
/// (its mask content is confirmed by the oracle trace, not this static gate).
pub fn zephyr_c02_schedule() -> Vec<SerialOrderEvent> {
    let mut s = vec![SerialOrderEvent::IcrClear];
    for site in [
        ZephyrSerialSite::IrqTxEnable,
        ZephyrSerialSite::IrqRxEnable,
        ZephyrSerialSite::IrqErrEnable,
    ] {
        s.extend(zephyr_scatter(site).into_iter().map(intent_event));
    }
    s.push(SerialOrderEvent::TeardownQuiesce);
    s.push(SerialOrderEvent::PortDisable);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(i: &ScatterIntent) -> (IrqSourceKind, IrqStateKind) {
        (i.source, i.state)
    }

    #[test]
    fn generic_schedule_arms_rx_and_error_at_startup_only() {
        let startup: Vec<_> = generic_scatter(GenericSerialPhase::Startup)
            .iter()
            .map(pair)
            .collect();
        assert_eq!(
            startup,
            vec![
                (IrqSourceKind::RxData, IrqStateKind::Armed),
                (IrqSourceKind::LineError, IrqStateKind::Armed),
            ]
        );
        // TxReady is never armed at startup — only at the TX kick (the per-source
        // distinction the golden pins, R04/I01).
        assert!(!startup
            .iter()
            .any(|(s, _)| *s == IrqSourceKind::TxReady));
    }

    #[test]
    fn generic_tx_kick_arms_then_isr_disarms_txready() {
        assert_eq!(
            generic_scatter(GenericSerialPhase::TxKick),
            vec![ScatterIntent::arm(IrqSourceKind::TxReady)]
        );
        assert_eq!(
            generic_scatter(GenericSerialPhase::IsrTxDrained),
            vec![ScatterIntent::disarm(IrqSourceKind::TxReady)]
        );
    }

    #[test]
    fn generic_teardown_disarms_rx_and_error() {
        let td: Vec<_> = generic_scatter(GenericSerialPhase::Teardown)
            .iter()
            .map(pair)
            .collect();
        assert_eq!(
            td,
            vec![
                (IrqSourceKind::RxData, IrqStateKind::Disarmed),
                (IrqSourceKind::LineError, IrqStateKind::Disarmed),
            ]
        );
    }

    #[test]
    fn linux_start_stop_tx_arm_disarm_txready() {
        assert_eq!(
            linux_scatter(LinuxSerialSite::StartTx),
            vec![ScatterIntent::arm(IrqSourceKind::TxReady)]
        );
        assert_eq!(
            linux_scatter(LinuxSerialSite::StopTx),
            vec![ScatterIntent::disarm(IrqSourceKind::TxReady)]
        );
    }

    #[test]
    fn linux_startup_arms_rx_and_error_and_shutdown_masks_all() {
        assert_eq!(linux_scatter(LinuxSerialSite::Startup).len(), 2);
        assert_eq!(linux_scatter(LinuxSerialSite::Shutdown).len(), 3);
    }

    #[test]
    fn zephyr_slots_are_one_intent_each() {
        for site in [
            ZephyrSerialSite::IrqTxEnable,
            ZephyrSerialSite::IrqTxDisable,
            ZephyrSerialSite::IrqRxEnable,
            ZephyrSerialSite::IrqRxDisable,
            ZephyrSerialSite::IrqErrEnable,
            ZephyrSerialSite::IrqErrDisable,
        ] {
            assert_eq!(zephyr_scatter(site).len(), 1, "{site:?} is one intent");
        }
        assert_eq!(
            zephyr_scatter(ZephyrSerialSite::IrqRxEnable),
            vec![ScatterIntent::arm(IrqSourceKind::RxData)]
        );
    }

    // -- C02 source-ordering gate (T13 / O2 / G-E1) --------------------------

    #[test]
    fn c02_accepts_all_backend_schedules() {
        verify_c02(&generic_c02_schedule()).expect("generic_c schedule satisfies C02");
        verify_c02(&linux_c02_schedule()).expect("linux_c schedule satisfies C02");
        verify_c02(&zephyr_c02_schedule()).expect("zephyr_c schedule satisfies C02");
    }

    #[test]
    fn c02_each_schedule_clears_icr_before_every_arm() {
        // Edge 3, stated positively: in every backend schedule the first event is
        // the ICR-clear, so no arm can precede it.
        for sched in [generic_c02_schedule(), linux_c02_schedule(), zephyr_c02_schedule()] {
            assert_eq!(
                sched.first(),
                Some(&SerialOrderEvent::IcrClear),
                "ICR-clear must lead every serial schedule"
            );
        }
    }

    #[test]
    fn c02_rejects_arm_before_icr_clear() {
        // Negative fixture: a deliberately mis-ordered arm (before ICR-clear).
        let bad = vec![
            SerialOrderEvent::Arm(IrqSourceKind::RxData),
            SerialOrderEvent::IcrClear,
            SerialOrderEvent::Disarm(IrqSourceKind::RxData),
            SerialOrderEvent::PortDisable,
        ];
        assert_eq!(
            verify_c02(&bad),
            Err(C02Violation::ArmBeforeIcrClear(IrqSourceKind::RxData)),
            "an arm before ICR-clear must be rejected (edge 3)"
        );
    }

    #[test]
    fn c02_rejects_disable_with_armed_source() {
        // Negative fixture: teardown disables the port with TxReady still armed.
        let bad = vec![
            SerialOrderEvent::IcrClear,
            SerialOrderEvent::Arm(IrqSourceKind::TxReady),
            SerialOrderEvent::PortDisable,
        ];
        assert_eq!(
            verify_c02(&bad),
            Err(C02Violation::DisableWithArmedSources),
            "disabling the port with a source still armed must be rejected (edge 4)"
        );
    }

    #[test]
    fn c02_dropping_a_teardown_disarm_is_caught() {
        // Negative fixture rooted in a REAL schedule: take the linux schedule and
        // drop the shutdown disarms — the port-disable then trips edge 4.
        let mut sched = vec![SerialOrderEvent::IcrClear];
        sched.extend(linux_scatter(LinuxSerialSite::Startup).into_iter().map(intent_event));
        // (no stop_tx/stop_rx/shutdown disarms) → RX + LineError stay armed.
        sched.push(SerialOrderEvent::PortDisable);
        assert_eq!(verify_c02(&sched), Err(C02Violation::DisableWithArmedSources));
    }

    #[test]
    fn c02_teardown_quiesce_disarms_all_before_disable() {
        // The authored on_deinit mask-all (Zephyr teardown) clears every source.
        let ok = vec![
            SerialOrderEvent::IcrClear,
            SerialOrderEvent::Arm(IrqSourceKind::TxReady),
            SerialOrderEvent::Arm(IrqSourceKind::RxData),
            SerialOrderEvent::Arm(IrqSourceKind::LineError),
            SerialOrderEvent::TeardownQuiesce,
            SerialOrderEvent::PortDisable,
        ];
        assert_eq!(verify_c02(&ok), Ok(()));
        // ...and without it, the still-armed sources trip edge 4 (the Zephyr
        // teardown's safety rests on the authored quiesce being present).
        let bad = vec![
            SerialOrderEvent::IcrClear,
            SerialOrderEvent::Arm(IrqSourceKind::RxData),
            SerialOrderEvent::PortDisable,
        ];
        assert_eq!(verify_c02(&bad), Err(C02Violation::DisableWithArmedSources));
    }
}
