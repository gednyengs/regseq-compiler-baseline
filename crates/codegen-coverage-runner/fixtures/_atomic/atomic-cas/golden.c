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

#define REGSEQ_ATOMIC_CELLS 1u

/* ---------------------------------------------------------------------------
 * Atomic-memory-access-and-ordering host oracle (atomicity construct, C05).
 * The atomic state lives in this cell table (the regseq_mmio_* model); each
 * hook applies value semantics to its cell and prints an "ATM <op>" line --
 * that trace IS the behavioral oracle (value + ordering tag + bit-test prior +
 * CAS branch). NOT weak-memory soundness (I06). REGSEQ_ATOMIC_CELLS is
 * #define'd just above, sized to the TU's Atomic-field count. */
static uint64_t regseq_atomic_cells[REGSEQ_ATOMIC_CELLS] __attribute__((unused));

/* Low `width` bits set (width in {32,64}). */
__attribute__((unused)) static uint64_t regseq_atomic_mask(uint32_t width) {
    return width >= 64u ? ~(uint64_t)0 : (((uint64_t)1 << width) - 1u);
}
/* The memory-ordering enum tag -> its name (C05 ABI: Relaxed=0 .. SeqCst=4),
 * printed so ordering-tag preservation (C01) is observable in the trace. */
__attribute__((unused)) static const char *regseq_atomic_order(int o) {
    switch (o) {
    case 0: return "Relaxed";
    case 1: return "Acquire";
    case 2: return "Release";
    case 3: return "AcqRel";
    case 4: return "SeqCst";
    default: return "?";
    }
}
/* The fetch-modify sub-op tag -> its name (FetchOp ABI: Add=0 .. Swap=5). */
__attribute__((unused)) static const char *regseq_atomic_fetch_op(int op) {
    switch (op) {
    case 0: return "add";
    case 1: return "sub";
    case 2: return "or";
    case 3: return "and";
    case 4: return "xor";
    case 5: return "swap";
    default: return "?";
    }
}

__attribute__((unused)) static uint64_t regseq_atomic_load(uint32_t cell, uint32_t width, int order) {
    uint64_t v = regseq_atomic_cells[cell] & regseq_atomic_mask(width);
    printf("ATM load cell=%u w=%u ord=%s => 0x%llx\n",
           (unsigned)cell, (unsigned)width, regseq_atomic_order(order),
           (unsigned long long)v);
    return v;
}
__attribute__((unused)) static void regseq_atomic_store(uint32_t cell, uint32_t width, uint64_t val, int order) {
    uint64_t v = val & regseq_atomic_mask(width);
    regseq_atomic_cells[cell] = v;
    printf("ATM store cell=%u w=%u ord=%s val=0x%llx\n",
           (unsigned)cell, (unsigned)width, regseq_atomic_order(order),
           (unsigned long long)v);
}
__attribute__((unused)) static uint64_t regseq_atomic_rmw(uint32_t cell, uint32_t width, int op, uint64_t val, int order) {
    uint64_t mask = regseq_atomic_mask(width);
    uint64_t prior = regseq_atomic_cells[cell] & mask;
    uint64_t arg = val & mask;
    uint64_t next;
    switch (op) {
    case 0: next = prior + arg; break;   /* add (two's-complement wrap via mask) */
    case 1: next = prior - arg; break;   /* sub */
    case 2: next = prior | arg; break;   /* or  */
    case 3: next = prior & arg; break;   /* and */
    case 4: next = prior ^ arg; break;   /* xor */
    case 5: next = arg;         break;   /* swap */
    default: next = prior;      break;
    }
    regseq_atomic_cells[cell] = next & mask;
    printf("ATM %s cell=%u w=%u ord=%s val=0x%llx prior=0x%llx\n",
           regseq_atomic_fetch_op(op), (unsigned)cell, (unsigned)width,
           regseq_atomic_order(order), (unsigned long long)arg,
           (unsigned long long)prior);
    return prior;
}
__attribute__((unused)) static int regseq_atomic_cmpxchg(uint32_t cell, uint32_t width, uint64_t expected, uint64_t desired, uint64_t *observed, int succ, int fail) {
    uint64_t mask = regseq_atomic_mask(width);
    uint64_t cur = regseq_atomic_cells[cell] & mask;
    int committed = (cur == (expected & mask));
    if (committed) {
        regseq_atomic_cells[cell] = desired & mask;
    }
    *observed = cur;
    printf("ATM cmpxchg cell=%u w=%u succ=%s fail=%s expected=0x%llx desired=0x%llx observed=0x%llx outcome=%s\n",
           (unsigned)cell, (unsigned)width, regseq_atomic_order(succ),
           regseq_atomic_order(fail), (unsigned long long)(expected & mask),
           (unsigned long long)(desired & mask), (unsigned long long)cur,
           committed ? "Ok" : "Err");
    return committed;
}
__attribute__((unused)) static int regseq_atomic_bittest(uint32_t cell, uint32_t width, int set_or_clear, uint32_t bit, int order) {
    if (bit >= width) {
        regseq_panic("atomic bit index out of range");
    }
    uint64_t b = (uint64_t)1 << bit;
    int prior = (regseq_atomic_cells[cell] & b) ? 1 : 0;
    if (set_or_clear == 0) {
        regseq_atomic_cells[cell] |= b;
    } else {
        regseq_atomic_cells[cell] &= ~b;
    }
    printf("ATM bit_%s cell=%u w=%u ord=%s bit=%u prior=%d\n",
           set_or_clear == 0 ? "set" : "clear", (unsigned)cell, (unsigned)width,
           regseq_atomic_order(order), (unsigned)bit, prior);
    return prior;
}
__attribute__((unused)) static void regseq_atomic_fence(int order) {
    printf("ATM fence ord=%s\n", regseq_atomic_order(order));
}
/* atomicity construct: seed the Atomic cells from the #[rseq::atomic(initial = N)] literals (R10/S10). */
__attribute__((constructor)) static void regseq_atomic_init(void) {
    regseq_atomic_cells[0] = 0xaull;
}

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_Ordering;
struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc;
struct tuple_cbf29ce484222325;
struct input_S;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c {
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c align");
_Static_assert(offsetof(struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c, value) == 0, "input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c.value offset");
struct input_Ordering {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Ordering) == 1, "input_Ordering size");
_Static_assert(_Alignof(struct input_Ordering) == 1, "input_Ordering align");
_Static_assert(offsetof(struct input_Ordering, tag) == 0, "input_Ordering.tag offset");
_Static_assert(offsetof(struct input_Ordering, _raw) == 0, "input_Ordering._raw offset");
struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc {
    union {
    struct {
    uint32_t tag;
    unsigned char _pad0[4];
    } _tag;
    struct {
    unsigned char _pad0[4];
    uint32_t _0;
    } _v0;
    struct {
    unsigned char _pad0[4];
    uint32_t _0;
    } _v1;
    struct {
    uint32_t tag;
    uint32_t _0;
    } _c0;
    struct {
    uint32_t tag;
    uint32_t _0;
    } _c1;
    unsigned char _raw[8];
    } u;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc) == 8, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc size");
_Static_assert(_Alignof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc) == 4, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc align");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._tag.tag) == 0, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._tag.tag offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._v0._0) == 4, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._v0._0 offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._v1._0) == 4, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._v1._0 offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._c0.tag) == 0, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._c0.tag offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._c0._0) == 4, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._c0._0 offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._c1.tag) == 0, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._c1.tag offset");
_Static_assert(offsetof(struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc, u._c1._0) == 4, "core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc.u._c1._0 offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct input_S {
    struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c cell;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_S) == 4, "input_S size");
_Static_assert(_Alignof(struct input_S) == 4, "input_S align");
_Static_assert(offsetof(struct input_S, cell) == 0, "input_S.cell offset");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc input__impl_0__compare_exchange__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *, uint32_t, uint32_t, struct input_Ordering, struct input_Ordering);
__attribute__((noinline)) uint32_t input__impl_0__fetch_add__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *, uint32_t, struct input_Ordering);
struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_0__new__5ae2a5e07d9ec8b0eb418e8c3168f23c(uint32_t);
__attribute__((noinline)) void input__impl_0__store__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *, uint32_t, struct input_Ordering);
inline struct input_Ordering input__impl_2__clone(const struct input_Ordering *);
void input_regseq_main(void);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc input__impl_0__compare_exchange__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_1, uint32_t _2, uint32_t _3, struct input_Ordering _4, struct input_Ordering _5) {
    struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc _0;
    uint32_t _6;
    bb_0: {
        _6 = (*_1).value;
        _0.u._tag.tag = ((uint32_t)(0U)); _0.u._v0._0 = _6;
        return _0;
    }
}
__attribute__((noinline)) uint32_t input__impl_0__fetch_add__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_1, uint32_t _2, struct input_Ordering _3) {
    uint32_t _0;
    bb_0: {
        _0 = (*_1).value;
        return _0;
    }
}
struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_0__new__5ae2a5e07d9ec8b0eb418e8c3168f23c(uint32_t _1) {
    struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c _0;
    uint32_t _2;
    bb_0: {
        _2 = _1;
        _0 = (struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c){ .value = _2 };
        return _0;
    }
}
__attribute__((noinline)) void input__impl_0__store__5ae2a5e07d9ec8b0eb418e8c3168f23c(const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_1, uint32_t _2, struct input_Ordering _3) {
    bb_0: {
        return;
    }
}
inline struct input_Ordering input__impl_2__clone(const struct input_Ordering *_1) {
    struct input_Ordering _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
void input_regseq_main(void) {
    struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c _1;
    struct input_S _2;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_3;
    struct input_Ordering _4;
    struct input_Ordering _5;
    struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc _6;
    intptr_t _7;
    uint32_t _8;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_9;
    uint32_t _10;
    struct input_Ordering _11;
    uint32_t _13;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_14;
    uint32_t _15;
    struct input_Ordering _16;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_18;
    struct input_Ordering _19;
    struct input_Ordering _20;
    struct core_result_Result__c218b0cbd5c9693f9c20e30b139bcedc _21;
    intptr_t _22;
    uint32_t _23;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_24;
    uint32_t _25;
    struct input_Ordering _26;
    uint32_t _28;
    const struct input_Atomic__5ae2a5e07d9ec8b0eb418e8c3168f23c *_29;
    uint32_t _30;
    struct input_Ordering _31;
    uint32_t _32;
    bb_0: {
        _1 = input__impl_0__new__5ae2a5e07d9ec8b0eb418e8c3168f23c(((uint32_t)(10U))); goto bb_1;
    }
    bb_1: {
        _2 = (struct input_S){ .cell = _1 };
        _3 = &(_2.cell);
        _4.tag = ((uint8_t)(3U));
        _5.tag = ((uint8_t)(1U));
        { uint64_t _cas_obs; int _cas_ok = regseq_atomic_cmpxchg(0, 32, (uint64_t)(((uint32_t)(10U))), (uint64_t)(((uint32_t)(20U))), &_cas_obs, 3, 1); if (_cas_ok) { _6.u._tag.tag = ((uint32_t)(0U)); _6.u._v0._0 = (uint32_t)_cas_obs; } else { _6.u._tag.tag = ((uint32_t)(1U)); _6.u._v1._0 = (uint32_t)_cas_obs; } } goto bb_2;
    }
    bb_2: {
        _7 = (_6.u._tag.tag == ((uint32_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_7) { case 0ULL: goto bb_5; case 1ULL: goto bb_3; default: goto bb_9; }
    }
    bb_3: {
        _13 = _6.u._v1._0;
        _14 = &(_2.cell);
        _15 = _13;
        _16.tag = ((uint8_t)(0U));
        regseq_atomic_store(0, 32, (uint64_t)(_15), 0); goto bb_4;
    }
    bb_4: {
        goto bb_7;
    }
    bb_5: {
        _8 = _6.u._v0._0;
        _9 = &(_2.cell);
        _10 = _8;
        _11.tag = ((uint8_t)(2U));
        regseq_atomic_store(0, 32, (uint64_t)(_10), 2); goto bb_6;
    }
    bb_6: {
        goto bb_7;
    }
    bb_7: {
        _18 = &(_2.cell);
        _19.tag = ((uint8_t)(4U));
        _20.tag = ((uint8_t)(0U));
        { uint64_t _cas_obs; int _cas_ok = regseq_atomic_cmpxchg(0, 32, (uint64_t)(((uint32_t)(99U))), (uint64_t)(((uint32_t)(30U))), &_cas_obs, 4, 0); if (_cas_ok) { _21.u._tag.tag = ((uint32_t)(0U)); _21.u._v0._0 = (uint32_t)_cas_obs; } else { _21.u._tag.tag = ((uint32_t)(1U)); _21.u._v1._0 = (uint32_t)_cas_obs; } } goto bb_8;
    }
    bb_8: {
        _22 = (_21.u._tag.tag == ((uint32_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_22) { case 0ULL: goto bb_12; case 1ULL: goto bb_10; default: goto bb_9; }
    }
    bb_9: {
        regseq_unreachable();
    }
    bb_10: {
        _28 = _21.u._v1._0;
        _29 = &(_2.cell);
        _30 = _28;
        _31.tag = ((uint8_t)(0U));
        _32 = (uint32_t)regseq_atomic_rmw(0, 32, 0, (uint64_t)(_30), 0); goto bb_11;
    }
    bb_11: {
        goto bb_14;
    }
    bb_12: {
        _23 = _21.u._v0._0;
        _24 = &(_2.cell);
        _25 = _23;
        _26.tag = ((uint8_t)(2U));
        regseq_atomic_store(0, 32, (uint64_t)(_25), 2); goto bb_13;
    }
    bb_13: {
        goto bb_14;
    }
    bb_14: {
        return;
    }
}
