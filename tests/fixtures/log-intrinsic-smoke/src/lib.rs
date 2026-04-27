//! Log intrinsic format-kind coverage fixture.
//!
//! Each method below issues exactly one `log_info!` call whose template
//! exercises a single `FormatKind` variant from
//! `regseq_model::ir::FormatKind`. The test in
//! `crates/regseq-driver/tests/construction_log_metadata.rs` translates
//! every method body and asserts that Rule 6 metadata extraction
//! produces the expected placeholders.
//!
//! `FormatKind::Pointer` (`{:p}`) is intentionally absent: the
//! Regseq IR does not currently model raw pointer types
//! (`crates/regseq-driver/src/construction/mir_mapping.rs` rejects
//! `ty::TyKind::RawPtr` via `ConstructionError::UnsupportedRustType`,
//! a constraint locked in by the existing
//! `map_ty_errors_on_raw_pointer` test). Adding raw pointer support
//! is out of scope for task regseq-model-50 -- it would require a new
//! `RegseqType` variant and updates to every Pass 2 mapping helper.
//! The `Pointer` parser arm is exercised by the unit test
//! `parse_template_all_format_kinds` in
//! `crates/regseq-driver/src/construction/log_metadata.rs`. End-to-end
//! `{:p}` coverage will land alongside raw-pointer support in a
//! later task.
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq;

#[rseq::driver(name = "log-intrinsic-smoke")]
pub struct LogSmoke {}

impl LogSmoke {
    pub fn log_display(value: u32) {
        crate::regseq::log_info!("display: {}", value);
    }

    pub fn log_lower_hex(value: u32) {
        crate::regseq::log_info!("lower hex: {:x}", value);
    }

    pub fn log_upper_hex(value: u32) {
        crate::regseq::log_info!("upper hex: {:X}", value);
    }

    pub fn log_lower_hex_prefixed(value: u32) {
        crate::regseq::log_info!("lower prefix: {:#x}", value);
    }

    pub fn log_upper_hex_prefixed(value: u32) {
        crate::regseq::log_info!("upper prefix: {:#X}", value);
    }

    pub fn log_padded_hex(value: u32) {
        crate::regseq::log_info!("padded: {:08x}", value);
    }

    pub fn log_padded_prefix_hex(value: u32) {
        crate::regseq::log_info!("padded prefix: {:#010x}", value);
    }

    pub fn log_two_args(value: u32) {
        crate::regseq::log_info!("two: {} {:#x}", value, value);
    }

    pub fn log_no_args() {
        crate::regseq::log_info!("no args at all");
    }
}
