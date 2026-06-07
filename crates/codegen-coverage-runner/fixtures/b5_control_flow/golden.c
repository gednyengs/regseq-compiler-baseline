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
 * Register-access host device model (register-access construct).
 *
 * A flat little-endian register file with TRACING accessors. Each access prints
 * a "<R|W><bits> @0x<offset> = 0x<value>" line to stdout: this trace IS the
 * behavioral oracle for register lowering (a wrong offset shows up as a wrong
 * trace line, which a pure write/read-back round-trip would hide). `width` is
 * in BYTES (1/2/4/8). Annotated `unused` so a TU with no register access is
 * still warning-free; a TU that lowers MMIO ops references them. */
#define REGSEQ_REGFILE_BYTES 65536u
static uint8_t regseq_regfile[REGSEQ_REGFILE_BYTES] __attribute__((unused));
__attribute__((unused)) static uint64_t regseq_mmio_read(uint32_t off, uint32_t width) {
    uint64_t v = 0;
    for (uint32_t i = 0; i < width && (off + i) < REGSEQ_REGFILE_BYTES; i++) {
        v |= (uint64_t)regseq_regfile[off + i] << (8u * i);
    }
    printf("R%u @0x%03x = 0x%0*llx\n", (unsigned)(width * 8u), (unsigned)off,
           (int)(width * 2u), (unsigned long long)v);
    return v;
}
__attribute__((unused)) static void regseq_mmio_write(uint32_t off, uint32_t width, uint64_t val) {
    printf("W%u @0x%03x = 0x%0*llx\n", (unsigned)(width * 8u), (unsigned)off,
           (int)(width * 2u), (unsigned long long)val);
    for (uint32_t i = 0; i < width && (off + i) < REGSEQ_REGFILE_BYTES; i++) {
        regseq_regfile[off + i] = (uint8_t)(val >> (8u * i));
    }
}

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
struct input_D;
struct tuple_cbf29ce484222325;
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_D {
    uint32_t v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_D) == 4, "input_D size");
_Static_assert(_Alignof(struct input_D) == 4, "input_D align");
_Static_assert(offsetof(struct input_D, v) == 0, "input_D.v offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
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

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
void input__impl_0__drop(struct input_D *);
void input__regseq_synth__drop_glue_a2ccf9a27036e808d1063c365206a1ba__a2ccf9a27036e808d1063c365206a1ba(struct input_D *);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) int32_t input_dbl(int32_t);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_17c0c12b1540688ab64bbeec9b114ba9[12];
extern const uint8_t input__regseq_synth__const_memory_static_4a8a4da5b16debe56e88578e1ca2ff7f[10];
extern const uint8_t input__regseq_synth__const_memory_static_7265f148a4504fc145dc001f7523fada[10];
extern const uint8_t input__regseq_synth__const_memory_static_a746e46ef8e1620a1fbc1fb41d73406b[11];
extern const uint8_t input__regseq_synth__const_memory_static_b9009caab6e0d94758eeb9c44d0c650d[8];
extern uint32_t input_DROPPED;
extern const int32_t (*input_F)(int32_t);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
void input__impl_0__drop(struct input_D *_1) {
    uint32_t _2;
    uint32_t *_3;
    bb_0: {
        _2 = (*_1).v;
        _3 = ((uint32_t *)&input_DROPPED);
        (*_3) = _2;
        return;
    }
}
void input__regseq_synth__drop_glue_a2ccf9a27036e808d1063c365206a1ba__a2ccf9a27036e808d1063c365206a1ba(struct input_D *_1) {
    struct input_D *_2;
    bb_0: {
        _2 = &((*_1));
        input__impl_0__drop(_2); goto bb_1;
    }
    bb_1: {
        return;
    }
}
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
__attribute__((noinline)) int32_t input_dbl(int32_t _1) {
    int32_t _0;
    int32_t _2;
    bb_0: {
        _2 = _1;
        _0 = ((_2) * (((int32_t)(2U))));
        return _0;
    }
}
void input_regseq_main(void) {
    int32_t _1;
    int32_t _2;
    int32_t _3;
    _Bool _4;
    int32_t _5;
    int32_t _12;
    int32_t _13;
    int32_t _14;
    int32_t (*const (*_21))(int32_t);
    int32_t (*_22)(int32_t);
    int32_t (*_23)(int32_t);
    int32_t _24;
    int32_t (*_31)(int32_t);
    int32_t (*_32)(int32_t);
    int32_t (*_33)(int32_t);
    int32_t _34;
    int32_t _35;
    struct input_D _42;
    uint32_t *_43;
    uint32_t _44;
    int32_t _51;
    struct input_D *_52;
    bb_0: {
        _1 = ((int32_t)(0U));
        _2 = ((int32_t)(0U));
        goto bb_1;
    }
    bb_1: {
        _3 = _1;
        _4 = ((_3) < (((int32_t)(4U))));
        switch (_4) { case 0ULL: goto bb_3; default: goto bb_2; }
    }
    bb_2: {
        _51 = _1;
        _2 = ((_2) + (_51));
        _1 = ((_1) + (((int32_t)(1U))));
        goto bb_1;
    }
    bb_3: {
        _5 = _2;
        goto bb_4;
    }
    bb_4: {
        fwrite("[INFO] loopsum=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_5)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_5;
    }
    bb_5: {
        _12 = ((int32_t)(2U));
        switch (_12) { case 0ULL: goto bb_10; case 1ULL: goto bb_9; case 2ULL: goto bb_8; case 3ULL: goto bb_7; default: goto bb_6; }
    }
    bb_6: {
        _13 = ((int32_t)(999U));
        goto bb_11;
    }
    bb_7: {
        _13 = ((int32_t)(400U));
        goto bb_11;
    }
    bb_8: {
        _13 = ((int32_t)(300U));
        goto bb_11;
    }
    bb_9: {
        _13 = ((int32_t)(200U));
        goto bb_11;
    }
    bb_10: {
        _13 = ((int32_t)(100U));
        goto bb_11;
    }
    bb_11: {
        _14 = _13;
        goto bb_12;
    }
    bb_12: {
        fwrite("[INFO] match=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_14)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_13;
    }
    bb_13: {
        _21 = ((int32_t (*const (*))(int32_t))&input_F);
        _22 = (*_21);
        _23 = _22;
        _24 = (_23)(((int32_t)(21U))); goto bb_14;
    }
    bb_14: {
        goto bb_15;
    }
    bb_15: {
        fwrite("[INFO] indirect=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_24)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_16;
    }
    bb_16: {
        _31 = _22;
        _32 = _31;
        _33 = _32;
        _34 = (_33)(((int32_t)(20U))); goto bb_17;
    }
    bb_17: {
        _35 = _34;
        goto bb_18;
    }
    bb_18: {
        fwrite("[INFO] unsafe_fp=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_35)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_19;
    }
    bb_19: {
        _42 = (struct input_D){ .v = ((uint32_t)(123U)) };
        _52 = &(_42);
        input__regseq_synth__drop_glue_a2ccf9a27036e808d1063c365206a1ba__a2ccf9a27036e808d1063c365206a1ba(_52); goto bb_20;
    }
    bb_20: {
        _43 = ((uint32_t *)&input_DROPPED);
        _44 = (*_43);
        goto bb_21;
    }
    bb_21: {
        fwrite("[INFO] dropped=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_44)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_22;
    }
    bb_22: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_17c0c12b1540688ab64bbeec9b114ba9[12] = { ((uint8_t)(117U)), ((uint8_t)(110U)), ((uint8_t)(115U)), ((uint8_t)(97U)), ((uint8_t)(102U)), ((uint8_t)(101U)), ((uint8_t)(95U)), ((uint8_t)(102U)), ((uint8_t)(112U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_4a8a4da5b16debe56e88578e1ca2ff7f[10] = { ((uint8_t)(100U)), ((uint8_t)(114U)), ((uint8_t)(111U)), ((uint8_t)(112U)), ((uint8_t)(112U)), ((uint8_t)(101U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_7265f148a4504fc145dc001f7523fada[10] = { ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(111U)), ((uint8_t)(112U)), ((uint8_t)(115U)), ((uint8_t)(117U)), ((uint8_t)(109U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_a746e46ef8e1620a1fbc1fb41d73406b[11] = { ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(100U)), ((uint8_t)(105U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(99U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_b9009caab6e0d94758eeb9c44d0c650d[8] = { ((uint8_t)(109U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(99U)), ((uint8_t)(104U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
uint32_t input_DROPPED = ((uint32_t)(0U));
const int32_t (*input_F)(int32_t) = (&input_dbl);
