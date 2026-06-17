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
 * Timed-cpu-delay-and-sleep host fake-clock hooks (TCD construct). DECLARATIONS
 * only; the recording implementation is linked by the test harness (C06/D08).
 * `busy_wait` lowers to regseq_delay_busy_wait_ns, `sleep` to
 * regseq_delay_sleep_ns -- the hook identity preserves author intent (I04). */
extern void regseq_delay_busy_wait_ns(uint64_t ns);
extern void regseq_delay_sleep_ns(uint64_t ns);

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_Duration;
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct tuple_cbf29ce484222325;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_Duration {
    uint64_t ns;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_Duration) == 8, "input_Duration size");
_Static_assert(_Alignof(struct input_Duration) == 8, "input_Duration align");
_Static_assert(offsetof(struct input_Duration, ns) == 0, "input_Duration.ns offset");
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
struct input_Duration input__impl_0__from_ms(uint64_t);
struct input_Duration input__impl_0__from_us(uint64_t);
inline struct input_Duration input__impl_2__clone(const struct input_Duration *);
__attribute__((noinline)) void input_busy_wait(struct input_Duration);
__attribute__((noinline)) uint64_t input_delay_usec(void);
void input_regseq_main(void);
__attribute__((noinline)) void input_sleep(struct input_Duration);
extern const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0];
extern const uint8_t input__regseq_synth__const_memory_static_c9ee4207949ac68d6383d1025e1a4e09[7];
extern const uint8_t input__regseq_synth__const_memory_static_d3d53a68bd8a2478a73769d9b80a1955[12];
extern const uint8_t input__regseq_synth__const_memory_static_de9180f9620a67781be3fe1ce0ec0f35[12];
extern const uint8_t input__regseq_synth__const_memory_static_f162741f780fede31c6fdc44bfa1c758[14];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct input_Duration input__impl_0__from_ms(uint64_t _1) {
    struct input_Duration _0;
    uint64_t _2;
    uint64_t _3;
    bb_0: {
        _2 = _1;
        _3 = ((_2) * (((uint64_t)(1000000ULL))));
        _0 = (struct input_Duration){ .ns = _3 };
        return _0;
    }
}
struct input_Duration input__impl_0__from_us(uint64_t _1) {
    struct input_Duration _0;
    uint64_t _2;
    uint64_t _3;
    bb_0: {
        _2 = _1;
        _3 = ((_2) * (((uint64_t)(1000ULL))));
        _0 = (struct input_Duration){ .ns = _3 };
        return _0;
    }
}
inline struct input_Duration input__impl_2__clone(const struct input_Duration *_1) {
    struct input_Duration _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
__attribute__((noinline)) void input_busy_wait(struct input_Duration _1) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) uint64_t input_delay_usec(void) {
    uint64_t _0;
    bb_0: {
        _0 = ((uint64_t)(500ULL));
        return _0;
    }
}
void input_regseq_main(void) {
    struct input_Duration _6;
    uint64_t _13;
    struct input_Duration _14;
    struct input_Duration _21;
    bb_0: {
        fwrite("[INFO] reset assert\n", 1, 20, stdout); goto bb_1;
    }
    bb_1: {
        _6 = input__impl_0__from_us(((uint64_t)(10ULL))); goto bb_2;
    }
    bb_2: {
        regseq_delay_busy_wait_ns((uint64_t)(_6).ns); goto bb_3;
    }
    bb_3: {
        fwrite("[INFO] reset deassert\n", 1, 22, stdout); goto bb_4;
    }
    bb_4: {
        _13 = input_delay_usec(); goto bb_5;
    }
    bb_5: {
        _14 = input__impl_0__from_us(_13); goto bb_6;
    }
    bb_6: {
        regseq_delay_busy_wait_ns((uint64_t)(_14).ns); goto bb_7;
    }
    bb_7: {
        fwrite("[INFO] eeprom ready\n", 1, 20, stdout); goto bb_8;
    }
    bb_8: {
        _21 = input__impl_0__from_ms(((uint64_t)(5ULL))); goto bb_9;
    }
    bb_9: {
        regseq_delay_sleep_ns((uint64_t)(_21).ns); goto bb_10;
    }
    bb_10: {
        fwrite("[INFO] link up\n", 1, 15, stdout); goto bb_11;
    }
    bb_11: {
        return;
    }
}
__attribute__((noinline)) void input_sleep(struct input_Duration _1) {
    bb_0: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0] = {0};
const uint8_t input__regseq_synth__const_memory_static_c9ee4207949ac68d6383d1025e1a4e09[7] = { ((uint8_t)(108U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(107U)), ((uint8_t)(32U)), ((uint8_t)(117U)), ((uint8_t)(112U)) };
const uint8_t input__regseq_synth__const_memory_static_d3d53a68bd8a2478a73769d9b80a1955[12] = { ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(116U)), ((uint8_t)(32U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(116U)) };
const uint8_t input__regseq_synth__const_memory_static_de9180f9620a67781be3fe1ce0ec0f35[12] = { ((uint8_t)(101U)), ((uint8_t)(101U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(111U)), ((uint8_t)(109U)), ((uint8_t)(32U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(100U)), ((uint8_t)(121U)) };
const uint8_t input__regseq_synth__const_memory_static_f162741f780fede31c6fdc44bfa1c758[14] = { ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(116U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(116U)) };
