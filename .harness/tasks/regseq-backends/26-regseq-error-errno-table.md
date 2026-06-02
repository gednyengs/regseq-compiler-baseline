# regseq-backends-26: Generic-C RegseqError errno table (`C02`) + exhaustive test (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (task **T05** + the
> pure-table half of **T10**; the exhaustive **adapter-switch** half of T10 lives in
> regseq-backends-28, which has the adapter), contract **C02**, invariants
> **I03/I04/I05**, requirements **R07/R08**.

## Description

The per-target host error-mapping table for generic-C — the **first concrete
instance** of the per-target errno table the roadmap calls for. A pure,
**name-keyed** function from a `RegseqError` variant name to a libc `<errno.h>`
**symbol**, plus the exhaustive unit test over all 9 rows. Lives in the backend
(a per-target concern), not the target-agnostic model.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/` — add a pure
  `errno_symbol(variant_name: &str) -> Option<&'static str>`.
- The backend's `EmitError` enum (or `Unsupported(&'static str)`) — the caller
  (regseq-backends-28) turns a `None` (unknown marked variant) into a **fail-loud
  emit error**, never `EIO`.

### C02 table (keyed by `VariantLayout.name`; emitted symbolically)

| variant | symbol | | variant | symbol |
|---|---|---|---|---|
| `InvalidParameter` | `EINVAL` | | `DeviceBusy` | `EBUSY` |
| `OutOfMemory` | `ENOMEM` | | `PermissionDenied` | `EPERM` |
| `HardwareError` | `EIO` | | `DeviceNotFound` | `ENODEV` |
| `Timeout` | `ETIMEDOUT` | | `GenericFailure` | `EIO` |
| `Unsupported` | `EOPNOTSUPP` | | *(success)* `Ok(())` | `0` (caller) |

Unknown marked variant → `None` → **fail emit** (not `EIO`).

## Implementation Notes

- Keep this a **standalone pure fn** so the exhaustive test is fast and the adapter
  task (regseq-backends-28) reuses it.
- Emit **symbols only** (`"EINVAL"`), never host numerics — the `.c` stays
  deterministic (**I03**); numerics are resolved at run time by the harness.
- `Ok(())` → `0` is handled by the adapter caller, not this fn.

## Non-Goals

- Not the adapter / discriminant reads / `<errno.h>` plumbing (regseq-backends-28).
- Not the **adapter-switch** exhaustive test (the other half of PLAN T10) — that needs
  the adapter and lives in regseq-backends-28.
- Not the classifier (regseq-backends-27).
- No model/bridge changes.

## Test Plan

1. Exhaustive unit test: every one of the 9 rows maps to its exact symbol by name,
   order-independent (`GenericFailure` and `HardwareError` both → `EIO` accepted).
2. A bogus variant name → `None`.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`cargo clippy -p regseq-backends --
-D warnings`); `RUST_TEST_THREADS=1 cargo test -p regseq-backends --
--test-threads=1`.

## Acceptance Criteria

1. All 9 C02 rows map exactly, by name, order-independent (**C02**, **I04**).
2. Unknown name → `None` (caller fail-emits, never `EIO`) (**R08**, **I05**).
3. Output is symbolic only; no host numeric (**I03**).
4. fmt + scoped clippy clean; exhaustive test green.
