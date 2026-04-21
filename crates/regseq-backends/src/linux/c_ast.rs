//! Re-export shim for the former `linux::c_ast` module.
//!
//! The C AST has moved to the backend-neutral [`crate::c`] module. This
//! shim preserves the `super::c_ast::*` import paths used by existing
//! callers within the Linux backend until tasks 9 / 10 retarget them
//! onto the new structured AST. It will be removed once all call sites
//! have migrated.

pub use crate::c::*;
