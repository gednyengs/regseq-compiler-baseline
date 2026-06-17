//! The model-independent bank-1 prelude (`generic-c-backend.md` section 15
//! bank 1) plus the fixed arithmetic / cast helper block (section 10 /
//! section 11).
//!
//! Everything in this module is a pure function of nothing (a `const`) or of
//! a fixed type set (`helper_block`); none of it reads the model. The per-`T`
//! fat-pointer structs (`rust_slice_<T>`, `trait_obj_<Trait>`, section 3.2)
//! are type-driven and belong to task c -- only the monomorphic
//! `struct rust_str` ships here. Use-site lowering (the `*WithOverflow` /
//! saturating helpers at `BinaryOp` / `Cast` sites) is task d; this module
//! emits only their definitions.
//!
//! All file-scope helpers are `static inline`: in a single translation unit
//! an unused `static inline` definition is warning-free under `-Wall`,
//! whereas a plain `static` routine would warn "defined but not used" when a
//! TU has no call site (e.g. the standalone-prelude compile test).

use std::collections::BTreeSet;

/// The static prelude: includes, the section 3.2 `struct rust_str`, the
/// section 16 panic infrastructure, and the section 16 `AssertKind` message
/// catalog. Emitted verbatim as bank 1's leading, model-independent text.
///
/// Compiles standalone under the section 7 flags (`-std=gnu11 -fwrapv
/// -Wall`) with no warnings.
pub const PRELUDE: &str = r#"/* ===========================================================================
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
"#;

/// Device-lifecycle trace-hook block (device-lifecycle construct, GRILL C01).
///
/// Unlike the always-on register-access block above, this is emitted ONLY for a
/// TU that has recognized lifecycle drivers (the caller gates on
/// `model.device_lifecycle.drivers`), so a no-lifecycle TU's `.c` is unchanged
/// — no golden churn. Each helper prints an `LC <event>` line to stdout: that
/// trace IS the behavioral oracle for lifecycle lowering (the `expected.stdout`
/// half of the dual oracle). `unused` keeps an unreferenced helper warning-free
/// within a lifecycle TU. The exact line spelling is blessed at the first
/// lifecycle golden (OI-3); the shape + ordering is the contract.
pub const LIFECYCLE_HELPERS: &str = r#"
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
"#;

/// Device-interrupt host line model (construct #6, GRILL C05).
///
/// Emitted ONLY for a TU with recognized `Interrupt` resources (the caller
/// gates on `model.interrupt.drivers`), so an interrupt-less TU's `.c` is
/// byte-unchanged — no golden churn. Each hook prints an `IRQ <event>` line to
/// stdout: that trace IS the behavioral oracle for the placement contract
/// (C01 made observable, I07). `regseq_irq_fire` is SYNCHRONOUS (GRILL D07,
/// superseding the buildout-PLAN "host thread" note): the harness calls it
/// between scenario steps and it invokes the registered wrapper inline iff
/// the line is enabled; otherwise it traces the inert form — the O1 spelling
/// is minted HERE as `IRQ fire line=<n> inert` and goldens assert it
/// positively. The supplementary `IRQ binding`/`IRQ result`/`IRQ poke` line
/// spellings are likewise harness-named (O1 latitude); the five-hook family
/// itself is the C05 contract.
pub const IRQ_HELPERS: &str = r#"
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
"#;

/// Bus-resource-management host bus-model block (construct #3, the keystone).
///
/// Emitted ONLY for a TU that has a resolved bus-resource binding (the caller
/// gates on `model_has_bus_binding`), so a no-bus TU's `.c` is byte-unchanged —
/// the existing register-access / diag-log / lifecycle goldens do not churn.
///
/// This is the host realization of bus-resource's keystone seam: the resolved
/// `(base, size)` (from the platform file's `[mmio.<id>]` literals, GRILL D06)
/// is installed at runtime by `regseq_bus_map_region` in the device-lifecycle
/// preamble; the based MMIO accessors then print the REAL mapped address
/// (`base + off`), not the faked global-regfile offset. Swapping the literal
/// source for a real `pci_resource_start` / `DT_REG_ADDR` changes no
/// register-access operation (I04) — only what `map_region` is handed.
///
/// The based accessors mirror the always-on `regseq_mmio_read/write` byte
/// machinery over `regseq_regfile` (relative offsets), but trace the absolute
/// mapped address. Each `BUS <event>` / based-access line is the behavioral
/// oracle for the keystone (the mapped base = the declared `[mmio.<id>] base`).
///
/// Construct #3 M3 adds the full four compiler-managed preamble ops in canonical
/// order `enable_device → claim_regions → set_bus_master → get_resource_handle`
/// (then register-access's `map_region`). The gating (which ops emit) is decided
/// in `emit_lifecycle_synthesis`; these helpers are the host realizations
/// (`enable`/`master` record host-model state, `claim_regions` is a trace-only
/// no-op on the host — the three real claim realizations are backend-[Open]).
pub const BUS_RESOURCE_HELPERS: &str = r#"
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
"#;

/// Timed-cpu-delay-and-sleep host fake-clock hooks (TCD construct, C06/D08): the
/// extern DECLARATIONS the `busy_wait`/`sleep` lowering (`emit_delay`) calls,
/// emitted only for a TU that uses a delay intrinsic. Unlike the other host
/// blocks above, this declares but does NOT define the hooks: the recording
/// implementation (append `{kind, ns, seq}` to an event log and advance a
/// VIRTUAL clock — no real time passes) is linked by the test harness, so the
/// same emitted TU is host-faithfully observable without baking the fake clock
/// into compiler output (the T05/T06 split). Intent is preserved as hook
/// identity: a `busy_wait` only ever calls `regseq_delay_busy_wait_ns`, a
/// `sleep` only `regseq_delay_sleep_ns` (I04).
pub const DELAY_HOOKS: &str = r#"
/* ---------------------------------------------------------------------------
 * Timed-cpu-delay-and-sleep host fake-clock hooks (TCD construct). DECLARATIONS
 * only; the recording implementation is linked by the test harness (C06/D08).
 * `busy_wait` lowers to regseq_delay_busy_wait_ns, `sleep` to
 * regseq_delay_sleep_ns -- the hook identity preserves author intent (I04). */
extern void regseq_delay_busy_wait_ns(uint64_t ns);
extern void regseq_delay_sleep_ns(uint64_t ns);
"#;

/// `(c_type, min_expr, max_expr)` for every primitive integer type in the
/// fixed helper set (section 10 / section 11). The order is fixed so the
/// emitted block is byte-stable. The 128-bit bounds reference the
/// `REGSEQ_(U)INT128_*` macros emitted at the top of the helper block (no
/// standard `*_MIN` / `*_MAX` macro exists for `__int128` / `__uint128_t`).
const INT_TYPES: &[(&str, &str, &str)] = &[
    ("int8_t", "INT8_MIN", "INT8_MAX"),
    ("int16_t", "INT16_MIN", "INT16_MAX"),
    ("int32_t", "INT32_MIN", "INT32_MAX"),
    ("int64_t", "INT64_MIN", "INT64_MAX"),
    ("intptr_t", "INTPTR_MIN", "INTPTR_MAX"),
    ("__int128", "REGSEQ_INT128_MIN", "REGSEQ_INT128_MAX"),
    ("uint8_t", "0", "UINT8_MAX"),
    ("uint16_t", "0", "UINT16_MAX"),
    ("uint32_t", "0", "UINT32_MAX"),
    ("uint64_t", "0", "UINT64_MAX"),
    ("size_t", "0", "SIZE_MAX"),
    ("__uint128_t", "0", "REGSEQ_UINT128_MAX"),
];

/// `(prefix, c_type, availability_macro)` for every source-float type of the
/// saturating `FloatToInt` helpers (section 11). `float` / `double` are
/// unconditional (empty macro); `_Float16` / `_Float128` are gated by a
/// source-level `#if defined(<macro>)`. The `#if` block is *always present in
/// the emitted text* -- the preprocessor, not the Rust backend, decides
/// whether to compile it -- so the emitted `.c` is byte-identical across
/// hosts (Doc 3 section 7), not branched on the build host.
const FLOAT_TYPES: &[(&str, &str, &str)] = &[
    ("f32", "float", ""),
    ("f64", "double", ""),
    ("f16", "_Float16", "__FLT16_MANT_DIG__"),
    ("f128", "_Float128", "__FLT128_MANT_DIG__"),
];

/// The arithmetic / cast helpers a translation unit actually references.
/// `emit_generic_c` fills this while emitting bank-5 bodies — each
/// `*WithOverflow` / `FloatToInt` site records the exact helper name it emits —
/// then [`helper_block`] emits only the recorded helpers (demand-driven; plan
/// 10). This replaces the former fixed superset, which clobbered every TU with
/// ~96 `static inline` definitions whether used or not.
#[derive(Default)]
pub struct HelperUsage {
    /// Overflow wrapper function names emitted, e.g. `regseq_add_int32_t`.
    wrappers: BTreeSet<String>,
    /// Saturating-cast function names emitted, e.g. `regseq_f64_to_int8_t`.
    casts: BTreeSet<String>,
    /// Whether the `RegseqError` -> errno boundary adapter (regseq-backends-28,
    /// regseq-error C02 / C03) is emitted, requiring a demand-driven
    /// `#include <errno.h>` (routed through this machinery per open item O4, so
    /// the include lands in bank 1 with the other includes rather than adjacent
    /// to the adapter at the end of the TU).
    errno_h: bool,
}

impl HelperUsage {
    /// Record an overflow wrapper call by its emitted function name.
    pub fn record_wrapper(&mut self, name: String) {
        self.wrappers.insert(name);
    }

    /// Record a saturating-cast call by its emitted function name.
    pub fn record_cast(&mut self, name: String) {
        self.casts.insert(name);
    }

    /// Record that the errno boundary adapter is emitted, so [`helper_block`]
    /// emits a demand-driven `#include <errno.h>` (regseq-backends-28).
    pub fn record_errno_include(&mut self) {
        self.errno_h = true;
    }

    /// Every helper over the fixed `INT_TYPES` / `FLOAT_TYPES` tables (the
    /// complete superset). Used by the reserved-identifier pin and the
    /// standalone helper-library test, which verify the full library
    /// independent of any one model's demand.
    pub fn all() -> Self {
        let mut u = Self::default();
        for (ty, _, _) in INT_TYPES {
            for op in ["add", "sub", "mul"] {
                u.wrappers.insert(format!("regseq_{op}_{ty}"));
            }
        }
        for (prefix, _, _) in FLOAT_TYPES {
            for (ity, _, _) in INT_TYPES {
                u.casts.insert(format!("regseq_{prefix}_to_{ity}"));
            }
        }
        u
    }

    /// Whether the named overflow wrapper was recorded.
    fn wrapper_used(&self, name: &str) -> bool {
        self.wrappers.contains(name)
    }

    /// Whether the named saturating cast was recorded.
    fn cast_used(&self, name: &str) -> bool {
        self.casts.contains(name)
    }

    /// Whether type `ty` needs its `struct regseq_overflow_<ty>` — true iff any
    /// `add`/`sub`/`mul` wrapper for `ty` was recorded. The struct is the
    /// wrapper return type and is referenced nowhere else (plan 10), so it is
    /// co-emitted with its wrappers.
    fn struct_used(&self, ty: &str) -> bool {
        ["add", "sub", "mul"]
            .iter()
            .any(|op| self.wrappers.contains(&format!("regseq_{op}_{ty}")))
    }
}

/// Emit the arithmetic / cast helper block (section 10.1 `*WithOverflow` +
/// section 11 saturating `FloatToInt`), demand-driven over `usage` (plan 10).
///
/// Only recorded helpers are emitted: a `regseq_{add,sub,mul}_<T>` wrapper iff
/// its name is in `usage`, a `struct regseq_overflow_<T>` iff any wrapper for
/// `T` is, and a `regseq_<f-prefix>_to_<T>` cast iff its name is. Rows are
/// walked in the fixed `INT_TYPES` / `FLOAT_TYPES` order (not the recorded set),
/// so the emitted block is byte-stable. The block header and 128-bit bound
/// macros are emitted unconditionally (harmless `#define`s). Naming scheme
/// (collision-free + stable across hosts):
///
/// - result struct: `struct regseq_overflow_<T>` (`{ <T> v; _Bool overflow; }`)
/// - overflow wrapper: `regseq_{add,sub,mul}_<T>(<T> a, <T> b)`
/// - saturating cast: `regseq_<f-prefix>_to_<T>(<F> x)`
///
/// where `<T>` is the integer C type spelling and `<f-prefix>` is
/// `f32`/`f64`/`f16`/`f128`.
pub fn helper_block(usage: &HelperUsage) -> String {
    let mut out = String::new();

    // regseq-backends-28 (regseq-error C02 / C03): the demand-driven errno
    // include for the boundary adapter. Emitted here (open item O4) so it sits in
    // bank 1 alongside the prelude includes, keeping the emitted `.c` byte-stable;
    // omitted entirely when no adapter is emitted (so existing unit-void / b4_enums
    // goldens are unchanged, invariant I02).
    if usage.errno_h {
        out.push_str(
            "\n/* regseq-error C02 / C03: the RegseqError -> errno boundary adapter, demand-driven. */\n\
             #include <errno.h>\n",
        );
    }

    out.push_str(
        "\n\
/* ---------------------------------------------------------------------------\n\
 * Arithmetic / cast helper block (section 10.1 + section 11), demand-driven.\n\
 * --------------------------------------------------------------------------- */\n\
\n\
/* 128-bit bounds: no standard *_MIN / *_MAX macro exists for __int128 /\n\
 * __uint128_t, so compute them (host two's-complement assumption). */\n\
#define REGSEQ_INT128_MAX ((__int128)(((__uint128_t)1 << 127) - 1))\n\
#define REGSEQ_INT128_MIN (-REGSEQ_INT128_MAX - 1)\n\
#define REGSEQ_UINT128_MAX (~(__uint128_t)0)\n\
\n",
    );

    // section 10.1: the `*WithOverflow` result structs + builtin wrappers,
    // emitted only for the (op, T) pairs the bodies actually use.
    if !usage.wrappers.is_empty() {
        out.push_str("/* section 10.1: {add,sub,mul}WithOverflow -> (T, bool). */\n");
    }
    for (ty, _min, _max) in INT_TYPES {
        if usage.struct_used(ty) {
            out.push_str(&format!(
                "struct regseq_overflow_{ty} {{ {ty} v; _Bool overflow; }};\n"
            ));
        }
        for op in ["add", "sub", "mul"] {
            if !usage.wrapper_used(&format!("regseq_{op}_{ty}")) {
                continue;
            }
            out.push_str(&format!(
                "static inline struct regseq_overflow_{ty} regseq_{op}_{ty}({ty} a, {ty} b) {{\n\
                 \x20\x20\x20\x20struct regseq_overflow_{ty} r;\n\
                 \x20\x20\x20\x20r.overflow = __builtin_{op}_overflow(a, b, &r.v);\n\
                 \x20\x20\x20\x20return r;\n\
                 }}\n"
            ));
        }
    }

    // section 11: saturating FloatToInt (bounds checked *before* the cast;
    // NaN -> 0; +/-inf handled by the comparisons), emitted only for the
    // (prefix, T) casts the bodies actually use.
    if !usage.casts.is_empty() {
        out.push_str("\n/* section 11: saturating FloatToInt (NaN -> 0; bounds before cast). */\n");
    }
    for (prefix, fty, avail_macro) in FLOAT_TYPES {
        let used: Vec<&(&str, &str, &str)> = INT_TYPES
            .iter()
            .filter(|(ity, _, _)| usage.cast_used(&format!("regseq_{prefix}_to_{ity}")))
            .collect();
        if used.is_empty() {
            continue;
        }
        let gated = !avail_macro.is_empty();
        if gated {
            out.push_str(&format!("#if defined({avail_macro})\n"));
        }
        for (ity, min, max) in used {
            out.push_str(&format!(
                "static inline {ity} regseq_{prefix}_to_{ity}({fty} x) {{\n\
                 \x20\x20\x20\x20if (isnan(x)) return 0;\n\
                 \x20\x20\x20\x20if (x <= ({fty}){min}) return {min};\n\
                 \x20\x20\x20\x20if (x >= ({fty}){max}) return {max};\n\
                 \x20\x20\x20\x20return ({ity})x;\n\
                 }}\n"
            ));
        }
        if gated {
            out.push_str("#endif\n");
        }
    }

    out
}
