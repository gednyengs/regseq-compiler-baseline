# track-a-validation-11: QEMU Boot + Probe Harness (Productized Eval Baseline)

**Depends on**: `regseq-model-60`.
**Parallel with**: `regseq-backends-02`, `regseq-backends-03`,
`regseq-model-61`, `regseq-model-62`.

## Description

Phase-7.0 infrastructure task. Productizes the already-frozen PL011
eval-env baseline (kernel, rootfs, QEMU machine, DT override) into
a reusable Rust test harness callable from later tasks -- critically
task `track-a-validation-12` (reference blessing) and as an ongoing
regression test thereafter.

**This task does NOT re-research the eval environment.** The repo has
already frozen:

- Kernel: pinned ARM64 `v6.18.21` on `virt` machine
  (`experiments/pl011-eval-env/notes/phase-04-kernel-build.md:12-18,26-32`).
- Rootfs: Buildroot `2026.02` + BusyBox + `kmod`
  (`experiments/pl011-eval-env/notes/phase-05-rootfs-strategy.md:18-33,62-81,189-217`).
- Console: stock PL011 on `ttyAMA0`, NOT a target for generated drivers
  (`experiments/pl011-eval-env/notes/phase-02-kernel-config-policy.md:37-66`).
- Second AMBA PL011 reserved via DT / periphid override
  (`experiments/pl011-eval-env/notes/phase-07-owned-qemu-second-pl011-strategy.md:156-226,261-269`).
- Existing proof-of-concept OOT module
  (`experiments/pl011-eval-env/artifacts/regseq-pl011-amba-probe/regseq_pl011_probe.c:1-66`
  + `Makefile`) that binds to the second PL011 via AMBA matching.

The task consumes these and wraps them in a Rust API. Only
implementation-detail selection remains (the exact userspace
test-command sequence for one-byte write + close + unload);
everything architectural is already codified in the eval-env
notes.

The harness is the executable backbone of migration.md Q5 Layer 7
(QEMU functional smoke), the strongest validation signal available
before hardware-in-the-loop testing.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q5 Layer 7
  (QEMU functional smoke definition + pass criteria).
- All files under `experiments/pl011-eval-env/notes/` -- read
  every phase note to understand decision history and current
  frozen state.
- `experiments/pl011-eval-env/artifacts/regseq-pl011-amba-probe/`
  -- existing OOT module artifact. Usable as the self-test target
  (a known-good module that should boot + probe).
- `tests/e2e/tests/pl011_linux_module_host_build_proof.rs` lines
  18-28 (the existing `#[ignore]` + dedicated-runner pattern for
  heavy environmental tests; this task adopts the same pattern).
  Lines 191-211 and 283-297 for content-hash + artifact discovery
  patterns.
- `crates/regseq-backends/src/linux/planner.rs` lines 101-104,
  126-137 -- `LinuxSerialCoreRealization::dev_name` -- the
  backend-generated tty device name that the harness must target.
  NOT `/dev/ttyAMA0` (that's the stock console; see Codex round-3
  review).

## Implementation Notes

### Deliverables

1. A new Rust crate or test-support module. Choice:
   **`crates/regseq-qemu-harness/`** as a new test-support crate,
   so it's not pulled into production builds.
2. Rust API: `qemu_boot_probe_module(ko_path, dev_name, test_ops)
   -> Result<BootResult, BootError>`.
3. Infrastructure scripts / fixtures under `crates/regseq-qemu-harness/assets/`
   OR pointers into `experiments/pl011-eval-env/` for the
   already-frozen kernel/rootfs/DT artifacts.
4. Self-test under `crates/regseq-qemu-harness/tests/self_test.rs`:
   builds `regseq-pl011-amba-probe` OOT module, calls
   `qemu_boot_probe_module`, asserts the module probes successfully.
5. A README at `crates/regseq-qemu-harness/README.md` covering:
   which artifacts are consumed from `experiments/pl011-eval-env/`,
   how a developer runs the harness locally, the `#[ignore]`
   pattern and how to unignore for CI, performance expectations
   (boot time budget, timeout thresholds).

### Public API

```rust
/// QEMU boot + probe harness for regseq-generated Linux drivers.
///
/// See migration.md Q5 Layer 7 for the pass criteria this harness
/// enforces. See docs/discussions/models/regseq-model/migration.md.

use std::path::Path;
use std::time::Duration;

/// Target device the harness should open after probe succeeds.
///
/// The backend-generated `dev_name` from `LinuxSerialCoreRealization`
/// (e.g., `"ttyRSEQ0"` -- NOT `"ttyAMA0"`, which is owned by the
/// stock PL011 console).
#[derive(Debug, Clone)]
pub struct TargetTty {
    /// Name under `/dev/`, without the leading slash.
    pub dev_name: String,
}

/// Operations the harness should perform after probe succeeds.
#[derive(Debug, Clone, Default)]
pub struct PostProbeOps {
    /// If true, open the target tty after probe.
    pub open_tty: bool,
    /// If set, write this byte sequence through the tty. Must
    /// have `open_tty = true`.
    pub write_bytes: Option<Vec<u8>>,
    /// If true, verify the written bytes appeared on the host
    /// side of the PL011 (via QEMU's serial mux -- see README for
    /// plumbing details). Independent of `write_bytes`; a no-op
    /// if `write_bytes` is None.
    pub verify_host_receive: bool,
    /// If true, close the opened tty before proceeding to
    /// rmmod. Default true when `open_tty` is true.
    pub close_tty: bool,
    /// If true, `rmmod` the module and capture diagnostic output
    /// (warnings, leaks, locking complaints). Default true.
    pub unload_module: bool,
}

/// Outcome of the boot + probe run.
#[derive(Debug)]
pub struct BootResult {
    /// Full dmesg captured after module load. Useful for
    /// debugging failed assertions.
    pub dmesg: String,
    /// True if probe reached success without any oops / WARN /
    /// BUG messages.
    pub probe_succeeded: bool,
    /// True if the target tty was opened (only meaningful if
    /// `PostProbeOps.open_tty` was set).
    pub tty_opened: bool,
    /// True if the write-bytes operation completed successfully
    /// (only meaningful if `PostProbeOps.write_bytes` was set).
    pub bytes_written: bool,
    /// True if `verify_host_receive` was set AND the expected
    /// bytes were observed on the host side.
    pub host_received: bool,
    /// True if `close_tty` was requested AND the close succeeded
    /// (tty closed without error). Required for migration.md Q5
    /// Layer 7 blessing criterion.
    pub tty_closed: bool,
    /// True if `unload_module` was requested AND `rmmod`
    /// succeeded (module unloaded with no leaks / warnings in
    /// dmesg). Required for migration.md Q5 Layer 7 blessing
    /// criterion.
    pub module_unloaded: bool,
    /// Post-unload dmesg snippet. Empty string if `unload_module`
    /// was not requested. Used by task 12 to assert no warnings
    /// / leaks surfaced during unload.
    pub unload_dmesg: String,
    /// Total wall-clock time of the boot + probe + ops run.
    pub elapsed: Duration,
}

/// Errors that abort the run before assertions can be checked.
#[derive(Debug, thiserror::Error)]
pub enum BootError {
    #[error("QEMU failed to launch: {0}")]
    QemuLaunch(String),
    #[error("Kernel boot timed out after {0:?}")]
    BootTimeout(Duration),
    #[error("Module load (insmod) failed: {0}")]
    InsmodFailure(String),
    #[error("Target tty {0} did not appear in /dev")]
    TargetTtyMissing(String),
    #[error("Guest I/O error: {0}")]
    GuestIoError(String),
    #[error("Harness internal error: {0}")]
    Internal(String),
}

/// Run the harness: boot, insmod, perform post-probe ops, unload.
///
/// Returns a `BootResult` regardless of whether probe succeeded;
/// the caller reads `probe_succeeded` etc. to determine pass/fail.
/// Returns `BootError` only if the run could not complete
/// (infrastructure failure, not a module defect).
pub fn qemu_boot_probe_module(
    ko_path: &Path,
    target: TargetTty,
    ops: PostProbeOps,
) -> Result<BootResult, BootError> { ... }
```

### QEMU invocation details

- ARM64 `virt` machine, `-cpu cortex-a72` (matches eval-env notes).
- Memory: 512MB (overkill, but matches eval-env recommendation).
- Kernel: load from `experiments/pl011-eval-env/artifacts/kernel/vmlinux`
  (path confirmed at harness-build time; if missing, return
  `BootError::Internal` with a helpful pointer to
  `phase-04-kernel-build.md`).
- Rootfs: load from
  `experiments/pl011-eval-env/artifacts/rootfs/rootfs.cpio.gz`
  (ditto).
- Console: `-append console=ttyAMA0`. The harness reads kernel
  console output via QEMU's `chardev` redirect to capture dmesg.
- Second PL011: reserved via DT override per
  `phase-07-owned-qemu-second-pl011-strategy.md`. The harness
  copies the `.dtb` from the eval-env artifacts directory.
- `-initrd`, `-kernel`, `-dtb` all pinned at specific paths.
- Module: the harness copies `ko_path` into the rootfs (via a
  prepared initramfs or via a 9p shared folder; choose the approach
  closer to what the eval-env notes endorsed; document the choice).

### Inner loop (boot -> insmod -> ops -> rmmod -> shutdown)

Implemented via a userspace init script in the rootfs that:
1. Waits for network/devtmpfs to settle (matches eval-env
   `phase-05-rootfs-strategy.md`).
2. Runs `insmod /module.ko` and captures stdout/stderr.
3. If `open_tty`: waits up to 2s for `/dev/<dev_name>` to appear.
4. If `write_bytes`: opens the tty via raw read/write; writes
   bytes; reads any echoed data if `verify_host_receive`.
5. Runs `rmmod` and captures diagnostic output.
6. Prints a marker line the harness greps for to signal clean
   completion.
7. Triggers QEMU shutdown.

The harness communicates with the guest by parsing the kernel
console output (the unique marker lines), not by interactive I/O.

### Timeout budgets

- Total wall-clock: 120s default (configurable via env var).
- Boot: 30s.
- Module load: 10s.
- Post-probe ops: 10s.
- Shutdown: 10s.

Any overrun returns `BootError::BootTimeout(Duration)` with the
relevant step.

### #[ignore] pattern

Follow the existing `pl011_linux_module_host_build_proof.rs:18-28`
pattern: every test that calls the harness is `#[ignore]` by
default. Running them requires an explicit
`cargo test -p regseq-qemu-harness -- --ignored` or a `REGSEQ_RUN_QEMU=1`
environment variable. This keeps CI green when QEMU is unavailable
and respects the "don't weaken the QEMU pass criterion" directive
from Codex review round 3.

### Self-test contents

`crates/regseq-qemu-harness/tests/self_test.rs`:

```rust
#[test]
#[ignore = "requires QEMU; opt in with cargo test -- --ignored"]
fn self_test_probe_module_boots_and_binds() {
    let ko = build_self_test_module();
    let target = TargetTty { dev_name: "ttyRSEQ_PROBE".into() };
    let ops = PostProbeOps { open_tty: false, ..Default::default() };
    let result = qemu_boot_probe_module(&ko, target, ops).expect("boot");
    assert!(result.probe_succeeded, "self-test module must probe; dmesg: {}", result.dmesg);
}

fn build_self_test_module() -> PathBuf {
    // Invoke make in experiments/pl011-eval-env/artifacts/regseq-pl011-amba-probe
    // Return the path to the produced .ko.
}
```

### What this task does NOT do

- Does not write or change any regseq emitter code.
- Does not exercise regseq-generated C -- only the hand-written
  probe-module from the eval-env artifacts. Task 12 exercises
  regseq-generated C.
- Does not implement a full UART behavioral suite (deferred to
  Phase-8+ evaluation track).
- Does not modify `experiments/pl011-eval-env/**` -- that is a
  frozen research corpus. Consume it, don't change it.
- Does not rebuild the kernel or rootfs. The harness expects
  pre-built artifacts at known paths.

## Non-Goals

- No new kernel configuration research.
- No rootfs customization beyond what eval-env notes already
  prescribe.
- No QEMU version pinning beyond "whatever works with the
  v6.18.21 ARM64 kernel"; document the version used but don't
  infrastructure-lock it.
- No CI runner configuration (separate ops concern; mention in
  the README but don't implement).
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Test Plan

- `cargo build -p regseq-qemu-harness` succeeds.
- `cargo test -p regseq-qemu-harness` passes (only non-`#[ignore]`
  tests; those are lightweight API-shape checks).
- Opt-in run: `cargo test -p regseq-qemu-harness -- --ignored`
  passes on a local developer machine with QEMU and build
  dependencies installed. This exercises:
  - QEMU launch.
  - Kernel boot.
  - `insmod` of the self-test module.
  - Probe success detection.
  - Clean shutdown.
- The README documents the exact dependencies (qemu-system-aarch64
  version, cross-compiler, build-essential, etc.) needed to run
  the opt-in tests.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review track-a-validation-11 against .harness/tasks/track-a-validation/11-qemu-boot-probe-harness.md.

Files to audit:
- crates/regseq-qemu-harness/ (entire new crate)
- Top-level Cargo.toml (new member registration)
- experiments/pl011-eval-env/ (MUST NOT be modified -- verify)
- tests/e2e/tests/pl011_linux_module_host_build_proof.rs (pattern
  reference; must not be changed)

Focus on:
- Does the task productize already-frozen eval-env decisions rather
  than re-researching? Any decision in the harness that is not
  already in experiments/pl011-eval-env/notes/ is a red flag.
- Does the harness target the backend-generated dev_name
  (LinuxSerialCoreRealization::dev_name), not /dev/ttyAMA0?
- Is the #[ignore] pattern applied consistently so CI stays green
  without QEMU?
- Does the self-test actually exercise the AMBA-probe OOT module
  from experiments/pl011-eval-env/artifacts/, or does it invent a
  new module?
- Is the BootResult / BootError API shape adequate for task 12's
  blessing-gate requirements (probe + open tty + write + close)?

Do NOT flag:
- Absence of full UART behavioral tests (deferred).
- Absence of CI runner setup (ops scope).
</task>
... (standard grounding + structured output per SESSION.md) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-qemu-harness
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-qemu-harness -- -D warnings
# Opt-in smoke (requires QEMU + cross-toolchain):
REGSEQ_RUN_QEMU=1 cargo test -p regseq-qemu-harness -- --ignored
```

## Acceptance Criteria

- `crates/regseq-qemu-harness/` exists with `Cargo.toml`, `src/`
  (public API), `tests/self_test.rs`, `README.md`, and `assets/`
  (or documented pointers into `experiments/pl011-eval-env/`).
- Top-level `Cargo.toml` includes the new crate as a workspace
  member.
- `qemu_boot_probe_module` has the signature specified in
  Implementation Notes.
- `BootResult` / `BootError` cover all pass-criterion fields from
  migration.md Q5 Layer 7.
- Self-test `#[ignore]`'d; passes when run with `--ignored` on a
  developer machine.
- README documents dependencies + how to run + the adopted
  `#[ignore]` convention.
- `experiments/pl011-eval-env/` is unchanged (`git diff` empty).
- `cargo test --workspace` green.
- Codex review APPROVED.
