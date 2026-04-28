use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{ExprKind, ImplItemKind, ItemKind, StmtKind};
use rustc_middle::ty::TyCtxt;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the ordering surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingSurface {
    pub ordered_regions: Vec<OrderedRegion>,
}

/// An ordered region detected from a block bracketed by canonical barrier
/// intrinsic calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedRegion {
    pub containing_fn: String,
    pub pre_barrier_kind: String,
    pub post_barrier_kind: String,
    pub enclosed_wrapper_calls: Vec<String>,
    pub enclosed_poll_calls: usize,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderingError {
    /// A block starts with a `memory_barrier` call but does not end with one.
    UnpairedBarrier(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the ordering surface from HIR after analysis.
pub fn extract_ordering_surface(tcx: TyCtxt<'_>) -> Result<OrderingSurface, OrderingError> {
    let mut ordered_regions = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        if let ItemKind::Impl(impl_block) = &item.kind {
            for impl_item_ref in impl_block.items {
                let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                    owner_id: impl_item_ref.owner_id,
                });
                collect_ordered_regions(tcx, impl_item, &mut ordered_regions)?;
            }
        }
    }

    Ok(OrderingSurface { ordered_regions })
}

fn collect_ordered_regions<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    regions: &mut Vec<OrderedRegion>,
) -> Result<(), OrderingError> {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let mut visitor = OrderingVisitor {
            current_fn: fn_name,
            regions: Vec::new(),
            error: None,
        };
        visitor.visit_expr(body.value);
        if let Some(err) = visitor.error {
            return Err(err);
        }
        regions.extend(visitor.regions);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Block-pattern visitor
// ---------------------------------------------------------------------------

struct OrderingVisitor {
    current_fn: String,
    regions: Vec<OrderedRegion>,
    error: Option<OrderingError>,
}

impl<'tcx> Visitor<'tcx> for OrderingVisitor {
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        if self.error.is_some() {
            return;
        }

        if let Some(result) = self.try_extract_ordered_region(block) {
            match result {
                Ok(region) => {
                    self.regions.push(region);
                    return;
                }
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }

        intravisit::walk_block(self, block);
    }
}

impl OrderingVisitor {
    fn try_extract_ordered_region(
        &self,
        block: &rustc_hir::Block<'_>,
    ) -> Option<Result<OrderedRegion, OrderingError>> {
        let stmts = block.stmts;
        if stmts.len() < 2 {
            return None;
        }

        let pre_kind = extract_barrier_from_stmt(&stmts[0])?;

        let post_kind = extract_barrier_from_stmt(&stmts[stmts.len() - 1]);
        if post_kind.is_none() {
            return Some(Err(OrderingError::UnpairedBarrier(self.current_fn.clone())));
        }

        let middle = &stmts[1..stmts.len() - 1];
        let (wrapper_calls, poll_calls) = extract_enclosed_ops(middle);

        Some(Ok(OrderedRegion {
            containing_fn: self.current_fn.clone(),
            pre_barrier_kind: pre_kind,
            post_barrier_kind: post_kind.unwrap(),
            enclosed_wrapper_calls: wrapper_calls,
            enclosed_poll_calls: poll_calls,
        }))
    }
}

// ---------------------------------------------------------------------------
// Barrier detection
// ---------------------------------------------------------------------------

fn extract_barrier_from_stmt(stmt: &rustc_hir::Stmt<'_>) -> Option<String> {
    let StmtKind::Semi(expr) = &stmt.kind else {
        return None;
    };
    let ExprKind::Call(callee, args) = &expr.kind else {
        return None;
    };
    if !is_memory_barrier_path(callee) {
        return None;
    }
    if args.len() != 1 {
        return None;
    }
    extract_barrier_kind(&args[0])
}

fn is_memory_barrier_path(expr: &rustc_hir::Expr<'_>) -> bool {
    if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind {
        let segs: Vec<&str> = path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect();
        segs.len() >= 2
            && segs[segs.len() - 2] == "sync"
            && segs[segs.len() - 1] == "memory_barrier"
    } else {
        false
    }
}

fn extract_barrier_kind(expr: &rustc_hir::Expr<'_>) -> Option<String> {
    if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind {
        path.segments
            .last()
            .map(|s| s.ident.name.as_str().to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Enclosed-operation collection
// ---------------------------------------------------------------------------

fn extract_enclosed_ops(stmts: &[rustc_hir::Stmt<'_>]) -> (Vec<String>, usize) {
    let mut visitor = EnclosedOpsVisitor {
        wrapper_calls: Vec::new(),
        poll_calls: 0,
    };
    for stmt in stmts {
        visitor.visit_stmt(stmt);
    }
    (visitor.wrapper_calls, visitor.poll_calls)
}

struct EnclosedOpsVisitor {
    wrapper_calls: Vec<String>,
    poll_calls: usize,
}

impl<'tcx> Visitor<'tcx> for EnclosedOpsVisitor {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        match &expr.kind {
            ExprKind::MethodCall(segment, _, _, _) => {
                let name = segment.ident.name.as_str();
                if matches!(name, "read" | "write" | "modify" | "set") {
                    self.wrapper_calls.push(name.to_string());
                }
            }
            ExprKind::Call(callee, _) => {
                if is_poll_path(callee) {
                    self.poll_calls += 1;
                }
            }
            _ => {}
        }
        intravisit::walk_expr(self, expr);
    }
}

fn is_poll_path(expr: &rustc_hir::Expr<'_>) -> bool {
    if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind {
        let segs: Vec<&str> = path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect();
        segs.len() >= 2 && segs[segs.len() - 2] == "poll" && segs[segs.len() - 1] == "until"
    } else {
        false
    }
}
