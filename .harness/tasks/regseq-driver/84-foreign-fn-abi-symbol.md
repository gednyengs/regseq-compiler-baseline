# regseq-driver-84: Foreign-fn ABI symbol — `emission_name` for `body: None` instances

## Description

A foreign function (`extern "C" { fn abs(i: i32) -> i32; }`, `InstanceKind::Function`
with `body: None`) is given a **mangled** `emission_name` (`input__extern_0__abs`)
instead of its real ABI link name (`abs`). The backend emits `extern int
input__extern_0__abs(int);` + a call to it, which compiles but **fails at link**:
`undefined reference to input__extern_0__abs` — the real symbol is `abs`.

Surfaced by the §06 push (b8_item_kinds). Blocks **IN-3** (foreign fn body:None →
extern prototype) and the `IT-7` Foreign-linkage end-to-end path.

## Input Context

### Code anchors (confirm at HEAD — per workflow diagnosis)

- The bridge's `emission_name` minting for `InstanceKind::Function` — a `body: None`
  foreign instance (and any `#[no_mangle]`/explicit-link-name item) must take its
  **link name** (`tcx.symbol_name` / the foreign item's identifier), not the
  mangled internal name. Grep the instance/emission-name minting in
  `crates/regseq-driver/src/bridge/` (the `Instance.emission_name` source —
  `instance.rs:139` is the field; find the minting site).
- Matrix IN-3 (`C prototype / extern declaration only`), IT-7 (linkage:
  Foreign → `extern` decl, no def).

## Implementation Notes

For a `body: None` foreign function instance, set `emission_name` to the foreign
ABI symbol (the unmangled link name rustc would use — `extern "C" { fn abs }`
→ `abs`). Confirm this composes with the existing `#[no_mangle]` handling (the
entry `regseq_main` already links by its verbatim name, so the mechanism likely
exists — extend it to foreign decls). Ensure the backend's `extern` prototype +
call both use the resolved name (they read `emission_name`, so fixing it there
suffices).

## Non-Goals

- No backend change expected (it emits `emission_name` verbatim); if a backend
  sanitize step mangles a valid C identifier like `abs`, fix that instead — confirm
  which layer holds the mangled name.
- Not foreign statics (separate, if needed).

## Test Plan

1. Build + workspace green.
2. `kind=rust` codegen-coverage fixture (extend `b8_item_kinds`):
   `extern "C" { fn abs(i: i32) -> i32; }` then `unsafe { abs(-5) }` → 5, logged.
   The emitted TU declares `extern int abs(int);`, links against libc, runs →
   IN-3 green; re-run `cargo run -p codegen-coverage-runner`, ledger byte-stable.
3. Bridge unit test: a foreign `body:None` instance's `emission_name` is the ABI
   link name.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `codegen-coverage-runner`,
`regseq-backends`); single-threaded scoped tests; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture calling a libc foreign fn (`abs`) links-and-runs;
   IN-3 green in the ledger.
2. The foreign `emission_name` is the unmangled ABI symbol; `#[no_mangle]` /
   normal-fn naming unaffected.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
