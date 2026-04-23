//! Linux kernel C code generation backend.
//!
//! During the RegseqModel-only rebuild, only the two target-native
//! ADAPT files survive: `body_index` (maps `FunctionId` to `ir::Body`
//! over a `RegseqModel`) and `conventions` (Linux kernel C idioms).
//!
//! The prior MIXED files -- `plan`, `planner`, `emit_module`,
//! `emit_semantic`, `emit_behavior`, `emit_regs`, `emitter` -- were
//! removed in Wave B because they depended on the deleted
//! `SemanticModel` / `compat` / `item::function::FunctionRole`
//! substrate. Their domain knowledge (platform_driver template,
//! uart_ops wiring, probe/remove sequencing, register helper
//! emission, kernel API mapping) is preserved at the
//! `pre-legacy-removal` git tag and will be re-ported against
//! `RegseqModel` when backends return.

pub mod body_index;
pub mod conventions;
