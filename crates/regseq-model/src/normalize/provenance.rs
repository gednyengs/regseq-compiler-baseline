//! Provenance + normalization-time call-stack tracking.
//!
//! Every normalized entity and every [`NormalizeError`](super::NormalizeError)
//! must cite two strings (spec.md Section 12 line 1356-1357, task spec
//! "Provenance contract"):
//!
//! - `surface_path`: a dotted path into the raw `RawFrontendArtifact`
//!   JSON, e.g., `"clock.fields[3].clock_id"`.
//! - `containing_item`: a human-readable label for the logical
//!   containing item, e.g., `"DriverStruct(PL011Driver)"` or
//!   `"Module(<root>)"`.
//!
//! [`NormalizeContext`] maintains a push/pop stack so per-surface
//! helpers can scope the current path and item without threading
//! extra parameters through each call. Rather than return RAII
//! guards that hold a `&mut` (which conflicts with rebuilding the
//! provenance on the error path), the context exposes explicit
//! `push_surface`/`pop_surface` methods. Helpers either manage the
//! pairing manually or, for the common case of a whole-function
//! scope, wrap their body with [`with_surface`].

/// Provenance anchor stamped on every [`NormalizeError`](super::NormalizeError).
///
/// Both strings are always populated -- empty values indicate a
/// normalizer bug and the task's acceptance criteria require a
/// non-empty `surface_path` + `containing_item` on every error.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NormalizeProvenance {
    /// Dotted path into the [`RawFrontendArtifact`](super::raw_artifact::RawFrontendArtifact)
    /// payload that caused the diagnostic.
    pub surface_path: String,
    /// Logical containing item, e.g., `"DriverStruct(<name>)"`.
    pub containing_item: String,
}

/// Push/pop stack tracking the current normalization location.
pub struct NormalizeContext {
    surface_segments: Vec<String>,
    containing_item: String,
}

impl NormalizeContext {
    /// Fresh context anchored at the raw artifact root.
    ///
    /// Initial `surface_path` is `"<root>"` and `containing_item` is
    /// `"<artifact>"`; both strings are guaranteed non-empty so an
    /// error raised before any helper pushes is still well-formed.
    pub fn root() -> Self {
        Self {
            surface_segments: vec!["<root>".to_string()],
            containing_item: "<artifact>".to_string(),
        }
    }

    /// Current provenance snapshot, suitable for stamping on an error.
    pub fn provenance(&self) -> NormalizeProvenance {
        NormalizeProvenance {
            surface_path: self.surface_path(),
            containing_item: self.containing_item.clone(),
        }
    }

    /// Joined `surface_path` string (e.g., `"root.clock.fields[0]"`).
    pub fn surface_path(&self) -> String {
        self.surface_segments.join(".")
    }

    /// Current containing-item label.
    pub fn containing_item(&self) -> &str {
        &self.containing_item
    }

    /// Push a new surface-path segment.
    pub fn push_surface(&mut self, segment: impl Into<String>) {
        self.surface_segments.push(segment.into());
    }

    /// Pop the most-recently-pushed surface segment.
    /// Panics (debug assertion) if called with only the root segment
    /// still on the stack -- a helper popped more than it pushed.
    pub fn pop_surface(&mut self) {
        debug_assert!(
            self.surface_segments.len() > 1,
            "NormalizeContext::pop_surface underflowed the root anchor",
        );
        self.surface_segments.pop();
    }

    /// Replace the containing-item label, returning the previous
    /// label so the caller can restore it via [`Self::set_item_from`].
    pub fn set_item(&mut self, item: impl Into<String>) -> String {
        std::mem::replace(&mut self.containing_item, item.into())
    }

    /// Restore the containing-item label from a previous
    /// [`Self::set_item`] call.
    pub fn set_item_from(&mut self, previous: String) {
        self.containing_item = previous;
    }
}

/// Run `body` with `segment` pushed onto `ctx`'s surface path,
/// guaranteeing the segment is popped even if `body` returns `Err`.
pub fn with_surface<T, E>(
    ctx: &mut NormalizeContext,
    segment: impl Into<String>,
    body: impl FnOnce(&mut NormalizeContext) -> Result<T, E>,
) -> Result<T, E> {
    ctx.push_surface(segment);
    let result = body(ctx);
    ctx.pop_surface();
    result
}

/// Run `body` with the containing-item label replaced, restoring the
/// previous label after `body` finishes (success or error).
pub fn with_item<T, E>(
    ctx: &mut NormalizeContext,
    item: impl Into<String>,
    body: impl FnOnce(&mut NormalizeContext) -> Result<T, E>,
) -> Result<T, E> {
    let previous = ctx.set_item(item);
    let result = body(ctx);
    ctx.set_item_from(previous);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_context_has_nonempty_strings() {
        let ctx = NormalizeContext::root();
        let p = ctx.provenance();
        assert!(!p.surface_path.is_empty());
        assert!(!p.containing_item.is_empty());
    }

    #[test]
    fn push_pop_restores_surface_path() {
        let mut ctx = NormalizeContext::root();
        let before = ctx.provenance().surface_path;
        ctx.push_surface("clock");
        assert!(ctx.provenance().surface_path.ends_with("clock"));
        ctx.pop_surface();
        assert_eq!(ctx.provenance().surface_path, before);
    }

    #[test]
    fn with_surface_pops_on_err() {
        let mut ctx = NormalizeContext::root();
        let before = ctx.provenance().surface_path;
        let r: Result<(), &'static str> = with_surface(&mut ctx, "clock", |inner| {
            assert!(inner.provenance().surface_path.ends_with("clock"));
            Err("boom")
        });
        assert!(r.is_err());
        assert_eq!(ctx.provenance().surface_path, before);
    }

    #[test]
    fn with_item_restores_previous_label() {
        let mut ctx = NormalizeContext::root();
        let before = ctx.provenance().containing_item;
        let _ = with_item::<_, &'static str>(&mut ctx, "DriverStruct(PL011)", |inner| {
            assert_eq!(inner.provenance().containing_item, "DriverStruct(PL011)");
            Ok(())
        });
        assert_eq!(ctx.provenance().containing_item, before);
    }
}
