use regseq::{Lifecycle, RegseqError};

struct TestDevice;

impl Lifecycle for TestDevice {
    fn init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[test]
fn lifecycle_is_publicly_exported() {
    fn assert_lifecycle<T: Lifecycle>() {}
    assert_lifecycle::<TestDevice>();
}

#[test]
fn lifecycle_methods_accept_mut_self_and_return_result() {
    let mut dev = TestDevice;
    let init_result: Result<(), RegseqError> = dev.init();
    let deinit_result: Result<(), RegseqError> = dev.deinit();
    assert!(init_result.is_ok());
    assert!(deinit_result.is_ok());
}

#[test]
fn lifecycle_methods_can_return_error() {
    struct FailingDevice;

    impl Lifecycle for FailingDevice {
        fn init(&mut self) -> Result<(), RegseqError> {
            Err(RegseqError::HardwareError)
        }

        fn deinit(&mut self) -> Result<(), RegseqError> {
            Err(RegseqError::GenericFailure)
        }
    }

    let mut dev = FailingDevice;
    assert_eq!(dev.init(), Err(RegseqError::HardwareError));
    assert_eq!(dev.deinit(), Err(RegseqError::GenericFailure));
}
