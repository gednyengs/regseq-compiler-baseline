use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Attribute, ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::TyCtxt;

use super::callback_context::CallbackContext;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the timed-delay intrinsic surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedDelaySurface {
    pub call_sites: Vec<TimedDelayCallSite>,
    pub duration_ctors: Vec<DurationCtorSite>,
}

/// Identity of a recognized timed-delay intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayIntrinsic {
    BusyWait,
    Sleep,
}

/// Whether the call came through the canonical or compatibility path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayPathKind {
    Canonical,
    Compatibility,
}

/// A recognized timed-delay intrinsic call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedDelayCallSite {
    pub intrinsic: DelayIntrinsic,
    pub path_kind: DelayPathKind,
    pub containing_fn: String,
}

/// The kind of a recognized `Duration` constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationCtor {
    FromUs,
    FromMs,
    FromS,
}

/// A recognized `Duration::from_*` constructor call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationCtorSite {
    pub ctor: DurationCtor,
    pub containing_fn: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimedDelayError {
    SleepInHandlerContext(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the timed-delay intrinsic surface from HIR after analysis.
pub fn extract_timed_delay_surface<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<TimedDelaySurface, TimedDelayError> {
    let mut call_sites = Vec::new();
    let mut duration_ctors = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        if let ItemKind::Impl(impl_block) = &item.kind {
            let context = infer_impl_callback_context(tcx, impl_block);

            for impl_item_ref in impl_block.items {
                let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                    owner_id: impl_item_ref.owner_id,
                });
                collect_timed_delay_sites(
                    tcx,
                    impl_item,
                    context,
                    &mut call_sites,
                    &mut duration_ctors,
                )?;
            }
        }
    }

    Ok(TimedDelaySurface {
        call_sites,
        duration_ctors,
    })
}

// ---------------------------------------------------------------------------
// Callback context inference
// ---------------------------------------------------------------------------

/// Infer the callback context for an impl block.
///
/// - ISR handlers (`#[rseq::isr]`) are HandlerManaged.
/// - Trait impls with known names get their standard context.
/// - Plain inherent impls default to FrameworkManaged.
fn infer_impl_callback_context<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_block: &rustc_hir::Impl<'tcx>,
) -> Option<CallbackContext> {
    // Check if any method has #[rseq::isr]
    let has_isr = impl_block.items.iter().any(|item_ref| {
        let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
            owner_id: item_ref.owner_id,
        });
        let attrs = tcx.hir_attrs(impl_item.hir_id());
        attrs.iter().any(|a| rseq_attr_name(a) == Some("isr"))
    });

    if has_isr {
        return Some(CallbackContext::HandlerManaged);
    }

    // Check the trait ref for known trait names
    if let Some(trait_ref) = &impl_block.of_trait {
        if let Some(last_seg) = trait_ref.trait_ref.path.segments.last() {
            return match last_seg.ident.name.as_str() {
                "Lifecycle" => Some(CallbackContext::FrameworkManaged),
                "PowerManagement" => Some(CallbackContext::FrameworkManaged),
                "Serial" => Some(CallbackContext::SubsystemSerialized),
                "SerialConsole" => Some(CallbackContext::SubsystemSerialized),
                _ => None,
            };
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_timed_delay_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    impl_context: Option<CallbackContext>,
    call_sites: &mut Vec<TimedDelayCallSite>,
    duration_ctors: &mut Vec<DurationCtorSite>,
) -> Result<(), TimedDelayError> {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);

        // Per-method context: check if this specific method has #[rseq::isr]
        let attrs = tcx.hir_attrs(impl_item.hir_id());
        let is_isr = attrs.iter().any(|a| rseq_attr_name(a) == Some("isr"));
        let method_context = if is_isr {
            Some(CallbackContext::HandlerManaged)
        } else {
            impl_context
        };

        let mut visitor = TimedDelayVisitor {
            current_fn: fn_name,
            context: method_context,
            call_sites: Vec::new(),
            duration_ctors: Vec::new(),
            error: None,
        };
        visitor.visit_expr(body.value);

        if let Some(err) = visitor.error {
            return Err(err);
        }
        call_sites.extend(visitor.call_sites);
        duration_ctors.extend(visitor.duration_ctors);
    }
    Ok(())
}

struct TimedDelayVisitor {
    current_fn: String,
    context: Option<CallbackContext>,
    call_sites: Vec<TimedDelayCallSite>,
    duration_ctors: Vec<DurationCtorSite>,
    error: Option<TimedDelayError>,
}

impl<'tcx> Visitor<'tcx> for TimedDelayVisitor {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }

        if let ExprKind::Call(callee, _) = expr.kind {
            // Check for delay intrinsic paths
            if let Some((intrinsic, path_kind)) = classify_delay_path(callee) {
                // Legality: sleep is rejected in handler-managed context
                if intrinsic == DelayIntrinsic::Sleep
                    && self.context == Some(CallbackContext::HandlerManaged)
                {
                    self.error = Some(TimedDelayError::SleepInHandlerContext(
                        self.current_fn.clone(),
                    ));
                    return;
                }

                self.call_sites.push(TimedDelayCallSite {
                    intrinsic,
                    path_kind,
                    containing_fn: self.current_fn.clone(),
                });
            }

            // Check for Duration::from_* constructors
            if let Some(ctor) = classify_duration_ctor(callee) {
                self.duration_ctors.push(DurationCtorSite {
                    ctor,
                    containing_fn: self.current_fn.clone(),
                });
            }
        }

        intravisit::walk_expr(self, expr);
    }
}

/// Classify a call expression as a delay intrinsic, returning the intrinsic
/// kind and path kind if matched.
fn classify_delay_path(callee: &rustc_hir::Expr<'_>) -> Option<(DelayIntrinsic, DelayPathKind)> {
    let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind else {
        return None;
    };

    let segs: Vec<&str> = path
        .segments
        .iter()
        .map(|s| s.ident.name.as_str())
        .collect();

    // Canonical: intrinsics::timed_delay::busy_wait / sleep
    if segs.len() >= 3
        && segs[segs.len() - 3] == "intrinsics"
        && segs[segs.len() - 2] == "timed_delay"
    {
        return match segs[segs.len() - 1] {
            "busy_wait" => Some((DelayIntrinsic::BusyWait, DelayPathKind::Canonical)),
            "sleep" => Some((DelayIntrinsic::Sleep, DelayPathKind::Canonical)),
            _ => None,
        };
    }

    // Compatibility: delay::busy_wait / sleep
    if segs.len() >= 2 && segs[segs.len() - 2] == "delay" {
        return match segs[segs.len() - 1] {
            "busy_wait" => Some((DelayIntrinsic::BusyWait, DelayPathKind::Compatibility)),
            "sleep" => Some((DelayIntrinsic::Sleep, DelayPathKind::Compatibility)),
            _ => None,
        };
    }

    None
}

/// Classify a call expression as a `Duration::from_*` constructor.
fn classify_duration_ctor(callee: &rustc_hir::Expr<'_>) -> Option<DurationCtor> {
    let ExprKind::Path(rustc_hir::QPath::TypeRelative(ty, segment)) = &callee.kind else {
        return None;
    };

    // Check the type is Duration
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if path.segments.last().map(|s| s.ident.name.as_str()) != Some("Duration") {
            return None;
        }
    } else {
        return None;
    }

    match segment.ident.name.as_str() {
        "from_us" => Some(DurationCtor::FromUs),
        "from_ms" => Some(DurationCtor::FromMs),
        "from_s" => Some(DurationCtor::FromS),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

fn rseq_attr_name(attr: &Attribute) -> Option<&str> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let segs = &item.path.segments;
    if segs.len() == 2 && segs[0].as_str() == "rseq" {
        Some(segs[1].as_str())
    } else {
        None
    }
}
