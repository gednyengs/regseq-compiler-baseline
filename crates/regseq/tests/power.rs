use regseq::{PowerManagement, RegseqError};

struct TestDevice;

impl PowerManagement for TestDevice {
    fn suspend(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[test]
fn power_management_is_publicly_exported() {
    fn assert_pm<T: PowerManagement>() {}
    assert_pm::<TestDevice>();
}

#[test]
fn power_management_methods_accept_mut_self_and_return_result() {
    let mut dev = TestDevice;
    let suspend_result: Result<(), RegseqError> = dev.suspend();
    let resume_result: Result<(), RegseqError> = dev.resume();
    assert!(suspend_result.is_ok());
    assert!(resume_result.is_ok());
}

#[test]
fn power_management_is_optional() {
    // A type that implements Lifecycle but not PowerManagement compiles fine.
    use regseq::Lifecycle;

    struct NoPmDevice;

    impl Lifecycle for NoPmDevice {
        fn init(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
        fn deinit(&mut self) -> Result<(), RegseqError> {
            Ok(())
        }
    }

    let mut dev = NoPmDevice;
    assert!(dev.init().is_ok());
}
