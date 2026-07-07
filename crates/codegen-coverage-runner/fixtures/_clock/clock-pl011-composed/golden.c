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
#define REGSEQ_CLOCK_SLOTS 1u

/* ---------------------------------------------------------------------------
 * Peripheral-clock-management host clock model (clock construct, C05). Each hook
 * prints a "CLK <event>" line to stdout -- this trace IS the behavioral oracle.
 * Emitted only for a TU with a Clock resource. GLOBAL singleton, no device
 * handle; keyed by the const char* clk_id. `acquire` records the resolved
 * default_frequency so a managed `get_rate` returns it (the oracle value).
 * REGSEQ_CLOCK_SLOTS is #defined just above this block, sized by the caller to
 * the TU's exact clock-resource count (like REGSEQ_ATOMIC_CELLS) -- so the slot
 * table can never overflow and a clock is never silently dropped. */
typedef struct {
    const char *clk_id;
    uint32_t rate;  /* recorded default_frequency in Hz (set at acquire) */
    int state;      /* 0 = released, 1 = acquired, 2 = enabled */
} regseq_clock_slot_t;
static regseq_clock_slot_t regseq_clock_slot_table[REGSEQ_CLOCK_SLOTS] __attribute__((unused));
__attribute__((unused)) static regseq_clock_slot_t *regseq_clock_slot(const char *clk_id) {
    for (uint32_t i = 0; i < REGSEQ_CLOCK_SLOTS; i++) {
        if (regseq_clock_slot_table[i].clk_id
            && strcmp(regseq_clock_slot_table[i].clk_id, clk_id) == 0) {
            return &regseq_clock_slot_table[i];
        }
    }
    return 0;
}
__attribute__((unused)) static void regseq_clock_acquire(const char *clk_id, uint32_t default_frequency) {
    printf("CLK acquire %s\n", clk_id);
    regseq_clock_slot_t *slot = regseq_clock_slot(clk_id);
    if (!slot) {
        for (uint32_t i = 0; i < REGSEQ_CLOCK_SLOTS; i++) {
            if (!regseq_clock_slot_table[i].clk_id) {
                slot = &regseq_clock_slot_table[i];
                break;
            }
        }
    }
    if (slot) {
        slot->clk_id = clk_id;
        slot->rate = default_frequency;
        slot->state = 1; /* acquired, not yet enabled */
    }
}
__attribute__((unused)) static void regseq_clock_release(const char *clk_id) {
    printf("CLK release %s\n", clk_id);
    regseq_clock_slot_t *slot = regseq_clock_slot(clk_id);
    if (slot) {
        slot->state = 0;
        slot->clk_id = 0; /* free the slot (a released clock is reclaimable) */
    }
}
__attribute__((unused)) static int regseq_clock_enable(const char *clk_id) {
    printf("CLK enable %s\n", clk_id);
    regseq_clock_slot_t *slot = regseq_clock_slot(clk_id);
    if (slot) slot->state = 2;
    return 0; /* 0 = ok */
}
__attribute__((unused)) static void regseq_clock_disable(const char *clk_id) {
    printf("CLK disable %s\n", clk_id);
    regseq_clock_slot_t *slot = regseq_clock_slot(clk_id);
    if (slot) slot->state = 1;
}
__attribute__((unused)) static uint32_t regseq_clock_get_rate(const char *clk_id) {
    regseq_clock_slot_t *slot = regseq_clock_slot(clk_id);
    uint32_t rate = slot ? slot->rate : 0u;
    printf("CLK rate %s = 0x%x\n", clk_id, (unsigned)rate);
    return rate;
}

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_Clock;
struct tuple_cbf29ce484222325;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct input_Pl011Composed;
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c;
struct input_pl011_Regs;
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e;
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c;
struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83;
struct input_ReadWrite__d743656c529b08e42785b912ffb751b0;
struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba;
struct input_WriteOnly__39471f0616179e541513e5a50bf71559;
struct input_IrqResult;
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83;
struct rust_slice_input_regseq_log_intrinsics_LogArg;
struct input_regseq_log_intrinsics_LogArg;
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818;
struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f;
struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17;
struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3;
struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd;
struct input_Interrupt;
struct input_pl011_cr_Register;
struct input_pl011_dr_Register;
struct input_pl011_icr_Register;
struct input_pl011_imsc_Register;
struct input_pl011_mis_Register;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_Clock {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_Clock) == 0, "input_Clock size");
_Static_assert(_Alignof(struct input_Clock) == 1, "input_Clock align");
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
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _reserved0[10];
    struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 cr;
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c ifls;
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
struct input_Pl011Composed {
    struct input_Mmio__b43d312b4ad71a71283184a14b095a0c regs;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Pl011Composed) == 72, "input_Pl011Composed size");
_Static_assert(_Alignof(struct input_Pl011Composed) == 4, "input_Pl011Composed align");
_Static_assert(offsetof(struct input_Pl011Composed, regs) == 0, "input_Pl011Composed.regs offset");
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
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 {
    uint32_t mask;
    uint32_t value;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 8, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 size");
_Static_assert(_Alignof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 align");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, mask) == 0, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.mask offset");
_Static_assert(offsetof(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, value) == 4, "input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83.value offset");
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
struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f) == 0, "core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f size");
_Static_assert(_Alignof(struct core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f) == 1, "core_marker_PhantomData__2974a5ae055dae657fdf978ea3d0cb3f align");
struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17) == 0, "core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17) == 1, "core_marker_PhantomData__44988074332eef1584ecc5fbf6a29e17 align");
struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3) == 0, "core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3) == 1, "core_marker_PhantomData__a6ce2439360a4ba2fa48b2259c6ce5d3 align");
struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd) == 0, "core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd size");
_Static_assert(_Alignof(struct core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd) == 1, "core_marker_PhantomData__ae10abed9e3303e99a36ab95b86fdfcd align");
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
struct input_pl011_icr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_icr_Register) == 0, "input_pl011_icr_Register size");
_Static_assert(_Alignof(struct input_pl011_icr_Register) == 1, "input_pl011_icr_Register align");
struct input_pl011_imsc_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_imsc_Register) == 0, "input_pl011_imsc_Register size");
_Static_assert(_Alignof(struct input_pl011_imsc_Register) == 1, "input_pl011_imsc_Register align");
struct input_pl011_mis_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_mis_Register) == 0, "input_pl011_mis_Register size");
_Static_assert(_Alignof(struct input_pl011_mis_Register) == 1, "input_pl011_mis_Register align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input__impl_0__disable(const struct input_Clock *);
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__enable(const struct input_Clock *);
__attribute__((noinline)) uint32_t input__impl_0__get_rate(const struct input_Clock *);
void input__impl_1__clear_all_interrupts(struct input_Pl011Composed *);
struct input_IrqResult input__impl_1__handle_irq(struct input_Pl011Composed *);
void input__impl_1__mask_all_interrupts(struct input_Pl011Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__on_deinit(struct input_Pl011Composed *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__on_init(struct input_Pl011Composed *);
inline struct input_IrqResult input__impl_4__clone(const struct input_IrqResult *);
inline _Bool input__impl_7__eq(const struct input_IrqResult *, const struct input_IrqResult *);
inline void input__impl_8__assert_receiver_is_total_eq(const struct input_IrqResult *);
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_11__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
__attribute__((noinline)) uint32_t input__impl_12__read__b3419fd74a8aecb75505fd74c030f9ba(const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *);
__attribute__((noinline)) void input__impl_13__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *, uint32_t);
__attribute__((noinline)) void input__impl_14__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *, uint32_t);
__attribute__((noinline)) void input__impl_14__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *, uint32_t);
__attribute__((noinline)) void input__impl_14__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83);
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_18__new(void);
const struct input_pl011_Regs *input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_debug(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_error(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_info(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str, struct rust_slice_input_regseq_log_intrinsics_LogArg);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) void input__impl_0__disable(const struct input_Clock *_1) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__enable(const struct input_Clock *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
__attribute__((noinline)) uint32_t input__impl_0__get_rate(const struct input_Clock *_1) {
    uint32_t _0;
    bb_0: {
        _0 = ((uint32_t)(0U));
        return _0;
    }
}
void input__impl_1__clear_all_interrupts(struct input_Pl011Composed *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_4;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).icr);
        regseq_bus_mmio_write(68, 4, (uint64_t)(((uint32_t)(2047U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
struct input_IrqResult input__impl_1__handle_irq(struct input_Pl011Composed *_1) {
    struct input_IrqResult _0;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *_4;
    uint32_t _5;
    uint32_t _6;
    _Bool _7;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_8;
    const struct input_pl011_Regs *_9;
    const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_10;
    uint32_t _11;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).mis);
        _5 = (uint32_t)regseq_bus_mmio_read(64, 4); goto bb_2;
    }
    bb_2: {
        _6 = _5;
        _7 = ((_6) == (((uint32_t)(0U))));
        switch (_7) { case 0ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        _0.tag = ((uint8_t)(1U));
        goto bb_7;
    }
    bb_4: {
        _8 = &((*_1).regs);
        _9 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_8); goto bb_5;
    }
    bb_5: {
        _10 = &((*_9).icr);
        _11 = _5;
        regseq_bus_mmio_write(68, 4, (uint64_t)(_11)); goto bb_6;
    }
    bb_6: {
        _0.tag = ((uint8_t)(0U));
        goto bb_7;
    }
    bb_7: {
        return _0;
    }
}
void input__impl_1__mask_all_interrupts(struct input_Pl011Composed *_1) {
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_2;
    const struct input_pl011_Regs *_3;
    const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_4;
    bb_0: {
        _2 = &((*_1).regs);
        _3 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_3).imsc);
        regseq_bus_mmio_write(56, 4, (uint64_t)(((uint32_t)(0U)))); goto bb_2;
    }
    bb_2: {
        return;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__on_deinit(struct input_Pl011Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    struct input_Pl011Composed *_2;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_4;
    const struct input_pl011_Regs *_5;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_6;
    struct input_Pl011Composed *_8;
    const struct input_Clock *_10;
    bb_0: {
        _2 = &((*_1));
        input__impl_1__mask_all_interrupts(_2); goto bb_1;
    }
    bb_1: {
        _4 = &((*_1).regs);
        _5 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_4); goto bb_2;
    }
    bb_2: {
        _6 = &((*_5).cr);
        regseq_bus_mmio_write(48, 4, (uint64_t)(((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(0U)) }).value & ((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(0U)) }).mask)); goto bb_3;
    }
    bb_3: {
        _8 = &((*_1));
        input__impl_1__clear_all_interrupts(_8); goto bb_4;
    }
    bb_4: {
        _10 = (const struct input_Clock *)&((*_1));
        regseq_clock_disable("uartclk"); goto bb_5;
    }
    bb_5: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__on_init(struct input_Pl011Composed *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_Clock *_2;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _3;
    intptr_t _5;
    const struct input_Clock *_6;
    uint32_t _7;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_8;
    const struct input_pl011_Regs *_9;
    const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_10;
    uint32_t _11;
    struct input_Pl011Composed *_13;
    struct input_Pl011Composed *_15;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_17;
    const struct input_pl011_Regs *_18;
    const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_19;
    struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _20;
    bb_0: {
        _2 = (const struct input_Clock *)&((*_1));
        (void)regseq_clock_enable("uartclk"); memcpy((unsigned char *)&(_3) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t)); goto bb_1;
    }
    bb_1: {
        goto bb_2;
    }
    bb_2: {
        { uint8_t _nd; memcpy(&_nd, (const unsigned char *)&(_3) + 0, sizeof(_nd)); _5 = (_nd == ((uint8_t)(2U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL))); }
        switch (_5) { case 0ULL: goto bb_6; case 1ULL: goto bb_4; default: goto bb_3; }
    }
    bb_3: {
        regseq_unreachable();
    }
    bb_4: {
        _0._0 = _3._0;
        goto bb_5;
    }
    bb_5: {
        goto bb_15;
    }
    bb_6: {
        _6 = (const struct input_Clock *)&((*_1));
        _7 = (uint32_t)regseq_clock_get_rate("uartclk"); goto bb_7;
    }
    bb_7: {
        _8 = &((*_1).regs);
        _9 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_8); goto bb_8;
    }
    bb_8: {
        _10 = &((*_9).dr);
        _11 = _7;
        regseq_bus_mmio_write(0, 4, (uint64_t)(_11)); goto bb_9;
    }
    bb_9: {
        _13 = &((*_1));
        input__impl_1__mask_all_interrupts(_13); goto bb_10;
    }
    bb_10: {
        _15 = &((*_1));
        input__impl_1__clear_all_interrupts(_15); goto bb_11;
    }
    bb_11: {
        _17 = &((*_1).regs);
        _18 = input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(_17); goto bb_12;
    }
    bb_12: {
        _19 = &((*_18).cr);
        _20 = input__impl_11__add__ae10abed9e3303e99a36ab95b86fdfcd((struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(1U)), .value = ((uint32_t)(1U)) }, (struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83){ .mask = ((uint32_t)(512U)), .value = ((uint32_t)(512U)) }); goto bb_13;
    }
    bb_13: {
        regseq_bus_mmio_write(48, 4, (uint64_t)((_20).value & (_20).mask)); goto bb_14;
    }
    bb_14: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_15;
    }
    bb_15: {
        return _0;
    }
}
inline struct input_IrqResult input__impl_4__clone(const struct input_IrqResult *_1) {
    struct input_IrqResult _0;
    bb_0: {
        _0 = (*_1);
        return _0;
    }
}
inline _Bool input__impl_7__eq(const struct input_IrqResult *_1, const struct input_IrqResult *_2) {
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
inline void input__impl_8__assert_receiver_is_total_eq(const struct input_IrqResult *_1) {
    bb_0: {
        return;
    }
}
struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 input__impl_11__add__ae10abed9e3303e99a36ab95b86fdfcd(struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
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
__attribute__((noinline)) uint32_t input__impl_12__read__b3419fd74a8aecb75505fd74c030f9ba(const struct input_ReadOnly__b3419fd74a8aecb75505fd74c030f9ba *_1) {
    uint32_t _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_13__write__39471f0616179e541513e5a50bf71559(const struct input_WriteOnly__39471f0616179e541513e5a50bf71559 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_14__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_14__write__d743656c529b08e42785b912ffb751b0(const struct input_ReadWrite__d743656c529b08e42785b912ffb751b0 *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
__attribute__((noinline)) void input__impl_14__write_field__efd54ac54c93b3a383b34f2f58eceb83(const struct input_ReadWrite__efd54ac54c93b3a383b34f2f58eceb83 *_1, struct input_FieldValue__efd54ac54c93b3a383b34f2f58eceb83 _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c input__impl_18__new(void) {
    struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c _0;
    bb_0: {
        _0 = (struct input_Reserved__5ae2a5e07d9ec8b0eb418e8c3168f23c){ ._v = ((uint32_t)(0U)) };
        return _0;
    }
}
const struct input_pl011_Regs *input__impl_20__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_1) {
    const struct input_pl011_Regs *_0;
    const struct input_pl011_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
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
__attribute__((noinline)) void input_regseq_log_intrinsics_log_info(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
__attribute__((noinline)) void input_regseq_log_intrinsics_log_warn(struct rust_str _1, struct rust_slice_input_regseq_log_intrinsics_LogArg _2) {
    bb_0: {
        return;
    }
}
struct __rseq_lc_0_pl011composed_target {
    void *__rseq_dev;
    struct input_Pl011Composed inner;
    uint32_t __rseq_state;
};

static void rseq_isr_pl011composed(void *__ctx) {
    struct __rseq_lc_0_pl011composed_target *__t = (struct __rseq_lc_0_pl011composed_target *)__ctx;
    struct input_IrqResult __r = input__impl_1__handle_irq(&__t->inner);
    if (__r.tag == ((uint8_t)(0U))) {
        regseq_irq_result("Handled");
    } else if (__r.tag == ((uint8_t)(1U))) {
        regseq_irq_result("NotHandled");
    } else {
        regseq_irq_result("DeferWork");
    }
}

static void __rseq_lc_0_pl011composed_orderly(void) {
    regseq_lc_register("pl011composed", "amba:0x41011/0xfffff");
    regseq_bus_register_driver("amba", "bound");
    regseq_lc_create();
    regseq_lc_associate();
    regseq_lc_preamble("regs");
    regseq_bus_claim_regions(-1);
    regseq_bus_get_handle();
    regseq_bus_map_region(0x9000000ull, 0x1000ull, -1);
    regseq_clock_acquire("uartclk", 24000000u);
    struct __rseq_lc_0_pl011composed_target inst = {0};
    regseq_irq_register(37u, rseq_isr_pl011composed, (void *)&inst);
    regseq_irq_binding(37u, "level", "active_high", 0, -1ll);
    regseq_lc_call("on_init");
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 __r = input__impl_2__on_init(&inst.inner);
    uint8_t __disc;
    { uint8_t __nd; memcpy(&__nd, (const unsigned char *)&(__r) + 0, sizeof(__nd)); __disc = (__nd == ((uint8_t)(2U)) ? ((uint8_t)(0U)) : ((uint8_t)(1U))); }
    if (__disc == ((uint8_t)(0U))) {
        regseq_irq_enable(37u);
    }
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    input__impl_2__on_deinit(&inst.inner);
    regseq_irq_disable(37u);
    regseq_irq_unregister(37u);
    regseq_clock_release("uartclk");
    regseq_lc_postamble("regs", 0);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_pl011composed_orderly();
}
