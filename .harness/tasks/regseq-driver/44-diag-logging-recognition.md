# regseq-driver-44: Diagnostic-Logging Intrinsic Recognition

## Description

Recognise diagnostic-logging macro emissions during Phase 2 of the
bridge per `spec.md` §3.6.4 and populate
`RegseqModel.domain_attrs.item_attrs[item]` with `IntrinsicKind::Log(LogLevel)`
AttrData entries per the §26.1-§26.5 author-surface and post-expansion
shape contract. The recognition codepath fires when a Phase-2 Call
translation visits a callee whose DefPath matches a `regseq::log::<sev>!`
macro emission (where `<sev>` is `debug` / `info` / `warn` / `error`).

This is the IR-shape recognition layer only. The DiagLogPeel body-level
pass + format-template grammar parsing + bridge-side validation
diagnostics live in regseq-driver-45. The §26.9 backend lowering
contract is phase-1b.

## Input Context

- `docs/discussions/regseq-model/spec.md` §3.6 (DomainAttr sibling
  tables) -- especially §3.6.4 Phase-2 recognition codepath, §3.6.5
  validation invariants, §3.6.7 per-family target-table mapping
- `docs/discussions/regseq-model/spec.md` §26.1-§26.5 (Diagnostic
  logging author surface + IR-shape contract + post-expansion shape)
- `crates/regseq-model/src/domain_attrs.rs` (DomainAttrTables +
  AttrData + IntrinsicKind + LogLevel + IntrinsicKind::from_name from
  regseq-model-74)
- `crates/regseq-driver/src/bridge/body/terminator.rs` (Call
  Terminator translation from regseq-driver-42)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt from
  regseq-driver-36)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/diag_log/` subtree. Wire
`pub mod diag_log;` from `crates/regseq-driver/src/bridge/mod.rs`. The
recognition codepath is invoked from regseq-driver-42's Call Terminator
translator via a hook.

### Module structure

- `crates/regseq-driver/src/bridge/diag_log/mod.rs` -- public entry
  point + the recognition codepath.
- `crates/regseq-driver/src/bridge/diag_log/recognition.rs` -- the
  per-callee match against `regseq::log::<sev>!` macro emission shapes.

### Public API

```rust
// bridge/diag_log/mod.rs

/// Phase-2 recognition hook invoked from the Call Terminator translator.
/// If the callee matches a `regseq::log::<sev>!` macro emission per
/// spec.md §3.6.4 + §26.1-§26.5, records an
/// `AttrData::Intrinsic(IntrinsicKind::Log(level))` entry into
/// `RegseqModel.domain_attrs.item_attrs[item_handle]`.
pub fn recognise_log_intrinsic_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    callee_handle: ItemHandle,
    callee_def_path: &regseq_model::DefPath,
) -> Result<(), BridgeError>;
```

### Recognition codepath (spec.md §3.6.4)

Per spec.md §3.6.4 the recognition codepath fires during Phase-2
translation when a Call's callee resolves to a DefPath matching the
`regseq::log::<sev>!` macro emission shape. The matching uses the
closed-vocabulary lookup `IntrinsicKind::from_name` provided by
regseq-model-74:

- DefPath ends in `log_debug` -> `LogLevel::Debug`
- DefPath ends in `log_info` -> `LogLevel::Info`
- DefPath ends in `log_warn` -> `LogLevel::Warn`
- DefPath ends in `log_error` -> `LogLevel::Error`

Per spec.md §26.1-§26.5 the author surface is the `regseq::log::error!`
macro family; post-expansion the bridge sees a Call whose callee has
the dash-form NAME mapping per spec.md:1542-1551 (`log-debug` /
`log-info` / `log-warn` / `log-error`).

### Per-callee `domain_attrs.item_attrs` population (spec.md §3.6.4)

Per spec.md §3.6.4 the recognition codepath records:

```text
domain_attrs.item_attrs[callee_item].push(
    AttrData::Intrinsic(IntrinsicKind::Log(level))
)
```

Per spec.md §3.6.6 the attribute namespace is `intrinsic`; per
spec.md §3.6.7 the per-family target table is `item_attrs`.

### Wiring into Call Terminator translator

The hook is invoked from `bridge::body::terminator::translate_terminator`
in the `Call` arm: after the callee handle is resolved, the code calls
`bridge::diag_log::recognise_log_intrinsic_call(cx, callee, callee_def_path)`.
On a recognition hit, the entry is appended to `domain_attrs.item_attrs`;
on a miss, no side effect occurs.

### IR-shape contract (spec.md §26.1-§26.5)

Per spec.md §26.1-§26.5 the Phase-2 view of a recognised log call
shape is preserved verbatim (the slice-aggregate `LogArg` carrier is
**not yet peeled** at recognition time -- DiagLogPeel runs in
regseq-driver-45). The recognition layer only annotates the callee
Item with the `IntrinsicKind::Log(level)` attribute; body-content
remains the raw post-expansion shape until DiagLogPeel.

## Non-Goals

- DiagLogPeel body-level pass (regseq-driver-45).
- Format-template grammar parsing (regseq-driver-45).
- Bridge-side validation diagnostics (regseq-driver-45).
- §26.9 backend C-emission contract (phase-1b).
- IntrinsicKind closed-vocabulary lookup table itself (owned by
  regseq-model-74).
- Body materialisation (regseq-driver-43).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/diag_log/recognition.rs` `#[cfg(test)] mod tests`)

- For each LogLevel variant (Debug / Info / Warn / Error), construct a
  stub callee DefPath whose final segment matches the dash-form NAME
  per spec.md:1542-1551; assert recognition records the matching
  `IntrinsicKind::Log(level)` entry in `domain_attrs.item_attrs`.
- Negative: construct a callee DefPath whose final segment is unrelated
  (e.g. `regular_function`); assert no entry is recorded.
- Idempotency: invoke recognition twice for the same callee handle;
  assert exactly one entry is recorded (or two if the spec requires
  per-call-site annotation -- the test asserts the spec-pinned
  semantics per spec.md §3.6.4).

### Integration tests (`crates/regseq-driver/tests/bridge_diag_log_recognition.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per LogLevel):

- Debug fixture -- a function calling `regseq::log::debug!("...")` --
  assert the post-bridge RegseqModel has
  `domain_attrs.item_attrs[callee_item]` containing
  `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug))`.
- Info fixture -- analogous with `regseq::log::info!`.
- Warn fixture -- analogous with `regseq::log::warn!`.
- Error fixture -- analogous with `regseq::log::error!`.

Negative coverage:

- Non-log call fixture -- a function calling a non-log function --
  assert `domain_attrs.item_attrs[callee_item]` is empty (or contains
  no `IntrinsicKind::Log(_)` entry).
- Multiple-severity fixture -- a function calling all four log macros
  -- assert each callee Item gets exactly the right level annotation.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-44 against its task spec at
.harness/tasks/regseq-driver/44-diag-logging-recognition.md. ALL CHECKS
MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/diag_log/mod.rs
- crates/regseq-driver/src/bridge/diag_log/recognition.rs
- crates/regseq-driver/src/bridge/body/terminator.rs (the recognition
  hook invocation site)
- crates/regseq-driver/tests/bridge_diag_log_recognition.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §3.6.4 / §3.6.5 / §3.6.6 / §3.6.7 /
  §26.1-§26.5.
- Recognition fires during Phase 2 (not deferred).
- Per-LogLevel coverage in tests.
- domain_attrs.item_attrs[callee] population shape matches AttrData
  variant per regseq-model-74.
- Recognition codepath does NOT peel LogArg (peeling is regseq-driver-45).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). DiagLogPeel + format parsing + validation diagnostics
are regseq-driver-45.
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/diag_log/{mod,recognition}.rs`
  created and wired from `bridge/mod.rs`.
- Recognition hook invoked from regseq-driver-42's Call terminator
  translator.
- Per-LogLevel coverage in unit + integration tests.
- `domain_attrs.item_attrs[callee]` population matches spec.md §3.6.4
  shape.
- All tests pass; Codex Task Review verdict APPROVED.
