# Stage 1 Status

Status: **CLOSED**. Spec implementation-ready after seven review
passes; all seven implementation steps complete and committed.

Last updated: 2026-04-28.

For top-level project status, see `../status.md`. For project-wide
invariants, see `../README.md`. For the stage-1 spec itself, see
`spec-snapshot.md`.

## Spec status

Implementation-ready after **seven review passes** (Codex round 1
against research; Codex rounds 2/4 against the spec; external
round 3; Codex round 5 + external round 5e + Codex round 6 across
the round-5 monomorphization-and-C-semantics revision). All 39
issues addressed.

- Spec: `spec-snapshot.md` (round-5-revised; covers
  Instance/InstanceKind, the DefPath family, the Rust -> C
  lowering reference, and `impl` lifting).
- Review trail: `review-log.md` (R1-R6 + R5.7-R5.21).
- Research synthesis and locked sub-decisions (F1-F4):
  `research.md`.
- Rustc-internal evidence: `rustc-excerpts.md` (covers `Idx`,
  `Symbol`, `Span`/`SourceMap`, `DefId`, `DefPath`/`DefPathData`,
  `DefPathHash`, `StableCrateId`, `Instance`/`InstanceKind`,
  `tcx.symbol_name`, `VtblEntry`).
- MIR/HIR empirical observations: `findings.md`.
- Inbound forward refs (now mostly resolved or routed forward):
  `expectations.md`.

## Implementation status

All seven steps complete and committed in `crates/regseq-model/`:

- **Step 1** (`b8f1e67`): `Idx` trait + `newtype_index!` macro +
  niche encoding via `NonMaxU32` + `CrateId`.
- **Step 2** (`fd0d3c8`): `IndexVec<I, T>` with serde-transparent
  flat array.
- **Step 3** (`82aaaed`): `StringInterner` + `StringId`;
  pre-interns empty string at `StringId::ZERO`.
- **Step 4** (`f34df65`, plus `f8e7b4e` follow-up):
  `SourceFile` + `Position` + `Span` + `SourceFileId` (with
  `SYNTHESIZED` alias). Resolution methods.
- **Round-5 spec revision** (`8215cdc`, after step 4) redesigned
  the monomorphization story (Instance shape, InstanceKind,
  lifting) and the C-semantics framing.
- **Step 5** (`d78db53`): forward-declared `ItemId` / `BodyId` /
  `TyId` / `ConstId` / `FnSigId` / `InstanceId`; `StableCrateId`
  + `CrateInfo`; `DefPathData` (19 variants) + `DefPathSegment`
  + `DefPath` + `DefPathHash`; `def_path_data_sort_rank`
  (0..=18); `GenericArg` + `GenericArgs`; `Instance` +
  `InstanceKind` (six variants); `regseq_instance_kind_rank`
  (0..=5); `RustcInstanceKindTag` (Regseq-side mirror of rustc's
  15-variant `ty::InstanceKind`) + `rustc_instance_kind_sort_rank`
  (0..=14, kept inside regseq-model so all rank values are pinned
  in one place; bridge converts the live rustc enum to a tag at
  the boundary). Codex round 1 returned MINOR (two test
  additions); both addressed.
- **Step 6** (`6c16cc9`): top-level `RegseqModel` struct +
  `RegseqModel::with_primary` constructor +
  `validate(&self) -> Result<(), InvariantViolation>` enforcing
  the stage-1-checkable subset of the 10 spec invariants
  (1, 2, 4, 5, 8, 10 enforced; 3, 6, 9 deferred to later stages;
  7 is structural). Empty placeholder structs `Item` / `Ty` /
  `Const` / `FnSig` / `Body` ship with the crate as forward-decls
  for stages 3, 4, 11, 12. Codex round 1 returned APPROVE (one
  doc nit + optional dangling-ID coverage); both addressed.
- **Step 7** (`5025759`): canonical `render_def_path(path, crates,
  strings) -> String` helper in `def_path.rs` -- pinned per-
  variant rendering against `nightly-2026-02-01`
  `rustc_hir/src/definitions.rs:185-204, :248-249, :505-520`
  (anonymous-namespace strings: `impl`, `extern`, `use`,
  `global_asm`, `closure`, `constructor`, `constant`, `constant`,
  `opaque`, `synthetic`, `nested`; AnonAssocTy hybrid
  `name::{anon_assoc#N}`; DesugaredAnonymousLifetime literal
  `'_` with named-segment disambig). New `pretty.rs` ships the
  `PrettyPrinter<'a>` scaffold per spec lines 1323-1348, with
  stage-1 functional methods (`render_span`,
  `render_def_path_value`) and stage-N stubs naming the stage
  that fills them. `RegseqModel::pretty()` is the convenience
  accessor. Plan went through three Codex rounds before
  implementation (BLOCK -> BLOCK -> APPROVE) + implementation
  review (MINOR; one missing negative test added).

229 integration tests passing across ten test files
(`idx_round_trip`, `index_vec`, `source`, `string_interner`,
`crate_info`, `def_path`, `instance`, `model`, `render_def_path`,
`pretty`); `cargo check --workspace --tests` clean.

## Workspace state at stage-1 close

- *Active crates (un-suffixed)*: `regseq-model`, `regseq-analysis`,
  `regseq-backends` are stubs (single `lib.rs` each) waiting for
  stage-by-stage population. `regseq-driver` carries only the
  Q6 multi-crate dispatch layer (`callbacks.rs`, `args.rs`,
  `lib.rs`, `main.rs`, plus four kept tests).
- *Reference crates (`*-old`)*: `regseq-model-old`,
  `regseq-analysis-old`, `regseq-backends-old`,
  `regseq-driver-old`, `tests/e2e-old` hold the prior IR design.
  Self-contained and fully buildable
  (`cargo build -p regseq-model-old` etc.); not on the active
  build path.
- *Multi-crate dispatch (Q6) intact.* `crates/regseq/` facade,
  three-tier dispatch, env-var plumbing, cleanup-pass whitelist
  unchanged. `RegseqCallbacks::after_analysis` still empty until
  bridge work begins.

## Cross-stage forwarded items

Items raised during stage-1 work that other stages address:

- **`newtype_index!` macro const-body arm** -- forwarded to
  `../stage-10/expectations.md` (BlockId::START_BLOCK) and
  `../stage-11/expectations.md` (LocalId::RETURN_PLACE).
- **Construction invariants 3, 6, 9 + `spec_crate_stable_id`
  half of invariant 4** -- deferred in `RegseqModel::validate`
  with in-source comments naming the closing stage. Tracked at
  `../status.md` "Deferred items."
- **`StringInterner` deserialize-path optimization** --
  optional/non-blocking; tracked at `../status.md` "Deferred
  items."
- **Stage-3 forward refs** -- canonical `Ty` representation rule
  (F4-2 `ArgsKey`), layout-pointing variants, `pretty.rs::render_ty`
  stub fill, pointer/reference provenance, niche assertion for
  `Option<TyId>`, bridge `Ty` interning. See
  `../stage-3/expectations.md`.
- **Stage-12 forward refs.** See `../stage-12/expectations.md`.
- **Stage-13 forward refs** (intrinsics, classification). See
  `../stage-14/expectations.md`.
