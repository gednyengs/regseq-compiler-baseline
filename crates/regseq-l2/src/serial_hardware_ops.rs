//! L2 capability gates for backend-specific serial hardware hooks.

use regseq_config::PlatformBindings;
use regseq_model::RegseqModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerialHardwareOpsL2Error {
    /// The platform enabled break-send but the serial driver did not author the
    /// matching backend capability hook.
    MissingBreakSendHook,
}

impl SerialHardwareOpsL2Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingBreakSendHook => "SERHW-L2-001",
        }
    }
}

impl std::fmt::Display for SerialHardwareOpsL2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBreakSendHook => write!(
                f,
                "SERHW-L2-001: [serial-hardware-ops] break-send = true requires an authored \
                 #[rseq::backend_capability(\"serial-hardware-ops.break-send\")] hook"
            ),
        }
    }
}

pub struct SerialHardwareOpsBinder;

impl crate::L2Binder for SerialHardwareOpsBinder {
    fn name(&self) -> &'static str {
        "serial-hardware-ops"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        if !bindings.serial_hardware_ops.break_send {
            return Ok(());
        }
        let Some(serial) = model.serial_port.drivers.values_mut().next() else {
            return Err(crate::L2Error::SerialHardwareOps(
                SerialHardwareOpsL2Error::MissingBreakSendHook,
            ));
        };
        if serial.hardware_ops.break_send.is_none() {
            return Err(crate::L2Error::SerialHardwareOps(
                SerialHardwareOpsL2Error::MissingBreakSendHook,
            ));
        }
        serial.hardware_ops.break_send_enabled = true;
        Ok(())
    }

    fn validate(&self, _model: &RegseqModel) -> Result<(), crate::L2Error> {
        Ok(())
    }
}
