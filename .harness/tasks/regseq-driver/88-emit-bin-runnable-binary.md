# regseq-driver-88: `--emit bin` — link the generic-C TU into a runnable binary

## Description

`cargo regseq --emit generic-c` (regseq-driver-87) writes a *library* C TU
(`regseq-out/generic.c`) with no `main` and a crate-qualified entry symbol
(`sanitize(emission_name)`, e.g. `regseq_gc_smoke_regseq_main`). Running it today
is a manual `cc` + wrapper-`main` dance (see `running.md`). This task productizes
that: **`cargo regseq --emit bin[=NAME]`** emits the TU **+** an entry shim
**+** links them with host `cc` into `regseq-out/<NAME|"generic">`.

Full design + decisions + the Codex review are in the plan doc
`docs/discussions/generic-c-backend/phase-1/08-emit-bin-productization-plan.md`
(read it first). v1 is **build-only** (no exec, no exit-code propagation).

## Input Context

### Architecture (the source/link split — Codex-endorsed)

- **regseq-driver** holds the validated `RegseqModel`, so it resolves the entry +
  writes the source. `emit_one(EmitSpec::Bin)` writes `generic.c` AND
  `generic_main.c` (the shim).
- **cargo-regseq** orchestrates around `cargo check`, so after a successful check it
  scans `options.emit` for `Bin` and runs the `cc` link.

### Code anchors (confirm at HEAD)

- `crates/regseq-config/src/emit.rs:19-91` — `EmitSpec` enum + `FromStr` + `Display`
  (serde `tag="type", rename_all="kebab-case"`). Add `Bin { name: Option<String> }`
  (serde tag `bin`); `NAME` is a binary name, not a path. Mirror the `json`
  subtype-rejection shape.
- `crates/regseq-config/src/lib.rs` / `options.rs` — add `pub const CC_FLAGS:
  &[&str] = &["-std=gnu11", "-fwrapv", "-Wall"]` (the link-flag home; cargo-regseq
  already deps `regseq-config`, NOT `regseq-backends`).
- `crates/codegen-coverage-runner/src/toolchain.rs:67` (`generate_main_c`) +`:84`
  (`CC_FLAGS`) — move `generate_main_c` to `regseq-backends` (root re-export in
  `lib.rs`, like `entry_c_symbol`); point `CC_FLAGS` at `regseq_config`. Update the
  runner's call sites + `tests/toolchain.rs:85`.
- `crates/regseq-backends/src/generic_c/naming.rs:45` — `entry_c_symbol(model,
  "regseq_main") -> EntrySymbol{Resolved|NotFound|Ambiguous}` (already root-exported
  at `lib.rs:11`). Default suffix `"regseq_main"` matches the runner
  (`pipeline.rs:235`, `manifest.rs:66`).
- `crates/regseq-driver/src/emit.rs:33-58` — `emit_one` (exhaustive match). Add the
  `Bin` arm; `collect_unsupported_warnings` (`:67-77`) → `Bin => None`.
  `crates/regseq-driver/src/callbacks.rs:220` already routes an `emit_one` `Err` to
  `tcx.dcx().fatal`. Driver already deps `regseq-backends` (driver-87).
- `crates/cargo-regseq/src/cli.rs:35-39` — `--emit` help (add `bin[=NAME]`); the spec
  flows through the existing `Vec<EmitSpec>` (no new arg).
  `crates/cargo-regseq/src/toolchain.rs:20-31` — add a production `$CC`-then-PATH `cc`
  probe (only the e2e test has one today).
  `crates/cargo-regseq/src/invoke.rs:47-70` / `lib.rs:31-40` — after a successful
  `cargo check`, scan `options.emit` for `Bin`, link the two known paths.

### Verification surfaces

- `crates/regseq-config/tests/emit.rs` — `bin` parse/display/round-trip/subtype.
- `crates/regseq-driver/src/emit.rs` inline tests — `emit_one(Bin)` writes both files
  with the resolved entry; `Ambiguous`/`NotFound` → error.
- `crates/cargo-regseq/tests/cli.rs` — `--emit bin` parse + help listing.
- A new `crates/cargo-regseq/tests/*` e2e (mirror `generic_c_emit.rs`): `--emit bin`
  on the playground → `regseq-out/generic` runnable → run → `[INFO] smoke=42`.
- `crates/codegen-coverage-runner/` — golden `.c` + ledger byte-stable after the move.

## Implementation Notes

Per the plan doc edit-point list (1–9). Order: config (`EmitSpec::Bin` + `CC_FLAGS`)
→ shared-helper move (`generate_main_c` → backends, update runner + its test) → driver
`emit_one(Bin)` arm → cargo-regseq `cc` probe + post-check link → docs + tests. The
`emit_one(Bin)` arm reuses the GenericC emit for `generic.c`, then resolves the entry
and writes `generic_main.c`. Keep `generate_main_c`'s output byte-identical so the
runner stays byte-stable.

## Non-Goals

- No exec / `--run`; no exit-code propagation from a non-`void` entry (deferred).
- No `bin` × `--emit generic-c=PATH` interaction — `bin` is self-contained at the
  default paths (D7).
- No crate-name default (cargo metadata doesn't resolve the primary package here) —
  fixed default `generic`.
- No cross-compile / custom link flags / extra objects.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-config`, `regseq-backends`,
`regseq-driver`, `cargo-regseq`, `codegen-coverage-runner`) zero NEW warnings;
single-threaded scoped + full suite green; codegen-coverage runner verdict `pass` +
ledger byte-stable. Codex soundness review of the final diff before close.

## Acceptance Criteria

1. `--emit bin` and `--emit bin=NAME` parse to `EmitSpec::Bin` (serde tag `bin`),
   round-trip through `Display`, subtype rejected.
2. The driver's `emit_one(Bin)` writes `generic.c` + `generic_main.c` with the
   resolved crate-qualified entry; `Ambiguous`/`NotFound` → fatal driver diagnostic.
3. cargo-regseq links the two after a successful `cargo check` → a runnable
   `regseq-out/<NAME|"generic">`; missing/failed `cc` → a clear error.
4. End-to-end: `cargo regseq --emit bin` on a supported crate produces a binary that
   runs and prints the expected diag-log output (`[INFO] smoke=42`).
5. The `generate_main_c`/`CC_FLAGS` move is behaviour-preserving: runner verdict
   `pass`, ledger byte-stable.
6. Help text + README + `running.md` updated; fmt + clippy (scoped) clean; suites green.
