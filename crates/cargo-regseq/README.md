# cargo-regseq

Cargo subcommand that orchestrates the Regseq compilation flow.

## Responsibilities

- Parse CLI arguments: `cargo regseq --platform-file <path> [OPTIONS]` (target identity —
  os/board/arch — comes from the platform file's `[platform]` table, not a `--target-os` flag)
- Select what to emit with `--emit <SPEC>`; when omitted, the default is
  `--emit model`, which writes the pretty-printed RegseqModel text (pass
  `--emit json` for the machine-readable form).
- Accept emit specs `model[=PATH]`, `json[=PATH]`, `generic-c[=PATH]`,
  `bin[=NAME]`, `driver[=MATCH_ID]`, and `analysis[.NAME][=PATH]`. `bin`
  emits the generic-C TU + an entry shim and links them with the host `cc`
  into a runnable `<output-dir>/<NAME|"generic">`.
- Detect the pinned nightly toolchain and locate the `regseq-driver` binary
- Set `RUSTC_WRAPPER=regseq-driver` and invoke `cargo check` with proper environment
- Separate Regseq-specific arguments from pass-through cargo arguments

## Binary

This crate produces the `cargo-regseq` binary. Cargo discovers it automatically
as a subcommand when it is on `$PATH`.

## Dependencies

- No rustc internals dependency
- Uses `regseq-config` for shared configuration types
