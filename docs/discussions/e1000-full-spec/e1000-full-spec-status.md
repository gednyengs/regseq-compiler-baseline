# E1000 Full Spec Status

> **⚠ SUPERSEDED (2026-05-19).** The QEMU-patch-gated Stage-1/2/3 plan
> referenced here no longer exists (no QEMU device-model patch program
> remains). Implementation/evaluation tracking now follows
> `../e1000-evaluation-strategy/`. Retained for provenance only.

**Status**: SUPERSEDED (was SKELETON). See banner above.

**Role**: tracks the implementation state of each requirement in the live
fixture at `tests/fixtures/e1000-full-spec/src/driver.rs`. Stage membership
for every REQ-* is authoritatively defined in
`e1000-spec-requirements.md` Section 1 (trace legend) and 1.2 (stage
membership summary); this document does not re-list the stage assignments
and defers to that source to avoid drift.

The two dimensions are:

- **stage membership** (what section of the plan a REQ belongs to;
  authoritative source: `e1000-spec-requirements.md`)
- **implementation state** (whether the fixture currently captures the
  requirement; authoritative source: this doc)

## Status labels

- `captured`: represented in the fixture and compile-checked through the
  real `regseq-driver` wrapper path
- `in-progress`: fixture work scheduled for the current stage but not yet
  merged
- `not-yet-captured`: requirement not yet expressed in the fixture
- `deferred`: intentionally out of scope for this version

## Scope boundary

- This version targets a single concrete Intel 82540EM configuration.
- e1000e family and PCIe-only features (TSO, MSI-X, multi-queue) are
  intentionally out of scope by the variant pin.
- Other deferrals are enumerated in `e1000-spec-requirements.md` Section 7.

## Implementation snapshot

To be filled in as fixture work begins. Until then, all claim-path
requirements are `not-yet-captured`.

### Stage 1 implementation-state snapshot

(pending stage definition in `e1000-spec-requirements.md`)

### Stage 2 implementation-state snapshot

(pending stage definition in `e1000-spec-requirements.md`)

### Stage 3 implementation-state snapshot

(pending stage definition in `e1000-spec-requirements.md`)

### Deferred (outside the staged plan)

(pending deferred-requirement enumeration in
`e1000-spec-requirements.md`)

## Verification discipline

Each status change to `captured` should be backed by a successful compile
check of the fixture through the real compiler path, not just plain Cargo
syntax checking. This mirrors the discipline established in
`pl011-full-spec-status.md`.

## Change history

- 2026-05-04: skeleton stood up.
- 2026-05-19: SUPERSEDED by `../e1000-evaluation-strategy/`. The
  QEMU-gated staged plan is void; tracking follows the minimal-sufficient
  strategy. Retained for provenance only.
