# Spec Batch: regseqmodel-phase-1a -- RegseqModel + pretty-print implementation

## Description

This batch elaborates the executable harness tasks that implement phase-1a of the RegseqModel refactor: the producer-side IR data shapes, the bridge from rustc's `optimized_mir` to those data shapes, the textual consumer surface (the pretty-printer), and the diagnostic-logging intrinsic vocabulary at the IR-shape level. Phase-1a is the first of two sequential sub-phases that together implement the locked phase-1 spec, mirroring the spec's co-equal merger-artifact structure (`spec.md` producer side / `pretty-printer.md` textual-consumer side / `generic-c-backend.md` codegen-consumer side; phase-1b owns the third file). Per `docs/discussions/regseq-model/README.md` §1.3 the split isolates bridge bugs from codegen bugs by construction and lets the pretty-printer stand on its own as a closeable milestone consistent with Q5's nomination of the pretty-printer as the primary serialization surface.

This brief authors **executable harness task specs** for phase-1a; the task specs that result will be executed by individual workers per `.harness/SESSION.md`. The brief itself is not a task spec -- it is a decomposition contract that names the tasks, the order they land, the test discipline each one inherits, and the workspace target for each (`regseq-model` for IR data shapes, validators, and rendering; `regseq-driver` for the bridge per Q1's two-crate split; `test-ir-fixtures` for shared cross-crate fixtures). The producer-side surface that phase-1a delivers is exhaustive for `spec.md` §1-§26 + Appendix A modulo the carve-outs in `## Non-Goals`; the consumer-side surface is exhaustive for `pretty-printer.md` §1-§11.

The existing `crates/regseq-model/src/` directory ships a *Stage-1-grandfathered* foundation (ID system, `StringInterner`, `IndexVec`, `CrateInfo`, `DefPath`, an `Instance` skeleton, the `RegseqModel` root struct with placeholder `Ty`/`Const`/`Item`/`FnSig`/`Body` value types, and a `PrettyPrinter` shell) -- see `crates/regseq-model/src/lib.rs:1-58` and `crates/regseq-model/src/placeholders.rs:30-50`. That foundation pre-dates the "spec before implementation" discipline (per README §2's grandfathering note for stage 1). The phase-1a tasks therefore extend rather than replace that surface: data-shape tasks expand placeholders into their full spec-mandated shape; the bridge tasks land entirely net-new code in `regseq-driver`; the pretty-printer tasks replace the shell with the §1-§11 ownership-table-pinned methods.

The decomposition follows the five-axis refinement specified in `Implementation Notes`. Phase-1a closes when every task ID below is `done`, the workspace-wide quality gate passes, `task.py phase-status 1a` returns PASS, and the Phase-Close Review (per `.harness/SESSION.md` Review section) emits VERDICT: APPROVED.

## Input Context

A cold worker authoring or executing a phase-1a task must read:

- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/README.md` (harness orientation -- briefs vs task specs, lifecycle, CLI surface)
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this brief)
- `docs/discussions/regseq-model/README.md` (especially §1.3 three-phase methodology, §2 binding methodology rules, §3 Q1-Q7 architectural decisions, §5.9 spec/pretty-printer/generic-c-backend co-equal artifact rules)
- `docs/discussions/regseq-model/spec.md` (the producer-side spec; phase-1a covers §1-§26 + Appendix A modulo `## Non-Goals`)
- `docs/discussions/regseq-model/pretty-printer.md` (the textual-consumer spec; phase-1a covers all 11 sections)
- `docs/discussions/regseq-model/architecture.md` (Q1-Q7 verbatim)
- `docs/discussions/regseq-model/decisions.md` (per-stage D-tag registry, scan-only)
- `crates/regseq-model/src/lib.rs` (existing Stage-1-grandfathered foundation; the data-shape and pretty-printer tasks extend this)
- `crates/regseq-model/src/placeholders.rs` (existing `Ty`/`Const`/`Item`/`FnSig`/`Body` placeholder types)
- `crates/regseq-model/src/model.rs` (existing `RegseqModel` root + validator)
- `crates/regseq-model/src/instance.rs` and `crates/regseq-model/src/pretty.rs` (existing skeletons)
- `crates/regseq-model/Cargo.toml` and `crates/regseq-driver/Cargo.toml`
- `crates/regseq-driver/src/lib.rs` and `crates/regseq-driver/src/callbacks.rs`
- `Cargo.toml` workspace root (the `test-ir-fixtures` crate is registered here)

Per-task input context is a subset of the above plus the specific spec/pretty-printer subsections cited in each task's `spec_refs`; the task-spec author cites those subsets via `task.py read-spec-refs`.

## Output Specification

Create (one file per task) under `.harness/tasks/`:

- `.harness/tasks/regseq-model/<NN>-<topic>.md` -- task specs for data-shape, validator, and pretty-printer tasks (target crate: `regseq-model`)
- `.harness/tasks/regseq-driver/<NN>-<topic>.md` -- task specs for bridge tasks (target crate: `regseq-driver`)
- `.harness/tasks/test-ir-fixtures/<NN>-<topic>.md` -- task spec for the test-support crate scaffold + first end-to-end smoke fixture (target crate: `test-ir-fixtures` -- new workspace member)
- `.harness/tasks/regseq-model/<NN>-e2e-<fixture>.md` -- task specs for end-to-end fixture tests (target group: `regseq-model` since the assertion surface is the IR + pretty-printer)

`<NN>` continues the existing per-group sequential numbering. The current highest IDs visible under `.harness/tasks/regseq-model/` reach the mid-50s (`55-verifier-framework-and-phase1.md`); phase-1a IDs **start at the next free integer per group** to avoid collision with historical archived task IDs. The exact base numbers are decided when registering tasks via `task.py add-task` after this brief is approved.

Each created task spec follows the canonical 8-section structure from `.harness/README.md` "Task spec markdown structure":

1. Description (1-2 paragraphs)
2. Input Context (files to read; subset of this brief's Input Context plus stage-N earlier-task headers)
3. Implementation Notes (public API signatures copy-pasted from `spec.md` / `pretty-printer.md`; conventions; serde shape; invariants)
4. Non-Goals (what later tasks own)
5. Test Plan (one or more of: unit tests in `src/`, integration tests in `tests/`, test-support crate fixtures -- chosen per the discipline in `## Implementation Notes` below)
6. Review (the four-block `codex_runner.py` template from `SESSION.md`)
7. Quality Gate (`cargo test -p <crate>` / `cargo fmt --all -- --check` / `cargo clippy -p <crate> -- -D warnings`)
8. Acceptance Criteria

Update when done:

- `.harness/task-spec-status.yaml` -- the new item `regseqmodel-phase-1a` (status `tasked` once all task specs are written and registered), with every generated task ID under `generated_task_ids`. The companion `regseqmodel-phase-1b` item appended at status `not_ready`.
- Per the workflow rules, register each new task via `uv run python scripts/python/task.py add-task --phase 1a --spec-refs ...` (writes to `.harness/tasks.json` atomically). The new `test-ir-fixtures` group is added by hand to `tasks.json` first (the harness has no `add-group` CLI; one-off direct edit, justified by absence).

The output is **executable harness tasks**.

## Implementation Notes

This section is the decomposition contract. Phase-1a is decomposed along **five axes**. Each task lands within one crate or one tightly related crate pair (per `task-spec-workflow.md` Authoring Rule §8). Test-organization discipline is binding: each task's Test Plan section names one or more of `unit-in-src`, `integration-in-tests`, or `test-support-crate` per `.harness/README.md`'s test-organization rules.

Tasks within an axis are listed in dependency order. Cross-axis dependencies are flagged inline; the dependency graph after all tasks are registered should form a DAG with the data-shape axis as the foundation, validators / pretty-printer / bridge depending on it, and end-to-end fixtures depending on everything.

The decomposition targets **42 tasks** -- exceeding the original 25-40 target band by 2; per the post-draft Codex right-sizing review, 42 is the minimum count that prevents tasks from exceeding the size threshold (the reference task `archive/.harness/tasks/regseq-model/24-place-and-projection.md` is ~117 lines of spec yielding one focused module). The breakdown: 18 data-shape tasks, 3 validator tasks, 10 bridge tasks, 8 pretty-printer tasks, 3 test-support+E2E tasks. Rationale per axis below.

---

### Axis 1: Data-shape tasks (18 tasks; target crate `regseq-model`)

**Rationale.** One task per `spec.md` section defining structs / enums / identity, with section splits where the underlying surface exceeds the size threshold. The post-draft Codex right-sizing review surfaced four splits relative to the v1 draft: (i) §13 splits between closure-family (DS-08a) and vtable-family + synthetic-key + DefPath/Hash construction (DS-08b); (ii) §20+§21 splits between Operand (DS-12a) and Rvalue+sub-enums (DS-12b) because §21's 9-variant Rvalue + CastKind + AggregateKind + 26-variant BinOp + UnOp exceeds the threshold; (iii) §22+§23 splits between Statement (DS-13a) and Terminator (DS-13b); (iv) §24+§25 splits between BasicBlock (DS-14a) and Body+LocalDecl+APIs+joint-validation hooks (DS-14b) because §25's Body + allocation API + joint-validation walk hooks alone match the size reference. The Ty enum (§10) and Layout (§11) split (DS-02 vs DS-03) is preserved because the inline-`Layout` carry on §10's Tuple/Struct/Enum/Union variants pulls §11's full machinery into a separate concern. The Item table (§8 + §13) splits between `ItemKind`/`Item` core (DS-07) and the synthesised-Item machinery (DS-08a + DS-08b).

The existing `crates/regseq-model/src/placeholders.rs` empty placeholders for `Ty` / `Const` / `Item` / `FnSig` / `Body` are replaced incrementally by these tasks. The existing `RegseqModel` root struct at `crates/regseq-model/src/model.rs:40-76` is amended *only* by P1a-DS-01 to add the spec-mandated fields and to drop the legacy `BodyId` table per stage-12 D-12-1.4 / Option B. DS-01 also explicitly extends foundation conformance for `spec.md` §2 (ID system), §4 (StringInterner), §5 (Source files and spans), §6 (CrateInfo), and §7 (DefPath) -- the foundation types pre-date the locked phase-1 spec and may need spec-mandated field additions or shape adjustments earlier stage-1 work did not cover.

| Task ID (proposed) | Name | Target | Depends on | Scope summary | Test Plan kind |
|---|---|---|---|---|---|
| P1a-DS-01 | Root `RegseqModel` + `TargetInfo` + `BuildInfo` + `Endian` + `ScalarLayouts` + construction-invariant-set scaffold + foundation conformance for §2 / §4 / §5 / §6 / §7 | `regseq-model` | (foundation already shipped) | Refresh existing root struct at `crates/regseq-model/src/model.rs` to spec §3 / §3.1 shape: drop `bodies: IndexVec<BodyId, Body>` per Option B (`spec.md:315-321`), add `target` / `build` / `item_synthetic_kinds` / `sig_c_abi_lowerings` / `vtables: HashMap<PrincipalTraitKey, ItemId>` (`#[serde(skip)]`) / `domain_attrs`. Add `BridgeInput` struct (§3.5.1) and Instance-side pull-through helpers (§3.5.2). Pin invariants 1-20 in a registry the validator tasks read. **Foundation alignment**: verify the existing §2 ID system, §4 StringInterner, §5 Source files and spans, §6 CrateInfo, and §7 DefPath types match the locked spec; extend with any spec-mandated fields missing from the existing implementation. | unit-in-src (construction + serde round-trip) |
| P1a-DS-02 | Ty enum (§10) + auxiliary closed enums + §17.2 IR-visible Ty helpers | `regseq-model` | P1a-DS-01 | The 19-variant closed `Ty` enum verbatim from `spec.md:3643-3663`; auxiliary closed enums (Provenance / Mutability / AutoTrait / FnAbi / Safety; IntKind / UintKind / FloatKind / *_KIND_COUNT) from `spec.md:3717-3736`. Unsized-marker placement rule (§3.1 invariant 12 / §10.2). Wide-scalar admission (§10.3). **§17.2 IR-visible helpers**: `contains_unsafe_cell(ty, model) -> bool` (`spec.md:6298+`) and any other §17.2 helper that operates on `Ty` without rustc dependencies. Fat-pointer mutability rules from §17.2 stay phase-1b (codegen-side). | unit-in-src (each variant ctor + equality; helpers covered) |
| P1a-DS-03 | Layout + LayoutKind + FieldLayout + EnumShape + VariantLayout + TagScalar + TagEncoding + WrappingRange + `UNIT_LAYOUT` constant + size/align query (§11) | `regseq-model` | P1a-DS-02 | Inline `Layout` on Tuple/Struct/Enum/Union per `spec.md:3805-3855`; §11.1 EnumShape variants; §11.2 tag encodings; §11.5 size_align_of query. | unit-in-src |
| P1a-DS-04 | TyInterner + TyIdentityKey + ArgsKey deep canonicaliser + GenericArgs encoding (§12) | `regseq-model` | P1a-DS-02, P1a-DS-03 | §12.1 identity rules; §12.2 self-referential ADT handling; §12.3 ArgsKey canonical byte form + SHA-256 truncation; §12.4 cross-build property; §12.5 serialisation. | unit-in-src (interning + identity + ArgsKey byte fixtures) |
| P1a-DS-05 | Const vocabulary (§9.7) + Byte three-state enum + InstanceRef + ConstInterner + ConstIdentityKey -- **data shape only** | `regseq-model` | P1a-DS-04 | The 11-variant closed `Const` enum + Byte + InstanceRef + content-keyed identity per `spec.md:2883-3286`. Per-variant byte-encoding tags. Reference-typed const **data shape only** (`spec.md:3328` Aggregate([Reloc, Uint])); the corresponding `ConstMemoryStatic` Item synthesis is bridge-side and lives in P1a-BR-03a per the post-draft Codex coverage review (synthesis fires during bridge Phase 2/4/5, not at data-shape time). | unit-in-src |
| P1a-DS-06 | FnAbi + FnSig + FnSigInterner + CAbiLowering + C-ABI admissibility predicate output shape (§14) | `regseq-model` | P1a-DS-04 | §14.1 FnAbi enum; §14.3 FnSig schema; §14.4 FnSigInterner; §14.5 CAbiLowering shape (Direct / Substituted { emit_as, reason }) -- carry the bridge's outcome, not the rustc-side predicate. CAbiPositionAdmissible / CFieldAdmissible **predicate outcomes** are stored, not re-derived. | unit-in-src |
| P1a-DS-07 | Item + ItemKind + Item attribute flags (linkage / inline_hint / lang-flags / link_section / opaque_handle_eligible / is_unsafe) (§8) | `regseq-model` | P1a-DS-04 | §8.0 schema; §8.1 lifting `impl` items; §8.2 lang-item flags. `opaque_handle_eligible` consistency (invariant 19). | unit-in-src |
| P1a-DS-08a | Closure-family synthesised Items (§13.1): ClosureCaptures Item + ClosureCall Function Item + ClosureDropGlue Function Item + ClosureCallKind enum | `regseq-model` | P1a-DS-07 | The 3 closure-family synthetic Items + ClosureCallKind per `spec.md:4503+`. Closure-as-struct identity-vs-layout split (D6.3): the parallel `Ty::Struct(ClosureCaptures, args, layout)` is computed at bridge time but the data-shape side defines the `SyntheticItemKind::ClosureCaptures` variant alongside the Item itself. | unit-in-src |
| P1a-DS-08b | VTable-family synthesised Items + SyntheticItemKind enum + PrincipalTraitKey + synthetic DefPath segment encoding + synthetic DefPathHash construction (§13.2-§13.4) | `regseq-model` | P1a-DS-07 | The 3 vtable-family synthetic Items (VTableType, VTableInstance, VTableShim) + synthetic vtable layout algorithm (§13.2); SyntheticItemKind covers all 6 synthetic Items (closure + vtable + drop-glue) (§13.3); PrincipalTraitKey + dedup table key shape (§13.3); `<regseq_synth>` reserved-prefix segments + StableCrateId(0)-domain construction (§13.4). | unit-in-src |
| P1a-DS-09 | DomainAttr sibling tables (§3.6): DomainAttrTables + AttrData (closed append-only) + IntrinsicKind + LogLevel + FieldPath + IntrinsicKind::from_name closed-vocabulary lookup | `regseq-model` | P1a-DS-07 | §3.6.1-§3.6.3. The dash-form NAME mapping (`log-debug` / `log-info` / `log-warn` / `log-error` per `spec.md:1542-1551`). Validation invariants (§3.6.5). Attribute-namespace storage rule (§3.6.6). Per-family target-table mapping (§3.6.7). | unit-in-src |
| P1a-DS-10 | Instance + InstanceKind + InstanceId interning key + FunctionInstanceFlavor (§9.1-§9.4) | `regseq-model` | P1a-DS-04, P1a-DS-05, P1a-DS-06, P1a-DS-07 | The Option-B per stage-12 D-12-1.4: `InstanceKind::Function { fn_sig: FnSigId, body: Option<Body>, flavor: FunctionInstanceFlavor, virtual_idx: Option<u32> }` per `spec.md` §9.4. The 6-variant InstanceKind. F4-2 RegseqInstanceKey. | unit-in-src (the existing `crates/regseq-model/src/instance.rs` shell must be amended) |
| P1a-DS-11 | Place + PlaceElem (6 variants) + structural invariants (§19) + Place's body-local identity scheme | `regseq-model` | P1a-DS-02, P1a-DS-03 | The 6-variant closed PlaceElem enum from `spec.md:6595-6692`. I-Deref-first + I-Downcast-followed-by-Field structural invariants. Type-narrowing-rules table (§19.4) **as documentation** -- enforcement lands in P1a-VAL-01. | unit-in-src |
| P1a-DS-12a | Operand + LiteralValue (§20) | `regseq-model` | P1a-DS-11, P1a-DS-05 | §20 operand vocabulary (Copy / Move / Constant) + LiteralValue. | unit-in-src |
| P1a-DS-12b | Rvalue + sub-enums: CastKind + BorrowKind + AggregateKind + BinOp (26 variants) + UnOp (§21) | `regseq-model` | P1a-DS-12a, P1a-DS-05 | §21.1 9-variant Rvalue; §21.3 sub-enums (CastKind / AggregateKind / BinOp / UnOp -- 26 BinOp variants per the historical task `archive/.harness/tasks/regseq-model/23-binary-and-unary-operators.md`). | unit-in-src |
| P1a-DS-13a | Statement + StatementKind (3 variants: Assign / SetDiscriminant / Intrinsic) + NonDivergingIntrinsic sub-enum (§22) | `regseq-model` | P1a-DS-12b | §22.1 statement struct + 3-variant StatementKind; §22.3 NonDivergingIntrinsic sub-enum (Assume / CopyNonOverlapping). | unit-in-src |
| P1a-DS-13b | Terminator + TerminatorKind (6 variants: Goto / SwitchInt / Return / Unreachable / Call / Assert) + CallCallee + SwitchTargets + AssertKind (§23) | `regseq-model` | P1a-DS-13a | §23.1 terminator struct + 6-variant TerminatorKind; §23.2 per-variant payload shapes; §23.3 sub-enums. The `Drop` rustc variant is collapsed at the bridge per §23.4 -- no IR-side `Drop` variant exists. | unit-in-src |
| P1a-DS-14a | BasicBlock + BlockId typed-index + container + block-ordering invariants (§24) | `regseq-model` | P1a-DS-13b | §24.1-§24.4 basic-block struct + container + ordering. | unit-in-src |
| P1a-DS-14b | Body struct + LocalDecl + LocalId typed-index + `Body::push_synthetic_local` API + LocalDecls allocation API (§25.8) + Body's joint validation walk hooks (§25.6) (§25) | `regseq-model` | P1a-DS-14a | §25.1-§25.5 Body struct + LocalDecl + LocalId + FnSig location + Body-vs-Item linkage (Option B); §25.8 LocalDecls allocation API; §25.9 per-stage invariants registry. | unit-in-src |

Cross-cutting note on data-shape tasks: each task defines the **public API surface** (struct fields, enum variants, function signatures) with `todo!()` bodies first, then writes tests against the API, then implements -- per `SESSION.md` "Doing the Work" §6-§10. Each public type / function must have at least one test (per `SESSION.md` "Testing Requirements"). Serde shape is included per task only where the type appears in the serialized model wire form; per `crates/regseq-model/src/placeholders.rs` precedent and per the size-reference's "Conventions", types containing `Symbol`/`StringId` are deferred from serde until the resolution-time pass lands.

---

### Axis 2: Invariant / validator tasks (3 tasks; target crate `regseq-model`)

**Rationale.** The prompt specifies standalone validators for the structural invariants the spec pins. Per `spec.md` §3.3 the validation phase is two-step (Phase A parsing -> Phase B validation). Phase-A is the deserialise pass already partially covered by serde derives; Phase-B is the validator surface. Phase-B is split into three tasks by invariant family: (a) Place's two structural invariants (§19.3) plus Place type-narrowing (§19.4) and Place validation (§19.7) form a coherent walk-implementation pair (P1a-VAL-01); (b) ID referential integrity (§3.1 invariants 1, 2, 6, 9, 11, 18) which extends the existing partial walk at `crates/regseq-model/src/model.rs:140-449` plus content-keyed interner uniqueness plus §3.3 surfaces (DefPath rendered validation, source-span bounds, Phase C `vtables` rebuild and duplicate rejection) plus §16 F4 / F4-2 sort comparator output check folded in (P1a-VAL-02); (c) Layout consistency (§3.1 invariants 14, 19, 20 + §11.5 layout rules + canonical UNIT_LAYOUT check) plus §3.3 surfaces (target/build validation, FnSig/CAbiLowering checks, const-graph acyclicity, domain-attr validation, source-file validation) (P1a-VAL-03).

The post-draft Codex review **deleted the original VAL-04 task** because (i) §15.1 rejection enforcement is at the bridge translation sites, not at the model side -- closed-enum exhaustiveness gives the model-side guarantee for free at the type level -- and (ii) the F4 / F4-2 sort comparator output check folds naturally into VAL-02's ID-and-ordering invariant family.

| Task ID | Name | Target | Depends on | Scope summary | Test Plan kind |
|---|---|---|---|---|---|
| P1a-VAL-01 | Place validator: type-coherence walk + structural-invariant enforcement (§19.7 rules 1-5) | `regseq-model` | P1a-DS-11, P1a-DS-14b | Implement the §19.4 type-narrowing walk + I-Deref-first + I-Downcast-followed-by-Field + FieldIdx range + VariantIdx range checks. Walk-implementation lives on `Body` (per §19.7 "Walk implementation: deferred to stage 11") and runs over body content. | unit-in-src + integration-in-tests (against constructed Places exercising each §19.4 row + each rejection case) |
| P1a-VAL-02 | ID referential integrity validator + content-keyed interner uniqueness + F4/F4-2 sort comparator output check + DefPath rendered validation + source-span bounds + Phase C vtables rebuild and duplicate rejection | `regseq-model` | P1a-DS-01 ... P1a-DS-10 | Extend the existing walk at `crates/regseq-model/src/model.rs:232-449` to cover: §3.1 invariants 1, 2, 6, 9, 11, 18 (item F4-sort, every DefPath.crate_id resolves, instance closure-completeness post Phase-A, TyInterner / FnSigInterner / ConstInterner content-keyed uniqueness, item_def_paths global uniqueness); §16.1 F4 sort comparator output check (the `def_path_data_sort_rank` table per `spec.md:6031-6053`); §16.3 F4-2 RegseqInstanceKey sort output check; §3.3 DefPath rendered validation; §3.3 source-span bounds; §3.3 Phase C `vtables` rebuild and duplicate rejection. | unit-in-src + integration-in-tests (against constructed model fixtures exercising the F4 ordering keys and Phase C rebuild) |
| P1a-VAL-03 | Layout consistency + invariant 14 (InstanceKind layout matches Ty nominal variant) + invariant 19 (`Item.opaque_handle_eligible`) + invariant 20 (StableCrateId(0) synthetic-domain enforcement) + target / build validation + FnSig / CAbiLowering checks + const-graph acyclicity + domain-attr validation + source-file validation | `regseq-model` | P1a-DS-03, P1a-DS-07, P1a-DS-08a, P1a-DS-08b, P1a-DS-10 | Per-invariant validators. Canonical UNIT_LAYOUT check on every `Ty::Tuple { fields: [] }` (per `spec.md:3849-3854`). §3.3 enrichment: target / build invariant validation, FnSig / CAbiLowering (`spec.md:5366+`) consistency, const-graph acyclicity, domain-attr validation per §3.6.5, source-file validation. | unit-in-src |

---

### Axis 3: Bridge translation tasks (10 tasks; target crate `regseq-driver`)

**Rationale.** Per `spec.md` §3.4 the bridge is a 5-phase pipeline (DISCOVERY -> TRANSLATION-with-interleaved-synthesis -> F4 SORT -> ItemId ASSIGNMENT + side-table population -> INSTANCE construction + cross-reference resolution + body-content rewrites). Per-construct bridge subsections exist at §15 (TyKind / special-ADT dispatch + §15.1 rejections), §19.5 (Place), §20.3 (Operand + RuntimeChecks lowering), §21.6 (Rvalue), §22.6 (Statement), §23.7 (Terminator + §23.4 Drop-as-Call collapse), §24.7 (basic-block), §25.7 (Body-level transformation passes: Drop-as-Call, vtable-indirect rewrite, DiagLogPeel), §26.6 (Diagnostic-logging recognition + Pipeline A). The 5-phase pipeline ordering is implemented as the **task ordering** itself: P1a-BR-01 builds the pipeline scaffold and the Phase 1 (DISCOVERY) + Phase 3 (F4 SORT) + Phase 4 (ItemId ASSIGNMENT) machinery; subsequent tasks fill the Phase 2 / Phase 5 translation per construct.

The post-draft Codex right-sizing review surfaced two splits: (i) BR-03 splits between Const + reference-typed const handling + ConstMemoryStatic Item synthesis trigger (BR-03a) and FnSig + CAbiLowering bridge (BR-03b) because the combined surface exceeds the size threshold; (ii) BR-08 splits between recognition + per-callee attribute population (BR-08a) and DiagLogPeel + format-template grammar parsing + bridge-side validation diagnostics (BR-08b) because the §26 surface spans 8 subsections. The ordering rule **BR-04 must land before BR-06 / BR-07** because Drop-as-Call collapse and vtable-indirect rewrite consume the synthetic Items BR-04 produces; the explicit dependency is recorded in each row.

The bridge crate is `regseq-driver` (Q1 two-crate split); all bridge tasks are in that crate. The `crates/regseq-driver/src/` directory currently has `callbacks.rs` / `args.rs` / `lib.rs` / `main.rs` shell; phase-1a bridge tasks land net-new modules under a `src/bridge/` subtree.

| Task ID | Name | Target | Depends on | Scope summary | Test Plan kind |
|---|---|---|---|---|---|
| P1a-BR-01 | Bridge construction-pipeline scaffold (§3.4): Phase 1 DISCOVERY (`collect_crate_mono_items`) + Phase 3 F4 SORT + Phase 4 ItemId ASSIGNMENT + side-table population (`item_def_paths`, `item_synthetic_kinds`, `domain_attrs.item_attrs` init) + Phase-2-handle-to-final-ItemId remap machinery (D-12-5.3) | `regseq-driver` | P1a-DS-01, P1a-DS-07, P1a-DS-08a, P1a-DS-08b, P1a-DS-09, P1a-VAL-02 | The 5-phase pipeline skeleton without per-construct translation. The dependency on P1a-VAL-02 ensures the F4 comparator the bridge calls is the spec-pinned one (sort check folded into VAL-02 per the post-draft Codex revision). | integration-in-tests (drive against a no-op `#![no_std]` crate fixture; verify Phase 1 closure, Phase 3 deterministic ItemId assignment) |
| P1a-BR-02 | TyKind translation (§15.0) + special-ADT dispatch + §15.1 rejection enforcement (type-translation rows) + region erasure with substantive-HRTB rejection (§17.1) + Appendix A.1 `*mut c_void` rejection fixture | `regseq-driver` | P1a-BR-01, P1a-DS-02, P1a-DS-03, P1a-DS-04 | Per-`TyKind` translation table per `spec.md:5849-5864`. Special-ADT ordering (PhantomData / UnsafeCell / ManuallyDrop lang-item-flagged; Box / c_void / Atomic* DSL-rejected). Substantive-HRTB binder predicate rejection per §17.1. **Test Plan includes the Appendix A.1 `*mut c_void` rejection fixture** (`spec.md:13028+`) as an explicit integration-test artifact. | unit-in-src + integration-in-tests (against fixture rustc compilations exercising each TyKind row + Appendix A.1) |
| P1a-BR-03a | Const bridge translation (§9.7) + reference-typed const handling + ConstMemoryStatic Item synthesis trigger (§3.4 Phase 2 trigger table) | `regseq-driver` | P1a-BR-02, P1a-DS-05, P1a-DS-08b | Per `spec.md` §9.7 const dispatch table. Reference-typed const handling synthesises ConstMemoryStatic Items during bridge Phase 2/4/5 (Codex coverage finding ERROR-2: synthesis is bridge-side, not data-shape). The synthesis machinery for ConstMemoryStatic lives here; closure / vtable / drop-glue synthesis lives in P1a-BR-04. | unit-in-src + integration-in-tests |
| P1a-BR-03b | FnSig translation (§14) + C-ABI admissibility (§14.5) + CAbiLowering output | `regseq-driver` | P1a-BR-02, P1a-DS-06 | Per `spec.md` §14 FnSig schema + §14.5 CAbiLowering Direct / Substituted shape. Stores the bridge's outcome of CAbiPositionAdmissible / CFieldAdmissible predicates. | unit-in-src + integration-in-tests |
| P1a-BR-04 | Closure + vtable + drop-glue Item synthesis (§13 + §3.4 Phase 2 trigger table): ClosureCaptures / ClosureCall / ClosureDropGlue / VTableType / VTableInstance / VTableShim / drop-glue Function Items. Phase-2 working caches per family; Phase-5 re-key to final ItemId. `RegseqModel.vtables` (`HashMap<PrincipalTraitKey, ItemId>`) population in Phase 5. | `regseq-driver` | P1a-BR-02, P1a-DS-08a, P1a-DS-08b | The synthesis-interleaving rule (§3.4 D-12-5.2) requires synthesis to fire **during** Phase-2 translation. | integration-in-tests |
| P1a-BR-05 | Place translation (§19.5) + Operand translation (§20.3) + RuntimeChecks operand-surface lowering (§20.3.1) + Rvalue translation (§21.6) + Statement translation (§22.6) + Statement-site rejections (§22.5.1) | `regseq-driver` | P1a-BR-03a, P1a-BR-03b, P1a-DS-11, P1a-DS-12a, P1a-DS-12b, P1a-DS-13a | The per-construct translation rules from §19.5 / §20.3 / §21.6 / §22.6. Coverage is body-content from the bottom up. | unit-in-src + integration-in-tests |
| P1a-BR-06 | Terminator translation (§23.7) + Drop-as-Call collapse (§23.4 / §25.7.1) + basic-block translation (§24.7) | `regseq-driver` | P1a-BR-05, P1a-BR-04, P1a-DS-13b, P1a-DS-14a | Drop-as-Call runs **inside** the atomic per-rustc-terminator translation envelope (§3.4 Phase 5 body-construction); the collapse targets the drop-glue Function Item synthesised by P1a-BR-04. No `Drop` variant survives in RegseqModel post-collapse. | unit-in-src + integration-in-tests |
| P1a-BR-07 | Body materialisation (§25.7) + LocalDecls allocation API (§25.8) + vtable-indirect rewrite (§25.7.2 post-construction Phase-5 pass) + joint validation walk (§25.6) wiring | `regseq-driver` | P1a-BR-06, P1a-BR-04, P1a-DS-14b, P1a-VAL-01 | Phase-5 body materialisation populates `InstanceKind::Function.body`. Vtable-indirect rewrite consumes the vtable Items synthesised by P1a-BR-04 and runs **after** Drop-as-Call collapse and **before** joint validation (§25.7 ordering rule). Joint validation walk invokes P1a-VAL-01's Place walker over body content. | integration-in-tests (against fixture crates exercising trait-object dispatch + drop-glue) |
| P1a-BR-08a | Diagnostic-logging intrinsic recognition (§3.6.4 Phase-2 recognition codepath) + IR-shape contract (§26.1-§26.5 author surface + post-expansion shape) + per-callee `domain_attrs.item_attrs` population with `IntrinsicKind::Log(_)` | `regseq-driver` | P1a-BR-07, P1a-DS-09 | The recognition codepath that fires during Phase-2 translation when a Call's callee matches a `regseq::log::<sev>!` macro emission shape. Populates `domain_attrs.item_attrs[item]` Vec with `IntrinsicKind::Log(LogLevel)` AttrData per §3.6.4. | unit-in-src + integration-in-tests |
| P1a-BR-08b | DiagLogPeel body-level pass (§25.7.3 / §26.6) + format-template grammar parsing (§26.7) + bridge-side validation diagnostics (§26.8) | `regseq-driver` | P1a-BR-08a | The body-level transform that peels the slice-aggregate `LogArg` carrier into a flat-arg Call (post-peel `I-DiagLog-CallArity` shape per §23.6.2 B-4 / B-6 exceptions); the format-template grammar parser (closed subset per §26.7: `{}` plus the explicit modifiers like `{:x}` / `{:#010x}`; Debug formatters such as `{:?}` are parsed only to reject per D3 at `spec.md:12840`); bridge-side validation diagnostics per §26.8. **No C-emission** -- the §26.9 backend lowering contract is phase-1b. | unit-in-src + integration-in-tests (against a minimal fixture using `regseq::log::error!("...", x, y)` and verifying the post-peel Call shape) |

---

### Axis 4: Pretty-printer tasks (8 tasks; target crate `regseq-model`)

**Rationale.** The §8 per-method ownership table at `pretty-printer.md:1148-1170` is the cleanest decomposition oracle. It enumerates 18 methods total: 2 stage-1-shipped helpers (`render_span`, `render_def_path_value`), 1 stage-12 method (`render_def_path`), 12 stage-13 building blocks (the type / item / body-level renderers), and 3 stage-14 composers (`render_const`, `render_instance`, top-level `render`).

The post-draft Codex right-sizing review surfaced two splits relative to the v1 draft: (i) `render_const` (§10.1) splits between scalar/simple values (PP-04a) and aggregate/enum/bytes/reloc recursion (PP-04b) because §10.1's 8 sub-decisions across 11 closed Const variants exceeds the size threshold; (ii) `render_instance` (§10.2) splits between Function/Static/Const variant rendering (PP-05a) and ADT-definition (struct/enum/union typedef) variant rendering plus attribute-stream composition (PP-05b) because §10.2 is the largest pretty-printer composer. PP-06 retains only the top-level `render` (§10.3) plus PrettyPrintOptions opt-in plumbing; the per-construct §3 envelope conventions are split across PP-01 (Item-header), PP-03 (body-level), and PP-05a (function envelope), and the per-construct §4 span sources (Statement / Terminator / LocalDecl / Item header) are owned by PP-01 and PP-03 per the post-draft Codex coverage review.

The stage-1 helpers (`render_span`, `render_def_path_value`) and stage-12 (`render_def_path`) are already shipped per `crates/regseq-model/src/pretty.rs` and `crates/regseq-model/src/def_path.rs::render_def_path`; phase-1a does not re-task them.

| Task ID | Name | Target | Depends on | Scope summary | Test Plan kind |
|---|---|---|---|---|---|
| P1a-PP-01 | Item-level rendering (§5): `render_item(ItemId) -> String` + `render_item_attribute_block` shared helper (§5.6) + §5.3 per-flag rendering table + §5.4 per-ItemKind variant rendering + §5.5 layout + **§4 span gating for Item header source position** | `regseq-model` | P1a-DS-07 | Per `pretty-printer.md` §5.1-§5.6. The shared §5.6 helper is consumed by both `render_item` and (later) `render_instance` per D-14-5.7. Specimens from §9.9 (`render_item` specimens) are the snapshot baseline. Owns the §4 span / debuginfo gating for the Item header source position. | unit-in-src + integration-in-tests (insta snapshots in `crates/regseq-model/tests/pretty.rs`) |
| P1a-PP-02 | Type rendering (§6): `render_ty(TyId) -> String` per-arm across the 19-variant Ty enum + §6.7 `render_generic_args(args, item)` helper with the stage-14 D-14-4.9 signature | `regseq-model` | P1a-DS-02, P1a-DS-04, P1a-DS-08a, P1a-DS-08b | The §6.1-§6.6 per-arm tables + §6.7 generic-args helper. Specimens 1-25 from §9.1 are the snapshot baseline. The synthesised vtable struct DefPath form (`<regseq_synth>::vtable::<Trait>::for::<ImplTy>`) per §13.4 is exercised by Specimen 23. | unit-in-src + integration-in-tests |
| P1a-PP-03 | Body-level building-block renderers (§7) + **§4 span gating for Statement / Terminator / LocalDecl source positions** | `regseq-model` | P1a-PP-02, P1a-DS-11, P1a-DS-12a, P1a-DS-12b, P1a-DS-13a, P1a-DS-13b, P1a-DS-14a, P1a-DS-14b | The 9 public renderers + 1 internal helper from §7.1-§7.10 (`render_place` / `render_operand` (pair-form per D-14-4.9) / `render_rvalue` (pair-form per D-14-4.9) / `render_statement` / `render_terminator` / `render_terminator_kind` (internal) / `render_call_callee` / `render_switch_targets` / `render_assert_kind` / `render_basic_block`). Specimens 26-... from §9.2-§9.8 are the snapshot baseline. Pair-form signatures thread consumer-context Ty so they can compose with `render_const` in P1a-PP-04a/b. **Owns §4 span / debuginfo gating** for Statement / Terminator / LocalDecl source positions. | unit-in-src + integration-in-tests |
| P1a-PP-04a | Composer: `render_const(ConstId, TyId) -> String` for scalar / simple variants (§10.1.1-§10.1.2) | `regseq-model` | P1a-PP-02, P1a-PP-03, P1a-DS-05 | Per-variant dispatch across simple Const variants: Int / Uint / Float / Bool / Char / ZST. Sentinel collapsing (`i32::MAX` etc.) + decimal-rendering precision + Char Debug + Float finite + non-finite (`inf_f32` / `NaN_f64`) per §10.1.1. ZST consumer-Ty dispatch per §10.1.2. | unit-in-src + integration-in-tests |
| P1a-PP-04b | Composer: `render_const(ConstId, TyId) -> String` for aggregate / enum / bytes / reloc variants with recursion (§10.1.3-§10.1.7) | `regseq-model` | P1a-PP-04a | Per-variant dispatch across the recursive Const variants: Aggregate / EnumValue / Bytes / NullPtr / Reloc. Aggregate / EnumValue recursion threads the per-field Ty resolved from the parent's layout per D-14-4.9. Specimens 47+ from §9.10 are the snapshot baseline (across both 04a and 04b). | unit-in-src + integration-in-tests |
| P1a-PP-05a | Composer: `render_instance(InstanceId) -> String` for Function / Static / Const InstanceKind variants (§10.2.1-§10.2.3) + body envelope composition + §3 conventions for function-body envelope | `regseq-model` | P1a-PP-01, P1a-PP-04b, P1a-DS-09, P1a-DS-10 | Per-variant rendering for Function (§10.2.1; composes `render_def_path` + `render_ty` + body envelope + body-level renderers) / Static (§10.2.2) / Const (§10.2.3). Owns the §3 function-body envelope conventions. Specimens from §9.11 (Function/Static/Const) are the snapshot baseline. | unit-in-src + integration-in-tests |
| P1a-PP-05b | Composer: `render_instance(InstanceId) -> String` for ADT-definition InstanceKind variants (Struct / Enum / Union typedef Items per §10.2.4-§10.2.6) + §10.2.7 attribute-stream composition (Stream 1 via §5.6 helper; Stream 2 via `domain_attrs.item_attrs` walk) | `regseq-model` | P1a-PP-05a, P1a-DS-09, P1a-DS-10 | Per-variant rendering for the ADT-definition Instances. Two-stream attribute-block composition per §10.2.7 (Stream 1 via the §5.6 shared helper from P1a-PP-01; Stream 2 via `domain_attrs.item_attrs[item]` Vec-index walk with per-AttrData variant render rule). Specimens from §9.11 (ADT specimens) are the snapshot baseline. | unit-in-src + integration-in-tests |
| P1a-PP-06 | Composer: top-level `render(&self) -> String` (§10.3) + `PrettyPrintOptions` opt-in machinery + §4 options propagation only (per-construct span emission lives in PP-01 / PP-03) | `regseq-model` | P1a-PP-05b | Iterates `model.instances.iter_enumerated()` in InstanceId order (canonical F4-2 sort per `spec.md` §16.3; rank-grouped via §9.3 `regseq_instance_kind_rank`); composes per-Instance via `render_instance(InstanceId)`; single blank line between consecutive Instances; no file-level header / trailer; empty `model.instances` -> empty `String`. **Owns only options propagation** for `PrettyPrintOptions { include_spans, include_debug_names }`; per-construct span emission lives in PP-01 (Item header) and PP-03 (body-level). Specimens from §9.12 are the snapshot baseline. | unit-in-src + integration-in-tests (insta snapshots; full-model golden) |

---

### Axis 5: Test-support + end-to-end fixture tasks (3 tasks; target crates `test-ir-fixtures` (new workspace member) + `tests/e2e`)

**Rationale.** The prompt directs: "at the close of phase-1a, a small cumulative set exercising the whole 1a surface (e.g., a no-op function, a diagnostic-logging fixture, a trait-object dispatch fixture, a closure-as-struct fixture). These are integration tests, not unit tests." Each fixture exercises a different bridge codepath against the cumulative phase-1a IR substrate; the pretty-printed output is snapshot-tested against a frozen baseline.

The post-draft Codex review surfaced two merges relative to the v1 draft: (i) the test-support crate scaffold + the no-op fixture combine into a single task because the no-op is the natural smoke test that the scaffold is correct -- standing them up separately is over-decomposition; (ii) the trait-object and closure fixtures combine into a single synthesis-e2e task because both exercise the synthesis families landed by P1a-BR-04 against the cumulative pipeline -- one fixture file with two test functions is sufficient.

A new test-support crate `test-ir-fixtures` is approved by the user. The crate is registered as a workspace member alongside the existing `*-old` crates. The harness's `tasks.json` group `test-ir-fixtures` is added by hand (one-off direct edit; the `task.py add-task` CLI requires the group to pre-exist, but the harness does not currently expose an `add-group` subcommand).

| Task ID | Name | Target | Depends on | Scope summary | Test Plan kind |
|---|---|---|---|---|---|
| P1a-TS-01-E2E-01 | `test-ir-fixtures` test-support crate scaffold + canonical shared fixtures + helpers + the no-op smoke fixture as the first end-to-end test | `test-ir-fixtures` (new) + `tests/e2e` | P1a-BR-01 .. P1a-BR-08b, P1a-PP-06 | Bootstraps the `crates/test-ir-fixtures/` workspace member with `Cargo.toml` + canonical Rust fixture programs (`fn no_op() {}`; the four diagnostic-logging severity calls; a `dyn Reader` trait-object call site; a `Fn(u32) -> u32` closure expression). Provides helpers for invoking the bridge against each fixture and exposing the resulting `RegseqModel` to integration tests. **First fixture as smoke test**: bridge `fn no_op() {}` end-to-end and assert the model serialises, validates, and pretty-prints; insta snapshot of the pretty-printed output. | test-support-crate + integration-in-tests (the merged scaffold + smoke) |
| P1a-E2E-02 | Diagnostic-logging fixture: bridge a function that calls all four severities + assert the four post-DiagLogPeel flat-arg Call shapes + assert the `IntrinsicKind::Log(_)` attribute on each callee's `domain_attrs.item_attrs` + pretty-printed snapshot | `tests/e2e` | P1a-TS-01-E2E-01, P1a-BR-08a, P1a-BR-08b | Asserts that `LogArg` does NOT survive into RegseqModel post-peel (per `spec.md` §26.4 carrier-survival rule) and the `I-DiagLog-CallArity` invariant. | integration-in-tests |
| P1a-E2E-03 | Synthesis fixture (combines trait-object dispatch + closure-as-struct): one fixture file with two test functions exercising (a) the `dyn Reader` synthesised VTableType + VTableInstance Items + post-vtable-indirect-rewrite Call shape (per §25.7.2; Indirect callee + 4-5 fresh locals), and (b) the `\|x\| x + 1` closure synthesised ClosureCaptures Item + parallel `Ty::Struct(ClosureCaptures, args, layout)` for layout + ClosureCall Function Item; pretty-printed snapshots for both | `tests/e2e` | P1a-TS-01-E2E-01, P1a-BR-04, P1a-BR-07 | Exercises both synthesis families landed by P1a-BR-04 (vtable + closure) plus the cumulative bridge pipeline (P1a-BR-07's vtable-indirect rewrite). Two test functions in one fixture file per Codex right-sizing. | integration-in-tests |

P1a-E2E-* tasks register under the `regseq-model` group (since the assertion surface is the RegseqModel + its pretty-printer); P1a-TS-01-E2E-01 registers under the `test-ir-fixtures` group (since its primary deliverable is the crate scaffold).

---

### Cross-cutting authoring constraints

1. **Test discipline per task spec.** Each task spec's Test Plan section names exactly one or more of `unit-in-src` (functions inside `crates/<crate>/src/<module>.rs` `#[cfg(test)] mod tests`), `integration-in-tests` (test files under `crates/<crate>/tests/`), or `test-support-crate` (cross-crate fixtures under the `test-ir-fixtures` crate).

2. **Public API first.** Per `SESSION.md` "Doing the Work" §6-§10, each task spec instructs the worker to define the public API (struct fields, enum variants, function signatures) with `todo!()` bodies first, then write tests against the API, then implement.

3. **Doc comments mandatory** on all public types and functions (`SESSION.md` "Coding Conventions").

4. **No unicode symbols in documentation or comments** (`SESSION.md` "Coding Conventions").

5. **Carry forward caution notes** for any accepted-with-caution constructs. The most load-bearing of these for phase-1a are:
   - The §10.3 wide-scalar admission (`Int(I128)`, `Uint(U128)`, `Float(F16)`, `Float(F128)`) is **bridge-side unconditional**; rejection is target-dependent and lives at the codegen boundary (phase-1b territory).
   - The §10.2 `Never` admission as a `Ref`/`Ptr` pointee is **type-level only**; backend may reject when emitting.
   - The §3.1 invariant 19 `opaque_handle_eligible` is **non-generic only** in v1; the bridge rejects generic candidates with `BridgeError::GenericOpaqueHandle`.
   - The §17.1 substantive-HRTB rejection covers both projection / nested / non-ref-slot trait predicates AND relational binders.
   - The §15.1 `core::sync::atomic::{fence, compiler_fence}` rejection is intentional v1-scope shrink, not a bridge bug.

6. **No invented DSL primitives or backend behavior** beyond what the locked phase-1 spec justifies. The §26 diagnostic-logging vocabulary at phase-1a is **recognition + IR shape only**; the per-target backend lowering (§26.9 + `generic-c-backend.md` §8) is phase-1b.

7. **Per-task `spec_refs`** populated via `task.py read-spec-refs docs/discussions/regseq-model/spec.md` and `... pretty-printer.md`. Subset of this brief's full `Spec References` list, scoped to the task's covered sections.

## Spec References

The full per-section set this batch covers. Per-task `spec_refs` arrays are subsets.

- `spec.md#1` (Mission and IR posture)
- `spec.md#2` (ID system)
- `spec.md#3` (Top-level RegseqModel)
- `spec.md#4` (StringInterner)
- `spec.md#5` (Source files and spans)
- `spec.md#6` (CrateInfo)
- `spec.md#7` (DefPath)
- `spec.md#8` (Item)
- `spec.md#9` (Instance and InstanceKind; covers Const vocabulary §9.7)
- `spec.md#10` (The Ty enum)
- `spec.md#11` (Layout)
- `spec.md#12` (Type interning)
- `spec.md#13` (Synthesised Items)
- `spec.md#14` (FnAbi and FnSig schema)
- `spec.md#15` (Bridge translation rules + rejections; §15.1 enforced at bridge sites)
- `spec.md#16` (Determinism F4 / F4-2)
- `spec.md#17.1` (Region erasure + substantive-HRTB rejection)
- `spec.md#17.2` (IR-visible helpers like `contains_unsafe_cell`; **fat-pointer mutability is phase-1b**)
- `spec.md#18` (Pretty-printer pointer; stub in `spec.md`, surface in `pretty-printer.md`)
- `spec.md#19` (Place and projections)
- `spec.md#20` (Operand vocabulary)
- `spec.md#21` (Rvalue vocabulary)
- `spec.md#22` (Statement vocabulary)
- `spec.md#23` (Terminator vocabulary)
- `spec.md#24` (Basic-block vocabulary)
- `spec.md#25` (Body-level vocabulary)
- `spec.md#26.1` .. `spec.md#26.8` (Diagnostic-logging recognition + IR shape; **§26.9 backend lowering is phase-1b**)
- `spec.md#appendix-a` (Bridge-rejection examples)
- `pretty-printer.md#1` (Mission and consumer model)
- `pretty-printer.md#2` (Master rendering style)
- `pretty-printer.md#3` (Body envelope, indentation, sigil conventions)
- `pretty-printer.md#4` (Span-rendering and debuginfo modes)
- `pretty-printer.md#5` (Item-level rendering shape)
- `pretty-printer.md#6` (Type rendering)
- `pretty-printer.md#7` (Body-level rendering)
- `pretty-printer.md#8` (Per-method ownership table)
- `pretty-printer.md#9` (Worked-example specimens)
- `pretty-printer.md#10` (Composer renderers)
- `pretty-printer.md#11` (Cross-references to spec.md)

## Phase

`1a`

Inherited by every task this brief spawns via `task.py add-task --phase 1a`.

## Review Tiers

| Tier | Scope | Trigger | Mandatory? |
| ---- | ----- | ------- | ---------- |
| Task Review | Per-task | After task tests pass; per `.harness/SESSION.md` Review section using `codex_runner.py` | Yes |
| Batch Review | Per-batch cross-task | After all phase-1a task IDs are `done` | **Yes (opt-in flagged)** -- phase-1a is a coherent multi-task feature (the cumulative bridge + IR + pretty-printer surface). Batch Review covers inter-task consistency on shared types (e.g., the `Place` / `PlaceElem` definitions consumed by both `regseq-model` validators and the `regseq-driver` bridge), invariant alignment across the data-shape and validator axes, and snapshot stability across the pretty-printer task set. |
| Phase-Close Review | Per-phase | `task.py phase-status 1a` returns PASS | Yes |

Per `.harness/SESSION.md` Review section, each Task Review invocation uses the four-block `codex_runner.py` template (task / grounding_rules / structured_output_contract / default_follow_through_policy). Batch Review uses the same pattern with an expanded `Files to audit` list across all phase-1a tasks. Phase-Close Review files a phase-closure brief using `.harness/spec-batches/PHASE_CLOSURE_BRIEF_TEMPLATE.md`.

## Non-Goals

Phase-1a explicitly does NOT cover:

1. **`generic-c-backend.md` content.** Every per-`Ty` C typedef table, fat-pointer C struct shape, `PlaceElem` / `Operand` C lvalue form, per-construct C lowering, intrinsic C-emission rule, and per-target call shape that lives in `generic-c-backend.md` is **phase-1b** territory per `docs/discussions/regseq-model/README.md` §1.3.

2. **Diagnostic-logging C-emission.** The §26.9 backend lowering contract (per-target printf-family emitter shape with leading severity tag, format-specifier mapping, the generic-C target's binding) is phase-1b. Phase-1a covers §26.1-§26.8 (author surface, intrinsic recognition, IR shape, DiagLogPeel pass, format-template grammar, bridge-side validation diagnostics).

3. **§17.2 fat-pointer mutability rules.** Backend-side lowering concern; phase-1b. Phase-1a covers §17.2's IR-visible helpers (e.g., `contains_unsafe_cell`) only.

4. **Per-construct C lowering subsections.** Every "C lowering" subsection across the body-content sections cross-references `generic-c-backend.md`; specifically the following anchors are deliberately uncovered: `spec.md#19.6`, `spec.md#20.4`, `spec.md#21` C-lowering rows, `spec.md#22` C-lowering rows, `spec.md#23` C-lowering rows, `spec.md#25.10`, `spec.md#26.9`. Phase-1a does not implement the C side.

5. **Linux backend.** All Linux-target codegen (`linux-backend.md` when it lands per phase-2 stage 15+) is out of scope. The §2 README rule mandates the Linux backend is a separate codegen path; phase-1 ships only the generic-C backend (phase-1b territory), not the Linux backend.

6. **Domain extensions beyond the diagnostic-logging foundation.** MMIO (stage 16+), interrupts (stage 17+), atomics (stage 18+), clock/reset, and every other `#[rseq::*]` domain family are phase-2+ territory.

7. **DSL -> IR -> C -> captured-stdout test suite.** This is the gate for **phase-1b close** per README §1.3. Phase-1a closes on the IR-side + pretty-printer-side surface alone.

8. **Spec authoring or revision.** The phase-1 spec is closed. If a phase-1a worker discovers an inconsistency, the worker files an inbound expectation per the README §2 "Forward expectations" rule (not a same-batch spec edit).

9. **Migration of `*-old` reference crates.** `regseq-model-old`, `regseq-driver-old`, etc. are reference material per `docs/discussions/regseq-model/README.md` §6.1; they are not touched by phase-1a tasks.

10. **Old archived task IDs.** All items in `.harness/task-spec-status.yaml` whose status is `archived` are superseded by this batch. Phase-1a tasks do not reference, depend on, or revive any archived task.

## Acceptance Criteria

Phase-1a is complete when:

1. Every phase-1a task spec listed in `## Implementation Notes` is written to `.harness/tasks/<group>/<NN>-<topic>.md`, registered via `task.py add-task --phase 1a --spec-refs ...`, and has status `done`.

2. `uv run python scripts/python/task.py phase-status 1a` returns PASS (every phase-1a task is `done` and all per-task dependencies are `done`).

3. The cumulative workspace passes the authoritative quality gate (`uv run python scripts/python/task.py done <last-id>` runs the full workspace-wide verification per `SESSION.md` "Testing Requirements": `cargo test --workspace` + `cargo fmt --all -- --check` + `cargo clippy --workspace -- -D warnings`).

4. `uv run python scripts/python/task.py spec-coverage docs/discussions/regseq-model/spec.md --include-subsections` reports zero **uncovered** anchors across `spec.md` §1-§17.1 + §17.2 IR-helper subset + §18-§25 + §26.1-§26.8 + Appendix A.1. The following subsection anchors are deliberately uncovered (phase-1b carve-outs per `## Non-Goals`): `spec.md#26.9`, `spec.md#19.6`, `spec.md#20.4`, every `spec.md#21.X` / `spec.md#22.X` / `spec.md#23.X` C-lowering subsection, `spec.md#25.10`, and the §17.2 fat-pointer mutability subsection. The `spec-coverage` report's `UNCOVERED` set MUST equal exactly this list; any additional uncovered anchor is a phase-1a coverage gap.

5. `uv run python scripts/python/task.py spec-coverage docs/discussions/regseq-model/pretty-printer.md --include-subsections` reports zero gaps across §1-§11.

6. Task Review verdict APPROVED per task; Batch Review verdict APPROVED across the cumulative phase-1a artifact; Phase-Close Review filed via `.harness/spec-batches/PHASE_CLOSURE_BRIEF_TEMPLATE.md` and verdict APPROVED.

7. `.harness/task-spec-status.yaml` has the `regseqmodel-phase-1a` item at status `tasked` with `generated_task_ids` populated.

8. A `regseqmodel-phase-1b` item exists in the tracker at status `not_ready` (or `ready_to_elaborate` if phase-1a closes before its dependency-graph promotion runs) with `depends_on: [regseqmodel-phase-1a]`.

## Update Instructions

After the phase-1a task specs are written and registered:

1. **Tracker item status flip.** Change the `regseqmodel-phase-1a` item in `.harness/task-spec-status.yaml` from `ready_to_elaborate` (or `in_elaboration`) to `tasked`.

2. **Populate `generated_task_ids`.** Add every phase-1a task ID returned by `task.py add-task` calls.

3. **Promote next dependency-satisfied item.** Move `regseqmodel-phase-1b` from `not_ready` to `ready_to_elaborate` once phase-1a is `tasked`.

4. **Do not touch unrelated tracker items.** Every item with `status: archived` stays `archived`.

5. **No partial-state flip.** If brief authoring is partially complete, leave the item at `in_elaboration`.

6. **Phase-Close-Review obligation.** After phase-1a is `done` and `phase-status 1a` returns PASS, file the phase-closure brief at `.harness/spec-batches/regseqmodel-phase-1a-closure.md`.

---

## Decomposition rationale

**Total task count: 42** (18 data-shape + 3 validator + 10 bridge + 8 pretty-printer + 3 test-support+E2E). Exceeds the original 25-40 target band by 2; per the post-draft Codex right-sizing review, 42 is the minimum count that prevents tasks from exceeding the size threshold (the reference task `archive/.harness/tasks/regseq-model/24-place-and-projection.md` is ~117 lines of spec yielding one focused module). The granularity is biased toward "narrow + testable" per `task-spec-workflow.md` Authoring Rules §7: every task fits in one session.

**v1-to-v2 changes (Codex right-sizing review feedback applied):**

- **Splits (+8 tasks)**: DS-08 -> DS-08a (closure family) + DS-08b (vtable family + synthetic-key + DefPath/Hash); DS-12 -> DS-12a (Operand) + DS-12b (Rvalue + sub-enums); DS-13 -> DS-13a (Statement) + DS-13b (Terminator); DS-14 -> DS-14a (BasicBlock) + DS-14b (Body + LocalDecl + APIs + walk hooks); BR-03 -> BR-03a (Const + ref-const + ConstMemoryStatic synthesis trigger) + BR-03b (FnSig + CAbi); BR-08 -> BR-08a (recognition + attribute population) + BR-08b (DiagLogPeel + format + validation); PP-04 -> PP-04a (scalar render_const) + PP-04b (aggregate/recursion render_const); PP-05 -> PP-05a (Function/Static/Const render_instance) + PP-05b (ADT-definition render_instance + attribute streams).
- **Reductions (-3 tasks)**: deleted VAL-04 (§15.1 rejection enforcement is at bridge sites; closed-enum exhaustiveness handles model-side; F4/F4-2 sort folded into VAL-02). Merged TS-01 + E2E-01 into a single P1a-TS-01-E2E-01 (the no-op fixture is the natural smoke for the scaffold). Merged E2E-03 + E2E-04 into a single P1a-E2E-03 (one fixture file with two test functions covers both vtable + closure synthesis families).
- **Coverage corrections**: DS-01 explicitly extends foundation conformance for §2 / §4 / §5 / §6 / §7. DS-02 explicitly takes ownership of §17.2 IR-visible helpers (fat-pointer mutability stays phase-1b). VAL-02 explicitly enriched with §3.3 surfaces (DefPath rendered validation; source-span bounds; Phase C `vtables` rebuild and duplicate rejection). VAL-03 explicitly enriched with §3.3 surfaces (target/build validation; FnSig/CAbiLowering checks; const-graph acyclicity; domain-attr validation; source-file validation). DS-05 narrowed to data-shape only (ConstMemoryStatic synthesis moved to BR-03a). PP-06 narrowed to top-level render + options propagation only (§3 envelope ownership distributed across PP-01 / PP-03 / PP-05a; per-construct §4 span sources owned by PP-01 / PP-03). BR-02 Test Plan explicitly includes the Appendix A.1 `*mut c_void` rejection fixture.
- **Dependency / ordering corrections**: BR-06 + BR-07 explicit dependency on BR-04 (Drop-as-Call collapse and vtable-indirect rewrite consume BR-04's synthetic Items). BR-03a depends on DS-08b (synth machinery share). PP-05a / PP-05b depend on DS-09 + DS-10 (instance + domain-attr surfaces).
- **Acceptance Criteria #4 rewrite** to enumerate the deliberately-uncovered anchors explicitly (was unverifiable as "zero gaps modulo carve-outs").

**Judgement calls preserved from v1.**

- Ty enum (§10) stays as one task (DS-02), not 19 per-variant tasks. Variants are densely related (every variant fits one `match` arm in `render_ty` + one row in §15.0).
- Const (§9.7, P1a-DS-05) and Instance (§9.1-§9.4, P1a-DS-10) stay as separate tasks because Const has its own 11-variant closed enum + interner + content-keyed identity scheme that is reusably independent of Instance's per-monomorphisation identity (Codex explicitly noted "do not merge DS-05 with DS-10").
- The Item table (§8 + §13) stays split between core Item (DS-07) and synthesised-Item machinery (DS-08a + DS-08b); the §13 axis itself splits as noted in the v1-to-v2 changes.

**Test-support crate.** `test-ir-fixtures` -- a new workspace member registered under `crates/test-ir-fixtures/` housing the shared Rust fixture programs and the helper that drives the bridge against each fixture and exposes the resulting `RegseqModel` to integration tests. Justified per the prompt's `## Hard constraints` §2 rule "Test-support crate (`test-X` in the workspace): when 2+ crates need to share non-trivial test fixtures, mocks, or setup code." Approved by user.

**Spec sections deliberately scoped out of phase-1a** (matching the AC #4 carve-out list):

- All `generic-c-backend.md` content (phase-1b per README §1.3 + §5.9 rule 7).
- `spec.md` §26.9 (Backend lowering contract for diagnostic logging) -- phase-1b.
- `spec.md` §17.2 fat-pointer mutability subsection -- phase-1b.
- Every `spec.md` §X.Y "C lowering" subsection across §19-§25 (specifically: §19.6, §20.4, §21.x C-lowering rows, §22.x C-lowering rows, §23.x C-lowering rows, §25.10) -- phase-1b.
- All Linux-backend material -- phase-2.
