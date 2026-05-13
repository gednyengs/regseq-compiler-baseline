# Expectations on stage 17 (incoming)

Stage 17 covers the **interrupt / ISR family**:
`#[rseq::interrupt(...)]` attribute family, ISR
identification, Linux-backend ISR scaffolding
(`request_irq`, threaded handlers), and the `IsrId`
typed-index.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

**Provenance note.** The `IsrId` typed-index + the
`InstanceKind` ISR-subkind framing were originally surfaced
under the monolithic "Stage 14: Domain-specific extensions"
plan via stage-1's Q3 forward reference; the 2026-05-12
strategy revision relocated them to this phase-2 stage. The
`#[rseq::interrupt(...)]` attribute family + Linux-backend
ISR scaffolding items are new -- introduced by the strategy
revision to scope the interrupt / ISR family stage. See
`../stage-14/expectations.md` for the foundation items that
stayed at stage 14 and the strategy-revision context.

## From stage 1 (relocated 2026-05-12)

- **`IsrId` typed-index** (and related domain-specific
  IDs). Per `../stage-1/spec-snapshot.md`'s "IDs added at
  later stages" note, stage 17 introduces additional typed
  IDs as the interrupt-handling surface needs them. They
  follow the same conventions (`newtype_index!` macro,
  niche-encoded u32, hand-written serde, single namespace
  per kind). The `IsrId` typed-index identifies a single
  ISR Item in the model.

- **ISR subkind of `InstanceKind::Function`** (per Q3
  single-level IR rule -- see
  `../stage-14/expectations.md` for the Q3 architecture
  note). Stage 17 picks the concrete extension shape:
  either an `is_isr: bool` flag on
  `InstanceKind::Function` (mirroring the existing
  `virtual_idx: Option<u32>` field), or a new
  `InstanceKind::Isr` variant with its own payload. Per
  Q3, append-only extensions to `InstanceKind` are valid;
  the choice drives `regseq_instance_kind_rank` updates
  (per stage-1 R5.1 / R5.2). Trade-off:
  - `is_isr: bool` flag: simpler structural change;
    function shape stays unified; dispatch logic checks
    the flag at codegen time. Mirrors the C-semantics
    methodology rule (preserve information rustc gives us
    for free, store the result not the input).
  - `InstanceKind::Isr` variant: explicit dispatch via
    match arm; carries ISR-specific payload (priority,
    trigger type, etc.) inline rather than via sibling
    table; rank-table entry appended.
  Stage 17 picks via Codex review against the per-family
  attribute architecture from stage-14's DomainAttr
  framework.

## New (introduced by 2026-05-12 strategy revision)

- [ ] **`#[rseq::interrupt(...)]` declarative attribute
  family.** Stage 17 specifies the attribute syntax and
  semantics. Candidate parameters:
  - `trigger_type = "level" | "edge"` -- interrupt
    trigger semantics.
  - `priority = N` -- interrupt priority (kernel
    interpretation).
  - `shared = bool` -- whether the IRQ line is shared
    with other devices (`IRQF_SHARED` in Linux).
  - `threaded = bool` -- whether to register via
    `request_threaded_irq` for bottom-half processing.
  The attribute attaches to a Rust function (the ISR
  body); the bridge parses the attribute into a
  structured `DomainAttr` per stage-14's sibling-table
  architecture (key kind: `ItemId` for the function
  Item).

- [ ] **ISR function shape constraints.** Stage 17
  specifies the constraints on ISR-attributed functions:
  - Signature: stage 17 picks the canonical ISR
    signature (Rust-side); the bridge enforces shape
    conformance and rejects mismatched signatures.
    Candidate shape: `fn(driver: &DriverState) ->
    IrqHandled` or `fn(driver: &DriverState, irq: i32) ->
    Result<IrqHandled, RegseqError>`.
  - The Rust-side return type maps to Linux's
    `irqreturn_t` (`IRQ_HANDLED` / `IRQ_NONE` /
    `IRQ_WAKE_THREAD`) per a small regseq-provided
    `IrqHandled` enum (`repr(int)` per `spec.md` §13.2
    C-ABI-admissibility rule).
  - Disallowed constructs inside ISR bodies: anything
    that can sleep (lock acquisition that may block,
    allocation, ...) -- enforcement is a separate
    sub-decision (likely via a bridge-level analysis,
    not by additional `Ty` rejections).

- [ ] **Linux-backend ISR scaffolding.** Stage 17
  specifies how ISR functions lower to Linux kernel
  interrupt registration:
  - `request_irq` / `request_threaded_irq` invocation
    site (in `probe()` per stage-15's `Lifecycle` trait);
    composes with the IRQ-number source from DT-binding
    metadata (stage 15).
  - `free_irq` in `remove()`.
  - ISR function shape: Linux-backend codegen emits the
    C-side ISR wrapper with return type `irqreturn_t`
    and argument shape `(int irq, void *dev_id)`,
    forwarding to the Rust-lowered ISR body.
  - Threaded-handler bottom-half scaffolding if
    specified via `threaded = true`.

## From stage 14 (filed 2026-05-12, per D-14-2.8 + D-14-2.7)

- [ ] **Call-site atomic-context fact for diagnostic-
  logging D5 validation.** Stage 14 deferred §26.8 D5
  ("Call site reachable from atomic context without
  compatible target emitter") to stage 17 because the
  call-site context fact is interrupt-construct
  territory. Stage 17 spec:
  - **Analysis surface.** A compiler analysis that
    answers, for any `emit_diagnostic` call site (or any
    other operation that wants to consult the same
    fact), "is this call site reachable from an atomic
    context for the target backend?" The analysis is
    intra-procedural at minimum (walks each Function
    Instance's Body and propagates a context flag
    through Calls); inter-procedural is in scope if
    cross-procedure reach is needed for accurate Linux
    spinlock-context / ISR-context propagation.
  - **Fact storage.** Stage 17 picks: side-table on
    `RegseqModel` keyed by Call-site identity (e.g.,
    `(InstanceId, BlockId, statement_index)` or a
    synthetic call-site ID), OR an analysis output
    consumed at backend codegen time without IR
    storage. Either fits; choice is a stage-17 design
    call.
  - **Per-target compatibility table.** Per
    `../stage-14/spec-snapshot.md` §26.8 D5 +
    refined-construct.md per-target `context_safety`
    fields: Linux is safe from any atomic context;
    Zephyr is safe under `LOG_MODE_DEFERRED`; FreeBSD
    `log()` is non-blocking but filter-routine context
    requires verification; Windows `DbgPrintEx` is
    DIRQL-bounded; macOS `os_log` is safe; `IOLog` is
    forbidden from primary interrupt context. Stage 17
    encodes the per-target rule.
  - **D5 diagnostic emission.** When the analysis
    detects an unsafe `emit_diagnostic` call site,
    emit a `RegseqError`-shaped diagnostic with the
    author-side span (preserving the
    diagnostic-quality requirement from §26.8). Phase-1
    + phase-2 stage 15 implementations operate as if
    every call site is atomic-safe (no D5 emission)
    until stage 17 lands.
  - Cross-ref: `../stage-14/spec-snapshot.md` §26.8 D5
    (deferral statement) + §26.10 (atomic-context
    analysis deferral); `regseq-taxonomy-work/.../
    03-diagnostic-logging/refined-construct.md`
    "Construct-Specific Analyses / Atomic-context check"
    + "per_target_lowering / context_safety" fields.

## From stage 14 (filed 2026-05-13, per D-14-10)

- **Atomic-acquire worked example dropped from stage-14
  Appendix A.** D-14-10 carved out the worked C-lowering
  examples (the former Appendix A.1-A.11 in stage-14
  spec-snapshot.md) into `generic-c-backend.md` §9. The
  Linux-target example A.9 (`Atomic<u32>` acquire-load
  lowering to `smp_load_acquire(&counter->value)`) was
  dropped rather than moved, per "no forward-looking
  placeholders" (README §5.9 rule 4). Stage 17 (atomic
  family) owns deriving:
    - The closed `IntrinsicKind::Atomic(_)` vocabulary
      (`load_acquire` / `store_release` / `compare_exchange`
      / etc.) per the Q7 intrinsic recognition mechanism.
    - The Linux backend's atomic lowering
      (`smp_load_acquire` / `smp_store_release` /
      `atomic_*` family + `READ_ONCE` / `WRITE_ONCE` for
      relaxed access) in the future `linux-backend.md`.
      The dropped example's observation that
      `smp_load_acquire` is preferred over
      `atomic_read_acquire` (because `atomic_read_acquire`
      takes `const atomic_t *` and type-punning a `const
      uint32_t *` to that is not valid in general) is
      informational context for the stage-17
      re-derivation.
    - The generic-C backend's atomic lowering -- the
      hosted-userspace target likely uses C11
      `<stdatomic.h>` primitives (`atomic_load_explicit(_,
      memory_order_acquire)`); stage 17 confirms.
  - Cross-ref: `../stage-14/spec-snapshot.md` Appendix A
    (carve-out pointer to `generic-c-backend.md` §9).

## Cross-refs

- `../stage-14/expectations.md` -- foundation items (Q7
  intrinsic recognition mechanism + DomainAttr
  sibling-table architecture) that this stage builds on.
- `../stage-15/expectations.md` -- `Lifecycle` trait +
  driver-struct discovery + DT-binding + `RegseqError`
  -- ISR registration lives in `Lifecycle::probe()`; the
  IRQ number is acquired from DT-binding metadata; ISR
  return type composes with `RegseqError`.
- `../stage-1/spec-snapshot.md` Q3 -- single-level IR
  rule; ISR subkind extension architecture.
