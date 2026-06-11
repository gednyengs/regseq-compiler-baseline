//! `codegen-coverage-runner` library surface: the codegen-coverage
//! harness modules (the codegen analog of the archived
//! `bridge-coverage-runner`), per
//! `docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`.
//!
//! The 6-stage pipeline (`pipeline`) composes the manifest schema
//! (`manifest`), the Doc-1 matrix parser (`matrix`), the dual oracle
//! (`oracle`), the host-`cc` toolchain (`toolchain`), and the coverage
//! ledger (`ledger`). The `main.rs` binary is a thin Mode-A CLI over
//! these; they live in the library (rather than as binary-private
//! modules) so the per-stage integration tests under `tests/` can drive
//! the public surface directly -- the testability escape hatch sanctioned
//! by the task spec's Change J, since emit is stubbed and no whole-loop
//! smoke test is possible this phase.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` are
//! required because this crate transitively links the rustc-backed bridge
//! (`test-ir-fixtures`); any final artifact (the binary, and each `tests/`
//! integration binary) must declare the same anchor so the rustc dylibs
//! resolve in a single format.

#![feature(rustc_private)]

extern crate rustc_driver;

pub mod conformance;
pub mod kbuild;
pub mod ledger;
pub mod manifest;
pub mod matrix;
pub mod oracle;
pub mod pipeline;
pub mod toolchain;
pub mod zephyr_build;
