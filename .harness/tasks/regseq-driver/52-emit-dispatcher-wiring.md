# regseq-driver-52: Wire Bridge Emit Dispatcher (`--emit model` = pretty-printed, `--emit json` = JSON)

## Description

Wire up the regseq-driver emit dispatcher for the first time.
`RegseqCallbacks::after_analysis` at
`crates/regseq-driver/src/callbacks.rs:121-136` is currently a
stub that prints `"bridge not yet wired -- skipping all
extraction"` and stops; no emission code path exists today.

This task lands two output formats with a breaking-naming
choice (acceptable because no external consumers exist yet on
the phase-1a code):

- **`--emit model[=PATH]`** → write the `PrettyPrinter`-rendered
  textual `RegseqModel` (the human-readable form already
  produced by `regseq_model::PrettyPrinter::new(&model).render()`
  per `pretty-printer.md` §10.3, identical to what
  `regseq-model/tests/snapshots/*` already snapshots).
- **`--emit json[=PATH]`** → write the normalized `RegseqModel`
  serialized as JSON via `serde_json::to_writer_pretty`. The
  `RegseqModel` already derives `Serialize`/`Deserialize` at
  `crates/regseq-model/src/model.rs:53`.

The `EmitSpec::Model { path }` variant's doc-comment at
`crates/regseq-config/src/emit.rs:32-35` currently aspires to
"emit the normalized RegseqModel as JSON", but no production
code reads that variant; this task repurposes `Model` to mean
"pretty-printed text" and introduces a new `Json` variant for
the JSON path.

Surfaced after `regseq-model-95` (P1a-E2E-03) closed the
synthesis E2E coverage and the user asked how to invoke the
compiler against arbitrary Rust source. The probe-test pattern
works (write a fixture file + a `#[test]` that calls
`bridge_fixture` + `render_default`), but a first-class
`cargo regseq --emit ...` CLI is the natural user-facing
surface.

After this task lands, the production invocation path is:

```
cargo regseq \
  --target-os linux \
  --platform-file path/to/platform.toml \
  --output-dir ./regseq-out/ \
  --emit model=./regseq-out/model.txt \
  --emit json=./regseq-out/model.json \
  -- -p <your-crate>
```

## Input Context

- `crates/regseq-driver/src/callbacks.rs:121-136`
  -- `RegseqCallbacks::after_analysis` (the stub this task
  replaces).
- `crates/regseq-driver/src/callbacks.rs:87-105`
  -- `RegseqCallbacks` struct + `new(options)` constructor.
- `crates/regseq-driver/src/lib.rs:48-69`
  -- `DriverMode` dispatch; `DriverMode::Regseq` routes to
  `RegseqCallbacks::new(options)` with options deserialised
  from the `REGSEQ_OPTIONS` env var (the cargo-to-driver
  JSON transport).
- `crates/regseq-driver/src/lib.rs:14`
  -- the natural place to declare the new `mod emit;`.
- `crates/regseq-driver/src/bridge/mod.rs:41-43`
  -- `BridgeInput::new(tcx: TyCtxt<'tcx>)`.
- `crates/regseq-driver/src/bridge/mod.rs:600+`
  -- `run_bridge(input) -> Result<RegseqModel, BridgeError>`.
- `crates/regseq-model/src/pretty.rs:97-102`
  -- `PrettyPrinter::new(&model).render()`.
- `crates/regseq-model/src/model.rs:1530-1630`
  -- `RegseqModel::validate(&mut self, &BridgeInput) ->
  Result<(), InvariantViolation>` (note `&mut self`; the
  callback needs `let mut model = run_bridge(input)?`).
- `crates/regseq-model/src/model.rs:53`
  -- `RegseqModel` derives `Serialize` / `Deserialize`.
- `crates/regseq-config/src/emit.rs:19-99`
  -- `EmitSpec` enum + `FromStr` + `Display` (the surface to
  amend).
- `crates/regseq-config/src/options.rs:10-20`
  -- `CompilerOptions { output_dir: PathBuf, emit:
  Vec<EmitSpec>, check: bool, ... }`.
- `crates/cargo-regseq/src/cli.rs:35-83`
  -- `Cli` struct: `--emit` (line 37), `--check` (line 40-42),
  `--output-dir` (line 32), `--target-os` (line 25),
  `--platform-file` (line 28). The doc-comment at line 35-36
  lists supported specs and needs updating.
- `crates/cargo-regseq/src/cli.rs:181-185`
  -- the default-emit-when-empty fallback (currently pushes
  `EmitSpec::Model { path: None }`; the variant name is
  unchanged but the semantic is now "pretty-printed text").
- `crates/regseq-config/tests/emit.rs`
  -- `FromStr` + `Display` roundtrip tests for each variant.
- `crates/cargo-regseq/tests/cli.rs:106,414`
  -- assertions about the default emit's variant name
  (parsing-level; should still pass since `EmitSpec::Model`
  is unchanged structurally).
- `crates/cargo-regseq/tests/invoke.rs:32,131-135`
  -- existing tests that build `CompilerOptions` and assert
  the JSON transport round-trips through `REGSEQ_OPTIONS`.
- `docs/discussions/compiler-refactoring/pipeline-refactor.md:798-888`
  -- the project's plan for `tcx.dcx().fatal(msg)` +
  `catch_with_exit_code` error handling in the driver
  callbacks.
- `docs/discussions/compiler-refactoring/pipeline-refactor.md:879`
  -- the plan for accepting `Driver` / `Analysis` emit specs
  but not connecting them until phase-1b.
- `crates/regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs`
  -- the in-process rustc-driver invocation pattern this task's
  integration test mirrors.
- `crates/regseq-model/src/domain_attr.rs:26,108`
  -- `domain_attrs.field_attrs: HashMap<FieldPath, Vec<AttrData>>`
  and `domain_attrs.span_attrs: HashMap<Span, Vec<AttrData>>`.
  These serialize with non-deterministic `HashMap::iter()`
  order; flagged as a known reproducibility limitation (see
  Non-Goals).

## Implementation Notes

Four changes in dependency order.

### Change 1: Add `EmitSpec::Json` variant + repurpose `EmitSpec::Model` doc + strict subtype rejection for `json`

In `crates/regseq-config/src/emit.rs`:

- **Line 32-35**: Replace the `Model` doc-comment from "Emit
  the normalized `RegseqModel` as JSON" to "Emit the
  pretty-printed textual `RegseqModel` per
  `pretty-printer.md` §10.3."
- **Line 36-39**: Add new variant `Json { path: Option<PathBuf> }`
  with its own doc-comment ("Emit the normalized `RegseqModel`
  as JSON via `serde_json::to_writer_pretty`.").
- **Line 11-13**: Update the `EmitParseError::UnknownType`
  error message to list `'json'` alongside `'driver',
  'analysis', or 'model'`.
- **`FromStr` (lines 42-70)**: Add a `"json"` match arm.
  Subtype handling: REJECT non-empty subtype. `json.foo` MUST
  return `Err(EmitParseError::UnknownType(...))` (or a new
  `SubtypeNotSupported` variant, agent's choice -- see below).
  Rationale: `analysis.name` is the only spec where `.subtype`
  is semantically meaningful (closed list of analysis names);
  `json` has no subtypes and accepting `json.foo` silently
  would be a footgun. The current `model` arm at
  `emit.rs:64-66` LEAVES subtype-drop lenient for backward
  compat (the legacy behavior is preserved unchanged); the
  new `json` arm is strict.
- **`Display` (lines 72-99)**: Add two arms for `Json { path:
  None }` → `"json"` and `Json { path: Some(p) }` →
  `format!("json={}", p.to_string_lossy())`.

`EmitSpec` is re-exported from `crates/regseq-config/src/lib.rs:11`;
no extra re-export needed.

**Optional error refinement**: if a `SubtypeNotSupported`
variant feels cleaner than reusing `UnknownType`, add it to
`EmitParseError`. The agent may choose; Test Plan A5 must
cover whichever form is chosen.

### Change 2: Wire `RegseqCallbacks::after_analysis` to call the bridge (consolidated skeleton — warning loop integrated)

In `crates/regseq-driver/src/callbacks.rs:121-136`, replace
the stub with the load-bearing emit-path skeleton (the
warning-on-unsupported pass is integrated here, NOT a
separate change):

```rust
fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
) -> rustc_driver::Compilation {
    // 1. Bridge rustc HIR/MIR -> RegseqModel.
    let input = crate::bridge::BridgeInput::new(tcx);
    let mut model = match crate::bridge::run_bridge(input) {
        Ok(m) => m,
        Err(e) => {
            tcx.dcx().fatal(format!("regseq-driver: bridge failed: {e}"));
        }
    };
    // 2. Joint validate (rebuilds vtables side table; takes
    //    &mut self).
    let bridge_input = regseq_model::BridgeInput::matching(&model);
    if let Err(e) = model.validate(&bridge_input) {
        tcx.dcx().fatal(format!("regseq-driver: model.validate failed: {e:?}"));
    }
    // 3. Honor `--check`: validate-only path, skip ALL emission.
    if self.options.check {
        return rustc_driver::Compilation::Stop;
    }
    // 4. Warn on unsupported emit specs (Driver / Analysis are
    //    documented but phase-1b territory per
    //    `pipeline-refactor.md:879`). The dispatcher itself
    //    returns Ok(()) silently for these; user-facing
    //    visibility comes from this warning pass.
    //    `collect_unsupported_warnings` is defined in
    //    `crate::emit` (rustc-agnostic) so the warning logic
    //    is unit-testable without a tcx.
    for warning in crate::emit::collect_unsupported_warnings(&self.options.emit) {
        tcx.dcx().warn(warning);
    }
    // 5. Dispatch each emit spec. Errors fail the build.
    for spec in &self.options.emit {
        if let Err(e) = crate::emit::emit_one(&model, spec, &self.options.output_dir) {
            tcx.dcx().fatal(format!("regseq-driver: emit {spec} failed: {e}"));
        }
    }
    rustc_driver::Compilation::Stop
}
```

Numbered steps map to the amendment justifications:

- **Step 2** uses `let mut model` because
  `RegseqModel::validate` takes `&mut self` (it rebuilds the
  `vtables` side table per `regseq-model/src/model.rs:91,3207`);
  subsequent steps borrow `&model` immutably.
- **Step 3** honors `CompilerOptions.check` per
  `cargo-regseq/src/cli.rs:40` ("Validate without writing
  output files"). The validate-only path is the only correct
  reading of `--check`; this short-circuits BEFORE warnings
  AND emission.
- **Steps 4-5** are two separate loops: the warning loop runs
  once over `options.emit` and routes strings through
  `tcx.dcx().warn`; the emit loop dispatches the same specs
  through `emit::emit_one`. Two passes (not merged) so that
  if a later iteration changes the warning-emission policy
  (e.g., gate behind a flag, deduplicate), the change is
  localised to the warning loop without touching the emit
  loop.
- **All error paths** use `tcx.dcx().fatal(msg)` (which
  diverges) per `pipeline-refactor.md:798,888`. `Compilation::Stop`
  on the success path is rustc's idiomatic "halt before codegen"
  signal.

### Change 3: Implement `crates/regseq-driver/src/emit.rs` dispatcher

New file `crates/regseq-driver/src/emit.rs`:

```rust
//! Emit dispatcher for regseq-driver. Wires `EmitSpec` variants
//! to the corresponding RegseqModel emission paths
//! (pretty-printed text via `PrettyPrinter::render`, or JSON via
//! `serde_json::to_writer_pretty`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use regseq_config::EmitSpec;
use regseq_model::{PrettyPrinter, RegseqModel};

/// Dispatch a single `EmitSpec` against the validated model.
/// `output_dir` is the user-configured base; per-spec `path`
/// overrides the default file name when set.
pub fn emit_one(
    model: &RegseqModel,
    spec: &EmitSpec,
    output_dir: &Path,
) -> std::io::Result<()> {
    match spec {
        EmitSpec::Model { path } => {
            let dest = resolve(path, output_dir, "model.txt");
            ensure_parent(&dest)?;
            let rendered = PrettyPrinter::new(model).render();
            fs::write(dest, rendered)
        }
        EmitSpec::Json { path } => {
            let dest = resolve(path, output_dir, "model.json");
            ensure_parent(&dest)?;
            let mut file = fs::File::create(dest)?;
            serde_json::to_writer_pretty(&mut file, model)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            file.write_all(b"\n")
        }
        EmitSpec::Driver { .. } | EmitSpec::Analysis { .. } => {
            // Phase-1b territory per
            // `pipeline-refactor.md:879`. The dispatcher
            // returns `Ok(())` silently here; the user-facing
            // warning is emitted by the caller
            // (`callbacks.rs::after_analysis`) via
            // `collect_unsupported_warnings` (below) routed
            // through `tcx.dcx().warn`. Keeping the warning
            // logic outside `emit_one` preserves the
            // rustc-agnostic property of the dispatcher.
            Ok(())
        }
    }
}

/// Collect human-readable warning strings for emit specs the
/// dispatcher cannot service (`Driver` / `Analysis`). The
/// caller is responsible for routing these strings through
/// rustc's diagnostic system (`tcx.dcx().warn`). Separating
/// collection from emission keeps emit.rs rustc-agnostic and
/// unit-testable WITHOUT requiring a `TyCtxt`.
pub fn collect_unsupported_warnings(specs: &[EmitSpec]) -> Vec<String> {
    specs
        .iter()
        .filter_map(|spec| match spec {
            EmitSpec::Driver { .. } | EmitSpec::Analysis { .. } => Some(format!(
                "regseq-driver: --emit {spec} is documented but not yet implemented (phase-1b); skipping",
            )),
            EmitSpec::Model { .. } | EmitSpec::Json { .. } => None,
        })
        .collect()
}

fn resolve(
    path: &Option<PathBuf>,
    output_dir: &Path,
    default_name: &str,
) -> PathBuf {
    path.clone().unwrap_or_else(|| output_dir.join(default_name))
}

fn ensure_parent(dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
```

Declared in `crates/regseq-driver/src/lib.rs:14` (near `mod
bridge;` and `mod callbacks;`):

```rust
pub mod emit;
```

### Change 4: Test updates (per Codex amendments A3, A4)

#### 4a. `crates/regseq-config/tests/emit.rs`

Add a `parse_json` test mirroring `parse_model` (the existing
test at line ~70-90). Cover both `Json { path: None }` and
`Json { path: Some(PathBuf::from("foo.json")) }` for FromStr
and Display.

Add a serde round-trip test for `EmitSpec::Json` (the cargo
→ driver `REGSEQ_OPTIONS` JSON transport must support it),
mirroring whatever pattern exists in
`crates/regseq-config/tests/options.rs:31` for the other
variants. If no serde round-trip exists for any
`EmitSpec` variant yet, add one for `Model` AND `Json` (so
both forms are coverage-equivalent).

#### 4b. `crates/cargo-regseq/` source comments + tests

**Source comment fix (REQUIRED).** At
`crates/cargo-regseq/src/cli.rs:35-36` the doc-comment on
`--emit` says "Supported specs: model[=PATH], driver[=MATCH_ID],
analysis[.NAME][=PATH]." Update to include `json[=PATH]`. At
`crates/cargo-regseq/src/cli.rs:181-184` the comment about the
default emit-when-empty says "Default emit during the
RegseqModel-only rebuild: produce the normalized RegseqModel
JSON." This is now WRONG (the variant produces pretty-printed
text after this task); rewrite the comment to read
"Default emit during the RegseqModel-only rebuild: produce
the pretty-printed RegseqModel text. Pass --emit json
explicitly for the machine-readable form."

**Test updates (REQUIRED).**

- Lines 106, 414: existing default-emit assertions check
  `EmitSpec::Model { path: None }`. These should STILL pass
  because the variant name is unchanged; verify and update
  only if the parsing path changes.
- Add new test `parses_emit_json_with_and_without_path`
  mirroring whatever shape `parses_emit_analysis_*` tests use
  in the same file. Cover `--emit json` and
  `--emit json=foo.json` parsing.
- Add a help-text test (if the file has one) asserting the
  help output now lists `json[=PATH]` alongside the other
  specs.

#### 4c. New integration test `crates/regseq-driver/tests/emit_dispatch.rs`

Mirror the in-process rustc-driver invocation pattern at
`crates/regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs`.
Bridge the `no_op` fixture (the smallest valid input) and run
the dispatcher with three different options shapes:

1. **`--emit model=$TMP/model.txt`** (only) -- assert the file
   exists, is non-empty, contains `"fn"` and `"no_op"`
   substrings (the pretty-printed rendering MUST include the
   `no_op` function signature per `pretty-printer.md` §10.2.1).
2. **`--emit json=$TMP/model.json`** (only) -- assert the file
   exists, is non-empty, deserializes via
   `serde_json::from_reader::<_, RegseqModel>(file)` without
   error. Optional: assert the deserialized model's items
   length matches the original.
3. **`check: true, --emit model=$TMP/should_not_exist.txt`**
   -- assert the file does NOT exist after the run, AND the
   bridge ran (verifiable indirectly via stderr or by
   confirming no fatal). This exercises Change 2's `--check`
   short-circuit.

The test must NOT shell out to `cargo regseq`; it must drive
the bridge in-process with `RegseqCallbacks::new(options)` and
custom `rustc_args`, mirroring the existing
`bridge_vtable_indirect_end_to_end.rs:63-141` pattern.

### Conventions

- The dispatcher in `emit.rs` is rustc-agnostic (no `tcx`,
  no rustc imports). All diagnostic emission lives in
  `callbacks.rs`'s `after_analysis`.
- Doc comments on all `pub` functions citing relevant
  spec / pipeline-refactor.md line ranges.
- The new `Json` variant's `Display` form MUST round-trip
  through `FromStr` (covered by Test Plan B3).
- Idempotency: re-running the same `cargo regseq` invocation
  MUST produce identical file contents for `--emit model`
  (the `PrettyPrinter` is deterministic per
  `pretty-printer.md` §4.1). For `--emit json`, content is
  deterministic ONLY when `domain_attrs.field_attrs` and
  `domain_attrs.span_attrs` are empty (see Non-Goals).

## Non-Goals

- **JSON output reproducibility when domain_attrs has non-empty
  HashMap-backed tables.** `crates/regseq-model/src/domain_attr.rs:26,108`
  defines `field_attrs: HashMap<FieldPath, Vec<AttrData>>` and
  `span_attrs: HashMap<Span, Vec<AttrData>>`. Serializing these
  via `HashMap::iter()` is non-deterministic across runs. The
  no_op + diag_logging + closure + trait_object fixtures all
  produce EMPTY field_attrs/span_attrs, so today's JSON output
  is reproducible for those fixtures. Fixing the underlying
  HashMap-to-BTreeMap (or sort-on-serialize) is a separate
  task; documented here as a known limitation. The integration
  test only asserts deserializability, not byte-for-byte
  re-serialize round-trip.

- **`Driver` and `Analysis` emit specs.** Per
  `pipeline-refactor.md:879`, these are documented as
  supported but unimplemented until phase-1b. This task
  preserves the CLI parsing (so users can pass `--emit driver`
  without a parse error), routes them to a non-fatal
  `tcx.dcx().warn` in `after_analysis`, and skips actual
  emission. Their full implementation is phase-1b territory.

- **`cargo regseq` end-to-end test that runs a real cargo
  build.** The integration test runs in-process via
  `rustc_driver::run_compiler` with custom `RegseqCallbacks`
  options, mirroring `bridge_vtable_indirect_end_to_end.rs`.
  A true end-to-end test would require shelling out to
  `cargo regseq` against a real cargo workspace; that's
  outside this task's scope.

- **Diagnostic output formatting.** The `tcx.dcx().fatal` and
  `.warn` calls use rustc's default diagnostic rendering; no
  custom `Diag` styling is in scope.

- **Default `output_dir` value or layout.** The user-configured
  `output_dir` (default `./regseq-out/` per
  `cargo-regseq/src/cli.rs:32`) is respected as-is. Driver
  subdirectory layout, `drivers/<match-id>/` paths, and
  analysis report folder structure are phase-1b territory.

- **Multiple `--emit model=PATH` invocations.** The CLI accepts
  `--emit` as `Vec<EmitSpec>`, so a user could pass
  `--emit model --emit model=foo.txt` and the dispatcher would
  write twice (once to the default path, once to `foo.txt`).
  This is acceptable per the existing `--emit` semantics; no
  special-case deduplication.

- **`check` interaction with `--emit json`.** Per Change 2,
  `--check` skips ALL emission regardless of spec. There is no
  scenario where `--check` validates AND emits JSON for review.
  That's by design (`--check` = "validate without writing
  output files" per `cli.rs:40`).

## Test Plan

### A. EmitSpec parsing

1. `EmitSpec::from_str("json")` returns
   `Ok(EmitSpec::Json { path: None })`.
2. `EmitSpec::from_str("json=foo.json")` returns
   `Ok(EmitSpec::Json { path: Some(PathBuf::from("foo.json")) })`.
3. `EmitSpec::Json { path: None }.to_string()` returns
   `"json"`; with a path, returns
   `"json=<path-display>"`.
4. `EmitSpec::from_str("nonsense")` returns
   `Err(EmitParseError::UnknownType("nonsense".to_string()))`
   with the error message listing `'json'` alongside the
   other supported specs.
5. The serde round-trip
   `serde_json::from_str(&serde_json::to_string(&EmitSpec::Json { path: None }).unwrap()).unwrap()`
   returns the original (the `REGSEQ_OPTIONS` JSON transport
   path).
5b. **json subtype rejection.** `EmitSpec::from_str("json.foo")`
    returns `Err(_)` (either `UnknownType("json.foo".to_string())`
    or a new `SubtypeNotSupported` variant per Change 1's
    "Optional error refinement" note). This is the strict
    behavior new code adopts; the legacy `model.foo`
    leniency is preserved unchanged for backward compat
    (NOT asserted here).

### B. Cargo-regseq CLI parsing

6. `cargo-regseq` parses `--emit json` and yields
   `vec![EmitSpec::Json { path: None }]`.
7. `cargo-regseq` parses `--emit json=foo.json` and yields
   `vec![EmitSpec::Json { path: Some(PathBuf::from("foo.json")) }]`.
8. `cargo-regseq`'s default-when-empty assertion at
   `tests/cli.rs:106,414` still passes
   (`EmitSpec::Model { path: None }`).
9. The CLI help output (if covered by a test) lists
   `json[=PATH]` alongside the other supported specs.

### C. emit_one dispatcher unit tests

10. `emit_one(&model, &EmitSpec::Model { path: None },
    &tmp_dir)` writes `tmp_dir/model.txt` containing the
    pretty-printed render.
11. `emit_one(&model, &EmitSpec::Model { path: Some(
    explicit_path) }, &tmp_dir)` writes `explicit_path` (the
    `tmp_dir` is ignored in this case).
12. `emit_one(&model, &EmitSpec::Json { path: None },
    &tmp_dir)` writes `tmp_dir/model.json` containing valid
    JSON that deserializes back to a `RegseqModel`.
13. `emit_one(&model, &EmitSpec::Driver { .. }, &tmp_dir)`
    returns `Ok(())` and writes nothing (the silent no-op at
    the dispatcher level; the warning lives in
    `after_analysis`).
13b. **`collect_unsupported_warnings` unit test.** Given a
     `Vec<EmitSpec>` containing one `Driver`, one `Analysis`,
     one `Model`, one `Json`, assert the returned Vec has
     exactly 2 entries, both containing the substring
     `"phase-1b"` and `"skipping"`, and one mentioning
     `"driver"` while the other mentions `"analysis"`. Given
     a Vec with ONLY `Model` and `Json`, assert the returned
     Vec is empty.
13c. **`ensure_parent` / `resolve` edge cases.** Cover the
     following paths via `emit_one` calls:
     - Explicit relative path `EmitSpec::Model { path:
       Some(PathBuf::from("foo.txt")) }` -- `Path::parent()`
       returns `Some("")` on Unix; the helper must skip the
       mkdir and write to the current directory successfully.
     - Explicit nested relative path `EmitSpec::Model { path:
       Some(tmp_dir.join("nested/dir/foo.txt")) }` -- the
       helper must `create_dir_all` the `nested/dir`
       components.
     - Default path (`path: None`, output_dir = nested
       not-yet-existing dir) -- the helper must
       `create_dir_all` the output_dir before writing.

### D. End-to-end integration (`tests/emit_dispatch.rs`)

14. Bridge the `no_op` fixture with options `emit: vec![
    EmitSpec::Model { path: Some(tmp/model.txt) }]`,
    `check: false`. Assert `tmp/model.txt` exists, is non-
    empty, and contains the substring `"no_op"` (the
    fixture's only function).
15. Bridge the `no_op` fixture with options `emit: vec![
    EmitSpec::Json { path: Some(tmp/model.json) }]`,
    `check: false`. Assert `tmp/model.json` exists and
    deserializes via `serde_json::from_reader::<_,
    RegseqModel>(file)` without error.
16. Bridge the `no_op` fixture with options `check: true,
    emit: vec![EmitSpec::Model { path: Some(
    tmp/should_not_exist.txt) }]`. Assert
    `tmp/should_not_exist.txt` does NOT exist (the `--check`
    short-circuit).
17. Bridge the `no_op` fixture with options `emit: vec![
    EmitSpec::Driver { match_id: None }]` (unsupported spec).
    Assert no panic, no fatal error; the test must NOT fail
    because `Driver` returns `Ok(())` silently at the
    dispatcher level. The warning STRING content is asserted
    at C13b's unit test on `collect_unsupported_warnings`
    (which is rustc-agnostic and unit-testable); D17 does NOT
    attempt to capture rustc's diagnostic output here because
    capturing through `tcx.dcx().warn` is brittle in the
    in-process test harness.

### E. Workspace regression-clean

18. `cargo test --workspace` -- all existing tests still
    pass (in particular, no regression in regseq-model-94's
    diag_logging E2E, regseq-model-95's synthesis E2E, or
    regseq-driver-50's vtable end-to-end test).
19. `cargo fmt --all -- --check` -- clean.
20. `cargo clippy --workspace --tests -- -D warnings` -- no
    new lint sites beyond the pre-existing baseline (35 sites
    on `fixes/audit-2026-05-15` at the time of this task's
    registration).

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash tool
(per `SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-52 against its task
spec at
.harness/tasks/regseq-driver/52-emit-dispatcher-wiring.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-config/src/emit.rs (EmitSpec amendments)
- crates/regseq-config/tests/emit.rs (parsing tests)
- crates/regseq-driver/src/emit.rs (NEW dispatcher module)
- crates/regseq-driver/src/callbacks.rs (after_analysis wiring)
- crates/regseq-driver/src/lib.rs (mod emit declaration)
- crates/regseq-driver/tests/emit_dispatch.rs (NEW integration)
- crates/cargo-regseq/tests/cli.rs (json parsing coverage)

Focus on:
- EmitSpec::Model semantic flipped from "JSON" doc to
  "pretty-printed text"; new EmitSpec::Json variant.
- after_analysis honors `CompilerOptions.check`: bridge +
  validate, then SKIP all emission when check=true.
- Error handling uses tcx.dcx().fatal(msg) (diverging) for
  bridge/validate/emit failures, NOT eprintln + Stop.
- Unsupported emit specs (Driver / Analysis) route through
  tcx.dcx().warn(msg) before the dispatcher loop; the
  dispatcher itself returns Ok(()) silently.
- The emit dispatcher is rustc-agnostic: emit.rs has NO
  `tcx` parameter and no rustc imports.
- `let mut model` because RegseqModel::validate takes &mut self.
- The integration test mirrors
  bridge_vtable_indirect_end_to_end.rs's in-process
  rustc-driver invocation pattern (no shelling to cargo regseq).
- JSON output is deserializable (round-trip via
  serde_json::from_reader::<_, RegseqModel>). Reproducibility
  beyond that is out of scope per Non-Goals.
- All Test Plan items A1-E20 are present.

Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate about code
you did not read. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate (fix → re-invoke `codex_runner.py`) until VERDICT:
APPROVED.

## Quality Gate

Workspace-wide because this task spans three crates
(`regseq-config` + `regseq-driver` + `cargo-regseq`):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/regseq-config/src/emit.rs` has a new
  `EmitSpec::Json { path: Option<PathBuf> }` variant with
  matching `FromStr` + `Display` + doc-comment; the existing
  `EmitSpec::Model` variant's doc-comment is updated from
  "JSON" to "pretty-printed text"; the
  `EmitParseError::UnknownType` error message lists `'json'`.
- `crates/regseq-driver/src/emit.rs` exists with a
  `pub fn emit_one(&RegseqModel, &EmitSpec, &Path) ->
  std::io::Result<()>` that dispatches Model →
  `PrettyPrinter::render` + file write and Json →
  `serde_json::to_writer_pretty`. `Driver` / `Analysis` arms
  return `Ok(())` silently.
- `crates/regseq-driver/src/callbacks.rs`'s
  `RegseqCallbacks::after_analysis` calls `run_bridge` →
  `model.validate(&bridge_input)` (the `&mut model` is the
  receiver; `bridge_input` is the
  `regseq_model::BridgeInput::matching(&model)` argument) →
  warn loop → emit loop, with `tcx.dcx().fatal(msg)` for all
  error paths and `tcx.dcx().warn(msg)` for `Driver`/
  `Analysis` no-op preludes; honors `self.options.check` by
  short-circuiting before warnings AND emission.
- `crates/regseq-driver/src/lib.rs` declares `pub mod emit;`.
- `crates/regseq-config/src/emit.rs`'s `FromStr` strictly
  rejects `json.foo` (or `json.<any>`) per Change 1's new
  subtype rule; the legacy `model.foo` lenient behavior is
  preserved unchanged.
- `crates/cargo-regseq/src/cli.rs:35-36` and
  `:181-184` are updated per Change 4b's source-comment fix.
- `crates/regseq-driver/src/emit.rs` carries both
  `emit_one(...)` AND
  `collect_unsupported_warnings(specs: &[EmitSpec]) ->
  Vec<String>`.
- `crates/regseq-config/tests/emit.rs` covers Test Plan
  items A1-A5b.
- `crates/cargo-regseq/tests/cli.rs` covers Test Plan items
  B6-B9.
- Unit tests cover Test Plan items C10-C13c (dispatcher
  edge cases including `collect_unsupported_warnings`
  shape and `ensure_parent`/`resolve` path resolution).
- `crates/regseq-driver/tests/emit_dispatch.rs` covers Test
  Plan items D14-D17.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
