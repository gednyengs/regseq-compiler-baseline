# regseq-diagnostics

Common diagnostics engine and macros reused across the Regseq toolchain.

## Responsibilities

- Diagnostic severity levels (Error, Warning, Hint)
- Diagnostic ID system (SYN-001, SHAD-001, PROP-001, etc.)
- Structured diagnostic messages with source spans
- Diagnostic emission and collection
- Macros for ergonomic diagnostic creation

## Dependencies

- No rustc internals dependency (uses its own span representation that can be
  populated from rustc spans by the driver)
