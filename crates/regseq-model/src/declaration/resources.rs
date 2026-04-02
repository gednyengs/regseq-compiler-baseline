//! Resource declarations: mmio, interrupts, clock, reset, pinctrl.
//!
//! Each resource sub-family retains its own declarations, optionality,
//! call-site evidence, and provenance. Synchronization is handled in a
//! separate module due to its structural complexity.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Top-level set of all resource declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDeclSet {
    /// MMIO field declarations.
    pub mmio: Vec<MmioFieldDecl>,
    /// Interrupt resource declarations.
    pub interrupts: InterruptDeclSet,
    /// Clock resource declarations.
    pub clock: ClockDeclSet,
    /// Reset line resource declarations.
    pub reset: ResetDeclSet,
    /// Pin controller resource declarations.
    pub pinctrl: PinCtrlDeclSet,
}

// ---- MMIO ----

/// A normalized MMIO field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmioFieldDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// MMIO binding id, if declared.
    pub mmio_id: Option<String>,
    /// Register block type name, if declared.
    pub register_block_type: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
}

// ---- Interrupts ----

/// Set of interrupt declarations and ISR bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptDeclSet {
    /// Interrupt resource field declarations.
    pub resources: Vec<InterruptResourceDecl>,
    /// ISR binding declarations.
    pub isr_bindings: Vec<IsrBindingDecl>,
}

/// A normalized interrupt resource declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptResourceDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Interrupt binding id.
    pub interrupt_id: String,
    /// Shape of the interrupt field.
    pub shape: InterruptFieldShape,
    /// Layer 2 binding hints preserved as an explicit sidecar.
    ///
    /// These are not declaration facts -- they are Layer 2 data that
    /// leaked into the raw artifact. Kept here as binding hints rather
    /// than normalized declaration facts.
    pub binding_hints: InterruptBindingHints,
    /// Provenance.
    pub provenance: Provenance,
}

/// Shape of an interrupt field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterruptFieldShape {
    /// A single interrupt resource.
    Single,
    /// An optional interrupt resource.
    Optional,
    /// An array of interrupt resources with a fixed count.
    Array(u32),
}

impl InterruptFieldShape {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Self {
        if s == "single" {
            Self::Single
        } else if s == "optional" {
            Self::Optional
        } else if let Some(inner) = s.strip_prefix("array(").and_then(|s| s.strip_suffix(')')) {
            if let Ok(n) = inner.parse::<u32>() {
                return Self::Array(n);
            }
            Self::Single
        } else {
            Self::Single
        }
    }
}

/// Layer 2 binding hints carried on interrupt declarations.
///
/// Explicitly separated from declaration facts. These are preserved as
/// opaque sidecar data so they are not lost before the binding IR exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptBindingHints {
    /// Trigger type hint (e.g. `"edge"`, `"level"`).
    pub trigger_type: Option<String>,
    /// Trigger polarity hint (e.g. `"rising"`, `"high"`).
    pub trigger_polarity: Option<String>,
    /// Priority hint.
    pub priority: Option<String>,
    /// Whether the interrupt is shared.
    pub shared: Option<bool>,
}

/// A normalized ISR binding declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsrBindingDecl {
    /// Handler function name.
    pub handler_name: String,
    /// Bind target for the ISR.
    pub bind_target: IsrBindTarget,
    /// Whether the handler takes an index parameter.
    pub has_index_param: bool,
    /// Callback context for the ISR handler.
    pub context: super::callbacks::CallbackContext,
    /// Provenance.
    pub provenance: Provenance,
}

/// ISR bind target identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsrBindTarget {
    /// Auto-bind when the resource type is unique.
    Auto,
    /// Explicitly bound to a named field.
    ExplicitField(String),
}

impl IsrBindTarget {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Self {
        if s == "auto" {
            Self::Auto
        } else {
            Self::ExplicitField(s.to_string())
        }
    }
}

// ---- Clock ----

/// Set of clock resource declarations and call-site evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockDeclSet {
    /// Clock field declarations.
    pub fields: Vec<ClockFieldDecl>,
    /// Clock operation call sites (construct-owned evidence).
    pub call_sites: Vec<ClockCallSiteDecl>,
}

/// A normalized clock field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockFieldDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Clock binding id.
    pub clock_id: String,
    /// Whether the clock resource is required.
    pub required: bool,
    /// Provenance.
    pub provenance: Provenance,
}

/// A normalized clock call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockCallSiteDecl {
    /// Clock operation method.
    pub method: ClockMethod,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed clock operation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClockMethod {
    /// Enable the clock.
    Enable,
    /// Disable the clock.
    Disable,
    /// Query the clock rate.
    GetRate,
}

impl ClockMethod {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "enable" => Some(Self::Enable),
            "disable" => Some(Self::Disable),
            "get_rate" => Some(Self::GetRate),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::GetRate => "get_rate",
        }
    }
}

// ---- Reset ----

/// Set of reset line declarations and call-site evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetDeclSet {
    /// Reset field declarations.
    pub fields: Vec<ResetFieldDecl>,
    /// Reset operation call sites (construct-owned evidence).
    pub call_sites: Vec<ResetCallSiteDecl>,
}

/// A normalized reset field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetFieldDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the reset resource is required.
    pub required: bool,
    /// Provenance.
    pub provenance: Provenance,
}

/// A normalized reset call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetCallSiteDecl {
    /// Reset operation method.
    pub method: ResetMethod,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed reset operation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResetMethod {
    /// Assert the reset line.
    Assert,
    /// Deassert the reset line.
    Deassert,
    /// Toggle the reset line.
    Toggle,
}

impl ResetMethod {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "assert" => Some(Self::Assert),
            "deassert" => Some(Self::Deassert),
            "toggle" => Some(Self::Toggle),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assert => "assert",
            Self::Deassert => "deassert",
            Self::Toggle => "toggle",
        }
    }
}

// ---- PinCtrl ----

/// Set of pin controller declarations and call-site evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinCtrlDeclSet {
    /// PinCtrl field declarations.
    pub fields: Vec<PinCtrlFieldDecl>,
    /// PinCtrl apply call sites (construct-owned evidence).
    pub call_sites: Vec<PinCtrlCallSiteDecl>,
}

/// A normalized pin controller field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinCtrlFieldDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the pinctrl resource is required.
    pub required: bool,
    /// PinCtrl binding id, if declared.
    pub pinctrl_id: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
}

/// A normalized pin controller call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinCtrlCallSiteDecl {
    /// State type applied at this call site.
    pub state_type: String,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}
