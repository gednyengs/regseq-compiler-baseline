# regseq-compiler-asplos

Regseq is a Rust-hosted compiler toolchain that turns portable device
specifications into OS-specific C device drivers.

A driver author writes one spec in Rust using Regseq annotations and
primitives. The compiler extracts, lowers, and emits target-specific C for
Linux, Zephyr, or other OS backends.

## How it works

Regseq operates on a three-layer model:

- **Layer 1 -- Driver Specification.** Portable device semantics written in
  Rust: register layouts, interrupt intent, DMA structure, synchronization,
  parameters, and lifecycle behavior. Uses `#[rseq::*]` attributes, resource
  types, traits, and compiler-recognized intrinsics. Captures *what* the
  device needs, not *how* any particular OS provides it.

- **Layer 2 -- Platform Binding.** Board- and SoC-specific integration data:
  MMIO base addresses, IRQ routing, clock/reset hookups, BAR assignments.
  Analogous to devicetree or ACPI configuration. Supplied separately from the
  Rust spec.

- **Layer 3 -- Compiler Backend.** OS-specific API mappings, generated
  lifecycle boilerplate, and mechanism selection. Built into the compiler. The
  driver author never touches this layer.

Layers 1 and 2 are inputs. Layer 3 is the compiler itself.

## Current scope

This implementation targets two artifact devices for an ASPLOS evaluation:

- **PL011** -- ARM UART. Narrower device, validates the end-to-end compiler
  path first.
- **E1000** -- Intel NIC. Richer device, stress-tests DMA, networking,
  interrupt complexity, and performance-sensitive lowering.

Target OS backends are Linux (primary) and Zephyr (secondary).

## Workspace layout

```
crates/
  cargo-regseq/       Cargo subcommand: CLI, toolchain detection, orchestration
  regseq-config/      Shared configuration types (no rustc dependency)
  regseq-driver/      rustc wrapper: HIR/MIR extraction, pipeline orchestration
  regseq-backends/    C code generation backends (stub)
  regseq-analysis/    Static analysis passes (stub)
  regseq/             Public DSL facade for driver authors (stub)
  regseq-macros/      Procedural macros (stub)
  regseq-diagnostics/ Diagnostics engine (stub)
```

`cargo-regseq`, `regseq-config`, and `regseq-driver` have working
implementations. The remaining crates are scaffolded stubs awaiting
implementation.

## Toolchain

Requires Rust nightly-2026-02-01 with `rustc-dev`, `llvm-tools-preview`,
and `rust-src` components. Pinned in `rust-toolchain.toml`.

## Development workflow

Work is managed through a task harness under `.harness/`.

```
.harness/
  config.md       Project conventions and coding rules
  SESSION.md      Step-by-step worker procedure
  tasks/          Task spec markdown files organized by crate
```

The harness CLI is `scripts/python/task.py`:

| Command                          | Purpose                              |
|----------------------------------|--------------------------------------|
| `task.py doctor`                 | Verify prerequisites                 |
| `task.py list`                   | Show all tasks and their status      |
| `task.py next`                   | Find the next unclaimed task         |
| `task.py start <id>`             | Claim a task                         |
| `task.py show <id>`              | Display task spec and input context  |
| `task.py done <id>`              | Complete task (runs verification)    |
| `task.py fail <id> --reason ".."` | Mark task as failed                 |

Verification on `done` runs `cargo test --workspace`, `cargo fmt --check`,
and `cargo clippy --workspace -- -D warnings`.

The worker procedure for each task:

1. Read the task spec
2. Define public API stubs with `todo!()` bodies
3. Write tests against the stubs
4. Implement
5. `cargo fmt` then `task.py done <id>`
6. Commit as `<task-id>: brief description`

## Guide documents

Detailed planning and handoff documents live in `docs/guide/`:

- `implementation-guide.md` -- phased implementation plan and design rules
- `construct-catalog.md` -- validated construct inventory (19 active constructs)
- `lowering-contract.md` -- per-construct lowering rules for each OS backend
- `device-coverage.md` -- PL011/E1000 construct coverage mapping
- `artifact-manifest.md` -- frozen research artifacts accepted for this scope
- `pl011-spec-outline.md` -- PL011 specification outline
- `e1000-spec-outline.md` -- E1000 specification outline
- `open-issues.md` -- known open design questions

## Quick start

```sh
# Install the pinned toolchain
rustup show

# Run tests
cargo test --workspace

# Run the codegen experiment
./scripts/run-codegen-experiment.sh
```
