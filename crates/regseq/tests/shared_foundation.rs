use regseq::{Duration, IrqResult, RegseqError};

#[test]
fn crate_exports_compile() {
    let _err: RegseqError = RegseqError::Timeout;
    let _dur: Duration = Duration::from_ms(100);
    let _irq: IrqResult = IrqResult::Handled;
}

#[test]
fn irq_result_variants() {
    let _ = IrqResult::Handled;
    let _ = IrqResult::NotHandled;
    let _ = IrqResult::DeferWork;
}

#[test]
fn regseq_error_variants() {
    let _ = RegseqError::InvalidParameter;
    let _ = RegseqError::OutOfMemory;
    let _ = RegseqError::HardwareError;
    let _ = RegseqError::Timeout;
    let _ = RegseqError::Unsupported;
    let _ = RegseqError::DeviceBusy;
    let _ = RegseqError::PermissionDenied;
    let _ = RegseqError::DeviceNotFound;
    let _ = RegseqError::GenericFailure;
}

#[test]
fn regseq_error_derives() {
    let a = RegseqError::Timeout;
    let b = a.clone();
    let c = a; // Copy
    assert_eq!(a, b);
    assert_eq!(a, c);
    // Debug
    let _ = format!("{:?}", a);
}

#[test]
fn duration_constructors() {
    let _ = Duration::from_us(500);
    let _ = Duration::from_ms(10);
    let _ = Duration::from_s(1);
}

#[test]
fn duration_derives_and_const() {
    const US: Duration = Duration::from_us(1_000);
    const MS: Duration = Duration::from_ms(1);
    const S: Duration = Duration::from_s(1);

    // from_us(1000) == from_ms(1)
    assert_eq!(US, MS);

    let copy = S;
    let clone = S.clone();
    assert_eq!(copy, clone);
    let _ = format!("{:?}", S);
}
