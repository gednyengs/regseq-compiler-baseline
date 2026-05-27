# Codegen-coverage fixture corpus

This directory is the home for the codegen-coverage harness fixtures
(Doc 3 section 3, `docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`).

**The corpus is empty in this task (Phase-1 task a).** A real fixture
needs a committed `golden.c`, which requires a working generic-C emit;
the backend is a stub until tasks b-e land the lowering. The first
fixture is authored in **task e**. No `<name>/` directories ship here
yet -- the harness runs over this empty corpus and writes the honest
`not-pass` baseline ledger to `.harness/generic-c/coverage_ledger.{json,md}`.

## Per-fixture layout

Each fixture is a directory `fixtures/<name>/` containing:

| file | when | purpose |
| ---- | ---- | ------- |
| `manifest.toml` | always | fixture metadata (schema below) |
| `input.rs` | `kind = "rust"` | Rust snippet compiled through the bridge (stage 1) |
| `model.rs` | `kind = "ir"` | hand-built `RegseqModel` (deferred; see Non-Goals) |
| `golden.c` | always | golden emitted-C snapshot (stage-3 oracle) |
| `expected.stdout` | always | golden behavioral output (stage-6 oracle) |
| `expected.stderr` | `expect = "panic"` only | golden panic stderr (stage-5 oracle) |

## `manifest.toml` schema

```toml
name   = "example_add"        # fixture name (string)
kind   = "rust"               # "rust" | "ir"   ("ir" is deferred this phase)
entry  = "regseq_main"        # entry instance emission_name; default "regseq_main"
covers = ["BO-1..5", "RV-6"]  # Doc-1 matrix row IDs this fixture exercises
expect = "pass"               # "pass" | "panic"
```

- `name` -- the fixture name; matches the directory name by convention.
- `kind` -- `"rust"` bridges `input.rs` through the real bridge (the
  supported path); `"ir"` is **deferred** in Phase-1 task a (the executor
  skips it with a recorded reason) and needs a builder-registration
  design.
- `entry` -- the `emission_name` of the entry instance the generated
  `main.c` calls (Doc 2 D3). Defaults to `regseq_main`. The entry takes
  no args and returns unit (`void`); `{i,u}32` exit-code propagation is a
  later harness feature.
- `covers` -- the Doc-1 matrix row IDs this fixture exercises, taken
  **verbatim** (grouped ranges like `BO-1..5` are a single ID -- no
  expansion). This is the sole coverage-attribution source the ledger
  aggregates.
- `expect` -- `"pass"` asserts a clean exit + golden stdout (stage 6);
  `"panic"` asserts a non-zero exit + golden stderr (stage 5 inverted),
  since panics land on stderr (Doc 2 D4).

## The "log a computed value" pattern

Each fixture should *compute* a result through the construct under test,
then log it, so the print is a genuine end-to-end test of the lowering
(Doc 3 section 2):

```rust
let x = compute_via(/* construct under test */);
rseq::log_info!("result={}", x);   // golden stdout: "[INFO] result=<expected>\n"
```

Goldens are **never auto-blessed**: regenerate them deliberately via the
runner's `--bless` flag (a human-invoked authoring action), then review
the diff.
