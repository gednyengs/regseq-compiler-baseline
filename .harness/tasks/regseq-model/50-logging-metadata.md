# regseq-model-50: Rule 6 — Logging Metadata Extraction

## Description

For logging intrinsic calls (`LogError`, `LogWarn`, `LogInfo`,
`LogDebug`), extract the template string from the rustc MIR
constant argument, parse the restricted Regseq format subset
into `Vec<FormatPlaceholder>`, strip the Rust-level `LogArg`
carrier wrappers from argument values, and store the structured
result in `IntrinsicMetadata::Log { template, placeholders }`.
Argument values become plain `Operand`s in `args`.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 IntrinsicMetadata::Log; FormatPlaceholder +
  FormatKind enum; Section 8 recognition rule 6)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.2
  intrinsic catalog with logging variants)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (logging intrinsic methods, restricted format subset)
- `crates/regseq-model/src/ir/intrinsic.rs` (IntrinsicMetadata,
  FormatPlaceholder, FormatKind)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-49)
- `crates/regseq/src/intrinsics/diagnostic_logging.rs` (facade
  surface defining `LogArg` carrier and the log intrinsics)
- `tests/fixtures/pl011-full-spec/src/driver.rs`

## Implementation Notes

### Recognition

When task 45 produces an `IntrinsicCall` whose `intrinsic` is
one of `LogError`, `LogWarn`, `LogInfo`, `LogDebug`:

1. Identify the template-string argument (a
   `mir::Operand::Constant` whose value is a `&str` literal).
   Evaluate via `tcx.const_eval_poly` (or inspect the const
   directly) to get the UTF-8 bytes.
2. Parse the template into `Vec<FormatPlaceholder>` covering
   every `{}` (with optional format modifiers) in positional
   order. Supported `FormatKind` variants per ir-reference.md
   Section 5:
   - `Display` (`{}`)
   - `LowerHex` (`{:x}`)
   - `UpperHex` (`{:X}`)
   - `LowerHexPrefixed` (`{:#x}`)
   - `UpperHexPrefixed` (`{:#X}`)
   - `PaddedHex { prefix, uppercase, width }` (e.g., `{:08x}`,
     `{:#010x}`)
   - `Pointer` (`{:p}`)
   Reject any other format specifier with
   `ConstructionError::UnsupportedLogFormat` citing the
   offending placeholder.
3. Strip the `LogArg` carrier wrappers from each argument
   operand. The facade's logging API wraps each argument in a
   `LogArg::<T>(value)` newtype-like carrier; recognition
   unwraps by identifying the `LogArg` constructor call (by
   live `DefId`) in the operand's definition chain and using
   the inner operand directly. The template-string arg is
   removed from `args` (it is carried in metadata).
4. Validate `placeholders.len() == args.len()` (after stripping
   the template). Mismatch →
   `ConstructionError::LogPlaceholderArityMismatch`.
5. Set `metadata = Some(IntrinsicMetadata::Log { template,
   placeholders })`.

### Fixture preparation

PL011 currently has no logging call. Task 54 (PL011 end-to-end)
validates the logging tier through the PL011 pipeline, so this
task **must** add at least one logging call to the PL011 fixture
(`tests/fixtures/pl011-full-spec/src/driver.rs`):

- Add a single `info!`-style call inside an existing helper
  that is reachable from a callback (e.g., a one-line log of
  the baud rate inside `program_polling_port`). Keep it minimal
  -- one call, one `Display` placeholder, one argument.

For broader `FormatKind` coverage, add a small dedicated test
fixture crate under `tests/fixtures/log-intrinsic-smoke/`
exercising one logging call per `FormatKind` variant. Use this
fixture for the unit/integration tests below; the PL011 fixture
covers the `Display` case for the end-to-end test.

### Argument extraction edge cases

- An argument of type `Option<T>` or `Result<T, E>` carried
  through `LogArg` retains its `Option`/`Result` wrapper --
  carrier stripping removes only the outer `LogArg` newtype.
- An argument whose source is a `Place` (not a `Constant` or
  `Literal`) becomes `Operand::Copy(Place)` or
  `Operand::Move(Place)` in the resulting args.

## Non-Goals

- No backend lowering of templates (Phase 8).
- No support for named placeholders (`{name}`) or implicit
  argument capture.
- No support for `FormatKind` variants beyond those documented.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Using the `log-intrinsic-smoke` fixture, translate logging
  calls covering all `FormatKind` variants including
  `PaddedHex { prefix: true, uppercase: false, width: 10 }`
  (i.e., `{:#010x}`). For each call, assert:
  - `IntrinsicMetadata::Log { template, placeholders }` is
    populated.
  - `template` is the exact source string.
  - `placeholders.len()` equals `args.len()`.
  - Each `placeholders[i].format` matches the source format
    specifier.
  - `args[i]` is a plain Operand of the inner type (no
    `LogArg` wrapper visible).
- Using the updated PL011 fixture, translate the `info!` call
  and assert the produced `IntrinsicMetadata::Log` is correct.
- Negative test: a logging call with `{:?}` (Debug format) →
  `ConstructionError::UnsupportedLogFormat`.
- Negative test: arity mismatch (template `{} {}` with one
  operand) → `ConstructionError::LogPlaceholderArityMismatch`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify all
`FormatKind` variants are parseable, the restricted-subset
rejection covers `{:?}` and other unsupported specs, and the
`LogArg` stripping leaves clean operand types. Iterate until
approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- Every `FormatKind` variant exercised by a passing test via
  the `log-intrinsic-smoke` fixture.
- PL011 fixture carries one `info!` call; its
  `IntrinsicMetadata::Log` matches expectations.
- Template extraction + placeholder parsing produce correct
  metadata.
- `LogArg` carrier stripping leaves `args` containing only
  inner operands.
- Negative tests for unsupported format and arity mismatch
  pass.
- Codex review approved.
