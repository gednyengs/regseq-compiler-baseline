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
/* regseq target platform: os=linux board=qemu-virt arch=aarch64 */

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
 * Device-interrupt host line model (construct #6). Each hook prints an
 * "IRQ <event>" line to stdout -- this trace IS the behavioral oracle for the
 * interrupt placement contract (C01). Emitted only for a TU with Interrupt
 * resources. regseq_irq_fire is SYNCHRONOUS (D07): the harness calls it
 * between scenario steps; it invokes the registered wrapper inline when the
 * line is enabled and traces "IRQ fire line=<n> inert" otherwise (O1). */
#define REGSEQ_IRQ_SLOTS 8u
typedef struct {
    uint32_t line;
    void (*wrapper)(void *);
    void *ctx;
    int state; /* 0 = unregistered, 1 = registered (line DISABLED, D02), 2 = enabled */
} regseq_irq_slot_t;
static regseq_irq_slot_t regseq_irq_slot_table[REGSEQ_IRQ_SLOTS] __attribute__((unused));
__attribute__((unused)) static regseq_irq_slot_t *regseq_irq_slot(uint32_t line) {
    for (uint32_t i = 0; i < REGSEQ_IRQ_SLOTS; i++) {
        if (regseq_irq_slot_table[i].state != 0 && regseq_irq_slot_table[i].line == line) {
            return &regseq_irq_slot_table[i];
        }
    }
    return 0;
}
__attribute__((unused)) static void regseq_irq_register(uint32_t line, void (*wrapper)(void *), void *ctx) {
    printf("IRQ register line=%u\n", (unsigned)line);
    regseq_irq_slot_t *slot = regseq_irq_slot(line);
    if (!slot) {
        for (uint32_t i = 0; i < REGSEQ_IRQ_SLOTS; i++) {
            if (regseq_irq_slot_table[i].state == 0) {
                slot = &regseq_irq_slot_table[i];
                break;
            }
        }
    }
    if (slot) {
        slot->line = line;
        slot->wrapper = wrapper;
        slot->ctx = ctx;
        slot->state = 1; /* registered with the line DISABLED (D02) */
    }
}
__attribute__((unused)) static void regseq_irq_enable(uint32_t line) {
    printf("IRQ enable line=%u\n", (unsigned)line);
    regseq_irq_slot_t *slot = regseq_irq_slot(line);
    if (slot) slot->state = 2;
}
__attribute__((unused)) static void regseq_irq_disable(uint32_t line) {
    printf("IRQ disable line=%u\n", (unsigned)line);
    regseq_irq_slot_t *slot = regseq_irq_slot(line);
    if (slot) slot->state = 1;
}
__attribute__((unused)) static void regseq_irq_unregister(uint32_t line) {
    printf("IRQ unregister line=%u\n", (unsigned)line);
    regseq_irq_slot_t *slot = regseq_irq_slot(line);
    if (slot) slot->state = 0;
}
__attribute__((unused)) static void regseq_irq_fire(uint32_t line) {
    regseq_irq_slot_t *slot = regseq_irq_slot(line);
    if (!slot || slot->state != 2) {
        printf("IRQ fire line=%u inert\n", (unsigned)line);
        return;
    }
    printf("IRQ fire line=%u\n", (unsigned)line);
    slot->wrapper(slot->ctx);
}
/* The bound handler's dispatch result, traced verbatim (C04). */
__attribute__((unused)) static void regseq_irq_result(const char *r) {
    printf("IRQ result=%s\n", r);
}
/* The resolved [interrupt.<id>] facts, traced verbatim -- validation/trace
 * inputs only, never flag semantics (D10/R12). "-" = unspecified trigger;
 * priority -1 = unspecified. */
__attribute__((unused)) static void regseq_irq_binding(uint32_t line, const char *trigger_type,
                                                       const char *trigger_polarity, int shared,
                                                       long long priority) {
    printf("IRQ binding line=%u trigger_type=%s trigger_polarity=%s shared=%d priority=%lld\n",
           (unsigned)line, trigger_type, trigger_polarity, shared, priority);
}
/* Harness device-model poke (a scenario step): latch a cause value into the
 * host regfile so the next fire's ISR observes it. Traced so the golden
 * records causality; NOT an MMIO access (no R/W line -- the ISR's own reads
 * trace through the register-access hooks). */
__attribute__((unused)) static void regseq_irq_dev_poke(uint32_t off, uint32_t val) {
    printf("IRQ poke @0x%03x = 0x%08x\n", (unsigned)off, (unsigned)val);
    for (uint32_t i = 0; i < 4u && (off + i) < REGSEQ_REGFILE_BYTES; i++) {
        regseq_regfile[off + i] = (uint8_t)(val >> (8u * i));
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
struct input_SerialPl011;
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c;
struct input_pl011_Regs;
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e;
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a;
struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228;
struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494;
struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685;
struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83;
struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f;
struct input_ReadWrite__d743656c529b08e42785b912ffb751b0;
struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba;
struct input_WriteOnly__39471f0616179e541513e5a50bf71559;
struct tuple_cbf29ce484222325;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct input_ReadyDir;
struct input_RxDatum;
struct input_RxError;
struct input_PortScope;
struct input_PrevState;
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83;
struct input_LineFormat;
struct input_DataBits;
struct input_Parity;
struct input_StopBits;
struct input_ConfigResult;
struct input_CauseSet;
struct input_SerialIrqSource;
struct input_IrqSourceState;
struct input_IrqResult;
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818;
struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62;
struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f;
struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17;
struct core_marker_PhantomData__5ec21ccfd9c212e921542115e3119b59;
struct core_marker_PhantomData__9a9d8cfaa0cf152914f8e40836b690c7;
struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3;
struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd;
struct core_marker_PhantomData__c6f124df7d299765b3412359240618e6;
struct core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3;
struct input_Interrupt;
struct input_pl011_cr_Register;
struct input_pl011_dr_Register;
struct input_pl011_fbrd_Register;
struct input_pl011_fr_Register;
struct input_pl011_ibrd_Register;
struct input_pl011_icr_Register;
struct input_pl011_ifls_Register;
struct input_pl011_imsc_Register;
struct input_pl011_lcr_h_Register;
struct input_pl011_mis_Register;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e size");
_Static_assert(_Alignof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e align");
_Static_assert(offsetof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e, _v) == 0, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e._v offset");
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c size");
_Static_assert(_Alignof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c) == 4, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c align");
_Static_assert(offsetof(struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c, _v) == 0, "input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c._v offset");
struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a) == 4, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a size");
_Static_assert(_Alignof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a) == 4, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a align");
_Static_assert(offsetof(struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a, _v) == 0, "input_ReadOnly__dcf610a28ad7155fe10496944bda173a._v offset");
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
struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f) == 4, "input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f size");
_Static_assert(_Alignof(struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f) == 4, "input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f align");
_Static_assert(offsetof(struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f, _v) == 0, "input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f._v offset");
struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0) == 4, "input_ReadWrite__d743656c529b08e42785b912ffb751b0 size");
_Static_assert(_Alignof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0) == 4, "input_ReadWrite__d743656c529b08e42785b912ffb751b0 align");
_Static_assert(offsetof(struct input_ReadWrite__d743656c529b08e42785b912ffb751b0, _v) == 0, "input_ReadWrite__d743656c529b08e42785b912ffb751b0._v offset");
struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba) == 4, "input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba size");
_Static_assert(_Alignof(struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba) == 4, "input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba align");
_Static_assert(offsetof(struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba, _v) == 0, "input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba._v offset");
struct input_WriteOnly__39471f0616179e541513e5a50bf71559 {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559) == 4, "input_WriteOnly__39471f0616179e541513e5a50bf71559 size");
_Static_assert(_Alignof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559) == 4, "input_WriteOnly__39471f0616179e541513e5a50bf71559 align");
_Static_assert(offsetof(struct input_WriteOnly__39471f0616179e541513e5a50bf71559, _v) == 0, "input_WriteOnly__39471f0616179e541513e5a50bf71559._v offset");
struct input_pl011_Regs {
    struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e dr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c rsr_ecr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved0[4];
    struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a fr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved1[2];
    struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 ibrd;
    struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 fbrd;
    struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 lcr_h;
    struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 cr;
    struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f ifls;
    struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 imsc;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c ris;
    struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba mis;
    struct input_WriteOnly__39471f0616179e541513e5a50bf71559 icr;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_pl011_Regs) == 72, "input_pl011_Regs size");
_Static_assert(_Alignof(struct input_pl011_Regs) == 4, "input_pl011_Regs align");
_Static_assert(offsetof(struct input_pl011_Regs, dr) == 0, "input_pl011_Regs.dr offset");
_Static_assert(offsetof(struct input_pl011_Regs, rsr_ecr) == 4, "input_pl011_Regs.rsr_ecr offset");
_Static_assert(offsetof(struct input_pl011_Regs, _reserved0) == 8, "input_pl011_Regs._reserved0 offset");
_Static_assert(offsetof(struct input_pl011_Regs, fr) == 24, "input_pl011_Regs.fr offset");
_Static_assert(offsetof(struct input_pl011_Regs, _reserved1) == 28, "input_pl011_Regs._reserved1 offset");
_Static_assert(offsetof(struct input_pl011_Regs, ibrd) == 36, "input_pl011_Regs.ibrd offset");
_Static_assert(offsetof(struct input_pl011_Regs, fbrd) == 40, "input_pl011_Regs.fbrd offset");
_Static_assert(offsetof(struct input_pl011_Regs, lcr_h) == 44, "input_pl011_Regs.lcr_h offset");
_Static_assert(offsetof(struct input_pl011_Regs, cr) == 48, "input_pl011_Regs.cr offset");
_Static_assert(offsetof(struct input_pl011_Regs, ifls) == 52, "input_pl011_Regs.ifls offset");
_Static_assert(offsetof(struct input_pl011_Regs, imsc) == 56, "input_pl011_Regs.imsc offset");
_Static_assert(offsetof(struct input_pl011_Regs, ris) == 60, "input_pl011_Regs.ris offset");
_Static_assert(offsetof(struct input_pl011_Regs, mis) == 64, "input_pl011_Regs.mis offset");
_Static_assert(offsetof(struct input_pl011_Regs, icr) == 68, "input_pl011_Regs.icr offset");
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c {
    struct input_pl011_Regs inner;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 72, "input_Mmio__b43d312b4ad71a71283184a14b095a0c size");
_Static_assert(_Alignof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 4, "input_Mmio__b43d312b4ad71a71283184a14b095a0c align");
_Static_assert(offsetof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c, inner) == 0, "input_Mmio__b43d312b4ad71a71283184a14b095a0c.inner offset");
struct input_SerialPl011 {
    struct input_Mmio__b43d312b4ad71a71283184a14b095a0c regs;
    uint32_t imsc_shadow;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_SerialPl011) == 76, "input_SerialPl011 size");
_Static_assert(_Alignof(struct input_SerialPl011) == 4, "input_SerialPl011 align");
_Static_assert(offsetof(struct input_SerialPl011, regs) == 0, "input_SerialPl011.regs offset");
_Static_assert(offsetof(struct input_SerialPl011, imsc_shadow) == 72, "input_SerialPl011.imsc_shadow offset");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
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
struct input_ReadyDir {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_ReadyDir) == 1, "input_ReadyDir size");
_Static_assert(_Alignof(struct input_ReadyDir) == 1, "input_ReadyDir align");
_Static_assert(offsetof(struct input_ReadyDir, tag) == 0, "input_ReadyDir.tag offset");
_Static_assert(offsetof(struct input_ReadyDir, _raw) == 0, "input_ReadyDir._raw offset");
struct input_RxError {
    uint32_t bits;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_RxError) == 4, "input_RxError size");
_Static_assert(_Alignof(struct input_RxError) == 4, "input_RxError align");
_Static_assert(offsetof(struct input_RxError, bits) == 0, "input_RxError.bits offset");
struct input_RxDatum {
    uint32_t unit;
    struct input_RxError errors;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_RxDatum) == 8, "input_RxDatum size");
_Static_assert(_Alignof(struct input_RxDatum) == 4, "input_RxDatum align");
_Static_assert(offsetof(struct input_RxDatum, unit) == 0, "input_RxDatum.unit offset");
_Static_assert(offsetof(struct input_RxDatum, errors) == 4, "input_RxDatum.errors offset");
struct input_PortScope {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_PortScope) == 1, "input_PortScope size");
_Static_assert(_Alignof(struct input_PortScope) == 1, "input_PortScope align");
_Static_assert(offsetof(struct input_PortScope, tag) == 0, "input_PortScope.tag offset");
_Static_assert(offsetof(struct input_PortScope, _raw) == 0, "input_PortScope._raw offset");
struct input_PrevState {
    _Bool was_on;
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_PrevState) == 1, "input_PrevState size");
_Static_assert(_Alignof(struct input_PrevState) == 1, "input_PrevState align");
_Static_assert(offsetof(struct input_PrevState, was_on) == 0, "input_PrevState.was_on offset");
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 8, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 size");
_Static_assert(_Alignof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 align");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, mask) == 0, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.mask offset");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, value) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.value offset");
struct input_DataBits {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_DataBits) == 1, "input_DataBits size");
_Static_assert(_Alignof(struct input_DataBits) == 1, "input_DataBits align");
_Static_assert(offsetof(struct input_DataBits, tag) == 0, "input_DataBits.tag offset");
_Static_assert(offsetof(struct input_DataBits, _raw) == 0, "input_DataBits._raw offset");
struct input_Parity {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Parity) == 1, "input_Parity size");
_Static_assert(_Alignof(struct input_Parity) == 1, "input_Parity align");
_Static_assert(offsetof(struct input_Parity, tag) == 0, "input_Parity.tag offset");
_Static_assert(offsetof(struct input_Parity, _raw) == 0, "input_Parity._raw offset");
struct input_StopBits {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_StopBits) == 1, "input_StopBits size");
_Static_assert(_Alignof(struct input_StopBits) == 1, "input_StopBits align");
_Static_assert(offsetof(struct input_StopBits, tag) == 0, "input_StopBits.tag offset");
_Static_assert(offsetof(struct input_StopBits, _raw) == 0, "input_StopBits._raw offset");
struct input_LineFormat {
    uint32_t baud;
    struct input_DataBits data_bits;
    struct input_Parity parity;
    struct input_StopBits stop_bits;
    unsigned char _pad0[1];
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_LineFormat) == 8, "input_LineFormat size");
_Static_assert(_Alignof(struct input_LineFormat) == 4, "input_LineFormat align");
_Static_assert(offsetof(struct input_LineFormat, baud) == 0, "input_LineFormat.baud offset");
_Static_assert(offsetof(struct input_LineFormat, data_bits) == 4, "input_LineFormat.data_bits offset");
_Static_assert(offsetof(struct input_LineFormat, parity) == 5, "input_LineFormat.parity offset");
_Static_assert(offsetof(struct input_LineFormat, stop_bits) == 6, "input_LineFormat.stop_bits offset");
struct input_ConfigResult {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_ConfigResult) == 1, "input_ConfigResult size");
_Static_assert(_Alignof(struct input_ConfigResult) == 1, "input_ConfigResult align");
_Static_assert(offsetof(struct input_ConfigResult, tag) == 0, "input_ConfigResult.tag offset");
_Static_assert(offsetof(struct input_ConfigResult, _raw) == 0, "input_ConfigResult._raw offset");
struct input_CauseSet {
    uint32_t bits;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_CauseSet) == 4, "input_CauseSet size");
_Static_assert(_Alignof(struct input_CauseSet) == 4, "input_CauseSet align");
_Static_assert(offsetof(struct input_CauseSet, bits) == 0, "input_CauseSet.bits offset");
struct input_SerialIrqSource {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SerialIrqSource) == 1, "input_SerialIrqSource size");
_Static_assert(_Alignof(struct input_SerialIrqSource) == 1, "input_SerialIrqSource align");
_Static_assert(offsetof(struct input_SerialIrqSource, tag) == 0, "input_SerialIrqSource.tag offset");
_Static_assert(offsetof(struct input_SerialIrqSource, _raw) == 0, "input_SerialIrqSource._raw offset");
struct input_IrqSourceState {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_IrqSourceState) == 1, "input_IrqSourceState size");
_Static_assert(_Alignof(struct input_IrqSourceState) == 1, "input_IrqSourceState align");
_Static_assert(offsetof(struct input_IrqSourceState, tag) == 0, "input_IrqSourceState.tag offset");
_Static_assert(offsetof(struct input_IrqSourceState, _raw) == 0, "input_IrqSourceState._raw offset");
struct input_IrqResult {
    union {
    struct {
    uint8_t tag;
    };
    unsigned char _raw[1];
    };
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_IrqResult) == 1, "input_IrqResult size");
_Static_assert(_Alignof(struct input_IrqResult) == 1, "input_IrqResult align");
_Static_assert(offsetof(struct input_IrqResult, tag) == 0, "input_IrqResult.tag offset");
_Static_assert(offsetof(struct input_IrqResult, _raw) == 0, "input_IrqResult._raw offset");
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
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 0, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 1, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 align");
struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62) == 0, "core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62) == 1, "core_marker_PhantomData__192b981e79185e5cfc4e4e7cb3debe62 align");
struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f) == 0, "core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f size");
_Static_assert(_Alignof(struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f) == 1, "core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f align");
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
struct core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3) == 0, "core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3) == 1, "core_marker_PhantomData__e4379341883c9ac8fc203097c8d374e3 align");
struct input_Interrupt {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Interrupt) == 0, "input_Interrupt size");
_Static_assert(_Alignof(struct input_Interrupt) == 1, "input_Interrupt align");
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
struct input_pl011_ifls_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_ifls_Register) == 0, "input_pl011_ifls_Register size");
_Static_assert(_Alignof(struct input_pl011_ifls_Register) == 1, "input_pl011_ifls_Register align");
struct input_pl011_imsc_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_imsc_Register) == 0, "input_pl011_imsc_Register size");
_Static_assert(_Alignof(struct input_pl011_imsc_Register) == 1, "input_pl011_imsc_Register align");
struct input_pl011_lcr_h_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_lcr_h_Register) == 0, "input_pl011_lcr_h_Register size");
_Static_assert(_Alignof(struct input_pl011_lcr_h_Register) == 1, "input_pl011_lcr_h_Register align");
struct input_pl011_mis_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_mis_Register) == 0, "input_pl011_mis_Register size");
_Static_assert(_Alignof(struct input_pl011_mis_Register) == 1, "input_pl011_mis_Register align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
void input__impl_0__clear_all_interrupts(struct input_SerialPl011 *);
void input__impl_0__commit_imsc_shadow(struct input_SerialPl011 *);
void input__impl_0__mask_all_interrupts(struct input_SerialPl011 *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_1__on_deinit(struct input_SerialPl011 *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_1__on_init(struct input_SerialPl011 *);
_Bool input__impl_2__data_ready_query(const struct input_SerialPl011 *, struct input_ReadyDir);
struct input_RxDatum input__impl_2__data_unit_read(struct input_SerialPl011 *);
void input__impl_2__data_unit_write(struct input_SerialPl011 *, uint32_t);
struct input_PrevState input__impl_2__port_service_set(struct input_SerialPl011 *, struct input_PortScope, _Bool);
struct input_ConfigResult input__impl_3__line_format_apply(struct input_SerialPl011 *, struct input_LineFormat);
struct input_CauseSet input__impl_4__irq_cause_decode(struct input_SerialPl011 *);
void input__impl_4__irq_source_set(struct input_SerialPl011 *, struct input_SerialIrqSource, struct input_IrqSourceState);
_Bool input__impl_5__tx_drain_query(const struct input_SerialPl011 *);
inline struct input_IrqResult input__impl_7__clone(const struct input_IrqResult *);
inline _Bool input__impl_10__eq(const struct input_IrqResult *, const struct input_IrqResult *);
inline void input__impl_11__assert_receiver_is_total_eq(const struct input_IrqResult *);
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_13__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
__attribute__((noinline)) uint32_t input__impl_14__read__b3419fd74a8aecb75505fd74c030f9ba(const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *);
__attribute__((noinline)) uint32_t input__impl_14__read__dcf610a28ad7155fe10496944bda173a(const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *);
__attribute__((noinline)) void input__impl_15__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *, uint32_t);
__attribute__((noinline)) uint32_t input__impl_16__read__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *);
__attribute__((noinline)) uint32_t input__impl_16__read__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *);
__attribute__((noinline)) void input__impl_16__write__12f6da63eb7da4bbe47ece8c97d11228(const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__65daba9b2aba7b1866e3e9b4c1c17494(const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__eddf52bcb943db2c1fde6ddb9bf48a9f(const struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f *, uint32_t);
__attribute__((noinline)) void input__impl_16__write__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *, uint32_t);
__attribute__((noinline)) void input__impl_16__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_20__new(void);
const struct input_pl011_Regs *input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *);
struct input_PrevState input__impl_27__new(_Bool);
uint32_t input__impl_28__bits(struct input_RxError);
struct input_RxError input__impl_28__empty(void);
struct input_RxError input__impl_28__from_bits(uint32_t);
struct input_RxDatum input__impl_29__new(uint32_t, struct input_RxError);
uint32_t input__impl_30__bits(struct input_CauseSet);
struct input_CauseSet input__impl_30__empty(void);
struct input_CauseSet input__impl_30__from_bits(uint32_t);
inline struct input_PortScope input__impl_32__clone(const struct input_PortScope *);
inline _Bool input__impl_35__eq(const struct input_PortScope *, const struct input_PortScope *);
inline void input__impl_36__assert_receiver_is_total_eq(const struct input_PortScope *);
inline struct input_ReadyDir input__impl_38__clone(const struct input_ReadyDir *);
inline _Bool input__impl_41__eq(const struct input_ReadyDir *, const struct input_ReadyDir *);
inline void input__impl_42__assert_receiver_is_total_eq(const struct input_ReadyDir *);
inline struct input_PrevState input__impl_44__clone(const struct input_PrevState *);
inline _Bool input__impl_47__eq(const struct input_PrevState *, const struct input_PrevState *);
inline void input__impl_48__assert_receiver_is_total_eq(const struct input_PrevState *);
inline struct input_RxError input__impl_50__clone(const struct input_RxError *);
inline _Bool input__impl_53__eq(const struct input_RxError *, const struct input_RxError *);
inline void input__impl_54__assert_receiver_is_total_eq(const struct input_RxError *);
inline struct input_RxDatum input__impl_56__clone(const struct input_RxDatum *);
inline _Bool input__impl_59__eq(const struct input_RxDatum *, const struct input_RxDatum *);
inline void input__impl_60__assert_receiver_is_total_eq(const struct input_RxDatum *);
inline struct input_DataBits input__impl_62__clone(const struct input_DataBits *);
inline _Bool input__impl_65__eq(const struct input_DataBits *, const struct input_DataBits *);
inline void input__impl_66__assert_receiver_is_total_eq(const struct input_DataBits *);
inline struct input_Parity input__impl_68__clone(const struct input_Parity *);
inline _Bool input__impl_71__eq(const struct input_Parity *, const struct input_Parity *);
inline void input__impl_72__assert_receiver_is_total_eq(const struct input_Parity *);
inline struct input_StopBits input__impl_74__clone(const struct input_StopBits *);
inline _Bool input__impl_77__eq(const struct input_StopBits *, const struct input_StopBits *);
inline void input__impl_78__assert_receiver_is_total_eq(const struct input_StopBits *);
inline struct input_LineFormat input__impl_80__clone(const struct input_LineFormat *);
inline _Bool input__impl_83__eq(const struct input_LineFormat *, const struct input_LineFormat *);
inline void input__impl_84__assert_receiver_is_total_eq(const struct input_LineFormat *);
inline struct input_ConfigResult input__impl_86__clone(const struct input_ConfigResult *);
inline _Bool input__impl_89__eq(const struct input_ConfigResult *, const struct input_ConfigResult *);
inline void input__impl_90__assert_receiver_is_total_eq(const struct input_ConfigResult *);
inline struct input_SerialIrqSource input__impl_92__clone(const struct input_SerialIrqSource *);
inline _Bool input__impl_95__eq(const struct input_SerialIrqSource *, const struct input_SerialIrqSource *);
inline void input__impl_96__assert_receiver_is_total_eq(const struct input_SerialIrqSource *);
inline struct input_IrqSourceState input__impl_98__clone(const struct input_IrqSourceState *);
inline _Bool input__impl_101__eq(const struct input_IrqSourceState *, const struct input_IrqSourceState *);
inline void input__impl_102__assert_receiver_is_total_eq(const struct input_IrqSourceState *);
inline struct input_CauseSet input__impl_104__clone(const struct input_CauseSet *);
inline _Bool input__impl_107__eq(const struct input_CauseSet *, const struct input_CauseSet *);
inline void input__impl_108__assert_receiver_is_total_eq(const struct input_CauseSet *);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
extern const uint8_t input__regseq_synth__const_memory_static_0a62532ae19ec6a93be2423d964ec66b[14];
extern const uint8_t input__regseq_synth__const_memory_static_14be334d3c723feb7f43498c995c5922[23];
extern const uint8_t input__regseq_synth__const_memory_static_323c8dea91d6f57c9b1afd93bfd9776f[25];
extern const uint8_t input__regseq_synth__const_memory_static_770590b04d61205c05c40de7a4cba37e[38];
extern const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0];

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
void input__impl_0__clear_all_interrupts(struct input_SerialPl011 *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_4;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).icr);
        regseq_bus_mmio_write(68, 4, (uint64_t)(((uint32_t)(2047U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
void input__impl_0__commit_imsc_shadow(struct input_SerialPl011 *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_4;
    uint32_t _5;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).imsc);
        _5 = (*_1).imsc_shadow;
        regseq_bus_mmio_write(56, 4, (uint64_t)(_5)); goto bb_2;
    }
    bb_2: {
        return;
    }
}
void input__impl_0__mask_all_interrupts(struct input_SerialPl011 *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_4;
    bb_0: {
        (*_1).imsc_shadow = ((uint32_t)(0U));
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).imsc);
        regseq_bus_mmio_write(56, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_1__on_deinit(struct input_SerialPl011 *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    struct input_SerialPl011 *_7;
    struct input_SerialPl011 *_9;
    bb_0: {
        fwrite("[WARN] serialpl011 deinit: masking interrupts\n", 1, 46, stdout); goto bb_1;
    }
    bb_1: {
        _7 = &((*_1));
        input__impl_0__mask_all_interrupts(_7); goto bb_2;
    }
    bb_2: {
        _9 = &((*_1));
        input__impl_0__clear_all_interrupts(_9); goto bb_3;
    }
    bb_3: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_1__on_init(struct input_SerialPl011 *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    struct input_SerialPl011 *_7;
    struct input_SerialPl011 *_9;
    bb_0: {
        fwrite("[DEBUG] serialpl011 probe start\n", 1, 32, stdout); goto bb_1;
    }
    bb_1: {
        _7 = &((*_1));
        input__impl_0__mask_all_interrupts(_7); goto bb_2;
    }
    bb_2: {
        _9 = &((*_1));
        input__impl_0__clear_all_interrupts(_9); goto bb_3;
    }
    bb_3: {
        fwrite("[INFO] serialpl011 up\n", 1, 22, stdout); goto bb_4;
    }
    bb_4: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
_Bool input__impl_2__data_ready_query(const struct input_SerialPl011 *_1, struct input_ReadyDir _2) {
    _Bool _0;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_3;
    const struct input_pl011_Regs *_4;
    const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *_5;
    uint32_t _6;
    intptr_t _7;
    uint32_t _8;
    uint32_t _9;
    uint32_t _10;
    uint32_t _11;
    bb_0: {
        _3 = &((*_1).regs);
        _4 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_3); goto bb_1;
    }
    bb_1: {
        _5 = &((*_4).fr);
        _6 = (uint32_t)regseq_bus_mmio_read(24, 4); goto bb_2;
    }
    bb_2: {
        _7 = (_2.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_7) { case 0ULL: goto bb_5; case 1ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        regseq_unreachable();
    }
    bb_4: {
        _10 = _6;
        _11 = ((_10) & (((uint32_t)(16U))));
        _0 = ((_11) == (((uint32_t)(0U))));
        goto bb_6;
    }
    bb_5: {
        _8 = _6;
        _9 = ((_8) & (((uint32_t)(32U))));
        _0 = ((_9) == (((uint32_t)(0U))));
        goto bb_6;
    }
    bb_6: {
        return _0;
    }
}
struct input_RxDatum input__impl_2__data_unit_read(struct input_SerialPl011 *_1) {
    struct input_RxDatum _0;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _8;
    uint32_t _9;
    uint32_t _10;
    _Bool _11;
    uint32_t _12;
    uint32_t _13;
    _Bool _14;
    uint32_t _15;
    uint32_t _16;
    _Bool _17;
    uint32_t _18;
    uint32_t _19;
    _Bool _20;
    uint32_t _21;
    uint32_t _22;
    struct input_RxError _23;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).dr);
        _5 = (uint32_t)regseq_bus_mmio_read(0, 4); goto bb_2;
    }
    bb_2: {
        _6 = _5;
        _7 = ((_6) & (((uint32_t)(255U))));
        _8 = ((uint32_t)(0U));
        _9 = _5;
        _10 = ((_9) & (((uint32_t)(2048U))));
        _11 = ((_10) != (((uint32_t)(0U))));
        switch (_11) { case 0ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        _8 = ((_8) | (((uint32_t)(4U))));
        goto bb_5;
    }
    bb_4: {
        goto bb_5;
    }
    bb_5: {
        _12 = _5;
        _13 = ((_12) & (((uint32_t)(1024U))));
        _14 = ((_13) != (((uint32_t)(0U))));
        switch (_14) { case 0ULL: goto bb_7; default: goto bb_6; }
    }
    bb_6: {
        _8 = ((_8) | (((uint32_t)(16U))));
        goto bb_8;
    }
    bb_7: {
        goto bb_8;
    }
    bb_8: {
        _15 = _5;
        _16 = ((_15) & (((uint32_t)(512U))));
        _17 = ((_16) != (((uint32_t)(0U))));
        switch (_17) { case 0ULL: goto bb_10; default: goto bb_9; }
    }
    bb_9: {
        _8 = ((_8) | (((uint32_t)(2U))));
        goto bb_11;
    }
    bb_10: {
        goto bb_11;
    }
    bb_11: {
        _18 = _5;
        _19 = ((_18) & (((uint32_t)(256U))));
        _20 = ((_19) != (((uint32_t)(0U))));
        switch (_20) { case 0ULL: goto bb_13; default: goto bb_12; }
    }
    bb_12: {
        _8 = ((_8) | (((uint32_t)(1U))));
        goto bb_14;
    }
    bb_13: {
        goto bb_14;
    }
    bb_14: {
        _21 = _7;
        _22 = _8;
        _23 = input__impl_28__from_bits(_22); goto bb_15;
    }
    bb_15: {
        _0 = input__impl_29__new(_21, _23); goto bb_16;
    }
    bb_16: {
        return _0;
    }
}
void input__impl_2__data_unit_write(struct input_SerialPl011 *_1, uint32_t _2) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_3;
    const struct input_pl011_Regs *_4;
    const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_5;
    uint32_t _6;
    uint32_t _7;
    bb_0: {
        _3 = &((*_1).regs);
        _4 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_3); goto bb_1;
    }
    bb_1: {
        _5 = &((*_4).dr);
        _6 = _2;
        _7 = ((_6) & (((uint32_t)(255U))));
        regseq_bus_mmio_write(0, 4, (uint64_t)(_7)); goto bb_2;
    }
    bb_2: {
        return;
    }
}
struct input_PrevState input__impl_2__port_service_set(struct input_SerialPl011 *_1, struct input_PortScope _2, _Bool _3) {
    struct input_PrevState _0;
    intptr_t _4;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_5;
    const struct input_pl011_Regs *_6;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_7;
    uint32_t _8;
    uint32_t _9;
    _Bool _10;
    _Bool _11;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_12;
    const struct input_pl011_Regs *_13;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_14;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_16;
    const struct input_pl011_Regs *_17;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_18;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _19;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _20;
    _Bool _22;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_23;
    const struct input_pl011_Regs *_24;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_25;
    uint32_t _26;
    uint32_t _27;
    uint32_t _28;
    uint32_t _29;
    _Bool _30;
    _Bool _31;
    uint32_t _32;
    uint32_t _33;
    uint32_t _34;
    uint32_t _35;
    uint32_t _36;
    uint32_t _37;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_38;
    const struct input_pl011_Regs *_39;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_40;
    uint32_t _41;
    _Bool _43;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_44;
    const struct input_pl011_Regs *_45;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_46;
    uint32_t _47;
    uint32_t _48;
    uint32_t _49;
    uint32_t _50;
    _Bool _51;
    _Bool _52;
    uint32_t _53;
    uint32_t _54;
    uint32_t _55;
    uint32_t _56;
    uint32_t _57;
    uint32_t _58;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_59;
    const struct input_pl011_Regs *_60;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_61;
    uint32_t _62;
    _Bool _64;
    bb_0: {
        _4 = (_2.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : (_2.tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        switch (_4) { case 0ULL: goto bb_20; case 1ULL: goto bb_11; case 2ULL: goto bb_2; default: goto bb_1; }
    }
    bb_1: {
        regseq_unreachable();
    }
    bb_2: {
        _44 = &((*_1).regs);
        _45 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_44); goto bb_3;
    }
    bb_3: {
        _46 = &((*_45).cr);
        _47 = (uint32_t)regseq_bus_mmio_read(48, 4); goto bb_4;
    }
    bb_4: {
        _48 = _47;
        _49 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(9U))) & 31)));
        _50 = ((_48) & (_49));
        _51 = ((_50) != (((uint32_t)(0U))));
        _52 = _3;
        switch (_52) { case 0ULL: goto bb_6; default: goto bb_5; }
    }
    bb_5: {
        _57 = _47;
        _58 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(9U))) & 31)));
        _56 = ((_57) | (_58));
        goto bb_7;
    }
    bb_6: {
        _53 = _47;
        _54 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(9U))) & 31)));
        _55 = (~(_54));
        _56 = ((_53) & (_55));
        goto bb_7;
    }
    bb_7: {
        _59 = &((*_1).regs);
        _60 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_59); goto bb_8;
    }
    bb_8: {
        _61 = &((*_60).cr);
        _62 = _56;
        regseq_bus_mmio_write(48, 4, (uint64_t)(_62)); goto bb_9;
    }
    bb_9: {
        _64 = _51;
        _0 = input__impl_27__new(_64); goto bb_10;
    }
    bb_10: {
        goto bb_33;
    }
    bb_11: {
        _23 = &((*_1).regs);
        _24 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_23); goto bb_12;
    }
    bb_12: {
        _25 = &((*_24).cr);
        _26 = (uint32_t)regseq_bus_mmio_read(48, 4); goto bb_13;
    }
    bb_13: {
        _27 = _26;
        _28 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(8U))) & 31)));
        _29 = ((_27) & (_28));
        _30 = ((_29) != (((uint32_t)(0U))));
        _31 = _3;
        switch (_31) { case 0ULL: goto bb_15; default: goto bb_14; }
    }
    bb_14: {
        _36 = _26;
        _37 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(8U))) & 31)));
        _35 = ((_36) | (_37));
        goto bb_16;
    }
    bb_15: {
        _32 = _26;
        _33 = ((uint32_t)((uint32_t)(((uint32_t)(1U))) << ((((int32_t)(8U))) & 31)));
        _34 = (~(_33));
        _35 = ((_32) & (_34));
        goto bb_16;
    }
    bb_16: {
        _38 = &((*_1).regs);
        _39 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_38); goto bb_17;
    }
    bb_17: {
        _40 = &((*_39).cr);
        _41 = _35;
        regseq_bus_mmio_write(48, 4, (uint64_t)(_41)); goto bb_18;
    }
    bb_18: {
        _43 = _30;
        _0 = input__impl_27__new(_43); goto bb_19;
    }
    bb_19: {
        goto bb_33;
    }
    bb_20: {
        _5 = &((*_1).regs);
        _6 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_5); goto bb_21;
    }
    bb_21: {
        _7 = &((*_6).cr);
        _8 = (uint32_t)regseq_bus_mmio_read(48, 4); goto bb_22;
    }
    bb_22: {
        _9 = ((_8) & (((uint32_t)(1U))));
        _10 = ((_9) != (((uint32_t)(0U))));
        _11 = _3;
        switch (_11) { case 0ULL: goto bb_28; default: goto bb_23; }
    }
    bb_23: {
        _16 = &((*_1).regs);
        _17 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_16); goto bb_24;
    }
    bb_24: {
        _18 = &((*_17).cr);
        _19 = input__impl_13__add__ae10abed9e3303e99a36ab95b86fdfcd((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(1U)) }, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(256U)), .value = ((uint32_t)(256U)) }); goto bb_25;
    }
    bb_25: {
        _20 = input__impl_13__add__ae10abed9e3303e99a36ab95b86fdfcd(_19, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(512U)), .value = ((uint32_t)(512U)) }); goto bb_26;
    }
    bb_26: {
        regseq_bus_mmio_write(48, 4, (uint64_t)((_20).value & (_20).mask)); goto bb_27;
    }
    bb_27: {
        goto bb_31;
    }
    bb_28: {
        _12 = &((*_1).regs);
        _13 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_12); goto bb_29;
    }
    bb_29: {
        _14 = &((*_13).cr);
        regseq_bus_mmio_write(48, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_30;
    }
    bb_30: {
        goto bb_31;
    }
    bb_31: {
        _22 = _10;
        _0 = input__impl_27__new(_22); goto bb_32;
    }
    bb_32: {
        goto bb_33;
    }
    bb_33: {
        return _0;
    }
}
struct input_ConfigResult input__impl_3__line_format_apply(struct input_SerialPl011 *_1, struct input_LineFormat _2) {
    struct input_ConfigResult _0;
    intptr_t _3;
    uint32_t _4;
    intptr_t _5;
    uint32_t _6;
    intptr_t _7;
    uint32_t _8;
    uint32_t _9;
    uint32_t _10;
    uint32_t _11;
    uint32_t _12;
    uint32_t _13;
    uint32_t _14;
    uint32_t _15;
    uint64_t _16;
    uint32_t _17;
    uint64_t _18;
    uint64_t _19;
    uint64_t _20;
    uint64_t _21;
    uint64_t _22;
    uint64_t _23;
    uint64_t _24;
    _Bool _25;
    uint64_t _26;
    uint64_t _27;
    uint64_t _28;
    uint32_t _29;
    uint64_t _30;
    uint64_t _31;
    uint32_t _32;
    uint32_t _33;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_40;
    const struct input_pl011_Regs *_41;
    const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *_42;
    uint32_t _43;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_45;
    const struct input_pl011_Regs *_46;
    const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *_47;
    uint32_t _48;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_50;
    const struct input_pl011_Regs *_51;
    const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_52;
    uint32_t _53;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_55;
    const struct input_pl011_Regs *_56;
    const struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f *_57;
    bb_0: {
        _3 = (_2.data_bits.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : (_2.data_bits.tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : (_2.data_bits.tag == ((uint8_t)(2U)) ? ((intptr_t)(2ULL)) : (_2.data_bits.tag == ((uint8_t)(3U)) ? ((intptr_t)(3ULL)) : ((intptr_t)(4ULL))))));
        switch (_3) { case 0ULL: goto bb_5; case 1ULL: goto bb_4; case 2ULL: goto bb_3; case 3ULL: goto bb_2; case 4ULL: goto bb_1; default: goto bb_10; }
    }
    bb_1: {
        _0.tag = ((uint8_t)(1U));
        goto bb_26;
    }
    bb_2: {
        _4 = ((uint32_t)(3U));
        goto bb_6;
    }
    bb_3: {
        _4 = ((uint32_t)(2U));
        goto bb_6;
    }
    bb_4: {
        _4 = ((uint32_t)(1U));
        goto bb_6;
    }
    bb_5: {
        _4 = ((uint32_t)(0U));
        goto bb_6;
    }
    bb_6: {
        _5 = (_2.stop_bits.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_5) { case 0ULL: goto bb_8; case 1ULL: goto bb_7; default: goto bb_10; }
    }
    bb_7: {
        _6 = ((uint32_t)(8U));
        goto bb_9;
    }
    bb_8: {
        _6 = ((uint32_t)(0U));
        goto bb_9;
    }
    bb_9: {
        _7 = (_2.parity.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : (_2.parity.tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        switch (_7) { case 0ULL: goto bb_13; case 1ULL: goto bb_12; case 2ULL: goto bb_11; default: goto bb_10; }
    }
    bb_10: {
        regseq_unreachable();
    }
    bb_11: {
        _8 = ((uint32_t)(2U));
        goto bb_14;
    }
    bb_12: {
        _8 = ((((uint32_t)(2U))) | (((uint32_t)(4U))));
        goto bb_14;
    }
    bb_13: {
        _8 = ((uint32_t)(0U));
        goto bb_14;
    }
    bb_14: {
        _9 = _4;
        _10 = ((uint32_t)((uint32_t)(_9) << ((((uint32_t)(5U))) & 31)));
        _11 = ((_10) | (((uint32_t)(16U))));
        _12 = _6;
        _13 = ((_11) | (_12));
        _14 = _8;
        _15 = ((_13) | (_14));
        _16 = ((uint64_t)(((uint32_t)(48000000U))));
        _17 = _2.baud;
        _18 = ((uint64_t)(_17));
        _19 = _16;
        _20 = ((_19) * (((uint64_t)(8ULL))));
        _21 = _18;
        _22 = ((_20) + (_21));
        _23 = _18;
        _24 = ((_23) * (((uint64_t)(2ULL))));
        _25 = ((_24) == (((uint64_t)(0ULL))));
        if ((_25) == 0) goto bb_15; else { regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO); }
    }
    bb_15: {
        _26 = ((_22) / (_24));
        _27 = _26;
        _28 = ((_27) >> ((((int32_t)(6U))) & 63));
        _29 = ((uint32_t)(_28));
        _30 = _26;
        _31 = ((_30) & (((uint64_t)(63ULL))));
        _32 = ((uint32_t)(_31));
        _33 = _2.baud;
        goto bb_16;
    }
    bb_16: {
        fwrite("[INFO] serialpl011 line: baud=", 1, 30, stdout); { char _n[24]; int _l = snprintf(_n, sizeof _n, "%u", (unsigned)(_33)); fwrite(_n, 1, _l, stdout); } fwrite("\n", 1, 1, stdout); goto bb_17;
    }
    bb_17: {
        _40 = &((*_1).regs);
        _41 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_40); goto bb_18;
    }
    bb_18: {
        _42 = &((*_41).ibrd);
        _43 = _29;
        regseq_bus_mmio_write(36, 4, (uint64_t)(_43)); goto bb_19;
    }
    bb_19: {
        _45 = &((*_1).regs);
        _46 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_45); goto bb_20;
    }
    bb_20: {
        _47 = &((*_46).fbrd);
        _48 = _32;
        regseq_bus_mmio_write(40, 4, (uint64_t)(_48)); goto bb_21;
    }
    bb_21: {
        _50 = &((*_1).regs);
        _51 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_50); goto bb_22;
    }
    bb_22: {
        _52 = &((*_51).lcr_h);
        _53 = _15;
        regseq_bus_mmio_write(44, 4, (uint64_t)(_53)); goto bb_23;
    }
    bb_23: {
        _55 = &((*_1).regs);
        _56 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_55); goto bb_24;
    }
    bb_24: {
        _57 = &((*_56).ifls);
        regseq_bus_mmio_write(52, 4, (uint64_t)(((uint32_t)(16U)))); goto bb_25;
    }
    bb_25: {
        _0.tag = ((uint8_t)(0U));
        goto bb_26;
    }
    bb_26: {
        return _0;
    }
}
struct input_CauseSet input__impl_4__irq_cause_decode(struct input_SerialPl011 *_1) {
    struct input_CauseSet _0;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *_4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    uint32_t _8;
    _Bool _9;
    uint32_t _10;
    uint32_t _11;
    _Bool _12;
    uint32_t _13;
    uint32_t _14;
    _Bool _15;
    uint32_t _16;
    uint32_t _17;
    _Bool _18;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_19;
    const struct input_pl011_Regs *_20;
    const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_21;
    uint32_t _22;
    uint32_t _23;
    uint32_t _25;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).mis);
        _5 = (uint32_t)regseq_bus_mmio_read(64, 4); goto bb_2;
    }
    bb_2: {
        _6 = ((uint32_t)(0U));
        _7 = _5;
        _8 = ((_7) & (((uint32_t)(32U))));
        _9 = ((_8) != (((uint32_t)(0U))));
        switch (_9) { case 0ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        _6 = ((_6) | (((uint32_t)(1U))));
        goto bb_5;
    }
    bb_4: {
        goto bb_5;
    }
    bb_5: {
        _10 = _5;
        _11 = ((_10) & (((uint32_t)(16U))));
        _12 = ((_11) != (((uint32_t)(0U))));
        switch (_12) { case 0ULL: goto bb_7; default: goto bb_6; }
    }
    bb_6: {
        _6 = ((_6) | (((uint32_t)(2U))));
        goto bb_8;
    }
    bb_7: {
        goto bb_8;
    }
    bb_8: {
        _13 = _5;
        _14 = ((_13) & (((uint32_t)(64U))));
        _15 = ((_14) != (((uint32_t)(0U))));
        switch (_15) { case 0ULL: goto bb_10; default: goto bb_9; }
    }
    bb_9: {
        _6 = ((_6) | (((uint32_t)(4U))));
        goto bb_11;
    }
    bb_10: {
        goto bb_11;
    }
    bb_11: {
        _16 = _5;
        _17 = ((_16) & (((uint32_t)(1920U))));
        _18 = ((_17) != (((uint32_t)(0U))));
        switch (_18) { case 0ULL: goto bb_15; default: goto bb_12; }
    }
    bb_12: {
        _6 = ((_6) | (((uint32_t)(8U))));
        _19 = &((*_1).regs);
        _20 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_19); goto bb_13;
    }
    bb_13: {
        _21 = &((*_20).icr);
        _22 = _5;
        _23 = ((_22) & (((uint32_t)(1920U))));
        regseq_bus_mmio_write(68, 4, (uint64_t)(_23)); goto bb_14;
    }
    bb_14: {
        goto bb_16;
    }
    bb_15: {
        goto bb_16;
    }
    bb_16: {
        _25 = _6;
        _0 = input__impl_30__from_bits(_25); goto bb_17;
    }
    bb_17: {
        return _0;
    }
}
void input__impl_4__irq_source_set(struct input_SerialPl011 *_1, struct input_SerialIrqSource _2, struct input_IrqSourceState _3) {
    intptr_t _4;
    uint32_t _5;
    intptr_t _6;
    uint32_t _7;
    uint32_t _8;
    uint32_t _9;
    struct input_SerialPl011 *_10;
    bb_0: {
        _4 = (_2.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : (_2.tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        switch (_4) { case 0ULL: goto bb_3; case 1ULL: goto bb_2; case 2ULL: goto bb_1; default: goto bb_5; }
    }
    bb_1: {
        _5 = ((uint32_t)(1920U));
        goto bb_4;
    }
    bb_2: {
        _5 = ((uint32_t)(80U));
        goto bb_4;
    }
    bb_3: {
        _5 = ((uint32_t)(32U));
        goto bb_4;
    }
    bb_4: {
        _6 = (_3.tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        switch (_6) { case 0ULL: goto bb_7; case 1ULL: goto bb_6; default: goto bb_5; }
    }
    bb_5: {
        regseq_unreachable();
    }
    bb_6: {
        _8 = _5;
        _9 = (~(_8));
        (*_1).imsc_shadow = (((*_1).imsc_shadow) & (_9));
        goto bb_8;
    }
    bb_7: {
        _7 = _5;
        (*_1).imsc_shadow = (((*_1).imsc_shadow) | (_7));
        goto bb_8;
    }
    bb_8: {
        _10 = &((*_1));
        input__impl_0__commit_imsc_shadow(_10); goto bb_9;
    }
    bb_9: {
        return;
    }
}
_Bool input__impl_5__tx_drain_query(const struct input_SerialPl011 *_1) {
    _Bool _0;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *_4;
    uint32_t _5;
    uint32_t _6;
    uint32_t _7;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).fr);
        _5 = (uint32_t)regseq_bus_mmio_read(24, 4); goto bb_2;
    }
    bb_2: {
        _6 = ((((uint32_t)(8U))) | (((uint32_t)(32U))));
        _7 = ((_5) & (_6));
        _0 = ((_7) == (((uint32_t)(0U))));
        return _0;
    }
}
inline struct input_IrqResult input__impl_7__clone(const struct input_IrqResult *_1) {
    struct input_IrqResult _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_10__eq(const struct input_IrqResult *_1, const struct input_IrqResult *_2) {
    _Bool _0;
    const struct input_IrqResult *_3;
    intptr_t _4;
    const struct input_IrqResult *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_11__assert_receiver_is_total_eq(const struct input_IrqResult *_1) {
    bb_0: {
        return;
    }
}
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_13__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
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
__attribute__((noinline)) uint32_t input__impl_14__read__b3419fd74a8aecb75505fd74c030f9ba(const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) uint32_t input__impl_14__read__dcf610a28ad7155fe10496944bda173a(const struct input_ReadOnly__dcf610a28ad7155fe10496944bda173a *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_15__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) uint32_t input__impl_16__read__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) uint32_t input__impl_16__read__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_16__write__12f6da63eb7da4bbe47ece8c97d11228(const struct input_ReadWrite__12f6da63eb7da4bbe47ece8c97d11228 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__65daba9b2aba7b1866e3e9b4c1c17494(const struct input_ReadWrite__65daba9b2aba7b1866e3e9b4c1c17494 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__b7ae42fd0448c9d81ff6641dc3fc5685(const struct input_ReadWrite__b7ae42fd0448c9d81ff6641dc3fc5685 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__eddf52bcb943db2c1fde6ddb9bf48a9f(const struct input_ReadWrite__eddf52bcb943db2c1fde6ddb9bf48a9f *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_16__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_20__new(void) {
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _0;
    bb_0: {
        _0 = (struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c){ ._v = ((uint32_t)(0U)) };
        return _0;
    }
}
const struct input_pl011_Regs *input__impl_22__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_1) {
    const struct input_pl011_Regs *_0;
    const struct input_pl011_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
struct input_PrevState input__impl_27__new(_Bool _1) {
    struct input_PrevState _0;
    _Bool _2;
    bb_0: {
        _2 = _1;
        _0 = (struct input_PrevState){ .was_on = _2 };
        return _0;
    }
}
uint32_t input__impl_28__bits(struct input_RxError _1) {
    uint32_t _0;
    bb_0: {
        _0 = _1.bits;
        return _0;
    }
}
struct input_RxError input__impl_28__empty(void) {
    struct input_RxError _0;
    bb_0: {
        _0 = (struct input_RxError){ .bits = ((uint32_t)(0U)) };
        return _0;
    }
}
struct input_RxError input__impl_28__from_bits(uint32_t _1) {
    struct input_RxError _0;
    uint32_t _2;
    bb_0: {
        _2 = _1;
        _0 = (struct input_RxError){ .bits = _2 };
        return _0;
    }
}
struct input_RxDatum input__impl_29__new(uint32_t _1, struct input_RxError _2) {
    struct input_RxDatum _0;
    uint32_t _3;
    struct input_RxError _4;
    bb_0: {
        _3 = _1;
        _4 = _2;
        _0 = (struct input_RxDatum){ .unit = _3, .errors = _4 };
        return _0;
    }
}
uint32_t input__impl_30__bits(struct input_CauseSet _1) {
    uint32_t _0;
    bb_0: {
        _0 = _1.bits;
        return _0;
    }
}
struct input_CauseSet input__impl_30__empty(void) {
    struct input_CauseSet _0;
    bb_0: {
        _0 = (struct input_CauseSet){ .bits = ((uint32_t)(0U)) };
        return _0;
    }
}
struct input_CauseSet input__impl_30__from_bits(uint32_t _1) {
    struct input_CauseSet _0;
    uint32_t _2;
    bb_0: {
        _2 = _1;
        _0 = (struct input_CauseSet){ .bits = _2 };
        return _0;
    }
}
inline struct input_PortScope input__impl_32__clone(const struct input_PortScope *_1) {
    struct input_PortScope _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_35__eq(const struct input_PortScope *_1, const struct input_PortScope *_2) {
    _Bool _0;
    const struct input_PortScope *_3;
    intptr_t _4;
    const struct input_PortScope *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_36__assert_receiver_is_total_eq(const struct input_PortScope *_1) {
    bb_0: {
        return;
    }
}
inline struct input_ReadyDir input__impl_38__clone(const struct input_ReadyDir *_1) {
    struct input_ReadyDir _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_41__eq(const struct input_ReadyDir *_1, const struct input_ReadyDir *_2) {
    _Bool _0;
    const struct input_ReadyDir *_3;
    intptr_t _4;
    const struct input_ReadyDir *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_42__assert_receiver_is_total_eq(const struct input_ReadyDir *_1) {
    bb_0: {
        return;
    }
}
inline struct input_PrevState input__impl_44__clone(const struct input_PrevState *_1) {
    struct input_PrevState _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_47__eq(const struct input_PrevState *_1, const struct input_PrevState *_2) {
    _Bool _0;
    _Bool _3;
    _Bool _4;
    bb_0: {
        _3 = (*_1).was_on;
        _4 = (*_2).was_on;
        _0 = ((_3) == (_4));
        return _0;
    }
}
inline void input__impl_48__assert_receiver_is_total_eq(const struct input_PrevState *_1) {
    bb_0: {
        return;
    }
}
inline struct input_RxError input__impl_50__clone(const struct input_RxError *_1) {
    struct input_RxError _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_53__eq(const struct input_RxError *_1, const struct input_RxError *_2) {
    _Bool _0;
    uint32_t _3;
    uint32_t _4;
    bb_0: {
        _3 = (*_1).bits;
        _4 = (*_2).bits;
        _0 = ((_3) == (_4));
        return _0;
    }
}
inline void input__impl_54__assert_receiver_is_total_eq(const struct input_RxError *_1) {
    bb_0: {
        return;
    }
}
inline struct input_RxDatum input__impl_56__clone(const struct input_RxDatum *_1) {
    struct input_RxDatum _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_59__eq(const struct input_RxDatum *_1, const struct input_RxDatum *_2) {
    _Bool _0;
    uint32_t _3;
    uint32_t _4;
    _Bool _5;
    const struct input_RxError *_6;
    const struct input_RxError *_7;
    bb_0: {
        _3 = (*_1).unit;
        _4 = (*_2).unit;
        _5 = ((_3) == (_4));
        switch (_5) { case 0ULL: goto bb_3; default: goto bb_1; }
    }
    bb_1: {
        _6 = &((*_1).errors);
        _7 = &((*_2).errors);
        _0 = input__impl_53__eq(_6, _7); goto bb_2;
    }
    bb_2: {
        goto bb_4;
    }
    bb_3: {
        _0 = 0;
        goto bb_4;
    }
    bb_4: {
        return _0;
    }
}
inline void input__impl_60__assert_receiver_is_total_eq(const struct input_RxDatum *_1) {
    bb_0: {
        return;
    }
}
inline struct input_DataBits input__impl_62__clone(const struct input_DataBits *_1) {
    struct input_DataBits _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_65__eq(const struct input_DataBits *_1, const struct input_DataBits *_2) {
    _Bool _0;
    const struct input_DataBits *_3;
    intptr_t _4;
    const struct input_DataBits *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((*_3).tag == ((uint8_t)(2U)) ? ((intptr_t)(2ULL)) : ((*_3).tag == ((uint8_t)(3U)) ? ((intptr_t)(3ULL)) : ((intptr_t)(4ULL))))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((*_5).tag == ((uint8_t)(2U)) ? ((intptr_t)(2ULL)) : ((*_5).tag == ((uint8_t)(3U)) ? ((intptr_t)(3ULL)) : ((intptr_t)(4ULL))))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_66__assert_receiver_is_total_eq(const struct input_DataBits *_1) {
    bb_0: {
        return;
    }
}
inline struct input_Parity input__impl_68__clone(const struct input_Parity *_1) {
    struct input_Parity _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_71__eq(const struct input_Parity *_1, const struct input_Parity *_2) {
    _Bool _0;
    const struct input_Parity *_3;
    intptr_t _4;
    const struct input_Parity *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_72__assert_receiver_is_total_eq(const struct input_Parity *_1) {
    bb_0: {
        return;
    }
}
inline struct input_StopBits input__impl_74__clone(const struct input_StopBits *_1) {
    struct input_StopBits _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_77__eq(const struct input_StopBits *_1, const struct input_StopBits *_2) {
    _Bool _0;
    const struct input_StopBits *_3;
    intptr_t _4;
    const struct input_StopBits *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_78__assert_receiver_is_total_eq(const struct input_StopBits *_1) {
    bb_0: {
        return;
    }
}
inline struct input_LineFormat input__impl_80__clone(const struct input_LineFormat *_1) {
    struct input_LineFormat _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_83__eq(const struct input_LineFormat *_1, const struct input_LineFormat *_2) {
    _Bool _0;
    uint32_t _3;
    uint32_t _4;
    _Bool _5;
    const struct input_DataBits *_6;
    const struct input_DataBits *_7;
    _Bool _8;
    const struct input_Parity *_9;
    const struct input_Parity *_10;
    _Bool _11;
    const struct input_StopBits *_12;
    const struct input_StopBits *_13;
    bb_0: {
        _3 = (*_1).baud;
        _4 = (*_2).baud;
        _5 = ((_3) == (_4));
        switch (_5) { case 0ULL: goto bb_9; default: goto bb_1; }
    }
    bb_1: {
        _6 = &((*_1).data_bits);
        _7 = &((*_2).data_bits);
        _8 = input__impl_65__eq(_6, _7); goto bb_2;
    }
    bb_2: {
        switch (_8) { case 0ULL: goto bb_8; default: goto bb_3; }
    }
    bb_3: {
        _9 = &((*_1).parity);
        _10 = &((*_2).parity);
        _11 = input__impl_71__eq(_9, _10); goto bb_4;
    }
    bb_4: {
        switch (_11) { case 0ULL: goto bb_7; default: goto bb_5; }
    }
    bb_5: {
        _12 = &((*_1).stop_bits);
        _13 = &((*_2).stop_bits);
        _0 = input__impl_77__eq(_12, _13); goto bb_6;
    }
    bb_6: {
        goto bb_11;
    }
    bb_7: {
        goto bb_10;
    }
    bb_8: {
        goto bb_10;
    }
    bb_9: {
        goto bb_10;
    }
    bb_10: {
        _0 = 0;
        goto bb_11;
    }
    bb_11: {
        return _0;
    }
}
inline void input__impl_84__assert_receiver_is_total_eq(const struct input_LineFormat *_1) {
    bb_0: {
        return;
    }
}
inline struct input_ConfigResult input__impl_86__clone(const struct input_ConfigResult *_1) {
    struct input_ConfigResult _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_89__eq(const struct input_ConfigResult *_1, const struct input_ConfigResult *_2) {
    _Bool _0;
    const struct input_ConfigResult *_3;
    intptr_t _4;
    const struct input_ConfigResult *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_90__assert_receiver_is_total_eq(const struct input_ConfigResult *_1) {
    bb_0: {
        return;
    }
}
inline struct input_SerialIrqSource input__impl_92__clone(const struct input_SerialIrqSource *_1) {
    struct input_SerialIrqSource _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_95__eq(const struct input_SerialIrqSource *_1, const struct input_SerialIrqSource *_2) {
    _Bool _0;
    const struct input_SerialIrqSource *_3;
    intptr_t _4;
    const struct input_SerialIrqSource *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_3).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((*_5).tag == ((uint8_t)(1U)) ? ((intptr_t)(1ULL)) : ((intptr_t)(2ULL))));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_96__assert_receiver_is_total_eq(const struct input_SerialIrqSource *_1) {
    bb_0: {
        return;
    }
}
inline struct input_IrqSourceState input__impl_98__clone(const struct input_IrqSourceState *_1) {
    struct input_IrqSourceState _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_101__eq(const struct input_IrqSourceState *_1, const struct input_IrqSourceState *_2) {
    _Bool _0;
    const struct input_IrqSourceState *_3;
    intptr_t _4;
    const struct input_IrqSourceState *_5;
    intptr_t _6;
    intptr_t _7;
    intptr_t _8;
    bb_0: {
        _3 = &((*_1));
        _4 = ((*_3).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _5 = &((*_2));
        _6 = ((*_5).tag == ((uint8_t)(0U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL)));
        _7 = _4;
        _8 = _6;
        _0 = ((_7) == (_8));
        return _0;
    }
}
inline void input__impl_102__assert_receiver_is_total_eq(const struct input_IrqSourceState *_1) {
    bb_0: {
        return;
    }
}
inline struct input_CauseSet input__impl_104__clone(const struct input_CauseSet *_1) {
    struct input_CauseSet _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_107__eq(const struct input_CauseSet *_1, const struct input_CauseSet *_2) {
    _Bool _0;
    uint32_t _3;
    uint32_t _4;
    bb_0: {
        _3 = (*_1).bits;
        _4 = (*_2).bits;
        _0 = ((_3) == (_4));
        return _0;
    }
}
inline void input__impl_108__assert_receiver_is_total_eq(const struct input_CauseSet *_1) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
const uint8_t input__regseq_synth__const_memory_static_0a62532ae19ec6a93be2423d964ec66b[14] = { ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(105U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(117U)), ((uint8_t)(112U)) };
const uint8_t input__regseq_synth__const_memory_static_14be334d3c723feb7f43498c995c5922[23] = { ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(105U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(112U)), ((uint8_t)(114U)), ((uint8_t)(111U)), ((uint8_t)(98U)), ((uint8_t)(101U)), ((uint8_t)(32U)), ((uint8_t)(115U)), ((uint8_t)(116U)), ((uint8_t)(97U)), ((uint8_t)(114U)), ((uint8_t)(116U)) };
const uint8_t input__regseq_synth__const_memory_static_323c8dea91d6f57c9b1afd93bfd9776f[25] = { ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(105U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(108U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(101U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(98U)), ((uint8_t)(97U)), ((uint8_t)(117U)), ((uint8_t)(100U)), ((uint8_t)(61U)), ((uint8_t)(123U)), ((uint8_t)(125U)) };
const uint8_t input__regseq_synth__const_memory_static_770590b04d61205c05c40de7a4cba37e[38] = { ((uint8_t)(115U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(105U)), ((uint8_t)(97U)), ((uint8_t)(108U)), ((uint8_t)(112U)), ((uint8_t)(108U)), ((uint8_t)(48U)), ((uint8_t)(49U)), ((uint8_t)(49U)), ((uint8_t)(32U)), ((uint8_t)(100U)), ((uint8_t)(101U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(105U)), ((uint8_t)(116U)), ((uint8_t)(58U)), ((uint8_t)(32U)), ((uint8_t)(109U)), ((uint8_t)(97U)), ((uint8_t)(115U)), ((uint8_t)(107U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(103U)), ((uint8_t)(32U)), ((uint8_t)(105U)), ((uint8_t)(110U)), ((uint8_t)(116U)), ((uint8_t)(101U)), ((uint8_t)(114U)), ((uint8_t)(114U)), ((uint8_t)(117U)), ((uint8_t)(112U)), ((uint8_t)(116U)), ((uint8_t)(115U)) };
const uint8_t input__regseq_synth__const_memory_static_b45482224b439a3d548c65378929b7dc[0] = {0};
struct __rseq_serial_0_serialpl011_target {
    void *__rseq_dev;
    struct input_SerialPl011 inner;
    uint32_t __rseq_state;
};

static void rseq_serial_isr_serialpl011(void *__ctx) {
    struct __rseq_serial_0_serialpl011_target *__t = (struct __rseq_serial_0_serialpl011_target *)__ctx;
    struct input_CauseSet __c = input__impl_4__irq_cause_decode(&__t->inner);
    if ((__c.bits & 6u) != 0u) { input__impl_2__data_unit_read(&__t->inner); }
    if ((__c.bits & 1u) != 0u) {
        input__impl_4__irq_source_set(&__t->inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(0U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(1U)) });
    }
}

static void __rseq_serial_0_serialpl011(void) {
    struct __rseq_serial_0_serialpl011_target inst = {0};
    regseq_lc_register("serialpl011", "amba:0x41011/0xfffff");
    regseq_lc_create();
    regseq_lc_associate();
    regseq_lc_preamble("regs");
    regseq_irq_register(37u, rseq_serial_isr_serialpl011, (void *)&inst);
    regseq_lc_call("on_init");
    input__impl_1__on_init(&inst.inner);
    regseq_irq_enable(37u);
    input__impl_2__port_service_set(&inst.inner, (struct input_PortScope){ .tag = ((uint8_t)(0U)) }, 0);
    input__impl_3__line_format_apply(&inst.inner, (struct input_LineFormat){ ((uint32_t)(115200U)), (struct input_DataBits){ .tag = ((uint8_t)(3U)) }, (struct input_Parity){ .tag = ((uint8_t)(0U)) }, (struct input_StopBits){ .tag = ((uint8_t)(0U)) } });
    input__impl_2__port_service_set(&inst.inner, (struct input_PortScope){ .tag = ((uint8_t)(0U)) }, 1);
    input__impl_4__irq_source_set(&inst.inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(1U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(0U)) });
    input__impl_4__irq_source_set(&inst.inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(2U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(0U)) });
    input__impl_4__irq_source_set(&inst.inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(0U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(0U)) });
    input__impl_2__data_ready_query(&inst.inner, (struct input_ReadyDir){ .tag = ((uint8_t)(0U)) });
    input__impl_2__data_unit_write(&inst.inner, ((uint32_t)(65U)));
    regseq_irq_dev_poke(0x040u, 0x20u);
    regseq_irq_fire(37u);
    regseq_irq_dev_poke(0x000u, 0x55u);
    regseq_irq_dev_poke(0x040u, 0x10u);
    regseq_irq_fire(37u);
    regseq_irq_dev_poke(0x000u, 0x56u);
    regseq_irq_dev_poke(0x040u, 0x40u);
    regseq_irq_fire(37u);
    input__impl_4__irq_source_set(&inst.inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(1U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(1U)) });
    input__impl_4__irq_source_set(&inst.inner, (struct input_SerialIrqSource){ .tag = ((uint8_t)(2U)) }, (struct input_IrqSourceState){ .tag = ((uint8_t)(1U)) });
    input__impl_2__port_service_set(&inst.inner, (struct input_PortScope){ .tag = ((uint8_t)(0U)) }, 0);
    regseq_lc_call("on_deinit");
    input__impl_1__on_deinit(&inst.inner);
    regseq_irq_disable(37u);
    regseq_irq_unregister(37u);
}

void __rseq_lc_run(void) {
    __rseq_serial_0_serialpl011();
}
