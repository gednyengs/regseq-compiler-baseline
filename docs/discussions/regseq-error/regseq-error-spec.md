# RegseqError Specification

## Purpose

This document records the current specification for `RegseqError` as the
 active design baseline for ongoing compiler and DSL work.

It is intended to be narrower than the surrounding discussion notes. Where the
 discussion notes explore alternatives, this document states the current
 contract and the current deferrals.

## Scope

This specification covers:

- what `RegseqError` is
- where semantic `RegseqError` is allowed to flow in authored DSL code
- where semantic `RegseqError` must be lowered
- what the compiler may assume for current Phase B work
- what is explicitly deferred

This specification does not finalize:

- the complete long-term intrinsic-local error strategy
- the final `RegseqError` variant taxonomy
- backend-specific lowering details for every target family

## 1. Core Definition

`RegseqError` is the shared compiler-facing semantic error space for authored
 fallible Regseq code.

It is the common error space that:

- fallible Regseq trait callback exits converge on
- the compiler analyzes as authored semantic failure
- backends lower into target-specific callback result mechanisms

`RegseqError` is not:

- a direct wrapper around OS-native error/status families
- a runtime host-language error framework
- a logging or diagnostic type

## 2. Host-Language Requirements

The current specification is:

- `RegseqError` must not depend on `std::error::Error`
- `RegseqError` must not require `Display` as part of its core meaning
- logging remains separate from `RegseqError`

This keeps the type aligned with DSL/compiler semantics rather than host-Rust
 runtime conventions.

## 3. Semantic Flow Inside Authored DSL

Inside authored DSL code, `RegseqError` may remain abstract semantic flow.

That means:

- helper functions may create `RegseqError`
- helper functions may propagate it with `?`
- helper functions may match on it
- helper functions may reclassify it

None of those actions force backend-specific error materialization on their own.

## 4. Outermost Authored Fallible Boundaries

Semantic `RegseqError` must be lowered only when it escapes authored DSL code
 through an outermost authored fallible boundary.

In the current design, those boundaries are the exits of fallible Regseq trait
 callback methods.

Current examples:

- `Lifecycle::init`
- `Lifecycle::deinit`
- `PowerManagement::suspend`
- `PowerManagement::resume`
- `Serial::port_open`
- `Serial::configure`

These are special because:

- they are backend-facing callback boundaries
- once semantic error escapes there, there is no authored DSL caller left to
  handle it
- the next consumer is compiler-generated or backend-generated glue

So callback exit is where semantic `RegseqError` stops being purely abstract and
 becomes target-specific lowering input.

## 5. Trait Bodies Versus Trait Exits

Trait callback bodies are still ordinary authored semantic-flow zones.

This means:

- inside the body, authored code may use helpers, `?`, and local handling as
  usual
- the special status of the trait method is at its exit boundary, not in every
  statement inside the body

This specification is consistent with the current trait-role direction:

- Regseq trait methods are callback/glue contracts
- they are not ordinary reusable authored APIs
- direct authored invocation of Regseq trait methods is expected to become a
  separate frontend usage rule

That rule is related, but not part of this specification directly.

## 6. Backend-Native Operational Failure

From the perspective of authored DSL modeling, backend-native operational
 failure should enter through fallible intrinsics or intrinsic-backed wrapper
 operations, not through Regseq trait callbacks.

This means:

- traits return semantic authored failure
- intrinsics may encapsulate operations that lower to target-specific
  mechanisms, including real failures or target-specific no-op realizations

The compiler may still have backend-native failures in compiler-generated
 preamble/postamble/resource-management code, but those are outside the authored
 semantic `RegseqError` surface.

Those compiler-generated failure paths use target-native status/errno
 mechanisms directly rather than flowing through authored semantic
 `RegseqError`.

> **Addendum (2026-06-01, design debate — guardrail G01).** A Claude/Codex
> debate (Axis A: "portable semantic enum vs native passthrough") converged
> on a sharper rule than the prose above. The authored/native boundary is a
> *provenance* property, not a syntactic one: a fallible **wrapper** can
> *look* authored while fronting a native operation. Therefore — **native-backed
> operational failures in wrappers or intrinsics must default to native/opaque
> propagation, or to construct-local error types that explicitly map upward,
> and must not be collapsed directly into `RegseqError` unless the failure is
> an authored semantic condition.** `RegseqError`'s portability value is a
> common authored *vocabulary* + a uniform boundary *contract* + common
> *diagnostics* — not identical runtime behavior across OSes. This does not
> require resolving the §9 local-error / wrapper-provenance mechanism before
> *any* `RegseqError` ships (the entry-only generic-C phase is fine), but it
> **must** be resolved before native-backed wrappers/intrinsics may expose
> `Result<_, RegseqError>` as their default authored surface. Tracked in
> `.claude/specs/regseq-error/` (G01) and `generic-c-backend/roadmap.md`
> (Phase 3+).

## 7. Current Phase-B Error Strategy

To optimize for progress, the active implementation baseline is:

- use `RegseqError` as the only authored semantic error type for now
- do not block ongoing DSL/compiler work on custom local error enums

This means current Phase B work may continue assuming surfaces like:

```rust
Result<(), RegseqError>
```

for authored fallible callback and intrinsic-facing APIs where needed by the
 accepted surface.

This is an **interim implementation baseline**, not a statement that every
 long-term intrinsic surface must permanently use `Result<_, RegseqError>`.
 Intrinsic-local error enums remain deferred future work.

## 8. Phase-B Implementation Shape

For current Phase B implementation work, the minimum concrete form is:

- the authored carrier is `core::result::Result<T, RegseqError>`
- no custom `RegseqResult<T>` carrier is required for the current baseline
- `RegseqError` should be defined in the `regseq` facade crate
- `RegseqError` should be marked `#[non_exhaustive]`
- `RegseqError` should derive:
  - `Debug`
  - `Clone`
  - `Copy`
  - `PartialEq`
  - `Eq`
- `RegseqError` must not implement `std::error::Error`
- `RegseqError` must not depend on `Display` as part of the active baseline

Until the taxonomy is explicitly resolved, Phase B implementation should use
 the provisional seed taxonomy recorded in:

- [current-synthesis.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/regseq-error/current-synthesis.md)

That working variant set remains provisional and may be refined later.

The initial working enum shape should be:

```rust
#[non_exhaustive]
pub enum RegseqError {
    InvalidParameter,
    OutOfMemory,
    HardwareError,
    Timeout,
    Unsupported,
    DeviceBusy,
    PermissionDenied,
    DeviceNotFound,
    GenericFailure,
}
```

## 9. Deferred Future Work: Intrinsic-Local Error Types

The design direction of intrinsic- or construct-local error enums remains
 useful, but it is explicitly deferred.

Examples of the deferred direction:

- `ClockError`
- `ResetError`
- `PollError`
- `CompletionError`
- `DmaMapError`

and conversions such as:

```rust
impl From<ClockError> for RegseqError { /* ... */ }
```

This direction is **not rejected**. It is simply **not part of the active
 specification** required to move forward now.

Reason for deferral:

- the DSL can work with `RegseqError` first
- local error families are a refinement, not a prerequisite
- delaying them reduces immediate design and task-authoring complexity

When revisited later, that work should address:

- which intrinsic families get local error enums
- local variant sets
- upward mappings into `RegseqError`
- any helper macro support for boilerplate reduction

## 10. Deferred Future Work: Helper Macro For Local Error Mapping

A helper macro or derive-based mechanism for local error mapping is also
 explicitly deferred.

Example future direction:

```rust
#[derive(RegseqErrorMap)]
pub enum ClockError {
    Busy,
    #[regseq_error(to = RegseqError::DeviceNotFound)]
    MissingClock,
}
```

This is future ergonomic work, not part of the current required baseline.

## 11. Provisional Taxonomy Status

The final `RegseqError` variant taxonomy is not fully frozen by this
 specification.

The project currently has a provisional seed taxonomy under discussion, but it
 remains provisional until explicitly resolved.

What is fixed now is not the final row set, but the role of `RegseqError`:

- compiler-facing common semantic error space
- used by fallible Regseq trait callback exits
- allowed to flow abstractly through authored helper logic until callback exit

### Note on target-dependent callback-exit lowering

The authored semantic model is shared, but callback-exit lowering is
 structurally target-dependent.

Examples:

- some targets preserve semantic distinctions in the return value through
  errno- or status-like families
- some targets collapse multiple semantic variants into a simpler success/fail
  channel

On targets with collapsed return channels, the compiler must still preserve
 per-site semantic variant information for diagnostics or related lowering
 decisions even if the return value itself discards that distinction.

## 12. Practical Compiler Guidance

Until the deferred refinements are revisited, compiler work should assume:

1. `RegseqError` is the common semantic error type.
2. Helper functions are not lowering boundaries.
3. Fallible Regseq trait callback exits are lowering boundaries.
4. Backend-generated native failures remain separate from authored semantic
   `RegseqError` flow.
5. No current work should require `std::error::Error`, `Display`, or logging
   semantics from `RegseqError`.

## 13. Summary

The current specification is intentionally simple:

- `RegseqError` is the shared semantic error space
- it stays abstract inside authored DSL flow
- it lowers when it escapes a fallible Regseq trait callback
- custom local error enums are deferred future work

That is the active baseline to use before the project revisits the more refined
 local-error strategy.
