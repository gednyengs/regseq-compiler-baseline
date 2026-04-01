//! User-facing logging surface.
//!
//! The accepted authored interface is macro-based. Each macro expands into a
//! call to the canonical logging anchor with a `template + LogArg[]` shape.

/// Emit an error-level log message.
#[macro_export]
macro_rules! log_error {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::error(
            $template,
            [$($crate::intrinsics::diagnostic_logging::LogArg::from($arg)),*],
        )
    };
}

/// Emit a warn-level log message.
#[macro_export]
macro_rules! log_warn {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::warn(
            $template,
            [$($crate::intrinsics::diagnostic_logging::LogArg::from($arg)),*],
        )
    };
}

/// Emit an info-level log message.
#[macro_export]
macro_rules! log_info {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::info(
            $template,
            [$($crate::intrinsics::diagnostic_logging::LogArg::from($arg)),*],
        )
    };
}

/// Emit a debug-level log message.
#[macro_export]
macro_rules! log_debug {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::debug(
            $template,
            [$($crate::intrinsics::diagnostic_logging::LogArg::from($arg)),*],
        )
    };
}

pub use log_debug as debug;
pub use log_error as error;
pub use log_info as info;
pub use log_warn as warn;
