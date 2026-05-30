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
struct input_Unit;
struct input_PointC;
struct input_PointRust;
struct input_PackedRegs;
union input_Bits;
struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_WithCell;
struct input_WithPhantom;
struct tuple_756241e1be8c9396;
struct tuple_de24483a76fbb974;
struct core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c;

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
struct input_Unit {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Unit) == 0, "input_Unit size");
_Static_assert(_Alignof(struct input_Unit) == 1, "input_Unit align");
struct input_PointC {
    uint8_t a;
    unsigned char _pad0[7];
    uint64_t b;
    uint16_t c;
    unsigned char _pad1[6];
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_PointC) == 24, "input_PointC size");
_Static_assert(_Alignof(struct input_PointC) == 8, "input_PointC align");
_Static_assert(offsetof(struct input_PointC, a) == 0, "input_PointC.a offset");
_Static_assert(offsetof(struct input_PointC, b) == 8, "input_PointC.b offset");
_Static_assert(offsetof(struct input_PointC, c) == 16, "input_PointC.c offset");
struct input_PointRust {
    uint64_t b;
    uint16_t c;
    uint8_t a;
    unsigned char _pad0[5];
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_PointRust) == 16, "input_PointRust size");
_Static_assert(_Alignof(struct input_PointRust) == 8, "input_PointRust align");
_Static_assert(offsetof(struct input_PointRust, b) == 0, "input_PointRust.b offset");
_Static_assert(offsetof(struct input_PointRust, c) == 8, "input_PointRust.c offset");
_Static_assert(offsetof(struct input_PointRust, a) == 10, "input_PointRust.a offset");
struct input_PackedRegs {
    uint8_t tag;
    uint32_t word;
} __attribute__((packed, aligned(1)));
_Static_assert(sizeof(struct input_PackedRegs) == 5, "input_PackedRegs size");
_Static_assert(_Alignof(struct input_PackedRegs) == 1, "input_PackedRegs align");
_Static_assert(offsetof(struct input_PackedRegs, tag) == 0, "input_PackedRegs.tag offset");
_Static_assert(offsetof(struct input_PackedRegs, word) == 1, "input_PackedRegs.word offset");
union input_Bits {
    uint32_t u;
    uint8_t bytes[4];
    unsigned char _raw[4];
} __attribute__((aligned(4)));
_Static_assert(sizeof(union input_Bits) == 4, "input_Bits size");
_Static_assert(_Alignof(union input_Bits) == 4, "input_Bits align");
_Static_assert(offsetof(union input_Bits, u) == 0, "input_Bits.u offset");
_Static_assert(offsetof(union input_Bits, bytes) == 0, "input_Bits.bytes offset");
_Static_assert(offsetof(union input_Bits, _raw) == 0, "input_Bits._raw offset");
struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c {
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c align");
_Static_assert(offsetof(struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c, value) == 0, "core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c.value offset");
struct input_WithCell {
    struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c cell;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_WithCell) == 4, "input_WithCell size");
_Static_assert(_Alignof(struct input_WithCell) == 4, "input_WithCell align");
_Static_assert(offsetof(struct input_WithCell, cell) == 0, "input_WithCell.cell offset");
struct input_WithPhantom {
    uint32_t val;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_WithPhantom) == 4, "input_WithPhantom size");
_Static_assert(_Alignof(struct input_WithPhantom) == 4, "input_WithPhantom align");
_Static_assert(offsetof(struct input_WithPhantom, val) == 0, "input_WithPhantom.val offset");
struct tuple_756241e1be8c9396 {
    uint16_t _1;
    uint8_t _0;
    unsigned char _pad0[1];
    uint32_t _2;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct tuple_756241e1be8c9396) == 8, "tuple_756241e1be8c9396 size");
_Static_assert(_Alignof(struct tuple_756241e1be8c9396) == 4, "tuple_756241e1be8c9396 align");
_Static_assert(offsetof(struct tuple_756241e1be8c9396, _1) == 0, "tuple_756241e1be8c9396._1 offset");
_Static_assert(offsetof(struct tuple_756241e1be8c9396, _0) == 2, "tuple_756241e1be8c9396._0 offset");
_Static_assert(offsetof(struct tuple_756241e1be8c9396, _2) == 4, "tuple_756241e1be8c9396._2 offset");
struct tuple_de24483a76fbb974 {
    uint16_t _0;
    uint16_t _1;
    uint16_t _2;
} __attribute__((aligned(2)));
_Static_assert(sizeof(struct tuple_de24483a76fbb974) == 6, "tuple_de24483a76fbb974 size");
_Static_assert(_Alignof(struct tuple_de24483a76fbb974) == 2, "tuple_de24483a76fbb974 align");
_Static_assert(offsetof(struct tuple_de24483a76fbb974, _0) == 0, "tuple_de24483a76fbb974._0 offset");
_Static_assert(offsetof(struct tuple_de24483a76fbb974, _1) == 2, "tuple_de24483a76fbb974._1 offset");
_Static_assert(offsetof(struct tuple_de24483a76fbb974, _2) == 4, "tuple_de24483a76fbb974._2 offset");
struct core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 0, "core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 1, "core_marker_PhantomData__5ae2a5e07d9ec8b0eb418e8c3168f23c align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_consume_unit(void);
__attribute__((noinline)) void input_consume_zst(void);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_009f190e31b85c24db7116c46e941f71[9];
extern const uint8_t input__regseq_synth__const_memory_static_01bfd3a7a434299729d097bfeb74b5c7[9];
extern const uint8_t input__regseq_synth__const_memory_static_26f545e37bbd5afa7250ff88423921d4[23];
extern const uint8_t input__regseq_synth__const_memory_static_2e6d98fdb2ec8e3e779a28f0575c2a17[7];
extern const uint8_t input__regseq_synth__const_memory_static_533db19f39dd0b991a23d66a180c0460[20];
extern const uint8_t input__regseq_synth__const_memory_static_571c569302dd439574161324abed85f8[14];
extern const uint8_t input__regseq_synth__const_memory_static_5838df12e9e40af39824d9cc8c800e8f[26];
extern const uint8_t input__regseq_synth__const_memory_static_764fe5928522ec17a58e098856e16de7[12];
extern const uint8_t input__regseq_synth__const_memory_static_93a9960e248dcd4051648901a03ec405[9];
extern const uint8_t input__regseq_synth__const_memory_static_adc87907558cf5820d3d1ab064665305[11];
extern const uint8_t input__regseq_synth__const_memory_static_aecdb2be640840ece06249cd41beca46[9];
extern const uint8_t input__regseq_synth__const_memory_static_bb6413893f6e2e5ef37ec9ca467bb8f2[9];
extern const uint8_t input__regseq_synth__const_memory_static_e389a77d4dec9f0e33cd2edd4566998b[11];
extern const uint8_t input__regseq_synth__const_memory_static_ff3f2247fdd3c8a5992283438e4446db[23];
extern const struct input_PointC input_APOINT;
extern const uint8_t input_BYTES[4];

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
__attribute__((noinline)) void input_consume_unit(void) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) void input_consume_zst(void) {
    bb_0: {
        return;
    }
}
void input_regseq_main(void) {
    struct input_PointC _1;
    uint8_t _2;
    uint64_t _4;
    uint16_t _6;
    struct input_PointRust _13;
    uint8_t _14;
    uint64_t _16;
    uint16_t _18;
    struct input_PackedRegs _25;
    uint8_t _26;
    uint32_t _27;
    uint8_t _28;
    uint32_t _30;
    union input_Bits _37;
    size_t _38;
    _Bool _39;
    uint8_t _40;
    uint8_t _41;
    struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c _48;
    struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c _49;
    struct input_WithCell _50;
    const struct core_cell_UnsafeCell__5ae2a5e07d9ec8b0eb418e8c3168f23c *_51;
    const uint32_t *_52;
    uint32_t _53;
    uint32_t _54;
    struct input_WithPhantom _61;
    uint32_t _62;
    uint8_t _69[3];
    size_t _70;
    size_t _71;
    _Bool _72;
    uint8_t _73;
    size_t _80;
    _Bool _81;
    uint8_t _82;
    uint8_t _89[4];
    size_t _90;
    _Bool _91;
    uint8_t _92;
    struct tuple_756241e1be8c9396 _99;
    uint8_t _100;
    uint16_t _102;
    uint32_t _104;
    const uint8_t (*_119)[4];
    size_t _120;
    _Bool _121;
    uint8_t _122;
    const struct input_PointC *_129;
    uint64_t _130;
    struct tuple_de24483a76fbb974 _137;
    uint16_t _138;
    bb_0: {
        _1 = (struct input_PointC){ .a = ((uint8_t)(9U)), .b = ((uint64_t)(100ULL)), .c = ((uint16_t)(7U)) };
        _2 = _1.a;
        goto bb_1;
    }
    bb_1: {
        _4 = _1.b;
        goto bb_2;
    }
    bb_2: {
        _6 = _1.c;
        goto bb_3;
    }
    bb_3: {
        fwrite("[INFO] pc.a=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_2)); fwrite(_n, 1, _l, stdout); } fwrite(" pc.b=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_4)); fwrite(_n, 1, _l, stdout); } fwrite(" pc.c=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_6)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_4;
    }
    bb_4: {
        _13 = (struct input_PointRust){ .a = ((uint8_t)(5U)), .b = ((uint64_t)(6ULL)), .c = ((uint16_t)(8U)) };
        _14 = _13.a;
        goto bb_5;
    }
    bb_5: {
        _16 = _13.b;
        goto bb_6;
    }
    bb_6: {
        _18 = _13.c;
        goto bb_7;
    }
    bb_7: {
        fwrite("[INFO] pr.a=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_14)); fwrite(_n, 1, _l, stdout); } fwrite(" pr.b=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_16)); fwrite(_n, 1, _l, stdout); } fwrite(" pr.c=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_18)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_8;
    }
    bb_8: {
        _25 = (struct input_PackedRegs){ .tag = ((uint8_t)(171U)), .word = ((uint32_t)(287454020U)) };
        _26 = _25.tag;
        _27 = _25.word;
        _28 = _26;
        goto bb_9;
    }
    bb_9: {
        _30 = _27;
        goto bb_10;
    }
    bb_10: {
        fwrite("[INFO] pk.tag=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_28)); fwrite(_n, 1, _l, stdout); } fwrite(" pk.word=", 1, 9, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_30)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_11;
    }
    bb_11: {
        _37 = (union input_Bits){ .u = ((uint32_t)(67305985U)) };
        _38 = ((size_t)(0ULL));
        _39 = ((_38) < (((size_t)(4ULL))));
        if ((_39) == 1) goto bb_12; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(4ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_38)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_12: {
        _40 = _37.bytes[_38];
        _41 = _40;
        goto bb_13;
    }
    bb_13: {
        fwrite("[INFO] bits.u_lo=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_41)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_14;
    }
    bb_14: {
        memcpy(&(_48), &(uint32_t){((uint32_t)(77U))}, sizeof(_48));
        _49 = _48;
        _50 = (struct input_WithCell){ .cell = _49 };
        _51 = &(_50.cell);
        _52 = ((const uint32_t *)(_51));
        _53 = (*_52);
        _54 = _53;
        goto bb_15;
    }
    bb_15: {
        fwrite("[INFO] cell=", 1, 12, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_54)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_16;
    }
    bb_16: {
        _61 = (struct input_WithPhantom){ .val = ((uint32_t)(88U)) };
        _62 = _61.val;
        goto bb_17;
    }
    bb_17: {
        fwrite("[INFO] phantom.val=", 1, 19, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_62)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_18;
    }
    bb_18: {
        _69[0] = ((uint8_t)(1U)); _69[1] = ((uint8_t)(2U)); _69[2] = ((uint8_t)(3U));
        _70 = ((size_t)(2ULL));
        _71 = _70;
        _72 = ((_71) < (((size_t)(3ULL))));
        if ((_72) == 1) goto bb_19; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_71)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_19: {
        _73 = _69[_71];
        goto bb_20;
    }
    bb_20: {
        fwrite("[INFO] arr[2]=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_73)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_21;
    }
    bb_21: {
        _80 = ((size_t)(0ULL));
        _81 = ((_80) < (((size_t)(3ULL))));
        if ((_81) == 1) goto bb_22; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(3ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_80)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_22: {
        _82 = _69[_80];
        goto bb_23;
    }
    bb_23: {
        fwrite("[INFO] arr[0]=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_82)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_24;
    }
    bb_24: {
        for (size_t _ri = 0; _ri < 4; _ri++) _89[_ri] = ((uint8_t)(7U));
        _90 = ((size_t)(3ULL));
        _91 = ((_90) < (((size_t)(4ULL))));
        if ((_91) == 1) goto bb_25; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(4ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_90)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_25: {
        _92 = _89[_90];
        goto bb_26;
    }
    bb_26: {
        fwrite("[INFO] rep[3]=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_92)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_27;
    }
    bb_27: {
        _99 = (struct tuple_756241e1be8c9396){ ._0 = ((uint8_t)(1U)), ._1 = ((uint16_t)(2U)), ._2 = ((uint32_t)(3U)) };
        _100 = _99._0;
        goto bb_28;
    }
    bb_28: {
        _102 = _99._1;
        goto bb_29;
    }
    bb_29: {
        _104 = _99._2;
        goto bb_30;
    }
    bb_30: {
        fwrite("[INFO] tup.0=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_100)); fwrite(_n, 1, _l, stdout); } fwrite(" tup.1=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_102)); fwrite(_n, 1, _l, stdout); } fwrite(" tup.2=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_104)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_31;
    }
    bb_31: {
        input_consume_unit(); goto bb_32;
    }
    bb_32: {
        input_consume_zst(); goto bb_33;
    }
    bb_33: {
        goto bb_34;
    }
    bb_34: {
        fwrite("[INFO] zst_ok=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(((uint8_t)(1U)))); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_35;
    }
    bb_35: {
        _119 = ((const uint8_t (*)[4])&input_BYTES);
        _120 = ((size_t)(1ULL));
        _121 = ((_120) < (((size_t)(4ULL))));
        if ((_121) == 1) goto bb_36; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(4ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_120)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_36: {
        _122 = (*_119)[_120];
        goto bb_37;
    }
    bb_37: {
        fwrite("[INFO] bytes[1]=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_122)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_38;
    }
    bb_38: {
        _129 = ((const struct input_PointC *)&input_APOINT);
        _130 = (*_129).b;
        goto bb_39;
    }
    bb_39: {
        fwrite("[INFO] apoint.b=", 1, 16, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%llu", (unsigned long long)(_130)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_40;
    }
    bb_40: {
        _137 = (struct tuple_de24483a76fbb974){ ._0 = ((uint16_t)(100U)), ._1 = ((uint16_t)(200U)), ._2 = ((uint16_t)(300U)) };
        _138 = _137._1;
        goto bb_41;
    }
    bb_41: {
        fwrite("[INFO] words1=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_138)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_42;
    }
    bb_42: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_009f190e31b85c24db7116c46e941f71[9] = { ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(114U)), ((uint8_t)(91U)), ((uint8_t)(48U)), ((uint8_t)(93U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_01bfd3a7a434299729d097bfeb74b5c7[9] = { ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(112U)), ((uint8_t)(91U)), ((uint8_t)(51U)), ((uint8_t)(93U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_26f545e37bbd5afa7250ff88423921d4[23] = { ((uint8_t)(112U)), ((uint8_t)(99U)), ((uint8_t)(46U)), ((uint8_t)(97U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(99U)), ((uint8_t)(46U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(99U)), ((uint8_t)(46U)), ((uint8_t)(99U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_2e6d98fdb2ec8e3e779a28f0575c2a17[7] = { ((uint8_t)(99U)), ((uint8_t)(101U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_533db19f39dd0b991a23d66a180c0460[20] = { ((uint8_t)(112U)), ((uint8_t)(107U)), ((uint8_t)(46U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(107U)), ((uint8_t)(46U)), ((uint8_t)(119U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_571c569302dd439574161324abed85f8[14] = { ((uint8_t)(112U)), ((uint8_t)(104U)), ((uint8_t)(97U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(111U)), ((uint8_t)(109U)), ((uint8_t)(46U)), ((uint8_t)(118U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5838df12e9e40af39824d9cc8c800e8f[26] = { ((uint8_t)(116U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(46U)), ((uint8_t)(48U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(46U)), ((uint8_t)(49U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(46U)), ((uint8_t)(50U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_764fe5928522ec17a58e098856e16de7[12] = { ((uint8_t)(98U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(115U)), ((uint8_t)(46U)), ((uint8_t)(117U)), ((uint8_t)(95U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_93a9960e248dcd4051648901a03ec405[9] = { ((uint8_t)(119U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(100U)), ((uint8_t)(115U)), ((uint8_t)(49U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_adc87907558cf5820d3d1ab064665305[11] = { ((uint8_t)(97U)), ((uint8_t)(112U)), ((uint8_t)(111U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(46U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_aecdb2be640840ece06249cd41beca46[9] = { ((uint8_t)(122U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(95U)), ((uint8_t)(111U)), ((uint8_t)(107U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_bb6413893f6e2e5ef37ec9ca467bb8f2[9] = { ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(114U)), ((uint8_t)(91U)), ((uint8_t)(50U)), ((uint8_t)(93U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_e389a77d4dec9f0e33cd2edd4566998b[11] = { ((uint8_t)(98U)), ((uint8_t)(121U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(115U)), ((uint8_t)(91U)), ((uint8_t)(49U)), ((uint8_t)(93U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_ff3f2247fdd3c8a5992283438e4446db[23] = { ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(46U)), ((uint8_t)(97U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(46U)), ((uint8_t)(98U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(46U)), ((uint8_t)(99U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const struct input_PointC input_APOINT = (struct input_PointC){ .a = ((uint8_t)(1U)), .b = ((uint64_t)(2ULL)), .c = ((uint16_t)(3U)) };
const uint8_t input_BYTES[4] = { ((uint8_t)(10U)), ((uint8_t)(20U)), ((uint8_t)(30U)), ((uint8_t)(40U)) };
