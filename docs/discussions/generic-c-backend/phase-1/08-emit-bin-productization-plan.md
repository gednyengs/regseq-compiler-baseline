# 08 — `--emit bin`: productizing a runnable binary from the generic-C backend

> **Plan-first design doc.** Resolve the architecture for turning the emitted
> generic-C translation unit into a host executable the user can run, then derive
> the implementation task. Decisions here are reviewed (Codex) before any code.

## Problem

`cargo regseq --emit generic-c` (regseq-driver-87) writes `regseq-out/generic.c`
— a **library** translation unit. To run it today a user must, by hand:

1. discover that the TU has **no `main`**, and
2. discover that the entry symbol is **crate-qualified** (`sanitize(emission_name)`,
   e.g. `regseq_gc_smoke_regseq_main`, not bare `regseq_main`), then
3. write a wrapper `main` that calls it, and `cc` the two together.

The end-to-end smoke test and `running.md` both have to *explain* these two
gotchas. Productizing means the tool does steps 1–3 for the user: one command
that yields a runnable binary.

## Goal & scope (v1)

**`cargo regseq --emit bin[=NAME]`** → emit the generic-C TU **+** an entry shim
**+** link them into an executable under the output dir. **Build-only**: the user
runs the binary themselves (no `--run`/exec in v1).

- `--emit bin` (bare) → `regseq-out/generic` (the executable)
- `--emit bin=myprog` → `regseq-out/myprog`
- The `bin` spec implies generic-C emission (writes `generic.c` + `generic_main.c`
  at the default location) — no need to also pass `--emit generic-c`.

**Non-goals (v1):** executing the binary; exit-code propagation from a non-`void`
entry (`u32`/`i32` → process exit — the runner's documented deferred feature, stays
deferred); custom link flags / extra objects / multi-entry; cross-compilation
(host `cc` only, consistent with the hosted-userspace scope).

## Key realization

"Final binary" is **not another emit artifact** — it is a **build action** layered
on emit. The two gotchas are one problem: the TU is a *library*, and making an
*executable* requires a `main` that calls the resolved entry. Almost every piece
already exists and is reused, not reinvented:

| Piece | Current location | Reuse |
| --- | --- | --- |
| `entry_c_symbol(model, suffix) → EntrySymbol` | `regseq-backends/src/generic_c/naming.rs:45` (already `pub`) | Resolve the crate-qualified entry symbol from the model |
| `generate_main_c(entry) → String` | `codegen-coverage-runner/src/toolchain.rs:67` | The `int main(void){ entry(); return 0; }` shim |
| `CC_FLAGS` + the `cc` invocation | `codegen-coverage-runner/src/toolchain.rs:84,102` | Pinned link flags (`-std=gnu11 -fwrapv -Wall`) |

## Architecture — who does what

The split follows where the needed information lives:

- **regseq-driver** is the only process holding the validated `RegseqModel`, so it
  is the only place that can resolve the entry symbol. → `emit_one(EmitSpec::Bin)`
  emits **`generic.c` (the library TU) *plus* `generic_main.c` (the entry shim)** —
  a complete, well-defined model→source job (the *source half* of "bin").
- **cargo-regseq** is the orchestrator that runs once around `cargo check`, so it
  owns the **link step** (the *link half*): after the check succeeds, if the emit vec
  contains `Bin`, invoke
  `cc CC_FLAGS regseq-out/generic.c regseq-out/generic_main.c -o regseq-out/<name>`.

This keeps each layer at its altitude (driver = produce C source; cargo-regseq =
link) and avoids the fragile alternative of cargo-regseq grepping the `.c` for the
entry symbol — it never needs to, because the symbol is baked into the shim by the
driver.

**The one honest wart (chosen deliberately).** `EmitSpec::Bin` is the single emit
variant whose artifact is **not complete after `emit_one`** — the `.c`+shim it writes
are link *inputs*, finished into an executable only by cargo-regseq's later `cc` step.
So cargo-regseq must **inspect `options.emit` for `Bin`** (it treats the emit vec as
opaque pass-through today). This is the cost of one uniform `--emit` vocabulary; it is
contained to (a) the `emit_one(Bin)` arm and (b) one post-check scan in cargo-regseq.
The alternative — a dedicated `CompilerOptions.build_bin` field surfaced as a separate
`--bin` flag — keeps `EmitSpec` purely "model→file" but costs a second top-level flag;
the project chose the uniform surface.

```
cargo regseq --emit bin=myprog
  │  (cargo-regseq) parse --emit -> CompilerOptions.emit contains EmitSpec::Bin{ name }
  │  serialize REGSEQ_OPTIONS; run `cargo check` (RUSTC_WRAPPER = regseq-driver)
  ▼
regseq-driver (after_analysis): run_bridge -> RegseqModel -> validate
  │  for spec in emit: emit_one(model, spec, output_dir)
  │    EmitSpec::Bin{..} => write generic.c  (the library TU)
  │                    AND write generic_main.c (the entry shim):
  │                        entry = entry_c_symbol(model, "regseq_main")
  │                          Resolved(sym)  -> generate_main_c(sym) -> generic_main.c
  │                          NotFound/Ambiguous -> fatal diagnostic
  ▼  (cargo check returns Ok)
cargo-regseq: emit vec contains Bin{name}? -> link:
  │  cc CC_FLAGS regseq-out/generic.c regseq-out/generic_main.c -o regseq-out/<name|"generic">
  ▼
regseq-out/myprog            (runnable executable)
```

## Refactor: share the toolchain helpers (no copy-paste)

`generate_main_c` + `CC_FLAGS` currently live in the **runner** crate
(`codegen-coverage-runner/src/toolchain.rs:67,84`). Promote each to the crate
whose consumers already depend on it, so there is one source of truth and **no new
cross-crate dependency** (Codex review):

- **`generate_main_c(entry) → String` → `regseq-backends`**, **re-exported at the
  crate root** (`pub use ... generate_main_c` in `lib.rs`, like `entry_c_symbol`)
  — NOT under `regseq_backends::generic_c::...`, because `generic_c` is a *private*
  module (`lib.rs:9`); the path would not be reachable. The shim text is a backend
  emission contract (it must call `sanitize(emission_name)`), so it belongs with the
  backend. Consumers: the **driver** (writes the shim) and the **runner** — both
  already depend on `regseq-backends`.
- **`CC_FLAGS: &[&str]` → `regseq-config`.** The pinned host-`cc` flags
  (`-std=gnu11 -fwrapv -Wall`) are a toolchain-invocation config, and the linker is
  **cargo-regseq**, which already depends on `regseq-config` but NOT on
  `regseq-backends`. Homing `CC_FLAGS` in `regseq-config` lets cargo-regseq link
  without taking a new `regseq-backends` dependency. The runner (which deps both)
  reads it from there too.
- `codegen-coverage-runner/src/toolchain.rs` calls/re-exports both instead of
  defining its own. **Behaviour-identical**: keep `generate_main_c`'s output string
  byte-identical so the runner's golden `.c` / ledger stay byte-stable (the snapshot
  oracle compares only the *emitted* TU vs `golden.c`, so an unchanged shim string is
  safe). **Update the runner's existing test consumer**
  `codegen-coverage-runner/tests/toolchain.rs:85` (`generate_main_c_calls_the_entry`)
  to the new path.

## Decisions

- **D1 — `EmitSpec::Bin { name: Option<String> }`, a real emit variant.**
  *(User-chosen, weighing the tradeoff below.)* `--emit bin` joins the existing
  `--emit` vocabulary (one surface, repeatable, composable through the existing
  `Vec<EmitSpec>`). **Accepted cost:** `Bin` is the one emit variant whose artifact
  is not complete after `emit_one` — `emit_one(Bin)` writes the `.c`+shim (the source
  half), and **cargo-regseq must scan `options.emit` for `Bin`** to do the `cc` link
  (the link half). The rejected alternative was a separate `--bin` flag backed by a
  dedicated `CompilerOptions.build_bin` field — purer layering (`EmitSpec` stays
  model→file) but a second top-level flag. We chose surface uniformity; the wart is
  contained to the `emit_one(Bin)` arm + one post-check scan.
- **D2 — build-only v1.** No exec, no exit-code propagation. *(User-confirmed.)*
- **D3 — entry shim is a separate file** (`generic_main.c`), keeping `generic.c` a
  pure library TU (matches the runner's separate `main.c` and the smoke test's
  wrapper). NOT appended to `generic.c`.
- **D4 — entry resolution = `entry_c_symbol(model, "regseq_main")`** in the driver
  (it holds the model). Default suffix is the `regseq_main` convention (the same
  default the coverage runner's `manifest.entry` uses; runner resolves via the root
  API at `pipeline.rs:235`). `Ambiguous`/`NotFound` → a clear fatal driver diagnostic
  (the precise symbols listed), never a silently-wrong binary. A `bin` entry override
  is deferred (not v1).
- **D5 — default binary name = fixed `generic`.** cargo-regseq's `cargo metadata`
  only resolves regseq-**aware** crates (`invoke.rs:95-108`), NOT the selected primary
  package, and it removes `CARGO_PRIMARY_PACKAGE` (`invoke.rs:26`) without interpreting
  `-p`/workspace selection — so a reliable crate name is not available here. v1
  defaults to **`regseq-out/generic`**; `--emit bin=NAME` overrides. (Crate-name
  defaulting can come later if package selection is implemented.)
- **D6 — link errors / missing `cc` are clean failures.** cargo-regseq has **no
  production `cc` discovery today** (only the test has one). v1 adds a small
  `$CC`-then-`cc`-on-PATH probe to cargo-regseq's toolchain detection; absence → a
  clear error pointing at the requirement. A non-zero `cc` exit surfaces its stderr.
  Out-of-surface constructs already fail loud upstream via `EmitError::Unsupported`
  (fatal driver diagnostic) before the link is reached.
- **D7 — `--emit bin` always emits the default `regseq-out/generic.c` +
  `generic_main.c`** (the link inputs at known paths). It does NOT honor an
  `--emit generic-c=PATH` in the same run (that writes elsewhere); `bin` is
  self-contained so the driver→linker path handoff is fixed and unambiguous.
  Combining `bin` with a custom emit path is a deferred nicety.

## Edit points (anticipated; updated per Codex review + D1=`EmitSpec::Bin`)

1. `crates/regseq-config/src/emit.rs` — add `EmitSpec::Bin { name: Option<String> }`
   to the enum (serde tag `bin`); `FromStr` arm for `"bin"` / `"bin=NAME"` (NAME is a
   binary name, not a path — no `PathBuf`); `Display` arms; update the `UnknownType`
   message. Add `tests/emit.rs` parse/display/round-trip coverage.
2. `crates/regseq-config/src/lib.rs` / `options.rs` — add `pub const CC_FLAGS:
   &[&str]` (the link-flag home; cargo-regseq already deps `regseq-config`).
3. `crates/regseq-backends` — move `generate_main_c` in, **re-export at the crate
   root** (`lib.rs`, next to `entry_c_symbol`); the runner reads it from there.
4. `crates/codegen-coverage-runner/src/toolchain.rs` — call `regseq_backends`'
   `generate_main_c` and `regseq_config::CC_FLAGS` instead of local defs; update
   `tests/toolchain.rs:85` to the new path. Re-run the runner → verdict `pass`, ledger
   byte-stable.
5. `crates/regseq-driver/src/emit.rs` — `emit_one`'s **new `EmitSpec::Bin { name }`
   arm**: write `output_dir/generic.c` (reuse the GenericC emit), then
   `entry_c_symbol(model, "regseq_main")` → `generate_main_c(sym)` →
   `output_dir/generic_main.c`; `NotFound`/`Ambiguous` → an error that the
   `after_analysis` call site (`callbacks.rs:220`) already turns into
   `tcx.dcx().fatal(...)`. (Driver already deps `regseq-backends`. The `name` is not
   used by the driver — it is the linker's output name, consumed in step 7.) Also add
   `EmitSpec::Bin { .. } => None` to `collect_unsupported_warnings`.
6. `crates/cargo-regseq/src/cli.rs` — `--emit bin[=NAME]` already flows through the
   existing `Vec<EmitSpec>` parsing (no new arg); update the `--emit` help block to
   list `bin[=NAME]`. `crates/cargo-regseq/src/toolchain.rs` — add a production
   `$CC`-then-PATH `cc` probe (today only the e2e test has one).
7. `crates/cargo-regseq/src/invoke.rs` / `lib.rs` — after a successful `cargo check`,
   **scan `options.emit` for `EmitSpec::Bin { name }`**; if present, link
   `output_dir/generic.c` + `output_dir/generic_main.c` with `regseq_config::CC_FLAGS`
   → `output_dir/<name|"generic">`; surface `cc` failures. (cargo-regseq stays off
   `regseq-backends` — flags come from `regseq-config`.)
8. Docs: the cargo-regseq `--emit` help, `crates/cargo-regseq/README.md` (emit list),
   and `docs/discussions/generic-c-backend/running.md` (the manual-wrapper-`main`
   section becomes "or just `--emit bin`").
9. Tests: config parse (`bin`, `bin=NAME`) + serde round-trip; driver `emit_one(Bin)`
   shim emission (unit); an end-to-end `cargo-regseq` test that runs `--emit bin` on
   the playground crate and **executes the produced binary**, asserting
   `[INFO] smoke=42` (mirrors `generic_c_emit.rs`, host-`cc`-gated).

## Test plan

1. Build + full single-threaded suite green.
2. Config: `bin` / `bin=myprog` parse into `EmitSpec::Bin`; `Display`↔`FromStr`
   round-trip; serde tag `bin`.
3. Driver unit: `emit_one(EmitSpec::Bin)` writes `generic.c` + `generic_main.c` with
   the resolved crate-qualified entry; `Ambiguous`/`NotFound` → fatal.
4. Runner: re-run `cargo run -p codegen-coverage-runner` — verdict `pass`, ledger
   byte-stable (the shared-toolchain refactor is behaviour-preserving).
5. **End-to-end:** `cargo regseq --emit bin` on the playground → `regseq-out/generic`
   is a runnable executable; run it → `[INFO] smoke=42` (host-`cc`-gated, like
   `generic_c_emit.rs`).

## Resolved after review

- **D1 surface** — `EmitSpec::Bin` real variant (`--emit bin`), user-chosen over a
  separate `--bin` flag; the contained wart is documented under D1 + Architecture.
- **D5 default name** — fixed `generic` (crate-name resolution is not reliable in
  cargo-regseq).
- **`generate_main_c`/`CC_FLAGS` consumers** — only the runner (prod) + its
  `tests/toolchain.rs` (Codex-confirmed workspace grep). The move updates both.
- **`bin` × `generic-c=PATH`** — `bin` is self-contained, fixed default paths (D7).
- **Seam** — `emit_one(Bin)` writes the source half (errors → existing fatal path);
  cargo-regseq scans the emit vec for `Bin` and links — the source/link split Codex
  endorsed, carried by the emit vec rather than a `build_bin` field.
- **Shared-helper homes** — `generate_main_c` → `regseq-backends` (root re-export);
  `CC_FLAGS` → `regseq-config` (so cargo-regseq links without a new backend dep).
