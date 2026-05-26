# Bridge coverage ledger (derived -- DO NOT hand-edit)

Authoritative per-row coverage truth, DERIVED by `scripts/python/coverage_drift.py` (regseq-driver-79, G1) from the bridge-coverage matrix + runner sidecar + legacy fixtures. Regenerate with `uv run python scripts/python/coverage_drift.py --write-ledger`; do not edit by hand. The matrix remains the human-owned prose source.

CAVEAT: coverage of the enumerated rows is NOT a proof the enumeration is exhaustive over the Rust surface.

| id | section | axis | truth | matrix_current_status | evidence | reason |
| -- | ------- | ---- | ----- | --------------------- | -------- | ------ |
| BD-001 | 2 | coverage | validated-legacy | covered | no_op.rs |  |
| BD-002 | 2 | coverage | validated-legacy-universal | covered | many |  |
| BD-003 | 2 | coverage | validated-legacy-universal | covered | many |  |
| BD-004 | 2 | coverage | validated-legacy-universal | covered | many |  |
| BD-005 | 2 | coverage | validated-legacy-universal | covered | many |  |
| BD-006 | 2 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| BD-007 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/BD-007.rs |  |
| BD-008 | 2 | coverage | validated-legacy | covered | trait_object.rs |  |
| BD-009 | 2 | coverage | validated-legacy | covered | instance_completeness.rs |  |
| BD-010 | 2 | coverage | validated-legacy | covered | foreign_rendering.rs |  |
| CT-001 | 4 | coverage | validated-legacy-universal | covered | every fixture via helper (crates/test-ir-fixtures/src/lib.rs:188-198) |  |
| CT-002 | 4 | coverage | out-of-scope | covered | Section 6 RJ-008 cross-ref | rejection-path covered |
| CT-003 | 4 | coverage | validated-legacy-universal | covered | every fixture |  |
| CT-004 | 4 | coverage | needs-validation | uncovered |  |  |
| CT-005 | 4 | coverage | validated-legacy | covered | diag_logging.rs only (other fixtures are plain #![no_std]) |  |
| CT-006 | 4 | coverage | validated-legacy-universal | covered | every fixture |  |
| CT-007 | 4 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| CT-008 | 4 | coverage | validated-legacy-universal | partial | implicit |  |
| CT-009 | 4 | coverage | validated-legacy | covered | diag_logging.rs |  |
| CT-010 | 4 | coverage | validated-legacy | partial | Linkage::Default ubiquitous; NoMangle / Foreign covered via foreign_rendering.rs; Weak uncovered |  |
| CT-011 | 4 | coverage | validated-runner | partial | sidecar:rows.toml |  |
| CT-012 | 4 | coverage | validated-legacy | covered | diag_logging.rs (LogArg<'_>) |  |
| FF-001 | 2 | coverage | validated-legacy-universal | covered | every fixture |  |
| FF-002 | 2 | coverage | validated-legacy | covered | diag_logging.rs (log intrinsics) |  |
| FF-003 | 2 | coverage | validated-legacy | covered | trait_object.rs |  |
| FF-004 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-004.rs |  |
| FF-005 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-005.rs |  |
| FF-006 | 2 | coverage | validated-legacy | covered | trait_object.rs |  |
| FF-007 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-007.rs |  |
| FF-008 | 2 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FF-009 | 2 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FF-010 | 2 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FF-011 | 2 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| FF-012 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-012.rs |  |
| FF-013 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-013.rs |  |
| FF-014 | 2 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FF-015 | 2 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FF-016 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-016.rs |  |
| FF-017 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FF-017.rs |  |
| FT-001 | 3 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| FT-002 | 3 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| FT-003 | 3 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FT-003.rs |  |
| FT-004 | 3 | coverage | needs-validation | uncovered |  |  |
| FT-005 | 3 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| FT-006 | 3 | coverage | validated-legacy | covered | trait_method_resolution.rs |  |
| FT-007 | 3 | coverage | validated-legacy | covered | trait_default_method.rs |  |
| FT-008 | 3 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| FT-009 | 3 | coverage | needs-validation | uncovered |  |  |
| FT-010 | 3 | coverage | validated-legacy | partial | instance_completeness.rs (Wrap<u32>, Wrap<u64>) |  |
| FT-011 | 3 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FT-011.rs |  |
| FT-012 | 3 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FT-012.rs |  |
| FT-013 | 3 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FT-013.rs |  |
| FT-014 | 3 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/FT-014.rs |  |
| FT-015 | 3 | coverage | validated-legacy | covered | trait_object.rs |  |
| FT-016 | 3 | coverage | out-of-scope | blocked-bridge-rejects-by-design | -- | blocked-bridge-rejects-by-design |
| FT-017 | 3 | coverage | needs-validation | partial |  |  |
| FT-018 | 3 | coverage | validated-legacy | covered | instance_completeness.rs, playground |  |
| FT-019 | 3 | coverage | needs-validation | covered |  |  |
| FT-020 | 3 | coverage | validated-legacy | covered | diag_logging.rs |  |
| FT-021 | 3 | coverage | needs-validation | partial |  |  |
| IK-001 | 2 | coverage | validated-legacy-universal | covered | every fixture |  |
| IK-002 | 2 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| IK-003 | 2 | coverage | validated-legacy | covered | instance_completeness.rs (FLAG) |  |
| IK-004 | 2 | coverage | validated-legacy | covered | instance_completeness.rs (LIMIT), playground (Reg::REG_0) |  |
| IK-005 | 2 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| IK-006 | 2 | coverage | validated-legacy | covered | enum_round_trip.rs, instance_completeness.rs (State) |  |
| IK-007 | 2 | coverage | validated-legacy | covered | instance_completeness.rs (Pair) |  |
| IX-001 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-002 | 5 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| IX-003 | 5 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| IX-004 | 5 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/IX-004.rs |  |
| IX-005 | 5 | coverage | validated-legacy | covered | closure_call_sig_sync.rs |  |
| IX-006 | 5 | coverage | validated-legacy | covered | fn_ptr_translation_sig_sync.rs |  |
| IX-007 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-008 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-009 | 5 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| IX-010 | 5 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/IX-010.rs |  |
| IX-011 | 5 | coverage | validated-legacy | covered | trait_default_method.rs |  |
| IX-012 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-013 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-014 | 5 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/IX-014.rs |  |
| IX-015 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-016 | 5 | coverage | needs-validation | covered |  |  |
| IX-017 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-018 | 5 | coverage | validated-legacy-universal | covered | implicit via driver-61 debug_assert |  |
| IX-019 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-020 | 5 | coverage | needs-validation | covered |  |  |
| IX-021 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-022 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-023 | 5 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| IX-024 | 5 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| IX-025 | 5 | coverage | validated-legacy | covered | instance_completeness.rs, trait_object.rs |  |
| OP-001 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| OP-002 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| OP-003 | 1 | coverage | validated-legacy-universal | partial | many |  |
| OP-003-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| OP-003-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| OP-003-C | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| OP-003-D | 1 | coverage | validated-legacy-universal | covered | many (e.g., 42_u32) |  |
| OP-003-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| OP-003-F | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| OP-003-G | 1 | coverage | validated-legacy | partial | diag_logging.rs (string literal -- ConstMemoryStatic-backed) |  |
| OP-003-G-1 | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| OP-003-G-2 | 1 | coverage | validated-legacy | covered | diag_logging.rs (string-literal bytes) |  |
| OP-003-G-3 | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| OP-003-H | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| OP-003-I | 1 | coverage | out-of-scope | blocked-bridge-rejects-by-design | enum-constant value. Reclassified 2026-05-23 (was uncovered): the only source route to a type-level enum value-tree is a const-generic argument (W<V> where V: E is an enum const), which the bridge rejects by design at translate_generic_args (crates/regseq-driver/src/bridge/ty.rs:743-748, UnsupportedConst -- cross-ref Section 6 RJ-029) and at reverse materialisation (crates/regseq-driver/src/bridge/body/materialise.rs:673-684, coordinated gap with driver-37). More durably, Const::EnumValue is deliberately excluded from type-level admissibility -- only scalar consts (Bool/Char/Int/Uint/Float) may be a GenericArg::Const per crates/regseq-model/src/const_value.rs:54-64. Enum/non-scalar const-generics are out of scope for #![no_std] driver specs. Decision + analysis: .harness/audits/2026-05-23-c1-enum-const-eval-plan.md | blocked-bridge-rejects-by-design |
| OP-003-J | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| OP-003-K | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| PL-001 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| PL-002 | 1 | coverage | validated-legacy | covered | trait_object.rs (via &self deref) |  |
| PL-003 | 1 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs (_1.0), instance_completeness.rs (p.x) |  |
| PL-004 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| PL-005 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| PL-006 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| PL-007 | 1 | coverage | validated-legacy | covered | enum_round_trip.rs (via enum match path) |  |
| RJ-001 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-002 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-003 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-004 | 6 | coverage | out-of-scope | uncovered | uncovered (no known surface from #![no_std] source) | no known surface from #![no_std] source |
| RJ-005 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-006 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-007 | 6 | coverage | out-of-scope | uncovered | uncovered (requires coroutine source -- blocked by RJ-016) | requires coroutine source -- blocked by RJ-016 |
| RJ-008 | 6 | coverage | out-of-scope | covered | covered (implicitly via helper enforcing panic=abort; would fire under default panic) | implicitly via helper enforcing panic=abort; would fire under default panic |
| RJ-009 | 6 | coverage | validated-runner | covered | regseq-driver-72 sub-task C path C.2.a.i (committed as a positive row in rows.toml). Reframed from "Param rejection" to positive-row coverage of the bridge bucket-4 carve-out at crates/regseq-driver/src/bridge/mod.rs:1644-1658. Semantic shift rationale: any Param-bearing signature makes the Item generic, which the four-bucket validator routes to bucket 3 (skipped at bridge/mod.rs:1584) before the carve-out, and trait-method Self Params are skipped at bridge/mod.rs:1600-1606; the carve-out flag is also kind-agnostic (records signature_non_rigid: bool, not Param vs Alias), so no source shape surfaces a NonRigidTyKind::Param rejection through the current oracle (F71's 3 probes + F72's 4th probe confirm this). The committed snippet crates/bridge-coverage-runner/snippets/RJ-009.rs (private uncalled -> impl Fn(i32) -> i32) drives the carve-out's canonical Alias(Opaque) path, setting signature_non_rigid = true; the new SignatureNonRigid inspection predicate reads that flag from the in-process model. Full disposition in .harness/expectations/regseq-driver-72.md |  |
| RJ-010 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-011 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-012 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-013 | 6 | coverage | validated-runner | covered | regseq-driver-82: RECLASSIFIED rejected->positive **covered-by-reveal** (cf. FF-005's covered-by-rewrite). The A1 alias folder (crates/regseq-driver/src/bridge/ty.rs) gained a body-context-gated Alias(Opaque) arm that reveals the opaque to its concrete hidden type via normalize_erasing_regions(fully_monomorphized, ..) (PostAnalysis); the revealed closure / primitive bridges through the existing rigid arms. The reveal is scoped to per-Instance body materialisation via the RevealOpaqueScope RAII guard at materialise_function_body_inner AND construct_closure_call_instances, and never fires during identity-signature translation, so RJ-009's bucket-4 carve-out is preserved. A reachable such fn is reactively instantiated (opaque revealed) and its now-stale signature_non_rigid flag is cleared by reconcile_revealed_signature_flags before completeness validation; an uncalled one (RJ-009 opaque_adder) keeps its zero-Instance carve-out. The committed snippet crates/bridge-coverage-runner/snippets/RJ-013.rs (mk() -> impl Fn() -> u32; f() { let g = mk(); g() }) bridges Ok and routes the revealed closure through a ClosureCall shim; the runner predicate is the coarse SyntheticItemKind = ClosureCall (the discriminating signal is reaching Ok at all -- at HEAD the crate Rejected on f's opaque). The precise per-shape proof (revealed-to-closure, no surviving Alias, impl Copy / nested-RPIT reveal, RJ-009 carve-out intact) lives in crates/regseq-driver/tests/bridge_rpit_opaque_reveal.rs. The NonRigidTyKind::Alias arm remains a defensive backstop for surviving non-authorized projection aliases. Plan: .harness/audits/2026-05-25-rj013-opaque-reveal-plan.md |  |
| RJ-014 | 6 | coverage | out-of-scope | blocked-rustc-infeasible | rustc rejects first | blocked-rustc-infeasible |
| RJ-015 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-016 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-017 | 6 | coverage | out-of-scope | blocked-rustc-infeasible | feature gated | blocked-rustc-infeasible |
| RJ-018 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-019 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-020 | 6 | coverage | out-of-scope | blocked-rustc-infeasible | feature gated | blocked-rustc-infeasible |
| RJ-021 | 6 | coverage | out-of-scope | blocked-rustc-infeasible | feature gated | blocked-rustc-infeasible |
| RJ-022 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-023 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-024 | 6 | coverage | out-of-scope | blocked-rustc-infeasible | The three rejection paths (Scalar::Union tag ty.rs:1320, Float tag ty.rs:1368, niche-out-of-range ty.rs:1446) are defensive bridge invariants for layouts rustc's solver does not emit from source: enum discriminants are integral (never Float/Union tags) and niche ranges respect rustc's guarantees. Probed niche / repr(u128) / niche-squeeze enums — none hit it. See .harness/audits/2026-05-23-b2-deferred-shim-probe.md (B2) | blocked-rustc-infeasible |
| RJ-025 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-026 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-027 | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-028 | 6 | coverage | needs-validation | partial |  |  |
| RJ-028-A | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-028-B | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-028-C | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-029 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-030 | 6 | coverage | needs-validation | uncovered |  |  |
| RJ-031 | 6 | coverage | needs-validation | partial |  |  |
| RJ-031-A | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-031-B | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-031-C | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-031-D | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-031-E | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-031-F | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032 | 6 | coverage | needs-validation | partial |  |  |
| RJ-032-A | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032-B | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032-C | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032-D | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032-E | 6 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RJ-032-F | 6 | coverage | out-of-scope | blocked-unimplemented-phase | D8 short-circuits to Ok(()) because target_str_arg_bound (bridge/diag_log/peel.rs:179-180) is hard-wired None pending the backend target-binding wiring that populates max_str_args_per_call (spec §26.8/26.9 + per-target <target>-backend.md). Unreachable from source until that phase lands. See .harness/audits/2026-05-23-f1-rj032f-watchlist.md (F1) | blocked-unimplemented-phase |
| RV-001 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| RV-002 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-003 | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| RV-004 | 1 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| RV-005 | 1 | coverage | needs-validation | partial |  |  |
| RV-005-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-F | 1 | coverage | validated-legacy | covered | instance_completeness.rs, trait_object.rs (&Point → &dyn Reader) |  |
| RV-005-G | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-H | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-I | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-J | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-K | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-L | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-M | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-005-N | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006 | 1 | coverage | needs-validation | partial |  |  |
| RV-006-A | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| RV-006-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-F | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-G | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-H | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-I | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| RV-006-J | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-K | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-L | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-M | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-N | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-O | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-P | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-Q | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-R | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-S | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-T | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-U | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-V | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-W | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-X | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-Y | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-006-Z | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-007 | 1 | coverage | needs-validation | uncovered |  |  |
| RV-007-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-007-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-007-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-008 | 1 | coverage | validated-legacy | covered | enum_round_trip.rs |  |
| RV-009 | 1 | coverage | needs-validation | partial |  |  |
| RV-009-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-009-B | 1 | coverage | validated-runner | partial | sidecar:rows.toml |  |
| RV-009-C | 1 | coverage | validated-legacy-universal | covered | many (e.g., Point { x, y }) |  |
| RV-009-D | 1 | coverage | validated-legacy | covered | enum_round_trip.rs |  |
| RV-009-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| RV-009-F | 1 | coverage | validated-legacy | covered | closure.rs, closure_call_sig_sync.rs |  |
| RV-009-G | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| SK-001 | 2 | coverage | needs-validation | covered |  |  |
| SK-002 | 2 | coverage | validated-legacy | covered | closure.rs, closure_call_sig_sync.rs |  |
| SK-003 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/SK-003.rs |  |
| SK-004 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/SK-004.rs |  |
| SK-005 | 2 | coverage | validated-runner | covered | crates/bridge-coverage-runner/snippets/SK-005.rs |  |
| SK-006 | 2 | coverage | needs-validation | uncovered |  |  |
| SK-007 | 2 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| SK-008 | 2 | coverage | validated-legacy | covered | trait_object.rs, instance_completeness.rs |  |
| SK-009 | 2 | coverage | validated-legacy | covered | trait_object.rs |  |
| SK-010 | 2 | coverage | validated-legacy | covered | trait_object.rs |  |
| SK-011 | 2 | coverage | needs-validation | uncovered |  |  |
| SK-012 | 2 | coverage | validated-legacy | partial | diag_logging.rs (string-literal backing) |  |
| ST-001 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| ST-002 | 1 | coverage | validated-legacy | covered | enum_round_trip.rs |  |
| ST-003 | 1 | coverage | needs-validation | partial |  |  |
| ST-003-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| ST-003-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TM-001 | 1 | coverage | validated-legacy-universal | covered | many |  |
| TM-002 | 1 | coverage | validated-legacy | covered | enum_round_trip.rs |  |
| TM-003 | 1 | coverage | validated-legacy-universal | covered | every fixture |  |
| TM-004 | 1 | coverage | validated-runner | partial | sidecar:rows.toml |  |
| TM-005 | 1 | coverage | validated-legacy-universal | covered | many |  |
| TM-005-A | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TM-005-B | 1 | coverage | validated-legacy | covered | fn_ptr_translation_sig_sync.rs |  |
| TM-005-C | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TM-005-D | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TM-006 | 1 | coverage | needs-validation | partial |  |  |
| TM-006-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TM-006-B | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TM-006-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TM-006-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TM-006-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TM-006-F | 1 | coverage | needs-validation | uncovered |  |  |
| TM-006-G | 1 | coverage | needs-validation | uncovered |  |  |
| TM-006-H | 1 | coverage | needs-validation | uncovered |  |  |
| TY-001 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-002 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-003 | 1 | coverage | validated-legacy | partial | instance_completeness.rs (i32 in Point) |  |
| TY-003-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-003-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-003-C | 1 | coverage | validated-legacy | covered | instance_completeness.rs (Point.x: i32) |  |
| TY-003-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-003-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-003-F | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-004 | 1 | coverage | validated-legacy-universal | partial | many |  |
| TY-004-A | 1 | coverage | validated-legacy | covered | trait_object.rs (read_byte() -> u8) |  |
| TY-004-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-004-C | 1 | coverage | validated-legacy-universal | covered | many |  |
| TY-004-D | 1 | coverage | validated-legacy | covered | instance_completeness.rs (Wrap<u64>) |  |
| TY-004-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-004-F | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-005 | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TY-005-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-005-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-005-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-005-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-006 | 1 | coverage | validated-legacy | covered | diag_logging.rs |  |
| TY-007 | 1 | coverage | validated-runner | blocked-generator-gap | sidecar:rows.toml |  |
| TY-008 | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-009 | 1 | coverage | validated-legacy | covered | diag_logging.rs |  |
| TY-010 | 1 | coverage | validated-legacy-universal | covered | many |  |
| TY-010-A | 1 | coverage | validated-legacy-universal | covered | many |  |
| TY-010-B | 1 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs (Drop::drop(&mut self)) |  |
| TY-011 | 1 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| TY-011-A | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-011-B | 1 | coverage | validated-legacy | covered | drop_glue_in_self_consuming_method.rs |  |
| TY-012 | 1 | coverage | validated-legacy-universal | covered | many |  |
| TY-013 | 1 | coverage | validated-legacy | covered | fn_ptr_translation_sig_sync.rs |  |
| TY-014 | 1 | coverage | validated-legacy | covered | trait_object.rs, instance_completeness.rs |  |
| TY-014-A | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TY-014-B | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-014-C | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-014-D | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-014-E | 1 | coverage | validated-runner | uncovered | sidecar:rows.toml |  |
| TY-015 | 1 | coverage | validated-legacy | covered | closure.rs, closure_call_sig_sync.rs |  |
| TY-016 | 1 | coverage | validated-runner | partial | sidecar:rows.toml |  |
| TY-017 | 1 | coverage | validated-legacy-universal | covered | many |  |
| TY-018 | 1 | coverage | validated-runner | covered | sidecar:rows.toml |  |
| TY-019 | 1 | coverage | validated-legacy | covered | instance_completeness.rs (Pair) |  |
| VL-001 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_1_dangling_item_id_in_instance + id_ref dangling/const/span tests |  |
| VL-002 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_2_item_def_paths_length_mismatch |  |
| VL-003 | 7 | validator | validator-exercised | yes | id_ref:invariant_2_reject_synthetic_kinds_short; model_tests.rs:invariant_2_item_synthetic_kinds_length_mismatch |  |
| VL-004 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_4_primary_crate_out_of_range |  |
| VL-005 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_4_primary_crate_stable_id_mismatch |  |
| VL-006 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_5_duplicate_stable_crate_id |  |
| VL-007 | 7 | validator | validator-exercised | yes | id_ref:invariant_6_reject_def_path_crate_id_out_of_bounds |  |
| VL-008 | 7 | validator | validator-exercised | yes | id_ref:invariant_11_{ty,fn_sig,const}_interner_pass_and_reject_duplicate, phase_b_then_c_ordering |  |
| VL-009 | 7 | validator | validator-exercised | yes | id_ref:invariant_18_reject_duplicate_item_def_path_hash |  |
| VL-010 | 7 | validator | validator-exercised | yes | id_ref:f4_sort_reject_permuted_input |  |
| VL-011 | 7 | validator | validator-exercised | yes | id_ref:f4_2_sort_reject |  |
| VL-012 | 7 | validator | validator-exercised | yes | NEW row (enum #12). validate_bridge_completeness_buckets.rs:bucket1_function_item_with_duplicate_shim_instances_rejected |  |
| VL-013 | 7 | validator | validator-exercised | yes | id_ref:def_path_render_reject_malformed |  |
| VL-014 | 7 | validator | validator-exercised | yes | id_ref:span_bounds_reject_lo_gt_hi, span_bounds_reject_hi_gt_content_len |  |
| VL-015 | 7 | validator | validator-exercised | yes | id_ref:span_bounds_reject_not_on_utf8_boundary_{lo,hi} |  |
| VL-016 | 7 | validator | validator-exercised | yes | id_ref:phase_c_vtables_rebuild_reject_duplicate_principal |  |
| VL-017 | 7 | validator | validator-exercised | yes | model_tests.rs:invariant_8_synthesized_sentinel_{invalid_path,with_content_rejected,with_wrong_line_starts_rejected} |  |
| VL-018 | 7 | validator | validator-gap | no | unreachable-by-construction backstop: StringInterner encapsulation (private fields; new()/intern() preserve the sentinel; Deserialize rejects non-empty index 0) makes the defect unrepresentable without a test-only hook. Not a fixture-batch task. See audit doc S.2/S.3 finding 5 |  |
| VL-019 | 7 | validator | validator-gap | no | unreachable-by-construction backstop: intern() is content-deduplicating and Deserialize rejects duplicates (string_interner.rs:49-50,157-161), so the defect needs a test-only hook. Not a fixture-batch task. See audit doc S.2/S.3 finding 5 |  |
| VL-020 | 7 | validator | validator-exercised | yes | id_ref:place_validator_structural_error_at_index_5_not_masked_by_type_error_at_index_0; place_validator.rs:joint_validator_integration_pass_and_reject |  |
| VL-021 | 7 | validator | validator-gap | no | constructed-model gap; ItemKind::Function item with SyntheticItemKind::ClosureCaptures (requires Struct). See audit doc S.2 |  |
| VL-022 | 7 | validator | validator-exercised | yes | id_ref:terminator_b7_direct_callee_out_of_range_rejects; place_validator.rs:joint_validator_rejects_dangling_direct_call_instance |  |
| VL-023 | 7 | validator | validator-exercised | yes | layout_validator.rs:target_reject_* (6 reasons) |  |
| VL-024 | 7 | validator | validator-exercised | yes | layout_validator.rs:build_reject_toolchain_empty (one of two reasons; reason-string) |  |
| VL-025 | 7 | validator | validator-exercised | yes | layout_validator.rs:fn_sig_c_variadic_reject_{non_c_abi,zero_inputs}, fn_sig_reject_inputs_never |  |
| VL-026 | 7 | validator | validator-exercised | yes | layout_validator.rs:cabi_lowering_reject_* (5 tests) |  |
| VL-027 | 7 | validator | validator-exercised | yes | layout_validator.rs:layout_reject_field_{overlap,offset_misaligned,overflows_layout_size} (struct-kind reasons; tuple/array/union/enum untested -- same variant) |  |
| VL-028 | 7 | validator | validator-exercised | yes | layout_validator.rs:tag_scalar_width_fit_reject_overflow |  |
| VL-029 | 7 | validator | validator-exercised | yes | layout_validator.rs:invariant_14_reject_kind_mismatch_struct_to_enum |  |
| VL-030 | 7 | validator | validator-exercised | yes | layout_validator.rs:invariant_14_reject_item_args_mismatch |  |
| VL-031 | 7 | validator | validator-exercised | yes | layout_validator.rs:const_graph_cycle_reject_{self_loop,two_node,enum_value_payload} |  |
| VL-032 | 7 | validator | validator-exercised | yes | layout_validator.rs:domain_attr_*_reject*, intrinsic_singleton_reject_two_intrinsics (6 tests) |  |
| VL-033 | 7 | validator | validator-exercised | yes | layout_validator.rs:invariant_19_reject_{non_struct_kind,generic,non_zst_layout,by_value_use} |  |
| VL-034 | 7 | validator | validator-exercised | yes | layout_validator.rs:invariant_20_reject_real_crate_zero |  |
| VL-035 | 7 | validator | validator-exercised | yes | layout_validator.rs:invariant_20_reject_{source_derived_synthetic_domain,synthetic_non_zero_domain} |  |
| VL-036 | 7 | validator | validator-exercised | yes | layout_validator.rs:unit_layout_reject_non_canonical |  |
| VL-037 | 7 | validator | validator-exercised | yes | layout_validator.rs:source_file_reject_line_start{s_non_monotonic,s_does_not_start_at_zero,_exceeds_content_len} |  |
| VL-038 | 7 | validator | validator-exercised | yes | layout_validator.rs:source_file_reject_duplicate_path |  |
| VL-039 | 7 | validator | validator-exercised | yes | id_ref:body_ty_closure_rejects_dangling_local_decl_ty |  |
| VL-040 | 7 | validator | validator-exercised | yes | id_ref:statement_b1_assign_type_mismatch_rejects |  |
| VL-041 | 7 | validator | validator-partial | partial | id_ref:statement_b1_assign_malformed_rejects_ptr_metadata_on_non_fat_pointer asserts only side="rvalue"; side="place" arm untested. See audit doc S.2 |  |
| VL-042 | 7 | validator | validator-exercised | yes | id_ref:statement_b2_set_discriminant_variant_idx_out_of_range_rejects |  |
| VL-043 | 7 | validator | validator-exercised | yes | id_ref:statement_b3_set_discriminant_non_enum_target_rejects |  |
| VL-044 | 7 | validator | validator-exercised | yes | id_ref:statement_b4_assume_operand_not_bool_rejects |  |
| VL-045 | 7 | validator | validator-partial | partial | id_ref:statement_b5_copy_non_overlapping_count_type_rejects asserts only which="count"; src/dst/pointee arms untested. See audit doc S.2 |  |
| VL-046 | 7 | validator | validator-exercised | yes | id_ref:terminator_b1_switch_int_discr_type_mismatch_rejects |  |
| VL-047 | 7 | validator | validator-exercised | yes | id_ref:terminator_b2_assert_cond_not_bool_rejects |  |
| VL-048 | 7 | validator | validator-exercised | yes | id_ref:terminator_b3_call_destination_type_mismatch_rejects{,_indirect} |  |
| VL-049 | 7 | validator | validator-exercised | yes | id_ref:terminator_b4_call_args_arity_mismatch_rejects_{direct,indirect}; model_tests.rs:{non_log_direct_call_uses_generic_b4,indirect_call_does_not_receive_log_exception} |  |
| VL-050 | 7 | validator | validator-exercised | yes | id_ref:terminator_b5_call_direct_callee_not_function_rejects |  |
| VL-051 | 7 | validator | validator-exercised | yes | id_ref:terminator_b5_call_indirect_callee_not_fn_ptr_rejects |  |
| VL-052 | 7 | validator | validator-exercised | yes | id_ref:terminator_b6_call_arg_type_mismatch_rejects_{direct,indirect} |  |
| VL-053 | 7 | validator | validator-exercised | yes | id_ref:terminator_b8_virtual_idx_inconsistent_rejects |  |
| VL-054 | 7 | validator | validator-exercised | yes | id_ref:terminator_b9_target_divergence_mismatch_rejects_{non_diverging_no_target,indirect_divergent_target_some} |  |
| VL-055 | 7 | validator | validator-exercised | yes | model_tests.rs:rejects_args_zero_not_str_ref |  |
| VL-056 | 7 | validator | validator-exercised | yes | model_tests.rs:rejects_empty_args |  |
| VL-057 | 7 | validator | validator-exercised | yes | model_tests.rs:rejects_args_one_not_admissible_log_arg_source |  |
| VL-058 | 7 | validator | validator-exercised | yes | validate_bridge_completeness_buckets.rs:bucket{1_struct,2_foreign_function}_zero_instances_rejected; validate_item_instance_coverage.rs:b{1,2}_*_without_instance_is_rejected |  |
| VL-059 | 7 | validator | validator-exercised | yes | validate_bridge_completeness_buckets.rs:bucket1_{struct_multiple_instances,function_item_with_two_primary_instances}_rejected |  |
| VL-060 | 7 | validator | validator-exercised | yes | validate_bridge_completeness_buckets.rs:bucket2_foreign_{function_body,static_init}_must_be_none |  |
| VL-061 | 7 | validator | validator-exercised | yes | validate_bridge_completeness_buckets.rs:bucket4_rpit_function_with_instance_rejected |  |
