# regseq-config

Shared configuration types passed between cargo-regseq, the driver, analyses,
and backends.

## Responsibilities

- Target OS enum (`Linux`, `Zephyr`, etc.)
- Compilation options and flags
- Platform file (.toml) parsing and representation
- Declaration-level model types extracted from HIR:
  - Register block metadata (field names, access types, bitfield layouts, annotations)
  - Driver struct metadata (resource handles, properties, user fields)
  - Device class binding information
  - Lifecycle trait presence and method signatures
- These declaration types are rustc-independent and can be unit-tested

## Dependencies

- `serde`, `toml` for platform file parsing
- No rustc internals dependency
