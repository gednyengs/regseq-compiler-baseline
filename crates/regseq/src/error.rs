/// Shared compiler-facing semantic error space for authored fallible Regseq code.
///
/// `RegseqError` is the common error type that fallible Regseq trait callback
/// exits converge on. It stays abstract inside authored DSL flow and is lowered
/// by backends when it escapes a fallible callback boundary.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegseqError {
    InvalidParameter,
    OutOfMemory,
    HardwareError,
    Timeout,
    Unsupported,
    DeviceBusy,
    PermissionDenied,
    DeviceNotFound,
    GenericFailure,
}
