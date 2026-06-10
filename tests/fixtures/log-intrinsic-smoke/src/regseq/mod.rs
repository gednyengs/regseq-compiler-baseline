//! Public DSL facade for Regseq driver authors.
//!
//! Driver specifications embed this module subtree as `crate::regseq` to access
//! helper macros such as `register_bitfields!`, resource types, marker traits,
//! and intrinsic stubs.
//!
//! `#[rseq::...]` attributes are compiler-recognized tool attributes registered
//! by the compiler wrapper (`regseq-driver`), not proc macros exported by this
//! module. The `rseq` attribute namespace is distinct from the `regseq` module
//! namespace; the spec crate is responsible for declaring
//! `#![feature(register_tool)]` and `#![register_tool(rseq)]` at its crate root.

mod clock;
pub mod delay;
mod error;
pub mod internal;
pub mod intrinsics;
mod irq;
mod lifecycle;
pub mod log;
mod mmio;
mod pinctrl;
pub mod poll;
mod power;
mod registers;
mod reset;
mod serial;
pub mod sync;
mod time;

pub use clock::Clock;
pub use error::RegseqError;
pub use irq::{Interrupt, IrqResult};
pub use lifecycle::Lifecycle;
pub use mmio::Mmio;
pub use pinctrl::{PinCtrl, PinState};
pub use power::PowerManagement;
pub use registers::{
    Field, FieldValue, ReadAccess, ReadOnly, ReadOnlyAccess, ReadWrite, ReadWriteAccess, RegWidth,
    RegisterId, RegisterOf, Reserved, WriteAccess, WriteOnly, WriteOnlyAccess,
};
pub use regseq_macros::ordered;
pub use regseq_macros::register_block;
pub use reset::ResetLine;
pub use serial::{
    DataBits, FlowControl, Parity, RxChar, Serial, SerialConfig, SerialConsole, SerialErrors,
    StopBits,
};
pub use sync::{Completion, Guard, IrqSpinLock, Mutex, SpinLock};
pub use time::Duration;

// `#[macro_export]` exports the log macros at the crate root. Re-export
// them inside this module as plumbing for the `log` submodule's aliases —
// the canonical authored spelling is `crate::regseq::log::{debug, info,
// warn, error}!(...)`.
pub use crate::{log_debug, log_error, log_info, log_warn};
