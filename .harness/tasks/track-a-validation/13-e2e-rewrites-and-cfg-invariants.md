# track-a-validation-13: E2E Rewrites + CFG Structural Invariants (Test B)

**Depends on**: `regseq-driver-35`.

## Description

Phase-7E E2E retarget. Rewrites `pl011_bus_identity_proof.rs`
against `RegseqModel`; **retires** `pl011_lowering_input_stability.rs`
outright per migration.md "Existing-test disposition" (its
assertions targeted `AnalysisInput` accessors, a shape that is
gone). Adds **Test B** (CFG structural invariants; migration.md
Q5 Layer 8) as a new test file. Verifies the other seven E2E
tests pass unchanged.

Per Codex round-3 review: validation precedes deletion, so this
task (task 13) lands BEFORE tasks 14/15 (cleanup). After this task,
every in-tree test exercises either `RegseqModel` directly, live
emitter output, or infrastructure (QEMU, host-build).

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q5 Layer 8
  (CFG structural invariants) + "Existing-test disposition" list.
- `.harness/tasks/regseq-driver/35-emit-driver-pipeline-cutover.md`
  (upstream: emit_driver uses the new pipeline).
- `.harness/tasks/track-a-validation/12-pl011-reference-blessing-gate.md`
  (upstream: blessed reference exists).
- `tests/e2e/tests/pl011_bus_identity_proof.rs` lines 17-19,
  168-220 -- legacy-bootstrap, rewritten.
- `tests/e2e/tests/pl011_lowering_input_stability.rs` lines
  13-16, 81-109, 148-175 -- legacy-bootstrap, retired (its
  stability assertions targeted `AnalysisInput`, which is gone).
- `tests/e2e/tests/pl011_driver_emission.rs`,
  `pl011_linux_module_host_build_proof.rs`,
  `pl011_frontend_qc.rs`, `pl011_layer_boundary.rs`,
  `pl011_spec_slice.rs`, `driver_e2e.rs`,
  `platform_file_boundary.rs` -- the seven tests expected
  unchanged.

## Implementation Notes

### Rewrite 1: `pl011_bus_identity_proof.rs`

The invariant: "bus identity (`amba`) survives from raw-frontend
artifact through normalization and into the downstream model." New
shape:

```rust
#[test]
fn pl011_bus_identity_survives_into_regseq_model() {
    let raw = load_raw_artifact(
        "tests/fixtures/pl011-full-spec/expected/raw-frontend.json"
    );
    let platform = load_platform_file(
        "tests/fixtures/pl011-full-spec/platform.toml"
    );

    let mut model = regseq_model::normalize(&raw).expect("normalize");
    regseq_model::populate_bindings(&mut model, &platform)
        .expect("populate_bindings");

    // Phase-1 survival: declaration-side bus identity.
    let drv = model.root_module.driver_module().unwrap();
    assert_eq!(drv.driver.identity.bus, Some(BusType::Amba));

    // Phase-2 survival: platform binding.
    let bus_binding = query::bus_binding(&model).expect("bus binding");
    assert_eq!(bus_binding.bus_type, BusType::Amba);
    assert_eq!(bus_binding.amba_id, Some(0x00041011));
}
```

Retire old assertions against `AnalysisInput::bus_identity` /
`DeclarationModel.driver.bus` / `JoinedArtifact`-derived values.

### Retirement: `pl011_lowering_input_stability.rs`

Per migration.md Q5 "Existing-test disposition": this file is
**retired** outright. Its assertions targeted `AnalysisInput`
accessor byte-stability, a shape that is gone after tasks 6/7.
The stability invariant is subsumed by:

- The byte-equal reference regression in task 12 (if the full
  pipeline output is byte-stable across runs, by transitivity the
  intermediate portable-plan output is too).
- The CFG-invariant test (`pl011_cfg_invariants.rs` below)
  which asserts lowering-plan shape matches enumerated expectations.

Delete the file: `git rm tests/e2e/tests/pl011_lowering_input_stability.rs`.
Remove any references from `tests/e2e/Cargo.toml` or module
declarations. The other six untouched E2E tests pass without it.

### Test B -- CFG structural invariants

New file: `tests/e2e/tests/pl011_cfg_invariants.rs`.

```rust
#[test]
fn pl011_ordered_regions_metadata_present() {
    // For every function body that contains ordered!{} in the authored
    // driver, assert the corresponding Body carries an
    // OrderedRegion { entry, exit }.
    let model = load_pl011_model_via_phase6();
    let ordered_expected = &[
        // Enumerate the functions in tests/fixtures/pl011-full-spec/src/driver.rs
        // that use the ordered! macro. Start narrow; expand as
        // additional fixtures land.
        "program_polling_port",
    ];
    for fn_name in ordered_expected {
        let body = find_function_body(&model, fn_name);
        assert!(!body.ordered_regions.is_empty(),
            "{} must carry an OrderedRegion", fn_name);
    }
}

#[test]
fn pl011_fallible_intrinsics_carry_error_target() {
    // Every IntrinsicCall whose `intrinsic` is fallible
    // (clock_enable, reset_toggle, etc.) must have
    // Some(error_target).
    let model = load_pl011_model_via_phase6();
    for fn_id in model.function_ids() {
        let body = function_body(&model, fn_id);
        for block in &body.basic_blocks {
            if let TerminatorKind::IntrinsicCall {
                intrinsic, error_target, ..
            } = &block.terminator.kind {
                if is_fallible(*intrinsic) {
                    assert!(error_target.is_some(),
                        "fallible intrinsic {:?} in {:?} missing error_target",
                        intrinsic, fn_id);
                }
            }
        }
    }
}

#[test]
fn pl011_verify_phase_2_rejects_missing_required_binding() {
    let mut model = load_pl011_model_via_phase6();
    // Mutate the model to unset a required binding slot.
    // (Use MutationGuard per task-57 conventions.)
    // Assert commit returns Err(VerifierError::RequiredBindingMissing).
}

#[test]
fn pl011_lowering_obligations_are_shape_stable() {
    let model = load_pl011_model_via_phase6();
    let plan = build_driver_lowering_plan_from_model(
        &model, &LoweringConfig::default()
    ).unwrap();
    // Obligation shapes + counts match an enumerated baseline
    // derived from the authored driver (e.g., "1 MMIO resource,
    // 1 required clock, 1 optional reset, 2 pin states").
    assert_eq!(plan.resources.mmio.len(), 1);
    // ...
}
```

### Six untouched tests

Run each of the following and confirm they pass unchanged:

- `driver_e2e.rs`
- `pl011_driver_emission.rs`
- `pl011_frontend_qc.rs`
- `pl011_layer_boundary.rs`
- `pl011_linux_module_host_build_proof.rs` (already retargeted to
  blessed reference in task 12; confirms it still passes).
- `pl011_spec_slice.rs`
- `platform_file_boundary.rs`

(Seven files listed minus `pl011_lowering_input_stability.rs` which
is deleted above = six that pass unchanged.)

If any fails, this task does NOT patch the test to pass; it stops
and escalates to the task whose changes broke it.

### What this task does NOT do

- Does not delete anything (tasks 14, 15).
- Does not change emitters / pipeline.
- **`crates/regseq-driver/src/construction*`** strict non-goal.

## Non-Goals

- No new test infrastructure (harness is task 3; blessing is task 12).
- No retargeting of the seven untouched tests.
- No golden-file refreshes (goldens unchanged through Phase 7;
  golden regen is Phase 9.2).

## Test Plan

- Both rewrites pass.
- Test B (three/four new CFG-invariant assertions) passes.
- Seven untouched tests pass unchanged.
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review track-a-validation-13 against .harness/tasks/track-a-validation/13-e2e-rewrites-and-cfg-invariants.md.

Files:
- tests/e2e/tests/pl011_bus_identity_proof.rs (rewritten)
- tests/e2e/tests/pl011_lowering_input_stability.rs (expected DELETED)
- tests/e2e/tests/pl011_cfg_invariants.rs (new)
- tests/e2e/Cargo.toml (module / test declarations cleaned up
  to reflect the deletion)
- The six other E2E files (git diff should show zero changes)

Focus on:
- Rewrite imports from regseq_model only; no legacy model types.
- pl011_lowering_input_stability.rs is fully removed (per
  migration.md Q5 "Existing-test disposition"), not retargeted.
- Test B asserts the CFG-level invariants documented in
  migration.md Q5 Layer 8.
- Six untouched E2E tests are literally byte-unchanged.
- No test assumes data that was deleted from DriverLoweringPlan
  (behavior, legacy_behavior, callback_actions).

Do NOT flag:
- Legacy modules still existing (tasks 14, 15).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-e2e --test pl011_bus_identity_proof
cargo test -p regseq-e2e --test pl011_cfg_invariants
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
git diff --stat main..HEAD -- tests/e2e/tests/
# Diff should show: pl011_bus_identity_proof.rs rewritten,
# pl011_lowering_input_stability.rs DELETED, pl011_cfg_invariants.rs
# ADDED. Six untouched tests show 0 changes.
```

## Acceptance Criteria

- `pl011_bus_identity_proof.rs` rewritten against RegseqModel.
- `pl011_lowering_input_stability.rs` deleted (per migration.md
  "Existing-test disposition").
- `pl011_cfg_invariants.rs` new file passes Test B assertions.
- Six remaining E2E tests byte-unchanged and green.
- `cargo test --workspace` green.
- Codex review APPROVED.
