#![no_std]
use regseq::log;
pub fn emit_logs(retries: u32) {
    log::debug!("debug event x={}", retries);
    log::info!("info event x={}", retries);
    log::warn!("warn event x={}", retries);
    log::error!("error event x={}", retries);
}
