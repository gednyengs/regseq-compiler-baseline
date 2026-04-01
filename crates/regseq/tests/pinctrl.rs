use regseq::{PinCtrl, PinState, RegseqError};

struct DefaultState;
impl PinState for DefaultState {}

struct SleepState;
impl PinState for SleepState {}

#[test]
fn pinctrl_and_pinstate_are_publicly_exported() {
    fn assert_pinctrl(_: &PinCtrl) {}
    fn assert_pinstate<T: PinState>() {}
    let _ = assert_pinctrl;
    assert_pinstate::<DefaultState>();
}

#[test]
fn pinctrl_compiles_as_driver_field_shapes() {
    struct DriverRequired {
        _pins: PinCtrl,
    }

    struct DriverOptional {
        _pins: Option<PinCtrl>,
    }

    let _ = core::mem::size_of::<DriverRequired>();
    let _ = core::mem::size_of::<DriverOptional>();
}

#[test]
fn apply_accepts_pinstate_marker() {
    fn check_apply(p: &PinCtrl) -> Result<(), RegseqError> {
        p.apply::<DefaultState>()
    }

    fn check_apply_sleep(p: &PinCtrl) -> Result<(), RegseqError> {
        p.apply::<SleepState>()
    }

    let _ = check_apply;
    let _ = check_apply_sleep;
}

#[test]
fn apply_wrapper_signature_compiles() {
    fn check_sig<S: PinState>(p: &PinCtrl) -> Result<(), RegseqError> {
        p.apply::<S>()
    }

    let _ = check_sig::<DefaultState>;
}
