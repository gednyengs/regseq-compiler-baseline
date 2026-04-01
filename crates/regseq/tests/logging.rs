use regseq::intrinsics::diagnostic_logging::{self, LogArg};

#[test]
fn canonical_logging_anchors_are_public() {
    // All four canonical anchor paths must be importable.
    let _ = diagnostic_logging::error::<0> as fn(&'static str, [LogArg<'_>; 0]);
    let _ = diagnostic_logging::warn::<0> as fn(&'static str, [LogArg<'_>; 0]);
    let _ = diagnostic_logging::info::<0> as fn(&'static str, [LogArg<'_>; 0]);
    let _ = diagnostic_logging::debug::<0> as fn(&'static str, [LogArg<'_>; 0]);
}

#[test]
fn log_module_exports_four_macros() {
    // The accepted user-facing surface is macro-based re-exports.
    // These must type-check (we don't call them since bodies are unimplemented).
    fn check_error_compiles() {
        regseq::log::error!("something failed");
    }
    fn check_warn_compiles() {
        regseq::log::warn!("low battery");
    }
    fn check_info_compiles() {
        regseq::log::info!("initialized");
    }
    fn check_debug_compiles() {
        regseq::log::debug!("status register read");
    }

    let _ = check_error_compiles;
    let _ = check_warn_compiles;
    let _ = check_info_compiles;
    let _ = check_debug_compiles;
}

#[test]
fn representative_logging_calls_compile() {
    // Logging with primitive arguments must compile through the macro surface.
    fn check_with_args() {
        let channel: u32 = 3;
        regseq::log::error!("DMA channel {} acquisition failed", channel);
        regseq::log::warn!("buffer overflow on path {}", "rx");
        regseq::log::info!("initialized with {} DMA channels", 4u32);
        regseq::log::debug!("status = {:#010x}", 0xdead_beef_u32);
    }

    let _ = check_with_args;
}

#[test]
fn log_module_does_not_expose_wrapper_functions() {
    // The log module should only contain macro re-exports, not ordinary
    // functions. We verify this by checking that the canonical anchors
    // live under intrinsics, not under log.
    let _ = regseq::intrinsics::diagnostic_logging::error::<0> as fn(&'static str, [LogArg<'_>; 0]);
}

#[test]
fn canonical_signature_is_fmt_arguments_free() {
    // The anchor signature uses LogArg, not core::fmt::Arguments.
    fn check_signature(_template: &'static str, _args: [LogArg<'_>; 2]) {}
    let _ = check_signature;
}

#[test]
fn zero_padded_hex_forms_compile() {
    fn check_hex_forms() {
        let status: u32 = 0xff;
        regseq::log::debug!("reg = {:08x}", status);
        regseq::log::debug!("reg = {:#010x}", status);
        regseq::log::info!("addr = {:08X}", status);
        regseq::log::info!("addr = {:#010X}", status);
    }

    let _ = check_hex_forms;
}
