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
struct rust_slice_uint8_t;
union input_UPad;

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
struct rust_slice_uint8_t {
    const uint8_t *data;
    size_t len;
};
union input_UPad {
    uint32_t w;
    uint8_t b3[3];
    unsigned char _raw[4];
} __attribute__((aligned(4)));
_Static_assert(sizeof(union input_UPad) == 4, "input_UPad size");
_Static_assert(_Alignof(union input_UPad) == 4, "input_UPad align");
_Static_assert(offsetof(union input_UPad, w) == 0, "input_UPad.w offset");
_Static_assert(offsetof(union input_UPad, b3) == 0, "input_UPad.b3 offset");
_Static_assert(offsetof(union input_UPad, _raw) == 0, "input_UPad._raw offset");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_17bc092cf616fbf282c0456f761a9db8[5];
extern const uint8_t input__regseq_synth__const_memory_static_1dfb50aafe312d093c9fce43fe5524c4[13];
extern const uint8_t input__regseq_synth__const_memory_static_36032358cca3978a176cdde41b22e122[10];
extern const uint8_t input__regseq_synth__const_memory_static_43ad15a56aff7f1e001fa60d99b75d5e[8];
extern const uint8_t input__regseq_synth__const_memory_static_4bb6ea1695c936bb54c0b5e9af6a3cab[12];
extern const uint8_t input__regseq_synth__const_memory_static_5802749320a13727a877a65fa5824568[20];
extern const uint8_t input__regseq_synth__const_memory_static_5fdcc487cb09505203683b6b71b9f120[16];
extern const uint8_t input__regseq_synth__const_memory_static_695981f59fcf6b02710964dec0425503[10];
extern const uint8_t input__regseq_synth__const_memory_static_6b1ef94ea77385be5fe5af2a31a5e6eb[17];
extern const uint8_t input__regseq_synth__const_memory_static_8547aa65d302bda156382f34f7579c73[5];
extern const uint8_t input__regseq_synth__const_memory_static_86d5535e82dfd12b3e4c388f812d143f[13];
extern const uint8_t input__regseq_synth__const_memory_static_8cfe3209f51f7623e6b03e0c919843c4[12];
extern const uint8_t input__regseq_synth__const_memory_static_8d5375239eafc51d415fd029a4ecb59a[9];
extern const uint8_t input__regseq_synth__const_memory_static_91020bb3d86febe82aaadf45429d9159[10];
extern const uint8_t input__regseq_synth__const_memory_static_92e776669391b159f7a392bef8015941[14];
extern const uint8_t input__regseq_synth__const_memory_static_9d1beee180ff29903027157f117dec13[12];
extern const uint8_t input__regseq_synth__const_memory_static_b2bec1dd493746970b009381c5201507[9];
extern const uint8_t input__regseq_synth__const_memory_static_bd082e2b41bdc16ab1bb976e0f1ae906[15];
extern const uint8_t input__regseq_synth__const_memory_static_c078510b3ac283920fe5c385dc0a4b82[10];
extern const uint8_t input__regseq_synth__const_memory_static_e86cfb64ee6fa4cec998ff6f6cbccbc8[9];
extern const const uint32_t *input_REF_ARR[1];
extern const uint32_t input_TARGET;
extern const union input_UPad input_UPAD;

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
    int32_t _1;
    const int32_t *_2;
    const int32_t *_3;
    int32_t _4;
    const int32_t *_11;
    const int32_t *_12;
    size_t _13;
    _Bool _14;
    uint8_t _15;
    int32_t _22;
    int32_t _23;
    _Bool _24;
    uint8_t _25;
    uint8_t _32[5];
    const uint8_t (*_33)[5];
    const uint8_t (*_34)[5];
    struct rust_slice_uint8_t _35;
    size_t _36;
    size_t _37;
    _Bool _38;
    uint8_t _39;
    size_t _46;
    size_t _47;
    _Bool _48;
    uint8_t _49;
    struct rust_slice_uint8_t _56;
    size_t _57;
    size_t _58;
    uint64_t _59;
    struct rust_str _66;
    struct rust_str _67;
    struct rust_str _68;
    size_t _69;
    size_t _70;
    uint64_t _71;
    const uint8_t (*_78)[5];
    const uint8_t *_79;
    const uint8_t *_80;
    struct rust_slice_uint8_t _81;
    struct rust_slice_uint8_t _82;
    size_t _83;
    size_t _84;
    uint64_t _85;
    size_t _92;
    size_t _93;
    size_t _94;
    _Bool _95;
    uint8_t _96;
    uint8_t _97;
    uint8_t _98;
    uint8_t _100;
    size_t _107;
    size_t _108;
    size_t _109;
    _Bool _110;
    const uint8_t *_111;
    struct rust_slice_uint8_t _112;
    struct rust_slice_uint8_t _113;
    size_t _114;
    size_t _115;
    uint64_t _116;
    size_t _118;
    size_t _119;
    _Bool _120;
    uint8_t _121;
    const uint8_t (*_128)[5];
    const uint8_t *_129;
    const uint8_t *_130;
    const uint8_t *_131;
    uint8_t _132;
    size_t _139;
    _Bool _140;
    uint8_t _141;
    uint8_t _142;
    _Bool _143;
    uint8_t _144;
    uint8_t _151[3];
    uint8_t _152[3];
    const uint8_t (*_153)[3];
    const uint8_t *_154;
    uint8_t (*_155)[3];
    uint8_t *_156;
    size_t _157;
    _Bool _158;
    uint8_t _159;
    size_t _161;
    _Bool _162;
    uint8_t _163;
    size_t _165;
    _Bool _166;
    uint8_t _167;
    const int32_t *_174;
    const int32_t *_175;
    size_t _176;
    _Bool _177;
    uint8_t _178;
    const uint32_t *_185;
    const uint32_t *_186;
    const uint32_t *_187;
    uint32_t _188;
    const uint32_t *_195;
    size_t _196;
    _Bool _197;
    uint8_t _198;
    const uint32_t *const (*_205)[1];
    size_t _206;
    _Bool _207;
    const uint32_t *_208;
    uint32_t _209;
    const union input_UPad *_222;
    size_t _223;
    _Bool _224;
    uint8_t _225;
    const union input_UPad *_226;
    size_t _227;
    _Bool _228;
    uint8_t _229;
    const union input_UPad *_230;
    size_t _231;
    _Bool _232;
    uint8_t _233;
    uint8_t _234;
    uint8_t _236;
    uint8_t _238;
    bb_0: {
        _1 = ((int32_t)(314U));
        _2 = &(_1);
        _3 = &((*_2));
        _4 = (*_3);
        goto bb_1;
    }
    bb_1: {
        fwrite("[INFO] deref_r=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%d", (int)(_4)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_2;
    }
    bb_2: {
        _11 = &(_1);
        _12 = _11;
        _13 = ((uintptr_t)(_12));
        _14 = ((_13) != (((size_t)(0ULL))));
        _15 = ((uint8_t)(_14));
        goto bb_3;
    }
    bb_3: {
        fwrite("[INFO] p_nonnull=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_15)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_4;
    }
    bb_4: {
        _22 = (*_11);
        _23 = _1;
        _24 = ((_22) == (_23));
        _25 = ((uint8_t)(_24));
        goto bb_5;
    }
    bb_5: {
        fwrite("[INFO] p_deref_eq=", 1, 18, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_25)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_6;
    }
    bb_6: {
        _32[0] = ((uint8_t)(10U)); _32[1] = ((uint8_t)(20U)); _32[2] = ((uint8_t)(30U)); _32[3] = ((uint8_t)(40U)); _32[4] = ((uint8_t)(50U));
        _33 = &(_32);
        _34 = &((*_33));
        _35 = (struct rust_slice_uint8_t){ .data = (const uint8_t *)(_34), .len = 5 };
        _36 = ((size_t)(0ULL));
        _37 = (_35).len;
        _38 = ((_36) < (_37));
        if ((_38) == 1) goto bb_7; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%zu", (size_t)(_37)); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_36)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_7: {
        _39 = _35.data[_36];
        goto bb_8;
    }
    bb_8: {
        fwrite("[INFO] s0=", 1, 10, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_39)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_9;
    }
    bb_9: {
        _46 = ((size_t)(4ULL));
        _47 = (_35).len;
        _48 = ((_46) < (_47));
        if ((_48) == 1) goto bb_10; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%zu", (size_t)(_47)); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_46)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_10: {
        _49 = _35.data[_46];
        goto bb_11;
    }
    bb_11: {
        fwrite("[INFO] s_last=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_49)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_12;
    }
    bb_12: {
        _56 = _35;
        _57 = (_56).len;
        _58 = _57;
        _59 = ((uint64_t)(_58));
        goto bb_13;
    }
    bb_13: {
        fwrite("[INFO] s_len=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_59)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_14;
    }
    bb_14: {
        _66 = (struct rust_str){ .data = (const uint8_t *)(&input__regseq_synth__const_memory_static_8547aa65d302bda156382f34f7579c73), .len = ((size_t)(5ULL)) };
        _67 = _66;
        _68 = _67;
        _69 = (_68).len;
        _70 = _69;
        _71 = ((uint64_t)(_70));
        goto bb_15;
    }
    bb_15: {
        fwrite("[INFO] st_len=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_71)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_16;
    }
    bb_16: {
        _78 = &(_32);
        _79 = ((const uint8_t *)(_78));
        _80 = _79;
        _81 = (struct rust_slice_uint8_t){ .data = (const uint8_t *)(_80), .len = ((size_t)(3ULL)) };
        _82 = _81;
        _83 = (_82).len;
        _84 = _83;
        _85 = ((uint64_t)(_84));
        goto bb_17;
    }
    bb_17: {
        fwrite("[INFO] fat_len=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_85)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_18;
    }
    bb_18: {
        _92 = (_35).len;
        _93 = _92;
        _94 = ((size_t)(2ULL));
        _95 = ((_93) >= (_94));
        switch (_95) { case 0ULL: goto bb_23; default: goto bb_19; }
    }
    bb_19: {
        _96 = _35.data[0];
        _97 = _35.data[_35.len - 1];
        _98 = _96;
        goto bb_20;
    }
    bb_20: {
        _100 = _97;
        goto bb_21;
    }
    bb_21: {
        fwrite("[INFO] first=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_98)); fwrite(_n, 1, _l, stdout); } fwrite(" last=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_100)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_22;
    }
    bb_22: {
        goto bb_24;
    }
    bb_23: {
        goto bb_24;
    }
    bb_24: {
        _107 = (_35).len;
        _108 = _107;
        _109 = ((size_t)(1ULL));
        _110 = ((_108) >= (_109));
        switch (_110) { case 0ULL: goto bb_30; default: goto bb_25; }
    }
    bb_25: {
        _111 = &(_35.data[0]);
        _112 = (struct rust_slice_uint8_t){ .data = (const uint8_t *)((_35.data) + 1), .len = (_35.len - 0 - 1) };
        _113 = _112;
        _114 = (_113).len;
        _115 = _114;
        _116 = ((uint64_t)(_115));
        goto bb_26;
    }
    bb_26: {
        _118 = ((size_t)(0ULL));
        _119 = (_112).len;
        _120 = ((_118) < (_119));
        if ((_120) == 1) goto bb_27; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%zu", (size_t)(_119)); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_118)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_27: {
        _121 = _112.data[_118];
        goto bb_28;
    }
    bb_28: {
        fwrite("[INFO] tail_len=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_116)); fwrite(_n, 1, _l, stdout); } fwrite(" tail0=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_121)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_29;
    }
    bb_29: {
        goto bb_31;
    }
    bb_30: {
        goto bb_31;
    }
    bb_31: {
        _128 = &(_32);
        _129 = ((const uint8_t *)(_128));
        _130 = _129;
        _131 = ((_130) + (((intptr_t)(2ULL))));
        _132 = (*_131);
        goto bb_32;
    }
    bb_32: {
        fwrite("[INFO] offset2=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_132)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_33;
    }
    bb_33: {
        _139 = ((size_t)(1ULL));
        _140 = ((_139) < (((size_t)(5ULL))));
        if ((_140) == 1) goto bb_34; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(5ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_139)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_34: {
        _141 = _32[_139];
        _142 = _141;
        _143 = ((_142) > (((uint8_t)(0U))));
        if (!(_143)) __builtin_unreachable();
        _144 = _141;
        goto bb_35;
    }
    bb_35: {
        fwrite("[INFO] assume_ok=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_144)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_36;
    }
    bb_36: {
        _151[0] = ((uint8_t)(7U)); _151[1] = ((uint8_t)(8U)); _151[2] = ((uint8_t)(9U));
        _152[0] = ((uint8_t)(0U)); _152[1] = ((uint8_t)(0U)); _152[2] = ((uint8_t)(0U));
        _153 = &(_151);
        _154 = ((const uint8_t *)(_153));
        _155 = &(_152);
        _156 = ((uint8_t *)(_155));
        memcpy(_156, _154, (((size_t)(3ULL))) * sizeof(*(_156)));
        _157 = ((size_t)(0ULL));
        _158 = ((_157) < (((size_t)(3ULL))));
        if ((_158) == 1) goto bb_37; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_157)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_37: {
        _159 = _152[_157];
        goto bb_38;
    }
    bb_38: {
        _161 = ((size_t)(1ULL));
        _162 = ((_161) < (((size_t)(3ULL))));
        if ((_162) == 1) goto bb_39; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_161)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_39: {
        _163 = _152[_161];
        goto bb_40;
    }
    bb_40: {
        _165 = ((size_t)(2ULL));
        _166 = ((_165) < (((size_t)(3ULL))));
        if ((_166) == 1) goto bb_41; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_165)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_41: {
        _167 = _152[_165];
        goto bb_42;
    }
    bb_42: {
        fwrite("[INFO] copied=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_159)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_163)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_167)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_43;
    }
    bb_43: {
        _174 = ((const int32_t *)0);
        _175 = _174;
        _176 = ((uintptr_t)(_175));
        _177 = ((_176) == (((size_t)(0ULL))));
        _178 = ((uint8_t)(_177));
        goto bb_44;
    }
    bb_44: {
        fwrite("[INFO] is_null=", 1, 15, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_178)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_45;
    }
    bb_45: {
        _185 = ((const uint32_t *)&input_TARGET);
        _186 = &((*_185));
        _187 = &((*_186));
        _188 = (*_187);
        goto bb_46;
    }
    bb_46: {
        fwrite("[INFO] target=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_188)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_47;
    }
    bb_47: {
        _195 = &((*_187));
        _196 = ((uintptr_t)(_195));
        _197 = ((_196) != (((size_t)(0ULL))));
        _198 = ((uint8_t)(_197));
        goto bb_48;
    }
    bb_48: {
        fwrite("[INFO] target_nonnull=", 1, 22, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_198)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_49;
    }
    bb_49: {
        _205 = ((const uint32_t *const (*)[1])&input_REF_ARR);
        _206 = ((size_t)(0ULL));
        _207 = ((_206) < (((size_t)(1ULL))));
        if ((_207) == 1) goto bb_50; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(1ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_206)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_50: {
        _208 = (*_205)[_206];
        _209 = (*_208);
        goto bb_51;
    }
    bb_51: {
        fwrite("[INFO] ref_reloc=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_209)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_52;
    }
    bb_52: {
        goto bb_53;
    }
    bb_53: {
        fwrite("[INFO] co13_marker=", 1, 19, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(((uint8_t)(1U)))); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_54;
    }
    bb_54: {
        _222 = ((const union input_UPad *)&input_UPAD);
        _223 = ((size_t)(0ULL));
        _224 = ((_223) < (((size_t)(3ULL))));
        if ((_224) == 1) goto bb_55; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_223)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_55: {
        _225 = (*_222).b3[_223];
        _226 = ((const union input_UPad *)&input_UPAD);
        _227 = ((size_t)(1ULL));
        _228 = ((_227) < (((size_t)(3ULL))));
        if ((_228) == 1) goto bb_56; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_227)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_56: {
        _229 = (*_226).b3[_227];
        _230 = ((const union input_UPad *)&input_UPAD);
        _231 = ((size_t)(2ULL));
        _232 = ((_231) < (((size_t)(3ULL))));
        if ((_232) == 1) goto bb_57; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_231)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_57: {
        _233 = (*_230).b3[_231];
        _234 = _225;
        goto bb_58;
    }
    bb_58: {
        _236 = _229;
        goto bb_59;
    }
    bb_59: {
        _238 = _233;
        goto bb_60;
    }
    bb_60: {
        fwrite("[INFO] upad=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_234)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_236)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_238)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_61;
    }
    bb_61: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_17bc092cf616fbf282c0456f761a9db8[5] = { ((uint8_t)(115U)), ((uint8_t)(48U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_1dfb50aafe312d093c9fce43fe5524c4[13] = { ((uint8_t)(112U)), ((uint8_t)(95U)), ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(102U)), ((uint8_t)(95U)), ((uint8_t)(101U)), ((uint8_t)(113U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_36032358cca3978a176cdde41b22e122[10] = { ((uint8_t)(102U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_43ad15a56aff7f1e001fa60d99b75d5e[8] = { ((uint8_t)(115U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_4bb6ea1695c936bb54c0b5e9af6a3cab[12] = { ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(115U)), ((uint8_t)(117U)), ((uint8_t)(109U)), ((uint8_t)(101U)), ((uint8_t)(95U)), ((uint8_t)(111U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5802749320a13727a877a65fa5824568[20] = { ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(105U)), ((uint8_t)(108U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(105U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5fdcc487cb09505203683b6b71b9f120[16] = { ((uint8_t)(102U)), ((uint8_t)(105U)), ((uint8_t)(114U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(108U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_695981f59fcf6b02710964dec0425503[10] = { ((uint8_t)(111U)), ((uint8_t)(102U)), ((uint8_t)(102U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(116U)), ((uint8_t)(50U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_6b1ef94ea77385be5fe5af2a31a5e6eb[17] = { ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(103U)), ((uint8_t)(101U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(110U)), ((uint8_t)(111U)), ((uint8_t)(110U)), ((uint8_t)(110U)), ((uint8_t)(117U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_8547aa65d302bda156382f34f7579c73[5] = { ((uint8_t)(104U)), ((uint8_t)(101U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(111U)) };
const uint8_t input__regseq_synth__const_memory_static_86d5535e82dfd12b3e4c388f812d143f[13] = { ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(97U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_8cfe3209f51f7623e6b03e0c919843c4[12] = { ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(102U)), ((uint8_t)(95U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(99U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_8d5375239eafc51d415fd029a4ecb59a[9] = { ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(103U)), ((uint8_t)(101U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_91020bb3d86febe82aaadf45429d9159[10] = { ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(102U)), ((uint8_t)(95U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_92e776669391b159f7a392bef8015941[14] = { ((uint8_t)(99U)), ((uint8_t)(111U)), ((uint8_t)(49U)), ((uint8_t)(51U)), ((uint8_t)(95U)), ((uint8_t)(109U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(107U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_9d1beee180ff29903027157f117dec13[12] = { ((uint8_t)(112U)), ((uint8_t)(95U)), ((uint8_t)(110U)), ((uint8_t)(111U)), ((uint8_t)(110U)), ((uint8_t)(110U)), ((uint8_t)(117U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_b2bec1dd493746970b009381c5201507[9] = { ((uint8_t)(115U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_bd082e2b41bdc16ab1bb976e0f1ae906[15] = { ((uint8_t)(99U)), ((uint8_t)(111U)), ((uint8_t)(112U)), ((uint8_t)(105U)), ((uint8_t)(101U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_c078510b3ac283920fe5c385dc0a4b82[10] = { ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(95U)), ((uint8_t)(110U)), ((uint8_t)(117U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_e86cfb64ee6fa4cec998ff6f6cbccbc8[9] = { ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const const uint32_t *input_REF_ARR[1] = { ((const uint32_t *)&input_TARGET) };
const uint32_t input_TARGET = ((uint32_t)(43981U));
const union input_UPad input_UPAD = (union input_UPad){ ._raw = { 119, 136, 153, 0 } };
