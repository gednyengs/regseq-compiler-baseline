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
struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda;
struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e;
struct tuple_cbf29ce484222325;
struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39;
struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39;
struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825;
struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825;
struct input_Field__8cb36d2526ae4a3d9219d36a5993db39;
struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d;
struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62;
struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6;
struct input_ReadWrite__787947e178fa2899797b7d93147718a7;
struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751;
struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544;
struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e;
struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95;
struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95;
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b;
struct input_e1000_Regs;
struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f;
struct input_e1000_RaEntry;
struct input_E1000Composed;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a;
struct core_marker_PhantomData__2461faaae176aa23b8290043178aca96;
struct core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511;
struct core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577;
struct core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad;
struct core_marker_PhantomData__8023637d63cff66abf14b05094f293b7;
struct core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de;
struct core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb;
struct core_marker_PhantomData__b62b53c72509c28288b0363e59aea163;
struct core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb;
struct core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020;
struct core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d;
struct core_marker_PhantomData__e9d6bb486817af9131266cec6067699a;
struct input_e1000_ctrl_Register;
struct input_e1000_icr_Register;
struct input_e1000_imc_Register;
struct input_e1000_mta_Register;
struct input_e1000_ra_rah_Register;
struct input_e1000_ra_ral_Register;
struct input_e1000_status_Register;
struct input_e1000_tctl_Register;
struct input_e1000_tdbah_Register;
struct input_e1000_tdbal_Register;
struct input_e1000_tdlen_Register;
struct input_e1000_tdt_Register;
struct input_e1000_vfta_Register;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda) == 4, "input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda size");
_Static_assert(_Alignof(struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda) == 4, "input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda align");
_Static_assert(offsetof(struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda, _v) == 0, "input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda._v offset");
struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e) == 4, "input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e size");
_Static_assert(_Alignof(struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e) == 4, "input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e align");
_Static_assert(offsetof(struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e, _v) == 0, "input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e._v offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39) == 4, "input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 size");
_Static_assert(_Alignof(struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39) == 4, "input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 align");
_Static_assert(offsetof(struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39, _v) == 0, "input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39._v offset");
struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39) == 8, "input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39 size");
_Static_assert(_Alignof(struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39) == 4, "input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39 align");
_Static_assert(offsetof(struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39, mask) == 0, "input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39.mask offset");
_Static_assert(offsetof(struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39, value) == 4, "input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39.value offset");
struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825) == 4, "input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 size");
_Static_assert(_Alignof(struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825) == 4, "input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 align");
_Static_assert(offsetof(struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825, _v) == 0, "input_ReadWrite__b98bc2a37766ab540cda3dcc89289825._v offset");
struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825) == 8, "input_FieldValue__b98bc2a37766ab540cda3dcc89289825 size");
_Static_assert(_Alignof(struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825) == 4, "input_FieldValue__b98bc2a37766ab540cda3dcc89289825 align");
_Static_assert(offsetof(struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825, mask) == 0, "input_FieldValue__b98bc2a37766ab540cda3dcc89289825.mask offset");
_Static_assert(offsetof(struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825, value) == 4, "input_FieldValue__b98bc2a37766ab540cda3dcc89289825.value offset");
struct input_Field__8cb36d2526ae4a3d9219d36a5993db39 {
    uint32_t mask;
    uint32_t shift;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Field__8cb36d2526ae4a3d9219d36a5993db39) == 8, "input_Field__8cb36d2526ae4a3d9219d36a5993db39 size");
_Static_assert(_Alignof(struct input_Field__8cb36d2526ae4a3d9219d36a5993db39) == 4, "input_Field__8cb36d2526ae4a3d9219d36a5993db39 align");
_Static_assert(offsetof(struct input_Field__8cb36d2526ae4a3d9219d36a5993db39, mask) == 0, "input_Field__8cb36d2526ae4a3d9219d36a5993db39.mask offset");
_Static_assert(offsetof(struct input_Field__8cb36d2526ae4a3d9219d36a5993db39, shift) == 4, "input_Field__8cb36d2526ae4a3d9219d36a5993db39.shift offset");
struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d) == 4, "input_ReadWrite__3ad6381d6292c506df782b501a31e46d size");
_Static_assert(_Alignof(struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d) == 4, "input_ReadWrite__3ad6381d6292c506df782b501a31e46d align");
_Static_assert(offsetof(struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d, _v) == 0, "input_ReadWrite__3ad6381d6292c506df782b501a31e46d._v offset");
struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62) == 4, "input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 size");
_Static_assert(_Alignof(struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62) == 4, "input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 align");
_Static_assert(offsetof(struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62, _v) == 0, "input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62._v offset");
struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6) == 4, "input_ReadWrite__60745c91da21f22bb6110b51da156fa6 size");
_Static_assert(_Alignof(struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6) == 4, "input_ReadWrite__60745c91da21f22bb6110b51da156fa6 align");
_Static_assert(offsetof(struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6, _v) == 0, "input_ReadWrite__60745c91da21f22bb6110b51da156fa6._v offset");
struct input_ReadWrite__787947e178fa2899797b7d93147718a7 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__787947e178fa2899797b7d93147718a7) == 4, "input_ReadWrite__787947e178fa2899797b7d93147718a7 size");
_Static_assert(_Alignof(struct input_ReadWrite__787947e178fa2899797b7d93147718a7) == 4, "input_ReadWrite__787947e178fa2899797b7d93147718a7 align");
_Static_assert(offsetof(struct input_ReadWrite__787947e178fa2899797b7d93147718a7, _v) == 0, "input_ReadWrite__787947e178fa2899797b7d93147718a7._v offset");
struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751) == 4, "input_ReadWrite__9a1c052655932b10bf288f0d224b9751 size");
_Static_assert(_Alignof(struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751) == 4, "input_ReadWrite__9a1c052655932b10bf288f0d224b9751 align");
_Static_assert(offsetof(struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751, _v) == 0, "input_ReadWrite__9a1c052655932b10bf288f0d224b9751._v offset");
struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544) == 4, "input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 size");
_Static_assert(_Alignof(struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544) == 4, "input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 align");
_Static_assert(offsetof(struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544, _v) == 0, "input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544._v offset");
struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e) == 4, "input_ReadWrite__f08033046edb2231a25687e8f4c78a7e size");
_Static_assert(_Alignof(struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e) == 4, "input_ReadWrite__f08033046edb2231a25687e8f4c78a7e align");
_Static_assert(offsetof(struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e, _v) == 0, "input_ReadWrite__f08033046edb2231a25687e8f4c78a7e._v offset");
struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95) == 4, "input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 size");
_Static_assert(_Alignof(struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95) == 4, "input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 align");
_Static_assert(offsetof(struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95, _v) == 0, "input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95._v offset");
struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95) == 8, "input_FieldValue__49fbe09786b1dbd483401b8932a7bc95 size");
_Static_assert(_Alignof(struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95) == 4, "input_FieldValue__49fbe09786b1dbd483401b8932a7bc95 align");
_Static_assert(offsetof(struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95, mask) == 0, "input_FieldValue__49fbe09786b1dbd483401b8932a7bc95.mask offset");
_Static_assert(offsetof(struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95, value) == 4, "input_FieldValue__49fbe09786b1dbd483401b8932a7bc95.value offset");
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c align");
_Static_assert(offsetof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c, _v) == 0, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c._v offset");
struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f) == 4, "input_ReadOnly__a3ee5aac30dfda4669a380944acd83f size");
_Static_assert(_Alignof(struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f) == 4, "input_ReadOnly__a3ee5aac30dfda4669a380944acd83f align");
_Static_assert(offsetof(struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f, _v) == 0, "input_ReadOnly__a3ee5aac30dfda4669a380944acd83f._v offset");
struct input_e1000_RaEntry {
    struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6 ral;
    struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 rah;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_e1000_RaEntry) == 8, "input_e1000_RaEntry size");
_Static_assert(_Alignof(struct input_e1000_RaEntry) == 4, "input_e1000_RaEntry align");
_Static_assert(offsetof(struct input_e1000_RaEntry, ral) == 0, "input_e1000_RaEntry.ral offset");
_Static_assert(offsetof(struct input_e1000_RaEntry, rah) == 4, "input_e1000_RaEntry.rah offset");
struct input_e1000_Regs {
    struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 ctrl;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved0;
    struct input_ReadOnly__a3ee5aac30dfda4669a380944acd83f status;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved1[45];
    struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda icr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved2[5];
    struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e imc;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved3[201];
    struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 tctl;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved4[3327];
    struct input_ReadWrite__787947e178fa2899797b7d93147718a7 tdbal;
    struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 tdbah;
    struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d tdlen;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved5[3];
    struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751 tdt;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved6[1657];
    struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 mta[128];
    struct input_e1000_RaEntry ra[16];
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved7[96];
    struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e vfta[128];
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_e1000_Regs) == 22528, "input_e1000_Regs size");
_Static_assert(_Alignof(struct input_e1000_Regs) == 4, "input_e1000_Regs align");
_Static_assert(offsetof(struct input_e1000_Regs, ctrl) == 0, "input_e1000_Regs.ctrl offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved0) == 4, "input_e1000_Regs._reserved0 offset");
_Static_assert(offsetof(struct input_e1000_Regs, status) == 8, "input_e1000_Regs.status offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved1) == 12, "input_e1000_Regs._reserved1 offset");
_Static_assert(offsetof(struct input_e1000_Regs, icr) == 192, "input_e1000_Regs.icr offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved2) == 196, "input_e1000_Regs._reserved2 offset");
_Static_assert(offsetof(struct input_e1000_Regs, imc) == 216, "input_e1000_Regs.imc offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved3) == 220, "input_e1000_Regs._reserved3 offset");
_Static_assert(offsetof(struct input_e1000_Regs, tctl) == 1024, "input_e1000_Regs.tctl offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved4) == 1028, "input_e1000_Regs._reserved4 offset");
_Static_assert(offsetof(struct input_e1000_Regs, tdbal) == 14336, "input_e1000_Regs.tdbal offset");
_Static_assert(offsetof(struct input_e1000_Regs, tdbah) == 14340, "input_e1000_Regs.tdbah offset");
_Static_assert(offsetof(struct input_e1000_Regs, tdlen) == 14344, "input_e1000_Regs.tdlen offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved5) == 14348, "input_e1000_Regs._reserved5 offset");
_Static_assert(offsetof(struct input_e1000_Regs, tdt) == 14360, "input_e1000_Regs.tdt offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved6) == 14364, "input_e1000_Regs._reserved6 offset");
_Static_assert(offsetof(struct input_e1000_Regs, mta) == 20992, "input_e1000_Regs.mta offset");
_Static_assert(offsetof(struct input_e1000_Regs, ra) == 21504, "input_e1000_Regs.ra offset");
_Static_assert(offsetof(struct input_e1000_Regs, _reserved7) == 21632, "input_e1000_Regs._reserved7 offset");
_Static_assert(offsetof(struct input_e1000_Regs, vfta) == 22016, "input_e1000_Regs.vfta offset");
struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b {
    struct input_e1000_Regs inner;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b) == 22528, "input_Mmio__a66ecc2e422b576eb594c739cf95cc1b size");
_Static_assert(_Alignof(struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b) == 4, "input_Mmio__a66ecc2e422b576eb594c739cf95cc1b align");
_Static_assert(offsetof(struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b, inner) == 0, "input_Mmio__a66ecc2e422b576eb594c739cf95cc1b.inner offset");
struct input_E1000Composed {
    struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b regs;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_E1000Composed) == 22528, "input_E1000Composed size");
_Static_assert(_Alignof(struct input_E1000Composed) == 4, "input_E1000Composed align");
_Static_assert(offsetof(struct input_E1000Composed, regs) == 0, "input_E1000Composed.regs offset");
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
struct core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a) == 0, "core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a size");
_Static_assert(_Alignof(struct core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a) == 1, "core_marker_PhantomData__34621ca40a920e21f03daf4c5070e1a align");
struct core_marker_PhantomData__2461faaae176aa23b8290043178aca96 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__2461faaae176aa23b8290043178aca96) == 0, "core_marker_PhantomData__2461faaae176aa23b8290043178aca96 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__2461faaae176aa23b8290043178aca96) == 1, "core_marker_PhantomData__2461faaae176aa23b8290043178aca96 align");
struct core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511) == 0, "core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511) == 1, "core_marker_PhantomData__29bfcc39c6a73dee049083b8096c5511 align");
struct core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577) == 0, "core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577) == 1, "core_marker_PhantomData__43d72caa26af2374830f3ef9c9e85577 align");
struct core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad) == 0, "core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad size");
_Static_assert(_Alignof(struct core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad) == 1, "core_marker_PhantomData__78ddfaa35887d144b543dbdee0e8e2ad align");
struct core_marker_PhantomData__8023637d63cff66abf14b05094f293b7 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__8023637d63cff66abf14b05094f293b7) == 0, "core_marker_PhantomData__8023637d63cff66abf14b05094f293b7 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__8023637d63cff66abf14b05094f293b7) == 1, "core_marker_PhantomData__8023637d63cff66abf14b05094f293b7 align");
struct core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de) == 0, "core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de size");
_Static_assert(_Alignof(struct core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de) == 1, "core_marker_PhantomData__8144feb433f7f4b2fc6d627a7f3da0de align");
struct core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb) == 0, "core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb size");
_Static_assert(_Alignof(struct core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb) == 1, "core_marker_PhantomData__96dd981e16e34900da6abf0039abc3cb align");
struct core_marker_PhantomData__b62b53c72509c28288b0363e59aea163 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__b62b53c72509c28288b0363e59aea163) == 0, "core_marker_PhantomData__b62b53c72509c28288b0363e59aea163 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__b62b53c72509c28288b0363e59aea163) == 1, "core_marker_PhantomData__b62b53c72509c28288b0363e59aea163 align");
struct core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb) == 0, "core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb size");
_Static_assert(_Alignof(struct core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb) == 1, "core_marker_PhantomData__ccc0494812e86fc608a0c3edc4c1b9eb align");
struct core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020) == 0, "core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020) == 1, "core_marker_PhantomData__ce30978b4a7677c21bbe294c3b89f020 align");
struct core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d) == 0, "core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d size");
_Static_assert(_Alignof(struct core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d) == 1, "core_marker_PhantomData__d7a7849c240c653bf6de66e5fbf4a13d align");
struct core_marker_PhantomData__e9d6bb486817af9131266cec6067699a {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__e9d6bb486817af9131266cec6067699a) == 0, "core_marker_PhantomData__e9d6bb486817af9131266cec6067699a size");
_Static_assert(_Alignof(struct core_marker_PhantomData__e9d6bb486817af9131266cec6067699a) == 1, "core_marker_PhantomData__e9d6bb486817af9131266cec6067699a align");
struct input_e1000_ctrl_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_ctrl_Register) == 0, "input_e1000_ctrl_Register size");
_Static_assert(_Alignof(struct input_e1000_ctrl_Register) == 1, "input_e1000_ctrl_Register align");
struct input_e1000_icr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_icr_Register) == 0, "input_e1000_icr_Register size");
_Static_assert(_Alignof(struct input_e1000_icr_Register) == 1, "input_e1000_icr_Register align");
struct input_e1000_imc_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_imc_Register) == 0, "input_e1000_imc_Register size");
_Static_assert(_Alignof(struct input_e1000_imc_Register) == 1, "input_e1000_imc_Register align");
struct input_e1000_mta_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_mta_Register) == 0, "input_e1000_mta_Register size");
_Static_assert(_Alignof(struct input_e1000_mta_Register) == 1, "input_e1000_mta_Register align");
struct input_e1000_ra_rah_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_ra_rah_Register) == 0, "input_e1000_ra_rah_Register size");
_Static_assert(_Alignof(struct input_e1000_ra_rah_Register) == 1, "input_e1000_ra_rah_Register align");
struct input_e1000_ra_ral_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_ra_ral_Register) == 0, "input_e1000_ra_ral_Register size");
_Static_assert(_Alignof(struct input_e1000_ra_ral_Register) == 1, "input_e1000_ra_ral_Register align");
struct input_e1000_status_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_status_Register) == 0, "input_e1000_status_Register size");
_Static_assert(_Alignof(struct input_e1000_status_Register) == 1, "input_e1000_status_Register align");
struct input_e1000_tctl_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_tctl_Register) == 0, "input_e1000_tctl_Register size");
_Static_assert(_Alignof(struct input_e1000_tctl_Register) == 1, "input_e1000_tctl_Register align");
struct input_e1000_tdbah_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_tdbah_Register) == 0, "input_e1000_tdbah_Register size");
_Static_assert(_Alignof(struct input_e1000_tdbah_Register) == 1, "input_e1000_tdbah_Register align");
struct input_e1000_tdbal_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_tdbal_Register) == 0, "input_e1000_tdbal_Register size");
_Static_assert(_Alignof(struct input_e1000_tdbal_Register) == 1, "input_e1000_tdbal_Register align");
struct input_e1000_tdlen_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_tdlen_Register) == 0, "input_e1000_tdlen_Register size");
_Static_assert(_Alignof(struct input_e1000_tdlen_Register) == 1, "input_e1000_tdlen_Register align");
struct input_e1000_tdt_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_tdt_Register) == 0, "input_e1000_tdt_Register size");
_Static_assert(_Alignof(struct input_e1000_tdt_Register) == 1, "input_e1000_tdt_Register align");
struct input_e1000_vfta_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_e1000_vfta_Register) == 0, "input_e1000_vfta_Register size");
_Static_assert(_Alignof(struct input_e1000_vfta_Register) == 1, "input_e1000_vfta_Register align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) uint32_t input__impl_2__read__b4a4782cc7c031fc977f9095aa75bfda(const struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda *);
__attribute__((noinline)) void input__impl_3__write__744f5cbde383cce61f4b1a0d77a23e4e(const struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e *, uint32_t);
__attribute__((noinline)) void input__impl_4__modify__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *, struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39);
__attribute__((noinline)) void input__impl_4__modify__b98bc2a37766ab540cda3dcc89289825(const struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 *, struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825);
__attribute__((noinline)) uint32_t input__impl_4__read_field__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *, struct input_Field__8cb36d2526ae4a3d9219d36a5993db39);
__attribute__((noinline)) void input__impl_4__write__3ad6381d6292c506df782b501a31e46d(const struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__5d8a36544fc755f42ec2440b4c151b62(const struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__60745c91da21f22bb6110b51da156fa6(const struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__787947e178fa2899797b7d93147718a7(const struct input_ReadWrite__787947e178fa2899797b7d93147718a7 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__9a1c052655932b10bf288f0d224b9751(const struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__9a9b5c3551d347b9ca1feaac6cc5c544(const struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 *, uint32_t);
__attribute__((noinline)) void input__impl_4__write__f08033046edb2231a25687e8f4c78a7e(const struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e *, uint32_t);
__attribute__((noinline)) void input__impl_4__write_field__49fbe09786b1dbd483401b8932a7bc95(const struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 *, struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95);
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_8__new(void);
const struct input_e1000_Regs *input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *);
void input__impl_11__program_tx_ring(struct input_E1000Composed *);
void input__impl_11__setup_filters(struct input_E1000Composed *, size_t, size_t, size_t);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_deinit(struct input_E1000Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_init(struct input_E1000Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_init_failure(struct input_E1000Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_surprise_remove(struct input_E1000Composed *);
__attribute__((noinline)) uint32_t input_regseq_bus_read_config__5ae2a5e07d9ec8b0eb418e8c3168f23c(uint32_t);
__attribute__((noinline)) uint16_t input_regseq_bus_read_config__72fa136db03f3626686211786dc3ffbc(uint32_t);
__attribute__((noinline)) uint8_t input_regseq_bus_read_config__c6e1a215476aa64c90d09f7c790ac72c(uint32_t);
__attribute__((noinline)) void input_regseq_bus_write_config__c6e1a215476aa64c90d09f7c790ac72c(uint32_t, uint8_t);
extern const uint8_t input__regseq_synth__const_memory_static_2121617241c70533c76e46f6b4b3004b[17];
extern const uint8_t input__regseq_synth__const_memory_static_25a04775023d8258409e6b57375668ea[31];
extern const uint8_t input__regseq_synth__const_memory_static_6bb28dd46b219f42e2f9bfc11322151e[44];
extern const uint8_t input__regseq_synth__const_memory_static_9088539bce4315e07ff159990a895e5a[32];
extern const uint8_t input__regseq_synth__const_memory_static_b0f2268669ba2d13f75eb33646083dac[56];
extern const uint8_t input__regseq_synth__const_memory_static_b1b2745428e81c21a87f2885e7b12e74[39];
extern const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0];
extern const uint8_t input__regseq_synth__const_memory_static_e89c3a9a7b5d976a65db04dbf7355e53[8];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) uint32_t input__impl_2__read__b4a4782cc7c031fc977f9095aa75bfda(const struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_3__write__744f5cbde383cce61f4b1a0d77a23e4e(const struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__modify__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_1, struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__modify__b98bc2a37766ab540cda3dcc89289825(const struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 *_1, struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) uint32_t input__impl_4__read_field__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_1, struct input_Field__8cb36d2526ae4a3d9219d36a5993db39 _2) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_4__write__3ad6381d6292c506df782b501a31e46d(const struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__5d8a36544fc755f42ec2440b4c151b62(const struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__60745c91da21f22bb6110b51da156fa6(const struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__787947e178fa2899797b7d93147718a7(const struct input_ReadWrite__787947e178fa2899797b7d93147718a7 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__8cb36d2526ae4a3d9219d36a5993db39(const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__9a1c052655932b10bf288f0d224b9751(const struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__9a9b5c3551d347b9ca1feaac6cc5c544(const struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write__f08033046edb2231a25687e8f4c78a7e(const struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_4__write_field__49fbe09786b1dbd483401b8932a7bc95(const struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 *_1, struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_8__new(void) {
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _0;
    bb_0: {
        _0 = (struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c){ ._v = ((uint32_t)(0U)) };
        return _0;
    }
}
const struct input_e1000_Regs *input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_1) {
    const struct input_e1000_Regs *_0;
    const struct input_e1000_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
void input__impl_11__program_tx_ring(struct input_E1000Composed *_1) {
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_2;
    const struct input_e1000_Regs *_3;
    const struct input_ReadWrite__9a9b5c3551d347b9ca1feaac6cc5c544 *_4;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_6;
    const struct input_e1000_Regs *_7;
    const struct input_ReadWrite__787947e178fa2899797b7d93147718a7 *_8;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_10;
    const struct input_e1000_Regs *_11;
    const struct input_ReadWrite__3ad6381d6292c506df782b501a31e46d *_12;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).tdbah);
        regseq_bus_mmio_write(14340, 4, (uint64_t)(((uint32_t)(1U)))); goto bb_2;
    }
    bb_2: {
        _6 = &((*_1).regs);
        _7 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_6); goto bb_3;
    }
    bb_3: {
        _8 = &((*_7).tdbal);
        regseq_bus_mmio_write(14336, 4, (uint64_t)(((uint32_t)(8192U)))); goto bb_4;
    }
    bb_4: {
        _10 = &((*_1).regs);
        _11 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_10); goto bb_5;
    }
    bb_5: {
        _12 = &((*_11).tdlen);
        regseq_bus_mmio_write(14344, 4, (uint64_t)(((uint32_t)(128U)))); goto bb_6;
    }
    bb_6: {
        return;
    }
}
void input__impl_11__setup_filters(struct input_E1000Composed *_1, size_t _2, size_t _3, size_t _4) {
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_5;
    const struct input_e1000_Regs *_6;
    size_t _7;
    _Bool _8;
    const struct input_ReadWrite__5d8a36544fc755f42ec2440b4c151b62 *_9;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_11;
    const struct input_e1000_Regs *_12;
    size_t _13;
    _Bool _14;
    const struct input_ReadWrite__f08033046edb2231a25687e8f4c78a7e *_15;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_17;
    const struct input_e1000_Regs *_18;
    size_t _19;
    _Bool _20;
    const struct input_ReadWrite__60745c91da21f22bb6110b51da156fa6 *_21;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_23;
    const struct input_e1000_Regs *_24;
    size_t _25;
    _Bool _26;
    const struct input_ReadWrite__49fbe09786b1dbd483401b8932a7bc95 *_27;
    bb_0: {
        _5 = &((*_1).regs);
        _6 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_5); goto bb_1;
    }
    bb_1: {
        _7 = _2;
        _8 = ((_7) < (((size_t)(128ULL))));
        if ((_8) == 1) goto bb_2; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(128ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_7)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_2: {
        _9 = &((*_6).mta[_7]);
        regseq_bus_mmio_write((20992 + (uint64_t)(_7) * 4), 4, (uint64_t)(((uint32_t)(0U)))); goto bb_3;
    }
    bb_3: {
        _11 = &((*_1).regs);
        _12 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_11); goto bb_4;
    }
    bb_4: {
        _13 = _3;
        _14 = ((_13) < (((size_t)(128ULL))));
        if ((_14) == 1) goto bb_5; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(128ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_13)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_5: {
        _15 = &((*_12).vfta[_13]);
        regseq_bus_mmio_write((22016 + (uint64_t)(_13) * 4), 4, (uint64_t)(((uint32_t)(0U)))); goto bb_6;
    }
    bb_6: {
        _17 = &((*_1).regs);
        _18 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_17); goto bb_7;
    }
    bb_7: {
        _19 = _4;
        _20 = ((_19) < (((size_t)(16ULL))));
        if ((_20) == 1) goto bb_8; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(16ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_19)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_8: {
        _21 = &((*_18).ra[_19].ral);
        regseq_bus_mmio_write((21504 + (uint64_t)(_19) * 8), 4, (uint64_t)(((uint32_t)(2864434397U)))); goto bb_9;
    }
    bb_9: {
        _23 = &((*_1).regs);
        _24 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_23); goto bb_10;
    }
    bb_10: {
        _25 = _4;
        _26 = ((_25) < (((size_t)(16ULL))));
        if ((_26) == 1) goto bb_11; else { { char _b0[40]; snprintf(_b0, sizeof _b0, "%lld", (long long)(((size_t)(16ULL)))); char _b1[40]; snprintf(_b1, sizeof _b1, "%zu", (size_t)(_25)); char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_BOUNDS_CHECK, _b0, _b1); regseq_panic(_m); } }
    }
    bb_11: {
        _27 = &((*_24).ra[_25].rah);
        regseq_bus_mmio_write((21508 + (uint64_t)(_25) * 8), 4, (uint64_t)(((struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95){ .mask = ((uint32_t)(2147483648U)), .value = ((uint32_t)(2147483648U)) }).value & ((struct input_FieldValue__49fbe09786b1dbd483401b8932a7bc95){ .mask = ((uint32_t)(2147483648U)), .value = ((uint32_t)(2147483648U)) }).mask)); goto bb_12;
    }
    bb_12: {
        return;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_deinit(struct input_E1000Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_7;
    const struct input_e1000_Regs *_8;
    const struct input_WriteOnly__744f5cbde383cce61f4b1a0d77a23e4e *_9;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_11;
    const struct input_e1000_Regs *_12;
    const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_13;
    bb_0: {
        fwrite("[WARN] e1000 deinit: masking interrupts\n", 1, 40, stdout); goto bb_1;
    }
    bb_1: {
        _7 = &((*_1).regs);
        _8 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_7); goto bb_2;
    }
    bb_2: {
        _9 = &((*_8).imc);
        regseq_bus_mmio_write(216, 4, (uint64_t)(((uint32_t)(4294967295U)))); goto bb_3;
    }
    bb_3: {
        _11 = &((*_1).regs);
        _12 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_11); goto bb_4;
    }
    bb_4: {
        _13 = &((*_12).tctl);
        regseq_bus_mmio_write(1024, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_5;
    }
    bb_5: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_init(struct input_E1000Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    uint16_t _7;
    uint16_t _8;
    uint8_t _9;
    uint32_t _10;
    uint16_t _11;
    uint16_t _13;
    uint8_t _15;
    uint32_t _17;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_25;
    const struct input_e1000_Regs *_26;
    const struct input_ReadWrite__b98bc2a37766ab540cda3dcc89289825 *_27;
    struct input_E1000Composed *_29;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_31;
    const struct input_e1000_Regs *_32;
    const struct input_ReadWrite__9a1c052655932b10bf288f0d224b9751 *_33;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_35;
    const struct input_e1000_Regs *_36;
    const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_37;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_39;
    const struct input_e1000_Regs *_40;
    const struct input_ReadWrite__8cb36d2526ae4a3d9219d36a5993db39 *_41;
    uint32_t _42;
    _Bool _43;
    struct input_E1000Composed *_44;
    const struct input_Mmio__a66ecc2e422b576eb594c739cf95cc1b *_46;
    const struct input_e1000_Regs *_47;
    const struct input_ReadOnly__b4a4782cc7c031fc977f9095aa75bfda *_48;
    uint32_t _49;
    struct input_RegseqError _60;
    bb_0: {
        fwrite("[DEBUG] e1000 probe start\n", 1, 26, stdout); goto bb_1;
    }
    bb_1: {
        _7 = (uint16_t)regseq_bus_on_config_read((uint32_t)(((uint32_t)(0U))), 2); goto bb_2;
    }
    bb_2: {
        _8 = (uint16_t)regseq_bus_on_config_read((uint32_t)(((uint32_t)(2U))), 2); goto bb_3;
    }
    bb_3: {
        _9 = (uint8_t)regseq_bus_on_config_read((uint32_t)(((uint32_t)(8U))), 1); goto bb_4;
    }
    bb_4: {
        _10 = (uint32_t)regseq_bus_on_config_read((uint32_t)(((uint32_t)(16U))), 4); goto bb_5;
    }
    bb_5: {
        _11 = _7;
        goto bb_6;
    }
    bb_6: {
        _13 = _8;
        goto bb_7;
    }
    bb_7: {
        _15 = _9;
        goto bb_8;
    }
    bb_8: {
        _17 = _10;
        goto bb_9;
    }
    bb_9: {
        fwrite("[INFO] e1000 cfg ", 1, 17, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#x", (unsigned)(_11)); fwrite(_n, 1, _l, stdout); } fwrite(":", 1, 1, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#x", (unsigned)(_13)); fwrite(_n, 1, _l, stdout); } fwrite(" rev ", 1, 5, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_15)); fwrite(_n, 1, _l, stdout); } fwrite(" bar0 ", 1, 6, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%#x", (unsigned)(_17)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_10;
    }
    bb_10: {
        regseq_bus_on_config_write((uint32_t)(((uint32_t)(12U))), 1, (uint64_t)(((uint8_t)(16U)))); goto bb_11;
    }
    bb_11: {
        _25 = &((*_1).regs);
        _26 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_25); goto bb_12;
    }
    bb_12: {
        _27 = &((*_26).ctrl);
        { uint64_t _rmw_cur = regseq_bus_mmio_read(0, 4); regseq_bus_mmio_write(0, 4, (_rmw_cur & ~(uint64_t)(((struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825){ .mask = ((uint32_t)(64U)), .value = ((uint32_t)(64U)) }).mask)) | (uint64_t)(((struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825){ .mask = ((uint32_t)(64U)), .value = ((uint32_t)(64U)) }).value & ((struct input_FieldValue__b98bc2a37766ab540cda3dcc89289825){ .mask = ((uint32_t)(64U)), .value = ((uint32_t)(64U)) }).mask)); } goto bb_13;
    }
    bb_13: {
        _29 = &((*_1));
        input__impl_11__program_tx_ring(_29); goto bb_14;
    }
    bb_14: {
        _31 = &((*_1).regs);
        _32 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_31); goto bb_15;
    }
    bb_15: {
        _33 = &((*_32).tdt);
        regseq_bus_mmio_write(14360, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_16;
    }
    bb_16: {
        _35 = &((*_1).regs);
        _36 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_35); goto bb_17;
    }
    bb_17: {
        _37 = &((*_36).tctl);
        { uint64_t _rmw_cur = regseq_bus_mmio_read(1024, 4); regseq_bus_mmio_write(1024, 4, (_rmw_cur & ~(uint64_t)(((struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39){ .mask = ((uint32_t)(2U)), .value = ((uint32_t)(2U)) }).mask)) | (uint64_t)(((struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39){ .mask = ((uint32_t)(2U)), .value = ((uint32_t)(2U)) }).value & ((struct input_FieldValue__8cb36d2526ae4a3d9219d36a5993db39){ .mask = ((uint32_t)(2U)), .value = ((uint32_t)(2U)) }).mask)); } goto bb_18;
    }
    bb_18: {
        _39 = &((*_1).regs);
        _40 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_39); goto bb_19;
    }
    bb_19: {
        _41 = &((*_40).tctl);
        _42 = (uint32_t)((regseq_bus_mmio_read(1024, 4) & ((struct input_Field__8cb36d2526ae4a3d9219d36a5993db39){ .mask = ((uint32_t)(2U)), .shift = ((uint32_t)(1U)) }).mask) >> ((struct input_Field__8cb36d2526ae4a3d9219d36a5993db39){ .mask = ((uint32_t)(2U)), .shift = ((uint32_t)(1U)) }).shift); goto bb_20;
    }
    bb_20: {
        _43 = ((_42) == (((uint32_t)(0U))));
        switch (_43) { case 0ULL: goto bb_23; default: goto bb_21; }
    }
    bb_21: {
        fwrite("[ERROR] e1000 tx enable readback failed\n", 1, 40, stdout); goto bb_22;
    }
    bb_22: {
        _60.tag = ((uint8_t)(0U));
        _0._0 = _60;
        goto bb_28;
    }
    bb_23: {
        _44 = &((*_1));
        input__impl_11__setup_filters(_44, ((size_t)(3ULL)), ((size_t)(5ULL)), ((size_t)(1ULL))); goto bb_24;
    }
    bb_24: {
        _46 = &((*_1).regs);
        _47 = input__impl_10__deref__a66ecc2e422b576eb594c739cf95cc1b(_46); goto bb_25;
    }
    bb_25: {
        _48 = &((*_47).icr);
        _49 = (uint32_t)regseq_bus_mmio_read(192, 4); goto bb_26;
    }
    bb_26: {
        fwrite("[INFO] e1000 up\n", 1, 16, stdout); goto bb_27;
    }
    bb_27: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_28;
    }
    bb_28: {
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_init_failure(struct input_E1000Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        fwrite("[ERROR] e1000 init failed: cleaning up partial state\n", 1, 53, stdout); goto bb_1;
    }
    bb_1: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_12__on_surprise_remove(struct input_E1000Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        fwrite("[ERROR] e1000 surprise remove: device gone, skipping hw teardown\n", 1, 65, stdout); goto bb_1;
    }
    bb_1: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
__attribute__((noinline)) uint32_t input_regseq_bus_read_config__5ae2a5e07d9ec8b0eb418e8c3168f23c(uint32_t _1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) uint16_t input_regseq_bus_read_config__72fa136db03f3626686211786dc3ffbc(uint32_t _1) {
    uint16_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) uint8_t input_regseq_bus_read_config__c6e1a215476aa64c90d09f7c790ac72c(uint32_t _1) {
    uint8_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input_regseq_bus_write_config__c6e1a215476aa64c90d09f7c790ac72c(uint32_t _1, uint8_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_2121617241c70533c76e46f6b4b3004b[17] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(111U)), ((uint8_t)(98U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(116U)) };
const uint8_t input__regseq_synth__const_memory_static_25a04775023d8258409e6b57375668ea[31] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(120U)), ((uint8_t)(32U)), ((uint8_t)(101U)), ((uint8_t)(110U)), ((uint8_t)(97U)), ((uint8_t)(98U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(100U)), ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(99U)), ((uint8_t)(107U)), ((uint8_t)(32U)), ((uint8_t)(102U)), ((uint8_t)(97U)), ((uint8_t)(105U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(100U)) };
const uint8_t input__regseq_synth__const_memory_static_6bb28dd46b219f42e2f9bfc11322151e[44] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(32U)), ((uint8_t)(102U)), ((uint8_t)(97U)), ((uint8_t)(105U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(100U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(99U)), ((uint8_t)(108U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(110U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(103U)), ((uint8_t)(32U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(116U)), ((uint8_t)(105U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(116U)), ((uint8_t)(101U)) };
const uint8_t input__regseq_synth__const_memory_static_9088539bce4315e07ff159990a895e5a[32] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(109U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(107U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(103U)), ((uint8_t)(32U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(114U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(116U)), ((uint8_t)(115U)) };
const uint8_t input__regseq_synth__const_memory_static_b0f2268669ba2d13f75eb33646083dac[56] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(117U)), ((uint8_t)(114U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(105U)), ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(109U)), ((uint8_t)(111U)), ((uint8_t)(118U)), ((uint8_t)(101U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(118U)), ((uint8_t)(105U)), ((uint8_t)(99U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(103U)), ((uint8_t)(111U)), ((uint8_t)(110U)), ((uint8_t)(101U)), ((uint8_t)(44U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(107U)), ((uint8_t)(105U)), ((uint8_t)(112U)), ((uint8_t)(112U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(103U)), ((uint8_t)(32U)), ((uint8_t)(104U)), ((uint8_t)(119U)), ((uint8_t)(32U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(100U)), ((uint8_t)(111U)), ((uint8_t)(119U)), ((uint8_t)(110U)) };
const uint8_t input__regseq_synth__const_memory_static_b1b2745428e81c21a87f2885e7b12e74[39] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(99U)), ((uint8_t)(102U)), ((uint8_t)(103U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(58U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(120U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(114U)), ((uint8_t)(101U)), ((uint8_t)(118U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(125U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(123U)), ((uint8_t)(58U)), ((uint8_t)(35U)), ((uint8_t)(120U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0] = {0};
const uint8_t input__regseq_synth__const_memory_static_e89c3a9a7b5d976a65db04dbf7355e53[8] = { ((uint8_t)(101U)), ((uint8_t)(49U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(48U)), ((uint8_t)(32U)), ((uint8_t)(117U)), ((uint8_t)(112U)) };
struct __rseq_lc_0_e1000_target {
    void *__rseq_dev;
    struct input_E1000Composed inner;
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
    input__impl_12__on_init(&inst.inner);
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    input__impl_12__on_deinit(&inst.inner);
    regseq_lc_postamble("regs", 0);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_e1000_orderly();
}
