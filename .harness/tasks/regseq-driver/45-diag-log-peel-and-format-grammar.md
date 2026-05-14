# regseq-driver-45: DiagLogPeel + Format-Template Grammar + Validation

## Description

Implement the DiagLogPeel body-level pass per `spec.md` §25.7.3 and
§26.6, the format-template grammar parser per §26.7 (closed subset:
`{}` plus the explicit modifiers like `{:x}` / `{:#010x}`; Debug
formatters such as `{:?}` are parsed only to reject per D3 at
spec.md:12840), and the bridge-side validation diagnostics per §26.8.

DiagLogPeel transforms the slice-aggregate `LogArg` carrier (the
post-expansion shape preserved by regseq-driver-44's recognition layer)
into a flat-arg Call that satisfies the post-peel `I-DiagLog-CallArity`
shape per spec.md §23.6.2 B-4 / B-6 exceptions. **No C-emission** -- the
§26.9 backend lowering contract is phase-1b territory.

## Input Context

- `docs/discussions/regseq-model/spec.md` §25.7.3 (DiagLogPeel
  body-level pass)
- `docs/discussions/regseq-model/spec.md` §26.6 (DiagLogPeel
  semantics)
- `docs/discussions/regseq-model/spec.md` §26.7 (format-template
  grammar -- CLOSED SUBSET)
- `docs/discussions/regseq-model/spec.md` §26.8 (bridge-side
  validation diagnostics)
- `docs/discussions/regseq-model/spec.md` §23.6.2 (I-DiagLog-CallArity
  invariant + B-4 / B-6 exceptions)
- `docs/discussions/regseq-model/spec.md` line 12840 (D3 -- Debug
  formatter rejection)
- `crates/regseq-driver/src/bridge/diag_log/` (recognition codepath
  from regseq-driver-44)
- `crates/regseq-driver/src/bridge/body/materialise.rs` (Phase-5 body
  passes from regseq-driver-43)
- `crates/regseq-model/src/body.rs` (Body API + push_synthetic_local
  from regseq-model-82)

## Implementation Notes

Extend the `crates/regseq-driver/src/bridge/diag_log/` subtree with:

- `crates/regseq-driver/src/bridge/diag_log/peel.rs` -- the body-level
  DiagLogPeel pass.
- `crates/regseq-driver/src/bridge/diag_log/format.rs` -- the
  format-template grammar parser (closed subset).
- `crates/regseq-driver/src/bridge/diag_log/validation.rs` -- the
  bridge-side validation diagnostics per spec.md §26.8.

Wire each new module from `crates/regseq-driver/src/bridge/diag_log/mod.rs`.
Wire DiagLogPeel into the Phase-5 body-pass set from regseq-driver-43.

### Public API

```rust
// bridge/diag_log/peel.rs

/// Body-level pass that peels the slice-aggregate LogArg carrier into
/// a flat-arg Call per spec.md §25.7.3 + §26.6. Runs during Phase 5
/// after the vtable-indirect rewrite (regseq-driver-43) and before
/// joint validation.
pub fn diag_log_peel_pass(
    cx: &mut BridgeCtxt<'_>,
    body: &mut regseq_model::Body,
) -> Result<(), BridgeError>;
```

```rust
// bridge/diag_log/format.rs

/// Parsed format template per the closed-subset grammar at spec.md §26.7.
pub struct ParsedFormatTemplate {
    pub literal_segments: Vec<String>,
    pub specifiers: Vec<FormatSpecifier>,
}

/// One format specifier per spec.md §26.7 closed subset. Debug
/// formatters (`{:?}`) parse-and-reject per D3 at spec.md:12840.
pub enum FormatSpecifier {
    /// `{}` -- the default Display specifier.
    Display,
    /// `{:x}` -- lowercase hex.
    Hex,
    /// `{:X}` -- uppercase hex.
    HexUpper,
    /// `{:#010x}` -- the explicit hex modifier with width / fill.
    HexFormatted { fill: char, width: u32, alternate: bool },
    /* additional explicit modifier variants per spec.md §26.7 */
}

/// Parse a format template string per spec.md §26.7 closed subset.
/// Returns `BridgeError::DiagLogFormat` if the template contains a
/// Debug formatter (`{:?}`) per D3 at spec.md:12840 or any other
/// non-subset construct.
pub fn parse_format_template(
    template: &str,
) -> Result<ParsedFormatTemplate, BridgeError>;
```

```rust
// bridge/diag_log/validation.rs

/// Bridge-side validation diagnostics per spec.md §26.8. Emits a
/// `BridgeError::DiagLogValidation` for each rule violation.
pub fn validate_log_call(
    cx: &BridgeCtxt<'_>,
    parsed_template: &ParsedFormatTemplate,
    arg_count: usize,
) -> Result<(), BridgeError>;
```

### DiagLogPeel pass (spec.md §25.7.3 / §26.6)

Per spec.md §25.7.3 and §26.6 the DiagLogPeel pass walks every Call
in the materialised Body whose callee Item carries an
`AttrData::Intrinsic(IntrinsicKind::Log(_))` annotation (set by
regseq-driver-44). For each such Call:

1. Locate the slice-aggregate LogArg carrier in the Call's argument
   list (the post-expansion shape).
2. Decompose the carrier into the format-template literal (Operand
   index 0) and the per-specifier argument Operands (indices 1..N).
3. Replace the Call's argument vector with the flat-arg form: format
   template Operand at index 0 followed by the N argument Operands.
4. Allocate any fresh locals required for intermediate Operand reads
   via `Body::push_synthetic_local`.

After peeling, the Call must satisfy the post-peel
`I-DiagLog-CallArity` shape per spec.md §23.6.2 B-4 / B-6 exceptions:
arg count = 1 (template) + N (specifier args) where N matches the
parsed format template's specifier count.

### Format-template grammar parser (spec.md §26.7 -- CLOSED SUBSET)

Per spec.md §26.7 the parser admits exactly the closed subset:

- `{}` -- default Display.
- `{:x}` / `{:X}` -- lowercase / uppercase hex.
- `{:#010x}` (and analogous explicit-modifier forms) -- explicit
  modifier with fill / width / alternate flag.

Per D3 at spec.md:12840, Debug formatters (`{:?}`) **are parsed only
to reject** -- the parser recognises the Debug syntax and returns
`BridgeError::DiagLogFormat { reason: DebugFormatterRejected, .. }`.

Any non-subset construct (e.g. positional args `{0}`, named args
`{name}`, padding flags outside the explicit modifier list) is also
rejected with `BridgeError::DiagLogFormat`.

### Bridge-side validation diagnostics (spec.md §26.8)

Per spec.md §26.8 the validation pass emits diagnostics for:

- Specifier count != argument count.
- Debug formatter present.
- Non-subset format constructs.
- Any other §26.8 rule.

Each diagnostic returns a `BridgeError::DiagLogValidation` variant
with a per-rule reason.

### Phase-5 ordering

Per spec.md §25.7 the DiagLogPeel pass slots into the Phase-5 body-pass
ordering after vtable-indirect rewrite and before joint validation
(joint validation needs the post-peel flat-arg shape). The
regseq-driver-43 `run_phase5_body_passes` is updated to:

1. `materialise_function_body`
2. `rewrite_vtable_indirect`
3. **`diag_log_peel_pass`** (new from this task)
4. `joint_validation_walk`

## Non-Goals

- Diagnostic-logging recognition + per-callee `domain_attrs.item_attrs`
  population (regseq-driver-44).
- §26.9 backend C-emission contract (phase-1b).
- IntrinsicKind closed-vocabulary lookup (owned by regseq-model-74).
- Body materialisation infrastructure (regseq-driver-43).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/diag_log/{peel,format,validation}.rs` `#[cfg(test)] mod tests`)

Format parser:

- Empty template `""` -- assert `ParsedFormatTemplate { literal_segments: [""], specifiers: [] }`.
- Display only `"x = {}"` -- assert one Display specifier.
- Hex `"x = {:x}"` -- assert Hex specifier.
- Uppercase hex `"{:X}"` -- assert HexUpper specifier.
- Formatted hex `"{:#010x}"` -- assert HexFormatted with the parsed
  fill / width / alternate fields.
- Multiple specifiers `"a = {}, b = {:x}"` -- assert two specifiers
  in order.
- Debug formatter `"{:?}"` -- assert
  `BridgeError::DiagLogFormat { reason: DebugFormatterRejected, .. }`
  per D3 at spec.md:12840.
- Positional argument `"{0}"` -- assert rejection.
- Named argument `"{name}"` -- assert rejection.
- Unclosed brace `"{"` -- assert rejection.
- Escaped braces `"{{"` -- assert rendered to literal `{` per the
  spec.md §26.7 grammar (if applicable; if not in the subset, assert
  rejection).

Validation:

- `validate_log_call` with matching specifier / arg counts -- assert
  `Ok(())`.
- Mismatched counts -- assert `BridgeError::DiagLogValidation`.

DiagLogPeel:

- Hand-construct a Body containing a Call with the slice-aggregate
  LogArg carrier; run `diag_log_peel_pass`; assert the Call now has
  arg count = 1 + N (per the parsed template) and the LogArg carrier
  is gone.
- Hand-construct a Body containing a Call without the LogArg carrier
  (a non-log call); run the pass; assert the Body is unchanged.

### Integration tests (`crates/regseq-driver/tests/bridge_diag_log_peel.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage:

- **Per the brief Test Plan** -- a minimal fixture using
  `regseq::log::error!("x = {}, y = {:x}", x, y)` -- assert (a) the
  post-bridge Call has 3 arguments (template + x + y) per the
  post-peel `I-DiagLog-CallArity` shape; (b) **no LogArg carrier
  survives** in the materialised Body; (c) the parsed format template
  records 2 specifiers (Display, Hex).
- All-four-severities fixture -- one function calling each of
  `regseq::log::{debug,info,warn,error}!` -- assert all four Calls
  satisfy the post-peel shape.
- Hex-formatted fixture -- `regseq::log::info!("addr = {:#010x}", a)`
  -- assert the Hex specifier carries the parsed fill / width /
  alternate fields.

Negative coverage (per spec.md §26.8 validation rules; one fixture
per diagnostic D1-D8):

- **D1 -- specifier / arg count mismatch** --
  `regseq::log::warn!("x = {}", x, y)` (extra arg) and
  `regseq::log::warn!("x = {} y = {}", x)` (missing arg) -- assert
  `BridgeError::DiagLogValidation { diagnostic: D1, .. }`.
- **D2 -- modifier / type compatibility** --
  `regseq::log::info!("addr = {:#010x}", "string-not-int")` (Hex
  modifier on a non-integer arg) -- assert
  `BridgeError::DiagLogValidation { diagnostic: D2, .. }`. Cover the
  full §26.8 modifier-vs-type compatibility table (e.g. `{:b}` /
  `{:o}` / `{:x}` / `{:X}` only on integer types; alternate-form
  `{:#}` only on numeric types; precision `{:.N}` only where defined).
- **D3 -- Debug formatter rejected** --
  `regseq::log::error!("x = {:?}", x)` -- assert
  `BridgeError::DiagLogFormat { reason: DebugFormatterRejected, .. }`
  per D3 at spec.md:12840.
- **D4 -- inadmissible LogArg operand types** -- a fixture passing
  an inadmissible operand type to `regseq::log::info!` (e.g. a `dyn
  Trait` value or a non-Copy non-LogArg-impl type) -- assert
  `BridgeError::DiagLogValidation { diagnostic: D4, .. }`.
- **D6 -- non-literal format operand** -- a fixture where the format
  string is a `let s: &str = "..."; regseq::log::info!(s, x)` (the
  format must be a string literal, not a variable) -- assert
  `BridgeError::DiagLogValidation { diagnostic: D6, .. }`.
- **D7 -- trailing newline** -- a fixture using
  `regseq::log::info!("hello\n", x)` (the format must NOT carry a
  trailing newline; the backend appends per-target line termination)
  -- assert `BridgeError::DiagLogValidation { diagnostic: D7, .. }`.
- **D8 -- selected-target string-arg bound** -- a fixture passing a
  string argument that exceeds the selected target's string-arg
  bound (per §26.8 D8) -- assert `BridgeError::DiagLogValidation {
  diagnostic: D8, .. }`.
- **Positional arg subset rejection** -- `regseq::log::info!("{0}", x)`
  -- assert rejection (positional args outside the closed subset per
  §26.7 / D3-related parsing rule).

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-45 against its task spec at
.harness/tasks/regseq-driver/45-diag-log-peel-and-format-grammar.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/diag_log/mod.rs
- crates/regseq-driver/src/bridge/diag_log/peel.rs
- crates/regseq-driver/src/bridge/diag_log/format.rs
- crates/regseq-driver/src/bridge/diag_log/validation.rs
- crates/regseq-driver/src/bridge/body/materialise.rs (Phase-5 pass
  ordering update)
- crates/regseq-driver/tests/bridge_diag_log_peel.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §25.7.3 / §26.6 / §26.7 / §26.8 /
  §23.6.2 B-4 / B-6.
- Format-grammar closed-subset enforcement (Display + Hex + Formatted
  Hex; Debug parses-and-rejects per D3 at spec.md:12840).
- Post-peel I-DiagLog-CallArity shape (no LogArg survives).
- Phase-5 pass ordering: peel runs after vtable-indirect and before
  joint validation per spec.md §25.7.
- No C-emission (the §26.9 backend lowering is phase-1b).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
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

- `crates/regseq-driver/src/bridge/diag_log/{peel,format,validation}.rs`
  created and wired from `bridge/diag_log/mod.rs`.
- Phase-5 body-pass ordering updated: peel runs after vtable-indirect
  and before joint validation per spec.md §25.7.
- Format-template grammar parser admits exactly the spec.md §26.7
  closed subset (Display + Hex + Formatted Hex); Debug parses and
  rejects per D3 at spec.md:12840.
- Post-peel Calls satisfy the I-DiagLog-CallArity shape per spec.md
  §23.6.2 B-4 / B-6 -- no LogArg carrier survives.
- Validation diagnostics emit per spec.md §26.8 rules.
- No C-emission (the §26.9 backend lowering is phase-1b).
- All tests pass; Codex Task Review verdict APPROVED.
