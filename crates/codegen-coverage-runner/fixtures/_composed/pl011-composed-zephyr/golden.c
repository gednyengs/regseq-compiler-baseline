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
/* regseq target platform: os=zephyr board=qemu_cortex_a53 arch=aarch64 */

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
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228;
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494;
struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685;
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83;
struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a;
struct input_Field__dcf610a28ad7155fe10496944bda173a;
struct input_WriteOnly__39471f0616179e541513e5a50bf71559;
struct tuple_cbf29ce484222325;
struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685;
struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83;
struct input_ReadWrite__d743656c529b08e42785b912ffb751b0;
struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228;
struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494;
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c;
struct input_pl011_Regs;
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e;
struct input_Pl011Composed;
struct tuple_a8c7f832281a39c5;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct input_Field__12f6da63eb7da4bbe47ece8c97d11228;
struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494;
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818;
struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62;
struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17;
struct core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59;
struct core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7;
struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3;
struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd;
struct core_marker_PhantomData__c6f124df7d299765b3412359240618e6;
struct input_pl011_cr_Register;
struct input_pl011_dr_Register;
struct input_pl011_fbrd_Register;
struct input_pl011_fr_Register;
struct input_pl011_ibrd_Register;
struct input_pl011_icr_Register;
struct input_pl011_imsc_Register;
struct input_pl011_lcr_h_Register;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228) == 8, "input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 size");
_Static_assert(_Alignof(struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228) == 4, "input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 align");
_Static_assert(offsetof(struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228, mask) == 0, "input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228.mask offset");
_Static_assert(offsetof(struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228, value) == 4, "input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228.value offset");
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494) == 8, "input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 size");
_Static_assert(_Alignof(struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494) == 4, "input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 align");
_Static_assert(offsetof(struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494, mask) == 0, "input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494.mask offset");
_Static_assert(offsetof(struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494, value) == 4, "input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494.value offset");
struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685) == 8, "input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 size");
_Static_assert(_Alignof(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685) == 4, "input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 align");
_Static_assert(offsetof(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685, mask) == 0, "input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685.mask offset");
_Static_assert(offsetof(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685, value) == 4, "input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685.value offset");
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 8, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 size");
_Static_assert(_Alignof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 align");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, mask) == 0, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.mask offset");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, value) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.value offset");
struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a) == 4, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a size");
_Static_assert(_Alignof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a) == 4, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a align");
_Static_assert(offsetof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a, _v) == 0, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a._v offset");
struct input_Field__dcf610a28ad7155fe10496944bda173a {
    uint32_t mask;
    uint32_t shift;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Field__dcf610a28ad7155fe10496944bda173a) == 8, "input_Field__dcf610a28ad7155fe10496944bda173a size");
_Static_assert(_Alignof(struct input_Field__dcf610a28ad7155fe10496944bda173a) == 4, "input_Field__dcf610a28ad7155fe10496944bda173a align");
_Static_assert(offsetof(struct input_Field__dcf610a28ad7155fe10496944bda173a, mask) == 0, "input_Field__dcf610a28ad7155fe10496944bda173a.mask offset");
_Static_assert(offsetof(struct input_Field__dcf610a28ad7155fe10496944bda173a, shift) == 4, "input_Field__dcf610a28ad7155fe10496944bda173a.shift offset");
struct input_WriteOnly__39471f0616179e541513e5a50bf71559 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559) == 4, "input_WriteOnly__39471f0616179e541513e5a50bf71559 size");
_Static_assert(_Alignof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559) == 4, "input_WriteOnly__39471f0616179e541513e5a50bf71559 align");
_Static_assert(offsetof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559, _v) == 0, "input_WriteOnly__39471f0616179e541513e5a50bf71559._v offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685) == 4, "input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 size");
_Static_assert(_Alignof(struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685) == 4, "input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 align");
_Static_assert(offsetof(struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685, _v) == 0, "input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685._v offset");
struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83) == 4, "input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 size");
_Static_assert(_Alignof(struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83) == 4, "input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 align");
_Static_assert(offsetof(struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83, _v) == 0, "input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83._v offset");
struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0) == 4, "input_ReadWrite__d743656c529b08e42785b912ffb751b0 size");
_Static_assert(_Alignof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0) == 4, "input_ReadWrite__d743656c529b08e42785b912ffb751b0 align");
_Static_assert(offsetof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0, _v) == 0, "input_ReadWrite__d743656c529b08e42785b912ffb751b0._v offset");
struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228) == 4, "input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 size");
_Static_assert(_Alignof(struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228) == 4, "input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 align");
_Static_assert(offsetof(struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228, _v) == 0, "input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228._v offset");
struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494) == 4, "input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 size");
_Static_assert(_Alignof(struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494) == 4, "input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 align");
_Static_assert(offsetof(struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494, _v) == 0, "input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494._v offset");
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c align");
_Static_assert(offsetof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c, _v) == 0, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c._v offset");
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e size");
_Static_assert(_Alignof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e align");
_Static_assert(offsetof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e, _v) == 0, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e._v offset");
struct input_pl011_Regs {
    struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e dr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c rsr_ecr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved0[4];
    struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a fr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved1;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c ilpr;
    struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 ibrd;
    struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 fbrd;
    struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 lcr_h;
    struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 cr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c ifls;
    struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 imsc;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved2[2];
    struct input_WriteOnly__39471f0616179e541513e5a50bf71559 icr;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_pl011_Regs) == 72, "input_pl011_Regs size");
_Static_assert(_Alignof(struct input_pl011_Regs) == 4, "input_pl011_Regs align");
_Static_assert(offsetof(struct input_pl011_Regs, dr) == 0, "input_pl011_Regs.dr offset");
_Static_assert(offsetof(struct input_pl011_Regs, rsr_ecr) == 4, "input_pl011_Regs.rsr_ecr offset");
_Static_assert(offsetof(struct input_pl011_Regs, _reserved0) == 8, "input_pl011_Regs._reserved0 offset");
_Static_assert(offsetof(struct input_pl011_Regs, fr) == 24, "input_pl011_Regs.fr offset");
_Static_assert(offsetof(struct input_pl011_Regs, _reserved1) == 28, "input_pl011_Regs._reserved1 offset");
_Static_assert(offsetof(struct input_pl011_Regs, ilpr) == 32, "input_pl011_Regs.ilpr offset");
_Static_assert(offsetof(struct input_pl011_Regs, ibrd) == 36, "input_pl011_Regs.ibrd offset");
_Static_assert(offsetof(struct input_pl011_Regs, fbrd) == 40, "input_pl011_Regs.fbrd offset");
_Static_assert(offsetof(struct input_pl011_Regs, lcr_h) == 44, "input_pl011_Regs.lcr_h offset");
_Static_assert(offsetof(struct input_pl011_Regs, cr) == 48, "input_pl011_Regs.cr offset");
_Static_assert(offsetof(struct input_pl011_Regs, ifls) == 52, "input_pl011_Regs.ifls offset");
_Static_assert(offsetof(struct input_pl011_Regs, imsc) == 56, "input_pl011_Regs.imsc offset");
_Static_assert(offsetof(struct input_pl011_Regs, _reserved2) == 60, "input_pl011_Regs._reserved2 offset");
_Static_assert(offsetof(struct input_pl011_Regs, icr) == 68, "input_pl011_Regs.icr offset");
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c {
    struct input_pl011_Regs inner;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 72, "input_Mmio__b43d312b4ad71a71283184a14b095a0c size");
_Static_assert(_Alignof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 4, "input_Mmio__b43d312b4ad71a71283184a14b095a0c align");
_Static_assert(offsetof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c, inner) == 0, "input_Mmio__b43d312b4ad71a71283184a14b095a0c.inner offset");
struct input_Pl011Composed {
    struct input_Mmio__b43d312b4ad71a71283184a14b095a0c regs;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Pl011Composed) == 72, "input_Pl011Composed size");
_Static_assert(_Alignof(struct input_Pl011Composed) == 4, "input_Pl011Composed align");
_Static_assert(offsetof(struct input_Pl011Composed, regs) == 0, "input_Pl011Composed.regs offset");
struct tuple_a8c7f832281a39c5 {
    uint32_t _0;
    uint32_t _1;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct tuple_a8c7f832281a39c5) == 8, "tuple_a8c7f832281a39c5 size");
_Static_assert(_Alignof(struct tuple_a8c7f832281a39c5) == 4, "tuple_a8c7f832281a39c5 align");
_Static_assert(offsetof(struct tuple_a8c7f832281a39c5, _0) == 0, "tuple_a8c7f832281a39c5._0 offset");
_Static_assert(offsetof(struct tuple_a8c7f832281a39c5, _1) == 4, "tuple_a8c7f832281a39c5._1 offset");
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
struct input_Field__12f6da63eb7da4bbe47ece8c97d11228 {
    uint32_t mask;
    uint32_t shift;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228) == 8, "input_Field__12f6da63eb7da4bbe47ece8c97d11228 size");
_Static_assert(_Alignof(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228) == 4, "input_Field__12f6da63eb7da4bbe47ece8c97d11228 align");
_Static_assert(offsetof(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228, mask) == 0, "input_Field__12f6da63eb7da4bbe47ece8c97d11228.mask offset");
_Static_assert(offsetof(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228, shift) == 4, "input_Field__12f6da63eb7da4bbe47ece8c97d11228.shift offset");
struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494 {
    uint32_t mask;
    uint32_t shift;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494) == 8, "input_Field__65daba9b2aba7b1866e3e9b4c1c17494 size");
_Static_assert(_Alignof(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494) == 4, "input_Field__65daba9b2aba7b1866e3e9b4c1c17494 align");
_Static_assert(offsetof(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494, mask) == 0, "input_Field__65daba9b2aba7b1866e3e9b4c1c17494.mask offset");
_Static_assert(offsetof(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494, shift) == 4, "input_Field__65daba9b2aba7b1866e3e9b4c1c17494.shift offset");
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 0, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 1, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 align");
struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62) == 0, "core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62) == 1, "core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 align");
struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17) == 0, "core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17) == 1, "core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 align");
struct core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59) == 0, "core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59) == 1, "core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59 align");
struct core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7) == 0, "core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7) == 1, "core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7 align");
struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3) == 0, "core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3) == 1, "core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 align");
struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd) == 0, "core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd size");
_Static_assert(_Alignof(struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd) == 1, "core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd align");
struct core_marker_PhantomData__c6f124df7d299765b3412359240618e6 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__c6f124df7d299765b3412359240618e6) == 0, "core_marker_PhantomData__c6f124df7d299765b3412359240618e6 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__c6f124df7d299765b3412359240618e6) == 1, "core_marker_PhantomData__c6f124df7d299765b3412359240618e6 align");
struct input_pl011_cr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_cr_Register) == 0, "input_pl011_cr_Register size");
_Static_assert(_Alignof(struct input_pl011_cr_Register) == 1, "input_pl011_cr_Register align");
struct input_pl011_dr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_dr_Register) == 0, "input_pl011_dr_Register size");
_Static_assert(_Alignof(struct input_pl011_dr_Register) == 1, "input_pl011_dr_Register align");
struct input_pl011_fbrd_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_fbrd_Register) == 0, "input_pl011_fbrd_Register size");
_Static_assert(_Alignof(struct input_pl011_fbrd_Register) == 1, "input_pl011_fbrd_Register align");
struct input_pl011_fr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_fr_Register) == 0, "input_pl011_fr_Register size");
_Static_assert(_Alignof(struct input_pl011_fr_Register) == 1, "input_pl011_fr_Register align");
struct input_pl011_ibrd_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_ibrd_Register) == 0, "input_pl011_ibrd_Register size");
_Static_assert(_Alignof(struct input_pl011_ibrd_Register) == 1, "input_pl011_ibrd_Register align");
struct input_pl011_icr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_icr_Register) == 0, "input_pl011_icr_Register size");
_Static_assert(_Alignof(struct input_pl011_icr_Register) == 1, "input_pl011_icr_Register align");
struct input_pl011_imsc_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_imsc_Register) == 0, "input_pl011_imsc_Register size");
_Static_assert(_Alignof(struct input_pl011_imsc_Register) == 1, "input_pl011_imsc_Register align");
struct input_pl011_lcr_h_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_lcr_h_Register) == 0, "input_pl011_lcr_h_Register size");
_Static_assert(_Alignof(struct input_pl011_lcr_h_Register) == 1, "input_pl011_lcr_h_Register align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 input__impl_1__new__12f6da63eb7da4bbe47ece8c97d11228(uint32_t, uint32_t);
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 input__impl_1__new__65daba9b2aba7b1866e3e9b4c1c17494(uint32_t, uint32_t);
struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 input__impl_2__add__5ec21ccfd9c212e921542115e3119b59(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685);
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
__attribute__((noinline)) uint32_t input__impl_3__read_field__dcf610a28ad7155fe10496944bda173a(const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *, struct input_Field__dcf610a28ad7155fe10496944bda173a);
__attribute__((noinline)) void input__impl_4__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *, uint32_t);
__attribute__((noinline)) void input__impl_5__modify__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685);
__attribute__((noinline)) uint32_t input__impl_5__read__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *);
__attribute__((noinline)) void input__impl_5__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *, uint32_t);
__attribute__((noinline)) void input__impl_5__write_field__12f6da63eb7da4bbe47ece8c97d11228(const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *, struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228);
__attribute__((noinline)) void input__impl_5__write_field__65daba9b2aba7b1866e3e9b4c1c17494(const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *, struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494);
__attribute__((noinline)) void input__impl_5__write_field__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685);
__attribute__((noinline)) void input__impl_5__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_9__new(void);
const struct input_pl011_Regs *input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *);
void input__impl_12__clear_all_interrupts(struct input_Pl011Composed *);
void input__impl_12__disable_uart(struct input_Pl011Composed *);
void input__impl_12__mask_all_interrupts(struct input_Pl011Composed *);
struct tuple_a8c7f832281a39c5 input__impl_12__set_baud_rate(struct input_Pl011Composed *);
_Bool input__impl_12__wait_tx_idle(const struct input_Pl011Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_13__on_init(struct input_Pl011Composed *);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 input_field_val__192b981e79185e5cfc4e4e7cb3debe62(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228, uint32_t);
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 input_field_val__9a9d8cfaa0cf152914f8e40836b690c7(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494, uint32_t);
extern const uint8_t input__regseq_synth__const_memory_static_134577ba6481ce8d78381364c5694e85[35];
extern const uint8_t input__regseq_synth__const_memory_static_50e61e8a82b1e2fbd8f11157633ea14a[26];
extern const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0];
extern const uint8_t input__regseq_synth__const_memory_static_c76f9251ccd091730c6d8aaa761ae26a[17];
extern const uint8_t input__regseq_synth__const_memory_static_d38b60cb79f88189da875e97be3a8174[8];
extern const uint8_t input__regseq_synth__const_memory_static_da6af18769e677572bf657f9e3b7a6c3[32];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 input__impl_1__new__12f6da63eb7da4bbe47ece8c97d11228(uint32_t _1, uint32_t _2) {
    struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 _0;
    uint32_t _3;
    uint32_t _4;
    bb_0: {
        _3 = _1;
        _4 = _2;
        _0 = (struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228){ .mask = _3, .value = _4 };
        return _0;
    }
}
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 input__impl_1__new__65daba9b2aba7b1866e3e9b4c1c17494(uint32_t _1, uint32_t _2) {
    struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 _0;
    uint32_t _3;
    uint32_t _4;
    bb_0: {
        _3 = _1;
        _4 = _2;
        _0 = (struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494){ .mask = _3, .value = _4 };
        return _0;
    }
}
struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 input__impl_2__add__5ec21ccfd9c212e921542115e3119b59(struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _1, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _2) {
    struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _0;
    uint32_t _3;
    uint32_t _4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _8;
    bb_0: {
        _3 = _1.mask;
        _4 = _2.mask;
        _5 = ((_3) | (_4));
        _6 = _1.value;
        _7 = _2.value;
        _8 = ((_6) | (_7));
        _0 = (struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = _5, .value = _8 };
        return _0;
    }
}
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _0;
    uint32_t _3;
    uint32_t _4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _8;
    bb_0: {
        _3 = _1.mask;
        _4 = _2.mask;
        _5 = ((_3) | (_4));
        _6 = _1.value;
        _7 = _2.value;
        _8 = ((_6) | (_7));
        _0 = (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = _5, .value = _8 };
        return _0;
    }
}
__attribute__((noinline)) uint32_t input__impl_3__read_field__dcf610a28ad7155fe10496944bda173a(const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *_1, struct input_Field__dcf610a28ad7155fe10496944bda173a _2) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_4__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_5__modify__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_1, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) uint32_t input__impl_5__read__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_5__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_5__write_field__12f6da63eb7da4bbe47ece8c97d11228(const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *_1, struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_5__write_field__65daba9b2aba7b1866e3e9b4c1c17494(const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *_1, struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_5__write_field__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_1, struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_5__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_9__new(void) {
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _0;
    bb_0: {
        _0 = (struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c){ ._v = ((uint32_t)(0U)) };
        return _0;
    }
}
const struct input_pl011_Regs *input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_1) {
    const struct input_pl011_Regs *_0;
    const struct input_pl011_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
void input__impl_12__clear_all_interrupts(struct input_Pl011Composed *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_4;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).icr);
        regseq_bus_mmio_write(68, 4, (uint64_t)(((uint32_t)(2047U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
void input__impl_12__disable_uart(struct input_Pl011Composed *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_4;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _5;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _6;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).cr);
        _5 = input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(0U)) }, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(256U)), .value = ((uint32_t)(0U)) }); goto bb_2;
    }
    bb_2: {
        _6 = input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd(_5, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(512U)), .value = ((uint32_t)(0U)) }); goto bb_3;
    }
    bb_3: {
        regseq_bus_mmio_write(48, 4, (uint64_t)((_6).value & (_6).mask)); goto bb_4;
    }
    bb_4: {
        return;
    }
}
void input__impl_12__mask_all_interrupts(struct input_Pl011Composed *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_4;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).imsc);
        regseq_bus_mmio_write(56, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
struct tuple_a8c7f832281a39c5 input__impl_12__set_baud_rate(struct input_Pl011Composed *_1) {
    struct tuple_a8c7f832281a39c5 _0;
    uint32_t _2;
    uint32_t _3;
    uint32_t _4;
    _Bool _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _8;
    uint32_t _9;
    uint32_t _10;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_11;
    const struct input_pl011_Regs *_12;
    const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *_13;
    uint32_t _14;
    struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 _15;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_17;
    const struct input_pl011_Regs *_18;
    const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *_19;
    uint32_t _20;
    struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 _21;
    uint32_t _23;
    uint32_t _24;
    bb_0: {
        _2 = ((((uint32_t)(24000000U))) * (((uint32_t)(8U))));
        _3 = ((_2) + (((uint32_t)(115200U))));
        _4 = ((((uint32_t)(2U))) * (((uint32_t)(115200U))));
        _5 = ((_4) == (((uint32_t)(0U))));
        if ((_5) == 0) goto bb_1; else { regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO); }
    }
    bb_1: {
        _6 = ((_3) / (_4));
        _7 = _6;
        _8 = ((_7) >> ((((int32_t)(6U))) & 31));
        _9 = _6;
        _10 = ((_9) & (((uint32_t)(63U))));
        _11 = &((*_1).regs);
        _12 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_11); goto bb_2;
    }
    bb_2: {
        _13 = &((*_12).ibrd);
        _14 = _8;
        _15 = input_field_val__192b981e79185e5cfc4e4e7cb3debe62((struct input_Field__12f6da63eb7da4bbe47ece8c97d11228){ .mask = ((uint32_t)(65535U)), .shift = ((uint32_t)(0U)) }, _14); goto bb_3;
    }
    bb_3: {
        regseq_bus_mmio_write(36, 4, (uint64_t)((_15).value & (_15).mask)); goto bb_4;
    }
    bb_4: {
        _17 = &((*_1).regs);
        _18 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_17); goto bb_5;
    }
    bb_5: {
        _19 = &((*_18).fbrd);
        _20 = _10;
        _21 = input_field_val__9a9d8cfaa0cf152914f8e40836b690c7((struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494){ .mask = ((uint32_t)(63U)), .shift = ((uint32_t)(0U)) }, _20); goto bb_6;
    }
    bb_6: {
        regseq_bus_mmio_write(40, 4, (uint64_t)((_21).value & (_21).mask)); goto bb_7;
    }
    bb_7: {
        _23 = _8;
        _24 = _10;
        _0 = (struct tuple_a8c7f832281a39c5){ ._0 = _23, ._1 = _24 };
        return _0;
    }
}
_Bool input__impl_12__wait_tx_idle(const struct input_Pl011Composed *_1) {
    _Bool _0;
    uint32_t _2;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_3;
    const struct input_pl011_Regs *_4;
    const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *_5;
    uint32_t _6;
    _Bool _7;
    uint32_t _8;
    _Bool _9;
    bb_0: {
        _2 = ((uint32_t)(0U));
        goto bb_1;
    }
    bb_1: {
        _3 = &((*_1).regs);
        _4 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_3); goto bb_2;
    }
    bb_2: {
        _5 = &((*_4).fr);
        _6 = (uint32_t)((regseq_bus_mmio_read(24, 4) & ((struct input_Field__dcf610a28ad7155fe10496944bda173a){ .mask = ((uint32_t)(8U)), .shift = ((uint32_t)(3U)) }).mask) >> ((struct input_Field__dcf610a28ad7155fe10496944bda173a){ .mask = ((uint32_t)(8U)), .shift = ((uint32_t)(3U)) }).shift); goto bb_3;
    }
    bb_3: {
        _7 = ((_6) != (((uint32_t)(0U))));
        switch (_7) { case 0ULL: goto bb_7; default: goto bb_4; }
    }
    bb_4: {
        _2 = ((_2) + (((uint32_t)(1U))));
        _8 = _2;
        _9 = ((_8) >= (((uint32_t)(1000U))));
        switch (_9) { case 0ULL: goto bb_6; default: goto bb_5; }
    }
    bb_5: {
        _0 = 0;
        goto bb_8;
    }
    bb_6: {
        goto bb_1;
    }
    bb_7: {
        _0 = 1;
        goto bb_8;
    }
    bb_8: {
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_13__on_init(struct input_Pl011Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    struct input_Pl011Composed *_7;
    const struct input_Pl011Composed *_9;
    _Bool _10;
    struct input_RegseqError _17;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_18;
    const struct input_pl011_Regs *_19;
    const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_20;
    struct input_Pl011Composed *_22;
    struct tuple_a8c7f832281a39c5 _23;
    uint32_t _24;
    uint32_t _25;
    uint32_t _26;
    uint32_t _28;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_35;
    const struct input_pl011_Regs *_36;
    const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_37;
    struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _38;
    struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _39;
    struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685 _40;
    struct input_Pl011Composed *_42;
    struct input_Pl011Composed *_44;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_46;
    const struct input_pl011_Regs *_47;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_48;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _49;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _50;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_52;
    const struct input_pl011_Regs *_53;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_54;
    uint32_t _55;
    uint32_t _56;
    uint32_t _57;
    _Bool _58;
    uint32_t _64;
    struct input_RegseqError _71;
    bb_0: {
        fwrite("[DEBUG] pl011 probe start\n", 1, 26, stdout); goto bb_1;
    }
    bb_1: {
        _7 = &((*_1));
        input__impl_12__disable_uart(_7); goto bb_2;
    }
    bb_2: {
        _9 = &((*_1));
        _10 = input__impl_12__wait_tx_idle(_9); goto bb_3;
    }
    bb_3: {
        switch (_10) { case 0ULL: goto bb_29; default: goto bb_4; }
    }
    bb_4: {
        _18 = &((*_1).regs);
        _19 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_18); goto bb_5;
    }
    bb_5: {
        _20 = &((*_19).lcr_h);
        { uint64_t _rmw_cur = regseq_bus_mmio_read(44, 4); regseq_bus_mmio_write(44, 4, (_rmw_cur & ~(uint64_t)(((struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(16U)), .value = ((uint32_t)(0U)) }).mask)) | (uint64_t)(((struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(16U)), .value = ((uint32_t)(0U)) }).value & ((struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(16U)), .value = ((uint32_t)(0U)) }).mask)); } goto bb_6;
    }
    bb_6: {
        _22 = &((*_1));
        _23 = input__impl_12__set_baud_rate(_22); goto bb_7;
    }
    bb_7: {
        _24 = _23._0;
        _25 = _23._1;
        _26 = _24;
        goto bb_8;
    }
    bb_8: {
        _28 = _25;
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] pl011 baud divisors: ", 1, 28, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_26)); fwrite(_n, 1, _l, stdout); } fwrite(" ", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_28)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        _35 = &((*_1).regs);
        _36 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_35); goto bb_11;
    }
    bb_11: {
        _37 = &((*_36).lcr_h);
        _38 = input__impl_2__add__5ec21ccfd9c212e921542115e3119b59((struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(96U)), .value = ((uint32_t)(96U)) }, (struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(8U)), .value = ((uint32_t)(0U)) }); goto bb_12;
    }
    bb_12: {
        _39 = input__impl_2__add__5ec21ccfd9c212e921542115e3119b59(_38, (struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(2U)), .value = ((uint32_t)(0U)) }); goto bb_13;
    }
    bb_13: {
        _40 = input__impl_2__add__5ec21ccfd9c212e921542115e3119b59(_39, (struct input_FieldValue__b7ae42fd0448c9d81ff6641dc3fc5685){ .mask = ((uint32_t)(16U)), .value = ((uint32_t)(16U)) }); goto bb_14;
    }
    bb_14: {
        regseq_bus_mmio_write(44, 4, (uint64_t)((_40).value & (_40).mask)); goto bb_15;
    }
    bb_15: {
        _42 = &((*_1));
        input__impl_12__mask_all_interrupts(_42); goto bb_16;
    }
    bb_16: {
        _44 = &((*_1));
        input__impl_12__clear_all_interrupts(_44); goto bb_17;
    }
    bb_17: {
        _46 = &((*_1).regs);
        _47 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_46); goto bb_18;
    }
    bb_18: {
        _48 = &((*_47).cr);
        _49 = input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(1U)) }, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(256U)), .value = ((uint32_t)(256U)) }); goto bb_19;
    }
    bb_19: {
        _50 = input__impl_2__add__ae10abed9e3303e99a36ab95b86fdfcd(_49, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(512U)), .value = ((uint32_t)(512U)) }); goto bb_20;
    }
    bb_20: {
        regseq_bus_mmio_write(48, 4, (uint64_t)((_50).value & (_50).mask)); goto bb_21;
    }
    bb_21: {
        _52 = &((*_1).regs);
        _53 = input__impl_11__deref__b43d312b4ad71a71283184a14b095a0c(_52); goto bb_22;
    }
    bb_22: {
        _54 = &((*_53).cr);
        _55 = (uint32_t)regseq_bus_mmio_read(48, 4); goto bb_23;
    }
    bb_23: {
        _56 = _55;
        _57 = ((_56) & (((uint32_t)(1U))));
        _58 = ((_57) == (((uint32_t)(0U))));
        switch (_58) { case 0ULL: goto bb_27; default: goto bb_24; }
    }
    bb_24: {
        _64 = _55;
        goto bb_25;
    }
    bb_25: {
        fwrite("[ERROR] pl011 enable readback failed: ", 1, 38, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#x", (unsigned)(_64)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_26;
    }
    bb_26: {
        _71.tag = ((uint8_t)(0U));
        _0._0 = _71;
        goto bb_32;
    }
    bb_27: {
        fwrite("[INFO] pl011 up\n", 1, 16, stdout); goto bb_28;
    }
    bb_28: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_32;
    }
    bb_29: {
        goto bb_30;
    }
    bb_30: {
        fwrite("[ERROR] pl011 tx busy past spin bound ", 1, 38, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(((uint32_t)(1000U)))); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_31;
    }
    bb_31: {
        _17.tag = ((uint8_t)(1U));
        _0._0 = _17;
        goto bb_32;
    }
    bb_32: {
        return _0;
    }
}
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 input_field_val__192b981e79185e5cfc4e4e7cb3debe62(struct input_Field__12f6da63eb7da4bbe47ece8c97d11228 _1, uint32_t _2) {
    struct input_FieldValue__12f6da63eb7da4bbe47ece8c97d11228 _0;
    uint32_t _3;
    uint32_t _4;
    uint32_t _5;
    uint32_t _6;
    bb_0: {
        _3 = _1.mask;
        _4 = _2;
        _5 = _1.shift;
        _6 = ((uint32_t)((uint32_t)(_4) << ((_5) & 31)));
        _0 = input__impl_1__new__12f6da63eb7da4bbe47ece8c97d11228(_3, _6); goto bb_1;
    }
    bb_1: {
        return _0;
    }
}
struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 input_field_val__9a9d8cfaa0cf152914f8e40836b690c7(struct input_Field__65daba9b2aba7b1866e3e9b4c1c17494 _1, uint32_t _2) {
    struct input_FieldValue__65daba9b2aba7b1866e3e9b4c1c17494 _0;
    uint32_t _3;
    uint32_t _4;
    uint32_t _5;
    uint32_t _6;
    bb_0: {
        _3 = _1.mask;
        _4 = _2;
        _5 = _1.shift;
        _6 = ((uint32_t)((uint32_t)(_4) << ((_5) & 31)));
        _0 = input__impl_1__new__65daba9b2aba7b1866e3e9b4c1c17494(_3, _6); goto bb_1;
    }
    bb_1: {
        return _0;
    }
}
const uint8_t input__regseq_synth__const_memory_static_134577ba6481ce8d78381364c5694e85[35] = { ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(97U)), ((uint8_t)(98U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(100U)), ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(32U)), ((uint8_t)(102U)), ((uint8_t)(97U)), ((uint8_t)(105U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(100U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(120U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_50e61e8a82b1e2fbd8f11157633ea14a[26] = { ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(117U)), ((uint8_t)(100U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(105U)), ((uint8_t)(118U)), ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(111U)), ((uint8_t)(114U)), ((uint8_t)(115U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0] = {0};
const uint8_t input__regseq_synth__const_memory_static_c76f9251ccd091730c6d8aaa761ae26a[17] = { ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(111U)), ((uint8_t)(98U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(116U)) };
const uint8_t input__regseq_synth__const_memory_static_d38b60cb79f88189da875e97be3a8174[8] = { ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(117U)), ((uint8_t)(112U)) };
const uint8_t input__regseq_synth__const_memory_static_da6af18769e677572bf657f9e3b7a6c3[32] = { ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(120U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(117U)), ((uint8_t)(115U)), ((uint8_t)(121U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(112U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(111U)), ((uint8_t)(117U)), ((uint8_t)(110U)), ((uint8_t)(100U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
struct __rseq_lc_0_pl011_target {
    void *__rseq_dev;
    struct input_Pl011Composed inner;
    uint32_t __rseq_state;
};

static void __rseq_lc_0_pl011_orderly(void) {
    regseq_lc_register("pl011", "dt:regseq,pl011");
    regseq_bus_register_driver("amba", "bound");
    regseq_lc_create();
    regseq_lc_associate();
    regseq_lc_preamble("regs");
    regseq_bus_claim_regions(-1);
    regseq_bus_get_handle();
    regseq_bus_map_region(0x9040000ull, 0x1000ull, -1);
    struct __rseq_lc_0_pl011_target inst = {0};
    regseq_lc_call("on_init");
    input__impl_13__on_init(&inst.inner);
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    regseq_lc_postamble("regs", 0);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_pl011_orderly();
}
