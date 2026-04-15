# regseq-facade-23: Mmio<T> register access methods with intrinsic annotations

## Description

Add register access methods to `Mmio<T>` with `#[rseq::intrinsic("...")]`
annotations. Strip ReadOnly/WriteOnly/ReadWrite of their access methods,
making them layout-only types.

This is the register access surface restructure -- the authored driver
surface changes from `self.regs.cr.write(...)` to
`self.regs.write_field(cr::UARTEN::ENABLED + ...)`.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.3)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.2)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Section 2, Mmio)
- `crates/regseq/src/mmio.rs`
- `crates/regseq/src/registers.rs` (post regseq-facade-22)

## Implementation Notes

### Add to Mmio<T> (mmio.rs)

```rust
impl<Block> Mmio<Block> {
    /// Read the full raw register value.
    #[rseq::intrinsic("mmio_read")]
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(
        &self, _reg: RegisterId<W, R>,
    ) -> W
    where R::Access: ReadAccess {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Read a specific field (masked and shifted).
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, _field: Field<W, R>,
    ) -> W
    where R::Access: ReadAccess {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Write a full register value.
    #[rseq::intrinsic("mmio_write")]
    pub fn write<W: RegWidth, R: RegisterOf<Block>>(
        &self, _reg: RegisterId<W, R>, _value: W,
    )
    where R::Access: WriteAccess {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Write specific fields (masked).
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, _values: FieldValue<W, R>,
    )
    where R::Access: WriteAccess {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Read-modify-write. NOT an intrinsic.
    /// Takes both a RegisterId (to identify which register to read) and
    /// a FieldValue (mask + value to apply). The authored surface becomes:
    ///   self.regs.modify(lcr_h::REG, lcr_h::FEN::DISABLED);
    /// This deviates from spec.md's single-argument modify(); the
    /// alternative (FieldValue carrying RegisterId internally) can be
    /// explored later if a more ergonomic surface is desired.
    pub fn modify<W: RegWidth, R: RegisterOf<Block>>(
        &self, _reg: RegisterId<W, R>, _values: FieldValue<W, R>,
    )
    where R::Access: ReadAccess + WriteAccess {
        let _current: W = self.read(_reg);
        // real impl: self.write(_reg, (current & !_values.mask) | _values.value)
        todo!("facade stub")
    }
}
```

`RegWidth` is defined in regseq-facade-22. It is a marker trait that
`u8`, `u16`, `u32` implement.

### Strip ReadOnly/WriteOnly/ReadWrite methods (registers.rs)

Remove `read`, `read_raw`, `write`, `set`, `modify` methods from these
types. Keep:
- `#[repr(transparent)]` struct definition
- Any associated types or trait impls needed for layout

This is a breaking change. After this task, the PL011 fixture will NOT
compile until phase-3d updates it.

### Conventions

- `modify()` is NOT an intrinsic. Its body reads the register via
  `self.read(reg)`, applies the mask from `FieldValue`, and writes back
  via `self.write(reg, ...)`. As a facade stub the final write can use
  `todo!()`, but the read call should be present to show the pattern.
- All intrinsic methods use `todo!()` bodies -- they are lowered by the
  compiler backend, never called directly.

## Non-Goals

- No PL011 fixture update (phase-3d).
- No register_block! macro (regseq-macros-04).
- No extraction changes.

## Test Plan

- `cargo build -p regseq` compiles with new Mmio methods.
- The PL011 fixture WILL break after this task -- that is expected.
  Phase-3d handles the PL011 update. Verify with `cargo build -p regseq`
  only (not `--workspace`).
- Unit test: construct `Mmio<T>` and verify the method signatures exist
  (compile-time check via trait bounds).
- Verify `modify()` takes both a `RegisterId` and a `FieldValue`
  parameter (two arguments, not one).

## Review

After regseq crate compiles, use `codex:rescue` to review. Verify all
5 methods on Mmio<T>, intrinsic attributes on 4 of them, and that
ReadOnly/WriteOnly/ReadWrite no longer have access methods.

## Quality Gate

```
cargo test -p regseq
cargo fmt --all -- --check
cargo clippy -p regseq -- -D warnings
```

Note: `cargo test --workspace` may fail if PL011 fixture depends on
removed methods. Use `-p regseq` for this task.

## Acceptance Criteria

- 5 methods on `Mmio<T>` (4 intrinsics + modify).
- ReadOnly/WriteOnly/ReadWrite stripped of access methods.
- `cargo build -p regseq` compiles. Codex review approved.
