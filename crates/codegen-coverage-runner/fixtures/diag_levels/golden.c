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
struct tuple_cbf29ce484222325;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
void input_regseq_main(void);
extern const uint8_t input__regseq_synth__const_memory_static_074bba1f653bfa0340ac9d7985949c2e[4];
extern const uint8_t input__regseq_synth__const_memory_static_09bd11d6b75a531bf92641c171621cbf[4];
extern const uint8_t input__regseq_synth__const_memory_static_8b92197a375aa8a5921ac6c2b08bb727[4];
extern const uint8_t input__regseq_synth__const_memory_static_de2787963c7555ab5a9f0eff44d56d41[4];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
void input_regseq_main(void) {
    uint32_t _1;
    uint32_t _2;
    _Bool _3;
    uint32_t _4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _14;
    uint32_t _21;
    uint32_t _22;
    uint32_t _29;
    uint32_t _30;
    bb_0: {
        _1 = ((((uint32_t)(2U))) + (((uint32_t)(3U))));
        _2 = _1;
        _3 = ((_2) > (((uint32_t)(4U))));
        switch (_3) { case 0ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        _6 = _1;
        _5 = ((_6) + (((uint32_t)(1U))));
        goto bb_3;
    }
    bb_2: {
        _4 = _1;
        _5 = ((_4) + (((uint32_t)(100U))));
        goto bb_3;
    }
    bb_3: {
        _7 = _1;
        goto bb_4;
    }
    bb_4: {
        fwrite("[DEBUG] d=", 1, 10, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(_7)); fwrite(_n0, 1, _l0, stdout); } fwrite("\n", 1, 1, stdout); goto bb_5;
    }
    bb_5: {
        _14 = _5;
        goto bb_6;
    }
    bb_6: {
        fwrite("[INFO] i=", 1, 9, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(_14)); fwrite(_n0, 1, _l0, stdout); } fwrite("\n", 1, 1, stdout); goto bb_7;
    }
    bb_7: {
        _21 = _1;
        _22 = ((_21) + (((uint32_t)(2U))));
        goto bb_8;
    }
    bb_8: {
        fwrite("[WARN] w=", 1, 9, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(_22)); fwrite(_n0, 1, _l0, stdout); } fwrite("\n", 1, 1, stdout); goto bb_9;
    }
    bb_9: {
        _29 = _1;
        _30 = ((_29) + (((uint32_t)(3U))));
        goto bb_10;
    }
    bb_10: {
        fwrite("[ERROR] e=", 1, 10, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(_30)); fwrite(_n0, 1, _l0, stdout); } fwrite("\n", 1, 1, stdout); goto bb_11;
    }
    bb_11: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_074bba1f653bfa0340ac9d7985949c2e[4] = { ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_09bd11d6b75a531bf92641c171621cbf[4] = { ((uint8_t)(105U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_8b92197a375aa8a5921ac6c2b08bb727[4] = { ((uint8_t)(101U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_de2787963c7555ab5a9f0eff44d56d41[4] = { ((uint8_t)(119U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
