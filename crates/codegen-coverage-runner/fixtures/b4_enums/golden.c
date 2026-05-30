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
struct input_Color;
struct input_E;
struct tuple_69bd35421fcc2557;
struct input_N;
struct input_One;
struct input_Void;
struct input_VoidRef;

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
struct input_Color {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Color) == 1, "input_Color size");
_Static_assert(_Alignof(struct input_Color) == 1, "input_Color align");
_Static_assert(offsetof(struct input_Color, tag) == 0, "input_Color.tag offset");
_Static_assert(offsetof(struct input_Color, _raw) == 0, "input_Color._raw offset");
struct input_E {
    union {
    struct {
    uint8_t tag;
    unsigned char _pad0[7];
    } _tag;
    struct {
    unsigned char _pad0[4];
    uint32_t _0;
    } _v0;
    struct {
    unsigned char _pad0[1];
    uint8_t _0;
    uint8_t _1;
    unsigned char _pad1[5];
    } _v1;
    struct {
    uint8_t tag;
    unsigned char _pad0[3];
    uint32_t _0;
    } _c0;
    struct {
    uint8_t tag;
    uint8_t _0;
    uint8_t _1;
    unsigned char _pad0[5];
    } _c1;
    unsigned char _raw[8];
    } u;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_E) == 8, "input_E size");
_Static_assert(_Alignof(struct input_E) == 4, "input_E align");
_Static_assert(offsetof(struct input_E, u._tag.tag) == 0, "input_E.u._tag.tag offset");
_Static_assert(offsetof(struct input_E, u._v0._0) == 4, "input_E.u._v0._0 offset");
_Static_assert(offsetof(struct input_E, u._v1._0) == 1, "input_E.u._v1._0 offset");
_Static_assert(offsetof(struct input_E, u._v1._1) == 2, "input_E.u._v1._1 offset");
_Static_assert(offsetof(struct input_E, u._c0.tag) == 0, "input_E.u._c0.tag offset");
_Static_assert(offsetof(struct input_E, u._c0._0) == 4, "input_E.u._c0._0 offset");
_Static_assert(offsetof(struct input_E, u._c1.tag) == 0, "input_E.u._c1.tag offset");
_Static_assert(offsetof(struct input_E, u._c1._0) == 1, "input_E.u._c1._0 offset");
_Static_assert(offsetof(struct input_E, u._c1._1) == 2, "input_E.u._c1._1 offset");
struct tuple_69bd35421fcc2557 {
    uint8_t _0;
    unsigned char _pad0[3];
    uint32_t _1;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct tuple_69bd35421fcc2557) == 8, "tuple_69bd35421fcc2557 size");
_Static_assert(_Alignof(struct tuple_69bd35421fcc2557) == 4, "tuple_69bd35421fcc2557 align");
_Static_assert(offsetof(struct tuple_69bd35421fcc2557, _0) == 0, "tuple_69bd35421fcc2557._0 offset");
_Static_assert(offsetof(struct tuple_69bd35421fcc2557, _1) == 4, "tuple_69bd35421fcc2557._1 offset");
struct input_N {
    union {
    struct {
    const uint32_t *_0;
    };
    unsigned char _raw[8];
    };
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_N) == 8, "input_N size");
_Static_assert(_Alignof(struct input_N) == 8, "input_N align");
_Static_assert(offsetof(struct input_N, _0) == 0, "input_N._0 offset");
_Static_assert(offsetof(struct input_N, _raw) == 0, "input_N._raw offset");
struct input_One {
    union {
    struct {
    uint16_t _0;
    };
    unsigned char _raw[2];
    };
} __attribute__((aligned(2)));
_Static_assert(sizeof(struct input_One) == 2, "input_One size");
_Static_assert(_Alignof(struct input_One) == 2, "input_One align");
_Static_assert(offsetof(struct input_One, _0) == 0, "input_One._0 offset");
_Static_assert(offsetof(struct input_One, _raw) == 0, "input_One._raw offset");
struct input_Void {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Void) == 0, "input_Void size");
_Static_assert(_Alignof(struct input_Void) == 1, "input_Void align");
struct input_VoidRef {
    const struct input_Void *p;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_VoidRef) == 8, "input_VoidRef size");
_Static_assert(_Alignof(struct input_VoidRef) == 8, "input_VoidRef align");
_Static_assert(offsetof(struct input_VoidRef, p) == 0, "input_VoidRef.p offset");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
uint8_t input_color_tag(const struct input_Color *);
struct tuple_69bd35421fcc2557 input_e_tag_and_payload(const struct input_E *);
struct tuple_69bd35421fcc2557 input_n_probe(const struct input_N *);
uint16_t input_one_payload(const struct input_One *);
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_29c8998c295344767bde5abeb21497ee[16];
extern const uint8_t input__regseq_synth__const_memory_static_31cdc13149637fc6e1442dae5cd1bbd2[6];
extern const uint8_t input__regseq_synth__const_memory_static_351f9d627e77d1cd85929879db6e350b[9];
extern const uint8_t input__regseq_synth__const_memory_static_57ab7efd8c9316e607f6b27c0f9f0658[18];
extern const uint8_t input__regseq_synth__const_memory_static_5c4f930a9ecdbb7afe322e0754d6577f[14];
extern const uint8_t input__regseq_synth__const_memory_static_5cd50c02839f40b434a28ce51eb82812[18];
extern const uint8_t input__regseq_synth__const_memory_static_5e65c8208417d4e1910dae720a9068c1[12];
extern const struct input_E input__regseq_synth__const_memory_static_970ade8ab110127516e62e8a5f06f32a;
extern const uint8_t input__regseq_synth__const_memory_static_a066035614e4af71fdaf2dd00286f518[18];
extern const uint8_t input__regseq_synth__const_memory_static_d73062b0771292d432436d2e895f740a[18];
extern const uint8_t input__regseq_synth__const_memory_static_e24357c26a053ae70c5c085e1d3ba04b[18];
extern const uint32_t input_GUARD;

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
uint8_t input_color_tag(const struct input_Color *_1) {
    uint8_t _0;
    intptr_t _2;
    bb_0: {
        _2 = ((*_1).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_1).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        switch (_2) { case 0ULL: goto bb_4; case 1ULL: goto bb_3; case 2ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        regseq_unreachable();
    }
    bb_2: {
        _0 = ((uint8_t)(2U));
        goto bb_5;
    }
    bb_3: {
        _0 = ((uint8_t)(1U));
        goto bb_5;
    }
    bb_4: {
        _0 = ((uint8_t)(0U));
        goto bb_5;
    }
    bb_5: {
        return _0;
    }
}
struct tuple_69bd35421fcc2557 input_e_tag_and_payload(const struct input_E *_1) {
    struct tuple_69bd35421fcc2557 _0;
    intptr_t _2;
    const uint32_t *_3;
    uint32_t _4;
    const uint8_t *_5;
    const uint8_t *_6;
    uint8_t _7;
    uint32_t _8;
    uint32_t _9;
    uint8_t _10;
    uint32_t _11;
    uint32_t _12;
    bb_0: {
        _2 = ((*_1).u._tag.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_1).u._tag.tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        switch (_2) { case 0ULL: goto bb_4; case 1ULL: goto bb_3; case 2ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        regseq_unreachable();
    }
    bb_2: {
        _0 = (struct tuple_69bd35421fcc2557){ ._0 = ((uint8_t)(2U)), ._1 = ((uint32_t)(0U)) };
        goto bb_5;
    }
    bb_3: {
        _5 = &((*_1).u._v1._0);
        _6 = &((*_1).u._v1._1);
        _7 = (*_5);
        _8 = ((uint32_t)(_7));
        _9 = ((uint32_t)((uint32_t)(_8) << ((((int32_t)(8U))) & 31)));
        _10 = (*_6);
        _11 = ((uint32_t)(_10));
        _12 = ((_9) | (_11));
        _0 = (struct tuple_69bd35421fcc2557){ ._0 = ((uint8_t)(1U)), ._1 = _12 };
        goto bb_5;
    }
    bb_4: {
        _3 = &((*_1).u._v0._0);
        _4 = (*_3);
        _0 = (struct tuple_69bd35421fcc2557){ ._0 = ((uint8_t)(0U)), ._1 = _4 };
        goto bb_5;
    }
    bb_5: {
        return _0;
    }
}
struct tuple_69bd35421fcc2557 input_n_probe(const struct input_N *_1) {
    struct tuple_69bd35421fcc2557 _0;
    intptr_t _2;
    const uint32_t *const *_3;
    const uint32_t *_4;
    uint32_t _5;
    bb_0: {
        { uint64_t _nd; memcpy(&_nd, (const unsigned char *)&((*_1)) + 0, sizeof(_nd)); _2 = (_nd == ((uint64_t)(0ULL)) ? ((intptr_t)(1ULL)) : ((intptr_t)(0ULL))); }
        switch (_2) { case 0ULL: goto bb_3; case 1ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        regseq_unreachable();
    }
    bb_2: {
        _0 = (struct tuple_69bd35421fcc2557){ ._0 = ((uint8_t)(0U)), ._1 = ((uint32_t)(0U)) };
        goto bb_4;
    }
    bb_3: {
        _3 = &((*_1)._0);
        _4 = (*_3);
        _5 = (*_4);
        _0 = (struct tuple_69bd35421fcc2557){ ._0 = ((uint8_t)(1U)), ._1 = _5 };
        goto bb_4;
    }
    bb_4: {
        return _0;
    }
}
uint16_t input_one_payload(const struct input_One *_1) {
    uint16_t _0;
    const uint16_t *_2;
    bb_0: {
        _2 = &((*_1)._0);
        _0 = (*_2);
        return _0;
    }
}
void input_regseq_main(void) {
    struct input_One _1;
    const struct input_One *_2;
    const struct input_One *_3;
    uint16_t _4;
    struct input_Color _11;
    struct input_Color _12;
    struct input_Color _13;
    const struct input_Color *_14;
    const struct input_Color *_15;
    uint8_t _16;
    const struct input_Color *_18;
    const struct input_Color *_19;
    uint8_t _20;
    const struct input_Color *_22;
    const struct input_Color *_23;
    uint8_t _24;
    struct input_E _31;
    struct input_E _32;
    struct input_E _33;
    const struct input_E *_34;
    const struct input_E *_35;
    struct tuple_69bd35421fcc2557 _36;
    uint8_t _37;
    uint32_t _38;
    const struct input_E *_39;
    const struct input_E *_40;
    struct tuple_69bd35421fcc2557 _41;
    uint8_t _42;
    uint32_t _43;
    const struct input_E *_44;
    const struct input_E *_45;
    struct tuple_69bd35421fcc2557 _46;
    uint8_t _47;
    uint32_t _48;
    uint8_t _49;
    uint32_t _51;
    uint8_t _58;
    uint32_t _60;
    uint8_t _67;
    uint32_t _69;
    struct input_E _76;
    uint32_t _77;
    _Bool _78;
    struct input_E _79;
    const struct input_E *_80;
    const struct input_E *_81;
    struct tuple_69bd35421fcc2557 _82;
    uint8_t _83;
    uint32_t _84;
    uint8_t _85;
    const struct input_E *_92;
    const struct input_E *_93;
    const struct input_E *_94;
    struct tuple_69bd35421fcc2557 _95;
    uint8_t _96;
    uint32_t _97;
    uint8_t _98;
    uint32_t _100;
    const uint32_t *_107;
    const uint32_t *_108;
    const uint32_t *_109;
    struct input_N _110;
    struct input_N _111;
    const struct input_N *_112;
    const struct input_N *_113;
    struct tuple_69bd35421fcc2557 _114;
    uint8_t _115;
    uint32_t _116;
    const struct input_N *_117;
    const struct input_N *_118;
    struct tuple_69bd35421fcc2557 _119;
    uint8_t _120;
    uint32_t _121;
    uint8_t _122;
    uint32_t _124;
    uint8_t _131;
    uint32_t _133;
    const struct input_Void *_140;
    struct input_VoidRef _141;
    const struct input_Void *_142;
    size_t _143;
    _Bool _144;
    uint8_t _145;
    bb_0: {
        _1._0 = ((uint16_t)(4242U));
        _2 = &(_1);
        _3 = &((*_2));
        _4 = input_one_payload(_3); goto bb_1;
    }
    bb_1: {
        goto bb_2;
    }
    bb_2: {
        fwrite("[INFO] one=", 1, 11, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_4)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_3;
    }
    bb_3: {
        _11.tag = ((uint8_t)(0U));
        _12.tag = ((uint8_t)(1U));
        _13.tag = ((uint8_t)(2U));
        _14 = &(_11);
        _15 = &((*_14));
        _16 = input_color_tag(_15); goto bb_4;
    }
    bb_4: {
        goto bb_5;
    }
    bb_5: {
        _18 = &(_12);
        _19 = &((*_18));
        _20 = input_color_tag(_19); goto bb_6;
    }
    bb_6: {
        goto bb_7;
    }
    bb_7: {
        _22 = &(_13);
        _23 = &((*_22));
        _24 = input_color_tag(_23); goto bb_8;
    }
    bb_8: {
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] color=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_16)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_20)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_24)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        _31.u._tag.tag = ((uint8_t)(0U)); _31.u._v0._0 = ((uint32_t)(57005U));
        _32.u._tag.tag = ((uint8_t)(1U)); _32.u._v1._0 = ((uint8_t)(18U)); _32.u._v1._1 = ((uint8_t)(52U));
        _33.u._tag.tag = ((uint8_t)(2U));
        _34 = &(_31);
        _35 = &((*_34));
        _36 = input_e_tag_and_payload(_35); goto bb_11;
    }
    bb_11: {
        _37 = _36._0;
        _38 = _36._1;
        _39 = &(_32);
        _40 = &((*_39));
        _41 = input_e_tag_and_payload(_40); goto bb_12;
    }
    bb_12: {
        _42 = _41._0;
        _43 = _41._1;
        _44 = &(_33);
        _45 = &((*_44));
        _46 = input_e_tag_and_payload(_45); goto bb_13;
    }
    bb_13: {
        _47 = _46._0;
        _48 = _46._1;
        _49 = _37;
        goto bb_14;
    }
    bb_14: {
        _51 = _38;
        goto bb_15;
    }
    bb_15: {
        fwrite("[INFO] ea_tag=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_49)); fwrite(_n, 1, _l, stdout); } fwrite(" ea_pl=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_51)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_16;
    }
    bb_16: {
        _58 = _42;
        goto bb_17;
    }
    bb_17: {
        _60 = _43;
        goto bb_18;
    }
    bb_18: {
        fwrite("[INFO] eb_tag=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_58)); fwrite(_n, 1, _l, stdout); } fwrite(" eb_pl=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_60)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_19;
    }
    bb_19: {
        _67 = _47;
        goto bb_20;
    }
    bb_20: {
        _69 = _48;
        goto bb_21;
    }
    bb_21: {
        fwrite("[INFO] ec_tag=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_67)); fwrite(_n, 1, _l, stdout); } fwrite(" ec_pl=", 1, 7, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_69)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_22;
    }
    bb_22: {
        _76.u._tag.tag = ((uint8_t)(0U)); _76.u._v0._0 = ((uint32_t)(1U));
        _77 = _38;
        _78 = ((_77) == (((uint32_t)(57005U))));
        switch (_78) { case 0ULL: goto bb_24; default: goto bb_23; }
    }
    bb_23: {
        _79.u._tag.tag = ((uint8_t)(2U));
        _76 = _79;
        goto bb_25;
    }
    bb_24: {
        goto bb_25;
    }
    bb_25: {
        _80 = &(_76);
        _81 = &((*_80));
        _82 = input_e_tag_and_payload(_81); goto bb_26;
    }
    bb_26: {
        _83 = _82._0;
        _84 = _82._1;
        _85 = _83;
        goto bb_27;
    }
    bb_27: {
        fwrite("[INFO] em_tag=", 1, 14, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_85)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_28;
    }
    bb_28: {
        _92 = ((const struct input_E *)&input__regseq_synth__const_memory_static_970ade8ab110127516e62e8a5f06f32a);
        _93 = &((*_92));
        _94 = &((*_93));
        _95 = input_e_tag_and_payload(_94); goto bb_29;
    }
    bb_29: {
        _96 = _95._0;
        _97 = _95._1;
        _98 = _96;
        goto bb_30;
    }
    bb_30: {
        _100 = _97;
        goto bb_31;
    }
    bb_31: {
        fwrite("[INFO] k_tag=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_98)); fwrite(_n, 1, _l, stdout); } fwrite(" k_pl=", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_100)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_32;
    }
    bb_32: {
        _107 = ((const uint32_t *)&input_GUARD);
        _108 = &((*_107));
        _109 = &((*_108));
        _110._0 = _109;
        memcpy((unsigned char *)&(_111) + 0, &(uint64_t){((uint64_t)(0ULL))}, sizeof(uint64_t));
        _112 = &(_110);
        _113 = &((*_112));
        _114 = input_n_probe(_113); goto bb_33;
    }
    bb_33: {
        _115 = _114._0;
        _116 = _114._1;
        _117 = &(_111);
        _118 = &((*_117));
        _119 = input_n_probe(_118); goto bb_34;
    }
    bb_34: {
        _120 = _119._0;
        _121 = _119._1;
        _122 = _115;
        goto bb_35;
    }
    bb_35: {
        _124 = _116;
        goto bb_36;
    }
    bb_36: {
        fwrite("[INFO] ns_is=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_122)); fwrite(_n, 1, _l, stdout); } fwrite(" ns_val=", 1, 8, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_124)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_37;
    }
    bb_37: {
        _131 = _120;
        goto bb_38;
    }
    bb_38: {
        _133 = _121;
        goto bb_39;
    }
    bb_39: {
        fwrite("[INFO] nn_is=", 1, 13, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_131)); fwrite(_n, 1, _l, stdout); } fwrite(" nn_val=", 1, 8, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_133)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_40;
    }
    bb_40: {
        _140 = ((const struct input_Void *)(((size_t)(0ULL))));
        _141 = (struct input_VoidRef){ .p = _140 };
        _142 = _141.p;
        _143 = ((uintptr_t)(_142));
        _144 = ((_143) == (((size_t)(0ULL))));
        _145 = ((uint8_t)(_144));
        goto bb_41;
    }
    bb_41: {
        fwrite("[INFO] void_null=", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_145)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_42;
    }
    bb_42: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_29c8998c295344767bde5abeb21497ee[16] = { ((uint8_t)(107U)), ((uint8_t)(95U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(107U)), ((uint8_t)(95U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_31cdc13149637fc6e1442dae5cd1bbd2[6] = { ((uint8_t)(111U)), ((uint8_t)(110U)), ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_351f9d627e77d1cd85929879db6e350b[9] = { ((uint8_t)(101U)), ((uint8_t)(109U)), ((uint8_t)(95U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_57ab7efd8c9316e607f6b27c0f9f0658[18] = { ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(95U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(95U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5c4f930a9ecdbb7afe322e0754d6577f[14] = { ((uint8_t)(99U)), ((uint8_t)(111U)), ((uint8_t)(108U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5cd50c02839f40b434a28ce51eb82812[18] = { ((uint8_t)(110U)), ((uint8_t)(110U)), ((uint8_t)(95U)), ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(110U)), ((uint8_t)(110U)), ((uint8_t)(95U)), ((uint8_t)(118U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_5e65c8208417d4e1910dae720a9068c1[12] = { ((uint8_t)(118U)), ((uint8_t)(111U)), ((uint8_t)(105U)), ((uint8_t)(100U)), ((uint8_t)(95U)), ((uint8_t)(110U)), ((uint8_t)(117U)), ((uint8_t)(108U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const struct input_E input__regseq_synth__const_memory_static_970ade8ab110127516e62e8a5f06f32a = (struct input_E){ .u._raw = { 0, 0, 0, 0, 7, 0, 0, 0 } };
const uint8_t input__regseq_synth__const_memory_static_a066035614e4af71fdaf2dd00286f518[18] = { ((uint8_t)(101U)), ((uint8_t)(99U)), ((uint8_t)(95U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(101U)), ((uint8_t)(99U)), ((uint8_t)(95U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_d73062b0771292d432436d2e895f740a[18] = { ((uint8_t)(101U)), ((uint8_t)(98U)), ((uint8_t)(95U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(103U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(101U)), ((uint8_t)(98U)), ((uint8_t)(95U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_e24357c26a053ae70c5c085e1d3ba04b[18] = { ((uint8_t)(110U)), ((uint8_t)(115U)), ((uint8_t)(95U)), ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(110U)), ((uint8_t)(115U)), ((uint8_t)(95U)), ((uint8_t)(118U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint32_t input_GUARD = ((uint32_t)(43981U));
