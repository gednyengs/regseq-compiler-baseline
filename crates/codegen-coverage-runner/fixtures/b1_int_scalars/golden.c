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
* Arithmetic / cast helper block (section 10.1 + section 11), fixed superset.
* --------------------------------------------------------------------------- */

/* 128-bit bounds: no standard *_MIN / *_MAX macro exists for __int128 /
* __uint128_t, so compute them (host two's-complement assumption). */
#define REGSEQ_INT128_MAX ((__int128)(((__uint128_t)1 << 127) - 1))
#define REGSEQ_INT128_MIN (-REGSEQ_INT128_MAX - 1)
#define REGSEQ_UINT128_MAX (~(__uint128_t)0)

/* section 10.1: {add,sub,mul}WithOverflow -> (T, bool). */
struct regseq_overflow_int8_t { int8_t v; _Bool overflow; };
static inline struct regseq_overflow_int8_t regseq_add_int8_t(int8_t a, int8_t b) {
    struct regseq_overflow_int8_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int8_t regseq_sub_int8_t(int8_t a, int8_t b) {
    struct regseq_overflow_int8_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int8_t regseq_mul_int8_t(int8_t a, int8_t b) {
    struct regseq_overflow_int8_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_int16_t { int16_t v; _Bool overflow; };
static inline struct regseq_overflow_int16_t regseq_add_int16_t(int16_t a, int16_t b) {
    struct regseq_overflow_int16_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int16_t regseq_sub_int16_t(int16_t a, int16_t b) {
    struct regseq_overflow_int16_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int16_t regseq_mul_int16_t(int16_t a, int16_t b) {
    struct regseq_overflow_int16_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_int32_t { int32_t v; _Bool overflow; };
static inline struct regseq_overflow_int32_t regseq_add_int32_t(int32_t a, int32_t b) {
    struct regseq_overflow_int32_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int32_t regseq_sub_int32_t(int32_t a, int32_t b) {
    struct regseq_overflow_int32_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int32_t regseq_mul_int32_t(int32_t a, int32_t b) {
    struct regseq_overflow_int32_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_int64_t { int64_t v; _Bool overflow; };
static inline struct regseq_overflow_int64_t regseq_add_int64_t(int64_t a, int64_t b) {
    struct regseq_overflow_int64_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int64_t regseq_sub_int64_t(int64_t a, int64_t b) {
    struct regseq_overflow_int64_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_int64_t regseq_mul_int64_t(int64_t a, int64_t b) {
    struct regseq_overflow_int64_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_intptr_t { intptr_t v; _Bool overflow; };
static inline struct regseq_overflow_intptr_t regseq_add_intptr_t(intptr_t a, intptr_t b) {
    struct regseq_overflow_intptr_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_intptr_t regseq_sub_intptr_t(intptr_t a, intptr_t b) {
    struct regseq_overflow_intptr_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_intptr_t regseq_mul_intptr_t(intptr_t a, intptr_t b) {
    struct regseq_overflow_intptr_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow___int128 { __int128 v; _Bool overflow; };
static inline struct regseq_overflow___int128 regseq_add___int128(__int128 a, __int128 b) {
    struct regseq_overflow___int128 r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow___int128 regseq_sub___int128(__int128 a, __int128 b) {
    struct regseq_overflow___int128 r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow___int128 regseq_mul___int128(__int128 a, __int128 b) {
    struct regseq_overflow___int128 r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_uint8_t { uint8_t v; _Bool overflow; };
static inline struct regseq_overflow_uint8_t regseq_add_uint8_t(uint8_t a, uint8_t b) {
    struct regseq_overflow_uint8_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint8_t regseq_sub_uint8_t(uint8_t a, uint8_t b) {
    struct regseq_overflow_uint8_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint8_t regseq_mul_uint8_t(uint8_t a, uint8_t b) {
    struct regseq_overflow_uint8_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_uint16_t { uint16_t v; _Bool overflow; };
static inline struct regseq_overflow_uint16_t regseq_add_uint16_t(uint16_t a, uint16_t b) {
    struct regseq_overflow_uint16_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint16_t regseq_sub_uint16_t(uint16_t a, uint16_t b) {
    struct regseq_overflow_uint16_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint16_t regseq_mul_uint16_t(uint16_t a, uint16_t b) {
    struct regseq_overflow_uint16_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_uint32_t { uint32_t v; _Bool overflow; };
static inline struct regseq_overflow_uint32_t regseq_add_uint32_t(uint32_t a, uint32_t b) {
    struct regseq_overflow_uint32_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint32_t regseq_sub_uint32_t(uint32_t a, uint32_t b) {
    struct regseq_overflow_uint32_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint32_t regseq_mul_uint32_t(uint32_t a, uint32_t b) {
    struct regseq_overflow_uint32_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_uint64_t { uint64_t v; _Bool overflow; };
static inline struct regseq_overflow_uint64_t regseq_add_uint64_t(uint64_t a, uint64_t b) {
    struct regseq_overflow_uint64_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint64_t regseq_sub_uint64_t(uint64_t a, uint64_t b) {
    struct regseq_overflow_uint64_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_uint64_t regseq_mul_uint64_t(uint64_t a, uint64_t b) {
    struct regseq_overflow_uint64_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow_size_t { size_t v; _Bool overflow; };
static inline struct regseq_overflow_size_t regseq_add_size_t(size_t a, size_t b) {
    struct regseq_overflow_size_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_size_t regseq_sub_size_t(size_t a, size_t b) {
    struct regseq_overflow_size_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow_size_t regseq_mul_size_t(size_t a, size_t b) {
    struct regseq_overflow_size_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}
struct regseq_overflow___uint128_t { __uint128_t v; _Bool overflow; };
static inline struct regseq_overflow___uint128_t regseq_add___uint128_t(__uint128_t a, __uint128_t b) {
    struct regseq_overflow___uint128_t r;
    r.overflow = __builtin_add_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow___uint128_t regseq_sub___uint128_t(__uint128_t a, __uint128_t b) {
    struct regseq_overflow___uint128_t r;
    r.overflow = __builtin_sub_overflow(a, b, &r.v);
    return r;
}
static inline struct regseq_overflow___uint128_t regseq_mul___uint128_t(__uint128_t a, __uint128_t b) {
    struct regseq_overflow___uint128_t r;
    r.overflow = __builtin_mul_overflow(a, b, &r.v);
    return r;
}

/* section 11: saturating FloatToInt (NaN -> 0; bounds before cast). */
static inline int8_t regseq_f32_to_int8_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)INT8_MIN) return INT8_MIN;
    if (x >= (float)INT8_MAX) return INT8_MAX;
    return (int8_t)x;
}
static inline int16_t regseq_f32_to_int16_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)INT16_MIN) return INT16_MIN;
    if (x >= (float)INT16_MAX) return INT16_MAX;
    return (int16_t)x;
}
static inline int32_t regseq_f32_to_int32_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)INT32_MIN) return INT32_MIN;
    if (x >= (float)INT32_MAX) return INT32_MAX;
    return (int32_t)x;
}
static inline int64_t regseq_f32_to_int64_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)INT64_MIN) return INT64_MIN;
    if (x >= (float)INT64_MAX) return INT64_MAX;
    return (int64_t)x;
}
static inline intptr_t regseq_f32_to_intptr_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)INTPTR_MIN) return INTPTR_MIN;
    if (x >= (float)INTPTR_MAX) return INTPTR_MAX;
    return (intptr_t)x;
}
static inline __int128 regseq_f32_to___int128(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)REGSEQ_INT128_MIN) return REGSEQ_INT128_MIN;
    if (x >= (float)REGSEQ_INT128_MAX) return REGSEQ_INT128_MAX;
    return (__int128)x;
}
static inline uint8_t regseq_f32_to_uint8_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)UINT8_MAX) return UINT8_MAX;
    return (uint8_t)x;
}
static inline uint16_t regseq_f32_to_uint16_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)UINT16_MAX) return UINT16_MAX;
    return (uint16_t)x;
}
static inline uint32_t regseq_f32_to_uint32_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)UINT32_MAX) return UINT32_MAX;
    return (uint32_t)x;
}
static inline uint64_t regseq_f32_to_uint64_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)UINT64_MAX) return UINT64_MAX;
    return (uint64_t)x;
}
static inline size_t regseq_f32_to_size_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)SIZE_MAX) return SIZE_MAX;
    return (size_t)x;
}
static inline __uint128_t regseq_f32_to___uint128_t(float x) {
    if (isnan(x)) return 0;
    if (x <= (float)0) return 0;
    if (x >= (float)REGSEQ_UINT128_MAX) return REGSEQ_UINT128_MAX;
    return (__uint128_t)x;
}
static inline int8_t regseq_f64_to_int8_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INT8_MIN) return INT8_MIN;
    if (x >= (double)INT8_MAX) return INT8_MAX;
    return (int8_t)x;
}
static inline int16_t regseq_f64_to_int16_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INT16_MIN) return INT16_MIN;
    if (x >= (double)INT16_MAX) return INT16_MAX;
    return (int16_t)x;
}
static inline int32_t regseq_f64_to_int32_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INT32_MIN) return INT32_MIN;
    if (x >= (double)INT32_MAX) return INT32_MAX;
    return (int32_t)x;
}
static inline int64_t regseq_f64_to_int64_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INT64_MIN) return INT64_MIN;
    if (x >= (double)INT64_MAX) return INT64_MAX;
    return (int64_t)x;
}
static inline intptr_t regseq_f64_to_intptr_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)INTPTR_MIN) return INTPTR_MIN;
    if (x >= (double)INTPTR_MAX) return INTPTR_MAX;
    return (intptr_t)x;
}
static inline __int128 regseq_f64_to___int128(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)REGSEQ_INT128_MIN) return REGSEQ_INT128_MIN;
    if (x >= (double)REGSEQ_INT128_MAX) return REGSEQ_INT128_MAX;
    return (__int128)x;
}
static inline uint8_t regseq_f64_to_uint8_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)UINT8_MAX) return UINT8_MAX;
    return (uint8_t)x;
}
static inline uint16_t regseq_f64_to_uint16_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)UINT16_MAX) return UINT16_MAX;
    return (uint16_t)x;
}
static inline uint32_t regseq_f64_to_uint32_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)UINT32_MAX) return UINT32_MAX;
    return (uint32_t)x;
}
static inline uint64_t regseq_f64_to_uint64_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)UINT64_MAX) return UINT64_MAX;
    return (uint64_t)x;
}
static inline size_t regseq_f64_to_size_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)SIZE_MAX) return SIZE_MAX;
    return (size_t)x;
}
static inline __uint128_t regseq_f64_to___uint128_t(double x) {
    if (isnan(x)) return 0;
    if (x <= (double)0) return 0;
    if (x >= (double)REGSEQ_UINT128_MAX) return REGSEQ_UINT128_MAX;
    return (__uint128_t)x;
}
#if defined(__FLT16_MANT_DIG__)
static inline int8_t regseq_f16_to_int8_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)INT8_MIN) return INT8_MIN;
    if (x >= (_Float16)INT8_MAX) return INT8_MAX;
    return (int8_t)x;
}
static inline int16_t regseq_f16_to_int16_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)INT16_MIN) return INT16_MIN;
    if (x >= (_Float16)INT16_MAX) return INT16_MAX;
    return (int16_t)x;
}
static inline int32_t regseq_f16_to_int32_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)INT32_MIN) return INT32_MIN;
    if (x >= (_Float16)INT32_MAX) return INT32_MAX;
    return (int32_t)x;
}
static inline int64_t regseq_f16_to_int64_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)INT64_MIN) return INT64_MIN;
    if (x >= (_Float16)INT64_MAX) return INT64_MAX;
    return (int64_t)x;
}
static inline intptr_t regseq_f16_to_intptr_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)INTPTR_MIN) return INTPTR_MIN;
    if (x >= (_Float16)INTPTR_MAX) return INTPTR_MAX;
    return (intptr_t)x;
}
static inline __int128 regseq_f16_to___int128(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)REGSEQ_INT128_MIN) return REGSEQ_INT128_MIN;
    if (x >= (_Float16)REGSEQ_INT128_MAX) return REGSEQ_INT128_MAX;
    return (__int128)x;
}
static inline uint8_t regseq_f16_to_uint8_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)UINT8_MAX) return UINT8_MAX;
    return (uint8_t)x;
}
static inline uint16_t regseq_f16_to_uint16_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)UINT16_MAX) return UINT16_MAX;
    return (uint16_t)x;
}
static inline uint32_t regseq_f16_to_uint32_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)UINT32_MAX) return UINT32_MAX;
    return (uint32_t)x;
}
static inline uint64_t regseq_f16_to_uint64_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)UINT64_MAX) return UINT64_MAX;
    return (uint64_t)x;
}
static inline size_t regseq_f16_to_size_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)SIZE_MAX) return SIZE_MAX;
    return (size_t)x;
}
static inline __uint128_t regseq_f16_to___uint128_t(_Float16 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float16)0) return 0;
    if (x >= (_Float16)REGSEQ_UINT128_MAX) return REGSEQ_UINT128_MAX;
    return (__uint128_t)x;
}
#endif
#if defined(__FLT128_MANT_DIG__)
static inline int8_t regseq_f128_to_int8_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)INT8_MIN) return INT8_MIN;
    if (x >= (_Float128)INT8_MAX) return INT8_MAX;
    return (int8_t)x;
}
static inline int16_t regseq_f128_to_int16_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)INT16_MIN) return INT16_MIN;
    if (x >= (_Float128)INT16_MAX) return INT16_MAX;
    return (int16_t)x;
}
static inline int32_t regseq_f128_to_int32_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)INT32_MIN) return INT32_MIN;
    if (x >= (_Float128)INT32_MAX) return INT32_MAX;
    return (int32_t)x;
}
static inline int64_t regseq_f128_to_int64_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)INT64_MIN) return INT64_MIN;
    if (x >= (_Float128)INT64_MAX) return INT64_MAX;
    return (int64_t)x;
}
static inline intptr_t regseq_f128_to_intptr_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)INTPTR_MIN) return INTPTR_MIN;
    if (x >= (_Float128)INTPTR_MAX) return INTPTR_MAX;
    return (intptr_t)x;
}
static inline __int128 regseq_f128_to___int128(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)REGSEQ_INT128_MIN) return REGSEQ_INT128_MIN;
    if (x >= (_Float128)REGSEQ_INT128_MAX) return REGSEQ_INT128_MAX;
    return (__int128)x;
}
static inline uint8_t regseq_f128_to_uint8_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)UINT8_MAX) return UINT8_MAX;
    return (uint8_t)x;
}
static inline uint16_t regseq_f128_to_uint16_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)UINT16_MAX) return UINT16_MAX;
    return (uint16_t)x;
}
static inline uint32_t regseq_f128_to_uint32_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)UINT32_MAX) return UINT32_MAX;
    return (uint32_t)x;
}
static inline uint64_t regseq_f128_to_uint64_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)UINT64_MAX) return UINT64_MAX;
    return (uint64_t)x;
}
static inline size_t regseq_f128_to_size_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)SIZE_MAX) return SIZE_MAX;
    return (size_t)x;
}
static inline __uint128_t regseq_f128_to___uint128_t(_Float128 x) {
    if (isnan(x)) return 0;
    if (x <= (_Float128)0) return 0;
    if (x >= (_Float128)REGSEQ_UINT128_MAX) return REGSEQ_UINT128_MAX;
    return (__uint128_t)x;
}
#endif

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct tuple_cbf29ce484222325;
struct tuple_299d3d74250e90bd;

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
struct tuple_299d3d74250e90bd {
    uint8_t _0;
    _Bool _1;
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_299d3d74250e90bd) == 2, "tuple_299d3d74250e90bd size");
_Static_assert(_Alignof(struct tuple_299d3d74250e90bd) == 1, "tuple_299d3d74250e90bd align");
_Static_assert(offsetof(struct tuple_299d3d74250e90bd, _0) == 0, "tuple_299d3d74250e90bd._0 offset");
_Static_assert(offsetof(struct tuple_299d3d74250e90bd, _1) == 1, "tuple_299d3d74250e90bd._1 offset");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
int32_t input_add_i32(int32_t, int32_t);
int32_t input_neg_i32(int32_t);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_003c4087039bcea0b922c3f84c906965[9];
extern const uint8_t input__regseq_synth__const_memory_static_01a1c3d973cb520e2d139719ae21224f[6];
extern const uint8_t input__regseq_synth__const_memory_static_08fc4e575cfb9bebce0c322c2174fe5e[6];
extern const uint8_t input__regseq_synth__const_memory_static_098a11f69ee64fae42951121e0b1a0c5[8];
extern const uint8_t input__regseq_synth__const_memory_static_09f589b09f8e282e2a034e0d52aa84e3[6];
extern const uint8_t input__regseq_synth__const_memory_static_0c2d87e556dfd3449bb4fe8118451708[6];
extern const uint8_t input__regseq_synth__const_memory_static_142333c60860539cc2453ac84c2b9314[6];
extern const uint8_t input__regseq_synth__const_memory_static_21543c6dd444e5a91910b1029e6874a8[7];
extern const uint8_t input__regseq_synth__const_memory_static_289a17933a81de4321b5c9569936ca55[7];
extern const uint8_t input__regseq_synth__const_memory_static_40c6ac168f757de60b66029c5791fe7a[6];
extern const uint8_t input__regseq_synth__const_memory_static_428d700e3bac08feee2759b950f5b729[7];
extern const uint8_t input__regseq_synth__const_memory_static_44bcf6ee8af6dab0e2804b598f406b85[6];
extern const uint8_t input__regseq_synth__const_memory_static_5411fa943ad3705cea5ca8394ed3b1c4[35];
extern const uint8_t input__regseq_synth__const_memory_static_566acb27e0d0f6a81dad14b2545bcf3c[6];
extern const uint8_t input__regseq_synth__const_memory_static_5828c2346bf3bad3d69843441f318b9a[8];
extern const uint8_t input__regseq_synth__const_memory_static_5f8f9e7a5fb1a569e1595dc780686814[6];
extern const uint8_t input__regseq_synth__const_memory_static_6285fac975f1527b604cfbfe5eaa6c9c[8];
extern const uint8_t input__regseq_synth__const_memory_static_654d68a7ba8f0ccf28458f5414e610da[7];
extern const uint8_t input__regseq_synth__const_memory_static_676008694dae925c5c754c02a84efae8[6];
extern const uint8_t input__regseq_synth__const_memory_static_70dad65e2b6be12883c1a34ffe7a6aa5[8];
extern const uint8_t input__regseq_synth__const_memory_static_73470e7de97c4ffa3965bc392410ff22[6];
extern const uint8_t input__regseq_synth__const_memory_static_7a92999d6d35548f408f42af77f5b134[6];
extern const uint8_t input__regseq_synth__const_memory_static_7c065424f6ca49cc1acb54143fc49b26[6];
extern const uint8_t input__regseq_synth__const_memory_static_87ef938a4cc394ba60088e43693d9698[6];
extern const uint8_t input__regseq_synth__const_memory_static_941c4bad0e9dd9166a160105e320a49d[13];
extern const uint8_t input__regseq_synth__const_memory_static_a38c772e2ad84eb6ceb930a71c9173a0[7];
extern const uint8_t input__regseq_synth__const_memory_static_ac70c7a8dc37ee656719e7beb52c5b23[14];
extern const uint8_t input__regseq_synth__const_memory_static_ad7b98ed0691cbeeccd13228c7883787[9];
extern const uint8_t input__regseq_synth__const_memory_static_b08c0ac34252856b9de2c9c86169fd00[6];
extern const uint8_t input__regseq_synth__const_memory_static_bc8f375971b482337a8ec782f3a2109f[5];
extern const uint8_t input__regseq_synth__const_memory_static_d4f842a088db707ad40fd50ee8860777[14];
extern const uint8_t input__regseq_synth__const_memory_static_ecd7ceb097b611bea9a8f95ec59c7f66[6];
extern const uint8_t input__regseq_synth__const_memory_static_f0df92fc56866e349175e48e1312e83f[6];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
void input_regseq_log_intrinsics_log_debug(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
void input_regseq_log_intrinsics_log_error(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
void input_regseq_log_intrinsics_log_warn(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
int32_t input_add_i32(int32_t _1, int32_t _2) {
    int32_t _0;
    int32_t _3;
    int32_t _4;
    bb_0: {
        _3 = _1;
        _4 = _2;
        _0 = ((_3) + (_4));
        return _0;
    }
}
int32_t input_neg_i32(int32_t _1) {
    int32_t _0;
    int32_t _2;
    bb_0: {
        _2 = _1;
        _0 = (-(_2));
        return _0;
    }
}
void input_regseq_main(void) {
    int32_t _1;
    int32_t _8;
    int32_t _15;
    int32_t _16;
    int32_t _17;
    int32_t _18;
    int32_t _19;
    int32_t _26;
    int32_t _27;
    int32_t _28;
    int32_t _35;
    int32_t _36;
    _Bool _37;
    _Bool _38;
    _Bool _39;
    _Bool _40;
    int32_t _41;
    int32_t _48;
    int32_t _49;
    _Bool _50;
    _Bool _51;
    _Bool _52;
    _Bool _53;
    int32_t _54;
    int32_t _61;
    int32_t _62;
    uint8_t _69;
    uint8_t _70;
    uint8_t _71;
    uint8_t _72;
    uint8_t _73;
    uint8_t _80;
    uint8_t _81;
    uint8_t _82;
    uint8_t _89;
    uint8_t _90;
    uint8_t _91;
    uint8_t _98;
    uint8_t _105;
    uint8_t _112;
    uint8_t _119;
    int32_t _126;
    int32_t _133;
    int32_t _140;
    struct tuple_299d3d74250e90bd _147;
    uint8_t _148;
    _Bool _149;
    uint8_t _150;
    _Bool _152;
    struct tuple_299d3d74250e90bd _159;
    uint8_t _160;
    _Bool _161;
    uint8_t _162;
    _Bool _164;
    struct tuple_299d3d74250e90bd _171;
    uint8_t _172;
    _Bool _173;
    uint8_t _174;
    _Bool _176;
    int32_t _183;
    int32_t _184;
    _Bool _185;
    int32_t _187;
    int32_t _188;
    _Bool _189;
    int32_t _191;
    int32_t _192;
    _Bool _193;
    int32_t _195;
    int32_t _196;
    _Bool _197;
    int32_t _199;
    int32_t _200;
    _Bool _201;
    int32_t _203;
    int32_t _204;
    _Bool _205;
    uint8_t _212;
    _Bool _219;
    uint32_t _232;
    int8_t _239;
    int16_t _246;
    int64_t _253;
    intptr_t _260;
    intptr_t _261;
    intptr_t _262;
    int64_t _263;
    size_t _270;
    size_t _271;
    size_t _272;
    uint64_t _273;
    __uint128_t _280;
    __uint128_t _281;
    _Bool _282;
    uint8_t _283;
    __int128 _290;
    __int128 _291;
    _Bool _292;
    uint8_t _293;
    bb_0: {
        _1 = input_add_i32(((int32_t)(7U)), ((int32_t)(3U))); goto bb_1;
    }
    bb_1: {
        goto bb_2;
    }
    bb_2: {
        fwrite("[INFO] add=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_1)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_3;
    }
    bb_3: {
        _8 = input_neg_i32(((int32_t)(7U))); goto bb_4;
    }
    bb_4: {
        goto bb_5;
    }
    bb_5: {
        fwrite("[INFO] neg=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_8)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_6;
    }
    bb_6: {
        _15 = ((int32_t)(7U));
        _16 = ((int32_t)(3U));
        _17 = _15;
        _18 = _16;
        _19 = ((_17) - (_18));
        goto bb_7;
    }
    bb_7: {
        fwrite("[INFO] sub=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_19)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_8;
    }
    bb_8: {
        _26 = _15;
        _27 = _16;
        _28 = ((_26) * (_27));
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] mul=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_28)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        _35 = _15;
        _36 = _16;
        _37 = ((_36) == (((int32_t)(0U))));
        if ((_37) == 0) goto bb_11; else { regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO); }
    }
    bb_11: {
        _38 = ((_36) == (((int32_t)(4294967295U))));
        _39 = ((_35) == (((int32_t)(2147483648U))));
        _40 = ((_38) & (_39));
        if ((_40) == 0) goto bb_12; else { { char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_OVERFLOW, "divide"); regseq_panic(_m); } }
    }
    bb_12: {
        _41 = ((_35) / (_36));
        goto bb_13;
    }
    bb_13: {
        fwrite("[INFO] div=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_41)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_14;
    }
    bb_14: {
        _48 = _15;
        _49 = _16;
        _50 = ((_49) == (((int32_t)(0U))));
        if ((_50) == 0) goto bb_15; else { regseq_panic(REGSEQ_MSG_REMAINDER_BY_ZERO); }
    }
    bb_15: {
        _51 = ((_49) == (((int32_t)(4294967295U))));
        _52 = ((_48) == (((int32_t)(2147483648U))));
        _53 = ((_51) & (_52));
        if ((_53) == 0) goto bb_16; else { { char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_OVERFLOW, "calculate the remainder"); regseq_panic(_m); } }
    }
    bb_16: {
        _54 = ((_48) % (_49));
        goto bb_17;
    }
    bb_17: {
        fwrite("[INFO] rem=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_54)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_18;
    }
    bb_18: {
        _61 = _15;
        _62 = _61;
        goto bb_19;
    }
    bb_19: {
        fwrite("[INFO] use=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_62)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_20;
    }
    bb_20: {
        _69 = ((uint8_t)(12U));
        _70 = ((uint8_t)(10U));
        _71 = _69;
        _72 = _70;
        _73 = ((_71) & (_72));
        goto bb_21;
    }
    bb_21: {
        fwrite("[INFO] band=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_73)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_22;
    }
    bb_22: {
        _80 = _69;
        _81 = _70;
        _82 = ((_80) | (_81));
        goto bb_23;
    }
    bb_23: {
        fwrite("[INFO] bor=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_82)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_24;
    }
    bb_24: {
        _89 = _69;
        _90 = _70;
        _91 = ((_89) ^ (_90));
        goto bb_25;
    }
    bb_25: {
        fwrite("[INFO] bxor=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_91)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_26;
    }
    bb_26: {
        _98 = ((uint8_t)((uint8_t)(((uint8_t)(1U))) << ((((int32_t)(3U))) & 7)));
        goto bb_27;
    }
    bb_27: {
        fwrite("[INFO] shl=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_98)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_28;
    }
    bb_28: {
        _105 = ((((uint8_t)(240U))) >> ((((int32_t)(2U))) & 7));
        goto bb_29;
    }
    bb_29: {
        fwrite("[INFO] shr=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_105)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_30;
    }
    bb_30: {
        _112 = ((uint8_t)((uint8_t)(((uint8_t)(1U))) << (((uint32_t)(3U)))));
        goto bb_31;
    }
    bb_31: {
        fwrite("[INFO] ushl=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_112)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_32;
    }
    bb_32: {
        _119 = ((((uint8_t)(240U))) >> (((uint32_t)(2U))));
        goto bb_33;
    }
    bb_33: {
        fwrite("[INFO] ushr=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_119)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_34;
    }
    bb_34: {
        _126 = ((((int32_t)(7U))) + (((int32_t)(3U))));
        goto bb_35;
    }
    bb_35: {
        fwrite("[INFO] uca=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_126)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_36;
    }
    bb_36: {
        _133 = ((((int32_t)(7U))) - (((int32_t)(3U))));
        goto bb_37;
    }
    bb_37: {
        fwrite("[INFO] ucs=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_133)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_38;
    }
    bb_38: {
        _140 = ((((int32_t)(7U))) * (((int32_t)(3U))));
        goto bb_39;
    }
    bb_39: {
        fwrite("[INFO] ucm=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_140)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_40;
    }
    bb_40: {
        { struct regseq_overflow_uint8_t _ov = regseq_add_uint8_t((((uint8_t)(250U))), (((uint8_t)(10U)))); _147._0 = _ov.v; _147._1 = _ov.overflow; }
        _148 = _147._0;
        _149 = _147._1;
        _150 = _148;
        goto bb_41;
    }
    bb_41: {
        _152 = _149;
        goto bb_42;
    }
    bb_42: {
        fwrite("[INFO] ova=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_150)); fwrite(_n, 1, _l, stdout); } fwrite(" ovo=", 1, 5, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_152)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_43;
    }
    bb_43: {
        { struct regseq_overflow_uint8_t _ov = regseq_sub_uint8_t((((uint8_t)(5U))), (((uint8_t)(10U)))); _159._0 = _ov.v; _159._1 = _ov.overflow; }
        _160 = _159._0;
        _161 = _159._1;
        _162 = _160;
        goto bb_44;
    }
    bb_44: {
        _164 = _161;
        goto bb_45;
    }
    bb_45: {
        fwrite("[INFO] ovs=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_162)); fwrite(_n, 1, _l, stdout); } fwrite(" ovso=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_164)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_46;
    }
    bb_46: {
        { struct regseq_overflow_uint8_t _ov = regseq_mul_uint8_t((((uint8_t)(200U))), (((uint8_t)(2U)))); _171._0 = _ov.v; _171._1 = _ov.overflow; }
        _172 = _171._0;
        _173 = _171._1;
        _174 = _172;
        goto bb_47;
    }
    bb_47: {
        _176 = _173;
        goto bb_48;
    }
    bb_48: {
        fwrite("[INFO] ovm=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_174)); fwrite(_n, 1, _l, stdout); } fwrite(" ovmo=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_176)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_49;
    }
    bb_49: {
        _183 = _15;
        _184 = _16;
        _185 = ((_183) == (_184));
        goto bb_50;
    }
    bb_50: {
        _187 = _15;
        _188 = _16;
        _189 = ((_187) < (_188));
        goto bb_51;
    }
    bb_51: {
        _191 = _15;
        _192 = _16;
        _193 = ((_191) <= (_192));
        goto bb_52;
    }
    bb_52: {
        _195 = _15;
        _196 = _16;
        _197 = ((_195) != (_196));
        goto bb_53;
    }
    bb_53: {
        _199 = _15;
        _200 = _16;
        _201 = ((_199) >= (_200));
        goto bb_54;
    }
    bb_54: {
        _203 = _15;
        _204 = _16;
        _205 = ((_203) > (_204));
        goto bb_55;
    }
    bb_55: {
        fwrite("[INFO] eq=", 1, 10, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_185)); fwrite(_n, 1, _l, stdout); } fwrite(" lt=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_189)); fwrite(_n, 1, _l, stdout); } fwrite(" le=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_193)); fwrite(_n, 1, _l, stdout); } fwrite(" ne=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_197)); fwrite(_n, 1, _l, stdout); } fwrite(" ge=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_201)); fwrite(_n, 1, _l, stdout); } fwrite(" gt=", 1, 4, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_205)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_56;
    }
    bb_56: {
        _212 = (~(((uint8_t)(0U))));
        goto bb_57;
    }
    bb_57: {
        fwrite("[INFO] not_u=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_212)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_58;
    }
    bb_58: {
        _219 = (!(1));
        goto bb_59;
    }
    bb_59: {
        fwrite("[INFO] not_b=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_219)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_60;
    }
    bb_60: {
        goto bb_61;
    }
    bb_61: {
        fwrite("[INFO] boolt=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(1)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_62;
    }
    bb_62: {
        _232 = ((uint32_t)((uint32_t)65U));
        goto bb_63;
    }
    bb_63: {
        fwrite("[INFO] char=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_232)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_64;
    }
    bb_64: {
        _239 = ((((int8_t)(5U))) + (((int8_t)(3U))));
        goto bb_65;
    }
    bb_65: {
        fwrite("[INFO] i8=", 1, 10, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_239)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_66;
    }
    bb_66: {
        _246 = ((((int16_t)(300U))) + (((int16_t)(100U))));
        goto bb_67;
    }
    bb_67: {
        fwrite("[INFO] i16=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_246)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_68;
    }
    bb_68: {
        _253 = ((((int64_t)(1000000000000ULL))) + (((int64_t)(1ULL))));
        goto bb_69;
    }
    bb_69: {
        fwrite("[INFO] i64=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%lld", (long long)(_253)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_70;
    }
    bb_70: {
        _260 = ((intptr_t)(18446744073709551609ULL));
        _261 = _260;
        _262 = ((_261) * (((intptr_t)(6ULL))));
        _263 = ((int64_t)(_262));
        goto bb_71;
    }
    bb_71: {
        fwrite("[INFO] isz=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%lld", (long long)(_263)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_72;
    }
    bb_72: {
        _270 = ((size_t)(7ULL));
        _271 = _270;
        _272 = ((_271) * (((size_t)(6ULL))));
        _273 = ((uint64_t)(_272));
        goto bb_73;
    }
    bb_73: {
        fwrite("[INFO] usz=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_273)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_74;
    }
    bb_74: {
        _280 = ((((__uint128_t)(__uint128_t)10000000000ULL)) * (((__uint128_t)(__uint128_t)10000000000ULL)));
        _281 = _280;
        _282 = ((_281) == (((__uint128_t)(((__uint128_t)5ULL << 64) | (__uint128_t)7766279631452241920ULL))));
        _283 = ((uint8_t)(_282));
        goto bb_75;
    }
    bb_75: {
        fwrite("[INFO] u128ck=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_283)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_76;
    }
    bb_76: {
        _290 = ((((__int128)(((__uint128_t)18446744073709551615ULL << 64) | (__uint128_t)18446744063709551616ULL))) * (((__int128)(__uint128_t)10000000000ULL)));
        _291 = _290;
        _292 = ((_291) == (((__int128)(((__uint128_t)18446744073709551610ULL << 64) | (__uint128_t)10680464442257309696ULL))));
        _293 = ((uint8_t)(_292));
        goto bb_77;
    }
    bb_77: {
        fwrite("[INFO] i128ck=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_293)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_78;
    }
    bb_78: {
        goto bb_79;
    }
    bb_79: {
        fwrite("[INFO] cuint=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(((uint32_t)(42U)))); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_80;
    }
    bb_80: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_003c4087039bcea0b922c3f84c906965[9] = { ((uint8_t)(117U)), ((uint8_t)(49U)), ((uint8_t)(50U)), ((uint8_t)(56U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_01a1c3d973cb520e2d139719ae21224f[6] = { ((uint8_t)(100U)), ((uint8_t)(105U)), ((uint8_t)(118U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_08fc4e575cfb9bebce0c322c2174fe5e[6] = { ((uint8_t)(98U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_098a11f69ee64fae42951121e0b1a0c5[8] = { ((uint8_t)(110U)), ((uint8_t)(111U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_09f589b09f8e282e2a034e0d52aa84e3[6] = { ((uint8_t)(109U)), ((uint8_t)(117U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_0c2d87e556dfd3449bb4fe8118451708[6] = { ((uint8_t)(97U)), ((uint8_t)(100U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_142333c60860539cc2453ac84c2b9314[6] = { ((uint8_t)(115U)), ((uint8_t)(117U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_21543c6dd444e5a91910b1029e6874a8[7] = { ((uint8_t)(98U)), ((uint8_t)(120U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_289a17933a81de4321b5c9569936ca55[7] = { ((uint8_t)(99U)), ((uint8_t)(104U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_40c6ac168f757de60b66029c5791fe7a[6] = { ((uint8_t)(115U)), ((uint8_t)(104U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_428d700e3bac08feee2759b950f5b729[7] = { ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(110U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_44bcf6ee8af6dab0e2804b598f406b85[6] = { ((uint8_t)(117U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5411fa943ad3705cea5ca8394ed3b1c4[35] = { ((uint8_t)(101U)), ((uint8_t)(113U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(108U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(110U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(103U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(103U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_566acb27e0d0f6a81dad14b2545bcf3c[6] = { ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(109U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5828c2346bf3bad3d69843441f318b9a[8] = { ((uint8_t)(110U)), ((uint8_t)(111U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(117U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5f8f9e7a5fb1a569e1595dc780686814[6] = { ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(122U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_6285fac975f1527b604cfbfe5eaa6c9c[8] = { ((uint8_t)(98U)), ((uint8_t)(111U)), ((uint8_t)(111U)), ((uint8_t)(108U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_654d68a7ba8f0ccf28458f5414e610da[7] = { ((uint8_t)(117U)), ((uint8_t)(115U)), ((uint8_t)(104U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_676008694dae925c5c754c02a84efae8[6] = { ((uint8_t)(105U)), ((uint8_t)(49U)), ((uint8_t)(54U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_70dad65e2b6be12883c1a34ffe7a6aa5[8] = { ((uint8_t)(99U)), ((uint8_t)(117U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_73470e7de97c4ffa3965bc392410ff22[6] = { ((uint8_t)(115U)), ((uint8_t)(104U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_7a92999d6d35548f408f42af77f5b134[6] = { ((uint8_t)(117U)), ((uint8_t)(115U)), ((uint8_t)(122U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_7c065424f6ca49cc1acb54143fc49b26[6] = { ((uint8_t)(117U)), ((uint8_t)(99U)), ((uint8_t)(109U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_87ef938a4cc394ba60088e43693d9698[6] = { ((uint8_t)(117U)), ((uint8_t)(99U)), ((uint8_t)(97U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_941c4bad0e9dd9166a160105e320a49d[13] = { ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(97U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(111U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_a38c772e2ad84eb6ceb930a71c9173a0[7] = { ((uint8_t)(117U)), ((uint8_t)(115U)), ((uint8_t)(104U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_ac70c7a8dc37ee656719e7beb52c5b23[14] = { ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(109U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(109U)), ((uint8_t)(111U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_ad7b98ed0691cbeeccd13228c7883787[9] = { ((uint8_t)(105U)), ((uint8_t)(49U)), ((uint8_t)(50U)), ((uint8_t)(56U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_b08c0ac34252856b9de2c9c86169fd00[6] = { ((uint8_t)(105U)), ((uint8_t)(54U)), ((uint8_t)(52U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_bc8f375971b482337a8ec782f3a2109f[5] = { ((uint8_t)(105U)), ((uint8_t)(56U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_d4f842a088db707ad40fd50ee8860777[14] = { ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(115U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(115U)), ((uint8_t)(111U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_ecd7ceb097b611bea9a8f95ec59c7f66[6] = { ((uint8_t)(110U)), ((uint8_t)(101U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_f0df92fc56866e349175e48e1312e83f[6] = { ((uint8_t)(117U)), ((uint8_t)(99U)), ((uint8_t)(115U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
