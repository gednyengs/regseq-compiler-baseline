**Q1**

Phase 1 gaps:
- Add/update macro parser tests for the actual register-block grammar. The current parser accepts `imsc: ReadWrite { ... }`, not `ReadWrite<...>` syntax, and square brackets are already used for reserved counts at [register_block.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:164). The PL011 fixture uses that current form at [registers.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs:161).
- If emitting metadata as constants, the per-register module emission path must change, not just parsing. It currently emits `Register`, `OFFSET`, `REG`, field constants, and `RegisterOf` at [register_block.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:329).

Phase 2 gaps:
- Add raw artifact and surface normalization coverage. `RawRegisterFieldDecl` has no shadow/reset fields at [raw_artifact.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/raw_artifact.rs:89), and `RegisterBlockField` is constructed without such fields at [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:399).
- Add driver-model construction/exports. `DriverStruct` has no `shadow_state` field at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/driver.rs:29), and model reexports live at [mod.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/mod.rs:21).
- Add verifier integration, not only struct fields. Phase 1 and 2 verifier entry points are fixed lists at [phase_1.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/phase_1.rs:51) and [phase_2.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/phase_2.rs:59). Cross-item declared-ID/source bookkeeping currently knows driver resources, properties, and user data, but not shadow state, at [cross_item.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/cross_item.rs:73) and [cross_item.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/cross_item.rs:143).
- Do not promise lint warnings unless the lint framework is extended. Existing lints only cover required properties with defaults at [lints.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/lints.rs:89) and [rules.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/lints/rules.rs:17).

Phase 3 gaps:
- RegisterIndex consumers must change, not just the index. Metadata resolution attaches `IntrinsicMetadata::Register` at [register_metadata.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:338), `is_register_intrinsic` enumerates register intrinsics at [register_metadata.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:452), and access validation is hard-coded at [register_metadata.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:569).
- Add construction error variants and display text for invalid shadow targets. `ConstructionError` is defined at [construction.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction.rs:127), with display arms starting at [construction.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction.rs:451).
- The plan should state how macro-emitted metadata is discovered. `build_register_block` currently derives entries from type shape only at [registers.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/registers.rs:21).

Phase 4 gaps:
- Add `RegisterForceRead` to all intrinsic enum match sites. `RegseqIntrinsic` variants, canonical names, and parser are hard-coded at [intrinsic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:20), [intrinsic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:65), and [intrinsic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:96).
- `modify` is not currently an intrinsic. `Mmio::modify` is an ordinary method body that calls `read` and `todo!` at [mmio.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:72), while only read/read_field/write/write_field carry intrinsic attributes at [mmio.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:37).
- Semantic lowering has more touch-points than named. Register calls are classified at [call_classifier.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs:20), lowered at [call_lowering.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:36), validated at [validation.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/validation.rs:157), and projected at [projection.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/projection.rs:147).
- Portable lowering maps register intrinsics to wrapper methods at [from_model.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/from_model.rs:742); `RegisterForceRead` must be handled there too.

Phase 5 gaps:
- Add backend-neutral plan data. `RegisterObligations` and `RegisterFieldInfo` currently carry no shadow information at [plan.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/plan.rs:322), and `from_model` copies only name, width, and offset at [from_model.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/from_model.rs:521).
- Linux shared struct planning must consume that data. Shared fields are built at [planner.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/planner.rs:140), while header emission simply prints planned fields at [emitter.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emitter.rs:118).
- Probe initialization needs a placement rule. `d = devm_kzalloc(...)` occurs at [emit_module.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_module.rs:694), before resource acquisition at [emit_module.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_module.rs:717).
- Update both semantic and legacy behavior emitters, or explicitly retire the latter. Semantic register reads/writes emit at [emit_semantic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:405) and [emit_semantic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:637); legacy behavior emits similar nodes at [emit_behavior.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_behavior.rs:106).
- Zephyr has no backend plan file to stub; it is only a module stub at [zephyr.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/zephyr.rs:1). The plan should still say Zephyr remains unsupported or add an explicit NotImplemented path if backend dispatch reaches it.

Phase 6 gaps:
- PL011 has more manual-shadow sites than the plan appears to cover: state declaration at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:65), commit helper at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:108), mask helper at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:115), interrupt helpers at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:236), init path at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:434), and close path at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:476).

Phase 7 gaps:
- Update serialized model format expectations. `RegseqModel` carries `format_version` at [regseq_model.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/regseq_model.rs:56), and whole-model serialization is derived at [serialize.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/serialize.rs:70).
- Add negative and non-shadow regression tests: non-shadow register reads/writes must still lower through existing hardware paths at [emit_semantic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:405).

**Q2**

Ordering correctness: Phase 2 can land before Phase 1 only if all new model fields default to `shadow = false` and `reset_value = None` everywhere `RegisterBlockField` is constructed, including normalization at [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:399). Phase 3 cannot meaningfully land before Phase 1 metadata exists and Phase 2 stores it.

Partial-landing risk: Phase 4 is not safely partial unless shadow rewriting is gated off. Adding semantic variants forces every emitter/validator/projector match to compile in the same change, including [validation.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/validation.rs:157) and [emit_semantic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:637). Fixture conversion should wait until Phase 4 and 5 are both functional.

Fixture-preview recommendation: yes, but use small preview fixtures rather than repeatedly churning PL011. Add a minimal shadow-register fixture after metadata/construction, then a backend fixture after shadow IR emission, then convert PL011 once the end-to-end path is stable.

**Q3**

3.1 Recommendation: use `#[rseq::register_entry(shadow = true, reset_value = 0)]`.
This is cleaner because the existing grammar already uses `name: Access { ... }` and square brackets for reserved counts at [register_block.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:164). An attribute is also easier to extend without making register entries visually noisy in the fixture source.

3.2 Recommendation: keep authored metadata on `RegisterBlockField`, and derive a separate compiler-managed `ShadowState` category.
`RegisterBlockField` is the right source of truth for whether a register is shadowed at [register.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:88). A separate `DriverStruct.shadow_state` is still useful for backend storage planning because `DriverStruct` is where generated driver-owned state belongs at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/driver.rs:29).

3.3 Recommendation: use a standalone rewrite pass before backend emission, not a branch buried in `call_lowering`.
The lowering stack already separates classification, lowering, validation, and projection at [call_classifier.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs:20) and [call_lowering.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:36). Shadow behavior is a cross-cutting rewrite over resolved register metadata, so it should be explicit and testable.

3.4 Recommendation: use explicit IR variants, not one parameterized `ShadowOp`.
The existing semantic IR uses explicit register read/write variants, with value-producing reads in [expr.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/semantic/expr.rs:50) and side-effecting writes in [stmt.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/semantic/stmt.rs:73). Explicit variants preserve exhaustive-match checking in validators and emitters.

3.5 Recommendation: keep an explicit IMSC hardware write to zero in `init()`.
Generated shadow initialization only initializes driver storage after `devm_kzalloc`; it does not mask hardware. Current PL011 init calls `mask_all_interrupts` at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:441), which writes IMSC through the commit helper at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:108). Keep that hardware resync behavior; do not replace it with `force_read`.

3.6 Recommendation: do not combine phases 1-3 and 4-5 into two large harness tasks.
The repo’s task-spec guidance asks for narrow, testable tasks within one crate or a tight crate pair at [task-spec-workflow.md](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/.harness/task-spec-workflow.md:120). Use phase-sized tasks, with Phase 2+3 optionally paired only if the model/construction invariant cannot be tested independently.

**Q4**

Phase 1: mostly accurate at 1-2 days if the attribute syntax is chosen. If trailing bracket syntax is kept, raise to 1.5-2.5 days because it fights existing grammar at [register_block.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:164).

Phase 2: underestimated. Adjust from 1-2 to 2-3 days because it touches model structs, raw artifacts, normalization, phase verifiers, cross-item bookkeeping, serialization goldens, and likely source-location handling.

Phase 3: underestimated. Adjust from 1-2 to 2-3 days because RegisterIndex changes cascade into metadata resolution, pass2 intrinsic handling, access validation, and construction errors.

Phase 4: underestimated. Adjust from 3-4 to 4-6 days. The plan misses that `modify` is not an intrinsic today at [mmio.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:72), and the semantic pipeline has classifier, lowerer, walker, validator, projector, and emitter dependencies.

Phase 5: underestimated. Adjust from 2-3 to 3-4 days because portable plan/from_model work is required before Linux can emit the shared field and initialization path.

Phase 6: accurate at 0.5-1 day, assuming compiler support is already complete.

Phase 7: slightly underestimated. Adjust from 1-2 to 1.5-2.5 days due to format/golden churn and cross-crate regression coverage.

Overall adjusted estimate: about 14-22 days, not 9.5-15.5.

**Q5**

I concur that Phase 4 is the highest risk and Phase 2 is secondary, but Phase 5 is close behind because the Linux backend consumes a portable lowering plan rather than the raw model at [planner.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/planner.rs:99).

Missed risks:
- The plan does not distinguish Regseq IR intrinsic calls from the semantic MIR-derived lowering pipeline. Both exist, and shadow rewrite placement must name the exact IR boundary.
- `modify` semantics are under-specified because current `Mmio::modify` is not a real intrinsic.
- Compiler-generated shadow declarations may interact with source-location coverage. Normalization intentionally treats compiler-generated locations specially at [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:271).
- Golden-file regeneration will be noisy because model serialization is derived and includes format version fields.
- Zephyr unsupported behavior is not stated.

**Q6**

Still too abstract to task-spec directly:
- Exact macro syntax and emitted metadata contract.
- Exact `ShadowState` model shape, IDs, naming, source provenance, and reset-value typing.
- Exact rewrite input/output IR and pass placement.
- Exact handling of `modify`, `write_field`, and `force_read` for shadowed and non-shadowed registers.
- Portable lowering-plan contract for shadow fields and initialization.
- Zephyr behavior.
- Test/golden regeneration order and expected artifact list.

**Q7**

Path-A user-data initialization is not a hard architectural dependency if shadow state is a separate compiler-managed model category. Current user-data acceptance applies to `DriverStruct.user_data`, and the driver verifier checks user-data fields directly at [driver_struct.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/driver_struct.rs:212). The current PL011 manual `imsc_shadow: u32` is a problem only because it is user-authored driver state at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:65), which Path A is meant to allow for zero-safe scalars per [user-data-initialization.md](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/models/regseq-model/user-data-initialization.md:131).

So shadow-register implementation can proceed without Path A if the fixture conversion removes manual `imsc_shadow` before relying on current construction. Path A is a sequencing convenience for the existing fixture baseline, not a hard dependency of compiler-generated shadow state.

PLAN-VERDICT: needs-plan-amendment

The plan is directionally sound, but not ready for task-spec decomposition. It misses several concrete compiler touch-points, especially normalization/verifier bookkeeping, RegisterIndex consumers, semantic lowering consumers, portable backend planning, and the fact that `modify` is not currently an intrinsic.

Amendments:
- Specify attribute syntax and metadata emission/discovery.
- Define exact `ShadowState` model shape and verifier/source-location behavior.
- Add all RegisterIndex, intrinsic enum, semantic lowering, validation, projection, and backend plan touch-points.
- Specify `modify`, `write_field`, and `force_read` lowering precisely.
- Add portable lowering-plan changes before Linux-specific work.
- Treat Path A as optional sequencing, not a hard prerequisite.
- Revise effort estimate to roughly 14-22 engineering days.
