# Task: regseq-macros-03 - Ordered register-ordering macro

## Description

Implement `regseq::ordered!` as the deferred user-facing ordering surface for
Register Access. This task owns only the macro surface and its re-export
plumbing. The macro is syntax sugar, not a canonical intrinsic.

## Input Context

Read before starting:

- crates/regseq-macros/src/lib.rs
- crates/regseq/README.md
- docs/compiler-collateral/compiler-constraints.md
- docs/compiler-collateral/constructs/register-access.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq-macros/src/ordered.rs
- crates/regseq-macros/tests/ordered.rs

Modify:

- crates/regseq-macros/src/lib.rs
- crates/regseq/src/lib.rs

## Implementation Notes

This task should establish the user-facing `ordered!` macro surface while
keeping canonical lowering identity on function-like intrinsic calls.

Accepted baseline:

- `ordered!` lives in `regseq-macros`
- `regseq` re-exports it for authored convenience
- `ordered!` is syntax sugar, not a canonical intrinsic
- the macro should expand into ordinary Rust block structure plus predictable
  canonical intrinsic calls

For this baseline, support the block form:

```rust
regseq::ordered! {
    self.regs.cr.write(CR_REG::UARTEN::SET);
    self.regs.lcr_h.modify(LCRH_REG::FEN::SET);
}
```

The expansion contract should be explicit and predictable. At minimum, the
expanded form should bracket the user block with canonical ordering intrinsic
calls such as:

```rust
{
    ::regseq::intrinsics::sync::memory_barrier(
        ::regseq::intrinsics::sync::BarrierKind::Full,
    );
    let __regseq_ordered_result = { /* user statements / final expr */ };
    ::regseq::intrinsics::sync::memory_barrier(
        ::regseq::intrinsics::sync::BarrierKind::Full,
    );
    __regseq_ordered_result
}
```

Phase-B constraints:

- keep the macro focused on ordered register-operation ergonomics
- keep the expansion predictable for later driver extraction
- do not add lowering behavior or target barrier policy
- do not add ownership of inert `#[rseq::...]` attributes

## Test Plan

Create:

- crates/regseq-macros/tests/ordered.rs

Tests to implement:

1. Verify `ordered!` is exported from `regseq-macros`.
2. Verify `regseq` re-exports `ordered!` for authored convenience.
3. Verify a representative ordered block around register operations compiles.
4. Verify the macro preserves the final expression value of the user block.

Test command: `cargo test -p regseq-macros --test ordered`

## Acceptance Criteria

- `ordered!` exists as an explicit helper macro surface
- the macro remains syntax sugar over canonical intrinsic calls
- `regseq` re-exports the macro
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
