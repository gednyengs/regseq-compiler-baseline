# regseq-driver-87: Wire the generic-C backend into the emit pipeline (`--emit generic-c`)

## Description

The generic-C backend (`regseq_backends::emit_generic_c(&RegseqModel) -> Result<String,
EmitError>`) is feature- and coverage-complete (Phase-1 codegen-coverage gate `pass`,
35/35), but it is **only reachable from the codegen-coverage-runner test harness** — there
is no command-line path to emit C from a real crate. The driver's emit dispatcher
(`emit_one`) services `Model` (pretty-print) and `Json`, and no-ops `Driver`/`Analysis`;
there is no `GenericC` emit target at all.

Add a new `EmitSpec::GenericC { path }` variant so a user can run
`cargo regseq --emit generic-c[=PATH]` and have the driver bridge → validate → emit the
generic-C translation unit to disk. This is a thin wiring task: the model handed to
`emit_one` is already bridged, validated, and post-monomorphization — exactly the
precondition `emit_generic_c` requires.

**Scope boundary.** This emits the `.c` translation unit only. It does NOT compile or run
it — the host-`cc` compile+run+assert loop lives solely in the codegen-coverage-runner
(`toolchain::compile_and_run`) and is out of scope. A productized "build to runnable
binary" command is separate, larger, and deliberately deferred.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-config/src/emit.rs:19-53` — the `EmitSpec` enum (`#[serde(tag = "type",
  rename_all = "kebab-case")]`); variants `Driver | Analysis | Model | Json`. Add
  `GenericC { path: Option<PathBuf> }`. The serde tag for `GenericC` is automatically
  `"generic-c"` (kebab-case), which lines up with the CLI string.
- `crates/regseq-config/src/emit.rs:55-91` — `FromStr`: add a `"generic-c"` arm →
  `Ok(Self::GenericC { path: value.map(PathBuf::from) })`. Reject a subtype
  (`generic-c.foo`) like `json` does (no subtypes). Only `generic-c` is accepted (no
  `c` alias).
- `crates/regseq-config/src/emit.rs:93-122` — `Display` (exhaustive `match`): add the
  None / `Some(path)` arms so `Display`↔`FromStr` round-trips.
- `crates/regseq-config/src/emit.rs:10-14` — `EmitParseError::UnknownType` message:
  add `'generic-c'` to the "expected …" list.
- `crates/regseq-driver/Cargo.toml:9-15` — `[dependencies]`: `regseq-backends` is NOT a
  dependency; add `regseq-backends = { path = "../regseq-backends" }`.
- `crates/regseq-driver/src/emit.rs:33-57` — `emit_one` dispatch (exhaustive `match`, no
  wildcard). Add the `GenericC` arm (see Implementation Notes). Default filename
  **`generic.c`** (NOT `model.c` — the artifact is a C translation unit, not the model).
- `crates/regseq-driver/src/emit.rs:67-77` — `collect_unsupported_warnings` (exhaustive
  `match`). Add `EmitSpec::GenericC { .. } => None` — it is *supported*, so no phase-1b
  warning.
- `crates/regseq-driver/src/emit.rs:79-82` — `resolve(path, output_dir, default_name)`:
  reused as-is. A user `path` (relative or absolute) wins verbatim; the default
  `output_dir/generic.c` is used only when `path` is `None`. (Same convention as
  `Model`/`Json`; relative user paths resolve against CWD, not `output_dir` — do not
  special-case.)
- `crates/cargo-regseq/src/cli.rs:38-39` — the `--emit` doc comment lists the supported
  specs (`model[=PATH], json[=PATH], driver[=MATCH_ID], analysis[.NAME][=PATH]`). Add
  `generic-c[=PATH]` to that help text. `EmitSpec` flows CLI → `CompilerOptions.emit`
  → JSON (`REGSEQ_OPTIONS`) → driver, so no other CLI plumbing changes are needed.
- `crates/cargo-regseq/README.md:8-11` — stale public emit list (Codex review): it already
  omits `json[=PATH]`. Add `generic-c[=PATH]` AND restore the missing `json[=PATH]` (the
  one-line list is being edited anyway).

### Verification surfaces

- `crates/regseq-config/tests/emit.rs` — `FromStr`/`Display` round-trip tests (house
  style: one assertion per `#[test]`). Add `generic-c` with-path / without-path parse +
  round-trip + subtype-rejection cases.
- `crates/regseq-driver/src/emit.rs` tests (inline `#[cfg(test)]` mod, ~line 93) — add an
  `emit_one_generic_c_*` test: a small model emits `generic.c` (default) and respects an
  explicit path. Mirror the existing `emit_one_model_*` tests.
- `crates/regseq-driver/tests/emit_dispatch.rs` — integration dispatch test surface; add a
  `GenericC` case if the file enumerates variants.
- `crates/cargo-regseq/tests/cli.rs` (Codex review) — CLI-level emit tests
  (`parses_emit_json_with_and_without_path` ~:127, `help_lists_json_emit_spec` ~:148). Add
  a `parses_emit_generic_c_with_and_without_path` case (mirror the json test) and, if a
  help-listing assertion is added, one for `generic-c[=PATH]`. No clap `value_parser` /
  possible-values list exists, so nothing auto-fails — these are additive.

## Implementation Notes

The `emit_one` arm:

```rust
EmitSpec::GenericC { path } => {
    let dest = resolve(path, output_dir, "generic.c");
    ensure_parent(&dest)?;
    let code = regseq_backends::emit_generic_c(model)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    fs::write(dest, code)
}
```

`emit_generic_c` returns `EmitError::Unsupported(&'static str)` for any construct outside
the Phase-1 matrix and `EmitError::ContainmentCycle` for a corrupt model; both map to a
fatal driver diagnostic via the existing `tcx.dcx().fatal(...)` at the `emit_one` call site
(`callbacks.rs::after_analysis`). That is the correct fail-loud behavior — do not swallow
it. No new validation is required: the model is already `model.validate`'d before dispatch.

The two `emit.rs` matches and the `Display` match are all exhaustive (no `_` arm), so the
compiler forces every site to be updated — nothing is silently missed.

## Non-Goals

- No compile/run/execute step — `.c` emission only (the runner owns host `cc`).
- No `c` short alias — `generic-c` only (per the chosen CLI spelling).
- No change to `Model`/`Json`/`Driver`/`Analysis` behavior.
- Not a `cargo regseq build`-to-binary command (separate, larger, deferred).

## Test Plan

1. Build the workspace; `regseq-driver` compiles with the new `regseq-backends` dep.
2. `cargo test -p regseq-config` — new `generic-c` parse/display/round-trip/subtype tests
   pass; existing emit tests unchanged.
3. `cargo test -p regseq-driver` — new `emit_one_generic_c_*` tests pass (default
   `generic.c`, explicit path honored); existing `emit_one_*` + `collect_unsupported_*`
   tests unchanged.
4. End-to-end smoke (manual or scripted): run `cargo regseq --emit generic-c` on a small
   domain-free crate within the supported surface and confirm a `generic.c` is written
   under `./regseq-out/` (default `output_dir`) that matches what the backend emits for
   that model. (Optional but recommended as the behavioral proof the hook actually fires.)
5. Full single-threaded workspace suite green:
   `RUST_TEST_THREADS=1 cargo test --workspace --no-fail-fast -- --test-threads=1`.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-config`, `regseq-driver`,
`cargo-regseq`) zero NEW warnings; single-threaded scoped tests green. Codex soundness
review of the dispatch wiring before close (per the project method).

## Acceptance Criteria

1. `EmitSpec::GenericC { path }` exists; `--emit generic-c` and `--emit generic-c=PATH`
   parse, round-trip through `Display`, and a subtype (`generic-c.x`) is rejected.
2. `regseq-driver` depends on `regseq-backends`; `emit_one` services `GenericC` by calling
   `emit_generic_c` and writing the TU (default `output_dir/generic.c`, explicit path
   honored); `collect_unsupported_warnings` does NOT warn for `GenericC`.
3. An `emit_generic_c` error surfaces as a fatal driver diagnostic (no silent miscompile).
4. The cargo-regseq `--emit` help text lists `generic-c[=PATH]`.
5. fmt + clippy (scoped) clean; scoped + full single-threaded suite green.
