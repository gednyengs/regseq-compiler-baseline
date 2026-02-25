use regseq_config::Error;

#[test]
fn crate_can_be_imported() {
    let error = Error;
    assert_eq!(error.to_string(), "regseq-config error");
}
