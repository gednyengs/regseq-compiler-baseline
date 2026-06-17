use crate::regseq::RegseqError;

/// Accepted driver lifecycle callback contract.
///
/// Every driver must implement `Lifecycle`. The compiler keys on this trait
/// family name during extraction and emits registration/lifecycle glue around
/// the authored callback bodies.
pub trait Lifecycle {
    fn init(&mut self) -> Result<(), RegseqError>;
    fn deinit(&mut self) -> Result<(), RegseqError>;
}
