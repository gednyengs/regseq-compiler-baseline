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
 * Device-lifecycle trace hooks (device-lifecycle construct). Each prints an
 * "LC <event>" line to stdout -- this trace IS the behavioral oracle for
 * lifecycle lowering. Emitted only for a TU with lifecycle drivers. */
__attribute__((unused)) static void regseq_lc_register(const char *name, const char *match_set) {
    printf("LC register %s match=%s\n", name, match_set);
}
__attribute__((unused)) static void regseq_lc_create(void) {
    printf("LC create\n");
}
__attribute__((unused)) static void regseq_lc_associate(void) {
    printf("LC associate\n");
}
__attribute__((unused)) static void regseq_lc_call(const char *cb) {
    printf("LC call %s\n", cb);
}
__attribute__((unused)) static void regseq_lc_teardown(int present) {
    printf("LC teardown present=%d\n", present);
}
__attribute__((unused)) static void regseq_lc_on_init_err(void) {
    printf("LC on_init -> Err\n");
}
__attribute__((unused)) static void regseq_lc_preamble(const char *res) {
    printf("LC preamble %s\n", res);
}
__attribute__((unused)) static void regseq_lc_postamble(const char *res, int skipped) {
    if (skipped) {
        printf("LC postamble %s skipped(surprise)\n", res);
    } else {
        printf("LC postamble %s\n", res);
    }
}
/* RSEQ-SERIAL-PORT pl011
 * absent operations are unsupported; no terminal event is implied
 */

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_Pl011;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct input_SerialDataBits;
struct input_SerialStopBits;
struct input_SerialParity;
struct input_SerialFlowControl;
struct input_SerialDirection;
struct input_SerialStatus;
struct tuple_cbf29ce484222325;
struct input_SerialParams;
struct input_ModemSignalSet;
struct input_LineErrorFlags;
struct input_SerialEventMask;
struct input_SerialTxBuffer;
struct input_SerialRxBuffer;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_Pl011 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Pl011) == 0, "input_Pl011 size");
_Static_assert(_Alignof(struct input_Pl011) == 1, "input_Pl011 align");
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
struct input_SerialDataBits {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialDataBits) == 1, "input_SerialDataBits size");
_Static_assert(_Alignof(struct input_SerialDataBits) == 1, "input_SerialDataBits align");
_Static_assert(offsetof(struct input_SerialDataBits, tag) == 0, "input_SerialDataBits.tag offset");
_Static_assert(offsetof(struct input_SerialDataBits, _raw) == 0, "input_SerialDataBits._raw offset");
struct input_SerialStopBits {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialStopBits) == 1, "input_SerialStopBits size");
_Static_assert(_Alignof(struct input_SerialStopBits) == 1, "input_SerialStopBits align");
_Static_assert(offsetof(struct input_SerialStopBits, tag) == 0, "input_SerialStopBits.tag offset");
_Static_assert(offsetof(struct input_SerialStopBits, _raw) == 0, "input_SerialStopBits._raw offset");
struct input_SerialParity {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialParity) == 1, "input_SerialParity size");
_Static_assert(_Alignof(struct input_SerialParity) == 1, "input_SerialParity align");
_Static_assert(offsetof(struct input_SerialParity, tag) == 0, "input_SerialParity.tag offset");
_Static_assert(offsetof(struct input_SerialParity, _raw) == 0, "input_SerialParity._raw offset");
struct input_SerialFlowControl {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialFlowControl) == 1, "input_SerialFlowControl size");
_Static_assert(_Alignof(struct input_SerialFlowControl) == 1, "input_SerialFlowControl align");
_Static_assert(offsetof(struct input_SerialFlowControl, tag) == 0, "input_SerialFlowControl.tag offset");
_Static_assert(offsetof(struct input_SerialFlowControl, _raw) == 0, "input_SerialFlowControl._raw offset");
struct input_SerialDirection {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialDirection) == 1, "input_SerialDirection size");
_Static_assert(_Alignof(struct input_SerialDirection) == 1, "input_SerialDirection align");
_Static_assert(offsetof(struct input_SerialDirection, tag) == 0, "input_SerialDirection.tag offset");
_Static_assert(offsetof(struct input_SerialDirection, _raw) == 0, "input_SerialDirection._raw offset");
struct input_SerialStatus {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialStatus) == 1, "input_SerialStatus size");
_Static_assert(_Alignof(struct input_SerialStatus) == 1, "input_SerialStatus align");
_Static_assert(offsetof(struct input_SerialStatus, tag) == 0, "input_SerialStatus.tag offset");
_Static_assert(offsetof(struct input_SerialStatus, _raw) == 0, "input_SerialStatus._raw offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct input_SerialParams {
    uint32_t baud_rate;
    struct input_SerialDataBits data_bits;
    struct input_SerialParity parity;
    struct input_SerialStopBits stop_bits;
    struct input_SerialFlowControl flow_control_mode;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_SerialParams) == 8, "input_SerialParams size");
_Static_assert(_Alignof(struct input_SerialParams) == 4, "input_SerialParams align");
_Static_assert(offsetof(struct input_SerialParams, baud_rate) == 0, "input_SerialParams.baud_rate offset");
_Static_assert(offsetof(struct input_SerialParams, data_bits) == 4, "input_SerialParams.data_bits offset");
_Static_assert(offsetof(struct input_SerialParams, parity) == 5, "input_SerialParams.parity offset");
_Static_assert(offsetof(struct input_SerialParams, stop_bits) == 6, "input_SerialParams.stop_bits offset");
_Static_assert(offsetof(struct input_SerialParams, flow_control_mode) == 7, "input_SerialParams.flow_control_mode offset");
struct input_ModemSignalSet {
    _Bool dtr;
    _Bool rts;
    _Bool cts;
    _Bool dsr;
    _Bool dcd;
    _Bool ri;
    _Bool changed_cts;
    _Bool changed_dsr;
    _Bool changed_dcd;
    _Bool changed_ri;
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_ModemSignalSet) == 10, "input_ModemSignalSet size");
_Static_assert(_Alignof(struct input_ModemSignalSet) == 1, "input_ModemSignalSet align");
_Static_assert(offsetof(struct input_ModemSignalSet, dtr) == 0, "input_ModemSignalSet.dtr offset");
_Static_assert(offsetof(struct input_ModemSignalSet, rts) == 1, "input_ModemSignalSet.rts offset");
_Static_assert(offsetof(struct input_ModemSignalSet, cts) == 2, "input_ModemSignalSet.cts offset");
_Static_assert(offsetof(struct input_ModemSignalSet, dsr) == 3, "input_ModemSignalSet.dsr offset");
_Static_assert(offsetof(struct input_ModemSignalSet, dcd) == 4, "input_ModemSignalSet.dcd offset");
_Static_assert(offsetof(struct input_ModemSignalSet, ri) == 5, "input_ModemSignalSet.ri offset");
_Static_assert(offsetof(struct input_ModemSignalSet, changed_cts) == 6, "input_ModemSignalSet.changed_cts offset");
_Static_assert(offsetof(struct input_ModemSignalSet, changed_dsr) == 7, "input_ModemSignalSet.changed_dsr offset");
_Static_assert(offsetof(struct input_ModemSignalSet, changed_dcd) == 8, "input_ModemSignalSet.changed_dcd offset");
_Static_assert(offsetof(struct input_ModemSignalSet, changed_ri) == 9, "input_ModemSignalSet.changed_ri offset");
struct input_LineErrorFlags {
    _Bool break_detected;
    _Bool framing;
    _Bool parity;
    _Bool overrun;
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_LineErrorFlags) == 4, "input_LineErrorFlags size");
_Static_assert(_Alignof(struct input_LineErrorFlags) == 1, "input_LineErrorFlags align");
_Static_assert(offsetof(struct input_LineErrorFlags, break_detected) == 0, "input_LineErrorFlags.break_detected offset");
_Static_assert(offsetof(struct input_LineErrorFlags, framing) == 1, "input_LineErrorFlags.framing offset");
_Static_assert(offsetof(struct input_LineErrorFlags, parity) == 2, "input_LineErrorFlags.parity offset");
_Static_assert(offsetof(struct input_LineErrorFlags, overrun) == 3, "input_LineErrorFlags.overrun offset");
struct input_SerialEventMask {
    _Bool rx_data_ready;
    _Bool tx_progress_or_idle;
    _Bool line_error;
    _Bool signal_change;
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialEventMask) == 4, "input_SerialEventMask size");
_Static_assert(_Alignof(struct input_SerialEventMask) == 1, "input_SerialEventMask align");
_Static_assert(offsetof(struct input_SerialEventMask, rx_data_ready) == 0, "input_SerialEventMask.rx_data_ready offset");
_Static_assert(offsetof(struct input_SerialEventMask, tx_progress_or_idle) == 1, "input_SerialEventMask.tx_progress_or_idle offset");
_Static_assert(offsetof(struct input_SerialEventMask, line_error) == 2, "input_SerialEventMask.line_error offset");
_Static_assert(offsetof(struct input_SerialEventMask, signal_change) == 3, "input_SerialEventMask.signal_change offset");
struct input_SerialTxBuffer {
    const uint8_t *data;
    size_t len;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_SerialTxBuffer) == 16, "input_SerialTxBuffer size");
_Static_assert(_Alignof(struct input_SerialTxBuffer) == 8, "input_SerialTxBuffer align");
_Static_assert(offsetof(struct input_SerialTxBuffer, data) == 0, "input_SerialTxBuffer.data offset");
_Static_assert(offsetof(struct input_SerialTxBuffer, len) == 8, "input_SerialTxBuffer.len offset");
struct input_SerialRxBuffer {
    uint8_t *data;
    size_t len;
    struct input_LineErrorFlags *errors;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_SerialRxBuffer) == 24, "input_SerialRxBuffer size");
_Static_assert(_Alignof(struct input_SerialRxBuffer) == 8, "input_SerialRxBuffer align");
_Static_assert(offsetof(struct input_SerialRxBuffer, data) == 0, "input_SerialRxBuffer.data offset");
_Static_assert(offsetof(struct input_SerialRxBuffer, len) == 8, "input_SerialRxBuffer.len offset");
_Static_assert(offsetof(struct input_SerialRxBuffer, errors) == 16, "input_SerialRxBuffer.errors offset");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__on_init(struct input_Pl011 *);
inline struct input_SerialDataBits input__impl_3__clone(const struct input_SerialDataBits *);
inline struct input_SerialStopBits input__impl_6__clone(const struct input_SerialStopBits *);
inline struct input_SerialParity input__impl_9__clone(const struct input_SerialParity *);
inline struct input_SerialFlowControl input__impl_12__clone(const struct input_SerialFlowControl *);
inline struct input_SerialDirection input__impl_15__clone(const struct input_SerialDirection *);
inline struct input_SerialStatus input__impl_18__clone(const struct input_SerialStatus *);
inline _Bool input__impl_21__eq(const struct input_SerialStatus *, const struct input_SerialStatus *);
inline void input__impl_22__assert_receiver_is_total_eq(const struct input_SerialStatus *);
inline struct input_SerialParams input__impl_24__clone(const struct input_SerialParams *);
inline struct input_ModemSignalSet input__impl_27__clone(const struct input_ModemSignalSet *);
inline struct input_LineErrorFlags input__impl_30__clone(const struct input_LineErrorFlags *);
inline struct input_SerialEventMask input__impl_33__clone(const struct input_SerialEventMask *);
inline struct input_SerialTxBuffer input__impl_36__clone(const struct input_SerialTxBuffer *);
inline struct input_SerialRxBuffer input__impl_39__clone(const struct input_SerialRxBuffer *);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__on_init(struct input_Pl011 *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
inline struct input_SerialDataBits input__impl_3__clone(const struct input_SerialDataBits *_1) {
    struct input_SerialDataBits _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialStopBits input__impl_6__clone(const struct input_SerialStopBits *_1) {
    struct input_SerialStopBits _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialParity input__impl_9__clone(const struct input_SerialParity *_1) {
    struct input_SerialParity _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialFlowControl input__impl_12__clone(const struct input_SerialFlowControl *_1) {
    struct input_SerialFlowControl _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialDirection input__impl_15__clone(const struct input_SerialDirection *_1) {
    struct input_SerialDirection _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialStatus input__impl_18__clone(const struct input_SerialStatus *_1) {
    struct input_SerialStatus _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_21__eq(const struct input_SerialStatus *_1, const struct input_SerialStatus *_2) {
    _Bool _0;
    const struct input_SerialStatus *_3;
    intptr_t _4;
    const struct input_SerialStatus *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((*_3).tag == ((uint8_t)(2U)) ? ((intptr_t)(2ULL)) : ((*_3).tag == ((uint8_t)(3U)) ? ((intptr_t)(3ULL)) : ((*_3).tag == ((uint8_t)(4U)) ? ((intptr_t)(4ULL)) : ((intptr_t)(5ULL)))))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((*_5).tag == ((uint8_t)(2U)) ? ((intptr_t)(2ULL)) : ((*_5).tag == ((uint8_t)(3U)) ? ((intptr_t)(3ULL)) : ((*_5).tag == ((uint8_t)(4U)) ? ((intptr_t)(4ULL)) : ((intptr_t)(5ULL)))))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_22__assert_receiver_is_total_eq(const struct input_SerialStatus *_1) {
    bb_0: {
        return;
    }
}
inline struct input_SerialParams input__impl_24__clone(const struct input_SerialParams *_1) {
    struct input_SerialParams _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_ModemSignalSet input__impl_27__clone(const struct input_ModemSignalSet *_1) {
    struct input_ModemSignalSet _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_LineErrorFlags input__impl_30__clone(const struct input_LineErrorFlags *_1) {
    struct input_LineErrorFlags _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialEventMask input__impl_33__clone(const struct input_SerialEventMask *_1) {
    struct input_SerialEventMask _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialTxBuffer input__impl_36__clone(const struct input_SerialTxBuffer *_1) {
    struct input_SerialTxBuffer _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline struct input_SerialRxBuffer input__impl_39__clone(const struct input_SerialRxBuffer *_1) {
    struct input_SerialRxBuffer _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
struct __rseq_lc_0_pl011_target {
    void *__rseq_dev;
    struct input_Pl011 inner;
    uint32_t __rseq_state;
};

static void __rseq_lc_0_pl011_shutdown(void) {
    regseq_lc_register("pl011", "amba:0x41011/0xfffff");
    regseq_lc_create();
    regseq_lc_associate();
    struct __rseq_lc_0_pl011_target inst = {0};
    regseq_lc_call("on_init");
    input__impl_0__on_init(&inst.inner);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_pl011_shutdown();
}
