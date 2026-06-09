use regseq_config::LogLevel;

#[test]
fn crate_can_be_imported() {
    // `TargetOs` moved to `regseq-model` (R5/D6/D8); smoke-test the crate via a
    // type it still owns.
    assert_eq!(LogLevel::default(), LogLevel::Warn);
}
