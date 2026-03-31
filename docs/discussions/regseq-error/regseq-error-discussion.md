# RegseqError Discussion

## Purpose

This note is a design discussion document for `RegseqError`.

It does not try to finalize the API. Its job is to make the design pressure
explicit before the Phase B facade and compiler-capture tasks harden the wrong
surface.

## 1. Why RegseqError Matters

`RegseqError` is the shared authored-error surface for standard fallible Regseq
operations.

It matters because it sits at the seam between:

- the Layer 1 DSL surface seen by spec authors
- the compiler's extraction and analysis pipeline
- target-specific lowering to Linux, Zephyr, and later OS backends

If this type is modeled incorrectly, several other surfaces drift at once:

- callback return conventions
- wrapper-method signatures
- intrinsic return shapes
- backend error lowering
- diagnostics and validation expectations

### What construct backs it up?

`RegseqError` is not best understood as a standalone construct.

It is primarily backed by the shared callback-and-error baseline used across
multiple constructs. In other words, it is a cross-construct convention rather
than one construct's private type.

The main construct families that currently rely on `RegseqError` are:

- `Driver Lifecycle`
- `Device power state management`
- `Serial/UART Management`
- `Peripheral clock management`
- `Hardware reset sequencing and post-reset polling`
- `Pin controller and GPIO configuration`
- `Synchronization`
- `Register Access` polling
- `DMA Management`

These are the places where the stable surface already uses signatures like:

```rust
Result<(), RegseqError>
```

### What does not use RegseqError?

This distinction is important because it tells us what `RegseqError` is not.

- bound-handler interrupt results use `IrqResult`, not `RegseqError`
- logging is separate and infallible
- atomic compare-exchange uses `Result<T, T>`, not `RegseqError`
- firmware property access is primarily declarative and compiler-managed, not a
  driver-authored `Result<_, RegseqError>` API

So `RegseqError` is not the universal result type for every Regseq operation.
It is the shared authored-error surface for standard fallible driver-facing
operations.

## 2. The Core Problem in Capturing It

The core difficulty is that `RegseqError` has to be meaningful in the DSL
without becoming a fake Rust runtime error framework.

That leads to four design pressures.

### A. Cross-OS lowering pressure

The same authored Regseq error has to lower into different OS-native mechanisms:

- Linux negative errno-style returns
- Zephyr errno-style returns with target-specific symbolic constants
- later target-specific mechanisms on other OSes

So `RegseqError` cannot simply be "the Linux error type" or "the Zephyr error
type" in disguise.

It has to be a portable authored-error vocabulary that the backend can map to
target-native representations.

### B. No-runtime pressure

Regseq specs are not ordinary Rust programs that execute as Rust artifacts.

They are source-level inputs for the compiler. Because of that:

- the stable surface should avoid depending on Rust runtime error ecosystems
- the surface should not assume allocation, trait-object error plumbing, or
  ordinary host-program formatting behavior
- the type should remain meaningful even when the final backend output is C or
  another non-Rust realization

This makes `std::error::Error` a suspicious dependency for the public DSL
surface.

### C. Authored-error versus backend-error pressure

There are two different sources of failure in Regseq-generated code:

1. the driver author explicitly expresses an error
2. the generated backend code calls an OS API that returns a native error

These are not the same thing.

If we force both through the exact same `RegseqError` path, we lose useful
information and make the lowering contract more artificial.

### D. Surface-signal pressure

The compiler needs a clean authored signal for:

- extraction
- analysis
- model synthesis
- backend lowering

If `RegseqError` carries too much Rust-library baggage, the DSL signal gets
weaker. If it carries too little semantic structure, the lowering contract gets
underspecified.

So the problem is not just "what enum variants should exist?" The deeper problem
is: what semantic role should the type play, and what should stay outside it?

## 3. Good Things To Preserve

The older error-model work contained several good ideas that are still worth
carrying forward.

### A. Treat RegseqError as a DSL-level semantic abstraction

This is the most important point.

`RegseqError` should represent driver-authored failure intent in the DSL. It
should not be modeled as a direct runtime wrapper around OS-native error codes.

That gives the compiler freedom to lower the same authored failure into:

- `-EINVAL` or `-ENOMEM` style Linux results
- Zephyr symbolic errno values
- later backend-native representations

### B. Keep authored errors separate from OS-native passthrough

A strong model is:

- when the driver author explicitly returns an authored Regseq error, the
  backend maps that semantic error to the target-native representation
- when generated backend code calls an OS API and that API returns a native
  error, the backend may propagate the native result directly rather than
  round-tripping it through `RegseqError`

This separation is useful because it preserves real backend semantics such as:

- Linux probe deferral
- target-specific resource acquisition failures
- target-native distinctions the authored DSL did not spell directly

### C. Keep logging separate

Logging should not be fused into the error model.

The stable logging surface already exists independently. That is good.

The compiler may choose to emit backend-specific logging around some failure
paths, but that should remain a lowering concern or an explicit logging
construct concern, not a defining part of `RegseqError` itself.

### D. Keep the public surface small

Even if the eventual semantic taxonomy grows, the initial stable surface should
stay small and purposeful.

The type should communicate:

- that authored failure happened
- what portable category it belongs to

It should not try to encode every target-specific edge case or every backend
framework status in the public DSL.

### E. Use target-native symbolic constants in lowering

When backends lower authored Regseq errors, they should lower to target-native
symbolic forms rather than hard-coded numeric literals.

That matters because some superficially similar targets still assign different
numeric values to their symbolic error constants.

### F. Keep the type compiler-friendly

The stable authored-error surface should be easy for the compiler to recognize
and reason about.

That argues for:

- a direct Regseq surface type
- stable authored construction sites
- avoidance of elaborate runtime trait dependencies just to fit Rust library
  conventions

## 4. Things To Avoid

### A. Do not make RegseqError depend on `std::error::Error`

That would make the public DSL surface look like a conventional Rust runtime
error API, which is not the right abstraction for Regseq.

The same caution applies to designing the type primarily around ordinary Rust
host-program ergonomics.

### B. Do not make `Display` or formatting a defining requirement

The logging construct already separates formatted diagnostics from error
propagation.

If `RegseqError` is forced to support formatting-oriented traits as part of its
core meaning, the design will drift toward host-Rust conventions that are not
required by the DSL.

This does not mean implementations can never derive or provide conveniences. It
means those traits should not define the semantic role of the type.

### C. Do not round-trip all backend-native errors through RegseqError

If generated code calls a target OS API and that API returns a native error, it
is often better to propagate that native result than to convert it into a
`RegseqError` and back again.

Forcing all failures through `RegseqError` would:

- lose target-native distinctions
- complicate lowering
- blur the boundary between authored failure intent and backend-native failure

### D. Do not collapse handler dispatch results into RegseqError

`IrqResult` is not a poor substitute for `RegseqError`; it is a different kind
of result.

Interrupt dispatch and authored callback failure must remain distinct.

### E. Do not fuse logging into the error type

The error surface should not imply automatic formatting, message generation, or
mandatory logging side effects.

Those are separate concerns.

### F. Do not overfit the first version to one OS

The initial implementation stream targets Linux and Zephyr first, but the
surface should still be defined semantically rather than as "Linux errno with a
different name."

### G. Do not reuse the same name for multiple error abstractions

If we later decide we need:

- a core authored-error enum
- a trait for custom authored errors
- backend mapping metadata

those should not all share the exact same type name. That would create needless
surface confusion.

### H. Do not finalize the category set casually

A portable authored-error vocabulary is necessary, but freezing an exact set of
semantic categories too early would be risky.

The important decision now is the model:

- authored semantic error abstraction
- separate from OS-native passthrough
- separate from logging
- separate from handler dispatch results

The final category inventory should be closed deliberately, not accidentally by
the first facade task.

## 5. How Error Management Connects To Other Constructs

There is no dedicated "error handling construct" in the current PL011 compiler
collateral. Instead, error management is spread across several construct roles.

That means we need an explicit tying model.

### A. Trait callback constructs

These are the clearest consumers of `RegseqError`.

Examples:

- `Lifecycle::init` / `deinit`
- `PowerManagement::suspend` / `resume`
- `Serial::port_open` / `configure`

For these constructs, `RegseqError` is the stable authored callback failure
surface.

### B. Typed-resource wrapper constructs

Wrapper families also consume `RegseqError` where the authored operation is
fallible.

Examples:

- `Clock::enable`
- `ResetLine::assert`
- `PinCtrl::apply`
- `Synchronization::wait`
- `poll::until`
- several DMA operations

For these constructs, `RegseqError` acts as the stable authored failure surface
for resource-local operations.

### C. Bound-handler constructs

Interrupts, timers, and deferred work should not be forced into the same error
surface model.

Their primary surface is handler dispatch, not standard authored callback
failure. This is why interrupt handling uses `IrqResult`.

So the rule should remain:

- trait callbacks and fallible wrapper operations may use `RegseqError`
- bound-handler dispatch uses handler-specific result models

### D. Declarative/compiler-managed constructs

Constructs like firmware property access mostly surface errors as:

- extraction-time validation failures
- compiler-generated init-preamble failure behavior

They are related to error management, but they do not automatically imply a
driver-authored `RegseqError` API.

This distinction matters because it keeps declarative constructs from inventing
fake driver-authored error surfaces.

### E. Diagnostic logging

Logging is adjacent to error handling, but separate from it.

The right relationship is:

- authored code may log and then return an error
- generated code may log and then propagate a native backend error
- but logging remains its own construct, not part of the meaning of
  `RegseqError`

### F. Backend lowering

The backend is where all of this must come together:

- authored `RegseqError` values map to target-native symbolic error forms
- backend-native OS API failures may propagate directly
- target-specific special cases such as probe deferral remain backend-aware
  concerns

So the backend contract must distinguish:

1. authored semantic failure
2. native API passthrough failure
3. dispatch-style results such as `IrqResult`
4. compiler validation failures that never become driver-authored results

## 6. Decision Pressure To Resolve Before Updating Specs

Before updating executable task specs or facade implementation guidance, we
should close these questions explicitly.

### A. What is the stable public shape of RegseqError?

Questions:

- enum only?
- enum plus mapping trait?
- enum plus helper constructors?
- do we permit custom authored error types in the initial baseline?

### B. What is the first stable authored category set?

Questions:

- what minimum portable semantic categories do we actually need now?
- which categories are definitely backend-only or framework-only and therefore
  should stay out?

### C. What trait obligations, if any, belong on the public type?

Questions:

- should it implement only `Debug`-like convenience traits?
- should even `Debug` be considered optional rather than semantically required?
- should the stable design avoid `Display` and `std::error::Error` entirely?

### D. What does the compiler extract as the authored error signal?

Questions:

- direct enum variants?
- explicit constructors?
- `.into()` from custom authored error types?

### E. What is the exact authored-error versus native-passthrough boundary?

Questions:

- which generated operations may return native OS errors directly?
- which authored surfaces must always lower through `RegseqError` mapping?

## 7. Working Position

The most defensible working position right now is:

- `RegseqError` should be a small DSL-level semantic error abstraction
- it should back standard fallible callback and wrapper surfaces
- it should not be defined by `std::error::Error`, `Display`, or Rust runtime
  error conventions
- it should stay separate from logging
- it should stay separate from interrupt dispatch results
- the compiler/backend should preserve a distinction between authored semantic
  errors and native OS error passthrough

That is enough to guide discussion and reject obviously wrong early designs,
without yet freezing the exact final enum shape.

