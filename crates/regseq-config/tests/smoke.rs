use regseq_config::TargetOs;

#[test]
fn crate_can_be_imported() {
    let os = TargetOs::Linux;
    assert_eq!(os.to_string(), "linux");
}
