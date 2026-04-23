# RegseqModel Rebuild: Legacy Removal Plan

## Status

Draft for review. Incorporates Codex review of an earlier iteration.

This plan is narrower and more recent than `execution-plan.md`. Where
they disagree, this plan wins — it reflects the current decision to
pause backends and analyses entirely, rebuild the Rust -> RegseqModel
path, and reintroduce downstream layers only after the core is stable.

## Goal

Reach a state where:

1. `regseq-driver` (rustc-linked) builds a `RegseqModel` directly from
   Rust HIR/MIR.
2. `regseq-model` serializes the result to JSON (`--emit model`).
3. No legacy IR layers (`DeclarationModel`, `BindingModel`,
   `JoinedArtifact`, `SemanticModel`, `ItemStore`, `CanonicalPath`,
   `SemanticType`) remain in the tree.
4. No intermediate `RawArtifact` schema exists; the 13 extraction
   surfaces are inlined into the construction pass.
5. Analyses and backends are deactivated; they will be re-ported to
   `RegseqModel` in a later effort.

The RegseqModel JSON output is the handoff artifact used to:

- Author golden C output files by hand as references.
- Drive test fixtures.
- Guide future backend-development agents.

## Target architecture

```
Rust source
  |
  |  regseq-driver (rustc-linked construction + Phase 1 population)
  v
RegseqModel (Phase 1)  ---- --emit model
  |
  |  regseq-model (rustc-free) Phase 2 platform join
  v
RegseqModel (Phase 2)  ---- --emit model
  |
  |  (analyses re-ported later)
  v
                       ---- --emit analysis
  |
  |  (backends re-ported later)
  v
C driver output        ---- --emit driver
```

## Motivation

1. **Spec is explicit that the legacy is dead code.** `spec.md` Section 1
   lines 62-84 states the existing codebase reflects "an earlier
   evolutionary stage" with `DeclarationModel` + `BindingModel` +
   `JoinedArtifact`, `Expr`/`Stmt` semantic IR, and
   `CanonicalPath`/`ItemId` identity — "Implementation work will rewrite
   or refactor the codebase to match these spec documents."

2. **No capability is preserved by keeping the legacy.** Backends do
   not generate correct C today, so there is no operational output to
   protect.

3. **Confusion cost is real.** The current tree has four parallel type
   systems (`RegseqType`, `MirTy`, `SemanticType`, `item::*`) and the
   "is this current?" question has already cost real time. Every new
   feature built on top risks landing on the wrong substrate.

4. **`RawArtifact` is redundant with `RegseqModel`.** Everything
   `RawArtifact` captures is either:

   - Re-shaped in `RegseqModel` (call sites become `IntrinsicCall`
     terminators, etc.),
   - An extraction-internal input (facade-crate registries), or
   - Transient diagnostics.

   Maintaining two schemas that evolve with every new device construct
   is pure overhead.

5. **The new surface is smaller and focused.** `--emit model` +
   eventual `--emit analysis` + `--emit driver` — one authoritative
   shape, one extraction pass, one JSON.

## Scope

### In scope

- Delete legacy IR and backend pipeline code.
- Delete `RawArtifact` and its surface structs.
- Move normalize logic into `regseq-driver/src/construction/`.
- Add `EmitSpec::Model` (named `model` in CLI).
- Temporarily remove or stub `EmitSpec::Driver` and `EmitSpec::Analysis`.
- Update `spec.md` to reflect the single-pass construction architecture.

### Out of scope (deferred)

- Porting `regseq-analysis` readiness/ordering analyses to RegseqModel.
- Porting backends (Linux, Zephyr) to RegseqModel.
- Authoring golden C output files.
- Full MIR body -> `ir::Body` translation (it starts here; completion
  is its own effort).

## What gets removed

Removal proceeds in ordered waves. Each wave must leave the tree
`cargo check`-clean before the next begins.

### Wave A — Driver emission and legacy backend chain

Removes the production consumers of the legacy model layers so the
legacy layers are free-standing for deletion.

- `crates/regseq-driver/src/emit_driver.rs` and the call site in
  `callbacks.rs:180-228`.
- `crates/regseq-analysis/src/legacy_input/` — transitional adapter.
- **`EmitSpec::Driver` and `EmitSpec::Analysis` variants are retained**
  (per DL-3); their dispatch in `regseq-driver` is replaced with a
  stub that errors or panics with a "not yet exercised" message.
- `EmitSpec::Driver` default at `cargo-regseq/src/cli.rs:180-182` is
  flipped to `EmitSpec::Model`.
- Tests gated on the legacy chain.
- **`crates/regseq-backends/`** — preserved through Wave A with a
  reduced surface. Audit output is
  `docs/discussions/models/regseq-model/regseq-backends-audit.md`;
  the DISCARD list there (11 files in `lowering/semantic_lowering/`,
  plus `lowering/{behavior_planner, body_interpreter, hierarchy,
  projection}.rs`, plus `lowering/mir_interpreter/{mod, context}.rs`,
  plus `linux/c_ast.rs`, plus `zephyr.rs`) is deleted in Wave A. The
  ADAPT and MIXED files stay; they are the donor content for the
  eventual backend re-port.

### Wave B — Legacy model layers

Atomic — these modules reference each other and must go together.

- `crates/regseq-model/src/binding/` — legacy `BindingModel`.
- `crates/regseq-model/src/joined/` — legacy `JoinedArtifact`.
- `crates/regseq-model/src/compat.rs` — migration bridge.
- `crates/regseq-model/src/item/` — `CanonicalPath`/`ItemId` identity
  scheme.
- `crates/regseq-model/src/semantic/` — `Expr`/`Stmt` semantic IR and
  `SemanticType`.
- `crates/regseq-model/src/declaration/bodies.rs` — dead Expr/Stmt body
  IR.
- `crates/regseq-model/src/declaration/mod.rs` legacy container bits.

Rewrite required: `regseq-analysis/src/readiness.rs:20-24` currently
imports `model::compat::CallbackFamily`. Replace with a model-native
equivalent or temporarily stub if readiness itself is being deactivated.

### Wave C — RawArtifact and raw-artifact normalize

Removes the intermediate JSON schema.

- `crates/regseq-driver/src/raw_artifact.rs`.
- `crates/regseq-driver/src/mir_repr.rs` re-export chain to
  `declaration::mir_body`.
- `crates/regseq-model/src/normalize/raw_artifact.rs`.
- Most of `crates/regseq-model/src/normalize/surface.rs` — its logic
  migrates into `regseq-driver/src/construction/` as per-domain
  builder methods.
- `EmitSpec::RawArtifact` variant (no replacement).
- `crates/regseq-driver/tests/source_to_extraction_*.rs` — replaced by
  new `source_to_regseq_model_*` tests.

### Wave D — Relocate extraction surface

Only after Waves A-C pass `cargo check`.

- `crates/regseq-model/src/declaration/mir_body.rs` relocates into
  `regseq-driver/` (per DL-5, settled). `MirTy`/`MirBody` are pure
  rustc-surface-lowering artifacts and live next to the rustc-linked
  code that produces them. `regseq-model` contains only the model
  definitions (`RegseqType`, `ir::Body`, etc.) — not the Rust-surface
  lowering types.

Design principle (from DL-5): **all Rust-surface-to-RegseqModel
lowering lives in `regseq-driver`; `regseq-model` contains only the
model definitions and model-level operations (verify, join,
serialize, query).**

## What stays

- `crates/regseq-driver/src/extract/` — the 13 per-domain extractors
  (`clock.rs`, `reset.rs`, `pinctrl.rs`, `interrupts.rs`,
  `register_access.rs`, `property.rs`, `synchronization.rs`,
  `timed_delay.rs`, `logging.rs`, `pm.rs`, `traits.rs`,
  `driver_identity.rs`, `callback_context.rs`, `source_locations.rs`,
  and supporting files). They remain — their role shifts from
  populating `RawArtifact` to populating `RegseqModel` fields directly.
- `crates/regseq-driver/src/construction/` — **the donor code**. Becomes
  the active `MirTy` -> `RegseqType` and `MirBody` -> `ir::Body`
  mapper. See especially `construction/mir_mapping.rs:153-245`.
- `crates/regseq-driver/src/extract/body.rs` and MIR query plumbing.
- `crates/regseq-driver/src/raw_artifact.rs` extraction-side production
  code is removed; the extractor calls now feed construction instead.
- `crates/regseq-model/src/{model, ir, identity, verify, join,
  platform.rs, platform_parser.rs, properties.rs, query}/` — target
  architecture.
- `crates/regseq-model/src/serialize.rs` — already implements
  `serialize_regseq_model(&RegseqModel) -> String`.
- `crates/regseq` — DSL facade.
- `crates/regseq-analysis/` minus `legacy_input/`, with `readiness.rs`'s
  `compat` import rewritten.

## What must be built (after removal)

Listed in rough dependency order; each is a separate effort.

1. **Callable-item construction** — `regseq-driver/src/construction/`
   must populate `free_functions`, `impl_blocks`, `trait_impls`, and
   `driver_impl_blocks`. Currently `normalize/surface.rs:155-195`
   initializes these empty.

2. **Body construction** — `MirBody` -> `ir::Body`. Seed from
   `construction/mir_mapping.rs`. Currently no production code
   populates `ir::Body`.

3. **Full `MirTy` -> `RegseqType` mapping** per `spec.md` Section 8
   lines 834-855.

4. **Eliminate silent scalar fallbacks** (`normalize/surface.rs:
   1123-1129, 1191-1195` and `construction/walk.rs:660-666, 707-715`)
   which violate spec Section 12's "explicit errors, never silent
   defaults."

5. **Fully-typed raw extraction** — `raw_artifact.rs` currently holds
   `serde_json::Value` for bodies and registries. Once surfaces feed
   construction directly, those untyped holes can close.

## CLI and emit surface changes

### Current (`regseq-config/src/emit.rs:19-33`)

```
EmitSpec::Driver { match_id }
EmitSpec::Analysis { analysis_name, path }
EmitSpec::RawArtifact
```

Default: `EmitSpec::Driver { match_id: None }` at
`cargo-regseq/src/cli.rs:180-182`.

### Target during rebuild

```
EmitSpec::Model { path: Option<PathBuf> }
```

Default: `Model { path: None }`.

CLI accepts: `--emit model`, `--emit model=path/to/output.json`.

`FromStr`/`Display` updated accordingly. `regseq-config` schema updated.

### Target after re-introduction

```
EmitSpec::Driver { match_id }
EmitSpec::Analysis { analysis_name, path }
EmitSpec::Model { path }
```

`RawArtifact` does not return. `Model` is retained as a diagnostic
alternative alongside the production `Driver` output.

### Driver and Analysis during the rebuild period (DL-3, settled)

`EmitSpec::Driver` and `EmitSpec::Analysis` stay in the enum. The
dispatch in `regseq-driver` emits a clear "not yet exercised" signal
when either variant is selected — either a typed error
(`EmitSpec::<variant>` is temporarily disabled pending re-port, see
`docs/discussions/models/regseq-model/legacy-removal-plan.md`) or an
explicit panic with the same message. When a layer is re-ported, the
stub is replaced by the real implementation — no enum churn, no CLI
surface changes.

Rationale: re-introduction is planned (not hypothetical), the
variants' presence documents the intended shape, and the stub text
tells users exactly why the path is inactive.

## Spec updates required

`spec.md` needs updates consistent with "spec is authoritative; update
spec first, then code":

- **Section 1 "Role in the pipeline" (lines 93-109)** — replace the
  three-stage diagram ("Extraction Surfaces -> normalization -> Regseq
  Model -> platform join -> Regseq Model") with the two-stage diagram
  ("rustc HIR/MIR -> construction -> RegseqModel (Phase 1) -> platform
  join -> RegseqModel (Phase 2)"). Remove the "13 Extraction Surfaces"
  framing; inline the concept into the construction pass.
- **Section 12 "Normalization Contract"**:
  - Phase 1 input is now "rustc HIR/MIR via the rustc-driver
    integration," not "the 13 Extraction Surfaces serialized as
    `RawFrontendArtifact` JSON."
  - Phase 1 post-conditions stay identical.
  - Phase 2 is unchanged.
- **Section 8 "Type construction from MIR"** — already describes the
  `rustc::ty::Ty` -> `RegseqType` mapping. The table stays; the
  surrounding framing may need minor edits so it reads as part of the
  single construction pass rather than a mapping applied after
  `RawArtifact` deserialization.

Do these edits **before** Wave C.

## Cross-dependencies to break

Codex identified the following consumers of remove-list code that must
be resolved during or before the relevant wave. File:line references
are current as of this document.

| Consumer | Remove-list dependency | Wave |
|---|---|---|
| `regseq-driver/src/emit_driver.rs:21-30` | `regseq_backends`, `regseq_model::{binding, joined, compat}`, `regseq_analysis::legacy_input` | A |
| `regseq-driver/src/callbacks.rs:180-228` | calls `emit_driver()` | A |
| `cargo-regseq/src/cli.rs:180-182` | defaults to `EmitSpec::Driver` | A |
| `regseq-config/src/emit.rs:19-33,50-59` | models driver emission as first-class | A |
| `regseq-analysis/src/readiness.rs:20-24` | imports `regseq_model::compat::CallbackFamily` | B |
| `regseq-model/src/item/function.rs:15` | depends on `semantic::*` | B (atomic) |
| `regseq-model/src/compat.rs:61-132` | re-exports from `declaration`, `binding`, `joined`, `semantic`, `item` | B (atomic) |
| `regseq-driver/src/mir_repr.rs:1-16` | re-exports `regseq_model::declaration::mir_body::*` | C / D |
| `regseq-model/src/normalize/surface.rs:992-1030` | depends on `CanonicalPath`-based identity enrichment for user-data validation | C |

## Risks

1. **Hidden dependencies during deletion.** Mitigation: delete one
   module at a time within each wave; cargo-check after each.
2. **Over-deleting `CanonicalPath`-based extraction input before
   replacing user-data-field resolution in `normalize/surface.rs:
   992-1030` regresses spec Section 4.4.** Mitigation: before Wave C,
   land a direct model-native user-data validation path.
3. **Losing reference patterns from legacy code.** Mitigation: git tag
   `pre-legacy-removal` before Wave A; all legacy code remains
   retrievable.
4. **Construction is not yet spec-complete.** The dormant code in
   `regseq-driver/src/construction/` is a seed, not a finished
   builder. After removal, the "what must be built" list is real work,
   not a trivial connection.
5. **Spec drift.** If spec edits lag code edits, the authoritative
   source diverges from the implementation. Mitigation: update
   `spec.md` before each code wave that affects the described
   architecture.

## Order of operations

1. **Tag** `pre-legacy-removal` at current HEAD.
2. **Spec update** for Section 1 diagram and Section 12 Phase 1 input.
3. **regseq-backends audit** (DL-4). Inventory the crate's content and
   classify each module as `ADAPT`, `DISCARD`, or `MIXED`. The audit
   output drives the Wave A deletion scope. **Completed**; results in
   `docs/discussions/models/regseq-model/regseq-backends-audit.md`.
4. **Wave A**: remove `emit_driver.rs`, its callers, `legacy_input/`,
   and the regseq-backends modules marked "discard" by step 3. Keep
   the `regseq-backends` crate itself (possibly reduced to an empty
   placeholder or to its adaptable modules marked as "pending
   re-port"). Add `EmitSpec::Driver`/`EmitSpec::Analysis` stubs in
   `regseq-driver` that emit the "not yet exercised" signal (DL-3).
5. **Wave B (atomic)**: remove `binding/`, `joined/`, `compat.rs`,
   `item/`, `semantic/`, `declaration/bodies.rs`, legacy
   `declaration/mod.rs` bits. Rewrite `readiness.rs` `compat` import.
   **Completed**; further MIXED→DISCARD upgrades in
   `regseq-backends-audit.md` ("Wave B" section).
6. **Add `EmitSpec::Model`** and wire it through the CLI. Default
   switch flips to `Model`. Library side (`serialize.rs:70`) already
   implements the emission. **Completed**; the JSON is written under
   `<output_dir>/drivers/<driver_name>.model.json` by default, or at
   the explicit `model=<path>` override. The driver-side raw-artifact
   to model-side roundtrip goes through JSON because the two
   `RawFrontendArtifact` mirrors are wire-compatible but distinct
   types; Wave C collapses both mirrors into direct
   rustc-to-RegseqModel construction.
7. **Wave C**: remove `raw_artifact.rs`,
   `normalize/raw_artifact.rs`, most of `normalize/surface.rs`;
   migrate surface logic into `construction/`. Replace
   `source_to_extraction_*` tests with `source_to_regseq_model_*`.
   Detailed playbook in
   `docs/discussions/models/regseq-model/wave-c-execution-plan.md`
   (settles DL-9, DL-10; aggressive-cut strategy; commits C1-C5).
8. **Wave D**: relocate `declaration/mir_body.rs` to
   `regseq-driver/`. Update `mir_repr.rs` re-export chain.
9. **Follow-up efforts** (separate task specs):
   - Callable-item construction.
   - Body construction (`MirBody` -> `ir::Body`).
   - Full MIR -> RegseqType mapping.
   - Silent-fallback elimination.
   - Fully-typed extraction surfaces.

## Re-introduction plan

### Analyses

When `regseq-analysis` is ready to re-port:

- Implement analyses on top of `RegseqModel` directly. No
  `AnalysisInput` shim.
- Re-add `EmitSpec::Analysis` (Option A) or flip the stub (Option B).
- Add `source_to_analysis_*` tests.

### Backends

When a backend is ready to re-port (Linux is the first target):

- Implement `RegseqModel` -> C emitter in a fresh crate or rebuilt
  `regseq-backends`.
- The emitter consumes `RegseqModel` directly; no legacy
  `SemanticModel` layer.
- Re-add `EmitSpec::Driver` (Option A) or flip the stub (Option B).
- Golden C files, authored by hand during the rebuild period,
  become the regression-test fixtures.

## Decision log

### Settled

- **DL-1** Legacy model layers: **delete**, not deactivate. Git tag is
  the safety net.
- **DL-2** Iteration mode for RegseqModel design: **spec-first**. When
  a gap is discovered, update `spec.md` before changing code.
- **DL-3** `EmitSpec::Driver` and `EmitSpec::Analysis` during the
  rebuild period: **keep the variants, stub the dispatch** in
  `regseq-driver` with a clear "not yet exercised" signal. When a
  layer is re-ported, the stub is replaced by the real implementation.
- **DL-5** `declaration/mir_body.rs` relocation: **move into
  `regseq-driver/`**. Design principle: all Rust-surface-to-RegseqModel
  lowering lives in `regseq-driver`; `regseq-model` contains only the
  model definitions (types, IR structures) and model-level operations
  (verify, join, serialize, query).
- **DL-6** Name of the new emit: **`model`**. Matches spec's consistent
  "RegseqModel" terminology.
- **DL-7** `RawArtifact` fate: **remove entirely**. Not retained as a
  separate emit.
- **DL-8** Default emit during rebuild: **`model`**.
- **DL-9** Register-block keying in Wave C: **`DefId ->
  RegisterBlockId`** everywhere. The name-based scheme from normalize
  disappears. See `wave-c-execution-plan.md`.
- **DL-10** User-data validation in Wave C: **ported into construction
  using rustc `TyCtxt` queries** for `Copy` resolution. The
  `identity_enrichment` surface's validation role drops. See
  `wave-c-execution-plan.md`.

### Open

_(no open decisions)_

### Closed by audit

- **DL-4** `regseq-backends` crate fate: **preserved** through Wave A
  with a reduced surface. The full per-file classification is in
  `regseq-backends-audit.md`. Outcome: 15 files DISCARD (deleted in
  Wave A), 12 ADAPT (kept as-is), 13 MIXED (kept as re-port
  reference), 3 follows-exports (updated to reflect survivors). The
  `c/` AST and Linux kernel domain knowledge (platform_driver
  templates, probe/remove, uart_ops wiring) are worth more than the
  maintenance cost of keeping the crate alive through the rebuild
  period.
