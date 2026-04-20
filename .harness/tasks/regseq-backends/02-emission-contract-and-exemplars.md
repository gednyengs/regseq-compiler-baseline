# regseq-backends-02: Linux Emission Contract + PL011 Exemplars + Hand-Authored PL011 Reference Driver

**Depends on**: `regseq-model-60`.
**Soft-depends on**: `track-a-validation-11` (QEMU harness) --
required for the hand-authored reference's final validation
step; task 1 can start and produce the contract / exemplars /
draft reference before the harness is ready, but cannot close
until QEMU smoke on the hand-authored reference passes.
**Parallel with**: `regseq-backends-03`, `regseq-model-61`,
`regseq-model-62`.
**Authoring model**: **co-authored by user + Claude** (NOT an
LLM implementer agent). The hand-authored PL011 reference + the
contract it codifies are foundational design artifacts that
require joint human + compiler-designer review. Downstream tasks
remain implementer-delegatable.

## Description

Phase-7.0 oracle foundation. This task produces the **specification**
the Linux CFG emitter (tasks `regseq-linux-backend-04` and
`regseq-linux-backend-05`) must obey: an emission contract document,
a small Rust data-table module, ~15 hand-authored C exemplar
fragments covering every CFG / intrinsic construct PL011 exercises,
**and a full hand-authored PL011 Linux reference driver** that the
emitter must produce byte-for-byte equivalent output for.

The contract + reference are the replacement oracle for the retired
"byte-parity vs the old pipeline" strategy (migration.md Q5). After
this task lands, the implementer has a concrete integration-level
target: "produce the exact hand-authored PL011 driver, construct
by construct, when the emitter walks the PL011 RegseqModel."
Construct-level exemplars catch unit mistakes; the full driver
reference catches composition mistakes that no set of fragments
can cover.

**This task is co-authored work (user + Claude), not implementer-
delegatable.** The hand-authored PL011 reference is a foundational
design artifact: it codifies, in C, every decision the emission
contract encodes in prose + data tables. Drift between the
reference and the contract is the #1 source of downstream defect;
joint authoring keeps them aligned.

**Critically**, this task does NOT write runnable emitter tests.
The emitter seam does not exist yet -- emit_semantic + emit_module
still walk the old `SemanticModel` / `Stmt`/`Expr` trees. Tests
that exercise the new CFG emitter against this task's exemplar
corpus + reference are authored by tasks `regseq-linux-backend-04`
(CFG body construct tests) and `regseq-linux-backend-05` (wrapper /
probe / remove integration tests; byte-equal-to-reference
end-to-end test).

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q5 (the D'
  validation layers and how this task contributes Layer 1-2),
  Q6 (the ordered migration plan naming this as Phase-7.0),
  Q8 (the C-AST / emission-contract material).
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (the raw material for the contract -- Section 2 "Abstraction Types"
  with per-intrinsic Linux lowering tables, Section "Resource Cleanup
  on Error Paths" with reversible-intrinsic pairs + handled cases,
  Section "OrderedRegion: SESE Invariant and CFG Modes" with barrier
  placement).
- `docs/discussions/models/regseq-model/spec.md` Section 10.2 (register
  access intrinsic architecture; relevant for MMIO emission rules).
- `docs/discussions/models/regseq-model/ir-reference.md` (the CFG IR
  constructs this task's exemplars encode: `Body`, `BasicBlock`,
  `Statement`, `Terminator`, `IntrinsicCall` + `IntrinsicMetadata`,
  `OrderedRegion`, `error_target`).
- `crates/regseq-model/src/ir/{body,statement,terminator,intrinsic}.rs`
  -- the CFG IR shape the exemplars' conceptual inputs come from.
- `crates/regseq-backends/src/linux/emit_semantic.rs` and
  `linux/emit_module.rs` -- current string-format-heavy emission.
  Read to understand what C shapes the emitter produces today (some
  patterns are reusable at the contract level; most need to be
  re-specified for CFG goto-based emission).
- `crates/regseq-backends/tests/linux_emission_scaffold.rs` lines
  834-1238 -- the existing pattern for backend emission tests
  (content assertions against output of the public emission entry
  point). This task's data tables + exemplars are novel artifacts
  not currently present in the repo; call that out in the contract
  doc.
- `tests/fixtures/pl011-full-spec/src/driver.rs` lines 156-393 --
  the PL011 spec the exemplars must cover. Enumerate every CFG
  shape and intrinsic call it exercises; each must map to an
  exemplar or data-table entry.
- `.harness/spec-batches/regseqmodel-phase-7.md` (superseded by
  migration.md, but retained for historical context).

## Implementation Notes

### Deliverables

1. `docs/discussions/models/regseq-model/linux-emission-contract.md`
   -- normative markdown, ~600-1000 lines. Structure below.
2. `crates/regseq-backends/src/linux/emission_tables.rs` -- small
   Rust data-table module with public, typed tables.
3. `tests/goldens/pl011/linux/constructs/*.c` -- ~15 hand-authored
   exemplar fragments. Each file is small (~20-60 lines) and
   exercises exactly one CFG / intrinsic construct.
4. `tests/goldens/pl011/linux/constructs/README.md` -- one-paragraph
   index mapping each exemplar file to its construct name + a
   pointer to the contract section that specifies it.
5. **`tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`**
   -- the full hand-authored PL011 Linux driver reference.
   Authored in the contract's normal form. This file IS the
   oracle every downstream emitter test (tasks 9, 10, 11, 12)
   compares against.

### Contract document structure

Outline for `linux-emission-contract.md`:

```
# Linux Emission Contract

## Purpose
  (This is the oracle per migration.md Q5 Layer 1.)

## Scope
  What this contract covers (CFG body emission + resource cleanup +
  ordered-region barriers + intrinsic calls + driver integration
  wrappers). Explicitly out of scope: Zephyr, other OSes (future
  per-OS contracts), register definition emission (owned by
  emit_regs).

## Normal form
  - One C function per `RegseqModel::Function`.
  - One C label per `BasicBlock`, named `bb<id>` where `<id>` is
    the `BasicBlockId` value.
  - Entry to any function is `bb<entry_id>`; a leading `goto
    bb<entry_id>` is elided when `entry_id` is the first block.
  - No inlining of called functions; every `Call` / `IntrinsicCall`
    becomes a C call statement.
  - Local declarations at function top, grouped by type, in
    `LocalId` order.
  - Canonical cleanup-label naming: `err_<resource>` for each
    cleanup step in a dominator-based chain.
  - Deterministic attribute ordering on functions (storage-class,
    type qualifiers, declarator).

## Normalization algorithm (for byte-equal regression)
  The exact algorithm used to produce `normalized_reference.c` in
  task `track-a-validation-12`:
  - Strip trailing whitespace on every line.
  - Exactly one blank line between top-level items.
  - Comment banner policy (no dates, no absolute paths).
  - Tab vs space policy.
  - Line ending policy.

## Per-construct rules

### Straight-line block
### Branch (SwitchInt lowering)
### Loop (back-edge Goto)
### IntrinsicCall (no error_target)
### IntrinsicCall with error_target (the `?`-propagation shape)
### OrderedRegion with barrier placement
### Cleanup ladder (dominator-based)
### Return
### Assignment / field projection
### Cast
### (etc. -- enumerate every CFG construct the PL011 spec exercises)

Each rule cites:
  - The IR primitive being lowered (with file:line in `ir/`).
  - The target C pattern (cite the corresponding exemplar file).
  - Any data-table entry consumed from `emission_tables.rs`.
  - Known-limitations / unsupported cases (cite
    compiler-knowledge.md where applicable, e.g., Case 7
    interprocedural cleanup is out-of-scope).

## Intrinsic lowering
  A subsection per intrinsic family (MMIO, clock, reset, pinctrl,
  interrupt, log, timed-delay, poll, completion, lock). For each:
  - Rust signature from `regseq` facade.
  - IR representation (the `IntrinsicCall` metadata shape).
  - Linux API mapping (cite `emission_tables.rs` entry).
  - Pre/post C pattern.
  - Return-value adaptation.

## Callback-family return adaptation
  Lifecycle / Serial / SerialConsole / PowerManagement callback
  return types from RegseqError -> Linux errno. Cite
  compiler-knowledge.md "Per-variant native error mapping".

## Resource cleanup
  The dominator-based cleanup strategy from compiler-knowledge.md
  "Resource Cleanup on Error Paths". Handled cases become a
  goto-chain exemplar; unhandled cases (2, 3, 5, 6, 7, 9) emit a
  diagnostic warning and fall back to best-effort cleanup.

## Adversarial review checkpoint
  This contract IS the oracle for the new emitter (migration.md Q5
  Layer 1). Before task `regseq-linux-backend-04` begins, the
  contract + exemplar corpus must pass an explicit Codex review
  with "is this internally consistent, does it cover every PL011
  construct, does it match `compiler-knowledge.md` lowering intent,
  are there latent Linux API misuses" as the review prompt. The
  review is a named acceptance gate on this task (see Test Plan).

## Extension policy
  How to add a new construct rule when a future driver needs one.
  How to add a new intrinsic to the data tables. How to version the
  contract when the normal form itself changes (rare; requires
  re-blessing the reference).

## Cross-OS note (informational)
  This contract is Linux-specific. A future `zephyr-emission-contract.md`
  will mirror the structure with Zephyr API mappings. The per-construct
  rules are mostly structural (goto-based CFG emission) and transfer
  unchanged; the data tables are per-OS.
```

### Data tables

`crates/regseq-backends/src/linux/emission_tables.rs` contents:

```rust
//! Linux-specific emission tables for the CFG emitter.
//!
//! These tables are the executable half of the emission contract
//! documented at docs/discussions/models/regseq-model/linux-emission-contract.md.
//!
//! Each table is consumed by linux::emit_semantic or linux::emit_module
//! (tasks regseq-linux-backend-04/05). Changing an entry here is a
//! contract change; see the contract doc's Extension policy.

use regseq_model::ir::intrinsic::RegseqIntrinsic;
use regseq_model::ir::intrinsic::IntrinsicMetadata;
// ...

/// Intrinsic -> Linux API function name mapping.
///
/// For each `RegseqIntrinsic` variant, the canonical Linux kernel
/// function called in the emitted C. Return-value wrapping is
/// handled separately via CALLBACK_RETURN_ADAPTATION.
pub static INTRINSIC_LINUX_API: &[(RegseqIntrinsic, &str)] = &[
    (RegseqIntrinsic::ClockEnable,      "clk_prepare_enable"),
    (RegseqIntrinsic::ClockDisable,     "clk_disable_unprepare"),
    (RegseqIntrinsic::ClockGetRate,     "clk_get_rate"),
    (RegseqIntrinsic::ResetAssert,      "reset_control_assert"),
    (RegseqIntrinsic::ResetDeassert,    "reset_control_deassert"),
    (RegseqIntrinsic::ResetToggle,      "reset_control_reset"),
    (RegseqIntrinsic::PinCtrlApply,     /* see PIN_STATE_API */ ""),
    (RegseqIntrinsic::MmioRead,         "readl"),   // default; MmioRead8/16/64 -> readb/readw/readq
    (RegseqIntrinsic::MmioWrite,        "writel"),
    (RegseqIntrinsic::MmioReadField,    /* synthesized: read + mask + shift */ ""),
    (RegseqIntrinsic::MmioWriteField,   /* synthesized: read + mask + or + write */ ""),
    (RegseqIntrinsic::LogError,         "dev_err"),
    (RegseqIntrinsic::LogWarn,          "dev_warn"),
    (RegseqIntrinsic::LogInfo,          "dev_info"),
    (RegseqIntrinsic::LogDebug,         "dev_dbg"),
    (RegseqIntrinsic::BusyWait,         "udelay"),       // Duration in us
    (RegseqIntrinsic::Sleep,            "usleep_range"), // min/max bounds via contract
    (RegseqIntrinsic::PollUntil,        /* synthesized: readl_poll_timeout */ ""),
    // ... every variant of RegseqIntrinsic
];

/// Reversible-intrinsic cleanup pairs for dominator-based rollback.
///
/// Per compiler-knowledge.md "Resource Cleanup on Error Paths" --
/// forward-intrinsic -> reverse-intrinsic. Only lifecycle operations
/// with known inverses. Configuration writes are not reversible.
pub static REVERSIBLE_CLEANUP_PAIRS: &[(RegseqIntrinsic, RegseqIntrinsic)] = &[
    (RegseqIntrinsic::ClockEnable,      RegseqIntrinsic::ClockDisable),
    (RegseqIntrinsic::PinCtrlApply,     RegseqIntrinsic::PinCtrlApply), // with teardown_state
    (RegseqIntrinsic::ResetAssert,      RegseqIntrinsic::ResetDeassert),
    (RegseqIntrinsic::ResetDeassert,    RegseqIntrinsic::ResetAssert),
];

/// Callback-family return-value adaptation.
///
/// Map from (callback family, authored return type) -> (Linux signature
/// return type, RegseqError -> errno conversion function).
pub static CALLBACK_RETURN_ADAPTATION: &[...] = &[ ... ];

/// Ordered-region barrier policy.
///
/// Where to emit compiler barriers: at region entry, at normal exit,
/// and at every abnormal exit (error_target edge leaving the region).
/// Per compiler-knowledge.md "Barrier kind" -- compiler barrier only,
/// not hardware memory barrier.
pub static ORDERED_REGION_BARRIER: BarrierPolicy = BarrierPolicy {
    kind: BarrierKind::CompilerOnly,
    c_statement: "__asm__ __volatile__(\"\" ::: \"memory\")",
    placement: BarrierPlacement::EntryAndEveryExit,
};

/// Pin-state logical id -> Linux API entry.
///
/// Per spec.md Section 2 "PinCtrl" -- logical-id -> native API call.
pub static PIN_STATE_API: &[(&str, &str)] = &[
    ("default", "pinctrl_pm_select_default_state"),
    ("sleep",   "pinctrl_pm_select_sleep_state"),
    ("idle",    "pinctrl_pm_select_idle_state"),
];

/// RegseqError variant -> Linux errno.
///
/// Per compiler-knowledge.md "Per-variant native error mapping".
pub static REGSEQ_ERROR_LINUX_ERRNO: &[(&str, &str)] = &[
    ("InvalidParameter",    "-EINVAL"),
    ("OutOfMemory",         "-ENOMEM"),
    ("HardwareError",       "-EIO"),
    ("Timeout",             "-ETIMEDOUT"),
    ("Unsupported",         "-EOPNOTSUPP"),
    ("DeviceBusy",          "-EBUSY"),
    ("PermissionDenied",    "-EACCES"),
    ("DeviceNotFound",      "-ENODEV"),
    ("GenericFailure",      "-EFAULT"),
];
```

All tables are `pub static` with documented types; each cited in the
contract doc.

### Exemplar corpus

Each exemplar under `tests/goldens/pl011/linux/constructs/` is a
minimal `.c` fragment. The task authors exemplars by hand, applying
the contract's normal form. Intended exemplar set (final count
determined during authoring; aim ~15):

- `straight_line.c` -- 3-statement straight-line block.
- `branch_if.c` -- `SwitchInt` with two targets, no back-edge.
- `branch_if_else.c` -- ditto but with `else` branch.
- `loop_while.c` -- back-edge Goto forming a `while`-shape.
- `intrinsic_infallible.c` -- `IntrinsicCall` with no `error_target`
  (e.g., `clock_disable` or `log_info`).
- `intrinsic_fallible_qpropagate.c` -- `IntrinsicCall` with
  `error_target` wired to a `?`-propagation pattern; shows goto to
  `err_<resource>` label.
- `ordered_region_straight.c` -- an `OrderedRegion { entry, exit }`
  with sequential calls, showing entry + exit barrier.
- `ordered_region_with_error.c` -- ordered region with an interior
  abnormal exit, showing barrier before the abnormal-exit goto.
- `cleanup_ladder_two_resources.c` -- cleanup goto chain for a
  sequence like clock.enable + pinctrl.apply, showing
  `err_clock` and `err_pinctrl` labels in reverse order.
- `return_result.c` -- `Return` terminator lowering
  `Result<(), RegseqError>` to Linux `int` errno via
  `REGSEQ_ERROR_LINUX_ERRNO`.
- `assignment_field.c` -- `Assign` statement projecting through
  a `Field`.
- `cast.c` -- `Rvalue::Cast` lowering.
- `mmio_read_field.c` -- synthesized read + mask + shift for
  `MmioReadField`.
- `mmio_write_field.c` -- synthesized read + modify + write for
  `MmioWriteField`.
- `log_emit_format.c` -- `LogEmit` with format placeholders
  lowered to `dev_info("template...", arg)`.

Each exemplar is accompanied by a Rust comment sketch (in the
contract doc, not in the `.c` file) that describes what minimal
CFG body the emitter must receive to produce this output. Task
`regseq-linux-backend-04` turns those sketches into runnable tests.

### PL011 coverage audit

Before committing the exemplar corpus, enumerate every CFG shape
and intrinsic in `tests/fixtures/pl011-full-spec/src/driver.rs` and
confirm each maps to an exemplar or data-table entry. This is the
"contract is complete for PL011" check. Omissions are logged as
known gaps with proposed exemplars to add.

### Hand-authored PL011 reference driver

The file `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
is the integration-level oracle. It is authored jointly (user +
Claude) by walking the PL011 RegseqModel construct-by-construct
and applying the contract's normal form + intrinsic mappings +
callback adaptations at every site.

**Normalization form (authoring rules; also in the contract
section on normal form):**

- One C function per `RegseqModel::Function`; the file contains
  no ad-hoc helpers that don't correspond to an authored function.
- One `bbN:` label per `BasicBlock`; unreachable blocks omitted
  (they're Phase-5-construction artifacts and should not appear).
- Canonical goto graph: every `Terminator` lowers to exactly the
  C shape its exemplar specifies; no stylistic flattening.
- Locals declared at function top, grouped by C type, in
  `LocalId` order.
- Cleanup ladders use `err_<resource>:` labels in reverse
  acquisition order, per `REVERSIBLE_CLEANUP_PAIRS` +
  `compiler-knowledge.md` "Resource Cleanup on Error Paths".
- Callback wrappers follow the inner-helper + outer-wrapper
  convention from `regseq-linux-backend-05`'s callback-return
  adaptation rule.
- Module integration (match table, probe, remove, module_init,
  module_exit, MODULE_LICENSE, etc.) follows `LinuxBackendPlan`'s
  declarative decisions -- the reference shows the resolved
  string form for PL011's specific plan values.
- No inlining. No partial specialization. If a construct looks
  "long" or "mechanical," that is the point: the emitter will
  produce equally mechanical output.

**Validation steps (acceptance gates on the reference):**

1. `cc -fsyntax-only` passes on the file.
2. The file compiles as an out-of-tree kernel module against the
   pinned kernel (`experiments/pl011-eval-env/notes/phase-04-kernel-build.md`)
   -- reuse the existing host-build-proof scaffolding, retargeted
   at this file.
3. When `track-a-validation-11` (QEMU harness) is available, the
   reference passes the full QEMU smoke: boot + load + probe +
   open backend-defined tty + write bytes + close + unload,
   with no kernel warnings / oops / leaks. This is the Layer-7
   functional validation per migration.md Q5.

If steps 1-3 all pass, the reference is **blessed** and becomes
the downstream oracle. If step 3 fails, this task goes back to
contract + reference authoring until it does. The reference is
NOT committed until all three steps pass.

**Authoring procedure:**

1. Generate or load the PL011 RegseqModel via the existing
   Phase-6 pipeline (`cargo test -p regseq-model --test
   pl011_pipeline_phase6`).
2. Walk the model constructs in source order. For each
   construct, consult the contract and apply the matching rule.
3. Emit the resulting C line(s) into the reference file.
4. At each module-integration site (probe / remove /
   module_init), apply the `LinuxBackendPlan` template with
   PL011's concrete values.
5. Cross-check: every construct in the reference has a
   corresponding exemplar in `tests/goldens/pl011/linux/constructs/`.
   No exemplar in the corpus is missing from the reference for
   reasons other than "PL011 doesn't exercise it."

**Callback-wrapper exemplars are NOT in this task**

Per Codex review round 3: "callback wrapper" (parameter binding,
return adaptation on the Linux-facing wrapper) is not a CFG
primitive -- it is Linux module-integration policy. Those
exemplars and tests belong to task `regseq-linux-backend-05`
(emit_module CFG rewrite). The contract document covers the
callback return-value adaptation rules (via
`CALLBACK_RETURN_ADAPTATION`), but the exemplar fragments live
with task 10.

### Adversarial review checkpoint (acceptance gate)

Before this task closes, invoke Codex via
`scripts/python/codex_runner.py` (per SESSION.md) with the prompt:

> "Adversarial review of `linux-emission-contract.md` +
> `emission_tables.rs` + the exemplar corpus under
> `tests/goldens/pl011/linux/constructs/`. Prompts: (a) is every
> PL011 CFG construct covered? (b) does every data-table entry
> match compiler-knowledge.md's stated intent? (c) are there
> latent Linux API misuses (wrong arg order, wrong devm_* scope,
> wrong completion vs wait semantics)? (d) does the normal form
> produce unambiguous output given any RegseqModel input? (e) does
> the normalization algorithm close every whitespace / ordering
> gap that would otherwise cause byte-equal regression false
> positives? Mark MANDATORY / RECOMMENDED / NIT."

Iterate fix -> re-review until APPROVED. The checkpoint is a
hard acceptance gate -- the contract becomes the downstream
oracle; unchecked defects in it become defects the emitter will
faithfully reproduce.

## Non-Goals

- No runnable Rust tests invoking the emitter (emitter does not
  exist yet; tests land with tasks 9/10).
- No PL011 full-driver reference (blessed in task 12).
- No callback-wrapper exemplars (owned by task 10).
- No Zephyr emission contract (future per-OS document).
- No construct beyond what PL011 exercises plus the immediate
  adjacent constructs (avoid boiling the ocean).
- No execution-plan-md / migration-md edits (those were landed
  pre-spec as doc prep).
- Do not modify the Rust code of `linux/emit_semantic.rs`,
  `linux/emit_module.rs`, or `linux/c_ast.rs` -- those rewrites
  are tasks 9/10 and 2 respectively.

## Test Plan

This task is mostly artifact-authoring; executable test coverage
lands with downstream tasks. The in-task tests:

- **Data-tables compile**: `cargo build -p regseq-backends` passes
  with `emission_tables.rs` added.
- **Exemplar compile check**: each exemplar under
  `tests/goldens/pl011/linux/constructs/*.c` is at least syntactically
  parseable by the system `cc -fsyntax-only` (or the repo's existing
  `cc` infrastructure if one exists). This is a weak check; semantic
  correctness comes later from the QEMU smoke. Wire this as a
  `#[test]` under `crates/regseq-backends/tests/construct_exemplars_syntax.rs`
  that iterates the directory and `cc -fsyntax-only`s each file.
- **Coverage audit**: a `#[test]` that loads
  `tests/fixtures/pl011-full-spec/expected/regseq-model.json`
  (Phase-6 golden), walks every `Function.body`, enumerates the
  CFG constructs + intrinsic-call kinds present, and asserts each
  either has a corresponding exemplar file or is documented in
  the contract's "out of scope for PL011" section.
- **Adversarial review result**: the Codex review transcript is
  attached to this task's final commit as a comment on the
  commit message or in the acceptance-criteria checklist.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Adversarial review of regseq-backends-02 deliverables against
.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md.

Files to audit:
- docs/discussions/models/regseq-model/linux-emission-contract.md
- crates/regseq-backends/src/linux/emission_tables.rs
- tests/goldens/pl011/linux/constructs/*.c
- tests/goldens/pl011/linux/constructs/README.md
- crates/regseq-backends/tests/construct_exemplars_syntax.rs
- crates/regseq-backends/tests/construct_exemplars_coverage.rs

Prompts:
(a) Is every PL011 CFG construct covered by an exemplar or
    contract rule? Cite tests/fixtures/pl011-full-spec/src/driver.rs
    to justify.
(b) Does every data-table entry match compiler-knowledge.md's
    stated Linux lowering intent?
(c) Are there latent Linux API misuses -- wrong arg order,
    wrong devm_* scope, wrong completion vs wait semantics,
    wrong udelay vs usleep_range threshold?
(d) Does the normal form produce unambiguous output given any
    RegseqModel input? Are there CFG shapes where the contract
    is silent?
(e) Does the normalization algorithm close every whitespace /
    ordering / banner gap that would cause byte-equal regression
    false positives in task 12?
(f) Are callback-wrapper concerns correctly scoped out of this
    task and into regseq-linux-backend-05?

Mark MANDATORY / RECOMMENDED / NIT.
</task>
... (standard grounding rules + structured output contract per SESSION.md) ...
EOF
```

Iterate until APPROVED. The review is a hard acceptance gate.

## Quality Gate

```
cargo test -p regseq-backends --test construct_exemplars_syntax
cargo test -p regseq-backends --test construct_exemplars_coverage
cargo build -p regseq-backends
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `docs/discussions/models/regseq-model/linux-emission-contract.md`
  committed with all sections from Implementation Notes, including
  the normalization algorithm and the per-construct rule set
  covering every PL011 CFG shape.
- `crates/regseq-backends/src/linux/emission_tables.rs` committed
  with `INTRINSIC_LINUX_API`, `REVERSIBLE_CLEANUP_PAIRS`,
  `CALLBACK_RETURN_ADAPTATION`, `ORDERED_REGION_BARRIER`,
  `PIN_STATE_API`, `REGSEQ_ERROR_LINUX_ERRNO` (and any additional
  tables identified during authoring). All `pub static` with
  typed entries; no `&str`-anywhere shortcuts.
- `tests/goldens/pl011/linux/constructs/` directory with ~15
  exemplar `.c` files + README index.
- **`tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`**
  committed with an explicit provenance comment: "Hand-authored
  reference, normal form per linux-emission-contract.md. The
  emitter (tasks 9/10) MUST produce byte-equal output to this
  file." File is validated per the three steps in "Hand-authored
  PL011 reference driver" -- including a passing QEMU smoke.
- `crates/regseq-backends/tests/construct_exemplars_syntax.rs`
  passes (each exemplar syntactically parses; includes the
  reference file).
- `crates/regseq-backends/tests/construct_exemplars_coverage.rs`
  passes (every PL011 CFG construct has a mapped exemplar or is
  documented out-of-scope).
- Adversarial Codex review returned APPROVED.
- `cargo test --workspace` green (no code behavior change).
- `git diff --name-only main..HEAD` touches only the files listed
  in Implementation Notes Deliverables plus the two new tests.
- Codex review approved.
