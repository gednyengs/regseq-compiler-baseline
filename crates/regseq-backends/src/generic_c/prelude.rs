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

/// Peripheral-clock-management host clock model (clock construct, GRILL C05).
///
/// Emitted ONLY for a TU that has a recognized `Clock` resource (the caller
/// gates on `model_has_clock_resource`), so a no-clock TU's `.c` is
/// byte-unchanged (I07) — no golden churn. The host model is a GLOBAL singleton
/// with no device handle (matching `regseq_mmio_read(off,width)`): every hook is
/// keyed by the `const char* clk_id` (the L1 id), which is both the per-clock
/// state key and the trace identity. Each hook prints a `CLK <event>` line to
/// stdout — that trace IS the behavioral oracle for the clock contract (C05).
///
/// `acquire`/`release` are the generic-c realization of the compiler-managed
/// `acquire_clock` (C07): `acquire` records the clock's resolved
/// `default_frequency` so a later managed `get_rate` returns it (the oracle
/// value, D10); `release` clears the slot. They are emitted under BOTH modes
/// (static still records the frequency; no provider lookup). The static
/// const-fold of `enable`/`disable`/`get_rate` happens in the BACKEND (D07/T12),
/// so under a static binding those three ops do not reach these hooks at all.
/// `enable` returns `int` (`0` = ok) so the author's fallible `enable()?` lowers
/// against a status. The exact `CLK …` spelling is blessed at the first golden
/// (OI-3); the shape (five `clk_id`-keyed hooks, `CLK <event>` trace) is the
/// contract.
pub const CLOCK_HELPERS: &str = r#"
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
"#;

/// DMA-management host DMA device model (dma-management construct, T11 / GRILL
/// C11). Emitted only for a TU that declares ANY DMA resource (region / channel /
/// stream — [`super::body_emit::model_has_dma_resource`]), gated on RESOURCE
/// presence (NOT intrinsic presence), so the compiler-managed `alloc_coherent`/
/// `free_coherent` lifecycle preamble has its hooks even when the author calls no
/// `dma_*` op. A DMA-less TU's `.c` is byte-unchanged (no golden churn).
///
/// The model mirrors `CLOCK_HELPERS`: a GLOBAL singleton, no device handle, with
/// each hook printing a `DMA <event>` line — that trace IS the behavioral oracle
/// for the DMA contract (C05). The coherent-region path additionally allocates a
/// REAL backing buffer (a fixed-size static slab per region id) so an
/// `element_access` closure's typed field stores land in observable memory: the
/// behavioral oracle asserts the descriptor BYTES in that buffer. `alloc_coherent`
/// returns `(void* cpu, uint64_t dev_addr)`; the dev addr is synthetic (a per-slot
/// base) and is what `device_addr()`/`device_addr_of(i)` program into a base
/// register.
///
/// `regseq_dma_wmb` is the store→doorbell publishing fence (C07/D09): the golden
/// asserts it falls between the region stores and the MMIO doorbell write.
/// `regseq_dma_rmb` is DEFINED but never emitted this cycle (the completion-read
/// ordering is designed-not-built, D09). The channel hooks record the fake-engine
/// call sequence (PL011 path, not exercised by the E1000 M1 golden);
/// `register_completion` records a binding but the engine never fires the callback
/// (D06). The exact `DMA …` spelling is blessed at the first golden; the shape
/// (resource-gated hooks, `DMA <event>` trace, real coherent backing buffer) is
/// the contract.
///
/// REGSEQ_DMA_SLOTS is `#define`d just above this block, sized by the caller to the
/// TU's exact DMA-region count (like REGSEQ_CLOCK_SLOTS), so the slot table never
/// overflows and a region's backing buffer is never silently dropped.
pub const DMA_HELPERS: &str = r#"
/* ---------------------------------------------------------------------------
 * DMA-management host DMA device model (dma-management construct, C05/C11). Each
 * hook prints a "DMA <event>" line -- this trace IS the behavioral oracle.
 * Emitted only for a TU with a DMA resource. GLOBAL singleton, no device handle.
 * The coherent-region path keeps a REAL per-region backing buffer (a fixed slab)
 * so an element_access closure's typed stores land in observable memory; the
 * behavioral oracle asserts those descriptor bytes. REGSEQ_DMA_SLOTS is #defined
 * just above, sized to the TU's exact region count. */
#ifndef REGSEQ_DMA_REGION_BYTES
#define REGSEQ_DMA_REGION_BYTES 65536u  /* per-region backing slab cap */
#endif
/* The backing store is a heap pointer from an ALIGNED allocator (NOT an inline
 * `unsigned char buf[]`): an inline array carries only `unsigned char` alignment,
 * so a typed store/memcpy at the element stride would assume an alignment the
 * array cannot guarantee. `aligned_alloc` honors the region's declared `align`,
 * so the coherent backing buffer is well-aligned and the per-element memcpy is
 * defined (FIX #3). */
typedef struct {
    const char *region_id;
    uint64_t    dev_base;   /* synthetic device base address handed to device_addr() */
    void       *buf;        /* real coherent backing store (aligned heap pointer) */
} regseq_dma_region_slot_t;
static regseq_dma_region_slot_t regseq_dma_region_table[REGSEQ_DMA_SLOTS] __attribute__((unused));
static uint64_t regseq_dma_next_dev_base = 0xD0000000ull;  /* synthetic dev-addr allocator */
__attribute__((unused)) static regseq_dma_region_slot_t *regseq_dma_region_slot(const char *region_id) {
    for (uint32_t i = 0; i < REGSEQ_DMA_SLOTS; i++) {
        if (regseq_dma_region_table[i].region_id
            && strcmp(regseq_dma_region_table[i].region_id, region_id) == 0) {
            return &regseq_dma_region_table[i];
        }
    }
    return 0;
}
/* coherent region: allocate a REAL aligned backing buffer + a synthetic device
 * base. The buffer is `aligned_alloc(align, size)` so typed element stores land in
 * properly-aligned memory; `aligned_alloc` requires `align` to be a power of two
 * (and at least `sizeof(void*)`) and `size` to be a multiple of `align`, so both
 * are rounded up here. The passed `size`/`align` are now HONORED (no longer
 * ignored). */
__attribute__((unused)) static void *regseq_dma_alloc_coherent(const char *region_id, uint64_t size, uint64_t align, uint64_t *out_dev_addr) {
    printf("DMA alloc_coherent %s size=0x%llx\n", region_id, (unsigned long long)size);
    regseq_dma_region_slot_t *slot = regseq_dma_region_slot(region_id);
    if (!slot) {
        for (uint32_t i = 0; i < REGSEQ_DMA_SLOTS; i++) {
            if (!regseq_dma_region_table[i].region_id) { slot = &regseq_dma_region_table[i]; break; }
        }
    }
    if (!slot) { if (out_dev_addr) *out_dev_addr = 0; return 0; }
    /* round align up to a power of two, at least sizeof(void*). */
    uint64_t a = align ? align : 1u;
    if (a < sizeof(void *)) a = sizeof(void *);
    { uint64_t p = 1u; while (p < a) p <<= 1; a = p; }
    /* size: at least the requested bytes, rounded up to a multiple of `a`. */
    uint64_t sz = size ? size : a;
    sz = (sz + (a - 1u)) & ~(a - 1u);
    void *mem = aligned_alloc((size_t)a, (size_t)sz);
    if (!mem) { if (out_dev_addr) *out_dev_addr = 0; return 0; }
    memset(mem, 0, (size_t)sz);
    slot->region_id = region_id;
    slot->dev_base = regseq_dma_next_dev_base;
    regseq_dma_next_dev_base += REGSEQ_DMA_REGION_BYTES;
    slot->buf = mem;
    if (out_dev_addr) *out_dev_addr = slot->dev_base;
    return mem;
}
__attribute__((unused)) static void regseq_dma_free_coherent(const char *region_id) {
    printf("DMA free_coherent %s\n", region_id);
    regseq_dma_region_slot_t *slot = regseq_dma_region_slot(region_id);
    if (slot) { free(slot->buf); slot->buf = 0; slot->region_id = 0; }
}
/* the CPU pointer + device base of a region's backing buffer (element_access /
 * device_addr lowering reads these). */
__attribute__((unused)) static void *regseq_dma_region_cpu(const char *region_id) {
    regseq_dma_region_slot_t *slot = regseq_dma_region_slot(region_id);
    return slot ? slot->buf : 0;
}
__attribute__((unused)) static uint64_t regseq_dma_region_dev_base(const char *region_id) {
    regseq_dma_region_slot_t *slot = regseq_dma_region_slot(region_id);
    return slot ? slot->dev_base : 0;
}
/* DMA-coherent publishing/consuming fences. wmb() is the store->doorbell fence
 * (C07/D09): the golden asserts it brackets the region stores and the doorbell.
 * rmb() is DEFINED but not emitted this cycle (completion-read ordering, D09). */
__attribute__((unused)) static void regseq_dma_wmb(void) { printf("DMA wmb\n"); }
__attribute__((unused)) static void regseq_dma_rmb(void) { printf("DMA rmb\n"); }
/* streaming map/unmap/sync (PL011 path; designed, not exercised by the E1000 M1
 * golden). map returns a synthetic device address for the mapped CPU buffer. */
__attribute__((unused)) static uint64_t regseq_dma_stream(void *cpu, uint64_t len, int dir) {
    (void)cpu; printf("DMA stream len=0x%llx dir=%d\n", (unsigned long long)len, dir);
    uint64_t a = regseq_dma_next_dev_base; regseq_dma_next_dev_base += (len ? len : 1u); return a;
}
__attribute__((unused)) static void regseq_dma_unmap(uint64_t dev_addr, uint64_t len, int dir) {
    printf("DMA unmap addr=0x%llx len=0x%llx dir=%d\n", (unsigned long long)dev_addr, (unsigned long long)len, dir);
}
__attribute__((unused)) static void regseq_dma_acquire_from_device(uint64_t dev_addr, uint64_t len) {
    printf("DMA acquire_from_device addr=0x%llx len=0x%llx\n", (unsigned long long)dev_addr, (unsigned long long)len);
}
__attribute__((unused)) static void regseq_dma_release_to_device(uint64_t dev_addr, uint64_t len) {
    printf("DMA release_to_device addr=0x%llx len=0x%llx\n", (unsigned long long)dev_addr, (unsigned long long)len);
}
/* channel (fake DMA engine that records the call sequence; PL011 path). */
__attribute__((unused)) static int regseq_dma_request_channel(const char *id, int dir) {
    printf("DMA request_channel %s dir=%d\n", id, dir); return 0;
}
__attribute__((unused)) static void regseq_dma_release_channel(const char *id) {
    printf("DMA release_channel %s\n", id);
}
__attribute__((unused)) static void regseq_dma_configure(const char *id) {
    printf("DMA configure %s\n", id);
}
__attribute__((unused)) static int regseq_dma_stage_transfer(const char *id, uint64_t addr, uint64_t len, int dir) {
    printf("DMA stage_transfer %s addr=0x%llx len=0x%llx dir=%d\n", id, (unsigned long long)addr, (unsigned long long)len, dir); return 0;
}
__attribute__((unused)) static int regseq_dma_start_transfer(const char *id) {
    printf("DMA start_transfer %s\n", id); return 0;
}
__attribute__((unused)) static void regseq_dma_stop(const char *id) {
    printf("DMA stop %s\n", id);
}
__attribute__((unused)) static int regseq_dma_remaining_bytes(const char *id, uint64_t *out) {
    printf("DMA remaining_bytes %s\n", id); if (out) *out = 0; return 0;
}
/* completion binding: recorded, but the fake engine NEVER fires the callback
 * (D06 -- registration shape only this cycle). */
__attribute__((unused)) static void regseq_dma_register_completion(const char *id) {
    printf("DMA register_completion %s\n", id);
}
"#;

/// Pin-controller-and-GPIO-configuration host pinctrl model (pinctrl
/// state-selection construct, GRILL C05/D05).
///
/// Emitted ONLY for a TU that has a **resolved** pinctrl resource (the caller
/// gates on `model_has_resolved_pinctrl_resource`), so a pinctrl-less or
/// optional-absent-only TU's `.c` is byte-unchanged (I06/D06) — no golden churn.
/// An optional-absent resource emits NO hook call (its `apply` compile-time-folds
/// to a direct `Err(PinctrlAbsent)`, D06/C04), so it never reaches this block.
///
/// The host model is a GLOBAL singleton with no device handle (matching
/// `regseq_clock_*(clk_id)` / `regseq_mmio_read(off,width)`): every hook is keyed
/// by the `const char* id` (the L1 `#[rseq::pinctrl(id)]`), which is both the
/// per-resource state key (a bound flag) and the trace identity. Each hook prints
/// a `PIN <event>` line to stdout — that trace IS the behavioral oracle for the
/// pinctrl contract (C05).
///
/// `acquire` is the generic-c realization of the compiler-managed
/// `acquire_pinctrl` (C07/D08): it records the resource bound and establishes the
/// default state via a shared internal `bind` helper, emitting exactly ONE
/// `PIN acquire <id> <default_state>` line (NOT a second `PIN apply` — D05). It
/// returns `int` (`0` = ok): the required `PinCtrl` form consumes the rc and
/// hard-fails on nonzero; the optional form ignores it. `apply` is the author op:
/// it traces `PIN apply <id> <state>` and returns `int` (`0` = ok, nonzero ⇒ a
/// backend apply error surfaced as `Err`; the rc MUST be consumed — D05).
/// `PinctrlAbsent` is NEVER this rc; it is the compile-time-folded absent path.
/// `release` is the generic-c-oracle-only deinit (D08): it traces
/// `PIN release <id>` (NO state) and clears the slot. The exact `PIN …` spelling
/// is blessed at the first golden (OI-3); the shape (three `id`-keyed hooks,
/// event-specific `PIN <event>` trace) is the contract.
pub const PINCTRL_HELPERS: &str = r#"
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

/// Atomic-memory-access-and-ordering host oracle (atomicity construct, C05),
/// emitted only for a TU that uses an atomic op. Unlike `DELAY_HOOKS`, this is
/// the ALWAYS-ON `regseq_mmio_*` model: the atomic state lives in a host cell
/// table (indexed by the deterministic per-`Atomic`-field cell id), and each of
/// the six hooks applies value semantics to its cell, then prints an `ATM <op>`
/// line. That printed trace IS the behavioral oracle — value correctness,
/// ordering-tag preservation (the `Ordering` name is printed), the bit-test
/// prior bit, and the `compare_exchange` commit/contention branch are all
/// observable. It is NOT a weak-memory-soundness model (I06): the cells are
/// plain memory and the ordering tag is recorded, not enforced. The recording
/// impl is DEFINED here (self-contained), so the same emitted TU is host-
/// faithfully observable with no harness-linked clock. Two's-complement wrapping
/// + width masking are bit-level (uint64_t), so a signed element's wrapping/
/// sign-extension is correct without a separate signed path (C07/I04).
///
/// The caller emits a `#define REGSEQ_ATOMIC_CELLS <n>` immediately before this
/// block, sized to the TU's actual `Atomic`-field count
/// ([`super::body_emit::atomic_cell_count`]), so the cell table never overflows
/// regardless of how many `Atomic` fields a driver declares — there is no fixed
/// cap.
pub const ATOMIC_HELPERS: &str = r#"
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
"#;

/// Synchronization lock-discipline oracle (synchronization construct, GRILL
/// C03/S11). The hooks model lock state over the ordered event log and ASSERT
/// the first-slice discipline: balanced acquire/release, no double-acquire
/// (non-recursive), LIFO guard-drop, release-only-the-held. Each hook prints a
/// `SYNC <event>` line to stdout (the trace IS the behavioral oracle, the
/// `expected.stdout` half); a discipline violation prints `SYNC error <why>`
/// and `abort()`s (non-zero exit, caught by the behavioral oracle). Emitted
/// ONLY for a TU with a synchronization surface (the caller gates on
/// `model.synchronization.drivers`), so a lock-less TU's `.c` is byte-unchanged
/// — no golden churn. The hook family + the four asserted properties are the
/// C03/S11 contract; the exact line spellings are blessed at the first sync
/// golden. (The synchronous `regseq_irq_fire` IRQ-reentry leg is added in M3.)
pub const SYNC_HELPERS: &str = r#"
/* ---------------------------------------------------------------------------
 * Synchronization lock-discipline oracle (synchronization construct). Models
 * lock state over the event log and asserts balanced acquire/release, no
 * double-acquire (non-recursive), LIFO guard-drop, release-only-the-held. Each
 * hook prints a "SYNC <event>" line; a violation prints "SYNC error <why>" and
 * aborts. Emitted only for a TU with a synchronization surface. */
#define REGSEQ_LOCK_SLOTS 16u
static int regseq_lock_inited[REGSEQ_LOCK_SLOTS] __attribute__((unused));
static uint32_t regseq_lock_held_stack[REGSEQ_LOCK_SLOTS] __attribute__((unused));
static uint32_t regseq_lock_held_depth __attribute__((unused));
static int regseq_lock_atexit_armed __attribute__((unused));
/* End-of-run balance check: at normal exit no lock may still be held (every
 * acquire must have a matching release). A still-held lock is a leaked critical
 * section -- print + abort (non-zero exit), so a forgotten/escaped guard cannot
 * pass the oracle silently. */
__attribute__((unused)) static void regseq_lock_atexit_check(void) {
    if (regseq_lock_held_depth != 0) {
        printf("SYNC error lock-held-at-exit depth=%u\n", (unsigned)regseq_lock_held_depth);
        fflush(stdout);
        abort();
    }
}
__attribute__((unused)) static void regseq_lock_arm_atexit(void) {
    if (!regseq_lock_atexit_armed) {
        regseq_lock_atexit_armed = 1;
        atexit(regseq_lock_atexit_check);
    }
}
/* Every id must fit the fixed slot table; an id past it would silently bypass
 * the init/held tracking (a false-pass for a >REGSEQ_LOCK_SLOTS-lock TU), so it
 * aborts loud instead. With this guard every other hook may index the arrays
 * unconditionally. */
__attribute__((unused)) static void regseq_lock_bounds(uint32_t id) {
    if (id >= REGSEQ_LOCK_SLOTS) {
        printf("SYNC error lock-id-overflow id=%u slots=%u\n",
               (unsigned)id, (unsigned)REGSEQ_LOCK_SLOTS);
        fflush(stdout);
        abort();
    }
}
/* The C05/I05 acquire-after-init guard, shared by lock acquire + every
 * Completion op: the primitive must have been initialized by `init_primitives`
 * and not since destroyed before any use (use-before-init / use-after-destroy
 * is a leaked lifecycle). */
__attribute__((unused)) static void regseq_lock_require_inited(uint32_t id, const char *what) {
    regseq_lock_bounds(id);
    if (!regseq_lock_inited[id]) {
        printf("SYNC error %s-before-init id=%u\n", what, (unsigned)id);
        fflush(stdout);
        abort();
    }
}
__attribute__((unused)) static void regseq_lock_init(uint32_t id, const char *kind) {
    regseq_lock_arm_atexit();
    regseq_lock_bounds(id);
    printf("SYNC lock_init id=%u kind=%s\n", (unsigned)id, kind);
    regseq_lock_inited[id] = 1;
}
/* destroy_primitives (C05): the symmetric teardown placed AFTER the interrupt
 * disable. A still-held lock at destroy is a leaked critical section; marking
 * the slot un-inited makes a later acquire fail the acquire-after-init check
 * (the use-after-destroy half of I05). On Zephyr destroy is a structural no-op
 * (static primitives) — this hook is the generic-c oracle's realization. */
__attribute__((unused)) static void regseq_lock_destroy(uint32_t id) {
    regseq_lock_bounds(id);
    printf("SYNC lock_destroy id=%u\n", (unsigned)id);
    for (uint32_t i = 0; i < regseq_lock_held_depth; i++) {
        if (regseq_lock_held_stack[i] == id) {
            printf("SYNC error destroy-while-held id=%u\n", (unsigned)id);
            fflush(stdout);
            abort();
        }
    }
    regseq_lock_inited[id] = 0;
}
__attribute__((unused)) static void regseq_lock_acquire(uint32_t id, const char *kind) {
    regseq_lock_arm_atexit();
    /* Guard (bounds + acquire-after-init) BEFORE the trace event, so a violation
     * is not preceded by a normal-looking `lock_acquire` line. */
    regseq_lock_require_inited(id, "acquire");
    printf("SYNC lock_acquire id=%u kind=%s\n", (unsigned)id, kind);
    /* No double-acquire (non-recursive): id must not already be held. */
    for (uint32_t i = 0; i < regseq_lock_held_depth; i++) {
        if (regseq_lock_held_stack[i] == id) {
            printf("SYNC error double-acquire id=%u\n", (unsigned)id);
            fflush(stdout);
            abort();
        }
    }
    if (regseq_lock_held_depth >= REGSEQ_LOCK_SLOTS) {
        printf("SYNC error held-stack-overflow\n");
        fflush(stdout);
        abort();
    }
    regseq_lock_held_stack[regseq_lock_held_depth++] = id;
}
__attribute__((unused)) static void regseq_lock_release(uint32_t id) {
    regseq_lock_bounds(id);
    printf("SYNC lock_release id=%u\n", (unsigned)id);
    /* LIFO guard-drop + release-only-the-held: the top of the held stack must
     * be exactly id. */
    if (regseq_lock_held_depth == 0) {
        printf("SYNC error release-without-acquire id=%u\n", (unsigned)id);
        fflush(stdout);
        abort();
    }
    if (regseq_lock_held_stack[regseq_lock_held_depth - 1] != id) {
        printf("SYNC error non-LIFO-release id=%u\n", (unsigned)id);
        fflush(stdout);
        abort();
    }
    regseq_lock_held_depth--;
}
/* Completion one-shot signaling: point events traced for the behavioral oracle
 * (signal is ISR-safe; wait is sleeping/thread-only; reset re-arms). Each op
 * asserts the completion was initialized by `init_primitives` (C05/I05); no
 * held-state discipline applies to a Completion. */
__attribute__((unused)) static void regseq_completion_signal(uint32_t id) {
    regseq_lock_require_inited(id, "completion");
    printf("SYNC completion_signal id=%u\n", (unsigned)id);
}
__attribute__((unused)) static void regseq_completion_wait(uint32_t id) {
    regseq_lock_require_inited(id, "completion");
    printf("SYNC completion_wait id=%u\n", (unsigned)id);
}
__attribute__((unused)) static void regseq_completion_reset(uint32_t id) {
    regseq_lock_require_inited(id, "completion");
    printf("SYNC completion_reset id=%u\n", (unsigned)id);
}
"#;

/// The T13 `IrqSpinLock`↔interrupt coupling state, emitted **only** for a TU
/// with BOTH an interrupt surface AND a synchronization surface (so a
/// single-construct TU's `.c` — and its committed golden — is byte-unchanged).
/// `regseq_local_irq_masked` models the CPU local-IRQ-disable nesting depth an
/// `IrqSpinLock` raises (Linux `spin_lock_irqsave` / Zephyr `k_spin_lock`);
/// `regseq_lock_held_is_irqspin[]` records, per held-stack slot, whether that
/// entry is an `IrqSpinLock` (so `regseq_lock_release` can lower the mask
/// without a kind argument — preserving the `regseq_lock_release(id)` ABI).
/// The array size mirrors `REGSEQ_LOCK_SLOTS` (16) but is a literal so this
/// block carries no dependency on the (later-emitted) sync helper bank.
pub const MASK_STATE: &str = r#"
/* --------------------------------------------------------------------------
 * IrqSpinLock <-> interrupt reentry coupling (T13). Emitted only when a TU has
 * BOTH constructs. A held IrqSpinLock disables local IRQs, so a SYNCHRONOUS
 * regseq_irq_fire while one is held is MASKED (dropped, NOT queued/replayed --
 * the synchronous oracle models no pending delivery). This is the IRQ-reentry
 * behavioral check, short of true SMP. */
static uint32_t regseq_local_irq_masked __attribute__((unused));
static int regseq_lock_held_is_irqspin[16] __attribute__((unused)); /* [REGSEQ_LOCK_SLOTS] */
"#;

/// [`IRQ_HELPERS`] with the T13 mask-aware `regseq_irq_fire`: a synchronous fire
/// while `regseq_local_irq_masked > 0` is traced `IRQ fire line=<n> masked` and
/// does NOT invoke the wrapper (no reentry → no double-acquire). Built by a
/// targeted, unique-anchor replacement so the single-construct [`IRQ_HELPERS`]
/// const (and the interrupt-only goldens) stay byte-identical. Used only by the
/// both-construct emission path.
pub fn irq_helpers_masked() -> String {
    replace_once(
        IRQ_HELPERS,
        "static void regseq_irq_fire(uint32_t line) {\n",
        "static void regseq_irq_fire(uint32_t line) {\n    \
         /* T13: a held IrqSpinLock masks local IRQs; drop the fire (no replay). */\n    \
         if (regseq_local_irq_masked > 0u) {\n        \
         printf(\"IRQ fire line=%u masked\\n\", (unsigned)line);\n        return;\n    }\n",
    )
}

/// Replace EXACTLY one occurrence of `anchor` in `src`, asserting it is unique.
/// The T13 coupling weaves the mask logic into the IRQ/SYNC helper text by
/// targeted replacement; if a future edit to those consts changes or duplicates
/// an anchor, this fails loud at emit time rather than silently emitting an
/// un-coupled (and therefore wrong) helper bank.
fn replace_once(src: &str, anchor: &str, to: &str) -> String {
    assert_eq!(
        src.matches(anchor).count(),
        1,
        "T13 coupling anchor must occur exactly once in the helper text: {anchor:?}"
    );
    src.replacen(anchor, to, 1)
}

/// [`SYNC_HELPERS`] with the T13 mask maintenance woven into `regseq_lock_acquire`
/// / `regseq_lock_release`: acquiring an `IrqSpinLock` raises
/// `regseq_local_irq_masked` (and records the held-stack slot's kind); releasing
/// it lowers the mask. Built by targeted unique-anchor replacements so the
/// single-construct [`SYNC_HELPERS`] const stays byte-identical. Used only by the
/// both-construct emission path. (Kind `"IRQSPIN"` is the only lock label
/// starting with `'I'` — `"MUTEX"`/`"SPIN"` start with `M`/`S` — so the
/// first-char test reliably selects it without `<string.h>`.)
pub fn sync_helpers_masked() -> String {
    let acquire_masked = replace_once(
        SYNC_HELPERS,
        "    regseq_lock_held_stack[regseq_lock_held_depth++] = id;\n",
        "    regseq_lock_held_stack[regseq_lock_held_depth++] = id;\n    \
         /* T13: record IrqSpinLock-ness of this held slot; raise the IRQ mask. */\n    \
         regseq_lock_held_is_irqspin[regseq_lock_held_depth - 1u] = (kind && kind[0] == 'I');\n    \
         if (kind && kind[0] == 'I') { regseq_local_irq_masked++; }\n",
    );
    replace_once(
        &acquire_masked,
        "    regseq_lock_held_depth--;\n",
        "    /* T13: lower the IRQ mask if this released slot was an IrqSpinLock. */\n    \
         if (regseq_lock_held_is_irqspin[regseq_lock_held_depth - 1u]) { regseq_local_irq_masked--; }\n    \
         regseq_lock_held_depth--;\n",
    )
}

/// The §7.1 ISR-legality conformance/audit note (synchronization construct, M4
/// T16 / GRILL `I03`/`D08`). Emitted into a TU that has BOTH an interrupt and a
/// synchronization surface — exactly when the §7.1 ISR-legality check is live (an
/// ISR body acquiring a lock). It states the construct's guarantee scope
/// HONESTLY: the check is sound-but-incomplete (intraprocedural/lexical), so the
/// construct **never** claims "ISR-safety verified". This is the
/// thesis-honesty gate made visible in the emitted artifact.
pub const SYNC_ISR_SAFETY_AUDIT: &str = r#"
/* ---------------------------------------------------------------------------
 * RSEQ-SYNC-AUDIT (synchronization construct, §7.1 ISR-legality). The §7.1 check
 * (an ISR body may acquire only the IRQ-safe IrqSpinLock; Mutex/SpinLock::lock
 * and Completion::wait are rejected) is SOUND-BUT-INCOMPLETE: it is
 * intraprocedural/lexical, so a transitively-IRQ-reachable illegal acquisition
 * one helper-call deeper is NOT caught until the interprocedural hardening lands.
 * This artifact is therefore NOT "ISR-safety verified" — the construct claims
 * feasibility + analytic generalization, not proven completeness. */
"#;

/// The full T13-coupled helper bank for a both-construct TU: the §7.1 audit note,
/// the mask state, then the mask-aware IRQ + synchronization helpers. The mask
/// state precedes the helpers so the mask-aware `regseq_irq_fire` and lock ops
/// see the globals declared.
pub fn sync_irq_coupled_helpers() -> String {
    format!(
        "{}{}{}{}",
        SYNC_ISR_SAFETY_AUDIT,
        MASK_STATE,
        irq_helpers_masked(),
        sync_helpers_masked()
    )
}

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
