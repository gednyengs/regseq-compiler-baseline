# track-a-validation-12: PL011 Emitter-vs-Hand-Authored-Reference Verification Gate

**Depends on**: `track-a-validation-11` (QEMU harness),
`regseq-linux-backend-05` (emit_module rewrite complete),
`regseq-backends-02` (hand-authored reference exists).

## Description

Phase-7C validation. The **critical quality gate** for the
Phase-7 refactor. Runs the new pipeline end-to-end on the PL011
fixture, asserts the emitter's output byte-equals the
**pre-authored reference** from task 1
(`tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`),
then re-runs the QEMU functional smoke on the emitter's output
as a redundant-but-cheap belt-and-suspenders check.

Crucially, this task does NOT "bless" unknown emitter output.
The oracle (hand-authored reference) was blessed back in task 1
via QEMU smoke. Task 12's job is to prove the emitter converges
on that oracle. If emitter output drifts from the reference, the
emitter is wrong (go back to tasks 9/10); if QEMU smoke fails on
the emitter output but not on the reference, the emitter
introduced a Linux-side defect the construct tests missed
(go back to tasks 9/10 + add the missing assertion).

Per migration.md Q5 Layers 4-7: this task delivers Layers 4-7 by
confirming emitter == pre-authored reference (Layer 4) that was
already validated by the existing tests.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q5 Layer 4
  ("Blessed PL011 normalized reference") + Layer 5 (host-build) +
  Layer 6 (determinism) + Layer 7 (QEMU smoke with pass criteria).
- `.harness/tasks/track-a-validation/11-qemu-boot-probe-harness.md`
  -- the harness API this task invokes.
- `.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md`
  -- the normalization algorithm used to produce the reference.
- `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`
  -- upstream emitter.
- `crates/regseq-backends/src/linux/planner.rs` lines 101-104,
  126-137 -- `LinuxSerialCoreRealization::dev_name`, the target
  the harness opens. NOT `/dev/ttyAMA0` (stock console).
- `tests/fixtures/pl011-full-spec/src/driver.rs` lines 284-365
  -- PL011 authored serial callback bodies (`port_open`,
  `tx_byte`, etc.) that must actually execute in the QEMU smoke.
- `crates/regseq-backends/src/linux/plan.rs` lines 472+
  (`LinuxUartOpsEntry` shape) -- the mapping of authored callbacks
  to Linux uart_ops slots.
- `tests/e2e/tests/pl011_linux_module_host_build_proof.rs`
  (existing host-build pattern; retargeted to the reference).

## Implementation Notes

### Flow

```
1. Run full new pipeline end-to-end on PL011 fixture:
     normalize() -> populate_bindings() -> build_driver_lowering_plan_from_model()
                  -> build_linux_backend_plan() -> emit_linux_artifacts()
   Output: set of .c/.h files under a scratch directory.

2. Apply the normalization algorithm from task 1's contract to
   each file:
     - Strip trailing whitespace.
     - Canonical blank-line policy.
     - Banner rewrite (no timestamps, no absolute paths).
     - Tab/space normalization.

3. **Byte-equal to pre-authored reference** (PRIMARY GATE):
     Assert that the normalized main-driver translation unit
     emitted by step 2 is byte-identical to the file
     `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
     committed by task `regseq-backends-02`. Any byte-level diff
     is a task-9 or task-10 defect.

4. Compile to .ko against the pinned kernel per the existing
   host-build-proof procedure (redundant with task 1's validation,
   but catches the case where someone modifies the reference out
   of band).

5. Invoke qemu_boot_probe_module (from task 3) with the emitter's
   .ko (equivalent to the reference's .ko by step 3, but run
   explicitly):
     - ko_path: the compiled .ko
     - target: TargetTty {
           dev_name: plan.serial_core.dev_name
       }
     - ops: PostProbeOps {
           open_tty: true,
           write_bytes: Some(b"regseq"),
           verify_host_receive: true,   // via QEMU char-dev mux
           close_tty: true,
           unload_module: true,
       }

6. Assert (migration.md Q5 Layer 7 full criterion):
     - result.probe_succeeded == true
     - result.tty_opened == true
     - result.bytes_written == true
     - result.host_received == true     // exact byte match
     - result.tty_closed == true
     - result.module_unloaded == true
     - result.dmesg contains no "oops", "WARNING", "BUG", "Unable
       to handle", "Call trace"
     - result.unload_dmesg contains no "WARNING", "leak"
     - result.elapsed <= 60 seconds

7. If ALL assertions (step 3 AND step 6) pass:
     - Commit regression tests (see below).
     - Task closes.

   If step 3 fails (diff from pre-authored reference):
     - Task fails; escalate to tasks 9/10 to fix emitter.
     - Reference is not modified (it was blessed by task 1).

   If step 3 passes but step 6 fails (QEMU smoke fails on
   emitter output when task 1 said it passed on the reference):
     - Investigate environmental drift (QEMU version, kernel
       version, rootfs). If truly the same bytes produce different
       smoke outcomes, infrastructure has drifted; this is an
       infrastructure ops issue, not an emitter issue. Escalate
       appropriately.
```

### Regression tests (committed by this task)

Three regression tests under `tests/e2e/tests/` or
`crates/regseq-backends/tests/` (pick the location consistent with
existing patterns):

1. **Byte-equal-to-pre-authored-reference regression** (runs on
   every `cargo test`):
   ```rust
   #[test]
   fn pl011_emitter_output_matches_hand_authored_reference() {
       let emitted = run_pipeline_on_pl011();
       let normalized = apply_normalization(&emitted);
       let expected = std::fs::read_to_string(
           "tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c"
       ).unwrap();
       assert_eq!(normalized, expected,
           "emitter output must exactly match the hand-authored reference");
   }
   ```

2. **Host-build regression against pre-authored reference**
   (`#[ignore]`'d to match existing pattern; requires
   cross-toolchain + pinned kernel tree):
   ```rust
   #[test]
   #[ignore = "requires cross-toolchain + kernel tree; opt in with cargo test -- --ignored"]
   fn pl011_reference_builds_against_kernel() {
       // Existing pl011_linux_module_host_build_proof.rs is
       // retargeted: its input becomes
       // tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c
       // (the hand-authored reference, NOT live emitter output --
       // test 1 above already asserts they match). The #[ignore]
       // pattern matches tests/e2e/tests/pl011_linux_module_host_build_proof.rs:18-28.
   }
   ```

3. **QEMU smoke regression against pre-authored reference**
   (`#[ignore]`'d; runs with explicit opt-in):
   ```rust
   #[test]
   #[ignore = "requires QEMU; opt in with cargo test -- --ignored"]
   fn pl011_reference_qemu_smoke() {
       // Builds the hand-authored reference (redundant with
       // task 1, but guards against environmental drift in the
       // kernel / QEMU / rootfs baseline).
       let ko = compile_reference_to_ko();
       let result = qemu_boot_probe_module(&ko, pl011_target(),
           pl011_ops()).expect("boot");
       assert_full_smoke_pass(&result);
   }
   ```

### Ongoing regression target

- **Test 1 (byte-equal)**: always re-runs the emitter pipeline
  and diffs against the hand-authored reference. Catches every
  emitter drift, by definition.
- **Test 2 (host-build)**: builds the hand-authored reference
  against the kernel. The emitter is not involved -- test 1
  already asserts emitter == reference. This test guards against
  the reference drifting out of kernel compatibility (e.g., if a
  kernel upgrade breaks the reference; signals infra drift).
- **Test 3 (QEMU smoke)**: same, against the hand-authored
  reference. Guards against environmental drift.

All three tests use the hand-authored reference as the ground
truth; the emitter is validated transitively through test 1.

### PL011 hardware-exercising callback coverage

For migration.md Q5 Layer 7 to actually falsify a self-consistent-
but-wrong contract, the QEMU smoke must execute register-touching
callback paths. At minimum:

- `port_open` (called on `startup` when the tty device is
  opened).
- `tx_byte` (called when bytes are written to the tty).
- `configure` (called when the tty's termios is set; triggered
  implicitly by the first open).

Verify in the test that these paths were entered by checking
dmesg for the `dev_info` / `dev_dbg` log messages emitted from
within them (per the authored driver's logging calls).

### Normalization algorithm application

The normalization algorithm from task 1's contract is applied via
a helper (`crates/regseq-backends/src/normalize_output.rs` or
similar). This task implements the helper if task 2 did not
already provide it. The algorithm is deterministic; a
determinism test (two runs produce byte-equal output) is included.

### What this task does NOT do

- Does not retarget E2E assertions beyond the three regression
  tests above (task 13 handles the broader E2E retarget).
- Does not delete any legacy code (tasks 14, 15).
- Does not modify emitters (tasks 9, 10).
- **`crates/regseq-driver/src/construction*`** strict non-goal.

## Non-Goals

- No QEMU behavioral suite beyond the smoke (deferred to a future
  eval track).
- No hardware-in-the-loop testing.
- No CI runner setup (ops scope).
- No attempt to prove functional equivalence with the stock
  Linux PL011 driver; the comparison is "our generated driver
  works" not "our generated driver matches upstream."

## Test Plan

- **Blessing run**: this task's manual procedure described in
  Implementation Notes. Pass/fail determines whether the golden
  commit lands.
- **Byte-equal regression**: passes on the committed golden.
- **Host-build regression**: passes against the committed golden.
- **QEMU smoke regression** (opt-in): passes against the
  committed golden or live emitter output.
- **Determinism test**: two runs of the pipeline produce
  byte-identical pre-normalization output (normalization is
  idempotent, so this is equivalent to full determinism).
- `cargo test --workspace` green.
- `cargo test --workspace -- --ignored` passes on a developer
  machine with QEMU set up.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review track-a-validation-12 against .harness/tasks/track-a-validation/12-pl011-reference-blessing-gate.md.

Files:
- tests/goldens/pl011/linux/normalized_reference.c (blessed golden)
- Byte-equal regression test
- Host-build regression test
- QEMU smoke regression test
- Any normalization-algorithm helper not landed in task 1

Focus on:
- Golden was actually blessed via QEMU pass, not committed
  from a failing run. Verify the blessing procedure ran and
  produced a passing result (commit message should describe).
- Target tty is the backend-defined dev_name, NOT ttyAMA0.
- QEMU test is #[ignore]'d so CI stays green without opt-in.
- Byte-equal test re-emits + normalizes (Option A), not only
  reads the golden.
- Normalization algorithm is deterministic + idempotent; a test
  asserts this.
- Register-touching callback paths (port_open, tx_byte,
  configure) are demonstrably executed in the smoke -- dmesg
  assertion or similar.
- No leftover "TODO" / unimplemented! paths in the emitter
  surface the blessing exercises (would imply the emitter
  implementation is incomplete for PL011).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED. If blessing fails, the task fails per
task.py convention and the author returns to task 1 / 9 / 10.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
# Opt-in QEMU smoke + host-build (scoped to the two tests this task owns):
REGSEQ_RUN_QEMU=1 cargo test -p regseq-e2e --test pl011_normalized_reference_qemu_smoke -- --ignored
cargo test -p regseq-e2e --test pl011_normalized_reference_builds_against_kernel -- --ignored
```

## Acceptance Criteria

- `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  already exists (authored by task 1); this task verifies the
  emitter converges on it. No new reference file is committed by
  this task.
- Byte-equal-to-pre-authored-reference regression test passes
  (runs on every `cargo test`).
- Host-build regression test `#[ignore]`'d; passes with explicit
  opt-in (matches existing
  `pl011_linux_module_host_build_proof.rs` pattern).
- QEMU smoke regression test `#[ignore]`'d; passes with explicit
  opt-in.
- Register-touching callback paths executed in the smoke (probe +
  tty open + byte write + close + unload without warnings).
- Determinism test passes.
- `cargo test --workspace` green.
- Codex review APPROVED.
