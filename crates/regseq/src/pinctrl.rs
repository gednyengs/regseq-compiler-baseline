use crate::RegseqError;

/// Typed pin-controller resource declaration surface.
///
/// `PinCtrl` is the opaque facade type that driver authors use to declare
/// pin-controller fields. The compiler extracts these declarations and
/// joins them with Layer 2 pin-state inventories during lowering.
///
/// `PinCtrl` vs `Option<PinCtrl>` encodes whether pin-controller support
/// is required or optional for the device.
pub struct PinCtrl {
    _opaque: (),
}

/// Compile-time marker trait for pin-state types.
///
/// Concrete pin-state markers are generated from Layer 2 pin-state
/// inventories. This trait is the stable compile-time contract that
/// `PinCtrl::apply` selects on.
pub trait PinState {}

impl PinCtrl {
    /// Apply the pin configuration described by state `S`.
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        unimplemented!("compiler-managed: PinCtrl::apply")
    }
}
