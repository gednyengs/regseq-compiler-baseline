use cargo_regseq::run;

#[test]
fn run_returns_ok() {
    assert!(run().is_ok());
}
