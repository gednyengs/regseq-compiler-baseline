#[test]
fn crate_compiles_and_imports() {
    // Verify the crate can be imported. The unused import is intentional:
    // this test exists only to prove the crate links.
    use regseq_diagnostics as _;
}
