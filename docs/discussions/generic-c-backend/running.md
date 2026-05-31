# Running the generic-C backend

How to take a Rust crate written against the supported surface and turn it
into a host-compilable C translation unit with the generic-C backend.

There are **two** ways to exercise the backend:

1. **The CLI** — `cargo regseq --emit generic-c` (the productized path; what a
   user runs). Covered first.
2. **The codegen-coverage runner** — the test harness that bridges a fixture,
   emits C, compiles it with a host `cc`, runs it, and asserts stdout. Covered
   second; this is the path with golden snapshots + the coverage ledger.

> **Scope.** The generic-C target is **hosted userspace** (libc, `stdio`,
> `main()`). "Runs" means the emitted C compiles and runs on a host toolchain —
> not real-OS integration (that is the per-target Linux/Zephyr backends). The
> canonical IR→C mapping contract is
> [`../regseq-model/generic-c-backend.md`](../regseq-model/generic-c-backend.md).

---

## Prerequisites

- **The pinned nightly toolchain.** The driver links `rustc_private`, so it must
  run under the exact channel in [`rust-toolchain.toml`](../../../rust-toolchain.toml)
  (`nightly-2026-02-01`, with `rustc-dev` + `rust-src`). `cargo-regseq` discovers
  it via `rustup which rustc`, so a `rustup` install honoring the pin is the
  simplest setup; any crate you compile should carry the same `rust-toolchain.toml`.
- **A host C compiler** (`cc` / `gcc` / `clang`) — only needed to *compile* the
  emitted `.c`. Emitting the C itself does not require one.
- **The two binaries built** from this workspace:

  ```sh
  cargo build -p cargo-regseq -p regseq-driver
  ```

  `cargo-regseq` is the cargo subcommand; `regseq-driver` is the `RUSTC_WRAPPER`
  that runs the bridge + emit inside the compiler.

---

## Path 1 — the CLI (`cargo regseq --emit generic-c`)

### How it wires together

```
cargo regseq --emit generic-c
  │  (cargo-regseq) parse CLI → CompilerOptions { emit: [GenericC] }
  │  serialize to JSON, set REGSEQ_OPTIONS; run `cargo check`
  │  with RUSTC_WRAPPER = regseq-driver
  ▼
regseq-driver  (Regseq mode, in `after_analysis`)
  │  run_bridge(tcx)  → RegseqModel       (rustc MIR → IR)
  │  model.validate()
  │  emit_one(GenericC) → regseq_backends::emit_generic_c(&model)
  ▼
  regseq-out/generic.c        (the C translation unit)
```

The model handed to the emitter is already bridged, validated, and
post-monomorphization — the backend just lowers it.

### Driver discovery

`cargo-regseq` locates the `regseq-driver` binary in this order
(`crates/cargo-regseq/src/toolchain.rs`):

1. `$REGSEQ_DRIVER` (an explicit path — handy when running from a build tree);
2. a sibling next to the `cargo-regseq` executable;
3. `which regseq-driver` on `$PATH`.

So either install both binaries on `$PATH`, or point `REGSEQ_DRIVER` at the
freshly built `target/debug/regseq-driver`.

### Required flags

`cargo regseq` requires `--target-os` and `--platform-file` even for the
generic-C target (they parameterize the broader pipeline). A minimal platform
file is enough:

```toml
# platform.toml
[platform]
name = "playground"
```

Emit targets (repeatable):

```
--emit generic-c            # → <output-dir>/generic.c
--emit generic-c=PATH       # → PATH (verbatim; relative paths resolve against CWD)
```

`--output-dir` defaults to `./regseq-out/`. Other emit specs (`model`, `json`,
`driver`, `analysis`) can be combined in one run.

### A worked example

A standalone crate within the supported surface, with an `extern "C"` entry that
logs a computed value:

```toml
# Cargo.toml
[package]
name = "regseq-gc-smoke"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]            # standalone: do not inherit a parent workspace
```

```rust
// src/lib.rs
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> { U32(u32), Str(&'a str) }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }
            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*])
            };
        }
        pub(crate) use info;
    }
}

#[inline(never)]
fn add(a: u32, b: u32) -> u32 { a + b }

#[no_mangle]
pub extern "C" fn regseq_main() {
    let r = add(20, 22);
    regseq::log::info!("smoke={}", r);
}
```

Also copy the workspace `rust-toolchain.toml` next to it so the driver's
`rustc_private` linkage matches.

Run it (from inside the crate dir), pointing at the built driver:

```sh
REGSEQ_DRIVER="/path/to/workspace/target/debug/regseq-driver" \
  /path/to/workspace/target/debug/cargo-regseq regseq \
    --target-os linux \
    --platform-file platform.toml \
    --emit generic-c
```

On success `cargo check` finishes and `regseq-out/generic.c` is written — a full
translation unit (prelude `#include <stdint.h>` … + the lowered functions).

### Compiling and running the emitted C

Two things to know before you `cc` it:

1. **The emitted TU is a library — it has no `main`.** Supply a wrapper `main`
   that calls the entry (this mirrors how the codegen-coverage runner links a
   generated `main.c`).
2. **The entry symbol is crate-qualified**, not the bare Rust name. The backend
   emits `sanitize(emission_name)` — a `DefPath`-derived symbol. For the crate
   above, `regseq_main` becomes **`regseq_gc_smoke_regseq_main`**. (Grep the
   emitted `.c` for the `…regseq_main(void)` definition to read off the exact
   symbol.)

```sh
cat > driver_main.c <<'EOF'
extern void regseq_gc_smoke_regseq_main(void);
int main(void) { regseq_gc_smoke_regseq_main(); return 0; }
EOF

cc -std=gnu11 -o gc_smoke regseq-out/generic.c driver_main.c
./gc_smoke
# → [INFO] smoke=42
```

That `[INFO] smoke=42` is `add(20, 22)` flowing through the diag-log intrinsic
(`log-info` → `fwrite` to stdout) — proof the whole chain computed and reported.

> A reproducible version of this exact walkthrough lives as the end-to-end test
> `crates/cargo-regseq/tests/generic_c_emit.rs`
> (`cargo_regseq_emit_generic_c_compiles_and_runs`): it builds the driver, writes
> this crate, runs `--emit generic-c`, then compiles + runs the result and
> asserts `[INFO] smoke=42`.

---

## Path 2 — the codegen-coverage runner

The runner is the backend's behavioral test harness. It owns the host-`cc`
toolchain, the golden-`.c` snapshots, and the coverage ledger.

```sh
# Run the whole corpus: bridge → emit → cc → run → assert stdout,
# then aggregate into the ledger and print the verdict.
cargo run -p codegen-coverage-runner
#   verdict: pass | not-pass     drift: <covered-without-green counts>

# Regenerate the golden .c snapshots after an intentional emit change:
cargo run -p codegen-coverage-runner -- --bless
```

- Fixtures: `crates/codegen-coverage-runner/fixtures/<name>/` — each has
  `manifest.toml` + `input.rs` (+ `expected.stdout`, `expected.stderr` for panic
  fixtures, `golden.c`).
- Ledger: `.harness/generic-c/coverage_ledger.{json,md}`.
- `--bless` regenerates `golden.c` only — never the behavioral oracles
  (`expected.stdout`/`.stderr`).

Use this path when you are adding/altering construct coverage; use Path 1 when
you want to emit C for a real crate.

---

## Supported-surface constraints (gotchas)

The backend covers the **Phase-1** surface: the supported Rust (non-domain)
constructs plus **diagnostic logging** (the only bridged domain construct). Keep
fixtures inside it:

- **`#![no_std]`** with the `register_tool(rseq)` attributes so the
  `#[rseq::intrinsic("log-…")]` markers parse and survive to the bridge.
- **Observable output comes only from the diag-log intrinsics** (`log-info`,
  `-debug`, `-warn`, `-error`) — they lower to `fwrite`/`printf` on stdout. There
  is no other I/O.
- **Prefer operators/constructs over core/std method calls.** A `kind=rust`
  fixture that calls `core`/`std` methods can hit undefined references at link;
  intrinsic-only needs go through `core::intrinsics::*` directly. (See the
  `codegen-fixture-no-core-methods` note.)
- **Out-of-surface constructs fail loud.** `emit_generic_c` returns
  `EmitError::Unsupported(...)` for anything outside the Phase-1 matrix, which the
  driver surfaces as a **fatal diagnostic** — not a silent miscompile. If a CLI
  run aborts with an "unsupported construct" message, the crate used something the
  backend does not yet lower.
- **Domain constructs beyond logging are not available yet.** MMIO / interrupts /
  atomics are not bridged (front-end stages 16/17/18), so a device-driver spec
  using them cannot be emitted yet — that is later-phase work
  (see [`roadmap.md`](roadmap.md)).

---

## References

- [`roadmap.md`](roadmap.md) — phase sequence, scope, the host-validatability boundary.
- [`../regseq-model/generic-c-backend.md`](../regseq-model/generic-c-backend.md) —
  the canonical IR→C mapping spec.
- [`phase-1/01-construct-surface-and-matrix.md`](phase-1/01-construct-surface-and-matrix.md) —
  the per-construct coverage matrix (what is in surface).
- `crates/cargo-regseq/tests/generic_c_emit.rs` — the runnable end-to-end CLI example.
- `crates/codegen-coverage-runner/` — the behavioral test harness + ledger.
- `.harness/generic-c/methodology.md` — the workstream process notes.
