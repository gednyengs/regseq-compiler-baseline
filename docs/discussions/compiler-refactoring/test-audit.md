# Test Audit -- Disable-and-Keep Classification

## Methodology

A test is **KEEP** iff it asserts a property of rustc HIR/MIR consumption,
or of pre-rustc setup (wrapper dispatch, callbacks config, argument
parsing, CLI parsing, configuration parsing, proc-macro expansion).

A test is **VOID** iff it asserts a property of anything we *produce*
from HIR/MIR -- the constructed RegseqModel, JSON serialization of the
model, IR snapshots, analysis outputs, RegseqType outputs from MIR-type
translation, or backend codegen artifacts.

Edge cases (extraction surfaces such as ClockSurface, registries such as
IntrinsicRegistry, and the Layer 2 platform-file parser) are evaluated
individually below. Defaults are recommended; the user has the final
call.

## Summary

- Total integration test functions classified: 750 across 81 files.
- Total inline `#[cfg(test)]` tests classified: 276 across 35 files.
- Combined: 1026 test functions across 116 files.

By verdict:
- KEEP: 27 files (mix of pre-rustc, HIR/MIR-boundary, regseq-facade
  surface tests, regseq-macros expansion, and HIR-walk extraction
  registries that are recommended-keep edge cases).
- VOID: 80 files (all construction tests, all model-level tests, all
  source-to-regseq-model harness tests, all backend tests, all analysis
  tests, all e2e tests that assert produced artifacts, every inline
  `#[cfg(test)]` module under `regseq-model/src/`, `regseq-driver/src/
  construction/` except `try_folding.rs`, and `regseq-backends/src/`).
- EDGE CASES (assessed and recommended below): 9 categories.

By crate:

| Crate | Files | KEEP | VOID |
|---|---|---|---|
| `cargo-regseq` | 3 | 3 | 0 |
| `regseq-driver` (tests/) | 47 | 21 | 26 |
| `regseq-driver` (src/ inline) | 8 | 1 | 7 |
| `regseq-model` (tests/) | 10 | 5 | 5 |
| `regseq-model` (src/ inline) | 26 | 0 | 26 |
| `regseq-backends` | 2 + 1 src | 1 | 2 |
| `regseq-analysis` | 2 | 1 | 1 |
| `regseq-config` | 5 | 5 | 0 |
| `regseq` (facade) | 19 | 19 | 0 |
| `regseq-macros` | 3 | 3 | 0 |
| `regseq-diagnostics` | 1 | 1 | 0 |
| `tests/e2e` | 5 | 1 | 4 |

## Per-file classification

### crates/cargo-regseq/tests/ (KEEP, all)

| File | Verdict | Rationale |
|---|---|---|
| cli.rs | KEEP | tests CLI parsing, pre-rustc |
| invoke.rs | KEEP | tests cargo command building, pre-rustc |
| toolchain.rs | KEEP | tests toolchain detection, pre-rustc |

### crates/regseq-driver/tests/ (mixed)

| File | Verdict | Rationale |
|---|---|---|
| args.rs | KEEP | dispatch logic, pre-rustc |
| config.rs | KEEP | callback config setup, pre-rustc-execution |
| mir_shape.rs | KEEP | asserts MIR shape under our flag set, HIR/MIR-boundary |
| smoke.rs | KEEP | symbol-export check, no IR assertion |
| callback_context.rs | KEEP | pure round-trip on a label enum |
| intrinsic_registry.rs | KEEP (edge) | HIR walk producing simple dictionary; survives redesign |
| framework_types.rs | KEEP (edge) | HIR walk producing simple dictionary |
| abstraction_types.rs | KEEP (edge) | HIR walk producing simple dictionary |
| attr_validation.rs | KEEP (edge) | HIR walk + report; produces simple lists |
| source_locations.rs | KEEP (edge) | HIR walk producing simple location records |
| driver_identity.rs | KEEP (edge) | HIR walk producing identity records |
| ordering.rs | KEEP (edge) | HIR walk producing ordering surface |
| clock.rs | KEEP (edge) | HIR walk producing ClockSurface |
| reset.rs | KEEP (edge) | HIR walk producing ResetSurface |
| pinctrl.rs | KEEP (edge) | HIR walk producing PinCtrlSurface |
| interrupts.rs | KEEP (edge) | HIR walk producing InterruptSurface |
| property.rs | KEEP (edge) | HIR walk producing PropertySurface |
| timed_delay.rs | KEEP (edge) | HIR walk producing TimedDelaySurface |
| logging.rs | KEEP (edge) | HIR walk producing LoggingSurface |
| synchronization.rs | KEEP (edge) | HIR walk producing SynchronizationSurface |
| traits.rs | KEEP (edge) | HIR walk producing TraitSurface |
| pm.rs | KEEP (edge) | HIR walk + trait surface for PM gate |
| register_access.rs | KEEP (edge) | HIR walk producing RegisterAccessSurface |
| mir_mapping.rs | VOID (edge) | tests rustc-Ty -> RegseqType translation; output type being redesigned |
| construction_pass1.rs | VOID | asserts produced RegseqModel structure -- post-HIR/MIR construction |
| construction_pass2.rs | VOID | asserts produced Body matches spec -- post-HIR/MIR construction |
| construction_try_folding.rs | VOID | asserts produced terminator shape (error_target population) -- post-HIR/MIR |
| construction_cross_crate.rs | VOID | asserts produced Function entries under synthetic module chains -- post-HIR/MIR |
| construction_module_deps.rs | VOID | asserts derived per-module dependency sets -- post-HIR/MIR |
| construction_intrinsic_recognition.rs | VOID | asserts IntrinsicCall terminator emission -- post-HIR/MIR |
| construction_register_metadata.rs | VOID | asserts IntrinsicMetadata::Register attachment -- post-HIR/MIR |
| construction_pinctrl_metadata.rs | VOID | asserts IntrinsicMetadata::PinCtrl attachment -- post-HIR/MIR |
| construction_log_metadata.rs | VOID | asserts IntrinsicMetadata::Log attachment -- post-HIR/MIR |
| construction_ordered_region.rs | VOID | asserts OrderedRegion records on Body -- post-HIR/MIR |
| construction_deref_collapse.rs | VOID | asserts deref-collapse rewrite on produced IR -- post-HIR/MIR |
| construction_switch_and_loops.rs | VOID | asserts produced Body for SwitchInt and loops -- post-HIR/MIR |
| pl011_construction.rs | VOID | end-to-end model construction with byte-equal JSON goldens -- post-HIR/MIR |
| model_emit.rs | VOID | tests JSON emission of constructed model -- post-HIR/MIR |
| source_to_regseq_model_03_driver_struct.rs | VOID | asserts produced RegseqModel structure -- post-HIR/MIR harness test |
| source_to_regseq_model_06_driver_constants.rs | VOID | same -- harness asserts model contents |
| source_to_regseq_model_07_helper_methods_a.rs | VOID | same -- harness asserts model contents |
| source_to_regseq_model_08_helper_methods_b.rs | VOID | same -- harness asserts model contents |
| source_to_regseq_model_09_lifecycle_callbacks.rs | VOID | same -- harness asserts model contents |
| source_to_regseq_model_10_serial_callbacks_a.rs | VOID | same -- harness asserts model contents |
| source_to_regseq_model_12_isr.rs | VOID | same -- harness asserts model contents |

`tests/support/regseq_model_harness.rs` is a helper module (no
`#[test]` items); flag for the audit but no annotation needed -- when
the harness's only consumers are `#[ignore]`'d, the harness becomes
dead code and either is preserved as reference for the rebuild or is
deleted alongside the consumers.

Proposed `#[ignore = "..."]` strings (uniform across files in this
section, varying only in the "replaced by" tail):

- VOID construction-pass / metadata / cross-crate / try-folding /
  switch-loops files:
  ```
  #[ignore = "post-HIR/MIR; covers what construction produces. Will be replaced by tests against MonoRust extraction + new RegseqModel construction. See docs/discussions/compiler-refactoring/, docs/discussions/monorust/overview.md, docs/discussions/models/regseq-model/spec.md."]
  ```
- VOID `pl011_construction.rs` and `model_emit.rs`:
  ```
  #[ignore = "post-HIR/MIR; end-to-end RegseqModel construction with JSON goldens. Will be replaced by an end-to-end test against MonoRust artifact + new RegseqModel + post-redesign goldens. See docs/discussions/monorust/overview.md."]
  ```
- VOID `source_to_regseq_model_*.rs` files:
  ```
  #[ignore = "post-HIR/MIR; harness-driven RegseqModel structural assertions. Will be replaced by harness tests against MonoRust + new RegseqModel. See docs/discussions/compiler-refactoring/."]
  ```
- VOID `mir_mapping.rs`:
  ```
  #[ignore = "post-HIR/MIR; tests rustc-Ty -> RegseqType translation. The boundary translator is rebornd as HIR/MIR -> MonoType under MonoRust; tests will land there. See docs/discussions/monorust/overview.md sections 6.3 and 7.1."]
  ```

### crates/regseq-driver/src/ inline `#[cfg(test)] mod tests` modules

| File | Verdict | Rationale |
|---|---|---|
| construction/cross_crate.rs | VOID | tests cross-crate construction helpers |
| construction/deref_collapse.rs | VOID | tests deref-collapse helpers |
| construction/id_allocator.rs | VOID | tests id allocation in construction |
| construction/intrinsics.rs | VOID | tests intrinsic-recognition helpers in construction |
| construction/log_metadata.rs | VOID | tests log-metadata extraction in construction |
| construction/ordered_region.rs | VOID | tests ordered-region recognizer in construction |
| construction/try_folding.rs | KEEP | pure-function tests over rustc types (`switch_triple_*`, `validate_from_residual_arg_*`, `is_unreachable_terminator_*`, `check_residual_root_*`, `try_markers_default_is_unresolved`); HIR/MIR-boundary |

Proposed `#[ignore = "..."]` for the VOID inline modules (apply to the
whole `mod tests { ... }` via `#[cfg(test)] #[ignore]`-style is not
possible at module level; instead annotate each `#[test]` fn or skip
the entire test module by gating its parent file). Recommend:

```
#[ignore = "post-HIR/MIR; tests construction-pipeline helpers in regseq-driver/src/construction/. Will be replaced under MonoRust + new RegseqModel. See docs/discussions/compiler-refactoring/."]
```

### crates/regseq-model/tests/ (mixed)

| File | Verdict | Rationale |
|---|---|---|
| smoke.rs | KEEP | crate import test; trivially survives |
| platform_parser.rs | KEEP (edge) | parses platform.toml; pre-construction input parsing, not HIR/MIR-derived |
| platform_properties.rs | KEEP (edge) | platform.toml property parsing; same |
| platform_schema.rs | KEEP (edge) | platform.toml schema struct construction; same |
| model_public_surface.rs | KEEP (edge) | re-export reachability check; pure compilation test |
| mutation_api_auto_verify.rs | VOID | tests RegseqModel mutation API contract; model is being redesigned |
| query_api.rs | VOID | tests RegseqModel query helpers on hand-built models; model being redesigned |
| lint_findings.rs | VOID | tests verifier on hand-built models; verifier and lints likely change |
| verify_phase_1.rs | VOID | tests verifier on hand-built models; phase verifier being reworked |
| verify_phase_2.rs | VOID | tests post-join verifier on hand-built models; same |

Proposed `#[ignore = "..."]`:

```
#[ignore = "post-HIR/MIR; tests RegseqModel mutation/query/verify behavior. Will be reborn against the redesigned RegseqModel. See docs/discussions/compiler-refactoring/, docs/discussions/models/regseq-model/spec.md."]
```

### crates/regseq-model/src/ inline `#[cfg(test)] mod tests` (all VOID)

All 26 inline test modules under `crates/regseq-model/src/{identity,ir,model,
verify}/*.rs`, `properties.rs`, `serialize.rs` test the data structures
of the current RegseqModel -- the exact surface being redesigned.
Verdict: VOID, all.

Proposed `#[ignore = "..."]` (apply to each `#[test]` within these
modules, or gate the parent `mod tests` block):

```
#[ignore = "post-HIR/MIR; tests current RegseqModel data structures and helpers. Surface is being redesigned. See docs/discussions/compiler-refactoring/, docs/discussions/models/regseq-model/spec.md."]
```

### crates/regseq-backends/tests/ + src/ (VOID, mostly)

| File | Verdict | Rationale |
|---|---|---|
| tests/smoke.rs | KEEP | crate import test |
| tests/c_ast_nodes.rs | VOID | tests C AST construction and rendering; backend redesigned with new RegseqModel |
| src/linux/body_index.rs (inline) | VOID | tests body-index helpers in the Linux backend |

Proposed `#[ignore = "..."]`:

```
#[ignore = "post-HIR/MIR; tests backend codegen artifacts. Will be reborn against the redesigned RegseqModel and a refreshed C AST. See docs/discussions/compiler-refactoring/."]
```

### crates/regseq-analysis/tests/ (VOID, mostly)

| File | Verdict | Rationale |
|---|---|---|
| smoke.rs | KEEP | crate import test |
| readiness.rs | VOID | tests analysis on hand-built models; analysis surface tied to current model |

Proposed `#[ignore = "..."]`:

```
#[ignore = "post-HIR/MIR; tests analysis output against current RegseqModel. Analysis surface will be reborn under the redesigned model. See docs/discussions/compiler-refactoring/."]
```

### crates/regseq-config/tests/ (KEEP, all)

| File | Verdict | Rationale |
|---|---|---|
| smoke.rs | KEEP | crate import |
| emit.rs | KEEP | tests EmitSpec parsing; pre-rustc config |
| options.rs | KEEP | tests CompilerOptions parsing; pre-rustc config |
| param.rs | KEEP | tests RegseqParam parsing; pre-rustc config |
| types.rs | KEEP | tests TargetOs/LogLevel/etc. parsing; pre-rustc config |

### crates/regseq/tests/ (KEEP, all)

These are facade-surface tests: every test verifies that types and
trait surfaces from the `regseq` facade crate are publicly exported,
type-check as struct fields, and accept the documented user-facing
forms. They do not consume HIR/MIR -- they are user-facing API tests
for the facade itself, which is the *input* to driver specs. The
facade survives the redesign.

19 files, all KEEP: `clock.rs`, `delay.rs`, `interrupt.rs`,
`intrinsics.rs`, `lifecycle.rs`, `logging.rs`, `mmio.rs`, `pinctrl.rs`,
`pm.rs`, `poll.rs`, `power.rs`, `property.rs`, `register_block.rs`,
`register_types.rs`, `reset.rs`, `serial.rs`, `shared_foundation.rs`,
`smoke.rs`, `synchronization.rs`.

### crates/regseq-macros/tests/ (KEEP, all)

| File | Verdict | Rationale |
|---|---|---|
| smoke.rs | KEEP | crate compiles |
| ordered.rs | KEEP | proc-macro expansion test; pre-HIR |
| register_bitfields.rs | KEEP | proc-macro expansion test; pre-HIR |

### crates/regseq-diagnostics/tests/ (KEEP)

| File | Verdict | Rationale |
|---|---|---|
| smoke.rs | KEEP | crate import |

### tests/e2e/tests/ (mixed; all currently `#[ignore]`'d)

Note: every test under `tests/e2e/tests/` is already marked
`#[ignore]` because it runs a full cargo-build cycle. The audit
question is whether to additionally tag them with the
post-HIR/MIR rationale (so future readers see the redesign-driven
disposition rather than the cost-driven one).

| File | Verdict | Rationale |
|---|---|---|
| driver_e2e.rs | KEEP | asserts wrapper reaches `after_analysis` for primary crate; routing/dispatch test, HIR/MIR-boundary |
| pl011_spec_slice.rs | KEEP (edge) | asserts wrapper reaches `after_analysis` on PL011 slice plus parses platform.toml; routing + config |
| platform_file_boundary.rs | KEEP (edge) | tests Layer 2 platform-file parser against fixture corpus; pre-HIR/MIR config concern |
| pl011_driver_emission.rs | VOID | asserts emitted Linux artifact contents on disk; post-HIR/MIR backend output |
| pl011_linux_module_host_build_proof.rs | VOID | host-builds emitted PL011 Linux module; post-HIR/MIR backend output |

Proposed `#[ignore = "..."]` for VOID e2e files (replacing the
existing cost-driven ignore message):

```
#[ignore = "post-HIR/MIR; asserts produced Linux backend artifacts. Will be reborn against the redesigned backend driven by new RegseqModel. Run with --ignored after that refresh. See docs/discussions/compiler-refactoring/."]
```

For KEEP-but-currently-ignored e2e files, the current `#[ignore]`
reason (cost) stays unchanged; no rewrite needed.

## Edge cases

The user flagged five categories explicitly. Each is argued both
directions and a default is recommended.

### 1. `tests/mir_mapping.rs` -- rustc Ty -> RegseqType translation

**Argument for KEEP:** The function under test is the boundary
translator between rustc types and our internal type representation.
The boundary itself is stable: every redesign has *some* mapping from
rustc HIR/MIR types into the compiler's first internal type
representation. The test invariants ("a `ty::Uint(U32)` becomes our
`U32`") survive any reasonable redesign.

**Argument for VOID:** The output type, `RegseqType`, is part of the
RegseqModel surface that is being redesigned. Under MonoRust this
boundary translator becomes HIR/MIR -> `MonoType`, with a different
output enum and likely different variant set (closures, raw pointers,
function pointers, all of which are absent from `RegseqType` but
present in `MonoType` per `monorust/overview.md` section 6.3). Tests
that assert specific `RegseqType` outputs are testing a surface that
will not exist post-redesign; they cannot be carried forward verbatim.

**Recommendation: VOID.** The strict-rule application wins here.
The new boundary translator under MonoRust is functionally similar
but its output enum differs in shape, set of variants, and naming.
Tests against the old output cannot be ported without rewriting; the
honest disposition is to disable now and write fresh tests under
MonoRust extraction.

### 2. Registry-building tests: `intrinsic_registry`, `framework_types`, `abstraction_types`

**Argument for KEEP:** These three tests cover HIR-walk extractors
that produce simple dictionaries indexing items by `#[rseq::*]`
attribute presence (`#[rseq::intrinsic]`, `#[rseq::framework_type]`,
`#[rseq::abstraction]`). The dictionaries are not transformed IR --
they are simple symbol/name maps. They survive the redesign mostly
intact: MonoRust extraction will need exactly the same registries
(under whatever name) to drive intrinsic recognition and abstraction
classification. The user explicitly listed "HIR-walk-only registries
that produce simple dictionaries reflecting attribute presence" as a
KEEP signal in the methodology.

**Argument for VOID:** Strict reading -- the registries are
*output produced from HIR* and therefore violate the narrowest reading
of the rule. Their internal types (`IntrinsicRegistry`,
`FrameworkTypeRegistry`, `AbstractionTypeRegistry`) live in
`regseq-driver/src/extract/` and may evolve.

**Recommendation: KEEP.** Aligns with the user's stated KEEP signal.
The registries are forward-compatible; even if they get renamed or
moved, the test invariants ("every `#[rseq::intrinsic]` annotation in
the facade is discovered") transfer. Voiding these would lose
genuine regression coverage on a stable, load-bearing surface.

### 3. `tests/construction_try_folding.rs` -- per-test classification

I read the file's intent: every test compiles a fixture, runs the
recognizer plus construction, and asserts on the constructed `Body`'s
terminators (e.g., `error_target` is populated, dropped statements are
absent, originating Call's `target` rewires to the fold's
`continue_target`). All of these are post-HIR/MIR construction
assertions. The recognizer-side invariant ("a `?` site exists in MIR
and was recognized") is asserted only via the post-construction Body
shape, not as a pure recognizer test.

**Recommendation: VOID, file-level.** No mixed classification within
the file -- every test exercises post-HIR/MIR construction.

The HIR/MIR-boundary recognizer tests live in `try_folding.rs`
itself (the inline `#[cfg(test)] mod tests` block at line 833+).
Those are KEEP, as covered in the inline-modules section above.

### 4. `try_folding.rs` inline tests -- explicit confirmation

Every test in the inline block is a pure function over rustc types
or over plain enum/data values:

- `switch_triple_*` -- pure functions over `mir::SwitchTargets` values
- `validate_from_residual_arg_*` and `check_residual_root_*` -- pure
  functions over synthesized `mir::Operand` and projection slices
- `is_unreachable_terminator_*` -- pure function over
  `mir::TerminatorKind` values
- `try_markers_default_is_unresolved` -- pure-data test on
  `TryMarkers::default()`

**Recommendation: KEEP, all.** These are the canonical example of
HIR/MIR-boundary tests under the audit rule -- pure pattern-matching
logic over rustc-side types.

### 5. `tests/e2e/` files -- post-HIR/MIR or routing?

Three of the five e2e files (`driver_e2e.rs`, `pl011_spec_slice.rs`,
`platform_file_boundary.rs`) only assert that the wrapper reaches
`after_analysis` for the primary crate, that dependencies route
through passthrough, and that `platform.toml` parses. These are
pre-/at-boundary assertions: they do not assert produced model or
backend artifacts.

The other two (`pl011_driver_emission.rs`,
`pl011_linux_module_host_build_proof.rs`) assert produced Linux
artifacts on disk -- post-HIR/MIR backend output.

**Recommendation:** Per-file as listed in the e2e section above.
KEEP the routing-and-parsing trio with their existing
cost-driven `#[ignore]`. VOID the two artifact-producing tests with a
redesign-driven `#[ignore]` rationale.

### 6. `regseq-model/tests/platform_parser.rs`, `platform_properties.rs`, `platform_schema.rs`

These test the Layer 2 platform-file parser (TOML -> typed Rust
structs). They do not consume HIR/MIR; they are a separate input
parser comparable to `regseq-config`.

**Recommendation: KEEP.** Pre-rustc input parsing is explicitly in
the KEEP signal set. The parser will continue to feed the redesigned
RegseqModel.

### 7. `regseq-model/tests/model_public_surface.rs`

A compile-time check that `regseq_model::model::*` re-exports remain
reachable via external paths. The module shape will change under
redesign, but a re-export reachability test is functionally a smoke
test.

**Recommendation: KEEP.** Once the redesigned model lands, this test
file will likely be rewritten or replaced; in the interim it does no
harm and may catch accidental visibility regressions.

### 8. `regseq-model/src/` inline `#[cfg(test)]` modules (all 26 files)

Every inline test in `regseq-model/src/identity/`, `ir/`, `model/`,
`verify/`, `properties.rs`, and `serialize.rs` exercises types that
are exactly the redesign target. These are not registry-style
dictionaries; they are the data structures being redesigned.

**Recommendation: VOID, all.** Already reflected in the per-file
section.

### 9. `tests/support/regseq_model_harness.rs`

Helper module shared by the seven `source_to_regseq_model_*.rs`
files. Once those consumers are `#[ignore]`'d, the harness becomes
dead reference code. It is not a test file -- no `#[test]` attribute
to annotate.

**Recommendation:** No annotation needed. Surface for retirement
alongside the consumers when the redesign criterion is met.

## Snapshot fixture inventory

Snapshot fixtures referenced by VOID tests, dormant once the tests
are `#[ignore]`'d (no annotation possible -- they are JSON files,
not Rust code):

```
tests/fixtures/pl011-full-spec/expected/regseq-model.json
tests/fixtures/pl011-full-spec/expected/ir/configure_line_control.json
tests/fixtures/pl011-full-spec/expected/ir/configure_line_control.actual.json
tests/fixtures/pl011-full-spec/expected/ir/disable_fifo.json
tests/fixtures/pl011-full-spec/expected/ir/disable_uart.json
tests/fixtures/pl011-full-spec/expected/ir/enable_clock_if_present.json
tests/fixtures/pl011-full-spec/expected/ir/enable_clock_if_present.actual.json
tests/fixtures/pl011-full-spec/expected/ir/flush_rx_fifo.json
tests/fixtures/pl011-full-spec/expected/ir/flush_rx_fifo.actual.json
tests/fixtures/pl011-full-spec/expected/ir/program_polling_port.json
tests/fixtures/pl011-full-spec/expected/ir/send_break.json
tests/fixtures/pl011-full-spec/expected/ir/tx_ready.json
```

Three `.actual.json` siblings already exist on disk -- these are the
drift artifacts from the Step 1.5 cleanup-pass bisection (left
behind because that diagnostic restored the helper but did not clean
the produced files). They can be deleted as part of the same atomic
annotation commit.

The directory `tests/fixtures/pl011-full-spec/expected/ir/` and the
`expected/regseq-model.json` file should remain in the working tree
until the retirement commit -- they are reference material for the
rebuild, not source of test signal. After retirement (criterion:
MonoRust + new RegseqModel pipeline produces semantically-equivalent
output for the PL011 fixture), they can be deleted.

## Open questions for review

1. **Recommendation for `mir_mapping.rs`:** I lean VOID. If you have
   strong intuition that the `RegseqType` -> `MonoType` rebirth will
   preserve enough variant overlap to make the existing tests
   straight-forwardly portable, you may prefer KEEP. My reading is
   that the variant sets diverge enough (closures, raw pointers,
   function pointers in `MonoType` vs. their absence in
   `RegseqType`) that disable-and-rewrite is honest.

2. **Inline `#[cfg(test)] mod tests` annotation mechanics.** Rust
   does not support `#[ignore]` at module level. Two options for the
   VOID inline modules:
   - Annotate every individual `#[test]` fn within (low edit-volume
     for files like `id_allocator.rs` (1 test), high edit-volume for
     `log_metadata.rs` (8 tests) and `model/*.rs` files (many
     tests).
   - Wrap the entire `mod tests { ... }` in `#[cfg(any(test, ignore_post_hir_mir))]`
     or similar feature gate, then never enable that feature.
   - Or simply prepend each `mod tests` with `#[cfg(feature =
     "post-hir-mir-tests")]` and leave the feature un-declared.

   Recommendation: per-`#[test]` annotation for visibility (each
   ignored test self-documents via its `#[ignore = "..."]` string).
   The volume is manageable -- approximately 270 inline VOID tests
   across 33 files, all sharing the same ignore string template.

3. **e2e file annotation overlap.** The artifact-emission e2e files
   (`pl011_driver_emission.rs`,
   `pl011_linux_module_host_build_proof.rs`) currently carry
   `#[ignore]` for cost reasons. The redesign-driven rationale is
   stronger. Recommendation: replace the existing ignore message,
   not stack a second one.

4. **Edge case 6 (platform-file parser tests).** I recommended KEEP.
   If your view is that the platform parser will be redesigned
   alongside RegseqModel (because the parser produces typed
   structures that feed the model), VOID those three files instead.
   I do not see evidence in `docs/discussions/` that the platform
   parser is in scope for the redesign, so I lean KEEP.

5. **Harness file disposition.** Confirming: leave
   `tests/support/regseq_model_harness.rs` as-is (no annotation,
   no deletion) until the retirement commit.

6. **Retirement criterion alignment.** This audit assumes the
   criterion you proposed earlier: the dormant pipeline
   (`construction/`, `extract/<output-producing-modules>`,
   `model/`, JSON serialization, snapshot fixtures, all VOID tests)
   is retired in a single deletion commit when the MonoRust + new
   RegseqModel pipeline produces semantically-equivalent output for
   the PL011 fixture. Confirm that criterion before annotation
   work proceeds; disabled tests are read against this contract.
