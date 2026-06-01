#![feature(register_tool)]
#![register_tool(rseq)]

include!("diag_log_common.rs");

#[no_mangle]
pub extern "C" fn caller() {
    regseq::log::info!(
        "{} {} {} {} {} {} {} {} {}",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
    );
}
