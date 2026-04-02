//! Build-context metadata separated from declaration identity.
//!
//! `target_os` and similar build-time compilation settings belong here,
//! not inside the declaration identity. This keeps the declaration model
//! focused on what the driver declares rather than how it was built.

use serde::{Deserialize, Serialize};

/// Build-time compilation context captured alongside the declaration model.
///
/// This is explicitly separate from [`super::driver::DriverDecl`] to ensure
/// that build settings do not leak into declaration identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationBuildContext {
    /// Target OS from compiler options (e.g. `"linux"`, `"zephyr"`).
    pub target_os: String,
}
