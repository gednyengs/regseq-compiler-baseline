//! The unified L2 binding error.
//!
//! One variant per construct tenant (see [`crate::L2Binder`]). [`Display`] and
//! [`L2Error::code`] **delegate** to the inner per-construct error, so a tenant's
//! diagnostic text and codes are preserved byte-for-byte at the crate boundary
//! (the production `regseq-driver` callback formats `{err}` directly).

use crate::bus_resource::BrmError;
use crate::clock::ClockL2Error;
use crate::interrupt::InterruptL2Error;
use crate::pinctrl::PinctrlL2Error;

/// An error surfaced by an [`crate::L2Binder`] tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L2Error {
    /// Bus-resource-management (construct #3) surface diagnostic.
    BusResource(BrmError),
    /// Device-interrupt (construct #6) surface diagnostic.
    Interrupt(InterruptL2Error),
    /// Peripheral-clock-management surface diagnostic.
    Clock(ClockL2Error),
    /// Pin-controller-and-GPIO-configuration surface diagnostic.
    Pinctrl(PinctrlL2Error),
}

impl L2Error {
    /// The diagnostic code of the underlying per-construct error (e.g. `BRM-002`).
    pub fn code(&self) -> &'static str {
        match self {
            L2Error::BusResource(e) => e.code(),
            L2Error::Interrupt(e) => e.code(),
            L2Error::Clock(e) => e.code(),
            L2Error::Pinctrl(e) => e.code(),
        }
    }
}

impl std::fmt::Display for L2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Delegate verbatim — the inner error owns its wording (zero churn).
            L2Error::BusResource(e) => write!(f, "{e}"),
            L2Error::Interrupt(e) => write!(f, "{e}"),
            L2Error::Clock(e) => write!(f, "{e}"),
            L2Error::Pinctrl(e) => write!(f, "{e}"),
        }
    }
}

impl From<BrmError> for L2Error {
    fn from(e: BrmError) -> Self {
        L2Error::BusResource(e)
    }
}

impl From<InterruptL2Error> for L2Error {
    fn from(e: InterruptL2Error) -> Self {
        L2Error::Interrupt(e)
    }
}

impl From<ClockL2Error> for L2Error {
    fn from(e: ClockL2Error) -> Self {
        L2Error::Clock(e)
    }
}

impl From<PinctrlL2Error> for L2Error {
    fn from(e: PinctrlL2Error) -> Self {
        L2Error::Pinctrl(e)
    }
}

/// Failure resolving the `[platform]` identity (R5): the raw os token is not a
/// known [`regseq_model::TargetOs`].
///
/// Deliberately **separate** from [`L2Error`] — that enum is one-variant-per
/// construct-tenant with diagnostics delegated to the tenant, whereas platform
/// identity resolution is a crate-level L2 responsibility, not a tenant. It also
/// lets the production callback prefix the message `"platform identity"` rather
/// than `"bus-resource"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformIdentityError {
    /// `[platform] os` is not a recognized `TargetOs` token.
    UnknownOs {
        /// The offending token, verbatim.
        token: String,
        /// The accepted vocabulary (`TargetOs::EXPECTED`).
        expected: &'static str,
    },
}

impl PlatformIdentityError {
    /// The diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            PlatformIdentityError::UnknownOs { .. } => "L2-OS-001",
        }
    }
}

impl std::fmt::Display for PlatformIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformIdentityError::UnknownOs { token, expected } => {
                write!(f, "unknown [platform] os {token:?}; expected one of: {expected}")
            }
        }
    }
}
