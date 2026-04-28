use std::collections::HashSet;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::{Ty, TyCtxt};

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the synchronization surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizationSurface {
    pub sync_fields: Vec<SyncFieldDecl>,
    pub lock_operations: Vec<LockOperation>,
    pub completion_operations: Vec<CompletionOperation>,
    pub guard_scopes: Vec<GuardScope>,
}

/// A synchronization wrapper field declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFieldDecl {
    pub field_name: String,
    pub kind: SyncFieldKind,
    pub payload_type: Option<String>,
}

/// Classification of synchronization wrapper types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncFieldKind {
    Mutex,
    SpinLock,
    IrqSpinLock,
    Completion,
}

/// A `.lock()` call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockOperation {
    pub lock_kind: SyncFieldKind,
    pub containing_fn: String,
}

/// A method call on a `Completion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionMethod {
    Wait,
    Signal,
    Reset,
}

/// A completion operation call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionOperation {
    pub method: CompletionMethod,
    pub containing_fn: String,
}

/// Guard-scope metadata preserved for later analysis passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardScope {
    pub lock_kind: SyncFieldKind,
    pub containing_fn: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynchronizationError {
    /// A non-ISR-safe lock (Mutex or SpinLock) was used in an ISR handler.
    NonIsrSafeLockInIsr(String),
    /// `Completion::wait(...)` was called in an ISR handler.
    CompletionWaitInIsr(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the synchronization surface from HIR after analysis.
///
/// `isr_handler_names` identifies methods that run in ISR (`H`) context,
/// derived from the interrupt extraction pass.
pub fn extract_synchronization_surface<'tcx>(
    tcx: TyCtxt<'tcx>,
    isr_handler_names: &HashSet<String>,
) -> Result<SynchronizationSurface, SynchronizationError> {
    let mut sync_fields = Vec::new();
    let mut lock_operations = Vec::new();
    let mut completion_operations = Vec::new();
    let mut guard_scopes = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        match &item.kind {
            ItemKind::Struct(_ident, _generics, variant_data) => {
                let fields = extract_sync_fields(variant_data);
                sync_fields.extend(fields);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    collect_sync_call_sites(
                        tcx,
                        impl_item,
                        &mut lock_operations,
                        &mut completion_operations,
                        &mut guard_scopes,
                    );
                }
            }
            _ => {}
        }
    }

    // ISR-context legality checks.
    for op in &lock_operations {
        if isr_handler_names.contains(&op.containing_fn)
            && !matches!(op.lock_kind, SyncFieldKind::IrqSpinLock)
        {
            return Err(SynchronizationError::NonIsrSafeLockInIsr(
                op.containing_fn.clone(),
            ));
        }
    }

    for op in &completion_operations {
        if isr_handler_names.contains(&op.containing_fn)
            && matches!(op.method, CompletionMethod::Wait)
        {
            return Err(SynchronizationError::CompletionWaitInIsr(
                op.containing_fn.clone(),
            ));
        }
    }

    Ok(SynchronizationSurface {
        sync_fields,
        lock_operations,
        completion_operations,
        guard_scopes,
    })
}

// ---------------------------------------------------------------------------
// Sync field extraction
// ---------------------------------------------------------------------------

fn extract_sync_fields(variant_data: &rustc_hir::VariantData<'_>) -> Vec<SyncFieldDecl> {
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        if let Some((kind, payload_type)) = classify_sync_type(field.ty) {
            fields.push(SyncFieldDecl {
                field_name: field.ident.name.as_str().to_string(),
                kind,
                payload_type,
            });
        }
    }

    fields
}

fn classify_sync_type(ty: &rustc_hir::Ty<'_>) -> Option<(SyncFieldKind, Option<String>)> {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let name = last_seg.ident.name.as_str();

    let kind = match name {
        "Mutex" => SyncFieldKind::Mutex,
        "SpinLock" => SyncFieldKind::SpinLock,
        "IrqSpinLock" => SyncFieldKind::IrqSpinLock,
        "Completion" => return Some((SyncFieldKind::Completion, None)),
        _ => return None,
    };

    // Extract generic payload type name.
    let payload = last_seg.args.and_then(|args| {
        for arg in args.args.iter() {
            if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, inner_path)) =
                    &inner_ty.kind
                {
                    return Some(
                        inner_path
                            .segments
                            .iter()
                            .map(|s| s.ident.name.as_str().to_string())
                            .collect::<Vec<_>>()
                            .join("::"),
                    );
                }
            }
        }
        None
    });

    Some((kind, payload))
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_sync_call_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    lock_ops: &mut Vec<LockOperation>,
    completion_ops: &mut Vec<CompletionOperation>,
    guard_scopes: &mut Vec<GuardScope>,
) {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = SyncCallSiteVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            lock_ops: Vec::new(),
            completion_ops: Vec::new(),
            guard_scopes: Vec::new(),
        };
        visitor.visit_expr(body.value);
        lock_ops.extend(visitor.lock_ops);
        completion_ops.extend(visitor.completion_ops);
        guard_scopes.extend(visitor.guard_scopes);
    }
}

struct SyncCallSiteVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    lock_ops: Vec<LockOperation>,
    completion_ops: Vec<CompletionOperation>,
    guard_scopes: Vec<GuardScope>,
}

impl<'tcx> Visitor<'tcx> for SyncCallSiteVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, receiver, _, _) = expr.kind {
            let method_name = segment.ident.name.as_str();
            let receiver_ty = self.typeck_results.expr_ty(receiver);
            let type_name = sync_type_name(self.tcx, receiver_ty);

            match (type_name.as_deref(), method_name) {
                (Some(ty_name @ ("Mutex" | "SpinLock" | "IrqSpinLock")), "lock") => {
                    let kind = match ty_name {
                        "Mutex" => SyncFieldKind::Mutex,
                        "SpinLock" => SyncFieldKind::SpinLock,
                        _ => SyncFieldKind::IrqSpinLock,
                    };
                    self.lock_ops.push(LockOperation {
                        lock_kind: kind,
                        containing_fn: self.current_fn.clone(),
                    });
                    self.guard_scopes.push(GuardScope {
                        lock_kind: kind,
                        containing_fn: self.current_fn.clone(),
                    });
                }
                (Some("Completion"), "wait") => {
                    self.completion_ops.push(CompletionOperation {
                        method: CompletionMethod::Wait,
                        containing_fn: self.current_fn.clone(),
                    });
                }
                (Some("Completion"), "signal") => {
                    self.completion_ops.push(CompletionOperation {
                        method: CompletionMethod::Signal,
                        containing_fn: self.current_fn.clone(),
                    });
                }
                (Some("Completion"), "reset") => {
                    self.completion_ops.push(CompletionOperation {
                        method: CompletionMethod::Reset,
                        containing_fn: self.current_fn.clone(),
                    });
                }
                _ => {}
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

fn sync_type_name<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<String> {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str().to_string())
}
