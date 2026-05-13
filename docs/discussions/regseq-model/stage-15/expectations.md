# Expectations on stage 15 (incoming)

Stage 15 covers **regseq-trait scaffolding**: the foundation
for regseq-provided glue traits (with `Lifecycle` as the
first concrete instance), the driver-struct discovery
mechanism, DT-binding metadata schema, and the
`RegseqError` taxonomic enum. This is the first phase-2
stage per README §1.3 + §2's "Domain extensions iterate per
stage in phase 2" rule.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

**Provenance note.** The Q6-facade-promotion item below was
originally filed against the monolithic "Stage 14:
Domain-specific extensions" plan; the 2026-05-12 strategy
revision relocated it here. Lifecycle / driver-struct /
DT-binding / RegseqError items are new -- introduced by the
strategy revision to scope the first phase-2 stage. See
`../stage-14/expectations.md` for the foundation items that
stayed at stage 14 and the strategy-revision context.

## From stage 1 (relocated 2026-05-12)

- **Promote items into the `regseq` facade (Q6).** The
  `regseq` crate (`crates/regseq/`) is currently empty per
  Q6's lazy promotion plan. Stage 15+ promotes items from
  each spec's `crate::regseq` submodule into the facade
  incrementally as redesign work touches them. The crate's
  symmetric `register_tool` declarations and
  `[package.metadata.regseq]` marker are already in place.
  Stage 15 owns the **first wave**: the `Lifecycle` trait +
  driver-struct support types + `RegseqError` taxonomic
  enum. Per-family stages (16 / 17 / 18 / ...) each promote
  their respective facade content as those stages open.

## New (introduced by 2026-05-12 strategy revision)

- [ ] **`Lifecycle` trait specification.** Stage 15
  specifies the regseq-provided `Lifecycle` glue trait that
  driver structs implement to declare their entry / exit /
  probe / remove behavior. Spec includes:
  - Trait surface: method signatures + parameter types +
    return types.
  - Semantic contract per method (probe-time invariants,
    remove-time cleanup obligations, error-handling
    expectations).
  - C-ABI-admissibility under the existing
    `spec.md` §13.2 trait-surface design rule (fieldless
    `repr(int)` status enums, primitive return types,
    `repr(C)` aggregates, raw pointers / refs to sized
    pointees). The trait must satisfy the
    `CAbiPositionAdmissible` predicate after receiver
    erasure to `*mut ()` because driver-side use is
    expected via `dyn Trait` at OS-integration
    boundaries.
  - Linux-backend lowering rule (how each method maps to
    Linux kernel driver hooks: `probe()` / `remove()` /
    `module_init()` / `module_exit()` / `module_exit`
    teardown semantics).
  - Composition with `RegseqError` for fallible methods.

- [ ] **Driver-struct discovery mechanism.** Stage 15
  specifies how the bridge identifies "this is the driver
  root Item" -- candidate mechanisms include:
  - Attribute-based: `#[rseq::driver]` on the Rust struct
    definition.
  - Trait-implementation-based: the struct implements
    `Lifecycle`.
  - Manifest-based: `[package.metadata.regseq]` records
    the driver root.
  Pinned decision feeds into the bridge's Item-discovery
  phase + the Linux-backend's `module_init` emission. The
  bridge must surface the driver root as a queryable
  property on `RegseqModel` (likely a new
  `RegseqModel.driver_root: Option<ItemId>` field or a
  sibling table) so downstream codegen can locate the
  entry point without re-walking the model.

- [ ] **DT-binding metadata schema.** Stage 15 specifies
  how device-tree binding information (compatible strings,
  `of_match_table` contents) is captured on the driver
  Item. Candidate mechanisms include:
  - `#[rseq::dt_binding("compatible-string")]` attribute
    (or `#[rseq::compatible(...)]`) attached to the driver
    struct; the bridge parses it into a structured
    `DomainAttr` per stage-14's sibling-table architecture
    (key kind: `ItemId` for the driver Item).
  - A structured trait method on `Lifecycle` that returns
    the compatible-string list at compile time.
  Pinned decision feeds into Linux-backend
  `of_match_table` emission and the `MODULE_DEVICE_TABLE`
  macro emission.

- [ ] **`RegseqError` taxonomic enum.** Stage 15 specifies
  the `RegseqError` taxonomy:
  - Which kinds of driver-error are first-class citizens
    (out-of-range parameter, hardware-fault, timeout,
    busy, no-such-device, permission-denied, ...) --
    closed set, append-only.
  - The enum's Rust shape: `repr(i32)` (or `repr(int)`
    more broadly) per `spec.md` §13.2 C-ABI-admissibility
    rule for trait-object boundaries, so
    `Result<T, RegseqError>` return types on `Lifecycle`
    methods stay C-ABI-admissible after receiver erasure.
  - Linux-backend mapping: which `errno` value each
    variant maps to (`-EINVAL`, `-EIO`, `-ETIMEDOUT`,
    `-EBUSY`, `-ENODEV`, `-EPERM`, ...). The mapping
    survives in the IR at the enum-variant level
    (probably via per-variant attributes or a
    `RegseqError`-specific sibling table; stage 15 picks
    the encoding).
  - Composition rule with `Result<T, RegseqError>` return
    types on `Lifecycle` methods (and on regseq-provided
    intrinsics that report failure).

- [ ] **Driver-struct probe context.** Stage 15 specifies
  how driver structs receive probe-time context: the
  device pointer, resource maps, IRQ numbers, clock
  handles, etc. This couples to the DT-binding metadata
  schema (a probe context is what the bridge derives from
  the DT match) and to future MMIO / interrupt / clock
  stages (which add their respective resource types to
  the probe context). Stage 15 picks the **abstract
  shape** of the probe context; per-family stages extend
  it with their resource types.

## From stage 14 (filed 2026-05-12, per D-14-2.8)

- [ ] **`register_log_module` compiler-emitted glue.**
  Stage 14 deferred the diagnostic-logging
  log-module-registration glue to stage 15 because the
  per-target binding composes with driver-identity /
  lifecycle machinery that stage 15 owns. Stage 15 spec:
  - Per-target binding contract:
    - Linux: `pr_fmt` setup at driver-module entry;
      module-name derived from compiler-known driver
      identity.
    - Zephyr (informational; not currently scheduled):
      `LOG_MODULE_REGISTER(name)` call at entry; per-
      module log level set from native Kconfig.
    - FreeBSD / Windows / macOS (informational): per
      `../stage-14/spec-snapshot.md` §26.9 informational-
      targets table.
  - When the glue emits: at driver-module entry, alongside
    or as part of `Lifecycle::module_init` (or whatever
    stage 15 names that entry-hook surface).
  - Surface on `RegseqModel`: stage 15 picks whether
    `register_log_module` is a compiler-managed Instance
    (synthesised per the §13.1 / §13.2 pattern) or a
    bridge-side directive consumed at backend codegen
    time. Either fits the §3.6 DomainAttr-table
    architecture; the choice is a stage-15 design call.
  - Cross-ref: `../stage-14/spec-snapshot.md` §26.10
    (deferral statement) + §26.9 (per-target lowering
    record).

- [ ] **Device-context joining for `dev_*` lowering.**
  Stage 14 deferred device-context joining to stage 15
  because the bridge-side mechanism that surfaces the
  device handle at diagnostic-logging call sites depends
  on driver-struct discovery (a stage-15 deliverable).
  Stage 15 spec:
  - Mechanism: how the bridge resolves "the driver's
    device handle" at every `emit_diagnostic` lowering
    site reachable from the driver's Lifecycle-method
    body. Candidate: a compiler-synthesised side-table
    on `RegseqModel` keyed by Instance, populated during
    Phase 5 body construction.
  - Linux-backend consumption: `dev_*` family
    (`dev_err`, `dev_warn`, `dev_info`, `dev_dbg`)
    consumes the joined device handle as the first
    argument; replaces the `pr_*`-family lowering pinned
    by stage 14's D-14-2.9 binding.
  - Phase-1 generic-C compatibility: the joining
    mechanism is no-op on the generic-C target (hosted
    userspace, no device handle); generic-C lowering
    stays at `printf("[<TAG>] " <fmt> "\n", <args>...)`.
  - Cross-ref: `../stage-14/spec-snapshot.md` §26.10
    (deferral statement) + §26.9 Linux-kernel binding
    note ("`dev_*` variants compose with the
    device-context joining that stage-15 ... supplies").

- [ ] **Linux `max_str_args_per_call` declaration for D8
  diagnostic.** Stage 14 D-14-2.7 D8 (§26.8) requires
  every binding target to declare `max_str_args_per_call`
  (the cap on `Ty::Ref(Ty::Str, _)` operand count in
  args[1..] of Log Calls; bounds per-call stack use
  against the per-`%s`-site temporary-buffer allocation
  inherent in C-string-sink lowering). Generic-C declared
  8 at stage-14 §26.9. Linux's declaration was deferred
  to stage 15 because the bound couples to Linux's
  `&str` materialisation strategy (which stage 15 owns
  as part of the `pr_*` / `dev_*` lowering pin).
  Stage 15 spec:
  - Pick Linux's `&str` materialisation strategy:
    candidates include `printk`'s built-in format-string
    copy (no caller-side buffer needed), stack-
    allocated buffers (same pattern as Generic-C),
    `kasprintf`-family helpers (heap-allocated;
    requires GFP-context awareness), or a custom
    kernel-string helper.
  - Declare Linux's `max_str_args_per_call` value
    consistent with the chosen strategy. Stack-buffer
    strategies should pick a bound similar to Generic-C
    (8 args) to bound the worst-case kernel-stack
    consumption (kernel stack is 8-16KB; the same
    `8 * 256 = 2KB` budget fits comfortably).
    Built-in-`printk`-copy strategies may declare no
    bound (or a very high bound) if the kernel handles
    the buffering internally.
  - The declaration lands as a binding row in the
    Linux backend's section of the per-target lowering
    contract (the analog to stage-14 §26.9 Generic-C
    binding section, extended to Linux at stage 15
    closure).
  - Cross-ref: `../stage-14/spec-snapshot.md` §26.8 D8
    (target-aware diagnostic + deferral note); the §26.9
    "Linux `max_str_args_per_call` declaration" paragraph
    was removed by D-14-10 -- see the D-14-10 entry below
    for the new home.

## From stage 14 (filed 2026-05-13, per D-14-10)

- **Linux binding for the diagnostic-logging surface has
  no shadow contract in stage-14 spec-snapshot.md.**
  D-14-10 lifted the generic-C diagnostic-logging backend
  contract out of stage-14 §26.9 into
  `generic-c-backend.md` §8 and **dropped** the parallel
  Linux rows (`pr_err` / `pr_warn` / `pr_info` /
  `pr_debug` call shapes; `dev_*` deferral note pending
  device-context joining; Linux `max_str_args_per_call`
  declaration) per "no forward-looking placeholders"
  (README §5.9 rule 4). Stage 15 owns the Linux backend's
  diagnostic-logging contract end-to-end and re-derives
  against pinned MIR + Linux kernel API evidence at its
  evidence pass. The earlier "From stage 14 (filed
  2026-05-12)" entries above remain in scope; D-14-10
  changes where the canonical Linux spec lands (the future
  `linux-backend.md` merger artifact per README §5.9 rule
  7), not what stage 15 is responsible for.
  - Cross-ref: `../stage-14/spec-snapshot.md` §26.9
    (Pipeline A uniformity rule + pointer into
    `generic-c-backend.md` §8); README §5.9 rule 7
    (`<target>-backend.md` merger artifact pattern).

## Cross-refs

- `../stage-14/expectations.md` -- foundation items (Q7
  mechanism + diagnostic-logging vocabulary + DomainAttr
  sibling-table architecture) that this stage builds on.
- `../spec.md` §13.2 -- trait-surface design rule for
  `dyn Trait` at OS-integration boundaries; C-ABI
  admissibility constraint that `Lifecycle` + status
  types must satisfy.
- `../stage-1/spec-snapshot.md` Q6 -- lazy facade
  promotion plan.
- `../stage-3/spec-snapshot.md` D1.4 -- opaque-struct
  pattern (referenced if driver-struct discovery is
  attribute-based with opaque metadata).
- `../stage-12/spec-snapshot.md` §13 / §13.2 -- vtable
  synthesis and trait-object lowering (the mechanism
  `Lifecycle` will compose with at the `dyn Lifecycle`
  use site).
