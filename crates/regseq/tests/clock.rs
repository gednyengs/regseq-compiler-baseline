use regseq::{Clock, RegseqError};

#[test]
fn clock_is_publicly_exported() {
    // Clock must be importable from the regseq crate root.
    fn assert_type_exists(_: &Clock) {}
    let _ = assert_type_exists;
}

#[test]
fn clock_compiles_as_driver_field_shapes() {
    struct DriverRequired {
        _clock: Clock,
    }

    struct DriverOptional {
        _clock: Option<Clock>,
    }

    // Both shapes must compile; no runtime construction needed.
    let _ = core::mem::size_of::<DriverRequired>();
    let _ = core::mem::size_of::<DriverOptional>();
}

#[test]
fn clock_wrapper_signatures_compile() {
    // Verify that the accepted wrapper method signatures type-check.
    fn check_enable(c: &Clock) -> Result<(), RegseqError> {
        c.enable()
    }

    fn check_disable(c: &Clock) {
        c.disable()
    }

    fn check_get_rate(c: &Clock) -> u32 {
        c.get_rate()
    }

    let _ = check_enable;
    let _ = check_disable;
    let _ = check_get_rate;
}

#[test]
fn clock_is_opaque_no_runtime_support_needed() {
    // Clock can be used in type positions without runtime support.
    let _ = core::mem::size_of::<Clock>();
    let _ = core::mem::size_of::<Option<Clock>>();
}
