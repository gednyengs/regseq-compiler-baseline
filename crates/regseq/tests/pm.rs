#![feature(register_tool)]
#![register_tool(rseq)]

//! Compilation tests for the `#[rseq::pm(...)]` declaration surface.
//!
//! These tests verify that the accepted PM gate declaration form compiles
//! as an inert tool attribute on driver types. The `PowerManagement` trait
//! remains the callback surface; `#[rseq::pm(...)]` is declarative metadata
//! only.

use regseq::{PowerManagement, RegseqError};

/// PM gate with `idle` and `wakeup_capable`.
#[test]
fn pm_gate_with_idle_and_wakeup_capable() {
    #[rseq::pm(idle = false, wakeup_capable = false)]
    #[allow(dead_code)]
    struct Driver {
        value: u32,
    }

    let _ = Driver { value: 0 };
}

/// PM gate combined with a valid `PowerManagement` impl.
#[test]
fn pm_gate_combined_with_power_management_impl() {
    #[rseq::pm(idle = false, wakeup_capable = false)]
    struct Driver;

    impl PowerManagement for Driver {
        fn suspend(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
    }

    let mut dev = Driver;
    assert!(dev.suspend().is_ok());
    assert!(dev.resume().is_ok());
}

/// The declaration surface does not introduce any new PM wrapper or helper
/// API beyond the existing `PowerManagement` trait. Only the trait and the
/// gate attribute are needed.
#[test]
fn no_pm_wrapper_api_needed() {
    #[rseq::pm(idle = false, wakeup_capable = false)]
    struct Driver;

    impl PowerManagement for Driver {
        fn suspend(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
    }

    // Prove only the trait methods exist -- no wrapper, no registration API.
    fn check_pm<T: PowerManagement>(_d: &mut T) {}
    let mut dev = Driver;
    check_pm(&mut dev);
}

/// `PowerManagement` remains the callback-family surface; `#[rseq::pm(...)]`
/// remains declarative metadata. A type with the gate but without the trait
/// compiles -- enforcement of the pairing is the compiler's job, not the
/// facade's.
#[test]
fn pm_gate_is_declarative_metadata_not_trait_enforcement() {
    #[rseq::pm(idle = false, wakeup_capable = false)]
    #[allow(dead_code)]
    struct GateOnly {
        field: u32,
    }

    // This compiles: the facade layer does not enforce that the PM trait
    // must be present when the gate attribute is used.
    let _ = GateOnly { field: 42 };
}
