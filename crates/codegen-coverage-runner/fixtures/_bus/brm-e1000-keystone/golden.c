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
/* regseq target platform: os=linux board=qemu-pc arch=x86_64 */

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

/* ---------------------------------------------------------------------------
 * Bus-resource-management host bus model (construct #3, the keystone). Emitted
 * only for a TU with a resolved bus-resource binding. `regseq_bus_map_region`
 * installs the real (base,size) the device-lifecycle preamble resolved from the
 * platform file's [mmio.<id>] literals; the based accessors trace the absolute
 * mapped address (base+off), retiring the faked base. */
static uint64_t regseq_bus_mapped_base __attribute__((unused)) = 0;
/* `register_driver` selector (construct #3 M6/T18): DL emits the registration;
 * bus-resource supplies the bus-path selector (`pci`/`amba`/`platform`/…) + a
 * marker of whether a resolved region binding was supplied. The real backend
 * maps the selector to `pci_register_driver` / `module_amba_driver` /
 * `DEVICE_DT_DEFINE` (backend-[Open]); the host trace just records it. */
__attribute__((unused)) static void regseq_bus_register_driver(const char *sel, const char *binding) {
    printf("BUS register_driver path=%s binding=%s\n", sel, binding);
}
/* Host bus-model state the four compiler-managed preamble ops record (design
 * §5: generic-c `enable_device`/`set_bus_master` "record in the host bus
 * model"). Inspectable so a fixture can witness that bring-up ran. */
static int regseq_bus_enabled __attribute__((unused)) = 0;
static int regseq_bus_mastered __attribute__((unused)) = 0;
/* `enable_device` (construct #3 M3): PCI-capable bus only (default-on on
 * platform, so not emitted there). Records "enabled" in the host bus model. */
__attribute__((unused)) static void regseq_bus_enable_device(void) {
    regseq_bus_enabled = 1;
    printf("BUS enable_device\n");
}
/* `claim_regions` (construct #3 M3): per region-bearing binding, on EVERY bus
 * (not PCI-gated). On the generic-c host there is no runtime reservation — the
 * region IS the [mmio.<id>] binding (design §5), so this is a trace-only marker
 * carrying the effective BAR. The three real realizations are backend-[Open]
 * (design §3/§8.2): Linux PCI reserves (pci_request_selected_regions); Linux
 * AMBA fuses claim∘map into one devm_ioremap_resource; Zephyr has no runtime
 * claim (the region is resolved/sized, not reserved). */
__attribute__((unused)) static void regseq_bus_claim_regions(int64_t bar) {
    printf("BUS claim_regions bar=%lld\n", (long long)bar);
}
/* `set_bus_master` (construct #3 M3): PCI-capable bus AND DMA-Management
 * surface present (design §3; DMA-presence is GRILL O2, the DMA construct's
 * fact). Defaults absent today, so this is emitted by no fixture yet. Records
 * "master" in the host bus model. */
__attribute__((unused)) static void regseq_bus_set_master(void) {
    regseq_bus_mastered = 1;
    printf("BUS set_master\n");
}
__attribute__((unused)) static void *regseq_bus_get_handle(void) {
    printf("BUS get_handle\n");
    return &regseq_bus_mapped_base;
}
__attribute__((unused)) static void regseq_bus_map_region(uint64_t base, uint64_t size, int64_t bar) {
    regseq_bus_mapped_base = base;
    /* `bar` is the resolved EFFECTIVE BAR (L2.bar ?? L1.bar) the real backend
     * would enumerate from (pci_resource_start(effective_bar)); -1 = no BAR (a
     * platform/DT region). Tracing it makes the L2 `bar` override observable:
     * an L2 `bar = 2` over an L1 `bar = 0` traces `bar=2` + the BAR-2 base. */
    printf("BUS map base=0x%llx size=0x%llx bar=%lld\n",
           (unsigned long long)base, (unsigned long long)size, (long long)bar);
}
__attribute__((unused)) static uint64_t regseq_bus_mmio_read(uint32_t off, uint32_t width) {
    uint64_t v = 0;
    for (uint32_t i = 0; i < width && (off + i) < REGSEQ_REGFILE_BYTES; i++) {
        v |= (uint64_t)regseq_regfile[off + i] << (8u * i);
    }
    printf("R%u @0x%llx = 0x%0*llx\n", (unsigned)(width * 8u),
           (unsigned long long)(regseq_bus_mapped_base + off),
           (int)(width * 2u), (unsigned long long)v);
    return v;
}
__attribute__((unused)) static void regseq_bus_mmio_write(uint32_t off, uint32_t width, uint64_t val) {
    printf("W%u @0x%llx = 0x%0*llx\n", (unsigned)(width * 8u),
           (unsigned long long)(regseq_bus_mapped_base + off),
           (int)(width * 2u), (unsigned long long)val);
    for (uint32_t i = 0; i < width && (off + i) < REGSEQ_REGFILE_BYTES; i++) {
        regseq_regfile[off + i] = (uint8_t)(val >> (8u * i));
    }
}
/* ---------------------------------------------------------------------------
 * Config-space host model (construct #3 M5). A tiny PCI config space the host
 * bus model answers for the `regseq::bus::read_config/write_config` intrinsics:
 *   - `read_config` witnesses the PCI_COMMAND register read, AND implements the
 *     PCI-ID-caching rule — the vendor/device id (config offset 0x00) is read
 *     from config space ONCE and cached; a later id read is served from the
 *     cache, not a fresh config read (design §3, GRILL R03).
 *   - `write_config` is recorded only (no behavioral witness, GRILL O3).
 * The fixture seeds `regseq_bus_config_space` before exercising the hooks. */
#define REGSEQ_PCI_VENDOR_ID 0x00u
#define REGSEQ_PCI_COMMAND   0x04u
static uint8_t regseq_bus_config_space[256] __attribute__((unused)) = {0};
static int regseq_bus_pci_id_cached __attribute__((unused)) = 0;
static uint64_t regseq_bus_pci_id_cache __attribute__((unused)) = 0;
__attribute__((unused)) static uint64_t regseq_bus_config_assemble(uint32_t off, uint32_t width) {
    uint64_t v = 0;
    for (uint32_t i = 0; i < width && (off + i) < sizeof(regseq_bus_config_space); i++) {
        v |= (uint64_t)regseq_bus_config_space[off + i] << (8u * i);
    }
    return v;
}
__attribute__((unused)) static uint64_t regseq_bus_on_config_read(uint32_t off, uint32_t width) {
    /* PCI-ID-caching rule: a second read of the vendor/device id offset is
     * served from the cache (one underlying config read for two id reads). */
    if (off == REGSEQ_PCI_VENDOR_ID) {
        if (regseq_bus_pci_id_cached) {
            printf("BUS config_read id_cache off=0x%x width=%u = 0x%llx\n",
                   off, width, (unsigned long long)regseq_bus_pci_id_cache);
            return regseq_bus_pci_id_cache;
        }
        regseq_bus_pci_id_cache = regseq_bus_config_assemble(off, width);
        regseq_bus_pci_id_cached = 1;
    }
    uint64_t v = regseq_bus_config_assemble(off, width);
    printf("BUS config_read off=0x%x width=%u = 0x%llx\n",
           off, width, (unsigned long long)v);
    return v;
}
__attribute__((unused)) static void regseq_bus_on_config_write(uint32_t off, uint32_t width, uint64_t val) {
    printf("BUS config_write off=0x%x width=%u val=0x%llx\n",
           off, width, (unsigned long long)val);
    for (uint32_t i = 0; i < width && (off + i) < sizeof(regseq_bus_config_space); i++) {
        regseq_bus_config_space[off + i] = (uint8_t)(val >> (8u * i));
    }
}

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685;
struct tuple_cbf29ce484222325;
struct input_Mmio__26db838a6cdd169cfbc246118ccc020d;
struct input_Regs;
struct input_E1000;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685) == 4, "input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 size");
_Static_assert(_Alignof(struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685) == 4, "input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 align");
_Static_assert(offsetof(struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685, _v) == 0, "input_ReadWrite__b8e1c27e9d753ab63985a9f709423685._v offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct input_Regs {
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 ctrl;
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 status;
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 icr;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Regs) == 12, "input_Regs size");
_Static_assert(_Alignof(struct input_Regs) == 4, "input_Regs align");
_Static_assert(offsetof(struct input_Regs, ctrl) == 0, "input_Regs.ctrl offset");
_Static_assert(offsetof(struct input_Regs, status) == 4, "input_Regs.status offset");
_Static_assert(offsetof(struct input_Regs, icr) == 8, "input_Regs.icr offset");
struct input_Mmio__26db838a6cdd169cfbc246118ccc020d {
    struct input_Regs inner;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Mmio__26db838a6cdd169cfbc246118ccc020d) == 12, "input_Mmio__26db838a6cdd169cfbc246118ccc020d size");
_Static_assert(_Alignof(struct input_Mmio__26db838a6cdd169cfbc246118ccc020d) == 4, "input_Mmio__26db838a6cdd169cfbc246118ccc020d align");
_Static_assert(offsetof(struct input_Mmio__26db838a6cdd169cfbc246118ccc020d, inner) == 0, "input_Mmio__26db838a6cdd169cfbc246118ccc020d.inner offset");
struct input_E1000 {
    struct input_Mmio__26db838a6cdd169cfbc246118ccc020d regs;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_E1000) == 12, "input_E1000 size");
_Static_assert(_Alignof(struct input_E1000) == 4, "input_E1000 align");
_Static_assert(offsetof(struct input_E1000, regs) == 0, "input_E1000.regs offset");
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
struct core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567) == 0, "core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567) == 1, "core_marker_PhantomData__8ee4b2501b210b87693e9ea3f7db3567 align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 input__impl_0__new__8ee4b2501b210b87693e9ea3f7db3567(void);
__attribute__((noinline)) uint32_t input__impl_0__read__8ee4b2501b210b87693e9ea3f7db3567(const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *);
__attribute__((noinline)) void input__impl_0__write__8ee4b2501b210b87693e9ea3f7db3567(const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *, uint32_t);
const struct input_Regs *input__impl_2__deref__26db838a6cdd169cfbc246118ccc020d(const struct input_Mmio__26db838a6cdd169cfbc246118ccc020d *);
struct input_Regs input__impl_3__new(void);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_init(struct input_E1000 *);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 input__impl_0__new__8ee4b2501b210b87693e9ea3f7db3567(void) {
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 _0;
    bb_0: {
        _0 = (struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685){ ._v = ((uint32_t)(0U)) };
        return _0;
    }
}
__attribute__((noinline)) uint32_t input__impl_0__read__8ee4b2501b210b87693e9ea3f7db3567(const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_0__write__8ee4b2501b210b87693e9ea3f7db3567(const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *_1, uint32_t _2) {
    bb_0: {
        return;
    }
}
const struct input_Regs *input__impl_2__deref__26db838a6cdd169cfbc246118ccc020d(const struct input_Mmio__26db838a6cdd169cfbc246118ccc020d *_1) {
    const struct input_Regs *_0;
    const struct input_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
struct input_Regs input__impl_3__new(void) {
    struct input_Regs _0;
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 _1;
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 _2;
    struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 _3;
    bb_0: {
        _1 = input__impl_0__new__8ee4b2501b210b87693e9ea3f7db3567(); goto bb_1;
    }
    bb_1: {
        _2 = input__impl_0__new__8ee4b2501b210b87693e9ea3f7db3567(); goto bb_2;
    }
    bb_2: {
        _3 = input__impl_0__new__8ee4b2501b210b87693e9ea3f7db3567(); goto bb_3;
    }
    bb_3: {
        _0 = (struct input_Regs){ .ctrl = _1, .status = _2, .icr = _3 };
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_init(struct input_E1000 *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_Mmio__26db838a6cdd169cfbc246118ccc020d *_2;
    const struct input_Regs *_3;
    const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *_4;
    const struct input_Mmio__26db838a6cdd169cfbc246118ccc020d *_6;
    const struct input_Regs *_7;
    const struct input_ReadWrite__b8e1c27e9d753ab63985a9f709423685 *_8;
    uint32_t _9;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_2__deref__26db838a6cdd169cfbc246118ccc020d(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).ctrl);
        regseq_bus_mmio_write(0, 4, (uint64_t)(((uint32_t)(64U)))); goto bb_2;
    }
    bb_2: {
        _6 = &((*_1).regs);
        _7 = input__impl_2__deref__26db838a6cdd169cfbc246118ccc020d(_6); goto bb_3;
    }
    bb_3: {
        _8 = &((*_7).icr);
        _9 = (uint32_t)regseq_bus_mmio_read(8, 4); goto bb_4;
    }
    bb_4: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct __rseq_lc_0_e1000_target {
    void *__rseq_dev;
    struct input_E1000 inner;
    uint32_t __rseq_state;
};

static void __rseq_lc_0_e1000_orderly(void) {
    regseq_lc_register("e1000", "pci:0x8086:0x100e");
    regseq_bus_register_driver("pci", "bound");
    regseq_lc_create();
    regseq_lc_associate();
    regseq_lc_preamble("regs");
    regseq_bus_enable_device();
    regseq_bus_claim_regions(0);
    regseq_bus_get_handle();
    regseq_bus_map_region(0xf0000000ull, 0x20000ull, 0);
    struct __rseq_lc_0_e1000_target inst = {0};
    regseq_lc_call("on_init");
    input__impl_4__on_init(&inst.inner);
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    regseq_lc_postamble("regs", 0);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_e1000_orderly();
}
