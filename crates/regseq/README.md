# regseq

Public facade crate for driver authors.

## Responsibilities

- Re-export procedural macros from `regseq-macros` (e.g., `register_bitfields!`)
- Surface the user-facing Layer 1 API alongside the compiler-recognized
  `#[rseq::...]` tool attributes
- Host runtime marker traits that macros expand against (e.g., `Lifecycle`, `Serial`)
- Provide DSL types used in driver specs: `ReadWrite<T,R>`, `ReadOnly<T,R>`,
  `WriteOnly<T,R>`, `Reserved<T>`, `Mmio<T>`, `Interrupt`, `Clock`, `ResetLine`,
  `PinCtrl`, `SpinLock<T>`, `IrqSpinLock<T>`, `Mutex<T>`, `Timer`, `Completion`,
  `Work`, `IrqResult`, `RegseqError`, `Duration`
- Provide intrinsic function stubs (`regseq::delay::busy_wait`, `regseq::sync::memory_barrier`, etc.)
  with `unimplemented!()` bodies that the compiler replaces during code generation

## Usage

Driver spec crates depend on this crate:
```toml
[dependencies]
regseq = { path = "../regseq" }
```

The `#[rseq::...]` attribute namespace is not provided by proc macros from
this crate. It is registered by the compiler wrapper as a custom tool
namespace so the attributes remain inert and extractable during compilation.

## Dependencies

- `regseq-macros` (proc macro crate, re-exported)
- No rustc internals dependency
