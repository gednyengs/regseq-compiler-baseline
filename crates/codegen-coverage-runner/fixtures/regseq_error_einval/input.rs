#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// RE-1 (Phase-2a) -- the RegseqError boundary mapping, error half. A
// self-declared `#[rseq::framework_type("regseq-error")]` enum (D06: identity is
// the attribute, NOT a facade crate) carrying only C02-valid variants, returned
// as the `Err` of a `Result<(), RegseqError>` entry. Returning
// `Err(RegseqError::InvalidParameter)` drives the `regseq_main__regseq_exit`
// boundary adapter's `case` arm, which returns the `EINVAL` errno symbol; the
// entry shim returns that as the process exit status (D07/C04). The enum
// implements no `Display`/`std::error::Error` and uses `core::result::Result`
// (C01/I06/I07). The entry is a plain Rust `fn` (NOT `extern "C"`): a `Result`
// return is not FFI-safe, and the backend names the entry by its sanitized
// emission path.

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    InvalidParameter,
    OutOfMemory,
}

pub fn regseq_main() -> Result<(), RegseqError> {
    Err(RegseqError::InvalidParameter)
}
