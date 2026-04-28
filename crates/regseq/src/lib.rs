//! Regseq facade crate. Empty placeholder.
//!
//! Items are promoted from each spec's `crate::regseq` submodule
//! incrementally as redesign work touches them. See `architecture.md`
//! Q6 in `docs/discussions/regseq-model/` for the rebuild plan.
//!
//! ## Source-declared compiler attributes (do not remove)
//!
//! The crate-level declarations below are deliberately HERE rather
//! than injected by the compiler driver (`regseq-driver`). This is
//! the symmetric `register_tool` rule: both `RegseqCallbacks` and
//! `RegseqAwareCallbacks` intentionally do NOT inject these. Without
//! source-side declarations, `cargo check` and `rust-analyzer` fail
//! on this crate outside the wrapper, because the `rseq` tool
//! attribute namespace is unrecognized at parse time.
//!
//! References:
//! - `docs/discussions/regseq-model/memory.md` (methodology rules)
//! - `docs/discussions/regseq-model/architecture.md` (Q6)
//!
//! Removing these declarations breaks plain rustc invocation. Don't.

#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
