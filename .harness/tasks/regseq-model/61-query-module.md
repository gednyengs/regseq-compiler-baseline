# regseq-model-61: `regseq-model::query` -- CFG Traversal Helpers

**Depends on**: `regseq-model-60`.
**Parallel with**: `regseq-model-62`, `regseq-backends-02`,
`regseq-backends-03`, `track-a-validation-11`.

## Description

Phase-7A foundation. Creates `crates/regseq-model/src/query/`: a
deterministic, read-only traversal module that answers questions
about a `&RegseqModel` without requiring a pre-computed auxiliary
structure.

This is the replacement for the pre-computed "families" that the
legacy `AnalysisInput` exposed (wrapper call sites, poll call sites,
log call sites, timed-delay call sites, duration-ctor call sites,
PM-gate call sites, helper-reachable function names, etc.). All of
these are derivable from the module tree + CFG bodies + intrinsic
metadata; none of them need to live on the model as storage. The
legacy computation happened at extraction time; here we compute on
demand, leveraging the fact that `RegseqModel`'s CFG IR carries
`IntrinsicCall` metadata directly (`ir-reference.md:1032-1065`).

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q4 (for each
  SURVIVING consumer, the list of families it reads that previously
  came from `AnalysisInput`; this task's query functions cover
  those), Q6 step 1 ("Build the `regseq-model::query` helper
  module"), Q3 / Q6 rationale for replacing pre-computed families
  with on-demand computation.
- `docs/discussions/models/regseq-model/ir-reference.md` (the CFG
  + `IntrinsicCall` metadata structure being traversed).
- `crates/regseq-analysis/src/input.rs` lines 55-564 -- the
  accessor surface being replaced. Every accessor whose semantics
  can be computed from RegseqModel gets a corresponding `query`
  function.
- `crates/regseq-model/src/model/{module, function, driver}.rs`
  -- module-tree structure.
- `crates/regseq-model/src/ir/{body, statement, terminator,
  intrinsic}.rs` -- CFG structure traversed.
- `.harness/tasks/regseq-model/58-normalize.md` + `59-populate-bindings.md`
  + `60-pl011-pipeline-end-to-end.md` -- Phase-6 pipeline context
  (RegseqModel is guaranteed to be Phase-1 or Phase-2 verified
  when a consumer calls a query).

## Implementation Notes

### Module layout

```
crates/regseq-model/src/query/
    mod.rs          -- public re-exports
    call_sites.rs   -- helpers finding call sites by intrinsic kind
    reachability.rs -- helpers computing helper-call transitive closures
    topology.rs     -- helpers answering "what families does this driver declare?"
```

Add `pub mod query;` to `crates/regseq-model/src/lib.rs`.

### Public API

Organized by the question each answers:

**`call_sites.rs`** -- for each intrinsic family, find every
`IntrinsicCall` terminator + containing function:

```rust
/// A located intrinsic call site: the terminator + the function it
/// appears in. Deterministic iteration order (sorted by
/// (FunctionId, BasicBlockId)).
#[derive(Debug, Clone)]
pub struct CallSite<'a> {
    pub function_id: FunctionId,
    pub block_id: BasicBlockId,
    pub metadata: Option<&'a IntrinsicMetadata>,
    pub intrinsic: RegseqIntrinsic,
}

pub fn wrapper_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn poll_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn log_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn timed_delay_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn completion_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn lock_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
pub fn register_access_sites(model: &RegseqModel) -> Vec<CallSite<'_>>;
// One helper per family currently exposed on AnalysisInput;
// enumerate by reading input.rs lines 55-564.
```

Implementation pattern: walk `model.root_module` recursively; for
each `Function`, walk `body.basic_blocks`; for each `Terminator`,
match on `TerminatorKind::IntrinsicCall { intrinsic, metadata, .. }`
and filter by family. Family membership is a `match` on
`RegseqIntrinsic` variants.

**`reachability.rs`** -- transitive call closures:

```rust
/// Returns the set of `FunctionId`s transitively called from any of
/// the given callback functions. Deterministic iteration order
/// (sorted by FunctionId).
pub fn transitively_reachable_from(
    model: &RegseqModel,
    seeds: &BTreeSet<FunctionId>,
) -> BTreeSet<FunctionId>;

/// Returns the set of helper functions reachable from at least one
/// callback. Equivalent to transitively_reachable_from the set of
/// all callback FunctionIds, minus the callback FunctionIds
/// themselves.
pub fn reachable_helper_names(model: &RegseqModel) -> BTreeSet<Symbol>;
```

Implementation: BFS over the call graph, where edges are
`TerminatorKind::Call { callee, .. }` and
`TerminatorKind::IntrinsicCall` is NOT an edge (intrinsics are
not user-defined functions).

**`topology.rs`** -- driver-module-level summaries that are
currently derived at extraction:

```rust
/// Names + kinds of every resource field on the driver struct.
/// Equivalent to the legacy DeclarationModel::all_resource_field_names.
pub fn resource_field_names(model: &RegseqModel) -> Vec<(ResourceKind, &str)>;

/// Callback families actually implemented by the driver. Computed
/// by enumerating `driver_impl_blocks` and matching trait paths
/// against the callback-family taxonomy.
pub fn callback_families_present(model: &RegseqModel) -> BTreeSet<CallbackFamily>;

/// Every CallbackImpl in the driver module. Equivalent to legacy
/// DeclarationModel::all_callback_impls.
pub fn callback_impls(model: &RegseqModel) -> Vec<CallbackImpl<'_>>;

// `CallbackImpl<'_>` is a lightweight projection struct:
pub struct CallbackImpl<'a> {
    pub family: CallbackFamily,
    pub function_id: FunctionId,
    pub driver_type: TypeDefId,
    // No owned strings; everything borrowed from model.
}

/// Bus-binding identity for the driver. Reads
/// `DriverStruct.platform_metadata.bus` (crates/regseq-model/src/model/driver.rs:44-54,101-133).
/// Returns `None` if the driver declares no bus (Phase-1 only) or
/// the `platform_metadata` slot is empty after Phase-2 join.
///
/// Required by task `track-a-validation-13`'s rewritten
/// `pl011_bus_identity_proof.rs`.
pub fn bus_binding(model: &RegseqModel) -> Option<&BusBinding>;
```

**`ResourceKind` canonical home**: the current `ResourceKind`
lives at `crates/regseq-model/src/semantic/types.rs:102-120`, in
a module scheduled for deletion by `regseq-model-63`. As part of
this task, move `ResourceKind` to a non-deleted home:
`crates/regseq-model/src/model/resources.rs` (or a new
`model/resource_kind.rs`). Re-export from `regseq_model::query`
and update the one other in-tree consumer
(`crates/regseq-backends/src/lowering/plan.rs:316` --
`pub use regseq_model::semantic::ResourceKind;`) to the new
path. This prevents a silent breakage when task 63 deletes
`semantic/`.

### Determinism rule

Every public function returns output in a deterministic order. When
iterating collections, iterate in `BTreeMap` / `BTreeSet` natural
order. When building `Vec` results, sort by a canonical key at the
end. A test enforces determinism (two calls on the same input return
the same bytes).

### No mutation, no caching

These helpers are pure functions over `&RegseqModel`. No caching
(Phase-6 verifier already guarantees the model is immutable
post-construction; downstream consumers that repeatedly query can
cache at the consumer level). No internal mutex / interior
mutability.

### Completeness audit

Before committing, grep `crates/regseq-analysis/src/{input,readiness,body_readiness}.rs`
for method calls on model types (`.wrapper_call_sites()`,
`.poll_call_sites()`, `.all_callback_impls()`, etc.). Every legacy
accessor must have a `query::*` counterpart, OR be explicitly
deferred to a later task with a comment in the migration doc.

If a legacy accessor CANNOT be computed from the current RegseqModel
(e.g., it read data that lived only on DeclarationModel and was not
preserved), this task STOPS and escalates to the user. Extending
RegseqModel is out of this task's scope.

### Error handling

Every query function accepts a valid (Phase-1 or Phase-2 verified)
RegseqModel; malformed models are a verifier's concern, not the
query's. Query functions do not return `Result`; they return plain
values. If a traversal encounters impossibility (e.g., a
`Function` with no body when the call site expects one), the
function may `panic!` with a diagnostic -- the verifier should have
prevented this state.

## Non-Goals

- No caching / interior mutability.
- No mutation of the model.
- No Phase-6 API changes.
- No consumer migration -- that is task 6 (`regseq-analysis-06`).
- No extension to `RegseqModel` storage.
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Test Plan

Unit tests under `crates/regseq-model/tests/query_api.rs`:

- For each public query function, a test on a minimal hand-built
  model that exercises the function's core path. E.g.:
  - `wrapper_call_sites` on a model containing one function with
    one `IntrinsicCall` terminator of a wrapper-method variant.
  - `reachable_helper_names` on a model with call graph
    `probe -> helper_a -> helper_b`; expect `{helper_a, helper_b}`.
  - `resource_field_names` on a driver with one clock + one mmio
    resource.
- **Determinism test**: for each query function, call it twice on
  the same model; assert byte-equal output.
- **PL011 integration test**: load the Phase-6 PL011 golden
  (`tests/fixtures/pl011-full-spec/expected/regseq-model.json`),
  call every query function, assert non-empty output for each
  family PL011 actually uses. Acts as a smoke test that the
  queries compile + run against real data.
- `cargo test -p regseq-model` passes; workspace green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-model-61 against .harness/tasks/regseq-model/61-query-module.md.

Files:
- crates/regseq-model/src/query/ (new module)
- crates/regseq-model/src/lib.rs (pub mod query;)
- crates/regseq-model/tests/query_api.rs

Focus on:
- Is every legacy AnalysisInput accessor covered by a query
  counterpart? Grep crates/regseq-analysis/src/input.rs for
  public methods returning call-site / family / reachability data,
  and verify each maps to a query::* function.
- Determinism: does every Vec-returning function sort before returning?
- Purity: any interior mutability, caching, global state?
- Error handling: does it panic or silently produce empty results
  on malformed models? Per spec, verifier enforces well-formedness,
  so panic with diagnostic is acceptable -- but silent empty is not.
- Performance: obvious O(n^2) traversals on the call graph?
  PL011 is small; Phase 7 doesn't need aggressive optimization,
  but document the complexity.

Do NOT flag:
- Absence of consumer migration (task 6).
- Cases where a legacy accessor depended on data no longer in
  RegseqModel -- that MUST have been escalated to the user; if
  it wasn't, call it out as MANDATORY.
</task>
... (standard grounding + structured output per SESSION.md) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-model --test query_api
cargo test -p regseq-model
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/query/` module exists with
  `call_sites.rs`, `reachability.rs`, `topology.rs`, and `mod.rs`.
- `pub mod query;` declared in `lib.rs`.
- Every legacy `AnalysisInput` accessor from
  `crates/regseq-analysis/src/input.rs` whose semantics are
  computable from `&RegseqModel` has a corresponding query
  function.
- Determinism test passes.
- PL011 smoke integration test passes.
- `cargo test --workspace` green.
- Codex review APPROVED.
