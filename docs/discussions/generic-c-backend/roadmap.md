# Generic-C Backend — Level-1 Roadmap

The phase sequence for the generic-C backend workstream, with per-phase
exit gates and the cross-track (front-end ↔ backend) dependency model.
This is the **Level-1** artifact: it sequences phases and fixes their
boundaries. **Level-2** detail (the open architecture decisions, the
codegen-coverage matrix, the implementation tasks) lives in a per-phase
*plan-first design doc* authored when that phase starts.

**Scope / verification boundary.** The generic-C target is hosted
userspace (libc, `printf`, `main()`). A construct is "done" when it
lowers to C that **compiles and runs on a host toolchain and
auto-validates** — NOT real-OS integration (that is the per-target
Linux/Zephyr backends, each a separate workstream). The canonical IR→C
mapping contract is `../regseq-model/generic-c-backend.md`.

## Method (per phase)

Each phase runs the project's proven loop: **plan-first design doc →
Codex → spec delta into `generic-c-backend.md` → Codex → implementation
tasks (`task.py`) → assess (+ Codex soundness pass on load-bearing
changes)**. Each phase ends at an **exit gate**, mirroring the
front-end's feature-complete gate:

> **Per-phase exit gate.** Every construct in the phase's scope lowers to
> C, compiles, runs, and auto-validates on the host; the codegen-coverage
> matrix reads "covered" across that scope; the verdict is `pass`.

## The dependency model (read this before sequencing)

The roadmap is **not a pure backend sequence** — it is a dependency graph
across two tracks, because **a domain construct can only be lowered once
it exists in `RegseqModel`**:

- **Front-end (bridge) track** — lands a construct in the IR.
- **Backend track** — lowers an IR construct to C.

At the start of this workstream, **diagnostic logging is the only domain
construct already bridged** (it survives as
`AttrData::Intrinsic(IntrinsicKind::Log(level))` on the callee item). The
other domain intrinsics are *spec'd but not bridged* — MMIO = stage 16,
interrupts = stage 17, atomics = stage 18 (atomics are currently
rejected). So **every domain phase after Phase 1 may require a front-end
phase first**, and "which domain next" is partly a front-end-roadmap
question, not a free backend choice. (The Rust *non-domain* surface is
fully bridged — the front-end feature-complete gate passed — so Phase 1's
non-domain scope has no front-end dependency.)

## Phase 1 — Rust surface + diagnostic logging  *(fully scoped; front-end READY)*

**Scope.** (a) The full supported **Rust (non-domain) surface** in
`RegseqModel`: types, instances, function bodies, the statement /
terminator / rvalue / operand / place / const vocabularies, and the C-ABI
admissibility rules. (b) The **diagnostic-logging** domain construct.

**Why these two together.** Diag-logging lowers to `printf`
(`generic-c-backend.md` §8) — i.e. it produces **observable output**,
which is what makes end-to-end *behavioral* auto-validation possible. It
is also the only bridged domain, so there is no other domain to start
with. Pairing the non-domain surface with diag-logging means the very
first backend can both compute and *report*, so a fixture can log a
computed value and the harness can assert the printed number.

**Deliverables.**
1. The generic-C backend in `crates/regseq-backends` (currently a stub):
   walks a validated `RegseqModel` and emits a host-compilable C
   translation unit.
2. A **codegen-coverage harness** — the codegen analog of the
   bridge-coverage runner: `bridge → RegseqModel → emit C → host cc →
   run → assert`, with a dual oracle (diag-log **golden stdout** for
   behavior + **golden C snapshot** for determinism/diff).
3. A **codegen-coverage matrix** (design-level, in this folder): one row
   per `RegseqModel` construct + its pinned C lowering + a fixture.

**Open architecture decisions** (resolved in the Phase-1 Level-2 plan
doc): naming (lean on `Instance.emission_name` + interned field names;
derive type names from `DefPath`); declaration order (forward-declare
everything, topo-sort only type *definitions* by by-value containment);
file layout (single translation unit first; `DefPath`-qualified names so
later partitioning is a no-rename split); host panic/abort strategy
(`printf` + `abort()`); **terminator emission** (`generic-c-backend.md`
§7 explicitly *defers* all 6 terminators — spec + implement them here).

**Exit gate.** Per the per-phase gate above, scoped to the supported Rust
surface + diag-logging.

## Phase 2 — RegseqError  *(definition only; detail deferred to Phase-2 start)*

**What it is.** RegseqError is the abstraction that lets driver specs be
authored with **OS-agnostic errors** that are **mapped to OS-specific
error codes during lowering**.

**Status: intentionally not yet described in detail.** Per the architect,
the full description, the front-end/bridge-status check, and the task +
spec breakdown happen **when Phase 2 starts** — not now. One breadcrumb
to resolve then (do not design it here): RegseqError's purpose is the
*OS-specific mapping*, so the generic-C (host) target must define what
OS-agnostic errors map to on a host (it has no kernel error-code space) —
this is the first concrete instance of the per-target "error mapping
table" that future Linux/Zephyr backends will each supply, and it ties
into the cross-backend strategy. Also confirm whether RegseqError is
already in `RegseqModel` (like diag-log) or needs a front-end phase.

## Phase 3+ — domain constructs, one at a time  *(deliberately unscheduled)*

After Phases 1 and 2, domain constructs are brought in **one at a time**,
each as its own phase (plan-first doc → spec → tasks → exit gate). They
are **not pre-scheduled**: once Phases 1–2 land, we choose the next
construct based on **immediate need**. *(Anticipated, not committed:
Register Access is the likely next pick — but that is decided then, not
now.)*

The known pool of remaining domain constructs (a taxonomy, **not** an
order): **Register Access (MMIO)**, **Interrupts**, **Atomics**,
**Lifecycle**, **Ordered regions** (the `ordered!` marker), and any
others surfaced by the intrinsics taxonomy. **Dependency reminder:** most
of these are not bridged yet (MMIO/interrupt/atomic = stages 16/17/18), so
each will typically need a front-end phase to land it in `RegseqModel`
before its backend phase — which is part of what "immediate need" weighs.

**RegseqError guardrail (applies the moment a native-backed fallible
construct is bridged).** From the Phase-2 RegseqError design debate
(`.claude/specs/regseq-error/`, decision **G01**): native-backed
operational failures in wrappers or intrinsics (`Clock::enable`,
`ResetLine::*`, `Completion::wait`, `poll::until`, DMA ops,
`PinCtrl::apply`, …) must default to **native/opaque propagation**, or to
**construct-local error types that explicitly map upward**, and must
**not** be collapsed directly into `RegseqError` unless the failure is an
authored *semantic* condition. Phase 2 (entry-only) does not trigger this;
it binds the first Phase-3+ phase that bridges a native-backed fallible
wrapper/intrinsic. The wrapper/intrinsic provenance question must be
resolved before such a construct may expose `Result<_, RegseqError>` as
its default authored surface.

**Host-validatability caveat (applies per domain).** Some domains
simulate *faithfully* on a host (diag-log → `printf`; register access →
backing byte array) — the host validates real behavior. Others are partly
*target* semantics (lifecycle, interrupts) — the host can only validate
structure/shape; real behavior validates against Linux/Zephyr. Each
Phase-3+ plan doc states which applies.

## Status table

| Phase | Scope | Front-end status | Backend status | Detail |
| ----- | ----- | ---------------- | -------------- | ------ |
| **P1** | Rust surface + diag-logging | **ready** (gate passed; diag-log bridged) | **done** (gate `pass`) | **fully scoped (above)** |
| **P2** | RegseqError | **done** for 2a (`framework_type` attr + certified `is_result` bridged) | **done** for 2a (entry-only `<entry>__regseq_exit` errno adapter; gate `pass`) | 2a shipped (`.claude/specs/regseq-error/`); trait-callback-exit lowering (spec §4) + G01 deferred |
| **P3+** | domain constructs (Register Access, Interrupts, Atomics, Lifecycle, …) | mostly **not bridged** (stages 16/17/18) | pending | unscheduled; pick on need after P1–P2 |
