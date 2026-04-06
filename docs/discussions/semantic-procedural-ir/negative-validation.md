# Negative Validation

This note records Step 9 of the semantic procedural IR validation checklist.

The goal is to stress the proposed design against the failure modes that have
already hurt this compiler effort, plus a small set of adversarial semantic
cases, and verify that the design fails cleanly rather than degrading silently.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [representative-function-trace-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md)
- [hir-mir-coverage-matrix.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/hir-mir-coverage-matrix.md)
- [backend-boundary-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/backend-boundary-validation.md)
- [paper-migration-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/paper-migration-validation.md)

## Negative Cases

### 1. Duplicate short method names on different owners

Failure mode we want to avoid:
- short-name matching picks the wrong function body
- backend or lowering associates a callback/helper with the wrong owner

Design response:
- `CanonicalPath`
- `FunctionKind`
- `ImplDef`
- `ItemId`

Result:
- the design handles this cleanly
- failure is prevented structurally, not heuristically

Verdict:
- passes

### 2. Overlapping associated constant names

Failure mode we want to avoid:
- `DEFAULT_SPEED` or similar constants collide by short name
- backend invents ad hoc stable names or binds the wrong constant

Design response:
- `ConstantDef.owner`
- `ConstantDef.canonical_path`
- `ConstantRef.canonical_path`
- deterministic `stable_name`

Result:
- the design handles this cleanly

Verdict:
- passes

### 3. Overlapping resource field names

Failure mode we want to avoid:
- field `regs` or `clock` is resolved by short name only
- declaration/resource linkage binds the wrong field

Design response:
- field-level `canonical_path`
- `CrossLinkTable.path_to_field`

Result:
- the design handles this cleanly

Verdict:
- passes

### 4. Value-producing helper with default path

Representative case:
- `uart_clock_hz`

Failure mode we want to avoid:
- value-producing helper is degraded into statement-only behavior
- backend must reconstruct return semantics or defaulting logic

Design response:
- `ResourcePropertyRead`
- `WithDefault`
- `Return { value: Some(...) }`

Result:
- the design handles this cleanly

Verdict:
- passes

### 5. ISR with multiple interrupt-class tests and explicit clear behavior

Representative case:
- `handle_irq`

Failure mode we want to avoid:
- backend reconstructs interrupt semantics from handwritten Linux heuristics
- interrupt-class routing or clear behavior is encoded by register-name guesswork

Design response:
- `InterruptClassTest`
- explicit helper-call sequencing
- explicit `clear_explicit_interrupts(status)` helper call
- interrupt class definitions in `SemanticModel`

Result:
- the design handles the supported slice cleanly

Remaining note:
- interrupt-class naming source is still listed as an open design question in
  the plan, but that is narrower than a structural blocker

Verdict:
- passes for the supported slice

### 6. Helper with multiple derived arithmetic temporaries

Representative case:
- `set_baud_rate`

Failure mode we want to avoid:
- MIR temps leak through as `_3`, `_14`, etc.
- backend or emitter invents declarations opportunistically

Design response:
- `SemanticLocal`
- `LocalOrigin::Derived`
- explicit `Let` / `Assign`
- strict normalization of raw MIR locals away

Result:
- the design handles this cleanly

Verdict:
- passes

### 7. Fallible helper with cleanup path

Representative cases:
- `enable_clock_if_present`
- `toggle_reset_if_present`

Failure mode we want to avoid:
- `Try::branch` and residual conversion leak to backend
- cleanup/error path is lost or informally reconstructed

Design response:
- `Stmt::FallibleOp`
- explicit `error_local`
- explicit `cleanup_on_error`

Result:
- the design handles the control-flow shape cleanly

Remaining note:
- richer error-carrier modeling is still deferred, but the structural control
  flow is explicit

Verdict:
- passes for the current slice

### 8. Unsupported MIR construct tries to survive lowering

Failure mode we want to avoid:
- unresolved MIR construct reaches the backend as:
  - TODO comment
  - debug string
  - zero default
  - raw Rust path

Design response:
- explicit hard-error rule in MIR normalization
- explicit prohibition on:
  - silent drops
  - TODO comments
  - debug-format interchange
  - zero-default semantic corruption

Result:
- the design handles this correctly in principle

Verdict:
- passes, assuming implementation follows the stated rule strictly

### 9. Unsupported type escapes into backend consumption

Failure mode we want to avoid:
- backend receives an unknown type and is forced to guess C emission

Design response:
- `SemanticType::Opaque`
- explicit rule that `Opaque` is diagnostic-only and must not reach backend
  emission for supported code

Result:
- the design handles this correctly in principle

Verdict:
- passes, assuming semantic lowering enforces the no-opaque-at-boundary rule

### 10. Option-carrier semantics collapse into implicit truthiness

Representative cases:
- `service_rx_fifo`
- `service_tx_fifo`

Failure mode we previously observed in the design:
- option-typed values used as if they were booleans
- `None` modeled as `false`

Design response:
- `IsSome`
- `OptionNone`
- `OptionSome`
- `OptionUnwrap`

Result:
- this failure mode is now repaired in the design

Verdict:
- passes after plan update

## Findings

### The design now fails in the right direction

The most important result of the negative pass is:

- when the design cannot represent something safely, it is specified to fail
  explicitly in semantic lowering

That is the correct compiler behavior. The negative cases do **not** reveal a
pressure to reintroduce:

- backend heuristics
- short-name matching
- TODO comments
- debug-string interchange
- implicit truthiness

### Main implementation discipline requirements

The negative pass confirms that the remaining risks are mostly implementation
discipline risks, not architecture gaps:

- semantic lowering must enforce hard errors
- `Opaque` must not leak to backend consumption
- `OptionUnwrap` must only be introduced where the control-flow invariant
  really guarantees presence

These are important, but they do not require another redesign round.

## Step 9 Verdict

Step 9 is **completed**.

Outcome:

- the design holds up under the key negative cases we care about
- previously observed failure modes now have explicit semantic answers or
  explicit hard-error behavior
- no new architectural blocker was discovered
