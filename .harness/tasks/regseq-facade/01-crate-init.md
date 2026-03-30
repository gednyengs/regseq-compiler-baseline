# Task: regseq-facade-01 - Crate initialization

## Description

Activate the `regseq` crate (the public facade) as a live workspace member.
This is the crate that driver authors depend on to write Regseq device
specifications. It re-exports procedural macros from `regseq-macros` and
provides the DSL types, resource wrappers, marker traits, and intrinsic
stubs that form the Layer 1 authoring surface. The `#[rseq::...]` attribute
namespace is compiler-recognized and separate from the `regseq` crate
namespace.

At activation time, the crate needs a minimal Cargo.toml, a lib.rs with
crate-level documentation, and a smoke test. No DSL types or re-exports
are added in this task.

## Input Context

Read before starting:
- crates/regseq/README.md
- Cargo.toml (workspace root)
- crates/regseq-macros/Cargo.toml (must already exist from regseq-macros-01)
- crates/regseq-driver/src/callbacks.rs

## Output Specification

Create:
- crates/regseq/Cargo.toml
- crates/regseq/src/lib.rs
- crates/regseq/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-macros = { path = "../regseq-macros" }
```

The `regseq-macros` dependency is the proc-macro crate whose helper-macro
exports this facade may re-export. Additional dependencies (e.g., for DSL
resource types) will be added by later tasks.

Do not model `#[rseq::...]` attributes as re-exported proc macros from this
crate. They are inert custom tool attributes registered by the compiler
wrapper under the distinct `rseq` namespace.

### src/lib.rs

Add a crate-level doc comment describing the crate as the public DSL facade
for Regseq driver authors. Driver specifications depend on this crate to
access helper macros such as `register_bitfields!`, resource types, marker
traits, and intrinsic stubs.

Document that `#[rseq::...]` attributes are compiler-recognized tool
attributes, not proc macros exported by the `regseq` facade.

No types, traits, or re-exports are added at activation time. Subsequent
tasks add the actual DSL surface.

## Test Plan

Create:
- crates/regseq/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq`

## Acceptance Criteria

- Crate compiles
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
