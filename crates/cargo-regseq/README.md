# cargo-regseq

Cargo subcommand that orchestrates the Regseq compilation flow.

## Responsibilities

- Parse CLI arguments: `cargo regseq --target-os <linux|zephyr> --platform-file <path> [OPTIONS]`
- Detect the pinned nightly toolchain and locate the `regseq-driver` binary
- Set `RUSTC_WRAPPER=regseq-driver` and invoke `cargo check` with proper environment
- Separate Regseq-specific arguments from pass-through cargo arguments

## Binary

This crate produces the `cargo-regseq` binary. Cargo discovers it automatically
as a subcommand when it is on `$PATH`.

## Dependencies

- No rustc internals dependency
- Uses `regseq-config` for shared configuration types
