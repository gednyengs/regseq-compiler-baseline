#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// RE-1 (Phase-2a) -- the RegseqError boundary mapping, `?`/match/reclassify
// half. A self-declared `#[rseq::framework_type("regseq-error")]` enum (D06:
// identity is the attribute, NOT a facade crate) carrying only C02-valid
// variants. `regseq_main` weaves authored-internal control flow that lowers as
// ordinary code (R02/I01): a nested helper `probe` returning the SAME
// `Result<(), RegseqError>` is `?`-propagated (the same-error-type `?`
// normalizer, regseq-driver-92; the Ok-continue path is exercised here), then a
// `match` reclassifies `Err(DeviceBusy)` to `Err(Timeout)` before it escapes.
// Only the escaping boundary value (`Err(Timeout)`) is mapped by the
// `regseq_main__regseq_exit` adapter -> the `ETIMEDOUT` errno symbol; the entry
// shim returns that as the process exit status (D07/C04). The `?` is in scope
// for the same-type normalizer (same error type `RegseqError`, non-`needs_drop`
// `Ok = ()`; no `From` conversion). The enum implements no
// `Display`/`std::error::Error` and uses `core::result::Result` (C01/I06/I07).
// The entry is a plain Rust `fn` (NOT `extern "C"`): a `Result` return is not
// FFI-safe, and the backend names the entry by its sanitized emission path.

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    DeviceBusy,
    Timeout,
}

fn probe(code: u8) -> Result<(), RegseqError> {
    if code == 0 {
        Ok(())
    } else {
        Err(RegseqError::DeviceBusy)
    }
}

pub fn regseq_main() -> Result<(), RegseqError> {
    probe(0)?;
    match probe(1) {
        Err(RegseqError::DeviceBusy) => Err(RegseqError::Timeout),
        other => other,
    }
}
