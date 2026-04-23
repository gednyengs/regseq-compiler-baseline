# Shadow Registers

## Status

**Deferred** (2026-04-22). Implementation planned but not scheduled
until after Stage-1 pipe-clean closes. Path-X sequencing decision:
pipe-clean unblocks via Path A in the user-data initialization
contract (see `user-data-initialization.md`); shadow-register feature
is revisited once a working Rust-to-C driver baseline is generated.

Two Codex adversarial review rounds closed this design:
- Round 1: design review -- `shadow-registers-codex-review/round-1-design-review.md`
  (verdict: `adopt-with-amendments`).
- Round 2: implementation-plan review --
  `shadow-registers-codex-review/round-2-plan-review.md`
  (verdict: `needs-plan-amendment`; amendments folded into this
  document).

## Motivation

Many hardware registers require a software shadow: the driver keeps
an in-memory mirror of what has been written to the register, because
some register read behaviors (read-clear, write-combined, write-only,
cache-line friendliness) make direct hardware reads unreliable or
expensive. The PL011 IMSC register is a concrete case captured by
REQ-CORR-MASK-SHADOW (datasheet 3.3.8).

Today, shadows are expressed by the driver author as manual state:
`imsc_shadow: u32` on the driver struct at
`tests/fixtures/pl011-full-spec/src/driver.rs:72`, with a
`commit_imsc_shadow()` helper at `driver.rs:108-112` and six manual
call sites manipulating the shadow and then committing.

This is a semantic responsibility the compiler should own:

- **Paper story**: a "semantic driver model" that leaves register
  coherence to the author is less compelling than one where the
  compiler understands the shadow pattern and emits the storage plus
  the coherence code.
- **Reviewer defensibility**: manual shadow management in driver
  code is a known bug source in real OS kernels; the compiler can
  statically eliminate the bug class.
- **Generalization**: shadows also appear for write-only mode
  registers, control-only registers, and any register where reads
  have side effects the driver must avoid. The same DSL mechanism
  covers them.

## Design

### DSL surface

Shadow is a per-register-block-field property, declared via an outer
attribute on each entry inside `register_block!`:

```rust
register_block! {
    #[endian = LittleEndian, access_width = 32]
    Pl011Registers {
        // ...
        #[rseq::register_entry(shadow = true, reset_value = 0)]
        imsc: ReadWrite {
            RIMIM  OFFSET(0) NUMBITS(1) [],
            CTSMIM OFFSET(1) NUMBITS(1) [],
            // ...
        },
        // ...
    }
}
```

Properties of the attribute:

- `shadow = <bool>`: declares the entry as shadowed. Default is
  `false` (non-shadowed).
- `reset_value = <literal>`: **required** when `shadow = true`.
  Must fit the register's declared access width. This is the value
  the compiler-generated shadow field is initialized to at driver-
  struct allocation time.

The attribute is a property of the ENTRY (the MMIO instance at its
offset), not of the reusable bitfield descriptor inside the entry.
This matches the model separation in
`crates/regseq-model/src/model/register.rs` between descriptors
(reusable layouts) and block fields (instance-specific metadata).

Alternative syntaxes considered (see plan-review transcript Q3.1):

- **Trailing square-brackets** (`} [shadow = true, ...]`):
  rejected. Square brackets are already used for enum-like variant
  lists in the bitfield body.
- **Inline metadata fields** (`shadow: true, ...` inside the brace
  body): rejected. Mixes metadata with bitfield definitions;
  parser ambiguity.
- **Attribute name `#[rseq::shadow(...)]`** (narrow alternative):
  rejected because it closes the door to future per-entry metadata
  (deprecation markers, trace hints, emit flags).

### Semantics

For a register marked `shadow = true`, the compiler rewrites every
MMIO operation on that register to operate on the cache + hardware
coherently. Five primitive operations need shadow-aware variants;
`modify` composes from `read` + `write`.

| Operation | Shadow-aware behavior |
|---|---|
| `read(REG)` | returns cached value; no hardware read |
| `read_field(REG, FIELD)` | returns field from cached value; no hardware read |
| `write(REG, val)` | writes to both cache and hardware (full-register write; cache replaced, hardware written) |
| `write_field(REG, FIELD.val(v))` | pure write: zero unmasked bits in both cache and hardware; coherent |
| `force_read(REG)` | reads hardware, updates cache, returns hardware value. Documented idiom for bootloader-inherited-state resync |

**Modify composition**. `Mmio::modify(REG, FIELD.val(v))` is not a
first-class intrinsic. It is a method body that composes `read` +
(compute new value preserving unmasked bits) + `write`. Because
`read` and `write` are shadow-aware, `modify` is automatically
shadow-aware via composition; no `ShadowModify` IR node is required.

**Pre-existing gap**. `crates/regseq/src/mmio.rs:72` currently has
`Mmio::modify` ending in `todo!()`. That gap is a separate
pipe-clean concern, not a shadow-register prerequisite. The shadow
rewrite pass relies on `modify` reaching the point where it invokes
`read` and `write` as intrinsic calls; once that path works for
non-shadowed registers, it works for shadowed registers via
composition.

### Validation (Stage 1)

- Reject `shadow = true` on `WriteOnly` access kind. Write-only
  registers have no meaningful readback; shadow-as-"last-written"
  stores stale information on the first read cycle. Emit
  `ConstructionError::InvalidShadowTarget { register, reason:
  "write-only register cannot be shadowed" }`.
- Any future access kind not explicitly shadow-eligible defaults to
  reject (fail-closed).

Validation deferred:

- **W1C** (write-1-to-clear): not a first-class access kind today.
  PL011's ICR is W1C by convention but stored as `WriteOnly` in the
  descriptor. Documented caller contract in the meantime: "the
  author must not mark read-side-effect registers as shadow." A
  future access-kind extension introducing W1C as a first-class
  kind should add validation rejecting `shadow = true` on it.
- **Reserved bit policy**: the compiler cannot cross-reference
  datasheet reserved-bit resets from the DSL source. The
  author-provided `reset_value` literal is trusted for reserved
  bits. Width validation only.

### Concurrency doctrine

Shadow operations are not thread-safe. The compiler relies on the
existing ISR-exclusion discipline provided by `#[rseq::isr]` to
prevent races. Documented explicitly:

> The compiler-generated shadow field is single-writer within the
> driver's existing synchronization contract. Concurrent access
> from an ISR is serialized via the compiler's ISR-exclusion
> discipline. Drivers requiring atomic shadow access must use
> explicit synchronization primitives (e.g., spinlocks provided
> by the target OS); that pattern is not covered by this design.

### Compiler-managed state category

The auto-generated shadow storage lives in the EMITTED C driver
struct, not in the Rust DSL fixture. The fixture's driver struct
has no `imsc_shadow: u32` field.

In the model, shadow state is a new category on the driver:

```
DriverStruct {
    ...
    shadow_state: Vec<ShadowState>,
}

ShadowState {
    id: ShadowStateId,
    register: RegisterBlockFieldRef,
    storage_type: ScalarType,   // matches the register's access width
    reset_value: u64,
}
```

`ShadowState` is distinct from `DriverStruct.user_data`: it bypasses
the user-data initialization rules (see `user-data-initialization.md`)
because the compiler, not the author, declares these fields.

### Lowering location

Shadow rewrites happen at the **semantic IR level, via a standalone
pass run before `call_lowering`**. The lowering stack already
separates classification, lowering, validation, and projection
(`crates/regseq-backends/src/lowering/`), so a cross-cutting pass
over resolved register metadata is the natural shape. Placing
shadow logic inside `call_lowering` would make it harder to test,
harder to reason about, and harder to extend if future backends need
the same rewrite.

Five new IR nodes capture shadow operations:

- `ShadowRead(REG)` -- read cache
- `ShadowReadField(REG, FIELD)` -- read a field from cache
- `ShadowWrite(REG, val)` -- write cache and hardware
- `ShadowWriteField(REG, FIELD, val)` -- pure write both
- `ShadowForceRead(REG)` -- read hardware, update cache

Modify has no shadow node (composes via `ShadowRead` + `ShadowWrite`).

Explicit IR variants (not a single parameterized `ShadowOp`) are
preferred per Codex Q3.4 review: explicit variants preserve
exhaustive-match checking in validators, projectors, and backend
emitters.

### Initialization semantics

At compile time, the compiler generates backend-specific code that:

1. Allocates the driver struct (e.g., `devm_kzalloc` on Linux).
   Shadow field starts at zero bits (all-zero bit pattern; kzalloc
   guarantee).
2. **After allocation, before any author lifecycle hook runs**,
   writes `reset_value` to the shadow field. On Linux, this appears
   as `priv->imsc_shadow = 0;` (for `reset_value = 0`, this is a
   no-op after kzalloc but still emitted for clarity).
3. Author's `init()` runs as normal. If the driver needs to
   synchronize the shadow with post-reset hardware state (e.g.,
   bootloader-inherited non-zero IMSC), it uses `force_read` or
   writes a known value to the register (writing updates both
   cache and hardware).

This matches Path-A user-data init for zero-initialized scalars:
the kzalloc zero-bit pattern is the compiler's baseline, and any
additional initialization is emitted as an explicit store.

## Alternatives considered

See `shadow-registers-codex-review/round-1-design-review.md` for the
full debate. Brief summary:

| Alternative | Why rejected |
|---|---|
| Manual `Option<u32>` user-data shadow field | Semantic tax for an always-seeded field; unwrap/expect don't lower cleanly (see `user-data-initialization.md` alternatives section) |
| Author-visible `Shadow<T>` type wrapper | Puts semantic burden on author; conflicts with the "compiler understands shadows" paper story |
| Per-descriptor shadow attribute | Rejected by Codex Q7. Shadow is per-instance, not per-layout. Two register blocks sharing a descriptor layout need independent shadow storage |
| Static inference of shadow-eligible registers | Codex Q7: "inference would be unsound because current metadata cannot express read-clear, W1C, hardware-updated, or reset-source facts" |
| Delete the shadow; use hardware RMW | Violates REQ-CORR-MASK-SHADOW (PL011 datasheet 3.3.8) |

## Implementation plan

Seven phases. One harness task per phase per Codex Q3.6 (repo task-
spec workflow at `.harness/task-spec-workflow.md:120` expects
narrow, testable tasks within one crate or a tight crate pair).
Effort estimate 13-20 engineering days total (revised down from
14-22 after the `ShadowModify`-node drop per clarification that
modify composes via read+write).

Phase ordering is a dependency chain: each phase's exit criteria
unblock the next. Fixture conversion (Phase 6) uses a **preview
fixture** strategy: build a minimal shadow preview fixture after
Phases 2-3 to test metadata propagation end-to-end without churning
PL011; fully convert PL011 only after Phases 4-5 are functional.

### Phase 1 -- Macro surface (1.5-2.5 days)

**Goal**: `register_block!` parses
`#[rseq::register_entry(shadow = true, reset_value = <lit>)]` on
each entry; emits per-register-module metadata carrying shadow +
reset_value constants.

**Primary touch-points**:
- `crates/regseq-macros/src/register_block.rs:164` (parser)
- `crates/regseq-macros/src/register_block.rs:329` (emission)
- Macro tests (add if `crates/regseq-macros/tests/` doesn't exist
  for this macro yet)

**Exit criteria**:
- Macro expands shadow-annotated entries to per-register modules
  containing `pub const SHADOW: bool` and `pub const RESET_VALUE:
  <width_type>` items.
- Negative tests reject: `shadow = true` without `reset_value`;
  `reset_value` literal wider than register's declared width;
  malformed attribute syntax.
- `#[rseq::register_entry(...)]` without `shadow = true` parses
  cleanly as no-op (future extensibility).

### Phase 2 -- Model carriage (2-3 days)

**Goal**: `RegseqModel` carries shadow metadata on
`RegisterBlockField`; `DriverStruct` carries a new `shadow_state`
category; raw-artifact / normalization / verifier / serialization
paths all propagate it.

**Primary touch-points**:
- `crates/regseq-model/src/model/register.rs:88` (`RegisterBlockField`)
- `crates/regseq-model/src/model/driver.rs:29` (`DriverStruct.shadow_state`)
- `crates/regseq-model/src/model/mod.rs:21` (reexports)
- `crates/regseq-model/src/normalize/raw_artifact.rs:89`
  (`RawRegisterFieldDecl` shadow fields)
- `crates/regseq-model/src/normalize/surface.rs:399`
  (`RegisterBlockField` construction in normalization)
- `crates/regseq-model/src/verify/phase_1.rs:51` (verifier entry)
- `crates/regseq-model/src/verify/phase_2.rs:59` (verifier entry)
- `crates/regseq-model/src/verify/rules/cross_item.rs:73,143`
  (declared-ID bookkeeping for shadow_state)
- `crates/regseq-model/src/model/regseq_model.rs:56` (format version
  bump)
- `crates/regseq-model/src/serialize.rs:70` (serialization)

**Exit criteria**:
- Model serialization round-trip preserves shadow + reset_value
  metadata on `RegisterBlockField` and all entries in
  `DriverStruct.shadow_state`.
- Verifier rejects models where a register has `shadow == true` but
  `DriverStruct.shadow_state` has no matching entry (or vice
  versa).
- Cross-item bookkeeping assigns unique IDs to shadow state entries.

### Phase 3 -- Construction propagation + validation (2-3 days)

**Goal**: `regseq-driver` construction reads the macro-emitted
metadata, populates `RegisterBlockField.shadow` and
`DriverStruct.shadow_state`, and rejects unsupported combinations.

**Primary touch-points**:
- `crates/regseq-driver/src/construction/registers.rs:21,58`
  (`build_register_block` reads emitted consts)
- `crates/regseq-driver/src/construction/register_metadata.rs:42`
  (`RegisterIndex` carries shadow info)
- `crates/regseq-driver/src/construction/register_metadata.rs:338`
  (metadata resolution attaches shadow flag to intrinsic metadata)
- `crates/regseq-driver/src/construction/register_metadata.rs:452`
  (`is_register_intrinsic` enumerates intrinsics; add
  `RegisterForceRead` if Phase 4 is paired)
- `crates/regseq-driver/src/construction/register_metadata.rs:569`
  (access validation adds shadow eligibility rejection for WO)
- `crates/regseq-driver/src/construction/driver.rs` (populate
  `DriverStruct.shadow_state` from register-block fields with
  `shadow == true`)
- `crates/regseq-driver/src/construction.rs:127` (add
  `InvalidShadowTarget` variant)
- `crates/regseq-driver/src/construction.rs:451` (display text for
  the new variant)

**Exit criteria**:
- Fixture with `shadow = true` on `ReadWrite` compiles; construction
  emits a `ShadowState` entry.
- Fixture with `shadow = true` on `WriteOnly` fails construction
  with a clear error message naming the offending register.
- Raw-frontend artifact preserves shadow metadata end-to-end.

### Phase 4 -- Semantic IR + intrinsic surface (4-5 days)

**Goal**: `force_read` becomes a first-class intrinsic on `Mmio<T>`;
semantic IR gains five `Shadow*` variants; a standalone rewrite
pass rewrites register accesses on shadowed registers; every match
site on register intrinsics is updated.

**Primary touch-points**:
- `crates/regseq/src/mmio.rs` (add `force_read` method with
  intrinsic attribute)
- `crates/regseq-driver/src/extract/intrinsic_registry.rs` (register
  `force_read` as canonical intrinsic name)
- `crates/regseq-model/src/ir/intrinsic.rs:20,65,96` (add
  `RegseqIntrinsic::RegisterForceRead` + enumeration sites)
- `crates/regseq-model/src/semantic/expr.rs:50` (add `ShadowRead`,
  `ShadowReadField`)
- `crates/regseq-model/src/semantic/stmt.rs:73` (add `ShadowWrite`,
  `ShadowWriteField`, `ShadowForceRead`)
- NEW:
  `crates/regseq-backends/src/lowering/semantic_lowering/shadow_rewrite.rs`
  (standalone rewrite pass)
- `crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs:20`
  (classify `RegisterForceRead`)
- `crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:36`
  (run shadow rewrite pass before call_lowering; after rewrite,
  call_lowering sees `Shadow*` nodes for shadowed register accesses)
- `crates/regseq-backends/src/lowering/semantic_lowering/validation.rs:157`
  (accept `Shadow*` in the same places `RegWrite`/`RegModify` are
  accepted)
- `crates/regseq-backends/src/lowering/projection.rs:147`
  (projection handles `Shadow*` nodes)
- `crates/regseq-backends/src/lowering/from_model.rs:742` (portable
  lowering maps `RegisterForceRead`)

**Exit criteria**:
- Golden semantic-IR output for shadow preview fixture shows
  `ShadowRead` / `ShadowWrite` / `ShadowReadField` / `ShadowWriteField`
  nodes where the author wrote `read` / `write` / `read_field` /
  `write_field` on shadowed registers.
- `modify` on a shadowed register lowers to `ShadowRead` followed
  by `ShadowWrite` (via composition in `Mmio::modify`'s body).
- Non-shadowed register accesses continue to lower to existing IR
  nodes unchanged (zero regression for non-shadow call paths).

### Phase 5 -- Portable plan + Linux backend emission (3-4 days)

**Goal**: portable lowering plan carries shadow metadata; Linux
backend emits the shadow storage in the driver struct, initializes
it after `devm_kzalloc`, and lowers `Shadow*` IR nodes to coherent C.
Zephyr backend gets an explicit NotImplemented dispatch stub.

**Primary touch-points**:
- `crates/regseq-backends/src/lowering/plan.rs:322`
  (`RegisterObligations` + `RegisterFieldInfo` carry shadow +
  reset_value)
- `crates/regseq-backends/src/lowering/from_model.rs:521` (copy
  shadow metadata into the portable plan)
- `crates/regseq-backends/src/linux/planner.rs:99,140,146`
  (`shared_struct_fields` includes `<reg>_shadow` fields for every
  shadowed register in `DriverStruct.shadow_state`)
- `crates/regseq-backends/src/linux/emitter.rs:118` (header emission
  prints the shadow fields)
- `crates/regseq-backends/src/linux/emit_module.rs:694` (probe
  generation: `devm_kzalloc` unchanged)
- `crates/regseq-backends/src/linux/emit_module.rs:703` (after
  kzalloc, emit `priv-><reg>_shadow = <reset_value>;` for each
  shadow field before resource acquisition at line 717)
- `crates/regseq-backends/src/linux/emit_semantic.rs:405,637` (lower
  `ShadowRead`/`ShadowWrite`/etc. to coherent C)
- `crates/regseq-backends/src/linux/emit_behavior.rs:106` (legacy
  behavior emitter: either update or explicitly retire with a
  "legacy path does not support shadow registers" compile error)
- `crates/regseq-backends/src/zephyr.rs:1` (explicit NotImplemented
  path if backend dispatch routes to Zephyr with a shadowed model)

**Exit criteria**:
- Linux C emission for the shadow preview fixture produces a driver
  struct with a `<reg>_shadow` field, post-kzalloc initialization
  to the declared `reset_value`, and coherent cache+hardware
  updates on every `Shadow*` operation.
- Zephyr dispatch on a shadowed model fails with a clear
  "shadow registers not yet supported in Zephyr backend" message,
  NOT with a panic or silent miscompilation.
- Non-shadowed emission remains byte-equal to today's output.

### Phase 6 -- Preview fixtures + PL011 conversion (1-1.5 days)

**Goal**: validate end-to-end with a minimal shadow preview fixture
first; convert PL011 only once the feature is stable.

**Primary touch-points**:
- NEW: `tests/fixtures/shadow-preview/` (minimal fixture with one
  shadowed register, used as a focused test target during Phases 4-5)
- `tests/fixtures/pl011-full-spec/src/registers.rs:161` (add
  `#[rseq::register_entry(shadow = true, reset_value = 0)]` to
  IMSC)
- `tests/fixtures/pl011-full-spec/src/driver.rs:65` (remove
  `imsc_shadow: u32` field)
- `tests/fixtures/pl011-full-spec/src/driver.rs:108` (remove
  `commit_imsc_shadow()` helper)
- `tests/fixtures/pl011-full-spec/src/driver.rs:115` (rewrite
  `mask_all_interrupts` body to direct `self.regs.write_field(...)`
  or equivalent; KEEP the explicit IMSC hardware write -- it's the
  hardware resync, not just shadow management)
- `tests/fixtures/pl011-full-spec/src/driver.rs:236` (interrupt
  enable/disable helpers: direct `modify` calls on the shadowed
  register; no manual `commit_imsc_shadow` needed)
- `tests/fixtures/pl011-full-spec/src/driver.rs:434,476` (init and
  close paths: continue to call IMSC manipulation helpers; the
  helpers themselves have changed but call-site shape is preserved)

**Keep explicit IMSC hardware writes in `init()` / `port_close()`**
per Codex Q3.5: the compiler-generated shadow initialization only
initializes DRIVER STORAGE; it does not mask HARDWARE. The explicit
write ensures hardware is in a known state regardless of bootloader-
inherited state.

**Exit criteria**:
- Preview fixture compiles end-to-end through the Regseq compiler
  and produces Linux C matching a hand-authored reference.
- PL011 fixture compiles after conversion; emitted Linux C carries
  an `imsc_shadow` field, correct initialization, and coherent
  writes on every IMSC manipulation.
- No regression in other PL011 semantic-IR goldens (other register
  operations continue to lower identically).

### Phase 7 -- Artifact regeneration + tests + Codex review gate (1.5-2.5 days)

**Goal**: regenerate all stale expected artifacts; add tests for
the shadow feature; close with a Codex review round confirming
feature stability.

**Primary work**:
- Regenerate
  `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` via
  the production `--emit=raw-artifact` path.
- Regenerate `expected/regseq-model.json` via
  `REGEN_GOLDEN=1 cargo test -p regseq-model --test pl011_pipeline_phase6`.
- Regenerate IR goldens under `expected/ir/` via
  `UPDATE_GOLDENS=1 cargo test -p regseq-driver --test pl011_construction`.
- Verify all downstream tests depending on
  `expected/raw-frontend.json` pass (13 tests across
  `regseq-backends`, `regseq-analysis`, `regseq-model`,
  `regseq-driver`).
- New tests per phase exit criteria.
- Codex review gate: one round reviewing the landed feature against
  this design doc. Convergence required before feature is
  considered stable.

**Exit criteria**:
- All regenerated artifacts commit cleanly.
- Codex review round converges.
- Preview fixture + PL011 fixture compile identically across
  multiple pipeline runs (determinism).

## Risk register

**Highest risk: Phase 4 (semantic IR + intrinsic surface)**.
Cross-cutting changes to all register-intrinsic match sites plus
addition of five new IR variants plus a standalone rewrite pass.
Mitigation:
- Land the Shadow* IR variants with default match-arms returning
  the existing lowering (as if shadow == false) FIRST, so every
  match site remains exhaustive at each step.
- Build the rewrite pass incrementally, one intrinsic at a time,
  with a dedicated test per step.
- Use the preview fixture (Phase 6 early deliverable) to validate
  the pipeline end-to-end before touching PL011.

**Secondary risk: Phase 5 (portable plan + backend)**. Linux backend
consumes a portable lowering plan rather than the raw model
(`linux/planner.rs:99`), so the portable plan's shape must change
before Linux emission can work. Mitigation: land portable-plan
changes (`plan.rs:322`, `from_model.rs:521`) as a discrete sub-step
with its own test before starting Linux emission work.

**Tertiary risk: Phase 2 (model carriage)**. Format-version bumps
on the serialized model force regeneration of every golden that
depends on the PL011 regseq-model.json. Mitigation: batch all
Phase-2 model changes into a single format-version bump;
regenerate all goldens in one commit.

**Legacy-emitter risk**. `emit_behavior.rs` is the legacy behavior
emitter. If it is still exercised anywhere in Stage 1, the design
doc must pick one of: (a) update the legacy emitter for Shadow*
nodes, (b) explicitly retire the legacy path with a compile error.
This is a Phase-5 decision point; default to (b) with a clear
error unless Stage-1 pipelines actively require (a).

**Pre-existing `Mmio::modify` todo!() gap** (`regseq/src/mmio.rs:72`).
Not blocking shadow-register scope; fixing it is a parallel
pipe-clean concern. Shadow-register work assumes `modify` will be
made to compose `read` + `write` as intrinsic calls. If that
assumption doesn't hold at Phase 4 implementation time, shadow
must add its own `ShadowModify` IR node (reopening Codex Q3.4).

## Dependencies

**Prerequisites** (must land or be accepted-as-parallel before
shadow implementation starts):
- `Mmio::modify`'s `todo!()` path resolved (either made to compose
  read+write as intrinsic calls, or shadow scope expands to include
  a `ShadowModify` node).

**Blocks**:
- Stage-1 Linux C golden reference co-authoring should include
  shadow-register-emitted code paths; writing the reference before
  shadow lands means rewriting when shadow lands. Per the Path-X
  decision, the Linux C reference proceeds without shadow
  (using manual `imsc_shadow: u32` as-is); the reference is
  updated when shadow-register feature lands.

**Parallelizable**:
- W1C access-kind extension (not required for Stage-1 shadow
  validation).
- Harness work (control-channel proof, topology-fairness DTBs).
- User-data Path-A implementation (lands first; frees the
  construction-pass blocker for pipe-clean).

## Paper framing

> Regseq's semantic driver model promotes the shadow-register
> pattern to a first-class DSL construct. A register marked
> `shadow = true` is read and written through a compiler-managed
> cache in the emitted driver struct; the compiler emits the
> storage, the reset initialization, and the coherence code on
> every read/write/modify operation. The driver author writes
> ordinary register accesses; the compiler emits correct shadow
> discipline. This subsumes a known bug class in manual driver
> code (shadow/hardware divergence) at zero author cost.

## Codex review provenance

Two adversarial review rounds preserved under
`shadow-registers-codex-review/`:

- `round-1-design-review.md` -- design-level review of the
  original per-descriptor proposal. Verdict:
  `adopt-with-amendments`. Six amendments folded into this design:
  attach shadow to block-fields not descriptors; define semantics
  for all register intrinsics (six in original proposal, five
  after modify-composition clarification); explicit concurrency
  doctrine; defer W1C validation; represent compiler-managed state
  separately from user-data; lower at semantic IR not backend-only.
- `round-2-plan-review.md` -- implementation-plan review. Verdict:
  `needs-plan-amendment`. Amendments folded into this plan:
  adopt `#[rseq::register_entry(...)]` attribute syntax; metadata
  on both `RegisterBlockField` and `DriverStruct.shadow_state`;
  standalone rewrite pass; five explicit IR variants (not six);
  all additional per-phase touch-points (normalization, verifier
  bookkeeping, portable lowering, legacy emitter, Zephyr stub);
  effort re-estimated at 13-20 days.

User clarifications folded in (2026-04-22):
- `Mmio::modify` composes `read` + `write` intrinsics; no
  `ShadowModify` IR node required; effort estimate adjusted down
  from 14-22 days to 13-20 days.
- Path-X sequencing: user-data Path-A lands first; shadow-register
  feature deferred until post-pipe-clean.

## Open follow-ups (for implementation phase)

- Confirm at implementation time whether `Mmio::modify`'s `todo!()`
  gap has been resolved. If not, Phase 4 scope expands to add a
  `ShadowModify` IR node; effort estimate reverts to 14-22 days.
- Decide at Phase-5 implementation time whether `emit_behavior.rs`
  legacy path is updated or retired with a compile error.
- Decide at Phase-6 implementation time whether PL011's explicit
  IMSC hardware writes in `init()` and `port_close()` stay as
  `self.regs.write_field(...)` or are refactored for clarity.
  Default: minimal refactoring; preserve the hardware-resync
  discipline.
