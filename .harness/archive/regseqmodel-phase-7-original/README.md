# Archived: Original Phase-7 Task Specs

These six task specs were authored against an earlier Phase-7 framing
("migrate consumers via a `compat.rs` adapter that projects
`RegseqModel` back to the legacy `DeclarationModel` /
`JoinedArtifact` / `SemanticModel` shape") that was found to be the
wrong abstraction during `regseq-driver-35` implementation.

The correct design is captured in
`docs/discussions/models/regseq-model/migration.md`, which supersedes
both these specs and the original brief at
`.harness/spec-batches/regseqmodel-phase-7.md`.

**Archived files** (preserved for historical reference only; do not
reintroduce):

- `tasks/61-compat-scaffold.md` (was `regseq-model-61`)
- `tasks/06-migrate-to-regseq-model-compat.md` (was `regseq-analysis-06`)
- `tasks/10-migrate-planner-imports-to-compat.md` (was `regseq-lowering-10`)
- `tasks/03-migrate-planner-imports-to-compat.md` (was `regseq-linux-backend-03`)
- `tasks/35-emit-driver-pipeline-cutover.md` (was `regseq-driver-35`)
- `tasks/11-pl011-e2e-regseq-model-rewrites.md` (was `track-a-validation-11`)

**Why retired**:

The `compat.rs` adapter required projecting `RegseqModel` back to the
old model shapes, but `RegseqModel` does not carry several of the
families the legacy `AnalysisInput` surface exposed (callback impls,
wrapper / poll / log / timed-delay / duration-ctor / PM-gate call
sites, guard scopes, lock / completion operations, property lints,
the full `SemanticModel` + `ItemStore`). The old shapes were
declared insufficient for generating correct drivers in the first
place; projecting back to them works against the refactor's goal.

See the final fail_reason recorded on the `regseq-driver-35` task
entry in `git log --follow -- .harness/tasks.json` prior to the
retirement commit for the full implementer analysis that triggered
the redesign.
