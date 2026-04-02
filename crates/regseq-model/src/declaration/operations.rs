//! Construct-local operation evidence for timed delay, logging, and PM gate.
//!
//! These families carry declaration-side facts that are already present in
//! the raw artifact. Each retains distinct typed sub-families rather than
//! collapsing into one opaque bucket.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

// ---- Timed delay ----

/// Set of timed-delay declaration-side evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedDelayDeclSet {
    /// Timed-delay call-site evidence.
    pub call_sites: Vec<TimedDelayCallSiteDecl>,
    /// Duration constructor call-site evidence.
    pub duration_ctors: Vec<DurationCtorSiteDecl>,
}

/// A normalized timed-delay call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedDelayCallSiteDecl {
    /// Delay intrinsic kind.
    pub intrinsic: DelayIntrinsic,
    /// Delay path kind (canonical vs compatibility).
    pub path_kind: DelayPathKind,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed delay intrinsic kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelayIntrinsic {
    /// Busy-wait delay.
    BusyWait,
    /// Sleep delay.
    Sleep,
}

impl DelayIntrinsic {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "busy_wait" => Some(Self::BusyWait),
            "sleep" => Some(Self::Sleep),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BusyWait => "busy_wait",
            Self::Sleep => "sleep",
        }
    }
}

/// Typed delay path kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelayPathKind {
    /// Canonical delay path.
    Canonical,
    /// Compatibility delay path.
    Compatibility,
}

impl DelayPathKind {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "canonical" => Some(Self::Canonical),
            "compatibility" => Some(Self::Compatibility),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Compatibility => "compatibility",
        }
    }
}

/// A normalized duration-constructor call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationCtorSiteDecl {
    /// Duration constructor kind.
    pub ctor: DurationCtor,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed duration constructor kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DurationCtor {
    /// Microseconds constructor.
    FromUs,
    /// Milliseconds constructor.
    FromMs,
    /// Seconds constructor.
    FromS,
}

impl DurationCtor {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "from_us" => Some(Self::FromUs),
            "from_ms" => Some(Self::FromMs),
            "from_s" => Some(Self::FromS),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FromUs => "from_us",
            Self::FromMs => "from_ms",
            Self::FromS => "from_s",
        }
    }
}

// ---- Logging ----

/// Set of logging declaration-side evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingDeclSet {
    /// Logging call-site evidence.
    pub call_sites: Vec<LogCallSiteDecl>,
}

/// A normalized logging call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogCallSiteDecl {
    /// Log level.
    pub level: LogLevel,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Whether this uses the canonical log path.
    pub canonical: bool,
    /// Log message template string.
    pub template: String,
    /// Number of arguments in this log call.
    pub arg_count: usize,
    /// Argument kinds.
    pub arg_kinds: Vec<LogArgKind>,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    /// Error level.
    Error,
    /// Warning level.
    Warn,
    /// Info level.
    Info,
    /// Debug level.
    Debug,
}

impl LogLevel {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

/// Typed log argument kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogArgKind {
    /// u8 argument.
    U8,
    /// u16 argument.
    U16,
    /// u32 argument.
    U32,
    /// u64 argument.
    U64,
    /// usize argument.
    Usize,
    /// i8 argument.
    I8,
    /// i16 argument.
    I16,
    /// i32 argument.
    I32,
    /// i64 argument.
    I64,
    /// isize argument.
    Isize,
    /// bool argument.
    Bool,
    /// String/str argument.
    Str,
    /// Pointer argument.
    Ptr,
}

impl LogArgKind {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "usize" => Some(Self::Usize),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "isize" => Some(Self::Isize),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "ptr" => Some(Self::Ptr),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Isize => "isize",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Ptr => "ptr",
        }
    }
}

// ---- PM gate ----

/// Set of PM gate declaration-side evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmGateDeclSet {
    /// PM gate entries.
    pub gates: Vec<PmGateDecl>,
}

/// A normalized PM gate declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmGateDecl {
    /// Driver type annotated with `#[rseq::pm(...)]`.
    pub driver_type: String,
    /// PM gate property flags.
    pub properties: PmGateProperties,
    /// Whether the driver type has a `PowerManagement` impl.
    pub has_pm_impl: bool,
    /// Provenance.
    pub provenance: Provenance,
}

/// PM gate property flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmGateProperties {
    /// Whether idle PM is enabled.
    pub idle: bool,
    /// Whether the device is wakeup-capable.
    pub wakeup_capable: bool,
}
