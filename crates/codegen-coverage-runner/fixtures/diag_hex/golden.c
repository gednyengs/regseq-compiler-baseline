/* ===========================================================================
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

/* ---------------------------------------------------------------------------
* Arithmetic / cast helper block (section 10.1 + section 11), demand-driven.
* --------------------------------------------------------------------------- */

/* 128-bit bounds: no standard *_MIN / *_MAX macro exists for __int128 /
* __uint128_t, so compute them (host two's-complement assumption). */
#define REGSEQ_INT128_MAX ((__int128)(((__uint128_t)1 << 127) - 1))
#define REGSEQ_INT128_MIN (-REGSEQ_INT128_MAX - 1)
#define REGSEQ_UINT128_MAX (~(__uint128_t)0)


/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct tuple_cbf29ce484222325;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct rust_slice_input_regseq_log_intrinsics_LogArg {
    const struct input_regseq_log_intrinsics_LogArg *data;
    size_t len;
};
struct input_regseq_log_intrinsics_LogArg {
    union {
    struct {
    uint8_t tag;
    unsigned char _pad0[23];
    } _tag;
    struct {
    unsigned char _pad0[1];
    uint8_t _0;
    unsigned char _pad1[22];
    } _v0;
    struct {
    unsigned char _pad0[2];
    uint16_t _0;
    unsigned char _pad1[20];
    } _v1;
    struct {
    unsigned char _pad0[4];
    uint32_t _0;
    unsigned char _pad1[16];
    } _v2;
    struct {
    unsigned char _pad0[8];
    uint64_t _0;
    unsigned char _pad1[8];
    } _v3;
    struct {
    unsigned char _pad0[1];
    int8_t _0;
    unsigned char _pad1[22];
    } _v4;
    struct {
    unsigned char _pad0[2];
    int16_t _0;
    unsigned char _pad1[20];
    } _v5;
    struct {
    unsigned char _pad0[4];
    int32_t _0;
    unsigned char _pad1[16];
    } _v6;
    struct {
    unsigned char _pad0[8];
    int64_t _0;
    unsigned char _pad1[8];
    } _v7;
    struct {
    unsigned char _pad0[1];
    _Bool _0;
    unsigned char _pad1[22];
    } _v8;
    struct {
    unsigned char _pad0[8];
    struct rust_str _0;
    } _v9;
    struct {
    uint8_t tag;
    uint8_t _0;
    unsigned char _pad0[22];
    } _c0;
    struct {
    uint8_t tag;
    unsigned char _pad0[1];
    uint16_t _0;
    unsigned char _pad1[20];
    } _c1;
    struct {
    uint8_t tag;
    unsigned char _pad0[3];
    uint32_t _0;
    unsigned char _pad1[16];
    } _c2;
    struct {
    uint8_t tag;
    unsigned char _pad0[7];
    uint64_t _0;
    unsigned char _pad1[8];
    } _c3;
    struct {
    uint8_t tag;
    int8_t _0;
    unsigned char _pad0[22];
    } _c4;
    struct {
    uint8_t tag;
    unsigned char _pad0[1];
    int16_t _0;
    unsigned char _pad1[20];
    } _c5;
    struct {
    uint8_t tag;
    unsigned char _pad0[3];
    int32_t _0;
    unsigned char _pad1[16];
    } _c6;
    struct {
    uint8_t tag;
    unsigned char _pad0[7];
    int64_t _0;
    unsigned char _pad1[8];
    } _c7;
    struct {
    uint8_t tag;
    _Bool _0;
    unsigned char _pad0[22];
    } _c8;
    struct {
    uint8_t tag;
    unsigned char _pad0[7];
    struct rust_str _0;
    } _c9;
    unsigned char _raw[24];
    } u;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_regseq_log_intrinsics_LogArg) == 24, "input_regseq_log_intrinsics_LogArg size");
_Static_assert(_Alignof(struct input_regseq_log_intrinsics_LogArg) == 8, "input_regseq_log_intrinsics_LogArg align");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._tag.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._tag.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v0._0) == 1, "input_regseq_log_intrinsics_LogArg.u._v0._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v1._0) == 2, "input_regseq_log_intrinsics_LogArg.u._v1._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v2._0) == 4, "input_regseq_log_intrinsics_LogArg.u._v2._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v3._0) == 8, "input_regseq_log_intrinsics_LogArg.u._v3._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v4._0) == 1, "input_regseq_log_intrinsics_LogArg.u._v4._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v5._0) == 2, "input_regseq_log_intrinsics_LogArg.u._v5._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v6._0) == 4, "input_regseq_log_intrinsics_LogArg.u._v6._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v7._0) == 8, "input_regseq_log_intrinsics_LogArg.u._v7._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v8._0) == 1, "input_regseq_log_intrinsics_LogArg.u._v8._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._v9._0) == 8, "input_regseq_log_intrinsics_LogArg.u._v9._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c0.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c0.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c0._0) == 1, "input_regseq_log_intrinsics_LogArg.u._c0._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c1.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c1.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c1._0) == 2, "input_regseq_log_intrinsics_LogArg.u._c1._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c2.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c2.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c2._0) == 4, "input_regseq_log_intrinsics_LogArg.u._c2._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c3.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c3.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c3._0) == 8, "input_regseq_log_intrinsics_LogArg.u._c3._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c4.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c4.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c4._0) == 1, "input_regseq_log_intrinsics_LogArg.u._c4._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c5.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c5.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c5._0) == 2, "input_regseq_log_intrinsics_LogArg.u._c5._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c6.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c6.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c6._0) == 4, "input_regseq_log_intrinsics_LogArg.u._c6._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c7.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c7.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c7._0) == 8, "input_regseq_log_intrinsics_LogArg.u._c7._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c8.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c8.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c8._0) == 1, "input_regseq_log_intrinsics_LogArg.u._c8._0 offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c9.tag) == 0, "input_regseq_log_intrinsics_LogArg.u._c9.tag offset");
_Static_assert(offsetof(struct input_regseq_log_intrinsics_LogArg, u._c9._0) == 8, "input_regseq_log_intrinsics_LogArg.u._c9._0 offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_52a7663f6fc8d4522c6062080ce2e41a[41];
extern const uint8_t input__regseq_synth__const_memory_static_5956483e6acaddb9fbcd80ac9ef9938d[31];
extern const uint8_t input__regseq_synth__const_memory_static_8a8c163c7c81c18280334651c91c9280[20];
extern const uint8_t input__regseq_synth__const_memory_static_a1f87505c1e05e3415d5fc5cbcca1f41[27];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
void input_regseq_main(void) {
    uint32_t _1;
    uint32_t _2;
    uint32_t _4;
    uint32_t _6;
    uint32_t _8;
    uint32_t _15;
    uint32_t _17;
    uint32_t _19;
    uint32_t _21;
    uint64_t _28;
    uint64_t _29;
    uint64_t _30;
    uint64_t _32;
    int8_t _39;
    int16_t _40;
    int32_t _41;
    int64_t _42;
    int8_t _43;
    int16_t _45;
    int32_t _47;
    int64_t _49;
    bb_0: {
        _1 = ((((uint32_t)(43968U))) + (((uint32_t)(13U))));
        _2 = _1;
        goto bb_1;
    }
    bb_1: {
        _4 = _1;
        goto bb_2;
    }
    bb_2: {
        _6 = _1;
        goto bb_3;
    }
    bb_3: {
        _8 = _1;
        goto bb_4;
    }
    bb_4: {
        fwrite("[INFO] x=", 1, 9, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%x", (unsigned)(_2)); fwrite(_n, 1, _l, stdout); } fwrite(" X=", 1, 3, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%X", (unsigned)(_4)); fwrite(_n, 1, _l, stdout); } fwrite(" ax=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#x", (unsigned)(_6)); fwrite(_n, 1, _l, stdout); } fwrite(" aX=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#X", (unsigned)(_8)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_5;
    }
    bb_5: {
        _15 = _1;
        goto bb_6;
    }
    bb_6: {
        _17 = _1;
        goto bb_7;
    }
    bb_7: {
        _19 = _1;
        goto bb_8;
    }
    bb_8: {
        _21 = _1;
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] z=", 1, 9, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%08x", (unsigned)(_15)); fwrite(_n, 1, _l, stdout); } fwrite(" Z=", 1, 3, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%08X", (unsigned)(_17)); fwrite(_n, 1, _l, stdout); } fwrite(" az=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#010x", (unsigned)(_19)); fwrite(_n, 1, _l, stdout); } fwrite(" aZ=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#010X", (unsigned)(_21)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        _28 = ((uint64_t)((uint64_t)(((uint64_t)(3735928559ULL))) << ((((int32_t)(8U))) & 63)));
        _29 = ((_28) | (((uint64_t)(255ULL))));
        _30 = _29;
        goto bb_11;
    }
    bb_11: {
        _32 = _29;
        goto bb_12;
    }
    bb_12: {
        fwrite("[INFO] w=", 1, 9, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%08llx", (unsigned long long)(_30)); fwrite(_n, 1, _l, stdout); } fwrite(" aw=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#010llX", (unsigned long long)(_32)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_13;
    }
    bb_13: {
        _39 = ((((int8_t)(5U))) - (((int8_t)(6U))));
        _40 = ((((int16_t)(5U))) - (((int16_t)(6U))));
        _41 = ((((int32_t)(5U))) - (((int32_t)(6U))));
        _42 = ((((int64_t)(5ULL))) - (((int64_t)(6ULL))));
        _43 = _39;
        goto bb_14;
    }
    bb_14: {
        _45 = _40;
        goto bb_15;
    }
    bb_15: {
        _47 = _41;
        goto bb_16;
    }
    bb_16: {
        _49 = _42;
        goto bb_17;
    }
    bb_17: {
        fwrite("[INFO] a=", 1, 9, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%x", (unsigned)(uint8_t)(_43)); fwrite(_n, 1, _l, stdout); } fwrite(" b=", 1, 3, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%x", (unsigned)(uint16_t)(_45)); fwrite(_n, 1, _l, stdout); } fwrite(" c=", 1, 3, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%x", (unsigned)(uint32_t)(_47)); fwrite(_n, 1, _l, stdout); } fwrite(" d=", 1, 3, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llx", (unsigned long long)(_49)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_18;
    }
    bb_18: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_52a7663f6fc8d4522c6062080ce2e41a[41] = { ((uint8_t)(122U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(48U)), ((uint8_t)(56U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(90U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(48U)), ((uint8_t)(56U)), ((uint8_t)(88U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(122U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(90U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(88U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5956483e6acaddb9fbcd80ac9ef9938d[31] = { ((uint8_t)(120U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(88U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(88U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(120U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(88U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(88U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_8a8c163c7c81c18280334651c91c9280[20] = { ((uint8_t)(119U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(48U)), ((uint8_t)(56U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(119U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(88U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_a1f87505c1e05e3415d5fc5cbcca1f41[27] = { ((uint8_t)(97U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(99U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(120U)), ((uint8_t)(125U)) };
