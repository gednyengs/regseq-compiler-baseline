//! The model-independent bank-1 prelude (`generic-c-backend.md` section 15
//! bank 1) plus the fixed arithmetic / cast helper block (section 10 /
//! section 11).
//!
//! Everything in this module is a pure function of nothing (a `const`) or of
//! a fixed type set (`helper_block`); none of it reads the model. The per-`T`
//! fat-pointer structs (`rust_slice_<T>`, `trait_obj_<Trait>`, section 3.2)
//! are type-driven and belong to task c -- only the monomorphic
//! `struct rust_str` ships here. Use-site lowering (the `*WithOverflow` /
//! saturating helpers at `BinaryOp` / `Cast` sites) is task d; this module
//! emits only their definitions.
//!
//! All file-scope helpers are `static inline`: in a single translation unit
//! an unused `static inline` definition is warning-free under `-Wall`,
//! whereas a plain `static` routine would warn "defined but not used" when a
//! TU has no call site (e.g. the standalone-prelude compile test).

/// The static prelude: includes, the section 3.2 `struct rust_str`, the
/// section 16 panic infrastructure, and the section 16 `AssertKind` message
/// catalog. Emitted verbatim as bank 1's leading, model-independent text.
///
/// Compiles standalone under the section 7 flags (`-std=gnu11 -fwrapv
/// -Wall`) with no warnings.
pub const PRELUDE: &str = r#"/* ===========================================================================
 * Generic-C backend translation unit (generic-c-backend.md section 15).
 * Bank 1: static prelude (includes + fat-pointer typedef + panic + helpers).
 * Banks 2-5 (types / declarations / definitions) follow, model-driven.
 * =========================================================================== */

#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

/* section 3.2: the one monomorphic fat-pointer typedef. The per-T
 * rust_slice_<T> / trait_obj_<Trait> structs are type-driven (task c). */
struct rust_str {
    const uint8_t *data;
    size_t len;
};

/* section 16 (D4): the host panic routine. Prints to stderr -- never stdout
 * -- so it cannot pollute the section 8 diag-log stdout oracle; the harness
 * recognizes a panic by non-zero exit + this stderr text. static inline so
 * the definition is warning-free under -Wall when a TU has no panic site. */
_Noreturn static inline void regseq_panic(const char *msg) {
    fprintf(stderr, "panic: %s\n", msg);
    abort();
}

/* section 16: Unreachable / panic=abort trap. The Phase-1 default is the
 * loud trap (not __builtin_unreachable) so the host oracle fails visibly. */
_Noreturn static inline void regseq_unreachable(void) {
    regseq_panic("internal error: entered unreachable code");
}

/* section 16 AssertKind message catalog. Fixed messages are used verbatim by
 * the task-d Assert terminator site as the regseq_panic argument; the
 * operand-bearing messages are printf-style templates whose %s positions the
 * task-d site fills with each operand rendered by its Ty (the values live at
 * the site, not here -- scope decision 3). One #define per AssertKind row of
 * the section 16 table; #defines never warn when unused. */
#define REGSEQ_MSG_BOUNDS_CHECK "index out of bounds: the len is %s but the index is %s" /* BoundsCheck{len,index} */
#define REGSEQ_MSG_OVERFLOW "attempt to %s with overflow"                                /* Overflow(op,l,r); %s = operator verb */
#define REGSEQ_MSG_OVERFLOW_NEG "attempt to negate with overflow"                        /* OverflowNeg(v) */
#define REGSEQ_MSG_DIVISION_BY_ZERO "attempt to divide by zero"                          /* DivisionByZero(v) */
#define REGSEQ_MSG_REMAINDER_BY_ZERO "attempt to calculate the remainder with a divisor of zero" /* RemainderByZero(v) */
#define REGSEQ_MSG_MISALIGNED "misaligned pointer dereference: address must be a multiple of %s but is %s" /* MisalignedPointerDereference{required,found} */
#define REGSEQ_MSG_NULL_POINTER_DEREFERENCE "null pointer dereference occurred"          /* NullPointerDereference */
#define REGSEQ_MSG_INVALID_ENUM_CONSTRUCTION "constructed invalid enum discriminant %s"  /* InvalidEnumConstruction(v) */
"#;

/// `(c_type, min_expr, max_expr)` for every primitive integer type in the
/// fixed helper set (section 10 / section 11). The order is fixed so the
/// emitted block is byte-stable. The 128-bit bounds reference the
/// `REGSEQ_(U)INT128_*` macros emitted at the top of the helper block (no
/// standard `*_MIN` / `*_MAX` macro exists for `__int128` / `__uint128_t`).
const INT_TYPES: &[(&str, &str, &str)] = &[
    ("int8_t", "INT8_MIN", "INT8_MAX"),
    ("int16_t", "INT16_MIN", "INT16_MAX"),
    ("int32_t", "INT32_MIN", "INT32_MAX"),
    ("int64_t", "INT64_MIN", "INT64_MAX"),
    ("intptr_t", "INTPTR_MIN", "INTPTR_MAX"),
    ("__int128", "REGSEQ_INT128_MIN", "REGSEQ_INT128_MAX"),
    ("uint8_t", "0", "UINT8_MAX"),
    ("uint16_t", "0", "UINT16_MAX"),
    ("uint32_t", "0", "UINT32_MAX"),
    ("uint64_t", "0", "UINT64_MAX"),
    ("size_t", "0", "SIZE_MAX"),
    ("__uint128_t", "0", "REGSEQ_UINT128_MAX"),
];

/// `(prefix, c_type, availability_macro)` for every source-float type of the
/// saturating `FloatToInt` helpers (section 11). `float` / `double` are
/// unconditional (empty macro); `_Float16` / `_Float128` are gated by a
/// source-level `#if defined(<macro>)`. The `#if` block is *always present in
/// the emitted text* -- the preprocessor, not the Rust backend, decides
/// whether to compile it -- so the emitted `.c` is byte-identical across
/// hosts (Doc 3 section 7), not branched on the build host.
const FLOAT_TYPES: &[(&str, &str, &str)] = &[
    ("f32", "float", ""),
    ("f64", "double", ""),
    ("f16", "_Float16", "__FLT16_MANT_DIG__"),
    ("f128", "_Float128", "__FLT128_MANT_DIG__"),
];

/// Emit the fixed arithmetic / cast helper block (section 10.1
/// `*WithOverflow` + section 11 saturating `FloatToInt`).
///
/// A fixed superset over the primitive type set (scope decision 2): the
/// helpers are *defined* here, *used* at task-d `BinaryOp` / `Cast` sites.
/// All helpers are `static inline`, so the unused ones are warning-free under
/// `-Wall`. Naming scheme (collision-free + stable across hosts):
///
/// - result struct: `struct regseq_overflow_<T>` (`{ <T> v; _Bool overflow; }`)
/// - overflow wrapper: `regseq_{add,sub,mul}_<T>(<T> a, <T> b)`
/// - saturating cast: `regseq_<f-prefix>_to_<T>(<F> x)`
///
/// where `<T>` is the integer C type spelling and `<f-prefix>` is
/// `f32`/`f64`/`f16`/`f128`. Output is a pure function of the fixed
/// `INT_TYPES` / `FLOAT_TYPES` tables, hence byte-stable.
pub fn helper_block() -> String {
    let mut out = String::new();

    out.push_str(
        "\n\
/* ---------------------------------------------------------------------------\n\
 * Arithmetic / cast helper block (section 10.1 + section 11), fixed superset.\n\
 * --------------------------------------------------------------------------- */\n\
\n\
/* 128-bit bounds: no standard *_MIN / *_MAX macro exists for __int128 /\n\
 * __uint128_t, so compute them (host two's-complement assumption). */\n\
#define REGSEQ_INT128_MAX ((__int128)(((__uint128_t)1 << 127) - 1))\n\
#define REGSEQ_INT128_MIN (-REGSEQ_INT128_MAX - 1)\n\
#define REGSEQ_UINT128_MAX (~(__uint128_t)0)\n\
\n",
    );

    // section 10.1: the `*WithOverflow` result structs + builtin wrappers.
    out.push_str("/* section 10.1: {add,sub,mul}WithOverflow -> (T, bool). */\n");
    for (ty, _min, _max) in INT_TYPES {
        out.push_str(&format!(
            "struct regseq_overflow_{ty} {{ {ty} v; _Bool overflow; }};\n"
        ));
        for op in ["add", "sub", "mul"] {
            out.push_str(&format!(
                "static inline struct regseq_overflow_{ty} regseq_{op}_{ty}({ty} a, {ty} b) {{\n\
                 \x20\x20\x20\x20struct regseq_overflow_{ty} r;\n\
                 \x20\x20\x20\x20r.overflow = __builtin_{op}_overflow(a, b, &r.v);\n\
                 \x20\x20\x20\x20return r;\n\
                 }}\n"
            ));
        }
    }

    // section 11: saturating FloatToInt (bounds checked *before* the cast;
    // NaN -> 0; +/-inf handled by the comparisons).
    out.push_str("\n/* section 11: saturating FloatToInt (NaN -> 0; bounds before cast). */\n");
    for (prefix, fty, avail_macro) in FLOAT_TYPES {
        let gated = !avail_macro.is_empty();
        if gated {
            out.push_str(&format!("#if defined({avail_macro})\n"));
        }
        for (ity, min, max) in INT_TYPES {
            out.push_str(&format!(
                "static inline {ity} regseq_{prefix}_to_{ity}({fty} x) {{\n\
                 \x20\x20\x20\x20if (isnan(x)) return 0;\n\
                 \x20\x20\x20\x20if (x <= ({fty}){min}) return {min};\n\
                 \x20\x20\x20\x20if (x >= ({fty}){max}) return {max};\n\
                 \x20\x20\x20\x20return ({ity})x;\n\
                 }}\n"
            ));
        }
        if gated {
            out.push_str("#endif\n");
        }
    }

    out
}
