//! Codegen experiment: MIR-structured C code generation.
//!
//! This module orchestrates the experiment that compares three C emission
//! strategies:
//!
//! 1. Naive goto-based emission from raw MIR basic blocks.
//! 2. Structured emission after CFG structuring (if/else/while/switch).
//! 3. Decorated emission: structured + HIR-derived names and DSL comments.
//!
//! The entry point is [`run`], called from `after_analysis`.

mod c_emit;
mod fold;
mod hir_collect;
mod mir_collect;
mod structure;
pub mod types;

use std::fmt::Write;
use std::path::Path;

use rustc_middle::ty::TyCtxt;

/// Run the codegen experiment and return a report string.
///
/// The report contains, for each function in the crate:
/// - The raw MIR dump (basic blocks).
/// - Naive C output (goto-based).
/// - Structured C output.
/// - Folded C output (structured + expression folding).
/// - Decorated C output (with HIR info).
pub fn run(tcx: TyCtxt<'_>, output_dir: Option<&Path>) -> String {
    let hir = hir_collect::collect_hir(tcx);
    let mir_funcs = mir_collect::collect_mir(tcx);

    let mut report = String::new();
    let _ = writeln!(report, "{}", "=".repeat(72));
    let _ = writeln!(report, "  CODEGEN EXPERIMENT REPORT");
    let _ = writeln!(report, "{}", "=".repeat(72));
    let _ = writeln!(report);

    // --- HIR Summary -------------------------------------------------------
    let _ = writeln!(report, "--- HIR Summary ---");
    let _ = writeln!(report);
    for s in &hir.structs {
        let _ = write!(report, "struct {}", s.name);
        for attr in &s.rseq_attrs {
            let _ = write!(report, " [rseq::{}]", attr.kind);
        }
        let _ = writeln!(report);
        for (fname, fty) in &s.fields {
            let _ = writeln!(report, "    {fname}: {fty}");
        }
    }
    for f in &hir.functions {
        let _ = write!(report, "fn {}", f.short_name);
        for attr in &f.rseq_attrs {
            let _ = write!(report, " [rseq::{}]", attr.kind);
        }
        let _ = write!(report, "(");
        for (i, (pname, pty)) in f.params.iter().enumerate() {
            if i > 0 {
                let _ = write!(report, ", ");
            }
            let _ = write!(report, "{pname}: {pty}");
        }
        let _ = writeln!(report, ") -> {}", f.return_ty);
    }
    let _ = writeln!(report);

    // --- Per-function analysis ---------------------------------------------
    for func in &mir_funcs {
        let _ = writeln!(report, "{}", "=".repeat(72));
        let _ = writeln!(report, "  FUNCTION: {}", func.name);
        let _ = writeln!(report, "{}", "=".repeat(72));
        let _ = writeln!(report);

        // -- Raw MIR --------------------------------------------------------
        let _ = writeln!(report, "--- Raw MIR ---");
        let _ = writeln!(report);
        let _ = writeln!(report, "locals:");
        for local in &func.locals {
            let dbg = func
                .debug_names
                .get(&local.index)
                .map(|n| format!(" (= {n})"))
                .unwrap_or_default();
            let role = if local.index == 0 {
                " [return]"
            } else if local.is_arg {
                " [arg]"
            } else {
                ""
            };
            let _ = writeln!(
                report,
                "    _{}: {}{role}{dbg}",
                local.index, local.ty
            );
        }
        let _ = writeln!(report);
        for blk in &func.blocks {
            let cleanup_tag = if blk.is_cleanup { " (cleanup)" } else { "" };
            let _ = writeln!(report, "  bb{}{cleanup_tag}:", blk.index);
            for stmt in &blk.statements {
                let _ = writeln!(report, "      {stmt};");
            }
            let _ = writeln!(report, "      {};", blk.terminator);
            let _ = writeln!(report);
        }

        // -- Naive C --------------------------------------------------------
        let _ = writeln!(report, "--- Naive C (goto-based) ---");
        let _ = writeln!(report);
        let _ = writeln!(report, "{}", c_emit::emit_naive(func));

        // -- Structured C ---------------------------------------------------
        let structured = structure::structurize(func);
        let _ = writeln!(report, "--- Structured C ---");
        let _ = writeln!(report);
        let _ = writeln!(report, "{}", c_emit::emit_structured(func, &structured));

        // -- Folded C -------------------------------------------------------
        let mut folded = structured.clone();
        fold::fold(&mut folded);
        let _ = writeln!(report, "--- Folded C (structured + expression folding) ---");
        let _ = writeln!(report);
        let _ = writeln!(report, "{}", c_emit::emit_structured(func, &folded));

        // -- Decorated C ----------------------------------------------------
        let _ = writeln!(report, "--- Decorated C (HIR-augmented, folded) ---");
        let _ = writeln!(report);
        let _ = writeln!(report, "{}", c_emit::emit_decorated(func, &folded, &hir));
    }

    // Write to file if output_dir is provided.
    if let Some(dir) = output_dir {
        let path = dir.join("codegen_experiment_report.txt");
        match std::fs::write(&path, &report) {
            Ok(()) => {
                eprintln!("regseq-experiment: report written to {}", path.display());
            }
            Err(e) => {
                eprintln!(
                    "regseq-experiment: failed to write report to {}: {e}",
                    path.display()
                );
            }
        }
    }

    report
}
