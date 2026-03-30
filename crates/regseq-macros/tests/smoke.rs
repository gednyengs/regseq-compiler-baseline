#[test]
fn crate_compiles() {
    // Proc-macro crates cannot be imported directly in tests via `use`.
    // This test verifies that `cargo test -p regseq-macros` succeeds,
    // confirming the crate compiles as a proc-macro crate.
}
