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
#define REGSEQ_DMA_SLOTS 1u

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
/* The symbolic byte length a streaming `dma::stream(buf, flow)` maps. The mapped
 * buffer's length is not on the L1 surface call (it is platform/runtime-derived),
 * so the generic-c host model uses a fixed non-zero symbolic length — the gc-shape
 * streaming oracle asserts the map/acquire/release/unmap CALL SEQUENCE, not exact
 * lengths (dma-management M4/T15). */
#ifndef REGSEQ_DMA_STREAM_LEN
#define REGSEQ_DMA_STREAM_LEN 4096u
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

/* ---------------------------------------------------------------------------
 * Bank 2 (section 15): aggregate tag forward-decls.
 * --------------------------------------------------------------------------- */
struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c;
struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea;
struct input_DmaRegionSpan;
struct input_DmaChannel;
struct tuple_cbf29ce484222325;
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486;
struct input_RegseqError;
struct input_Pl011Dma;
struct input_DmaCompletion;
struct core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c;

/* ---------------------------------------------------------------------------
 * Bank 3 (section 15): aggregate definitions + section 14 layout guards.
 * --------------------------------------------------------------------------- */
struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c) == 0, "input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c size");
_Static_assert(_Alignof(struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c) == 1, "input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c align");
struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea {
    size_t start;
    size_t end;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea) == 16, "core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea size");
_Static_assert(_Alignof(struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea) == 8, "core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea align");
_Static_assert(offsetof(struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea, start) == 0, "core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea.start offset");
_Static_assert(offsetof(struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea, end) == 8, "core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea.end offset");
struct input_DmaRegionSpan {
    uint64_t _addr;
    uint64_t _len;
} __attribute__((aligned(8)));
_Static_assert(sizeof(struct input_DmaRegionSpan) == 16, "input_DmaRegionSpan size");
_Static_assert(_Alignof(struct input_DmaRegionSpan) == 8, "input_DmaRegionSpan align");
_Static_assert(offsetof(struct input_DmaRegionSpan, _addr) == 0, "input_DmaRegionSpan._addr offset");
_Static_assert(offsetof(struct input_DmaRegionSpan, _len) == 8, "input_DmaRegionSpan._len offset");
struct input_DmaChannel {
    uint32_t _id;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_DmaChannel) == 4, "input_DmaChannel size");
_Static_assert(_Alignof(struct input_DmaChannel) == 4, "input_DmaChannel align");
_Static_assert(offsetof(struct input_DmaChannel, _id) == 0, "input_DmaChannel._id offset");
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
struct input_Pl011Dma {
    struct input_DmaChannel rx;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_Pl011Dma) == 4, "input_Pl011Dma size");
_Static_assert(_Alignof(struct input_Pl011Dma) == 4, "input_Pl011Dma align");
_Static_assert(offsetof(struct input_Pl011Dma, rx) == 0, "input_Pl011Dma.rx offset");
struct input_DmaCompletion {
    uint32_t status;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct input_DmaCompletion) == 4, "input_DmaCompletion size");
_Static_assert(_Alignof(struct input_DmaCompletion) == 4, "input_DmaCompletion align");
_Static_assert(offsetof(struct input_DmaCompletion, status) == 0, "input_DmaCompletion.status offset");
struct core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c {
} __attribute__((aligned(1)));
_Static_assert(sizeof(struct core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c) == 0, "core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c size");
_Static_assert(_Alignof(struct core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c) == 1, "core_marker_PhantomData__c6e1a215476aa64c90d09f7c790ac72c align");

/* ---------------------------------------------------------------------------
 * Bank 4 (section 15): prototypes + extern statics.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) struct input_DmaRegionSpan input__impl_0__span__c6e1a215476aa64c90d09f7c790ac72c(const struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c *, struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea);
__attribute__((noinline)) void input__impl_2__configure(const struct input_DmaChannel *);
struct input_DmaChannel input__impl_2__new(void);
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__stage_transfer__6856e4d5016799d3d6c1d839b3cd0657(const struct input_DmaChannel *, struct input_DmaRegionSpan);
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__start_transfer(const struct input_DmaChannel *);
__attribute__((noinline)) void input__impl_2__stop(const struct input_DmaChannel *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_3__rx_complete(struct input_Pl011Dma *, struct input_DmaCompletion);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_deinit(struct input_Pl011Dma *);
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_init(struct input_Pl011Dma *);

/* ---------------------------------------------------------------------------
 * Bank 5 (section 15): function bodies + static initializers.
 * --------------------------------------------------------------------------- */
__attribute__((noinline)) struct input_DmaRegionSpan input__impl_0__span__c6e1a215476aa64c90d09f7c790ac72c(const struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c *_1, struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea _2) {
    struct input_DmaRegionSpan _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_2__configure(const struct input_DmaChannel *_1) {
    bb_0: {
        return;
    }
}
struct input_DmaChannel input__impl_2__new(void) {
    struct input_DmaChannel _0;
    bb_0: {
        _0 = (struct input_DmaChannel){ ._id = ((uint32_t)(0U)) };
        return _0;
    }
}
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__stage_transfer__6856e4d5016799d3d6c1d839b3cd0657(const struct input_DmaChannel *_1, struct input_DmaRegionSpan _2) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_2__start_transfer(const struct input_DmaChannel *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        goto bb_1;
    }
    bb_1: {
        goto bb_1;
    }
}
__attribute__((noinline)) void input__impl_2__stop(const struct input_DmaChannel *_1) {
    bb_0: {
        return;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_3__rx_complete(struct input_Pl011Dma *_1, struct input_DmaCompletion _2) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    bb_0: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_deinit(struct input_Pl011Dma *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_DmaChannel *_2;
    bb_0: {
        _2 = &((*_1).rx);
        regseq_dma_stop("rx"); goto bb_1;
    }
    bb_1: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        return _0;
    }
}
struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 input__impl_4__on_init(struct input_Pl011Dma *_1) {
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _0;
    const struct input_DmaChannel *_2;
    const struct input_DmaChannel *_4;
    const struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c *_5;
    struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea _6;
    struct input_DmaRegionSpan _7;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _8;
    intptr_t _10;
    const struct input_DmaChannel *_11;
    struct core_result_Result__a96be460e4ad8f92502af3ac2c2f6486 _12;
    intptr_t _14;
    bb_0: {
        _2 = &((*_1).rx);
        regseq_dma_configure("rx"); goto bb_1;
    }
    bb_1: {
        _4 = &((*_1).rx);
        _5 = (const struct input_DmaRegion__c6e1a215476aa64c90d09f7c790ac72c *)&((*_1));
        _6 = (struct core_ops_range_Range__5a076906d29ccff5f85fa387d52d1aea){ .start = ((size_t)(0ULL)), .end = ((size_t)(32ULL)) };
        /* dma region.span(range) on rx_buf: value-symbolic endpoint; the consuming stage_transfer re-derives dev_base + offset*stride + len (M3/T26) */ goto bb_2;
    }
    bb_2: {
        { int __dma_rc = regseq_dma_stage_transfer("rx", (regseq_dma_region_dev_base("rx_buf") + 0ull * 1ull), 32ull, 0); if (__dma_rc == 0) { memcpy((unsigned char *)&(_8) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t)); } else { /* dma channel fallible Err branch (dead in the generic-c host model; rc is always 0) */  _8._0.tag = ((uint8_t)(0U)); } } goto bb_3;
    }
    bb_3: {
        goto bb_4;
    }
    bb_4: {
        { uint8_t _nd; memcpy(&_nd, (const unsigned char *)&(_8) + 0, sizeof(_nd)); _10 = (_nd == ((uint8_t)(2U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL))); }
        switch (_10) { case 0ULL: goto bb_7; case 1ULL: goto bb_5; default: goto bb_10; }
    }
    bb_5: {
        _0._0 = _8._0;
        goto bb_6;
    }
    bb_6: {
        goto bb_14;
    }
    bb_7: {
        _11 = &((*_1).rx);
        { int __dma_rc = regseq_dma_start_transfer("rx"); if (__dma_rc == 0) { memcpy((unsigned char *)&(_12) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t)); } else { /* dma channel fallible Err branch (dead in the generic-c host model; rc is always 0) */  _12._0.tag = ((uint8_t)(0U)); } } goto bb_8;
    }
    bb_8: {
        goto bb_9;
    }
    bb_9: {
        { uint8_t _nd; memcpy(&_nd, (const unsigned char *)&(_12) + 0, sizeof(_nd)); _14 = (_nd == ((uint8_t)(2U)) ? ((intptr_t)(0ULL)) : ((intptr_t)(1ULL))); }
        switch (_14) { case 0ULL: goto bb_13; case 1ULL: goto bb_11; default: goto bb_10; }
    }
    bb_10: {
        regseq_unreachable();
    }
    bb_11: {
        _0._0 = _12._0;
        goto bb_12;
    }
    bb_12: {
        goto bb_14;
    }
    bb_13: {
        memcpy((unsigned char *)&(_0) + 0, &(uint8_t){((uint8_t)(2U))}, sizeof(uint8_t));
        goto bb_14;
    }
    bb_14: {
        return _0;
    }
}
struct __rseq_lc_0_pl011dma_target {
    void *__rseq_dev;
    struct input_Pl011Dma inner;
    uint32_t __rseq_state;
};

static void __rseq_lc_0_pl011dma_orderly(void) {
    regseq_lc_register("pl011dma", "amba:0x41011/0xfffff");
    regseq_bus_register_driver("amba", "none");
    regseq_lc_create();
    regseq_lc_associate();
    { uint64_t __dma_dev; (void)regseq_dma_alloc_coherent("rx_buf", 64ull, 1ull, &__dma_dev); }
    (void)regseq_dma_request_channel("rx", 0);
    regseq_dma_register_completion("rx");
    struct __rseq_lc_0_pl011dma_target inst = {0};
    regseq_lc_call("on_init");
    input__impl_4__on_init(&inst.inner);
    regseq_lc_teardown(1);
    regseq_lc_call("on_deinit");
    input__impl_4__on_deinit(&inst.inner);
    regseq_dma_release_channel("rx");
    regseq_dma_free_coherent("rx_buf");
}

void __rseq_lc_run(void) {
    __rseq_lc_0_pl011dma_orderly();
}
