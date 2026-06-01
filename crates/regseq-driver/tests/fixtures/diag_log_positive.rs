#![feature(register_tool)]
#![register_tool(rseq)]

include!("diag_log_common.rs");

#[no_mangle]
pub extern "C" fn caller() {
    regseq::log::debug!("debug {}", 1u32);
    regseq::log::info!("value = {:#010x}", 42u32);
    regseq::log::warn!("warn {}", 2u32);
    regseq::log::error!("error {}", 3u32);
}
