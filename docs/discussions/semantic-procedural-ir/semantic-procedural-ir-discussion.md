# Semantic Procedural IR Discussion

## Purpose

This note captures the architectural gap that has become visible in the
`pl011-full-spec` bring-up work:

- we now capture authored executable bodies from rustc MIR
- we have a partial portable behavior model
- but the backend-facing semantic boundary is still too weak

As a result, MIR- and Rust-shaped residue is still leaking into Linux emission.
The latest canonical wrapper-driven output proved that this is not just a Linux
string-rendering bug. It is a compiler architecture issue.

The purpose of this note is to gather the current issues, working assumptions,
and design questions before we formalize the next task/spec changes.

## Current Position

The current intended pipeline is:

1. rustc MIR capture in `regseq-driver`
2. rustc-independent MIR model in `regseq-model`
3. supported-subset validation in `regseq-analysis`
4. portable lowering in `regseq-backends/src/lowering`
5. OS-specific backend planning and emission

In repo terms, the current raw executable-body source is:

- [`mir_drops_elaborated_and_const_checked`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/body.rs)

The current portable body/behavior layer is primarily:

- [`behavior.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/behavior.rs)
- [`plan.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/plan.rs)

The current Linux-specific code-emission layer is primarily:

- [`emit_behavior.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_behavior.rs)
- [`emit_module.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_module.rs)
- [`c_ast.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/c_ast.rs)

## Main Architectural Diagnosis

We do have a portable procedural model today, but it is only a **partial**
semantic IR.

That is why the Linux backend has still been exposed to:

- raw MIR local structure
- wrapper-call residue
- incomplete value semantics
- incomplete return semantics
- incomplete helper-body realization

The problem is therefore not:

- "Linux emission needs better string cleanup"

The problem is:

- the backend-facing semantic IR is not yet strong enough to fully decouple MIR
  and Rust constructs from backend code generators

## What The Backend Should Not Be Seeing

The OS backends should not have to reason about:

- raw MIR locals such as `_3`, `_14`, `_25`
- raw Rust paths such as `regseq::ReadWrite::<T, R>::modify`
- raw `core::option::Option::...` or `core::ops::...` call surfaces
- rustc-specific temporary structure
- wrapper-specific generic syntax
- Rust method dispatch spelling
- raw MIR return-place conventions

The backend may still need to emit target-language locals and expressions, but
those should be **portable semantic locals/values**, not raw MIR spellings.

## What The Backend Should Be Seeing

The backend should consume a Regseq-owned semantic procedural IR with concepts
like:

- helper call
- register write / modify / field read / raw read
- resource enable / disable / apply / read
- local binding
- assignment
- branch
- loop
- fallible operation
- cleanup region
- explicit ordered region
- return with semantic value

and a Regseq-owned expression/value vocabulary such as:

- literal
- local ref
- param ref
- field ref
- constant ref
- cast
- unary / binary op
- resource read
- enum/variant or aggregate values where they survive semantically

This IR should be backend-facing and OS-neutral.

## Concrete Failure Evidence That Exposed The Gap

The reopened `regseq-linux-backend-02` work surfaced fresh canonical output
under:

- [`tests/fixtures/pl011-full-spec/regseq-out/regseq_pl011_full_spec_module.c`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/regseq-out/regseq_pl011_full_spec_module.c)

That fresh wrapper-driven output no longer leaked raw Rust path spellings, but
it still failed as a backend artifact because the semantic IR boundary was still
too weak.

Observed defects included:

- undeclared temporaries such as `tmp3`, `tmp6`, `tmp11`, `tmp14`, `tmp24`,
  `tmp30`
- undeclared `ret`
- malformed duplicated return structures such as `if (ret) { return 0; return ret; }`
- `uart_clock_hz` emitted as `static u32 ... { return; }`
- helper bodies still referencing unresolved temporary values instead of valid
  lowered expressions
- malformed ISR body text, including incomplete statements and invalid
  `irqreturn_t` returns

These are not cosmetic issues. They are evidence that:

- MIR semantics are not yet fully elaborated into a backend-safe semantic IR

## Procedural IR Versus C AST

There are two distinct layers:

1. portable procedural IR
   - currently represented partially by `BehaviorPlan` and related lowering
     types
   - this is **not** a C AST

2. Linux C AST / C source representation
   - currently represented by `linux/c_ast.rs`
   - this is Linux-specific

The current failure mode is that the portable procedural IR is too weak, so the
Linux layer is being asked to cope with residues that should already have been
resolved before any C AST emission begins.

## Wrapper Calls And Intrinsics

### Current Reality

For register access, the compiler is currently recognizing wrapper surfaces such
as:

- `ReadWrite::read_raw`
- `ReadWrite::read`
- `ReadWrite::write`
- `ReadWrite::modify`
- `ReadWrite::set`

Recognition is largely path- and method-name-based today.

The relevant current code paths include:

- [`register_access.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/register_access.rs)
- [`call_classifier.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs)

### Important Clarification

`ReadWrite::modify` is **not** currently implemented as a thin wrapper around a
canonical `regseq::intrinsics::register_access::...` function.

The facade in:

- [`registers.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/registers.rs)

currently defines these methods as compiler-lowered facade stubs, for example:

- `unimplemented!("facade stub: lowered by the compiler backend")`

So the current compiler has to treat wrapper methods as a recognized semantic
surface in their own right.

### Working Position

For now, wrapper-based extraction is acceptable if:

- wrapper methods are treated as a stable compiler-known semantic surface
- all wrapper-specific Rust details are normalized away before backend emission

Future intrinsic redesign is possible, but it is not a blocker for the current
IR-elaboration problem.

## Inlining And MIR Phase Boundary

The compiler currently captures executable bodies from:

- `mir_drops_elaborated_and_const_checked`

That query is intentionally used instead of `optimized_mir`.

Important consequence:

- we should **not** expect wrapper calls to have been inlined away by rustc

The rustc pipeline confirms:

- `Inline` runs in the optimization pipeline used by `optimized_mir`
- it does **not** run as part of the `mir_drops_elaborated_and_const_checked`
  query we extract from

Relevant source:

- [`rustc_mir_transform/src/lib.rs`](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_mir_transform/src/lib.rs)

Working conclusion:

- do not rely on rustc inlining to expose backend-meaningful operations
- do not force wrapper inlining as a semantic strategy
- instead, normalize wrapper calls into Regseq semantic operations in the
  portable lowering/model layer

## MIR Temporaries And Why They Matter

Raw MIR locals such as `_3`, `_13`, `_14` are not cosmetic names.

They carry real semantics:

- evaluation order
- data dependencies
- intermediate values
- references and wrapper handles
- desugared control flow
- helper argument preparation

That means:

- simply stripping or renaming them textually in emitted C is not a valid fix

What must happen instead:

- the semantic IR must define a proper local/value environment
- authored names, parameter names, return place, and compiler temps must be
  normalized into stable Regseq-owned identities
- target backends can then map those portable locals into C locals, temporaries,
  or inline expressions as appropriate

## What The Semantic Procedural IR Must Cover

At a minimum, the backend-facing semantic IR should:

1. capture all relevant Regseq facts
   - registers
   - resources
   - callbacks/helpers/ISR roles
   - properties
   - synchronization
   - cleanup/fallibility
   - provenance

2. represent statements and expressions in a Regseq-owned form
   - explicit procedural structure
   - no raw Rust syntax
   - no raw MIR syntax

3. normalize naming and local identity
   - params
   - return value
   - authored locals
   - generated temps
   - stable backend-safe naming

4. classify calls semantically
   - helper call
   - register access
   - resource operation
   - synchronization operation
   - framework/value constructor
   - structural control-flow glue
   - unsupported contract gap

5. carry a Regseq-owned type model
   - scalar types
   - structs
   - enums
   - arrays
   - references/pointers if needed
   - other backend-relevant value/type categories

6. define deterministic naming for generic constructs
   - functions
   - structs
   - enums
   - variants
   - helper symbols
   - generated locals

## Current Open Questions

The following questions remain explicitly open and should be resolved as part of
the semantic IR design work:

1. Should the existing `BehaviorPlan` family be extended until it fully serves
   as the semantic procedural IR, or should a new IR layer be introduced?

2. What value model is needed so helper bodies like `uart_clock_hz` can be
   represented faithfully before any OS backend sees them?

3. How should local/value environments be represented so that:
   - authored locals stay readable
   - generated temps remain deterministic
   - return-place semantics remain explicit
   - cleanup/fallibility semantics remain preserved

4. How rich does the portable type model need to be for backend work?
   In particular:
   - structs
   - enums
   - arrays
   - references
   - option-like / result-like carriers

5. Which wrapper calls and framework calls should survive as semantic
   operations, and which should be normalized away before backend planning?

6. How should generic constructs be named deterministically in a backend-safe
   way without leaking Rust path spelling into emitted target code?

7. Where should explicit producer-side failure boundaries live if the compiler
   cannot elaborate a MIR/body construct into semantic IR?

## Working Direction

The current working direction is:

- do not keep teaching Linux to understand more MIR/Rust residue
- strengthen the backend-facing semantic procedural IR
- make that IR the only executable-body input OS backends consume

That implies:

- raw MIR remains the authoritative extraction source
- semantic procedural IR becomes the backend-facing body source
- Linux/Zephyr/etc. should lower from semantic procedural IR, not from MIR

## Immediate Implication For Current Tasking

The latest `regseq-linux-backend-02` rejection is not just a Linux backend bug.
It is evidence that semantic procedural IR work is now a first-class compiler
concern.

That means upcoming task/spec work should likely:

- formalize the semantic procedural IR boundary explicitly
- decide whether to extend or replace the current partial `BehaviorPlan` layer
- make backend tasks depend on that boundary being strong enough

## Non-Goals For This Discussion

This note does not yet settle:

- the final IR type names
- the exact crate/module ownership of the new or strengthened IR
- the full migration sequence for all current backend consumers
- future intrinsic redesign beyond the current wrapper-based surface

Those should follow from the design discussion that this note is intended to
enable.
