use regseq::{RegseqError, RxChar, Serial, SerialConfig, SerialConsole};

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
}

#[test]
fn serial_types_are_publicly_exported() {
    fn assert_serial<T: Serial>() {}
    fn assert_serial_console<T: SerialConsole>() {}
    assert_serial::<TestUart>();

    // SerialConfig and RxChar are usable as types.
    fn _takes_config(_c: &SerialConfig) {}
    fn _returns_rx() -> Option<RxChar> {
        None
    }

    // SerialConsole is independently implementable.
    struct ConsoleDev;
    impl SerialConsole for ConsoleDev {
        fn console_write(&mut self, _text: &str) {}
    }
    assert_serial_console::<ConsoleDev>();
}

#[test]
fn serial_impl_compiles_with_accepted_signatures() {
    let mut dev = TestUart;
    let _open: Result<(), RegseqError> = dev.port_open();
    let _close: () = dev.port_close();
    let _tx: () = dev.tx_byte(0x41);
    let _rx: Option<RxChar> = dev.rx_char();
    let _ready: bool = dev.tx_ready();
}

#[test]
fn serial_console_is_independent_optional_companion() {
    // A type can implement SerialConsole without implementing Serial.
    struct ConsoleOnly;
    impl SerialConsole for ConsoleOnly {
        fn console_write(&mut self, _text: &str) {}
    }

    let mut c = ConsoleOnly;
    c.console_write("hello");
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
    }

    let mut dev = FailUart;
    assert_eq!(dev.port_open(), Err(RegseqError::DeviceBusy));
}
