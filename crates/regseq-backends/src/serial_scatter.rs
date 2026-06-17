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
}
