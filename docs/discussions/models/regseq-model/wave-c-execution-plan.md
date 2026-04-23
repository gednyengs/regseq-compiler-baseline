# Wave C Execution Plan (RawArtifact Removal)

## Status

Playbook for Wave C of the legacy-removal refactor
(`legacy-removal-plan.md`). Captures two architectural sub-decisions
and the commit sequence that implements the aggressive-cut strategy.

## Context

Waves A and B have completed. Current state:

- Pipeline is single-path: `extract -> RawArtifact -> normalize -> RegseqModel`.
- `regseq-driver/src/construction/` is **dormant** — compiled but never
  invoked from `callbacks.rs`.
- `regseq-model/src/normalize/surface.rs` handles all 8 resource
  vectors (clocks, resets, pinctrl, interrupts, sync, properties,
  user_data, pm_info); `construction/` handles zero of them.
- Strategy: Option B aggressive cut. Single migration wave; build may
  briefly fail during the transition.

Detailed reconnaissance findings are in the session history preceding
this document.

## Architectural sub-decisions

### DL-9: Register-block keying (settled)

**Decision**: adopt `DefId -> RegisterBlockId` throughout. The
string-name keying used by normalize disappears entirely.

**Where it shows up today**:
- `normalize/surface.rs:65` holds a `register_block_by_name` map.
- `construction/register_metadata.rs` builds the `DefId ->
  RegisterBlockId` map as part of Pass 1.
- The MMIO extraction surface already carries `DefId` information at
  HIR-attribute time.

**Migration**: MMIO field population in construction looks up
`RegisterBlockId` via `DefId`. No name-based path survives Wave C.

**Rationale**: DefId is authoritative, survives monomorphization, and
matches how the rest of construction already keys items. The
name-based scheme was an artifact of the rustc-free model crate that
no longer needs to be rustc-free inside the construction pass.

### DL-10: User-data validation re-anchoring (settled)

**Decision**: port Rule 1 (zero-init whitelist) and Rule 2 (`Option<T:
Copy>`) validation into construction, using rustc's `TyCtxt` to query
the `Copy` trait directly. Drop the `identity_enrichment` extraction
surface's role in user-data validation.

**Where it shows up today**:
- `normalize/surface.rs:1035-1150` implements Rule 1 and Rule 2 via
  string/path lookups against the `identity_enrichment` surface.
- Wave B deleted `CanonicalPath` proper; the enrichment surface was
  carrying path strings that the model-crate validator consumed.

**Migration**: user-data validation moves into
`construction/driver.rs` (or a new `construction/user_data.rs`). It
uses `TyCtxt::non_blanket_impls_of` or `check_is_copy`-style queries
to decide Rule 2 admission, and a construction-local whitelist of
scalar / array shapes for Rule 1.

**Rationale**: rustc answers the `Copy` question definitively.
Construction already has `TyCtxt`. The string/enrichment workaround
was a consequence of putting validation in a rustc-free crate; once
validation moves to construction, rustc queries are the cleaner
substrate.

## Commit sequence

### C1 — This document (settles DL-9, DL-10)

### C2 — Resource population in `construction/`

Add eight population routines that consume extraction surfaces
directly and populate the target `DriverStruct` vectors. New module
`construction/resources.rs` (or split if clearer):

- `populate_clocks`
- `populate_resets`
- `populate_pinctrl`
- `populate_interrupts` (including ISR bindings)
- `populate_sync`
- `populate_properties`
- `populate_user_data` (includes Rule 1 / Rule 2 validation per DL-10)
- `populate_pm_info`

MMIO field population (already partial in `construction/driver.rs`)
gains the `DefId -> RegisterBlockId` lookup per DL-9.

Each function mirrors the shape of its `normalize/surface.rs`
counterpart, translated to read from the extraction surface structs
instead of `RawFrontendArtifact` mirror types.

Wired into `construction::build_declaration_skeleton` so Pass 1
produces a fully populated Phase 1 `RegseqModel`.

**Success criterion**: `construction::build_declaration_skeleton` on
the PL011 fixture produces a `RegseqModel` whose resource vectors
match the normalize-produced model byte-for-byte after JSON
serialization (spot check).

### C3 — Rewire `callbacks.rs`

Replace the `extract -> RawArtifact -> normalize -> RegseqModel` chain
at `callbacks.rs:207-268` with a direct call to
`construction::build_declaration_skeleton`. `--emit model` now goes
through construction.

`raw_artifact.rs` and `normalize/` remain in the tree but no
production code calls them. `source_to_extraction_*` tests fail at
this point (they assert on a path that is no longer wired).

### C4 — Delete dead code

- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/normalize/raw_artifact.rs`
- `crates/regseq-model/src/normalize/surface.rs`
- `crates/regseq-model/src/normalize/` directory if nothing else
  remains
- `EmitSpec::RawArtifact` variant in `regseq-config/src/emit.rs`
- `raw-artifact` CLI token parsing in `cargo-regseq`
- `callbacks.rs` remnants referring to `wants_raw_artifact()` / raw
  paths
- `crates/regseq-driver/tests/raw_artifact.rs`

### C5 — Test migration

Replace `source_to_extraction_{01..12}` with
`source_to_regseq_model_{01..12}`. Each new test compiles the same
inline fixture and asserts on the resulting `RegseqModel` shape (JSON
or typed) instead of the `RawFrontendArtifact` shape.

Batched into 1-3 commits depending on which tests share structure.

## Risks and mitigations

1. **C2 is large** (~8 population functions plus wiring). Mitigation:
   each function is independent; implement in a fixed order
   (clocks -> resets -> pinctrl -> interrupts -> sync -> properties ->
   user_data -> pm_info) and cross-check against
   `normalize/surface.rs` as the reference.

2. **User-data validation port may have edge cases** (generic `Copy`
   resolution, associated-type shapes). Mitigation: cross-validate
   against `crates/regseq-model/tests/verify_phase_2.rs` and PL011
   fixture's `user_data` field before deleting `normalize/surface.rs`.

3. **MMIO `DefId` lookup may fail for cross-crate register blocks.**
   Mitigation: construction's existing cross-crate type handling
   (`cross_crate.rs`, `cross_crate_types.rs`) already resolves
   cross-crate `DefId`s; reuse its lookup.

4. **Build may stay broken across C3 until C5 finishes.** Mitigation:
   C3 and C4 land on the same working session; C5 test migration can
   be incremental without blocking merge-to-main.

## Definition of done

- `construction::build_declaration_skeleton` produces a fully
  populated Phase 1 `RegseqModel` for the PL011 fixture.
- `--emit model` end-to-end through construction; output JSON matches
  the normalize-produced JSON on the PL011 fixture (diff-checked once
  before C4 deletes normalize).
- `raw_artifact.rs` (both sides), `normalize/surface.rs`, and
  `EmitSpec::RawArtifact` deleted.
- All surviving tests pass, with at least one
  `source_to_regseq_model_*` test covering the PL011 fixture end to
  end.
