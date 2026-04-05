# Task: regseq-lowering-03 - PL011 register, resource, and property realization lowering

## Description

Extend the PL011 lowering path so the authored full-spec fixture carries enough
typed realization data for backend emission without backend-owned PL011
knowledge.

This task should enrich `DriverLoweringPlan` and `LinuxBackendPlan` with the
register-layout, resource, property, and deployment facts that the Linux
backend currently cheats on by reconstructing or hardcoding.

## Input Context

Read before starting:

- `.harness/tasks/regseq-lowering/02-pl011-supported-slice-lowering-into-plans.md`
- `.harness/tasks/track-a-validation/09-pl011-bus-identity-end-to-end-proof.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `tests/fixtures/pl011-full-spec/`
- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/src/linux/`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`

Add the typed lowering data needed so register helpers, probe-time resource
acquisition, and property/default handling can be emitted from plans rather
than from device-specific knowledge embedded in the Linux backend.

## Implementation Notes

This task should make the following authored facts visible in lowering plans:

- register-block layout:
  - enough structural metadata to compute per-register byte offsets from the
    authored block layout without a hardcoded table
  - register offsets
  - register names
  - bitfield offsets and widths
  - named values where present
- firmware-property realization:
  - property id
  - destination field name
  - field type
  - required/optional policy
  - authored default literal
- resource realization:
  - MMIO join/field identity
  - interrupt join/field identity
  - optional clock metadata, including default frequency if deployment supplies one
  - optional reset metadata, including shared-ness if modeled
  - optional pinctrl metadata, including available states if modeled
- bus deployment metadata:
  - AMBA id/revision already normalized in earlier phases

Implementation continuity note:

- extend the existing lowering infrastructure from `regseq-lowering-01/02`
  rather than replacing it wholesale
- preserve any scaffolding that remains consistent with the richer plan-driven
  direction
- if an existing lowering representation is too weak for these requirements,
  refactor it in place rather than layering new ad hoc side channels beside it

The key rule for this task is:

- if Linux emission needs a register, property, clock, reset, pinctrl, or bus
  fact, that fact must already exist in the lowering plan

Do not allow the backend to:

- reconstruct property field names from property ids
- invent property default literals
- assume a UART input clock from a baud-rate property
- hardcode register offsets or bit masks in generic backend code

This task should not yet perform Linux code emission itself. It should make the
required facts available so later emission can stay plan-driven.

## Test Plan

Add tests to verify:

1. lowered register plans preserve offsets and bitfield information from the authored fixture
2. lowered property plans preserve field names, types, and default literals
3. lowered clock/reset/pinctrl plans preserve optionality and deployment metadata
4. Linux planning can see these facts without re-reading raw frontend models
5. output is deterministic and reviewer-readable

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- lowering plans carry the register/resource/property facts needed for backend emission
- property/default/resource behavior no longer depends on backend reconstruction
- register-helper emission can derive per-register byte offsets from lowered
  metadata rather than a hardcoded offset table
- named bitfield values are present in lowered data and do not require the
  backend to hardcode symbolic constants
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
