# Task: track-a-pl011-01 - Spec crate scaffold and compiler path

## Description

Create the first checked-in external-style Cargo project for the PL011 frontend
 evidence stream under `tests/fixtures/pl011-spec-slice/`. This task establishes
 the project layout, path dependencies on the user-facing Regseq crates, the
 minimal Layer 2 platform fixture, and the canonical compile path through the
 Regseq compiler wrapper.

This task is about scaffold and compile-route proof, not the final PL011 slice
 contents.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/discussions/pl011-spec-slice/pl011-spec-slice-discussion.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md
- tests/e2e/tests/driver_e2e.rs
- tests/fixtures/minimal-driver/Cargo.toml
- tests/fixtures/minimal-driver/platform.toml

## Output Specification

Create:

- tests/fixtures/pl011-spec-slice/.gitignore
- tests/fixtures/pl011-spec-slice/Cargo.toml
- tests/fixtures/pl011-spec-slice/platform.toml
- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/e2e/tests/pl011_spec_slice.rs

Modify if needed:

- tests/e2e/Cargo.toml
- tests/e2e/src/lib.rs

## Implementation Notes

This fixture must be modeled as an **external-style** driver-spec project,
 not as a workspace implementation crate.

Required manifest rules:

- `Cargo.toml` must use local path dependencies on:
  - `regseq`
  - `regseq-macros`
- the fixture manifest must include an empty `[workspace]` table so Cargo does
  not try to fold it into the repo workspace
- keep `publish = false`
- use `edition = "2021"`

Important compile-path rule for this task:

- the canonical compile route is through `regseq-driver` as `RUSTC_WRAPPER`
- plain `cargo check` is not the proof target for this task because the
  `#[rseq::...]` tool-attribute namespace is registered by the Regseq compiler
  path, not by Cargo alone

The initial fixture source may be minimal, but it must be a real Regseq-authored
 spec crate. At minimum it should include:

- `#[rseq::driver(...)]`
- `#[rseq::device_class(serial)]`
- a public driver type for the later PL011 slice work

The initial `platform.toml` must stay inside the current B-07 boundary:

- required `version`
- required `[platform]` with:
  - `os`
  - `board`
  - `arch`
- `[properties]` containing the initial PL011 slice property vocabulary:
  - `reg-io-width`
  - `current-speed`

Use these canonical fixture values unless there is a strong documented reason
to choose different ones:

- `reg-io-width = 4`
- `current-speed = 115200`

The e2e test should prove:

- the fixture crate can be built through the real Regseq compiler path
- `regseq-driver` reaches `after_analysis`
- the minimal driver identity is visible in the compile output

The e2e test may follow the same overall pattern as `tests/e2e/tests/driver_e2e.rs`,
 but it should target `tests/fixtures/pl011-spec-slice` and use the fixture's
 checked-in `platform.toml`.

Keep this task narrow:

- do not add the full register schema yet
- do not add the full polling serial slice yet
- do not add Layer 2 sections beyond the B-07 subset

## Test Plan

Tests/commands to support:

1. E2E compile-path smoke for `tests/fixtures/pl011-spec-slice` through
   `regseq-driver` as `RUSTC_WRAPPER`
2. Verify the fixture uses its own isolated Cargo workspace via the empty
   `[workspace]` table
3. Verify the checked-in `platform.toml` parses under the current B-07 parser
   contract

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_spec_slice -- --nocapture`
- `cargo test -p regseq-model --test platform_parser --test platform_properties`
- `cargo test --workspace`

## Acceptance Criteria

- `tests/fixtures/pl011-spec-slice/` exists as an external-style Cargo project
- the fixture uses local path dependencies on the user-facing Regseq crates
- the fixture manifest includes an empty `[workspace]` table
- the checked-in `platform.toml` stays within the current B-07 subset
- the fixture compiles through the Regseq compiler path
- the e2e compile-path smoke passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
