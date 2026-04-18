# regseq-model-51: Rule 7 — Register Metadata + Mmio::modify Body Validation

## Description

For register access intrinsics (`RegisterRead`,
`RegisterReadField`, `RegisterWrite`, `RegisterWriteField`) on
`Mmio<T>`, resolve the monomorphized rustc type parameters
(`Block`, `R`, `R::Access`) and field argument into
`IntrinsicMetadata::Register { register_block, register_offset,
access_width, field }`. Validates the extracted `Mmio::modify`
body (cross-crate function from task 47) contains exactly two
`IntrinsicCall` terminators (a `mmio_read` followed by a
`mmio_write`), and verifies a volatile dead-temp read produces a
`RegisterRead` retained in IR.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 10.2
  register intrinsic architecture, identity resolution)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 IntrinsicMetadata::Register, RegisterFieldMeta;
  Section 8 recognition rule 7)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (Section 2 Mmio + register intrinsics)
- `crates/regseq-model/src/ir/intrinsic.rs` (IntrinsicMetadata,
  RegisterFieldMeta)
- `crates/regseq-model/src/model/register.rs` (RegisterBlock,
  RegisterDescriptor)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-50; this task depends on 45 + 47 + 48)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (line 75
  `disable_fifo()`, 120 `configure_line_control()`, 346
  `flush_rx_fifo()`)
- `tests/fixtures/pl011-full-spec/src/registers.rs`
  (`register_block!` definitions)

## Implementation Notes

### RegisterIndex (Pass 1 extension owned by this task)

Build a typed `RegisterIndex` during Pass 1 that maps every
register marker type's rustc `DefId` to a small record:

```rust
struct RegisterEntry {
    block: RegisterBlockId,
    offset: u32,            // byte offset within the block
    access_width: u32,      // bits (8/16/32)
    access_kind: AccessKind, // ReadOnly / WriteOnly / ReadWrite
    fields: HashMap<DefId, RegisterFieldMeta>, // keyed by field constant DefId
}
```

Populate by walking each `RegisterBlock` item registered by
task 41: each block's ordered field list gives the byte offset
and access kind; the descriptor's bitfield definitions give
field mask/shift/name. The marker-type → entry mapping is the
single source of truth -- no separate OFFSET-constant lookup at
recognition time.

### Recognition

When task 45 produces an `IntrinsicCall` whose `intrinsic` is
one of `RegisterRead`, `RegisterReadField`, `RegisterWrite`,
`RegisterWriteField`:

1. Resolve **block identity** from the receiver type
   (`args[0]`'s `ty::Ty`). The receiver is `&Mmio<Block>`; look
   up `Block`'s `DefId` → `RegisterBlockId` in the register-
   block index from Pass 1.
2. Resolve **register identity** from the intrinsic's
   monomorphized `R` type parameter (available through the
   `Instance`'s `GenericArgs`). Look up `R`'s `DefId` in
   `RegisterIndex` → `RegisterEntry { block, offset,
   access_width, access_kind, fields }`. Confirm `entry.block
   == register_block` from step 1 (cross-block sanity check).
3. Resolve **access width** from `entry.access_width`.
4. For `RegisterReadField` / `RegisterWriteField` only, resolve
   **field identity** from the field argument operand:
   - The field arg is `mir::Operand::Constant(c)` referencing
     one of the `register_block!`-generated `Field<W, R>` or
     `FieldValue<W, R>` constants. Its `DefId` is the index
     key.
   - Look up `RegisterFieldMeta { mask, shift, name }` in
     `entry.fields`.
   - Store as `Some(RegisterFieldMeta { mask, shift, name })`.
   - For `RegisterRead` / `RegisterWrite`, `field: None`.
5. Set `metadata = Some(IntrinsicMetadata::Register {
   register_block, register_offset, access_width, field })`.

### Cross-block bound checking

If `entry.block` does not match the receiver's
`RegisterBlockId`, emit
`ConstructionError::CrossBlockFieldUsage` carrying both block
IDs -- defensive guard against extraction surprises.

### Access-kind verification

Check `entry.access_kind` against the intrinsic's required
access:
- `RegisterRead`/`RegisterReadField` require
  `ReadOnly | ReadWrite`.
- `RegisterWrite`/`RegisterWriteField` require
  `WriteOnly | ReadWrite`.

Mismatch is unreachable in well-formed input but produce
`ConstructionError::WrongRegisterAccessKind` defensively,
citing the register's `DefId` and the offending intrinsic.

### Mmio::modify body validation

After this task lands, the extracted `Mmio::<Block>::modify`
function (from task 47) must satisfy:
- `Body.basic_blocks` walked end-to-end contains exactly two
  `IntrinsicCall` terminators in order: one `RegisterRead`
  (with `metadata: Some(IntrinsicMetadata::Register { ... })`),
  then one `RegisterWrite` (with metadata).
- Both target the same `register_block` and `register_offset`.
- Both `error_target: None` (MMIO is infallible).

### Volatile dead-temp

Construct a fixture (or use `flush_rx_fifo()`'s
`let _ = self.regs.read(dr::REG)`) and verify:
- The MIR Assign whose destination is a throwaway local
  translates to an `IntrinsicCall { intrinsic: RegisterRead,
  .. }`.
- The `IntrinsicCall` is retained in `Body.basic_blocks` (NOT
  dropped as dead).
- `BodySourceMap` carries the user's span for this call
  (volatile reads have visible side effects via MMIO and must
  remain).

## Non-Goals

- No backend elision behaviour (Phase 8). This task only
  verifies the IR retains the read.
- No metadata for `RegisterPoll`/`RegisterPollField` (deferred
  -- closure support not yet in IR).
- No I2C/SPI register-intrinsic support.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Translate `disable_fifo()` and `configure_line_control()`
  from PL011 and assert:
  - Every register IntrinsicCall has
    `metadata: Some(IntrinsicMetadata::Register { ... })`.
  - `register_block` resolves to the PL011 register block ID.
  - `register_offset` matches the byte offset recorded on the
    `RegisterBlock`'s field in Pass 1.
  - `access_width` is 32.
  - For `WriteField` calls, `field` carries the correct
    `mask`/`shift`/`name`.
- Translate a PL011 helper exercising `RegisterReadField` and
  assert `field` carries the correct `mask`/`shift`/`name`.
- Walk the extracted `Mmio::<pl011::Regs>::modify` body; assert
  exactly 2 `IntrinsicCall` terminators (RegisterRead →
  RegisterWrite) targeting the same register, with matching
  `IntrinsicMetadata::Register`.
- Add or use `flush_rx_fifo()`'s
  `let _ = self.regs.read(dr::REG)` and assert the resulting
  `IntrinsicCall` is retained with `RegisterRead` metadata.
- Negative test: hand-build a `RegisterIndex` entry whose
  `block` differs from the receiver's `RegisterBlockId` →
  `ConstructionError::CrossBlockFieldUsage`.
- Negative test: hand-build a `RegisterIndex` entry whose
  `access_kind` is `ReadOnly` and feed a synthetic
  `RegisterWrite` IntrinsicCall →
  `ConstructionError::WrongRegisterAccessKind`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify:
- Type-parameter resolution uses live `Instance::args`, not
  string matching.
- The field mask/shift come from the descriptor (not
  re-derived).
- The extracted `Mmio::modify` body assertion holds.
- The volatile dead-temp `RegisterRead` is retained.
- Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- Every PL011 register access function carries correct
  `IntrinsicMetadata::Register`.
- Extracted `Mmio::modify` body contains exactly 2
  `IntrinsicCall` terminators (RegisterRead → RegisterWrite,
  same register).
- Volatile dead-temp `let _ = self.regs.read(dr::REG)`
  produces a retained `RegisterRead` in IR.
- Negative cross-block and access-kind tests pass.
- Codex review approved.
