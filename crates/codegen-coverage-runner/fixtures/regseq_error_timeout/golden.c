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

/* regseq-error C02 / C03: the RegseqError -> errno boundary adapter, demand-driven. */
#include <errno.h>

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
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct tuple_cbf29ce484222325;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_RegseqError {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_RegseqError) == 1, "input_RegseqError size");
_Static_assert(_Alignof(struct input_RegseqError) == 1, "input_RegseqError align");
_Static_assert(offsetof(struct input_RegseqError, tag) == 0, "input_RegseqError.tag offset");
_Static_assert(offsetof(struct input_RegseqError, _raw) == 0, "input_RegseqError._raw offset");
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 {
    union {
    struct {
    struct input_RegseqError _0;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486) == 1, "core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 size");
_Static_assert(_Alignof(struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486) == 1, "core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 align");
_Static_assert(offsetof(struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486, _0) == 0, "core_result_Result__a96be460e4ad8f92502af3ac2c2f6486._0 offset");
_Static_assert(offsetof(struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486, _raw) == 0, "core_result_Result__a96be460e4ad8f92502af3ac2c2f6486._raw offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input_probe(uint8_t);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input_regseq_main(void);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input_probe(uint8_t _1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    uint8_t _2;
    _Bool _3;
    struct input_RegseqError _4;
    bb_0: {
        _2 = _1;
        _3 = ((_2) == (((uint8_t)(0U))));
        switch (_3) { case 0ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_3;
    }
    bb_2: {
        _4.tag = ((uint8_t)(0U));
        _0._0 = _4;
        goto bb_3;
    }
    bb_3: {
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input_regseq_main(void) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _1;
    intptr_t _3;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _4;
    intptr_t _5;
    intptr_t _6;
    struct input_RegseqError _7;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _8;
    bb_0: {
        _1 = input_probe(((uint8_t)(0U))); goto bb_1;
    }
    bb_1: {
        goto bb_2;
    }
    bb_2: {
        { uint8_t _nd; memcpy(&_nd, (const unsigned char *)&(_1) + 0, sizeof(_nd)); _3 = (_nd == ((uint8_t)(2U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL))); }
        switch (_3) { case 0ULL: goto bb_6; case 1ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        regseq_unreachable();
    }
    bb_4: {
        _0._0 = _1._0;
        goto bb_5;
    }
    bb_5: {
        goto bb_12;
    }
    bb_6: {
        _4 = input_probe(((uint8_t)(1U))); goto bb_7;
    }
    bb_7: {
        { uint8_t _nd; memcpy(&_nd, (const unsigned char *)&(_4) + 0, sizeof(_nd)); _5 = (_nd == ((uint8_t)(2U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL))); }
        switch (_5) { case 1ULL: goto bb_8; default: goto bb_9; }
    }
    bb_8: {
        _6 = (_4._0.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_6) { case 0ULL: goto bb_10; default: goto bb_9; }
    }
    bb_9: {
        _8 = _4;
        _0 = _8;
        goto bb_11;
    }
    bb_10: {
        _7.tag = ((uint8_t)(1U));
        _0._0 = _7;
        goto bb_11;
    }
    bb_11: {
        goto bb_12;
    }
    bb_12: {
        return _0;
    }
}

/* ---------------------------------------------------------------------------
 * RegseqError -> errno boundary adapter (regseq-error C02 / C03).
 * --------------------------------------------------------------------------- */
int input_regseq_main__regseq_exit(void) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 __r = input_regseq_main();
    uint8_t __disc;
    { uint8_t __nd; memcpy(&__nd, (const unsigned char *)&(__r) + 0, sizeof(__nd)); __disc = (__nd == ((uint8_t)(2U)) ? ((uint8_t)(0U)) : ((uint8_t)(1U))); }
    if (__disc == ((uint8_t)(0U))) {
        return 0;
    }
    switch (__r._0.tag) {
    case ((uint8_t)(0U)): return EBUSY;
    case ((uint8_t)(1U)): return ETIMEDOUT;
    default: return EIO;
    }
}
