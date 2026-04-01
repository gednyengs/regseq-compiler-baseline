use exp01_regseq::{log, Backend, LogArg, LogLevel};

#[test]
fn macro_surface_captures_level_template_and_typed_args() {
    let channel: u32 = 3;
    let event = log::info!("DMA channel {} acquisition failed", channel);

    assert_eq!(event.level, LogLevel::Info);
    assert_eq!(event.template, "DMA channel {} acquisition failed");
    assert_eq!(event.args, [LogArg::U32(3)]);
}

#[test]
fn lower_default_decimal_for_linux_and_zephyr() {
    let event = log::info!("DMA channel {} acquisition failed", 7u32);

    let linux = event.lower_format(Backend::Linux).unwrap();
    let zephyr = event.lower_format(Backend::Zephyr).unwrap();

    assert_eq!(linux.format_string, "DMA channel %u acquisition failed");
    assert_eq!(zephyr.format_string, "DMA channel %u acquisition failed");
}

#[test]
fn lower_hex_and_pointer_styles() {
    let status: u32 = 0x1234_abcd;
    let ptr: *const u8 = 0x1000usize as *const u8;

    let status_event = log::debug!("status = {:#x}", status);
    let ptr_event = log::debug!("buffer at {:p}", ptr);

    let status_linux = status_event.lower_format(Backend::Linux).unwrap();
    let ptr_linux = ptr_event.lower_format(Backend::Linux).unwrap();

    assert_eq!(status_linux.format_string, "status = %#x");
    assert_eq!(ptr_linux.format_string, "buffer at %p");
}

#[test]
fn lower_zero_padded_hex_width_forms() {
    let status: u32 = 0x1234_abcd;
    let addr: usize = 0x2000;

    let status_event = log::debug!("status = {:#010x}", status);
    let addr_event = log::debug!("addr = {:08x}", addr);

    let status_linux = status_event.lower_format(Backend::Linux).unwrap();
    let addr_windows = addr_event.lower_format(Backend::Windows).unwrap();
    let addr_freebsd = addr_event.lower_format(Backend::FreeBsd).unwrap();

    assert_eq!(status_linux.format_string, "status = %#010x");
    assert_eq!(addr_windows.format_string, "addr = %08Ix");
    assert_eq!(addr_freebsd.format_string, "addr = %08zx");
}

#[test]
fn lower_windows_size_types_differ_from_unix_family() {
    let addr: usize = 0x2000;

    let event = log::info!("mapped RX buffer at {:#x}", addr);

    let windows = event.lower_format(Backend::Windows).unwrap();
    let freebsd = event.lower_format(Backend::FreeBsd).unwrap();
    let macos = event.lower_format(Backend::MacOs).unwrap();

    assert_eq!(windows.format_string, "mapped RX buffer at %#Ix");
    assert_eq!(freebsd.format_string, "mapped RX buffer at %#zx");
    assert_eq!(macos.format_string, "mapped RX buffer at %#zx");
}

#[test]
fn lowering_rejects_pointer_without_pointer_style() {
    let ptr: *const u8 = 0x1000usize as *const u8;
    let event = log::warn!("buffer {}", ptr);

    let err = event.lower_format(Backend::Linux).unwrap_err();
    assert!(matches!(err, exp01_regseq::LowerError::InvalidPointerStyle));
}

#[test]
fn lowering_rejects_placeholder_count_mismatch() {
    let event = exp01_regseq::intrinsics::diagnostic_logging::info(
        "x={} y={}",
        [LogArg::U32(1)],
    );

    let err = event.lower_format(Backend::Linux).unwrap_err();
    assert!(matches!(
        err,
        exp01_regseq::LowerError::PlaceholderCountMismatch {
            placeholders: 2,
            args: 1
        }
    ));
}
