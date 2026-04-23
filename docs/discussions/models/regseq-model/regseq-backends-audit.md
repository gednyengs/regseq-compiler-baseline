# regseq-backends Audit (DL-4)

## Status

Output of step 3 of the legacy-removal plan
(`legacy-removal-plan.md`). Classifies every `.rs` file under
`crates/regseq-backends/src/` into `DISCARD`, `ADAPT`, or `MIXED`.
This document is the authoritative input to the Wave A deletion
scope.

## Methodology

Two independent audits run in parallel:

- **Claude Explore agent** — per-file inventory of imports,
  purpose, and classification.
- **Codex** — same ask, separate pass, with file:line grounding.

Both audits agreed on the overall shape and on ~75% of files.
Divergences were reconciled in favor of the more conservative
classification (prefer MIXED over DISCARD when domain knowledge is
embedded, even if the substrate is dead).

## Classification buckets

- **DISCARD** — core input type is a legacy type being deleted
  (`SemanticModel`, `SemanticBody`, `SemanticType`, `AnalysisInput`,
  `ItemStore`, `CanonicalPath`, legacy `Stmt`/`Expr`) or the module
  is a thin orchestration wrapper around dead substrate. Remove in
  Wave A.
- **ADAPT** — embodies OS-agnostic or Linux-specific scaffolding
  whose domain content is valuable. Keep as-is through Wave A; may
  need minor import adjustments after Waves B-D, but no rewrite.
- **MIXED** — both kinds. Keep the file in place through Wave A as
  reference material; when backends are re-introduced, extract the
  domain knowledge and rewrite around RegseqModel. Do **not** delete
  in Wave A — the embedded knowledge (Linux kernel patterns, MIR
  classification heuristics, etc.) is re-used in the rewrite.

## Classification table

### Top-level (`src/`)

| File | Class | Reason |
|---|---|---|
| `lib.rs` | follows-exports | Barrel only; edit to reflect what survives. |
| `zephyr.rs` | DISCARD | 1-line empty stub. |

### `src/c/`

| File | Class | Reason |
|---|---|---|
| `mod.rs` | ADAPT | Backend-neutral C11 translation-unit AST. |
| `declarators.rs` | ADAPT | C type/declarator AST. No legacy deps. |
| `expressions.rs` | ADAPT | C expression AST. No legacy deps. |
| `initializers.rs` | ADAPT | C initializer AST (designated initializers). No legacy deps. |
| `statements.rs` | ADAPT | C statement AST. No legacy deps. |

**Directory-level**: uniformly ADAPT. Preserve wholesale.

### `src/linux/`

| File | Class | Reason |
|---|---|---|
| `mod.rs` | follows-exports | Barrel; update after file-level decisions. |
| `body_index.rs` | ADAPT | Already target-native (`FunctionId` -> `ir::Body` over `RegseqModel`). Imports at `linux/body_index.rs:25-27`. |
| `conventions.rs` | ADAPT | Linux kernel idioms (SPDX banner, naming). No legacy deps. |
| `c_ast.rs` | DISCARD | Shim re-exporting `c::*`. Re-import from `c::` directly in the re-port. |
| `emit_regs.rs` | ADAPT | Register helper header emission (offsets, masks, named values). Plan-driven; no legacy deps. |
| `emitter.rs` | MIXED | File I/O orchestration survives; main body path delegates to legacy emitters. `linux/emitter.rs:15`. |
| `plan.rs` | MIXED | Registration/probe/serial-core/register-layout struct definitions are valuable; embedded `SemanticModel` field at `linux/plan.rs:11` is legacy. |
| `planner.rs` | MIXED | Linux realization logic (registration shape, serial-core mapping, probe orchestration) worth re-porting; legacy imports at `linux/planner.rs:7-9`. |
| `emit_behavior.rs` | MIXED | `BehaviorNode` -> Linux C mapping; API mapping valuable, input substrate dead. |
| `emit_module.rs` | MIXED | Linux platform_driver / probe / remove / uart_ops / ISR templates embedded; legacy `SemanticModel`/`SemanticBody`/`Stmt`/`Expr` imports at `linux/emit_module.rs:14-19`. |
| `emit_semantic.rs` | MIXED | Linux C lowering rules (operator mapping, resource APIs) embedded; legacy imports at `linux/emit_semantic.rs:12-22`. |

### `src/lowering/` (top-level)

| File | Class | Reason |
|---|---|---|
| `mod.rs` | follows-exports | Barrel. |
| `from_model.rs` | ADAPT | RegseqModel-native lowering entry. Already consumes target types (`lowering/from_model.rs:28-33`). |
| `plan.rs` | ADAPT | Portable plan types, already using `RegseqType` for user-data (`lowering/plan.rs:141`). |
| `diagnostics.rs` | ADAPT | Reporting-only structs. No dep on any IR. |
| `behavior.rs` | ADAPT | Portable behavior vocabulary; minor `DelayPathKind`/`DelayIntrinsic` import replaceable. |
| `planner.rs` | MIXED | Ordering/resource logic reusable; entry/projection path is legacy (`lowering/planner.rs:19-22`). |
| `behavior_planner.rs` | DISCARD | Thin `AnalysisInput` -> `BehaviorPlan` wrapper. |
| `body_interpreter.rs` | DISCARD | Legacy HIR body interpreter; explicitly deprecated. |
| `hierarchy.rs` | DISCARD | Builds `ItemHierarchy` (legacy substrate). Phase 2A collapses. |
| `projection.rs` | DISCARD | Projects `SemanticModel` — the output type is being deleted. |

### `src/lowering/mir_interpreter/`

| File | Class | Reason |
|---|---|---|
| `mod.rs` | DISCARD | `AnalysisInput`-centric orchestration wrapper. |
| `context.rs` | DISCARD | `AnalysisInput`-backed metadata assembly. |
| `call_classifier.rs` | MIXED | MIR call classification heuristics (register write, resource op) are reusable; consumes `MirTy`. |
| `cfg_walker.rs` | MIXED | CFG traversal + cleanup recognition logic worth re-porting. |
| `cfg_helpers.rs` | MIXED | Loop/join/`?`-operator heuristics. |
| `place_resolver.rs` | MIXED | Self/mmio/register/resource place resolution. |
| `value_lowering.rs` | MIXED | Value/condition lowering with interrupt/branch patterns. |

Note: the MIXED files depend on `MirTy` from `declaration/mir_body.rs`,
which relocates to `regseq-driver` in Wave D. After Wave D their
imports need updating even though the file contents stay.

### `src/lowering/semantic_lowering/`

Every file imports `SemanticType`, `SemanticBody`, legacy `Stmt`/`Expr`,
`ItemStore`, `CanonicalPath`, `AnalysisInput`, or combinations.

| File | Class |
|---|---|
| `mod.rs` | DISCARD |
| `context.rs` | DISCARD |
| `expr_lowering.rs` | DISCARD |
| `call_lowering.rs` | DISCARD |
| `type_lowering.rs` | DISCARD |
| `semantic_walker.rs` | DISCARD |
| `ref_resolver.rs` | DISCARD |
| `external_items.rs` | DISCARD |
| `local_env.rs` | DISCARD |
| `option_lowering.rs` | DISCARD |
| `validation.rs` | DISCARD |

**Directory-level**: uniformly DISCARD. Delete wholesale in Wave A.

## Summary counts

| Class | Count |
|---|---|
| DISCARD | 15 |
| ADAPT | 12 |
| MIXED | 13 |
| follows-exports | 3 |

Total: 43 `.rs` files inventoried.

## Group recommendations

**Delete wholesale in Wave A**:
- `src/lowering/semantic_lowering/` (11 files, entire directory)
- `src/lowering/behavior_planner.rs`
- `src/lowering/body_interpreter.rs`
- `src/lowering/hierarchy.rs`
- `src/lowering/projection.rs`
- `src/lowering/mir_interpreter/mod.rs`
- `src/lowering/mir_interpreter/context.rs`
- `src/linux/c_ast.rs`
- `src/zephyr.rs`

**Preserve wholesale**:
- `src/c/` (5 files, entire directory)

**Preserve as reference (MIXED — re-ported when backends return)**:
- Linux emission domain knowledge: `linux/{emit_module, emit_semantic, emit_behavior, emit_regs, emitter, plan, planner, body_index, conventions}.rs`
- Portable lowering target path: `lowering/{from_model, plan, behavior, diagnostics}.rs`
- MIR interpretation substrate: `lowering/mir_interpreter/{call_classifier, cfg_walker, cfg_helpers, place_resolver, value_lowering}.rs`
- Legacy planner with salvageable ordering logic: `lowering/planner.rs`

**Follows-exports** (update after file-level decisions):
- `src/lib.rs`, `src/linux/mod.rs`, `src/lowering/mod.rs`

## Wave A action list (backends portion)

Ordered, applied after the audit is committed:

1. Delete `src/lowering/semantic_lowering/` (directory).
2. Delete `src/lowering/behavior_planner.rs`, `body_interpreter.rs`,
   `hierarchy.rs`, `projection.rs`.
3. Delete `src/lowering/mir_interpreter/{mod,context}.rs`. The
   remaining mir_interpreter files need a new `mod.rs` exposing only
   the ADAPT/MIXED survivors; or the directory is restructured
   wholesale.
4. Delete `src/linux/c_ast.rs`, `src/zephyr.rs`.
5. Update `src/lib.rs`, `src/linux/mod.rs`, `src/lowering/mod.rs` to
   reflect the surviving module tree.
6. Gate the MIXED/ADAPT survivors behind `#[cfg(feature = "legacy-reference")]`
   or similar, or leave them compiling with stubbed usage from
   `regseq-driver`'s removed `emit_driver.rs`. (Exact mechanism
   depends on what Wave A's `emit_driver.rs` removal leaves behind.)

## Notes on reconciliation

Divergences between the two audits and how they were resolved:

- **`semantic_lowering/{type_lowering,call_lowering,semantic_walker,
  local_env,validation}.rs`**: Explore marked ADAPT (portable logic);
  Codex marked DISCARD (dead substrate). Resolved as DISCARD. The
  logic may be useful as reference, but these files consume `MirTy`
  or produce `SemanticType`; extracting the insight into new code
  is faster than picking through legacy substrate. The pattern is
  preserved in git history at the `pre-legacy-removal` tag.
- **`linux/emit_semantic.rs`, `linux/emit_module.rs`**: Explore
  marked DISCARD (core input is legacy); Codex marked MIXED (Linux
  kernel domain knowledge is reusable). Resolved as MIXED. These
  files embody hard-won knowledge about Linux platform_driver /
  uart_ops / probe / remove wiring that should not be re-discovered.
- **`linux/c_ast.rs`**: DISCARD. Single-line shim; import from
  `c::` directly in the re-port.
- **`lowering/planner.rs`**: MIXED (Codex). Ordering/resource logic
  is worth preserving through Wave A. Explore's DISCARD was driven
  by the legacy entry point; the salvageable content is the ordering
  heuristics, which stay for the re-port.

## DL-4 resolution

The `regseq-backends` crate is **preserved** through Wave A, with
the DISCARD files deleted and the ADAPT/MIXED files retained.

After Wave A:
- The crate continues to compile with a reduced surface.
- No production pipeline consumes it (the `emit_driver.rs` chain
  that called into `regseq-backends` is also removed in Wave A; the
  `EmitSpec::Driver` dispatch stubs out per DL-3).
- When backends are re-introduced, the ADAPT content survives
  unchanged and the MIXED content is rewritten to consume
  `RegseqModel` directly.

Full deletion of the crate is **not** recommended. The `c/` AST
and the Linux domain knowledge embedded in MIXED files are worth
more than the cost of maintaining a mostly-idle crate through the
rebuild period.
