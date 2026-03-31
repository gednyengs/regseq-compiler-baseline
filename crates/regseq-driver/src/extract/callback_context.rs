/// Canonical internal baseline for callback-entry contexts.
///
/// Every Regseq callback surface has an entry context that determines the
/// synchronization environment the handler body runs in:
///
/// - `F` -- framework-managed entry (e.g. `Lifecycle::init`, `Lifecycle::deinit`)
/// - `S` -- subsystem-serialized entry (e.g. `Serial::port_open`)
/// - `H` -- explicit handler-managed entry (e.g. bound ISR handlers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallbackContext {
    /// Framework-managed entry. The framework guarantees safe entry conditions.
    FrameworkManaged,
    /// Subsystem-serialized entry. The subsystem serializes calls so the handler
    /// does not race with other callbacks in the same family.
    SubsystemSerialized,
    /// Explicit handler-managed entry. The handler body is responsible for its
    /// own lock discipline.
    HandlerManaged,
}

impl CallbackContext {
    /// The accepted short label used by diagnostics and snapshots.
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::FrameworkManaged => "F",
            Self::SubsystemSerialized => "S",
            Self::HandlerManaged => "H",
        }
    }

    /// Human-readable name for the context class.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::FrameworkManaged => "framework-managed",
            Self::SubsystemSerialized => "subsystem-serialized",
            Self::HandlerManaged => "handler-managed",
        }
    }

    /// Parse from the accepted short label (`F`, `S`, or `H`).
    pub fn from_short_label(label: &str) -> Option<Self> {
        match label {
            "F" => Some(Self::FrameworkManaged),
            "S" => Some(Self::SubsystemSerialized),
            "H" => Some(Self::HandlerManaged),
            _ => None,
        }
    }
}
