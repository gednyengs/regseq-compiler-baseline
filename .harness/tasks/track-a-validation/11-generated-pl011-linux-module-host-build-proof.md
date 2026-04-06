# Task: track-a-validation-11 - Generated PL011 Linux module host-build proof

## Description

Prove that the generated PL011 Linux artifact set builds cleanly as an
out-of-tree module against the owned evaluation kernel before any guest/QEMU
smoke validation is attempted.

This task exists because backend unit tests previously passed while the real
generated module still failed against the pinned kernel build.

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/23-backend-emission-integration-for-driver-output.md`
- `.harness/tasks/regseq-lowering/08-semanticmodel-projection-and-driverloweringplan-behavior-migration.md`
- `.harness/tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md`
- `.harness/tasks/track-a-validation/10-pl011-full-spec-lowering-completeness-and-backend-independence-proof.md`
- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md`
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `scripts/` or `experiments/pl011-eval-env/scripts/` if narrow host-build
  automation is needed for deterministic proof
- retained build logs or proof artifacts where needed for reviewer inspection

Add a deterministic host-side proof that emitted PL011 Linux artifacts compile
against the owned kernel with the exact evaluation build settings.

## Implementation Notes

Canonical build environment for this task:

- kernel tree:
  - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- kernel revision:
  - `v6.18.21`
- generated driver root:
  - `output/linux/pl011-full-spec/`
- required tool variables:
  - `ARCH=arm64`
  - `CROSS_COMPILE=aarch64-linux-gnu-`

The canonical host-side module build shape is:

```bash
export KDIR="$HOME/regseq-eval/pl011/kernel-src/pl011-arm64-linux"
export MODULE_DIR="$PWD/output/linux/pl011-full-spec"

make -C "$KDIR" \
  M="$MODULE_DIR" \
  ARCH=arm64 \
  CROSS_COMPILE=aarch64-linux-gnu- \
  modules
```

This task should retain enough evidence to show:

- the generated artifact set was produced by the canonical Regseq emission path
  for `pl011-full-spec` during this task, not reused from a stale prior run
- the generated artifact set was refreshed
- the actual `.ko` build completed successfully
- there were no hidden handwritten compile fixes outside the generated output
- the emitted code targets the pinned evaluation kernel APIs

The proof must use the real compiler/emission flow that the repository
currently supports. If the current canonical emission path requires explicit
authored constant provisioning or another non-default-but-supported compiler
input step, that step must be shown explicitly in the proof instead of being
hidden behind an implication that the default lowering path resolved
everything automatically.

Semantic-IR continuity note:

- this proof is downstream of the new semantic-IR chain and must validate the
  canonical emission path after `DriverLoweringPlan.behavior` has migrated to
  `SemanticModel`
- the validation harness must not bypass that path or revive older
  `BehaviorPlan`-centric code paths as a convenience

This task must consume the production compiler-path emission implemented for
`EmitSpec::Driver`. It must not reconstruct backend plans directly inside the
validation test as a substitute for the canonical emission flow.

The proof must not:

- patch generated C by hand between emission and build
- patch the kernel tree as part of this task
- compensate for backend gaps outside the canonical generation/build flow

This task is the host-build gate. It should not boot QEMU or load the module
in the guest. That remains the responsibility of `track-a-validation-07`.

## Test Plan

Add validation to verify:

1. the generated PL011 artifact set can be emitted deterministically
2. the generated module builds out-of-tree against the owned kernel
3. build logs are retained deterministically
4. module metadata can be inspected deterministically after build
5. the module build consumed freshly generated output without manual edits
   between emission and build

Test commands should include the exact host build command or a narrow wrapper
around it.

## Acceptance Criteria

- the emitted PL011 Linux module builds successfully against the owned kernel
- the proof shows the exact emission command and exact host build command that
  produced the reviewed artifact set
- retained artifacts are sufficient to prove the build consumed freshly
  generated output with no handwritten edits between emission and build
- retained artifacts/logs are sufficient for reviewer inspection
- retained reviewer evidence includes:
  - generated file list under `output/linux/pl011-full-spec/`
  - host kernel build log
  - resulting `.ko` path
  - deterministic `modinfo` output for the built module
- this host-build gate passes before any guest smoke validation is considered
- tests / scripted validation pass
