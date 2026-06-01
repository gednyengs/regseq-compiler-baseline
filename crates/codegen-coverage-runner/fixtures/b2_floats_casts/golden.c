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


/* section 11: saturating FloatToInt (NaN -> 0; bounds before cast). */
static inline int32_t regseq_f64_to_int32_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INT32_MIN) return INT32_MIN;
    if (x >= (double)INT32_MAX) return INT32_MAX;
    return (int32_t)x;
}

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
extern const uint8_t input__regseq_synth__const_memory_static_071e94a472199b5f07cadc25e6bae0c3[17];
extern const uint8_t input__regseq_synth__const_memory_static_1294c7106225ac2f6c018a59f8050012[13];
extern const uint8_t input__regseq_synth__const_memory_static_1bd98f77c11d56673da2cc08fcc1b1c7[9];
extern const uint8_t input__regseq_synth__const_memory_static_2c8899d307d33723a1088d903ee4fc3a[12];
extern const uint8_t input__regseq_synth__const_memory_static_321139581b0a2b22ec1150e9a7c064ea[16];
extern const uint8_t input__regseq_synth__const_memory_static_355715664fb785f0bafcd1ef8ab2d330[15];
extern const uint8_t input__regseq_synth__const_memory_static_4e4f3e75c092b4cd0812dcaa593b3892[11];
extern const uint8_t input__regseq_synth__const_memory_static_674ae500770dd16282ef7363d3ae1bab[13];
extern const uint8_t input__regseq_synth__const_memory_static_6daacce96ed9138f586cb6b24c82f87b[14];
extern const uint8_t input__regseq_synth__const_memory_static_81b287df206bcf9e51a2f8845117cfb4[10];
extern const uint8_t input__regseq_synth__const_memory_static_c127b92c094d5a3741d5b3c7933811b2[11];
extern const uint8_t input__regseq_synth__const_memory_static_d0f92abd4d5fd63c3d5e42d664ef4f4c[8];
extern const uint8_t input__regseq_synth__const_memory_static_fe16bca26323c716d86ccbef1ab6aa97[8];

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
    double _1;
    double _2;
    double _3;
    _Bool _4;
    uint8_t _5;
    float _12;
    float _13;
    float _14;
    _Bool _15;
    uint8_t _16;
    uint64_t _23;
    uint64_t _24;
    uint32_t _31;
    uint32_t _32;
    _Bool _33;
    uint8_t _34;
    uint32_t _41;
    uint8_t _48;
    uint8_t _49;
    int32_t _56;
    int32_t _57;
    int32_t _64;
    int32_t _65;
    float _72;
    float _73;
    _Bool _74;
    uint8_t _75;
    double _82;
    double _83;
    _Bool _84;
    uint8_t _85;
    int32_t _92;
    const int32_t *_93;
    const int32_t *_94;
    const int32_t *_95;
    const uint8_t *_96;
    uint8_t _97;
    const int32_t *_104;
    size_t _105;
    size_t _106;
    _Bool _107;
    uint8_t _108;
    size_t _115;
    const int32_t *_116;
    int32_t _117;
    int32_t _118;
    _Bool _119;
    uint8_t _120;
    bb_0: {
        _1 = 0x1.cp1;
        _2 = _1;
        _3 = ((_2) + (0x1p0));
        _4 = ((_3) == (0x1.2p2));
        _5 = ((uint8_t)(_4));
        goto bb_1;
    }
    bb_1: {
        fwrite("[INFO] f64ck=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_5)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_2;
    }
    bb_2: {
        _12 = 0x1.2p1f;
        _13 = _12;
        _14 = ((_13) * (0x1p1f));
        _15 = ((_14) == (0x1.2p2f));
        _16 = ((uint8_t)(_15));
        goto bb_3;
    }
    bb_3: {
        fwrite("[INFO] f32ck=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_16)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_4;
    }
    bb_4: {
        memcpy(&(_23), &(double){0x1.8p0}, sizeof(_23));
        _24 = _23;
        goto bb_5;
    }
    bb_5: {
        fwrite("[INFO] f64bits=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_24)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_6;
    }
    bb_6: {
        memcpy(&(_31), &(float){0x1.8p0f}, sizeof(_31));
        _32 = _31;
        _33 = ((_32) == (((uint32_t)(1069547520U))));
        _34 = ((uint8_t)(_33));
        goto bb_7;
    }
    bb_7: {
        fwrite("[INFO] transmute=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_34)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_8;
    }
    bb_8: {
        _41 = _31;
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] transmute_raw=", 1, 21, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_41)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        _48 = ((uint8_t)(((int32_t)(300U))));
        _49 = _48;
        goto bb_11;
    }
    bb_11: {
        fwrite("[INFO] inttoint=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_49)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_12;
    }
    bb_12: {
        _56 = regseq_f64_to_int32_t(0x1.f333333333333p1);
        _57 = _56;
        goto bb_13;
    }
    bb_13: {
        fwrite("[INFO] floattoint=", 1, 18, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_57)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_14;
    }
    bb_14: {
        _64 = regseq_f64_to_int32_t(0x1.5af1d78b58c4p66);
        _65 = _64;
        goto bb_15;
    }
    bb_15: {
        fwrite("[INFO] floattoint_sat=", 1, 22, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_65)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_16;
    }
    bb_16: {
        _72 = ((float)(0x1.cp1));
        _73 = _72;
        _74 = ((_73) == (0x1.cp1f));
        _75 = ((uint8_t)(_74));
        goto bb_17;
    }
    bb_17: {
        fwrite("[INFO] floattofloat=", 1, 20, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_75)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_18;
    }
    bb_18: {
        _82 = ((double)(((int32_t)(7U))));
        _83 = _82;
        _84 = ((_83) == (0x1.cp2));
        _85 = ((uint8_t)(_84));
        goto bb_19;
    }
    bb_19: {
        fwrite("[INFO] inttofloat=", 1, 18, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_85)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_20;
    }
    bb_20: {
        _92 = ((int32_t)(1145258561U));
        _93 = &(_92);
        _94 = &((*_93));
        _95 = _94;
        _96 = ((const uint8_t *)(_95));
        _97 = (*_96);
        goto bb_21;
    }
    bb_21: {
        fwrite("[INFO] ptrtoptr=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_97)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_22;
    }
    bb_22: {
        _104 = _94;
        _105 = ((uintptr_t)(_104));
        _106 = _105;
        _107 = ((_106) != (((size_t)(0ULL))));
        _108 = ((uint8_t)(_107));
        goto bb_23;
    }
    bb_23: {
        fwrite("[INFO] expose=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_108)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_24;
    }
    bb_24: {
        _115 = _105;
        _116 = ((const int32_t *)(_115));
        _117 = (*_116);
        _118 = _92;
        _119 = ((_117) == (_118));
        _120 = ((uint8_t)(_119));
        goto bb_25;
    }
    bb_25: {
        fwrite("[INFO] withexposed=", 1, 19, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_120)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_26;
    }
    bb_26: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_071e94a472199b5f07cadc25e6bae0c3[17] = { ((uint8_t)(102U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(115U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_1294c7106225ac2f6c018a59f8050012[13] = { ((uint8_t)(102U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_1bd98f77c11d56673da2cc08fcc1b1c7[9] = { ((uint8_t)(101U)), ((uint8_t)(120U)), ((uint8_t)(112U)), ((uint8_t)(111U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_2c8899d307d33723a1088d903ee4fc3a[12] = { ((uint8_t)(116U)), ((uint8_t)(114U)), ((uint8_t)(97U)), ((uint8_t)(110U)), ((uint8_t)(115U)), ((uint8_t)(109U)), ((uint8_t)(117U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_321139581b0a2b22ec1150e9a7c064ea[16] = { ((uint8_t)(116U)), ((uint8_t)(114U)), ((uint8_t)(97U)), ((uint8_t)(110U)), ((uint8_t)(115U)), ((uint8_t)(109U)), ((uint8_t)(117U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(95U)), ((uint8_t)(114U)), ((uint8_t)(97U)), ((uint8_t)(119U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_355715664fb785f0bafcd1ef8ab2d330[15] = { ((uint8_t)(102U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(102U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_4e4f3e75c092b4cd0812dcaa593b3892[11] = { ((uint8_t)(112U)), ((uint8_t)(116U)), ((uint8_t)(114U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(112U)), ((uint8_t)(116U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_674ae500770dd16282ef7363d3ae1bab[13] = { ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(102U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_6daacce96ed9138f586cb6b24c82f87b[14] = { ((uint8_t)(119U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(104U)), ((uint8_t)(101U)), ((uint8_t)(120U)), ((uint8_t)(112U)), ((uint8_t)(111U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_81b287df206bcf9e51a2f8845117cfb4[10] = { ((uint8_t)(102U)), ((uint8_t)(54U)), ((uint8_t)(52U)), ((uint8_t)(98U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(115U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_c127b92c094d5a3741d5b3c7933811b2[11] = { ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_d0f92abd4d5fd63c3d5e42d664ef4f4c[8] = { ((uint8_t)(102U)), ((uint8_t)(51U)), ((uint8_t)(50U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_fe16bca26323c716d86ccbef1ab6aa97[8] = { ((uint8_t)(102U)), ((uint8_t)(54U)), ((uint8_t)(52U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
