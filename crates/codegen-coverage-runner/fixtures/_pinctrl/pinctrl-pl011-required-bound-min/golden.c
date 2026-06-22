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
#define REGSEQ_PINCTRL_SLOTS 1u

/* ---------------------------------------------------------------------------
 * Pin-controller-and-GPIO-configuration host pinctrl model (pinctrl construct,
 * C05/D05). Each hook prints a "PIN <event>" line to stdout -- this trace IS the
 * behavioral oracle. Emitted only for a TU with a resolved pinctrl resource;
 * an optional-absent resource compile-time-folds to Err(PinctrlAbsent) and never
 * reaches here. GLOBAL singleton, no device handle; keyed by the const char* id.
 * REGSEQ_PINCTRL_SLOTS is #defined just above this block, sized by the caller to
 * the TU's exact resolved-pinctrl count (like REGSEQ_CLOCK_SLOTS) -- so the slot
 * table can never overflow and a resource is never silently dropped. */
typedef struct {
    const char *id;
    int bound;  /* 1 once acquired, 0 once released */
} regseq_pinctrl_slot_t;
static regseq_pinctrl_slot_t regseq_pinctrl_slot_table[REGSEQ_PINCTRL_SLOTS] __attribute__((unused));
__attribute__((unused)) static regseq_pinctrl_slot_t *regseq_pinctrl_slot(const char *id) {
    for (uint32_t i = 0; i < REGSEQ_PINCTRL_SLOTS; i++) {
        if (regseq_pinctrl_slot_table[i].id
            && strcmp(regseq_pinctrl_slot_table[i].id, id) == 0) {
            return &regseq_pinctrl_slot_table[i];
        }
    }
    return 0;
}
/* Shared internal helper: find-or-allocate a slot and mark it bound. Emits NO
 * trace line -- so `acquire` prints exactly one "PIN acquire" (not a second
 * "PIN apply") when it establishes the default state (D05). */
__attribute__((unused)) static regseq_pinctrl_slot_t *regseq_pinctrl_bind(const char *id) {
    regseq_pinctrl_slot_t *slot = regseq_pinctrl_slot(id);
    if (!slot) {
        for (uint32_t i = 0; i < REGSEQ_PINCTRL_SLOTS; i++) {
            if (!regseq_pinctrl_slot_table[i].id) {
                slot = &regseq_pinctrl_slot_table[i];
                break;
            }
        }
    }
    if (slot) {
        slot->id = id;
        slot->bound = 1;
    }
    return slot;
}
/* Acquire-failure injection knob (T18): the synthesized harness sets this to a
 * pinctrl id (via the `backend.pinctrl.fail-acquire` param) to force THAT id's
 * acquire to fail, exercising the required form's hard-fail. NULL (the default)
 * = acquire never fails. The failing acquire still TRACES its attempt ("PIN
 * acquire <id> <default>") then returns nonzero WITHOUT binding. */
static const char *regseq_pinctrl_fail_acquire_id __attribute__((unused)) = 0;
__attribute__((unused)) static int regseq_pinctrl_acquire(const char *id, const char *default_state) {
    printf("PIN acquire %s %s\n", id, default_state);
    if (regseq_pinctrl_fail_acquire_id
        && strcmp(regseq_pinctrl_fail_acquire_id, id) == 0) {
        return -1; /* injected acquire failure (T18) — not bound */
    }
    regseq_pinctrl_slot_t *slot = regseq_pinctrl_bind(id);
    return slot ? 0 : -1; /* 0 = ok; nonzero ⇒ acquire failure (required hard-fails) */
}
__attribute__((unused)) static int regseq_pinctrl_apply(const char *id, const char *state) {
    printf("PIN apply %s %s\n", id, state);
    regseq_pinctrl_slot_t *slot = regseq_pinctrl_slot(id);
    return (slot && slot->bound) ? 0 : -1; /* nonzero ⇒ backend apply error (Err) */
}
__attribute__((unused)) static void regseq_pinctrl_release(const char *id) {
    printf("PIN release %s\n", id);
    regseq_pinctrl_slot_t *slot = regseq_pinctrl_slot(id);
    if (slot) {
        slot->bound = 0;
        slot->id = 0; /* free the slot (a released resource is reclaimable) */
    }
}

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_Pl011Pin;
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c;
struct input_pl011_Regs;
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct input_PinCtrl;
struct tuple_cbf29ce484222325;
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818;
struct input_DefaultState;
struct input_SleepState;
struct input_pl011_dr_Register;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e {
    uint32_t _v;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e size");
_Static_assert(_Alignof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e) == 4, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e align");
_Static_assert(offsetof(struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e, _v) == 0, "input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e._v offset");
struct input_pl011_Regs {
    struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e dr;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_pl011_Regs) == 4, "input_pl011_Regs size");
_Static_assert(_Alignof(struct input_pl011_Regs) == 4, "input_pl011_Regs align");
_Static_assert(offsetof(struct input_pl011_Regs, dr) == 0, "input_pl011_Regs.dr offset");
struct input_Mmio__b43d312b4ad71a71283184a14b095a0c {
    struct input_pl011_Regs inner;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 4, "input_Mmio__b43d312b4ad71a71283184a14b095a0c size");
_Static_assert(_Alignof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c) == 4, "input_Mmio__b43d312b4ad71a71283184a14b095a0c align");
_Static_assert(offsetof(struct input_Mmio__b43d312b4ad71a71283184a14b095a0c, inner) == 0, "input_Mmio__b43d312b4ad71a71283184a14b095a0c.inner offset");
struct input_Pl011Pin {
    struct input_Mmio__b43d312b4ad71a71283184a14b095a0c regs;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Pl011Pin) == 4, "input_Pl011Pin size");
_Static_assert(_Alignof(struct input_Pl011Pin) == 4, "input_Pl011Pin align");
_Static_assert(offsetof(struct input_Pl011Pin, regs) == 0, "input_Pl011Pin.regs offset");
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
struct input_PinCtrl {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_PinCtrl) == 0, "input_PinCtrl size");
_Static_assert(_Alignof(struct input_PinCtrl) == 1, "input_PinCtrl align");
struct tuple_cbf29ce484222325 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct tuple_cbf29ce484222325) == 0, "tuple_cbf29ce484222325 size");
_Static_assert(_Alignof(struct tuple_cbf29ce484222325) == 1, "tuple_cbf29ce484222325 align");
struct core_marker_PhantomData__cb99244166901638c2061e4a3497818 {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 0, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 size");
_Static_assert(_Alignof(struct core_marker_PhantomData__cb99244166901638c2061e4a3497818) == 1, "core_marker_PhantomData__cb99244166901638c2061e4a3497818 align");
struct input_DefaultState {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_DefaultState) == 0, "input_DefaultState size");
_Static_assert(_Alignof(struct input_DefaultState) == 1, "input_DefaultState align");
struct input_SleepState {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_SleepState) == 0, "input_SleepState size");
_Static_assert(_Alignof(struct input_SleepState) == 1, "input_SleepState align");
struct input_pl011_dr_Register {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_pl011_dr_Register) == 0, "input_pl011_dr_Register size");
_Static_assert(_Alignof(struct input_pl011_dr_Register) == 1, "input_pl011_dr_Register align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__on_init(struct input_Pl011Pin *);
__attribute__((noinline)) void input__impl_1__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *, uint32_t);
const struct input_pl011_Regs *input__impl_4__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *);
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_5__apply__b7509ae662df0ba33120306a3198f673(const struct input_PinCtrl *);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_0__on_init(struct input_Pl011Pin *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_PinCtrl *_2;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _3;
    intptr_t _5;
    const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_6;
    const struct input_pl011_Regs *_7;
    const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_8;
    bb_0: {
        _2 = (const struct input_PinCtrl *)&((*_1));
        (void)regseq_pinctrl_apply("uart_pins", "sleep"); memcpy((unsigned char *)&(_3) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t)); goto bb_1;
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
        goto bb_9;
    }
    bb_6: {
        _6 = &((*_1).regs);
        _7 = input__impl_4__deref__b43d312b4ad71a71283184a14b095a0c(_6); goto bb_7;
    }
    bb_7: {
        _8 = &((*_7).dr);
        regseq_bus_mmio_write(0, 4, (uint64_t)(((uint32_t)(66U)))); goto bb_8;
    }
    bb_8: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_9;
    }
    bb_9: {
        return _0;
    }
}
__attribute__((noinline)) void input__impl_1__write__bf5254dbba1fccfbf0b5fe572846ab6e(const struct input_ReadWrite__bf5254dbba1fccfbf0b5fe572846ab6e *_1, uint32_t _2) {
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        return;
    }
}
const struct input_pl011_Regs *input__impl_4__deref__b43d312b4ad71a71283184a14b095a0c(const struct input_Mmio__b43d312b4ad71a71283184a14b095a0c *_1) {
    const struct input_pl011_Regs *_0;
    const struct input_pl011_Regs *_2;
    bb_0: {
        _2 = &((*_1).inner);
        _0 = &((*_2));
        return _0;
    }
}
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_5__apply__b7509ae662df0ba33120306a3198f673(const struct input_PinCtrl *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct __rseq_lc_0_pl011pin_target {
    void *__rseq_dev;
    struct input_Pl011Pin inner;
    uint32_t __rseq_state;
};

static void __rseq_lc_0_pl011pin_orderly(void) {
    regseq_lc_register("pl011pin", "amba:0x41011/0xfffff");
    regseq_bus_register_driver("amba", "bound");
    regseq_lc_create();
    regseq_lc_associate();
    regseq_lc_preamble("regs");
    regseq_bus_claim_regions(-1);
    regseq_bus_get_handle();
    regseq_bus_map_region(0x9000000ull, 0x1000ull, -1);
    if (regseq_pinctrl_acquire("uart_pins", "default") != 0) { printf("LC acquire-fail pinctrl %s\n", "uart_pins"); return; }
    struct __rseq_lc_0_pl011pin_target inst = {0};
    regseq_lc_call("on_init");
    input__impl_0__on_init(&inst.inner);
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    regseq_pinctrl_release("uart_pins");
    regseq_lc_postamble("regs", 0);
}

void __rseq_lc_run(void) {
    __rseq_lc_0_pl011pin_orderly();
}
