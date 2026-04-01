use regseq::{Duration, RegseqError, ResetLine};

#[test]
fn resetline_is_publicly_exported() {
    fn assert_type_exists(_: &ResetLine) {}
    let _ = assert_type_exists;
}

#[test]
fn resetline_compiles_as_driver_field_shapes() {
    struct DriverRequired {
        _reset: ResetLine,
    }

    struct DriverOptional {
        _reset: Option<ResetLine>,
    }

    let _ = core::mem::size_of::<DriverRequired>();
    let _ = core::mem::size_of::<DriverOptional>();
}

#[test]
fn resetline_wrapper_signatures_compile() {
    fn check_assert(r: &ResetLine) -> Result<(), RegseqError> {
        r.assert()
    }

    fn check_deassert(r: &ResetLine) -> Result<(), RegseqError> {
        r.deassert()
    }

    fn check_toggle(r: &ResetLine) -> Result<(), RegseqError> {
        r.toggle(Duration::from_us(100))
    }

    let _ = check_assert;
    let _ = check_deassert;
    let _ = check_toggle;
}

#[test]
fn toggle_uses_shared_duration_type() {
    // Verify that toggle accepts the shared Duration facade type.
    fn takes_duration(_: Duration) {}
    let d = Duration::from_ms(10);
    takes_duration(d);
}
