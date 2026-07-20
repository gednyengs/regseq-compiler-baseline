# regseq-compiler-asplos

Regseq is a Rust-hosted compiler toolchain that turns portable device
specifications into OS-specific C device drivers.

A driver author writes one spec in Rust using Regseq annotations and
primitives. The compiler extracts, lowers, and emits target-specific C for
Linux, Zephyr, or other OS backends.

## How it works

Regseq operates on a three-layer model:

- **Layer 1 -- Driver Specification.** Portable device semantics written in
  Rust: register layouts, interrupt intent, synchronization,
  parameters, and lifecycle behavior. Uses `#[rseq::*]` attributes, resource
  types, traits, and compiler-recognized intrinsics. Captures *what* the
  device needs, not *how* any particular OS provides it. The `rseq`
  attribute namespace is a compiler-registered custom tool namespace, kept
  separate from the `regseq` crate namespace used for helper macros and
  facade types.

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
- **E1000** -- Intel NIC. Richer device, stress-tests networking,
  interrupt complexity, and performance-sensitive lowering.

Target OS backends are Linux (primary) and Zephyr (secondary).

## Workspace layout

Current workspace members:

```
crates/
  cargo-regseq/        Cargo subcommand: CLI, toolchain detection, orchestration
  regseq/              Public Layer 1 facade for driver authors
  regseq-analysis/     Analysis/bootstrap over normalized models
  regseq-backends/     Backend crate for Linux/Zephyr lowering work
  regseq-config/       Shared configuration and CLI types
  regseq-diagnostics/  Shared diagnostics support
  regseq-driver/       rustc wrapper: frontend extraction and RegseqModel emission
  regseq-macros/       Proc macros and register-surface helpers
  regseq-model/        Rustc-independent declaration/binding/joined IR
tests/
  e2e/                 End-to-end test workspace member
```

Important maturity note:

- `regseq`, `regseq-driver`, `regseq-model`, and `regseq-analysis` are active
  compiler pipeline crates.
- `regseq-backends` exists, but Phase D lowering/emission work is still the
  next major step; do not assume Linux/Zephyr backend logic is already
  implemented there.

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
8. Read [docs/handoff.md](docs/handoff.md) for the current authority map and
   repo state.
9. Read the implementation plan at `docs/plans/asplos-implementation-plan.md`
   and the frozen planning docs under `archive/guide/` only as directed by the
   task spec, batch brief, or workflow files.

Priority rule:

- executable harness tasks take precedence over task-spec authoring work
- task-spec authoring is only the fallback mode when no executable tasks exist
- do not use `.harness/task-spec-status.yaml` to track implementation execution progress

## Planning documents

The current live authority chain is:

- `docs/handoff.md`
- `.harness/`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/`
- targeted refinement notes under `docs/discussions/`

Frozen planning and research baseline documents live under `archive/guide/`,
including:

- `implementation-guide.md`
- `lowering-contract.md`
- `device-coverage.md`
- `artifact-manifest.md`
- `pl011-spec-outline.md`
- `e1000-spec-outline.md`
- `open-issues.md`

Current refinement/decision docs that matter in day-to-day work include:

- `docs/discussions/register-access/`
- `docs/discussions/regseq-model/`
- `docs/discussions/os-capability-disposition/`

Use `docs/handoff.md` to decide which of these are currently authoritative and
which are supporting or historical only.

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

# Read the current repo handoff before starting new work
sed -n '1,220p' docs/handoff.md
```
