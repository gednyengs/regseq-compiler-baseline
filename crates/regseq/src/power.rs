use crate::RegseqError;

/// Accepted optional power-management callback contract.
///
/// Drivers that declare `#[rseq::pm(...)]` must implement this trait.
/// If the PM attribute is absent, this trait is not required.
pub trait PowerManagement {
    fn suspend(&mut self) -> Result<(), RegseqError>;
    fn resume(&mut self) -> Result<(), RegseqError>;
}
