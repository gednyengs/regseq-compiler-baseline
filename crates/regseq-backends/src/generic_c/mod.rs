//! The generic-C backend driver: the `generic-c-backend.md` section 15
//! five-bank single-translation-unit assembly (Phase-1 task b).
//!
//! Task b ships the model-independent scaffolding: the bank-ordering driver
//! plus bank 1 (the [`prelude`] static prelude + helper block). Banks 2-5
//! (types / declarations / definitions) are model-driven and land in tasks
//! c/d; task b ships them as empty hooks so [`emit_generic_c`] returns a real
//! -- and standalone-compilable -- translation unit for any valid model.

use regseq_model::RegseqModel;

pub mod prelude;

/// Lower a post-mono [`RegseqModel`] to a single generic-C translation unit
/// in `generic-c-backend.md` section 15 five-bank order.
///
/// The five banks are assembled in order: (1) the prelude + helper block
/// (model-independent, this task), (2) type forward-decls, (3) type
/// definitions, (4) declarations, (5) definitions (banks 2-5 are task c/d
/// hooks, empty for now). The output is a *pure function of the model* (no
/// wall-clock / iteration nondeterminism) so the snapshot oracle's
/// byte-stability holds.
///
/// Returns `Ok(tu)` for any valid model; the bank hooks keep a `Result`
/// return so tasks c/d can later surface model-corruption errors (e.g. a
/// section 15 by-value containment cycle), but task b never errors.
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(prelude::PRELUDE);
    out.push_str(&prelude::helper_block());
    out.push_str(&bank2_type_forward_decls(model)?);
    out.push_str(&bank3_type_definitions(model)?);
    out.push_str(&bank4_declarations(model)?);
    out.push_str(&bank5_definitions(model)?);
    Ok(out)
}

/// Bank 2 (section 15): tag forward-decls for every aggregate in F4 order
/// (`struct <T>;` / `union <T>;`).
fn bank2_type_forward_decls(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    // TODO(task c): section 15 bank 2 -- forward-decl every aggregate.
    // Closes Doc-1 rows TY-17 / TY-18 (struct / enum type references) and the
    // tuple / closure aggregate rows.
    Ok(String::new())
}

/// Bank 3 (section 15): full `struct` / `union` bodies, topologically sorted
/// by by-value containment (the only load-bearing emission order).
fn bank3_type_definitions(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    // TODO(task c): section 15 bank 3 -- emit aggregate definitions + the
    // by-value-containment topo-sort. Closes Doc-1 rows IN-9 (struct layout),
    // IN-10 / LY-5 (enum -> tagged/niche struct), LY-3 (field offsets).
    Ok(String::new())
}

/// Bank 4 (section 15): function prototypes for every function instance and
/// `extern` declarations for every static.
fn bank4_declarations(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    // TODO(task c/d): section 15 bank 4 -- prototypes + extern statics.
    // Closes Doc-1 rows IN-2 (function prototype envelope), IN-7 / IN-8
    // (extern statics / consts). Diag-log exclusion (section 15): an instance
    // whose item carries IntrinsicKind::Log(..) is SKIPPED here (no
    // prototype) -- wired by task e.
    Ok(String::new())
}

/// Bank 5 (section 15): function bodies + static initializers, F4 order.
fn bank5_definitions(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    // TODO(task d): section 15 bank 5 -- function bodies + static
    // initializers. Closes Doc-1 rows IN-2 (body interior), BD-* / ST-* /
    // RV-* / BO-* / AG-* / CO-* (locals, statements, rvalues, binops,
    // aggregates, const initializers). Diag-log exclusion (section 15): a
    // Log(..) instance is SKIPPED here (no definition) -- wired by task e.
    Ok(String::new())
}

/// Errors from generic-C emission.
///
/// `#[non_exhaustive]` lets tasks c/d add variants (e.g. a detected section
/// 15 by-value containment cycle = model corruption) without a breaking
/// change. The `NotImplemented` marker from task a is retained as a
/// placeholder but is *no longer returned on the task-b happy path*: the
/// bank-2-5 hooks return `Ok(String::new())`, so [`emit_generic_c`] returns
/// `Ok` for any valid model.
#[derive(Debug)]
#[non_exhaustive]
pub enum EmitError {
    /// The generic-C backend lowering is not yet implemented. Retained as a
    /// placeholder; not returned by the task-b emit path.
    NotImplemented,
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::NotImplemented => write!(f, "generic-C emission not implemented"),
        }
    }
}

impl std::error::Error for EmitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::StableCrateId;

    /// A trivial empty model: bank 1 is model-independent, banks 2-5 are
    /// empty hooks, so the emitted TU is the prelude + helper block.
    fn trivial_model() -> RegseqModel {
        RegseqModel::with_primary("trivial", StableCrateId(1))
    }

    #[test]
    fn emit_returns_ok_and_never_not_implemented() {
        // Acceptance criterion 8: the happy path returns Ok for any valid
        // model; NotImplemented is never returned by task b's emit.
        let tu = emit_generic_c(&trivial_model()).expect("emit must return Ok");
        assert!(!tu.is_empty());
    }

    #[test]
    fn tu_starts_with_the_includes() {
        let tu = emit_generic_c(&trivial_model()).unwrap();
        // The prelude leads with the section 15 banner then the includes.
        assert!(tu.contains("#include <stdint.h>"));
        assert!(tu.contains("#include <stddef.h>"));
        assert!(tu.contains("#include <stdio.h>"));
        assert!(tu.contains("#include <stdlib.h>"));
        assert!(tu.contains("#include <string.h>"));
        assert!(tu.contains("#include <math.h>"));
        // includes precede the helper block.
        let inc = tu.find("#include <stdint.h>").unwrap();
        let helpers = tu.find("regseq_add_int8_t").unwrap();
        assert!(inc < helpers);
    }

    #[test]
    fn tu_contains_rust_str_panic_and_unreachable() {
        let tu = emit_generic_c(&trivial_model()).unwrap();
        assert!(tu.contains("struct rust_str {"));
        assert!(tu.contains("const uint8_t *data;"));
        assert!(tu.contains("_Noreturn static inline void regseq_panic(const char *msg)"));
        assert!(tu.contains("fprintf(stderr, \"panic: %s\\n\", msg);"));
        assert!(tu.contains("abort();"));
        assert!(tu.contains("internal error: entered unreachable code"));
    }

    #[test]
    fn tu_contains_the_assertkind_catalog() {
        let tu = emit_generic_c(&trivial_model()).unwrap();
        // All 8 section 16 AssertKind rows present (fixed + templated).
        assert!(tu.contains("index out of bounds: the len is %s but the index is %s"));
        assert!(tu.contains("attempt to %s with overflow"));
        assert!(tu.contains("attempt to negate with overflow"));
        assert!(tu.contains("attempt to divide by zero"));
        assert!(tu.contains("attempt to calculate the remainder with a divisor of zero"));
        assert!(tu.contains(
            "misaligned pointer dereference: address must be a multiple of %s but is %s"
        ));
        assert!(tu.contains("null pointer dereference occurred"));
        assert!(tu.contains("constructed invalid enum discriminant %s"));
    }

    #[test]
    fn tu_contains_overflow_and_saturating_helper_signatures() {
        let tu = emit_generic_c(&trivial_model()).unwrap();
        // *WithOverflow: result struct + the three wrappers, across the set.
        assert!(tu.contains("struct regseq_overflow_int8_t { int8_t v; _Bool overflow; };"));
        assert!(tu.contains(
            "static inline struct regseq_overflow_int32_t regseq_add_int32_t(int32_t a, int32_t b)"
        ));
        assert!(tu.contains("regseq_sub_uint64_t"));
        assert!(tu.contains("regseq_mul___int128"));
        assert!(tu.contains("__builtin_add_overflow(a, b, &r.v);"));
        // Saturating FloatToInt: f32 + f64 unconditional; f16 / f128 gated.
        assert!(tu.contains("static inline int32_t regseq_f64_to_int32_t(double x)"));
        assert!(tu.contains("static inline uint8_t regseq_f32_to_uint8_t(float x)"));
        assert!(tu.contains("if (isnan(x)) return 0;"));
        assert!(tu.contains("#if defined(__FLT16_MANT_DIG__)"));
        assert!(tu.contains("regseq_f16_to_int32_t"));
        assert!(tu.contains("#if defined(__FLT128_MANT_DIG__)"));
        assert!(tu.contains("regseq_f128_to_int64_t"));
    }

    #[test]
    fn emit_is_byte_identical_across_two_calls() {
        // Determinism (acceptance criterion 6): a pure function of the model.
        let a = emit_generic_c(&trivial_model()).unwrap();
        let b = emit_generic_c(&trivial_model()).unwrap();
        assert_eq!(a, b);
    }
}
