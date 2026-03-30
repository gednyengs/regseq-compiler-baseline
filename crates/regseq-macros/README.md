# regseq-macros

Procedural macros used in Regseq driver specifications.

## Responsibilities

- `register_bitfields!` macro: defines register layouts with typed bitfield access
- Any future helper proc macros needed by driver specs
- Does not own `#[rseq::...]` attributes; those are inert custom tool
  attributes registered by the compiler wrapper

## Crate Type

This is a proc-macro crate (`proc-macro = true` in Cargo.toml). Rust requires
proc macros to be in a separate crate from regular items, which is why this is
separate from the `regseq` facade crate.

## Dependencies

- Standard proc-macro dependencies (`proc-macro2`, `quote`, `syn`)
- No rustc internals dependency
