#![feature(register_tool)]
#![register_tool(rseq)]

//! Public DSL facade for Regseq driver authors.
//!
//! Driver specifications depend on this crate to access helper macros such as
//! `register_bitfields!`, resource types, marker traits, and intrinsic stubs.
//!
//! `#[rseq::...]` attributes are compiler-recognized tool attributes registered
//! by the compiler wrapper (`regseq-driver`), not proc macros exported by this
//! facade. The `rseq` attribute namespace is distinct from the `regseq` crate
//! namespace.

mod clock;
pub mod delay;
mod error;
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
pub use reset::ResetLine;
pub use serial::{
    DataBits, FlowControl, Parity, RxChar, Serial, SerialConfig, SerialConsole, SerialErrors,
    StopBits,
};
pub use sync::{Completion, Guard, IrqSpinLock, Mutex, SpinLock};
pub use time::Duration;
