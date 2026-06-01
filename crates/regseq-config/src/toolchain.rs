//! Shared host-`cc` toolchain configuration.
//!
//! Lives in `regseq-config` (rustc-independent) so every consumer that
//! invokes the host C compiler — the codegen-coverage runner and
//! `cargo-regseq`'s `--emit bin` linker — uses one pinned flag set without
//! taking a dependency on `regseq-backends`.

/// Pinned `cc` flags per `generic-c-backend.md` Doc 3 section 7: `-std=gnu11`
/// (GNU-mode float builtins / `__int128`), `-fwrapv` (signed wrap, D7),
/// `-Wall`. `-Werror` is deliberately NOT set: Doc 3 section 1 stage-4
/// "no warnings-as-errors" means warnings are surfaced but do not fail the
/// compile.
pub const CC_FLAGS: &[&str] = &["-std=gnu11", "-fwrapv", "-Wall"];
