// Foreign-decl rendering fixture for regseq-driver-59 (Test Plan B8).
// Mirrors the regseq-driver-58 B3 test source shape: an `extern "C"`
// block at the crate root declaring one foreign fn and one foreign
// static, plus a source-defined `entry` fn so the MonoItem walk has
// at least one starting point. The eager HIR walk added in
// regseq-driver-58 enumerates the foreign decls into `model.items`
// and `model.instances`.
//
// Driven by `crates/regseq-model/tests/e2e_foreign_rendering.rs`
// which bridges the fixture end-to-end, pretty-prints the resulting
// model, and snapshots the rendered output. The snapshot pins the
// `#[foreign]\nfn ...;` (Specimen 113) and `#[foreign]\nstatic ...;`
// (Specimen 120) rendering shapes per
// `docs/discussions/regseq-model/pretty-printer.md:2098-2117`
// and `:2236-2254`.

#![no_std]

extern "C" {
    pub fn extern_helper(x: u32) -> u32;
    pub static EXTERN_VALUE: u32;
}

pub fn entry() {}
