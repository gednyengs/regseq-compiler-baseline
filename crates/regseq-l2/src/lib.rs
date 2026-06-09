//! L2 (platform-file) binding: join parsed platform metadata into a
//! [`regseq_model::RegseqModel`] and validate the joined L1+L2 surface.
//!
//! This crate is **construct-agnostic**. Each construct that supplies L2
//! properties contributes an [`L2Binder`] tenant (a module here plus one
//! `REGISTRY` entry); `bus-resource-management` (construct #3) is the
//! first. The crate depends only on `regseq-model` + `regseq-config`, so the
//! bind runs identically in production (post-bridge, in `regseq-driver`) and in
//! the test harness — without the rustc-private wall the driver sits behind.
//!
//! # Ordering contract
//!
//! [`bind_all`] runs every tenant's [`L2Binder::bind`] in `REGISTRY`
//! order, then [`validate_l2_surface`] runs every tenant's
//! [`L2Binder::validate`]. Two invariants follow, and are load-bearing as
//! tenants are added:
//!
//! - **`bind`** is deterministic model enrichment. It MAY read state bound by an
//!   *earlier-registered* binder (registry order matters); it must not depend on
//!   a *later* one.
//! - **`validate`** MUST be side-effect-free, and runs only after every tenant's
//!   `bind` has completed (so a tenant's validation can see another's bound
//!   state).

pub mod bus_resource;
mod error;

pub use error::{L2Error, PlatformIdentityError};

use regseq_config::{PlatformBindings, PlatformIdentityRaw};
use regseq_model::{PlatformIdentity, RegseqModel, TargetOs};

/// A construct that supplies L2 (platform-file) properties. Implemented by one
/// zero-sized tenant per construct; see the crate-level ordering contract.
///
/// `Sync` is required so the `REGISTRY` table can become a `static`
/// later (e.g. behind a `OnceLock`) without a churn-y signature change; it is
/// free for the unit-struct binders we register today.
pub trait L2Binder: Sync {
    /// Stable id for diagnostics / registry ordering (e.g. `"bus-resource"`).
    fn name(&self) -> &'static str;

    /// Resolve this construct's platform metadata into `model`. Deterministic
    /// enrichment only; MAY read state bound by an earlier-registered binder. An
    /// infallible join (unmatched fields are simply left unresolved) returns
    /// `Ok(())`.
    fn bind(&self, model: &mut RegseqModel, bindings: &PlatformBindings) -> Result<(), L2Error>;

    /// Validate this construct's joined L1+L2 surface. MUST be side-effect-free;
    /// runs after every tenant's [`L2Binder::bind`].
    fn validate(&self, model: &RegseqModel) -> Result<(), L2Error>;
}

/// The L2 tenants, in bind/validate order — **the one line that grows per
/// construct**. Binders are zero-sized, so this is a `const` table (no
/// allocation).
const REGISTRY: &[&dyn L2Binder] = &[&bus_resource::BusResourceBinder];

/// Bind every tenant's platform metadata into `model`. This is the production
/// post-bridge "1b" join seam (`regseq-driver`'s callback) and the identical
/// call the test harness makes, so the bound model is the verified artifact.
pub fn bind_all(model: &mut RegseqModel, bindings: &PlatformBindings) -> Result<(), L2Error> {
    for binder in REGISTRY {
        binder.bind(model, bindings)?;
    }
    Ok(())
}

/// Validate every tenant's joined L1+L2 surface. Runs after [`bind_all`].
pub fn validate_l2_surface(model: &RegseqModel) -> Result<(), L2Error> {
    for binder in REGISTRY {
        binder.validate(model)?;
    }
    Ok(())
}

/// Resolve a structurally-parsed [`PlatformIdentityRaw`] (from `regseq-config`'s
/// `[platform]` table) into the typed [`PlatformIdentity`] a backend consumes,
/// validating the os token against the closed [`TargetOs`] vocabulary.
///
/// This is the producer→model bridge step (R5/D8): `regseq-config` stays a
/// `serde`-only leaf doing structural parsing, and the *semantic* os check lives
/// here in `regseq-l2` (which already depends on both `regseq-config` and
/// `regseq-model`), alongside [`validate_l2_surface`]. An absent os stays `None`
/// (D7); `board`/`arch` are free-form and pass through verbatim.
pub fn resolve_platform_identity(
    raw: &PlatformIdentityRaw,
) -> Result<PlatformIdentity, PlatformIdentityError> {
    let os = match raw.os.as_deref() {
        None => None,
        Some(token) => Some(TargetOs::from_token(token).ok_or_else(|| {
            PlatformIdentityError::UnknownOs {
                token: token.to_string(),
                expected: TargetOs::EXPECTED,
            }
        })?),
    };
    Ok(PlatformIdentity {
        os,
        board: raw.board.clone(),
        arch: raw.arch.clone(),
    })
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    fn raw(os: Option<&str>, board: Option<&str>, arch: Option<&str>) -> PlatformIdentityRaw {
        PlatformIdentityRaw {
            os: os.map(str::to_string),
            board: board.map(str::to_string),
            arch: arch.map(str::to_string),
        }
    }

    #[test]
    fn resolves_known_os_and_passes_through_board_arch() {
        let id = resolve_platform_identity(&raw(Some("linux"), Some("qemu-pc"), Some("x86_64")))
            .expect("resolve");
        assert_eq!(id.os, Some(TargetOs::Linux));
        assert_eq!(id.board.as_deref(), Some("qemu-pc"));
        assert_eq!(id.arch.as_deref(), Some("x86_64"));
    }

    #[test]
    fn os_token_is_case_insensitive() {
        let id = resolve_platform_identity(&raw(Some("ZEPHYR"), None, None)).expect("resolve");
        assert_eq!(id.os, Some(TargetOs::Zephyr));
    }

    #[test]
    fn absent_os_stays_none() {
        let id = resolve_platform_identity(&raw(None, Some("brd"), None)).expect("resolve");
        assert_eq!(id.os, None);
        assert_eq!(id.board.as_deref(), Some("brd"));
    }

    #[test]
    fn unknown_os_token_is_rejected_with_code() {
        let err = resolve_platform_identity(&raw(Some("plan9"), None, None)).unwrap_err();
        assert_eq!(err.code(), "L2-OS-001");
        assert!(
            matches!(&err, PlatformIdentityError::UnknownOs { token, .. } if token == "plan9"),
            "got {err:?}"
        );
        assert!(err.to_string().contains("plan9"), "{err}");
    }
}
