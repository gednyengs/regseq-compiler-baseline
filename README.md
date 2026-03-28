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

Current active workspace members:

```
crates/
  cargo-regseq/   Cargo subcommand: CLI, toolchain detection, orchestration
  regseq-config/  Shared configuration types (no rustc dependency)
  regseq-driver/  rustc wrapper: HIR/MIR extraction, pipeline orchestration
tests/
  e2e/            End-to-end test workspace member
```

Planned implementation crates for the ASPLOS track:

```
crates/
  regseq/             Public DSL facade for driver authors
  regseq-macros/      Procedural macros and register-surface macros
  regseq-model/       Rustc-independent declaration and binding models
  regseq-analysis/    Static analysis passes over internal models
  regseq-backends/    C code generation backends (Linux, Zephyr)
  regseq-diagnostics/ Shared diagnostics engine
```

## Toolchain

Requires Rust nightly-2026-02-01 with `rustc-dev`, `llvm-tools-preview`,
and `rust-src` components. Pinned in `rust-toolchain.toml`.

## Development workflow

Work is managed through a task harness under `.harness/`.

```
.harness/
  config.md               Project conventions and coding rules
  SESSION.md              Step-by-step worker bootstrap
  task-spec-workflow.md   Fallback workflow for authoring harness task specs
  task-spec-status.yaml   Task-spec authoring progress tracker
  tasks/                  Task spec markdown files organized by crate
```

The harness CLI is `uv run python scripts/python/task.py`:

| Command                          | Purpose                              |
|----------------------------------|--------------------------------------|
| `task.py doctor`                  | Verify prerequisites                 |
| `task.py next`                    | Find the next executable harness task |
| `task.py next-step`               | Determine the next executable or task-spec action |
| `task.py list`                    | Show all executable tasks and their status |
| `task.py start <id>`              | Claim a task                         |
| `task.py show <id>`               | Display task spec and input context  |
| `task.py done <id>`               | Complete task (runs verification)    |
| `task.py fail <id> --reason ".."` | Mark task as failed                  |

Verification on `done` runs `cargo test --workspace`, `cargo fmt --check`,
and `cargo clippy --workspace -- -D warnings`.

The executable-task procedure is:

1. Read the task spec
2. Define public API stubs with `todo!()` bodies
3. Write tests against the stubs
4. Implement
5. `cargo fmt` then `uv run python scripts/python/task.py done <id>`
6. Commit as `<task-id>: brief description`

## Cold-start workflow

If you are a cold LLM session starting in this repository, do not guess the
workflow from the codebase layout alone. Use this order:

1. Read `.harness/config.md`.
2. Read `.harness/SESSION.md`.
3. Run `uv run python scripts/python/task.py doctor`.
4. Run `uv run python scripts/python/task.py next-step`.
5. If the next action is an executable harness task, do that task and follow `.harness/SESSION.md`.
6. If the next action is `TASK_SPEC_BATCH`, read `.harness/task-spec-workflow.md`, then read the brief path printed by `next-step`.
7. If the next action is `BRIEF_AUTHORING_BATCH`, read `.harness/task-spec-workflow.md`, then create the missing brief using `.harness/spec-batches/BRIEF_TEMPLATE.md`.
8. Read the guide documents under `docs/guide/` and the implementation plan at `docs/plans/asplos-implementation-plan.md` only as directed by the task spec, batch brief, or workflow files.

Priority rule:

- executable harness tasks take precedence over task-spec authoring work
- task-spec authoring is only the fallback mode when no executable tasks exist
- do not use `.harness/task-spec-status.yaml` to track implementation execution progress

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

Derived execution planning documents live in `docs/plans/`:

- `asplos-implementation-plan.md` -- operational phase breakdown used to create harness tasks from the frozen guide documents

## Quick start

```sh
# Install Python dependencies for the harness
uv sync

# Install the pinned toolchain
rustup show

# Run tests
cargo test --workspace

# Determine the next action for a cold session
uv run python scripts/python/task.py next-step

# Run the codegen experiment
./scripts/run-codegen-experiment.sh
```
