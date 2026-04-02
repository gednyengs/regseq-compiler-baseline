use regseq::{
    DataBits, ErrorKind, FlowControl, Parity, RegseqError, RxChar, Serial, SerialConfig,
    SerialConsole, SerialErrors, StopBits,
};

struct TestUart;

impl Serial for TestUart {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn port_close(&mut self) {}

    fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
        Ok(())
    }

    fn tx_byte(&mut self, _byte: u8) {}

    fn rx_char(&mut self) -> Option<RxChar> {
        None
    }

    fn tx_ready(&mut self) -> bool {
        true
    }

    fn tx_empty(&mut self) -> bool {
        true
    }

    fn flush_rx_fifo(&mut self) {}

    fn flush_tx_fifo(&mut self) {}

    fn send_break(&mut self, _on: bool) {}
}

#[test]
fn serial_types_are_publicly_exported() {
    fn assert_serial<T: Serial>() {}
    fn assert_serial_console<T: SerialConsole>() {}
    assert_serial::<TestUart>();

    // SerialConfig, SerialErrors, ErrorKind, and RxChar are usable as types.
    fn _takes_config(_c: &SerialConfig) {}
    fn _returns_rx() -> Option<RxChar> {
        None
    }
    fn _takes_errors(_e: SerialErrors) {}
    fn _takes_error_kind(_e: ErrorKind) {}

    // SerialConsole is independently implementable.
    struct ConsoleDev;
    impl SerialConsole for ConsoleDev {
        fn console_write(&mut self, _bytes: &[u8]) {}
    }
    assert_serial_console::<ConsoleDev>();
}

#[test]
fn serial_impl_compiles_with_v1_signatures() {
    let mut dev = TestUart;
    let _open: Result<(), RegseqError> = dev.port_open();
    let _close: () = dev.port_close();
    let _tx: () = dev.tx_byte(0x41);
    let _rx: Option<RxChar> = dev.rx_char();
    let _ready: bool = dev.tx_ready();
    let _empty: bool = dev.tx_empty();
    let _flush_rx: () = dev.flush_rx_fifo();
    let _flush_tx: () = dev.flush_tx_fifo();
    let _brk: () = dev.send_break(true);
}

#[test]
fn serial_console_uses_byte_slice_signature() {
    struct ConsoleOnly;
    impl SerialConsole for ConsoleOnly {
        fn console_write(&mut self, _bytes: &[u8]) {}
    }

    let mut c = ConsoleOnly;
    c.console_write(b"hello");
    c.console_write(&[0x1b, 0x5b, 0x30, 0x6d]);
}

#[test]
fn serial_console_is_independent_optional_companion() {
    struct ConsoleOnly;
    impl SerialConsole for ConsoleOnly {
        fn console_write(&mut self, _bytes: &[u8]) {}
    }

    let mut c = ConsoleOnly;
    c.console_write(b"hello");
}

#[test]
fn serial_fallible_methods_can_return_error() {
    struct FailUart;

    impl Serial for FailUart {
        fn port_open(&mut self) -> Result<(), RegseqError> {
            Err(RegseqError::DeviceBusy)
        }

        fn port_close(&mut self) {}

        fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
            Err(RegseqError::InvalidParameter)
        }

        fn tx_byte(&mut self, _byte: u8) {}

        fn rx_char(&mut self) -> Option<RxChar> {
            None
        }

        fn tx_ready(&mut self) -> bool {
            false
        }

        fn tx_empty(&mut self) -> bool {
            false
        }

        fn flush_rx_fifo(&mut self) {}
        fn flush_tx_fifo(&mut self) {}
        fn send_break(&mut self, _on: bool) {}
    }

    let mut dev = FailUart;
    assert_eq!(dev.port_open(), Err(RegseqError::DeviceBusy));
}

#[test]
fn rx_char_carries_byte_and_serial_errors() {
    let rx = RxChar::new(0x41, SerialErrors::PARITY | SerialErrors::OVERRUN);

    assert_eq!(rx.byte(), 0x41);
    assert!(rx.errors().contains(SerialErrors::PARITY));
    assert!(rx.errors().contains(SerialErrors::OVERRUN));
    assert!(!rx.errors().contains(SerialErrors::BREAK));
}

#[test]
fn rx_char_error_kind_compat_accessor_works() {
    // ErrorKind is a type alias for SerialErrors.
    let rx = RxChar::new(0x42, ErrorKind::FRAMING);

    // The error() compat accessor still works.
    assert!(rx.error().contains(ErrorKind::FRAMING));
    // errors() is the preferred v1 accessor.
    assert!(rx.errors().contains(SerialErrors::FRAMING));
}

#[test]
fn serial_errors_naming_is_publicly_usable() {
    // SerialErrors is the v1 public name.
    let e = SerialErrors::OVERRUN | SerialErrors::BREAK;
    assert!(e.contains(SerialErrors::OVERRUN));
    assert!(e.contains(SerialErrors::BREAK));
    assert!(!e.contains(SerialErrors::PARITY));

    // ErrorKind alias is interchangeable.
    let e2: ErrorKind = SerialErrors::PARITY;
    assert!(e2.contains(ErrorKind::PARITY));
}

#[test]
fn serial_config_support_enums_are_public() {
    // DataBits variants.
    let _five = DataBits::Five;
    let _six = DataBits::Six;
    let _seven = DataBits::Seven;
    let _eight = DataBits::Eight;
    let _nine = DataBits::Nine;

    // Parity variants.
    let _none = Parity::None;
    let _even = Parity::Even;
    let _odd = Parity::Odd;

    // StopBits variants.
    let _one = StopBits::One;
    let _two = StopBits::Two;

    // FlowControl variants.
    let _no_fc = FlowControl::None;
    let _rtscts = FlowControl::RtsCts;
    let _dtrdsr = FlowControl::DtrDsr;
}

#[test]
fn serial_config_has_v1_fields() {
    let config = SerialConfig {
        baud_rate: 115200,
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::One,
        flow_control: FlowControl::None,
    };

    assert_eq!(config.baud_rate, 115200);
    assert_eq!(config.data_bits, DataBits::Eight);
    assert_eq!(config.parity, Parity::None);
    assert_eq!(config.stop_bits, StopBits::One);
    assert_eq!(config.flow_control, FlowControl::None);
}
